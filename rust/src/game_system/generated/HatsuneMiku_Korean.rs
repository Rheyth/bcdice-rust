//! P4で手書き移植した `lib/bcdice/game_system/HatsuneMiku_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `HatsuneMiku` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! 判定・表の引き方は [`super::HatsuneMiku`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データだけを置く。
//! 表データは `i18n/HatsuneMiku/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。

use super::HatsuneMiku::{eval_specific_command, SystemTables};
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_FT_ITEMS: &[&str] = &[
    "주변에서 활기가 사라진다. 검정(흑) 이외의 모든 음색을 하나씩 줄인다.",
    "동료에게 폐를 끼쳐버린다. 자신 이외의 PC 전원의【생명력】이 1점 감소한다.",
    "이 실패가 나중에 화가 될지도 모른다……. 자신의【생명력】이 1D6점 감소한다.",
    "마음에 피로감이 가득 차간다. 자신이 스트레스를 1점 받는다.",
    "1D6을 굴린다. 그 PC의 코어가, 주사위 눈이 1이면 다크로, 2이면 핫으로, 3이면 러브로, 4이면 엑센트릭으로, 5이면 멜랑콜리로 변화한다. 6이었을 경우, 코어는 변화하지 않는다.",
    "럭키! 아무 일도 일어나지 않는다.",
];
static KO_FT: Table = Table::from_dice("펌블표", 1, 6, KO_FT_ITEMS);

static KO_CWT_ITEMS: &[&str] = &[
    "절망적인 공격을 받는다. 그 캐릭터는 강제 퇴출된다.",
    "고통의 비명을 지르며 비참하게 쓰러진다. 그 캐릭터는 행동불능이 된다. 또한, 검정(흑) 음색이 하나 늘어난다.",
    "오토쿠이의 일격으로 당신은 날아가버린다. 그 캐릭터는 행동불능이 된다. 또한, 분류가 장비인 넘버에 스트레스를 1점 받는다.",
    "강렬한 일격을 받고 기절한다. 그 캐릭터는 행동불능이 된다.",
    "의식은 있지만 일어설 수 없다. 그 캐릭터는 행동불능이 된다. 다음 장면에 아직【생명력】이 0점이었을 경우, 자동으로 1점 회복한다.",
    "기적적으로 버텨내며 견뎌낸다. 【생명력】이 1점이 된다.",
];
static KO_CWT: Table = Table::from_dice("치명상표", 1, 6, KO_CWT_ITEMS);

static KO_BT_ITEMS: &[&str] = &[
    "동료와의 즐거운 시간. 자신의【소중한 사람】의 파토스를 하나 회복한다.",
    "지금까지의 모험을 되돌아본다. 자신의【능력치】의 파토스를 하나 회복한다.",
    "자신의 오토다마와 대화한다.【협력자】의 파토스나, 넘버의 파토스를 하나 회복한다.",
    "몸을 천천히 쉰다. 자신의【생명력】을 2D6점 회복한다. 원한다면, 회복을 하기 전에, 자신의【활력】을 다시 결정해도 좋다.",
    "오, 럭키! 좋은 걸 찾았다! 자신의 코인을 1개 늘린다.",
    "노이즈 스토어에 접속했다. 각 PC는, 자신의【두뇌】의 주사위 수와 같은 개수까지, 어플(アプリ)을 구입할 수 있다.",
];
static KO_BT: Table = Table::from_dice("휴식표", 1, 6, KO_BT_ITEMS);

static KO_TT_ITEMS: &[&str] = &[
    "악의. PC 중에서【생명력】이 가장 낮은 한 명을 목표로 선택한다.【생명력】이 가장 낮은 PC가 여럿일 경우, 그중에서 GM이 임의로 한 명을 선택한다.",
    "교활. 패러그래프 1~5 중에서, 숫자가 가장 높은 패러그래프에 있는 PC 한 명을 목표로 선택한다. 전원이 권외에 있을 경우, 권외에 있는 PC 전원을 목표로 선택한다.",
    "견실. PC 중에서, 해당 위협의 「판정란」에 적힌 능력치 랭크가 가장 낮은 캐릭터 한 명을 목표로 선택한다. 랭크가 가장 낮은 캐릭터가 여럿일 경우, 그중에서 수정치가 가장 낮은 캐릭터 한 명을 선택한다. 수정치까지 같을 경우, GM이 임의로 한 명을 선택한다.",
    "호쾌. PC 중에서【무용】 랭크가 가장 높은 한 명을 목표로 선택한다. 랭크가 가장 높은 PC가 여럿일 경우, 그중에서 수정치가 가장 높은 한 명을 선택한다. 수정치까지 같을 경우, GM이 임의로 한 명을 선택한다.",
    "단순. 패러그래프 1~5 중에서, 숫자가 가장 낮은 패러그래프에 있는 PC 한 명을 목표로 선택한다. 전원이 권외에 있을 경우, 권외에 있는 PC 전원을 목표로 선택한다.",
    "난전. 해당 위협이 있는 패러그래프와, 숫자가 1만큼 차이 나는 패러그래프에 있는 PC 전원을 목표로 선택한다. 해당 패러그래프에 PC가 없을 경우, GM이 임의로 한 명을 선택한다.",
];
static KO_TT: Table = Table::from_dice("목표표", 1, 6, KO_TT_ITEMS);

static KO_RT_ITEMS: &[&str] = &[
    "연심(플러스)／살의(마이너스)",
    "동정(플러스)／멸시(마이너스)",
    "동경(플러스)／질투(마이너스)",
    "신뢰(플러스)／의심(마이너스)",
    "공감(플러스)／섬뜩함(마이너스)",
    "소중함(플러스)／귀찮음(마이너스)",
];
static KO_RT: Table = Table::from_dice("관계표", 1, 6, KO_RT_ITEMS);

