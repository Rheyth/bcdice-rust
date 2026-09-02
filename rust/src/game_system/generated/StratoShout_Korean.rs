//! P4で手書き移植した `lib/bcdice/game_system/StratoShout_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `StratoShout` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! 判定と表引きの実装は [`super::StratoShout`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの表と定型文だけを置く。
//!
//! 表データは `i18n/StratoShout/ko_kr.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use super::StratoShout::{check_result_2d6, eval_specific_command, result_ndx, SystemTables};
use crate::dice_table::sai_fic_skill_table::DEFAULT_SKILL_FORMAT;
use crate::dice_table::{
    D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table, TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// ko_kr ロケールの表と定型文
// ---------------------------------------------------------------------------

/// i18n `StratoShout.table.VOT`（보컬 트러블표(P167)）。
static KO_TABLE_VOT: Table = Table::from_dice(
    "보컬 트러블표(P167)",
    1,
    6,
    &[
        "가사를 까먹어버렸다! 아무 말도 나오지 않아…",
        "마이크선에 발이 걸리고 말았다! 위험해!",
        "마이크 스탠드가 쓰러져버렸다!",
        "음정이 안 맞게 됐는데 다시 잘 부르지 못하겠어!",
        "박자가 안 맞는 것 같은데… 못 맞추겠어!",
        "목이 말라서 갈라질 것 같아. 무리가 가지 않게 잘해야…!",
    ],
);

/// i18n `StratoShout.table.GUT`（기타 트러블표(P169)）。
static KO_TABLE_GUT: Table = Table::from_dice(
    "기타 트러블표(P169)",
    1,
    6,
    &[
        "큰일났다. 코드를 틀렸어! 어떻게 얼버무려 보자…",
        "윽, 실드(앰프에 연결하는 실드 케이블)가 빠졌어! 소리가 안 나와!",
        "기타 소리에 노이즈가 낀 것 같은데… 좀 고쳐져라…!",
        "어라? 지금 곡의 어느 부분이었지…?",
        "현이 끊어져버렸어! 너무 불길한데…",
        "피크가 날아가버렸어! 핑거링으로 칠 수밖에…!",
    ],
);

/// i18n `StratoShout.table.BAT`（베이스 트러블표(P171)）。
static KO_TABLE_BAT: Table = Table::from_dice(
    "베이스 트러블표(P171)",
    1,
    6,
    &[
        "큰일났다. 코드를 틀렸어! 어떻게 얼버무려 보자…",
        "윽, 실드(앰프에 연결하는 실드 케이블)가 빠졌어! 소리가 안 나와!",
        "베이스 소리에 노이즈가 낀 것 같은데… 좀 고쳐져라…!",
        "어라? 지금 곡의 어느 부분이었지…?",
        "손끝의 감각이 마비되기 시작했다. 안 움직여…!",
        "템포가 빨라지기 시작했는데 멈출 수가 없어!",
    ],
);

/// i18n `StratoShout.table.KEYT`（키보드 트러블표(P173)）。
static KO_TABLE_KEYT: Table = Table::from_dice(
    "키보드 트러블표(P173)",
    1,
    6,
    &[
        "손끝의 감각이 마비되기 시작했다. 안 움직여…!",
        "볼륨 슬라이드를 잘못 건드려 버렸어! 엄청난 소음이!",
        "어라? 지금 곡의 어느 부분이었지…?",
        "소리가 안 나는 건반이 있어… 고장?!",
        "음색이 틀렸어! 원래 음색이 몇 번이었지…?!",
        "손을 놓는 위치가 잘못됐어! 불협화음이야!",
    ],
);

/// i18n `StratoShout.table.DRT`（드럼 트러블표(P175)）。
static KO_TABLE_DRT: Table = Table::from_dice(
    "드럼 트러블표(P175)",
    1,
    6,
    &[
        "손이 꼬여버렸다! 얼른 바로잡아야…!",
        "어라? 지금 곡의 어느 부분이었지…?",
        "하이햇이 안 열려! 볼트가 풀려있나…?!",
        "애드리브 했는데 다음 프레이즈가 안 떠올라…!",
        "템포가 빨라지기 시작했는데 멈출 수가 없어!",
        "스틱이 날아가버렸다! 대신 쓸만한 게 있나…",
    ],
);

/// i18n `StratoShout.table.EMO`（감정표(P183)）。
static KO_TABLE_EMO: Table = Table::from_dice(
    "감정표(P183)",
    1,
    6,
    &[
        "공감/불신",
        "우정/질투",
        "호적수/짜증남",
        "필요/기피",
        "존경/열등감",
        "애정/빚",
    ],
);

/// i18n `StratoShout.table.SCENE`（장면표(P199)）。
static KO_TABLE_SCENE: Table =
    Table::from_dice("장면표(P199)", 2, 6, &[
        "혼자만의 시간. 불현듯 과거의 기억이 떠오른다. 그러고 보니 이전에 그런 일이 있었던 것 같은…",
        "어디선가 말싸움을 하는 것 같은 소리가 들렸다. 싸움인가?",
        "날이 어두워지고, 주변은 적막에 휩싸였다. 그녀석은 지금 뭘하고 있을까.",
        "동료와 함께 식사를 하고 있자니 화제가 자연스럽게 그 이야기로…",
        "웃음이 가득한 공간. 이런 시간이 계속 이어지면 좋을 텐데.",
        "햇볕이 드는 곳. 바쁜 일상에서 벗어나서 조용한 시간을 보낸다.",
        "스마트폰에서 알림음이 울렸다. 전화? 문자? 누구일까.",
        "갑자기 당신에게 찾아온 사람이 있었다. 뭔가 전하고 싶은 것이 있는 듯하다.",
        "분실물 하나를 발견했다. 찾아주는 것이 좋을까.",
        "누가 뜬소문을 얘기하고 있다. 들을 생각은 없었지만 멋대로 듣게 됐다.",
        "왠지 오한이 느껴진다. 뭔가 좋지 않은 일이 일어난 것 같은데…",
    ]);

/// i18n `StratoShout.table.MACHI`（거리 장면표(P199)）。
static KO_TABLE_MACHI: Table =
    Table::from_dice("거리 장면표(P199)", 2, 6, &[
        "가본 적 없는 곳에 처음으로 오고 말았다. 약간 긴장된다.",
        "알바하는 곳. 알바 동료가 의외의 사실을 알려주었다.",
        "말도 제대로 안 들릴 정도의 대음량으로 들리는 음악. 그 자리에 있는 것만으로도 기분이 고조된다.",
        "횡단보도에서 신호가 바뀌기를 기다리니 낯익은 인물의 모습을 발견했다.",
        "갑작스러운 비에 발길을 서두르는 사람들. 나도 빨리 돌아가야지.",
        "그냥 들린 가게에서 지인과 마주쳤다. 이런 곳에서 뭘 하는 거지?",
        "연습을 끝내고 들린 음식점에서 의외의 인물을 발견했다. 잠깐 상황을 볼까.",
        "여기저기서 아이들이 웃고 떠드는 소리가 들린다. 나도 저런 시절이 있었나.",
        "소리 하나 없는 정적 속 세계. 가끔은 소리에서 벗어나는 것도 좋다.",
        "전철 안. 손잡이를 잡은 채로 흔들리고 있자니 낯익은 승객을 발견했다.",
        "노래방의 복도를 걷고 있자니 어디선가 낯익은 목소리가…?",
    ]);

/// i18n `StratoShout.table.GAKKO`（학교 장면표(P199)）。
static KO_TABLE_GAKKO: Table =
    Table::from_dice("학교 장면표(P199)", 2, 6, &[
        "교사 뒤편. 무언가 얘기를 나누던 두 사람을 발견했다. 대체 무슨 얘기를 하고 있던 걸까…?",
        "어느 부실. 부원들은 부활동에 전념하고 있는 것 같지만…",
        "선생님이 타깃에 대해 물어본다. 뭔가 신경 쓰일만한 점이 있는 것 같다.",
        "나무들 사이로 아침햇살이 비추는 통학로. 어떤 이는 서두르고, 어떤 이는 즐겁게 학교로 가고 있다.",
        "쉬는 시간. 교실 여기저기서 들려오는 시덥잖은 잡담. 그 중에서 신경쓰이는 얘기를 들었다.",
        "모든 것이 붉게 물드는 해질녘. 학생들은 학업에서 해방되어 남은 하루를 자유롭게 보낸다.",
        "이동 수업. 이동하는 복도에서 아래를 보니 낯익은 사람이 있었다.",
        "점심시간. 학생들은 각자 자리를 찾아 점심을 먹는다. 자, 나는 어디서 먹을까.",
        "선생님에게 심부름 하나를 부탁받았다. 빨리 끝내버리자.",
        "슬슬 학교가 문닫을 시간이다. 불이 켜져 있는 교실은 별로 없다.",
        "스피커에서 교내 방송이 울려퍼진다. 누군가를 부르는 것 같은데…?",
    ]);

/// i18n `StratoShout.table.BAND`（밴드 장면표(P199)）。
static KO_TABLE_BAND: Table = Table::from_dice(
    "밴드 장면표(P199)",
    2,
    6,
    &[
        "인터넷에서 음악 전문 뉴스 사이트를 확인한다. 꽤 다양한 기사가 실려있다.",
        "의외의 장소에서 연습하는 인물을 발견했다. 잠깐 말 걸어볼까.",
        "약간 벽을 느끼고 말았다. 다른 사람이랑 상담해보는 게 좋을지도…",
        "라이브를 보려고 라이브 하우스에 왔다. 어떤 무대일까.",
        "상의하기 위해서 라이브 하우스에 와있는 것은 우리 뿐만이 아닌 것 같다.",
        "연습을 끝내고 집에 돌아가는 길. 그녀석도 연습이 끝났을 즈음인가.",
        "어디선가 악기 소리가 들린다. 누가 연주하고 있는 건가.",
        "열기로 가득 찬 방을 나와서 잠시 숨을 돌리는 스튜디오 대기실. 소파에 앉아있는 것은…",
        "들린 악기점에서 낯익은 인물을 발견했다. 뭘 하러 온 걸까.",
        "최신 히트송을 틀어놓은 CD샵 내부. 다음은 어떤 곡을 할까…",
        "그냥 내본 소리가 어느새 즉석 세션으로 이어졌다. 가볍게 실력을 발휘해볼까.",
    ],
);

/// i18n `StratoShout.table.TENKAI`（장면 전개표(P201)）。
static KO_TABLE_TENKAI: D66Table =
    D66Table::new("장면 전개표(P201)", D66SortType::Asc, &[
        (11, TableItem::Text("절망 : 스텝을 더 어렵게 하거나, 장면 플레이어를 파멸로 몰아넣는 상황에 빠집니다. 【디스코드】 +2점.")),
        (12, TableItem::Text("붕괴 : 스텝으로 인해 장면 플레이어의 소중한 것이 붕괴하는, 또는 붕괴 직전에 놓입니다. 【디스코드】 +2점.")),
        (13, TableItem::Text("단절 : 장면 플레이어는 스텝에 의해 무언가와 절연하게 됩니다. 【디스코드】 +2점.")),
        (14, TableItem::Text("공포 : 스텝을 두렵게 만드는 일과 조우합니다. 【디스코드】 +2점.")),
        (15, TableItem::Text("오해 : 장면 플레이어가 스텝에 관한 어떤 오해를 삽니다. 【디스코드】 +2점.")),
        (16, TableItem::Text("시련 : 장면 플레이어는 스텝에 관한 시련과 직면합니다. 【디스코드】 +2점.")),
        (22, TableItem::Text("심마 : 장면 플레이어는 나쁜 생각을 갖게 되어 스텝을 불합리하게 해결하려고 합니다. 【디스코드】 +1점.")),
        (23, TableItem::Text("속박 : 스텝에 관한 무언가에 속박되어 자유롭게 행동하지 못하게 됩니다. 【디스코드】 +1점.")),
        (24, TableItem::Text("흉조 : 스텝에 관해서 무언가 나쁜 일이 일어날 것 같은 전조가 찾아옵니다. 【디스코드】 +1점.")),
        (25, TableItem::Text("가속 : 장면 플레이어는 스텝 해결에 쫓기게 됩니다. 【디스코드】 +1점.")),
        (26, TableItem::Text("일상 : 장면 플레이어는 느긋하게 일상을 보냅니다. 【컨디션】 +1점.")),
        (33, TableItem::Text("휴식 : 스텝에 대해 잊어버릴 것 같은 평온한 한때를 보냅니다. 【컨디션】 +1점.")),
        (34, TableItem::Text("길조 : 스텝에 관해서 무언가 좋은 일이 일어날 것 같은 전조가 찾아옵니다. 【컨디션】 +1점.")),
        (35, TableItem::Text("발견 : 장면 플레이어는 스텝에 대해 무언가를 발견합니다. 【컨디션】 +1점.")),
        (36, TableItem::Text("희망 : 장면 플레이어에게 스텝에 대한 긍정적인 노력을 할 의지가 생깁니다. 【컨디션】 +1점.")),
        (44, TableItem::Text("성장 : 스텝을 통해 장면 플레이어가 성장합니다. 【컨디션】 +2점.")),
        (45, TableItem::Text("애정 : 스텝을 통해 장면 플레이어가 애정을 느낍니다. 【컨디션】 +2점.")),
        (46, TableItem::Text("낭보 : 스텝에 관한 좋은 소식을 맞이합니다. 【컨디션】 +2점.")),
        (55, TableItem::Text("호전 : 스텝이 좋은 방향으로 향하는 사건이 일어납니다. 【컨디션】 +3점.")),
        (56, TableItem::Text("직감 : 스텝을 해결할 수 있는 결정적인 깨달음을 얻습니다. 【컨디션】 +3점.")),
        (66, TableItem::Text("기적 : 스텝에 관한 기적적인 행운을 맞이합니다. 【컨디션】 +3점.")),
    ]);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static KO_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("VOT", &KO_TABLE_VOT),
    ("GUT", &KO_TABLE_GUT),
    ("BAT", &KO_TABLE_BAT),
    ("KEYT", &KO_TABLE_KEYT),
    ("DRT", &KO_TABLE_DRT),
    ("EMO", &KO_TABLE_EMO),
    ("SCENE", &KO_TABLE_SCENE),
    ("MACHI", &KO_TABLE_MACHI),
    ("GAKKO", &KO_TABLE_GAKKO),
    ("BAND", &KO_TABLE_BAND),
    ("TENKAI", &KO_TABLE_TENKAI),
];

