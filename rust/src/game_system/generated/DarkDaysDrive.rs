//! P4で手書き移植した `lib/bcdice/game_system/DarkDaysDrive.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DarkDaysDrive#result_2d6`
//! - `#eval_game_system_specific_command` → `roll_tables` / `command_iax` / `RTT.roll_command`
//! - `command_iax`（`IAX`。接頭辞 `IA` にマッチしたうえで本体が `IAX` を見る）
//! - `RTT` と `TABLES`

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::sai_fic_skill_table::{DEFAULT_RCT_FORMAT, DEFAULT_RTTN_FORMAT};
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
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `DarkDaysDrive#result_2d6`。
fn check_result_2d6(
    total: crate::Int,
    dice_total: i64,
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    // Ruby: return nil unless cmp_op == :>=
    if cmp_op != CmpOp::Ge {
        return None;
    }

    if dice_total <= 2 {
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            "ファンブル(判定失敗。失敗表(FT)を追加で１回振る)",
        ))))
    } else if dice_total >= 12 {
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            "スペシャル(判定成功。大成功表(GJT)を１回使用可能)",
        ))))
    } else {
        match target {
            // Ruby: elsif target == "?" -> Result.nothing
            Target::Question => Some(CheckOutcome::Nothing),
            Target::Number(target) if total >= target => {
                Some(CheckOutcome::Result(Box::new(EvalResult::success("成功"))))
            }
            Target::Number(_) => Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗")))),
        }
    }
}

/// Ruby `DarkDaysDrive#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = roll_tables(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = command_iax(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(RTT
        .roll_command(rng, command)?
        .map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `/(\([A-Z]+\))/` 相当。`ia.body.match(/\(([A-Z]+)\)/)`。
fn ia_code_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\(([A-Z]+)\)").expect("valid regex"))
}

/// Ruby `#command_iax`。
fn command_iax(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "IAX" {
        return Ok(None);
    }

    let ia = TABLE_IA.choice(rng.roll_d66(D66SortType::Asc)?);
    let Some(caps) = ia_code_pattern().captures(ia.last_body()) else {
        return Ok(Some(ia.to_string()));
    };

    let n = rng.roll_once(6)?;
    let ia2 = match &caps[1] {
        "IAA" => TABLE_IAA.choice(n),
        "IAB" => TABLE_IAB.choice(n),
        "IAC" => TABLE_IAC.choice(n),
        "IAD" => TABLE_IAD.choice(n),
        "IAE" => TABLE_IAE.choice(n),
        "IAF" => TABLE_IAF.choice(n),
        "IAG" => TABLE_IAG.choice(n),
        "IAH" => TABLE_IAH.choice(n),
        "IAI" => TABLE_IAI.choice(n),
        "IAJ" => TABLE_IAJ.choice(n),
        _ => return Ok(Some(ia.to_string())),
    };
    Ok(Some(format!("{ia} ＞ {ia2}")))
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

static RTT_SKILLS_HAKEI: &[&str] = &[
    "呪い",
    "絶望",
    "孤児",
    "死別",
    "一般人",
    "獲物",
    "憧れ",
    "友人",
    "挑戦者",
    "血縁",
    "永遠",
];
static RTT_SKILLS_SHIGOTO: &[&str] = &[
    "脅迫",
    "捨てる",
    "拉致",
    "盗む",
    "ハッキング",
    "侵入",
    "変装",
    "だます",
    "隠す",
    "のぞく",
    "聞き出す",
];
static RTT_SKILLS_SOSAKU: &[&str] = &[
    "トイレ",
    "食事",
    "自然",
    "運動施設",
    "街",
    "友愛会",
    "暗部",
    "史跡",
    "文化施設",
    "温泉",
    "宿泊",
];
static RTT_SKILLS_SHUMI: &[&str] = &[
    "お酒",
    "グルメ",
    "ダンス",
    "スポーツ",
    "健康",
    "ファッション",
    "恋愛",
    "フェス",
    "音楽",
    "物語",
    "学問",
];
static RTT_SKILLS_FUNIKI: &[&str] = &[
    "だらしない",
    "のんびり",
    "暖かい",
    "明るい",
    "甘い",
    "普通",
    "洗練",
    "渋い",
    "静か",
    "真面目",
    "冷たい",
];
static RTT_SKILLS_SENTO: &[&str] = &[
    "忍術",
    "古武術",
    "剣術",
    "棒術",
    "拳法",
    "ケンカ",
    "総合格闘技",
    "レスリング",
    "軍隊格闘術",
    "射撃",
    "弓術",
];

static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("背景", RTT_SKILLS_HAKEI),
    SaiFicCategory::new("仕事", RTT_SKILLS_SHIGOTO),
    SaiFicCategory::new("捜索", RTT_SKILLS_SOSAKU),
    SaiFicCategory::new("趣味", RTT_SKILLS_SHUMI),
    SaiFicCategory::new("雰囲気", RTT_SKILLS_FUNIKI),
    SaiFicCategory::new("戦闘法", RTT_SKILLS_SENTO),
];

