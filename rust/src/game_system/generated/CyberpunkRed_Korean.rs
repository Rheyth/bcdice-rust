//! P4で手書き移植した `lib/bcdice/game_system/CyberpunkRed_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `CyberpunkRed` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! 表の引き方・コマンド解釈は [`super::CyberpunkRed`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データだけを置く。
//! 表データは `i18n/CyberpunkRed/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。

use super::CyberpunkRed::{
    eval_specific_command, ScreamSheetTables, ShopPeopleTables, SystemTables, TableRef,
};
use crate::dice_table::{ChainTable, RangeInc, RangeTable, Table, TableItem};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_FFD_ITEMS: &[&str] = &[
    "한쪽 팔 상실 ＞ 한쪽 팔을 완전히 상실한 경우. 팔에 들고 있던 물건을 즉시 떨어뜨립니다. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "한 손 손실 ＞ 한 손을 완전히 잃어버렸습니다. 손에 들고 있던 물건을 즉시 떨어뜨립니다. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "폐 손상은 ＞ [이동력]에서 -2(최소 이동력 1)를 합니다. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "갈비뼈 피해 ＞ 한 턴에 도보로 4미터(2칸) 이상 이동할 때마다 해당 턴이 끝날 때 이 치명적인 손상으로 HP에 직접 5 보너스 포인트를 받습니다.",
    "외팔 부상 ＞ 손상된 팔은 쓸모가 없습니다. 팔에 들고 있던 물건을 즉시 떨어뜨립니다.",
    "외상성 이물질 ＞ 한 턴에 도보로 4미터(2칸) 이상 이동할 때마다 해당 턴이 끝날 때 HP에 치명적인 손상을 입힌 후 직접 5의 보너스 데미지 포인트를 받습니다.",
    "한쪽 다리 부상 ＞ [이동력]에 -4(최소 이동력 1)를 합니다.",
    "근육 찢어짐 ＞근접 공격에 -2를 합니다.",
    "척추 부상 ＞ 다음 턴에 행동을 할 수 없지만 이동 행동은 할 수 있습니다. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "손가락 부상 ＞ 해당 손과 관련된 모든 행동에 대해 -4입니다.",
    "한쪽 다리 상실 ＞ 한쪽 다리를 완전히 상실한 경우. [이동력]에  -6(최소 이동력 1)을 합니다. 데스 세이브 패널티의 기본값이 1 증가합니다.",
];
static KO_FFD: Table = Table::from_dice("신체에 치명적인 손상", 2, 6, KO_FFD_ITEMS);

static KO_HFD_ITEMS: &[&str] = &[
    "한쪽 눈 상실 ＞ 한쪽 눈을 완전히 상실한 경우. 원거리 공격 및 시야와 관련된 \"인식\" 판정에 -2. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "뇌 손상 ＞ 모든 행동에 대해  -2. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "한쪽 눈 피해＞ 원거리 공격에 -2, 시야과 관련된 지각(Perception)에 -2입니다.",
    "뇌진탕 ＞ 모든 행동에 -2.",
    "턱 부상 ＞ 발성과 관련된 모든 행동에 대해 -2.",
    "외상성 이물질 ＞ 한 턴에 도보로 4미터(2칸) 이상 이동할 때마다 해당 턴이 끝날 때 치명적인 손상을 입힌 후  HP에 직접 5의 보너스 데미지 포인트를 받습니다.",
    "경추 손상 ＞ 사망 저장 패널티 값의 기본 값이 1 증가합니다.",
    "두개골 부상 ＞ 머리에 총을 쏘면 SP를 관통한 후 두 배가 아닌 세 배의 피해를 입힙니다. 데스 세이브 패널티의 기본값이 1 증가합니다.",
    "한쪽 귀 손상 ＞  한 턴에 도보로 4 미터 (2 칸) 이상 이동하면 다음 턴에 이동 동작을 수행 할 수 없습니다. 청각과 관련된 지각 판정(perception)에 -2를 합니다.",
    "기도 손상＞ 말을 할 수 없음. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
    "한쪽 귀 상실 ＞ 한쪽 귀를 완전히 상실한 경우. 한 턴에 도보로 4미터(2칸) 이상 이동하면 다음 턴에 이동 동작을 수행할 수 없습니다. 청각과 관련된 지각 판정(Perception)에 -4를 합니다.. 데스 세이브 패널티의 기본 값이 1 증가합니다.",
];
static KO_HFD: Table = Table::from_dice("머리에 치명적인 손상", 2, 6, KO_HFD_ITEMS);

