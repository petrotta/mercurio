use crate::language_contracts::ast::SourceSpan;
use crate::language_contracts::diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Package,
    Import,
    Part,
    Def,
    Specializes,
    Redefines,
    Doc(String),
    BlockDoc(String),
    Identifier(String),
    Number(String),
    String(String),
    Star,
    DoubleStar,
    Caret,
    LBrace,
    RBrace,
    LAngle,
    RAngle,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Colon,
    ScopeSep,
    Dot,
    Tilde,
    Equals,
    DoubleEquals,
    BangEquals,
    Plus,
    Minus,
    LessEqual,
    GreaterEqual,
    Slash,
    Bang,
    Ampersand,
    Pipe,
    Dollar,
    At,
    Hash,
    Question,
    Semicolon,
    Comma,
    Eof,
}

pub fn lex(input: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer::new(input);
    lexer.lex_all()
}

struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    index: usize,
    line: usize,
    col: usize,
    comment_doc_candidate: bool,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            index: 0,
            line: 1,
            col: 1,
            comment_doc_candidate: false,
        }
    }

    fn lex_all(&mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments()?;
            let start_line = self.line;
            let start_col = self.col;
            let Some(ch) = self.peek_char() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: SourceSpan {
                        start_line,
                        start_col,
                        end_line: start_line,
                        end_col: start_col,
                    },
                });
                return Ok(tokens);
            };

            let kind = match ch {
                '{' => {
                    self.advance_char();
                    TokenKind::LBrace
                }
                '}' => {
                    self.advance_char();
                    TokenKind::RBrace
                }
                '<' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::LessEqual
                    } else {
                        TokenKind::LAngle
                    }
                }
                '>' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::GreaterEqual
                    } else {
                        TokenKind::RAngle
                    }
                }
                '[' => {
                    self.advance_char();
                    TokenKind::LBracket
                }
                ']' => {
                    self.advance_char();
                    TokenKind::RBracket
                }
                '(' => {
                    self.advance_char();
                    TokenKind::LParen
                }
                ')' => {
                    self.advance_char();
                    TokenKind::RParen
                }
                ':' => {
                    self.advance_char();
                    if self.peek_char() == Some(':') {
                        self.advance_char();
                        TokenKind::ScopeSep
                    } else if self.peek_char() == Some('>') {
                        self.advance_char();
                        if self.peek_char() == Some('>') {
                            self.advance_char();
                            TokenKind::Redefines
                        } else {
                            TokenKind::Specializes
                        }
                    } else {
                        TokenKind::Colon
                    }
                }
                '*' => {
                    self.advance_char();
                    if self.peek_char() == Some('*') {
                        self.advance_char();
                        TokenKind::DoubleStar
                    } else {
                        TokenKind::Star
                    }
                }
                ';' => {
                    self.advance_char();
                    TokenKind::Semicolon
                }
                ',' => {
                    self.advance_char();
                    TokenKind::Comma
                }
                '.' => {
                    if self
                        .peek_next_char()
                        .is_some_and(|next| next.is_ascii_digit())
                    {
                        TokenKind::Number(self.lex_fractional_number())
                    } else {
                        self.advance_char();
                        TokenKind::Dot
                    }
                }
                '~' => {
                    self.advance_char();
                    TokenKind::Tilde
                }
                '=' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::DoubleEquals
                    } else {
                        TokenKind::Equals
                    }
                }
                '+' => {
                    self.advance_char();
                    TokenKind::Plus
                }
                '^' => {
                    self.advance_char();
                    TokenKind::Caret
                }
                '-' => {
                    self.advance_char();
                    TokenKind::Minus
                }
                '/' if self.peek_next_char() == Some('*') && self.comment_doc_candidate => {
                    TokenKind::BlockDoc(self.consume_doc_block()?)
                }
                '/' => {
                    self.advance_char();
                    TokenKind::Slash
                }
                '!' => {
                    self.advance_char();
                    if self.peek_char() == Some('=') {
                        self.advance_char();
                        TokenKind::BangEquals
                    } else {
                        TokenKind::Bang
                    }
                }
                '&' => {
                    self.advance_char();
                    TokenKind::Ampersand
                }
                '|' => {
                    self.advance_char();
                    TokenKind::Pipe
                }
                '$' => {
                    self.advance_char();
                    TokenKind::Dollar
                }
                '@' => {
                    self.advance_char();
                    TokenKind::At
                }
                '#' => {
                    self.advance_char();
                    TokenKind::Hash
                }
                '?' => {
                    self.advance_char();
                    TokenKind::Question
                }
                '\'' => TokenKind::Identifier(self.lex_quoted_identifier()?),
                '"' => TokenKind::String(self.lex_string_literal()?),
                _ if ch.is_ascii_digit() => TokenKind::Number(self.lex_number()),
                _ if is_ident_start(ch) => self.lex_identifier_or_keyword()?,
                _ => {
                    return Err(Diagnostic::new(
                        format!("unexpected character `{ch}`"),
                        Some(SourceSpan {
                            start_line,
                            start_col,
                            end_line: start_line,
                            end_col: start_col,
                        }),
                    ));
                }
            };

            self.update_comment_doc_candidate(&kind);
            tokens.push(Token {
                kind,
                span: SourceSpan {
                    start_line,
                    start_col,
                    end_line: self.line,
                    end_col: self.col.saturating_sub(1),
                },
            });
        }
    }

    fn lex_number(&mut self) -> String {
        let start = self.index;
        let mut saw_decimal = false;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.advance_char();
            } else if ch == '.'
                && !saw_decimal
                && self
                    .peek_next_char()
                    .is_some_and(|next| next.is_ascii_digit())
            {
                saw_decimal = true;
                self.advance_char();
            } else {
                break;
            }
        }
        self.input[start..self.index].to_string()
    }

    fn lex_fractional_number(&mut self) -> String {
        let start = self.index;
        self.advance_char();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.advance_char();
            } else {
                break;
            }
        }
        self.input[start..self.index].to_string()
    }

    fn lex_quoted_identifier(&mut self) -> Result<String, Diagnostic> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance_char();
        let start = self.index;

        while let Some(ch) = self.peek_char() {
            if ch == '\'' {
                let ident = self.input[start..self.index].to_string();
                self.advance_char();
                return Ok(ident);
            }
            self.advance_char();
        }

        Err(Diagnostic::new(
            "unterminated quoted identifier",
            Some(SourceSpan {
                start_line,
                start_col,
                end_line: self.line,
                end_col: self.col,
            }),
        ))
    }

    fn lex_string_literal(&mut self) -> Result<String, Diagnostic> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance_char();
        let start = self.index;

        while let Some(ch) = self.peek_char() {
            if ch == '"' {
                let value = self.input[start..self.index].to_string();
                self.advance_char();
                return Ok(value);
            }
            self.advance_char();
        }

        Err(Diagnostic::new(
            "unterminated string literal",
            Some(SourceSpan {
                start_line,
                start_col,
                end_line: self.line,
                end_col: self.col,
            }),
        ))
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.peek_char() {
                Some(ch) if ch.is_whitespace() => {
                    self.advance_char();
                }
                Some('/')
                    if self.peek_next_char() == Some('/')
                        && self.bytes.get(self.index + 2) == Some(&b'*') =>
                {
                    self.consume_line_prefixed_block_comment()?;
                }
                Some('/') if self.peek_next_char() == Some('/') => {
                    self.advance_char();
                    self.advance_char();
                    while let Some(ch) = self.peek_char() {
                        self.advance_char();
                        if ch == '\n' {
                            break;
                        }
                    }
                }
                Some('/') if self.peek_next_char() == Some('*') && self.comment_doc_candidate => {
                    return Ok(());
                }
                Some('/') if self.peek_next_char() == Some('*') => {
                    self.consume_block_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn lex_identifier_or_keyword(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.index;
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.advance_char();
            } else {
                break;
            }
        }

        let ident = &self.input[start..self.index];
        match ident {
            "package" => Ok(TokenKind::Package),
            "import" => Ok(TokenKind::Import),
            "part" => Ok(TokenKind::Part),
            "def" => Ok(TokenKind::Def),
            "specializes" => Ok(TokenKind::Specializes),
            "doc" => {
                self.skip_plain_whitespace();
                if self.peek_char().is_some_and(is_ident_start) {
                    while let Some(ch) = self.peek_char() {
                        if is_ident_continue(ch) {
                            self.advance_char();
                        } else {
                            break;
                        }
                    }
                    self.skip_plain_whitespace();
                }
                if self.peek_char() == Some('<') {
                    self.advance_char();
                    while let Some(ch) = self.peek_char() {
                        self.advance_char();
                        if ch == '>' {
                            break;
                        }
                    }
                    self.skip_plain_whitespace();
                }
                if self.peek_char() == Some('"') {
                    let _ = self.lex_string_literal()?;
                    self.skip_plain_whitespace();
                }
                let body = self.consume_doc_block()?;
                Ok(TokenKind::Doc(body))
            }
            _ => Ok(TokenKind::Identifier(ident.to_string())),
        }
    }

    fn consume_doc_block(&mut self) -> Result<String, Diagnostic> {
        if self.peek_char() != Some('/') || self.peek_next_char() != Some('*') {
            return Err(Diagnostic::new(
                "expected block comment after `doc`",
                Some(SourceSpan {
                    start_line: self.line,
                    start_col: self.col,
                    end_line: self.line,
                    end_col: self.col,
                }),
            ));
        }

        self.advance_char();
        self.advance_char();
        let start = self.index;

        while let Some(ch) = self.peek_char() {
            if ch == '*' && self.peek_next_char() == Some('/') {
                let body = self.input[start..self.index].trim().to_string();
                self.advance_char();
                self.advance_char();
                return Ok(body);
            }
            self.advance_char();
        }

        Err(Diagnostic::new("unterminated doc block", None))
    }

    fn update_comment_doc_candidate(&mut self, kind: &TokenKind) {
        match kind {
            TokenKind::Identifier(value) if value == "comment" => {
                self.comment_doc_candidate = true;
            }
            TokenKind::Doc(_)
            | TokenKind::BlockDoc(_)
            | TokenKind::Semicolon
            | TokenKind::LBrace
            | TokenKind::RBrace
            | TokenKind::Eof
            | TokenKind::Package
            | TokenKind::Import
            | TokenKind::Part
            | TokenKind::At
            | TokenKind::Hash => {
                self.comment_doc_candidate = false;
            }
            _ => {}
        }
    }

    fn skip_plain_whitespace(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.advance_char();
        }
    }

    fn consume_block_comment(&mut self) -> Result<(), Diagnostic> {
        self.advance_char();
        self.advance_char();

        while let Some(ch) = self.peek_char() {
            if ch == '*' && self.peek_next_char() == Some('/') {
                self.advance_char();
                self.advance_char();
                return Ok(());
            }
            self.advance_char();
        }

        Err(Diagnostic::new("unterminated block comment", None))
    }

    fn consume_line_prefixed_block_comment(&mut self) -> Result<(), Diagnostic> {
        self.advance_char();
        self.advance_char();
        self.advance_char();

        while let Some(ch) = self.peek_char() {
            if ch == '*' && self.peek_next_char() == Some('/') {
                self.advance_char();
                self.advance_char();
                return Ok(());
            }
            self.advance_char();
        }

        Err(Diagnostic::new("unterminated block comment", None))
    }

    fn peek_char(&self) -> Option<char> {
        self.bytes.get(self.index).map(|byte| *byte as char)
    }

    fn peek_next_char(&self) -> Option<char> {
        self.bytes.get(self.index + 1).map(|byte| *byte as char)
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.index += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, lex};

    #[test]
    fn lexes_minimal_model_subset() {
        let tokens = lex(
            "package Demo { doc /* hi */ part def Vehicle specializes Model::Systems::PartDefinition { part engine: Engine; } }",
        )
        .unwrap();

        assert!(matches!(tokens[0].kind, TokenKind::Package));
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Doc(_)))
        );
        assert!(tokens.iter().any(
            |token| matches!(&token.kind, TokenKind::Identifier(value) if value == "Vehicle")
        ));
    }

    #[test]
    fn lexes_quoted_identifiers_and_wildcards() {
        let tokens =
            lex("package 'Port Example' { import ScalarValues::*; import Pkg::*::**; }").unwrap();

        assert!(tokens.iter().any(
            |token| matches!(&token.kind, TokenKind::Identifier(value) if value == "Port Example")
        ));
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Star))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::DoubleStar))
        );
    }

    #[test]
    fn lexes_specialization_shorthand() {
        let tokens = lex("part def Engine :> Vehicle;").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Specializes))
        );
    }

    #[test]
    fn lexes_loose_surface_syntax_used_in_pilot_examples() {
        let tokens = lex("part <'1'> b[0..2]: ~C = X::y(0) { part x; }").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::LAngle))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::RAngle))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::LBracket))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::RBracket))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::LParen))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::RParen))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Dot))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Tilde))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Equals))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(&token.kind, TokenKind::Number(value) if value == "0"))
        );
    }

    #[test]
    fn lexes_double_gt_relation_shorthand() {
        let tokens = lex("part x :>> y;").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Redefines))
        );
    }

    #[test]
    fn lexes_arithmetic_and_annotation_surface_syntax() {
        let tokens =
            lex("calc def Power { return : Value = a + b - c / d & e; @tag #note }").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Plus))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Minus))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Slash))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Ampersand))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::At))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Hash))
        );
    }

    #[test]
    fn lexes_expression_operator_tokens() {
        let tokens =
            lex("attribute x = (1 + 2) * 3 >= 4 and not false == !false != true;").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Star))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::GreaterEqual))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::DoubleEquals))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::BangEquals))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Bang))
        );
    }

    #[test]
    fn lexes_named_doc_and_string_literals() {
        let tokens = lex("doc Document1 /* hi */ attribute code = \"uncl\"; opaque ?;").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(&token.kind, TokenKind::Doc(value) if value == "hi"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(&token.kind, TokenKind::String(value) if value == "uncl"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Question))
        );
    }

    #[test]
    fn treats_line_prefixed_placeholder_comments_as_block_comments() {
        let tokens = lex("attribute x = ( //* ... */ );").unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::LParen))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::RParen))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Semicolon))
        );
    }
}
