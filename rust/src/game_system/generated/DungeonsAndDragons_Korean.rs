//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DungeonsAndDragons` を継承し、`check_result` を上書きして
//! 成功／失敗／クリティカル／ファンブルの文言を韓国語にする。
//! 1D20 では出目 20 をクリティカル、出目 1 をファンブルとする。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `DungeonsAndDragons_Korean#success_text`。
const SUCCESS_TEXT: &str = "성공";
/// Ruby `DungeonsAndDragons_Korean#failure_text`。
const FAILURE_TEXT: &str = "실패";
/// Ruby `DungeonsAndDragons_Korean#critical_text`。
const CRITICAL_TEXT: &str = "크리티컬";
/// Ruby `DungeonsAndDragons_Korean#fumble_text`。
const FUMBLE_TEXT: &str = "펌블";

/// Ruby `BCDice::GameSystem::DungeonsAndDragons_Korean`（ID: `DungeonsAndDragons:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons_Korean;

impl GameSystem for DungeonsAndDragons_Korean {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons:Korean"
    }

    fn name(&self) -> &'static str {
        "던전 앤 드래곤"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:던전 앤 드래곤"
    }

    fn help_message(&self) -> &'static str {
        r"※ 이 다이스봇은 방의 시스템 이름 표시용입니다.
"
    }

    /// Ruby `DungeonsAndDragons_Korean#check_result`。
    ///
    /// 親の `Base#check_result` は呼ばない（Ruby も `super` しない）。
    fn check_result(
        &self,
        total: crate::Int,
        rand_results: &[(i64, i64)],
        cmp_op: CmpOp,
        target: Target,
        _rng: &mut Randomizer,
    ) -> Result<Option<EvalResult>, EvalError> {
        // Ruby: return nil if target.is_a?(String)
        let Target::Number(target) = target else {
            return Ok(None);
        };

        // Ruby: dice_total = rand_results.map(&:value).sum
        let dice_total: i64 = rand_results.iter().fold(0i64, |a, r| a.wrapping_add(r.1));
        // Ruby: rand_results.map(&:sides) == [20]
        let sides: Vec<i64> = rand_results.iter().map(|r| r.0).collect();
        if sides.as_slice() == [20] {
            if dice_total == 20 {
                return Ok(Some(EvalResult::critical(CRITICAL_TEXT)));
            }
            if dice_total == 1 {
                return Ok(Some(EvalResult::fumble(FUMBLE_TEXT)));
            }
        }

        // Ruby: total.send(cmp_op, target)
        if cmp_op.apply(&total, &target) {
            Ok(Some(EvalResult::success(SUCCESS_TEXT)))
        } else {
            Ok(Some(EvalResult::failure(FAILURE_TEXT)))
        }
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
            .join("test/data/DungeonsAndDragons_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DungeonsAndDragons_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/DungeonsAndDragons_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DungeonsAndDragons_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            8,
            "case count in test/data/DungeonsAndDragons_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DungeonsAndDragons:Korean",
                "unexpected game system in DungeonsAndDragons_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("DungeonsAndDragons:Korean"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL DungeonsAndDragons:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DungeonsAndDragons:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
