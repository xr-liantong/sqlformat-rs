//! Port of `sqlparse/engine/grouping.py`: the rules that turn a flat token
//! list into the tree the reindent filter walks.
//!
//! The Python code uses closures for the middle-token rules; here one `Rule`
//! enum carries the same predicates, which keeps the scan loop shared without
//! fighting the borrow checker.

use crate::tokens::Tk;
use crate::tree::{Class, Id, Tree};

pub const MAX_GROUPING_DEPTH: usize = 100;
pub const MAX_GROUPING_TOKENS: usize = 10_000;

/// Numerical, string and name types the grouping rules accept as operands.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupError(pub String);

type R = Result<(), GroupError>;

fn depth_check(depth: usize, tokens: usize) -> R {
    if depth > MAX_GROUPING_DEPTH {
        return Err(GroupError(format!(
            "Maximum grouping depth exceeded ({MAX_GROUPING_DEPTH})."
        )));
    }
    if tokens > MAX_GROUPING_TOKENS {
        return Err(GroupError(format!(
            "Maximum number of tokens exceeded ({MAX_GROUPING_TOKENS})."
        )));
    }
    Ok(())
}

/// `_group_matching`: group tokens that have an opening and a closing token.
fn group_matching(tree: &mut Tree, node: Id, class: Class, depth: usize) -> R {
    depth_check(depth, tree.nodes[node].children.len())?;
    let (Some((open_type, open_values)), Some((close_type, close_values))) =
        (class.m_open(), class.m_close())
    else {
        return Ok(());
    };
    let mut opens: Vec<usize> = Vec::new();
    let mut offset: usize = 0;
    let snapshot = tree.nodes[node].children.clone();

    for (idx, token) in snapshot.iter().copied().enumerate() {
        let Some(tidx) = idx.checked_sub(offset) else {
            continue;
        };
        if tree.is_whitespace(token) {
            continue;
        }
        if tree.is_group(token) && !tree.is_class(token, class) {
            group_matching(tree, token, class, depth + 1)?;
            continue;
        }
        if tree.matches_any(token, open_type, open_values) {
            opens.push(tidx);
        } else if tree.matches_any(token, close_type, close_values) {
            let Some(open_idx) = opens.pop() else {
                continue;
            };
            let close_idx = tidx;
            tree.group_tokens(node, class, open_idx, close_idx, false);
            offset += close_idx - open_idx;
        }
    }
    Ok(())
}

/// The middle-token rules of `_group`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    Typecasts,
    Tzcasts,
    TypedLiteralOpen,
    TypedLiteralExtend,
    Period,
    As,
    Assignment,
    Comparison,
    Operator,
    IdentifierList,
    Arrays,
}

impl Rule {
    fn class(self) -> Class {
        match self {
            Rule::Typecasts | Rule::Tzcasts | Rule::Period | Rule::Arrays => Class::Identifier,
            Rule::TypedLiteralOpen | Rule::TypedLiteralExtend => Class::TypedLiteral,
            Rule::As => Class::Identifier,
            Rule::Assignment => Class::Assignment,
            Rule::Comparison => Class::Comparison,
            Rule::Operator => Class::Operation,
            Rule::IdentifierList => Class::IdentifierList,
        }
    }

    fn extend(self) -> bool {
        !matches!(
            self,
            Rule::TypedLiteralOpen | Rule::Comparison | Rule::Operator
        )
    }

    fn recurses(self) -> bool {
        self != Rule::Arrays
    }
}

/// `_DELIMITED_GROUPS` membership plus "is this the first or last child".
fn is_delimiter(tree: &Tree, node: Id, token: Id) -> bool {
    let Some(class) = tree.class(node) else {
        return false;
    };
    if !class.delimited() {
        return false;
    }
    let children = &tree.nodes[node].children;
    children.first() == Some(&token) || children.last() == Some(&token)
}

