//! P4で手書き移植した `lib/bcdice/game_system/NegikureNegimaki_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `NegikureNegimaki` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::NegikureNegimaki`] の関数をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。

use super::NegikureNegimaki::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr` の定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    result_level: "성공 레벨%{success_level}/요구%{required_level}",
    success_level: "성공 레벨%{success_level}",
    damage: "일반 피해%{normal_damage}/직격 피해%{direct_damage}",
    guts_loss: "거츠 감소%{guts_loss}",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::NegikureNegimaki_Korean`（ID: `NegikureNegimaki:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegikureNegimaki_Korean;

impl GameSystem for NegikureNegimaki_Korean {
    fn id(&self) -> &'static str {
        "NegikureNegimaki:Korean"
    }

    fn name(&self) -> &'static str {
        "네지쿠레 네지마키"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:네지쿠레 네지마키"
    }

    fn help_message(&self) -> &'static str {
        r"■ 행위 판정
nNNx#y: n개의 D6을 굴려, x 이상의 주사위 결과값의 개수를 성공 레벨로 판정.
n: 주사위 수（생략 시 1）
x: 난이도（생략 시 4）
y: 요구 성공 레벨（생략 시 1, 0은 1로 처리）

■ 전투 판정（공격 판정）
nNAx#y: n개의 D6을 굴려, x 이상을 성공으로 간주. y 이상의 성공은 직격 피해가 된다.
n: 주사위 수（생략 시 1）
x: 난이도（생략 시 4）
y: 크리티컬 값（생략 시 6, 0은 1로 처리）
일반 피해 = 성공 레벨 - 직격 피해
직격 피해 = 성공한 눈 중 y 이상의 개수
거츠 감소 = 주사위 결과값 1의 개수

■ 스트라이크 판정
nNS: n개의 D6을 굴려, 주사위 결과값 1의 개수만큼 거츠 감소를 산출한다
n: 주사위 수（생략 시 1）
거츠 감소가 0이면 성공, 1 이상이면 실패
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*NN\d*(#\d+)?", r"\d*NA\d*(#\d+)?", r"\d*NS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `NegikureNegimaki#eval_game_system_specific_command`（`ko_kr` の定型文で）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
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
            .join("test/data/NegikureNegimaki_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NegikureNegimaki_Korean.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/NegikureNegimaki_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NegikureNegimaki_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            26,
            "case count in test/data/NegikureNegimaki_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NegikureNegimaki:Korean",
                "unexpected game system in NegikureNegimaki_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("NegikureNegimaki:Korean"),
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
                    "FAIL NegikureNegimaki_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NegikureNegimaki_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
