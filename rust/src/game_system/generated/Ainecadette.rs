//! P4で手書き移植した `lib/bcdice/game_system/Ainecadette.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Ainecadette#roll_action`（先輩 `nAI` / 後輩 `nCA` の行為判定）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `SUCCESS_THRESHOLD`（成功の目標値）。
const SUCCESS_THRESHOLD: i64 = 4;

/// Ruby `SPECIAL_DICE`（スペシャルとなる出目）。
const SPECIAL_DICE: i64 = 6;

/// Ruby `BCDice::GameSystem::Ainecadette`（ID: `Ainecadette`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ainecadette;

impl GameSystem for Ainecadette {
    fn id(&self) -> &'static str {
        "Ainecadette"
    }

    fn name(&self) -> &'static str {
        "エネカデット"
    }

    fn sort_key(&self) -> &'static str {
        "えねかてつと"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定
- 先輩 (AI) 10面ダイスを2つ振って判定します。『有利』なら【3AI】、『不利』なら【1AI】を使います。
- 後輩 (CA) 6面ダイスを2つ振って判定します。『有利』なら【3CA】、『不利』なら【1CA】を使います。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?AI", r"(\d+)?CA"]
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

/// Ruby `roll_action` のコマンド抽出（`/^(\d+)?(AI|CA)$/`）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)?(AI|CA)$").expect("valid regex"))
}

/// Ruby `Ainecadette#roll_action`。
fn roll_action(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: return nil unless m
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let is_senpai = &m[2] == "AI";

    // Ruby: times = m[1]&.to_i || 2
    // 桁あふれする入力は Ruby だと Bignum のまま roll_barabara に渡り、
    // 個数上限（200）を超えて空配列になる。i64 に収まらない場合も同じ経路へ落とす。
    let times: i64 = match m.get(1) {
        Some(digits) => digits.as_str().parse().unwrap_or(i64::MAX),
        None => 2,
    };
    let sides = if is_senpai { 10 } else { 6 };
    // Ruby: return nil if times <= 0
    if times <= 0 {
        return Ok(None);
    }

    let dice_list = rng.roll_barabara(times, sides)?;
    // Ruby `Array#max`。空配列（個数上限超え）では nil になり、続く `max <= 1` が
    // NoMethodError でクラッシュする。ここでは 0 として扱う。
    let max = dice_list.iter().copied().max().unwrap_or(0);

    let mut result = if max <= 1 {
        EvalResult::fumble("ファンブル（もやもやカウンターを2個獲得）")
    } else if dice_list.contains(&SPECIAL_DICE) {
        let me = if is_senpai { "先輩" } else { "後輩" };
        let target = if is_senpai { "後輩" } else { "先輩" };
        EvalResult::critical(format!(
            "スペシャル（絆カウンターを1個獲得し、{target}は{me}への感情を1つ獲得）"
        ))
    } else if max >= SUCCESS_THRESHOLD {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };

    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    result.text = format!("({command}) ＞ [{dice_str}] ＞ {}", result.text);

    Ok(Some(SpecificCommandOutput::result(result)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Ainecadette",
            "Ainecadette.toml",
            19,
        );
    }
}
