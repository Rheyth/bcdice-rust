//! P4で手書き移植した `lib/bcdice/game_system/Ryutama.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Ryutama#eval_game_system_specific_command`（判定 `Rx,y+m>=t`）と
//!   その下請け（`get_dice_type` / `get_roll_value` / `get_result_text` / `get_base_text`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `Ryutama#initialize` の `@valid_dice_types`。
static VALID_DICE_TYPES: &[i64] = &[20, 12, 10, 8, 6, 4, 2];

/// Ruby `BCDice::GameSystem::Ryutama`（ID: `Ryutama`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ryutama;

impl GameSystem for Ryutama {
    fn id(&self) -> &'static str {
        "Ryutama"
    }

    fn name(&self) -> &'static str {
        "りゅうたま"
    }

    fn sort_key(&self) -> &'static str {
        "りゆうたま"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　Rx,y>=t（x,y：使用する能力値、t：目標値）
　1ゾロ、クリティカルも含めて判定結果を表示します
　能力値１つでの判定は Rx>=t で行えます
例）R8,6>=13
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["R"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Ryutama#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_roll(command, rng)
    }
}

/// Ruby `/^R(\d+)(,(\d+))?([+\-\d]+)?(>=(\d+))?/`。
///
/// 末尾は固定していないので、後ろに余計な文字が続いても先頭が合えばマッチする
/// （原典どおり）。
fn roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^R(\d+)(,(\d+))?([+\-\d]+)?(>=(\d+))?").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 桁あふれする能力値は `@valid_dice_types` に含まれないので、
/// 飽和しても最終的に「不正なダイス」として弾かれ、挙動は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `Ryutama#eval_game_system_specific_command` 本体。
fn eval_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: unless command =~ /.../ -> return ''
    let Some(captures) = roll_pattern().captures(command) else {
        return Ok(Some(SpecificCommandOutput::text("")));
    };

    let dice1 = to_i(&captures[1]);
    // Ruby: Regexp.last_match(3).to_i（nil.to_i == 0）
    let dice2 = captures.get(3).map_or(0, |m| to_i(m.as_str()));
    let modify_string = captures.get(4).map_or("", |m| m.as_str());
    let difficulty = captures.get(6).map(|m| to_i(m.as_str()));

    let (dice1, dice2) = get_dice_type(dice1, dice2);
    if dice1 == 0 {
        return Ok(Some(SpecificCommandOutput::text("")));
    }

    // Ruby: ArithmeticEvaluator.eval(modify_string)（不正な式は0）
    let modify = arithmetic::eval(modify_string, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let value1 = get_roll_value(dice1, rng)?;
    let value2 = get_roll_value(dice2, rng)?;
    let total = value1 + value2 + modify;

    let result = get_result_text(value1, value2, dice1, dice2, difficulty, total);
    let result = if result.is_empty() {
        String::new()
    } else {
        format!(" ＞ {result}")
    };

    let value1_text = format!("{value1}({dice1})");
    // Ruby: value2 == 0 ? "" : "+#{value2}(#{dice2})"
    let value2_text = if value2 == 0 {
        String::new()
    } else {
        format!("+{value2}({dice2})")
    };
    let modify_text = modifier(&crate::Int::from(modify));

    let base_text = get_base_text(dice1, dice2, modify, difficulty);
    Ok(Some(SpecificCommandOutput::text(format!(
        "({base_text}) ＞ {value1_text}{value2_text}{modify_text} ＞ {total}{result}"
    ))))
}

/// Ruby `Ryutama#get_dice_type`。
///
/// `Rxy` のように能力値2つが連結された表記を、`@valid_dice_types` に載る組へ
/// 分解する。分解できなければ `(0, 0)`。
fn get_dice_type(dice1: i64, dice2: i64) -> (i64, i64) {
    // Ruby: dice2 が指定されている場合は dice1 しか検査しない
    //       （`R6,7` は d7 を振る）
    if dice2 != 0 {
        if valid_dice_one(dice1) {
            return (dice1, dice2);
        }
        return (0, 0);
    }

    if valid_dice(dice1, dice2) {
        return (dice1, dice2);
    }

    let dice_base = dice1;

    // `(\d+)` 由来なので常に非負。Rubyの `Integer#/`（床除算）と一致する。
    let (d1, d2) = (dice_base / 10, dice_base % 10);
    if valid_dice(d1, d2) {
        return (d1, d2);
    }

    let (d1, d2) = (dice_base / 100, dice_base % 100);
    if valid_dice(d1, d2) {
        return (d1, d2);
    }

    if valid_dice_one(dice_base) {
        return (dice_base, 0);
    }

    (0, 0)
}

/// Ruby `Ryutama#valid_dice?`。
fn valid_dice(dice1: i64, dice2: i64) -> bool {
    valid_dice_one(dice1) && valid_dice_one(dice2)
}

/// Ruby `Ryutama#valid_dice_one?`。
fn valid_dice_one(dice: i64) -> bool {
    VALID_DICE_TYPES.contains(&dice)
}

/// Ruby `Ryutama#get_roll_value`。`dice == 0` ではダイスを振らない。
fn get_roll_value(dice: i64, rng: &mut Randomizer) -> Result<i64, EvalError> {
    if dice == 0 {
        return Ok(0);
    }
    rng.roll_once(dice)
}

/// Ruby `Ryutama#get_result_text`。
fn get_result_text(
    value1: i64,
    value2: i64,
    dice1: i64,
    dice2: i64,
    difficulty: Option<i64>,
    total: i64,
) -> &'static str {
    if famble(value1, value2) {
        return "１ゾロ【１ゾロポイント＋１】";
    }

    if critical(value1, value2, dice1, dice2) {
        return "クリティカル成功";
    }

    let Some(difficulty) = difficulty else {
        return "";
    };

    if total >= difficulty {
        "成功"
    } else {
        "失敗"
    }
}

/// Ruby `Ryutama#famble?`（原典の綴りのまま）。
fn famble(value1: i64, value2: i64) -> bool {
    value1 == 1 && value2 == 1
}

/// Ruby `Ryutama#critical?`。
fn critical(value1: i64, value2: i64, dice1: i64, dice2: i64) -> bool {
    if value2 == 0 {
        return false;
    }

    if value1 == 6 && value2 == 6 {
        return true;
    }

    value1 == dice1 && value2 == dice2
}

/// Ruby `Ryutama#get_base_text`。
///
/// Ruby `get_modify_string` は `Format.modifier` と同じ分岐なので
/// [`crate::format::modifier`] を使う。
fn get_base_text(dice1: i64, dice2: i64, modify: i64, difficulty: Option<i64>) -> String {
    let mut base_text = format!("R{dice1}");

    if dice2 != 0 {
        base_text.push_str(&format!(",{dice2}"));
    }

    base_text.push_str(&modifier(&crate::Int::from(modify)));

    if let Some(difficulty) = difficulty {
        base_text.push_str(&format!(">={difficulty}"));
    }

    base_text
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Ryutama", "Ryutama.toml", 46);
    }
}
