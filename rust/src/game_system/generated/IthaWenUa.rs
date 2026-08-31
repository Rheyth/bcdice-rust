//! P4で手書き移植した `lib/bcdice/game_system/IthaWenUa.rb`。

use crate::game_system::int_helpers::int_rem_euclid;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::result::{CheckOutcome, EvalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IthaWenUa;

impl GameSystem for IthaWenUa {
    fn id(&self) -> &'static str {
        "IthaWenUa"
    }

    fn name(&self) -> &'static str {
        "イサー・ウェン＝アー"
    }

    fn sort_key(&self) -> &'static str {
        "いさあうえんああ"
    }

    fn help_message(&self) -> &'static str {
        "1D100<=m 方式の判定で成否、クリティカル(01)・ファンブル(00)を自動判定します。\n"
    }

    /// Ruby `IthaWenUa#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Le {
            return None;
        }

        match sat_i64(&int_rem_euclid(&total, 100)) {
            1 => Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "01 ＞ クリティカル",
            )))),
            0 => Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "00 ＞ ファンブル",
            )))),
            _ if matches!(target, Target::Question) => Some(CheckOutcome::Nothing),
            _ => None,
        }
    }
}
