//! P4で手書き移植した `lib/bcdice/game_system/Kamigakari_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Kamigakari` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! 表の引き方・コマンド解釈は [`super::Kamigakari`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データと定型文だけを置く。

use super::Kamigakari::{eval_specific_command, SystemTables};
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_RT_ITEMS: &[&str] = &[
    "사신화：물리법칙을 너무 초월한 대가로, 영혼이 왜곡되어, PC는 즉시 아라미타마로 변모한다. 아라미타마화한 PC는, 어딘가로 떠나간다.",
    "존재소멸：아라미타마화를 최후의 힘으로 억누른다. 하지만 그 결과, PC의 영혼은 불타버려, 이 세계에서 소멸한다. 그 PC는 [상태변화: 사망]이 되어, 시체도 남지 않는다.",
    "사망：영혼의 왜곡을 어떻게든 막지만, 영혼이 붕괴한다. PC는 [상태변화: 사망]이 되나 유체는 남는다.",
    "영혼반괴：영혼의 왜곡을 막아냈지만, 영혼 자체에 치명적인 부상을 입어, 전신에 장해가 남는다. 거기에 동반하여 영문도 소실해, 일반인으로 돌아간다.",
    "기억소멸：기적적으로, 영혼의 마모에 의한 신체적 악영향을 피한다. 시간을 두는 것으로 영문도 회복되나, 정신적인 영향을 받아, 모든 기억을 잃는다.",
    "영향없음：기적적으로, 영혼의 마모에 의한 악영향을 완전히 피하고, 또한 영문의 회복도 빠를 것으로 보인다. 육체나 정신 모두, 딱히 영향은 없다.",
];
static KO_RT: Table = Table::from_dice("영문소비의 댓가표", 1, 6, KO_RT_ITEMS);

static KO_ET_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("운명/그 캐릭터에게, 운명적, 또는 숙명적인 것을 느끼고 있다.")),
    (12, TableItem::Text("운명/그 캐릭터에게, 운명적, 또는 숙명적인 것을 느끼고 있다.")),
    (13, TableItem::Text("가족/그 캐릭터에게, 가족같은 친근감을 품고 있다.")),
    (14, TableItem::Text("가족/그 캐릭터에게, 가족같은 친근감을 품고 있다.")),
    (15, TableItem::Text("악연/그 캐릭터에게, 악연을 느끼고 있다.")),
    (16, TableItem::Text("악연/그 캐릭터에게, 악연을 느끼고 있다.")),
    (21, TableItem::Text("사제/그 캐릭터와는, 마치 사제지간의 감정을 느끼고 있다. 누가 제자고 누가 스승인지, 플레이어 끼리(또는 GM과)상담해 결정한다.")),
    (22, TableItem::Text("사제/그 캐릭터와는, 마치 사제지간의 감정을 느끼고 있다. 누가 제자고 누가 스승인지, 플레이어 끼리(또는 GM과)상담해 결정한다.")),
    (23, TableItem::Text("호적수/그 캐릭터를, 호적수라 여기고 있다.")),
    (24, TableItem::Text("호적수/그 캐릭터를, 호적수라 여기고 있다.")),
    (25, TableItem::Text("친근감/그 캐릭터에게, 친근감을 품고 있다.")),
    (26, TableItem::Text("친근감/그 캐릭터에게, 친근감을 품고 있다.")),
    (31, TableItem::Text("성의/그 캐릭터에게, 성실함을 느끼고 있다.")),
    (32, TableItem::Text("성의/그 캐릭터에게, 성실함을 느끼고 있다.")),
    (33, TableItem::Text("우정/그 캐릭터에게, 우정을 품고 있다.")),
    (34, TableItem::Text("우정/그 캐릭터에게, 우정을 품고 있다.")),
    (35, TableItem::Text("존경/그 캐릭터에게, 존경을 품고 있다.")),
    (36, TableItem::Text("존경/그 캐릭터에게, 존경을 품고 있다.")),
    (41, TableItem::Text("비호/그 캐릭터에게, 비호의 감정을 품고 있다. 누가 보호자고 누가 피보호자인지, 플레이어 끼리(또는 GM과)상담해 결정한다.")),
    (42, TableItem::Text("비호/그 캐릭터에게, 비호의 감정을 품고 있다. 누가 보호자고 누가 피보호자인지, 플레이어 끼리(또는 GM과)상담해 결정한다.")),
    (43, TableItem::Text("호감/그 캐릭터에게, 호감을 품고 있다.")),
    (44, TableItem::Text("호감/그 캐릭터에게, 호감을 품고 있다.")),
    (45, TableItem::Text("흥미/그 캐릭터에게, 흥미를 품고 있다.")),
    (46, TableItem::Text("흥미/그 캐릭터에게, 흥미를 품고 있다.")),
    (51, TableItem::Text("감명/그 캐릭터에게, 감명을 느끼고 있다.")),
    (52, TableItem::Text("감명/그 캐릭터에게, 감명을 느끼고 있다.")),
    (53, TableItem::Text("외경/그 캐릭터를 두려워하고 있다.")),
    (54, TableItem::Text("외경/그 캐릭터를 두려워하고 있다.")),
    (55, TableItem::Text("마음에 듬/그 캐릭터를 마음에 들어한다.")),
    (56, TableItem::Text("마음에 듬/그 캐릭터를 마음에 들어한다.")),
    (61, TableItem::Text("애정/그 캐릭터에게 애정, 또는 거기에 가까운 집착심을 품고 있다.")),
    (62, TableItem::Text("애정/그 캐릭터에게 애정, 또는 거기에 가까운 집착심을 품고 있다.")),
    (63, TableItem::Text("신뢰/그 캐릭터에게 신뢰를 느끼고 있다.")),
    (64, TableItem::Text("신뢰/그 캐릭터에게 신뢰를 느끼고 있다.")),
    (65, TableItem::Text("＊PC의 임의/플레이어, 또는 GM이 설정한 임의의 감정을 품고 있다.")),
    (66, TableItem::Text("＊PC의 임의/플레이어, 또는 GM이 설정한 임의의 감정을 품고 있다.")),
];
static KO_ET: D66Table = D66Table::new("감정표", D66SortType::NoSort, KO_ET_ITEMS);

