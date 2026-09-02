//! P4で手書き移植した `lib/bcdice/game_system/BeginningIdol_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `BeginningIdol` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! 表の引き方・コマンド解釈は [`super::BeginningIdol`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データ（`KO_` 接頭辞の `static` 群）だけを置く。
//!
//! データは `i18n/BeginningIdol/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ko_kr に無いキーは i18n gem の fallback と同じく `ja_jp` の値になる。fallback の粒度は
//! Ruby の `I18n.t` 呼び出し1つ分（例: `BeginningIdol.BT` は name/items が ko_kr、
//! `prefix_format` だけ ja_jp。`BeginningIdol.item_table` はハッシュごと ja_jp）。

use super::BeginningIdol::{
    check_result_nd6, eval_specific_command, Abnormality, AbnormalitySource, AbnormalityTable,
    BadStatusTable, BiChainD66Table, BiChainTable, D6TwiceTable, ItemTable, MySkillNameTable, Node,
    RandomEventTable, RollText, SkillGetTable, SkillHometown, SkillTable, SubTable, SystemTables,
    WorkWithChanceTable,
};
use crate::dice_table::sai_fic_skill_table::DEFAULT_SKILL_FORMAT;
use crate::dice_table::{
    D66Table, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table, TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::CheckOutcome;

static KO_SKILL_CATEGORY1: &[&str] = &[
    "～125", "131", "136", "141", "146", "156", "166", "171", "176", "180", "190～",
];

static KO_SKILL_CATEGORY2: &[&str] = &[
    "에스닉",
    "다크",
    "섹시",
    "페미닌",
    "큐트",
    "플레인",
    "패션",
    "팝",
    "버닝",
    "쿨",
    "스타",
];

static KO_SKILL_CATEGORY3: &[&str] = &[
    "이국문화",
    "스타일",
    "집중력",
    "담력",
    "체력",
    "미소",
    "운동신경",
    "배려",
    "학력",
    "기품",
    "연기력",
];

static KO_SKILL_CATEGORY4: &[&str] = &[
    "중2병",
    "신비",
    "마이페이스",
    "유순",
    "말버릇",
    "캐릭터분야의 공백",
    "건강",
    "외고집",
    "언행정중",
    "덜렁이",
    "바보",
];

static KO_SKILL_CATEGORY5: &[&str] = &[
    "오컬트",
    "펫",
    "스포츠",
    "멋내기",
    "요리",
    "취미분야의 공백",
    "쇼핑",
    "댄스",
    "ゲーム",
    "음악",
    "아이돌",
];

static KO_SKILL_CATEGORY6: &[&str] = &[
    "오키나와",
    "큐슈",
    "시코쿠",
    "주코쿠",
    "킨키",
    "주부",
    "간토",
    "호쿠리쿠",
    "도호쿠",
    "훗카이도",
    "해외",
];

static KO_SKILL_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("신장", KO_SKILL_CATEGORY1),
    SaiFicCategory::new("속성", KO_SKILL_CATEGORY2),
    SaiFicCategory::new("재능", KO_SKILL_CATEGORY3),
    SaiFicCategory::new("캐릭터", KO_SKILL_CATEGORY4),
    SaiFicCategory::new("취미", KO_SKILL_CATEGORY5),
    SaiFicCategory::new("출신", KO_SKILL_CATEGORY6),
];

/// Ruby `translate_skill_table(:ko_kr)`。`rtt:`/`rttn:` は `AT`/`AT1`〜`AT6`。
static KO_SKILL_TABLE: SkillTable = SkillTable::new(
    SaiFicSkillTable::new(KO_SKILL_CATEGORIES)
        .with_commands(
            Some("AT"),
            None,
            &["AT1", "AT2", "AT3", "AT4", "AT5", "AT6"],
        )
        .with_formats(SaiFicFormats {
            rtt: "랜덤 특기 결정표(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
            rct: "랜덤 분야표(%<category_dice>d) ＞ %<category_name>s",
            rttn: "지정특기(%<category_name>s)표(%<row_dice>d) ＞ %<text>s",
            skill: DEFAULT_SKILL_FORMAT,
        }),
);

static KO_ITEM_TABLE_ITEMS: &[&str] = &[
    "スタミナドリンク",
    "トレーニングウェア",
    "ドリーミングシューズ",
    "キャラアイテム",
    "お菓子",
    "差し入れ",
];

/// Ruby `ItemTable.new(:ko_kr)`。
static KO_ITEM_TABLE: ItemTable = ItemTable::new(
    "アイテム",
    "「%{item}」",
    "%{item}%{count}つ",
    "と",
    KO_ITEM_TABLE_ITEMS,
);

static KO_BAD_STATUS_ITEMS: &[&str] = &[
    "「불온한 공기」　PC의 【멘탈】이 감소할 때, 감소하는 수치가 1점 상승한다.",
    "「미묘한 거리감」　【이해도】가 상승하지 않게 된다.",
    "「유리의 마음」　PC의 펌블치가 1점 상승한다.",
    "「부상」　막간 때, 프로듀서는 「회상」만 실시할 수 있다.",
    "「믿지 못하는」　PC전원의 【이해도】를 1점 낮은 것으로 취급한다.",
    "「엇갈림」　PC는 아이템 사용과 리절트 페이즈에 「부탁」을 할 수 없게 된다.",
];

/// Ruby `BadStatusTable.new(:ko_kr)`。
static KO_BAD_STATUS_TABLE: BadStatusTable = BadStatusTable::new(
    "변조",
    "以下の%{count_bad_status}つが発生する。",
    KO_BAD_STATUS_ITEMS,
);

static KO_LOCAL_WORK_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("오프")),
    (13, TableItem::Text("握手会をすることになった。遠方から自分たち目当てにやって来るお客さんも多数見える。チャンスが5以下なら오프。\n特技 : 《아이돌／취미12》")),
    (14, TableItem::Text("ミニコンサートが全国放送で小さく紹介される。ちょっとだけ、外見が注目されたみたいだ。チャンスが4以下なら오프。\n特技 : 《스타일／재능3》")),
    (15, TableItem::Text("地元ラジオ局で自分たちの番組が始まる。チャンスが3以下なら오프。\n特技 : 《캐릭터분야의 공백／취미7》")),
    (16, TableItem::Text("地元のテレビ局にゲスト出演。うまく自分たちを紹介できるだろうか？　チャンスが3以下なら오프。\n特技 : 好きな출신분야의특기")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("街頭でティッシュ配りの手伝いをする。笑顔を忘れずに、がんばろう。\n特技 : 《미소／재능7》")),
    (24, TableItem::Text("地元のお手伝いの一環として、害虫退治に駆り出された。なぜ、こんなことに。\n特技 : 《담력／재능5》")),
    (25, TableItem::Text("畑仕事のお手伝いをすることになった。とりあえず、体力が要求される。\n特技 : 《체력／재능6》")),
    (26, TableItem::Text("ショッピングモールのお手伝いをすることになった。うまくお手伝いができれば、繁盛するかも。\n特技 : 《쇼핑／취미8》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("インターネットラジオに出演。声とトークで。地域のことを伝えていこう。\n特技 : 《이국문화／재능2》")),
    (35, TableItem::Text("地元のテレビ局の取材が入る。テーマは、地方でがんばっている人たちだ。\n特技 : 《건강／캐릭터8》")),
    (36, TableItem::Text("デパートで風船を配るお手伝い。子どもたち相手のお仕事は、ちょっと大変です。\n特技 : 《배려／재능9》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("着ぐるみを着て、市民と交流。暑くてつらい仕事だけど、大切な交流の時間です。\n特技 : 《버닝／속성10》")),
    (46, TableItem::Text("観光地の物販コーナーで地元の特産品を売るお手伝い。로컬 아이돌的的に、大切なお仕事。\n特技 : 好きな출신분야의특기")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("動画サイトのチャンネルで、自分たちの宣伝を行なうことに。世界中に発信！\n特技 : 《스타／속성12》")),
    (66, TableItem::Text("오프")),
];

/// Ruby `translate_local_work_table(:ko_kr)`（`LO[n]`）。
static KO_LOCAL_WORK_TABLE: WorkWithChanceTable = WorkWithChanceTable::new(
    D66Table::new("로컬 아이돌 업무표", D66SortType::Asc, KO_LOCAL_WORK_ITEMS),
    "チャンスが(\\d{1,2})以下なら오프。",
    "오프",
);

static KO_ABNORMALITY_NUM_MAP: &[&str] = &["一", "二", "三"];

/// Ruby `WithAbnormality`（`変調がランダムにN つ発生する。` の置換）。
static KO_ABNORMALITY: Abnormality = Abnormality::new(
    "変調がランダムに(一|二|三)つ発生する。",
    KO_ABNORMALITY_NUM_MAP,
    &KO_BAD_STATUS_TABLE,
);

static KO_DT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("12&88\n자신의 【퍼포먼스치】가 결정되었을 때, 그 값을 2점 상승한다.")),
    (12, TableItem::Text("Glow Up Princess\n퍼포먼스를 할 때 주사위를 추가로 1개 더 굴린다.")),
    (13, TableItem::Text("시즈쿠\n라이브페이즈 개시 시 【멘탈】이 5점 상승한다.")),
    (14, TableItem::Text("Pop☆Sweet\n자신의 【멘탈】이 상승할 때 추가로 1점 더 상승한다.")),
    (15, TableItem::Text("Ttype\n단기돌파 시 【멘탈】이 감소하지 않는다. 또한 단기돌파를 할 때 달성치가 1점 상승한다.")),
    (16, TableItem::Text("Vampire Story\n퍼포먼스의 【퍼포먼스치】가 10이상일 경우, 자신의 【멘탈】이 3점 상승한다.")),
    (22, TableItem::Text("Pure Mermaid\n【비주얼】 공연 중 지정 특기를 《스타일》로 변경할 수 있다. 지정 특기가 《스타일》인 공연에서는 【퍼포먼스치】가 2점 상승한다.")),
    (23, TableItem::Text("I'm cute\n라이브 페이즈 개시 시 【멘탈】이 1점 상승한다. 막간 개시 시 능력치를 1개 선택한다. 선택된 능력치는 이 라이브 페이즈 간에 1점 상승한다.")),
    (24, TableItem::Text("No.1 Girl\n【퍼포먼스치】가 결정될 때 【멘탈】을 1점 감소시키고 【퍼포먼스치】가 3점 상승한다.")),
    (25, TableItem::Text("Final Romance\n【비주얼】 퍼포먼스를 실시할 때 캐릭터를 1인 선택한다. 선택한 캐릭터의 자신에 대한 【이해도】와 같은 【퍼포먼스치】가 상승한다.")),
    (26, TableItem::Text("Prism Line\n퍼포먼스 1회에 1번만 퍼포먼스에 사용한 주사위 1개를 다시 굴릴 수 있다.")),
    (33, TableItem::Text("서번트 서비스\n심포니를 할 때마다 그 퍼포먼스 스의 【퍼포먼스치】가 3점 상승한다.")),
    (34, TableItem::Text("Travel Bag\n막간에 자신의 【이해도】 체크 1개를 해제할 수 있다.")),
    (35, TableItem::Text("JewelC\n개막공연과 막간에 아이템을 1개 선택해 획득한다.")),
    (36, TableItem::Text("Sweet Girl\n퍼포먼스를 실시한 PC는 【멘탈】이 2점 상승한다.")),
    (44, TableItem::Text("Satisfaction West\n미라클, 미라클 싱크로, 퍼펙트 미라클이 발생했을 때 【퍼포먼스치】가 5점 상승한다.")),
    (45, TableItem::Text("Under Big Ben\n사용 능력이 【보이스】인 퍼포먼스의 【퍼포먼스치】가 10이상인 경우 자신에 대한 【이해도】 체크 1개를 해제할 수 있다.")),
    (46, TableItem::Text("PIERO\n단기돌파의 달성치가 2점 상승한다.")),
    (55, TableItem::Text("캉캉냥냥\n사용 능력이 【비주얼】인 퍼포먼스를 실시 할 때 【퍼포먼스치】가 3점 상승한다.")),
    (56, TableItem::Text("화조풍월\n심포니를 실시할 때 굴리는 주사위의 수를 1개 늘린다, 혹은 1개 줄일 수 있다.")),
    (66, TableItem::Text("Jingle Bells\n리절트페이즈에서 이하의 효과가 발생한다. 리절트페이즈에서 【획득 팬 인원수】가 1D6점 상승한다. 또 PC전원은 조건을 채우지 않아도 「부탁」을 할 수 있다.")),
];

/// Ruby `CostumeTable` (`DT`)。
static KO_DT: D66Table = D66Table::new("의상(챌린지 걸즈)", D66SortType::Asc, KO_DT_ITEMS);

static KO_DT_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("12&88")),
    (12, TableItem::Text("Glow Up Princess")),
    (13, TableItem::Text("시즈쿠")),
    (14, TableItem::Text("Pop☆Sweet")),
    (15, TableItem::Text("Ttype")),
    (16, TableItem::Text("Vampire Story")),
    (22, TableItem::Text("Pure Mermaid")),
    (23, TableItem::Text("I'm cute")),
    (24, TableItem::Text("No.1 Girl")),
    (25, TableItem::Text("Final Romance")),
    (26, TableItem::Text("Prism Line")),
    (33, TableItem::Text("서번트 서비스")),
    (34, TableItem::Text("Travel Bag")),
    (35, TableItem::Text("JewelC")),
    (36, TableItem::Text("Sweet Girl")),
    (44, TableItem::Text("Satisfaction West")),
    (45, TableItem::Text("Under Big Ben")),
    (46, TableItem::Text("PIERO")),
    (55, TableItem::Text("캉캉냥냥")),
    (56, TableItem::Text("화조풍월")),
    (66, TableItem::Text("Jingle Bells")),
];

/// Ruby `CostumeTable#brand_only` (`DT`)。
static KO_DT_BRAND: D66Table =
    D66Table::new("의상(챌린지 걸즈)", D66SortType::Asc, KO_DT_BRAND_ITEMS);

static KO_RC_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("Angel kiss\n퍼포먼스를 할 때 1의 눈이 나온 주사위는 제거되지 않는다. 심포니를 실시했을 때, 1의 눈이 나온 주사위는 제거한다.")),
    (12, TableItem::Text("Pirate ship\n공연목록에서 지정특기가 속성 분야일 경우 그 지정특기를 《섹시/속성4》로 변경할 수 있다.")),
    (13, TableItem::Text("로드 투 프린스\n미러클 ・ 미라클 싱크로 ・ 퍼펙트 미라클・ 퍼펙트 미라클 싱크로가 발생했을 때, 그 캐릭터는 【멘탈】이 10점 상승한다.")),
    (14, TableItem::Text("Princess Guardian\n자신 이외의 캐릭터가 【멘탈】이 o점이 되었을 때, 《배려/재능9》로 판정을 한다. 이 판정에 개성특기는 사용할 수 없다. 성공한다면 그 캐릭터는 [획득 팬 인원수]가 반감 되지 않는다.")),
    (15, TableItem::Text("Starlight TourS\n라이브 페이즈 간에, 공연 목록을 1개 선택하고, 지정특기를 《스타/속성12》로 변경할 수 있다.")),
    (16, TableItem::Text("화조풍월•속편\n라이프 페이즈 중, 한 번 남아있는 모든 주사위의 눈을 재반전(1이라면 4, 2라면 5, 3이라면 6으로)할 수 있다.")),
    (22, TableItem::Text("시쿠라마\n판정에 사용한 주사위의 값이 7인 경우 【멘탈】이 7점 상승한다.")),
    (23, TableItem::Text("Chime\n미러클 ・ 미라클 싱크로 ・ 퍼펙트 미라클・ 퍼펙트 미라클 싱크로가 발생했을 때, 그 캐릭터는 랜덤으로 아이템 1개를 얻는다.")),
    (24, TableItem::Text("사상지광\n심포니를 할 때 심포니를 받은 캐릭터의 【멘탈】이 5점상승한다.")),
    (25, TableItem::Text("Air by me\n막간 개시 시 【멘탈】이 5점 상승한다.")),
    (26, TableItem::Text("전국 스트리트\n공연목록의 사용 능력이 【피지컬】이면 【퍼포먼스치】가 2점 상승한다. 또한 지정 특기가 《댄스/취미 9》일 경우, 【퍼포먼스치】가 2점 상승한다.")),
    (33, TableItem::Text("Wild man\n단기돌파의 달성치가 2점 상승한다. 다만, 스페셜은 발생하지 않는다.")),
    (34, TableItem::Text("Gray Stand\n【획득 팬 인원수】가 감소했을 때, 감소한 값의 반(소수점이하 버림)과 같은 값의 【획득 팬 인원수】가 상승한다.")),
    (35, TableItem::Text("토이ARM\n공연을 개시할 때, 2D6을 굴린다. 그 결과가 11이상이면, 이 공연에서는 【멘탈】이 감소하지 않는다,")),
    (36, TableItem::Text("white plan\n펌블이 발생해도 변조를 받지 않는다.。")),
    (44, TableItem::Text("SINOBI\n개막 공연을 실시할 때, 나오지 않는 의상을 선택할 수 있다..")),
    (45, TableItem::Text("V-X\n미라클이 발생하면 퍼포먼스치를 15로 할 수 있다.")),
    (46, TableItem::Text("드래곤 너클\n막간 후에, PC가 행하는 퍼포먼스의 【퍼포먼스치】가 4점 상승한다.")),
    (55, TableItem::Text("Halloween Magic\n후반PP로 【멘탈】이 감소할 때, 그 값을 4점 감소시킨다. (최저 0)")),
    (56, TableItem::Text("Satisfaction East\n【획득 팬 인원수】가 감소하게 되면 【멘탈】을 20점으로 할 수 있다.")),
    (66, TableItem::Text("Devil kiss\n퍼포먼스를 할 때 6의 눈이 나온 주사위는 제거되지 않는다. 심포니를 실시했을 때, 6의 눈이 나온 주사위는 제거한다.")),
];

/// Ruby `CostumeTable` (`RC`)。
static KO_RC: D66Table = D66Table::new("의상(로드 투 프린스)", D66SortType::Asc, KO_RC_ITEMS);

static KO_RC_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("Angel kiss")),
    (12, TableItem::Text("Pirate ship")),
    (13, TableItem::Text("로드 투 프린스")),
    (14, TableItem::Text("Princess Guardian")),
    (15, TableItem::Text("Starlight TourS")),
    (16, TableItem::Text("화조풍월•속편")),
    (22, TableItem::Text("시쿠라마")),
    (23, TableItem::Text("Chime")),
    (24, TableItem::Text("사상지광")),
    (25, TableItem::Text("Air by me")),
    (26, TableItem::Text("전국 스트리트")),
    (33, TableItem::Text("Wild man")),
    (34, TableItem::Text("Gray Stand")),
    (35, TableItem::Text("토이ARM")),
    (36, TableItem::Text("white plan")),
    (44, TableItem::Text("SINOBI")),
    (45, TableItem::Text("V-X")),
    (46, TableItem::Text("드래곤 너클")),
    (55, TableItem::Text("Halloween Magic")),
    (56, TableItem::Text("Satisfaction East")),
    (66, TableItem::Text("Devil kiss")),
];

/// Ruby `CostumeTable#brand_only` (`RC`)。
static KO_RC_BRAND: D66Table =
    D66Table::new("의상(로드 투 프린스)", D66SortType::Asc, KO_RC_BRAND_ITEMS);