static KO_NCDT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "지역 경찰관＞ 순찰 경찰관으로 PC 그룹의 절반 인원이 있습니다. p. 417 참조."),
    (RangeInc::new(6, 11), "기업 경비원 ＞ 해당 지역을 순찰하는 회사의 하위 수준 보안 요원입니다. PC 그룹과 같은 인원입니다. p. 417 참조."),
    (RangeInc::new(12, 13), "테키 ＞ PC 그룹에 있는 기술자(기술자) 수의 절반입니다. p. 417 참조."),
    (RangeInc::new(14, 17), "사립 탐정 ＞ 1 명의 사립 탐정. p. 417 참조."),
    (RangeInc::new(18, 20), "회사 직원 ＞ 현지 회사의 직원이 택시를 찾고 있습니다. 인원수는 PC의 수와 같습니다. p. 417 참조."),
    (RangeInc::new(21, 27), "지역 주민 ＞ 이 지역에 사는 두 명의 젊은이. p. 418 참조."),
    (RangeInc::new(28, 32), "Reclaimer ＞ PC는 잘 갖춰진 Reclaimer 갱단을 만나게 됩니다. 구성원 수는 (PC 그룹 수 -2) 이고 리더는 1입니다. p. 418 참조."),
    (RangeInc::new(33, 37), "미디어 ＞ 카메라와 인터뷰어의 미디어 듀오. 그들은 특별한 기능을 찾기 위해 한 건물에 말뚝을 박고 있습니다. p. 418 참조."),
    (RangeInc::new(38, 41), "사립 탐정 ＞ 1 명의 사립 탐정. p. 418 참조."),
    (RangeInc::new(42, 46), "트라우마팀 ＞ AV-4는 총격전 도중에 강제 착륙을 하고, 의료진이 내려와 부상당한 6명 정도의 갱단에게 응급 처치를 시작한다. p. 418 참조."),
    (RangeInc::new(47, 57), "스캐버 ＞ 불타버린 도시 블록 근처의 폐허나 쓰레기통을 뒤지고 있는 불쌍하고 더러운 방랑자 무리가 PC만큼 많습니다. p. 419 참조."),
    (RangeInc::new(58, 63), "노마드 ＞ 같은 수의 PC를 가진 유목민 그룹입니다. p. 419 참조."),
    (RangeInc::new(64, 70), "부스터 갱(Booster Gang) ＞ 피라냐즈(Piranhaz)라는 부스터 갱단에 속한 저급 스트리트 펑크 그룹입니다. 인원수는 PC의 수와 같습니다. p. 419 참조."),
    (RangeInc::new(71, 76), "스트리트 펑크(Street Punk) ＞ 합성 마약 중독자 그룹이 돈을 벌기 위해 오리를 찾고 있습니다(PC만큼 많습니다). p. 419 참조."),
    (RangeInc::new(77, 82), "컬티(광신자) ＞ 종말론적 컬트 「심판자」가 떼를 지어 몰려오고 있습니다. p. 419 참조."),
    (RangeInc::new(83, 88), "노마드의 트럭 ＞ 부서진 트럭 주변에서 강철 발로크의 유목민들(PC 수의 절반, 최소 두 대)이 뭔가를 하고 있습니다. p. 419 참조."),
    (RangeInc::new(89, 94), "부스터 갱 ＞ 아이언 사이트라는 갱단의 멤버입니다. 인원은 PC와 같습니다. p. 419 참조."),
    (RangeInc::new(95, 100), "중범 ＞ pc들은 무자비한 빌센코(Virshenko) 조직의 거대한 작전 현장에 있게 됩니다. p. 419 참조."),
];
static KO_NCDT: RangeTable =
    RangeTable::from_dice("나이트 시티의 낮의 만남", 1, 100, KO_NCDT_ITEMS);