fn rule_match(tree: &Tree, id: Id, rule: Rule) -> bool {
    match rule {
        Rule::Typecasts => tree.matches(id, Tk::Punctuation, "::"),
        Rule::Tzcasts => tree.ttype(id) == Some(Tk::KeywordTzCast),
        Rule::TypedLiteralOpen => {
            tree.ttype(id) == Some(Tk::NameBuiltin) || tree.matches(id, Tk::Keyword, "TIMESTAMP")
        }
        Rule::TypedLiteralExtend => tree.is_class(id, Class::TypedLiteral),
        Rule::Period => {
            tree.matches(id, Tk::Punctuation, ".")
                || tree.matches(id, Tk::Operator, "->")
                || tree.matches(id, Tk::Operator, "->>")
        }
        Rule::As => tree.is_keyword(id) && tree.normalized(id) == "AS",
        Rule::Assignment => tree.matches(id, Tk::Assignment, ":="),
        Rule::Comparison => tree.ttype(id) == Some(Tk::OperatorComparison),
        Rule::Operator => tree.ttype_in(id, &[Tk::Operator, Tk::Wildcard]),
        Rule::IdentifierList => tree.matches(id, Tk::Punctuation, ","),
        Rule::Arrays => tree.is_class(id, Class::Brackets),
    }
}

/// `valid_prev`; `None` means the Python `token is None` case.
fn rule_valid_prev(tree: &Tree, prev: Option<Id>, rule: Rule) -> bool {
    let Some(prev) = prev else {
        return false;
    };
    match rule {
        Rule::Typecasts | Rule::Tzcasts | Rule::TypedLiteralOpen | Rule::TypedLiteralExtend => true,
        Rule::Period => {
            tree.is_one_of(prev, &[Class::Brackets, Class::Identifier])
                || tree.ttype_in(prev, &[Tk::Name, Tk::StringSymbol])
        }
        Rule::As => tree.normalized(prev) == "NULL" || !tree.is_keyword(prev),
        Rule::Assignment => tree.ttype(prev) != Some(Tk::Keyword),
        Rule::Comparison => {
            tree.is_one_of(
                prev,
                &[
                    Class::Parenthesis,
                    Class::Function,
                    Class::Identifier,
                    Class::Operation,
                    Class::TypedLiteral,
                ],
            ) || tree.ttype_in(
                prev,
                &[
                    Tk::NumberInteger,
                    Tk::NumberFloat,
                    Tk::NumberHex,
                    Tk::StringSingle,
                    Tk::StringSymbol,
                    Tk::Name,
                    Tk::NameBuiltin,
                    Tk::NamePlaceholder,
                ],
            ) || (tree.is_keyword(prev) && tree.normalized(prev) == "NULL")
        }
        Rule::Operator => {
            tree.is_one_of(
                prev,
                &[
                    Class::Brackets,
                    Class::Parenthesis,
                    Class::Function,
                    Class::Identifier,
                    Class::Operation,
                    Class::TypedLiteral,
                ],
            ) || tree.ttype_in(
                prev,
                &[
                    Tk::NumberInteger,
                    Tk::NumberFloat,
                    Tk::NumberHex,
                    Tk::StringSingle,
                    Tk::StringSymbol,
                    Tk::Name,
                    Tk::NameBuiltin,
                    Tk::NamePlaceholder,
                ],
            ) || tree.matches_any(
                prev,
                Tk::Keyword,
                &["CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP"],
            )
        }
        Rule::IdentifierList => {
            tree.is_one_of(
                prev,
                &[
                    Class::Function,
                    Class::Case,
                    Class::Identifier,
                    Class::Comparison,
                    Class::IdentifierList,
                    Class::Operation,
                ],
            ) || tree.matches_any(prev, Tk::Keyword, &["NULL", "ROLE"])
                || tree.ttype_in(
                    prev,
                    &[
                        Tk::NumberInteger,
                        Tk::NumberFloat,
                        Tk::NumberHex,
                        Tk::StringSingle,
                        Tk::StringSymbol,
                        Tk::Name,
                        Tk::NamePlaceholder,
                        Tk::Keyword,
                        Tk::CommentSingle,
                        Tk::CommentMultiline,
                        Tk::Wildcard,
                    ],
                )
        }
        Rule::Arrays => {
            tree.is_one_of(prev, &[Class::Brackets, Class::Identifier, Class::Function])
                || tree.ttype_in(prev, &[Tk::Name, Tk::StringSymbol])
        }
    }
}

