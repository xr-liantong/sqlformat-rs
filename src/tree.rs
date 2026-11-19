//! The token tree: an arena of nodes with parent links, mirroring
//! `sqlparse.sql.Token` / `TokenList` closely enough for the grouping engine
//! and the reindent filter.
//!
//! Python keeps a mutable object graph; indices into one arena give the same
//! mutation powers without fighting the borrow checker: every operation takes
//! `&mut Tree` plus node ids.

use crate::lexer::Token;
use crate::tokens::Tk;

pub type Id = usize;

/// `TokenList` subclasses that grouping can create.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Plain `TokenList`: only used to extend an existing group (align_comments).
    TokenList,
    Statement,
    Identifier,
    IdentifierList,
    Function,
    Parenthesis,
    Brackets,
    Case,
    Where,
    Having,
    Over,
    Operation,
    Comparison,
    Values,
    Comment,
    Assignment,
    TypedLiteral,
    If,
    For,
    Begin,
}

impl Class {
    /// `cls.M_OPEN` as `(type, values)`; an empty value list means any value.
    pub fn m_open(self) -> Option<(Tk, &'static [&'static str])> {
        Some(match self {
            Class::Parenthesis => (Tk::Punctuation, &["("]),
            Class::Brackets => (Tk::Punctuation, &["["]),
            Class::Case => (Tk::Keyword, &["CASE"]),
            Class::If => (Tk::Keyword, &["IF"]),
            Class::For => (Tk::Keyword, &["FOR", "FOREACH"]),
            Class::Begin => (Tk::Keyword, &["BEGIN"]),
            _ => return None,
        })
    }

    /// `cls.M_CLOSE` as `(type, values)`; `For` and `Case` close on a keyword.
    pub fn m_close(self) -> Option<(Tk, &'static [&'static str])> {
        Some(match self {
            Class::Parenthesis => (Tk::Punctuation, &[")"]),
            Class::Brackets => (Tk::Punctuation, &["]"]),
            Class::Case => (Tk::Keyword, &["END"]),
            Class::If => (Tk::Keyword, &["END IF"]),
            Class::For => (Tk::Keyword, &["END LOOP"]),
            Class::Begin => (Tk::Keyword, &["END"]),
            _ => return None,
        })
    }

    /// `cls.M_OPEN` of `Where`/`Over`/`Having`/`TypedLiteral`, whose delimiters
    /// are keywords that only the grouping rules test for.
    pub fn m_open_values(self) -> &'static [&'static str] {
        match self {
            Class::Where => &["WHERE"],
            Class::Over => &["OVER"],
            Class::Having => &["HAVING"],
            _ => &[],
        }
    }

    /// `cls.M_CLOSE` of `Where` and `Having`.
    pub fn m_close_values(self) -> &'static [&'static str] {
        match self {
            Class::Where => &[
                "ORDER BY",
                "GROUP BY",
                "LIMIT",
                "UNION",
                "UNION ALL",
                "EXCEPT",
                "INTERSECT",
                "HAVING",
                "RETURNING",
                "INTO",
            ],
            Class::Having => &["ORDER BY", "LIMIT"],
            _ => &[],
        }
    }

    /// Groups whose first and last children are delimiters.
    pub fn delimited(self) -> bool {
        matches!(
            self,
            Class::Parenthesis
                | Class::Brackets
                | Class::Case
                | Class::If
                | Class::For
                | Class::Begin
        )
    }
}

pub struct Node {
    /// `None` for leaves.
    pub class: Option<Class>,
    /// Leaf token type; `None` for groups (Python's `ttype is None`).
    pub ttype: Option<Tk>,
    /// Leaf value; empty for groups, whose text is their children's.
    pub value: String,
    /// `python len(token.value)`: the value cached when the node was built.
    /// Grouping and the reindent filter read this instead of the live text.
    pub cached_len: usize,
    pub children: Vec<Id>,
    pub parent: Option<Id>,
}

impl Node {
    pub fn is_group(&self) -> bool {
        self.class.is_some()
    }
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: Id,
}

impl Default for Tree {
    fn default() -> Tree {
        Tree::new()
    }
}

impl Tree {
    pub fn new() -> Tree {
        let root = Node {
            class: Some(Class::Statement),
            ttype: None,
            value: String::new(),
            cached_len: 0,
            children: Vec::new(),
            parent: None,
        };
        Tree {
            nodes: vec![root],
            root: 0,
        }
    }

    /// Build a statement tree from lexer tokens.
    pub fn from_tokens(tokens: Vec<Token>) -> Tree {
        let mut tree = Tree::new();
        let mut children = Vec::with_capacity(tokens.len());
        for tok in tokens {
            children.push(tree.push_leaf(tok.ttype, tok.value));
        }
        for child in &children {
            tree.nodes[*child].parent = Some(tree.root);
        }
        tree.nodes[tree.root].children = children;
        tree
    }

