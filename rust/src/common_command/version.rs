//! バージョン表示コマンド。Ruby `lib/bcdice/common_command/version.rb` の移植。

use crate::common_command::lexer::first_word;
use crate::eval::EvalResult;

/// Ruby `BCDice::VERSION`（lib/bcdice/version.rb）。
pub const BCDICE_VERSION: &str = "3.17.0";

/// Ruby `Version.eval(command, _game_system, _randomizer)`。
pub fn eval(command: &str) -> Option<EvalResult> {
    let command = first_word(command);
    // Ruby: command.match?(/^BCDiceVersion$/i)
    if command.eq_ignore_ascii_case("BCDiceVersion") {
        Some(EvalResult::with_text(format!("BCDice {BCDICE_VERSION}")))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_case_insensitively() {
        assert_eq!(eval("BCDiceVersion").unwrap().text, "BCDice 3.17.0");
        assert_eq!(
            eval("bcdiceversion コメント").unwrap().text,
            "BCDice 3.17.0"
        );
        assert!(eval("BCDiceVersionX").is_none());
    }
}