/// `valid_next`; `None` means the Python `token is None` case.
fn rule_valid_next(tree: &Tree, next: Option<Id>, rule: Rule) -> bool {
    match rule {
        Rule::Typecasts | Rule::Period | Rule::Arrays => true,
        Rule::Tzcasts => next.is_some_and(|next| {
            tree.is_whitespace(next)
                || tree.matches(next, Tk::Keyword, "AS")
                || tree.ttype(next) == Some(Tk::StringSingle)
        }),
        Rule::TypedLiteralOpen => {
            next.is_some_and(|next| tree.ttype(next) == Some(Tk::StringSingle))
        }
        Rule::TypedLiteralExtend => next.is_some_and(|next| {
            tree.matches_any(
                next,
                Tk::Keyword,
                &["DAY", "HOUR", "MINUTE", "MONTH", "SECOND", "YEAR"],
            )
        }),
        Rule::As => next.is_some_and(|next| {
            !tree.ttype_in(next, &[Tk::KeywordDml, Tk::KeywordDdl, Tk::KeywordCte])
        }),
        Rule::Assignment => next.is_some_and(|next| tree.ttype(next) != Some(Tk::Keyword)),
        Rule::Comparison => next.is_some_and(|next| rule_valid_prev(tree, Some(next), rule)),
        Rule::Operator => next.is_some_and(|next| rule_valid_prev(tree, Some(next), rule)),
        Rule::IdentifierList => next.is_some_and(|next| rule_valid_prev(tree, Some(next), rule)),
    }
}

/// `post`: the index range to hand to `group_tokens`.
fn rule_post(
    tree: &Tree,
    node: Id,
    rule: Rule,
    pidx: usize,
    tidx: usize,
    nidx: Option<usize>,
) -> (usize, usize) {
    match rule {
        Rule::TypedLiteralOpen | Rule::TypedLiteralExtend | Rule::Arrays => (pidx, tidx),
        Rule::Assignment => {
            let nidx = nidx.unwrap_or(tidx);
            let semi = tree.find_child(node, nidx + 1, |tree, id| {
                tree.matches(id, Tk::Punctuation, ";")
            });
            match semi {
                Some((sidx, _)) => (pidx, sidx),
                None => (pidx, nidx),
            }
        }
        Rule::Period => {
            let nidx = nidx.unwrap_or(tidx);
            let next = tree.nodes[node].children.get(nidx).copied();
            let valid = next.is_some_and(|next| {
                tree.is_one_of(next, &[Class::Brackets, Class::Function])
                    || tree.ttype_in(
                        next,
                        &[Tk::Name, Tk::StringSymbol, Tk::Wildcard, Tk::StringSingle],
                    )
            });
            if valid {
                (pidx, nidx)
            } else {
                (pidx, tidx)
            }
        }
        _ => (pidx, nidx.unwrap_or(tidx)),
    }
}

