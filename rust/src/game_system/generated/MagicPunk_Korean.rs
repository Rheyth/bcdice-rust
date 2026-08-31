//! P4で手書き移植した `lib/bcdice/game_system/MagicPunk_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `MagicPunk` を継承し `@locale = :ko_kr` にするだけなので、
//! 判定は [`super::MagicPunk`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く
//! （`i18n/MagicPunk/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない）。

use super::MagicPunk::{roll_mp, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_SYSTEM: SystemTables = SystemTables {
    bad_beat: "실패(BB)",
    jackpot: "성공(JP)",
    success: "성공(%<value>d)",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::MagicPunk_Korean`（ID: `MagicPunk:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicPunk_Korean;

impl GameSystem for MagicPunk_Korean {
    fn id(&self) -> &'static str {
        "MagicPunk:Korean"
    }

    fn name(&self) -> &'static str {
        "매직펑크TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:매직펑크TRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 판정 (nMPm)
nD20을 굴려, m 이하의 눈이 있으면 성공.
m과 같은 눈이 있으면 잭팟(자동 성공).
모든 눈이 1이면 배드 비트(자동 실패).

■ 챌린지 판정 (nMPmCx)
통상 판정에 더해, 챌린지 값 x 이상의 눈이 필요.

■ 주사위 수 0개 (0MPmCx)
수정치 등으로 주사위 수가 0개가 된 경우 2d20을 굴림.
두 개의 눈 중 더 나쁜 쪽의 결과를 적용.
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"^\d*MP\d+"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_mp(&KO_SYSTEM, command, rng)?.map(SpecificCommandOutput::result))
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
            .join("test/data/MagicPunk_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MagicPunk_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MagicPunk_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MagicPunk_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            14,
            "case count in test/data/MagicPunk_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MagicPunk:Korean",
                "unexpected game system in MagicPunk_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("MagicPunk:Korean"), &tc.input, &mut src) {
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
                    "FAIL MagicPunk_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MagicPunk_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