static KO_OT_ITEMS: &[&str] = &[
    "당신의 마음에 큰 변화가 찾아온다. 1D6을 굴린다. 그 PC의 코어가, 주사위 눈이 1이면 다크로, 2이면 핫으로, 3이면 러브로, 4이면 엑센트릭으로, 5이면 멜랑콜리로 변화한다. 6이었을 경우, 코어는 변화하지 않는다.",
    "당신은 육체적으로 큰 대미지를 입는다. 1D6점의 대미지를 받는다.",
    "노이즈의 도움을 빌려 문제를 해결한다. 코인을 1D6개 지불할 필요가 있다. 코인을 지불할 경우, 다른 PC에게 코인을 받아도 좋다. 지불이 부족했을 경우, 그 차액만큼 자신의 【생명력】을 줄인다.",
    "큰 피로감을 느낀다. 스트레스를 1점 받는다.",
    "무심코 시간을 써버린다.【타임】이 1점 감소한다.",
    "그 자리에 불길한 기운이 가득 찬다. 검정(흑) 음색이 하나 늘어난다.",
];
static KO_OT: Table = Table::from_dice("장애표", 1, 6, KO_OT_ITEMS);

static KO_RQT_ITEMS: &[&str] = &[
    "그 구역의 풍경이, 당신의【정경】으로 바뀌어간다. 「너의 시작에 얽힌 이야기를 들려다오. 너는 이 땅에서 무엇을 느꼈느냐?」",
    "당신은, 그 구역의 풍경 속에서 그리운 무언가를 발견하고, 자신의 죄를 떠올렸다. 「무엇을 보았느냐? 무엇을 후회하고 있지?」",
    "그 구역의 풍경이, 당신의 코어와 같은 색으로 물든다. 당신은 그 풍경 속에서 자신이 되고 싶은 모습을 발견한다. 「그것이 너의 바람인가? 너는 미래에서 무엇을 추구하고자 하느냐?」",
    "당신의 뇌리에, 인물란에 적힌 인물 한 명의 이미지가 떠오른다. 그 인물은 무언가를 속삭이고, 당신의 마음은 상처받았다. 「그 자는 누구냐? 네게 뭐라고 했지?」",
    "당신은 그 구역의 풍경 속에서 기묘한 것을 발견하고, 공포에 떨었다. 「무엇을 보았느냐? 어째서 그것을 두려워하지?」",
    "그 구역에 코코로 던전의 소유자가 나타난다. 그 인물은 당신에게 질문을 던진다. 「나를 어떻게 생각해? 어째서, 나를 돕는 거야?」",
    "당신의 오토다마의 모습이, 당신이 잘 아는 인물로 변한다 「그 자는 누구인가? 그 녀석을 어떻게 생각하고 있지?」",
    "그 구역에, 당신이 가진 넘버가 울려 퍼진다. 「이것이 너의 노래인가? 그 노래의 이름은 무엇이냐?」",
    "당신의 오토다마의 모습이, 당신이 원하는 인물의 모습으로 변한다. 「그것이 네가 갈망하는 인물인가. 그자를 어떻게 하고 싶지?」",
    "그 구역의 풍경에 당신의 일상이 떠오른다. 「너는 무엇을 하고 있지? 그 삶에 대해 어떻게 생각하고 있느냐?」",
    "당신의 눈앞에, 당신의 시체가 놓여있다. 「너를 죽이는 것은 무엇이냐? 너는 누구에게 살해당하는가?」",
];
static KO_RQT: Table = Table::from_dice("리퀘스트표", 2, 6, KO_RQT_ITEMS);

static KO_CLT_ITEMS: &[&str] = &[
    "비밀번호가 유출되었다! 소지금이 사라졌다! 자신의 코인을 3개 잃는다.",
    "과거에 같은 오토쿠이와 마주친 적이 있는 인물과 만난다.【기술】로 판정한다. 성공하면, 「특수 어플 개발」을 행할 수 있다(이 간주 액션에는【타임】이 필요 없다). 필요한 코인은 1개 줄어든다.",
    "근처에 있는 파워스팟을 알게된다.【영력】으로 판정한다. 성공하면, 자신의【생명력】을 【활력】의 값만큼 회복할 수 있다.",
    "당신을 아는 인물과 만난다. 어떤 추억 이야기를 나누었을까? 이 질문은 리퀘스트로 취급한다.",
    "사적인 친구에게서 메일이 와있다.【사랑】으로 판정한다. 성공하면, 원하는 NPC를 협력자로 설정할 수 있다. 판정에 실패하면 도착한 건 당신에게 불만을 표현하는 메일이었다. 스트레스를 1점 받는다.",
    "노이즈 멤버에게서 응원 메시지를 받는다. 원하는 음색을 1개 획득한다(이 효과로. 특정 음색을 7개 이상으로 할 수는 없다).",
    "맛있는 음식점에 관한 정보를 알려받는다.【일상】으로 판정한다. 성공하면, 자신의 스트레스를 1점 회복할 수 있다.",
    "오토쿠이에 관한 정보를 구하는 노이즈 멤버와 만난다. 공개된 위협 1개당, 그 정보를 코인 1개에 매각할 수 있다. 이 이벤트가 2번 이상 일어났을 경우, 이미 매각한 위협의 정보를 다시 팔 수는 없다.",
    "시제품 어플의 테스터를 모집하고 있다. 원하는 어플 1개를 획득한다. 단, 이 어플을 사용할 때 주사위를 1개 굴린다. 1이나 2가 나오면, 그 어플은 효과를 발휘하지 않는다. 세션 중에 시제품 어플을 사용했다면 세션 종료 시에 리포트를 제출할 수 있다. 【두뇌】로 판정한다. 성공하면, 코인을 1개 획득할 수 있다.",
    "자신에 관한 험담을 발견한다. 거기에는 어떤 험담이 쓰여있었을까. 이 질문은 리퀘스트로 취급한다.",
    "같은 종류의 오토다마와 계약한 오토다마 사용자와 의기투합한다. 이 세션 동안, 자신의 넘버 1개를 습득 가능한 다른 넘버로 변경할 수 있다.",
];
static KO_CLT: Table = Table::from_dice("크롤표", 2, 6, KO_CLT_ITEMS);

