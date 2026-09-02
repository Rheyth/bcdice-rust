//! P4で手書き移植した `lib/bcdice/game_system/WitchQuest.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `WitchQuest#challenge`（チャレンジ `WQn`）と `getSuccessText`
//! - `WitchQuest#getStructureEncounter`（ストラクチャーカードの遭遇表 `SETn`）と
//!   人物表1〜4（`getPersonTable1`〜`getPersonTable4`）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。
//!
//! # Ruby の `Proc` 項目
//!
//! 人物表の `[56, gotoNextTable]` は `lambda { "表２へ" + getPersonTable2() }` で、
//! `Base#get_table_value` が `Proc` を呼び出して次の表へ連鎖する。Rust側は
//! [`PersonItem::Next`] で表し、常に「次の表」を引く（原典でも連鎖先は必ず次の表）。

use std::sync::OnceLock;

use regex::Regex;

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// 人物表の項目。Ruby では `String` か `Proc`。
enum PersonItem {
    /// 文字列の項目
    Text(&'static str),
    /// Ruby `gotoNextTable`（`"表N へ" + 次の人物表`）。保持する文字列は連結される接頭辞。
    Next(&'static str),
}

/// Ruby `getStructureEncounter` の `tables`（ストラクチャーカード番号 → 1D6の項目）。
///
/// 26番だけ項目が5個しかない（原典のデータどおり）。`get_table_by_1d6` が範囲外を引くと
/// Ruby は `["1", 0]` を返すので、その分岐も保っている。
static STRUCTURE_ENCOUNTER_TABLES: &[(i64, &[&str])] = &[
    (
        1,
        &[
            "船から降りてきた",
            "魚を売っている",
            "仕事で忙しそうな",
            "異国から来た",
            "おもしろおかしい",
            "汗水流している",
        ],
    ),
    (
        2,
        &[
            "おかしな格好をした",
            "歌を歌っている",
            "ステキな笑顔をした",
            "日なたぼっこをしている",
            "悩んでいる",
            "旅をしている",
        ],
    ),
    (
        3,
        &[
            "待ちぼうけをしている",
            "壁に登っている",
            "タバコを吸っている",
            "踊りを踊っている",
            "幸せそうな",
            "向こうから走ってくる",
        ],
    ),
    (
        4,
        &[
            "見張りをしている",
            "しゃべれない",
            "見張りをしている",
            "一輪車に乗った",
            "元気いっぱいの",
            "真面目な",
        ],
    ),
    (
        5,
        &[
            "ウソつきな",
            "買い物をしている",
            "ギターを弾いている",
            "あなたのほうをじっと見ている",
            "ポップコーンを売っている",
            "屋台を出している",
        ],
    ),
    (
        6,
        &[
            "子供を探している",
            "時計を直している",
            "物乞いをしている",
            "気象実験をしている",
            "飛び降りようとしている",
            "時間をきにしている",
        ],
    ),
    (
        7,
        &[
            "目の見えない",
            "金持ちそうな",
            "一人歩きをしたことがない",
            "ふられてしまった",
            "待ち合わせをしている",
            "道に迷った",
        ],
    ),
    (
        8,
        &[
            "お祈りをしている",
            "スケッチをしている",
            "勉強熱心な",
            "記念碑を壊そうとしている",
            "大きな声で文句をいっている",
            "記念撮影している",
        ],
    ),
    (
        9,
        &[
            "隠れている",
            "はしごに登っている",
            "鐘を鳴らしている",
            "共通語の通じない",
            "記憶を失った",
            "あなたのほうにバタッと倒れた",
        ],
    ),
    (
        10,
        &[
            "暇そうな",
            "笑ったことがない",
            "ぶくぶくと太った",
            "後継者を探している",
            "王様におつかえしている",
            "愛国心旺盛な",
        ],
    ),
    (
        11,
        &[
            "閉じ込められた",
            "悲しそうな",
            "怒っている",
            "降りれなくなっている",
            "もの憂げな",
            "飛ぼうとしている",
        ],
    ),
    (
        12,
        &[
            "釣りをしている",
            "泳いでいる",
            "川に物を落としてしまった",
            "砂金を掘っている",
            "川にゴミを捨てている",
            "カエルに化かされてしまった",
        ],
    ),
    (
        13,
        &[
            "世間話をしている",
            "結婚を薦めたがる",
            "いやらしい話の好きな",
            "選択をしている",
            "水を汲んでいる",
            "井戸に落ちてしまった",
        ],
    ),
    (
        14,
        &[
            "人におごりたがる",
            "踊り子をしている",
            "賭けをしている",
            "泣き上戸な",
            "飲み比べをしている",
            "自慢話をしている",
        ],
    ),
    (
        15,
        &[
            "素朴そうな",
            "田舎者の",
            "あなたをだまそうとしている",
            "ケンカをしている",
            "泊まるお金のない",
            "あなたに依頼をしにきた",
        ],
    ),
    (
        16,
        &[
            "悪い占いの結果しか言わない",
            "あなたに嫉妬している",
            "魅惑的な",
            "おしつけがましい",
            "いいかげんな占いしかしない",
            "変わった占いをしている",
        ],
    ),
    (
        17,
        &[
            "かくれんぼをしている",
            "あまやどりをしている",
            "(ここにはだれもいません)",
            "家の掃除をしている",
            "取り壊しをしようとしている",
            "昔ここに住んでいた",
        ],
    ),
    (
        18,
        &[
            "畑を耕している",
            "畑を荒らしている",
            "畑泥棒の",
            "収穫している",
            "日焼けして真っ黒な",
            "嫁いできた(婿にきた)",
        ],
    ),
    (
        19,
        &[
            "粉をひいている",
            "馬に乗って風車に突進している",
            "風が吹かなくて困っている",
            "寝ている",
            "筋骨りゅうりゅうな",
            "遊んでいる",
        ],
    ),
    (
        20,
        &[
            "パーティーをしている",
            "酔っ払っている",
            "酒を仕込んでいる",
            "即売会をしている",
            "笑っている",
            "太った",
        ],
    ),
    (
        21,
        &[
            "ひとりたたずむ",
            "花から生まれた",
            "花が大好きな",
            "花粉症の",
            "花を買いにきた",
            "ラグビーをやって花をあらしてる",
        ],
    ),
    (
        22,
        &[
            "几帳面な",
            "眼鏡をかけた",
            "なまいきな",
            "なわとびをしている",
            "困っている",
            "ませている",
        ],
    ),
    (
        23,
        &[
            "本を読んでいる",
            "世間話をしたがる",
            "派手な格好をした",
            "勉強熱心な",
            "うるさい",
            "魔女のことについて調べている",
        ],
    ),
    (
        24,
        &[
            "神父さんに相談をしている",
            "結婚式を挙げている",
            "物静かな",
            "片足の無い",
            "熱い視線を送ってくる",
            "挑発してくる",
        ],
    ),
    (
        25,
        &[
            "頑固な",
            "刀の切れ味をためしたがる",
            "いいかげんな性格の",
            "スグに弟子にしたがる",
            "見せの前でウロウロしている",
            "道を尋ねている",
        ],
    ),
    (
        26,
        &[
            "不機嫌な",
            "客の意見を聞かない",
            "物を売らない",
            "不幸な気前のいい",
            "発明家の",
        ],
    ),
    (
        27,
        &[
            "恋人にプレゼントを探している",
            "香り中毒になった",
            "客に手伝わせる",
            "おまじないの好きな",
            "人好きのする",
            "いじめっこな",
        ],
    ),
    (
        28,
        &[
            "騒がしい",
            "お菓子を食べて涙を流している",
            "笑いの止まらない",
            "甘い物に目がない",
            "別れ話をしている",
            "あなたをお茶に誘う",
        ],
    ),
    (
        29,
        &[
            "フランスパンを盗んで走る",
            "しらけた顔をした",
            "店番をする",
            "あなたをバイトで使いたがる",
            "変なパンしか作らない",
            "朝が苦手な",
        ],
    ),
    (
        30,
        &[
            "偏屈な",
            "威勢のいい",
            "ケンカっぱやい",
            "野次馬根性の強い",
            "肉が食べれない",
            "心優しく気前がいい",
        ],
    ),
    (
        31,
        &[
            "夫婦ケンカをしている",
            "猫に魚を盗られた",
            "助けを求めている",
            "魚の種類がわからない",
            "『おいしい』としかいわない",
            "あやしい",
        ],
    ),
    (
        32,
        &[
            "ヤンキー風の",
            "自分がかっこいいと思っている",
            "力自慢の",
            "元は王様だといいはる",
            "魔女のファンだという",
            "子沢山の",
        ],
    ),
    (
        33,
        &[
            "わがままな",
            "かっこいい",
            "独り言を言っている",
            "変わった料理しかださない",
            "目茶苦茶辛い料理を食べている",
            "デートをしている",
        ],
    ),
    (
        34,
        &[
            "仮病を使っている",
            "不治の病を持った",
            "”おめでた”の",
            "フケた顔した",
            "髪の毛を染めた",
            "(健康でも)病名をいいたがる",
        ],
    ),
    (
        35,
        &[
            "実験をしたがる",
            "精力をつけたがっている",
            "惚れ薬を探している",
            "薬づけになっている",
            "この町まで薬を売りに来た",
            "睡眠薬で自殺をしようとしている",
        ],
    ),
    (
        36,
        &[
            "服まで質に入れた",
            "値段にケチをつけている",
            "疲れている",
            "子供を質に入れようとしている",
            "涙もろい",
            "人間不信な",
        ],
    ),
    (
        37,
        &[
            "着飾った",
            "おねだりしている",
            "退屈そうな",
            "見栄っぱりな",
            "高いものを薦める",
            "宝石など買うつもりのない",
        ],
    ),
    (
        38,
        &[
            "だだをこねている",
            "ぬいぐるみを抱いている",
            "あなたを侵略者と考えている",
            "あなたの”おしり”にさわる",
            "幸せのおもちゃを売っている",
            "あなたを自分の子と間違えている",
        ],
    ),
    (
        39,
        &[
            "人の話を聞かない",
            "気分屋な",
            "カリアゲしかできない",
            "うわさ話の好きな",
            "自動販売機を開発したという",
            "おせっかいな",
        ],
    ),
    (
        40,
        &[
            "お風呂あがりの",
            "こきつかわれている",
            "シェイプアップしている",
            "人から追われている",
            "人の体をじろじろと見る",
            "この町を案内してほしいという",
        ],
    ),
    (
        41,
        &[
            "サングラスをかけた",
            "みんな自分のファンと思っている",
            "あなたを役者と勘違いしている",
            "あなたはスターになれるという",
            "手品をしている",
            "『いそがしい』をいい続けている",
        ],
    ),
    (
        42,
        &[
            "ギャンブルをしている",
            "競技に出場している",
            "全財産を賭けている",
            "勇敢な",
            "参加者を募っている",
            "情けない競技(闘技)をしてる",
        ],
    ),
    (
        43,
        &[
            "ダンスを踊っている",
            "ブレイクダンスをして場違いな",
            "子供を背中におんぶしている",
            "あなたと踊りたがる",
            "踊ったことのない",
            "食べることに夢中な",
        ],
    ),
    (
        44,
        &[
            "２階からお金をばらまいている",
            "窓の奥で涙をながしている",
            "窓から忍びこもう",
            "ピアノを弾いている",
            "ここに住んでいる",
            "家に招待したがる",
        ],
    ),
    (
        45,
        &[
            "馬にブラシをかけている",
            "気性の激しい",
            "騎手を探している",
            "馬と話ができる",
            "馬の生まれ変わりという",
            "馬を安楽死させようか迷っている",
        ],
    ),
    (
        46,
        &[
            "いたずら好きな",
            "ライバル意識の強い",
            "魔法の下手な",
            "魔法を信じない",
            "自分を神と思っている",
            "魔法を使って人を化かしたがる",
        ],
    ),
    (
        47,
        &[
            "傷だらけな",
            "両手に宝物を持った",
            "かわいい",
            "地図を見ながら出てきている",
            "剣を持った",
            "ダンジョンの主といわれる",
        ],
    ),
    (
        48,
        &[
            "墓参りをしている",
            "耳の遠い",
            "死んでしまった",
            "葬式をしている",
            "きもだめしをしている",
            "墓守をしている",
        ],
    ),
];

/// Ruby `getPersonTable1` の表。
static PERSON_TABLE1: &[(i64, PersonItem)] = &[
    (11, PersonItem::Text("おじさん")),
    (12, PersonItem::Text("おばさん")),
    (13, PersonItem::Text("おじいさん")),
    (14, PersonItem::Text("おばあさん")),
    (15, PersonItem::Text("男の子")),
    (16, PersonItem::Text("女の子")),
    (22, PersonItem::Text("美少女")),
    (23, PersonItem::Text("美少年")),
    (24, PersonItem::Text("青年")),
    (25, PersonItem::Text("少年")),
    (26, PersonItem::Text("男女(カップル)")),
    (33, PersonItem::Text("新婚さん")),
    (34, PersonItem::Text("お兄さん")),
    (35, PersonItem::Text("お姉さん")),
    (36, PersonItem::Text("店主(お店の人)")),
    (44, PersonItem::Text("王様")),
    (45, PersonItem::Text("衛兵")),
    (46, PersonItem::Text("魔女")),
    (55, PersonItem::Text("お姫様")),
    (56, PersonItem::Next("表２へ")),
    (66, PersonItem::Next("表２へ")),
];

/// Ruby `getPersonTable2` の表。
static PERSON_TABLE2: &[(i64, PersonItem)] = &[
    (11, PersonItem::Text("魔法使い")),
    (12, PersonItem::Text("観光客")),
    (13, PersonItem::Text("先生")),
    (14, PersonItem::Text("探偵")),
    (15, PersonItem::Text("刷")),
    (16, PersonItem::Text("お嬢様")),
    (22, PersonItem::Text("お嬢様")),
    (23, PersonItem::Text("紳士")),
    (24, PersonItem::Text("ご婦人")),
    (25, PersonItem::Text("女王様")),
    (26, PersonItem::Text("職人さん")),
    (33, PersonItem::Text("女子高生")),
    (34, PersonItem::Text("学生")),
    (35, PersonItem::Text("剣闘士")),
    (36, PersonItem::Text("鳥")),
    (44, PersonItem::Text("猫")),
    (45, PersonItem::Text("犬")),
    (46, PersonItem::Text("カエル")),
    (55, PersonItem::Text("蛇")),
    (56, PersonItem::Next("表３へ")),
    (66, PersonItem::Next("表３へ")),
];

/// Ruby `getPersonTable3` の表。
static PERSON_TABLE3: &[(i64, PersonItem)] = &[
    (11, PersonItem::Text("貴族")),
    (12, PersonItem::Text("いるか")),
    (13, PersonItem::Text("だいこん")),
    (14, PersonItem::Text("じゃがいも")),
    (15, PersonItem::Text("にんじん")),
    (16, PersonItem::Text("ドラゴン")),
    (22, PersonItem::Text("ゾンビ")),
    (23, PersonItem::Text("幽霊")),
    (24, PersonItem::Text("うさぎ")),
    (25, PersonItem::Text("天使")),
    (26, PersonItem::Text("悪魔")),
    (33, PersonItem::Text("赤ちゃん")),
    (34, PersonItem::Text("馬")),
    (35, PersonItem::Text("石")),
    (36, PersonItem::Text("お母さん")),
    (44, PersonItem::Text("妖精")),
    (45, PersonItem::Text("守護霊")),
    (46, PersonItem::Text("猫神様")),
    (55, PersonItem::Text("ロボット")),
    (56, PersonItem::Text("恐ろしい人")),
    (66, PersonItem::Next("表４へ")),
];

/// Ruby `getPersonTable4` の表。
static PERSON_TABLE4: &[(i64, PersonItem)] = &[
    (11, PersonItem::Text("魔女エディス")),
    (12, PersonItem::Text("魔女レーデルラン")),
    (13, PersonItem::Text("魔女キリル")),
    (14, PersonItem::Text("大魔女”ロロ”様")),
    (15, PersonItem::Text("エディスのお母さん”エリー”")),
    (16, PersonItem::Text("猫トンガリ")),
    (22, PersonItem::Text("猫ヒューベ")),
    (23, PersonItem::Text("猫ゆうのす")),
    (24, PersonItem::Text("猫集会の集団の一団")),
    (25, PersonItem::Text("岩")),
    (26, PersonItem::Text("PCの母")),
    (33, PersonItem::Text("PCの父")),
    (34, PersonItem::Text("PCの兄")),
    (35, PersonItem::Text("PCの姉")),
    (36, PersonItem::Text("PCの弟")),
    (44, PersonItem::Text("PCの妹")),
    (45, PersonItem::Text("PCの遠い親戚")),
    (46, PersonItem::Text("PCの死んだはずの両親")),
    (55, PersonItem::Text("初恋の人")),
    (
        56,
        PersonItem::Text(
            "分かれた女(男)、不倫中の相手、または独身PCの場合、二股をかけている二人の両方",
        ),
    ),
    (66, PersonItem::Text("宇宙人")),
];

/// Ruby の `getPersonTable1`〜`getPersonTable4`。
///
/// `PersonItem::Next` は Ruby の `gotoNextTable` lambda に対応し、
/// 常に「次の表」を引いて結果を連結する。
static PERSON_TABLES: &[&[(i64, PersonItem)]] =
    &[PERSON_TABLE1, PERSON_TABLE2, PERSON_TABLE3, PERSON_TABLE4];

/// Ruby `getSuccessText` の `table`。
static SUCCESS_TEXT_TABLE: &[(i64, &str)] = &[
    (0, "失敗"),
    (1, "１レベル成功(成功)"),
    (2, "２レベル成功(大成功)"),
    (3, "３レベル成功(奇跡的大成功)"),
    (4, "４レベル成功(歴史的大成功)"),
    (5, "５レベル成功(伝説的大成功)"),
    (6, "６レベル成功(神話的大成功)"),
];
/// Ruby `BCDice::GameSystem::WitchQuest`（ID: `WitchQuest`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitchQuest;

impl GameSystem for WitchQuest {
    fn id(&self) -> &'static str {
        "WitchQuest"
    }

    fn name(&self) -> &'static str {
        "ウィッチクエスト"
    }