static KO_FC_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("常峰製作所\n第一演目では、【メンタル】が減少しない。")),
    (12, TableItem::Text("フォーチュンスター\n最終演目の【パフォーマンス値】が「【メンタル】÷2（端数切り捨て）」点上昇する。")),
    (13, TableItem::Text("ファイタースケイル\n【メンタル】が5点以下の場合、【パフォーマンス値】が1D6点上昇する。")),
    (14, TableItem::Text("Blood Scissors\n自分以外のキャラクター一人の【メンタル】を5点減少するか、プロデューサーに変調「怪我」を与えることで、自分の【メンタル】が5点上昇する。この効果は、プロデューサーが既に「怪我」の変調を受けていると、使用できない。")),
    (15, TableItem::Text("蒸気式演技服\n判定を行うとき、【メンタル】を1点消費することで、判定の達成値が1点上昇する。")),
    (16, TableItem::Text("ウェイトスター\n「スタミナドリンク」によって、他のキャラクターの【メンタル】を上昇する場合、さらに4点上昇する。")),
    (22, TableItem::Text("Little Stage\n判定のサイコロやパフォーマンスで「1」の出目が1つ以上出た場合、【思い出】を1つ獲得する。")),
    (23, TableItem::Text("Check It\n開幕演目前に、最終演目以外の好きな演目を指定する。指定された演目では、自分の【メンタル】が減少しない。")),
    (24, TableItem::Text("12 Sword\nアイドル戦闘ルールを使用しているとき、与えるダメージが3点上昇し、上昇する【獲得ファン人数】が5点上昇する。")),
    (25, TableItem::Text("Magi Magic\nパフォーマンスや自分が行うシンフォニーでサイコロを取り除くたびに、【メンタル】が2点上昇する。")),
    (26, TableItem::Text("Jokers\n最終演目に行う一芸突破の目標値が3点減少する。")),
    (33, TableItem::Text("Papillon Club\n自分以外のキャラクターがタイプが「補助」のアイドルスキルを使用するたびに、【メンタル】が3点上昇する。")),
    (34, TableItem::Text("ネイキッドチャレンジ\n開幕演目開始時に、【メンタル】が5点減少する。このライブフェイズの間、好きな能力値が3点上昇する。")),
    (35, TableItem::Text("Cold Vivit\n好きなギャップを1つ埋める。このギャップは、ライブフェイズ終了時に元に戻る。")),
    (36, TableItem::Text("対魔絶伏\n特別な演目では、【メンタル】が減少しない。")),
    (44, TableItem::Text("Rescue Power\n演目の判定でファンブルが発生した場合、好きな能力値でパフォーマンスを行うことができる。")),
    (45, TableItem::Text("アニマルエンジン\n幕間の終了時に、好きな動物からの【理解度】が2点上昇する。")),
    (46, TableItem::Text("ふわふわキッチン\n好きなときに、「お菓子」を一つ消費することで、好きなキャラクターの【メンタル】が1D6点上昇できる。また、幕間に「お菓子」を1つ獲得する。")),
    (55, TableItem::Text("Night Talk\n幕間で「信用」を行ったとき、上昇する【メンタル】が10点になる。")),
    (56, TableItem::Text("ティーチングタイム\n自分以外のキャラクターを1人指定する。このライブフェイズの間、指定されたPCの能力値が1点上昇する。")),
    (66, TableItem::Text("See Diver\n演目名に「海」「水」「泡」「湖」「風呂」を含む演目を行った場合、【獲得ファン人数】が1D6点上昇する。")),
];

/// Ruby `CostumeTable` (`FC`)。
static KO_FC: D66Table = D66Table::new("의상(フォーチュンスターズ)", D66SortType::Asc, KO_FC_ITEMS);

static KO_FC_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("常峰製作所")),
    (12, TableItem::Text("フォーチュンスター")),
    (13, TableItem::Text("ファイタースケイル")),
    (14, TableItem::Text("Blood Scissors")),
    (15, TableItem::Text("蒸気式演技服")),
    (16, TableItem::Text("ウェイトスター")),
    (22, TableItem::Text("Little Stage")),
    (23, TableItem::Text("Check It")),
    (24, TableItem::Text("12 Sword")),
    (25, TableItem::Text("Magi Magic")),
    (26, TableItem::Text("Jokers")),
    (33, TableItem::Text("Papillon Club")),
    (34, TableItem::Text("ネイキッドチャレンジ")),
    (35, TableItem::Text("Cold Vivit")),
    (36, TableItem::Text("対魔絶伏")),
    (44, TableItem::Text("Rescue Power")),
    (45, TableItem::Text("アニマルエンジン")),
    (46, TableItem::Text("ふわふわキッチン")),
    (55, TableItem::Text("Night Talk")),
    (56, TableItem::Text("ティーチングタイム")),
    (66, TableItem::Text("See Diver")),
];

/// Ruby `CostumeTable#brand_only` (`FC`)。
static KO_FC_BRAND: D66Table = D66Table::new(
    "의상(フォーチュンスターズ)",
    D66SortType::Asc,
    KO_FC_BRAND_ITEMS,
);

/// Ruby `bland`（アクセサリーブランド決定表 `ACB`）。
static KO_ACB: BiChainTable = BiChainTable::new(
    "アクセサリーブランド決定表",
    1,
    6,
    &[
        &[
            Node::Text("『챌린지 걸즈』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_DT_BRAND),
        ],
        &[
            Node::Text("『챌린지 걸즈』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_DT_BRAND),
        ],
        &[
            Node::Text("『로드 투 프린스』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_RC_BRAND),
        ],
        &[
            Node::Text("『로드 투 프린스』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_RC_BRAND),
        ],
        &[
            Node::Text("『フォーチュンスターズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_FC_BRAND),
        ],
        &[
            Node::Text("『フォーチュンスターズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&KO_FC_BRAND),
        ],
    ],
);

static KO_RARE_SKILL_TABLE_ITEMS: &[&str] = &[
    "【秘めたる素質】を修得する。",
    "【王者の風格】を修得する。",
    "【万能アイドル】を修得する。",
    "【最強の負けず嫌い】を修得する。",
    "【超絶無敵コーデ】を修得する。",
    "【強く正しく美しく】を修得する。",
];

/// Ruby `rare_skill_table`（`GG` の 23/24/25 から連鎖）。
static KO_RARE_SKILL_TABLE: Table =
    Table::from_dice("レアアイドルスキル修得表", 1, 6, KO_RARE_SKILL_TABLE_ITEMS);

/// Ruby `tn`（夜語りシチュエーション表 `TN`。4番目に特技表が連鎖する）。
static KO_TN: BiChainTable = BiChainTable::new(
    "夜語りシチュエーション表",
    1,
    6,
    &[
        &[Node::Text("みんなが寝静まった寝室。二人だけのお話。"), Node::Text("特技 : 好きな特技")],
        &[Node::Text("夜の街を歩きながら、【背景】をぽつぽつと語り出す。"), Node::Text("特技 : 씬 플레이어의個性特技")],
        &[Node::Text("「好きなもの」を探しに出かけた帰り道。"), Node::Text("特技 : 씬 플레이어가 보유한 취미분야의특기")],
        &[Node::Text("「嫌いなもの」から逃げてきて、二人きりになってしまった。"), Node::Table(&KO_SKILL_TABLE)],
        &[Node::Text("暗い道を往くとき、ふとしたきっかけで、「身体的特徴」に触れてしまう。"), Node::Text("特技 : 씬 플레이어가 보유한 신장분야의특기")],
        &[Node::Text("「ファッション特徴」の話に夢中になっていたら、いつの間にか二人になっていたことに気づく。"), Node::Text("特技 : 씬 플레이어가 보유한 속성분야의특기")],
    ],
);

/// Ruby `cg`（コモン成長表 `CG`。4・5番目にアイテム表が連鎖する）。
static KO_CG: BiChainTable = BiChainTable::new(
    "コモン成長表",
    1,
    6,
    &[
        &[Node::Text("【メンタル】が2点上昇する。")],
        &[Node::Text("【メンタル】が4点上昇する。")],
        &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")],
        &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&KO_ITEM_TABLE)],
        &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&KO_ITEM_TABLE)],
        &[Node::Text("【獲得ファン人数】が3点上昇する。")],
    ],
);

/// Ruby `gg`（ゴールド成長表 `GG`。23/24/25 はレアアイドルスキル、56 はアイテムが連鎖する）。
static KO_GG: BiChainD66Table = BiChainD66Table::new(
    "ゴールド成長表",
    &[
        (11, &[Node::Text("好きなアイドルスキルを一つ選んで修得する。")]),
        (12, &[Node::Text("『챌린지 걸즈』か『로드 투 프린스』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (13, &[Node::Text("『챌린지 걸즈』か『로드 투 프린스』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (14, &[Node::Text("『챌린지 걸즈』か『로드 투 프린스』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (15, &[Node::Text("『챌린지 걸즈』か『로드 투 프린스』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (16, &[Node::Text("『챌린지 걸즈』か『로드 투 프린스』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (22, &[Node::Text("好きなアイドルスキルを一つ選んで修得する。")]),
        (23, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&KO_RARE_SKILL_TABLE)]),
        (24, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&KO_RARE_SKILL_TABLE)]),
        (25, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&KO_RARE_SKILL_TABLE)]),
        (26, &[Node::Text("好きな能力値一つが1点上昇する。")]),
        (33, &[Node::Text("好きな能力値一つが2点上昇する。")]),
        (34, &[Node::Text("【ボイス】が1点上昇する。")]),
        (35, &[Node::Text("【フィジカル】が1点上昇する。")]),
        (36, &[Node::Text("【ビジュアル】が1点上昇する。")]),
        (44, &[Node::Text("個性特技を別の特技に変更することができる。")]),
        (45, &[Node::Text("好きな能力値二つが1点上昇する。")]),
        (46, &[Node::Text("すべての能力値が1点上昇する。")]),
        (55, &[Node::Text("【メンタル】が10点上昇する。")]),
        (56, &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&KO_ITEM_TABLE)]),
        (66, &[Node::Text("個性特技の目標値が1点減少する。")]),
    ],
);

/// Ruby `SkillHometown`（`HA` の 22 から出身分野の特技表を引く）。
static KO_SKILL_HOMETOWN: SkillHometown = SkillHometown::new(&KO_SKILL_TABLE);

/// Ruby `ha`（ハプニング表 `HA`。22 に出身分野の特技表が連鎖する）。
static KO_HA: BiChainD66Table = BiChainD66Table::new(
    "ハプニング表",
    &[
        (11, &[Node::Text("ハプニングなし")]),
        (12, &[Node::Text("ハプニングなし")]),
        (13, &[Node::Text("ハプニングなし")]),
        (14, &[Node::Text("ハプニングなし")]),
        (15, &[Node::Text("ハプニングなし")]),
        (16, &[Node::Text("ハプニングなし")]),
        (22, &[Node::Text("パートナープレイヤーに、地方からオファーが来た。その土地独特の文化を学んで、パートナープレイヤーに伝えよう。"), Node::Table(&KO_SKILL_HOMETOWN)]),
        (23, &[Node::Text("グラビア撮影だが、用意された衣装のサイズがパートナープレイヤーに合わなかった。何とかして、衣装を合わせなければいけない。"), Node::Text("特技 : パートナープレイヤーが修得している身長分野の特技")]),
        (24, &[Node::Text("ダンス撮影中。パートナープレイヤーのダンスに迷いが見えた。何かアドバイスをして、迷いを取り払いたい。"), Node::Text("特技 : 《ダンス／趣味9》")]),
        (25, &[Node::Text("歌の仕事だが、パートナープレイヤーの歌がどこかぎこちない。うまく本来の歌を取り戻させよう。"), Node::Text("特技 : パートナープレイヤーが修得している属性分野の特技")]),
        (26, &[Node::Text("体力を消費する仕事の最中に、パートナープレイヤーが倒れてしまった！　急いで処置をしなければ！"), Node::Text("特技 : 《気配り／才能9》")]),
        (33, &[Node::Text("パートナープレイヤーにマイナースポーツのCMが回ってきたが、知らない様子だ。ルールを教えよう。"), Node::Text("特技 : 《スポーツ／趣味4》")]),
        (34, &[Node::Text("パートナープレイヤーのキャラに合わない仕事が舞い込んだ。演技力で乗り切ってほしい。"), Node::Text("特技 : 《演技力／才能12》")]),
        (35, &[Node::Text("パートナープレイヤーが風邪をひいてしまう。次の仕事までに、なんとか治してもらわなければ。"), Node::Text("特技 : 《元気／キャラ8》")]),
        (36, &[Node::Text("パートナープレイヤーの属性らしくない衣装が来てしまった。うまくアレンジできればいいけど。"), Node::Text("特技 : 《おしゃれ／趣味5》")]),
        (44, &[Node::Text("パートナープレイヤーのテンションが低い。テンションを上げるようなことを言おう。"), Node::Text("特技 : 《バーニング／属性10》")]),
        (45, &[Node::Text("パートナープレイヤーの仕事に必要な小道具が足りなくなった。調達しよう。"), Node::Text("特技 : 《ショッピング／趣味8》")]),
        (46, &[Node::Text("パートナープレイヤーに外国から仕事が舞い込んできた。外国の文化に合わせた仕事をしなければ。"), Node::Text("特技 : 《異国文化／才能2》")]),
        (55, &[Node::Text("パートナープレイヤーに大会社からの仕事のオファーがやって来る。プレッシャーに負けないように後押ししよう。"), Node::Text("特技 : 《胆力／才能5》")]),
        (56, &[Node::Text("パートナープレイヤーと他のアイドルグループとのコラボイベントが行われる。そのアイドルの情報を集めてこよう。"), Node::Text("特技 : 《アイドル／趣味12》")]),
        (66, &[Node::Text("パートナープレイヤーの周りで、幽霊騒ぎが起こる。安心させるためにも、調査に乗り出そう。"), Node::Text("特技 : 《オカルト／趣味2》")]),
    ],
);

static KO_ACT_HEAD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アイマスク")),
    (12, TableItem::Text("うさみみ")),
    (13, TableItem::Text("ねこみみ")),
    (14, TableItem::Text("サングラス")),
    (15, TableItem::Text("ピアス")),
    (16, TableItem::Text("シュシュ")),
    (22, TableItem::Text("仮面")),
    (23, TableItem::Text("ティアラ")),
    (24, TableItem::Text("エクステ")),
    (25, TableItem::Text("バンダナ")),
    (26, TableItem::Text("ヘアバンド")),
    (33, TableItem::Text("インカム")),
    (34, TableItem::Text("イヤリング")),
    (35, TableItem::Text("ホワイトプリム")),
    (36, TableItem::Text("ゴーグル")),
    (44, TableItem::Text("つけひげ")),
    (45, TableItem::Text("ヘッドホン")),
    (46, TableItem::Text("耳あて")),
    (55, TableItem::Text("トナカイの角")),
    (56, TableItem::Text("花飾り")),
    (66, TableItem::Text("かんざし")),
];

static KO_ACT_HEAD: D66Table =
    D66Table::new("頭アクセサリー表", D66SortType::Asc, KO_ACT_HEAD_ITEMS);

static KO_ACT_HAT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヘルメット")),
    (12, TableItem::Text("麦わら帽子")),
    (13, TableItem::Text("サンタ帽子")),
    (14, TableItem::Text("花冠")),
    (15, TableItem::Text("学帽")),
    (16, TableItem::Text("ハンチング帽")),
    (22, TableItem::Text("シルクハット")),
    (23, TableItem::Text("テンガロンハット")),
    (24, TableItem::Text("ナイトキャップ")),
    (25, TableItem::Text("ロシア帽")),
    (26, TableItem::Text("ベレー帽")),
    (33, TableItem::Text("コック帽")),
    (34, TableItem::Text("パーティコーン")),
    (35, TableItem::Text("とんがり帽子")),
    (36, TableItem::Text("キャップ")),
    (44, TableItem::Text("ナースキャップ")),
    (45, TableItem::Text("カンカン帽")),
    (46, TableItem::Text("ハット帽")),
    (55, TableItem::Text("ターバン")),
    (56, TableItem::Text("セーラーキャップ")),
    (66, TableItem::Text("中共帽子")),
];

static KO_ACT_HAT: D66Table =
    D66Table::new("帽子アクセサリー表", D66SortType::Asc, KO_ACT_HAT_ITEMS);

static KO_ACT_BODY_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("首かけ指輪")),
    (12, TableItem::Text("カウベル")),
    (13, TableItem::Text("ネックレス")),
    (14, TableItem::Text("蝶ネクタイ")),
    (15, TableItem::Text("メガホン")),
    (16, TableItem::Text("ペンダント")),
    (22, TableItem::Text("ブローチ")),
    (23, TableItem::Text("金の首輪")),
    (24, TableItem::Text("チョーカー")),
    (25, TableItem::Text("南京錠")),
    (26, TableItem::Text("タスキ")),
    (33, TableItem::Text("エプロン")),
    (34, TableItem::Text("名札")),
    (35, TableItem::Text("階級章")),
    (36, TableItem::Text("胸当て")),
    (44, TableItem::Text("ベルト")),
    (45, TableItem::Text("ポシェット")),
    (46, TableItem::Text("マフラー")),
    (55, TableItem::Text("首かけカメラ")),
    (56, TableItem::Text("リボン")),
    (66, TableItem::Text("肩パット")),
];

static KO_ACT_BODY: D66Table =
    D66Table::new("胴アクセサリー表", D66SortType::Asc, KO_ACT_BODY_ITEMS);

static KO_ACT_ARM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("動物の手")),
    (12, TableItem::Text("宝石の腕輪")),
    (13, TableItem::Text("動物のマペット")),
    (14, TableItem::Text("グローブ")),
    (15, TableItem::Text("指ぬきグローブ")),
    (16, TableItem::Text("リストバンド")),
    (22, TableItem::Text("鍋掴み")),
    (23, TableItem::Text("手袋")),
    (24, TableItem::Text("長手袋")),
    (25, TableItem::Text("腕章")),
    (26, TableItem::Text("腕時計")),
    (33, TableItem::Text("ドリル")),
    (34, TableItem::Text("楽器")),
    (35, TableItem::Text("マフ")),
    (36, TableItem::Text("カフス")),
    (44, TableItem::Text("ボクシンググローブ")),
    (45, TableItem::Text("シルバーアクセサリー")),
    (46, TableItem::Text("ゴールドアクセサリー")),
    (55, TableItem::Text("ぬいぐるみ")),
    (56, TableItem::Text("ミサンガ")),
    (66, TableItem::Text("手甲")),
];

static KO_ACT_ARM: D66Table = D66Table::new("腕アクセサリー表", D66SortType::Asc, KO_ACT_ARM_ITEMS);

static KO_ACT_FOOT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("革靴")),
    (12, TableItem::Text("ブーツ")),
    (13, TableItem::Text("スポーツシューズ")),
    (14, TableItem::Text("スキー靴")),
    (15, TableItem::Text("アンクル")),
    (16, TableItem::Text("バスケットシューズ")),
    (22, TableItem::Text("スリッパ")),
    (23, TableItem::Text("ミサンガ")),
    (24, TableItem::Text("動物の足")),
    (25, TableItem::Text("作業靴")),
    (26, TableItem::Text("ルーズウォーマー")),
    (33, TableItem::Text("ニーパッド")),
    (34, TableItem::Text("ガーターリング")),
    (35, TableItem::Text("ポーチ")),
    (36, TableItem::Text("ローラースケート")),
    (44, TableItem::Text("へんなタイツ")),
    (45, TableItem::Text("白タイツ")),
    (46, TableItem::Text("網タイツ")),
    (55, TableItem::Text("ガラスの靴")),
    (56, TableItem::Text("グリープ")),
    (66, TableItem::Text("ベル")),
];

static KO_ACT_FOOT: D66Table =
    D66Table::new("足アクセサリー表", D66SortType::Asc, KO_ACT_FOOT_ITEMS);

static KO_ACT_OTHER_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ボンボン")),
    (12, TableItem::Text("マント")),
    (13, TableItem::Text("蝶の羽")),
    (14, TableItem::Text("天使の羽")),
    (15, TableItem::Text("悪魔の羽")),
    (16, TableItem::Text("猫のしっぽ")),
    (22, TableItem::Text("トレンチコート")),
    (23, TableItem::Text("ばんそうこう")),
    (24, TableItem::Text("パラソル")),
    (25, TableItem::Text("ステッキ")),
    (26, TableItem::Text("タトゥーシール")),
    (33, TableItem::Text("バーコード")),
    (34, TableItem::Text("バレーボール")),
    (35, TableItem::Text("大きなリボン")),
    (36, TableItem::Text("鎖")),
    (44, TableItem::Text("キャラクターグッズ")),
    (45, TableItem::Text("イメージカラーのネイル")),
    (46, TableItem::Text("メガネ")),
    (55, TableItem::Text("旗")),
    (56, TableItem::Text("ジャケット")),
    (66, TableItem::Text("サスペンダー")),
];

static KO_ACT_OTHER: D66Table =
    D66Table::new("その他アクセサリー表", D66SortType::Asc, KO_ACT_OTHER_ITEMS);

