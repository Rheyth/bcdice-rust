//! P4で手書き移植した `lib/bcdice/game_system/YankeeYogSothoth.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `YankeeYogSothoth#result_2d6`（ファンブル / スペシャル）
//! - `#eval_game_system_specific_command`（`TABLES` と `RTT` のランダム特技表）
//! - `NICKNAME_TABLE1`〜`4`（二つ名決定表 `FTNT` が連鎖して引くD66表）

use crate::dice_table::{
    ChainTable, D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table,
    TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::YankeeYogSothoth`（ID: `YankeeYogSothoth`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YankeeYogSothoth;

impl GameSystem for YankeeYogSothoth {
    fn id(&self) -> &'static str {
        "YankeeYogSothoth"
    }

    fn name(&self) -> &'static str {
        "ヤンキー＆ヨグ＝ソトース"
    }

    fn sort_key(&self) -> &'static str {
        "やんきいあんとよくそとおす"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
スペシャル／ファンブル／成功／失敗を判定
・各種表
※うろつき～決闘フェイズ
FT	ファンブル表
WT	変調表
RTTn	ランダム特技決定表(n：分野番号、省略可能)
RCT ランダム分野決定表
KKT	関係表
DBRT	他愛のない会話表
TKT	戦う理由表

※武勇伝フェイズ
BUDT	武勇伝表
GUDT	ガイヤンキー武勇伝表
FTNT	二つ名決定表
DAIT	第一印象表
TKKT	ツレ関係表

※帰還フェイズ
GSST	現実世界生活表
GYST	ガイヤンキー生活表
HPST	病院生活表
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTT[1-6]?",
            "RCT",
            "FTNT",
            "FT",
            "WT",
            "KKT",
            "DBRT",
            "TKT",
            "BUDT",
            "GUDT",
            "DAIT",
            "TKKT",
            "GSST",
            "GYST",
            "HPST",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> crate::enums::D66SortType {
        crate::enums::D66SortType::Asc
    }

    /// Ruby `YankeeYogSothoth#result_2d6`。
    ///
    /// ファンブル / スペシャル以外は `nil` を返し、`Base#check_result` の
    /// 成功 / 失敗判定（`result_ndx`）に委ねる。
    fn result_2d6(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :>=
        if cmp_op != CmpOp::Ge {
            return None;
        }

        if dice_total <= 2 {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "ファンブル(判定失敗。ファンブル表（FT）を振ること)",
            ))))
        } else if dice_total >= 12 {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "スペシャル(判定成功。【テンション】が１段階上昇)",
            ))))
        } else {
            None
        }
    }

    /// Ruby `YankeeYogSothoth#eval_game_system_specific_command`。
    ///
    /// `roll_tables(command, TABLES) || RTT.roll_command(randomizer, command)`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(RTT
            .roll_command(rng, command)?
            .map(SpecificCommandOutput::text))
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

// ---------------------------------------------------------------------------
// 二つ名表（FTNT の連鎖先）
// ---------------------------------------------------------------------------

/// Ruby `NICKNAME_TABLE1`。
static NICKNAME_TABLE1: D66Table = D66Table::new(
    "二つ名表1",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「愛死天流（あいしてる）」")),
        (12, TableItem::Text("「喧嘩上等（けんかじょうとう）」")),
        (13, TableItem::Text("「正々堂々（せいせいどうどう）」")),
        (14, TableItem::Text("「天下無敵（てんかむてき）」")),
        (15, TableItem::Text("「一騎当千（いっきとうせん）」")),
        (16, TableItem::Text("「威風堂々（いふうどうどう）」")),
        (22, TableItem::Text("「焼肉定食（やきにくていしょく）」")),
        (23, TableItem::Text("「完全無欠（かんぜんむけつ）」")),
        (24, TableItem::Text("「獅子奮迅（ししふんじん）」")),
        (25, TableItem::Text("「臥薪嘗胆（がしんしょうたん）」")),
        (26, TableItem::Text("「疾風迅雷（しっぷうじんらい）」")),
        (33, TableItem::Text("「夜露死苦（よろしく）」")),
        (34, TableItem::Text("「天上天下（てんじょうてんげ）」")),
        (35, TableItem::Text("「唯我独尊（ゆいがどくそん）」")),
        (36, TableItem::Text("「電光石火（でんこうせっか）」")),
        (44, TableItem::Text("「仏恥義理（ぶっちぎり）」")),
        (
            45,
            TableItem::Text("「百戦百勝（ひゃくせんひゃくしょう）」"),
        ),
        (46, TableItem::Text("「百戦錬磨（ひゃくせんれんま）」")),
        (55, TableItem::Text("「残酷非道（ざんこくひどう）」")),
        (56, TableItem::Text("「一意専心（いちいせんしん）」")),
        (66, TableItem::Text("「時給千円（じきゅうせんえん）」")),
    ],
);

