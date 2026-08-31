//! P4で手書き移植した `lib/bcdice/game_system/BlackJacket_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `BlackJacket` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! コマンド解釈・判定・表の引き方は [`super::BlackJacket`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データ（`KO_` 接頭辞の `static` 群）だけを置く。
//!
//! データは `i18n/BlackJacket/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。

use super::BlackJacket::{eval_specific_command, DeathChart, SystemTables};
use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::BlackJacket_Korean`（ID: `BlackJacket:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlackJacket_Korean;

impl GameSystem for BlackJacket_Korean {
    fn id(&self) -> &'static str {
        "BlackJacket:Korean"
    }

    fn name(&self) -> &'static str {
        "블랙재킷RPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:블랙재킷RPG"
    }

    fn help_message(&self) -> &'static str {
        r"・행위 판정（BJx）
　x：성공률
　예）BJ80
　크리티컬,펌블 여부는 자동으로 판정합니다.
　「BJ50+20-30」처럼 값을 가감하여 기재할 수 있습니다.
　성공률의 상한은 100％, 하한은 ０％ 입니다.
・데스 차트 (DCxY)
　x：차트 종류. 육체：DCL, 정신：DCS, 환경：DCC
　Y=마이너스 값
　예）DCL5：라이프 마이너스 값 5 + 1D10 판정
　　　DCS3：새니티 마이너스 값 3 + 1D10 판정
　　　DCC0：크레딧 마이너스 값 0 + 1D10 판정
・챌린지・패널티 차트（CPC）
・사이드 트랙 차트（STC）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["BJ", "DC[LSC]", "CPC", "STC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
    }
}

// ---------------------------------------------------------------------------
// ko_kr ロケールの定型文
// ---------------------------------------------------------------------------

/// i18n `ko_kr.BlackJacket.action_judge`。
fn ko_action_judge(rate: i64) -> String {
    format!("행위판정(성공률:{rate}％)")
}

/// i18n `ko_kr.BlackJacket.death_chart_result`。
fn ko_death_chart_result(
    name: &str,
    minus: i64,
    dice: i64,
    key: i64,
    key_text: &str,
    chosen: &str,
) -> String {
    format!("데스 차트（{name}）[마이너스 값:{minus} + 1D10(->{dice}) = {key}] ＞ {key_text} ： {chosen}")
}

// ---------------------------------------------------------------------------
// 表データ（i18n/BlackJacket/ko_kr.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `BlackJacket.death_charts.physical`。
static KO_DEATH_CHART_PHYSICAL: &[&str] = &[
    "효과 없음. 당신은 기적적으로 목숨을 건졌다. 싸움은 계속된다.",
    "격한 통증을 느낀다. 이후 이벤트가 끝날 때까지 모든 판정의 성공률에 -10%.",
    "더이상 몸이 움직이지 않는다…… 당신은 [경직 2]를 받는다.",
    "혼신의 일격!! 당신은 〈생존〉 판정을 한다. 실패할 경우 [사망]한다.",
    "갑자기 눈앞이 캄캄해진다. 당신은 [기절 2]를 받는다.",
    "이후, 이벤트 종료까지 모든 판정의 성공률 -20%.",
    "기록적인 일격!! 당신은 〈생존〉 -20% 으로 판정한다. 실패할 경우 [사망]한다.",
    "사느냐 죽느냐. 당신은 [빈사 2]를 받는다.",
    "역사에 한 획을 그을 일격!! 당신은 <생존> -30% 으로 판정한다. 실패할 경우 [사망]한다.",
    "이후, 이벤트 종료 시까지 모든 판정의 성공률 -30%.",
    "신화적 일격!! 공중에서 세 바퀴 정도 회전한 후 땅바닥에 내동댕이쳐진다. 보기에도 끔찍한 모습. 육체는 원형을 유지하지 못했다. 당신은 [사망]한다.",
];

