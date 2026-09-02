use num_traits::ToPrimitive;

use crate::common_command::lexer::digits_to_int;
use crate::common_command::scanner::Scanner;
use crate::eval::{self, EvalError, EvalResult};
use crate::game_system::GameSystem;
use crate::randomizer::Randomizer;
use crate::Int;

/// 繰り返し回数の上限。Ruby `Repeat::REPEAT_LIMIT`。
pub const REPEAT_LIMIT: i64 = 100;

/// Ruby `Repeat.eval(command, game_system, randomizer)`。
pub fn eval(
    command: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(cmd) => cmd.roll(game_system, rng).map(Some),
        None => Ok(None),
    }
}

/// Ruby `Repeat` インスタンス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeat {
    secret: bool,
    times: Int,
    trailer: String,
}

impl Repeat {
    /// Ruby `#roll(game_system, randomizer)`。
    pub fn roll(&self, gs: &dyn GameSystem, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        if let Some(err) = self.validate() {
            return Ok(err);
        }

        let times_usize = self.times.to_usize().unwrap_or(0);
        let mut results: Vec<Option<EvalResult>> = Vec::with_capacity(times_usize);
        for _ in 0..times_usize {
            results.push(eval::eval_raw(gs, &self.trailer, rng)?);
        }

        if results.iter().all(|r| r.is_none()) {
            return Ok(self.result_with_text(format!(
                "繰り返し対象のコマンドが実行できませんでした ({})",
                self.trailer
            )));
        }

        // Ruby は結果に nil が混ざると `nil.text` で NoMethodError になる（本家のバグ）。
        // ここでは空文字列として扱う。
        let text = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "#{}\n{}",
                    i + 1,
                    r.as_ref().map(|x| x.text.as_str()).unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let secret = self.secret || results.iter().any(|r| r.as_ref().is_some_and(|x| x.secret));

        Ok(EvalResult {
            text,
            secret,
            ..EvalResult::default()
        })
    }

    /// Ruby `#validate`。
    fn validate(&self) -> Option<EvalResult> {
        // Ruby: /\A(repeat|rep|x)\d+/ は **大文字小文字を区別する**
        if starts_with_repeat_keyword_case_sensitive(&self.trailer) {
            Some(self.result_with_text("Repeatコマンドの重複はできません".to_string()))
        } else if self.times < 1.into() || Int::from(REPEAT_LIMIT) < self.times {
            Some(self.result_with_text(format!(
                "繰り返し回数は1以上、{REPEAT_LIMIT}以下としてください"
            )))
        } else {
            None
        }
    }

    fn result_with_text(&self, text: String) -> EvalResult {
        EvalResult {
            text,
            secret: self.secret,
            ..EvalResult::default()
        }
    }
}

/// Ruby `/\A(repeat|rep|x)\d+/.match?(trailer)`（大文字小文字を区別）。
fn starts_with_repeat_keyword_case_sensitive(trailer: &str) -> bool {
    for kw in ["repeat", "rep", "x"] {
        if let Some(rest) = trailer.strip_prefix(kw) {
            if rest.starts_with(|c: char| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

/// Ruby `Repeat.parse(command)`。
///
/// `StringScanner` を `s?` → キーワード → 回数 → 空白 の順で舐め、
/// 残り（`post_match`）を `trailer` にする。空白が無い場合は `nil`。
pub fn parse(command: &str) -> Option<Repeat> {
    let mut scanner = Scanner::new(command);

    // scanner.scan(/s/i)
    let secret = scanner.scan_char_ci('s');

    // scanner.scan(/repeat|rep|x/i) : Rubyの選択は左優先なので "repeat" が先
    if !["repeat", "rep", "x"]
        .iter()
        .any(|kw| scanner.scan_literal_ci(kw))
    {
        return None;
    }

    // scanner.scan(/\d+/)
    let digits = scanner.scan_digits()?;
    let times = digits_to_int(digits);

    // scanner.scan(/\s+/) : 空白が必須
    let ws_start = scanner.rest().len();
    scanner.skip_whitespace();
    if scanner.rest().len() == ws_start {
        return None;
    }

    // scanner.post_match（直前のマッチ以降＝残り全部）
    let trailer = scanner.rest();
    if trailer.is_empty() {
        return None;
    }

    Some(Repeat {
        secret,
        times,
        trailer: trailer.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keywords() {
        assert_eq!(parse("x5 1D6").unwrap().times, 5.into());
        assert_eq!(parse("rep5 1D6").unwrap().times, 5.into());
        assert_eq!(parse("repeat5 1D6").unwrap().trailer, "1D6");
        assert!(parse("sx5 1D6").unwrap().secret);
        assert!(parse("rep1").is_none());
        assert!(parse("rep1 ").is_none());
        assert!(parse("2R6").is_none());
    }

    #[test]
    fn nesting_check_is_case_sensitive() {
        assert!(starts_with_repeat_keyword_case_sensitive("rep100 10D100"));
        assert!(starts_with_repeat_keyword_case_sensitive("x2 1D6"));
        assert!(!starts_with_repeat_keyword_case_sensitive("X2 1D6"));
        assert!(!starts_with_repeat_keyword_case_sensitive("repeat 1D6"));
    }
}
