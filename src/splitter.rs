//! Port of `sqlparse/engine/statement_splitter.py`.
//!
//! The splitter tracks nesting so that a `;` inside `BEGIN ... END` or inside
//! parentheses does not end a statement; the state machine below is a literal
//! transcription.

use crate::lexer::Token;
use crate::tokens::Tk;
use crate::tree::Tree;

#[derive(Default)]
pub struct Splitter {
    block_stack: Vec<String>,
    parenthesis_level: i64,
    unconfirmed_start: Option<String>,
    is_create: bool,
    seen_begin: bool,
    consume_ws: bool,
    level: i64,
    tokens: Vec<Token>,
}

impl Splitter {
    fn reset(&mut self) {
        *self = Splitter::default();
    }

    fn top_is(&self, name: &str) -> bool {
        self.block_stack.last().is_some_and(|top| top == name)
    }

    fn handle_nested_block(&mut self, unified: &str) -> Option<i64> {
        if unified == "FOR" {
            self.unconfirmed_start = Some("FOR".to_string());
            return Some(0);
        }
        if unified == "WHILE" {
            self.unconfirmed_start = Some("WHILE".to_string());
            return Some(0);
        }
        if unified == "LOOP" || unified == "DO" {
            if matches!(
                self.unconfirmed_start.as_deref(),
                Some("FOR") | Some("WHILE")
            ) {
                let started = self.unconfirmed_start.take().unwrap();
                self.block_stack.push(started);
                return Some(1);
            }
            if unified == "LOOP" {
                self.block_stack.push("LOOP".to_string());
                return Some(1);
            }
        }
        if unified == "IF" || unified == "CASE" {
            self.block_stack.push(unified.to_string());
            return Some(1);
        }
        None
    }

    fn handle_closing_keyword(&mut self, unified: &str) -> i64 {
        match unified {
            "END IF" => {
                if self.top_is("IF") {
                    self.block_stack.pop();
                    return -1;
                }
            }
            "END FOR" => {
                if self.top_is("FOR") {
                    self.block_stack.pop();
                    return -1;
                }
            }
            "END WHILE" => {
                if self.top_is("WHILE") {
                    self.block_stack.pop();
                    return -1;
                }
            }
            "END LOOP" => {
                if matches!(
                    self.block_stack.last().map(String::as_str),
                    Some("LOOP") | Some("FOR") | Some("WHILE")
                ) {
                    self.block_stack.pop();
                    return -1;
                }
            }
            "END CASE" => {
                if self.top_is("CASE") {
                    self.block_stack.pop();
                    return -1;
                }
            }
            "END" => {
                if !self.block_stack.is_empty() {
                    self.block_stack.pop();
                }
                return -1;
            }
            _ => {}
        }
        0
    }

    /// `_change_splitlevel`.
    fn change_splitlevel(&mut self, ttype: Tk, value: &str) -> i64 {
        if ttype == Tk::Punctuation && value == ";" {
            self.unconfirmed_start = None;
            if self.seen_begin {
                self.seen_begin = false;
                if self.top_is("BEGIN") {
                    self.block_stack.pop();
                    return -1;
                }
            }
            return 0;
        }
        if ttype == Tk::Punctuation && value == "(" {
            self.parenthesis_level += 1;
            return 1;
        }
        if ttype == Tk::Punctuation && value == ")" {
            self.parenthesis_level = (self.parenthesis_level - 1).max(0);
            return -1;
        }
        if !ttype.is_keyword() {
            return 0;
        }

        let unified = value.to_uppercase();

        if ttype == Tk::KeywordDdl && unified.starts_with("CREATE") {
            self.is_create = true;
            return 0;
        }
        if unified == "DECLARE" && self.is_create && self.block_stack.is_empty() {
            self.block_stack.push("DECLARE".to_string());
            return 1;
        }
        if unified == "BEGIN" {
            self.seen_begin = true;
            if self.top_is("DECLARE") {
                self.block_stack.pop();
                self.block_stack.push("BEGIN".to_string());
                return 0;
            }
            self.block_stack.push("BEGIN".to_string());
            return 1;
        }
        if self.seen_begin
            && (ttype == Tk::Keyword || ttype == Tk::Name)
            && matches!(
                unified.as_str(),
                "TRANSACTION"
                    | "WORK"
                    | "TRAN"
                    | "DISTRIBUTED"
                    | "DEFERRED"
                    | "IMMEDIATE"
                    | "EXCLUSIVE"
            )
        {
            self.seen_begin = false;
            if self.top_is("BEGIN") {
                self.block_stack.pop();
                return -1;
            }
            return 0;
        }
        if self.block_stack.iter().any(|block| block == "BEGIN") {
            if let Some(result) = self.handle_nested_block(&unified) {
                return result;
            }
        }
        self.handle_closing_keyword(&unified)
    }

    /// Feed one token; returns a complete statement when the splitter cuts one.
    pub fn feed(&mut self, ttype: Tk, value: &str) -> Option<Vec<Token>> {
        let is_eos = ttype == Tk::Whitespace || ttype == Tk::CommentSingle;
        let mut finished = None;
        if self.consume_ws && !is_eos {
            finished = Some(std::mem::take(&mut self.tokens));
            self.reset();
        }

        self.level += self.change_splitlevel(ttype, value);

        let is_semicolon = ttype == Tk::Punctuation && value == ";";
        let is_go = ttype == Tk::Keyword && value.split(' ').next() == Some("GO");
        let is_update = ttype == Tk::Keyword && value.to_uppercase() == "BEGIN";

        self.tokens.push(Token {
            ttype,
            value: value.to_string(),
        });

        if is_semicolon {
            self.seen_begin = false;
            if self.level <= 0 && !self.block_stack.iter().any(|block| block == "BEGIN") {
                self.consume_ws = true;
            }
        } else if is_go {
            self.consume_ws = true;
        } else if !matches!(
            self.tokens.last().map(|token| token.ttype),
            Some(Tk::Whitespace)
                | Some(Tk::Newline)
                | Some(Tk::CommentSingle)
                | Some(Tk::CommentMultiline)
        ) && !is_update
        {
            self.seen_begin = false;
        }
        finished
    }

    /// The trailing statement, if it holds anything but whitespace.
    pub fn finish(&mut self) -> Option<Vec<Token>> {
        if !self.tokens.is_empty() && self.tokens.iter().any(|token| !token.ttype.is_whitespace()) {
            Some(std::mem::take(&mut self.tokens))
        } else {
            self.tokens.clear();
            None
        }
    }
}

/// Split a token stream into statements.
pub fn split(tokens: Vec<Token>) -> Vec<Tree> {
    let mut splitter = Splitter::default();
    let mut statements: Vec<Vec<Token>> = Vec::new();
    for token in tokens {
        if let Some(statement) = splitter.feed(token.ttype, &token.value) {
            statements.push(statement);
        }
    }
    if let Some(statement) = splitter.finish() {
        statements.push(statement);
    }
    statements.into_iter().map(Tree::from_tokens).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn splits_on_semicolon() {
        let trees = split(tokenize("select 1; select 2;"));
        assert_eq!(trees.len(), 2);
    }

    #[test]
    fn keeps_begin_end_together() {
        let trees = split(tokenize("BEGIN select 1; END;"));
        assert_eq!(trees.len(), 1);
    }

    #[test]
    fn ignores_trailing_whitespace() {
        let trees = split(tokenize("select 1;\n\n"));
        assert_eq!(trees.len(), 1);
    }
}