static KO_KT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("시공의 비틀림\n현재 위치의 시공이 비틀려, PC전원은 즉시 [침입 에리어]로 돌아간다.")),
    (12, TableItem::Text("시공의 비틀림\n현재 위치의 시공이 비틀려, PC전원은 즉시 [침입 에리어]로 돌아간다.")),
    (13, TableItem::Text("강적등장\n갑자기, 〈재앙신〉화한 [모노노케]가 출현한다. GM은 PC의 [세계간섭LV]의 평균+3의 [LV]을 가진 임의의 [모노노케]를 1체 골라, 임의의 [탐색 에리어]에 배치. 거기서는 [우회]불가로 [전투]가 발생한다.")),
    (14, TableItem::Text("강적등장\n갑자기, 〈재앙신〉화한 [모노노케]가 출현한다. GM은 PC의 [세계간섭LV]의 평균+3의 [LV]을 가진 임의의 [모노노케]를 1체 골라, 임의의 [탐색 에리어]에 배치. 거기서는 [우회]불가로 [전투]가 발생한다.")),
    (15, TableItem::Text("그림자의 손\n장기로 형성된 무수한 손이, PC들을 붙잡으려고 한다. PC전원은 [효과종류: 마법공격/거리: 전투지대/대상: 전투지대/달성치: 20+PC의 [세계간섭LV]의 평균/마법 데미지: 20×PC의 [세계간섭LV]의 평균/저항[반감]]을 받는다.")),
    (16, TableItem::Text("그림자의 손\n장기로 형성된 무수한 손이, PC들을 붙잡으려고 한다. PC전원은 [효과종류: 마법공격/거리: 전투지대/대상: 전투지대/달성치: 20+PC의 [세계간섭LV]의 평균/마법 데미지: 20×PC의 [세계간섭LV]의 평균/저항[반감]]을 받는다.")),
    (21, TableItem::Text("무수한 마안\n공간 전체게 무시무시한 마안이 출현한다. PC전원은 [대휴식]할 때까지 [상태변화: 암흑·고통]이 된다.")),
    (22, TableItem::Text("무수한 마안\n공간 전체게 무시무시한 마안이 출현한다. PC전원은 [대휴식]할 때까지 [상태변화: 암흑·고통]이 된다.")),
    (23, TableItem::Text("공간붕괴\n갑자기, 마경의 공간이 붕괴한다. PC전원은 [효과종류: 물리공격/거리: 전투지대/대상: 전투지대/달성치: 30+PC의 [세계간섭LV]의 평균/물리 데미지: 30×PC의 [세계간섭LV]의 평균]을 받는다.")),
    (24, TableItem::Text("공간붕괴\n갑자기, 마경의 공간이 붕괴한다. PC전원은 [효과종류: 물리공격/거리: 전투지대/대상: 전투지대/달성치: 30+PC의 [세계간섭LV]의 평균/물리 데미지: 30×PC의 [세계간섭LV]의 평균]을 받는다.")),
    (25, TableItem::Text("방어구 부식\n이질적인 안개가 나타나, 방어구를 부식시킨다. PC전원은, [소지·장비]중인 임의의 [아이템: 방어구]를 1개 잃는다.")),
    (26, TableItem::Text("방어구 부식\n이질적인 안개가 나타나, 방어구를 부식시킨다. PC전원은, [소지·장비]중인 임의의 [아이템: 방어구]를 1개 잃는다.")),
    (31, TableItem::Text("소재소실\n주변에서 수상한 빛이 떨어져, 소지 중인 [소재]를 소실시킨다. PC 전원이 [소지]중인 [소재]가 전부 소멸한다.")),
    (32, TableItem::Text("소재소실\n주변에서 수상한 빛이 떨어져, 소지 중인 [소재]를 소실시킨다. PC 전원이 [소지]중인 [소재]가 전부 소멸한다.")),
    (33, TableItem::Text("없음\n딱히 아무 일도 일어나지 않는다.")),
    (34, TableItem::Text("없음\n딱히 아무 일도 일어나지 않는다.")),
    (35, TableItem::Text("모노노케 강습\n갑자기, 〈재앙신〉화한 [모노노케]가 출현해, PC들을 덮친다. GM은 PC의 [세계간섭LV]의 평균+2의 [LV]을 가진 임의의 [모노노케]를 2체 골라, PC의 앞에 출현시키고, 즉시 [전투]를 개시한다.")),
    (36, TableItem::Text("모노노케 강습\n갑자기, 〈재앙신〉화한 [모노노케]가 출현해, PC들을 덮친다. GM은 PC의 [세계간섭LV]의 평균+2의 [LV]을 가진 임의의 [모노노케]를 2체 골라, PC의 앞에 출현시키고, 즉시 [전투]를 개시한다.")),
    (41, TableItem::Text("휴식방해\nPC가 휴식하려고 할 때마다, 다양한 공간에서 촉수나 독충 등이 출현해 덮쳐든다. PC들은 이후, [마경토벌]이 종료될 때까지 [대휴식]을 할 수 없다.")),
    (42, TableItem::Text("휴식방해\nPC가 휴식하려고 할 때마다, 다양한 공간에서 촉수나 독충 등이 출현해 덮쳐든다. PC들은 이후, [마경토벌]이 종료될 때까지 [대휴식]을 할 수 없다.")),
    (43, TableItem::Text("용맥파괴\n영력이 폭주해 공간이 일그러져, [영력]이 어그러진다. PC전원은 즉시 [영력]을 모두 다시 굴린다.")),
    (44, TableItem::Text("용맥파괴\n영력이 폭주해 공간이 일그러져, [영력]이 어그러진다. PC전원은 즉시 [영력]을 모두 다시 굴린다.")),
    (45, TableItem::Text("고유시간정지\nPC들의 육체의 일부가 잿빛으로 변하고, 움직일 수 없게 된다. PC 전원은, [타이밍: 준비·방어·특수]에서 1개 골라, 이후 그 [타이밍]을 소비할 수 없게 된다.")),
    (46, TableItem::Text("고유시간정지\nPC들의 육체의 일부가 잿빛으로 변하고, 움직일 수 없게 된다. PC 전원은, [타이밍: 준비·방어·특수]에서 1개 골라, 이후 그 [타이밍]을 소비할 수 없게 된다.")),
    (51, TableItem::Text("용맥불순\n영력이 갑자기 고갈되어, [영력]의 순환에 악영향이 발생한다. PC전원은 이후, [마경토벌]이 종료될 때까지 [영력조작]을 할 수 없다.")),
    (52, TableItem::Text("용맥불순\n영력이 갑자기 고갈되어, [영력]의 순환에 악영향이 발생한다. PC전원은 이후, [마경토벌]이 종료될 때까지 [영력조작]을 할 수 없다.")),
    (53, TableItem::Text("술식봉인\n주변의 공기가 변모해, 악영향이 일어난다. PC전원은 이후, 취득한 《탤런트》중, 사용하는 [코스트]가 가장 큰 것 1개가 [마경토벌]종료까지 사용불능이 된다. [코스트: 없음]뿐일 경우, GM이 임의로 1개 결정한다.")),
    (54, TableItem::Text("술식봉인\n주변의 공기가 변모해, 악영향이 일어난다. PC전원은 이후, 취득한 《탤런트》중, 사용하는 [코스트]가 가장 큰 것 1개가 [마경토벌]종료까지 사용불능이 된다. [코스트: 없음]뿐일 경우, GM이 임의로 1개 결정한다.")),
    (55, TableItem::Text("장식품 소멸\n주변이 푸른 빛으로 감싸이고, 어째선지 PC들의 장식품이 사라진다. PC전원은, [소지·장비]중인 [아이템: 장식]을 모두 잃는다.。")),
    (56, TableItem::Text("장식품 소멸\n주변이 푸른 빛으로 감싸이고, 어째선지 PC들의 장식품이 사라진다. PC전원은, [소지·장비]중인 [아이템: 장식]을 모두 잃는다.")),
    (61, TableItem::Text("우자의 황금 소실\n주변이 붉은 빛으로 감싸이고, 어째선지 PC들의 [G]가 사라진다. PC 전원은, [소지금]이 [반감]한다.")),
    (62, TableItem::Text("우자의 황금 소실\n주변이 붉은 빛으로 감싸이고, 어째선지 PC들의 [G]가 사라진다. PC 전원은, [소지금]이 [반감]한다.")),
    (63, TableItem::Text("GM의 임의\n이 표 중에서 GM이 효과를 1개 골라 발생시킨다.")),
    (64, TableItem::Text("GM의 임의\n이 표 중에서 GM이 효과를 1개 골라 발생시킨다.")),
    (65, TableItem::Text("임계중복\n[마경임계]가 2번 발생한다. GM은 이 표를 2번 굴려, 효과를 각각 적용할 수 있다. 다시 「임계중복」이 발생한 경우, [GM의 임의] 1번으로 취급한다.")),
    (66, TableItem::Text("임계중복\n[마경임계]가 2번 발생한다. GM은 이 표를 2번 굴려, 효과를 각각 적용할 수 있다. 다시 「임계중복」이 발생한 경우, [GM의 임의] 1번으로 취급한다.")),
];
static KO_KT: D66Table = D66Table::new("마경임계표", D66SortType::NoSort, KO_KT_ITEMS);

