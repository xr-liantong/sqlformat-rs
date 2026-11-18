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
}

impl Default for FormatOptions {
    fn default() -> FormatOptions {
        FormatOptions {
            reindent: false,
            keyword_case: None,
            identifier_case: None,
            indent_width: 2,
        }
    }
}

#[derive(Debug)]
pub enum PortError {
    Group(&'static str),
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

fn emit(
    tokens: Vec<Token>,
    options: &FormatOptions,
    strip: &mut StripWhitespace,
    reindent: &mut ReindentFilter,
    out: &mut String,
) -> Result<(), PortError> {
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
