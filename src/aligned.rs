//! `AlignedIndentFilter` (`-a/--reindent_aligned`): the `-r` layout with the
//! keywords aligned in a column instead of indented by width, plus `CASE`
//! condition/`END` alignment.

use crate::tokens::Tk;
use crate::tree::{Class, Id, Tree};

/// `self._max_kwd_len = len('select')`.
const MAX_KWD_LEN: i64 = 6;
/// `len('case ') + len('when ')`.
const CASE_WHEN: i64 = 10;

const SPLIT_WORDS: [&str; 12] = [
    "FROM", "ON", "WHERE", "AND", "OR", "HAVING", "LIMIT", "UNION", "VALUES", "SET", "BETWEEN",
    "EXCEPT",
];

pub struct AlignedIndentFilter {
    offset: i64,
    indent: i64,
}

impl Default for AlignedIndentFilter {
    fn default() -> AlignedIndentFilter {
        AlignedIndentFilter::new()
    }
}

impl AlignedIndentFilter {
    pub fn new() -> AlignedIndentFilter {
        AlignedIndentFilter {
            offset: 0,
            indent: 0,
        }
    }

    /// `nl(offset)`: `'\n'` plus `max_kwd_len + offset + indent*9 + offset`
    /// spaces. A non-positive count is an empty string, as `' ' * n` in Python.
    fn nl(&self, offset: i64) -> String {
        let count = MAX_KWD_LEN + offset + self.indent * (2 + MAX_KWD_LEN) + self.offset;
        if count <= 0 {
            "\n".to_string()
        } else {
            format!("\n{}", " ".repeat(count as usize))
        }
    }

    fn new_nl(&self, tree: &mut Tree, offset: i64) -> Id {
        let value = self.nl(offset);
        tree.push_leaf(Tk::Whitespace, value)
    }

    pub fn process(&mut self, tree: &mut Tree, stmt: Id) {
        // `_process_statement`: a statement never starts with whitespace.
        if self.indent == 0 {
            if let Some(first) = tree.child_at(stmt, 0) {
                if tree.is_whitespace(first) {
                    tree.remove_child(stmt, 0);
                }
            }
        }
        self.process_default(tree, stmt);
    }

    fn process_node(&mut self, tree: &mut Tree, node: Id) {
        match tree.class(node) {
            Some(Class::Parenthesis) => self.process_parenthesis(tree, node),
            Some(Class::IdentifierList) => self.process_identifierlist(tree, node),
            Some(Class::Case) => self.process_case(tree, node),
            _ => self.process_default(tree, node),
        }
    }

    fn process_parenthesis(&mut self, tree: &mut Tree, tlist: Id) {
        // A parenthesis without a `SELECT` inside is not re-indented.
        let is_subquery = tree
            .find_child(tlist, 0, |t, id| t.matches(id, Tk::KeywordDml, "SELECT"))
            .is_some();
        if !is_subquery {
            return;
        }
        self.indent += 1;
        let nl = self.new_nl(tree, -6);
        tree.insert_after(tlist, 0, nl, true);
        self.process_default(tree, tlist);
        self.indent -= 1;
        if tree.child_count(tlist) > 0 {
            let last = tree.child_count(tlist) - 1;
            let nl = self.new_nl(tree, 1);
            tree.insert_before(tlist, last, nl);
        }
    }

    fn process_identifierlist(&mut self, tree: &mut Tree, tlist: Id) {
        let mut identifiers = tree.identifiers(tlist);
        if identifiers.is_empty() {
            self.process_default(tree, tlist);
            return;
        }
        identifiers.remove(0);
        for token in identifiers {
            if let Some(idx) = tree.index_of(tlist, token) {
                let nl = self.new_nl(tree, 1);
                tree.insert_before(tlist, idx, nl);
            }
        }
        self.process_default(tree, tlist);
    }

    fn next_split(&self, tree: &Tree, tlist: Id, from: usize) -> Option<(usize, Id)> {
        tree.find_child(tlist, from, |t, id| {
            if t.ttype(id) != Some(Tk::Keyword) {
                return false;
            }
            let normalized = t.normalized(id);
            is_join(&normalized)
                || is_by(&normalized)
                || SPLIT_WORDS.iter().any(|word| normalized.contains(word))
        })
    }

    /// `_next_token`: `BETWEEN x AND y` is one statement, not two.
    fn next_token(&self, tree: &Tree, tlist: Id, from: usize) -> Option<(usize, Id)> {
        let (tidx, token) = self.next_split(tree, tlist, from)?;
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
        while let Some((mut tidx, token)) = self.next_token(tree, tlist, from) {
            let normalized = tree.normalized(token);
            // Joins and `GROUP/ORDER BY` align on their first word only.
            let width = if is_join(&normalized) || is_by(&normalized) {
                normalized
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .count() as i64
            } else {
                tree.text(token).chars().count() as i64
            };
            let nl = self.new_nl(tree, -width);
            tree.insert_before(tlist, tidx, nl);
            tidx += 1;
            from = tidx + 1;
        }
    }