/// Ruby `translate_accessories_table`（`ACT`）。
static KO_ACT: BiChainTable = BiChainTable::new(
    "アクセサリー種別決定表",
    1,
    6,
    &[
        &[
            Node::Text("頭アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_HEAD),
        ],
        &[
            Node::Text("帽子アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_HAT),
        ],
        &[
            Node::Text("胴アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_BODY),
        ],
        &[
            Node::Text("腕アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_ARM),
        ],
        &[
            Node::Text("足アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_FOOT),
        ],
        &[
            Node::Text("その他アクセサリー表を使用する。"),
            Node::Table(&KO_ACT_OTHER),
        ],
    ],
);

static KO_MS_ARTICLE_ITEMS: &[&str] = &[
    "アイドル",
    "ガール／ボーイ",
    "パラダイス",
    "プリンセス／プリンス",
    "スタイル",
    "クイーン／キング",
];

static KO_MS_ARTICLE: Table = Table::from_dice("称号表", 1, 6, KO_MS_ARTICLE_ITEMS);

static KO_MS_DESCRIBE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ビギニング")),
    (12, TableItem::Text("パワフル")),
    (13, TableItem::Text("ビューティフル")),
    (14, TableItem::Text("エターナル")),
    (15, TableItem::Text("きらめき")),
    (16, TableItem::Text("シャイニング")),
    (22, TableItem::Text("パーフェクト")),
    (23, TableItem::Text("1000%")),
    (24, TableItem::Text("フレッシュ")),
    (25, TableItem::Text("ドキドキ")),
    (26, TableItem::Text("ワイルド")),
    (33, TableItem::Text("ロイヤル")),
    (34, TableItem::Text("ときめき")),
    (35, TableItem::Text("ふわふわ")),
    (36, TableItem::Text("スタイリッシュ")),
    (44, TableItem::Text("小悪魔")),
    (45, TableItem::Text("スーパー")),
    (46, TableItem::Text("ウルトラ")),
    (55, TableItem::Text("ハイパー")),
    (56, TableItem::Text("ダイナマイト")),
    (66, TableItem::Text("アルティメット")),
];

static KO_MS_DESCRIBE: D66Table = D66Table::new("形容表", D66SortType::Asc, KO_MS_DESCRIBE_ITEMS);

static KO_MS_SCENE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("マーメイド")),
    (12, TableItem::Text("ドリーム")),
    (13, TableItem::Text("ピュア")),
    (14, TableItem::Text("アニマル")),
    (15, TableItem::Text("サンシャイン")),
    (16, TableItem::Text("ムーンライト")),
    (22, TableItem::Text("かわいい／かっこいい")),
    (23, TableItem::Text("フューチャリング")),
    (24, TableItem::Text("ライジング")),
    (25, TableItem::Text("バーニング")),
    (26, TableItem::Text("スターライト")),
    (33, TableItem::Text("ボンバー")),
    (34, TableItem::Text("レインボー")),
    (35, TableItem::Text("フローズン")),
    (36, TableItem::Text("ヒート")),
    (44, TableItem::Text("ダーク")),
    (45, TableItem::Text("ぴかぴか")),
    (46, TableItem::Text("サンライズ")),
    (55, TableItem::Text("スターダスト")),
    (56, TableItem::Text("オーロラ")),
    (66, TableItem::Text("ギャラクシー")),
];

static KO_MS_SCENE: D66Table = D66Table::new("情景表", D66SortType::Asc, KO_MS_SCENE_ITEMS);

static KO_MS_MATERIAL_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バスケット")),
    (12, TableItem::Text("エクスプレス")),
    (13, TableItem::Text("エアプレーン")),
    (14, TableItem::Text("ロケット")),
    (15, TableItem::Text("ハリケーン")),
    (16, TableItem::Text("バイク")),
    (22, TableItem::Text("タイガー")),
    (23, TableItem::Text("ドルフィン")),
    (24, TableItem::Text("ドッグ")),
    (25, TableItem::Text("キャット")),
    (26, TableItem::Text("バニー")),
    (33, TableItem::Text("ドラゴン")),
    (34, TableItem::Text("ソード")),
    (35, TableItem::Text("ランス")),
    (36, TableItem::Text("パラソル")),
    (44, TableItem::Text("ローズ")),
    (45, TableItem::Text("ロータス")),
    (46, TableItem::Text("コスモス")),
    (55, TableItem::Text("キャンディ")),
    (56, TableItem::Text("ハート")),
    (66, TableItem::Text("フェニックス")),
];

static KO_MS_MATERIAL: D66Table =
    D66Table::new("マテリアル表", D66SortType::Asc, KO_MS_MATERIAL_ITEMS);

static KO_MS_ACTION_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("スパイラル")),
    (12, TableItem::Text("フライ")),
    (13, TableItem::Text("シャワー")),
    (14, TableItem::Text("ダイブ")),
    (15, TableItem::Text("イリュージョン")),
    (16, TableItem::Text("ラッシュ")),
    (22, TableItem::Text("ターン")),
    (23, TableItem::Text("ラブ")),
    (24, TableItem::Text("ハグ")),
    (25, TableItem::Text("ダッシュ")),
    (26, TableItem::Text("シュート")),
    (33, TableItem::Text("ダイビング")),
    (34, TableItem::Text("クロス")),
    (35, TableItem::Text("トリック")),
    (36, TableItem::Text("ビーム")),
    (44, TableItem::Text("スラッシュ")),
    (45, TableItem::Text("ボイス")),
    (46, TableItem::Text("ドライブ")),
    (55, TableItem::Text("くるくる")),
    (56, TableItem::Text("ジャンプ")),
    (66, TableItem::Text("アクション")),
];

static KO_MS_ACTION: D66Table = D66Table::new("アクション表", D66SortType::Asc, KO_MS_ACTION_ITEMS);

static KO_MS_FORMATS: &[&str] = &[
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%sもしくは%s＋%s＋PCの名前",
    "%sもしくは%s＋%s＋PCの名前",
];

/// Ruby `MySkillNameTable.new(:ko_kr)`（`MS`）。
static KO_MS: MySkillNameTable = MySkillNameTable::new(
    "マイスキル名決定表",
    KO_MS_FORMATS,
    &[
        &[
            SubTable::D66(&KO_MS_DESCRIBE),
            SubTable::D66(&KO_MS_SCENE),
            SubTable::D66(&KO_MS_MATERIAL),
        ],
        &[
            SubTable::D66(&KO_MS_DESCRIBE),
            SubTable::D66(&KO_MS_SCENE),
            SubTable::D66(&KO_MS_ACTION),
        ],
        &[
            SubTable::D66(&KO_MS_DESCRIBE),
            SubTable::D66(&KO_MS_MATERIAL),
            SubTable::D66(&KO_MS_ACTION),
        ],
        &[
            SubTable::D66(&KO_MS_SCENE),
            SubTable::D66(&KO_MS_MATERIAL),
            SubTable::D66(&KO_MS_ACTION),
        ],
        &[
            SubTable::D66(&KO_MS_DESCRIBE),
            SubTable::D66(&KO_MS_SCENE),
            SubTable::Plain(&KO_MS_ARTICLE),
        ],
        &[
            SubTable::D66(&KO_MS_MATERIAL),
            SubTable::D66(&KO_MS_ACTION),
            SubTable::Plain(&KO_MS_ARTICLE),
        ],
    ],
);

static KO_RE_ON_EVENT_ITEMS: &[(i64, &str, i64)] = &[
    (11, "雨女は誰？", 96),
    (12, "千客万来☆アイドル喫茶", 97),
    (13, "フチドル", 98),
    (14, "生放送は踊る", 99),
    (15, "貸し切りプールの誘惑", 100),
    (16, "ケーオンストリート！", 101),
    (21, "アイドル×アニメ×ドリーマー！", 102),
    (22, "一日警察署長、緊急出動!?", 103),
    (23, "アイドルフィン！", 104),
    (24, "「カラオケ採点ガチバトル☆」", 105),
    (25, "「大正乙女ろまんてぃっく」", 106),
    (26, "鳩時計ラジオ", 107),
    (31, "「ガチ学院」ＣＭ", 108),
    (32, "「カラフルアイスクリーム」モデル", 109),
    (33, "忙しすぎる毎日", 110),
    (34, "悩める新人デザイナー", 112),
    (35, "「スクール☆ライフ」", 113),
    (36, "魔法のように", 114),
    (41, "食レポとその後", 115),
    (42, "ソロライブ！", 116),
    (43, "お昼の放送", 117),
    (44, "文化祭！", 118),
    (45, "商店街を救え！", 120),
    (46, "二つの仕事", 121),
    (51, "温泉にて", 122),
    (52, "アイドル探偵と豪華客船", 124),
    (53, "のうぎょう", 125),
    (54, "コント撮影", 127),
    (55, "アイドルＶＳサメ", 128),
    (56, "駅前で歌う", 130),
    (61, "街の清掃ボランティア", 131),
    (62, "ミニユニット活動", 132),
    (63, "カブトムシ狩り", 134),
    (64, "ポスター作り", 135),
    (65, "メロディ", 136),
    (66, "さいてい新聞部の取材", 138),
];

static KO_RE_OFF_EVENT_ITEMS: &[(i64, &str, i64)] = &[
    (11, "アイドル、未知との遭遇", 139),
    (12, "神様おねがい！", 140),
    (13, "プチ合宿の罠!?", 141),
    (14, "どこかで会ったような……", 142),
    (15, "アイデンティティがっ！", 143),
    (16, "ホリダシ×オオソウジ", 144),
    (21, "エンドレス!?　握手会", 146),
    (22, "不安な路線変更", 147),
    (23, "全力ねこレース", 148),
    (24, "恐怖の再テスト！", 149),
    (25, "たくさんのファンレター", 150),
    (26, "夕暮れの帰り道。", 152),
    (31, "どきどき♪　調理実習", 153),
    (32, "超アイドル衣装？", 154),
    (33, "おもいでの修学旅行", 156),
    (34, "アルバイト！", 158),
    (35, "ドライブしよう！", 159),
    (36, "ファミレス攻防戦", 160),
    (41, "総合練習", 162),
    (42, "歌声はお腹から", 164),
    (43, "メイクレッスン基本から", 165),
    (44, "怪我", 166),
    (45, "エゴサ", 168),
    (46, "喫茶店でひと息", 169),
    (51, "天体観測ツアー", 170),
    (52, "謎のコーチ", 172),
    (53, "屋上にて", 174),
    (54, "クラスメイトより", 176),
    (55, "最強アイドル伝", 177),
    (56, "イメチェンしよう", 178),
    (61, "郊外ショッピング施設", 179),
    (62, "お見舞い", 180),
    (63, "ライブを観よう！", 181),
    (64, "頂を目指す", 182),
    (65, "重いコンダラ", 183),
    (66, "アイドル改造計画", 184),
];

/// Ruby `RandomEventTable.new(:ko_kr)`（`RE`）。
static KO_RE: RandomEventTable = RandomEventTable::new(
    "ランダムイベント",
    "%{event}（『ビギニングロード』%{page}ページ）",
    "オンイベント表",
    KO_RE_ON_EVENT_ITEMS,
    "オフイベント表",
    KO_RE_OFF_EVENT_ITEMS,
);

