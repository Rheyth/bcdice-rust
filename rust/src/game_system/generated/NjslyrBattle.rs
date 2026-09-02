//! P4で手書き移植した `lib/bcdice/game_system/NjslyrBattle.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NjslyrBattle#result_2d6`（カラテロール）と `#juuten`（重点）

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::NjslyrBattle`（ID: `NjslyrBattle`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NjslyrBattle;

impl GameSystem for NjslyrBattle {
    fn id(&self) -> &'static str {
        "NjslyrBattle"
    }

    fn name(&self) -> &'static str {
        "NJSLYRBATTLE"
    }

    fn sort_key(&self) -> &'static str {
        "にんしやすれいやあはとる"
    }

    fn help_message(&self) -> &'static str {
        r"・カラテロール
2d6<=(カラテ点)
例）2d6<=5
(2D6<=5) ＞ 2[1,1] ＞ 2 ＞ 成功 重点 3 溜まる
"
    }

    /// Ruby `NjslyrBattle#result_2d6`。
    fn result_2d6(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == "?"
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return nil if cmp_op != :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        let mut result = if total <= target {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        };
        result.text.push_str(&juuten(value_list));
        Some(CheckOutcome::Result(Box::new(result)))
    }
}

/// Ruby `NjslyrBattle#juuten`。
fn juuten(dice_list: &[i64]) -> String {
    let mut juuten = dice_list.iter().filter(|&&d| d == 1).count()
        + dice_list.iter().filter(|&&d| d == 6).count();

    // Ruby: if dice_list[0] == dice_list[1]
    if dice_list.len() >= 2 && dice_list[0] == dice_list[1] {
        juuten += 1;
    }

    if juuten > 0 {
        format!(" 重点 {juuten} 溜まる")
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NjslyrBattle",
            "NjslyrBattle.toml",
            9,
        );
    }
}
