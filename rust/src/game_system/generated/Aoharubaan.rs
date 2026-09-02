//! P4で手書き移植した `lib/bcdice/game_system/Aoharubaan.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Aoharubaan#roll_judge`（`1D6+m>=t` の判定）
//! - `TABLES`（カレカノ反応表 `KR` / `KReaction`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Aoharubaan`（ID: `Aoharubaan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aoharubaan;

impl GameSystem for Aoharubaan {
    fn id(&self) -> &'static str {
        "Aoharubaan"
    }

    fn name(&self) -> &'static str {
        "あおはるばーんっ"
    }

    fn sort_key(&self) -> &'static str {
        "あおはるはあんつ"
    }

    fn help_message(&self) -> &'static str {
        r"カレカノ反応表（ KR, KReaction ）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(1d6?|d6)(\+\d+)?(>=|=>)(\d+)", "KR", "KREACTION"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Aoharubaan#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: command = ALIAS[command] || command
        let command = ALIAS
            .iter()
            .find(|(from, _)| *from == command)
            .map(|(_, to)| *to)
            .unwrap_or(command);

        if let Some(m) = judge_roll_pattern().captures(command) {
            let result = roll_judge(m.get(2).map(|x| x.as_str()), &m[4], rng)?;
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `JUDGE_ROLL_REG`（`/^(1d6?|d6)(\+\d+)?(>=|=>)(\d+)$/i`）。
fn judge_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(1d6?|d6)(\+\d+)?(>=|=>)(\d+)$").expect("valid regex"))
}

/// Ruby `Aoharubaan#roll_judge`。
fn roll_judge(
    modifier_expression: Option<&str>,
    border_expression: &str,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    // Ruby: modifier_expression ? Arithmetic.eval(modifier_expression, RoundType::FLOOR) : nil
    let modifier = match modifier_expression {
        Some(expr) => arithmetic::eval(expr, RoundType::Floor)?,
        None => None,
    };
    // Ruby: border_expression.to_i
    let border: i64 = border_expression.parse().unwrap_or(i64::MAX);

    let command_text = make_command_text(modifier.as_ref().map(crate::randomizer::sat_i64), border);

    let dice = rng.roll_once(6)?;
    // Ruby: dice + modifier.to_i（nil.to_i は 0）
    let score = dice + modifier.clone().unwrap_or_default();

    let is_success = score >= crate::Int::from(border); // 「成功」か？
    let is_right = is_success && score == crate::Int::from(border); // 「ピタリ賞」か？
    let is_excellent = is_success && score >= crate::Int::from(7); // 「限界突破」か？

    let mut result_elements: Vec<&str> = Vec::new();
    result_elements.push(if is_success { "成功" } else { "失敗" });
    if is_right {
        result_elements.push("ピタリ賞");
    }
    if is_excellent {
        result_elements.push("限界突破");
    }

    let mut message_elements: Vec<String> = Vec::new();
    message_elements.push(command_text);
    if let Some(modifier) = modifier {
        message_elements.push(format!("{dice}+{modifier}"));
    }
    message_elements.push(score.to_string());
    message_elements.push(result_elements.join(" ＆ "));

    let mut result = EvalResult::with_text(message_elements.join(" ＞ "));
    result.set_condition(is_success);
    result.critical = is_right || is_excellent;
    Ok(result)
}

/// Ruby `Aoharubaan#make_command_text`。
fn make_command_text(modifier: Option<i64>, border: i64) -> String {
    let mut command = "1D6".to_owned();
    if let Some(modifier) = modifier {
        command = format!("{command}+{modifier}");
    }
    command = format!("{command}>={border}");
    format!("({command})")
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    table_helpers::roll_range_table(command, TABLES, rng)
}

/// Ruby `ALIAS`（キー・値ともに `upcase` 済み）。
static ALIAS: &[(&str, &str)] = &[("KR", "KREACTION")];

/// Ruby `TABLES["KREACTION"]`（カレカノ反応表）の項目。
static K_REACTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "何となく素っ気ない気がする。"),
    (RangeInc::new(3, 4), "いつもと変わらない安心感。"),
    (
        RangeInc::new(5, 6),
        "何故だかすごくデレてきた！　嬉しくて〈テンション〉１回復。",
    ),
];

/// Ruby `DiceTable::RangeTable.new("カレカノ反応表", "1D6", …)`。
static K_REACTION: RangeTable = RangeTable::from_dice("カレカノ反応表", 1, 6, K_REACTION_ITEMS);

/// Ruby `TABLES`（キーは `transform_keys(&:upcase)` 済み）。
static TABLES: &[(&str, &RangeTable)] = &[("KREACTION", &K_REACTION)];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Aoharubaan",
            "Aoharubaan.toml",
            13,
        );
    }
}
