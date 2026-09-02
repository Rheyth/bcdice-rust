//! P4で手書き移植した `lib/bcdice/game_system/Elric.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `Elric#result_1d100`（1D100の貫通・クリティカル・ファンブル判定）

use crate::arithmetic::ceil_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Elric`（ID: `Elric`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elric;

impl GameSystem for Elric {
    fn id(&self) -> &'static str {
        "Elric"
    }

    fn name(&self) -> &'static str {
        "エルリック！"
    }

    fn sort_key(&self) -> &'static str {
        "えるりつく"
    }

    fn help_message(&self) -> &'static str {
        "貫通、クリティカル、ファンブルの自動判定を行います。\n"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Elric#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        let result = if total <= I::ONE {
            EvalResult::critical("貫通") // 1は常に貫通
        } else if total >= I::from(100) {
            EvalResult::fumble("致命的失敗") // 100は常に致命的失敗
        } else if let Target::Number(target) = target {
            if total <= ceil_div(target.clone(), I::from(5)).unwrap_or(I::from(i64::MAX)) {
                EvalResult::critical("決定的成功")
            } else if total <= target {
                EvalResult::success("成功")
            } else if (total >= I::from(99)) && (target < I::from(100)) {
                EvalResult::fumble("致命的失敗")
            } else {
                EvalResult::failure("失敗")
            }
        } else {
            // Ruby: target == '?' のとき Result.nothing
            EvalResult::new()
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/Elric.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Elric", "Elric.toml", 12);
    }
}
