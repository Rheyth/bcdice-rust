//! P4で手書き移植した `lib/bcdice/game_system/NervWhitePaper.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NervWhitePaper#resolute_regular_action`（通常ロール `NR`）
//! - `NervWhitePaper#resolute_advantage_action`（長所ロール `NA`）
//! - `NervWhitePaper#resolute_disadvantage_action`（短所ロール `ND`）

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::NervWhitePaper`（ID: `NervWhitePaper`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NervWhitePaper;

impl GameSystem for NervWhitePaper {
    fn id(&self) -> &'static str {
        "NervWhitePaper"
    }

    fn name(&self) -> &'static str {
        "新世紀エヴァンゲリオンRPG NERV白書/使徒降臨"
    }

    fn sort_key(&self) -> &'static str {
        "しんせいきえうあんけりおんああるひいしいねるふはくしよしとこおりん"
    }

    fn help_message(&self) -> &'static str {
        r"■通常ロール(NR)：成功、失敗、絶対成功、絶対失敗を表示します。
例) NR

■長所ロール(NA)：成功、失敗、絶対成功、絶対失敗を表示します。
例) NA

■短所ロール(ND)：成功、失敗、絶対成功、絶対失敗を表示します。
例) ND

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["N[RAD]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `NervWhitePaper#eval_game_system_specific_command`。
    ///
    /// Ruby: `resolute_regular_action || resolute_advantage_action || resolute_disadvantage_action`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        for (needle, kind) in [
            ("NR", RollKind::Regular),
            ("NA", RollKind::Advantage),
            ("ND", RollKind::Disadvantage),
        ] {
            // Ruby: /NR/.match(command) — 先頭固定ではない部分一致
            if command.contains(needle) {
                return Ok(resolute_action(needle, kind, rng)?.map(SpecificCommandOutput::result));
            }
        }

        Ok(None)
    }
}

/// ロールの種別。ゾロ目・偶数などの「失敗」条件だけが異なる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollKind {
    /// Ruby `resolute_regular_action`（通常ロール）
    Regular,
    /// Ruby `resolute_advantage_action`（長所ロール）
    Advantage,
    /// Ruby `resolute_disadvantage_action`（短所ロール）
    Disadvantage,
}

/// Ruby `resolute_regular_action` / `resolute_advantage_action` / `resolute_disadvantage_action`。
///
/// 3メソッドは「合計7 → 絶対成功」「合計2または12 → 絶対失敗」までが共通で、
/// 残りの分岐だけが違う。短所ロールだけは `elsif dice_add != 7` で終わるため、
/// どの条件にも当てはまらない場合に Ruby が `nil` を返す形になっている
/// （合計7は上の分岐で返るので実際には到達しない）。
fn resolute_action(
    label: &str,
    kind: RollKind,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let dices = rng.roll_barabara(2, 6)?;
    let dice_text = dices
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let dice_add: i64 = dices.iter().sum();

    let output = format!("({label}) ＞ {dice_text}");

    if dice_add == 7 {
        return Ok(Some(EvalResult::critical(format!("{output} ＞ 絶対成功"))));
    }
    if dice_add == 2 || dice_add == 12 {
        return Ok(Some(EvalResult::fumble(format!("{output} ＞ 絶対失敗"))));
    }

    let is_failure = match kind {
        // Ruby: dice_add.modulo(2) == 0
        RollKind::Regular => dice_add % 2 == 0,
        // Ruby: dices[0] == dices[1]
        RollKind::Advantage => dices[0] == dices[1],
        // Ruby: dice_add != 7（ここに来る時点で常に真）
        RollKind::Disadvantage => dice_add != 7,
    };

    if is_failure {
        return Ok(Some(EvalResult::failure(format!("{output} ＞ 失敗"))));
    }

    match kind {
        // Ruby: 短所ロールは else 節が無く、暗黙に nil を返す
        RollKind::Disadvantage => Ok(None),
        _ => Ok(Some(EvalResult::success(format!("{output} ＞ 成功")))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NervWhitePaper",
            "NervWhitePaper.toml",
            14,
        );
    }
}