/// `_group`: group tokens joined by a middle token, i.e. `x < y`.
fn group_middle(tree: &mut Tree, node: Id, rule: Rule, depth: usize) -> R {
    depth_check(depth, tree.nodes[node].children.len())?;
    let class = rule.class();
    let extend = rule.extend();
    let recurse = rule.recurses();

    let mut offset: usize = 0;
    let mut prev: Option<(usize, Id)> = None;
    let snapshot = tree.nodes[node].children.clone();

    for (idx, token) in snapshot.iter().copied().enumerate() {
        let Some(tidx) = idx.checked_sub(offset) else {
            continue;
        };
        if tree.is_whitespace(token) {
            continue;
        }
        if recurse && tree.is_group(token) && !tree.is_class(token, class) {
            group_middle(tree, token, rule, depth + 1)?;
        }
        if !rule_match(tree, token, rule) {
            prev = Some((tidx, token));
            continue;
        }
        let next = tree.token_next(node, tidx, true, false);
        let prev_ok = rule_valid_prev(tree, prev.map(|(_, id)| id), rule);
        let next_ok = rule_valid_next(tree, next.map(|(_, id)| id), rule);
        if prev_ok
            && next_ok
            && !prev.is_some_and(|(_, id)| is_delimiter(tree, node, id))
            && !next.is_some_and(|(_, id)| is_delimiter(tree, node, id))
        {
            let (from_idx, to_idx) = rule_post(
                tree,
                node,
                rule,
                prev.map_or(tidx, |(pidx, _)| pidx),
                tidx,
                next.map(|(nidx, _)| nidx),
            );
            let group = tree.group_tokens(node, class, from_idx, to_idx, extend);
            offset += to_idx - from_idx;
            prev = Some((from_idx, group));
            continue;
        }
        prev = Some((tidx, token));
    }
    Ok(())
}

/// `recurse(*cls)`: apply `f` bottom-up, skipping subtrees of the given class.
fn recurse_with(
    tree: &mut Tree,
    node: Id,
    skip: &[Class],
    f: &mut dyn FnMut(&mut Tree, Id) -> R,
) -> R {
    let subs = tree.child_groups(node);
    for sub in subs {
        if tree.class(sub).is_some_and(|class| skip.contains(&class)) {
            continue;
        }
        recurse_with(tree, sub, skip, f)?;
    }
    f(tree, node)
}

/// `group_comments`.
fn group_comments(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) = tree.find_child(node, from, |tree, id| {
        tree.ttype(id).is_some_and(Tk::is_comment)
    }) {
        let Some((eidx0, _)) = tree.token_not_matching(node, tidx, |tree, id| {
            tree.ttype(id).is_some_and(Tk::is_comment) || tree.is_newline(id)
        }) else {
            break;
        };
        let Some((eidx, _)) = tree.token_prev(node, eidx0, false, false) else {
            break;
        };
        tree.group_tokens(node, Class::Comment, tidx, eidx, false);
        from = tidx + 1;
    }
    Ok(())
}

/// `group_where`.
fn group_where(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) = tree.find_child(node, from, |tree, id| {
        tree.matches(id, Tk::Keyword, "WHERE")
    }) {
        let close = tree.find_child(node, tidx + 1, |tree, id| {
            tree.matches_any(id, Tk::Keyword, Class::Where.m_close_values())
        });
        let end_idx = match close {
            Some((eidx, _)) => eidx.saturating_sub(1).max(tidx),
            None => match tree.last_groupable(node) {
                Some(last) => tree.index_of(node, last).unwrap_or(tidx),
                None => tidx,
            },
        };
        tree.group_tokens(node, Class::Where, tidx, end_idx, false);
        from = tidx + 1;
    }
    Ok(())
}

/// `group_over`.
fn group_over(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) =
        tree.find_child(node, from, |tree, id| tree.matches(id, Tk::Keyword, "OVER"))
    {
        if let Some((nidx, next)) = tree.token_next(node, tidx, true, false) {
            if tree.is_class(next, Class::Parenthesis) || tree.ttype_isa(next, &[Tk::Name]) {
                tree.group_tokens(node, Class::Over, tidx, nidx, false);
            }
        }
        from = tidx + 1;
    }
    Ok(())
}