static KO_NT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("미츠루기(御剣)　리쿠/린")),
    (12, TableItem::Text("시시우치(獅子内)　야마토/카에데")),
    (13, TableItem::Text("하쿠긴(白銀)　하야토/사쿠라")),
    (14, TableItem::Text("타케노우치(竹内)　마코토/하루카")),
    (15, TableItem::Text("코다치(古太刀)　다이치/미사")),
    (16, TableItem::Text("쿠가(空閑)　슌/마오")),
    (21, TableItem::Text("오니가타(鬼形)　료/마이")),
    (22, TableItem::Text("미칸나기(御巫)　타쿠미/나나미")),
    (23, TableItem::Text("고마도우(護摩堂)　히토시/치히로")),
    (24, TableItem::Text("류엔(龍円)　타쿠마/아카네")),
    (25, TableItem::Text("카가미베(鏡部)　쿄우/아스카")),
    (26, TableItem::Text("이누가미(犬神)　코우/시오리")),
    (31, TableItem::Text("메이게츠인(明月院)　아오이/유이")),
    (32, TableItem::Text("도우메키(百目鬼)　렌야/모에")),
    (33, TableItem::Text("오소가미(怨神)　타츠야/아야카")),
    (34, TableItem::Text("아라라기(蘭)　류노스케/아즈사")),
    (35, TableItem::Text("타마키(珠輝)　아키라/히토미")),
    (36, TableItem::Text("간류(眼龍)　케이/사오리")),
    (41, TableItem::Text("텟포즈카(鉄砲塚)　마사토/사라")),
    (42, TableItem::Text("오리가미(檻神)　나오야/야요이")),
    (43, TableItem::Text("후지와라(不死原)　쥰/치아키")),
    (44, TableItem::Text("쿠로우자(九朗座)　무사시/하루나")),
    (45, TableItem::Text("츠치미카도(土御門)　쿄스케/스이")),
    (46, TableItem::Text("이자요이(十六夜)　케이지/후타바")),
    (51, TableItem::Text("텐포우린(転法輪)　히로/레나")),
    (52, TableItem::Text("시교우(執行)　히비키/사유리")),
    (53, TableItem::Text("호우리(祝)　료타로/히나")),
    (54, TableItem::Text("코우소(神尊)　토모/시온")),
    (55, TableItem::Text("아시야(芦屋)　타카유키/카스미")),
    (56, TableItem::Text("나나샤(七社)　카즈키/후카")),
    (61, TableItem::Text("키바(騎馬)　테츠야/시노")),
    (62, TableItem::Text("토우마(当麻)　켄/사야")),
    (63, TableItem::Text("키츠네즈카(狐塚)　호쿠토/마야")),
    (64, TableItem::Text("텐진바야시(天神林)　소라/아키라")),
    (65, TableItem::Text("메아라시(明嵐)　야쿠모/오토하")),
    (66, TableItem::Text("쿠사카베(草壁)　다이고/아야")),
];
static KO_NT: D66Table = D66Table::new("전기 성씨・이름 결정표", D66SortType::NoSort, KO_NT_ITEMS);

