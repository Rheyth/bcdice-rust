//! 生成コード（`generated/`）共通のTOMLテストハーネス。
//!
//! `generated/` 配下の各システムは `#[cfg(test)] mod tests` で
//! `test/data/<System>.toml` の全ケース評価を1テストとして持っていたが、
//! 内容はシステム名・tomlパス・ケース数assert以外同一である。
//! このモジュールの [`assert_toml_cases`] に一本化する
//! （docs/refactor_candidates_20260901.md 第1群R1）。
//!
//! 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
//! （出力文字列・5フラグ・注入乱数を使い切ったか）。

use std::path::Path;

use crate::eval::eval_command;
use crate::game_system::GameSystemId;
use crate::randomizer::SeededRandomizer;
use crate::toml_test::TestDataFile;

/// `test/data/<file>` の全ケースが `system` で期待どおり評価されること。
///
/// - `expected_count`: TOMLのケース数assert（旧 `assert_eq!(data.tests.len(), N)`）
/// - `surplus_rands_allowed`: 注入乱数が余るケースの例外
///   （1始まりのケース番号 → 余る本数）。空なら全ケースで余り0本を要求する。
pub(crate) fn assert_toml_cases(
    system: &str,
    file: &str,
    expected_count: usize,
    surplus_rands_allowed: &[(usize, usize)],
) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data")
        .join(file);
    if !path.exists() {
        // worktree外でクレート単体ビルドされた場合
        eprintln!("skip: {} not found", path.display());
        return;
    }

    let data = TestDataFile::load(&path).unwrap_or_else(|e| panic!("{file} must parse: {e}"));
    assert_eq!(data.tests.len(), expected_count, "case count in {file}");

    let mut failures: Vec<String> = Vec::new();
    for (index, tc) in data.tests.iter().enumerate() {
        assert_eq!(tc.game_system, system, "unexpected game system in {file}");

        let mut reasons: Vec<String> = Vec::new();
        let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
        let mut src = SeededRandomizer::new(rands);

        match eval_command(&GameSystemId::new(system), &tc.input, &mut src) {
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
                for (name, expected, actual) in [
                    ("secret", tc.secret, result.secret),
                    ("success", tc.success, result.success),
                    ("failure", tc.failure, result.failure),
                    ("critical", tc.critical, result.critical),
                    ("fumble", tc.fumble, result.fumble),
                ] {
                    if expected != actual {
                        reasons.push(format!(
                            "{name} flag mismatch: expected {expected}, actual {actual}"
                        ));
                    }
                }
            }
        }

        let allowed_surplus = surplus_rands_allowed
            .iter()
            .find(|(case, _)| *case == index + 1)
            .map_or(0, |(_, remaining)| *remaining);
        if src.remaining() != allowed_surplus {
            reasons.push(format!(
                "unconsumed rands remain ({}, allowed {allowed_surplus})",
                src.remaining()
            ));
        }

        if !reasons.is_empty() {
            failures.push(format!(
                "FAIL {system}:{}:{}\n  - {}",
                index + 1,
                tc.input,
                reasons.join("\n  - ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{}/{} {system} cases failed:\n{}",
        failures.len(),
        data.tests.len(),
        failures.join("\n")
    );
}

/// [`assert_toml_cases`] の薄いラッパ（surplus例外なし）。
pub(crate) fn assert_toml_cases_strict(system: &str, file: &str, expected_count: usize) {
    assert_toml_cases(system, file, expected_count, &[]);
}