/// `group_functions`.
fn group_functions(tree: &mut Tree, node: Id) -> R {
    let mut has_create = false;
    let mut has_table = false;
    let mut has_as = false;
    for child in tree.nodes[node].children.clone() {
        let upper = tree.text(child).to_uppercase();
        if upper == "CREATE" {
            has_create = true;
        }
        if upper == "TABLE" {
            has_table = true;
        }
        if upper == "AS" {
            has_as = true;
        }
    }
    if has_create && has_table && !has_as {
        return Ok(());
    }

    let mut from = 0;
    while let Some((tidx, _)) =
        tree.find_child(node, from, |tree, id| tree.ttype_isa(id, &[Tk::Name]))
    {
        if let Some((nidx, next)) = tree.token_next(node, tidx, true, false) {
            if tree.is_class(next, Class::Parenthesis) {
                let eidx = match tree.token_next(node, nidx, true, false) {
                    Some((oidx, over)) if tree.is_class(over, Class::Over) => oidx,
                    _ => nidx,
                };
                tree.group_tokens(node, Class::Function, tidx, eidx, false);
            }
        }
        from = tidx + 1;
    }
    Ok(())
}

/// `group_identifier`: wrap every `Name` / `String.Symbol` in an `Identifier`.
fn group_identifier(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) = tree.find_child(node, from, |tree, id| {
        // upstream `token_next_by(t=(T.String.Symbol, T.Name))` is an exact
        // match: `Name.Builtin` / `Name.Placeholder` are *not* wrapped here.
        matches!(tree.ttype(id), Some(Tk::StringSymbol) | Some(Tk::Name))
    }) {
        tree.group_tokens(node, Class::Identifier, tidx, tidx, false);
        from = tidx + 1;
    }
    Ok(())
}

/// `group_order`: fold `ASC` / `DESC` into the preceding identifier.
fn group_order(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) = tree.next_by_ttype(node, from, Tk::KeywordOrder) {
        let mut current = tidx;
        if let Some((pidx, prev)) = tree.token_prev(node, current, true, false) {
            if tree.is_class(prev, Class::Identifier)
                || tree.ttype(prev).is_some_and(|t| t.is_number())
            {
                tree.group_tokens(node, Class::Identifier, pidx, current, false);
                current = pidx;
            }
        }
        from = current + 1;
    }
    Ok(())
}

/// `group_aliased`.
fn group_aliased(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) = tree.find_child(node, from, |tree, id| {
        tree.is_one_of(
            id,
            &[
                Class::Parenthesis,
                Class::Function,
                Class::Case,
                Class::Identifier,
                Class::Operation,
                Class::Comparison,
            ],
        ) || tree.ttype(id) == Some(Tk::NumberInteger)
            || tree.ttype(id) == Some(Tk::NumberFloat)
    }) {
        if let Some((nidx, next)) = tree.token_next(node, tidx, true, false) {
            if tree.is_class(next, Class::Identifier) {
                tree.group_tokens(node, Class::Identifier, tidx, nidx, true);
            }
        }
        from = tidx + 1;
    }
    Ok(())
}

/// `align_comments`: merge a comment into the group before it.
fn align_comments(tree: &mut Tree, node: Id) -> R {
    let mut from = 0;
    while let Some((tidx, _)) =
        tree.find_child(node, from, |tree, id| tree.is_class(id, Class::Comment))
    {
        let mut current = tidx;
        if let Some((pidx, prev)) = tree.token_prev(node, current, true, false) {
            if tree.is_group(prev) {
                tree.group_tokens(node, Class::TokenList, pidx, current, true);
                current = pidx;
            }
        }
        from = current + 1;
    }
    Ok(())
}

