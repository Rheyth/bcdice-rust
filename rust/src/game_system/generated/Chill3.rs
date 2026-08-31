//! P4で手書き移植した `lib/bcdice/game_system/Chill3.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `Chill3#result_1d100`（1D100の成否・Botch判定）

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Chill3`（ID: `Chill3`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chill3;

impl GameSystem for Chill3 {
    fn id(&self) -> &'static str {
        "Chill3"
    }

    fn name(&self) -> &'static str {
        "Chill 3rd Edition"
    }

    fn sort_key(&self) -> &'static str {
        "ちる3"
    }

    fn help_message(&self) -> &'static str {
        r"・1D100で判定時に成否、Botchを判定
　例）1D100<=50
　　 (1D100<=50) ＞ 55 ＞ Botch
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Chill3#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil if target == '?'
        let Target::Number(target) = target else {
            return None;
        };
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        // ゾロ目ならC-ResultかBotch
        let tens = (dice_total / 10) % 10;
        let ones = dice_total % 10;

        let result = if tens == ones {
            if (total > target) || (dice_total == 100) {
                // 00は必ず失敗
                if target > I::from(100) {
                    // 目標値が100を超えている場合は、00を振ってもBotchにならない
                    EvalResult::failure("Failure")
                } else {
                    EvalResult::fumble("Botch")
                }
            } else {
                EvalResult::critical("Colossal Success")
            }
        } else if (total <= target) || (dice_total == 1) {
            // 01は必ず成功
            if total <= target / 2 {
                EvalResult::success("High Success")
            } else {
                EvalResult::success("Low Success")
            }
        } else {
            EvalResult::failure("Failure")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/Chill3.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        super::super::SwordWorld2_0::assert_toml_cases("Chill3", "Chill3.toml", 17);
    }
}