static KO_NCMT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 10), "시 경찰 ＞ PC 그룹의 절반 인원을 가진 경찰관입니다. p. 422 참조."),
    (RangeInc::new(11, 22), "기업 경비원 ＞ 이 구역을 순찰하는 기업 경비원. PC 그룹과 같은 인원입니다. p. 422 참조."),
    (RangeInc::new(23, 24), "사설 탐정 ＞ 초대형 권총과 마체테로 무장하고 경장갑 잭을 착용한 사설 탐정 1명. p. 422 참조."),
    (RangeInc::single(25), "미디어 ＞ 카메라와 인터뷰어의 미디어 듀오. 그들은 특별한 기능을 찾기 위해 한 건물에 말뚝을 박고 있습니다. p. 422 참조."),
    (RangeInc::new(26, 29), "Cromer ＞ 지역 크로매틱 록 밴드의 하드코어 팬 그룹입니다. p. 423 참조."),
    (RangeInc::new(30, 39), "엣지러너 팀(Edgerunner Team) ＞ 엣지러너로 구성된 소규모 팀입니다. 네트 러너 1명, 솔로 1명, 노마드 1명. p. 423 참조."),
    (RangeInc::new(40, 42), "외상팀 ＞ AV-4는 총격전 도중에 강제 착륙을 하고, 의료진이 내려와 부상당한 6명 정도의 갱단에게 응급 처치를 시작한다. p. 423 참조."),
    (RangeInc::new(43, 45), "레인저 ＞ 로만(보안관)과 그의 배정된 보안관 보조는 도시에 숨어 있는 방랑자(지역 갱단원)를 찾고 있습니다. p. 423 참조."),
    (RangeInc::new(46, 58), "노마드 ＞ Wildman Pack에 속한 유목민 그룹입니다. PC보다 2명 더 많습니다. p. 423 참조."),
    (RangeInc::new(59, 63), "컬티(광신자) ＞ 심문 시간이다! 심문관들이 최대 병력으로 나왔다. pc 맞먹는 무리가 쌍절곤, 권총, 채찍으로 무장하고 당신을 몰아붙입니다. p. 423 참조."),
    (RangeInc::new(64, 73), "스트리트 펑크 ＞ 블랙 레이스 중독자들은 알약을 사기 위해 돈을 원합니다. PC보다 2명 더 많습니다. p. 423 참조."),
    (RangeInc::single(74), "주요 범죄자 ＞ PC는 악명 높은 스카가탈리아)Skagattaria 가족의 주요 작전의 한가운데에 있음을 알게 됩니다. p. 423 참조."),
    (RangeInc::new(75, 79), "갱 전쟁＞ 굉장합니다. PC는 이 지역에서 가장 큰 두 갱단 간의 영토 다툼 현장에 있었습니다. p. 423 참조."),
    (RangeInc::new(80, 87), "방화범(Arsonists) ＞ 지역의 누군가에게 원한을 품고 있는 급진적 무정부주의자 그룹입니다. 화염방사기, 도끼, 대형 권총으로 무장한 사이버디드 갱어 한 명이 PC보다 부스터 3개(최소 2개) 적은 부스터를 이끌고 있습니다. p. 424 참조."),
    (RangeInc::new(88, 92), "갱 전쟁＞ 굉장합니다. PC는 이 지역에서 가장 큰 두 갱단 간의 영토 다툼 현장에 있었습니다. p. 424 참조."),
    (RangeInc::new(93, 99), "주요 범죄자 ＞ PC는 악명 높은 스카가탈리아(Skagattaria) 가족의 주요 작전의 한가운데에 있음을 알게 됩니다. p. 424 참조."),
    (RangeInc::single(100), "날뛰는 사이버 사이코 ＞ 반짝이는 메탈 바디의 사이버 사이코. 자신을 멀리 밀쳐낸 행인에 의해 마지막 가장자리를 넘어 놓아 버린 것 같고, 그에게 분노를 쏟아내고 있는 것 같다. p. 424 참조."),
];
static KO_NCMT: RangeTable =
    RangeTable::from_dice("나이트 시티의 심야의 만남", 1, 100, KO_NCMT_ITEMS);

static KO_NMCT_ITEMS: &[&str] = &[
    "음식과 약물",
    "개인용 전자기기",
    "무기와 갑옷",
    "사이버웨어",
    "의류 및 패션웨어",
    "생존 장비(servival gear)",
];
static KO_NMCT: Table = Table::from_dice("야시장 테이블", 1, 6, KO_NMCT_ITEMS);

static KO_NMCFO_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "통조림　10ed(저렴함)"),
    (RangeInc::new(6, 10), "패키지 상품　10ed(저렴함)"),
    (RangeInc::new(11, 15), "냉동식품　10ed(저렴함)"),
    (RangeInc::new(16, 20), "곡물 가방　20ed(일상적임)"),
    (RangeInc::new(21, 25), "키블 팩　10ed(저렴함)"),
    (RangeInc::new(26, 30), "프리팩 가방　20ed(일상적임)"),
    (RangeInc::new(31, 35), "20eb 이하의 길거리 약물"),
    (RangeInc::new(36, 40), "낮은 품질의 술　10ed(저렴함)"),
    (RangeInc::new(41, 45), "술　20ed(일상적임)"),
    (RangeInc::new(46, 50), "훌륭한 품질의 술　100ed(프리미엄)"),
    (RangeInc::new(51, 55), "MRE(전투식량)　10ed(저렴함)"),
    (
        RangeInc::new(56, 60),
        "살아있는 닭　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(61, 65),
        "살아있는 물고기　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(66, 70),
        "신선한 과일　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(71, 75),
        "신선한 채소　50ed(값이 나감(costly))",
    ),
    (RangeInc::new(76, 80), "뿌리채소　20ed(일상적임)"),
    (RangeInc::new(81, 85), "살아있는 돼지　100ed(프리미엄)"),
    (RangeInc::new(86, 90), "희귀한 과일　100ed(프리미엄)"),
    (RangeInc::new(91, 95), "희귀한 채소　100ed(프리미엄)"),
    (RangeInc::new(96, 99), "정확히 50eb의 길거리 약물"),
    (
        RangeInc::single(100),
        "통조림　10ed(저렴함) or 정확히 50eb의 길거리 약물",
    ),
];
static KO_NMCFO: RangeTable = RangeTable::from_dice("음식과 약물", 1, 100, KO_NMCFO_ITEMS);