    fn sort_key(&self) -> &'static str {
        "ういつちくえすと"
    }

    fn help_message(&self) -> &'static str {
        r"・チャレンジ(成功判定)(WQn)
　n回2d6ダイスを振って判定を行います。
　例）WQ3
・SET（ストラクチャーカードの遭遇表(SETn)
　ストラクチャーカードの番号(n)の遭遇表結果を得ます。
　例）SET1　SET48
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["WQ", "SET"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `WitchQuest#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /WQ(\d+)/ ... when /SET(\d+)/ ...（どちらもアンカー無し）
        if let Some(m) = challenge_pattern().captures(command) {
            let number = ruby_to_i(&m[1]);
            return Ok(Some(SpecificCommandOutput::text(challenge(number, rng)?)));
        }

        if let Some(m) = structure_encounter_pattern().captures(command) {
            let number = ruby_to_i(&m[1]);
            return Ok(structure_encounter(number, rng)?.map(SpecificCommandOutput::text));
        }

        Ok(None)
    }
}

/// Ruby `/WQ(\d+)/`。
fn challenge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"WQ(\d+)").expect("valid regex"))
}

/// Ruby `/SET(\d+)/`。
fn structure_encounter_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"SET(\d+)").expect("valid regex"))
}

/// Ruby `WitchQuest#challenge`。2D6のゾロ目の回数を数える。
fn challenge(number: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut success = 0i64;
    let mut results: Vec<String> = Vec::new();

    // Ruby: number.times do ... end（number <= 0 なら1度も回らない）
    let mut i = 0;
    while i < number {
        let value1 = rng.roll_once(6)?;
        let value2 = rng.roll_once(6)?;

        if value1 == value2 {
            success += 1;
        }

        results.push(format!("{value1},{value2}"));
        i += 1;
    }

    Ok(format!(
        "({}) ＞ {}",
        results.join(" / "),
        success_text(success)
    ))
}

/// Ruby `WitchQuest#getSuccessText`。
fn success_text(success: i64) -> &'static str {
    // Ruby: return table.last.last if success >= table.last.first
    if let Some((last_key, last_text)) = SUCCESS_TEXT_TABLE.last() {
        if success >= *last_key {
            return last_text;
        }
    }
    get_table_by_number(success, SUCCESS_TEXT_TABLE)
}

