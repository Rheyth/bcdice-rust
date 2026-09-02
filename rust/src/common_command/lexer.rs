//! 汎用コマンド用の字句解析。Ruby `lib/bcdice/common_command/lexer.rb` の移植。
//!
//! Ruby側は Racc が `next_token` を1個ずつ引く形だが、字句解析は構文解析の状態に
//! 依存しない（`StringScanner` を順に舐めるだけ）ので、Rust側では先に全トークンを
//! 取り出してからスライスを走査する。受理する字句列は同一になる。

use crate::normalize::{self, CmpOp};

/// トークン。Ruby側はシンボル（`:NUMBER` など）と値の組。
///
/// `CommonCommand::Lexer` と `Command::Lexer`（[`crate::command_parser`]）の
/// 両方の記号表の和集合。各字句解析器は自分の記号表に載っている種別しか生成しない
/// （例: `[` は CommonCommand では `BracketL`、Command では `Sym("[")`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Number(crate::Int),
    /// Ruby: `[:CMP_OP, Normalize.comparison_operator(cmp_op)]`。
    /// 正規化に失敗すると値が `nil` になるため `Option` で持つ
    /// （各文法の `raise ParseError unless cmp_op` で弾かれる）。
    CmpOp(Option<CmpOp>),
    Plus,
    Minus,
    Asterisk,
    Slash,
    ParenL,
    ParenR,
    BracketL,
    BracketR,
    Question,
    At,
    /// `Command::Lexer` 専用（`#`）
    Sharp,
    /// `Command::Lexer` 専用（`$`）
    Dollar,
    /// `Command::Lexer` 専用。コマンド表記にマッチした文字列。
    Notation(String),
    /// `Command::Lexer` 専用。比較演算子として正規化できなかった記号列。
    /// どの文法規則にも現れないので必ず構文エラーになる。
    Illegal,
    /// 記号表にない1文字を大文字化したもの。Ruby: `char.to_sym`。
    ///
    /// Ruby の `String#upcase` は 1文字が複数文字になりうる（`"ß" => "SS"`）ので
    /// `String` で保持する。文法が使うのは ASCII 1文字のトークンのみ。
    Sym(String),
}

/// Rubyの正規表現 `\s` に相当する文字集合（`[ \t\r\n\f\v]`）。
pub const RUBY_WHITESPACE: [char; 6] = [' ', '\t', '\n', '\u{b}', '\u{c}', '\r'];

/// Ruby `String#split(" ", 2).first || ""` 相当。
///
/// 区切りに単一の半角スペース文字列を渡した場合、Rubyはawk風分割になり
/// 先頭の空白を読み飛ばして最初の空白までを1要素目とする。
pub fn first_word(source: &str) -> &str {
    let start = source
        .find(|c: char| !RUBY_WHITESPACE.contains(&c))
        .unwrap_or(source.len());
    let rest = &source[start..];
    match rest.find(RUBY_WHITESPACE) {
        Some(end) => &rest[..end],
        None => rest,
    }
}

/// 数字列を整数にする。Ruby `String#to_i`（多倍長）相当。
///
/// 呼び出し側は [`scan_digits`] の結果（ASCII数字のみの列）を渡す契約なので、
/// パースは失敗しない。契約違反時は panic ではなく 0 で握り、デバッグビルドでは
/// assert で検知できるようにする。
///
/// [`scan_digits`]: crate::common_command::lexer::scan_digits
pub(crate) fn digits_to_int(digits: &str) -> crate::Int {
    debug_assert!(
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        "digits should be numeric: {digits:?}"
    );
    digits.parse::<crate::Int>().unwrap_or_default()
}

/// Ruby `StringScanner#getch` + `String#upcase` 相当。
pub(crate) fn upcase_char(c: char) -> String {
    c.to_uppercase().collect()
}