static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES).with_formats(SaiFicFormats {
    rtt:
        "ランダム指定特技表(%<category_dice>d,%<row_dice>d) ＞ %<category_name>s《%<skill_name>s》",
    rct: DEFAULT_RCT_FORMAT,
    rttn: DEFAULT_RTTN_FORMAT,
    skill: crate::dice_table::sai_fic_skill_table::DEFAULT_SKILL_FORMAT,
});

static TABLE_ABRT: D66Table = D66Table::new(
    "アビリティ決定表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("インストラクター(P155)")),
        (12, TableItem::Text("運送業(P155)")),
        (13, TableItem::Text("運転手(P155)")),
        (14, TableItem::Text("カフェ店員(P155)")),
        (15, TableItem::Text("趣味人(P155)")),
        (16, TableItem::Text("ショップ店員(P155)")),
        (22, TableItem::Text("正社員(P156)")),
        (23, TableItem::Text("大工(P156)")),
        (24, TableItem::Text("探偵(P156)")),
        (25, TableItem::Text("バイヤー(P156)")),
        (26, TableItem::Text("俳優(P156)")),
        (33, TableItem::Text("派遣社員(P156)")),
        (34, TableItem::Text("犯罪者(P157)")),
        (35, TableItem::Text("バンドマン(P157)")),
        (36, TableItem::Text("バーテンダー(P157)")),
        (44, TableItem::Text("ヒモ(P157)")),
        (45, TableItem::Text("ホスト(P157)")),
        (46, TableItem::Text("ホテルマン(P157)")),
        (55, TableItem::Text("無職(P158)")),
        (56, TableItem::Text("用心棒(P158)")),
        (66, TableItem::Text("料理人(P158)")),
    ],
);

static TABLE_DT: Table = Table::from_dice(
    "ダメージ表",
    1,
    6,
    &["疲れ", "痛み", "焦り", "不調", "ショック", "ケガ"],
);

static TABLE_FT: Table = Table::from_dice(
    "失敗表",
    1,
    6,
    &[
        "任意のアイテムを一つ失う",
        "１ダメージを受ける",
        "【所持金ランク】が１減少する（最低０）",
        "２ダメージを受ける",
        "【所持金ランク】が２減少する（最低０）",
        "標的レベルが１増加する",
    ],
);

static TABLE_GJT: Table = Table::from_dice(
    "大成功表",
    1,
    6,
    &[
        "主人からお褒めの言葉をいただく",
        "ダメージを１回復する",
        "ダメージを１回復する",
        "関係のチェックを一つ消す",
        "ダメージを２回復する",
        "【所持金ランク】が１増加する",
    ],
);

static TABLE_ITT: Table = Table::from_dice(
    "移動トラブル表",
    1,
    6,
    &[
        "検問（P220)",
        "急な腹痛（P220)",
        "黒煙（P221)",
        "蚊（P221)",
        "落とし物（P222)",
        "空腹（P222)",
    ],
);

static TABLE_NTT: Table = Table::from_dice(
    "任務トラブル表",
    1,
    6,
    &[
        "通報（P223)",
        "プレッシャー（P223)",
        "マナー違反（P224)",
        "志願者（P224)",
        "仲間割れ（P225)",
        "狩人の噂（P225)",
    ],
);

static TABLE_STT: Table = Table::from_dice(
    "襲撃トラブル表",
    1,
    6,
    &[
        "孤独な追跡者（P226)",
        "地元の若者たち（P226)",
        "V-FILES（P227)",
        "チンピラの群れ（P227)",
        "孤独な狩人（P228)",
        "狩人の群れ（P228)",
    ],
);

