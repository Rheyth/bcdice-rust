//! P4で手書き移植した `lib/bcdice/game_system/AnimaAnimus_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `AnimaAnimus` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! 判定・表の引き方は [`super::AnimaAnimus`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データと定型文だけを置く。

use super::AnimaAnimus::{eval_specific_command, SystemTables};
use crate::dice_table::{RangeInc, RangeTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_IGT_ITEMS: &[&str] = &[
    "스트리트 파이트/<격투>/「나한테 이기면 알려주지」정보를 아는 혼원자에게 승부를 도전받았다. 살아남기 위해서라도 이 싸움, 질 수는 없다.",
    "추적!/<추적/도주>/유익한 정보를 가진 인물을 발견했지만, 이쪽 얼굴을 보자마자 도망쳐 버렸다. 어떻게든 잡아야만 한다.",
    "협박/<위압>/불량배들이 모이는 바에 찾아왔다. 뒷세계에 사는 그들을 협박하면 유익한 정보를 얻을 수 있을 것이다.",
    "인터넷/<컴퓨터>/SNS나 뉴스 등, 인터넷상의 정보를 조사한다. 가짜 뉴스에 속지 않도록 주의해야 한다.",
    "빈사의 정보 제공자/<의학>/정보를 아는 인물이 있다는 말을 듣고 찾아갔더니, 그 인물이 빈사 상태의 중상을 입고 있었다. 어떻게든 소생시켜야 한다.",
    "잠입 수사/<은밀>/적대하는 혼원자 그룹에 잠입해 조사 활동을 펼친다. 위험은 높지만 유익한 정보를 얻을 확률도 높다.",
    "정보 교환/<교섭>/우호적인 관계에 있는 혼원자와 정보를 교환한다. 잘 이야기를 들어낼 수 있으면 좋겠지만.",
    "마연의 정보상/<조달>/마연의 정보상에 접촉해 정보를 캐내기로 했다. 만만찮은 상대인 것 같은데, 과연……?",
    "불법 조사/<범죄>/법에 저촉되는 방법으로 정보를 모으기로 했다. 해킹, 절도, 공갈, 어떤 수단을 선택할까.",
    "탐문/<자아>/거리를 오가는 사람들에게 탐문 수사를 실시한다. 꾸준한 활동이야말로 목표에 도달하기 위한 가장 빠른 방법이다.",
];
static KO_IGT: Table = Table::from_dice("정보 수집표", 1, 10, KO_IGT_ITEMS);

static KO_LT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 2), "존재/존재가 희미해져, 지인이나 친구에게 자신의 존재를 잊혀버린다. 머지않아 소중한 파트너의 기억에서도 사라져, 이 세계에서 혼자가 된다.\n자신의 출신을 삭제할 것."),
    (RangeInc::new(3, 4), "기억/소중한 기억을 하나 잃는다. 앞으로는 힘을 사용할 때마다 기억을 하나씩 잃게 되며, 마지막에는 소중한 파트너의 일도 떠올릴 수 없게 된다.\n자신의 메모리아를 하나 선택하여 삭제할 것. 시나리오 메모리아는 선택할 수 없다."),
    (RangeInc::new(5, 6), "용모/점점 예전과는 동떨어진 모습으로 변해간다. 언젠가 아무도 자신을 자신이라고 알아보지 못하게 되는 것일까.\n자신의 특징적인 외모를 잃는다. 내용을 적절한 것으로 바꿔 쓸 것(특징적인 외모가 아름다운 머리카락이라면 흉한 머리카락 등)."),
    (RangeInc::new(7, 8), "감정/희노애락의 감정 중 어느 하나를 잃는다. 힘을 사용할 때마다 다른 감정도 잃어가며, 마지막에는 그저 살아남기 위해 싸우는 기계가 된다.\n포지티브와 네거티브 중 하나를 선택한다. 선택한 감정을 모든 메모리아에서 삭제한다. 삭제한 결과, 발현 감정이 없어진 경우, 남은 감정을 발현 감정으로 할 것. 또한, 새로운 메모리아를 취득한 경우에도 선택한 감정을 얻을 수 없다."),
    (RangeInc::new(9, 10), "오감/조금씩 오감이 둔해진다. 지금까지 할 수 있었던 일을 할 수 없게 되어버린다.\n자신의 특기를 하나 선택한다. 선택한 특기에 ×를 체크할 것. ×가 체크된 기능으로는 판정을 할 수 없으며, 판정 시 자동으로 실패한다."),
];
static KO_LT: RangeTable = RangeTable::from_dice("상실표", 1, 10, KO_LT_ITEMS);

/// i18n `ko_kr` の表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    igt: &KO_IGT,
    lt: &KO_LT,
    achievement_value: "달성치",
    critical: "크리티컬 발생",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::AnimaAnimus_Korean`（ID: `AnimaAnimus:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimaAnimus_Korean;

impl GameSystem for AnimaAnimus_Korean {
    fn id(&self) -> &'static str {
        "AnimaAnimus:Korean"
    }

    fn name(&self) -> &'static str {
        "아니마 아니무스"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:아니마 아니무스"
    }

    fn help_message(&self) -> &'static str {
        r"・행위 판정(xAN<=y±z)
　십면체 주사위를 x개 굴려서 판정합니다. 달성치가 산출됩니다(크리티컬 발생 시 2 증가).
　x：굴리는 주사위의 수. 혼백값이나 공격값.
　y：성공값.
　z：성공값에 대한 보정. 생략 가능.
　(예) 2AN<=3+1 5AN<=7
・각종 표
　정보 수집표　IGT/상실표　LT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AN<=", "IGT", "LT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AnimaAnimus#eval_game_system_specific_command`（`ko_kr` の表・定型文で）。
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
            .join("test/data/AnimaAnimus_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/AnimaAnimus_Korean.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/AnimaAnimus_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("AnimaAnimus_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            23,
            "case count in test/data/AnimaAnimus_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "AnimaAnimus:Korean",
                "unexpected game system in AnimaAnimus_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("AnimaAnimus:Korean"),
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
                    "FAIL AnimaAnimus_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} AnimaAnimus_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
