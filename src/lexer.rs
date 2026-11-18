//! The SQL lexer: `sqlparse.lexer.Lexer` plus the delimiter pre-scan from
//! `sqlparse.keywords.find_delimited_spans`.
//!
//! Matching is anchored at the current offset and the first `SQL_REGEX`
//! alternative that matches wins — the order in `kgenerated.rs` is the
//! specification, so the list is never merged into one alternation.
//!
//! Seven alternatives use lookaround. Python's `re` runs them anchored in
//! microseconds, but a backtracking engine has to scan every position of the
//! remaining text for a lookbehind, which cost 240 µs per attempt here (O(n²)
//! over the file). They are therefore hand-written as matchers that mirror the
//! regex semantics, including the order in which Python's engine backtracks.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::kgenerated::{KEYWORDS, SQL_REGEX};
use crate::tokens::Tk;

/// One `(type, value)` pair, as `Lexer.get_tokens` yields them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub ttype: Tk,
    pub value: String,
}

/// Hand-written counterpart of one lookaround alternative.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Manual {
    /// `(?<!\w)[$:?]\w+`
    Placeholder,
    /// `[A-ZÀ-Ü]\w*(?=\s*\.(?!\d))`
    NameBeforeDot,
    /// `(?<=\.)[A-ZÀ-Ü]\w*`
    NameAfterDot,
    /// `[A-ZÀ-Ü]\w*(?=\()`
    NameBeforeParen,
    /// `(?![_A-ZÀ-Ü])-?(\d+(\.\d*)|\.\d+)(?![_A-ZÀ-Ü])`
    NumberFloat,
    /// `(?![_A-ZÀ-Ü])-?\d+(?![_A-ZÀ-Ü])`
    NumberInteger,
    /// `(?<![\w\])])(\[[^\]\[]+\])`
    BracketedName,
}

struct Rule {
    matcher: Matcher,
    ttype: Tk,
}

enum Matcher {
    Regex(Regex),
    Manual(Manual),
}

fn manual_for(pattern: &str) -> Option<Manual> {
    Some(match pattern {
        r"(?<!\w)[$:?]\w+" => Manual::Placeholder,
        r"[A-ZÀ-Ü]\w*(?=\s*\.(?!\d))" => Manual::NameBeforeDot,
        r"(?<=\.)[A-ZÀ-Ü]\w*" => Manual::NameAfterDot,
        r"[A-ZÀ-Ü]\w*(?=\()" => Manual::NameBeforeParen,
        r"(?![_A-ZÀ-Ü])-?(\d+(\.\d*)|\.\d+)(?![_A-ZÀ-Ü])" => Manual::NumberFloat,
        r"(?![_A-ZÀ-Ü])-?\d+(?![_A-ZÀ-Ü])" => Manual::NumberInteger,
        r"(?<![\w\])])(\[[^\]\[]+\])" => Manual::BracketedName,
        _ => return None,
    })
}

fn rules() -> &'static Vec<Rule> {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        SQL_REGEX
            .iter()
            .filter_map(|(pattern, ttype)| {
                if let Some(manual) = manual_for(pattern) {
                    return Some(Rule {
                        matcher: Matcher::Manual(manual),
                        ttype: *ttype,
                    });
                }
                // `\A` keeps a failed attempt to a prefix check.
                let anchored = format!(r"\A(?:{pattern})");
                Regex::new(&format!("(?i:{anchored})")).ok().map(|re| Rule {
                    matcher: Matcher::Regex(re),
                    ttype: *ttype,
                })
            })
            .collect()
    })
}

fn keyword_table() -> &'static HashMap<&'static str, Tk> {
    static TABLE: OnceLock<HashMap<&'static str, Tk>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::with_capacity(KEYWORDS.len());
        for (key, ttype) in KEYWORDS {
            // The generator already resolved shadowing: first table wins.
            map.entry(*key).or_insert(*ttype);
        }
        map
    })
}

/// `Lexer.is_keyword`.
fn classify_word(value: &str) -> Tk {
    let upper = value.to_uppercase();
    keyword_table()
        .get(upper.as_str())
        .copied()
        .unwrap_or(Tk::Name)
}