static TABLE_HTT: D66Table = D66Table::new(
    "変身トラブル表",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("あれを食べたい(P214)")),
        (12, TableItem::Text("あれを着たい(P214)")),
        (13, TableItem::Text("あれを見たい(P215)")),
        (14, TableItem::Text("あれを狩りたい(P215)")),
        (15, TableItem::Text("あれを踊りたい(P216)")),
        (16, TableItem::Text("あれに入りたい(P216)")),
        (21, TableItem::Text("強奪(P217)")),
        (22, TableItem::Text("暴行(P217)")),
        (23, TableItem::Text("虐殺(P218)")),
        (24, TableItem::Text("誘拐(P218)")),
        (25, TableItem::Text("無精(P219)")),
        (26, TableItem::Text("失踪(P219)")),
        (31, TableItem::Text("あれを食べたい(P214)")),
        (32, TableItem::Text("あれを着たい(P214)")),
        (33, TableItem::Text("あれを見たい(P215)")),
        (34, TableItem::Text("あれを狩りたい(P215)")),
        (35, TableItem::Text("あれを踊りたい(P216)")),
        (36, TableItem::Text("あれに入りたい(P216)")),
        (41, TableItem::Text("強奪(P217)")),
        (42, TableItem::Text("暴行(P217)")),
        (43, TableItem::Text("虐殺(P218)")),
        (44, TableItem::Text("誘拐(P218)")),
        (45, TableItem::Text("無精(P219)")),
        (46, TableItem::Text("失踪(P219)")),
        (51, TableItem::Text("あれを食べたい(P214)")),
        (52, TableItem::Text("あれを着たい(P214)")),
        (53, TableItem::Text("あれを見たい(P215)")),
        (54, TableItem::Text("あれを狩りたい(P215)")),
        (55, TableItem::Text("あれを踊りたい(P216)")),
        (56, TableItem::Text("あれに入りたい(P216)")),
        (61, TableItem::Text("強奪(P217)")),
        (62, TableItem::Text("暴行(P217)")),
        (63, TableItem::Text("虐殺(P218)")),
        (64, TableItem::Text("誘拐(P218)")),
        (65, TableItem::Text("無精(P219)")),
        (66, TableItem::Text("失踪(P219)")),
    ],
);

static TABLE_DET: Table = Table::from_dice(
    "ドライブイベント表",
    1,
    6,
    &[
        "身の上話をする。目標が背景分野で選択している特技がドライブ判定の指定特技になる。",
        "スキル自慢をする。目標が仕事分野で選択している特技がドライブ判定の指定特技になる。",
        "むかし行った場所の話をする。目標が捜索分野で選択している特技がドライブ判定の指定特技になる。",
        "趣味の話をする。目標が趣味分野で選択している特技がドライブ判定の指定特技になる。",
        "テーマがない雑談をする。目標が雰囲気分野で選択している特技がドライブ判定の指定特技になる。",
        "物騒な話をする。目標が戦闘法分野で選択している特技がドライブ判定の指定特技になる。",
    ],
);

static TABLE_BET: Table = Table::from_dice(
    "ブレイクイベント表",
    1,
    6,
    &[
        "イケメンの車は風光明美な場所に到着する。197ページの「観光地」を参照。",
        "イケメンの車は明るい光に照らされた小さな店に到着する。197ページの「コンビニ」を参照。",
        "イケメンの車は巨大かつ何でも売っている店に到着する。198ページの「ホームセンター」を参照。",
        "イケメンの車はドライバーたちの憩いの地に到着する。198ページの「サービスエリア」を参照。",
        "イケメンの車は大きなサービスエリアのような場所に到着する。199ページの「道の駅」を参照。",
        "イケメンの車は闇の底に隠された秘密の場所に到着する。199ページの「友愛会支部」を参照。",
    ],
);

static TABLE_CT: Table = Table::from_dice(
    "キャンプ表",
    1,
    6,
    &[
        "無料仮眠所・いい感じの空き地：定員無制限／居住性-2／価格0／発見率2",
        "カプセルホテル：定員1／居住性-1／価格3／発見率2",
        "ラブホテル：定員2／居住性0／価格4／発見率1",
        "ビジネスホテル：定員2／居住性0／価格4／発見率1",
        "観光ホテル：定員4／居住性1／価格5／発見率1",
        "高級ホテル：定員4／居住性2／価格6／発見率0",
    ],
);

static TABLE_KZT: Table = Table::from_dice(
    "関係属性表",
    1,
    6,
    &["軽蔑", "反感", "混乱", "興味", "共感", "憧れ"],
);

