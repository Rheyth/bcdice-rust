//! `lib/bcdice/game_system/WARPS.rb` の手書き移植。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WARPS;

impl GameSystem for WARPS {
    fn id(&self) -> &'static str {
        "WARPS"
    }

    fn name(&self) -> &'static str {
        "ワープス"
    }

    fn sort_key(&self) -> &'static str {
        "わあふす"
    }

    fn help_message(&self) -> &'static str {
        "失敗、成功度の自動判定を行います。\n"
    }

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Le {
            return None;
        }

        Some(if dice_total <= 2 {
            CheckOutcome::Result(Box::new(EvalResult::critical("クリティカル")))
        } else if dice_total >= 12 {
            CheckOutcome::Result(Box::new(EvalResult::fumble("ファンブル")))
        } else if let Target::Number(target) = target {
            if total <= target {
                CheckOutcome::Result(Box::new(EvalResult::success(format!(
                    "{}成功",
                    target - total
                ))))
            } else {
                CheckOutcome::Result(Box::new(EvalResult::failure("失敗")))
            }
        } else {
            CheckOutcome::Nothing
        })
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/WARPS.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("WARPS", "WARPS.toml", 31);
    }
}
