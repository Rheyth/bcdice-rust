//! P4で手書き移植した `lib/bcdice/game_system/KemonoNoMori_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `KemonoNoMori` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! コマンド解釈・判定・表の引き方は [`super::KemonoNoMori`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データ（`KO_` 接頭辞の `static` 群）だけを置く。
//!
//! データは `i18n/KemonoNoMori/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。

use super::KemonoNoMori::{eval_specific_command, SystemTables, TableRef};
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::KemonoNoMori_Korean`（ID: `KemonoNoMori:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KemonoNoMori_Korean;

impl GameSystem for KemonoNoMori_Korean {
    fn id(&self) -> &'static str {
        "KemonoNoMori:Korean"
    }

    fn name(&self) -> &'static str {
        "짐승의 숲"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:짐승의 숲"
    }

    fn help_message(&self) -> &'static str {
        r"・행위 판정（성공도 자동 산출）（P119）: KAx[±y]
・지속 판정（성공도+1 고정）: KCx[±y]
   x=목표치
   y=목표치에 대한 수정（임의） x+y-z 처럼 여러 개 지정 가능
     예1）KA7+3 → 목표치7에 +3 수정을 더한 행위 판정
     예2）KC6 → 목표치6의 지속 판정
・함정 작동 체크+먹잇감표（P163）: CTR
   함정마다 1D12를 굴려, 12가 나온 경우 생물이 함정을 작동시켜 그 영향을 받고 있다.
・각종 표（기본 룰북）
  ・대실패표（P120）: FT
  ・능력치 무작위 결정표（P121）: RST
  ・무작위 소요 시간표（P122）: RTT
  ・무작위 소모표（P122）: RET
  ・무작위 날씨표（P128）: RWT
  ・무작위 날씨 지속표（P128）: RWDT
  ・무작위 엄폐물표（야외）（P140）: ROMT
  ・무작위 엄폐물표（실내）（P140）: RIMT
  ・도주 체험표（P144）: EET
  ・식재료 채집표（P157）: GFT
  ・물 채집표（P157）: GWT
  ・백색 마석 효과표（P186）: WST
・부위 피해 관련 표（참조 페이지는 리플레이&데이터북 「가미신의 연회」 기준）
  ・인간 부위표（P216）: HPT
  ・부위 피해 단계표（P217）: PDT
  ・네발 동물 부위표（P225）: QPT
  ・무족류 부위표（P225）: APT
  ・두발 동물 부위표（P226）: TPT
  ・새 부위표（P226）: BPT
  ・두족류 부위표（P227）: CPT
  ・곤충 부위표（P227）: IPT
  ・거미 부위표（P228）: SPT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "K[AC]", "CTR", "FT", "RST", "RTT", "RET", "RWT", "RWDT", "ROMT", "RIMT", "EET", "GFT",
            "GWT", "WST", "HPT", "PDT", "QPT", "APT", "TPT", "BPT", "CPT", "IPT", "SPT",
        ]
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

/// i18n `ko_kr.KemonoNoMori.critical`。
fn ko_critical(success_degree: i64) -> String {
    format!("대성공（성공도+{success_degree}, 다음 지속 판정의 목표치를 10으로 변경）")
}

/// i18n `ko_kr.KemonoNoMori.success`。
fn ko_success(success_degree: i64) -> String {
    format!("성공（성공도+{success_degree}）")
}

/// i18n `ko_kr.KemonoNoMori.trap_not_activated`。
fn ko_trap_not_activated(check_num: i64) -> String {
    format!("함정 작동 체크(1D12) ＞ {check_num} ＞ 함정이 작동하지 않았다")
}

/// i18n `ko_kr.KemonoNoMori.trap_activated_small`。
fn ko_trap_activated_small(check_num: i64, chase_num: i64) -> String {
    format!("함정 작동 체크(1D12) ＞ {check_num} ＞ 함정이 작동했다！ ＞ 먹잇감표({chase_num}) ＞ 소형 동물이 함정에 걸려 있었다")
}

/// i18n `ko_kr.KemonoNoMori.trap_activated_large`。
fn ko_trap_activated_large(check_num: i64, chase_num: i64) -> String {
    format!("함정 작동 체크(1D12) ＞ {check_num} ＞ 함정이 작동했다！ ＞ 먹잇감표({chase_num}) ＞ 대형 동물이 함정에 걸려 있었다")
}