static KO_NMCME_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(1, 5),
        "에이전트(휴대용 통신기)　100ed(프리미엄)",
    ),
    (RangeInc::new(6, 10), "100ed 이하의 프로그램 또는 하드웨어"),
    (RangeInc::new(11, 15), "오디오 레코더　100ed(프리미엄)"),
    (RangeInc::new(16, 20), "버그 디텍터　500ed(비쌈(expensive))"),
    (
        RangeInc::new(21, 25),
        "화학 분석기　1,000ed(매우 비쌈(very Expensive))",
    ),
    (RangeInc::new(26, 30), "컴퓨터　50ed(값이 나감(costly))"),
    (RangeInc::new(31, 35), "사이버덱　500ed(비쌈(expensive))"),
    (
        RangeInc::new(36, 40),
        "일회용 휴대폰　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(41, 45),
        "일렉기타 또는 기타 악기　500ed(비쌈(expensive))",
    ),
    (
        RangeInc::new(46, 50),
        "정확히 500ed의 프로그램 또는 하드웨어",
    ),
    (
        RangeInc::new(51, 55),
        "메드스캐너(의료 스캐너)　1,000ed(매우 비쌈(very Expensive))",
    ),
    (
        RangeInc::new(56, 60),
        "호밍 트레이서(위치 추적기)　500ed(비쌈(expensive))",
    ),
    (
        RangeInc::new(61, 65),
        "라디오 커뮤니케이터(무선 통신기)　100ed(프리미엄)",
    ),
    (
        RangeInc::new(66, 70),
        "테크스캐너ー　1,000ed(매우 비쌈(very Expensive))",
    ),
    (RangeInc::new(71, 75), "스마트 안경　500ed(비쌈(expensive))"),
    (
        RangeInc::new(76, 80),
        "레이더 감지기　500ed(비쌈(expensive))",
    ),
    (
        RangeInc::new(81, 85),
        "스크램블러／디스크램블러(통신 암호화 장비)　500ed(비쌈(expensive))",
    ),
    (
        RangeInc::new(86, 90),
        "라디오 스캐너／뮤직플레이어　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(91, 95),
        "브레인댄스 뷰어　1,000ed(매우 비쌈(very Expensive))",
    ),
    (
        RangeInc::new(96, 99),
        "가상 고글(virtuality goggles)　100ed(프리미엄)",
    ),
    (
        RangeInc::single(100),
        "에이전트　100ed(프리미엄) or 가상 고글(virtuality goggles)　100ed(프리미엄)",
    ),
];
static KO_NMCME: RangeTable = RangeTable::from_dice("개인용 전자기기", 1, 100, KO_NMCME_ITEMS);

static KO_NMCWE_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "미들 피스톨　50ed(값이 나감(costly))"),
    (
        RangeInc::new(6, 10),
        "헤비피스톨 or 베리 헤비 피스톨　100ed(프리미엄)",
    ),
    (RangeInc::new(11, 15), "SMG　100ed(프리미엄)"),
    (RangeInc::new(16, 20), "헤비 SMG　100ed(프리미엄)"),
    (RangeInc::new(21, 25), "샷건　500ed(비쌈(expensive))"),
    (
        RangeInc::new(26, 30),
        "어썰트 라이플(돌격 소총)　500ed(비쌈(expensive))",
    ),
    (
        RangeInc::new(31, 35),
        "스나이퍼 라이플(저격 총)　500ed(비쌈(expensive))",
    ),
    (RangeInc::new(36, 40), "활 또는 석궁　100ed(프리미엄)"),
    (
        RangeInc::new(41, 45),
        "유탄발사기(grenade launcher) 또는 로켓발사기(rocket launcher)　500ed(비쌈(expensive))",
    ),
    (RangeInc::new(46, 50), "500ed 이하의 탄약"),
    (
        RangeInc::new(51, 55),
        "GM이 선택한 하나의 이국적인 무기(exotic weapon)",
    ),
    (
        RangeInc::new(56, 60),
        "가벼운 근접 무기　50ed(값이 나감(costly))",
    ),
    (
        RangeInc::new(61, 65),
        "중간 근접 무기　50ed(값이 나감(costly))",
    ),
    (RangeInc::new(66, 70), "무거운 근접 무기　100ed(프리미엄)"),
    (
        RangeInc::new(71, 75),
        "매우 무거운 근접 무기　100ed(프리미엄)",
    ),
    (RangeInc::new(76, 80), "100ed 이하의 갑옷"),
    (RangeInc::new(81, 85), "정확히 500ed의 갑옷"),
    (RangeInc::new(86, 90), "정확히 1000ed의 갑옷"),
    (RangeInc::new(91, 95), "100ed 이하의 무기 부착물들"),
    (RangeInc::new(96, 99), "500ed 이상의 무기 부착물들"),
    (
        RangeInc::single(100),
        "미들 피스톨　50ed(값이 나감(costly)) or 500ed 이상의 무기 부착물들",
    ),
];
static KO_NMCWE: RangeTable = RangeTable::from_dice("무기와 갑옷", 1, 100, KO_NMCWE_ITEMS);

