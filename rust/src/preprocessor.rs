//! 入力文字列の前処理。Ruby `lib/bcdice/preprocessor.rb` の移植。
//!
//! 注意: `Base#eval` は前処理後の文字列を `dice_command`（ゲームシステム固有コマンド）
//! にのみ渡し、共通コマンドには**前処理前の生入力**を渡す。DiceBotは `prefixes` が
//! 空で `dice_command` が常に `nil` を返すため、P1のTOMLテストでは前処理結果は
//! 使われない。P4のゲームシステム移植で必要になるので先に実装しておく。

use crate::arithmetic;
use crate::eval::EvalError;
use crate::game_system::GameSystem;
use regex::Regex;
use std::sync::OnceLock;

/// カッコ書きの数式にマッチする正規表現。Ruby `%r{\([\d/+*\-CURF]+\)}`。
fn paren_expr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\([\d/+*\-CURF]+\)").expect("valid regex"))
}

/// `nD` にマッチする正規表現。Ruby `/(\d+)D([^\w]|$)/i`。
///
/// Rubyの `\w` はASCIIのみだが、Rustの `regex` は既定でUnicode対応なので
/// `(?-u:\w)` 相当の明示クラス `[^0-9A-Za-z_]` を使う。
/// Rubyの `$` は行末にもマッチするため `(?m)` を付ける。
fn implicit_d_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?im)(\d+)D([^0-9A-Za-z_]|$)").expect("valid regex"))
}

/// Ruby `Preprocessor#trim_after_whitespace` が `nil` を返す入力かどうか。
///
/// Ruby は `@text.strip.split(/\s/, 2).first` で、空白のみ（および空文字）の入力に
/// 対して `nil` を返し、続く `replace_parentheses` の `nil.gsub` が `NoMethodError` を
/// 送出する（lib/bcdice/preprocessor.rb:37,45）。本家の未定義動作だが、`eval` が
/// nil を返すのか例外になるのかは呼び出し側から観測できるので、
/// [`crate::eval::eval_raw`] が [`crate::eval::EvalError::BlankInput`] として再現する。
///
/// `String#strip` が落とす文字集合は NUL・水平タブ・改行・垂直タブ・改ページ・復帰・空白。
/// 正規表現の `\s` はこの部分集合なので、`strip` 後の先頭が `\s` になることはなく、
/// 「`first` が nil」は「strip 結果が空」と同値になる。
/// U+3000（全角空白）は Ruby の `strip` にも `\s` にも含まれないため対象外。
pub fn is_blank(text: &str) -> bool {
    text.chars()
        .all(|c| matches!(c, '\0' | '\t' | '\n' | '\x0b' | '\x0c' | '\r' | ' '))
}

/// Ruby `Preprocessor.process(text, game_system)`。
pub fn process(text: &str, game_system: &dyn GameSystem) -> Result<String, EvalError> {
    let mut s = trim_after_whitespace(text).to_string();
    s = replace_parentheses(&s, game_system)?;
    s = game_system.change_text(&s).into_owned();
    Ok(replace_implicit_d(&s, game_system))
}

/// 空白より前だけを取る。Ruby `@text.strip.split(/\s/, 2).first`。
///
/// Ruby側は入力が空白のみの場合 `nil` になり、以降の `gsub` で NoMethodError に
/// なる（本家のバグ）。ここでは空文字列として扱う。
fn trim_after_whitespace(text: &str) -> &str {
    // Ruby String#strip は " \t\n\v\f\r\0" を落とす
    let trimmed = text
        .trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\0'));
    match trimmed.find(crate::common_command::lexer::RUBY_WHITESPACE) {
        Some(i) => &trimmed[..i],
        None => trimmed,
    }
}

/// カッコ書きの数式を事前計算する。Ruby `#replace_parentheses`。
fn replace_parentheses(text: &str, game_system: &dyn GameSystem) -> Result<String, EvalError> {
    let mut current = text.to_string();
    loop {
        let mut out = String::with_capacity(current.len());
        let mut last = 0usize;
        for m in paren_expr_re().find_iter(&current) {
            out.push_str(&current[last..m.start()]);
            // Ruby: Arithmetic.eval(expr, round_type) || expr
            match arithmetic::eval(m.as_str(), game_system.round_type())? {
                Some(v) => out.push_str(&v.to_string()),
                None => out.push_str(m.as_str()),
            }
            last = m.end();
        }
        out.push_str(&current[last..]);

        if out == current {
            return Ok(current);
        }
        current = out;
    }
}

/// nDをゲームシステムに応じて置き換える。Ruby `#replace_implicit_d`。
fn replace_implicit_d(text: &str, game_system: &dyn GameSystem) -> String {
    let sides = game_system.sides_implicit_d();
    implicit_d_re()
        .replace_all(text, |caps: &regex::Captures| {
            format!("{}D{}{}", &caps[1], sides, &caps[2])
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dice_bot() -> &'static dyn GameSystem {
        crate::game_system::game_system_class("DiceBot").expect("DiceBot is implemented")
    }

    #[test]
    fn processes_doc_example() {
        // Ruby の doc は `#=> "1d6+4D6+7"` としているが、`(3*4)` は 12 なので
        // doc 側の誤記。実装（`Arithmetic.eval("(3*4)")`）どおり 12 になる。
        let out = process("1d6+4D+(3*4) 切り取られる部分", dice_bot()).unwrap();
        assert_eq!(out, "1d6+4D6+12");
    }

    #[test]
    fn keeps_expression_on_zero_division() {
        // Arithmetic.eval が nil を返すので元の文字列が残る
        let out = process("(1/0)", dice_bot()).unwrap();
        assert_eq!(out, "(1/0)");
    }

    #[test]
    fn replaces_implicit_d_at_end_of_string() {
        assert_eq!(process("201D", dice_bot()).unwrap(), "201D6");
        assert_eq!(process("2D+1", dice_bot()).unwrap(), "2D6+1");
        // \w が続く場合は置換しない
        assert_eq!(process("2D6", dice_bot()).unwrap(), "2D6");
    }

    #[test]
    fn trims_after_whitespace() {
        assert_eq!(process("  2D6  コメント", dice_bot()).unwrap(), "2D6");
        assert_eq!(process("   ", dice_bot()).unwrap(), "");
    }

    /// `is_blank` は Ruby の `trim_after_whitespace` が `nil` を返す入力集合と
    /// 正確に一致する（差分ファズ degenerate 5件の根拠）。
    #[test]
    fn is_blank_matches_ruby_trim_after_whitespace() {
        let blanks = ["", " ", "   ", "\t", "\n", " \t\n\r\u{b}\u{c}\0"];
        for blank in blanks {
            assert!(is_blank(blank), "must be blank: {blank:?}");
        }

        let not_blank = ["1D6", "　", " 1D6", "1 ", "S2D6"];
        for not_blank in not_blank {
            assert!(!is_blank(not_blank), "must not be blank: {not_blank:?}");
        }
    }
}
