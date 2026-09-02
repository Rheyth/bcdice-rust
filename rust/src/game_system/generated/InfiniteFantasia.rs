//! P4で手書き移植した `lib/bcdice/game_system/InfiniteFantasia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `InfiniteFantasia#result_1d20`（1D20の成功レベル判定）

use crate::arithmetic::floor_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::InfiniteFantasia`（ID: `InfiniteFantasia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfiniteFantasia;

impl GameSystem for InfiniteFantasia {
    fn id(&self) -> &'static str {
        "InfiniteFantasia"
    }

    fn name(&self) -> &'static str {
        "無限のファンタジア"
    }

    fn sort_key(&self) -> &'static str {
        "むけんのふあんたしあ"
    }

    fn help_message(&self) -> &'static str {
        r"1D20に目標値を設定した場合に、成功レベルの自動判定を行います。
例： 1D20<=16
"
    }

    /// Ruby `InfiniteFantasia#result_1d20`。
    fn result_1d20(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == '?'
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        if total > target {
            return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
        }

        let mut output = if total <= floor_div(target.clone(), I::from(32)) {
            "32レベル成功(32Lv+)".to_owned()
        } else if total <= floor_div(target.clone(), I::from(16)) {
            "16レベル成功(16Lv+)".to_owned()
        } else if total <= floor_div(target.clone(), I::from(8)) {
            "8レベル成功".to_owned()
        } else if total <= floor_div(target.clone(), I::from(4)) {
            "4レベル成功".to_owned()
        } else if total <= floor_div(target.clone(), I::from(2)) {
            "2レベル成功".to_owned()
        } else {
            "1レベル成功".to_owned()
        };

        // Ruby: Result.new.tap { r.text = output; r.success = true;
        //        if total <= 1; r.critical = true; r.text += "/クリティカル"; end }
        let result = if total <= I::ONE {
            output.push_str("/クリティカル");
            EvalResult::critical(output)
        } else {
            EvalResult::success(output)
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "InfiniteFantasia",
            "InfiniteFantasia.toml",
            14,
        );
    }
}
