//! P4で手書き移植した `lib/bcdice/game_system/Hieizan.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Hieizan#result_1d100`（大成功・自動成功・失敗・自動失敗・大失敗）

use crate::arithmetic::floor_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Hieizan`（ID: `Hieizan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hieizan;

impl GameSystem for Hieizan {
    fn id(&self) -> &'static str {
        "Hieizan"
    }

    fn name(&self) -> &'static str {
        "比叡山炎上"
    }

    fn sort_key(&self) -> &'static str {
        "ひえいさんえんしよう"
    }

    fn help_message(&self) -> &'static str {
        r"大成功、自動成功、失敗、自動失敗、大失敗の自動判定を行います。
"
    }

    /// Ruby `Hieizan#result_1d100`。
    fn result_1d100(
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

        let result = if total >= I::from(100) {
            EvalResult::fumble("大失敗")
        } else if total >= I::from(96) {
            EvalResult::failure("自動失敗")
        } else if total <= floor_div(target.clone(), I::from(5)) {
            EvalResult::critical("大成功")
        } else if total <= I::ONE {
            EvalResult::success("自動成功")
        } else if total <= target {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Hieizan", "Hieizan.toml", 14);
    }
}