static KO_NMCCY_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "사이버아이　100ed(프리미엄)"),
    (
        RangeInc::new(6, 10),
        "사이버오디오 슈트(청각 기본 시스템)　500ed(비쌈(expensive))",
    ),
    (RangeInc::new(11, 15), "뉴럴링크　500ed(비쌈(expensive))"),
    (RangeInc::new(16, 20), "사이버암　500ed(비쌈(expensive))"),
    (RangeInc::new(21, 25), "사이버레그　100ed(프리미엄)"),
    (RangeInc::new(26, 30), "정확히 1000ed의 외부용 사이버웨어"),
    (RangeInc::new(31, 35), "500ed 이하의 외부용 사이버웨어"),
    (RangeInc::new(36, 40), "정확히 1000ed의 내부용 사이버웨어"),
    (RangeInc::new(41, 45), "500ed 이하의 내부용 사이버웨어"),
    (RangeInc::new(46, 50), "정확히 1000ed의 사이버아이 옵션"),
    (RangeInc::new(51, 55), "500ed 이하의 사이버아이 옵션"),
    (RangeInc::new(56, 60), "정확히 1000ed의 사이버오디오 옵션"),
    (RangeInc::new(61, 65), "500ed 이하의 사이버오디오 옵션"),
    (RangeInc::new(66, 70), "정확히 1000ed의 뉴럴웨어 옵션"),
    (RangeInc::new(71, 75), "500ed 이하의 뉴럴웨어 옵션"),
    (
        RangeInc::new(76, 80),
        "정확히 1000ed의 사이버림 옵션(사이버암, 사이버레그)",
    ),
    (
        RangeInc::new(81, 85),
        "500ed 이하의 사이버림 옵션(사이버암, 사이버레그)",
    ),
    (RangeInc::new(86, 90), "GM이 선택한 패션웨어"),
    (RangeInc::new(91, 95), "GM이 선택한 보그웨어"),
    (RangeInc::new(96, 99), "GM이 선택한 사이버웨어(무엇이든)"),
    (
        RangeInc::single(100),
        "사이버아이　100ed(프리미엄) or GM이 선택한 사이버웨어(무엇이든)",
    ),
];
static KO_NMCCY: RangeTable = RangeTable::from_dice("사이버웨어", 1, 100, KO_NMCCY_ITEMS);

static KO_NMCFA_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "백 레이디 시크"),
    (RangeInc::new(6, 10), "갱 컬러"),
    (RangeInc::new(11, 15), "제네릭 시크"),
    (RangeInc::new(16, 20), "보헤미안"),
    (RangeInc::new(21, 25), "레저 웨어"),
    (RangeInc::new(26, 30), "노마드 레더"),
    (RangeInc::new(31, 35), "아시아 팝"),
    (RangeInc::new(36, 40), "어반 플래쉬"),
    (RangeInc::new(41, 45), "비지니스 웨어"),
    (RangeInc::new(46, 50), "하이패션"),
    (RangeInc::new(51, 55), "바이오모니터　100ed(프리미엄)"),
    (
        RangeInc::new(56, 60),
        "춤스킨(chemskin - 피부 색갈 변경)　100ed(프리미엄)",
    ),
    (RangeInc::new(61, 65), "EMP쓰레딩　10ed(저렴함)"),
    (RangeInc::new(66, 70), "라이트 타투　100ed(프리미엄)"),
    (
        RangeInc::new(71, 75),
        "시프트 텍트(눈색상 변화 렌즈)　100ed(프리미엄)",
    ),
    (RangeInc::new(76, 80), "스킨워치　100ed(프리미엄)"),
    (RangeInc::new(81, 85), "테크헤어　100ed(프리미엄)"),
    (RangeInc::new(86, 90), "제네릭 시크"),
    (RangeInc::new(91, 95), "레저 웨어"),
    (RangeInc::new(96, 99), "갱 컬러"),
    (RangeInc::single(100), "백 레이디 시크 or 갱 컬러"),
];
static KO_NMCFA: RangeTable = RangeTable::from_dice("의류 및 패션웨어", 1, 100, KO_NMCFA_ITEMS);

