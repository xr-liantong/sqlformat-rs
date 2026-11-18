//! Ports of the postprocessing filters: `StripWhitespaceFilter`,
//! `ReindentFilter` and `SerializerUnicode`, plus their small helpers.

use crate::lexer::Token;
use crate::tokens::Tk;
use crate::tree::{Class, Id, Tree};

/// `str.splitlines()`: breaks on every Unicode line boundary, and keeps no
/// trailing empty line.
pub fn py_splitlines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' | '\u{000b}' | '\u{000c}' | '\u{001c}' | '\u{001d}' | '\u{001e}' | '\u{0085}'
            | '\u{2028}' | '\u{2029}' => lines.push(std::mem::take(&mut current)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// `utils.split_unquoted_newlines`: newlines inside quotes are not breaks.
pub fn split_unquoted_newlines(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let length = chars.len();
    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut pos = 0;
    while pos < length {
        let c = chars[pos];
        if c == '\r' || c == '\n' {
            lines.push(chars[line_start..pos].iter().collect());
            pos += if c == '\r' && pos + 1 < length && chars[pos + 1] == '\n' {
                2
            } else {
                1
            };
            line_start = pos;
        } else if c == '"' || c == '\'' {
            let end = string_content_end(&chars, pos + 1, c);
            if end < length && chars[end] == c {
                pos = end + 1;
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }
    lines.push(chars[line_start..].iter().collect());
    lines
}

/// End of `(?:[^"\\]|\\.)*` starting at `start`.
fn string_content_end(chars: &[char], start: usize, quote: char) -> usize {
    let mut pos = start;
    while pos < chars.len() {
        let c = chars[pos];
        if c == quote {
            return pos;
        }
        if c == '\\' && pos + 1 < chars.len() && chars[pos + 1] != '\n' {
            pos += 2;
            continue;
        }
        pos += 1;
    }
    pos
}

/// `str.capitalize`: first character upper, the rest lower.
pub fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.push_str(&chars.as_str().to_lowercase());
            out
        }
    }
}

