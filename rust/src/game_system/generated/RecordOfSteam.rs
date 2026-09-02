//! P4で手書き移植した `lib/bcdice/game_system/RecordOfSteam.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `RecordOfSteam#eval_game_system_specific_command`（判定 `nSt@c`）
//! - `#getDiceRollResult`（クリティカル値以下の出目ごとに2個振り足す回転処理）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `diceCount >= 150` / `criticalValue >= 3` のときの文言。
const TOO_MANY_DICE: &str =
    "(多分)無限個なので振れません！ ヤメテクダサイ、(プロセスが)死んでしまいますっ";

/// Ruby `/(\d+)[sS](\d+)(@(\d+))?/i`。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)[sS](\d+)(@(\d+))?").expect("valid regex"))
}

/// Ruby `#getDiceRollResult` の戻り値。
struct RollOutcome {
    roll_result: String,
    success_count: i64,
    round_count: i64,
    special_count: i64,
    fumble_count: i64,
}

/// Ruby `RecordOfSteam#eval_game_system_specific_command`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok("1".to_owned());
    };

    let dice_count = to_i(&m[1]);
    let target_number = to_i(&m[2]);
    // Ruby: criticalValue ||= 1
    let critical_value = m.get(4).map_or(1, |x| to_i(x.as_str()));

    if dice_count >= 150 {
        return Ok(TOO_MANY_DICE.to_owned());
    }

    if critical_value >= 3 {
        return Ok(TOO_MANY_DICE.to_owned());
    }

    let special_value = critical_value;

    let outcome = get_dice_roll_result(
        dice_count,
        target_number,
        critical_value,
        special_value,
        rng,
    )?;

    let output = format!("({command}) ＞ {}", outcome.roll_result);

    // Ruby: "#{output}#{roundCountText}#{specialText}#{successText}#{fumbleText}"
    Ok(format!(
        "{output}{}{}{}{}",
        round_count_text(outcome.round_count),
        special_text(outcome.special_count),
        success_text(outcome.success_count),
        fumble_text(outcome.fumble_count),
    ))
}

/// Ruby `RecordOfSteam#getDiceRollResult`。
fn get_dice_roll_result(
    dice_count: i64,
    target_number: i64,
    critical_value: i64,
    special_value: i64,
    rng: &mut Randomizer,
) -> Result<RollOutcome, EvalError> {
    let mut dice_count = dice_count;
    let mut success_count = 0i64;
    let mut round_count = 0i64;
    let mut roll_result = String::new();
    let mut special_flag = false;
    let mut fumble_flag = false;

    while dice_count > 0 {
        // Ruby側はソートしない（出目は振った順に並ぶ）。
        let dice_list = rng.roll_barabara(dice_count, 6)?;
        let dice_list_text = dice_text::join_dice(&dice_list);

        if !roll_result.is_empty() {
            roll_result.push(',');
        }
        roll_result.push_str(&dice_list_text);

        // Ruby: diceList.uniq.length == 1 && roundCount == 0
        if uniq_len(&dice_list) == 1 && round_count == 0 {
            // uniq が1要素＝全て同じ出目なので、先頭が `diceList.uniq.first`。
            let first = dice_list[0];
            if first <= special_value {
                special_flag = true;
            } else if first == 6 {
                fumble_flag = true;
            }
        }

        if special_flag {
            return Ok(RollOutcome {
                roll_result,
                success_count: dice_count.saturating_mul(3),
                round_count,
                special_count: 1,
                fumble_count: 0,
            });
        } else if fumble_flag {
            return Ok(RollOutcome {
                roll_result,
                success_count,
                round_count,
                special_count: 0,
                fumble_count: 1,
            });
        }

        dice_count = 0;

        for &dice_value in &dice_list {
            if dice_value <= critical_value {
                dice_count = dice_count.saturating_add(2);
                round_count = round_count.saturating_add(1);
            }

            if dice_value <= target_number {
                success_count = success_count.saturating_add(1);
            }
        }
    }

    Ok(RollOutcome {
        roll_result,
        success_count,
        round_count,
        special_count: 0,
        fumble_count: 0,
    })
}

/// Ruby `#getRoundCountText`。
fn round_count_text(round_count: i64) -> String {
    if round_count <= 0 {
        return String::new();
    }

    format!(" ＞ {round_count}回転")
}

/// Ruby `#getSuccessText`。
fn success_text(success_count: i64) -> String {
    if success_count > 0 {
        return format!(" ＞ 成功数{success_count}");
    }

    " ＞ 失敗".to_owned()
}

/// Ruby `#getSpecialText`（該当しない場合は `nil` ＝ 空文字列として連結される）。
fn special_text(special_count: i64) -> &'static str {
    if special_count == 1 {
        " ＞ スペシャル"
    } else {
        ""
    }
}

/// Ruby `#getFumbleText`（該当しない場合は `nil` ＝ 空文字列として連結される）。
fn fumble_text(fumble_count: i64) -> &'static str {
    if fumble_count == 1 {
        " ＞ ファンブル"
    } else {
        ""
    }
}

/// Ruby `Array#uniq` の要素数。
fn uniq_len(dice_list: &[i64]) -> usize {
    let mut uniq: Vec<i64> = Vec::new();
    for d in dice_list {
        if !uniq.contains(d) {
            uniq.push(*d);
        }
    }
    uniq.len()
}

/// Ruby `String#to_i`。i64に収まらない値は飽和させる（Rubyでは Bignum）。
/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `BCDice::GameSystem::RecordOfSteam`（ID: `RecordOfSteam`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordOfSteam;

impl GameSystem for RecordOfSteam {
    fn id(&self) -> &'static str {
        "RecordOfSteam"
    }

    fn name(&self) -> &'static str {
        "Record of Steam"
    }

    fn sort_key(&self) -> &'static str {
        "れこおとおふすちいむ"
    }

    fn help_message(&self) -> &'static str {
        r"2S2@1
RecordOfSteam : (2S2@1) ＞ 1,2,3,4 ＞ 1回転 ＞ 成功数2

4S3@2
RecordOfSteam : (4S3@2) ＞ 2,1,2,4,4,4,2,3,4,5,6,6 ＞ 4回転 ＞ 成功数5
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+S\d+"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(Some(SpecificCommandOutput::text(eval_specific_command(
            command, rng,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "RecordOfSteam",
            "RecordOfSteam.toml",
            8,
        );
    }
}
