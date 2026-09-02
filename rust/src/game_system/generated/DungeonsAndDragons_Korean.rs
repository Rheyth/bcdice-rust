//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DungeonsAndDragons` を継承し、`check_result` を上書きして
//! 成功／失敗／クリティカル／ファンブルの文言を韓国語にする。
//! 1D20 では出目 20 をクリティカル、出目 1 をファンブルとする。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `DungeonsAndDragons_Korean#success_text`。
const SUCCESS_TEXT: &str = "성공";
/// Ruby `DungeonsAndDragons_Korean#failure_text`。
const FAILURE_TEXT: &str = "실패";
/// Ruby `DungeonsAndDragons_Korean#critical_text`。
const CRITICAL_TEXT: &str = "크리티컬";
/// Ruby `DungeonsAndDragons_Korean#fumble_text`。
const FUMBLE_TEXT: &str = "펌블";

/// Ruby `BCDice::GameSystem::DungeonsAndDragons_Korean`（ID: `DungeonsAndDragons:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons_Korean;

impl GameSystem for DungeonsAndDragons_Korean {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons:Korean"
    }

    fn name(&self) -> &'static str {
        "던전 앤 드래곤"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:던전 앤 드래곤"
    }

    fn help_message(&self) -> &'static str {
        r"※ 이 다이스봇은 방의 시스템 이름 표시용입니다.
"
    }

    /// Ruby `DungeonsAndDragons_Korean#check_result`。
    ///
    /// 親の `Base#check_result` は呼ばない（Ruby も `super` しない）。
    fn check_result(
        &self,
        total: crate::Int,
        rand_results: &[(i64, i64)],
        cmp_op: CmpOp,
        target: Target,
        _rng: &mut Randomizer,
    ) -> Result<Option<EvalResult>, EvalError> {
        // Ruby: return nil if target.is_a?(String)
        let Target::Number(target) = target else {
            return Ok(None);
        };

        // Ruby: dice_total = rand_results.map(&:value).sum
        let dice_total: i64 = rand_results.iter().fold(0i64, |a, r| a.wrapping_add(r.1));
        // Ruby: rand_results.map(&:sides) == [20]
        let sides: Vec<i64> = rand_results.iter().map(|r| r.0).collect();
        if sides.as_slice() == [20] {
            if dice_total == 20 {
                return Ok(Some(EvalResult::critical(CRITICAL_TEXT)));
            }
            if dice_total == 1 {
                return Ok(Some(EvalResult::fumble(FUMBLE_TEXT)));
            }
        }

        // Ruby: total.send(cmp_op, target)
        if cmp_op.apply(&total, &target) {
            Ok(Some(EvalResult::success(SUCCESS_TEXT)))
        } else {
            Ok(Some(EvalResult::failure(FAILURE_TEXT)))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DungeonsAndDragons:Korean",
            "DungeonsAndDragons_Korean.toml",
            8,
        );
    }
}