/// Ruby `NICKNAME_TABLE2`。
static NICKNAME_TABLE2: D66Table = D66Table::new(
    "二つ名表2",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「みんなの」")),
        (12, TableItem::Text("「スルー推奨」")),
        (13, TableItem::Text("「暴れん坊」")),
        (14, TableItem::Text("「仲間思い」")),
        (15, TableItem::Text("「サボり魔」")),
        (16, TableItem::Text("「熱血番長の」")),
        (22, TableItem::Text("「今日がダメでも明日がある」")),
        (23, TableItem::Text("「すぐカッとなる」")),
        (24, TableItem::Text("「夢を応援する」")),
        (25, TableItem::Text("「地元じゃ有名な」")),
        (26, TableItem::Text("「喧嘩慣れている」")),
        (33, TableItem::Text("「いつかビックになる」")),
        (34, TableItem::Text("「いいやつの」")),
        (35, TableItem::Text("「意外とまじめな」")),
        (36, TableItem::Text("「イイ感じの」")),
        (44, TableItem::Text("「家族想いの」")),
        (45, TableItem::Text("「とにかくモテる」")),
        (46, TableItem::Text("「学校を代表するワル」")),
        (55, TableItem::Text("「邪神ハンター」")),
        (56, TableItem::Text("「男前／イイ女」")),
        (66, TableItem::Text("「悪そうなやつはだいたい友達」")),
    ],
);

/// Ruby `NICKNAME_TABLE3`。
static NICKNAME_TABLE3: D66Table = D66Table::new(
    "二つ名表3",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「ファッションヤンキー」")),
        (12, TableItem::Text("「誰もが知っている」")),
        (13, TableItem::Text("「チャラい」")),
        (14, TableItem::Text("「ツヨメ」")),
        (15, TableItem::Text("「中学時代はすごかった」")),
        (16, TableItem::Text("「イカれたやつ」")),
        (22, TableItem::Text("「道徳の授業で泣いた」")),
        (23, TableItem::Text("「マジか」")),
        (24, TableItem::Text("「イケイケ」")),
        (25, TableItem::Text("「鬼語り」")),
        (26, TableItem::Text("「とりま」")),
        (33, TableItem::Text("「ちょっと眠たい」")),
        (34, TableItem::Text("「パネエ」")),
        (35, TableItem::Text("「エモい」")),
        (36, TableItem::Text("「やべーぞ！」")),
        (44, TableItem::Text("「お腹が減っている」")),
        (45, TableItem::Text("「むっつりスケベの」")),
        (46, TableItem::Text("「いじわるな」")),
        (55, TableItem::Text("「全国区に報道された」")),
        (56, TableItem::Text("「毎日が楽しい」")),
        (66, TableItem::Text("「おやじ狩り狩り」")),
    ],
);

/// Ruby `NICKNAME_TABLE4`。
static NICKNAME_TABLE4: D66Table = D66Table::new(
    "二つ名表4",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「国産」")),
        (12, TableItem::Text("「ブレブレ」")),
        (13, TableItem::Text("「ロボ」")),
        (14, TableItem::Text("「大銀河」")),
        (15, TableItem::Text("「超獣」")),
        (16, TableItem::Text("「ミステリー」")),
        (22, TableItem::Text("「超電磁」")),
        (23, TableItem::Text("「危険な」")),
        (24, TableItem::Text("「湯上がり」")),
        (25, TableItem::Text("「すごい」")),
        (26, TableItem::Text("「エロ」")),
        (33, TableItem::Text("「福岡」")),
        (34, TableItem::Text("「エリート」")),
        (35, TableItem::Text("「どんまい」")),
        (36, TableItem::Text("「がり勉」")),
        (44, TableItem::Text("「東京」")),
        (45, TableItem::Text("「スペース」")),
        (46, TableItem::Text("「永遠の」")),
        (55, TableItem::Text("「大阪」")),
        (56, TableItem::Text("「輝け！」")),
        (66, TableItem::Text("「名古屋」")),
    ],
);

