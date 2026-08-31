//! P4で手書き移植した `lib/bcdice/game_system/Bloodorium_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Bloodorium` を継承し `@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Bloodorium`] のものをそのまま使い、ここには
//! `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Bloodorium/ko_kr.yml` と `i18n/ko_kr.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use super::Bloodorium::dicecheck;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `Bloodorium.triumph`（`i18n/Bloodorium/ko_kr.yml`）。
const TRIUMPH_KO_KR: &str = "《트라이엄프》(*%{triumph})";

/// i18n `success`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の成功文言。
const GLOBAL_SUCCESS: &str = "성공";
/// i18n `failure`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の失敗文言。
const GLOBAL_FAILURE: &str = "실패";

/// Ruby `BCDice::GameSystem::Bloodorium_Korean`（ID: `Bloodorium:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloodorium_Korean;

impl GameSystem for Bloodorium_Korean {
    fn id(&self) -> &'static str {
        "Bloodorium:Korean"
    }

    fn name(&self) -> &'static str {
        "블러도리움"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:블러도리움"
    }

    fn help_message(&self) -> &'static str {
        r"・주사위 체크 xDC+y
　【주사위 체크】를 실행한다.《트라이엄프》를 결과에 자동 반영한다.
　x: 주사위 수
　y: 결과에 대한 수정값 (생략 가능)
"
    }

    /// Ruby `register_prefix_from_super_class()`（`Bloodorium` と同じ接頭辞）。
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@locale = :ko_kr` により `Base#result_ndx` の `translate` が
    /// `i18n/ko_kr.yml` を引くようになる分。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(GLOBAL_SUCCESS))
        } else {
            Some(EvalResult::failure(GLOBAL_FAILURE))
        }
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(dicecheck(command, TRIUMPH_KO_KR, rng)?.map(SpecificCommandOutput::result))
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
            .join("test/data/Bloodorium_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Bloodorium_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Bloodorium_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Bloodorium_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            10,
            "case count in test/data/Bloodorium_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Bloodorium:Korean",
                "unexpected game system in Bloodorium_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Bloodorium:Korean"), &tc.input, &mut src) {
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
                    "FAIL Bloodorium:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Bloodorium:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// `@locale = :ko_kr` により汎用コマンドの成否文言も韓国語になること。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            (
                "2D6>=7",
                vec![(4, 6), (5, 6)],
                "(2D6>=7) ＞ 9[4,5] ＞ 9 ＞ 성공",
            ),
            (
                "2D6>=10",
                vec![(4, 6), (5, 6)],
                "(2D6>=10) ＞ 9[4,5] ＞ 9 ＞ 실패",
            ),
        ];

        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("Bloodorium:Korean"), input, &mut src)
                .expect("eval")
                .expect("some output");
            assert_eq!(result.text, expected, "input: {input}");
        }
    }
}