static KO_NMCSU_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 5), "스모그 방지 마스크　20ed(일상적임)"),
    (
        RangeInc::new(6, 10),
        "자동 청각 보호기(Auto Level Dampening Ear Protectors)　1,000ed(매우 비쌈(very Expensive))",
    ),
    (RangeInc::new(11, 15), "쌍안경　50ed(값이 나감(costly))"),
    (
        RangeInc::new(16, 20),
        "여행용 대형 가방(carryall)　20ed(일상적임)",
    ),
    (RangeInc::new(21, 25), "손전등　20ed(일상적임)"),
    (RangeInc::new(26, 30), "덕트 테이프　20ed(일상적임)"),
    (RangeInc::new(31, 35), "팽창식 침대 & 침낭　20ed(일상적임)"),
    (RangeInc::new(36, 40), "자물쇠열기 세트　20ed(일상적임)"),
    (RangeInc::new(41, 45), "수갑　50ed(값이 나감(costly))"),
    (RangeInc::new(46, 50), "메드테키 가방　100ed(프리미엄)"),
    (
        RangeInc::new(51, 55),
        "텐트 & 캠핑용품　50ed(값이 나감(costly))",
    ),
    (RangeInc::new(56, 60), "로프(60m)　20ed(일상적임)"),
    (
        RangeInc::new(61, 65),
        "테크 도구(techtool)　100ed(프리미엄)",
    ),
    (
        RangeInc::new(66, 70),
        "개인 치유 장비(personal CarePak　20ed(일상적임)",
    ),
    (
        RangeInc::new(71, 75),
        "방사능 방호복(Radiation Suit)　1,000ed(매우 비쌈(very Expensive))",
    ),
    (
        RangeInc::new(76, 80),
        "로드플레어(road flare)　10ed(저렴함)",
    ),
    (RangeInc::new(81, 85), "그래플 건　100ed(프리미엄)"),
    (RangeInc::new(86, 90), "테크 백　500ed(비쌈(expensive))"),
    (
        RangeInc::new(91, 95),
        "삽 또는 도끼　50ed(값이 나감(costly))",
    ),
    (RangeInc::new(96, 99), "에어하이포　50ed(값이 나감(costly))"),
    (
        RangeInc::single(100),
        "스모그 방지 마스크　20ed(일상적임) or 에어하이포　50ed(값이 나감(costly))",
    ),
];
static KO_NMCSU: RangeTable =
    RangeTable::from_dice("생존 장비(servival gear)", 1, 100, KO_NMCSU_ITEMS);

static KO_SCST_ITEMS: &[&str] = &["국제", "전국", "주(state)", "지역", "경제", "가십"];
static KO_SCST: Table = Table::from_dice("스크림시트 카테고리", 1, 6, KO_SCST_ITEMS);

static KO_SCSA_ITEMS: &[&str] = &[
    "(기업 1개 선택)",
    "상원의원/의원",
    "대통령/회장/사장",
    "기업/기업들",
    "시의회",
    "사이버사이코",
    "킬러/살인마",
    "슬레이어",
    "비극",
    "수사관",
];
static KO_SCSA: Table = Table::from_dice("헤드라인A", 1, 10, KO_SCSA_ITEMS);

static KO_SCSB_ITEMS: &[&str] = &[
    "법인을",
    "도시를",
    "타협을",
    "경고를",
    "계획을",
    "스캔들을",
    "여성을",
    "남성을",
    "사고를",
    "희망을",
];
static KO_SCSB: Table = Table::from_dice("헤드라인B", 1, 10, KO_SCSB_ITEMS);

static KO_SCSC_ITEMS: &[&str] = &[
    " 제안 / 제공",
    " 위협",
    " 타협",
    " 살해(murders)",
    " 살해 당하다(killed)",
    " 죽다(dies)",
    " 칭찬",
    " 발표",
    " 공개",
    " 지속",
];
static KO_SCSC: Table = Table::from_dice("헤드라인C", 1, 10, KO_SCSC_ITEMS);

static KO_SCSOF_ITEMS: &[&str] = &[
    "[을/를] ",
    "[이/가] ",
    "[에 대해] ",
    "[와 함꼐] ",
    "[보다 더] ",
    "[을 향해] ",
];
static KO_SCSOF: Table = Table::from_dice("헤드라인 조사(을,를,이,가)", 1, 6, KO_SCSOF_ITEMS);

static KO_VMCT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 3), "음식"),
    (RangeInc::new(4, 5), "개인 용품"),
    (RangeInc::single(6), "이상한 것"),
];
static KO_VMCT: RangeTable = RangeTable::from_dice("가장 가까운 자판기 유형", 1, 6, KO_VMCT_ITEMS);

