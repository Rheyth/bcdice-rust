//! P4で手書き移植した `lib/bcdice/game_system/YearZeroEngine_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `YearZeroEngine` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! コマンド解釈・判定は [`super::YearZeroEngine`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 定型文は `i18n/YearZeroEngine/ko_kr.yml` から書き写したもので、値は1文字も変えていない。

use super::YearZeroEngine::{eval_specific_command, SystemStrings};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::YearZeroEngine_Korean`（ID: `YearZeroEngine:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearZeroEngine_Korean;

impl GameSystem for YearZeroEngine_Korean {
    fn id(&self) -> &'static str {
        "YearZeroEngine:Korean"
    }

    fn name(&self) -> &'static str {
        "이어 제로 엔진(Year Zero Engine)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:이어 제로 엔진(Year Zero Engine)"
    }

    fn help_message(&self) -> &'static str {
        r"・다이스 풀 판정 커맨드(nYZEx+x+x+m)
  (난이도)YZE(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)+(수정치)  # (6만 셈함)
  (난이도)YZE(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)-(수정치)  # (6만 셈함)

・다이스 풀 판정 커맨드(nMYZx+x+x)
  (난이도)MYZ(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)  # (1과 6을 세어 푸시 가능 수 표시)
  (난이도)MYZ(능력 주사위 수)-(기능 주사위 수)+(아이템 주사위 수)  # (1과 6을 세어 푸시 가능 수 표시, 기능 마이너스 지정)

  ※ 난이도, 기능 주사위 수, 아이템 주사위 수는 생략 가능

・스텝 다이스 판정 커맨드(nYZSx+x+m+f)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)   # (1, 6을 세어 푸시 가능 수 표시)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)   # (1, 6을 세어 푸시 가능 수 표시)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)A  # (1, 6을 세어 푸시 가능 수 표시, 유리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)A  # (1, 6을 세어 푸시 가능 수 표시, 유리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)D  # (1, 6을 세어 푸시 가능 수 표시, 불리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)D  # (1, 6을 세어 푸시 가능 수 표시, 불리)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?(YZE|MYZ|YZS)"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_STRINGS, command, rng)
    }
}

/// `ko_kr` ロケールの定型文（`i18n/YearZeroEngine/ko_kr.yml`）。
static KO_STRINGS: SystemStrings = SystemStrings {
    success_count: "성공 수",
    difficulty: "난이도",
    success_msg: "판정 성공!",
    failure_msg: "판정 실패!",
    roll_one: "주사위 눈 1",
    ability: "능력",
    skill: "기능",
    item: "아이템",
    pushable: "푸시 가능",
    dice: "주사위",
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
            .join("test/data/YearZeroEngine_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/YearZeroEngine_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/YearZeroEngine_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("YearZeroEngine_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            90,
            "case count in test/data/YearZeroEngine_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "YearZeroEngine:Korean",
                "unexpected game system in YearZeroEngine_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("YearZeroEngine:Korean"),
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
                    "FAIL YearZeroEngine:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} YearZeroEngine:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
