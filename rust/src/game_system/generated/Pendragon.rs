//! P4で手書き移植した `lib/bcdice/game_system/Pendragon.rb`。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pendragon;

impl GameSystem for Pendragon {
    fn id(&self) -> &'static str {
        "Pendragon"
    }

    fn name(&self) -> &'static str {
        "ペンドラゴン"
    }

    fn sort_key(&self) -> &'static str {
        "へんとらこん"
    }

    fn help_message(&self) -> &'static str {
        "クリティカル、成功、失敗、ファンブルの自動判定を行います。\n"
    }

    /// Ruby `Pendragon#result_1d20`。
    fn result_1d20(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        if cmp_op != CmpOp::Le {
            return None;
        }

        let result = if total <= target {
            if total >= I::from(40) - target.clone() || total == target {
                EvalResult::critical("クリティカル")
            } else {
                EvalResult::success("成功")
            }
        } else if total == I::from(20) {
            EvalResult::fumble("ファンブル")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}
