//! P4で手書き移植した `lib/bcdice/game_system/StrangerOfSwordCity.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command` → `checkRoll`（`xSR[±y][>=z]`）
//!   クリティカル（6が2個以上）／ファンブル（1が全ダイス数以上）の自動判定つき

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `checkRoll` の `/^(\d+)SR([+-]?\d+)?(>=(\d+))?$/i`。
///
/// 入力は `dice_command` が大文字化済みだが、Ruby側も `/i` なのでここでも大文字小文字を無視する。
fn check_roll_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)SR([+-]?\d+)?(>=(\d+))?$").unwrap())
}

/// Ruby `getModifyText`。
fn modify_text(modify: i64) -> String {
    if modify == 0 {
        String::new()
    } else if modify < 0 {
        modify.to_string()
    } else {
        format!("+{modify}")
    }
}

/// Ruby `getCriticalResult`。6の個数が2個以上ならその個数を返す。
fn critical_result(dice_list: &[i64]) -> Option<usize> {
    let count = dice_list.iter().filter(|d| **d == 6).count();
    (count >= 2).then_some(count)
}

/// Ruby `isFumble`。1の個数がダイス数以上（＝全部1）ならファンブル。
fn is_fumble(dice_list: &[i64], dice_count: i64) -> bool {
    let count = dice_list.iter().filter(|d| **d == 1).count() as i64;
    count >= dice_count
}

/// Ruby `checkRoll(command)`。
///
/// Ruby側は書式に合わない入力に対しても「テキストが空の `Result`」を返すので、
/// ここでも常に `EvalResult` を返す（`eval_game_system_specific_command` の
/// `instance_of?(Result)` が必ず true になるのと同じ）。
fn check_roll(command: &str, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let mut result = EvalResult::new();

    let Some(caps) = check_roll_re().captures(command) else {
        return Ok(result);
    };

    let dice_count: i64 = caps[1].parse().unwrap_or(0);
    // Ruby: Regexp.last_match(2).to_i（nil.to_i == 0）
    let modify: i64 = caps
        .get(2)
        .and_then(|m| m.as_str().trim_start_matches('+').parse::<i64>().ok())
        .unwrap_or(0);
    let difficulty: Option<i64> = caps.get(4).and_then(|m| m.as_str().parse::<i64>().ok());

    let mut dice_list = rng.roll_barabara(dice_count, 6)?;
    dice_list.sort_unstable();
    let dice: i64 = dice_list.iter().sum();

    let total_value = dice + modify;
    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    result.text = format!(
        "({command}) ＞ {dice}[{dice_str}]{} ＞ {total_value}",
        modify_text(modify)
    );

    if let Some(count) = critical_result(&dice_list) {
        result.critical = true;
        result.success = true;
        result.text += &format!(" ＞ クリティカル(+{count}D6)");
        return Ok(result);
    }

    if is_fumble(&dice_list, dice_count) {
        result.fumble = true;
        result.failure = true;
        result.text += " ＞ ファンブル";
        return Ok(result);
    }

    if let Some(difficulty) = difficulty {
        if total_value >= difficulty {
            result.success = true;
            result.text += " ＞ 成功";
        } else {
            result.failure = true;
            result.text += " ＞ 失敗";
        }
    }

    Ok(result)
}

/// Ruby `BCDice::GameSystem::StrangerOfSwordCity`（ID: `StrangerOfSwordCity`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrangerOfSwordCity;

impl GameSystem for StrangerOfSwordCity {
    fn id(&self) -> &'static str {
        "StrangerOfSwordCity"
    }

    fn name(&self) -> &'static str {
        "剣の街の異邦人TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "つるきのまちのいほうしんTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　xSR or xSRy or xSR+y or xSR-y or xSR+y>=z
　x=ダイス数、y=修正値(省略可、±省略時は＋として扱う)、z=難易度(省略可)
　判定時はクリティカル、ファンブルの自動判定を行います。
・通常のnD6ではクリティカル、ファンブルの自動判定は行いません。
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+SR"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `#eval_game_system_specific_command`。
    ///
    /// `checkRoll` の戻り値は常に `Result` なので、Ruby側の
    /// `return result if result.instance_of?(Result)` で必ず打ち切られる。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: command = command.upcase（@enabled_upcase_input で既に大文字化済み）
        let command = command.to_uppercase();
        Ok(Some(SpecificCommandOutput::result(check_roll(
            &command, rng,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "StrangerOfSwordCity",
            "StrangerOfSwordCity.toml",
            33,
        );
    }
}