// ---------------------------------------------------------------------------
// ランダム特技表
// ---------------------------------------------------------------------------

/// Ruby `RTT` の分野「苦手」。
static RTT_SKILLS1: &[&str] = &[
    "大人",
    "勉強",
    "敗北",
    "昆虫",
    "親",
    "異性",
    "孤独",
    "高所",
    "暗がり",
    "ホラー",
    "子供",
];

/// Ruby `RTT` の分野「部活」。
static RTT_SKILLS2: &[&str] = &[
    "柔道",
    "プロレス",
    "テコンドー",
    "空手",
    "ボクシング",
    "帰宅",
    "剣道",
    "野球",
    "応援団",
    "科学",
    "文系",
];

/// Ruby `RTT` の分野「中学時代」。
static RTT_SKILLS3: &[&str] = &[
    "悪ガキ",
    "統一",
    "バイト",
    "習い事",
    "喧嘩",
    "サボり",
    "マジメくん",
    "遊び歩き",
    "真似ごと",
    "部活",
    "何もしない",
];

/// Ruby `RTT` の分野「趣味」。
static RTT_SKILLS4: &[&str] = &[
    "すけべ",
    "車・バイク",
    "家事",
    "料理",
    "運動",
    "修行",
    "ファッション",
    "つるむ",
    "寝る",
    "ゲーム",
    "読書",
];

/// Ruby `RTT` の分野「スタイル」。
static RTT_SKILLS5: &[&str] = &[
    "テキトー",
    "ばか",
    "オラオラ",
    "熱血",
    "硬派",
    "自然体",
    "軟派",
    "自分大好き",
    "腹黒",
    "クール",
    "インテリ",
];

/// Ruby `RTT` の分野「好み」。
static RTT_SKILLS6: &[&str] = &[
    "だらだら",
    "食事",
    "逆転",
    "家族",
    "支配",
    "褒められる",
    "恋愛",
    "友情",
    "勝利",
    "金",
    "静寂",
];

/// Ruby `RTT` の分野一覧。
static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("苦手", RTT_SKILLS1),
    SaiFicCategory::new("部活", RTT_SKILLS2),
    SaiFicCategory::new("中学時代", RTT_SKILLS3),
    SaiFicCategory::new("趣味", RTT_SKILLS4),
    SaiFicCategory::new("スタイル", RTT_SKILLS5),
    SaiFicCategory::new("好み", RTT_SKILLS6),
];

/// Ruby `RTT`（`rtt_format` だけ独自書式）。
static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES).with_formats(SaiFicFormats {
    rtt: "ランダム指定特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
    ..SaiFicFormats::DEFAULT
});

// ---------------------------------------------------------------------------
// TABLES
// ---------------------------------------------------------------------------

/// Ruby `TABLES["FTNT"]`（二つ名決定表）。1D6 で二つ名表1〜4のどれかへ連鎖する。
static TABLE_FTNT: ChainTable = ChainTable::from_dice(
    "二つ名決定表",
    1,
    6,
    &[
        TableItem::Table(&NICKNAME_TABLE1),
        TableItem::Table(&NICKNAME_TABLE1),
        TableItem::Table(&NICKNAME_TABLE2),
        TableItem::Table(&NICKNAME_TABLE2),
        TableItem::Table(&NICKNAME_TABLE3),
        TableItem::Table(&NICKNAME_TABLE4),
    ],
);

/// Ruby `TABLES["FT"]`（ファンブル表）。
static TABLE_FT: Table = Table::from_dice(
    "ファンブル表",
    1,
    6,
    &[
        "やっちまった……。テンションが1段階減少する。",
        "ひょうなことから嫌な状況になる。ランダムに変調（WT）を1つ受ける。",
        "あまりにもカッコ悪いところが伝わってしまう。自分に対して【友情度】を持つPC全員は、リスペクトにチェックを入れる。",
        "自分の絶望を観測し、邪神が活性化する。バッドヤンキーの「ケツモチ邪神の加護」が1点上昇する。",
        "つまらないことで怪我をする。自分の【HP】が1D6点減少する。",
        "逆境に燃える。テンションが1段階上昇する。",
    ],
);

