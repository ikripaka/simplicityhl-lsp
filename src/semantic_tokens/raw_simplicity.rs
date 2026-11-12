//! Lexer for highlighting the human-readable Simplicity encoding.

use std::ops::Range;

use logos::Logos;

// Token patterns are adapted from rust-simplicity's human-encoding lexer (CC0-1.0).
#[derive(Logos)]
#[logos(skip r"[ \t\r\n]+")]
pub(super) enum TokenKind {
    #[regex(r"--[^\n]*")]
    Comment,

    #[token("const")]
    #[token("assertl")]
    #[token("assertr")]
    #[token("fail")]
    #[token("disconnect")]
    #[token("case")]
    #[token("comp")]
    #[token("pair")]
    #[token("injl")]
    #[token("injr")]
    #[token("take")]
    #[token("drop")]
    #[token("unit")]
    #[token("iden")]
    #[token("witness")]
    Keyword,

    #[regex(r"jet_[a-z0-9_]+")]
    Jet,

    #[regex(r"0b[01]+")]
    #[regex(r"0x[0-9a-f]+")]
    #[regex(r"#[a-fA-F0-9]{64}")]
    Number,

    #[token("1")]
    #[token("2")]
    #[regex(r"2\^[1-9][0-9]*")]
    Type,

    #[token(":=")]
    #[token("->")]
    #[token("#{")]
    #[token("(")]
    #[token(")")]
    #[token("+")]
    #[token("*")]
    #[token(":")]
    #[token("}")]
    #[token("?")]
    Operator,

    #[regex(r"[a-zA-Z_\-.'][0-9a-zA-Z_\-.']*", priority = 1)]
    Symbol,

    #[token("_")]
    Wildcard,
}

/// Yield tokens and their UTF-8 byte ranges, skipping unrecognized input.
pub(super) fn lex(source: &str) -> impl Iterator<Item = (TokenKind, Range<usize>)> + '_ {
    TokenKind::lexer(source)
        .spanned()
        .filter_map(|(token, span)| token.ok().map(|kind| (kind, span)))
}
