//! P4で手書き移植した `lib/bcdice/game_system/DoubleCross_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DoubleCross` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! コマンド解釈・判定・表の引き方は [`super::DoubleCross`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データ（`KO_` 接頭辞の `static` 群）だけを置く。
//!
//! データは `i18n/DoubleCross/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。

use super::DoubleCross::{eval_specific_command, SystemTables, TableRef};
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::DoubleCross_Korean`（ID: `DoubleCross:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoubleCross_Korean;

impl GameSystem for DoubleCross_Korean {
    fn id(&self) -> &'static str {
        "DoubleCross:Korean"
    }

    fn name(&self) -> &'static str {
        "더블크로스2nd,3rd"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:더블크로스"
    }

    fn help_message(&self) -> &'static str {
        r#"・판정 커맨드（xDX+y@c or xDXc+y）
　"(개수)DX(수정)@(크리티컬치)" 혹은 "(개수)DX(크리티컬치)(수정)" 으로 지정합니다.
　수정치도 붙일 수 있습니다.
　예）10dx　　10dx+5@8（OD tool식)　　5DX7+7-3（질풍노도식）
・각종표
　・감정표（ET）
　　포지티브와 네거티브 양쪽을 굴려, 겉으로 나타는 쪽에 O를 붙여 표시합니다.
　　물론 임의로 정하는 부분을 변경해도 괜찮습니다.
・해프닝차트（HC)
・RW프롤로그차트 포지티브（PCP）
・RW프롤로그차트 네거티브（PCN）
・D66다이스 있음
"#
    }

    /// Ruby は `register_prefix_from_super_class()` の後にもう一度 `register_prefix` するので
    /// 親クラス分の接頭辞が重複して並ぶ。生成スタブの値をそのまま保つ。
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+DX", "ET", r"\d+DX", "HC", "PCP", "PCN", r"\d+DX", "HC", "PCP", "PCN",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sides_implicit_d(&self) -> i64 {
        10
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
    }
}

// ---------------------------------------------------------------------------
// 表データ（i18n/DoubleCross/ko_kr.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `DoubleCross.ET.positive.items`。
static KO_POSITIVE_EMOTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "호기심"),
    (RangeInc::new(6, 10), "동경"),
    (RangeInc::new(11, 15), "존경"),
    (RangeInc::new(16, 20), "연대감"),
    (RangeInc::new(21, 25), "자애"),
    (RangeInc::new(26, 30), "감복"),
    (RangeInc::new(31, 35), "순애"),
    (RangeInc::new(36, 40), "우정"),
    (RangeInc::new(41, 45), "모정(慕情)"),
    (RangeInc::new(46, 50), "동정"),
    (RangeInc::new(51, 55), "유지(遺志)"),
    (RangeInc::new(56, 60), "비호"),
    (RangeInc::new(61, 65), "행복감"),
    (RangeInc::new(66, 70), "신뢰"),
    (RangeInc::new(71, 75), "집착"),
    (RangeInc::new(76, 80), "친근감"),
    (RangeInc::new(81, 85), "성의"),
    (RangeInc::new(86, 90), "호의"),
    (RangeInc::new(91, 95), "유위(有為)"),
    (RangeInc::new(96, 100), "진력"),
];

/// Ruby `POSITIVE_EMOTION_TABLE`（`1D100`）。
static KO_POSITIVE_EMOTION_TABLE: RangeTable =
    RangeTable::from_dice("감정표（포지티브）", 1, 100, KO_POSITIVE_EMOTION_ITEMS);

/// i18n `DoubleCross.ET.negative.items`。
static KO_NEGATIVE_EMOTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "식상"),
    (RangeInc::new(6, 10), "위협"),
    (RangeInc::new(11, 15), "질투"),
    (RangeInc::new(16, 20), "회개"),
    (RangeInc::new(21, 25), "공포"),
    (RangeInc::new(26, 30), "불안"),
    (RangeInc::new(31, 35), "열등감"),
    (RangeInc::new(36, 40), "소외감"),
    (RangeInc::new(41, 45), "치욕"),
    (RangeInc::new(46, 50), "연민"),
    (RangeInc::new(51, 55), "편애"),
    (RangeInc::new(56, 60), "증오"),
    (RangeInc::new(61, 65), "격의"),
    (RangeInc::new(66, 70), "혐오"),
    (RangeInc::new(71, 75), "시의심"),
    (RangeInc::new(76, 80), "싫음"),
    (RangeInc::new(81, 85), "불신감"),
    (RangeInc::new(86, 90), "불쾌감"),
    (RangeInc::new(91, 95), "분만(憤懣)"),
    (RangeInc::new(96, 100), "적개심"),
];