/// Ruby `TABLES["WT"]`（変調表）。
static TABLE_WT: Table = Table::from_dice(
    "変調表",
    1,
    6,
    &[
        "毒：サイクル終了時もしくはラウンド終了時に、2D6点のダメージを受ける。",
        "呪い：熱血蘇生の達成値が2点減少する。NPCが受けた場合、受けるダメージが2点上昇する。",
        "火傷：テンションの効果によって、【攻撃力】が上昇しない。NPCは【攻撃力】が2点低いものとして扱う（最低０点）。",
        "骨折：判定に失敗するたびに、5点のダメージを受ける。",
        "出血：サイクル終了時もしくはラウンド終了時に、2点のダメージを受ける。また、施設やアイテムの効果で【HP】が上昇しない。",
        "目つぶし：判定の達成値が2点減少する。",
    ],
);

/// Ruby `TABLES["KKT"]`（関係表）。
static TABLE_KKT: Table = Table::from_dice(
    "関係表",
    1,
    6,
    &[
        "「家族／気に食わない」",
        "「親友／近寄るな」",
        "「悪友／こざかしい」",
        "「ライバル／チンピラ」",
        "「いい奴／悪い奴」",
        "「利用できる／ヘタレ」",
    ],
);

/// Ruby `TABLES["DBRT"]`（他愛のない会話表）。
static TABLE_DBRT: D66Table = D66Table::new(
    "他愛のない会話表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「政治の話」")),
        (12, TableItem::Text("「勉強の話」")),
        (13, TableItem::Text("「友達の話」")),
        (14, TableItem::Text("「兄弟姉妹の話」")),
        (15, TableItem::Text("「好きなものの話」")),
        (16, TableItem::Text("「嫌いなものの話」")),
        (22, TableItem::Text("「ラーメンの話」")),
        (23, TableItem::Text("「コンビニの話」")),
        (24, TableItem::Text("「学校生活の話」")),
        (25, TableItem::Text("「先輩後輩の話」")),
        (26, TableItem::Text("「趣味の話」")),
        (33, TableItem::Text("「肉の話」")),
        (34, TableItem::Text("「中学時代の話」")),
        (35, TableItem::Text("「喧嘩の話」")),
        (36, TableItem::Text("「ファッションの話」")),
        (44, TableItem::Text("「家の話」")),
        (45, TableItem::Text("「好みの異性の話」")),
        (46, TableItem::Text("「テレビ番組の話」")),
        (55, TableItem::Text("「野菜の話」")),
        (56, TableItem::Text("「部活の話」")),
        (66, TableItem::Text("「ダブりの話」")),
    ],
);

/// Ruby `TABLES["TKT"]`（戦う理由表）。
static TABLE_TKT: D66Table = D66Table::new(
    "戦う理由表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("「なんとなく」")),
        (12, TableItem::Text("「好みのエルフがいた」")),
        (13, TableItem::Text("「エルフに世話になった」")),
        (14, TableItem::Text("「ドワーフの飯がうまかった」")),
        (
            15,
            TableItem::Text("「ドワーフにファッション特徴を作ってもらった」"),
        ),
        (
            16,
            TableItem::Text("「妖精たちのいたずらがほほえましかった」"),
        ),
        (
            22,
            TableItem::Text("「バッドヤンキーと昔からの因縁があるから」"),
        ),
        (23, TableItem::Text("「バッドヤンキーが気に入らなかった」")),
        (24, TableItem::Text("「強いやつと戦いたい」")),
        (25, TableItem::Text("「異世界にワクワクしているから」")),
        (
            26,
            TableItem::Text("「バッドヤンキー集団に迷惑を受けたから」"),
        ),
        (33, TableItem::Text("「夢見るNPCが好みだったから」")),
        (34, TableItem::Text("「夢見るNPCの夢に共感したから」")),
        (
            35,
            TableItem::Text("「夢見るNPCの夢を応援したいと思ったから」"),
        ),
        (36, TableItem::Text("「夢見るNPCを放っておけないから」")),
        (44, TableItem::Text("「家に帰りたいから」")),
        (45, TableItem::Text("「夢見るNPCは友達だから」")),
        (46, TableItem::Text("「他のPCと気が合ったから」")),
        (55, TableItem::Text("「マーメイドと仲良くなった」")),
        (56, TableItem::Text("「退屈を紛らわせられそうだから」")),
        (66, TableItem::Text("「ただ暴れたかった」")),
    ],
);