/// i18n `ko_kr.KemonoNoMori.trap_activated_human`。
fn ko_trap_activated_human(check_num: i64, chase_num: i64) -> String {
    format!("함정 작동 체크(1D12) ＞ {check_num} ＞ 함정이 작동했다！ ＞ 먹잇감표({chase_num}) ＞ 인간 방랑자가 함정에 걸려 있었다")
}

/// i18n `ko_kr.KemonoNoMori.reappear`。
fn ko_reappear(hours: i64) -> String {
    format!("재등장: {hours}시간 후")
}

// ---------------------------------------------------------------------------
// 表データ（i18n/KemonoNoMori/ko_kr.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `KemonoNoMori.table.FT.items`。
static KO_FT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "【여유】가 3점 감소한다（최소 0까지）"),
    (
        RangeInc::new(4, 5),
        "무작위 짐 1개가 떨어져 행방불명이 된다（대실패한 구역의 아이템 조사로 발견 가능）",
    ),
    (RangeInc::new(6, 7), "무작위 짐 1개가 파괴된다"),
    (
        RangeInc::new(8, 9),
        "무작위 날씨표(RWT)를 사용하고, 결과를 턴 종료까지 적용한다",
    ),
    (
        RangeInc::new(10, 10),
        "무작위로 준비 중인 소도구 1개가 파괴된다",
    ),
    (RangeInc::new(11, 11), "장착 중인 방어구가 파괴된다"),
    (RangeInc::new(12, 12), "준비 중인 무기가 파괴된다"),
];

/// Ruby `TABLES["FT"]`（`1D12`）。
static KO_FT: RangeTable = RangeTable::from_dice("대실패표", 1, 12, KO_FT_ITEMS);

/// i18n `KemonoNoMori.table.RST.items`。
static KO_RST_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "【이동】"),
    (RangeInc::new(3, 4), "【격투】"),
    (RangeInc::new(5, 6), "【사격】"),
    (RangeInc::new(7, 8), "【제작】"),
    (RangeInc::new(9, 10), "【감지】"),
    (RangeInc::new(11, 12), "【자제】"),
];

/// Ruby `TABLES["RST"]`（`1D12`）。
static KO_RST: RangeTable = RangeTable::from_dice("능력치 무작위 결정표", 1, 12, KO_RST_ITEMS);

/// i18n `KemonoNoMori.table.RTT.items`。
static KO_RTT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "2"),
    (RangeInc::new(4, 6), "3"),
    (RangeInc::new(7, 9), "4"),
    (RangeInc::new(10, 12), "5"),
];

/// Ruby `TABLES["RTT"]`（`1D12`）。
static KO_RTT: RangeTable = RangeTable::from_dice("무작위 소요 시간표", 1, 12, KO_RTT_ITEMS);

/// i18n `KemonoNoMori.table.RET.items`。
static KO_RET_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "0"),
    (RangeInc::new(4, 6), "1"),
    (RangeInc::new(7, 9), "2"),
    (RangeInc::new(10, 12), "4"),
];

/// Ruby `TABLES["RET"]`（`1D12`）。
static KO_RET: RangeTable = RangeTable::from_dice("무작위 소모표", 1, 12, KO_RET_ITEMS);

/// i18n `KemonoNoMori.table.RWT.items`。
static KO_RWT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "짙은 안개"),
    (RangeInc::new(3, 4), "폭우"),
    (RangeInc::new(5, 6), "뇌우"),
    (RangeInc::new(7, 8), "강풍"),
    (RangeInc::new(9, 10), "혹서"),
    (RangeInc::new(11, 12), "혹한"),
];

/// Ruby `TABLES["RWT"]`（`1D12`）。
static KO_RWT: RangeTable = RangeTable::from_dice("무작위 날씨표", 1, 12, KO_RWT_ITEMS);

/// i18n `KemonoNoMori.table.RWDT.items`。
static KO_RWDT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "1턴"),
    (RangeInc::new(3, 4), "3턴"),
    (RangeInc::new(5, 6), "6턴"),
    (RangeInc::new(7, 8), "24턴"),
    (RangeInc::new(9, 10), "72턴"),
    (RangeInc::new(11, 12), "156턴"),
];

/// Ruby `TABLES["RWDT"]`（`1D12`）。
static KO_RWDT: RangeTable = RangeTable::from_dice("무작위 날씨 지속표", 1, 12, KO_RWDT_ITEMS);