static KO_MT_ITEMS: &[&str] = &[
    "진홍의 단편",
    "까끌까끌한 단편",
    "비취의 단편",
    "예리한 단편",
    "황금의 단편",
    "말랑말랑한 단편",
    "은색의 단편",
    "뾰족한 단편",
    "순백의 단편",
    "딱딱한 단편",
    "칠흑의 단편",
    "빛나는 단편",
    "매끄러운 단편",
    "탁한 단편",
    "덥수룩한 단편",
    "사악한 단편",
    "끈적이는 단편",
    "성스러운 단편",
    "작열의 단편",
    "불꽃의 단편",
    "빙결의 단편",
    "얼음의 단편",
    "뜨거운 단편",
    "바람의 단편",
    "차가운 단편",
    "번개의 단편",
    "흙의 단편",
    "환상의 단편",
    "뼈형태의 단편",
    "봉인의 단편",
    "이빨형태의 단편",
    "비늘형태의 단편",
    "돌형태의 단편",
    "보석형태의 단편",
    "모피형태의 단편",
    "깃털형태의 단편",
];

static KO_COMMON_ITEMS: &[(i64, &str)] = &[
    (13, "체력+n"),
    (16, "민첩+n"),
    (23, "지성+n"),
    (26, "정신+n"),
    (33, "행운+n"),
    (35, "물D+n"),
    (41, "마D+n"),
    (43, "행동+n"),
    (46, "생명+n×3"),
    (53, "장갑+n"),
    (56, "결계+n"),
    (63, "이동+n마스"),
    (66, "※PC의 임의"),
];

