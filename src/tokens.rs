//! Token types of the SQL lexer, mirroring `sqlparse.tokens`.
//!
//! sqlparse builds its types as a tree (`Token.Keyword.DML` is a child of
//! `Token.Keyword`); the ports we need are "is this token inside subtree X",
//! so the enum keeps one flat variant per leaf and answers the subtree
//! questions with match arms.

/// One leaf token type. `KeywordLookup` is not a type: it marks a
/// `SQL_REGEX` alternative whose match still has to be looked up in the
/// keyword table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tk {
    Assignment,
    CommentSingle,
    CommentSingleHint,
    CommentMultiline,
    CommentMultilineHint,
    GenericCommand,
    Keyword,
    KeywordCte,
    KeywordDcl,
    KeywordDdl,
    KeywordDml,
    KeywordOrder,
    KeywordTzCast,
    Literal,
    NumberFloat,
    NumberHex,
    NumberInteger,
    StringSingle,
    StringSymbol,
    Name,
    NameBuiltin,
    NamePlaceholder,
    Operator,
    OperatorComparison,
    Punctuation,
    Whitespace,
    Newline,
    Wildcard,
    Error,
    Other,
    /// Marker: the match must be classified through the keyword table.
    KeywordLookup,
}

impl Tk {
    /// `ttype in T.Keyword`.
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            Tk::Keyword
                | Tk::KeywordCte
                | Tk::KeywordDcl
                | Tk::KeywordDdl
                | Tk::KeywordDml
                | Tk::KeywordOrder
                | Tk::KeywordTzCast
        )
    }

    /// `ttype in T.Text.Whitespace`.
    pub fn is_whitespace(self) -> bool {
        matches!(self, Tk::Whitespace | Tk::Newline)
    }

    /// `ttype in T.Whitespace.Newline`.
    pub fn is_newline(self) -> bool {
        matches!(self, Tk::Newline)
    }

    /// `ttype in T.Comment`.
    pub fn is_comment(self) -> bool {
        matches!(
            self,
            Tk::CommentSingle
                | Tk::CommentSingleHint
                | Tk::CommentMultiline
                | Tk::CommentMultilineHint
        )
    }

    /// `ttype in T.Name`.
    pub fn in_name(self) -> bool {
        matches!(self, Tk::Name | Tk::NameBuiltin | Tk::NamePlaceholder)
    }

    /// `ttype in T.Literal.Number`.
    pub fn is_number(self) -> bool {
        matches!(self, Tk::NumberFloat | Tk::NumberHex | Tk::NumberInteger)
    }

    /// `ttype in T.Literal.String`.
    pub fn is_string(self) -> bool {
        matches!(self, Tk::StringSingle | Tk::StringSymbol)
    }

    /// `ttype in T.Operator`.
    pub fn in_operator(self) -> bool {
        matches!(self, Tk::Operator | Tk::OperatorComparison)
    }

    /// `ttype in parent`: is this type the given one or a child of it?
    pub fn is_a(self, parent: Tk) -> bool {
        if self == parent {
            return true;
        }
        match parent {
            Tk::Keyword => self.is_keyword(),
            Tk::Name => self.in_name(),
            Tk::NumberInteger => false,
            Tk::Literal => self.is_number() || self.is_string() || self == Tk::Literal,
            Tk::CommentSingle => self.is_comment(),
            Tk::CommentMultiline => self.is_comment(),
            Tk::Whitespace => self.is_whitespace(),
            Tk::Newline => self.is_newline(),
            Tk::Operator => self.in_operator(),
            _ => false,
        }
    }

    /// Turn a produced token type back into a `Keyword` leaf when the value
    /// came out of a keyword lookup that matched nothing.
    pub fn resolve_lookup(self) -> Tk {
        match self {
            Tk::KeywordLookup => Tk::Name,
            other => other,
        }
    }

    /// Stable name used by the token dumps the parity tests compare.
    pub fn name(self) -> &'static str {
        match self {
            Tk::Assignment => "Token.Assignment",
            Tk::CommentSingle => "Token.Comment.Single",
            Tk::CommentSingleHint => "Token.Comment.Single.Hint",
            Tk::CommentMultiline => "Token.Comment.Multiline",
            Tk::CommentMultilineHint => "Token.Comment.Multiline.Hint",
            Tk::GenericCommand => "Token.Generic.Command",
            Tk::Keyword => "Token.Keyword",
            Tk::KeywordCte => "Token.Keyword.CTE",
            Tk::KeywordDcl => "Token.Keyword.DCL",
            Tk::KeywordDdl => "Token.Keyword.DDL",
            Tk::KeywordDml => "Token.Keyword.DML",
            Tk::KeywordOrder => "Token.Keyword.Order",
            Tk::KeywordTzCast => "Token.Keyword.TZCast",
            Tk::Literal => "Token.Literal",
            Tk::NumberFloat => "Token.Literal.Number.Float",
            Tk::NumberHex => "Token.Literal.Number.Hexadecimal",
            Tk::NumberInteger => "Token.Literal.Number.Integer",
            Tk::StringSingle => "Token.Literal.String.Single",
            Tk::StringSymbol => "Token.Literal.String.Symbol",
            Tk::Name => "Token.Name",
            Tk::NameBuiltin => "Token.Name.Builtin",
            Tk::NamePlaceholder => "Token.Name.Placeholder",
            Tk::Operator => "Token.Operator",
            Tk::OperatorComparison => "Token.Operator.Comparison",
            Tk::Punctuation => "Token.Punctuation",
            Tk::Whitespace => "Token.Text.Whitespace",
            Tk::Newline => "Token.Text.Whitespace.Newline",
            Tk::Wildcard => "Token.Wildcard",
            Tk::Error => "Token.Error",
            Tk::Other => "Token.Other",
            Tk::KeywordLookup => "PROCESS_AS_KEYWORD",
        }
    }
}