static KO_RWT_ITEMS: &[&str] = &[
    "노이즈에게서 오토쿠이 퇴치 보상을 받을 수 있다. [쓰러뜨린 오토쿠이 본체의 레벨]개의 코인을 획득한다.",
    "노이즈에게 오토다마의 정보를 매각할 수 있다. [자신의【두뇌】의 주사위 수]개의 코인을 획득한다.",
    "모험을 통해 인연이 싹튼다. 이번에 등장한 캐릭터 중에서 한 명을 선택한다. 그 캐릭터를, 자신의【소중한 사람】으로 한다.",
    "모험을 통해 인연이 맺어진다. 이번에 등장한 NPC 중에서 한 명을 선택한다. 그 캐릭터를, 자신의【협력자】로 한다.",
    "모험의 추억이【노래의 조각】이 된다. 이번 모험에 등장한 동료, 정경, 사건 등에서, 키워드를 하나 선택한다. 그 키워드를【노래의 조각】의 키워드란에 추가한다.",
    "싸움의 경험이【노래의 조각】이 된다. 이번 모험에 등장한 적, 정경, 사건 등에서, 키워드를 하나 선택한다. 그 키워드를【노래의 조각】의 키워드란에 추가한다.",
];
static KO_RWT: Table = Table::from_dice("보상표", 1, 6, KO_RWT_ITEMS);

static KO_NMT_ITEMS: &[&str] = &[
    "절망의 노래에 지각이 차단된다. 등 뒤에서 오토쿠이의 기척을 느꼈다고 생각했을 때는 이미 늦었다. 비열한 공격이 당신을 덮친다. 원하는 능력치로 판정한다. 실패하면 당신의 캐릭터는, 오토나시가 되어 두 번 다시 모험에 참가할 수 없다.",
    "절망의 노래에 섞여, 비통한 비명이 들려온다. 코코로 던전의 소유자일까. 당신은 구해내지 못한 것이다. 【일상】으로 판정한다. 실패하면, 자신의 능력치 하나를 선택한다. 다음 세션은, 그 능력치에 스트레스를 받은 상태로 시작한다.",
    "절망의 노래에 섞여, 오토쿠이의 웃음소리가 메아리친다. 그것은 조소의 웃음이었다. 오토쿠이나 동료들……무엇보다 자신에 대한 분노가 치밀어 오른다. 【일상】으로 판정한다. 실패하면, 자신의 소중한 사람에 대한 【마음】을 하나 잃는다.",
    "절망의 노래 속에 혼자 남겨진다. 아무도 당신을 알아채지 못한다. 고독을 견디면서, 겨우 일상으로 귀환했지만……그때의 공포가 지워지지 않는다. 【일상】으로 판정한다. 실패하면, 다음 세션은, 자신의 【생명력】의 현재값이 통상의 절반(소수점 올림) 상태로 시작한다.",
    "코코로 던전에서 귀환한 당신을 기다리고 있던 것은 변함없는 일상이었다. 당신이 임무에 실패해도 세계는 변하지 않는다. 그렇다면, 이제, 그런 무서운 일을 할 필요는 없는 게 아닐까? 【일상】으로 판정한다. 실패하면, 자신의 넘버 하나를 선택한다. 다음 세션은 그 넘버에 스트레스를 받은 상태로 시작한다.",
    "절망의 노래 속을 필사적으로 도망쳤다. 등 뒤에서 동료의 목소리가 들린 것 같다. 하지만, 당신은 돌아볼 수 없었다. 【일상】으로 판정한다. 실패하면, 자신에 대해【마음】을 가진 PC 한 명을 선택한다. 그 PC가 당신에게 가지고 있던【마음】은 소실된다",
];
static KO_NMT: Table = Table::from_dice("악몽표", 1, 6, KO_NMT_ITEMS);

static KO_OIT_ITEMS: &[&str] = &[
    "소인",
    "나／나(여)",
    "자기 이름",
    "나／저",
    "저(격식)",
    "나",
    "저／나(친근)",
    "자신",
    "이 몸／나(여, 격식)",
    "과인／소첩",
    "미(Me)",
];
static KO_OIT: Table = Table::from_dice("오토다마 1인칭표", 2, 6, KO_OIT_ITEMS);

static KO_OYT_ITEMS: &[&str] = &[
    "유(You)",
    "（PC의 이름）땅／큥",
    "동지（PC의 이름）",
    "그대",
    "（PC의 이름）군／짱",
    "마스터",
    "（PC의 이름）씨",
    "（PC의 이름）님",
    "당신",
    "（PC의 이름）씨／여사",
    "（PC의 이름）전하",
];
static KO_OYT: Table = Table::from_dice("오토다마 호칭표", 2, 6, KO_OYT_ITEMS);

static KO_ORT_ITEMS: &[&str] = &[
    "오토다마의 겉 성격을 나타내는 대사",
    "오토다마의 속 성격을 나타내는 대사",
    "PC를 응원하는 대사",
    "PC를 놀리는 대사",
    "취미와 관련된 대사",
    "공격을 행할 때의 대사",
];
static KO_ORT: Table = Table::from_dice("리액션표", 1, 6, KO_ORT_ITEMS);