/// i18n `KemonoNoMori.table.ROMT.items`。
static KO_ROMT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "【덤불】내구도3, 경감치1, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -1 수정 부가"),
    (RangeInc::new(3, 5), "【나무】내구도5, 경감치2, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -1 수정 부가"),
    (RangeInc::new(6, 8), "【큰 나무】내구도7, 경감치3, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -2 수정 부가"),
    (RangeInc::new(9, 10), "【바위】내구도6, 경감치4, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -1 수정 부가 / 컨택트 내에서 이루어지는 격투 공격의 대미지 +1"),
    (RangeInc::new(11, 12), "【암벽】내구도8, 경감치4, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -2 수정 부가 / 컨택트 내에서 이루어지는 격투 공격의 대미지 +2"),
];

/// Ruby `TABLES["ROMT"]`（`1D12`）。
static KO_ROMT: RangeTable = RangeTable::from_dice("무작위 엄폐물표（야외）", 1, 12, KO_ROMT_ITEMS);

/// i18n `KemonoNoMori.table.RIMT.items`。
static KO_RIMT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 4), "【목재 벽】내구도4, 경감치2, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -1 수정 부가"),
    (RangeInc::new(5, 8), "【목재 문】내구도4, 경감치2, 특수효과: 컨택트 내 캐릭터에 대한 사격 공격 판정에 -1, 접촉 판정과 돌격 판정에 -2 수정 부가"),
    (RangeInc::new(9, 12), "【목제 가구】내구도3, 경감치2, 특수효과: 컨택트 내에서 이루어지는 격투 공격의 대미지 +1"),
];

/// Ruby `TABLES["RIMT"]`（`1D12`）。
static KO_RIMT: RangeTable = RangeTable::from_dice("무작위 엄폐물표（실내）", 1, 12, KO_RIMT_ITEMS);

/// i18n `KemonoNoMori.table.EET.items`。
static KO_EET_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "【여유】가 0이 된다"),
    (RangeInc::new(4, 6), "임의의 【유대】를 합계 2점 감소시킨다"),
    (
        RangeInc::new(7, 9),
        "모든 짐을 잃는다（도주한 구역에 배치되며, 조사로 발견 가능）",
    ),
    (
        RangeInc::new(10, 12),
        "모든 무기와 방어구와 소도구와 짐을 잃는다（도주한 구역에 배치되며, 조사로 발견 가능）",
    ),
];

/// Ruby `TABLES["EET"]`（`1D12`）。
static KO_EET: RangeTable = RangeTable::from_dice("도주 체험표", 1, 12, KO_EET_ITEMS);

/// i18n `KemonoNoMori.table.GFT.items`。
static KO_GFT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "먹을 수 있는 뿌리（영양가:2）"),
    (RangeInc::new(3, 5), "먹을 수 있는 풀（영양가:3）"),
    (RangeInc::new(6, 8), "먹을 수 있는 열매（영양가:5）"),
    (RangeInc::new(9, 10), "소형 동물（영양가:10）"),
    (RangeInc::new(11, 11), "대형 동물（영양가:40）"),
    (RangeInc::new(12, 12), "징그러운 벌레（영양가:1）"),
];

/// Ruby `TABLES["GFT"]`（`1D12`）。
static KO_GFT: RangeTable = RangeTable::from_dice("식재료 채집표", 1, 12, KO_GFT_ITEMS);

/// i18n `KemonoNoMori.table.GWT.items`。
static KO_GWT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 6), "오염수"),
    (RangeInc::new(7, 11), "음료수"),
    (RangeInc::new(12, 12), "독수"),
];

/// Ruby `TABLES["GWT"]`（`1D12`）。
static KO_GWT: RangeTable = RangeTable::from_dice("물 채집표", 1, 12, KO_GWT_ITEMS);

/// i18n `KemonoNoMori.table.WST.items`。
static KO_WST_ITEMS: &[&str] = &[
    "도움이 안 되는 것의 색을 바꾼다",
    "도움이 안 되는 것을 크게 만든다",
    "도움이 안 되는 것을 작게 만든다",
    "도움이 안 되는 것을 보존한다",
    "도움이 안 되는 것을 복원한다",
    "도움이 안 되는 것을 소환한다",
    "도움이 안 되는 것을 움직인다",
    "도움이 안 되는 것을 늘린다",
    "도움이 안 되는 것을 붙인다",
    "도움이 안 되는 것을 만들어낸다",
    "소형 동물을 소환한다",
    "대형 동물을 소환한다",
];

