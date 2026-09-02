//! P4で手書き移植した `lib/bcdice/game_system/PhantasmAdventure.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `PhantasmAdventure#result_1d20`（技能値修正・クリティカル／ファンブル値計算）
//!
//! Ruby の `result_1d20` は判定のたびに `@randomizer.roll_once(20)` する。
//! Rust の [`GameSystem::result_1d20`] には乱数器が渡らないので、
//! [`GameSystem::check_result`] を上書きして 1D20 の枝だけここで処理する。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::PhantasmAdventure`（ID: `PhantasmAdventure`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhantasmAdventure;

impl GameSystem for PhantasmAdventure {
    fn id(&self) -> &'static str {
        "PhantasmAdventure"
    }

    fn name(&self) -> &'static str {
        "ファンタズム・アドベンチャー"
    }

    fn sort_key(&self) -> &'static str {
        "ふあんたすむあとへんちやあ"
    }

    fn help_message(&self) -> &'static str {
        r"成功、失敗、決定的成功、決定的失敗の表示とクリティカル・ファンブル値計算の実装。
"
    }

    /// Ruby `Base#check_result` のうち、1D20 を `PhantasmAdventure#result_1d20` 相当で処理する。
    ///
    /// このシステムは 1D20 以外の判定フックを持たないので、他面数は既定の
    /// [`GameSystem::result_ndx`] へ落とす（`result_1d100` / `result_2d6` / `result_nd6`
    /// の既定はいずれも `None`）。
    fn check_result(
        &self,
        total: crate::Int,
        rand_results: &[(i64, i64)],
        cmp_op: CmpOp,
        target: Target,
        rng: &mut Randomizer,
    ) -> Result<Option<EvalResult>, EvalError> {
        let sides_list: Vec<i64> = rand_results.iter().map(|r| r.0).collect();
        if sides_list.as_slice() == [20] {
            match result_1d20(&total, cmp_op, &target, rng)? {
                Some(CheckOutcome::Nothing) => return Ok(None),
                Some(CheckOutcome::Result(r)) => return Ok(Some(*r)),
                None => {}
            }
        }
        Ok(self.result_ndx(total, cmp_op, target))
    }
}

/// Ruby `PhantasmAdventure#result_1d20`。
fn result_1d20(
    total: &crate::Int,
    cmp_op: CmpOp,
    diff: &Target,
    rng: &mut Randomizer,
) -> Result<Option<CheckOutcome>, EvalError> {
    // Ruby: return Result.nothing if diff == '?'
    let Target::Number(diff) = diff else {
        return Ok(Some(CheckOutcome::Nothing));
    };
    // Ruby: return nil unless cmp_op == :<=
    if cmp_op != CmpOp::Le {
        return Ok(None);
    }

    // 技能値の修正を計算する
    let skill_mod = if *diff < I::ONE {
        diff.clone() - crate::Int::from(1)
    } else if *diff > I::from(20) {
        diff.clone() - crate::Int::from(20)
    } else {
        crate::Int::ZERO
    };

    // Ruby: fumble = 20 + skill_mod; fumble = 20 if fumble > 20
    let fumble = (crate::Int::from(20) + &skill_mod).min(crate::Int::from(20));
    let critical = crate::Int::from(1) + &skill_mod;
    let dice_now = rng.roll_once(20)?;

    if *total >= fumble || *total >= crate::Int::from(20) {
        // Ruby: fum_num を 1..=20 に丸める
        let fum_num = (crate::Int::from(dice_now) - &skill_mod)
            .clamp(crate::Int::from(1), crate::Int::from(20));

        let fum_str = if skill_mod < crate::Int::ZERO {
            format!("{dice_now}+{}={fum_num}", -&skill_mod)
        } else {
            format!("{dice_now}-{skill_mod}={fum_num}")
        };
        Ok(Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            format!("致命的失敗({fum_str})"),
        )))))
    } else if *total <= critical || *total <= crate::Int::from(1) {
        if skill_mod < crate::Int::ZERO {
            return Ok(Some(CheckOutcome::Result(Box::new(EvalResult::success(
                "成功",
            )))));
        }

        // Ruby: crit_num を 1..=20 に丸める
        let crit_num = (crate::Int::from(dice_now) + &skill_mod)
            .clamp(crate::Int::from(1), crate::Int::from(20));
        Ok(Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            format!("決定的成功({dice_now}+{skill_mod}={crit_num})"),
        )))))
    } else if *total <= *diff {
        Ok(Some(CheckOutcome::Result(Box::new(EvalResult::success(
            "成功",
        )))))
    } else {
        Ok(Some(CheckOutcome::Result(Box::new(EvalResult::failure(
            "失敗",
        )))))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "PhantasmAdventure",
            "PhantasmAdventure.toml",
            31,
        );
    }
}
