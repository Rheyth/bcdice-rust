//! P4で手書き移植した `lib/bcdice/game_system/NanimonaiMura.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NanimonaiMura#eval_game_system_specific_command` → `#roll_action`（行為判定 `xNMy`）
//! - `#normalize_d10` / `#select_best_result` / `#build_result` / `#zoro_bonus`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `STAGE_NAMES`。
static STAGE_NAMES: &[&str] = &["ノーマル", "エピック", "レジェンダリ", "ミシック"];

/// Ruby `BASE_SUCCESS_LEVELS`。
static BASE_SUCCESS_LEVELS: &[i64] = &[1, 11, 21, 31];

/// Ruby `/\A(\d+)NM(\d+)\z/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(\d+)NM(\d+)\z").expect("valid regex"))
}

/// Ruby `NanimonaiMura::ResultData`。
struct ResultData {
    /// 達成値
    score: i64,
    /// 成功したか
    success: bool,
    /// 大成功（ゾロ目）か
    critical: bool,
    /// 結果の表示文字列
    result_text: String,
    /// 成功度
    success_level: i64,
}

/// Ruby `NanimonaiMura#roll_action`。
fn roll_action(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = to_i(&m[1]);
    let target = to_i(&m[2]);
    if dice_count < 1 {
        return Ok(None);
    }

    let dice_list: Vec<i64> = rng
        .roll_barabara(dice_count, 10)?
        .into_iter()
        .map(normalize_d10)
        .collect();

    // Ruby側は `roll_barabara` の個数上限（200個）を超えると空配列が返り、
    // 候補が1件も作れずに `nil.score` で NoMethodError になる（本家のバグ）。
    // ここでは同じ入力を nil（コマンド不成立）に畳む。
    let Some(result) = select_best_result(&dice_list, target) else {
        return Ok(None);
    };

    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut eval_result = EvalResult::with_text(format!(
        "{command} ({dice_count}D10) ＞ [{dice_str}] ＞ {} ＞ {} ＞ 成功度{}",
        result.score, result.result_text, result.success_level
    ));
    // Ruby: r.condition= は success / failure の両方を、r.critical= は critical だけを立てる
    eval_result.set_condition(result.success);
    eval_result.critical = result.critical;

    Ok(Some(SpecificCommandOutput::result(eval_result)))
}

/// Ruby `NanimonaiMura#normalize_d10`。10の目を0として扱う。
fn normalize_d10(value: i64) -> i64 {
    if value == 10 {
        0
    } else {
        value
    }
}

/// Ruby `NanimonaiMura#select_best_result`。
///
/// 成功度が最も高くなる組み合わせを選び、同値なら達成値が低い方を優先する。
fn select_best_result(dice_list: &[i64], target: i64) -> Option<ResultData> {
    let candidates: Vec<ResultData> = if dice_list.len() == 1 {
        vec![build_result(dice_list[0] * 10, target)]
    } else {
        let mut candidates = Vec::new();
        for (i, tens) in dice_list.iter().enumerate() {
            for (j, ones) in dice_list.iter().enumerate() {
                if i == j {
                    continue;
                }
                candidates.push(build_result(tens * 10 + ones, target));
            }
        }
        candidates
    };

    let mut candidates = candidates.into_iter();
    let mut best = candidates.next()?;
    for candidate in candidates {
        // Ruby: next if candidate.success_level < best.success_level
        if candidate.success_level < best.success_level {
            continue;
        }
        // Ruby: next if 同成功度で達成値が高い（低い方を優先する）
        if candidate.success_level == best.success_level && candidate.score > best.score {
            continue;
        }
        best = candidate;
    }
    Some(best)
}

/// Ruby `NanimonaiMura#build_result`。
fn build_result(score: i64, target: i64) -> ResultData {
    if score > target {
        return ResultData {
            score,
            success: false,
            critical: false,
            result_text: "失敗".to_owned(),
            success_level: 0,
        };
    }

    let diff = target - score;
    // Ruby `Integer#div` は床除算
    let stage_index = (diff).div_euclid(100).min(STAGE_NAMES.len() as i64 - 1) as usize;
    let critical_bonus = zoro_bonus(score);
    // Ruby: critical = !critical_bonus.nil?（ボーナス0でも大成功）
    let critical = critical_bonus.is_some();
    let success_level = BASE_SUCCESS_LEVELS[stage_index] + critical_bonus.unwrap_or(0);
    let result_text = format!(
        "{}{}",
        STAGE_NAMES[stage_index],
        if critical { "大成功" } else { "成功" }
    );

    ResultData {
        score,
        success: true,
        critical,
        result_text,
        success_level,
    }
}

/// Ruby `NanimonaiMura#zoro_bonus`。十の位と一の位が同じならその値を返す。
fn zoro_bonus(score: i64) -> Option<i64> {
    let tens = (score).div_euclid(10);
    let ones = score.rem_euclid(10);
    (tens == ones).then_some(ones)
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない指定は飽和させる。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `BCDice::GameSystem::NanimonaiMura`（ID: `NanimonaiMura`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NanimonaiMura;

impl GameSystem for NanimonaiMura {
    fn id(&self) -> &'static str {
        "NanimonaiMura"
    }

    fn name(&self) -> &'static str {
        "なにもない村"
    }

    fn sort_key(&self) -> &'static str {
        "なにもないむら"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定 xNMy
　x個のD10を振り、2つの出目から達成値を作成して判定します。
　達成値は成功度が最も高くなる組み合わせを自動選択し、同値なら低い値を優先します。
　x: ダイス数（1以上）
　y: 判定値（0以上）
　例）4NM55 6NM199 2NM35
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+NM\d+"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_action(command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NanimonaiMura",
            "NanimonaiMura.toml",
            10,
        );
    }
}