static TABLE_IA: D66Table = D66Table::new(
    "イケメンアクション決定表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("遠距離(IAA)")),
        (12, TableItem::Text("遠距離(IAA)")),
        (13, TableItem::Text("移動(IAB)")),
        (14, TableItem::Text("移動(IAB)")),
        (15, TableItem::Text("近距離(IAC)")),
        (16, TableItem::Text("近距離(IAC)")),
        (22, TableItem::Text("善人(IAD)")),
        (23, TableItem::Text("善人(IAD)")),
        (24, TableItem::Text("悪人(IAE)")),
        (25, TableItem::Text("悪人(IAE)")),
        (26, TableItem::Text("幼い(IAF)")),
        (33, TableItem::Text("幼い(IAF)")),
        (34, TableItem::Text("バカ(IAG)")),
        (35, TableItem::Text("バカ(IAG)")),
        (36, TableItem::Text("渋い(IAH)")),
        (44, TableItem::Text("渋い(IAH)")),
        (45, TableItem::Text("賢い(IAI)")),
        (46, TableItem::Text("賢い(IAI)")),
        (55, TableItem::Text("超自然(IAJ)")),
        (56, TableItem::Text("超自然(IAJ)")),
        (66, TableItem::Text("振り直しor自由選択")),
    ],
);

static TABLE_IAA: Table = Table::from_dice(
    "イケメンアクション（遠距離）表(P172)",
    1,
    6,
    &[
        "目を合わせて微笑む（かっこよさ：4）",
        "場所を譲る（かっこよさ：4）",
        "髪をかきあげる（かっこよさ：5）",
        "複雑なポーズで座る（かっこよさ：5）",
        "物憂げな表情で振り返る（かっこよさ：6）",
        "ものを上に持つ（かっこよさ：6）",
    ],
);

static TABLE_IAB: Table = Table::from_dice(
    "イケメンアクション（移動）表(P172)",
    1,
    6,
    &[
        "車道側を歩く（かっこよさ：4）",
        "乗り物から降りる（かっこよさ：4）",
        "真剣な表情で近づく（かっこよさ：4）",
        "馬に乗る（かっこよさ：6）",
        "ダメージを受けつつ移動（かっこよさ：6）",
        "瞬間移動（かっこよさ：6）",
    ],
);

static TABLE_IAC: Table = Table::from_dice(
    "イケメンアクション（近距離）表(P173)",
    1,
    6,
    &[
        "黙って見つめる（かっこよさ：3）",
        "ちょっとしたプレゼント（かっこよさ：3）",
        "顎クイ（かっこよさ：5）",
        "壁ドン（かっこよさ：5）",
        "お姫様抱っこ（かっこよさ：7）",
        "床ドン（かっこよさ：7）",
    ],
);

static TABLE_IAD: Table = Table::from_dice(
    "イケメンアクション（善人）表(P173)",
    1,
    6,
    &[
        "手を引いて逃げる（かっこよさ：4）",
        "毛布を掛ける（かっこよさ：4）",
        "守る（かっこよさ：5）",
        "笑って去る（かっこよさ：5）",
        "全てを捧げる（かっこよさ：6）",
        "悪堕ち（かっこよさ：6）",
    ],
);

static TABLE_IAE: Table = Table::from_dice(
    "イケメンアクション（悪人）表(P174)",
    1,
    6,
    &[
        "攻撃する（かっこよさ：4）",
        "暗く笑う（かっこよさ：4）",
        "奪う（かっこよさ：4）",
        "目論見を口にする（かっこよさ：6）",
        "罠にかける（かっこよさ：6）",
        "改心する（かっこよさ：6）",
    ],
);

static TABLE_IAF: Table = Table::from_dice(
    "イケメンアクション（幼い）表(P174)",
    1,
    6,
    &[
        "甘える（かっこよさ：3）",
        "疲れる（かっこよさ：3）",
        "無邪気な発言（かっこよさ：5）",
        "おねだり（かっこよさ：5）",
        "上目遣い（かっこよさ：7）",
        "抱きつく（かっこよさ：7）",
    ],
);

static TABLE_IAG: Table = Table::from_dice(
    "イケメンアクション（バカ）表(P175)",
    1,
    6,
    &[
        "苦悩する（かっこよさ：4）",
        "屈託のない笑顔（かっこよさ：4）",
        "転ぶ（かっこよさ：4）",
        "叫ぶ（かっこよさ：6）",
        "間違える（かっこよさ：6）",
        "怖がる（かっこよさ：6）",
    ],
);

static TABLE_IAH: Table = Table::from_dice(
    "イケメンアクション（渋い）表(P175)",
    1,
    6,
    &[
        "説教（かっこよさ：4）",
        "気づかせる（かっこよさ：4）",
        "見守る（かっこよさ：5）",
        "残心（かっこよさ：5）",
        "称える（かっこよさ：6）",
        "いい位置を取る（かっこよさ：6）",
    ],
);