/// Python `\w`: `isalnum() or '_'` (Rust's `\w` is wider).
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `[A-ZÀ-Ü]` under `re.IGNORECASE`: ASCII letters plus Latin-1 uppercase
/// U+00C0..=U+00DC and their lowercase forms U+00E0..=U+00FC.
fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
        || ('\u{00C0}'..='\u{00DC}').contains(&c)
        || ('\u{00E0}'..='\u{00FC}').contains(&c)
}

fn char_at(text: &str, pos: usize) -> Option<char> {
    text.get(pos..)?.chars().next()
}

fn prev_char(text: &str, pos: usize) -> Option<char> {
    text.get(..pos)?.chars().next_back()
}

fn take_char(text: &str, pos: usize) -> Option<usize> {
    char_at(text, pos).map(|c| pos + c.len_utf8())
}

/// End of the run of characters accepted by `accept` starting at `pos`.
fn run_end(text: &str, pos: usize, accept: impl Fn(char) -> bool) -> usize {
    let mut i = pos;
    while let Some(end) = take_char(text, i) {
        match char_at(text, i) {
            Some(c) if accept(c) => i = end,
            _ => break,
        }
    }
    i
}

/// True when the character after `end` may follow a number: not `_` and not
/// in `[A-ZÀ-Ü]`.
fn number_boundary_ok(text: &str, end: usize) -> bool {
    match char_at(text, end) {
        None => true,
        Some(c) => !(c == '_' || is_latin_letter(c)),
    }
}

fn match_manual(kind: Manual, text: &str, pos: usize) -> Option<usize> {
    match kind {
        Manual::Placeholder => {
            if !matches!(char_at(text, pos)?, '$' | ':' | '?') {
                return None;
            }
            if prev_char(text, pos).is_some_and(is_word) {
                return None;
            }
            let end = run_end(text, pos + 1, is_word);
            (end > pos + 1).then_some(end)
        }
        Manual::NameBeforeDot => {
            if !is_latin_letter(char_at(text, pos)?) {
                return None;
            }
            let end = run_end(text, pos + 1, is_word);
            let mut probe = end;
            while let Some(c) = char_at(text, probe) {
                if !c.is_whitespace() {
                    break;
                }
                probe += c.len_utf8();
            }
            if char_at(text, probe) != Some('.') {
                return None;
            }
            match char_at(text, probe + 1) {
                Some(c) if c.is_ascii_digit() => None,
                _ => Some(end),
            }
        }
        Manual::NameAfterDot => {
            if prev_char(text, pos) != Some('.') || !is_latin_letter(char_at(text, pos)?) {
                return None;
            }
            Some(run_end(text, pos + 1, is_word))
        }
        Manual::NameBeforeParen => {
            if !is_latin_letter(char_at(text, pos)?) {
                return None;
            }
            let end = run_end(text, pos + 1, is_word);
            (char_at(text, end) == Some('(')).then_some(end)
        }
        Manual::NumberFloat => number_float(text, pos),
        Manual::NumberInteger => number_integer(text, pos),
        Manual::BracketedName => {
            if prev_char(text, pos).is_some_and(|c| is_word(c) || c == ']' || c == ')') {
                return None;
            }
            if char_at(text, pos) != Some('[') {
                return None;
            }
            let end = run_end(text, pos + 1, |c| c != ']' && c != '[');
            (end > pos + 1 && char_at(text, end) == Some(']')).then_some(end + 1)
        }
    }
}

/// `-?(\d+(\.\d*)|\.\d+)`: the dot is required, so a plain integer falls
/// through to the next alternative. `\d*` may shrink to zero digits, which is
/// how `1.` matches.
fn number_float(text: &str, pos: usize) -> Option<usize> {
    if !matches!(char_at(text, pos), Some(c) if !(c == '_' || is_latin_letter(c))) {
        return None;
    }
    let mut i = pos;
    if char_at(text, i) == Some('-') {
        i += 1;
    }
    let digits = run_end(text, i, |c| c.is_ascii_digit());
    if digits > i && char_at(text, digits) == Some('.') {
        let frac_start = digits + 1;
        let frac_end = run_end(text, frac_start, |c| c.is_ascii_digit());
        for frac in (0..=frac_end - frac_start).rev() {
            if number_boundary_ok(text, frac_start + frac) {
                return Some(frac_start + frac);
            }
        }
    }
    if char_at(text, i) == Some('.') {
        let frac_start = i + 1;
        let frac_end = run_end(text, frac_start, |c| c.is_ascii_digit());
        for frac in (1..=frac_end - frac_start).rev() {
            if number_boundary_ok(text, frac_start + frac) {
                return Some(frac_start + frac);
            }
        }
    }
    None
}