    pub fn push_leaf(&mut self, ttype: Tk, value: String) -> Id {
        let id = self.nodes.len();
        let cached_len = value.chars().count();
        self.nodes.push(Node {
            class: None,
            ttype: Some(ttype),
            value,
            cached_len,
            children: Vec::new(),
            parent: None,
        });
        id
    }

    pub fn push_group(&mut self, class: Class, children: Vec<Id>) -> Id {
        let id = self.nodes.len();
        for child in &children {
            self.nodes[*child].parent = Some(id);
        }
        let cached_len = children
            .iter()
            .map(|child| self.nodes[*child].cached_len)
            .sum();
        self.nodes.push(Node {
            class: Some(class),
            ttype: None,
            value: String::new(),
            cached_len,
            children,
            parent: None,
        });
        id
    }

    pub fn is_group(&self, id: Id) -> bool {
        self.nodes[id].is_group()
    }

    pub fn class(&self, id: Id) -> Option<Class> {
        self.nodes[id].class
    }

    pub fn is_class(&self, id: Id, class: Class) -> bool {
        self.nodes[id].class == Some(class)
    }

    /// `str(token)`: the concatenated leaf values.
    pub fn text(&self, id: Id) -> String {
        let mut out = String::new();
        self.write_text(id, &mut out);
        out
    }

    fn write_text(&self, id: Id, out: &mut String) {
        let node = &self.nodes[id];
        if !node.is_group() {
            out.push_str(&node.value);
            return;
        }
        for child in &node.children {
            self.write_text(*child, out);
        }
    }

    /// `len(token.value)` as Python sees it: cached at construction time.
    pub fn cached_len(&self, id: Id) -> usize {
        self.nodes[id].cached_len
    }

    pub fn text_len(&self, id: Id) -> usize {
        let node = &self.nodes[id];
        if !node.is_group() {
            return node.value.chars().count();
        }
        node.children
            .iter()
            .map(|child| self.text_len(*child))
            .sum()
    }

    /// `token.normalized`: keywords compare upper-cased.
    pub fn normalized(&self, id: Id) -> String {
        let node = &self.nodes[id];
        if let Some(ttype) = node.ttype {
            if ttype.is_keyword() {
                return node.value.to_uppercase();
            }
        }
        self.text(id)
    }

    pub fn ttype(&self, id: Id) -> Option<Tk> {
        self.nodes[id].ttype
    }

    pub fn is_whitespace(&self, id: Id) -> bool {
        self.nodes[id]
            .ttype
            .is_some_and(|ttype| ttype.is_whitespace())
    }

    pub fn is_newline(&self, id: Id) -> bool {
        self.nodes[id].ttype.is_some_and(|ttype| ttype.is_newline())
    }

    pub fn is_keyword(&self, id: Id) -> bool {
        self.nodes[id].ttype.is_some_and(|ttype| ttype.is_keyword())
    }

    /// `imt(token, t=...)`: is the type inside the given subtree(s)?
    /// `token.ttype in (a, b, c)`: tuple membership is an **exact** compare.
    ///
    /// sqlparse is subtle here: `imt(t=(A, B))` / `token_next_by(t=(A, B))`
    /// test `token.ttype in tuple`, i.e. equality with each element, so
    /// `Name.Builtin` does *not* match `Name`. Passing a single
    /// `_TokenType` instead uses its `__contains__`, which is the parent
    /// test -- see [`Tree::ttype_isa`].
    pub fn ttype_in(&self, id: Id, parents: &[Tk]) -> bool {
        match self.nodes[id].ttype {
            None => false,
            Some(ttype) => parents.contains(&ttype),
        }
    }

    /// `token.ttype in Token.Name`: the parent/subtype test.
    pub fn ttype_isa(&self, id: Id, parents: &[Tk]) -> bool {
        match self.nodes[id].ttype {
            None => false,
            Some(ttype) => parents.iter().any(|parent| ttype.is_a(*parent)),
        }
    }

    /// `token.match(ttype, value)`: exact type, then value (keywords upper).
    pub fn matches(&self, id: Id, ttype: Tk, value: &str) -> bool {
        let node = &self.nodes[id];
        if node.ttype != Some(ttype) {
            return false;
        }
        if node.value == value {
            return true;
        }
        node.ttype.is_some_and(|t| t.is_keyword())
            && node.value.to_uppercase() == value.to_uppercase()
    }

    /// `token.match(ttype, values)` for a set of accepted values.
    pub fn matches_any(&self, id: Id, ttype: Tk, values: &[&str]) -> bool {
        values.iter().any(|value| self.matches(id, ttype, value))
    }