/// i18n `StratoShout.RTT.items[0]`（가치관）。
static KO_RTT_SKILLS1: &[&str] = &[
    "과거", "연인", "동료", "가족", "자신", "지금", "이유", "꿈", "세계", "행복", "미래",
];
/// i18n `StratoShout.RTT.items[1]`（신체）。
static KO_RTT_SKILLS2: &[&str] = &[
    "머리", "눈", "귀", "입", "가슴", "심장", "피", "등", "손", "XXX", "발",
];
/// i18n `StratoShout.RTT.items[2]`（모티브）。
static KO_RTT_SKILLS3: &[&str] = &[
    "어둠", "무기", "마법", "동물", "도시", "노래", "창문", "꽃", "하늘", "계절", "빛",
];
/// i18n `StratoShout.RTT.items[3]`（정서）。
static KO_RTT_SKILLS4: &[&str] = &[
    "슬픔",
    "분노",
    "불안",
    "공포",
    "놀람",
    "두근거림",
    "정열",
    "확신",
    "기대",
    "즐거움",
    "기쁨",
];
/// i18n `StratoShout.RTT.items[4]`（행동）。
static KO_RTT_SKILLS5: &[&str] = &[
    "울다",
    "잊다",
    "지우다",
    "부수다",
    "외치다",
    "노래하다",
    "춤추다",
    "달리다",
    "만나다",
    "부르다",
    "웃다",
];
/// i18n `StratoShout.RTT.items[5]`（역경）。
static KO_RTT_SKILLS6: &[&str] = &[
    "죽음",
    "상실",
    "폭력",
    "고독",
    "후회",
    "실력",
    "따분함",
    "본성",
    "재산",
    "연애",
    "삶",
];

