//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons5_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DungeonsAndDragons5` を継承し、`@locale` を `:ko_kr` に変えて
//! `register_prefix_from_super_class` するだけで、判定の実装は上書きしない。
//! そのため評価は [`super::DungeonsAndDragons5::eval_specific_command`] をそのまま使い、
//! ここには `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/ko_kr.yml` から写したもので、値は1文字も変えていない。

use super::DungeonsAndDragons5::{eval_specific_command, Translations};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr`（`i18n/ko_kr.yml`）。
static KO_KR: Translations = Translations {
    critical: "크리티컬",
    fumble: "펌블",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::DungeonsAndDragons5_Korean`（ID: `DungeonsAndDragons5:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons5_Korean;

impl GameSystem for DungeonsAndDragons5_Korean {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons5:Korean"
    }

    fn name(&self) -> &'static str {
        "던전 앤 드래곤 5판"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:던전 앤 드래곤 5판"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AT", "AR", "2H"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng, &KO_KR)
    }
}

/// Ruby `HELP_MESSAGE` 定数。
const HELP_MESSAGE: &str = r"・명중 굴림　AT[x][@c][>=t][y]
　x: +- 수정치 (생략 가능)
　c: 크리티컬 수치 (생략 가능)
　t: 목표 AC (>= 포함, 생략 가능)
　y: 유리(A), 불리(D) (생략 가능)
　B: 브레스나 가이던스 등의 보너스 (생략 가능)
　※보충 설명: B만 입력하면 +1d4를, B[1D4+1D8] 와 같이 입력하면 []안의 주사위를 추가로 굴립니다.


　펌블/실패/성공/크리티컬을 자동으로 판정합니다.
　예시）AT AT>=10 AT+5>=18 AT-3>=16 ATA AT>=10A AT+3>=18A AT-3>=16 ATD AT>=10D AT+5>=18D AT-5>=16D
　    AT@19 AT+5@18 AT-2@19>=15

・능력 판정　AR[x][>=t][y]
　명중 굴림과 동일. 성공/실패 결과를 자동 판정합니다.
　예시）AR AR>=10 AR+5>=18 AR-3>=16 ARA AR>=10A AR+3>=18A AR-3>=16 ARD AR>=10D AR+5>=18D AR-5>=16D

・대형 무기 전투술 대미지 계산(베이직 룰북 32p)　2HnDx[m]
　n: 주사위 개수
　x: 주사위 면수(1d6의 6, 1d8의 8 등)
　m: +- 수정치 (생략 가능)
　팔라딘과 파이터의 무기를 양손으로 사용할 경우, 대미지 주사위에서 1 또는 2가 나오면 다시 굴립니다.
　예시)2H3D6 2H1D10+3 2H2D8-1
";

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
            .join("test/data/DungeonsAndDragons5_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DungeonsAndDragons5_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/DungeonsAndDragons5_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DungeonsAndDragons5_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            74,
            "case count in test/data/DungeonsAndDragons5_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DungeonsAndDragons5:Korean",
                "unexpected game system in DungeonsAndDragons5_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("DungeonsAndDragons5:Korean"),
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
                    "FAIL DungeonsAndDragons5:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DungeonsAndDragons5:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