    /// `imt(token, i=...)`: class membership.
    pub fn is_one_of(&self, id: Id, classes: &[Class]) -> bool {
        match self.nodes[id].class {
            Some(class) => classes.contains(&class),
            None => false,
        }
    }

    /// `tlist._groupable_tokens`: children, minus the delimiters of a group.
    pub fn groupable_children(&self, id: Id) -> Vec<Id> {
        let node = &self.nodes[id];
        if node.class.is_some_and(Class::delimited) && node.children.len() >= 2 {
            node.children[1..node.children.len() - 1].to_vec()
        } else {
            node.children.clone()
        }
    }

    pub fn index_of(&self, parent: Id, child: Id) -> Option<usize> {
        self.nodes[parent]
            .children
            .iter()
            .position(|candidate| *candidate == child)
    }

    /// `token_next(idx, skip_ws, skip_cm)` -> (index, id).
    pub fn token_next(
        &self,
        parent: Id,
        idx: usize,
        skip_ws: bool,
        skip_cm: bool,
    ) -> Option<(usize, Id)> {
        let children = &self.nodes[parent].children;
        for (i, child) in children.iter().enumerate().skip(idx + 1) {
            if self.skipped(*child, skip_ws, skip_cm) {
                continue;
            }
            return Some((i, *child));
        }
        None
    }

    /// `token_prev(idx, skip_ws, skip_cm)`.
    pub fn token_prev(
        &self,
        parent: Id,
        idx: usize,
        skip_ws: bool,
        skip_cm: bool,
    ) -> Option<(usize, Id)> {
        let children = &self.nodes[parent].children;
        if idx == 0 {
            return None;
        }
        for i in (0..idx).rev() {
            let child = children[i];
            if self.skipped(child, skip_ws, skip_cm) {
                continue;
            }
            return Some((i, child));
        }
        None
    }

    fn skipped(&self, id: Id, skip_ws: bool, skip_cm: bool) -> bool {
        (skip_ws && self.is_whitespace(id)) || (skip_cm && self.is_class(id, Class::Comment))
    }

    /// `token_first(skip_ws, skip_cm)`.
    pub fn token_first(&self, parent: Id, skip_ws: bool, skip_cm: bool) -> Option<Id> {
        self.nodes[parent]
            .children
            .iter()
            .copied()
            .find(|child| !self.skipped(*child, skip_ws, skip_cm))
    }

    /// `_token_matching`: first child at or after `from` that `pred` accepts.
    pub fn find_child(
        &self,
        parent: Id,
        from: usize,
        pred: impl Fn(&Tree, Id) -> bool,
    ) -> Option<(usize, Id)> {
        let children = self.nodes[parent].children.clone();
        for (i, child) in children.iter().enumerate().skip(from) {
            if pred(self, *child) {
                return Some((i, *child));
            }
        }
        None
    }

    /// `token_next_by(m=(ttype, value))`.
    pub fn next_by_match(
        &self,
        parent: Id,
        from: usize,
        ttype: Tk,
        value: &str,
    ) -> Option<(usize, Id)> {
        self.find_child(parent, from, |tree, id| tree.matches(id, ttype, value))
    }

    /// `token_next_by(t=ttype)`.
    pub fn next_by_ttype(&self, parent: Id, from: usize, ttype: Tk) -> Option<(usize, Id)> {
        self.find_child(parent, from, |tree, id| tree.ttype_isa(id, &[ttype]))
    }

    /// `group_tokens(cls, start, end, extend)` on `parent`'s children.
    pub fn group_tokens(
        &mut self,
        parent: Id,
        class: Class,
        start: usize,
        end: usize,
        extend: bool,
    ) -> Id {
        let end = end.min(self.nodes[parent].children.len() - 1);
        let existing = self.nodes[parent].children[start];
        if extend && self.extends_class(class, existing) {
            let taken: Vec<Id> = self.nodes[parent].children.drain(start + 1..=end).collect();
            for child in &taken {
                self.nodes[*child].parent = Some(existing);
            }
            self.nodes[existing].children.extend(taken);
            return existing;
        }
        let taken: Vec<Id> = self.nodes[parent].children.drain(start..=end).collect();
        let group = self.push_group(class, taken);
        self.nodes[group].parent = Some(parent);
        self.nodes[parent].children.insert(start, group);
        group
    }

    /// `isinstance(token, cls)` for `group_tokens(..., extend=True)`: a plain
    /// `TokenList` accepts any group, everything else needs its own class.
    fn extends_class(&self, class: Class, id: Id) -> bool {
        match class {
            Class::TokenList => self.nodes[id].is_group(),
            other => self.nodes[id].class == Some(other),
        }
    }