/// Ruby `RTT` の特技リスト（分野は1D6の出目順）。
static KO_RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("가치관", KO_RTT_SKILLS1),
    SaiFicCategory::new("신체", KO_RTT_SKILLS2),
    SaiFicCategory::new("모티브", KO_RTT_SKILLS3),
    SaiFicCategory::new("정서", KO_RTT_SKILLS4),
    SaiFicCategory::new("행동", KO_RTT_SKILLS5),
    SaiFicCategory::new("역경", KO_RTT_SKILLS6),
];

/// Ruby `RTT`（`SaiFicSkillTable.from_i18n("StratoShout.RTT", :ko_kr, rtt: 'AT', rttn: [...])`）。
///
/// `from_i18n` は `I18n.t("RTT", locale:)`（グローバル）に
/// `I18n.t("StratoShout.RTT", locale:)` を `merge` する。`i18n/ko_kr.yml` の
/// グローバル `RTT` には `rtt_format` / `rttn_format` / `rct_format` があり、
/// 前2つはシステム側の指定で上書きされ、`rct_format` だけがグローバルの値で残る。
/// `s_format` はどちらにも無いので既定のまま。
static KO_RTT: SaiFicSkillTable = SaiFicSkillTable::new(KO_RTT_CATEGORIES)
    .with_commands(
        Some("AT"),
        None,
        &["AT1", "AT2", "AT3", "AT4", "AT5", "AT6"],
    )
    .with_formats(SaiFicFormats {
        rtt: "특기 리스트 ＞ [%<category_dice>d,%<row_dice>d] ＞ %<text>s",
        // i18n `ko_kr.RTT.rct_format`
        rct: "랜덤 분야표(%<category_dice>d) ＞ %<category_name>s",
        rttn: "특기 리스트(%<category_name>s분야) ＞ [%<row_dice>d] ＞ %<text>s",
        skill: DEFAULT_SKILL_FORMAT,
    });

