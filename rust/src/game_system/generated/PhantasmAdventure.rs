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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/PhantasmAdventure.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/PhantasmAdventure.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/PhantasmAdventure.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("PhantasmAdventure.toml must parse");
        assert_eq!(
            data.tests.len(),
            31,
            "case count in test/data/PhantasmAdventure.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "PhantasmAdventure",
                "unexpected game system in PhantasmAdventure.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("PhantasmAdventure"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL PhantasmAdventure:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} PhantasmAdventure cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