    fn process_default(&mut self, tree: &mut Tree, tlist: Id) {
        self.split_kwds(tree, tlist);
        for sub in tree.child_groups(tlist) {
            let Some(idx) = tree.index_of(tlist, sub) else {
                continue;
            };
            let by = match tree.token_prev(tlist, idx, true, false) {
                Some((_, prev)) => is_by(&tree.normalized(prev)),
                None => false,
            };
            // HACK in upstream: `GROUP BY`/`ORDER BY` shift their sublist by 3.
            let offset = if by { 3 } else { 0 };
            self.offset += offset;
            self.process_node(tree, sub);
            self.offset -= offset;
        }
    }

    /// `Case.get_cases(skip_ws=True)`.
    fn get_cases(&self, tree: &Tree, node: Id) -> Vec<(Option<Vec<Id>>, Vec<Id>)> {
        let mut cases: Vec<(Option<Vec<Id>>, Vec<Id>)> = Vec::new();
        // Some(true) = CONDITION, Some(false) = VALUE, None = off.
        let mut mode: Option<bool> = Some(true);
        for token in tree.nodes[node].children.clone() {
            if tree.matches(token, Tk::Keyword, "CASE") || tree.is_whitespace(token) {
                continue;
            } else if tree.matches(token, Tk::Keyword, "WHEN") {
                cases.push((Some(Vec::new()), Vec::new()));
                mode = Some(true);
            } else if tree.matches(token, Tk::Keyword, "THEN") {
                mode = Some(false);
            } else if tree.matches(token, Tk::Keyword, "ELSE") {
                cases.push((None, Vec::new()));
                mode = Some(false);
            } else if tree.matches(token, Tk::Keyword, "END") {
                mode = None;
            }
            if mode.is_some() && cases.is_empty() {
                cases.push((Some(Vec::new()), Vec::new()));
            }
            match mode {
                Some(true) => {
                    if let Some(last) = cases.last_mut() {
                        last.0.get_or_insert_with(Vec::new).push(token);
                    }
                }
                Some(false) => {
                    if let Some(last) = cases.last_mut() {
                        last.1.push(token);
                    }
                }
                None => {}
            }
        }
        cases
    }

    fn process_case(&mut self, tree: &mut Tree, tlist: Id) {
        let mut cases = self.get_cases(tree, tlist);
        // Align `END` as well.
        if let Some((_, end)) = tree.next_by_match(tlist, 0, Tk::Keyword, "END") {
            cases.push((None, vec![end]));
        }
        if cases.is_empty() {
            return;
        }
        let widths: Vec<i64> = cases
            .iter()
            .map(|(cond, _)| match cond {
                Some(tokens) => tokens
                    .iter()
                    .map(|token| tree.text(*token))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .count() as i64,
                None => 0,
            })
            .collect();
        let max_width = widths.iter().copied().max().unwrap_or(0);
        for (index, (cond, value)) in cases.iter().enumerate() {
            let stmt = match cond {
                Some(tokens) => tokens.first().copied(),
                None => value.first().copied(),
            };
            let Some(stmt) = stmt else {
                continue;
            };
            if index > 0 {
                if let Some(idx) = tree.index_of(tlist, stmt) {
                    let width = tree.text(stmt).chars().count() as i64;
                    let nl = self.new_nl(tree, CASE_WHEN - width);
                    tree.insert_before(tlist, idx, nl);
                }
            }
            if let Some(tokens) = cond {
                let pad = max_width - widths[index];
                if pad > 0 {
                    if let Some(last) = tokens.last() {
                        let ws = tree.push_leaf(Tk::Whitespace, " ".repeat(pad as usize));
                        tree.insert_after(
                            tlist,
                            tree.index_of(tlist, *last).unwrap_or(0),
                            ws,
                            true,
                        );
                    }
                }
            }
        }
    }
}

fn words(normalized: &str) -> Vec<&str> {
    normalized.split_whitespace().collect()
}

fn has_words(normalized: &str, sequence: &[&str]) -> bool {
    words(normalized)
        .windows(sequence.len())
        .any(|window| window == sequence)
}

/// `join_words`: `…JOIN` on its own word boundary.
fn is_join(normalized: &str) -> bool {
    has_words(normalized, &["JOIN"])
}

/// `by_words`: `(GROUP|ORDER)\s+BY`.
fn is_by(normalized: &str) -> bool {
    has_words(normalized, &["GROUP", "BY"]) || has_words(normalized, &["ORDER", "BY"])
}