    pub fn set_ttype(&mut self, id: Id, ttype: Tk) {
        self.nodes[id].ttype = Some(ttype);
    }

    /// `tlist.get_sublists()`, in document order.
    pub fn child_groups(&self, parent: Id) -> Vec<Id> {
        self.nodes[parent]
            .children
            .iter()
            .copied()
            .filter(|child| self.nodes[*child].is_group())
            .collect()
    }

    /// `token_not_matching(pred, idx)`: first child at or after `idx` that
    /// `pred` rejects.
    pub fn token_not_matching(
        &self,
        parent: Id,
        from: usize,
        pred: impl Fn(&Tree, Id) -> bool,
    ) -> Option<(usize, Id)> {
        let children = self.nodes[parent].children.clone();
        for (i, child) in children.iter().enumerate().skip(from) {
            if !pred(self, *child) {
                return Some((i, *child));
            }
        }
        None
    }

    /// `tlist._groupable_tokens[-1]`.
    pub fn last_groupable(&self, parent: Id) -> Option<Id> {
        self.groupable_children(parent).last().copied()
    }

    pub fn child_at(&self, parent: Id, idx: usize) -> Option<Id> {
        self.nodes[parent].children.get(idx).copied()
    }

    pub fn child_count(&self, parent: Id) -> usize {
        self.nodes[parent].children.len()
    }

    pub fn parent_of(&self, id: Id) -> Option<Id> {
        self.nodes[id].parent
    }

    pub fn set_value(&mut self, id: Id, value: String) {
        self.nodes[id].value = value;
    }

    pub fn value_of(&self, id: Id) -> &str {
        &self.nodes[id].value
    }

    /// `del tlist.tokens[idx]`; the node stays in the arena, detached.
    pub fn remove_child(&mut self, parent: Id, idx: usize) -> Id {
        let removed = self.nodes[parent].children.remove(idx);
        self.nodes[removed].parent = None;
        removed
    }

    /// `tlist.within(cls)`: is any ancestor of this class?
    pub fn within(&self, id: Id, class: Class) -> bool {
        let mut parent = self.nodes[id].parent;
        while let Some(node) = parent {
            if self.nodes[node].class == Some(class) {
                return true;
            }
            parent = self.nodes[node].parent;
        }
        false
    }

    /// `next(group.flatten())`: the first leaf of a group.
    pub fn first_leaf(&self, id: Id) -> Option<Id> {
        let node = &self.nodes[id];
        if !node.is_group() {
            return Some(id);
        }
        node.children
            .iter()
            .find_map(|child| self.first_leaf(*child))
    }

    /// `list(tlist.get_identifiers())`: children that are neither whitespace
    /// nor a comma.
    pub fn identifiers(&self, node: Id) -> Vec<Id> {
        self.nodes[node]
            .children
            .iter()
            .copied()
            .filter(|child| {
                !self.is_whitespace(*child) && !self.matches(*child, Tk::Punctuation, ",")
            })
            .collect()
    }

    /// `insert_before(idx, token)`.
    pub fn insert_before(&mut self, parent: Id, idx: usize, node: Id) {
        self.nodes[node].parent = Some(parent);
        self.nodes[parent].children.insert(idx, node);
    }

    /// `insert_after(idx, token, skip_ws)`.
    pub fn insert_after(&mut self, parent: Id, idx: usize, node: Id, skip_ws: bool) {
        self.nodes[node].parent = Some(parent);
        match self.token_next(parent, idx, skip_ws, false) {
            Some((nidx, _)) => self.nodes[parent].children.insert(nidx, node),
            None => self.nodes[parent].children.push(node),
        }
    }

    /// `flatten()`: leaf ids in document order.
    pub fn leaves(&self, id: Id) -> Vec<Id> {
        let mut out = Vec::new();
        self.collect_leaves(id, &mut out);
        out
    }

    fn collect_leaves(&self, id: Id, out: &mut Vec<Id>) {
        let node = &self.nodes[id];
        if node.is_group() {
            for child in &node.children {
                self.collect_leaves(*child, out);
            }
        } else {
            out.push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn tree_text_round_trips() {
        let sql = "select a, b from t where x = 1";
        let tree = Tree::from_tokens(tokenize(sql));
        assert_eq!(tree.text(tree.root), sql);
    }

    #[test]
    fn grouping_replaces_children() {
        let mut tree = Tree::from_tokens(tokenize("(a)"));
        let group = tree.group_tokens(tree.root, Class::Parenthesis, 0, 2, false);
        assert_eq!(tree.nodes[group].children.len(), 3);
        assert_eq!(tree.nodes[tree.root].children.len(), 1);
        assert_eq!(tree.text(tree.root), "(a)");
    }
}