/// Ruby `NEGATIVE_EMOTION_TABLE`（`1D100`）。
static KO_NEGATIVE_EMOTION_TABLE: RangeTable =
    RangeTable::from_dice("감정표(네거티브)", 1, 100, KO_NEGATIVE_EMOTION_ITEMS);

/// i18n `DoubleCross.HC.items`。
static KO_HC_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "아무 일도 없다. 수정은 특별히 없다."),
    (RangeInc::new(6, 10), "전문적 지식이 필요. 그 라운드 동안, 지정된 기능이 4레벨 이하인 캐릭터가 획득하는 진행치는 -1이 된다. (최저 0)"),
    (RangeInc::new(11, 15), "초조하다. 그 라운드 동안 난이도 +1D10."),
    (RangeInc::new(16, 20), "자칫하면 치명적인 상황. 다음 진행판정에 실패할 경우, 지금까지 획득한 진행치가 0이 된다."),
    (RangeInc::new(21, 25), "비정상적인 흥분. 그 라운드 동안, 진행판정에 실패한 캐릭터는 폭주를 받는다."),
    (RangeInc::new(26, 30), "프레셔. 그 라운드 동안에  진행판정을 한 캐릭터는, 판정 직후 중압을 받는다."),
    (RangeInc::new(31, 35), "행운이 함께한다. 이번 라운드 동안에 하는 진행판정은 모두 크리티컬치 -1 된다."),
    (RangeInc::new(36, 40), "파멸적 불운. 이번 라운드 동안에 하는 진행판정은 모두 크리티컬치 +1 된다."),
    (RangeInc::new(41, 45), "있을지 없을지 모르는 찬스. 이번 라운드 동안, 최대달성치의 난이도가 +10."),
    (RangeInc::new(46, 55), "교착된 진행. 수정은 특별히 없다."),
    (RangeInc::new(56, 60), "줄타기 같은 상황. 이번 라운드 동안, 난이도+1D10."),
    (RangeInc::new(61, 65), "다시 없을 찬스. 이번 라운드 동안, 최대 달성치+10."),
    (RangeInc::new(66, 70), "소모를 동반한 작업. 이번 라운드 동안에 진행판정을 한 캐릭터는, 판정 직후에 1D10점의 대미지를 받는다."),
    (RangeInc::new(71, 75), "찬스의 도래. 이번 라운드 동안에 하는 진행판정은, 다이스를 +5개 된다."),
    (RangeInc::new(76, 80), "예상외의 핀치. 이번 라운드 동안에 하는 진행판정은, 다이스가 -5개 된다."),
    (RangeInc::new(81, 85), "긴장이 레니게이드를 활성화. 그 라운드 동안에 진행판정을 한 캐릭터는, 판정 직후에 1D10점 침식률이 증가."),
    (RangeInc::new(86, 90), "돌파구의 발견. 이번 씬 동안의 최대달성치+10. 이 효과는 중복되지 않는다."),
    (RangeInc::new(91, 95), "사태의 간헐적인 악화. 이번 씬 동안의 난이도+1D10. 이 효과는 중복된다."),
    (RangeInc::new(96, 100), "순조로운 진행. 이번 라운드 동안에 진행판정에 성공한 캐릭터는, 진행치를 +1 얻는다."),
];

/// Ruby `TABLES["HC"]`（`1D100`）。
static KO_HC: RangeTable = RangeTable::from_dice("해프닝 차트", 1, 100, KO_HC_ITEMS);

/// i18n `DoubleCross.PCP.items`。
static KO_PCP_ITEMS: &[&str] = &[
    "【빅토리】 빌런 집단과 싸워, 승리한다. 보도진과 관중들이 그 승리를 칭찬한다.",
    "【해프닝】 은행이나 상점 등에 있을 때, 돌발 범죄에 휘말려 해결한다.",
    "【레스큐】 화재나 폭발사고, 붕괴 등 재난 현장에서 시민을 구출한다.",
    "【버서스】 라이벌인 강력한 빌런과 대결하고 있다. 결말이 나지않고 빌런은 도망친다.",
    "【히어로 인터뷰】 미디어로부터 취재를 받다. 사건의 해결이나 장관으로부터의 표창을 받거나, 혹은 주목받는 히어로로서 등.",
    "【트레이닝】 히어로로서 훈련을 한다. 신체 능력이나 이펙트 훈련, 지식 보강 등.",
    "【오리진】 자신이 히어로가 된 계기, 시작 장면을 회상한다. 처음 오버드로 각성한 장면이나, 처음으로 다른 사람을 구한 순간, 예전에 동경했던 히어로에 대해서 등.",
    "【에브리데이 라이프】 일상생활을 보낸다. 오랜만의 휴가나, 히어로가 아닌 생활이나. 로이스 대상과의 대화하는 것도 좋을 것이다.",
    "【뉴 파워】 새로운 이펙트나 장비를 몸에 익히거나, 받는다. 이로써 새로운 힘을 얻은 셈이다.",
    "【석세스】 뭔가에 대성공한 장면이다. 일이라도 좋고 휴가 중에 하는 게임이나 스포츠일지도 모른다.",
];

