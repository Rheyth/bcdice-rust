//! P4で手書き移植した `lib/bcdice/game_system/EclipsePhase.rb`。

use crate::game_system::int_helpers::int_rem_euclid;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EclipsePhase;

impl GameSystem for EclipsePhase {
    fn id(&self) -> &'static str {
        "EclipsePhase"
    }

    fn name(&self) -> &'static str {
        "エクリプス・フェイズ"
    }

    fn sort_key(&self) -> &'static str {
        "えくりふすふえいす"
    }

    fn help_message(&self) -> &'static str {
        "1D100<=m 方式の判定で成否、クリティカル・ファンブルを自動判定"
    }

    /// Ruby `EclipsePhase#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op != CmpOp::Le {
            return None;
        }

        let dice_value = int_rem_euclid(&total, 100);
        let dice_ten_place = &dice_value / 10;
        let dice_one_place = &dice_value % 10;

        let result = if dice_ten_place == dice_one_place {
            if dice_value == I::from(99) {
                EvalResult::fumble("決定的失敗")
            } else if dice_value == I::ZERO {
                EvalResult::critical("00 ＞ 決定的成功")
            } else if total <= target {
                EvalResult::critical("決定的成功")
            } else {
                EvalResult::fumble("決定的失敗")
            }
        } else if total <= target {
            if total >= I::from(30) {
                EvalResult::success("エクセレント")
            } else {
                EvalResult::success("成功")
            }
        } else if total - target >= I::from(30) {
            EvalResult::failure("シビア")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}
