//! P4で手書き移植した `lib/bcdice/game_system/JamesBond.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `JamesBond#result_1d100`（1D100の効果レーティング判定）

use crate::arithmetic::floor_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::JamesBond`（ID: `JamesBond`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JamesBond;

impl GameSystem for JamesBond {
    fn id(&self) -> &'static str {
        "JamesBond"
    }

    fn name(&self) -> &'static str {
        "ジェームズ・ボンド007"
    }

    fn sort_key(&self) -> &'static str {
        "しええむすほんと007"
    }

    fn help_message(&self) -> &'static str {
        r"・1D100の目標値判定で、効果レーティングを1～4で自動判定。
　例）1D100<=50
　　　JamesBond : (1D100<=50) ＞ 20 ＞ 効果3（良）
"
    }

    /// Ruby `JamesBond#result_1d100`。
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

        // Ruby: base = ((target + 9) / 10).floor
        // Integer#/ は床除算で、Integer#floor は恒等。
        let base = floor_div(target.clone() + I::from(9), I::from(10));

        let result = if total >= I::from(100) {
            // 100は常に失敗
            EvalResult::failure("失敗")
        } else if total <= base {
            EvalResult::success("効果1（完璧）")
        } else if total <= &base * 2 {
            EvalResult::success("効果2（かなり良い）")
        } else if total <= &base * 5 {
            EvalResult::success("効果3（良）")
        } else if total <= target {
            EvalResult::success("効果4（まあまあ）")
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
        crate::game_system::test_support::assert_toml_cases_strict(
            "JamesBond",
            "JamesBond.toml",
            17,
        );
    }
}
