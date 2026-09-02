//! P4で手書き移植した `lib/bcdice/game_system/KinAriel.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KinAriel#resolute_action`（判定 `KA<=t`）
//! - `KinAriel#resolute_competition`（対抗判定 `VS<=t`）と `get_roll_result`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::KinAriel`（ID: `KinAriel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KinAriel;

impl GameSystem for KinAriel {
    fn id(&self) -> &'static str {
        "KinAriel"
    }

    fn name(&self) -> &'static str {
        "キナリエル"
    }

    fn sort_key(&self) -> &'static str {
        "きなりえる"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　KA<=t            t: 目標値

例)KA<=50: 目標値50で結果を表示(クリティカル、ファンブル、成功、失敗)

■対抗判定　VS<=t        t: 目標値

例)VS<=50: 目標値50で最大5回振って、その結果を表示。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["KA", "VS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `KinAriel#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: resolute_action(command) || resolute_competition(command)
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = resolute_competition(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `/KA<=(\d+)/`。前後を固定していないので部分一致でよい（原典どおり）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"KA<=(\d+)").expect("valid regex"))
}

/// Ruby `/VS<=(\d+)/`。前後を固定していないので部分一致でよい（原典どおり）。
fn competition_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"VS<=(\d+)").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 目標値は1D100の出目との比較にしか使わないので、飽和させても分岐は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `KinAriel#resolute_action`（通常判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let target = to_i(&captures[1]);
    let dice = rng.roll_once(100)?;

    let mut output = format!("(KA<={target}) ＞ [{dice}]");

    // クリティカル判定は成功した場合の中、ファンブル判定は失敗した場合の中にある
    // （原典の入れ子構造をそのまま保つ）。
    if dice <= target {
        if dice <= 5 {
            output.push_str(" ＞ クリティカル");
            Ok(Some(EvalResult::critical(output)))
        } else {
            output.push_str(" ＞ 成功");
            Ok(Some(EvalResult::success(output)))
        }
    } else if dice >= 96 {
        output.push_str(" ＞ ファンブル");
        Ok(Some(EvalResult::fumble(output)))
    } else {
        output.push_str(" ＞ 失敗");
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `KinAriel#resolute_competition`（対抗判定）。
fn resolute_competition(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = competition_pattern().captures(command) else {
        return Ok(None);
    };

    let target = to_i(&captures[1]);
    let mut output = format!("(VS<={target}) ＞ ");
    let mut dice_arr: Vec<i64> = Vec::new();
    // Ruby: result = Result.new（5回とも回るので、この初期値は出力に出ない）
    let mut result = EvalResult::new();

    for _ in 0..5 {
        let dice = rng.roll_once(100)?;
        dice_arr.push(dice);
        result = get_roll_result(dice, target);

        // Result.fumble は failure も立てるので、この判定順は入れ替えられない。
        let suffix = if result.critical {
            "クリティカル"
        } else if result.fumble {
            "ファンブル"
        } else if result.failure {
            "失敗"
        } else {
            continue;
        };

        output.push_str(&format!(
            "[{}] ＞ {}回目で{}",
            join_comma(&dice_arr),
            dice_arr.len(),
            suffix
        ));
        result.text = output;
        return Ok(Some(result));
    }

    output.push_str(&format!(
        "[{}] ＞ {}回成功",
        join_comma(&dice_arr),
        dice_arr.len()
    ));
    result.text = output;
    Ok(Some(result))
}

/// Ruby `KinAriel#get_roll_result`。
fn get_roll_result(dice: i64, target: i64) -> EvalResult {
    if dice <= target {
        if dice <= 5 {
            EvalResult::critical("")
        } else {
            EvalResult::success("")
        }
    } else if dice >= 96 {
        EvalResult::fumble("")
    } else {
        EvalResult::failure("")
    }
}

/// Ruby `Array#join(",")`。
fn join_comma(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("KinAriel", "KinAriel.toml", 11);
    }
}
