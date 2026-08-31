//! D66ダイス。Ruby `lib/bcdice/common_command/d66_dice.rb` の移植。

use crate::common_command::lexer::first_word;
use crate::enums::D66SortType;
use crate::eval::{EvalError, EvalResult};
use crate::game_system::GameSystem;
use crate::randomizer::Randomizer;

/// Ruby `D66Dice.eval(command, game_system, randomizer)`。
pub fn eval(
    command: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    match parse(command, game_system) {
        Some(cmd) => cmd.eval(rng).map(Some),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D66Dice {
    secret: bool,
    sort_type: D66SortType,
    /// 出力にそのまま埋め込まれる接尾辞（大文字化済み）。
    suffix: Option<String>,
}

impl D66Dice {
    /// Ruby `D66Dice#eval(randomizer)`。
    pub fn eval(&self, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        let value = self.roll(rng)?;
        Ok(EvalResult {
            text: format!("(D66{}) ＞ {}", self.suffix.as_deref().unwrap_or(""), value),
            secret: self.secret,
            ..EvalResult::default()
        })
    }

    fn roll(&self, rng: &mut Randomizer) -> Result<i64, EvalError> {
        let mut dice_list = [rng.roll_once(6)?, rng.roll_once(6)?];
        match self.sort_type {
            D66SortType::Asc => dice_list.sort_unstable(),
            D66SortType::Desc => {
                dice_list.sort_unstable();
                dice_list.reverse();
            }
            D66SortType::NoSort => {}
        }
        Ok(dice_list[0] * 10 + dice_list[1])
    }
}

/// Ruby `D66Dice.parse(command, game_system)`。
///
/// 正規表現 `/^(S)?D66([ANSD])?$/i` は**大文字化した文字列**に適用され、
/// 接尾辞もそこから取るので、出力の `(D66A)` は常に大文字になる。
pub fn parse(command: &str, game_system: &dyn GameSystem) -> Option<D66Dice> {
    let command = first_word(command).to_uppercase();
    let mut rest = command.as_str();

    let secret = if let Some(r) = rest.strip_prefix('S') {
        rest = r;
        true
    } else {
        false
    };

    rest = rest.strip_prefix("D66")?;

    let suffix = match rest {
        "" => None,
        "A" | "N" | "S" | "D" => Some(rest.to_string()),
        _ => return None,
    };

    let sort_type = sort_type_from_suffix(suffix.as_deref()).unwrap_or(game_system.d66_sort_type());

    Some(D66Dice {
        secret,
        sort_type,
        suffix,
    })
}

fn sort_type_from_suffix(suffix: Option<&str>) -> Option<D66SortType> {
    match suffix {
        Some("A") | Some("S") => Some(D66SortType::Asc),
        Some("D") => Some(D66SortType::Desc),
        Some("N") => Some(D66SortType::NoSort),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dice_bot() -> &'static dyn GameSystem {
        crate::game_system::game_system_class("DiceBot").unwrap()
    }

    #[test]
    fn parses_suffixes() {
        assert!(parse("D66", dice_bot()).is_some());
        assert!(parse("d66a 小文字対応テスト", dice_bot()).is_some());
        assert_eq!(
            parse("d66a", dice_bot()).unwrap().suffix.as_deref(),
            Some("A")
        );
        assert!(parse("SD66", dice_bot()).unwrap().secret);
        assert!(parse("D66X", dice_bot()).is_none());
        assert!(parse("2D66", dice_bot()).is_none());
    }
}
