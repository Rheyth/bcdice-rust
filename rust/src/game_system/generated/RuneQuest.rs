//! P4で手書き移植した `lib/bcdice/game_system/RuneQuest.rb`。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuneQuest;

impl GameSystem for RuneQuest {
    fn id(&self) -> &'static str {
        "RuneQuest"
    }

    fn name(&self) -> &'static str {
        "ルーンクエスト"
    }

    fn sort_key(&self) -> &'static str {
        "るうんくえすと"
    }

    fn help_message(&self) -> &'static str {
        "クリティカル、エフェクティブ(効果的成功)、ファンブルの自動判定を行います。\n"
    }

    /// Ruby `RuneQuest#result_1d100`。
    fn result_1d100(
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

        let critical_value = ((sat_i64(&target) as f64) / 20.0).round() as i64;
        let result = if total <= I::ONE || total <= I::from(critical_value) {
            EvalResult::critical("決定的成功")
        } else if total >= I::from(100) {
            EvalResult::fumble("致命的失敗")
        } else if total <= I::from((sat_i64(&target) as f64 / 5.0).round() as i64) {
            EvalResult::success("効果的成功")
        } else if total <= target {
            EvalResult::success("成功")
        } else if total >= I::from(95 + critical_value) {
            EvalResult::fumble("致命的失敗")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}
