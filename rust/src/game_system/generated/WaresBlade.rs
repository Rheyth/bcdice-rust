//! `lib/bcdice/game_system/WaresBlade.rb` の手書き移植。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaresBlade;

impl GameSystem for WaresBlade {
    fn id(&self) -> &'static str {
        "WaresBlade"
    }

    fn name(&self) -> &'static str {
        "ワースブレイド"
    }

    fn sort_key(&self) -> &'static str {
        "わあすふれいと"
    }

    fn help_message(&self) -> &'static str {
        "nD10>=m 方式の判定で成否、完全成功、完全失敗を自動判定します。\n"
    }

    fn result_nd10(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        values: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Ge {
            return None;
        }
        if values.iter().all(|value| *value == 10) {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "完全成功",
            ))))
        } else if values.iter().all(|value| *value == 1) {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "絶対失敗",
            ))))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/WaresBlade.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WaresBlade",
            "WaresBlade.toml",
            13,
        );
    }
}