/// `group_values`.
fn group_values(tree: &mut Tree, node: Id) -> R {
    let Some((start_idx, _)) = tree.next_by_match(node, 0, Tk::Keyword, "VALUES") else {
        return Ok(());
    };
    let mut end_idx: Option<usize> = None;
    let mut idx = start_idx;
    while let Some((nidx, next)) = tree.token_next(node, idx, true, false) {
        if tree.is_class(next, Class::Parenthesis) {
            end_idx = Some(nidx);
        }
        idx = nidx;
    }
    if let Some(end_idx) = end_idx {
        tree.group_tokens(node, Class::Values, start_idx, end_idx, true);
    }
    Ok(())
}

/// `group()`: run every rule in the upstream order, once, on the statement.
pub fn group(tree: &mut Tree) -> R {
    let root = tree.root;

    recurse_with(tree, root, &[Class::Comment], &mut |tree, node| {
        group_comments(tree, node)
    })?;

    for class in [
        Class::Brackets,
        Class::Parenthesis,
        Class::Case,
        Class::If,
        Class::For,
        Class::Begin,
    ] {
        group_matching(tree, root, class, 0)?;
    }

    recurse_with(tree, root, &[Class::Over], &mut |tree, node| {
        group_over(tree, node)
    })?;
    recurse_with(tree, root, &[Class::Function], &mut |tree, node| {
        group_functions(tree, node)
    })?;
    recurse_with(tree, root, &[Class::Where], &mut |tree, node| {
        group_where(tree, node)
    })?;

    group_middle(tree, root, Rule::Period, 0)?;
    group_middle(tree, root, Rule::Arrays, 0)?;

    recurse_with(tree, root, &[Class::Identifier], &mut |tree, node| {
        group_identifier(tree, node)
    })?;
    recurse_with(tree, root, &[Class::Identifier], &mut |tree, node| {
        group_order(tree, node)
    })?;

    group_middle(tree, root, Rule::Typecasts, 0)?;
    group_middle(tree, root, Rule::Tzcasts, 0)?;
    group_middle(tree, root, Rule::TypedLiteralOpen, 0)?;
    group_middle(tree, root, Rule::TypedLiteralExtend, 0)?;
    group_middle(tree, root, Rule::Operator, 0)?;
    group_middle(tree, root, Rule::Comparison, 0)?;
    group_middle(tree, root, Rule::As, 0)?;

    recurse_with(tree, root, &[], &mut |tree, node| group_aliased(tree, node))?;

    group_middle(tree, root, Rule::Assignment, 0)?;

    recurse_with(tree, root, &[], &mut |tree, node| {
        align_comments(tree, node)
    })?;

    group_middle(tree, root, Rule::IdentifierList, 0)?;
    group_values(tree, root)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn grouped(sql: &str) -> Tree {
        let mut tree = Tree::from_tokens(tokenize(sql));
        group(&mut tree).unwrap();
        tree
    }

    fn classes(tree: &Tree) -> Vec<Class> {
        let mut out = Vec::new();
        collect(tree, tree.root, &mut out);
        out
    }

    fn collect(tree: &Tree, node: Id, out: &mut Vec<Class>) {
        if let Some(class) = tree.class(node) {
            out.push(class);
        }
        for child in tree.nodes[node].children.clone() {
            collect(tree, child, out);
        }
    }

    #[test]
    fn parenthesis_and_operation() {
        let tree = grouped("select (a + 1) from t");
        assert!(classes(&tree).contains(&Class::Parenthesis));
        assert!(classes(&tree).contains(&Class::Operation));
    }

    #[test]
    fn identifier_with_alias() {
        let tree = grouped("select a as b from t");
        assert!(classes(&tree).contains(&Class::Identifier));
    }

    #[test]
    fn where_clause_and_identifier_list() {
        let tree = grouped("select a, b from t where x = 1");
        assert!(classes(&tree).contains(&Class::Where));
        assert!(classes(&tree).contains(&Class::IdentifierList));
        assert!(classes(&tree).contains(&Class::Comparison));
    }

    #[test]
    fn function_call() {
        let tree = grouped("select count(1) from t");
        assert!(classes(&tree).contains(&Class::Function));
    }
}