static KO_SH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("思いがけず、ブランドもの衣装を買えてしまった。これを、うまく使えないだろうか？\nシーンプレイヤーとパートナープレイヤーは、好きなブランドの衣装チケットを一つ獲得する。")),
    (12, TableItem::Text("雑貨コーナーを回って、自分たちらしいアクセサリーを探す。\nシーンプレイヤーとパートナープレイヤーは、アイテム「キャラアイテム」を一つ獲得する。")),
    (13, TableItem::Text("お歳暮コーナーが盛況だった。今のトレンドはなんだろうか。\nシーンプレイヤーとパートナープレイヤーは、アイテム「差し入れ」を一つ獲得する。")),
    (14, TableItem::Text("お菓子売り場で、気になっていたお菓子のシリーズを買い漁る。\nシーンプレイヤーとパートナープレイヤーは、アイテム「お菓子」を一つ獲得する。")),
    (15, TableItem::Text("スポーツショップに立ち寄って、スポーツドリンクを買い貯める。いつか使うかもしれない。\nシーンプレイヤーとパートナープレイヤーは、アイテム「スナミナドリンク」を一つ獲得する。")),
    (16, TableItem::Text("スポーツウェアの展示会をやっていたので、見て回る。びびっと来たアイテムも見つかった。\nシーンプレイヤーとパートナープレイヤーは、アイテム「トレーニングウェア」を一つ獲得する。")),
    (22, TableItem::Text("特売品を買い漁る。さて、使えるものかな？\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに二つ獲得する。")),
    (23, TableItem::Text("シューズコーナーで、普段使いの靴を調達する。ダンスにちょうどいいのはどれかな？\nシーンプレイヤーとパートナープレイヤーは、アイテム「ドリーミングシューズ」を一つ獲得する。")),
    (24, TableItem::Text("ふらりと買い物にやって来て、目についたものをとりあえず買ってみる。ちょっと疲れてたかも。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (25, TableItem::Text("色々な洋服を見て回って、自分やパートナーに合ったコーディネートを考えてみる。\nシーンプレイヤーとパートナープレイヤーは、【ビジュアル】が1点上昇する。")),
    (26, TableItem::Text("ちょうど買いたかったものが、格安で売られていた。タイミングがよかったみたいだ。\nシーンプレイヤーとパートナープレイヤーは、好きなアイテムを一つ獲得する。")),
    (33, TableItem::Text("ショッピングモールを歩いているうちに、アイテムを落としてしまう。\nシーンプレイヤーとパートナープレイヤーは、アイテムをすべて失う。")),
    (34, TableItem::Text("ショッピングモールを歩いていると、声をかけられた。地元の人たちから、応援のメッセージをもらう。\nシーンプレイヤーとパートナープレイヤーは、【獲得ファン人数】が4点上昇する。")),
    (35, TableItem::Text("ショッピングモールでは何も買わなかったが、交わした会話はお互いを知るきっかけになった。\nシーンプレイヤーとパートナープレイヤーは、お互いに対する【理解度】が3点上昇する。")),
    (36, TableItem::Text("ベンチで一休みしながら、お互いの長所について話し合う。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (44, TableItem::Text("混雑中のフードコートで食事をしようとしたところ、一時間近く待たされる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が2点減少する。")),
    (45, TableItem::Text("フードコートで定番メニューを頼み、勝手知ったる味を楽しむ。やっぱり、知っている味がいい。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が2点上昇する。")),
    (46, TableItem::Text("フードコートで新しいメニューにチャレンジ！\nシーンプレイヤーとパートナープレイヤーは、1D6を振る。出目が奇数の場合、そのPCは【メンタル】が5点減少し、出目が偶数の場合、そのPCは【メンタル】が5点上昇する。")),
    (55, TableItem::Text("CDコーナーを探しているうちに、迷ってしまった。さて、ここはどこだろう？\n変調がランダムに一つ発生する。")),
    (56, TableItem::Text("カフェコーナーで一休み。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が5点上昇する。")),
    (66, TableItem::Text("家具や家電コーナーを回るうちに、自分たちの将来が不安になってきた。\n変調がランダムに二つ発生する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SH", ...)`。
static KO_SH: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new(
        "ショッピングモール散策表",
        D66SortType::Asc,
        KO_SH_ITEMS,
    )),
    &KO_ABNORMALITY,
);

static KO_MO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("山で迷っていたら、謎の仙人に助けられる。そのついでに、体をうまく動かす方法まで教わる。ありがとう！　謎の仙人！\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が10点上昇する。")),
    (12, TableItem::Text("山の幸を頂いて、幸福感に包まれる。うまい！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (13, TableItem::Text("星空の下で、二人の【背景】を語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (14, TableItem::Text("森林浴をして、体調を整える。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がすべて回復する。")),
    (15, TableItem::Text("山登りを繰り返して、足腰を鍛える。\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (16, TableItem::Text("二人で一緒に朝日を見て、訳も分からず感動する。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (22, TableItem::Text("山で迷った。\n変調がランダムに二つ発生する。")),
    (23, TableItem::Text("山奥に住んでいるという、伝説のファッションデザイナーに着こなしの秘密を教えてもらう。\nシーンプレイヤーとパートナープレイヤーは、好きなブランドの衣装チケットを一つ獲得する。")),
    (24, TableItem::Text("山奥に住んでいるという、伝説のレッスントレーナーに教えを乞いに足を延ばす。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (25, TableItem::Text("ふらっと立ち寄った地元の飲食店で郷土料理を食べる。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (26, TableItem::Text("山奥にある神社まで登って、お祈りをする。無事にライブが成功しますように。\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が5点上昇する。")),
    (33, TableItem::Text("虫にたかられて、嫌な思いをする。\n変調がランダムに一つ発生する。")),
    (34, TableItem::Text("仲間たちみんなとバーベキューをして楽しむ。\nシーンプレイヤーとパートナープレイヤーは、PC全員に対する【理解度】が3点上昇する。")),
    (35, TableItem::Text("キノコ狩りをする。\nシーンプレイヤーとパートナープレイヤーは、1D6を振る。その値が偶数だった場合、アイテム「スタミナドリンク」を一つ獲得する。奇数だった場合、【メンタル】が6点減少する。")),
    (36, TableItem::Text("避暑地の喫茶店で一杯飲みながら、お互いのアイドル論について語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (44, TableItem::Text("山を移動中に、落し物をしてしまう。\nシーンプレイヤーとパートナープレイヤーは、アイテムを一つ失う。")),
    (45, TableItem::Text("山小屋で一晩を過ごす。お互いの生活習慣が見えてきた。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (46, TableItem::Text("ハイキングをしながら、お互いの嫌いなものについて理解を深める。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (55, TableItem::Text("野生の鹿に襲われそうになったので、プロデューサーが盾になった。\n変調「怪我」が発生する。")),
    (56, TableItem::Text("高原の広々としたテニスコートでテニスを楽しむ。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (66, TableItem::Text("山道を歩き疲れて、足が棒になる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.MO", ...)`。
static KO_MO: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new("山散策表", D66SortType::Asc, KO_MO_ITEMS)),
    &KO_ABNORMALITY,
);

static KO_SEA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("浜辺で行われていたミスコンに強制的に参加させられる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が1D6点上昇する。")),
    (12, TableItem::Text("浜辺を散歩しながら、お互いの好きなものについて語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (13, TableItem::Text("とれたての魚を使った寿司を食べて満足する。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (14, TableItem::Text("浜辺を歩いていると、アイドルだと気づいた地元の人たちから声援が飛んでくる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が5点上昇する。")),
    (15, TableItem::Text("童心に返って、海に向かって走り出す。やったー海だー！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (16, TableItem::Text("水泳で体を鍛える。荒れやすい海は特訓にもってこいだ！\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (22, TableItem::Text("せっかくだから釣りをしてみる。何が釣れるかな？\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (23, TableItem::Text("二人で競い合いながら泳ぎまわる。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (24, TableItem::Text("ちょっとだけ日に焼けて、違う自分をセルフプロデュース。ちゃんと事務所の許可はとれてます！\nシーンプレイヤーとパートナープレイヤーの【ビジュアル】が1点上昇する。")),
    (25, TableItem::Text("砂浜にいい感じのタイヤがあったので、それを引っ張りながら走り込みをする。\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (26, TableItem::Text("海に向かって叫んで、すっきりする。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇し、【ボイス】が1点上昇する。")),
    (33, TableItem::Text("しつこいナンパに誘われて、ちょっと意気消沈してしまう。\n変調がランダムに二つ発生する。")),
    (34, TableItem::Text("みんなで花火をして楽しむ。\nシーンプレイヤーとパートナープレイヤーは、PC全員に対する【理解度】が2点上昇する。")),
    (35, TableItem::Text("水着で決めるファンションをコーディネートして、浜辺の視線を一人占め。\nシーンプレイヤーとパートナープレイヤーは、【ビジュアル】が1点上昇する。")),
    (36, TableItem::Text("魚料理を満足いくまで食べたはいいものの、食べ過ぎじゃないかが心配。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇する。")),
    (44, TableItem::Text("穏やかな海を見ながら、これまでのことを思い返す。\n変調が一つ回復する。")),
    (45, TableItem::Text("水族館に立ち寄ったら、お土産に色々持たされる。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (46, TableItem::Text("水族館のイルカショーから、新しい技のヒントをもらう。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (55, TableItem::Text("海で遊んでいたら、溺れかける。\n変調がランダムに三つ発生する。")),
    (56, TableItem::Text("サーファーたちから、人を惹きつける技術について聞き出す。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (66, TableItem::Text("夜の海に出没するという幽霊らしきものを見かけてしまい、ぞっとする。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が5点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SEA", ...)`。
static KO_SEA: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new("海散策表", D66SortType::Asc, KO_SEA_ITEMS)),
    &KO_ABNORMALITY,
);

static KO_SPA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("地元のお祭りに遭遇！　一緒になって盛り上げる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が5点上昇する。")),
    (12, TableItem::Text("卓球台を使って、お互いの力を出し切る。何かが掴めたような気がする。\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が5点上昇する。")),
    (13, TableItem::Text("お土産屋さんで、色々なものを買い込む。しかし、これは役に立つのだろうか。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (14, TableItem::Text("温泉からあがった後に、ゆっくりと涼む。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がすべて回復する。")),
    (15, TableItem::Text("温泉街の名物料理を出してもらう。うまい！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (16, TableItem::Text("浴衣で活気のある街並みを歩きながら、お互いの【背景】を語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が1D6点上昇する。")),
    (22, TableItem::Text("湯に浸かり過ぎて目が回る。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がランダムに一つ発生する。")),
    (23, TableItem::Text("たくさんの温泉に次々浸かる。楽しいけど疲れた。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (24, TableItem::Text("湯船に浸かってリフレッシュ。ひとまずは一息入れましょう。\n変調がすべて回復し、シーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (25, TableItem::Text("ジャグジー風呂で肩こりや足のむくみを癒す。温泉地でもこういう施設はあるんだな。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (26, TableItem::Text("みんなやプロデューサーを連れて足湯に浸かる。いつもご苦労様。\n変調がすべて回復する。")),
    (33, TableItem::Text("のぼせる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
    (34, TableItem::Text("貸切風呂で、ゆったりとしながらお互いを理解をするための話し合いをする。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (35, TableItem::Text("マッサージ機を使って、日ごろの疲れをたたき出す。\n変調をすべて回復する。")),
    (36, TableItem::Text("お風呂の後の牛乳もうまい！\nシーンプレイヤーとパートナープレイヤーは、アイテム「スナミナドリンク」を一つ獲得する。")),
    (44, TableItem::Text("ついつい無駄遣いをしてしまう。てへへ。\n変調がランダムに一つ発生する。")),
    (45, TableItem::Text("屋台での観光客向けの料理に舌鼓をうつ。こういう味もありなのか。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇する。")),
    (46, TableItem::Text("温泉街の女将さんたちに、人の心を豊かにする術を教えてもらう。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (55, TableItem::Text("部屋の中でぼけっと過ごす。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が1点上昇する。")),
    (56, TableItem::Text("観光客の人々と会話をして、自分たちの知名度を確認する。\nアイドルランク係数が「10」以上の場合、【獲得ファン人数】が4D6点上昇する。")),
    (66, TableItem::Text("複雑な地形と坂道で疲れ始める。目的の場所はまだ？\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SPA", ...)`。
static KO_SPA: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new(
        "温泉街散策表",
        D66SortType::Asc,
        KO_SPA_ITEMS,
    )),
    &KO_ABNORMALITY,
);

static KO_LN_ITEMS: &[&str] = &[
    "誰のことも信じられない。私は一人でやってみせる。\nPC全員の【理解度】すべてにチェックを入れる。センターPCは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。",
    "事件がきっかけで、事務所の空気が悪くなった。嫌な雰囲気。\n変調がランダムに三つ発生する。",
    "口調が荒くなり、きつい一言を仲間に言ってしまう。\nPC全員の【メンタル】が5点減少し、【理解度】すべてにチェックを入れる。",
    "ちょっとした注意がきっかけで、仲間と喧嘩をしてしまう。\nPC全員の【メンタル】が10点減少する。",
    "誰も、話をしない。気まずさと静寂が場を包んだ。このままでは、会場の空気も悪くなる。\n【目標動員数】が二倍になる。",
    "突然の強い雨に打たれる。ずぶぬれのところに一人。そんなところを誰かに目撃されてしまう。\nセンターPCの【獲得ファン人数】が半分になる。",
];

/// Ruby `TableWithAbnormality.from_i18n("BeginningIdol.tables.LN", ...)`。
static KO_LN: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::Plain(Table::from_dice("孤独表", 1, 6, KO_LN_ITEMS)),
    &KO_ABNORMALITY,
);

static KO_SGT_ITEMS: &[&str] = &[
    "씬 플레이어가 보유한 재능분야의특기가 지정 특기인 아이돌 스킬",
    "씬 플레이어가 보유한 캐릭터분야의특기가 지정 특기인 아이돌 스킬",
    "씬 플레이어가 보유한 취미분야의특기가 지정 특기인 아이돌 스킬",
    "랜덤으로 지정한 특기가 지정 특기인 아이돌 스킬(신장분야、속성분야、출신분야가 나올경우 재굴림)",
    "《멘탈 업》《퍼포먼스 업》《아이템 업》(챌린지 걸즈 216 페이지) 중 어느 쪽이든 1개",
    "《멘탈 업》《퍼포먼스 업》《아이템 업》(챌린지 걸즈 216 페이지) 중 어느 쪽이든 1개",
];

/// Ruby `SkillGetTable.from_i18n("BeginningIdol.tables.SGT", ...)`。
static KO_SGT: SkillGetTable = SkillGetTable::new(
    Table::from_dice("아이돌 스킬 습득표(챌린지 걸즈)", 1, 6, KO_SGT_ITEMS),
    &KO_SKILL_TABLE,
    "(신장)분야、(속성|재능)분야、(출신)분야가 나올경우 재굴림",
    "振り直し",
    "特技リスト",
    DEFAULT_SKILL_FORMAT,
);

static KO_RS_ITEMS: &[&str] = &[
    "씬 플레이어가 보유한 속성분야의특기가 지정 특기인 아이돌 스킬",
    "씬 플레이어가 보유한 캐릭터분야의특기가 지정 특기인 아이돌 스킬",
    "씬 플레이어가 보유한 취미분야의특기가 지정 특기인 아이돌 스킬",
    "랜덤으로 지정한 특기가 지정 특기인 아이돌 스킬(신장분야、재능분야、출신분야가 나올경우 재굴림)",
    "《멘탈 디펜스》《판정 업》《개성 업》중 어느 쪽이든 1개",
    "《멘탈 디펜스》《판정 업》《개성 업》중 어느 쪽이든 1개",
];

/// Ruby `SkillGetTable.from_i18n("BeginningIdol.tables.RS", ...)`。
static KO_RS: SkillGetTable = SkillGetTable::new(
    Table::from_dice("아이돌 스킬 습득표(로드 투 프린스)", 1, 6, KO_RS_ITEMS),
    &KO_SKILL_TABLE,
    "(신장)분야、(속성|재능)분야、(출신)분야가 나올경우 재굴림",
    "振り直し",
    "特技リスト",
    DEFAULT_SKILL_FORMAT,
);

static KO_CBT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("바뀌는 말투")),
    (12, TableItem::Text("~버릇")),
    (13, TableItem::Text("동물 같은")),
    (14, TableItem::Text("일본식")),
    (15, TableItem::Text("경박함")),
    (16, TableItem::Text("계산이 빠름")),
    (22, TableItem::Text("여동생／누나 캐릭터")),
    (23, TableItem::Text("포지티브!")),
    (24, TableItem::Text("네거티브……")),
    (25, TableItem::Text("응석받이")),
    (26, TableItem::Text("연령")),
    (33, TableItem::Text("동물의상")),
    (34, TableItem::Text("지기 싫어함")),
    (35, TableItem::Text("노력가")),
    (36, TableItem::Text("말하고 싶어함")),
    (44, TableItem::Text("천연")),
    (45, TableItem::Text("흉내")),
    (46, TableItem::Text("특징 없음")),
    (55, TableItem::Text("직감")),
    (56, TableItem::Text("피아노")),
    (66, TableItem::Text("소중한 사람")),
];

static KO_CBT: D66Table =
    D66Table::new("캐릭터공백표(챌린지 걸즈)", D66SortType::Asc, KO_CBT_ITEMS);

static KO_RCB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("걱정이 많은")),
    (12, TableItem::Text("본좌 (어르신)")),
    (13, TableItem::Text("남동생")),
    (14, TableItem::Text("노력가")),
    (15, TableItem::Text("차분함")),
    (16, TableItem::Text("서투름")),
    (22, TableItem::Text("이중인격")),
    (23, TableItem::Text("럭키보이")),
    (24, TableItem::Text("귀여운")),
    (25, TableItem::Text("소악마")),
    (26, TableItem::Text("유유자적")),
    (33, TableItem::Text("조용한 광기")),
    (34, TableItem::Text("육체파")),
    (35, TableItem::Text("시인")),
    (36, TableItem::Text("참견")),
    (44, TableItem::Text("연애를 좋아함")),
    (45, TableItem::Text("비평가")),
    (46, TableItem::Text("차가움")),
    (55, TableItem::Text("고고함")),
    (56, TableItem::Text("오빠")),
    (66, TableItem::Text("여자를 접하기 싫어함")),
];

static KO_RCB: D66Table = D66Table::new(
    "캐릭터공백표(로드 투 프린스)",
    D66SortType::Asc,
    KO_RCB_ITEMS,
);

static KO_HBT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("취미없음")),
    (12, TableItem::Text("티타임")),
    (13, TableItem::Text("시")),
    (14, TableItem::Text("자격증 습득")),
    (15, TableItem::Text("일러스트레이트")),
    (16, TableItem::Text("인형")),
    (22, TableItem::Text("수면")),
    (23, TableItem::Text("천체관측")),
    (24, TableItem::Text("산책")),
    (25, TableItem::Text("식사")),
    (26, TableItem::Text("뜨개딜")),
    (33, TableItem::Text("곤충채집")),
    (34, TableItem::Text("문자")),
    (35, TableItem::Text("긴통화")),
    (36, TableItem::Text("카페순례")),
    (44, TableItem::Text("목욕")),
    (45, TableItem::Text("수집")),
    (46, TableItem::Text("조경")),
    (55, TableItem::Text("등산")),
    (56, TableItem::Text("역사 애호가")),
    (66, TableItem::Text("가사")),
];

static KO_HBT: D66Table = D66Table::new("취미공백표(챌린지 걸즈)", D66SortType::Asc, KO_HBT_ITEMS);

static KO_RHB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("철도")),
    (12, TableItem::Text("꽃꽃이")),
    (13, TableItem::Text("여행")),
    (14, TableItem::Text("일요목수")),
    (15, TableItem::Text("서예")),
    (16, TableItem::Text("단형시 짓기")),
    (22, TableItem::Text("식도락")),
    (23, TableItem::Text("근력 트레이닝")),
    (24, TableItem::Text("공작")),
    (25, TableItem::Text("자격증 습득")),
    (26, TableItem::Text("낚시")),
    (33, TableItem::Text("산보")),
    (34, TableItem::Text("패션")),
    (35, TableItem::Text("사육")),
    (36, TableItem::Text("장난")),
    (44, TableItem::Text("거리에서 헌팅")),
    (45, TableItem::Text("독서")),
    (46, TableItem::Text("가사전반")),
    (55, TableItem::Text("곤충채집")),
    (56, TableItem::Text("아트")),
    (66, TableItem::Text("수면")),
];

static KO_RHB: D66Table =
    D66Table::new("취미공백표(로드 투 프린스)", D66SortType::Asc, KO_RHB_ITEMS);

static KO_RU_ITEMS: &[&str] = &[
    "激しいアクションで興味を持った人たちを呼び寄せる。\nPC全員の【獲得ファン人数】が5点上昇する。",
    "マスコットキャラクターから聞こえてはいけない音が聞こえてきて、次の瞬間には動かなくなってしまった。\nこのセッションの間、マスコットキャラクターが使用できなくなる。",
    "マスコットキャラクターが行方不明！　プロデューサーが代わりに着ぐるみを着たけれども、負担が大きかった。\n変調「怪我」が発生する。",
    "マスコットキャラクターが不適切な発言をしてしまい、連帯責任で謝罪することになってしまう。\nPC全員の【獲得ファン人数】が、それぞれ5点減少する。",
    "マスコットキャラクターが転んで起き上がれなくなってしまった！　みんなで力を合わせて助け起こそう。\nPC全員の【メンタル】が3点減少する。",
    "マスコットが突然PCに物申す。問題点を挙げて、鍛えてくれる。\nPC一人は、「アイドルスキル修得表」を使って、アイドルスキルを一つ修得する。",
];

static KO_RU: Table = Table::from_dice("マスコット暴走表", 1, 6, KO_RU_ITEMS);

static KO_SIP_ITEMS: &[&str] = &[
    "テレビ番組に出て、ライブの宣伝をする。",
    "ラジオに出演して、ライブの宣伝をする。",
    "動画を配信して、ライブの宣伝をする。",
    "ライブの宣伝のために、街でビラ配りをする。",
    "ライブに人を集めるために、派手なパフォーマンスを街中でする。",
    "ライブの宣伝のために、あちこちを走り回る。",
];

static KO_SIP: Table = Table::from_dice("かんたんパーソン表", 1, 6, KO_SIP_ITEMS);

static KO_BU_ITEMS: &[&str] = &[
    "熱い！　熱い！\n【メンタル】が2点減少する。",
    "慌てて浴槽から出ようとしたが、足を滑らせて浴槽に落ちる。ウケたはいいが、とても熱い。\n【メンタル】が1D6点減少し、【獲得ファン人数】が3D6点上昇する。",
    "温かい目で見守っていた仲間の手を力いっぱい引っ張り、浴槽に引きずり込む。\n自分以外のPCを一人選ぶ。選ばれたPCは、【メンタル】を3点減少させ、「バーストタイム」を行う。",
    "あまりの熱さに浴槽へ入り損ねていたら、仲間の一人に叩き落とされる。\n【メンタル】を2点減少してから、PCを一人選ぶ。選んだPCに対する【理解度】が3点上昇し、チェックを外す。",
    "思い切って氷を頭から浴びる。クールダウン完了！\n【メンタル】を2点減少させることで、もう一度「バーストタイム」を行うことができる。",
    "熱湯風呂に入るための着替えに手間取ってしまい、急かされてしまう。結果、満足に着替えができなかった。\nこのライブフェイズの間、衣装の効果が無効化される。",
];

static KO_BU: Table = Table::from_dice("バースト表", 1, 6, KO_BU_ITEMS);

static KO_HW_ITEMS: &[&str] = &[
    "誰もいない屋内。静寂が世界を包んでいる。嵐の前の静けさだ。",
    "話し声が絶えない夕暮れの帰り道。みんなが明るい声を上げる中、自分の周りだけがぽっかり穴が空いたかのように静かだ。",
    "曇り空になってきた。早く屋内に行かないと、雨でぬれてしまうかもしれない。",
    "ゲリラ豪雨だ。傘も持ってきていないので、激しい雨に打たれるしかない。ついてないな。",
    "夜空を雲が覆いつくしてしまっている。空を見上げても、星の輝きは見えない。",
    "屋内の電気がトラブルで点灯しないようだ。暗い世界は、気分まで滅入ってしまう。",
];

static KO_HW: Table = Table::from_dice("向かい風シーン表", 1, 6, KO_HW_ITEMS);

static KO_FL_ITEMS: &[&str] = &[
    "トレーニングルームで、皆が真剣な顔をしている。真面目な雰囲気が場を支配しており、軽い冗談も言えなさそう。",
    "いつものたまり場。なのに、今日に限って、騒がしさがどこかに行ってしまったようだ。",
    "誰も歩いていない夜道。人気もなく、不安を抱くような暗闇に、足音だけが響いている。",
    "強い風と強い雨が吹きつける事務所の中。外に出れば、吹き飛ばされてしまいそう。",
    "曇り空の下。そこにいるだけで、暗い気持ちになるような、どんよりとした天気。",
    "雨が降り続けている。雨は、ずっと続いている。いつになったら晴れるのだろう。",
];

static KO_FL: Table = Table::from_dice("駆け出しシーン表", 1, 6, KO_FL_ITEMS);

static KO_MSE_ITEMS: &[&str] = &[
    "演目を行ったときに使用できる。自分の【メンタル】が15点になる。この効果は、1回のセッションに1度まで使用できる。",
    "ミラクル・ミラクルシンクロ・パーフェクトミラクルを発生させたときに使用できる。【パフォーマンス値】が10点上昇する。この効果は、1回のセッションに1度まで使用できる。",
    "ファンブルではない判定の後に使用する。判定の達成値を12にする。この効果は、1回のセッションに1度まで使用できる。",
    "演目を行ったときに使用できる。自分以外のPC全員の【メンタル】が2D6点上昇する。この効果は、1回のセッションに1度まで使用できる。",
    "ライブフェイズ開始時に使用する。このフェイズの間、すべての判定の達成値にプラス2の修正がつく。この効果は、1回のセッションに1度まで使用できる。",
    "パフォーマンスのサイコロを振った後に使用する。サイコロ1つの出目を6にすることができる。この効果は、1回のセッションに1度まで使用できる。",
];

static KO_MSE: Table = Table::from_dice("マイスキル効果表", 1, 6, KO_MSE_ITEMS);

static KO_ST_ITEMS: &[&str] = &[
    "見事なパフォーマンスに、人々が感動する。",
    "その声に観客が聞き惚れる。",
    "一糸乱れぬダンスが決まる。",
    "宙に飛ばしたマイクを見事にキャッチする。",
    "トランポリンなどを使って、会場の天井近くまでジャンプ。",
    "観客と一体になって決めポーズ。",
];

static KO_ST: Table = Table::from_dice("演出表", 1, 6, KO_ST_ITEMS);

static KO_FST_ITEMS: &[&str] = &[
    "会場を覆っていた暗雲を退散させる。",
    "会場に花が咲く。",
    "炎の旋風が観客を燃え上がらせる。",
    "ハートの風船が会場中を飛び交う。",
    "羽を生やして会場を飛び回る。",
    "打ち上がった花火と共に決めポーズ。",
];

static KO_FST: Table = Table::from_dice("ファンタジー演出表", 1, 6, KO_FST_ITEMS);

static KO_BWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("선배 아이돌이 사회를 맡는 버라이어티 프로그램에 출연! 어떤 코너를 맡는지?\n特技 : 趣味分野からランダム")),
    (13, TableItem::Text("선배 아이돌과 로드 워킹 프로그램에서 공동 출연. 이 거리에는 무엇이 있는지?\n特技 : 出身分野からランダム")),
    (14, TableItem::Text("선배 아이돌이 음악 프로그램에 출연. 백 댄서를 맡게 되었다\n特技 : 《댄스／취미9》")),
    (15, TableItem::Text("선배 아이돌과 함께 그라비아 촬영. 지지 않게 눈에 띄자.\n特技 : 《마이페이스／캐릭터4》")),
    (16, TableItem::Text("아이돌 소개 프로그램에서 선배 아이돌에게 소개된다. 기운차게 임하자\n特技 : 《건강／캐릭터8》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("선배 아이돌의 라디오 프로그램에 게스트로 출연. 토크로 잘 북돋을 수 있을까?\n特技 : 《캐릭터 분야 공백／캐릭터7》")),
    (24, TableItem::Text("동기 아이돌들과 함께, 대대적인 에스테틱 세트에 도전!\n特技 : 《운동신경／재능8》")),
    (25, TableItem::Text("특별한 의상으로 실시하는 연극의 일이 왔다. 어떤 의상일까?\n特技 : パートナープレイヤーが修得しているキャラ分野の特技")),
    (26, TableItem::Text("프로덕션의 초대형 중진이 출연하는 프로그램에 불린다.\n特技 : 《담력／재능5》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("씬 플레이어의 캐릭터를 살린 미니 드라마가 제작된다.\n特技 : 씬 플레이어가 보유한 캐릭터분야의특기")),
    (35, TableItem::Text("해외 로케를 하는 프로덕션 내 여행 프로그램 출연. 오늘은 어디까지 갈 건가?\n特技 : 《해외／출신12》")),
    (36, TableItem::Text("프로덕션 내 극장에서 씬 플레이어가 「좋아하는 것」을 사용한 연극이 시작된다.\n特技 : 씬 플레이어가 보유한 취미분야의특기")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("패션 브랜드와 제휴한 패션쇼에 초대된다.\n特技 : 《멋내기／취미5》")),
    (46, TableItem::Text("아이돌 잡지 출판사로부터 취재가 왔다. 잘 대응해야.\n特技 : 《언행정중／캐릭터10》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("시리즈 드라마에 단역으로 출연!\n特技 : 《연기력／재능12》")),
    (66, TableItem::Text("プロダクション内で総選挙が開始！　今回のテーマは……？\n特技 : ランダム")),
];

static KO_BWT: D66Table =
    D66Table::new("대형 예능 프로덕션 업무표", D66SortType::Asc, KO_BWT_ITEMS);

static KO_LWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("파트너 플레이어의 「신체적 특징」에 느낌이 확 온 TV방송국으로부터 섭외가 온다.\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (13, TableItem::Text("스턴트맨이 없는 액션 드라마가 제작 개시! 오디션을 받자.\n特技 : 《운동신경／재능8》")),
    (14, TableItem::Text("가요 프로그램 오디션에 도전! 많은 라이벌 중에 선택되는 것을 목표로 한다!\n特技 : 《집중력／캐릭터4》")),
    (15, TableItem::Text("선술집 영업. 아이돌에 별로 흥미 없을 것 같은 손님 층이지만…….\n特技 : 《마이페이스／캐릭터4》")),
    (16, TableItem::Text("결혼식의 팜플렛 사진을 찍는 촬영. 신부는 어떤 기분일까?\n特技 : 《스타일／재능3》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("CD 데뷔를 걸고 버라이어티 프로그램에서 다른 아이돌과 대결!\n特技 : キャラ分野からランダム")),
    (24, TableItem::Text("CD숍에서, CD를 손수 판매. 잘 부탁 드립니다.\n特技 : 《건강／캐릭터8》")),
    (25, TableItem::Text("로컬 프로그램에 출연. 이 지방에서는 무엇이 유행하고 있지?\n特技 : 《プレーン／属性7》")),
    (26, TableItem::Text("극 조역을 차지하기 위해 오디션을 받는다. 평범한 한 명의 여자아이를 연기하는 것 같다.\n特技 : 《플레인／속성7》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("파트너 플레이어의 「좋아하는 것」이 소재인 드라마가 제작 중. 오디션을 받자.\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (35, TableItem::Text("파트너 플레이어의 「싫은 것」을 소재로 한 드라마에 섭외가…….\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (36, TableItem::Text("코러스에서 결원이 나온 아이돌 라이브의 보충으로서 선택된다. 주역에게 맞추지 않으면.\n特技 : 属性分野からランダム")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("PC들의 사무소에 리포트 카메라가 들어간다. 작은 곳이지만 힘내고 있습니다!\n特技 : ランダム")),
    (46, TableItem::Text("오프")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("오프")),
    (66, TableItem::Text("오프")),
];

static KO_LWT: D66Table =
    D66Table::new("약소 예능 프로덕션 업무표", D66SortType::Asc, KO_LWT_ITEMS);

static KO_TWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("시어터 내 드라마를 수록. 테마는 파트너 플레이어의 「좋아하는 것」이다.\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (13, TableItem::Text("시어터 내 판매용 CD를 제작. 테마는 씬 플레이어의 「좋아하는 것」이다.。\n特技 : 씬 플레이어가 보유한 취미분야의특기")),
    (14, TableItem::Text("시어터의 매점에 끌려간다. 직접 손님과 접촉하는 찬스!\n特技 : 《미소／재능7》")),
    (15, TableItem::Text("시어터 내에서 상품을 판매. 지금의 판매되는 유행은 무엇이지?\n特技 : キャラ分野からランダム")),
    (16, TableItem::Text("시어터 내에서 악수회를 개최! 와준 모두에게 감사를.\n特技 : 《배려／재능9》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("이번 극은 씬 플레이어의 【배경】을 바탕으로 한 논픽션 드라마!\n特技 : 趣味分野からランダム")),
    (24, TableItem::Text("시어터를 이용한 버라이어티 기획이 개시되었다. 모두를 웃기자.\n特技 : 《바보／캐릭터12》")),
    (25, TableItem::Text("시어터에 흐르는 미니 라디오를 수록. 맴버의 엉뚱함 다루는 코너가 볼거리\n特技 : キャラ分野からランダム")),
    (26, TableItem::Text("시어터에 패션 디자이너를 불러 패션쇼. 잘 매료시켜 나가자.。\n特技 : 属性分野からランダム")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("시어터 기획, 1D6시간 집중 댄스가 시작된다…….\n特技 : 《체력／재능6》")),
    (35, TableItem::Text("시어터 기획, 파트너 플레이어는 「싫은 것」에 몇 번이나 도전할 수 있을까!\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (36, TableItem::Text("시어터 기획, 씬 플레이어 vs 「싫은 것」!\n特技 : 씬 플레이어가 보유한 캐릭터분야의특기")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("시어터 기획, 매우 높은 골을 위해 패스! …… 정말로 할 수 있는 거야?\n特技 : 《190～／신장12》")),
    (46, TableItem::Text("시어터 기획, 동물을 대하는 아이돌의 모습을 중계!\n特技 : 《펫／취미3》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("극장 밖에서 실시하는 일을 해낸다.\n特技 : ランダム")),
    (66, TableItem::Text("오프")),
];

static KO_TWT: D66Table = D66Table::new("라이브 시어터 업무표", D66SortType::Asc, KO_TWT_ITEMS);

static KO_CWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("선생님에게 부탁 받고 입학 희망자들의 교내 안내를 맡게 되었다.\n特技 : 《언행정중／캐릭터10》")),
    (13, TableItem::Text("교내 이벤트의 사회 진행을 하게 되었다. 잘 분위기를 북돋울 수 있을까?\n特技 : 《팝／속성9》")),
    (14, TableItem::Text("교내 방송에 출연. 전교생 앞에서 긴장하지 하지 않으려면.。\n特技 : 《담력／재능5》")),
    (15, TableItem::Text("동아리 대회에 응원을 하러 간다. 긴 시합은 체력 승부다.\n特技 : 《체력／재능6》")),
    (16, TableItem::Text("아이돌 부를 취재하는 기자가 왔다. 스스로의 말을 잘할 수 있을까?\n特技 : 《배려／재능9》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("가까운 유치원에서 노래를 선보이게 되었다. 작은 아이와 시선을 맞추지 않으면…….\n特技 : 《～125／신장2》")),
    (24, TableItem::Text("메이드 카페를 하게 되었다. 파트너 플레이어의 「좋아하는 것」이 주된 메뉴다.\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (25, TableItem::Text("교내의 이벤트를 취재하게 되었다. 거기에는 파트너 플레이어의 「싫은 것」이…….\n特技 : 파트너 플레이어가 보유한 캐릭터분야의특기")),
    (26, TableItem::Text("파트너 플레이어의 「패션 특징」을 살린 교내 패션 쇼.\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("현지 상가에 가게의 심부름을 의뢰 받는다. 마을을 북돋워 주러 가자.\n特技 : 《쇼핑／취미8》")),
    (35, TableItem::Text("현지 이벤트에 출연. 향토애를 시험 받는다!\n特技 : 프로듀서가 출신분야의특기를 골라준다")),
    (36, TableItem::Text("파트너 플레이어의 「신체적 특징」에 매료된 기업으로부터 섭외가 왔다!\n特技 : 《섹시／속성4》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("오프")),
    (46, TableItem::Text("오프")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("오프")),
    (66, TableItem::Text("오프")),
];

static KO_CWT: D66Table = D66Table::new("아이돌 부 업무표", D66SortType::Asc, KO_CWT_ITEMS);

static KO_SU_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("음료수 커머셜(광고). 맛있을 것 같이 마시자.\n特技 : 《연기력／재능12》")),
    (13, TableItem::Text("여름의 퍼레이드의 커머셜. 이번 테마는 파트너 플레이어의 「좋아하는 것」.\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (14, TableItem::Text("해수욕장 커머셜. 보는 사람에게 활기를 나누어 줄 수 있으면 좋겠는데.\n特技 : 《팝／속성9》")),
    (15, TableItem::Text("피서지 커머셜. 시원한 곳에서 느긋하게 보냄을 전해 주도록 하자\n特技 : 《마이페이스／캐릭터4》")),
    (16, TableItem::Text("벌레 퇴치 용품의 커머셜. 벌레를 향한 남자다움이 요구된다.\n特技 : 《외고집／캐릭터9》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("수족관에서 활기찬 돌고래들과 쇼를 한다.\n特技 : 《건강／캐릭터8》")),
    (24, TableItem::Text("소년야구 시구식을 맡는다. 야구소년들에게 부끄럽지 않은 피칭으로 매료 시키지 않으면.\n特技 : 《스포츠／취미4》")),
    (25, TableItem::Text("여름 패션을 잡지로 발표하게 되었다. 이 시기의 코디는 이것!\n特技 : 《멋내기／취미5》")),
    (26, TableItem::Text("프로그램에서 여름들판의 나물을 맛있게 먹을 필요성을 느낀다. 여주를 아이돌답게 먹자.\n特技 : 《오키나와／출신2》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("비치 발리볼 적 아이돌과 대결이다! 여름 더위에 지치지 말자!\n特技 : 《버닝／속성10》")),
    (35, TableItem::Text("비치에서 타 아이돌과 헌팅대결을 하게 되었다. 어느 쪽을 잘 해낼 수 있을까?\n特技 : 《유순／캐릭터5》")),
    (36, TableItem::Text("여름의 음식 특집! 더위 방지를 위해서라도 많이 먹는 곳은 빠져야\n特技 : 《요리／취미6》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("여름방학의 아이들과 접촉하는 프로그램에 불린다. 아이들 상대도 큰 일이다.\n特技 : 《배려／재능9》")),
    (46, TableItem::Text("여름의 여행 프로그램. 여름에만 가능한 현지의 강점을 몇 가지 있을 것이다.\n特技 : 씬 플레이어가 보유한 출신분야의특기")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("여름이니까 뜨끈뜨끈한 욕실이 난처할 수 있다. 리액션을 찍고 싶은 거 같다.\n特技 : 《바보／캐릭터12》")),
    (66, TableItem::Text("오프")),
];

static KO_SU: D66Table = D66Table::new("정열의 여름 업무표", D66SortType::Asc, KO_SU_ITEMS);

static KO_WI_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("크리스마스 테마의 커머셜 송을 노래한다. 연인들에게 축복을!\n特技 : 《패션／속성8》")),
    (13, TableItem::Text("스키장의 커머셜 무비를 찍는다. 잘 탈 수 있으면 좋겠다.\n特技 : 《스포츠／취미4》")),
    (14, TableItem::Text("씁쓸한 실연을 테마로 한 커머셜 무비를 찍게 되었다. 쿨하게 가자.\n特技 : 《쿨／속성11》")),
    (15, TableItem::Text("스케이트 링크의 커머셜 무비에 출연. 빙상에서 화려하게 댄스를 하자.\n特技 : 《댄스／취미9》")),
    (16, TableItem::Text("아이스크림의 커머셜 무비에 출현. 추운 것을 참아라!\n特技 : 《담력／재능5》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("연말 연시에 운행되는 철도 커머셜 무비에 기용된다. 깔끔한 연기가 필요하다.\n特技 : 《플레인／속성7》")),
    (24, TableItem::Text("온천지의 커머셜. 온천에 필요한 것은 역시 섹시함일까?\n特技 : 《섹시／속성4》")),
    (25, TableItem::Text("겨울 패션을 잡지로 소개하게 되었다. 겨울 코디는 이것으로 정한다!\n特技 : 《멋내기／취미5》")),
    (26, TableItem::Text("눈치우기가 큰일임을 알린다, 도호쿠의 눈치우기를 돕는다. 이것은 중노동이다.\n特技 : 《도호쿠／출신10》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("연말 버라이어티 프로그램에서, 칸사이 연예인과 콩트를 하게 되었다. 말을 맞추자.\n特技 : 《킨키／출신6》")),
    (35, TableItem::Text("연말 페스티벌에 유명 아이돌 들과 공동출연 스타에게 지지 않는 박력을 내자.\n特技 : 《스타／속성12》")),
    (36, TableItem::Text("겨울 홋카이도 음식을 알리기 위해서, 홋카이도로 발 빠르게 가자. 과연, 추위에 견딜 수 있는 것 인가.\n特技 : 《훗카이도／출신11》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("겨울의 음식 특집. 냄비요리를 만드는 방법을 가르칩니다.\n特技 : 《요리／취미6》")),
    (46, TableItem::Text("발렌타인으로 향하는 여자아이에게 어드바이스.\n特技 : 《페미닌／속성5》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("겨울이니까 뜨끈뜨끈한 욕실에 찾아본다. 따뜻하다고 할까 뜨겁다!\n特技 : 《바보／캐릭터12》")),
    (66, TableItem::Text("오프")),
];

static KO_WI: D66Table = D66Table::new("온기의 겨울 업무표", D66SortType::Asc, KO_WI_ITEMS);

static KO_NA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("계류에서 낚시 대결! 많이 낚시하는 쪽이 승리!\n特技 : 《집중력／재능4》")),
    (13, TableItem::Text("해안에서 낚시를 한다. 낚은 생선이 맛있는 쪽이 이긴다.\n特技 : 《담력／재능5》")),
    (14, TableItem::Text("곤충채집에 도전. 건강하게 노는 그림을 찍고 싶다.\n特技 : 《건강／캐릭터8》")),
    (15, TableItem::Text("캠프를 하자. 모두 쾌적하게 숙박을 할 수 있도록 하는 배려가 중요.\n特技 : 《배려／재능9》")),
    (16, TableItem::Text("바다 헤엄의 대결. 자신의 페이스를 지키면서 싸우자.\n特技 : 《마이페이스／캐릭터4》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("숲에서 헌팅 대결! 동물에게 인기 있는 쪽이 승리!\n特技 : 《펫／취미3》")),
    (24, TableItem::Text("숲에서 술래잡기를 하게 되었다. 상대의 움직임을 읽으면 이길 수 있다!\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (25, TableItem::Text("숲 속 동물과의 싸움이 시작되었다. 아니 할 수 밖에 없다.\n特技 : 《운동신경／재능8》")),
    (26, TableItem::Text("오프")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("요리할 수 있는 남자를 여기서 어필! 반합취사에 도전!\n特技 : 《요리／취미6》")),
    (35, TableItem::Text("등산 대결. 빨리 오르는 것보다도 다치지 않게 조심하고 싶다.\n特技 : 《체력／재능6》")),
    (36, TableItem::Text("땔감 줍기. 수수한 장면이 되어 버리므로 싫증을 나지 않게 하자!\n特技 : 《캐릭터분야의 공백／캐릭터7》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("시청자들의 기뻐할 이 자리에서만의 이야기를 파트너 플레이어와 이야기 한다.\n特技 : 파트너 플레이어가 보유한 캐릭터분야의특기")),
    (46, TableItem::Text("사소한 일로 파트너 플레이어와 싸움이 났다. 어느 쪽이 강한지 승부다!\n特技 : 파트너 플레이어가 보유한 재능분야의특기")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("드럼통 욕조를 준비하자. ……뜨겁지만!\n特技 : 《버닝／속성10》")),
    (66, TableItem::Text("오프")),
];

static KO_NA: D66Table = D66Table::new("대자연 업무표", D66SortType::Asc, KO_NA_ITEMS);

static KO_GA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("学園が主催しているアイドル触れ合いイベントに出演。美しく振舞おう。\n特技 : 《スタイル／才能3》")),
    (23, TableItem::Text("学園のアイドルたちが出ているラジオに出演。先輩たちに負けないように、がんばろう。\n特技 : 《元気／キャラ8》")),
    (24, TableItem::Text("学園と提携しているブランドのファッションショーに登場。\n特技 : 《おしゃれ／趣味5》")),
    (25, TableItem::Text("学園を紹介するDVDに出演。うまく魅力を紹介できるかな？\n特技 : 《気配り／才能9》")),
    (26, TableItem::Text("学内オーディションに出場。勝ち抜けば、歌番組に出場できる！\n特技 : 《胆力／才能5》")),
    (33, TableItem::Text("学園が制作しているアイドルドラマに吸血鬼役で出演。恐ろし気な演技、できるかな？\n特技 : 《オカルト／趣味2》")),
    (34, TableItem::Text("学園が制作しているドラマに出演。演技の中で、どうやって個性を出していこうか。\n特技 : キャラ分野からランダム")),
    (35, TableItem::Text("学園主催のミニライブに出演。たくさんの出演者の中から、どうやって目立とう。\n特技 : キャラ分野からランダム")),
    (36, TableItem::Text("学園の先輩たちと共演するライブイベントに出演。\n特技 : 《胆力／才能5》")),
    (44, TableItem::Text("学園の紹介で、おいしい芋の紹介番組に出演。北海道に出発だ！\n特技 : 《北海道／出身11》")),
    (45, TableItem::Text("学内オーディションで、ポップなCMのイメージガールを決定。戦い抜こう。\n特技 : 《ポップ／属性9》")),
    (46, TableItem::Text("学内のミュージカルに出演することになった。自分たちの実力を舞台の上で発揮しよう。\n特技 : 《演技力／才能12》")),
    (55, TableItem::Text("市内の店舗を手伝うドキュメンタリー番組を撮ることに。お店を手伝おう。\n特技 : 《物腰丁寧／キャラ10》")),
    (56, TableItem::Text("市内のスタジオで収録されている朝の情報番組に出演。朝から元気に行こう。\n特技 : 《元気／キャラ8》")),
    (66, TableItem::Text("裏山を使った簡単なPV撮影！　山での撮影は体力が要求される。\n特技 : 《体力／才能6》")),
];

static KO_GA: D66Table = D66Table::new("聖デトワール女学園仕事表", D66SortType::Asc, KO_GA_ITEMS);

static KO_BA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("アカデミーの中でも、特に優秀な成績を収めた者を表彰する式が開催される。\n特技 : 《スター／属性12》")),
    (23, TableItem::Text("アカデミー主催の音楽祭に出演。楽器を演奏して、その姿で魅せよう。\n特技 : 《音楽／趣味11》")),
    (24, TableItem::Text("アカデミーが製作しているドラマに出演。脇役だけど、しっかりと存在感を出していこう。\n特技 : 《演技力／才能12》")),
    (25, TableItem::Text("番組の企画で、アカデミー生のアクションを見せることになった。\n特技 : 《運動神経／才能9》")),
    (26, TableItem::Text("番組の1コーナーで、アカデミー生が様々なスポーツに挑戦する必要があるらしい。がんばろう。\n特技 : 《スポーツ／趣味4》")),
    (33, TableItem::Text("先輩と一緒にバラエティ番組に出演。面白いリアクションを期待される。\n特技 : 《ばか／キャラ10》")),
    (34, TableItem::Text("先輩のライブでバックコーラスに参加。美しい声を添えよう。\n特技 : 《音楽／趣味11》")),
    (35, TableItem::Text("先輩のライブでバックダンサーとして出演。ダイナミックな演出に負けないようにしよう。\n特技 : 《ダンス／趣味9》")),
    (36, TableItem::Text("先輩が主演を務めているアニメ映画に脇役の声優として出演。光る演技を見せよう。\n特技 : 《演技力／才能12》")),
    (44, TableItem::Text("同級生と一緒に、漫画作品をモデルにしたミュージカルに出演。熱い気合を求められる。\n特技 : 《バーニング／属性10》")),
    (45, TableItem::Text("同級生と一緒にキャラ付けの強いビジュアル系バンドを組んで、試験のステージで発表。\n特技 : キャラ分野からランダム")),
    (46, TableItem::Text("同級生とファッションを競い合う、セルフプロデュース試験が始まる。\n特技 : 《おしゃれ／趣味5》")),
    (55, TableItem::Text("一般教養の試験が始まる。恐ろしいことに、その様子を生中継するらしい。\n特技 : 《学力／才能10》")),
    (56, TableItem::Text("アイドルの歴史を使った、クイズ試験が始まる。\n特技 : 《アイドル／趣味12》")),
    (66, TableItem::Text("試験のテーマは耽美。セクシーさを仲間と競い合おう。\n特技 : 《セクシー／属性4》")),
];

static KO_BA: D66Table = D66Table::new("アカデミー仕事表", D66SortType::Asc, KO_BA_ITEMS);

static KO_WT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("악수회가 시작된다. 아이돌로서는 중요한 순간, 집중하자.\n特技 : 《아이돌／취미12》")),
    (13, TableItem::Text("파트너 플레이어의 「싫은 것」 에 대해 취재를 하게 되었다. 괜찮을까……?\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (14, TableItem::Text("씬 플레이어의 「좋아하는 것」에 관한 프로그램 일이다! 텐션 오른다.\n特技 : 씬 플레이어가 보유한 취미분야의특기")),
    (15, TableItem::Text("파트너 플레이어의 「패션 특징」을 살리는 일이 왔다. 파트너.\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (16, TableItem::Text("클라이언트로부터 파트너 플레이어의 「개성특기」를 헤아려 보자고 제안 받는다.\n特技 : 파트너 플레이어의 개성특기")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("학원 드라마를 촬영! 둘의 캐릭터는 어떻게 될까?\n特技 : キャラ分野からランダム")),
    (24, TableItem::Text("드라마 액션씬을 찍게 되었다. 콤비네이션으로 헤쳐나가자!\n特技 : 身長分野からランダム")),
    (25, TableItem::Text("감동계열의 드라마에 출연. 어떤 능력이 필요하게 될까?\n特技 : 才能分野からランダム")),
    (26, TableItem::Text("취미 프로그램에 출연. 이런 세계가 있는 것인가…….\n特技 : 趣味分野からランダム")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("아이돌이 우글거리는 버라이어티 프로그램에 참전! 어느 아이돌보다 높은 점수를 받아야 할 것 같다!\n特技 : 《바보／캐릭터12》")),
    (35, TableItem::Text("스포츠 프로그램을 떠들썩하게 하기 위해 불려졌다! 스포츠에 대해서 압니까?\n特技 : 《스포츠／취미4》")),
    (36, TableItem::Text("퀴즈 프로그램. 둘이서 게스트로 출연. 힘을 합쳐 이겨낼지, 그렇지 않으면 자신다움을 중시해야 할까…….\n特技 : 《학력／재능10》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("요리프로그램에 출현. 어떤 요리를 만들까?\n特技 : 《요리／취미8》")),
    (46, TableItem::Text("CD샵에서 사인회를 개최. 팬을 기쁘게 할 수 있을까?\n特技 : 《배려／재능9》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("둘에게 그라비아 촬영 일이 왔다. 좋은 스타일로 매료 시킬 찬스?\n特技 : 《스타일／재능3》")),
    (66, TableItem::Text("오프")),
];

static KO_WT: D66Table = D66Table::new("업무표", D66SortType::Asc, KO_WT_ITEMS);

static KO_VA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("먹는 것에 관한 리포트를 하는 프로그램에 출연. 어떻게 해야 맛을 잘 표현할 수 있을까?\n特技 : 《요리／취미6》")),
    (13, TableItem::Text("스포츠를 하는 프로그램에 출연. 얼마나 활약할 수 있을 지 시험 받는다.\n特技 : 《스포츠／취미4》")),
    (14, TableItem::Text("토크 프로그램에 출연. 두 명의 말할 때 잘 이끌어 줄 수 있을까?\n特技 : 《배려／재능9》")),
    (15, TableItem::Text("맹수의 우리에게 들어가 볼 놀이를 한다. 담력이 중요!\n特技 : 《담력／재능5》")),
    (16, TableItem::Text("파트너 플레이어의 「좋아하는 것」를 소재로 한 프로그램을 맡았다.\n特技 : 파트너 플레이어가 보유한  \t취미분야의특기")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("거리는 걷는 프로그램에 출연. 경쾌한 토크로 일반인들을 상대로 능숙하게 말을 걸어보자\n特技 : 《유순／캐릭터5》")),
    (24, TableItem::Text("낚시 프로그램에 나오게 되었지만, 거물을 낚을 때까지는 돌아갈 수 없다고 한다. 그런데 움직임의 적은 낚시로 어떻게 눈에 띌까?\n特技 : 《캐릭터분야의 공백／캐릭터7》")),
    (25, TableItem::Text("파트너 플레이어의 「좋아하는 것」를 소재로 한 프로그램 코너로, 장난 치는 역을 맡게 되었다.\n特技 : 파트너 플레이어가 보유한 캐릭터분야의특기")),
    (26, TableItem::Text("퀴즈 프로그램에 출현. 문제를 어떻게 맞추는지 그것이 중요하다.\n特技 : 《학력／재능10》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("농사일 체험 프로그램에 출연. 괭이를 가지고 밭으로 가자.\n特技 : 《체력／재능6》")),
    (35, TableItem::Text("공작 체험 프로그램에 출연. 좋은걸 스튜디오에 전달할 수 있도록 물건 만들기를 진지하게 실시하자.\n特技 : 《집중력／재능4》")),
    (36, TableItem::Text("전자게임이나 아날로그게임을 플레이 하는 프로그램에 출연. 어떤 식으로 북돋울 수 있을까?\n特技 : 《게임／취미10》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("오늘은 만담을 하는 것 같다. 웃기려면 무엇이 필요할 것 일까?\n特技 : 《바보／캐릭터12》")),
    (46, TableItem::Text("시츄에이션 코너에 출현. 거창한 연기가 요구 된다.\n特技 : 《연기력／재능12》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("취미에 관한 프로그램에 나오게 되었다. 오늘의 테마는 무엇이지.\n特技 : 《취미분야의 공백／취미7》")),
    (66, TableItem::Text("오프")),
];

static KO_VA: D66Table = D66Table::new("버라이어티 업무표", D66SortType::Asc, KO_VA_ITEMS);

static KO_MU_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("씬 플레이어와 파트너 플레이어가 미니라이브를 하게 되었다. 서로 소리를 잘 맞추어 보도록 하자.\n特技 : 파트너 플레이어가 보유한속성분야의특기")),
    (13, TableItem::Text("가요 프로그램에서 다루어진다. 어떤 기분으로 노래했는지 잘 표현해 전하자.\n特技 : 《패션／속성8》")),
    (14, TableItem::Text("파트너 플레이어와 하모니를 거듭하는 노래가 왔다. 둘의 특징을 목소리에 잘 실리도록 하자.\n特技 : 파트너 플레이어가 보유한재능분야의특기")),
    (15, TableItem::Text("CD를 손수 전하는 판매를 개시. 마음을 담아 웃는 얼굴로 손수 전하자.\n特技 : 《미소／재능7》")),
    (16, TableItem::Text("레코딩 음악 업계의 거물이 입회한다. 긴장하지 말고 자신의 실력을 발휘하자.\n特技 : 《담력／재능5》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("파트너 플레이어하고 서로 이야기하며 가사를 만드는 것에 집중한다. 서로를 잘 이해하도록 하자,\n特技 : 파트너 플레이어가 보유한캐릭터분야의특기")),
    (24, TableItem::Text("활동 범위를 넓히기 위해 다양한 악기에 도전!\n特技 : 《음악／취미11》")),
    (25, TableItem::Text("고급스런 바에서 노래하는 것. 고급감이 있는 패션을 몸에 익히는 것을 조건으로 노래하는 것이 용서된다.\n特技 : 《기품／재능11》")),
    (26, TableItem::Text("중학생이 타겟으로 CD를 판매하게 되었다. 마음 속의 중학생을 해방할 때가 왔다!\n特技 : 《중2병／캐릭터2》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("어린이용의 노래를 만들게 되었다. 시선을 아이에게 맞추지 않으면.\n特技 : 《～125／신장2》")),
    (35, TableItem::Text("결혼식의 노래를 맡는다. 축복의 기분을 담아 노래하자.\n特技 : 《배려／재능9》")),
    (36, TableItem::Text("레이스를 북돋우는 업 템포인 곡을 만드는 것이다.\n特技 : 《건강／캐릭터8》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("파트너 플레이어의 현지를 상징하는 노래를 만들게 되었다. 현지의 이야기를 듣기 시작하자.\n特技 : 파트너 플레이어가 보유한출신분야의특기")),
    (46, TableItem::Text("호러 무비의 주제가를 담당하게 된, 무서움을 소리로 표현할 수 있을까?\n特技 : 《오컬트／취미2》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("작곡자 선생님과의 회의. 자신의 이미지를 잘 전달해 줄 수 있을까?\n特技 : 《언행정중／캐릭터10》")),
    (66, TableItem::Text("오프")),
];

static KO_MU: D66Table = D66Table::new("음악 관련 업무표", D66SortType::Asc, KO_MU_ITEMS);

static KO_DR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("엑스트라로 출연. 가능한 한 눈에 띄지 않게 하자.\n特技 : 《플레인／속성7》")),
    (13, TableItem::Text("형사 드라마에 형사 역으로 출연. 쿨하게 하자.\n特技 : 《쿨／속성11》")),
    (14, TableItem::Text("형사 드라마에 범인 역으로 출연. 나쁜 것을 강조하는 연기가 필요하다.\n特技 : 《ミステリアス／캐릭터3》")),
    (15, TableItem::Text("의료 드라마에 의사 역으로 출연. 장기나 피 등에도 꿋꿋하게 힘내자.\n特技 : 《담력／재능5》")),
    (16, TableItem::Text("재현 애니메이션의 더빙에 도전. 가능한 한 정중하게.\n特技 : 《언행정중／캐릭터10》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("악역에 도전. 어두운 기분이 필요하다.\n特技 : 《다크／속성3》")),
    (24, TableItem::Text("학원 드라마에 고뇌하는 학생 역으로 출연. 청춘인 것 같음을 어디까지 보여야?\n特技 : 《중2병／캐릭터2》")),
    (25, TableItem::Text("시대극에 출연. 강경파가 된 씬의 차례가 왔다.\n特技 : 《외고집／캐릭터9》")),
    (26, TableItem::Text("특수 촬영에 히어로 역으로 출연. 뜨거운 연기를 보일 때!\n特技 : 《버닝／속성10》")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("출연진 중에 초유명인이! 긴장하지 말고 힘내자.\n特技 : 《마이페이스／캐릭터4》")),
    (35, TableItem::Text("까다로운 감독에게 여러 가지 주의를 받는다. 어떻게 대응해 할 것인가\n特技 : 《스타／속성12》")),
    (36, TableItem::Text("파트너 플레이어의 [배경]을 재현한 미니 드라마를 찍는 것 같다. 그의 과거를 어떻게 표현할 것 인가.\n特技 : 파트너 플레이어가 보유한속성분야의특기")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("액션 드라마에 출연. 액션 씬을 잘 할 수 있을 지가 키 포인트다.\n特技 : 《운동신경／재능8》")),
    (46, TableItem::Text("연애 드라마에 출연. 출연진를 두근두근 시키는 연기가 필요라는 것.\n特技 : 《섹시／속성4》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("감동계 드라마에 출연. 우는 씬이 여기의 제일 볼만한 장면이다!\n特技 : 《연기력／재능12》")),
    (66, TableItem::Text("오프")),
];

static KO_DR: D66Table = D66Table::new("드라마 관련 업무표", D66SortType::Asc, KO_DR_ITEMS);

static KO_VI_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("비치에서 그라비아 촬영의 일이 생긴다. 육체미를 보여주어야.\n特技 : 《섹시／속성4》")),
    (13, TableItem::Text("패션잡지의 취재가 오고, 자신다운 패션을 보이면 좋겠다고 부탁받는다.\n特技 : 《캐릭터분야의 공백／캐릭터7》")),
    (14, TableItem::Text("지방마다의 패션을 도입하는 패션 쇼가 개막. 출연하게 되었다.\n特技 : 出身分野からランダム")),
    (15, TableItem::Text("패션 쇼에 모델로 등장. 완벽한 스타일을 과시해라.\n特技 : 《스타일／재능3》")),
    (16, TableItem::Text("잡지로 씬 플레이어 추천 코디를 소개한다. 어떤 조합을 소개 할까?\n特技 : 《멋내기／취미5》")),
    (22, TableItem::Text("오프")),
    (23, TableItem::Text("파트너 플레이어의 「신체적 특징」이 약간 유행이 되었다. 이 빅 웨이브에 편승하려면\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (24, TableItem::Text("심야 프로그램 코너에, 씬 플레이어의 「신체적 특징」 특집이 짜여 지는 것 같다.그것을 나타내야한다.\n特技 : 씬 플레이어가 보유한 신장분야의특기")),
    (25, TableItem::Text("뉴스 프로그램 코너에서, 파트너 플레이어의 「패션 특징」이 소개된다. 잘 어시스트 하자.\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (26, TableItem::Text("씬 플레이어의 「패션 특징」을 팔고 있는 기업으로부터 CM에 대해 부탁 받는다.\n特技 : 씬 플레이어가 보유한 속성분야의특기")),
    (33, TableItem::Text("오프")),
    (34, TableItem::Text("여성의 패션에 대해 말하는 프로그램에 출연.\n特技 : 《페미닌／속성5》")),
    (35, TableItem::Text("높은 신장 전용의 의상이 입게 된다. 이것을 입고 잘 나타낼 수 있으려나?\n特技 : 《190～／신장12》")),
    (36, TableItem::Text("TV프로에서 애니메이션 캐릭의 코스프레를 한다. 자신답게 결정 포즈.\n特技 : 《취미분야의 공백／취미7》")),
    (44, TableItem::Text("오프")),
    (45, TableItem::Text("작은 신장을 위한 코디를 만든 디자이너로부터 연락이 들어온다. 그것을 보기 좋고 맵시 있게 입었으면 좋다는 것 같다.\n特技 : 《146／신장6》")),
    (46, TableItem::Text("아이와 공동으로 출연하는 CM를 찍게 되었다. 아이의 귀여운 패션을 생각하자.\n特技 : 《～125／신장2》")),
    (55, TableItem::Text("오프")),
    (56, TableItem::Text("파트너 플레이어의 선전용 촬영의 코디를 하게 되었다. 그런데 어떻게 해야?\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (66, TableItem::Text("오프")),
];

static KO_VI: D66Table = D66Table::new("비주얼 관련 업무표", D66SortType::Asc, KO_VI_ITEMS);

static KO_SP_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("오프")),
    (13, TableItem::Text("오프")),
    (14, TableItem::Text("오프")),
    (15, TableItem::Text("오프")),
    (16, TableItem::Text("오프")),
    (22, TableItem::Text("マラソン大会に参加することになった。とにかく、走ろう。\n特技 : 《체력／재능6》")),
    (23, TableItem::Text("サッカー選手たちにインタビュー！　サッカーの魅力を聞き出そう。\n特技 : 《스포츠／취미4》")),
    (24, TableItem::Text("野球の始球式をすることになった。自分らしく、캐릭터クターを前面に出して投げるのがコツ。\n特技 : 《캐릭터분야의 공백／캐릭터7》")),
    (25, TableItem::Text("バスケットボールを体験！　その魅力を伝えよう。\n特技 : 《스포츠／취미4》")),
    (26, TableItem::Text("ラグビーのパワフルさを間近で体験。見ている人たちにも迫力を伝えないと。\n特技 : 《스포츠／취미4》")),
    (33, TableItem::Text("アメフトのハーフタイムショーの短い時間を任される。集中して魅力を出し切ろう。\n特技 : 《집중력／재능4》")),
    (34, TableItem::Text("チアリーディング（男性アイドルは応援団）で스포츠選手たちの応援をすることに。みんながんばれ！\n特技 : 《キュート／속성6》")),
    (35, TableItem::Text("陸上競技を一通り体験！　それぞれの種目の見どころを発信しよう。\n特技 : 《운동신경／재능8》")),
    (36, TableItem::Text("水泳をする仕事がやってきた。競泳水着をカッコよく着こなそう。\n特技 : 《쿨／속성11》")),
    (44, TableItem::Text("스포츠ドリンクのCMだ。「生き返る」感じを出していこう。\n特技 : 《패션／속성8》")),
    (45, TableItem::Text("運動靴のCM。パートナープレイヤーを力強く追い抜いて、速くなれることをアピール！\n特技 : 《버닝／속성10》")),
    (46, TableItem::Text("ジャージや体操服のCMが入ってきた。うまく着こなして、運動着も멋내기なことを証明しよう。\n特技 : 《멋내기／취미5》")),
    (55, TableItem::Text("近々開催される大会の応援団長を任された。出場する選手たちにエールを送ろう！\n特技 : 《건강／캐릭터8》")),
    (56, TableItem::Text("テニスの試合をパートナープレイヤーとやることになった。ダブルスでいこう。\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (66, TableItem::Text("ゴルフコースを回りながら、プロから手ほどきを受けることに。丁寧な言葉遣いで、大人っぽく振舞おう。\n特技 : 《언행정중／캐릭터10》")),
];

static KO_SP: D66Table = D66Table::new("스포츠 업무표", D66SortType::Asc, KO_SP_ITEMS);

static KO_CHR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("오프")),
    (13, TableItem::Text("오프")),
    (14, TableItem::Text("오프")),
    (15, TableItem::Text("오프")),
    (16, TableItem::Text("오프")),
    (22, TableItem::Text("雪の積もる野外コンサートホールでミニライブ。苛酷な環境だけど、耐え抜かないと。\n特技 : 《체력／재능6》")),
    (23, TableItem::Text("ラジオの公開録音中に、クリスマスケーキ作りにチャレンジすることになった。うまく作れるかな？\n特技 : 《요리／취미6》")),
    (24, TableItem::Text("アイドルが提案するクリスマスデート用のファッションを雑誌で紹介。勝てるコーデを考えてみよう。\n特技 : 《멋내기／취미5》")),
    (25, TableItem::Text("ケーキ屋さんと提携して、クリスマスケーキの売り込みをすることに。\n特技 : 《쇼핑／취미8》")),
    (26, TableItem::Text("地元の中学校のクリスマスイベントに登場。学生のみんなと一緒に盛り上がろう。\n特技 : 《패션／속성8》")),
    (33, TableItem::Text("クリスマスに放映される特別ドラマにパートナープレイヤーの恋人役で出演。恋愛をうまく演じられるかな？\n特技 : 《연기력／재능12》")),
    (34, TableItem::Text("トークバラエティのクリスマス特番に呼ばれて収録を始める。本日のテーマは、「恋」について。\n特技 : 《유순／캐릭터5》")),
    (35, TableItem::Text("ラジオ番組で、家族と過ごすクリスマスの思い出について語ることになった。\n特技 : 《이국문화／재능2》")),
    (36, TableItem::Text("セクシーサンタグランプリというファッション大会に出ることになった。セクシーアイドルたちと対決だ！\n特技 : 《섹시／속성4》")),
    (44, TableItem::Text("遊園地で行われるクリスマスイベントのCMを撮影することになった。楽しそうな미소を見せよう。\n特技 : 《미소／재능7》")),
    (45, TableItem::Text("サンタクロースの格好をして、小学生たちにプレゼントを配ることになった。오프ァーはかわいさ重視！\n特技 : 《キュート／속성6》")),
    (46, TableItem::Text("サンタクロースの格好をして、街でイベントをすることに。異国っぽさをうまく出せるかな？\n特技 : 《해외／출신12》")),
    (55, TableItem::Text("クリスマスをテーマにした写真集が発売。そのうちの何枚かを担当することに。\n特技 : 《스타일／재능3》")),
    (56, TableItem::Text("新人アイドルたちが歌うクリスマスソングを収めたカバーアルバムが発売。自分たちも収録されています。\n特技 : 《음악／취미11》")),
    (66, TableItem::Text("アイドルとデートをした気分になれるDVDが発売。自分たちも、クリスマス編の収録を行った。\n特技 : 《아이돌／취미12》")),
];

static KO_CHR: D66Table = D66Table::new("クリスマス 업무표", D66SortType::Asc, KO_CHR_ITEMS);

static KO_PAR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("오프")),
    (12, TableItem::Text("오프")),
    (13, TableItem::Text("오프")),
    (14, TableItem::Text("오프")),
    (15, TableItem::Text("오프")),
    (16, TableItem::Text("오프")),
    (22, TableItem::Text("パートナープレイヤーの【背景】に関する仕事がやって来る。こいつは何を見てきたんだ？\n特技 : 파트너 플레이어의 개성특기")),
    (23, TableItem::Text("パートナープレイヤーの「好きなもの」に関する仕事がやって来る。場を盛り上げていこう。\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (24, TableItem::Text("パートナープレイヤーの「嫌いなもの」に関する仕事がやって来る。どうフォローしたものか。\n特技 : 파트너 플레이어가 보유한 캐릭터분야의특기")),
    (25, TableItem::Text("パートナープレイヤーの「身体的特徴」に関する仕事がやって来る。どこがいいのかを聞いてみる。\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (26, TableItem::Text("パートナープレイヤーの「ファッション特徴」に関する仕事がやって来る。自分も真似をすることに。\n特技 : 파트너 플레이어가 보유한 신장분야의특기")),
    (33, TableItem::Text("파트너 플레이어의 개성특기に関する仕事がやって来る。合わせてみよう。\n特技 : 파트너 플레이어의 개성특기")),
    (34, TableItem::Text("パートナープレイヤーの「身体的特徴」に関する仕事がやって来る。どこがいいのかを聞いてみる。\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (35, TableItem::Text("파트너 플레이어가 보유한 속성분야의특기に関する仕事がやって来る。\n特技 : 파트너 플레이어가 보유한 속성분야의특기")),
    (36, TableItem::Text("파트너 플레이어가 보유한 캐릭터분야의특기に関する仕事がやって来る。\n特技 : 파트너 플레이어가 보유한 캐릭터분야의특기")),
    (44, TableItem::Text("씬 플레이어의【背景】を振り返らせるような仕事がやって来た。今はアイドルとしてそれをこなそう。\n特技 : 씬 플레이어의個性特技")),
    (45, TableItem::Text("파트너 플레이어가 보유한 취미분야의특기に関する仕事がやって来る。\n特技 : 파트너 플레이어가 보유한 취미분야의특기")),
    (46, TableItem::Text("파트너 플레이어가 보유한 출신분야의특기に関する仕事がやって来る。\n特技 : 파트너 플레이어가 보유한 출신분야의특기")),
    (55, TableItem::Text("씬 플레이어의個性特技に関する仕事がやって来る。今こそ見せ場だ！\n特技 : 씬 플레이어의個性特技")),
    (56, TableItem::Text("파트너 플레이어가 보유한 재능분야의특기に関する仕事がやって来る。\n特技 : 파트너 플레이어가 보유한 재능분야의특기")),
    (66, TableItem::Text("씬 플레이어의「好きなもの」に関する仕事がやって来る。やったぜ！\n特技 : 씬 플레이어의個性特技")),
];

static KO_PAR: D66Table = D66Table::new("パートナー関係 업무표", D66SortType::Asc, KO_PAR_ITEMS);

static KO_SW_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("おいし過ぎて止まらない様子を描く、ふわふわなお菓子のCMを行う。\n特技 : 《ポップ／属性9》")),
    (23, TableItem::Text("苦い恋模様を描く、ビターチョコレートのCM撮影を行う。\n特技 : 《ダーク／属性3》")),
    (24, TableItem::Text("甘い恋模様を描く、甘いチョコレートのCM撮影を行う。\n特技 : 《キュート／属性6》")),
    (25, TableItem::Text("家でポリポリ食べているところを描く、スナック菓子のCMを行う。\n特技 : 《プレーン／属性7》")),
    (26, TableItem::Text("青春の汗を流す様子を描く、甘い飲料水のCMを行う。\n特技 : 《バーニング／属性10》")),
    (33, TableItem::Text("チョコレートを食べて脳細胞を活性化させる探偵のドラマに出演する。\n特技 : 《集中力／才能4》")),
    (34, TableItem::Text("朝のシリアルを食べて元気いっぱいな様子を描く、CM撮影を行う。\n特技 : 《元気／キャラ8》")),
    (35, TableItem::Text("眠いときに目がすっきりと覚める様子を描く、刺激の強いお菓子のCM撮影を行う。\n特技 : 《パッション／属性8》")),
    (36, TableItem::Text("一本で栄養補給ができる様子を描く、健康補助食品なお菓子のCM撮影を行う。\n特技 : 《クール／属性11》")),
    (44, TableItem::Text("地元にある駄菓子屋さんのプロモーションを手伝う。\n特技 : 《ショッピング／趣味8》")),
    (45, TableItem::Text("料理番組で、市販のお菓子を使った新しいおやつの開発を任される。\n特技 : 《料理／趣味6》")),
    (46, TableItem::Text("お菓子の家を再現したアトラクション施設を宣伝する。\n特技 : 《フェミニン／属性5》")),
    (55, TableItem::Text("情報番組の1コーナーで、お勧めのケーキを紹介する。\n特技 : 《料理／趣味6》")),
    (56, TableItem::Text("お菓子をテーマにした、夢いっぱいの遊園地の宣伝を行う。\n特技 : 《笑顔／才能7》")),
    (66, TableItem::Text("チョコレートを使ったグラビア撮影をすることになる。\n特技 : 《スタイル／才能3》")),
];

static KO_SW: D66Table = D66Table::new("お菓子仕事表", D66SortType::Asc, KO_SW_ITEMS);

static KO_AN_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("ライオンの檻に、餌を届ける仕事がやって来る。勇気を出して踏み込もう。\n特技 : 《胆力／才能5》")),
    (23, TableItem::Text("ハムスターと戯れる絵を作る。\n特技 : 《ペット／趣味4》")),
    (24, TableItem::Text("牛の乳しぼり体験を動画にしよう。\n特技 : 《集中力／才能4》")),
    (25, TableItem::Text("かわいい猫の動画を撮るために、きまぐれな猫の機嫌をとりにいく。\n特技 : 《ペット／趣味4》")),
    (26, TableItem::Text("犬の散歩シーンを撮るためにも、犬と信頼関係を作る。\n特技 : 《ペット／趣味4》")),
    (33, TableItem::Text("公園の鳩に餌をあげるシーンの手伝いをする。\n特技 : 《マイペース／キャラ4》")),
    (34, TableItem::Text("象の上に乗って、コメントを述べる仕事。\n特技 : 《エスニック／属性2》")),
    (35, TableItem::Text("ぬるぬるしたどじょうを手づかみする絵を要求される。\n特技 : 《セクシー／属性4》")),
    (36, TableItem::Text("ウサギをなでる絵を作る仕事がやって来る。不安そうなウサギを安心させよう。\n特技 : 《ペット／趣味4》")),
    (44, TableItem::Text("奈良の鹿に餌をあげるドラマに出演。\n特技 : 《近畿地方／出身6》")),
    (45, TableItem::Text("馬に乗って、競馬場を駆ける映像を撮ることに。うまく乗りこなそう。\n特技 : 《セレブ／才能11》")),
    (46, TableItem::Text("水族館でペンギンたちと一緒に遊ぶシーンを撮影。\n特技 : 《キュート／属性6》")),
    (55, TableItem::Text("蛇を手づかみする番組企画が入る。\n特技 : 《胆力／才能5》")),
    (56, TableItem::Text("海に入って、魚や貝を見つける企画をすることになった。\n特技 : 《元気／キャラ8》")),
    (66, TableItem::Text("山奥で歩き回って、色々な昆虫を見つける仕事を行う。\n特技 : 《体力／才能6》")),
];

static KO_AN: D66Table = D66Table::new("動物仕事表", D66SortType::Asc, KO_AN_ITEMS);

static KO_MOV_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("ゾンビ映画にゾンビ役で出演。動く死体らしい演技を心がけよう。\n特技 : 《ダーク／属性3》")),
    (23, TableItem::Text("時代劇映画にサムライ役で出演。厚い忠義を見せよう。\n特技 : 《硬派／キャラ9》")),
    (24, TableItem::Text("西部劇映画にガンマン役で出演。静かに熱い役を演じよう。\n特技 : 《バーニング／属性10》")),
    (25, TableItem::Text("SF映画に未来のエンジニア役で出演。難しい言葉をクールに言い放とう。\n特技 : 《クール／属性11》")),
    (26, TableItem::Text("カンフー映画に若き拳法家役で出演。激しいアクションで敵と戦おう。\n特技 : 《運動神経／才能8》")),
    (33, TableItem::Text("恋愛映画に当て馬役として出演。フラれたあとに感情的になる演技が大事。\n特技 : 《演技力／才能12》")),
    (34, TableItem::Text("現代劇に中学二年生役として出演。現代の若者を演技で表現しよう。\n特技 : 《中二病／キャラ2》")),
    (35, TableItem::Text("特撮ヒーローにヒーロー役として出演。スーツアクターのアクションに、アフレコで魂を載せよう。\n特技 : 《演技力／才能12》")),
    (36, TableItem::Text("ホラー映画に主役として出演。悲鳴や動きで怖がっているところを見せよう。\n特技 : 《ダーク／属性3》")),
    (44, TableItem::Text("インド映画にダンサーとして出演。情熱的な踊りで映画を盛り上げよう。\n特技 : 《ダンス／趣味9》")),
    (45, TableItem::Text("ミステリー映画の犯人役として出演。怪しげな演技で人々を魅了しよう。\n特技 : 《ミステリアス／キャラ3》")),
    (46, TableItem::Text("戦争映画に出演。哀しみの叫びを上げよう。\n特技 : 《演技力／才能12》")),
    (55, TableItem::Text("ちょっとしたお色気シーンを撮ることに。\n特技 : 《セクシー／属性4》")),
    (56, TableItem::Text("ドキュメンタリー映画で、過去の偉人を演じることに。その人の一生をトレースしよう。\n特技 : 《演技力／才能12》")),
    (66, TableItem::Text("おバカな映画に、突き抜けたバカ役として出演。バカになれ！\n特技 : 《ばか／キャラ12》")),
];

static KO_MOV: D66Table = D66Table::new("映画仕事表", D66SortType::Asc, KO_MOV_ITEMS);

static KO_FA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (
        22,
        TableItem::Text(
            "ドラゴンと対峙しても、引かない勇気を見せるシーン。\n特技 : 《胆力／才能5》",
        ),
    ),
    (
        23,
        TableItem::Text("怪物の群れを魔法で一掃するシーン。\n特技 : 《ポップ／属性9》"),
    ),
    (
        24,
        TableItem::Text("剣を使って街を荒らす盗賊たちを成敗するシーン。\n特技 : 《硬派／キャラ9》"),
    ),
    (
        25,
        TableItem::Text("斧を振るって、動く植物を破壊するシーン。\n特技 : 《体力／才能6》"),
    ),
    (
        26,
        TableItem::Text(
            "仲間と一緒に槍で突いて、敵の兵士を追い返すシーン。\n特技 : 《気配り／才能9》",
        ),
    ),
    (
        33,
        TableItem::Text(
            "歌と踊りでファンタジーの住民たちを惹きつけるシーン。\n特技 : 《音楽／趣味11》",
        ),
    ),
    (
        34,
        TableItem::Text("酒場で芸を披露して、人気者になるシーン。\n特技 : 《軟派／キャラ5》"),
    ),
    (
        35,
        TableItem::Text(
            "無実の罪でとらえられ、牢屋から脱出するシーン。\n特技 : 《ミステリアス／キャラ3》",
        ),
    ),
    (
        36,
        TableItem::Text("突然現れた魔物の群れに襲われるシーン。\n特技 : 《どじ／キャラ11》"),
    ),
    (
        44,
        TableItem::Text("巨大な魔物に、みんなで立ち向かっていくシーン。\n特技 : 《気配り／才能9》"),
    ),
    (
        45,
        TableItem::Text("悪の魔法使いによって、呪いをかけられるシーン。\n特技 : 《ダーク／属性3》"),
    ),
    (
        46,
        TableItem::Text("新しい武器と防具を調達して、着こなすシーン。\n特技 : 《おしゃれ／趣味5》"),
    ),
    (
        55,
        TableItem::Text("一面の草原を駆け抜けるシーン。\n特技 : 《元気／キャラ8》"),
    ),
    (
        56,
        TableItem::Text("疲れている王様を元気づけてあげるシーン。\n特技 : 《パッション／属性8》"),
    ),
    (
        66,
        TableItem::Text("空を駆ける不思議な船に乗って、感動するシーン。\n特技 : 《笑顔／才能7》"),
    ),
];

static KO_FA: D66Table = D66Table::new("ファンタジー仕事表", D66SortType::Asc, KO_FA_ITEMS);

static KO_BVT_ITEMS: &[&str] = &[
    "자사내 TV 스튜디오",
    "사내 라이브 스테이지",
    "자사 프로그램",
    "이벤트 회장",
    "쇼핑센터",
    "자사 주최 페스티벌",
];

static KO_BVT: Table = Table::from_dice("대형 예능 프로덕션 회장표", 1, 6, KO_BVT_ITEMS);

static KO_LVT_ITEMS: &[&str] = &[
    "지방 축제",
    "CD샵 앞",
    "소극장",
    "음악 프로그램",
    "로컬 TV",
    "예능 관계사 공동개최 페스티벌",
];

static KO_LVT: Table = Table::from_dice("약소 예능 프로덕션 회장표", 1, 6, KO_LVT_ITEMS);

static KO_TVT_ITEMS: &[&str] = &[
    "라이브 시어터",
    "라이브 시어터",
    "라이브 시어터",
    "라이브 시어터",
    "라이브 시어터",
    "라이브 시어터",
];

static KO_TVT: Table = Table::from_dice("라이브 시어터 회장표", 1, 6, KO_TVT_ITEMS);

static KO_CVT_ITEMS: &[&str] = &[
    "運動場",
    "体育館",
    "屋上",
    "街中",
    "地元のイベント",
    "学園アイドル大会",
];

static KO_CVT: Table = Table::from_dice("아이돌 부 회장표", 1, 6, KO_CVT_ITEMS);

static KO_BST_ITEMS: &[&str] = &[
    "사내 프로젝트 룸",
    "사내 카페",
    "사내 풀",
    "사내 레슨 룸",
    "쇼핑센터",
    "사내 에스테틱 룸",
];

static KO_BST: Table = Table::from_dice("대형 예능 프로덕션 장소표", 1, 6, KO_BST_ITEMS);

static KO_LST_ITEMS: &[&str] = &["급탕실", "응접실", "거실", "옥상", "사장실", "근처공원"];

static KO_LST: Table = Table::from_dice("약소 예능 프로덕션 장소표", 1, 6, KO_LST_ITEMS);

static KO_TST_ITEMS: &[&str] = &[
    "우리의 무대",
    "대기실",
    "매점",
    "극장 앞",
    "레슨 룸",
    "무대 옆",
];

static KO_TST: Table = Table::from_dice("라이브 시어터 장소표", 1, 6, KO_TST_ITEMS);

static KO_CST_ITEMS: &[&str] = &["부실", "음악실", "교실", "옥상", "운동장", "체육관"];

static KO_CST: Table = Table::from_dice("아이돌 부 장소표", 1, 6, KO_CST_ITEMS);

static KO_BPT_ITEMS: &[&str] = &[
    "선배 아이돌의 기대",
    "후배 아이돌의 동경",
    "사장의 시찰",
    "동기 아이돌들과의 경쟁",
    "거물 게스트 등장",
    "TV프로그램의 프로듀서",
];

static KO_BPT: Table =
    Table::from_dice("대형 예능 프로덕션 프레셔 종류 결정표", 1, 6, KO_BPT_ITEMS);

static KO_LPT_ITEMS: &[&str] = &[
    "열심히 라이브 와주는 팬",
    "매우 나쁜 조건의 스테이지",
    "대형 프로덕션의 시찰",
    "드레스 디자이너의 품평",
    "신곡을 제공한 뮤지션",
    "취재하러 온 예능기자",
];

static KO_LPT: Table =
    Table::from_dice("약소 예능 프로덕션 프레셔 종류 결정표", 1, 6, KO_LPT_ITEMS);

static KO_TPT_ITEMS: &[&str] = &[
    "라이브 시어터에 매일 와주는 팬",
    "라이브 시어터에서 일하는 스텝",
    "시어터 경영자의 시찰",
    "시어터에 우연히 들린 많은 관객",
    "병행으로 행해지는 이벤트의 팬",
    "라이벌 시어터의 아이돌 유닛",
];

static KO_TPT: Table = Table::from_dice("라이브 시어터 프레셔 종류 결정표", 1, 6, KO_TPT_ITEMS);

static KO_CPT_ITEMS: &[&str] = &[
    "라이벌 팀 「카이와라교」의 도전",
    "최강 팀 「Tiara's」의 정찰",
    "학부모 교사 협회(PTA)의 시찰",
    "PC의 부모",
    "친한 동급생",
    "라이벌 팀 「성 국제 여학원」의 도전",
];

static KO_CPT: Table = Table::from_dice("아이돌 부 프레셔 종류 결정표", 1, 6, KO_CPT_ITEMS);

static KO_BIT_ITEMS: &[&str] = &[
    "영양 드링크",
    "자동 판매기",
    "선배 아이돌의 포스터",
    "선배 아이돌의 CD",
    "아이돌 잡지",
    "대본",
];

static KO_BIT: Table = Table::from_dice("대형 예능 프로덕션 도구표", 1, 6, KO_BIT_ITEMS);

static KO_LIT_ITEMS: &[&str] = &[
    "셀로판 테이프",
    "냉장고",
    "백엔 동전",
    "너덜너덜한 소파",
    "주먹밥",
    "키친",
];

static KO_LIT: Table = Table::from_dice("약소 예능 프로덕션 도구표", 1, 6, KO_LIT_ITEMS);

static KO_TIT_ITEMS: &[&str] = &[
    "무대의 조명",
    "기획서",
    "PC의 굿즈",
    "화이트 보드",
    "라이브 포스터",
    "우동",
];

static KO_TIT: Table = Table::from_dice("라이브 시어터 도구표", 1, 6, KO_TIT_ITEMS);

static KO_CIT_ITEMS: &[&str] = &[
    "퍼스널 컴퓨터",
    "책상",
    "가방",
    "핸드 카메라",
    "저지",
    "투표함",
];

static KO_CIT: Table = Table::from_dice("아이돌 부 도구표", 1, 6, KO_CIT_ITEMS);

static KO_CHO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("모험이다 / 어드벤처\n이 공연간에, 스페셜치가 1점 감소.")),
    (12, TableItem::Text("온 세상의 사랑 / 러브\nPC전원의【멘탈】이 3점 상승.")),
    (13, TableItem::Text("지금 보내고 싶은 / 기다릴 수 없는\n이 공연의 【퍼포먼스치】가 1점 상승.")),
    (14, TableItem::Text("지지 않는 거야 / 이기고 싶은\n이 공연의 PP가 5 점 감소(최저 0).。")),
    (15, TableItem::Text("감사의 말 / 고마워요\n변조가 모두 회복한다.")),
    (16, TableItem::Text("너라면 / 당신들이\n이 공연 간에 모든 【이해도】가 2점 높은 것으로서 취급한다.")),
    (22, TableItem::Text("동료와 함께라면 / 동료의\nPC전원은 자신 이외의 캐릭터에 대해 【이해도】가 1점 상승.")),
    (23, TableItem::Text("정말로 좋아해 / MAX\nPC 전원의 【멘탈】이 1D6점 상승.。")),
    (24, TableItem::Text("죽고 싶을 정도로 / 어둠으로 떨어져\n이 공연 간에 스페셜치와 펌블치가 1점 감소.")),
    (25, TableItem::Text("이지만 / 에서도、이 공연 간에 단기돌파 목표치가 5점이 된다.")),
    (26, TableItem::Text("키스보다 / 속삭이는 것보다\n이 공연 목록의 간에 【비주얼】이 1점 상승한다.")),
    (33, TableItem::Text("함께 있으면 좋은 / 쭉\n아이돌 클래스가 「훈훈한」인 PC전원은 【추억】을 획득")),
    (34, TableItem::Text("달리다 / 달리는\n이 공연 간에 【피지컬】이 1점 상승한다.")),
    (35, TableItem::Text("기다림에 애태우는 / 언제까지나\n공연 간에 심포니를 실시했을 때 주사위 눈 하나를 1로 변경할 수 있다.")),
    (36, TableItem::Text("한밤 중에 / 한낮에の\n미라클• 미라클 싱크로가 발생했을 때 【퍼포먼스치】에 +5.")),
    (44, TableItem::Text("笑おうぜ／미소で\n아이돌 클래스가 「코메디」인 PC전원은 【획득 팬 인원수】가 [2D6에] 랭크 계수】를 적용한 수만큼 상승.")),
    (45, TableItem::Text("파티다 / 돈으로도\nPC전원은, 아이템을 1개 랜덤에 획득한다.")),
    (46, TableItem::Text("이니까 / 그리고\n공연 간에 단기돌파 이외의 판정 달성치가 1점 상승한다")),
    (55, TableItem::Text("나만 봐 / 독점하고 싶은\n단기돌파를 실시해도 【멘탈】이 감소하지 않는다.")),
    (56, TableItem::Text("우리들의 노래 / 노래하자\n이 공연 간에 【보이스】가 1점 상승한다.。")),
    (66, TableItem::Text("사랑하고 있는 것 / 좋아해\n아이돌 클래스가 「진지한」인 PC전원의 【멘탈】이 5점 상승.")),
];

static KO_CHO: D66Table = D66Table::new("하이라이트 표", D66SortType::Asc, KO_CHO_ITEMS);

static KO_SCH_ITEMS: &[&str] = &[
    "여름은\n이 공연간에 PC전원의 【퍼포먼스치】가 1점 상승.",
    "뜨거운 거야!\n이 공연간에 PC전원의 스페셜치가 1점 감소.",
    "수영복이\n이 공연간에 의상효과에 의해 상승하는 값이 +1.",
    "목 마른\n이 공연간에 PC전원의 펌블치가 3점 상승.",
    "촉촉한\n【멘탈】이 5점 이하인 PC전원은 【멘탈】이 10점 상승.",
    "튀는\n이 공연간에 퍼펙트 미러클의【퍼포먼스치】가 15점 상승.",
];

static KO_SCH: Table = Table::from_dice("정열의 여름 하이라이트 표", 1, 6, KO_SCH_ITEMS);

static KO_WCH_ITEMS: &[&str] = &[
    "눈과 같은\nPC전원의 【멘탈】이 3점 상승.",
    "초콜릿에\nPC1인의 【멘탈】이 10점 상승.",
    "녹여 주는\n이 공연 간, PC전원의 판정의 달성치가 1점 상승.",
    "특별한 날\n이 공연 간, PC1인의 스페셜치가 3점 감소.",
    "눈보라가\n이 공연 간, 미라클의 【퍼포먼스치】가 10점 상승.",
    "추위도 잊어\n이 공연 간, 펌블이 발생해도 변조가 생기지 않는다.",
];

static KO_WCH: Table = Table::from_dice("온기의 겨울 하이라이트 표", 1, 6, KO_WCH_ITEMS);

static KO_NCH_ITEMS: &[&str] = &[
    "야생의\n전원 【멘탈】이 1D6점 상승.",
    "파워로\n이 공연간에 PC 1명의 【퍼포먼스치】가 1D6점 상승.",
    "단련된 몸\n전원 【멘탈】이 3점상승.",
    "잡고 있는\n이 공연 간에 PC 전원 【파포먼스치】가 1점 상승.",
    "부축 하는\n이 공연 간에 PC 1명의 【퍼포먼스치】가 3점 상승.",
    "매일 싸워 나가는\nPC 1명의 【획득 팬 인원수】가 2D6에 【랭크계수】 걸친 수만큼 상승.",
];

static KO_NCH: Table = Table::from_dice("대자연 하이라이트 표", 1, 6, KO_NCH_ITEMS);

static KO_GCH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("女の子だから／キュンキュンしてる\nPC全員の【メンタル】が1D6点上昇する。")),
    (12, TableItem::Text("見つめていたい／心の声\nこの演目の間、【パフォーマンス値】が2点上昇する。")),
    (13, TableItem::Text("私の気持ち／あなたへ届け\nこの演目の間、【協調値】が1点上昇する。")),
    (14, TableItem::Text("繋がりたい／夜を過ごしたい\nPC全員の【メンタル】が1D6点上昇する。")),
    (15, TableItem::Text("手と手を繋いで／みんなと一緒に\nこの演目の間、シンフォニーをするたびに、【メンタル】が5点上昇する。")),
    (16, TableItem::Text("ファッションで／おしゃれして\n衣装の効果が1点上昇する。")),
    (22, TableItem::Text("アイドルだけど／アイドルとして\nこの演目の間、意地判定の達成値が2点上昇する。")),
    (23, TableItem::Text("愛してる／好きです\nこの演目の間、【協調値】が1点上昇する。")),
    (24, TableItem::Text("恋したい／恋してる\nこの演目の間、【協調値】が1点上昇する。")),
    (25, TableItem::Text("LOVE／「大好き」\nこの演目の間、【協調値】が1点上昇する。")),
    (26, TableItem::Text("お母さんには秘密／ヴェールでかくして\nこの演目の間、【メンタル】が減少しない。")),
    (33, TableItem::Text("愛に溺れて／沈んでいく\nこの演目の間、シンフォニーをするたびに、【パフォーマンス値】が2点上昇する。")),
    (34, TableItem::Text("潰してほしい／壊して\nこの演目の間、判定に失敗したPCは【獲得ファン人数】が2D6点上昇する。")),
    (35, TableItem::Text("どんなに遠くに／離れても\nこの演目の間、すべてのギャップは埋まっているものとして扱う。")),
    (36, TableItem::Text("想いを届けて／胸に秘めた鼓動\nPC全員の【メンタル】が1D6点上昇する。")),
    (44, TableItem::Text("私のことが好きなら／一緒に死にたい\nこの演目の間、【メンタル】が0になっても、行動不能にならない。")),
    (45, TableItem::Text("（台詞）／（ピアノソロ）\nPC全員の【メンタル】が1D6点上昇する。")),
    (46, TableItem::Text("せーのっ／いくよー！\nこの演目の間、PCはパフォーマンスのサイコロすべてを一度だけ振り直すことができる。")),
    (55, TableItem::Text("あの日みたいに／あの子のこと忘れて\nこの演目の間、判定に失敗しても、判定のサイコロを一度だけ振り直すことができる。")),
    (56, TableItem::Text("歌を届けよう／声に想いを\nこの演目の間、【パフォーマンス値】が2点上昇する。")),
    (66, TableItem::Text("（ユニット名）／（PCの名前）\n好きな能力値が1点上昇する。")),
];

static KO_GCH: D66Table = D66Table::new("女性向けサビ表", D66SortType::Asc, KO_GCH_ITEMS);

static KO_PCH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バトル／戦いに臨む\nこの演目の間、判定の達成値が2点上昇する。")),
    (12, TableItem::Text("宇宙に／銀河が\nこの演目の間、パフォーマンスのサイコロは取り除かれない。")),
    (13, TableItem::Text("空へ／天に向けて\nこの演目の判定に成功したPCは、【メンタル】が10点上昇する。")),
    (14, TableItem::Text("ぶち壊すぜ／むしゃくしゃして\nこの演目の間、一芸突破を行ったときの目標値が4になる。")),
    (15, TableItem::Text("バイクに乗って／ヘリで飛ばして\nPC全員は、アイテム「キャラアイテム」を1個獲得する。")),
    (16, TableItem::Text("アタック／殴りかかる\nこの演目の間、一芸突破を行ったときの判定の達成値が3点上昇する。")),
    (22, TableItem::Text("情熱／熱情\nこの演目の間、スペシャル値が1点減少。")),
    (23, TableItem::Text("走り切るのさ／星の輝き\nこの演目の間、PCの【メンタル】が減少しない。")),
    (24, TableItem::Text("心赴くまま／願いを込めて\nPC全員の【メンタル】が［自分からの【理解度】の合計］点上昇する。")),
    (25, TableItem::Text("高みへ／打ち破る\nこの演目の間、スペシャル値が1点減少。")),
    (26, TableItem::Text("イメージを／覚悟を\nこの演目の間、スペシャル値が1点減少。")),
    (33, TableItem::Text("弱気な自分に／暗闇裂く\nPC全員は、アイテム「ドリーミングシューズ」を1個獲得する。")),
    (34, TableItem::Text("衝動（リビドー）／強敵（ライバル）\nこの演目の間、スペシャル値が1点減少。")),
    (35, TableItem::Text("覚悟を決めて／クライマックス\nこの演目が最終演目の場合、判定の達成値が4点上昇する。")),
    (36, TableItem::Text("最高の力を／最弱脱ぎ捨て\nこの演目の間、スペシャル値が1点減少。")),
    (44, TableItem::Text("自我（エゴ）／瞬間（とき）\nこの演目の間、判定に失敗しても、判定のサイコロを一度だけ振り直すことができる。")),
    (45, TableItem::Text("（台詞）／（ギターソロ）\nこの演目の間、スペシャル値が1点減少。")),
    (46, TableItem::Text("Let's／try\nこの演目の間、判定の達成値が1点上昇する。")),
    (55, TableItem::Text("起死回生／負けたりしない\nPC全員の【メンタル】が1D6点上昇する。")),
    (56, TableItem::Text("共鳴していく／想いを束ねて\nこの演目の間、シンフォニーするたびに、【パフォーマンス値】が2点上昇する。")),
    (66, TableItem::Text("運命（デスティニー）／正義（ジャスティス）\nこの演目の間、スペシャル値が1点減少。")),
];

static KO_PCH: D66Table = D66Table::new("力強いサビ表", D66SortType::Asc, KO_PCH_ITEMS);

static KO_LUR_ITEMS1: &[&str] = &[
    "地元の商店街で",
    "マスコットキャラクターと",
    "地元のプールで",
    "地元の小学校で",
    "地元のショッピングモールで",
    "田んぼの真ん中で",
];

static KO_LUR_ITEMS2: &[&str] = &[
    "愛について叫ぶ",
    "民謡を歌う",
    "ファッションショー",
    "水着で宣伝",
    "ネット配信",
    "お祭り騒ぎ",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.LUR", ...)`。
static KO_LUR: D6TwiceTable =
    D6TwiceTable::new("로컬 아이돌 엉망진창표", KO_LUR_ITEMS1, KO_LUR_ITEMS2);

static KO_SUR_ITEMS1: &[&str] = &[
    "해수욕장에서",
    "훌륭한 사람의 앞에서",
    "그 유명 아이돌의 앞에서",
    "동료 앞에서",
    "카메라 앞에서",
    "일반관객 앞에서",
];

static KO_SUR_ITEMS2: &[&str] = &[
    "빙수를 단번에 먹는다",
    "헌팅",
    "수박을 깬다.",
    "멋진 포즈",
    "만면에 미소",
    "싸움의 행세",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.SUR", ...)`。
static KO_SUR: D6TwiceTable =
    D6TwiceTable::new("정열의 여름 엉망진창표", KO_SUR_ITEMS1, KO_SUR_ITEMS2);

static KO_WUR_ITEMS1: &[&str] = &[
    "크리스마스 트리 앞에서",
    "아이들 앞에서",
    "폭설 속에서",
    "눈 내리기 시작한 거리에서",
    "따뜻한 방 안에서",
    "난방이 너무 지나친 방에서",
];

static KO_WUR_ITEMS2: &[&str] = &[
    "눈 치우기",
    "아이스크림를 먹는다.",
    "얇게 입고 등장.",
    "노래를 불러 주세요.",
    "산타 코스프레",
    "오뎅을 서둘러 먹는다.",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.WUR", ...)`。
static KO_WUR: D6TwiceTable =
    D6TwiceTable::new("온기의 겨울 엉망진창표", KO_WUR_ITEMS1, KO_WUR_ITEMS2);

static KO_NUR_ITEMS1: &[&str] = &[
    "도끼를 가지고",
    "괭이를 가지고",
    "낚시대를 가지고",
    "포충망를 가지고",
    "영양 드링크 선전을 하면서",
    "생명 줄을 달고서",
];

static KO_NUR_ITEMS2: &[&str] = &[
    "나무를 넘어 뜨리다.",
    "밭을 경작한다.",
    "곤충채집.",
    "거물을 낚시한다.",
    "겁 없이 통과한다.",
    "벼랑을 오른다.",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.NUR", ...)`。
static KO_NUR: D6TwiceTable = D6TwiceTable::new("대자연 엉망진창표", KO_NUR_ITEMS1, KO_NUR_ITEMS2);

static KO_GUR_ITEMS1: &[&str] = &[
    "裏山で",
    "食堂で",
    "先輩の前で",
    "全国放送で",
    "全校生徒の前で",
    "学園の様子を伝えるネット中継で",
];

static KO_GUR_ITEMS2: &[&str] = &[
    "歌を披露",
    "乗馬",
    "テニス",
    "「個性とは何か」を語る",
    "「アイドルとは何か」を語る",
    "「アイドルをやっていてよかった瞬間」を語る",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.GUR", ...)`。
static KO_GUR: D6TwiceTable =
    D6TwiceTable::new("聖デトワール女学園無茶ぶり表", KO_GUR_ITEMS1, KO_GUR_ITEMS2);

static KO_BUR_ITEMS1: &[&str] = &[
    "TVカメラの前で",
    "ライバルと一緒に",
    "試験で",
    "寮で",
    "幼年部で",
    "初等部で",
];

static KO_BUR_ITEMS2: &[&str] = &[
    "反省会",
    "ゲリラライブ",
    "宿題をこなす",
    "食事を作る",
    "自作の歌を披露",
    "自作のポエムを披露",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.BUR", ...)`。
static KO_BUR: D6TwiceTable =
    D6TwiceTable::new("アカデミー無茶ぶり表", KO_BUR_ITEMS1, KO_BUR_ITEMS2);

static KO_ACE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("好きな能力値が1点高いものとして扱う。")),
    (12, TableItem::Text("【ボイス】が1点高いものとして扱う。")),
    (13, TableItem::Text("【フィジカル】が1点高いものとして扱う。")),
    (14, TableItem::Text("【ビジュアル】が1点高いものとして扱う。")),
    (15, TableItem::Text("このアクセサリーを装備したとき、【メンタル】が5点上昇する。この効果は、1回のセッションに1度まで使用できる。")),
    (16, TableItem::Text("【パフォーマンス値】が2点上昇する。")),
    (22, TableItem::Text("このアクセサリーを装備したとき、「アイドルスキル修得表」を使って、ランダムにアイドルスキルを1つ修得する。リザルトフェイズにそのアイドルスキルは失われる。この効果は、1回のセッションに1度まで使用できる。")),
    (23, TableItem::Text("開幕演目と最終演目で行う判定の達成値が2点上昇する。")),
    (24, TableItem::Text("【協調値】が1点上昇する。")),
    (25, TableItem::Text("アイドルスキルの効果が1点上昇する。")),
    (26, TableItem::Text("意地判定の達成値が3点上昇する。")),
    (33, TableItem::Text("特殊な演目によって上昇する【獲得ファン人数】が3点上昇する。")),
    (34, TableItem::Text("一芸突破を行ったとき、判定の達成値が2点上昇する。")),
    (35, TableItem::Text("このアクセサリーを装備したとき、好きな特技を1つ選ぶ。選んだ特技は、ライブフェイズの間、修得しているものとして扱う。この効果は、1回のセッションに1度まで使用できる。")),
    (36, TableItem::Text("幕間での判定の達成値が2点上昇する。")),
    (44, TableItem::Text("思い出を使用したとき、【メンタル】が3点上昇する。")),
    (45, TableItem::Text("ミラクルが発生したときの【パフォーマンス値】が15点になる。")),
    (46, TableItem::Text("アイドルスキルを使用したときの判定の達成値が2点上昇する。")),
    (55, TableItem::Text("特別な演目を行っても、【メンタル】が減少しない。")),
    (56, TableItem::Text("最終演目での【メンタル】減少が半分（端数切り捨て）になる。")),
    (66, TableItem::Text("スペシャルが発生したとき、【メンタル】が10点上昇する。")),
];

static KO_ACE: D66Table = D66Table::new("アクセサリー効果表", D66SortType::Asc, KO_ACE_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表の対応）。
static KO_TABLES: &[(&str, &dyn RollText)] = &[
    ("DT", &KO_DT),
    ("RC", &KO_RC),
    ("FC", &KO_FC),
    ("ACB", &KO_ACB),
    ("TN", &KO_TN),
    ("CG", &KO_CG),
    ("GG", &KO_GG),
    ("HA", &KO_HA),
    ("CBT", &KO_CBT),
    ("RCB", &KO_RCB),
    ("HBT", &KO_HBT),
    ("RHB", &KO_RHB),
    ("RU", &KO_RU),
    ("SIP", &KO_SIP),
    ("BU", &KO_BU),
    ("HW", &KO_HW),
    ("FL", &KO_FL),
    ("MSE", &KO_MSE),
    ("ST", &KO_ST),
    ("FST", &KO_FST),
    ("BWT", &KO_BWT),
    ("LWT", &KO_LWT),
    ("TWT", &KO_TWT),
    ("CWT", &KO_CWT),
    ("SU", &KO_SU),
    ("WI", &KO_WI),
    ("NA", &KO_NA),
    ("GA", &KO_GA),
    ("BA", &KO_BA),
    ("WT", &KO_WT),
    ("VA", &KO_VA),
    ("MU", &KO_MU),
    ("DR", &KO_DR),
    ("VI", &KO_VI),
    ("SP", &KO_SP),
    ("CHR", &KO_CHR),
    ("PAR", &KO_PAR),
    ("SW", &KO_SW),
    ("AN", &KO_AN),
    ("MOV", &KO_MOV),
    ("FA", &KO_FA),
    ("BVT", &KO_BVT),
    ("LVT", &KO_LVT),
    ("TVT", &KO_TVT),
    ("CVT", &KO_CVT),
    ("BST", &KO_BST),
    ("LST", &KO_LST),
    ("TST", &KO_TST),
    ("CST", &KO_CST),
    ("BPT", &KO_BPT),
    ("LPT", &KO_LPT),
    ("TPT", &KO_TPT),
    ("CPT", &KO_CPT),
    ("BIT", &KO_BIT),
    ("LIT", &KO_LIT),
    ("TIT", &KO_TIT),
    ("CIT", &KO_CIT),
    ("CHO", &KO_CHO),
    ("SCH", &KO_SCH),
    ("WCH", &KO_WCH),
    ("NCH", &KO_NCH),
    ("GCH", &KO_GCH),
    ("PCH", &KO_PCH),
    ("LUR", &KO_LUR),
    ("SUR", &KO_SUR),
    ("WUR", &KO_WUR),
    ("NUR", &KO_NUR),
    ("GUR", &KO_GUR),
    ("BUR", &KO_BUR),
    ("ACE", &KO_ACE),
    ("ACT", &KO_ACT),
    ("MS", &KO_MS),
    ("RE", &KO_RE),
    ("SH", &KO_SH),
    ("MO", &KO_MO),
    ("SEA", &KO_SEA),
    ("SPA", &KO_SPA),
    ("LN", &KO_LN),
    ("SGT", &KO_SGT),
    ("RS", &KO_RS),
];

/// `ko_kr` ロケールの表と定型文一式。
pub(crate) static KO_SYSTEM: SystemTables = SystemTables {
    skill_table: &KO_SKILL_TABLE,
    item_table: &KO_ITEM_TABLE,
    bad_status_table: &KO_BAD_STATUS_TABLE,
    local_work_table: &KO_LOCAL_WORK_TABLE,
    tables: KO_TABLES,
    success: "성공",
    failure: "실패",
    fumble: "ファンブル(変調がランダムに1つ発生し、PCは【思い出】を1つ獲得する)",
    special: "스페셜!(PC는 【추억】 1개 획득)",
    burst_name: "バーストタイム",
    burst_burst: "Burst!\n「バースト表」を使用する。",
    burst_critical_success: "大成功\n【獲得ファン人数】が2D6点上昇する。\nPC全員が挑戦者ではない場合、自分以外のPCを一人指名する。指名されたPCは、新たな挑戦者として、【メンタル】を減少させずに「バーストタイム」を行う。",
    burst_success: "成功\n【獲得ファン人数】が2D6点上昇する。",
    attack_name: "攻撃",
    attack_damage: "%{total}ダメージ",
    pd_paformance: "パフォーマンス",
    pd_symphony: "シンフォニー",
    pd_miracle: "【ミラクル】%{value}",
    pd_perfect_miracle: "【パーフェクトミラクル】%{value}",
    pd_miracle_synchro: "【ミラクルシンクロ】%{value}＋シンフォニーを行った人数",
};

/// Ruby `BCDice::GameSystem::BeginningIdol_Korean`（ID: `BeginningIdol:Korean`）。
///
/// 表とメッセージは `ko_kr` ロケール（欠けているキーは `ja_jp` へフォールバックする）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginningIdol_Korean;

impl GameSystem for BeginningIdol_Korean {
    fn id(&self) -> &'static str {
        "BeginningIdol:Korean"
    }

    fn name(&self) -> &'static str {
        "비기닝 아이돌"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:비기닝 아이돌"
    }

    fn help_message(&self) -> &'static str {
        r"・퍼포먼스　[r]PDn[+m/-m](r：남은 주사위 눈　n：굴릴 갯수　m：수정치)
・월드세팅 업무표　BWT：대형 연예 프로덕션　LWT：약소 연예 프로덕션
　TWT：라이브 시어터　CWT：아이돌 부　LO[n]：로컬 아이돌(n：찬스)
　SU：열정의 여름　WI：온기의 겨울　NA：대자연　GA：女学園　BA：アカデミー
・업무표　WT　VA：버라이어티　MU：음악 관련　DR：드라마 관련
　VI：비주얼 관련　SP：스포츠　CHR：크리스마스　PAR：파트너 관련
・특기 리스트　AN：動物　MOV：映画　FA：ファンタジー
・ハプニング表　HA
・特技リスト　AT[n](n：分野No.)
・아이돌 스킬 습득표　SGT：챌린지 걸즈　RS：로드 투 프린스
・변조　BT[n](n：주사위눈)
・아이템　IT[n](n：보유 갯수)
・アクセサリー　ACT：種別決定　ACB：ブランド決定　ACE：効果表
・의상　DT：챌린지 걸즈　RC：로드 투 프린스　FC:フォーチュンスターズ
・엉망진창 표　LUR：로컬 아이돌　SUR：정열의 여름　WUR：온기의 겨울
　NUR：대자연　GUR：女学園　BUR：アカデミー
・센터 룰　HW：역풍 씬표　FL：신출내기 씬표　LN：고독표
　マイスキル【MS：名前決定　MSE：効果表】　演出表【ST　FST：ファンタジー】
・합숙 룰　산책표 【SH：쇼핑몰　MO：산　SEA：바다　SPA：온천】
　TN：야밤의 대화 시츄에이션　성장표 【CG：커먼　GG：골드】
・작사표　CHO　SCH：정열의 여름　WCH：온기의 겨울　NCH：대자연
・캐릭터 공백표　CBT：챌린지 걸즈　RCB：로드 투 프린스
・취미 공백표　HBT：챌린지 걸즈　RHB：로드 투 프린스
・마스코트 폭주표　RU
・버스트 타임　nC：バーストタイム(n：온도)　BU：バースト表
・攻撃　n[S]A[r][+m/-m](n：振る数　S：失敗しない　r：取り除く出目　m：修正値)
・かんたんパーソン表　SIP
・회장표
　BVT：대형 예능 프로덕션　LVT：약소 예능 프로덕션　TVT：라이브 시어터　CVT：아이돌 부
・장소표
　BST：대형 예능 프로덕션　LST：약소 예능 프로덕션　TST：라이브 시어터　CST：아이돌 부
・프레셔 종류 결정표
　BPT：대형 예능 프로덕션　LPT：약소 예능 프로덕션　TPT：라이브 시어터　CPT：아이돌 부
・도구표
　BIT：대형 예능 프로덕션　LIT：약소 예능 프로덕션　TIT：라이브 시어터　CIT：아이돌 부
[]内は省略可　D66 다이스가 존재
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "LO",
            "DT",
            "RC",
            "FC",
            "ACB",
            "TN",
            "CG",
            "GG",
            "HA",
            "CBT",
            "RCB",
            "HBT",
            "RHB",
            "RU",
            "SIP",
            "BU",
            "HW",
            "FL",
            "MSE",
            "ST",
            "FST",
            "BWT",
            "LWT",
            "TWT",
            "CWT",
            "SU",
            "WI",
            "NA",
            "GA",
            "BA",
            "WT",
            "VA",
            "MU",
            "DR",
            "VI",
            "SP",
            "CHR",
            "PAR",
            "SW",
            "AN",
            "MOV",
            "FA",
            "BVT",
            "LVT",
            "TVT",
            "CVT",
            "BST",
            "LST",
            "TST",
            "CST",
            "BPT",
            "LPT",
            "TPT",
            "CPT",
            "BIT",
            "LIT",
            "TIT",
            "CIT",
            "CHO",
            "SCH",
            "WCH",
            "NCH",
            "GCH",
            "PCH",
            "LUR",
            "SUR",
            "WUR",
            "NUR",
            "GUR",
            "BUR",
            "ACE",
            "ACT",
            "MS",
            "RE",
            "SH",
            "MO",
            "SEA",
            "SPA",
            "LN",
            "SGT",
            "RS",
            "RTT[1-6]?",
            "RCT",
            "AT",
            "AT1",
            "AT2",
            "AT3",
            "AT4",
            "AT5",
            "AT6",
            "IT",
            "BT",
            r"\d{2}C",
            r"\d+S?A",
            "[1-7]*PD",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `BeginningIdol#result_nd6`。
    fn result_nd6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_nd6(&KO_SYSTEM, total, dice_total, cmp_op, target)
    }

    /// Ruby `BeginningIdol#eval_game_system_specific_command`。
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
        crate::game_system::test_support::assert_toml_cases(
            "BeginningIdol:Korean",
            "BeginningIdol_Korean.toml",
            196,
            &[
                (144, 16),
                (145, 16),
                (146, 16),
                (147, 16),
                (148, 16),
                (149, 16),
                (150, 16),
                (151, 16),
                (152, 16),
                (153, 16),
                (154, 16),
                (155, 16),
                (156, 16),
                (157, 16),
                (158, 16),
                (159, 16),
                (160, 16),
                (161, 16),
                (162, 16),
                (163, 16),
                (164, 16),
                (165, 16),
                (166, 16),
            ],
        );
    }
}
