//! P4で手書き移植した `lib/bcdice/game_system/GardenOrder_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `GardenOrder` を継承し、判定結果の文言と負傷表を韓国語に差し替えている
//! だけなので、コマンド解釈は [`super::GardenOrder`] の実装をそのまま使い、
//! ここには `ko_kr` の文言と表データ（[`KO_TABLES`]）だけを置く。
//!
//! # 表データ
//!
//! `DAMAGE_TABLE` / `DAMAGE_TABLE_SE` は `GardenOrder_Korean.rb` から機械的に
//! 書き出したもので、値は1文字も変えていない。

use super::GardenOrder::{
    eval_specific_command, DamageRow, DamageTable, DamageTableSet, SystemTables,
};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::GardenOrder_Korean`（ID: `GardenOrder:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenOrder_Korean;

impl GameSystem for GardenOrder_Korean {
    fn id(&self) -> &'static str {
        "GardenOrder:Korean"
    }

    fn name(&self) -> &'static str {
        "가든 오더"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:가든 오더"
    }

    fn help_message(&self) -> &'static str {
        r"・기본 판정
　GOx/y@z　x：성공률, y：연속 공격 횟수(생략 가능), z：크리티컬치 (생략 가능)
　(연속 공격은1회의 판정만 실행됩니다.)
　예시）GO55　GO100/2　GO70@10　GO155/3@44

・부상 표
　DCxxy
　xx：속성 (절단: SL, 총탄: BL, 충격: IM, 작열: BR, 냉각: RF, 전격: EL)
　y：대미지
　예시）DCSL7　DCEL22

・부상 표(소울 인코더용)
　SExxy
　xx：속성 (절단: SL, 총탄: BL, 충격: IM, 작열: BR, 냉각: RF, 전격: EL)
　y：대미지
　예시）SESL7　SEEL22
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["GO", "DC(SL|BL|IM|BR|RF|EL).+", "SE(SL|BL|IM|BR|RF|EL).+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `GardenOrder_Korean#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_TABLES, command, rng)
    }
}