static KO_OMT_ITEMS: &[&str] = &[
    "명문 오토다마 사용자. 당신은, 대대로 오토다마를 다루는 일족에서 태어났습니다. 당신에게는, 어릴 때부터 파트너로 함께 한 오토다마가 있습니다. 당신은 그 오토다마와 함께 자랐습니다.",
    "상처받은 오토다마. 어느 날, 당신은 상처받은 오토다마를 발견했습니다. 의식을 잃고, 곧 사라질 것 같은 오토다마에 손을 대자, 오토다마는 의식을 되찾고 당신을 은인으로 따르게 되었습니다.",
    "보이지 않는 친구. 당신은 고독한 유년기를 보내왔습니다. 그때, 당신을 이끌어 준 것이, 당신의 오토다마입니다. 오토다마는 당신에게 타인의 마음의 노래를 듣고 사람들을 돕는 방법을 가르쳐 주었습니다.",
    "재생. 당신은 오토쿠이에게 자신의 마음의 노래를 먹혔습니다. 오토다마 사용자에게 빙의한 오토쿠이가 쓰러졌을 때, 자신의 마음 속에서 새로운 오토다마가 태어났습니다.",
    "사랑하는 마음. 당신에게는, 어릴 때부터 매우 좋아했던 노래가 있었습니다. 어느 날, 그 노래를 흥얼거리고 있을 때, 갑자기 뒤에서 박수 소리가 들려왔습니다. 돌아보니 거기에 오토다마가 있었습니다.",
    "동영상. 당신은 동영상을 통해 노래를 듣는 것을 좋아했습니다. 어느 때, 들어본 적 없는 멋진 노래가 들려오나 했더니, 화면 너머에서 오토다마가 튀어나왔습니다.",
    "상실. 어느 날, 당신은 비극에 휩쓸렸습니다. 그때, 당신은 매우 소중히 했던 무언가를 잃었습니다. 그 잃은 것을 보완하듯이, 당신 곁에 오토다마가 나타났습니다.",
    "이어지는 노래. 당신의 오토다마는, 당신이 매우 좋아했던 사람의 파트너였던 오토다마였습니다. 하지만, 그 사람은 비극을 겪고 당신 곁을 떠났습니다. 그때, 그 자는 당신에게 오토다마를 맡긴 것입니다.",
    "수수께끼의 메일. 어느 날, 친구가 당신에게 한 통의 메일을 보냈습니다. 그 메일을 열자 신기한 음악이 흘러나오고 오토다마가 나타났습니다. 그 친구와는 그 이후로 연락이 닿지 않습니다.",
    "봉인. 어느 날, 당신은 낡은 레코드 가게에서 한 곡의 음반을 만납니다. 그 음반을 재생해보자 오토다마가 나타났습니다. 그리고, 오토다마는 「봉인을 풀어준 보답으로, 잠시 함께 어울려줄게」라고 말해왔습니다.",
    "첫눈에 반함. 이전에, 당신은 다양한 악곡을 발표했습니다. 그러자, 그 악곡에 첫눈에 반했다며 당신에게 오토다마가 찾아왔습니다. 이후, 그 오토다마에게 쫓기는 나날입니다.",
];
static KO_OMT: Table = Table::from_dice("만남표", 2, 6, KO_OMT_ITEMS);

static KO_ST_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("줄지어 선 서가의 숲")),
    (12, TableItem::Text("석양이 비치는 교실")),
    (13, TableItem::Text("멈추지 않는 건널목")),
    (14, TableItem::Text("빌딩에서 내려다본 거리")),
    (15, TableItem::Text("둘이서 본 별하늘")),
    (16, TableItem::Text("액정 화면에 비치는 기묘한 광경")),
    (22, TableItem::Text("유리창에 늘어선 빗방울")),
    (23, TableItem::Text("식물원의 온실")),
    (24, TableItem::Text("포장마차가 늘어선 축제 풍경")),
    (25, TableItem::Text("아지랑이가 피어오르는 아스팔트")),
    (26, TableItem::Text("0시를 가리키는 시계 바늘")),
    (33, TableItem::Text("무기질적인 하얀 천장")),
    (34, TableItem::Text("어둠 속에 떠오르는 헤드라이트")),
    (35, TableItem::Text("뒤에서 따라오는 길고양이")),
    (36, TableItem::Text("온통 꽃밭")),
    (44, TableItem::Text("당신을 바라보는 많은 관중")),
    (45, TableItem::Text("쌓여가는 눈")),
    (46, TableItem::Text("낡은 서양관의 응접실")),
    (55, TableItem::Text("동화에 나오는 듯한 숲")),
    (56, TableItem::Text("심야의 편의점")),
    (66, TableItem::Text("아무도 없는 체육관")),
];
static KO_ST: D66Table = D66Table::new("정경표", D66SortType::Asc, KO_ST_ITEMS);

static KO_DKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("붕괴하는 낙원")),
    (12, TableItem::Text("하늘로 떨어진다")),
    (13, TableItem::Text("부드러운 폭력")),
    (14, TableItem::Text("침묵의 계율")),
    (15, TableItem::Text("어둠에 빠진다")),
    (16, TableItem::Text("흘러넘친 생명")),
    (22, TableItem::Text("막다른 절망")),
    (23, TableItem::Text("칠흑의 날개")),
    (24, TableItem::Text("잠못드는 밤")),
    (25, TableItem::Text("피할 수 없는 운명")),
    (26, TableItem::Text("베어진 풍경")),
    (33, TableItem::Text("텅 빈 자신")),
    (34, TableItem::Text("가면의 안쪽")),
    (35, TableItem::Text("월광 중독")),
    (36, TableItem::Text("어두운 마술")),
    (44, TableItem::Text("……오브 더 데드")),
    (45, TableItem::Text("마음을 죽인다")),
    (46, TableItem::Text("감염하는 파멸")),
    (55, TableItem::Text("사랑의 쇠사슬")),
    (56, TableItem::Text("잔혹한 진실")),
    (66, TableItem::Text("데스게임")),
];
static KO_DKT: D66Table = D66Table::new("다크・키워드표", D66SortType::Asc, KO_DKT_ITEMS);

static KO_HKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("한밤중을 부숴라")),
    (12, TableItem::Text("꿈을 쏘아올려라")),
    (13, TableItem::Text("양보할 수 없는 내일")),
    (14, TableItem::Text("넘쳐흐르는 충동")),
    (15, TableItem::Text("짐승을 해방시켜라")),
    (16, TableItem::Text("증발한 눈물")),
    (22, TableItem::Text("높이 외쳐라")),
    (23, TableItem::Text("질 수 없는 싸움")),
    (24, TableItem::Text("움켜쥔 주먹")),
    (25, TableItem::Text("질주하는 청춘")),
    (26, TableItem::Text("마음이 외치는 대로 따라라")),
    (33, TableItem::Text("힘내라")),
    (34, TableItem::Text("그대로 나아가라")),
    (35, TableItem::Text("자신의 깃발")),
    (36, TableItem::Text("저항하고 부수고 돌진한다")),
    (44, TableItem::Text("활짝 피는 열정의 꽃")),
    (45, TableItem::Text("뜨거운 우정")),
    (46, TableItem::Text("내 색으로 물들어라")),
    (55, TableItem::Text("세상에 화풀이")),
    (56, TableItem::Text("지울 수 없는 불꽃")),
    (66, TableItem::Text("오버드라이브")),
];
static KO_HKT: D66Table = D66Table::new("핫・키워드표", D66SortType::Asc, KO_HKT_ITEMS);

