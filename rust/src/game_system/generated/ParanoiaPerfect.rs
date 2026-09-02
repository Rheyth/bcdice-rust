//! P4で手書き移植した `lib/bcdice/game_system/ParanoiaPerfect.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ParanoiaPerfect#get_node_dice_roll`（ノードダイス判定 `NDx,y`）
//! - `ParanoiaPerfect#generate_roll_results`（コンピュータダイスの表示）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::ParanoiaPerfect`（ID: `ParanoiaPerfect`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParanoiaPerfect;

impl GameSystem for ParanoiaPerfect {
    fn id(&self) -> &'static str {
        "ParanoiaPerfect"
    }

    fn name(&self) -> &'static str {
        "パラノイア・パーフェクト エディション"
    }

    fn sort_key(&self) -> &'static str {
        "はらのいあはあふえくとえていしよん"
    }

    fn help_message(&self) -> &'static str {
        r"※コマンドは入力内容の前方一致で検出しています。
・通常の判定　NDx,y
　x：ノードダイスの数.マイナスも可.
　y: 反逆スターの数.省略可.省略時0
　ノードダイスの絶対値 + 1個(コンピュータダイス)のダイスがロールされる.
例）ND2　ND-3　ND2,1　ND-3,2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ND"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ParanoiaPerfect#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        get_node_dice_roll(command, rng)
    }
}

/// Ruby `/^ND((-)?\d+)(,(\d+))?$/i`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn node_dice_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^ND((-)?[0-9]+)(,([0-9]+))?$").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i_saturating(text: &str) -> i64 {
    str_helpers::to_i_signed_saturating(text)
}

/// Ruby `ParanoiaPerfect#get_node_dice_roll`。
fn get_node_dice_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = node_dice_pattern().captures(command) else {
        return Ok(None);
    };

    let parameter_num = to_i_saturating(&m[1]);
    // Ruby: m[4].to_i（nil.to_i は 0）
    let traitorous_star = m.get(4).map_or(0, |g| to_i_saturating(g.as_str()));
    let dice_count = parameter_num.saturating_abs().saturating_add(1);

    let dices = rng.roll_barabara(dice_count, 6)?;

    let mut success_rate = dices.iter().filter(|d| **d >= 5).count() as i64;
    if parameter_num < 0 {
        success_rate -= dices.iter().filter(|d| **d < 5).count() as i64;
    }

    let (results, computer_dice_message) = generate_roll_results(traitorous_star, &dices);

    Ok(Some(SpecificCommandOutput::text(format!(
        "({command}) ＞ [{}] ＞ 成功度{success_rate}{computer_dice_message}",
        results.join(", ")
    ))))
}

/// Ruby `ParanoiaPerfect#generate_roll_results`。
///
/// 最後のダイス（コンピュータダイス）が `6 - 反逆スター` 以上なら
/// 出目に `C` を付けて `(Computer)` を添える。
fn generate_roll_results(traitorous_star: i64, dices: &[i64]) -> (Vec<String>, &'static str) {
    let mut results: Vec<String> = dices.iter().map(|d| d.to_string()).collect();

    // Ruby: last_die = results[-1].to_i（空配列なら nil.to_i で 0）
    let Some(last_die) = dices.last().copied() else {
        return (results, "");
    };

    if last_die >= 6 - traitorous_star {
        let last = results.len() - 1;
        results[last] = format!("{last_die}C");
        return (results, "(Computer)");
    }

    (results, "")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ParanoiaPerfect",
            "ParanoiaPerfect.toml",
            15,
        );
    }
}