/// Ruby `WitchQuest#getStructureEncounter`。
fn structure_encounter(number: i64, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    // Ruby: get_table_by_number(number, tables, nil) — 該当が無ければ nil
    let table = STRUCTURE_ENCOUNTER_TABLES
        .iter()
        .find(|(key, _)| *key >= number)
        .map(|(_, items)| *items);
    let Some(table) = table else {
        return Ok(None);
    };

    let (text, index) = get_table_by_1d6(table, rng)?;
    let person = person_table(0, rng)?;

    Ok(Some(format!("SET{number} ＞ {index}:{text}{person}")))
}

/// Ruby `Base#get_table_by_1d6`（= `get_table_by_nDx(table, 1, 6)`）。
///
/// 項目が無ければ Ruby と同じく `["1", 0]` を返す（26番の表は項目が5個しかない）。
fn get_table_by_1d6(
    table: &[&'static str],
    rng: &mut Randomizer,
) -> Result<(&'static str, i64), EvalError> {
    let num = rng.roll_sum(1, 6)?;
    let text = usize::try_from(num - 1)
        .ok()
        .and_then(|i| table.get(i))
        .copied();
    match text {
        Some(text) => Ok((text, num)),
        None => Ok(("1", 0)),
    }
}

/// Ruby `WitchQuest#getPersonTable`（`getPersonTable1`〜`4` の共通部分）。
///
/// `index` は [`PERSON_TABLES`] の添字。`PersonItem::Next` を引いたら次の表へ連鎖する。
fn person_table(index: usize, rng: &mut Randomizer) -> Result<String, EvalError> {
    let Some(table) = PERSON_TABLES.get(index) else {
        // 原典では連鎖先が必ず存在する（表4に `gotoNextTable` は無い）
        return Ok(String::new());
    };

    let number = rng.roll_d66(D66SortType::Asc)?;

    // Ruby: get_table_by_number(number, table)（既定値 "1"）。Proc なら呼び出す。
    let item = table.iter().find(|(key, _)| *key >= number).map(|(_, v)| v);
    let body = match item {
        Some(PersonItem::Text(text)) => (*text).to_owned(),
        Some(PersonItem::Next(prefix)) => {
            format!("{prefix}{}", person_table(index + 1, rng)?)
        }
        None => "1".to_owned(),
    };

    Ok(format!(" ＞ {number}:{body}"))
}

/// Ruby `Base#get_table_by_number(index, table)`（既定値は `"1"`）。
///
/// 「最初に `item[0] >= index` となった項目」を返す。完全一致ではない。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    table
        .iter()
        .find(|(number, _)| *number >= index)
        .map_or("1", |(_, text)| *text)
}

/// Ruby `String#to_i`。ここに来るのは `\d+` なので符号や空白は現れない。
fn ruby_to_i(s: &str) -> i64 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        // Ruby: "".to_i == 0
        return 0;
    }
    // 桁あふれは Ruby だと Bignum になる。i64 に収まらない場合は飽和させ、
    // 振る回数の上限（TooManyRandsError）へ落ちるようにする。
    digits.parse().unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WitchQuest",
            "WitchQuest.toml",
            10,
        );
    }
}
