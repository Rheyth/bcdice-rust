//! P4で手書き移植した `lib/bcdice/game_system/WorldsEndFrontline_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `WorldsEndFrontline` を継承し、`@locale` を `:ko_kr` に変えるだけで
//! `eval_game_system_specific_command` の上書きは無い。したがってコマンド解釈・判定は
//! [`super::WorldsEndFrontline`] の実装をそのまま使い、ここには `ko_kr` ロケールの
//! 定型文（`i18n/Bloodorium/ko_kr.yml`）だけを置く。

use super::WorldsEndFrontline::{eval_specific_command, SystemTexts, HELP_MESSAGE};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::WorldsEndFrontline_Korean`（ID: `WorldsEndFrontline:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldsEndFrontline_Korean;

impl GameSystem for WorldsEndFrontline_Korean {
    fn id(&self) -> &'static str {
        "WorldsEndFrontline:Korean"
    }

    fn name(&self) -> &'static str {
        "월드 엔드 프론트라인"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:월드 엔드 프론트라인"
    }

    /// Ruby は `HELP_MESSAGE` を上書きしないので、親から引き継いだ日本語の文面のまま。
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    /// Ruby `register_prefix_from_super_class()`。
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_TEXTS, command, rng)
    }
}

/// i18n `i18n/Bloodorium/ko_kr.yml`（`"《트라이엄프》(*%{triumph})"`）。
static KO_TEXTS: SystemTexts = SystemTexts {
    triumph_before: "《트라이엄프》(*",
    triumph_after: ")",
};

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
            .join("test/data/WorldsEndFrontline_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/WorldsEndFrontline_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/WorldsEndFrontline_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("WorldsEndFrontline_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            10,
            "case count in test/data/WorldsEndFrontline_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "WorldsEndFrontline:Korean",
                "unexpected game system in WorldsEndFrontline_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("WorldsEndFrontline:Korean"),
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
                    "FAIL WorldsEndFrontline:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} WorldsEndFrontline:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
