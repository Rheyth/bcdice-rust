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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/NjslyrBattle.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NjslyrBattle.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/NjslyrBattle.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NjslyrBattle.toml must parse");
        assert_eq!(
            data.tests.len(),
            9,
            "case count in test/data/NjslyrBattle.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NjslyrBattle",
                "unexpected game system in NjslyrBattle.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("NjslyrBattle"), &tc.input, &mut src) {
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
                    "FAIL NjslyrBattle:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NjslyrBattle cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