/// Ruby `TABLES["BUDT"]`（武勇伝表）。
static TABLE_BUDT: D66Table = D66Table::new(
    "武勇伝表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("バッドヤンキーのチームに自分が所属する学校を破壊されたが、最後まで戦った。／テンションが1段階上昇")),
        (12, TableItem::Text("バッドヤンキーチームの兵隊が襲い掛かってきたが、撃退した。／【HP】＋３")),
        (13, TableItem::Text("バッドヤンキーと何度も戦い、ライバルとして認識されていた。／【打たれ強さ】＋１")),
        (14, TableItem::Text("バッドヤンキー配下の集団をいくつか潰してまわっていた。／【攻撃力】＋１")),
        (15, TableItem::Text("バッドヤンキーのチームに入りそうになった後輩を説得した。／【HP】＋３")),
        (16, TableItem::Text("バッドヤンキーに支配されていた店を救った。／【攻撃力】＋１")),
        (22, TableItem::Text("アメリカで暴れた。／テンションが1段階上昇")),
        (23, TableItem::Text("学校をサボって、日本全国を旅をしてまわった。／【HP】＋３")),
        (24, TableItem::Text("好きなアーティストのライブに行き、マナーの悪いファンを黙らせた。／【打たれ強さ】＋１")),
        (25, TableItem::Text("抗争中の学校に一人で乗り込んで、戦いを終わらせた。／【攻撃力】＋１")),
        (26, TableItem::Text("へまをした仲間を助けるため、頭を下げた。／【HP】＋３")),
        (33, TableItem::Text("大規模な運動会で活躍し、最優秀賞を獲得した。／【攻撃力】＋１")),
        (34, TableItem::Text("家族や仲間に迷惑をかけたチームを潰した。／【打たれ強さ】＋１")),
        (35, TableItem::Text("暴走族を一人で潰した。／【攻撃力】＋１")),
        (36, TableItem::Text("本職（ヤクザ）と戦って謝らせた。／【打たれ強さ】＋１")),
        (44, TableItem::Text("ドッジボール大会に出場し、優勝をして賞品を手に入れた。／「絆創膏」「テンアゲアイテム」「ポーション」「お守り」のうち１つを選んで獲得する")),
        (45, TableItem::Text("仲間たちと一緒に学校行事を盛り上げた。／【打たれ強さ】＋１")),
        (46, TableItem::Text("仲間と一緒にディスカウントストアで買い物をし、キャンプをした。／「絆創膏」「テンアゲアイテム」「ポーション」「お守り」のうち１つを選んで獲得する")),
        (55, TableItem::Text("隣のプレイヤーのPCが所属する高校と大きな抗争をした。／右隣のプレイヤーのPCに対する【友情度】が1点上昇")),
        (56, TableItem::Text("修学旅行先で喧嘩し、その後友情を深めた。／【攻撃力】＋１")),
        (66, TableItem::Text("隣のプレイヤーのPCと一緒に、大きな悪の組織を潰した。／右隣のプレイヤーのPCに対する【友情度】が1点上昇")),
    ],
);