/// 先頭からの数字列を切り出す。Ruby `@scanner.scan(/\d+/)`。
pub(crate) fn scan_digits(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

/// 先頭からの比較演算子文字列を切り出す。Ruby `@scanner.scan(/[<>!=]+/)`。
pub(crate) fn scan_cmp_op(s: &str) -> Option<&str> {
    let end = s
        .find(|c: char| !matches!(c, '<' | '>' | '!' | '='))
        .unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

/// 字句解析の結果。
#[derive(Debug, Clone)]
pub struct Lexed {
    /// 空白で切り詰めた後の入力。Ruby `Lexer#source`（`@scanner.string`）。
    pub source: String,
    /// トークン列（`$end` は含まない。終端は列の末尾で表す）。
    pub tokens: Vec<Tok>,
}

/// Ruby `CommonCommand::Lexer` 相当の字句解析。
pub fn lex(source: &str) -> Lexed {
    let source = first_word(source).to_string();
    let mut tokens = Vec::new();
    let mut rest = source.as_str();

    while !rest.is_empty() {
        if let Some(d) = scan_digits(rest) {
            tokens.push(Tok::Number(digits_to_int(d)));
            rest = &rest[d.len()..];
            continue;
        }
        if let Some(op) = scan_cmp_op(rest) {
            tokens.push(Tok::CmpOp(normalize::comparison_operator(op)));
            rest = &rest[op.len()..];
            continue;
        }
        // scan_digits/scan_cmp_op のどちらにも当てはまらなければ先頭は非ASCII数字・
        // 非比較演算子の1文字なので、`Some` が保証される
        let Some(c) = rest.chars().next() else {
            break;
        };
        rest = &rest[c.len_utf8()..];
        let upper = upcase_char(c);
        tokens.push(match upper.as_str() {
            "+" => Tok::Plus,
            "-" => Tok::Minus,
            "*" => Tok::Asterisk,
            "/" => Tok::Slash,
            "(" => Tok::ParenL,
            ")" => Tok::ParenR,
            "[" => Tok::BracketL,
            "]" => Tok::BracketR,
            "?" => Tok::Question,
            "@" => Tok::At,
            _ => Tok::Sym(upper),
        });
    }

    Lexed { source, tokens }
}

/// トークン列のカーソル。各パーサはこれを共有して再帰下降する。
#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    tokens: &'a [Tok],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(tokens: &'a [Tok]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    pub fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub fn advance(&mut self) {
        self.pos += 1;
    }

    /// 指定トークンなら1つ進めて `true`。
    pub fn accept(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// 指定の記号トークン（`Sym`）なら1つ進めて `true`。
    pub fn accept_sym(&mut self, sym: &str) -> bool {
        if self.peek_is_sym(sym) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn peek_is_sym(&self, sym: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(s)) if s == sym)
    }

    /// `term` の開始トークン（`PARENL` または `NUMBER`）か。
    pub fn peek_starts_term(&self) -> bool {
        matches!(self.peek(), Some(Tok::ParenL) | Some(Tok::Number(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_first_whitespace() {
        assert_eq!(first_word("2D6 コメント"), "2D6");
        assert_eq!(first_word("  2D6  x"), "2D6");
        assert_eq!(first_word(""), "");
        assert_eq!(first_word("   "), "");
    }

    #[test]
    fn lexes_add_dice_command() {
        let lexed = lex("2D6>=7 コメント");
        assert_eq!(lexed.source, "2D6>=7");
        assert_eq!(
            lexed.tokens,
            vec![
                Tok::Number(2.into()),
                Tok::Sym("D".into()),
                Tok::Number(6.into()),
                Tok::CmpOp(Some(CmpOp::Ge)),
                Tok::Number(7.into()),
            ]
        );
    }

    #[test]
    fn lexes_illegal_cmp_op_as_none() {
        let lexed = lex("2D6!7");
        assert_eq!(lexed.tokens[3], Tok::CmpOp(None));
    }

    #[test]
    fn upcases_letters() {
        let lexed = lex("2d6kh1");
        assert_eq!(
            lexed.tokens,
            vec![
                Tok::Number(2.into()),
                Tok::Sym("D".into()),
                Tok::Number(6.into()),
                Tok::Sym("K".into()),
                Tok::Sym("H".into()),
                Tok::Number(1.into()),
            ]
        );
    }
}