static KO_LKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("어른의 사랑")),
    (12, TableItem::Text("두근거림이 멈추질 않아")),
    (13, TableItem::Text("잡은 손")),
    (14, TableItem::Text("세상을 적으로 돌려도")),
    (15, TableItem::Text("겹치는 목소리")),
    (16, TableItem::Text("너를 위해서라면 죽을 수 있어")),
    (22, TableItem::Text("달콤한 입맞춤")),
    (23, TableItem::Text("눈꺼풀을 감고")),
    (24, TableItem::Text("너와 나")),
    (25, TableItem::Text("좋다든가 싫다든가")),
    (26, TableItem::Text("언제까지나")),
    (33, TableItem::Text("껴안고 싶어")),
    (34, TableItem::Text("75억하고도 1천5백만 명을 사랑해")),
    (35, TableItem::Text("자동적인 사랑")),
    (36, TableItem::Text("만나고 싶어")),
    (44, TableItem::Text("전하고 싶은 말")),
    (45, TableItem::Text("고마워")),
    (46, TableItem::Text("시간을 멈춰")),
    (55, TableItem::Text("좋아해")),
    (56, TableItem::Text("멋진 선물")),
    (66, TableItem::Text("뷰티풀 월드")),
];
static KO_LKT: D66Table = D66Table::new("러브・키워드표", D66SortType::Asc, KO_LKT_ITEMS);

static KO_EKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("셰프의 변덕 니르바나")),
    (12, TableItem::Text("반찬 너무 먹음")),
    (13, TableItem::Text("바이바이 비아그라")),
    (14, TableItem::Text("이불 왕국의 공방")),
    (15, TableItem::Text("빙글빙글과 뱅글뱅글")),
    (16, TableItem::Text("고릴라의 천년왕국")),
    (22, TableItem::Text("구몬식 프랑켄슈타이너")),
    (23, TableItem::Text("외계인과 데이트")),
    (24, TableItem::Text("매일매일 방콕 설날")),
    (25, TableItem::Text("고양이가 냐옹")),
    (26, TableItem::Text("농땡이의 쓴맛")),
    (33, TableItem::Text("부시도 록! 사무라이 펑크!")),
    (34, TableItem::Text("히야시 인도")),
    (35, TableItem::Text("살아있어서 다행이야")),
    (36, TableItem::Text("탱글탱글")),
    (44, TableItem::Text("새벽의 투탕카멘")),
    (45, TableItem::Text("반값 세일의 연회")),
    (46, TableItem::Text("초기분좋은무언가")),
    (55, TableItem::Text("이아! 이아! 하스터!")),
    (56, TableItem::Text("초딩에게 저금으로 졌다")),
    (66, TableItem::Text("초속 1폴론크세마")),
];
static KO_EKT: D66Table = D66Table::new("엑센트릭・키워드표", D66SortType::Asc, KO_EKT_ITEMS);

static KO_MKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("미안해요")),
    (12, TableItem::Text("달콤달콤한 도피")),
    (13, TableItem::Text("혼자")),
    (14, TableItem::Text("치사한 세상")),
    (15, TableItem::Text("이루지 못한 약속")),
    (16, TableItem::Text("돌이킬 수 없는 말")),
    (22, TableItem::Text("차라리 죽고 싶어")),
    (23, TableItem::Text("버려둔 꿈")),
    (24, TableItem::Text("올려다본 파란 하늘")),
    (25, TableItem::Text("너의 거짓말")),
    (26, TableItem::Text("엇갈리는 말")),
    (33, TableItem::Text("행복했던 어제")),
    (34, TableItem::Text("이럴 줄 몰랐는데")),
    (35, TableItem::Text("헤어진 두 갈래 길")),
    (36, TableItem::Text("또 만날 수 있으면 좋겠어")),
    (44, TableItem::Text("여기가 아닌 어딘가")),
    (45, TableItem::Text("청춘의 끝")),
    (46, TableItem::Text("좋아했던 무릎 위")),
    (55, TableItem::Text("누가 나를 칭찬해줘")),
    (56, TableItem::Text("고결한 배신")),
    (66, TableItem::Text("나르시시즘")),
];
static KO_MKT: D66Table = D66Table::new("멜랑콜리・키워드표", D66SortType::Asc, KO_MKT_ITEMS);

static KO_DNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("다크／濁、搦　네로／音呂、寝路")),
    (12, TableItem::Text("쿠로토／黒斗、玄徒　야미／夜美、闇")),
    (13, TableItem::Text("네크로／根黒、寝喰　마코／魔子、混乎")),
    (
        14,
        TableItem::Text("카게오／影男、陰夫　오니코／鬼子、隠忍呼"),
    ),
    (15, TableItem::Text("아쿠타／芥、悪太　호타루／蛍、歩足")),
    (
        16,
        TableItem::Text("마오우／魔王、万凹　미다라／淫、美堕裸"),
    ),
    (
        22,
        TableItem::Text("마미야／魔美也、狸夜　쟈미／邪美、蛇実"),
    ),
    (23, TableItem::Text("도쿠로／髑髏、毒炉　요미／黄泉、詠")),
    (24, TableItem::Text("마쿠라／枕、真暗　사츠키／殺鬼、五月")),
    (25, TableItem::Text("게도우／外道、戯堂　사야／小夜、鞘")),
    (26, TableItem::Text("지고쿠／地獄、慈極　우마루／埋、兎丸")),
    (33, TableItem::Text("엔도／怨人、終　요하네／夜羽、世刎")),
    (34, TableItem::Text("노로이／呪、鈍　카바네／屍、椛音")),
    (35, TableItem::Text("아쿠무／悪夢、飽夢　쿠사리／腐、鎖")),
    (36, TableItem::Text("바츠／罰、×　니에／贄、沸")),
    (
        44,
        TableItem::Text("네가／音我、願　리리스／璃々子、離里素"),
    ),
    (45, TableItem::Text("우츠로／虚、洞　네타미／妬美、寝多実")),
    (46, TableItem::Text("하지메／始、創　호로비／滅、亡")),
    (55, TableItem::Text("자인／罪印、沙陰　린보／淋墓、辺獄")),
    (
        56,
        TableItem::Text("하라와타／腑、祓輪太　유가미／歪、由神"),
    ),
    (
        66,
        TableItem::Text("이미／忌、逝美　무이미／無意味、無為巳"),
    ),
];
static KO_DNT: D66Table = D66Table::new("다크・이름표", D66SortType::Asc, KO_DNT_ITEMS);