/// Ruby `TABLES["PCP"]`（`1D10`）。
static KO_PCP: Table = Table::from_dice("프롤로그 차트 (포지티브)", 1, 10, KO_PCP_ITEMS);

/// i18n `DoubleCross.PCN.items`。
static KO_PCN_ITEMS: &[&str] = &[
    "【디피트】 빌런과 싸워 패배한 장면을 회상한다. 그때 부상은 이미 회복됐지만, 긍지는 아직 회복되지 않았다.",
    "【액시던트】 불운에 휘말린다. 사고에 연루되거나, 우연히 빌런의 공격을 받는 등. 그 불행으로 누군가가 구해질 지도 몰라.",
    "【디재스터】 사고나 재난에 휘말리거나, 과거에 휘말렸던 장면의 회상. 자신은 오버드의 능력으로 살아남았지만, 다른 사람들은⋯.",
    "【위크포인트】 라이벌인 강력한 빌런과 대결하고 있다. 빌런은 당신의 약점이나 치명적인 결함, 불길한 미래를 알리고 떠난다.",
    "【배싱】 미디어나 시민들로부터 비판을 받는다. 과거의 실패나 지금의 난폭한 해결법 등.",
    "【리커버리】 치료를 받고 있다. 최근에 다쳤거, 과거의 오래된 상처가 남아 있거나, 이제 육체가 한계에 다다랐다⋯등.",
    "【트라우마】 과거의 불행, 비극, 실패 등을 회상하고 있다. 그게 히어로가 된 계기일지도 몰라.",
    "【어큐즈】 당신을 비난하는 대화. 로이스의 대상, 피해자의 가족 등. 그래도 히어로를 계속할 수 밖에 없다⋯.",
    "【타임 리미트】 진료를 받고 한계가 가까워졌다는 말을 듣는다. 오버드의 능력이 육체가 견딜 수 없다, 능력이 쇠퇴하고 있다 등.",
    "【플리즈】 로이스의 대상 등에게로부터 히어로를 은퇴하라는 부탁을 받는다. 당신이 걱정된다, 너무 위험하다, 등등. 들어줄 수는 없지만⋯.",
];

/// Ruby `TABLES["PCN"]`（`1D10`）。
static KO_PCN: Table = Table::from_dice("프롤로그 차트 (네거티브)", 1, 10, KO_PCN_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static KO_TABLES: &[(&str, TableRef)] = &[
    ("HC", TableRef::Range(&KO_HC)),
    ("PCP", TableRef::Plain(&KO_PCP)),
    ("PCN", TableRef::Plain(&KO_PCN)),
];

/// `ko_kr` ロケールの表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    positive_emotion_table: &KO_POSITIVE_EMOTION_TABLE,
    negative_emotion_table: &KO_NEGATIVE_EMOTION_TABLE,
    tables: KO_TABLES,
    et_name: "감정표",
    invalid_critical: "크리티컬치가 너무 낮습니다. 2 이상을 지정해주세요.",
    auto_failure: "자동실패",
    fumble: "펌블",
    success: "성공",
    failure: "실패",
};

#[cfg(test)]
mod tests {

    use super::*;

    /// Ruby `RangeTable#store` が構築時に行う検査（隙間・重なり・端の被覆）。
    #[test]
    fn range_tables_are_complete() {
        for (name, table) in [
            ("positive_emotion_table", KO_POSITIVE_EMOTION_TABLE),
            ("negative_emotion_table", KO_NEGATIVE_EMOTION_TABLE),
            ("HC", KO_HC),
        ] {
            assert_eq!(table.validate(), Ok(()), "{name}");
        }
    }

    /// `test/data/DoubleCross_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DoubleCross:Korean",
            "DoubleCross_Korean.toml",
            95,
        );
    }
}