static KO_ATTRIBUTE: &[(i64, &str)] = &[
    (21, "［화염］"),
    (33, "［냉기］"),
    (43, "［전격］"),
    (53, "［풍압］"),
    (56, "［환각］"),
    (62, "［마독］"),
    (64, "［자력］"),
    (66, "［섬광］"),
];

/// Ruby `TABLES`（`translate_tables(:ko_kr)`）。
static KO_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("RT", &KO_RT),
    ("ET", &KO_ET),
    ("KT", &KO_KT),
    ("NT", &KO_NT),
];

/// i18n `ko_kr` の表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    mt_name: "획득 소재 차트",
    mt_items: KO_MT_ITEMS,
    result_format: "%{material}.%{effect}",
    common_name: "자주 발견되는 소재",
    common_items: KO_COMMON_ITEMS,
    rare_name: "드문 소재",
    give_attribute: "부여",
    halve_damage: "반감",
    optional_by_gm: "※GM의 임의",
    attribute: KO_ATTRIBUTE,
    effect_power: "효과치",
};

/// Ruby `BCDice::GameSystem::Kamigakari_Korean`（ID: `Kamigakari:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kamigakari_Korean;

impl GameSystem for Kamigakari_Korean {
    fn id(&self) -> &'static str {
        "Kamigakari:Korean"
    }

    fn name(&self) -> &'static str {
        "카미가카리"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:카미가카리"
    }

    fn help_message(&self) -> &'static str {
        r"・각종표
 ・감정표(ET)
 ・영문소비의 댓가표(RT)
 ・전기 성씨・이름 결정표(NT)
 ・마경임계표(KT)
 ・획득 소재 차트(MTx x는［법칙장해］의［강도］.생략할 때는１)
　　예） MT　MT3　MT9
・D66주사위 가능
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["MT", "RT", "ET", "KT", "NT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Kamigakari#eval_game_system_specific_command`（`ko_kr` の表・定型文で）。
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
            .join("test/data/Kamigakari_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Kamigakari_Korean.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/Kamigakari_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Kamigakari_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            23,
            "case count in test/data/Kamigakari_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Kamigakari:Korean",
                "unexpected game system in Kamigakari_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Kamigakari:Korean"), &tc.input, &mut src) {
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
                    "FAIL Kamigakari_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Kamigakari_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
