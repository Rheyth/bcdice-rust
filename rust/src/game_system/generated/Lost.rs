//! P4で手書き移植した `lib/bcdice/game_system/Lost.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Lost#roll_explode`（`exp10_deX`）
//! - `Lost#roll_golden`（`g01_denX`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Lost`（ID: `Lost`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lost;

impl GameSystem for Lost {
    fn id(&self) -> &'static str {
        "Lost"
    }

    fn name(&self) -> &'static str {
        "10_st"
    }

    fn sort_key(&self) -> &'static str {
        "ろすと"
    }

    fn help_message(&self) -> &'static str {
        r"■ exp10_deX
　特殊な10面ダイスをロールして、1の出目の数をカウントします。「a11_0ut」と「s10_th」を判定します。
　X: ダイス数（省略時 1）

■ g01_denX
　特殊な20面ダイスをロールして、「0_lation」かの判定をします。
　X: ダイス数（省略時 1）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["exp10_de", "g01_den"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Lost#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_explode(command) || roll_golden(command)
        if let Some(result) = roll_explode(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = roll_golden(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `/^exp10_de([+\-\d]+)?$/i`。
fn explode_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^exp10_de([+\-\d]+)?$").expect("valid regex"))
}

/// Ruby `/^g01_den([+\-\d]+)?$/i`。
fn golden_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^g01_den([+\-\d]+)?$").expect("valid regex"))
}

/// ダイス数の式を評価する。
///
/// Ruby: `times = m[1] ? Arithmetic.eval(m[1], round_type: round_type) : 1`
/// に続く `return nil if times.nil? || times <= 0`。
/// `Lost` は `round_type` を上書きしないので既定の切り捨て。
fn parse_times(digits: Option<&str>) -> Result<Option<i64>, EvalError> {
    let times = match digits {
        Some(source) => match arithmetic::eval(source, RoundType::Floor)? {
            Some(v) => v,
            None => return Ok(None),
        },
        None => I::ONE,
    };

    Ok((times > I::ZERO).then_some(crate::randomizer::sat_i64(&times)))
}

/// Ruby `times > 1 ? times.to_i : ""`。
fn times_str(times: i64) -> String {
    if times > 1 {
        times.to_string()
    } else {
        String::new()
    }
}

/// Ruby `values.join(',')`。
fn join_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `Lost#roll_explode`（`exp10_deX`）。
fn roll_explode(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = explode_pattern().captures(command) else {
        return Ok(None);
    };

    let Some(times) = parse_times(m.get(1).map(|g| g.as_str()))? else {
        return Ok(None);
    };

    // Ruby: 出目5以下を1、6以上を0に読み替える
    let values: Vec<i64> = rng
        .roll_barabara(times, 10)?
        .into_iter()
        .map(|v| i64::from(v <= 5))
        .collect();

    let is_allout = values.iter().all(|&v| v == 1);
    let is_sloth = values.iter().all(|&v| v == 0);

    let times_str = times_str(times);
    let joined = join_values(&values);

    let result = if is_allout {
        EvalResult::success(format!("(exp10_de{times_str}) ＞ [{joined}] ＞ a11_0ut"))
    } else if is_sloth {
        EvalResult::failure(format!("(exp10_de{times_str}) ＞ [{joined}] ＞ s10_th"))
    } else {
        let count_one = values.iter().filter(|&&v| v == 1).count();
        EvalResult::with_text(format!(
            "(exp10_de{times_str}) ＞ [{joined}] ＞ {count_one}"
        ))
    };

    Ok(Some(result))
}

/// Ruby `Lost#roll_golden`（`g01_denX`）。
fn roll_golden(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = golden_pattern().captures(command) else {
        return Ok(None);
    };

    let Some(times) = parse_times(m.get(1).map(|g| g.as_str()))? else {
        return Ok(None);
    };

    // Ruby: 出目1だけを1、それ以外を0に読み替える
    let values: Vec<i64> = rng
        .roll_barabara(times, 20)?
        .into_iter()
        .map(|v| i64::from(v == 1))
        .collect();

    let is_olation = values.iter().all(|&v| v == 0);

    let times_str = times_str(times);
    let joined = join_values(&values);

    let result = if is_olation {
        EvalResult::failure(format!("(g01_den{times_str}) ＞ [{joined}] ＞ 0_lation"))
    } else {
        EvalResult::with_text(format!("(g01_den{times_str}) ＞ [{joined}]"))
    };

    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Lost", "Lost.toml", 17);
    }
}