/// Ruby `TABLES["GUDT"]`（ガイヤンキー武勇伝表）。
static TABLE_GUDT: D66Table = D66Table::new(
    "ガイヤンキー武勇伝表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("アザトースが突然接触してきたので、殴って追い返した。／テンションが1段階上昇")),
        (12, TableItem::Text("シュブ＝ニグラスのサバトに乗り込んで潰した。／【HP】＋３")),
        (13, TableItem::Text("クトゥルフの落とし子を殴り倒して追い返した。／【打たれ強さ】＋１")),
        (14, TableItem::Text("ヨグ=ソトースの勧誘を受けたが、断ってやった。／【攻撃力】＋１")),
        (15, TableItem::Text("深きものどもが住む漁村を訪ね、罠にはめられたが脱出した。／【HP】＋３")),
        (16, TableItem::Text("一晩飲み明かした相手がナイアーラトテップだった。／【攻撃力】＋１")),
        (22, TableItem::Text("生きてる恐竜と出会った。／テンションが1段階上昇")),
        (23, TableItem::Text("ファンタジー世界を冒険者として旅してまわった。／【HP】＋３")),
        (24, TableItem::Text("町で起こった少女たちの失踪事件を解決した。／【打たれ強さ】＋１")),
        (25, TableItem::Text("バッドヤンキーに潰された騎士団を鼓舞して、立て直しに協力した。／【攻撃力】＋１")),
        (26, TableItem::Text("大きな城下町に起こった殺人事件や傷害事件を幾つも解決した。／【HP】＋３")),
        (33, TableItem::Text("大きな城下町で、テンションが上がっていろいろ買い込んでしまった。／「絆創膏」「テンアゲアイテム」「ポーション」「お守り」のうち１つを選んで獲得する")),
        (34, TableItem::Text("エルフの森を燃やしつくそうとする拝火暴走族をこらしめた。／【打たれ強さ】＋１")),
        (35, TableItem::Text("ドワーフの洞窟に現われた巨大ワームを投げ飛ばした。／【攻撃力】＋１")),
        (36, TableItem::Text("妖精たちの村に迷い込んで、村を荒らそうとするゴブリンをブッ飛ばした。／【打たれ強さ】＋１")),
        (44, TableItem::Text("巨大な王国が主催している武術大会で優勝し、名誉とアイテムを手に入れた。／「絆創膏」「テンアゲアイテム」「ポーション」「お守り」のうち１つを選んで獲得する")),
        (45, TableItem::Text("ゴブリンの襲撃から町を守り切った。／【打たれ強さ】＋１")),
        (46, TableItem::Text("悪いチームにさらわれた姫や王子様を助けたら、惚れられた。／【攻撃力】＋１")),
        (55, TableItem::Text("次に会うヤンキーのために、この世界の土産話を作ってきた。／右隣のプレイヤーのPCに対する【友情度】が1点上昇")),
        (56, TableItem::Text("悪い魔法使いの儀式を突き止めて、潰した。／【攻撃力】＋１")),
        (66, TableItem::Text("次に会うヤンキーのために、うまいものを用意した。／右隣のプレイヤーのPCに対する【友情度】が1点上昇")),
    ],
);

/// Ruby `TABLES["DAIT"]`（第一印象表）。
static TABLE_DAIT: Table = Table::from_dice(
    "第一印象表",
    1,
    6,
    &[
        "「ヤベエ」",
        "「パネエ」",
        "「スゲエ」",
        "「びっくり」",
        "「たばい」",
        "「アウトオブ眼中」",
    ],
);

/// Ruby `TABLES["TKKT"]`（ツレ関係表）。
static TABLE_TKKT: Table = Table::from_dice(
    "ツレ関係表",
    1,
    6,
    &[
        "「すごそう」",
        "「勇者様」",
        "「つよい」",
        "「いい人」",
        "「かっこいい」",
        "「利用できる」",
    ],
);

/// Ruby `TABLES["GSST"]`（現実世界生活表）。
static TABLE_GSST: D66Table = D66Table::new(
    "現実世界生活表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("喧嘩に明け暮れた")),
        (12, TableItem::Text("真面目に授業を受けた")),
        (13, TableItem::Text("今回の仲間と食事をしに行った")),
        (14, TableItem::Text("チーム同士の抗争を沈めた")),
        (15, TableItem::Text("ぼーっとしていた")),
        (16, TableItem::Text("バイトに専念した")),
        (22, TableItem::Text("仲間とバーベキューをした")),
        (23, TableItem::Text("自分の体を鍛えることにした")),
        (24, TableItem::Text("仲間との毎日をより大切にした")),
        (25, TableItem::Text("家族とゆっくりすごした")),
        (26, TableItem::Text("喧嘩の技術を磨いた")),
        (33, TableItem::Text("本職（ヤクザ）と喧嘩をした")),
        (34, TableItem::Text("好きなだけ寝た")),
        (35, TableItem::Text("ツレができた")),
        (36, TableItem::Text("今回の仲間と旅に出た")),
        (44, TableItem::Text("異性と遊園地に行くことになった")),
        (45, TableItem::Text("あの戦いの日々を思い返していた")),
        (46, TableItem::Text("次の戦いに備えた")),
        (55, TableItem::Text("運動部の助っ人として、大会に出た")),
        (56, TableItem::Text("好きなだけ好物を食べた")),
        (
            66,
            TableItem::Text("汚い大人の罠にはめられたが、なんとかした"),
        ),
    ],
);

