//! The formatting pipeline: `sqlparse.format()` and the `sqlformat` CLI body.

use std::fmt;

use crate::filters::{self, ReindentFilter, StripWhitespace};
use crate::group;
use crate::lexer::{Lexer, Token};
use crate::splitter::Splitter;
use crate::tokens::Tk;
use crate::tree::Tree;

/// CLI/formatter options in the subset this port implements.
#[derive(Clone, Debug)]
pub struct FormatOptions {
    pub reindent: bool,
    pub keyword_case: Option<String>,
    pub identifier_case: Option<String>,
    pub indent_width: usize,
    /// `--wrap_after`: wrap identifier lists once a line passes this column.
    pub wrap_after: i64,
    /// `--comma_first`: break before the comma instead of after the item.
    pub comma_first: bool,
    /// `--compact`: skip the line breaks `CASE` would otherwise get.
    pub compact: bool,
    /// `--strip-comments`: drop comments, keeping their trailing line breaks.
    pub strip_comments: bool,
}

impl Default for FormatOptions {
    fn default() -> FormatOptions {
        FormatOptions {
            reindent: false,
            keyword_case: None,
            identifier_case: None,
            indent_width: 2,
            wrap_after: 0,
            comma_first: false,
            compact: false,
            strip_comments: false,
        }
    }
}

#[derive(Debug)]
pub enum PortError {
    Group(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortError::Group(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PortError {}

impl From<group::GroupError> for PortError {
    fn from(error: group::GroupError) -> PortError {
        PortError::Group(error.0)
    }
}

/// `sqlparse.format(sql, **options)` for the supported option subset.
///
/// `-r` implies `strip_whitespace`, exactly as `validate_options` does; the
/// case filters run on the raw token stream before splitting.
///
/// The pipeline is streamed: one statement is lexed, grouped, stripped,
/// reindented and serialized before the next one is read, so peak memory stays
/// near the input size plus a single statement.
pub fn format_sql(sql: &str, options: &FormatOptions) -> Result<String, PortError> {
    let mut lexer = Lexer::new(sql);
    let mut splitter = Splitter::default();
    let mut strip = StripWhitespace;
    let mut reindent = ReindentFilter::new(options.indent_width);
    reindent.wrap_after = options.wrap_after;
    reindent.comma_first = options.comma_first;
    reindent.compact = options.compact;
    let mut out = String::with_capacity(sql.len() + sql.len() / 4);

    while let Some((ttype, start, end)) = lexer.next_token() {
        let mut value = sql[start..end].to_string();
        if let Some(case) = options.keyword_case.as_deref() {
            if ttype.is_keyword() {
                value = filters::transform(&value, case);
            }
        }
        if let Some(case) = options.identifier_case.as_deref() {
            if matches!(ttype, Tk::Name | Tk::StringSymbol) && !value.trim_start().starts_with('"')
            {
                value = filters::transform(&value, case);
            }
        }
        if let Some(statement) = splitter.feed(ttype, &value) {
            emit(statement, options, &mut strip, &mut reindent, &mut out)?;
        }
    }
    if let Some(statement) = splitter.finish() {
        emit(statement, options, &mut strip, &mut reindent, &mut out)?;
    }
    Ok(out)
}

/// `StripCommentsFilter._get_insert_token`: the comment's trailing run of
/// `\r`/`\n` characters (`re.search(r'([\r\n]+) *$', value)`), else one space.
fn comment_placeholder(comment: &str) -> String {
    let body = comment.trim_end_matches(' ');
    let mut start = body.len();
    for (index, ch) in body.char_indices().rev() {
        if ch == '\r' || ch == '\n' {
            start = index;
        } else {
            break;
        }
    }
    if start < body.len() {
        body[start..].to_string()
    } else {
        " ".to_string()
    }
}

/// Drop comment tokens the way `StripCommentsFilter` does, before grouping.
///
/// Comments are grouped first, exactly like `sqlparse.engine.grouping.
/// group_comments`: a comment absorbs the newline-only whitespace (and any
/// further comment) that follows it, so the line breaks of a removed comment
/// go with it. A group is dropped outright when nothing precedes it or the
/// previous token is `(`; otherwise it becomes one whitespace token
/// (`comment_placeholder`). SQL hints (`/*+ … */`, `--+ …`) are kept.
fn strip_comments(tokens: Vec<Token>) -> Vec<Token> {
    let is_hint = |ttype: Tk| matches!(ttype, Tk::CommentSingleHint | Tk::CommentMultilineHint);
    let is_newline = |token: &Token| {
        token.ttype == Tk::Newline
            && !token.value.is_empty()
            && token.value.chars().all(|c| c == '\r' || c == '\n')
    };
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if !token.ttype.is_comment() || is_hint(token.ttype) {
            out.push(tokens[index].clone());
            index += 1;
            continue;
        }
        let mut end = index;
        let mut text = String::new();
        while end < tokens.len() {
            let current = &tokens[end];
            if !current.ttype.is_comment() && !is_newline(current) {
                break;
            }
            text.push_str(&current.value);
            end += 1;
        }
        if !is_hint(token.ttype) {
            let keep_placeholder = match out.last() {
                None => false,
                Some(previous) => previous.value != "(",
            };
            if keep_placeholder {
                out.push(Token {
                    ttype: Tk::Whitespace,
                    value: comment_placeholder(&text),
                });
            }
        }
        index = end;
    }
    out
}

fn emit(
    tokens: Vec<Token>,
    options: &FormatOptions,
    strip: &mut StripWhitespace,
    reindent: &mut ReindentFilter,
    out: &mut String,
) -> Result<(), PortError> {
    let tokens = if options.strip_comments {
        strip_comments(tokens)
    } else {
        tokens
    };
    let mut tree = Tree::from_tokens(tokens);
    let root = tree.root;
    if options.reindent {
        group::group(&mut tree)?;
        strip.process(&mut tree, root);
        reindent.process(&mut tree, root);
    }
    out.push_str(&filters::serialize(&tree, root));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ident_case_only() {
        let options = FormatOptions {
            identifier_case: Some("upper".to_string()),
            ..FormatOptions::default()
        };
        let out = format_sql("select id, name from users", &options).unwrap();
        assert_eq!(out, "select ID, NAME from USERS");
    }

    #[test]
    fn keyword_case_only() {
        let options = FormatOptions {
            keyword_case: Some("lower".to_string()),
            ..FormatOptions::default()
        };
        let out = format_sql("SELECT a FROM t", &options).unwrap();
        assert_eq!(out, "select a from t");
    }
}