/// i18n `BlackJacket.death_charts.mental`。
static KO_DEATH_CHART_MENTAL: &[&str] = &[
    "효과 없음. 당신은 이를 악물고 스트레스를 견뎌냈다.",
    "이후, 이벤트 종료 시까지 모든 판정의 성공률 -10%.",
    "말할 수 없는 공포가 당신을 엄습한다. 당신은 [공포 2]를 받는다.",
    "상처를 많이 받았다. 당신은 〈의지〉 판정을 한다. 실패할 경우 [절망] 상태가 되어서 NPC가 된다.",
    "의식을 잃었다. 당신은 [기절 2]를 받는다.",
    "이후, 이벤트 종료 시까지 모든 판정의 성공률 -20%.",
    "신뢰했던 자에게 속은 아픔. 당신은 〈의지〉 -20% 으로 판정한다. 실패할 경우, [절망] 상태가 되어서 NPC가 된다.",
    "동료에게 배신 당한 것일지도 모른다. 당신은 [혼란 2]를 받는다.",
    "너무나 참혹한 현실. 당신은 〈의지〉 -30% 으로 판정한다. 실패할 경우 [절망] 상태가 되어서 NPC가 된다.",
    "이후, 이벤트 종료 시까지 모든 판정의 성공률 -30%.",
    "천지개벽의 이치 그 이상. 그것은 인류의 인식한계를 뛰어넘는 무언가였다. 당신은 [절망] 상태가 된 후 NPC가 된다.",
];

/// i18n `BlackJacket.death_charts.social`。
static KO_DEATH_CHART_SOCIAL: &[&str] = &[
    "효과 없음. 당신은 뒤숭숭한 소문을 무시했다.",
    "이후, 이벤트 종료 시까지 모든 판정의 성공률 -10%.",
    "위험한 상태! 이후, 라운드 종료 시까지 당신은 카르마를 사용할 수 없다.",
    "나쁜 소문이 돈다. 당신은 〈교섭〉 판정을 한다. 실패할 경우 당신은 동료들의 신뢰를 잃고 [무연고] 상태가 된 후 NPC가 된다.",
    "이후, 시나리오 종료 시까지 대가(코스트)에 크레딧을 소비하는 파워를 사용할 수 없다.",
    "당신의 악평이 세상에 널리 알려진다. 협력자로부터의 지원이 중단된다. 이후 시나리오 종료 시까지 모든 판정의 성공률 -20%.",
    "배신!! 당신은 〈경제〉 -20% 으로 판정한다. 실패할 경우 당신은 주위로부터 신용을 잃고, [무연고] 상태가 되어 NPC가 된다.",
    "이후, 시나리오 종료 시까지 【환경】 계열의 기능 레벨이 모두 0이 된다.",
    "날조 보도? 기억나지 않는 배신 행위가 특종으로 보도된다. 당신은 〈심리〉 -30% 으로 판정한다. 실패할 경우 당신은 인간으로서의 존엄성을 잃고, [무연고]가 된다.",
    "이후, 이벤트 종료 시까지 모든 판정 성공률 -30%.",
    "당신의 이름은 사상 최악의 오점으로 영원히 역사에 새겨진다. 이제 당신을 믿는 동료는 없고 당신을 돕는 사회도 없다. 당신은 [무연고] 상태가 된 후 NPC가 된다.",
];

/// i18n `BlackJacket.table.CPC.items`。
static KO_CPC_ITEMS: &[&str] = &[
    "사망\n도와야 할 NPC (히로인 등)가 사망한다.",
    "흑성\n적이 목적을 성취하고, 사건은 PC의 패배로 끝난다. 그대로 여운 페이즈로 넘어갈 것.",
    "활성\n적 보스의 라이프를 2배로 한 다음 결전 페이즈를 개시한다.",
    "공세\n적 보스의 대미지에 +2D6의 수정을 준 후 결전 페이즈를 개시한다.",
    "대거\n적의 수(보스 제외)를 2배로 한 후 결전 페이즈를 개시한다.",
    "암흑\n모든 에리어(구역)을 [어둠]으로 만든 다음 결전 페이즈를 개시한다.",
    "맹화\n전투 에리어(구역) 2개를 [대미지 존 2]로 취급한 후, 결전 페이즈를 개시한다.",
    "복병\n적의 절반을 에리어(구역) 1과 에리어(구역) 2로 이동시킨 후, 결전 페이즈를 개시한다.",
    "만복\n보스 이외의 적의 라이프를 모두 2배로 한 다음, 결전 페이즈를 개시한다.",
    "봉인\nPC는 결전 페이즈 동안 카르마를 사용할 수 없다. 결전 페이즈를 개시한다.",
];