static KO_VMCE_ITEMS: &[&str] = &[
    "라멘",
    "피자",
    "햄버거",
    "스매시",
    "스시",
    "따뜻한 고기",
    "키블 1kg 가방",
    "캔에 들어있는 커피",
    "캔에 들어있는 쥬스",
    "캔에 들어있는 탄산음료",
];
static KO_VMCE: Table = Table::from_dice("음식", 1, 10, KO_VMCE_ITEMS);

static KO_VMCF_ITEMS: &[&str] = &[
    "캔에 들어있는 티셔츠",
    "성인용품",
    "우산",
    "넥타이",
    "수술용 마스크",
    "속옷",
    "꽃다발",
    "총과 총알",
    "브레인댄스 칩",
    "비디오 게임",
];
static KO_VMCF: Table = Table::from_dice("개인 용품", 1, 10, KO_VMCF_ITEMS);

static KO_VMCS_ITEMS: &[&str] = &[
    "애완동물 가발",
    "버그 스낵 (곤충 요리)",
    "양상추 1알",
    "날치 스프",
    "인공 배양 해산물",
    "냄새 통조림",
    "살아있는 장수풍뎅이",
    "종이 코스프레 의상",
    "이상한 캡슐 장난감",
    "사용한 팬티",
];
static KO_VMCS: Table = Table::from_dice("이상한 것", 1, 10, KO_VMCS_ITEMS);

static KO_STOREA_ITEMS: &[&str] = &[
    "모성 넘치는 노파. 손님을 친자식처럼 대하고, 잔소리를 합니다.",
    "모든 사람에게 화난 늙은이.  특히 당신을 증오합니다.",
    "지루한 yo-ganger. 그는 부모에 의해 계산대에 묶여있습니다.",
    "지나치게 친한척하고 당신에게 접근하는 성가신 사람.",
    "뻔뻔한 마약 중독자.  너에게 손을 흔들며 「이제 뭐든 상관없어, 친구....」라고 말합니다.",
    "이해할 수 없는 중년. 어쨌든 그는 당신과 논쟁할 것 같습니다.",
];
static KO_STOREA: Table = Table::from_dice("점주 또는 종업원", 1, 6, KO_STOREA_ITEMS);

static KO_STOREB_ITEMS: &[&str] = &[
    "계속 중얼거리다가 갑자기 멈추고 당신을 응시하는 손님.",
    "시끄럽고 성가신 술취한 손님.",
    "그에게만 보이는 무언가를 향해 허공에 손짓하는 약물에 취한 손님",
    "돈이 부족하지만 스매시를 사주면 부탁을 하나 들어줄 것 같은 조이걸/보이.",
    "근무중이 아닌 경찰관. 빨리 먹을 수 있는 것을 찾고 있지만 수다를 할 분위기는 아님.",
    "길거리에서 습격당할까봐 불안에 떠는 손님. 누군가가 다가간다면 달려들것 같습니다 (총을 뽑을지도 모릅니다).",
];
static KO_STOREB: Table = Table::from_dice("별난 손님 1", 1, 6, KO_STOREB_ITEMS);

static KO_STOREC_ITEMS: &[&str] = &[
    "yo-ganger는 이 가게를 살펴보며 털 준비를 하고 있습니다.",
    "1d6 턴 후에 이 가게를 칠 ganger.",
    "가게 주인을 다치게 하고 '보호비'를 챙기려는 삼류 폭력배.",
    "사랑스러운 네 살짜리 길 잃은 아이. 엄마를 찾아 헤맵니다.",
    "가게에 들어오자 마자 큰소리로 싸우던 커플. 점점 시끄러워집니다.",
    "고급 파티를 즐긴 후 술을 사려고 들어온 비싼 옷을 입은 부유한 커플.",
];
static KO_STOREC: Table = Table::from_dice("별난 손님 2", 1, 6, KO_STOREC_ITEMS);
/// Ruby `"VMCR" => DiceTable::ChainTable.new(VendingMachineTable.name, "1D6", [...])`。
static KO_VMCR_ITEMS: &[TableItem] = &[
    TableItem::Table(&KO_VMCE),
    TableItem::Table(&KO_VMCE),
    TableItem::Table(&KO_VMCE),
    TableItem::Table(&KO_VMCF),
    TableItem::Table(&KO_VMCF),
    TableItem::Table(&KO_VMCS),
];
static KO_VMCR: ChainTable =
    ChainTable::from_dice("가장 가까운 자판기 테이블", 1, 6, KO_VMCR_ITEMS);