/// `ko_kr` ロケールの表と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    rtt: &KO_RTT,
    critical: "스페셜! (【컨디션】+2)",
    fumble: "펌블! (드라마페이즈: 【디스코드】+2 / 라이브페이즈: 【컨디션】-2)",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::StratoShout_Korean`（ID: `StratoShout:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StratoShout_Korean;

impl GameSystem for StratoShout_Korean {
    fn id(&self) -> &'static str {
        "StratoShout:Korean"
    }

    fn name(&self) -> &'static str {
        "스트라토 샤우트"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:스트라토 샤우트"
    }

    fn help_message(&self) -> &'static str {
        r"VOT, GUT, BAT, KEYT, DRT: (보컬, 기타, 베이스, 키보드, 드럼)트러블표
EMO: 감정표
RTT[1-6], AT[1-6]: 특기표(공백: 랜덤 1: 가치관 2: 신체 3: 모티브 4: 이모션 5: 행동 6: 역경)
SCENE, MACHI, GAKKO, BAND: (범용, 거리, 학교, 밴드)장면표. 접근 장면에 사용
TENKAI: 장면 전개표. 분주 장면, 연습 장면에 사용
[]내는 생략가능　D66는 변동있음
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "VOT",
            "GUT",
            "BAT",
            "KEYT",
            "DRT",
            "EMO",
            "SCENE",
            "MACHI",
            "GAKKO",
            "BAND",
            "TENKAI",
            "RTT[1-6]?",
            "RCT",
            "AT",
            "AT1",
            "AT2",
            "AT3",
            "AT4",
            "AT5",
            "AT6",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `StratoShout#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `StratoShout#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// `result_2d6` が `nil` を返す経路（ファンブルでもスペシャルでもない出目、
    /// `>=` 以外の比較演算子、`2D6` 以外のダイス）で通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx(&KO_SYSTEM, total, cmp_op, target)
    }

    /// Ruby `StratoShout#result_2d6`（`ko_kr` の定型文で）。
    fn result_2d6(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&KO_SYSTEM, crate::Int::from(dice_total), cmp_op)
    }

    /// Ruby `StratoShout#eval_game_system_specific_command`（`ko_kr` の表で）。
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

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/StratoShout_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "StratoShout:Korean",
            "StratoShout_Korean.toml",
            23,
        );
    }

    /// `result_2d6` が nil を返す経路が `ko_kr` の定型文になること。
    ///
    /// Ruby は `Base#result_ndx` の `translate("success")` が `@locale = :ko_kr` を見るため
    /// `성공` / `실패` になる。TOML の `2d6>=4` などで既に通る経路だが、
    /// `>=` 以外や 2D6 以外のダイスは TOML に無いのでここで固定する。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            // `>=` 以外の比較演算子（StratoShout#result_2d6 は nil を返す）
            (
                "2D6<=5",
                vec![(1, 6), (3, 6)],
                "(2D6<=5) ＞ 4[1,3] ＞ 4 ＞ 성공",
            ),
            (
                "2D6<=5",
                vec![(4, 6), (5, 6)],
                "(2D6<=5) ＞ 9[4,5] ＞ 9 ＞ 실패",
            ),
            // 2D6 以外のダイス（result_2d6 自体が呼ばれない）
            (
                "3D6>=10",
                vec![(4, 6), (5, 6), (6, 6)],
                "(3D6>=10) ＞ 15[4,5,6] ＞ 15 ＞ 성공",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("StratoShout:Korean"), input, &mut src)
                .expect("must not error")
                .expect("must produce output");
            assert_eq!(result.text, expected, "input: {input}");
        }
    }
}