/// Ruby `TABLES["WST"]`（`1D12`）。
static KO_WST: Table = Table::from_dice("백색 마석 효과표", 1, 12, KO_WST_ITEMS);

/// i18n `KemonoNoMori.table.HPT.items`。
static KO_HPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "오른팔 부위"),
    (RangeInc::new(3, 4), "왼팔 부위"),
    (RangeInc::new(5, 6), "오른쪽 다리 부위"),
    (RangeInc::new(7, 8), "왼쪽 다리 부위"),
    (RangeInc::new(9, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["HPT"]`（`1D12`）。
static KO_HPT: RangeTable = RangeTable::from_dice("인간 부위표", 1, 12, KO_HPT_ITEMS);

/// i18n `KemonoNoMori.table.PDT.items`。
static KO_PDT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 6), "경상"),
    (RangeInc::new(7, 10), "중상"),
    (RangeInc::new(11, 11), "파괴"),
    (RangeInc::new(12, 12), "상실"),
];

/// Ruby `TABLES["PDT"]`（`1D12`）。
static KO_PDT: RangeTable = RangeTable::from_dice("부위 피해 단계표", 1, 12, KO_PDT_ITEMS);

/// i18n `KemonoNoMori.table.QPT.items`。
static KO_QPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "이형"),
    (RangeInc::new(3, 3), "무기"),
    (RangeInc::new(4, 4), "오른쪽 앞다리 부위"),
    (RangeInc::new(5, 5), "왼쪽 앞다리 부위"),
    (RangeInc::new(6, 6), "오른쪽 뒷다리 부위"),
    (RangeInc::new(7, 7), "왼쪽 뒷다리 부위"),
    (RangeInc::new(8, 10), "몸통 부위"),
    (RangeInc::new(11, 12), "머리 부위"),
];

/// Ruby `TABLES["QPT"]`（`1D12`）。
static KO_QPT: RangeTable = RangeTable::from_dice("네발 동물 부위표", 1, 12, KO_QPT_ITEMS);

/// i18n `KemonoNoMori.table.APT.items`。
static KO_APT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "이형"),
    (RangeInc::new(4, 6), "무기"),
    (RangeInc::new(7, 10), "몸통 부위"),
    (RangeInc::new(11, 12), "머리 부위"),
];

/// Ruby `TABLES["APT"]`（`1D12`）。
static KO_APT: RangeTable = RangeTable::from_dice("무족류 부위표", 1, 12, KO_APT_ITEMS);

/// i18n `KemonoNoMori.table.TPT.items`。
static KO_TPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "이형"),
    (RangeInc::new(2, 2), "무기"),
    (RangeInc::new(3, 3), "오른팔 부위"),
    (RangeInc::new(4, 4), "왼팔 부위"),
    (RangeInc::new(5, 6), "오른쪽 다리 부위"),
    (RangeInc::new(7, 8), "왼쪽 다리 부위"),
    (RangeInc::new(9, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["TPT"]`（`1D12`）。
static KO_TPT: RangeTable = RangeTable::from_dice("두발 동물 부위표", 1, 12, KO_TPT_ITEMS);

/// i18n `KemonoNoMori.table.BPT.items`。
static KO_BPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "이형"),
    (RangeInc::new(2, 2), "무기"),
    (RangeInc::new(3, 4), "오른쪽 날개（오른팔 부위）"),
    (RangeInc::new(5, 6), "왼쪽 날개（왼팔 부위）"),
    (RangeInc::new(7, 7), "오른쪽 다리 부위"),
    (RangeInc::new(8, 8), "왼쪽 다리 부위"),
    (RangeInc::new(9, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["BPT"]`（`1D12`）。
static KO_BPT: RangeTable = RangeTable::from_dice("새 부위표", 1, 12, KO_BPT_ITEMS);

/// i18n `KemonoNoMori.table.CPT.items`。
static KO_CPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "이형"),
    (RangeInc::new(2, 2), "무기"),
    (RangeInc::new(3, 3), "오른팔 부위"),
    (RangeInc::new(4, 4), "왼팔 부위"),
    (RangeInc::new(5, 7), "오른쪽 다리 부위"),
    (RangeInc::new(8, 10), "왼쪽 다리 부위"),
    (RangeInc::new(11, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["CPT"]`（`1D12`）。
static KO_CPT: RangeTable = RangeTable::from_dice("두족류 부위표", 1, 12, KO_CPT_ITEMS);

/// i18n `KemonoNoMori.table.IPT.items`。
static KO_IPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "이형"),
    (RangeInc::new(3, 3), "무기"),
    (RangeInc::new(4, 4), "오른쪽 앞다리 부위"),
    (RangeInc::new(5, 5), "왼쪽 앞다리 부위"),
    (RangeInc::new(6, 6), "오른쪽 중간다리 부위"),
    (RangeInc::new(7, 7), "왼쪽 중간다리 부위"),
    (RangeInc::new(8, 8), "오른쪽 뒷다리 부위"),
    (RangeInc::new(9, 9), "왼쪽 뒷다리 부위"),
    (RangeInc::new(10, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["IPT"]`（`1D12`）。
static KO_IPT: RangeTable = RangeTable::from_dice("곤충 부위표", 1, 12, KO_IPT_ITEMS);

/// i18n `KemonoNoMori.table.SPT.items`。
static KO_SPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "이형"),
    (RangeInc::new(2, 2), "무기"),
    (RangeInc::new(3, 3), "오른쪽 첫번째 다리 부위"),
    (RangeInc::new(4, 4), "왼쪽 첫번째 다리 부위"),
    (RangeInc::new(5, 5), "오른쪽 두번째 다리 부위"),
    (RangeInc::new(6, 6), "왼쪽 두번째 다리 부위"),
    (RangeInc::new(7, 7), "오른쪽 세번째 다리 부위"),
    (RangeInc::new(8, 8), "왼쪽 세번째 다리 부위"),
    (RangeInc::new(9, 9), "오른쪽 네번째 다리 부위"),
    (RangeInc::new(10, 10), "왼쪽 네번째 다리 부위"),
    (RangeInc::new(11, 11), "몸통 부위"),
    (RangeInc::new(12, 12), "머리 부위"),
];

/// Ruby `TABLES["SPT"]`（`1D12`）。
static KO_SPT: RangeTable = RangeTable::from_dice("거미 부위표", 1, 12, KO_SPT_ITEMS);

/// Ruby `TABLES`（`general_tables` → `field_tables` → `body_part_tables` のマージ順）。
static KO_TABLES: &[(&str, TableRef)] = &[
    ("FT", TableRef::Range(&KO_FT)),
    ("RST", TableRef::Range(&KO_RST)),
    ("RTT", TableRef::Range(&KO_RTT)),
    ("RET", TableRef::Range(&KO_RET)),
    ("RWT", TableRef::Range(&KO_RWT)),
    ("RWDT", TableRef::Range(&KO_RWDT)),
    ("ROMT", TableRef::Range(&KO_ROMT)),
    ("RIMT", TableRef::Range(&KO_RIMT)),
    ("EET", TableRef::Range(&KO_EET)),
    ("GFT", TableRef::Range(&KO_GFT)),
    ("GWT", TableRef::Range(&KO_GWT)),
    ("WST", TableRef::Plain(&KO_WST)),
    ("HPT", TableRef::Range(&KO_HPT)),
    ("PDT", TableRef::Range(&KO_PDT)),
    ("QPT", TableRef::Range(&KO_QPT)),
    ("APT", TableRef::Range(&KO_APT)),
    ("TPT", TableRef::Range(&KO_TPT)),
    ("BPT", TableRef::Range(&KO_BPT)),
    ("CPT", TableRef::Range(&KO_CPT)),
    ("IPT", TableRef::Range(&KO_IPT)),
    ("SPT", TableRef::Range(&KO_SPT)),
];

/// `ko_kr` ロケールの表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    fumble: "대실패",
    critical: ko_critical,
    success: ko_success,
    failure: "실패",
    trap_not_activated: ko_trap_not_activated,
    trap_activated_small: ko_trap_activated_small,
    trap_activated_large: ko_trap_activated_large,
    trap_activated_human: ko_trap_activated_human,
    reappear: ko_reappear,
};

#[cfg(test)]
mod tests {

    use super::*;

    /// Ruby `RangeTable#store` が構築時に行う検査（隙間・重なり・端の被覆）。
    #[test]
    fn range_tables_are_complete() {
        for (name, table) in KO_TABLES {
            let TableRef::Range(table) = table else {
                continue;
            };
            assert_eq!(table.validate(), Ok(()), "{name}");
        }
    }

    /// `test/data/KemonoNoMori_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "KemonoNoMori:Korean",
            "KemonoNoMori_Korean.toml",
            89,
        );
    }
}