/// `SerializerUnicode`: re-split, strip trailing spaces per line.
pub fn serialize(tree: &Tree, stmt: Id) -> String {
    let text = tree.text(stmt);
    split_unquoted_newlines(&text)
        .iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct StripWhitespace;

impl StripWhitespace {
    pub fn process(&mut self, tree: &mut Tree, stmt: Id) {
        self.recurse(tree, stmt);
        if let Some(last) = tree.child_at(stmt, tree.child_count(stmt).saturating_sub(1)) {
            if tree.is_whitespace(last) {
                let idx = tree.child_count(stmt) - 1;
                tree.remove_child(stmt, idx);
            }
        }
    }

    fn recurse(&mut self, tree: &mut Tree, node: Id) {
        for sub in tree.child_groups(node) {
            self.recurse(tree, sub);
        }
        match tree.class(node) {
            Some(Class::IdentifierList) => self.stripws_identifierlist(tree, node),
            Some(Class::Parenthesis) => self.stripws_parenthesis(tree, node),
            _ => self.stripws_default(tree, node),
        }
    }

    fn stripws_default(&mut self, tree: &mut Tree, node: Id) {
        let mut last_was_ws = false;
        let mut is_first = true;
        for idx in 0..tree.child_count(node) {
            let Some(child) = tree.child_at(node, idx) else {
                break;
            };
            if tree.is_whitespace(child) {
                let value = if last_was_ws || is_first { "" } else { " " };
                tree.set_value(child, value.to_string());
                last_was_ws = true;
            } else {
                last_was_ws = false;
            }
            is_first = false;
        }
    }

    fn stripws_identifierlist(&mut self, tree: &mut Tree, node: Id) {
        let mut last_nl: Option<Id> = None;
        let snapshot = tree.nodes[node].children.clone();
        for child in snapshot {
            if let Some(nl) = last_nl {
                if tree.matches(child, Tk::Punctuation, ",") {
                    if let Some(idx) = tree.index_of(node, nl) {
                        tree.remove_child(node, idx);
                    }
                }
            }
            last_nl = if tree.is_whitespace(child) {
                Some(child)
            } else {
                None
            };
        }
        self.stripws_default(tree, node);
    }

    fn stripws_parenthesis(&mut self, tree: &mut Tree, node: Id) {
        let count = tree.child_count(node);
        let is_paren = count >= 2
            && tree
                .child_at(node, 0)
                .is_some_and(|child| tree.matches(child, Tk::Punctuation, "("))
            && tree
                .child_at(node, count - 1)
                .is_some_and(|child| tree.matches(child, Tk::Punctuation, ")"));
        if !is_paren {
            self.stripws_default(tree, node);
            return;
        }
        while tree.child_count(node) > 1 && tree.is_whitespace(tree.child_at(node, 1).unwrap()) {
            tree.remove_child(node, 1);
        }
        while tree.child_count(node) > 1
            && tree.is_whitespace(tree.child_at(node, tree.child_count(node) - 2).unwrap())
        {
            tree.remove_child(node, tree.child_count(node) - 2);
        }
        let count = tree.child_count(node);
        if count >= 2 {
            let second_last = tree.child_at(node, count - 2).unwrap();
            if tree.is_group(second_last) {
                while let Some(last) =
                    tree.child_at(second_last, tree.child_count(second_last).saturating_sub(1))
                {
                    if tree.is_whitespace(last) && tree.child_count(second_last) > 0 {
                        let idx = tree.child_count(second_last) - 1;
                        tree.remove_child(second_last, idx);
                    } else {
                        break;
                    }
                }
            }
        }
        self.stripws_default(tree, node);
    }
}

/// `ReindentFilter`.
pub struct ReindentFilter {
    pub width: usize,
    pub wrap_after: i64,
    pub comma_first: bool,
    pub indent_after_first: bool,
    pub indent_columns: bool,
    pub compact: bool,
    indent: i64,
    offset: i64,
    curr_stmt: Option<Id>,
    last_stmt_text: Option<String>,
    last_func: Option<Id>,
}

impl ReindentFilter {
    pub fn new(width: usize) -> ReindentFilter {
        ReindentFilter {
            width,
            wrap_after: 0,
            comma_first: false,
            indent_after_first: false,
            indent_columns: false,
            compact: false,
            indent: 0,
            offset: 0,
            curr_stmt: None,
            last_stmt_text: None,
            last_func: None,
        }
    }

    fn leading_ws(&self) -> i64 {
        self.offset + self.indent * self.width as i64
    }

    fn nl(&self, offset: i64) -> String {
        let count = (self.leading_ws() + offset).max(0) as usize;
        format!("\n{}", " ".repeat(count))
    }

    fn new_nl(&self, tree: &mut Tree, offset: i64) -> Id {
        let value = self.nl(offset);
        tree.push_leaf(Tk::Whitespace, value)
    }

    fn current_line_len(&self, tree: &Tree, token: Id) -> i64 {
        let mut length: i64 = 0;
        let mut node = token;
        let stop = self.curr_stmt;
        while Some(node) != stop {
            let Some(parent) = tree.parent_of(node) else {
                break;
            };
            let Some(pos) = tree.index_of(parent, node) else {
                break;
            };
            let mut stack: Vec<Id> = (0..pos)
                .filter_map(|idx| tree.child_at(parent, idx))
                .collect();
            while let Some(prev) = stack.pop() {
                if tree.is_group(prev) {
                    stack.extend(tree.nodes[prev].children.iter().copied());
                    continue;
                }
                let value = tree.value_of(prev);
                let size = value.chars().count();
                if size == 0 {
                    continue;
                }
                let lines = py_splitlines(value);
                if lines.len() == 1 && lines[0].chars().count() == size {
                    length += size as i64;
                    continue;
                }
                let mut sentinel = value.to_string();
                sentinel.push('.');
                let lines = py_splitlines(&sentinel);
                let tail = lines
                    .last()
                    .map(|line| line.chars().count() as i64)
                    .unwrap_or(0)
                    - 1
                    + length;
                if tail != 0 {
                    return tail;
                }
                if lines.len() > 2 {
                    return lines[lines.len() - 2].chars().count() as i64;
                }
                length = lines[0].chars().count() as i64;
            }
            node = parent;
        }
        length
    }

    fn get_offset(&self, tree: &Tree, token: Id) -> i64 {
        self.current_line_len(tree, token) - self.leading_ws().max(0)
    }

    fn find_split(&self, tree: &Tree, tlist: Id, from: usize) -> Option<(usize, Id)> {
        const WORDS: [&str; 14] = [
            "FROM",
            "STRAIGHT_JOIN",
            "JOIN",
            "AND",
            "OR",
            "GROUP BY",
            "ORDER BY",
            "UNION",
            "VALUES",
            "SET",
            "BETWEEN",
            "EXCEPT",
            "HAVING",
            "LIMIT",
        ];
        const SUFFIXED: [usize; 2] = [1, 2];
        tree.find_child(tlist, from, |t, id| {
            if t.ttype(id) != Some(Tk::Keyword) {
                return false;
            }
            let normalized = t.normalized(id).to_uppercase();
            WORDS.iter().enumerate().any(|(index, word)| {
                if SUFFIXED.contains(&index) {
                    normalized.ends_with(word)
                } else {
                    normalized.contains(word)
                }
            })
        })
    }

    fn next_token(&self, tree: &Tree, tlist: Id, from: usize) -> Option<(usize, Id)> {
        let (tidx, token) = self.find_split(tree, tlist, from)?;
        if tree.normalized(token) == "BETWEEN" {
            let (tidx, token) = self.next_token(tree, tlist, tidx + 1)?;
            if tree.normalized(token) == "AND" {
                return self.next_token(tree, tlist, tidx + 1);
            }
            return Some((tidx, token));
        }
        Some((tidx, token))
    }

    fn split_kwds(&self, tree: &mut Tree, tlist: Id) {
        let mut from = 0;
        while let Some((mut tidx, _)) = self.next_token(tree, tlist, from) {
            let prev = tree.token_prev(tlist, tidx, false, false);
            let uprev = match prev {
                Some((_, token)) => tree.text(token),
                None => "None".to_string(),
            };
            if let Some((pidx, token)) = prev {
                if tree.is_whitespace(token) {
                    tree.remove_child(tlist, pidx);
                    tidx -= 1;
                }
            }
            if !(uprev.ends_with('\n') || uprev.ends_with('\r')) {
                let nl = self.new_nl(tree, 0);
                tree.insert_before(tlist, tidx, nl);
                tidx += 1;
            }
            from = tidx + 1;
        }
    }

    fn split_statements(&self, tree: &mut Tree, tlist: Id) {
        let mut from = 0;
        while let Some((mut tidx, _)) = tree.find_child(tlist, from, |t, id| {
            matches!(t.ttype(id), Some(Tk::KeywordDml) | Some(Tk::KeywordDdl))
        }) {
            let prev = tree.token_prev(tlist, tidx, false, false);
            if let Some((pidx, token)) = prev {
                if tree.is_whitespace(token) {
                    tree.remove_child(tlist, pidx);
                    tidx -= 1;
                }
            }
            if prev.is_some() {
                let nl = self.new_nl(tree, 0);
                tree.insert_before(tlist, tidx, nl);
                tidx += 1;
            }
            from = tidx + 1;
        }
    }

    fn process_node(&mut self, tree: &mut Tree, node: Id) {
        match tree.class(node) {
            Some(Class::Where) => self.process_where(tree, node),
            Some(Class::Parenthesis) => self.process_parenthesis(tree, node),
            Some(Class::Function) => self.process_function(tree, node),
            Some(Class::IdentifierList) => self.process_identifierlist(tree, node),
            Some(Class::Case) => self.process_case(tree, node),
            Some(Class::Values) => self.process_values(tree, node),
            _ => self.process_default(tree, node, true),
        }
    }

    fn process_default(&mut self, tree: &mut Tree, tlist: Id, stmts: bool) {
        if stmts {
            self.split_statements(tree, tlist);
        }
        self.split_kwds(tree, tlist);
        for sub in tree.child_groups(tlist) {
            self.process_node(tree, sub);
        }
    }

    fn process_where(&mut self, tree: &mut Tree, tlist: Id) {
        let Some((tidx, _)) = tree.next_by_match(tlist, 0, Tk::Keyword, "WHERE") else {
            return;
        };
        let nl = self.new_nl(tree, 0);
        tree.insert_before(tlist, tidx, nl);
        self.indent += 1;
        self.process_default(tree, tlist, true);
        self.indent -= 1;
    }

    fn process_parenthesis(&mut self, tree: &mut Tree, tlist: Id) {
        let is_dml = tree
            .find_child(tlist, 0, |t, id| {
                matches!(t.ttype(id), Some(Tk::KeywordDml) | Some(Tk::KeywordDdl))
            })
            .is_some();
        let Some((_, first)) =
            tree.find_child(tlist, 0, |t, id| t.matches(id, Tk::Punctuation, "("))
        else {
            return;
        };
        if is_dml {
            self.indent += 1;
            let nl = self.new_nl(tree, 0);
            tree.insert_before(tlist, 0, nl);
        }
        let offset = self.get_offset(tree, first) + 1;
        self.offset += offset;
        self.process_default(tree, tlist, !is_dml);
        self.offset -= offset;
        if is_dml {
            self.indent -= 1;
        }
    }

    fn process_function(&mut self, tree: &mut Tree, tlist: Id) {
        self.last_func = tree.child_at(tlist, 0);
        self.process_default(tree, tlist, true);
    }

    fn process_identifierlist(&mut self, tree: &mut Tree, tlist: Id) {
        let mut identifiers = tree.identifiers(tlist);
        if identifiers.is_empty() {
            self.process_default(tree, tlist, true);
            return;
        }
        let (first, num_offset) = if self.indent_columns {
            let first = tree.first_leaf(identifiers[0]);
            let num_offset = self.width as i64;
            (first, num_offset)
        } else {
            let first_id = identifiers.remove(0);
            let first = tree.first_leaf(first_id);
            let num_offset = first.map(|token| self.get_offset(tree, token)).unwrap_or(0);
            (first, num_offset)
        };
        let _ = first;

        let within_wrap = tree.within(tlist, Class::Function) || tree.within(tlist, Class::Values);
        if !within_wrap {
            self.offset += num_offset;
            let mut position: i64 = 0;
            for token in identifiers {
                position += tree.cached_len(token) as i64 + 1;
                if position > self.wrap_after - self.offset {
                    let adjust = 0;
                    let Some(idx) = tree.index_of(tlist, token) else {
                        continue;
                    };
                    let nl = self.new_nl(tree, adjust);
                    tree.insert_before(tlist, idx, nl);
                    position = 0;
                }
            }
            self.offset -= num_offset;
        } else {
            let snapshot = tree.nodes[tlist].children.clone();
            for token in snapshot {
                let Some(idx) = tree.index_of(tlist, token) else {
                    continue;
                };
                let next = tree.token_next(tlist, idx, false, false);
                if let Some((_, next_ws)) = next {
                    if tree.value_of(token) == "," && !tree.is_whitespace(next_ws) {
                        let nl = tree.push_leaf(Tk::Whitespace, " ".to_string());
                        tree.insert_after(tlist, idx, nl, true);
                    }
                }
            }
            let end_at: i64 = self.offset
                + identifiers
                    .iter()
                    .map(|token| tree.cached_len(*token) as i64 + 1)
                    .sum::<i64>();
            let mut adjusted_offset: i64 = 0;
            if let Some(last_func) = self.last_func {
                if self.wrap_after > 0 && end_at > self.wrap_after - self.offset {
                    adjusted_offset = -(tree.cached_len(last_func) as i64) - 1;
                }
            }
            self.offset += adjusted_offset;
            self.indent += 1;
            if adjusted_offset < 0 {
                if let Some(first_id) = identifiers.first() {
                    let nl = self.new_nl(tree, 0);
                    if let Some(idx) = tree.index_of(tlist, *first_id) {
                        tree.insert_before(tlist, idx, nl);
                    }
                }
            }
            let mut position: i64 = 0;
            for token in &identifiers {
                position += tree.cached_len(*token) as i64 + 1;
                if self.wrap_after > 0 && position > self.wrap_after - self.offset {
                    if let Some(idx) = tree.index_of(tlist, *token) {
                        let nl = self.new_nl(tree, 0);
                        tree.insert_before(tlist, idx, nl);
                    }
                    position = 0;
                }
            }
            self.indent -= 1;
            self.offset -= adjusted_offset;
        }
        self.process_default(tree, tlist, true);
    }

    fn get_cases(&self, tree: &Tree, node: Id) -> Vec<(Option<Vec<Id>>, Vec<Id>)> {
        let mut ret: Vec<(Option<Vec<Id>>, Vec<Id>)> = Vec::new();
        // Some(true) = CONDITION, Some(false) = VALUE, None = off.
        let mut mode: Option<bool> = Some(true);
        for child in tree.nodes[node].children.clone() {
            let token = child;
            if tree.matches(token, Tk::Keyword, "CASE") {
                continue;
            } else if tree.matches(token, Tk::Keyword, "WHEN") {
                ret.push((Some(Vec::new()), Vec::new()));
                mode = Some(true);
            } else if tree.matches(token, Tk::Keyword, "THEN") {
                mode = Some(false);
            } else if tree.matches(token, Tk::Keyword, "ELSE") {
                ret.push((None, Vec::new()));
                mode = Some(false);
            } else if tree.matches(token, Tk::Keyword, "END") {
                mode = None;
            }

            if mode.is_some() && ret.is_empty() {
                ret.push((Some(Vec::new()), Vec::new()));
            }
            match mode {
                Some(true) => {
                    if let Some(last) = ret.last_mut() {
                        last.0.get_or_insert_with(Vec::new).push(token);
                    }
                }
                Some(false) => {
                    if let Some(last) = ret.last_mut() {
                        last.1.push(token);
                    }
                }
                None => {}
            }
        }
        ret
    }

    fn process_case(&mut self, tree: &mut Tree, tlist: Id) {
        let cases = self.get_cases(tree, tlist);
        if cases.is_empty() {
            return;
        }
        let first = cases[0]
            .0
            .as_ref()
            .and_then(|cond| cond.first().copied())
            .and_then(|token| tree.first_leaf(token));
        let Some(first) = first else {
            return;
        };
        let outer = tree
            .child_at(tlist, 0)
            .map(|token| self.get_offset(tree, token))
            .unwrap_or(0);
        self.offset += outer;
        let inner = self.get_offset(tree, first);
        self.offset += inner;

        for (cond, value) in cases.iter().skip(1) {
            let str_cond = match cond {
                Some(tokens) => tokens
                    .iter()
                    .map(|token| tree.text(*token))
                    .collect::<String>(),
                None => String::new(),
            };
            let str_value: String = value.iter().map(|token| tree.text(*token)).collect();
            let end_pos = self.offset
                + 1
                + str_cond.chars().count() as i64
                + str_value.chars().count() as i64;
            if !self.compact && end_pos > self.wrap_after {
                let target = match cond {
                    Some(tokens) => tokens.first().copied(),
                    None => value.first().copied(),
                };
                if let Some(target) = target {
                    if let Some(idx) = tree.index_of(tlist, target) {
                        let nl = self.new_nl(tree, 0);
                        tree.insert_before(tlist, idx, nl);
                    }
                }
            }
        }

        self.offset += 5;
        self.process_default(tree, tlist, true);
        self.offset -= 5;

        self.offset -= inner;
        if let Some((end_idx, _)) = tree.next_by_match(tlist, 0, Tk::Keyword, "END") {
            if !self.compact {
                let nl = self.new_nl(tree, 0);
                tree.insert_before(tlist, end_idx, nl);
            }
        }

        self.offset -= outer;
    }

    fn process_values(&mut self, tree: &mut Tree, tlist: Id) {
        let nl = self.new_nl(tree, 0);
        tree.insert_before(tlist, 0, nl);
        let mut current =
            tree.find_child(tlist, 0, |t, id| t.class(id) == Some(Class::Parenthesis));
        while let Some((tidx, token)) = current {
            if let Some((pidx, _)) = tree.next_by_match(tlist, tidx + 1, Tk::Punctuation, ",") {
                if self.comma_first {
                    let offset = self.get_offset(tree, token) - 2;
                    let nl = self.new_nl(tree, offset);
                    tree.insert_before(tlist, pidx, nl);
                } else {
                    let offset = self.get_offset(tree, token);
                    let nl = self.new_nl(tree, offset);
                    tree.insert_after(tlist, pidx, nl, true);
                }
            }
            current = tree.find_child(tlist, tidx + 1, |t, id| {
                t.class(id) == Some(Class::Parenthesis)
            });
        }
    }

    /// `process(stmt)`: reindent one statement, keeping the cross-statement state.
    pub fn process(&mut self, tree: &mut Tree, stmt: Id) {
        self.curr_stmt = Some(stmt);
        self.process_node(tree, stmt);

        if let Some(last) = self.last_stmt_text.clone() {
            let nl = if last.ends_with('\n') { "\n" } else { "\n\n" };
            let token = tree.push_leaf(Tk::Whitespace, nl.to_string());
            tree.insert_before(stmt, 0, token);
        }
        self.last_stmt_text = Some(tree.text(stmt));
    }
}

/// `KeywordCaseFilter` / `IdentifierCaseFilter`.
/// Apply one case option to a single value.
pub fn transform(value: &str, case: &str) -> String {
    match case {
        "lower" => value.to_lowercase(),
        "capitalize" => capitalize(value),
        _ => value.to_uppercase(),
    }
}

pub fn apply_case(tokens: &mut [Token], case: &str, keywords: bool) {
    for token in tokens {
        let hit = if keywords {
            token.ttype.is_keyword()
        } else {
            matches!(token.ttype, Tk::Name | Tk::StringSymbol)
                && !token.value.trim_start().starts_with('"')
        };
        if !hit {
            continue;
        }
        token.value = transform(&token.value, case);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_unquoted_newlines_ignores_quoted_breaks() {
        let lines = split_unquoted_newlines("select 'a\nb'\nfrom t");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "select 'a\nb'");
    }

    #[test]
    fn py_splitlines_drops_trailing_empty() {
        assert_eq!(py_splitlines("a\n"), vec!["a".to_string()]);
        assert!(py_splitlines("").is_empty());
    }

    #[test]
    fn capitalize_matches_python() {
        assert_eq!(capitalize("abc"), "Abc");
        assert_eq!(capitalize("ABC"), "Abc");
    }
}