/// Ruby `TABLES`（`translate_tables(:ko_kr)`）。
static KO_TABLES: &[(&str, TableRef)] = &[
    ("FFD", TableRef::Plain(&KO_FFD)),
    ("HFD", TableRef::Plain(&KO_HFD)),
    ("NCDT", TableRef::Range(&KO_NCDT)),
    ("NCMT", TableRef::Range(&KO_NCMT)),
    ("NMCT", TableRef::Plain(&KO_NMCT)),
    ("NMCFO", TableRef::Range(&KO_NMCFO)),
    ("NMCME", TableRef::Range(&KO_NMCME)),
    ("NMCWE", TableRef::Range(&KO_NMCWE)),
    ("NMCCY", TableRef::Range(&KO_NMCCY)),
    ("NMCFA", TableRef::Range(&KO_NMCFA)),
    ("NMCSU", TableRef::Range(&KO_NMCSU)),
    ("SCST", TableRef::Plain(&KO_SCST)),
    ("SCSA", TableRef::Plain(&KO_SCSA)),
    ("SCSB", TableRef::Plain(&KO_SCSB)),
    ("SCSC", TableRef::Plain(&KO_SCSC)),
    ("SCSR", TableRef::ScreamSheet),
    ("VMCT", TableRef::Range(&KO_VMCT)),
    ("VMCE", TableRef::Plain(&KO_VMCE)),
    ("VMCF", TableRef::Plain(&KO_VMCF)),
    ("VMCS", TableRef::Plain(&KO_VMCS)),
    ("VMCR", TableRef::Chain(&KO_VMCR)),
    ("STOREA", TableRef::Plain(&KO_STOREA)),
    ("STOREB", TableRef::Plain(&KO_STOREB)),
    ("STOREC", TableRef::Plain(&KO_STOREC)),
    ("STORE", TableRef::ShopPeople),
];

static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    critical: "크리티컬！",
    fumble: "펌블！",
    success: "성공",
    failure: "실패",
    news: "뉴스",
    scream_sheet: ScreamSheetTables {
        type_table: &KO_SCST,
        a_table: &KO_SCSA,
        of_table: &KO_SCSOF,
        b_table: &KO_SCSB,
        c_table: &KO_SCSC,
    },
    shop_people: ShopPeopleTables {
        staff_table: &KO_STOREA,
        people_a_table: &KO_STOREB,
        people_b_table: &KO_STOREC,
        intro: "당신이 들른 상점(보데가)에는 ―― ",
        shop_staff: " ―― 와 같은 점원과, ",
        people_a: " ―― 라는 인상을 가지고 있는 손님과, ",
        people_b: " ―― 라는 느낌의 손님이 있어 보입니다. ",
        outro: "정말 기분 나쁜 예감이 듭니다.",
    },
};

/// Ruby `BCDice::GameSystem::CyberpunkRed_Korean`（ID: `CyberpunkRed:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CyberpunkRed_Korean;

impl GameSystem for CyberpunkRed_Korean {
    fn id(&self) -> &'static str {
        "CyberpunkRed:Korean"
    }

    fn name(&self) -> &'static str {
        "사이버펑크 RED"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:사이버펑크"
    }

    fn help_message(&self) -> &'static str {
        r"・판정　CPx+y>z
  　(x＝능력치와 기능치의 합(base)、y＝수정치(mod)、z＝난이도(DV) or 방어자의 값　x、y、z는 생략 가능)
  　예시）CP12, CP10+2>12,　CP7-1,　CP8+4,　CP7>12,　CP,　CP>9

  각종 표
  ・치명적인 손상표
  　FFD　：신체에 치명적 손상
  　HFD　：머리에 치명적 손상
  ・조우 표
  　NCDT　：나이트시티(낮)
  　NCMT　：나이트 시티(심야)
  ・스크림 시트(신문)
  　SCSR　：스크림 시트(랜덤)
  　SCST　：스크림 시트 카테고리
  　SCSA　：헤드 라인A
  　SCSB　：헤드 라인B
  　SCSC　：헤드 라인C
  ・가장 가까운 자판기
  　VMCR　：가장 가까운 자판기표
  　VMCT　：자판기 유형 결정표
  　VMCE　：식품
  　VMCF　：패션
  　VMCS　：이상한 물건
  ・보데가(상점) 손님
  　STORE　：상점 손님과 점원
  　STOREA　：점주 또는 계산원
  　STOREB　：별난 손님 1
  　STOREC　：별난 손님 2
  ・야시장
  　NMCT　：상품의 분야
  　NMCFO　：음식과 약물
  　NMCME　：개인용 전자기기
  　NMCWE　：무기와 갑옷
  　NMCCY　：사이버웨어
  　NMCFA　：의류 및 패션웨어
  　NMCSU　：생존 장비(servival gear)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "CP", "FFD", "HFD", "NCDT", "NCMT", "NMCT", "NMCFO", "NMCME", "NMCWE", "NMCCY",
            "NMCFA", "NMCSU", "SCST", "SCSA", "SCSB", "SCSC", "SCSR", "VMCT", "VMCE", "VMCF",
            "VMCS", "VMCR", "STOREA", "STOREB", "STOREC", "STORE",
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

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "CyberpunkRed:Korean",
            "CyberpunkRed_Korean.toml",
            56,
        );
    }
}