static TABLE_IAI: Table = Table::from_dice(
    "イケメンアクション（賢い）表(P176)",
    1,
    6,
    &[
        "難しい本を読む（かっこよさ：3）",
        "アドバイスをする（かっこよさ：3）",
        "眼鏡を持ち上げる（かっこよさ：5）",
        "状況を解説する（かっこよさ：5）",
        "計算できなくなる（かっこよさ：7）",
        "大丈夫だと言う（かっこよさ：7）",
    ],
);

static TABLE_IAJ: Table = Table::from_dice(
    "イケメンアクション（超自然）表(P176)",
    1,
    6,
    &[
        "水に濡れる（かっこよさ：4）",
        "風を纏う（かっこよさ：4）",
        "地割れ（かっこよさ：5）",
        "火を放つ（かっこよさ：5）",
        "闇を生み出す（かっこよさ：6）",
        "光る（かっこよさ：6）",
    ],
);

static TABLE_CAC: Table = Table::from_dice(
    "センターの行動決定表",
    1,
    6,
    &["逃走", "不意打ち", "連続行動", "対話", "威嚇", "攻撃"],
);

static TABLE_DDC: Table = Table::from_dice(
    "対話ダメージ表",
    1,
    6,
    &["焦り", "焦り", "不調", "不調", "ショック", "ショック"],
);

static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("ABRT", &TABLE_ABRT),
    ("DT", &TABLE_DT),
    ("FT", &TABLE_FT),
    ("GJT", &TABLE_GJT),
    ("ITT", &TABLE_ITT),
    ("NTT", &TABLE_NTT),
    ("STT", &TABLE_STT),
    ("HTT", &TABLE_HTT),
    ("DET", &TABLE_DET),
    ("BET", &TABLE_BET),
    ("CT", &TABLE_CT),
    ("KZT", &TABLE_KZT),
    ("IA", &TABLE_IA),
    ("IAA", &TABLE_IAA),
    ("IAB", &TABLE_IAB),
    ("IAC", &TABLE_IAC),
    ("IAD", &TABLE_IAD),
    ("IAE", &TABLE_IAE),
    ("IAF", &TABLE_IAF),
    ("IAG", &TABLE_IAG),
    ("IAH", &TABLE_IAH),
    ("IAI", &TABLE_IAI),
    ("IAJ", &TABLE_IAJ),
    ("CAC", &TABLE_CAC),
    ("DDC", &TABLE_DDC),
];

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::DarkDaysDrive`（ID: `DarkDaysDrive`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkDaysDrive;

impl GameSystem for DarkDaysDrive {
    fn id(&self) -> &'static str {
        "DarkDaysDrive"
    }

    fn name(&self) -> &'static str {
        "ダークデイズドライブ"
    }

    fn sort_key(&self) -> &'static str {
        "たあくていすとらいふ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
スペシャル／ファンブル／成功／失敗を判定
・各種表
RTTn ランダム特技決定表(n：分野番号、省略可能)
RCT  ランダム分野決定表
ABRT アビリティ決定表
DT ダメージ表
FT 失敗表
GJT 大成功表
ITT 移動トラブル表
NTT 任務トラブル表
STT 襲撃トラブル表
HTT 変身トラブル表
DET ドライブイベント表
BET ブレイクイベント表
CT キャンプ表
KZT 関係属性表
IA イケメンアクション決定表
 IAA 遠距離 IAB 移動 IAC 近距離 IAD 善人 IAE 悪人
 IAF 幼い IAG バカ IAH 渋い IAI 賢い IAJ 超自然
IAX イケメンアクション決定表 → IA表
■本格的な戦闘
CAC センターの行動決定
DDC 対話ダメージ表
・D66ダイス昇順
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTT[1-6]?",
            "RCT",
            "ABRT",
            "DT",
            "FT",
            "GJT",
            "ITT",
            "NTT",
            "STT",
            "HTT",
            "DET",
            "BET",
            "CT",
            "KZT",
            "IA",
            "IAA",
            "IAB",
            "IAC",
            "IAD",
            "IAE",
            "IAF",
            "IAG",
            "IAH",
            "IAI",
            "IAJ",
            "CAC",
            "DDC",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `DarkDaysDrive#result_2d6`。
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(total, dice_total, cmp_op, target)
    }

    /// Ruby `DarkDaysDrive#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
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
            .join("test/data/DarkDaysDrive.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DarkDaysDrive.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/DarkDaysDrive.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DarkDaysDrive.toml must parse");
        assert_eq!(
            data.tests.len(),
            56,
            "case count in test/data/DarkDaysDrive.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DarkDaysDrive",
                "unexpected game system in DarkDaysDrive.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("DarkDaysDrive"), &tc.input, &mut src) {
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
                    "FAIL DarkDaysDrive:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DarkDaysDrive cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