/// Ruby `TABLES["CPC"]`（`1D10`）。
static KO_CPC: Table = Table::from_dice("챌린지・패널티 차트", 1, 10, KO_CPC_ITEMS);

/// i18n `BlackJacket.table.STC.items`。
static KO_STC_ITEMS: &[&str] = &[
    "해후\n우연히 NPC와 만난다. 어떤 NPC가 나타날지는 GM이 결정한다.",
    "사고\n교통사고를 당한다. 주변에서 패닉이 일어나고 있을지도 모른다.",
    "낮잠\n지독한 졸음이 몰려온다. 설마, 신참 빌런의 능력인가?",
    "고백\nNPC 한 명이 지금까지 간직하고 있던 마음을 당신에게 고백한다.",
    "설정\n새로운 설정이 밝혀진다. 사실은 NPC의 아버지였다든가, 선천적으로 눈이 보이지 않는다든가.",
    "자객\n누군가로부터 공격을 받는다. 제3세력인가?",
    "불청객\n우연히 원수 한 명과 마주친다. 상황에 따라서 바로 전투가 발생할지도 모른다.",
    "의심\n수상한 사람을 눈치챘다. 따라가야 하나? 무시해야 하나?",
    "조우\n시나리오와 관계없는 빌런 조직과 조우한다.",
    "평화\n별일 없었다.",
];

/// Ruby `TABLES["STC"]`（`1D10`）。
static KO_STC: Table = Table::from_dice("사이드 트랙 차트", 1, 10, KO_STC_ITEMS);

/// Ruby `DEATH_CHARTS`（i18n `BlackJacket.chart_name.*` が表名）。
static KO_DEATH_CHARTS: &[(&str, DeathChart)] = &[
    ("L", DeathChart::new("육체", KO_DEATH_CHART_PHYSICAL)),
    ("S", DeathChart::new("정신", KO_DEATH_CHART_MENTAL)),
    ("C", DeathChart::new("환경", KO_DEATH_CHART_SOCIAL)),
];

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static KO_TABLES: &[(&str, &Table)] = &[("CPC", &KO_CPC), ("STC", &KO_STC)];

/// `ko_kr` ロケールの表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    death_charts: KO_DEATH_CHARTS,
    tables: KO_TABLES,
    action_judge: ko_action_judge,
    death_chart_result: ko_death_chart_result,
    death_chart_under: "10이하",
    death_chart_over: "20이상",
    fumble: "실패 ＞ 펌블! 파워의 대가(코스트) 2배 & 재굴림 불가",
    critical: "성공 ＞ 크리티컬! 파워의 대가(코스트) 절반으로 감소",
    misery: "실패 ＞ 미저리! 파워의 대가(코스트) 2배 & 재굴림 불가",
    success: "성공",
    failure: "실패",
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
            .join("test/data/BlackJacket_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/BlackJacket_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/BlackJacket_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("BlackJacket_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            89,
            "case count in test/data/BlackJacket_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "BlackJacket:Korean",
                "unexpected game system in BlackJacket_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("BlackJacket:Korean"),
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

            if tc.expects_nil() {
                // nil を返すケースの `rands` は上流のTOMLに残った書き換え漏れで、
                // 出目のオラクルにならない（Ruby側のテストランナーは結果が nil のとき
                // 出力しか見ないため、余った出目もフラグも検査されない）。
                // ここでは「nil を返す経路ではダイスを振らない」ことだけ確かめる。
                if src.remaining() != tc.rands.len() {
                    reasons.push(format!(
                        "dice were rolled for a nil result ({} of {} rands consumed)",
                        tc.rands.len() - src.remaining(),
                        tc.rands.len()
                    ));
                }
            } else if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL BlackJacket:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} BlackJacket:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