/// `ko_kr` ロケールの表と定型文。
static KO_TABLES: SystemTables = SystemTables {
    fumble: "펌블",
    critical: "크리티컬",
    success: "성공",
    failure: "실패",
    repeat_attack_warning: "연속 공격은 성공률이 50% 이상 필요합니다.",
    damage_chart_title: "부상 표",
    damage_table: DamageTableSet {
        sl: &KO_DC_SL,
        bl: &KO_DC_BL,
        im: &KO_DC_IM,
        br: &KO_DC_BR,
        rf: &KO_DC_RF,
        el: &KO_DC_EL,
    },
    damage_table_se: DamageTableSet {
        sl: &KO_SE_SL,
        bl: &KO_SE_BL,
        im: &KO_SE_IM,
        br: &KO_SE_BR,
        rf: &KO_SE_RF,
        el: &KO_SE_EL,
    },
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["SL"]`（절단）。
static KO_DC_SL: DamageTable = DamageTable {
    name: "절단",
    rows: &[
        DamageRow {
            max: 5,
            name: "베인 상처",
            text: "피부가 찢어진다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 10,
            name: "다리 부상",
            text: "다리가 찢어져 무심코 무릎을 꿇는다.",
            damage: "경상 2/마비",
        },
        DamageRow {
            max: 13,
            name: "출혈",
            text: "베인 상처에서 출혈이 계속된다.",
            damage: "경상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 16,
            name: "몸통 부상",
            text: "몸통에 큰 상처를 입는다.",
            damage: "경상 4",
        },
        DamageRow {
            max: 19,
            name: "몸통 부상",
            text: "팔에 큰 상처를 입는다.",
            damage: "중상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 22,
            name: "몸통 부상",
            text: "복부가 깊게 찢어진다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 25,
            name: "대량 출혈",
            text: "상처가 깊게 나 대량으로 피를 흘린다.",
            damage: "중상 2/DOT: 경상 2",
        },
        DamageRow {
            max: 28,
            name: "열상(裂傷)",
            text: "낫기 어려운 상처를 입는다.",
            damage: "중상 3",
        },
        DamageRow {
            max: 31,
            name: "시야 불량",
            text: "머리를 다쳐 피가 흐르고 시야가 가려진다.",
            damage: "중상 3/스턴",
        },
        DamageRow {
            max: 34,
            name: "흉부 부상",
            text: "가슴에서 허리까지 크게 찢어진다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 37,
            name: "동맥 절단",
            text: "동맥이 찢어져 뿜어져 나오듯 출혈한다.",
            damage: "치명상 1/경상 3",
        },
        DamageRow {
            max: 39,
            name: "흉부 절단",
            text: "상처가 폐까지 이르러 객혈한다.",
            damage: "치명상 2",
        },
        DamageRow {
            max: 9999,
            name: "척수 손상",
            text: "척수가 손상된다.",
            damage: "치명상 2/방심, 스턴, 마비",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["BL"]`（총탄）。
static KO_DC_BL: DamageTable = DamageTable {
    name: "총탄",
    rows: &[
        DamageRow {
            max: 5,
            name: "팔 부상",
            text: "총알이 팔을 스쳤다.",
            damage: "경상 2",
        },
        DamageRow {
            max: 10,
            name: "팔 관통",
            text: "총알이 팔을 관통한다. 고통은 있지만 움직임에 지장은 없다.",
            damage: "경상 3",
        },
        DamageRow {
            max: 13,
            name: "몸통 부상",
            text: "다리에 총알이 박힌다. 고통으로 움직임이 둔해진다.",
            damage: "경상 3/슬로우: -3",
        },
        DamageRow {
            max: 16,
            name: "어깨 부상",
            text: "어깨를 관통한다. 뼈가 으스러진 것 같다.",
            damage: "중상 1",
        },
        DamageRow {
            max: 19,
            name: "복부 부상",
            text: "복부를 관통한다. 간신히 내장은 무사한 것 같다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 22,
            name: "다리 관통",
            text: "총알이 다리를 관통해 그 자리에서 무릎 꿇는다.",
            damage: "중상 2/마비",
        },
        DamageRow {
            max: 25,
            name: "소화 기관 손상",
            text: "위장 같은 소화 기관에 손상을 입는다.",
            damage: "중상 3",
        },
        DamageRow {
            max: 28,
            name: "맹관(盲管) 총상",
            text: "신체에 탄환이 깊숙이 박힌다. 격통이 온다.",
            damage: "중상 3/슬로우: -5",
        },
        DamageRow {
            max: 31,
            name: "내장 손상",
            text: "총알이 내장 몇 개를 헤집었다.",
            damage: "치명상 1/스턴",
        },
        DamageRow {
            max: 34,
            name: "몸통 관통",
            text: "복부를 관통당해 출혈이 생긴다.",
            damage: "치명상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 37,
            name: "흉부 부상",
            text: "탄환이 폐를 관통한다.",
            damage: "치명상 2",
        },
        DamageRow {
            max: 39,
            name: "치명적인 일격",
            text: "총알이 머리에 명중. 쇼크로 의식이 날아간다.",
            damage: "치명상 2/방심",
        },
        DamageRow {
            max: 9999,
            name: "필살의 일격",
            text: "총알이 심장 근처를 관통한다. 동맥이 손상된 것 같다.",
            damage: "치명상 2/DOT: 중상 1",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["IM"]`（충격）。
static KO_DC_IM: DamageTable = DamageTable {
    name: "충격",
    rows: &[
        DamageRow {
            max: 5,
            name: "타박상",
            text: "공격을 받은 부분이 거무칙칙하게 붓는다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 10,
            name: "휘청거림",
            text: "충격으로 넘어진다.",
            damage: "경상 1/마비",
        },
        DamageRow {
            max: 13,
            name: "평형 감각 상실",
            text: "충격으로 반고리관이 손상된다.",
            damage: "경상 2, 피로 2",
        },
        DamageRow {
            max: 16,
            name: "보디 블로우",
            text: "복부에 직격. 고통이 계속해서 체력을 빼앗는다.",
            damage: "경상 3/DOT: 피로 3",
        },
        DamageRow {
            max: 19,
            name: "통타(痛打)",
            text: "몸통이나 다리 부분 등에 타격을 입는다.",
            damage: "경상 4/스턴",
        },
        DamageRow {
            max: 22,
            name: "두부 통타",
            text: "머리에 클린 히트. 정신이 몽롱하다.",
            damage: "경상 5/방심",
        },
        DamageRow {
            max: 25,
            name: "다리 골절",
            text: "공격이 다리에 명중해 골절된다.",
            damage: "중상 1/슬로우: -5",
        },
        DamageRow {
            max: 28,
            name: "크게 휘청거림",
            text: "심한 충격으로 부상과 동시에 자세가 크게 흐트러진다.",
            damage: "중상 1/마비, 스턴",
        },
        DamageRow {
            max: 31,
            name: "뇌진탕",
            text: "뇌가 거세게 흔들려 의식이 혼미해진다.",
            damage: "중상 2/방심",
        },
        DamageRow {
            max: 34,
            name: "복합 골절",
            text: "공격을 받은 부분이 크게 찌부러져 복합 골절된 것 같다.",
            damage: "중상 3/방심, 스턴",
        },
        DamageRow {
            max: 37,
            name: "두부 열상",
            text: "머리에 명중. 피부가 크게 찢어진다.",
            damage: "치명상 1, 피로 3",
        },
        DamageRow {
            max: 39,
            name: "늑골 부상",
            text: "부러진 갈비뼈가 폐에 박혀 제대로 숨을 쉴 수가 없다.",
            damage: "치명상 1/방심, 스턴",
        },
        DamageRow {
            max: 9999,
            name: "내장 손상",
            text: "충격이 몸의 중심까지 닿아 내장을 몇 군데 다친 것 같다.",
            damage: "치명상 2/DOT: 중상 1",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["BR"]`（작열）。
static KO_DC_BR: DamageTable = DamageTable {
    name: "작열",
    rows: &[
        DamageRow {
            max: 5,
            name: "화상",
            text: "피부에 작은 화상을 입는다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 10,
            name: "온도 상승",
            text: "열에 의해 부상 뿐만 아니라 체력도 빼앗긴다.",
            damage: "경상 2, 피로 1",
        },
        DamageRow {
            max: 13,
            name: "공포",
            text: "타오르는 불길에 공포를 느끼고 몸이 움츠러들어 움직임이 멈춘다.",
            damage: "경상 3/방심",
        },
        DamageRow {
            max: 16,
            name: "발화",
            text: "옷이나 신체 일부에 불이 옮겨붙는다.",
            damage: "경상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 19,
            name: "폭발",
            text: "폭발로 바람에 날아가 넘어진다.",
            damage: "중상 1/마비",
        },
        DamageRow {
            max: 22,
            name: "대화상",
            text: "자국이 남을 정도로 큰 화상을 입는다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 25,
            name: "열파",
            text: "화상과 거센 열로 의식이 몽롱하다.",
            damage: "중상 2/스턴",
        },
        DamageRow {
            max: 28,
            name: "대폭발",
            text: "격렬한 폭발로 날아가 부상을 입고 넘어진다.",
            damage: "중상 3/마비",
        },
        DamageRow {
            max: 31,
            name: "대발화",
            text: "광범위하게 불이 옮겨붙는다.",
            damage: "중상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 34,
            name: "탄화",
            text: "고열 때문에 탄 부분이 탄화되어 버린다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 37,
            name: "내장(기도) 화상",
            text: "뜨거운 공기를 들이마셔 기도에도 화상을 입는다.",
            damage: "치명상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 39,
            name: "전신 화상",
            text: "신체 곳곳에 깊은 화상을 입는다.",
            damage: "치명상 2",
        },
        DamageRow {
            max: 9999,
            name: "치명적 화상",
            text: "신체 대부분에 화상을 입는다.",
            damage: "치명상 2/스턴",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["RF"]`（냉각）。
static KO_DC_RF: DamageTable = DamageTable {
    name: "냉각",
    rows: &[
        DamageRow {
            max: 5,
            name: "냉기",
            text: "가벼운 동상을 입는다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 10,
            name: "서리 옷",
            text: "신체가 얇은 얼음으로 덮여 움직임이 무뎌진다.",
            damage: "경상 2, 피로 1",
        },
        DamageRow {
            max: 13,
            name: "동상",
            text: "동상에 의해 신체가 손상된다.",
            damage: "경상 3",
        },
        DamageRow {
            max: 16,
            name: "체온 저하",
            text: "냉기에 체온을 빼앗긴다.",
            damage: "경상 3/DOT: 피로 1",
        },
        DamageRow {
            max: 19,
            name: "얼음 족쇄",
            text: "팔꿈치나 무릎 등이 얼음으로 덮여 움직이기 어려워진다.",
            damage: "중상 1/마비",
        },
        DamageRow {
            max: 22,
            name: "대동상",
            text: "신체 곳곳에 동상을 입는다.",
            damage: "중상 1/DOT: 피로 2",
        },
        DamageRow {
            max: 25,
            name: "얼음의 속박",
            text: "하반신이 얼어붙어 움직일 수 없다.",
            damage: "중상 2/마비",
        },
        DamageRow {
            max: 28,
            name: "시야 불량",
            text: "머리에도 얼음이 얼어서 시야가 가려진다.",
            damage: "중상 2/스턴",
        },
        DamageRow {
            max: 31,
            name: "팔 동결",
            text: "팔이 얼어붙어 움직일 수가 없다.",
            damage: "중상 3/방심",
        },
        DamageRow {
            max: 34,
            name: "중증 동상",
            text: "체온이 더 떨어져 심각한 동상을 입는다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 37,
            name: "전신 동결",
            text: "온몸이 얼어붙는다.",
            damage: "치명상 1/DOT: 피로 2",
        },
        DamageRow {
            max: 39,
            name: "치명적 동상",
            text: "전신에 동상을 입는다.",
            damage: "치명상 2",
        },
        DamageRow {
            max: 9999,
            name: "얼음 관",
            text: "얼음에 완전히 갇힌다.",
            damage: "치명상 2/스턴, 마비",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE["EL"]`（전격）。
static KO_DC_EL: DamageTable = DamageTable {
    name: "전격",
    rows: &[
        DamageRow {
            max: 5,
            name: "정전기",
            text: "온몸에 소름이 끼친다.",
            damage: "피로 3",
        },
        DamageRow {
            max: 10,
            name: "전열상",
            text: "전류에 의해 손상된다.",
            damage: "피로 1, 경상 1",
        },
        DamageRow {
            max: 13,
            name: "감전",
            text: "몸에 전류가 통함과 동시에 몸이 가볍게 저려온다.",
            damage: "피로 2, 경상 2",
        },
        DamageRow {
            max: 16,
            name: "섬광",
            text: "격렬한 전광에 의해 일시적으로 시야가 차단된다.",
            damage: "경상 3/스턴",
        },
        DamageRow {
            max: 19,
            name: "다리 감전",
            text: "전류에 의해 다리가 강하게 저려오며 움직일 수 없게 된다.",
            damage: "중상 1/마비",
        },
        DamageRow {
            max: 22,
            name: "대전열상",
            text: "신체 곳곳이 전류에 의해 손상된다.",
            damage: "피로 2, 중상 2",
        },
        DamageRow {
            max: 25,
            name: "팔 부상",
            text: "전류에 의해 팔이 저려 움직일 수 없게 된다.",
            damage: "경상 1, 중상 2/방심",
        },
        DamageRow {
            max: 28,
            name: "대감전",
            text: "전류에 의해 온몸이 저려 움직일 수 없게 된다.",
            damage: "중상 2/스턴, 마비",
        },
        DamageRow {
            max: 31,
            name: "일시적인 심정지",
            text: "강력한 전격 쇼크로 심장이 잠시 멈춘다.",
            damage: "피로 3, 중상 3",
        },
        DamageRow {
            max: 34,
            name: "대전류",
            text: "전신에 전류가 돈다.",
            damage: "중상 3/방심, 마비",
        },
        DamageRow {
            max: 37,
            name: "치명 전열상",
            text: "전신이 전류에 의해 손상된다.",
            damage: "중상 1, 치명상 1",
        },
        DamageRow {
            max: 39,
            name: "심정지",
            text: "강력한 전격 쇼크로 심장이 일시적으로 멈춘다. 죽음의 문턱이 보인다.",
            damage: "피로 3, 중상 1, 치명상 1",
        },
        DamageRow {
            max: 9999,
            name: "조직 탄화",
            text: "온몸이 전류로 타들어가고 곳곳의 조직이 탄화된다.",
            damage: "치명상 2/스턴",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["SL"]`（절단）。
static KO_SE_SL: DamageTable = DamageTable {
    name: "절단",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 충격",
            text: "별 상처는 아니지만 충격을 받는다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "작은 상처",
            text: "외부 장비에 상처가 난다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 13,
            name: "큰 상처",
            text: "외부 장비에 큰 상처가 난다.",
            damage: "경상 2",
        },
        DamageRow {
            max: 16,
            name: "매우 큰 상처",
            text: "외부 장비에 아주 큰 상처가 나 내부도 대미지를 받는다.",
            damage: "경상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 19,
            name: "외관 파손",
            text: "외부 장비 일부가 빠진다.",
            damage: "경상 4",
        },
        DamageRow {
            max: 22,
            name: "내부 파손",
            text: "외부 장비 일부가 파손되어 내부도 대미지를 입는다.",
            damage: "중상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 25,
            name: "내부 대파손",
            text: "내부의 일부가 크게 파손된다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 28,
            name: "일시적 부전",
            text: "외부 장비 일부가 망가져 내부도 큰 대미지를 입는다.",
            damage: "중상 2/DOT: 경상 2",
        },
        DamageRow {
            max: 31,
            name: "열상",
            text: "상처를 입어 내부가 드러난다.",
            damage: "중상 3",
        },
        DamageRow {
            max: 34,
            name: "시야 불량",
            text: "장착된 카메라에 문제가 생긴다.",
            damage: "중상 3/스턴",
        },
        DamageRow {
            max: 37,
            name: "대열상",
            text: "크게 갈가리 찢겨 내부가 드러난다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 39,
            name: "기능 부전",
            text: "중요한 부품이 망가졌다. 이러다가는 기능에 큰 장애가 생긴다.",
            damage: "치명상 1/DOT: 경상 3",
        },
        DamageRow {
            max: 9999,
            name: "치명적 손상",
            text: "기능이 정지할 수도 있을 만한 큰 손상을 받는다.",
            damage: "치명상 2",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["BL"]`（총탄）。
static KO_SE_BL: DamageTable = DamageTable {
    name: "총탄",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 충격",
            text: "총알은 튕겨져 나갔지만 충격을 받는다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "작은 총상",
            text: "총알은 튕겨져 나갔지만 외부 장비가 함몰되었다.",
            damage: "경상 2",
        },
        DamageRow {
            max: 13,
            name: "큰 총상",
            text: "총알은 간신히 튕겼지만 외부 장비에 큰 함몰이 생겼다.",
            damage: "경상 3",
        },
        DamageRow {
            max: 16,
            name: "기능 저하",
            text: "총에 맞은 충격으로 내부 기능에 문제가 생겼다.",
            damage: "경상 4/슬로우: -3",
        },
        DamageRow {
            max: 19,
            name: "매우 큰 총상",
            text: "총알이 외부 기관에 박혔다.",
            damage: "중상 1",
        },
        DamageRow {
            max: 22,
            name: "총탄 정지",
            text: "총알에 관통당헀다. 가까스로 내부는 다치지 않은 것 같다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 25,
            name: "내부 손상",
            text: "총알에 관통당했을 때 내부 기능이 충격을 받는다.",
            damage: "중상 2/방심",
        },
        DamageRow {
            max: 28,
            name: "내부 파괴",
            text: "총알에 관통당했을 때 내부 기능에도 큰 대미지를 받는다.",
            damage: "중상 3",
        },
        DamageRow {
            max: 31,
            name: "내부 대파괴",
            text: "총알에 관통당했을 때의 충격으로 내부에 일시적인 손상이 생긴다.",
            damage: "중상 3/슬로우: -5",
        },
        DamageRow {
            max: 34,
            name: "기능 일부 정지",
            text: "총에 맞아 내부 기능이 일시적으로 쓸모없어졌다.",
            damage: "치명상 1/스턴",
        },
        DamageRow {
            max: 37,
            name: "파손",
            text: "총에 맞아 외부 장비가 날아가 내부 일부가 파괴된다.",
            damage: "치명상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 39,
            name: "대파손",
            text: "총에 맞아 외부 장비와 내부의 일부가 날아간다.",
            damage: "치명상 2",
        },
        DamageRow {
            max: 9999,
            name: "치명적인 일격",
            text: "총알이 중요 부위에 명중한 충격으로 기능 대부분이 일시 정지한다.",
            damage: "치명상 2/방심",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["IM"]`（충격）。
static KO_SE_IM: DamageTable = DamageTable {
    name: "충격",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 충격",
            text: "상처도 함몰도 없지만 충격을 받는다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "충격",
            text: "충격을 받아 외부 장비가 패인다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 13,
            name: "큰 충격",
            text: "충격을 받아 외부 장비가 크게 패인다.",
            damage: "경상 2",
        },
        DamageRow {
            max: 16,
            name: "매우 큰 충격",
            text: "충격을 받아 외부 장비가 크게 패여 기능이 순간적으로 정지한다.",
            damage: "경상 2/방심",
        },
        DamageRow {
            max: 19,
            name: "내부 압박",
            text: "외부 장비가 패여 내부를 압박하고 있다.",
            damage: "경상 3/DOT:  피로 3",
        },
        DamageRow {
            max: 22,
            name: "통타(痛打)",
            text: "맞은 곳이 손상되어 카메라가 일시 정지한다.",
            damage: "경상 4/스턴",
        },
        DamageRow {
            max: 25,
            name: "내부 충격",
            text: "맞은 곳이 손상되어 내부에 큰 대미지가 간다. 기능이 일시적으로 정지한다.",
            damage: "경상 5/방심",
        },
        DamageRow {
            max: 28,
            name: "기능 장애",
            text: "충격으로 동작에 장애가 생긴다.",
            damage: "중상 1/슬로우: -5",
        },
        DamageRow {
            max: 31,
            name: "외관 손상",
            text: "손상된 외부 장비의 파편이 내부에 박힌다.",
            damage: "중상 1/방심, 스턴",
        },
        DamageRow {
            max: 34,
            name: "외관 대손상",
            text: "외부 장비가 크게 손상되고 내부도 곳곳이 파괴된다.",
            damage: "중상 2/방심",
        },
        DamageRow {
            max: 37,
            name: "외관 파괴",
            text: "외부 장비 일부가 날아가고 내부도 파괴된다.",
            damage: "중상 3/방심, 스턴",
        },
        DamageRow {
            max: 39,
            name: "외관 대파괴",
            text: "외부 장비 일부와 내부가 날아간다.",
            damage: "치명상 1, 피로 3",
        },
        DamageRow {
            max: 9999,
            name: "치명적 파괴",
            text: "외부 장비 대부분이 파괴되고 내부도 으스러진다.",
            damage: "치명상 1/방심, 스턴",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["BR"]`（작열）。
static KO_SE_BR: DamageTable = DamageTable {
    name: "작열",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 용해",
            text: "외부 장비가 약간 녹아 기능이 저하한다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "용해",
            text: "외부 장비가 녹는다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 13,
            name: "온도 상승",
            text: "열로 인해 외부 장비 뿐만 아니라 내부도 약간 녹는다.",
            damage: "경상 2, 피로 1",
        },
        DamageRow {
            max: 16,
            name: "온도 대상승",
            text: "열로 인해 외부 장비 뿐만 아니라 내부도 녹는다.",
            damage: "경상 3/방심",
        },
        DamageRow {
            max: 19,
            name: "발화",
            text: "외부 장비에 불이 옮겨 붙는다.",
            damage: "경상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 22,
            name: "폭발",
            text: "폭발로 인해 외부 장비 일부가 날아간다.",
            damage: "중상 1/스턴",
        },
        DamageRow {
            max: 25,
            name: "대용해",
            text: "외부 장비에 자국이 남을 정도로 크게 녹는다.",
            damage: "중상 2",
        },
        DamageRow {
            max: 28,
            name: "열파",
            text: "강한 열로 인해 내부 기능이 저하한다.",
            damage: "중상 2/스턴",
        },
        DamageRow {
            max: 31,
            name: "대폭발",
            text: "격렬한 폭발로 외부 장비가 날아가 내부도 대미지를 받는다.",
            damage: "중상 3/방심",
        },
        DamageRow {
            max: 34,
            name: "대발화",
            text: "광범위하게 불이 옮겨 붙는다.",
            damage: "중상 3/DOT: 경상 1",
        },
        DamageRow {
            max: 37,
            name: "탄화",
            text: "너무 뜨거운 나머지 불에 탄 부분이 탄화된다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 39,
            name: "내부 용해",
            text: "내부에 열이나 불길이 들어가 크게 녹아내린다.",
            damage: "치명상 1/DOT: 경상 1",
        },
        DamageRow {
            max: 9999,
            name: "치명적 용해",
            text: "외부 장비나 내부 대부분이 녹는다.",
            damage: "치명상 2",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["RF"]`（냉각）。
static KO_SE_RF: DamageTable = DamageTable {
    name: "냉각",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 냉각",
            text: "냉기 때문에 기능이 저하한다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "냉기",
            text: "외부 장비가 얇은 얼음으로 덮인다.",
            damage: "경상 1",
        },
        DamageRow {
            max: 13,
            name: "서리 옷",
            text: "외부 장비가 얇은 얼음으로 덮여 동작이 무뎌진다.",
            damage: "경상 2/피로 1",
        },
        DamageRow {
            max: 16,
            name: "가벼운 동결",
            text: "추위로 얼어붙어 외부 장비 일부가 상한다.",
            damage: "경상 3",
        },
        DamageRow {
            max: 19,
            name: "온도 저하",
            text: "냉기 때문에 기능이 매우 저하한다.",
            damage: "경상 3/DOT: 피로 1",
        },
        DamageRow {
            max: 22,
            name: "얼음 족쇄",
            text: "가동부가 얼음으로 덮여 움직임이 어려워진다.",
            damage: "중상 1/스턴",
        },
        DamageRow {
            max: 25,
            name: "대동결",
            text: "외부 장비 대부분이 얼어붙는다.",
            damage: "중상 1/DOT: 피로 2",
        },
        DamageRow {
            max: 28,
            name: "얼음의 속박",
            text: "가동부가 완전히 얼어 움직일 수가 없다.",
            damage: "중상 2/방심",
        },
        DamageRow {
            max: 31,
            name: "시야 불량",
            text: "카메라 렌즈에도 얼음이 끼어 시야가 가려진다.",
            damage: "중상 2/스턴",
        },
        DamageRow {
            max: 34,
            name: "동작 불량",
            text: "동결 때문에 일부 동작에 어려움이 생긴다.",
            damage: "중상 3/방심",
        },
        DamageRow {
            max: 37,
            name: "중증 동결",
            text: "더욱 온도가 내려가 내부도 심각한 대미지를 받는다.",
            damage: "치명상 1",
        },
        DamageRow {
            max: 39,
            name: " 전신 동결",
            text: "온몸이 얼어붙는다.",
            damage: "치명상 1/DOT: 피로 2",
        },
        DamageRow {
            max: 9999,
            name: "치명적 동결",
            text: "외부 장비 뿐만이 아니라 내부도 치명적인 대미지를 받는다.",
            damage: "치명상 2",
        },
    ],
};

/// Ruby `GardenOrder_Korean::DAMAGE_TABLE_SE["EL"]`（전격）。
static KO_SE_EL: DamageTable = DamageTable {
    name: "전격",
    rows: &[
        DamageRow {
            max: 5,
            name: "가벼운 전격",
            text: "전격 때문에 기능이 저하한다.",
            damage: "스턴",
        },
        DamageRow {
            max: 10,
            name: "대전(帶電)",
            text: "대전되어 가벼운 대미지를 받는다.",
            damage: "피로 3",
        },
        DamageRow {
            max: 13,
            name: "전열상(電熱傷)",
            text: "전류로 인해 외부 장비가 손상된다.",
            damage: "피로 1, 경상 1",
        },
        DamageRow {
            max: 16,
            name: "가벼운 감전",
            text: "전류로 손상됨과 동시에 내부에 가벼운 대미지를 받는다.",
            damage: "피로 2, 경상 2",
        },
        DamageRow {
            max: 19,
            name: "섬광",
            text: "격렬한 섬광 때문에 일시적으로 카메라 기능이 마비된다.",
            damage: "경상 3/스턴",
        },
        DamageRow {
            max: 22,
            name: "감전",
            text: "전류로 인해 내부에 대미지를 받아 일시적으로 동작이 마비된다.",
            damage: "중상 1/방심",
        },
        DamageRow {
            max: 25,
            name: "대전열상",
            text: "외부 장비 곳곳이 전류로 손상된다.",
            damage: "피로 2, 중상 2",
        },
        DamageRow {
            max: 28,
            name: "감전으로 인한 부상",
            text: "외부 장비 뿐만이 아니라 내부도 큰 대미지를 받아 기능이 마비된다.",
            damage: "경상 1, 중상 2/방심",
        },
        DamageRow {
            max: 31,
            name: "대감전",
            text: "전류로 인해 기능의 대부분이 마비되어 동작하지 않게 된다.",
            damage: "중상 2/방심, 스턴",
        },
        DamageRow {
            max: 34,
            name: "대전류",
            text: "강력한 전격 쇼크로 내부도 상당한 대미지를 받는다.",
            damage: "피로 3, 중상 3",
        },
        DamageRow {
            max: 37,
            name: "일시 정지",
            text: "전신에 전류가 돌아 기능 대부분이 동작 불량을 일으킨다.",
            damage: "중상 3/방심, 스턴",
        },
        DamageRow {
            max: 39,
            name: "치명적 전열상",
            text: "전신이 전류로 손상된다.",
            damage: "중상 1, 치명상 1",
        },
        DamageRow {
            max: 9999,
            name: "기능 정지",
            text: "강력한 전격 쇼크로 고장 직전의 대미지를 받는다.",
            damage: "피로 3, 중상 1, 치명상 1",
        },
    ],
};

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GardenOrder:Korean",
            "GardenOrder_Korean.toml",
            52,
        );
    }
}