static KO_HNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("레드／烈怒、煉集　아카네／赤音、茜")),
    (12, TableItem::Text("아츠시／熱、純志　칸나／神奈、柑菜")),
    (13, TableItem::Text("카케루／駆、賭　하루／晴、春")),
    (14, TableItem::Text("갓츠／牙突、勝　아카리／紅莉、明里")),
    (15, TableItem::Text("켄／剣、拳　아스카／明日香、飛鳥")),
    (16, TableItem::Text("고우／豪、剛　히미코／日美子、卑弥呼")),
    (22, TableItem::Text("히이로／火色、陽彩　아키라／晶、爽")),
    (23, TableItem::Text("타케루／武、猛　히토미／瞳、仁美")),
    (24, TableItem::Text("구렌／紅蓮、九煉　나츠코／夏子、懐子")),
    (25, TableItem::Text("아라시／嵐、荒　히카루／光、晃")),
    (
        26,
        TableItem::Text("엔죠우／炎上、円定　코마치／小町、小真知"),
    ),
    (33, TableItem::Text("레츠／烈、裂　리즈무／理澄、李珠夢")),
    (34, TableItem::Text("리키／力、陸希　쿄우카／響歌、驚花")),
    (35, TableItem::Text("호무라／焔、吠叢　카구야／輝夜、赫映")),
    (36, TableItem::Text("죠우／情、丈　아오리／煽、亜織")),
    (44, TableItem::Text("록쿠／六句、麓　포르테／鳳流弖、彫照")),
    (
        45,
        TableItem::Text("야마토／大和、岳斗　이사미／伊佐美、勇美"),
    ),
    (
        46,
        TableItem::Text("류우세이／流星、龍盛　미라이／未来、美良依"),
    ),
    (
        55,
        TableItem::Text("이카루／怒、鵤　히마와리／向日葵、火回"),
    ),
    (56, TableItem::Text("츠토무／努、勉　하나비／花火、羽夏妃")),
    (66, TableItem::Text("레오／伶央、獅王　마츠리／祭、茉莉")),
];
static KO_HNT: D66Table = D66Table::new("핫・이름표", D66SortType::Asc, KO_HNT_ITEMS);

static KO_LNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("시안／詩庵、思杏　아오이／葵、蒼生")),
    (
        12,
        TableItem::Text("소나타／奏名太、其方　이즈미／泉、出海"),
    ),
    (13, TableItem::Text("츠나구／繋、継　카렌／可憐、歌恋")),
    (14, TableItem::Text("미노루／実、稔　코이／恋、鯉")),
    (15, TableItem::Text("유우／優、悠　라부／良舞、羅步")),
    (16, TableItem::Text("레인／玲音、霊印　아마미／甘味、天海")),
    (22, TableItem::Text("소우야／想夜、添也　후미／文、芙美")),
    (
        23,
        TableItem::Text("이토시／糸糸、意俊　코이시／恋志、小石"),
    ),
    (24, TableItem::Text("에가오／笑顔、描生　오모이／想、念")),
    (25, TableItem::Text("마코토／誠、真実　마나／真菜、愛")),
    (26, TableItem::Text("유우리／有理、悠里　케이／恵、佳")),
    (33, TableItem::Text("치히로／千尋、茅紘　우라라／麗、占")),
    (34, TableItem::Text("토모／友、杜望　히나／雛、比奈")),
    (35, TableItem::Text("소라／空、宙　츠유／露、梅雨")),
    (
        36,
        TableItem::Text("유우다이／雄大、優大　노조미／望、希海"),
    ),
    (44, TableItem::Text("하구／剥、抱　키스／喜好、口吻")),
    (45, TableItem::Text("쇼우타／翔太、祥太　아이／愛、藍")),
    (46, TableItem::Text("준／純、潤　미사오／美沙緒、操")),
    (55, TableItem::Text("료우／涼、猟　이치즈／一途、意地図")),
    (
        56,
        TableItem::Text("시구레／時雨、紫暮　아오바／青葉、碧羽"),
    ),
    (
        66,
        TableItem::Text("로미오／路美雄、露澪　로만／浪漫、絽萬"),
    ),
];
static KO_LNT: D66Table = D66Table::new("러브・이름표", D66SortType::Asc, KO_LNT_ITEMS);

