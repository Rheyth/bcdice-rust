//! P4で手書き移植した `lib/bcdice/game_system/InfiniteFantasia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `InfiniteFantasia#result_1d20`（1D20の成功レベル判定）

use crate::arithmetic::floor_div;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::InfiniteFantasia`（ID: `InfiniteFantasia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfiniteFantasia;

impl GameSystem for InfiniteFantasia {
    fn id(&self) -> &'static str {
        "InfiniteFantasia"
    }

    fn name(&self) -> &'static str {
        "無限のファンタジア"
    }

    fn sort_key(&self) -> &'static str {
        "むけんのふあんたしあ"
    }

    fn help_message(&self) -> &'static str {
        r"1D20に目標値を設定した場合に、成功レベルの自動判定を行います。
例： 1D20<=16
"
    }

    /// Ruby `InfiniteFantasia#result_1d20`。
    fn result_1d20(
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

        if total > target {
            return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
        }

        let mut output = if total <= floor_div(target.clone(), I::from(32)) {
            "32レベル成功(32Lv+)".to_owned()
        } else if total <= floor_div(target.clone(), I::from(16)) {
            "16レベル成功(16Lv+)".to_owned()
        } else if total <= floor_div(target.clone(), I::from(8)) {
            "8レベル成功".to_owned()
        } else if total <= floor_div(target.clone(), I::from(4)) {
            "4レベル成功".to_owned()
        } else if total <= floor_div(target.clone(), I::from(2)) {
            "2レベル成功".to_owned()
        } else {
            "1レベル成功".to_owned()
        };

        // Ruby: Result.new.tap { r.text = output; r.success = true;
        //        if total <= 1; r.critical = true; r.text += "/クリティカル"; end }
        let result = if total <= I::ONE {
            output.push_str("/クリティカル");
            EvalResult::critical(output)
        } else {
            EvalResult::success(output)
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
            .join("test/data/InfiniteFantasia.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/InfiniteFantasia.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/InfiniteFantasia.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("InfiniteFantasia.toml must parse");
        assert_eq!(
            data.tests.len(),
            14,
            "case count in test/data/InfiniteFantasia.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "InfiniteFantasia",
                "unexpected game system in InfiniteFantasia.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("InfiniteFantasia"), &tc.input, &mut src) {
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
                    "FAIL InfiniteFantasia:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} InfiniteFantasia cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