/// `-?\d+`, longest digits first.
fn number_integer(text: &str, pos: usize) -> Option<usize> {
    if !matches!(char_at(text, pos), Some(c) if !(c == '_' || is_latin_letter(c))) {
        return None;
    }
    let mut i = pos;
    if char_at(text, i) == Some('-') {
        i += 1;
    }
    let digits = run_end(text, i, |c| c.is_ascii_digit());
    for take in (1..=digits - i).rev() {
        if number_boundary_ok(text, i + take) {
            return Some(i + take);
        }
    }
    None
}

/// Opener positions that the lexer can resolve into one multi-char token.
#[derive(Default)]
struct Spans {
    /// byte offset of an opener -> (end offset, closer tag, token type)
    openers: HashMap<usize, (usize, Tag, Tk)>,
    /// tag -> (closer start offsets, closer end offsets), both ascending
    closers: HashMap<Tag, (Vec<usize>, Vec<usize>)>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Tag {
    Comment,
    /// Bucketed by the delimiter text, e.g. `$$` or `$tag$`.
    Dollar(String),
}

impl Spans {
    fn resolve(&self, pos: usize) -> Option<(usize, Tk)> {
        let (opener_end, tag, ttype) = self.openers.get(&pos)?.clone();
        let (starts, ends) = self.closers.get(&tag)?;
        let idx = starts.partition_point(|start| *start < opener_end);
        ends.get(idx).map(|end| (*end, ttype))
    }
}

fn find_delimited_spans(text: &str) -> Spans {
    let mut spans = Spans::default();
    if !text.contains('$') && !text.contains("/*") {
        return spans;
    }

    if text.contains('$') {
        // `$tag$` / `$$`. Every delimiter can close; one preceded by a word
        // char, `"` or `$` cannot open. Closers are bucketed by the delimiter
        // text, so `$$$$` still pairs the way Python's does.
        let mut pos = 0usize;
        while let Some(found) = text[pos..].find('$') {
            let start = pos + found;
            if let Some((end, delim)) = dollar_delimiter(text, start) {
                let tag = Tag::Dollar(delim);
                let closers = spans.closers.entry(tag.clone()).or_default();
                closers.0.push(start);
                closers.1.push(end);
                let opens = match prev_char(text, start) {
                    None => true,
                    Some(prev) => !(is_word(prev) || prev == '"' || prev == '$'),
                };
                if opens {
                    spans.openers.insert(start, (end, tag, Tk::Literal));
                }
            }
            pos = start + 1;
        }
    }

    if text.contains("/*") {
        let mut pos = 0usize;
        while let Some(found) = text[pos..].find("/*") {
            let start = pos + found;
            let opener_len = if text[start..].starts_with("/*+") {
                3
            } else {
                2
            };
            let ttype = if opener_len == 3 {
                Tk::CommentMultilineHint
            } else {
                Tk::CommentMultiline
            };
            spans
                .openers
                .insert(start, (start + opener_len, Tag::Comment, ttype));
            pos = start + 2;
        }

        let mut starts = Vec::new();
        let mut ends = Vec::new();
        pos = 0;
        while let Some(found) = text[pos..].find("*/") {
            let start = pos + found;
            starts.push(start);
            ends.push(start + 2);
            pos = start + 2;
        }
        spans.closers.insert(Tag::Comment, (starts, ends));
    }

    spans
}

/// End offset and text of a `$tag$` delimiter starting at `start`.
fn dollar_delimiter(text: &str, start: usize) -> Option<(usize, String)> {
    let mut end_of_tag = None;
    for (idx, c) in text[start + 1..].char_indices() {
        if c == '$' {
            end_of_tag = Some(start + 2 + idx);
            break;
        }
        let ok = if idx == 0 {
            c == '_' || c.is_alphabetic()
        } else {
            is_word(c)
        };
        if !ok {
            return None;
        }
    }
    let end = end_of_tag?;
    Some((end, text[start..end].to_string()))
}

/// Streaming tokenizer: yields `(type, start, end)` byte ranges in the input.
///
/// The collection form `tokenize` is a thin wrapper; the pipeline uses this
/// cursor so that peak memory stays at one statement instead of the whole file.
pub struct Lexer<'a> {
    text: &'a str,
    spans: Spans,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(text: &'a str) -> Lexer<'a> {
        Lexer {
            text,
            spans: find_delimited_spans(text),
            pos: 0,
        }
    }

    /// Next token as a byte range, or `None` at end of input.
    pub fn next_token(&mut self) -> Option<(Tk, usize, usize)> {
        if self.pos >= self.text.len() {
            return None;
        }
        let text = self.text;

        if let Some((end, ttype)) = self.spans.resolve(self.pos) {
            let start = self.pos;
            self.pos = end;
            return Some((ttype, start, end));
        }

        for rule in rules().iter() {
            let end = match &rule.matcher {
                Matcher::Regex(re) => re
                    .find(&text[self.pos..])
                    .map(|m| self.pos + m.end())
                    .filter(|end| *end > self.pos),
                Matcher::Manual(kind) => match_manual(*kind, text, self.pos),
            };
            let Some(end) = end else { continue };
            let start = self.pos;
            let value = &text[start..end];
            let ttype = if rule.ttype == Tk::KeywordLookup {
                classify_word(value)
            } else {
                rule.ttype
            };
            self.pos = end;
            return Some((ttype, start, end));
        }

        // No alternative matched: sqlparse yields `Error` for one character.
        let ch_len = char_at(text, self.pos).map(char::len_utf8).unwrap_or(1);
        let start = self.pos;
        self.pos += ch_len;
        Some((Tk::Error, start, self.pos))
    }
}

/// Tokenize `text` exactly like `sqlparse.lexer.tokenize`.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(text);
    let mut out = Vec::new();
    while let Some((ttype, start, end)) = lexer.next_token() {
        out.push(Token {
            ttype,
            value: text[start..end].to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(sql: &str) -> Vec<(Tk, String)> {
        tokenize(sql)
            .into_iter()
            .map(|t| (t.ttype, t.value))
            .collect()
    }

    #[test]
    fn select_is_dml() {
        let toks = kinds("select 1");
        assert_eq!(toks[0].0, Tk::KeywordDml);
        assert_eq!(toks[0].1, "select");
        assert_eq!(toks[1].0, Tk::Whitespace);
        assert_eq!(toks[2].0, Tk::NumberInteger);
        assert_eq!(toks[2].1, "1");
    }

    #[test]
    fn newline_and_comment() {
        let toks = kinds("-- x\nselect 1");
        assert_eq!(toks[0].0, Tk::CommentSingle);
        assert_eq!(toks[0].1, "-- x\n");
    }

    #[test]
    fn multiline_comment_is_one_token() {
        let toks = kinds("a /* x\ny */ b");
        assert_eq!(toks[2].0, Tk::CommentMultiline);
        assert_eq!(toks[2].1, "/* x\ny */");
    }

    #[test]
    fn dollar_quoted_body_is_one_token() {
        let toks = kinds("select $tag$ a ; b $tag$");
        assert_eq!(toks[2].0, Tk::Literal);
        assert_eq!(toks[2].1, "$tag$ a ; b $tag$");
    }

    #[test]
    fn quoted_string_wins_over_lookalike_delimiter() {
        let toks = kinds("select '--not a comment'");
        assert_eq!(toks[2].0, Tk::StringSingle);
    }

    #[test]
    fn placeholder_after_word_is_not_one() {
        // `(?<!\w)` keeps `$1` inside `a$1` from starting a placeholder.
        let toks = kinds("a$1");
        assert!(!toks.iter().any(|t| t.0 == Tk::NamePlaceholder));
        assert_eq!(toks[0], (Tk::Name, "a$1".to_string()));
        let toks = kinds("$1");
        assert_eq!(toks[0].0, Tk::NamePlaceholder);
    }

    #[test]
    fn number_splits_before_letter() {
        // The trailing `(?![_A-ZÀ-Ü])` makes the integer give up its last digit.
        let toks = kinds("123X");
        assert_eq!(toks[0], (Tk::NumberInteger, "12".to_string()));
        assert_eq!(toks[1], (Tk::Name, "3X".to_string()));
    }

    #[test]
    fn function_call_name_needs_paren() {
        let toks = kinds("count(1)");
        assert_eq!(toks[0].0, Tk::Name);
        let toks = kinds("count 1");
        assert_eq!(toks[0].0, Tk::Keyword);
    }
}