static KO_ENT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("라이무／来夢、雷鵡　미도리／緑、美登里"),
    ),
    (12, TableItem::Text("란포／乱歩、蘭舗　비비리／恐、美々裏")),
    (
        13,
        TableItem::Text("시라즈／不知、調頭　히스이／翡翠、陽彗"),
    ),
    (14, TableItem::Text("무우／夢生、無　키노코／茸、紀乃子")),
    (
        15,
        TableItem::Text("네코히코／猫彦、寝子日子　이누코／犬子、夷猫"),
    ),
    (16, TableItem::Text("다다／駄々、蛇陀　키리코／切子、霧湖")),
    (
        22,
        TableItem::Text("이케멘／活面、逝麺　라무네／来夢音、螺旨"),
    ),
    (
        23,
        TableItem::Text("쿄우스케／狂介、京助　란마／乱麻、爛漫"),
    ),
    (
        24,
        TableItem::Text("네지／螺子、寝児　아리스／有栖、亜梨子"),
    ),
    (25, TableItem::Text("마와루／回、環　타타미／畳、多々実")),
    (26, TableItem::Text("큐우／球、Ｑ　즈킨／頭巾、厨琴")),
    (
        33,
        TableItem::Text("사반／沙蛮、裂卍　마니아／摩尼亜、間合"),
    ),
    (
        34,
        TableItem::Text("카에루／帰、蛙　에리마키／襟巻、絵里真希"),
    ),
    (35, TableItem::Text("나조우／謎宇、何造　칸논／観音、疳暢")),
    (
        36,
        TableItem::Text("잇큐우／一休、逸宮　미로쿠／弥勒、診録"),
    ),
    (
        44,
        TableItem::Text("슈루／酒潤、終琉　카프리／華降、噛布里"),
    ),
    (
        45,
        TableItem::Text("키진／奇人、鬼神　후시기／不思議、節黄"),
    ),
    (46, TableItem::Text("카부키／歌舞伎、傾　메론／芽論、女侖")),
    (
        55,
        TableItem::Text("죠카／冗歌、浄化　피에로／秘絵呂、道化師"),
    ),
    (
        56,
        TableItem::Text("우이로우／外郎、初弄　맛챠／抹茶、末耶"),
    ),
    (66, TableItem::Text("빅쿠리／吃驚、！　하테나／果菜、？")),
];
static KO_ENT: D66Table = D66Table::new("엑센트릭・이름표", D66SortType::Asc, KO_ENT_ITEMS);

static KO_MNT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("바이스／灰主、唄守　마시로／真白、万代"),
    ),
    (12, TableItem::Text("키즈／傷、疵　다레카／誰香、惰麗華")),
    (13, TableItem::Text("유레루／揺、遊玲流　에모／絵萌、恵面")),
    (14, TableItem::Text("오보로／朧、憶露　호노카／仄、穂乃香")),
    (15, TableItem::Text("메로／夢露、芽朗　시즈／静、志津")),
    (
        16,
        TableItem::Text("히이라기／柊、氷刺木　카타미／形見、片実"),
    ),
    (22, TableItem::Text("리넨／理然、離念　스노우／素皇、珠瑙")),
    (23, TableItem::Text("세츠나／切、刹那　시노부／偲、忍")),
    (24, TableItem::Text("나미다／涙、波太　카스카／霞歌、幽")),
    (25, TableItem::Text("무스비／結、息日　카코／過去、寡子")),
    (26, TableItem::Text("우소／嘘、宇曽　아이카／哀歌、愛香")),
    (33, TableItem::Text("페인／閉音、病印　츠라미／辛美、貫実")),
    (
        34,
        TableItem::Text("요리미치／寄道、頼道　시라유키／白雪、知由樹"),
    ),
    (35, TableItem::Text("히토리／独、一人　오토나／音鳴、乙菜")),
    (36, TableItem::Text("스바루／昴、透遙　하루카／遥、晴香")),
    (
        44,
        TableItem::Text("바이바이／梅云、吠々　바니라／香子蘭、芭韮"),
    ),
    (45, TableItem::Text("토오루／透、通　리츠／律、慄")),
    (46, TableItem::Text("타비／旅、足袋　치기리／契、千切")),
    (55, TableItem::Text("사이고／彩吾、最期　사쿠라／桜、咲良")),
    (56, TableItem::Text("아와레／憐、哀　히메이／悲鳴、姫衣")),
    (66, TableItem::Text("헤분／戸聞、天国　가라스／硝子、枯州")),
];
static KO_MNT: D66Table = D66Table::new("멜랑콜리・이름표", D66SortType::Asc, KO_MNT_ITEMS);

static KO_OPA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("상쾌함")),
    (12, TableItem::Text("단순함")),
    (13, TableItem::Text("주목받고 싶어함")),
    (14, TableItem::Text("잘 웃음")),
    (15, TableItem::Text("P를 매우 좋아함")),
    (16, TableItem::Text("열심히 하는 타입")),
    (22, TableItem::Text("익살스러움")),
    (23, TableItem::Text("쉽게 반함")),
    (24, TableItem::Text("용감함")),
    (25, TableItem::Text("호기심 왕성")),
    (26, TableItem::Text("다정함")),
    (33, TableItem::Text("팔방미인")),
    (34, TableItem::Text("박애주의")),
    (35, TableItem::Text("감정적")),
    (36, TableItem::Text("수다스러움")),
    (44, TableItem::Text("무모함")),
    (45, TableItem::Text("활기참")),
    (46, TableItem::Text("낙관적")),
    (55, TableItem::Text("자신만만")),
    (56, TableItem::Text("자유로움")),
    (66, TableItem::Text("호전적")),
];
static KO_OPA: D66Table = D66Table::new("오토다마 성격표A", D66SortType::Asc, KO_OPA_ITEMS);

static KO_OPB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("비관적")),
    (12, TableItem::Text("얌전함")),
    (13, TableItem::Text("겁쟁이")),
    (14, TableItem::Text("쿨함")),
    (15, TableItem::Text("느긋함")),
    (16, TableItem::Text("진지함")),
    (22, TableItem::Text("몽상가")),
    (23, TableItem::Text("상식인")),
    (24, TableItem::Text("사이코")),
    (25, TableItem::Text("너그러움")),
    (26, TableItem::Text("평화주의자")),
    (33, TableItem::Text("신중함")),
    (34, TableItem::Text("합리주의자")),
    (35, TableItem::Text("말수가 적음")),
    (36, TableItem::Text("부끄러움을 잘 탐")),
    (44, TableItem::Text("착한 사람")),
    (45, TableItem::Text("게으름뱅이")),
    (46, TableItem::Text("겸손함")),
    (55, TableItem::Text("의심이 많음")),
    (56, TableItem::Text("겸허함")),
    (66, TableItem::Text("거짓말쟁이")),
];
static KO_OPB: D66Table = D66Table::new("오토다마 성격표B", D66SortType::Asc, KO_OPB_ITEMS);