/// Ruby `TABLES["GYST"]`（ガイヤンキー生活表）。
static TABLE_GYST: D66Table = D66Table::new(
    "ガイヤンキー生活表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("ツレと生活をした")),
        (12, TableItem::Text("異世界について学んだ")),
        (
            13,
            TableItem::Text("エルフの美形（平均年齢200歳）に接待を受けた"),
        ),
        (14, TableItem::Text("ドワーフから地元の酒をもらった")),
        (15, TableItem::Text("妖精の村に迷い込んでしまった")),
        (16, TableItem::Text("この世界の遺跡を回った")),
        (22, TableItem::Text("この世界に野球などのスポーツを広めた")),
        (
            23,
            TableItem::Text("広大な森の中で迷ってしまい、数か月ほどサバイバルした"),
        ),
        (24, TableItem::Text("不思議な力が溢れる泉の水を飲み干した")),
        (
            25,
            TableItem::Text("魔法使いの研究に協力したが、さっぱりだった"),
        ),
        (26, TableItem::Text("ハーピィに誘われて空の旅を満喫した")),
        (33, TableItem::Text("この世界にヤンキー文化を伝えた")),
        (
            34,
            TableItem::Text("バッドヤンキーに荒らされた小さな村を復興した"),
        ),
        (35, TableItem::Text("悪徳領主にさらわれた少女を助けた")),
        (36, TableItem::Text("わるい商人を殴り飛ばした")),
        (44, TableItem::Text("エルフの漫画家が誕生するのを見届けた")),
        (45, TableItem::Text("巨大なドラゴンと殴りあって勝利した")),
        (
            46,
            TableItem::Text("海中に住むマーメイドを脅かす悪人を退治した"),
        ),
        (
            55,
            TableItem::Text("邪神を信奉している神殿に殴り込みをして、企みを阻止した"),
        ),
        (
            56,
            TableItem::Text("天使っぽいのが悪いことをしていたので蹴り飛ばした"),
        ),
        (66, TableItem::Text("農作業をした")),
    ],
);

/// Ruby `TABLES["HPST"]`（病院生活表）。
static TABLE_HPST: D66Table = D66Table::new(
    "病院生活表",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("治療に専念した")),
        (12, TableItem::Text("見舞いでもらった漫画を読み倒した")),
        (13, TableItem::Text("ゲームをひたすらやった")),
        (14, TableItem::Text("悪化した病と闘った")),
        (15, TableItem::Text("入院している子供と約束をした")),
        (16, TableItem::Text("看護師と仲良くなった")),
        (
            22,
            TableItem::Text("現代の医術では治療できなかったので、異世界の魔法に賭けた"),
        ),
        (23, TableItem::Text("院内パーティを盛り上げた")),
        (24, TableItem::Text("飯がまずくて苦労した")),
        (25, TableItem::Text("飯がうまくて感動をした")),
        (26, TableItem::Text("やることがなくて暇だった")),
        (
            33,
            TableItem::Text("スーパードクターが現われて、自分の怪我を見事に治してくれた"),
        ),
        (
            34,
            TableItem::Text("とにかくテレビを見続けて、知識がついた"),
        ),
        (
            35,
            TableItem::Text("勉強をしてみたら、いつも以上にはかどった"),
        ),
        (
            36,
            TableItem::Text("たくさんの人たちが見舞いに来て、感動した"),
        ),
        (
            44,
            TableItem::Text("入院をしている爺さんから色々教えてもらった"),
        ),
        (45, TableItem::Text("リハビリに思ったより時間がかかった")),
        (
            46,
            TableItem::Text("次に喧嘩するときのイメージトレーニングをした"),
        ),
        (55, TableItem::Text("ヤンキー漫画に感動をした")),
        (56, TableItem::Text("院内で喧嘩をした")),
        (
            66,
            TableItem::Text("売店で売っているお菓子をコンプリートした"),
        ),
    ],
);

/// Ruby `TABLES`（定義順）。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("FTNT", &TABLE_FTNT),
    ("FT", &TABLE_FT),
    ("WT", &TABLE_WT),
    ("KKT", &TABLE_KKT),
    ("DBRT", &TABLE_DBRT),
    ("TKT", &TABLE_TKT),
    ("BUDT", &TABLE_BUDT),
    ("GUDT", &TABLE_GUDT),
    ("DAIT", &TABLE_DAIT),
    ("TKKT", &TABLE_TKKT),
    ("GSST", &TABLE_GSST),
    ("GYST", &TABLE_GYST),
    ("HPST", &TABLE_HPST),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "YankeeYogSothoth",
            "YankeeYogSothoth.toml",
            52,
        );
    }
}
