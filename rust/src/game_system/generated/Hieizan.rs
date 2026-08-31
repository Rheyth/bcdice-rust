//! P4で手書き移植した `lib/bcdice/game_system/Hieizan.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Hieizan#result_1d100`（大成功・自動成功・失敗・自動失敗・大失敗）

use crate::arithmetic::floor_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Hieizan`（ID: `Hieizan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hieizan;

impl GameSystem for Hieizan {
    fn id(&self) -> &'static str {
        "Hieizan"
    }

    fn name(&self) -> &'static str {
        "比叡山炎上"
    }

    fn sort_key(&self) -> &'static str {
        "ひえいさんえんしよう"
    }

    fn help_message(&self) -> &'static str {
        r"大成功、自動成功、失敗、自動失敗、大失敗の自動判定を行います。
"
    }

    /// Ruby `Hieizan#result_1d100`。
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

        let result = if total >= I::from(100) {
            EvalResult::fumble("大失敗")
        } else if total >= I::from(96) {
            EvalResult::failure("自動失敗")
        } else if total <= floor_div(target.clone(), I::from(5)) {
            EvalResult::critical("大成功")
        } else if total <= I::ONE {
            EvalResult::success("自動成功")
        } else if total <= target {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
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
            .join("test/data/Hieizan.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Hieizan.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Hieizan.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Hieizan.toml must parse");
        assert_eq!(data.tests.len(), 14, "case count in test/data/Hieizan.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Hieizan",
                "unexpected game system in Hieizan.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Hieizan"), &tc.input, &mut src) {
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
                    "FAIL Hieizan:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Hieizan cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