static KO_OHT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("산책")),
    (12, TableItem::Text("소문 이야기")),
    (13, TableItem::Text("자기")),
    (14, TableItem::Text("독서")),
    (15, TableItem::Text("아이돌")),
    (16, TableItem::Text("단것")),
    (22, TableItem::Text("술자리")),
    (23, TableItem::Text("온천")),
    (24, TableItem::Text("도박")),
    (25, TableItem::Text("동물")),
    (26, TableItem::Text("애니메이션")),
    (33, TableItem::Text("가드닝")),
    (34, TableItem::Text("만화")),
    (35, TableItem::Text("드라마")),
    (36, TableItem::Text("경매")),
    (44, TableItem::Text("퍼즐")),
    (45, TableItem::Text("점술")),
    (46, TableItem::Text("고기구이")),
    (55, TableItem::Text("스포츠 관전")),
    (56, TableItem::Text("게임")),
    (66, TableItem::Text("동영상 스트리밍")),
];
static KO_OHT: D66Table = D66Table::new("오토다마 취미표", D66SortType::Asc, KO_OHT_ITEMS);

static KO_OLT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("기본")),
    (12, TableItem::Text("왕자님／공주님")),
    (13, TableItem::Text("일본 전통 의상")),
    (14, TableItem::Text("수인계")),
    (15, TableItem::Text("고스")),
    (16, TableItem::Text("안경")),
    (22, TableItem::Text("스포츠")),
    (23, TableItem::Text("군복")),
    (24, TableItem::Text("천사／악마의 날개")),
    (25, TableItem::Text("교복")),
    (26, TableItem::Text("메가폰")),
    (33, TableItem::Text("스포츠계")),
    (34, TableItem::Text("펑크")),
    (35, TableItem::Text("포멀")),
    (36, TableItem::Text("저지")),
    (44, TableItem::Text("계절 이벤트")),
    (45, TableItem::Text("흰 가운")),
    (46, TableItem::Text("동화 코스")),
    (55, TableItem::Text("바니")),
    (56, TableItem::Text("수영복")),
    (66, TableItem::Text("전대 코스")),
];
static KO_OLT: D66Table = D66Table::new("오토다마 외견표", D66SortType::Asc, KO_OLT_ITEMS);
/// Ruby `TABLES`（`translate_tables(:ko_kr)`）。
static KO_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("FT", &KO_FT),
    ("CWT", &KO_CWT),
    ("BT", &KO_BT),
    ("TT", &KO_TT),
    ("RT", &KO_RT),
    ("OT", &KO_OT),
    ("RQT", &KO_RQT),
    ("CLT", &KO_CLT),
    ("RWT", &KO_RWT),
    ("NMT", &KO_NMT),
    ("OIT", &KO_OIT),
    ("OYT", &KO_OYT),
    ("ORT", &KO_ORT),
    ("OMT", &KO_OMT),
    ("ST", &KO_ST),
    ("DKT", &KO_DKT),
    ("HKT", &KO_HKT),
    ("LKT", &KO_LKT),
    ("EKT", &KO_EKT),
    ("MKT", &KO_MKT),
    ("DNT", &KO_DNT),
    ("HNT", &KO_HNT),
    ("LNT", &KO_LNT),
    ("ENT", &KO_ENT),
    ("MNT", &KO_MNT),
    ("OPA", &KO_OPA),
    ("OPB", &KO_OPB),
    ("OHT", &KO_OHT),
    ("OLT", &KO_OLT),
];

static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    special: "스페셜",
    neiro_acquire: "　음색에 %{pickup_dice}(%{color})를 취득한 경우 %{total}:%{result}",
    colors: ["흑", "적", "청", "녹", "백", "임의"],
    fumble: "펌블",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::HatsuneMiku_Korean`（ID: `HatsuneMiku:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HatsuneMiku_Korean;

impl GameSystem for HatsuneMiku_Korean {
    fn id(&self) -> &'static str {
        "HatsuneMiku:Korean"
    }

    fn name(&self) -> &'static str {
        "하츠네 미쿠TRPG 코코로 던전"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:하츠네 미쿠TRPG 코코로 던전"
    }

    fn help_message(&self) -> &'static str {
        r"・판정(Rx±y@z>=t)
　능력치의 주사위마다 성공・실패 판정을 행합니다.
　x:능력 랭크(S,A~D). 숫자 지정으로 직접 그 개수의 주사위를 굴릴 수 있습니다
　y:수정값. A+2 또는 A++ 처럼 표기. 혼재 시 A++,+1 처럼 기술도 가능
　z:스페셜 최저값(생략 시 6)　t:목표값(생략 시 4)
　　예) RA　R2　RB+1　RC++　RD+,+2　RA>=5　RS-1@5>=6
　결과는 음색을 취득한 나머지에서 최대값을 표시
예) RB
　HatsuneMiku : (RB>=4) ＞ [3,5] ＞
　　음색에 3(파랑)을 취득한 경우 5:성공
　　음색에 5(하양)을 취득한 경우 3:실패

・각종 표
　펌블표 FT/치명상표 CWT/휴식표 BT/목표표 TT/관계표 RT
　장애표 OT/리퀘스트표 RQT/크롤표 CLT/보상표 RWT/악몽표 NMT/정경표 ST

・키워드 표
　다크 DKT/핫 HKT/러브 LKT/엑센트릭 EKT/멜랑콜리 MKT

・이름표 NT
　코어별　다크 DNT/핫 HNT/러브 LNT/엑센트릭 ENT/멜랑콜리 MNT

・오토다마 각종 표
　성격표A OPA/성격표B OPB/취미표 OHT/외견표 OLT/1인칭표 OIT/호칭표 OYT
　리액션표 ORT/만남표 OMT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "R[A-DS]?", "FT", "CWT", "BT", "TT", "RT", "OT", "RQT", "CLT", "RWT", "NMT", "OIT",
            "OYT", "ORT", "OMT", "ST", "DKT", "HKT", "LKT", "EKT", "MKT", "DNT", "HNT", "LNT",
            "ENT", "MNT", "OPA", "OPB", "OHT", "OLT",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

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
            .join("test/data/HatsuneMiku_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/HatsuneMiku_Korean.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/HatsuneMiku_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("HatsuneMiku_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            57,
            "case count in test/data/HatsuneMiku_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "HatsuneMiku:Korean",
                "unexpected game system in HatsuneMiku_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("HatsuneMiku:Korean"),
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
                    "FAIL HatsuneMiku_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} HatsuneMiku_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
