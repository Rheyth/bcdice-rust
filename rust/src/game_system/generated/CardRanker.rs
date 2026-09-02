//! P4で手書き移植した `lib/bcdice/game_system/CardRanker.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`roll_tables` → `get_monster` → `RTT.roll_command`）
//! - `#result_2d6`（2以下でファンブル、12以上でスペシャル＋ランダムモンスター選択）
//! - `#get_monster`（`CMxy`）と `COLOR_TABLE`
//! - `RTT`（`SaiFicSkillTable`。`RM` 別名と専用書式つき）、`TABLES` 10種
//!
//! # `result_2d6` がダイスを振ること
//!
//! Ruby の `result_2d6` はスペシャル（2D6が12以上）のとき
//! `RTT.roll_command(@randomizer, "RM")` を呼び、**判定の中で追加のダイスを振る**。
//! Rust の [`GameSystem::result_2d6`] は `Randomizer` を受け取らないので、
//! 本システムは [`GameSystem::result_2d6_with_randomizer`] のほうを上書きする。
//! ここで振ったダイスは Ruby と同じく加算ロールの `rand_results` には入らない
//! （出力の `12[6,6]` は元の2D6のまま）。
//!
//! # 表データ
//!
//! `RTT_` / `TABLE_` 接頭辞の `static` 群は `.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::sai_fic_skill_table::{DEFAULT_RCT_FORMAT, DEFAULT_RTTN_FORMAT};
use crate::dice_table::{SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `#eval_game_system_specific_command`。
///
/// Ruby: `table_helpers::roll_table(command, TABLES, TABLES) || get_monster(command) || RTT.roll_command(randomizer, command)`
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = table_helpers::roll_table(command, TABLES, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = get_monster(command) {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(RTT
        .roll_command(rng, command)?
        .map(SpecificCommandOutput::text))
}

/// Ruby `#result_2d6`。
///
/// スペシャルのときだけ `RM`（ランダムモンスター選択）を振るので `rng` を取る。
fn check_result_2d6(
    total: crate::Int,
    dice_total: i64,
    cmp_op: CmpOp,
    target: Target,
    rng: &mut Randomizer,
) -> Result<Option<CheckOutcome>, EvalError> {
    // Ruby: return nil unless cmp_op == :>=
    if cmp_op != CmpOp::Ge {
        return Ok(None);
    }

    if dice_total <= 2 {
        return Ok(Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            "ファンブル",
        )))));
    }

    if dice_total >= 12 {
        // Ruby: Result.critical("スペシャル ＞ " + RTT.roll_command(@randomizer, "RM"))
        // `"RM"` は `RTT` の `rtt:` 別名なので `roll_command` は必ず結果を返す。
        let monster = RTT.roll_command(rng, "RM")?.unwrap_or_default();
        return Ok(Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            format!("スペシャル ＞ {monster}"),
        )))));
    }

    match target {
        // Ruby: elsif target == "?" -> Result.nothing
        Target::Question => Ok(Some(CheckOutcome::Nothing)),
        Target::Number(target) if total >= target => Ok(Some(CheckOutcome::Result(Box::new(
            EvalResult::success("成功"),
        )))),
        Target::Number(_) => Ok(Some(CheckOutcome::Result(Box::new(EvalResult::failure(
            "失敗",
        ))))),
    }
}

// ---------------------------------------------------------------------------
// モンスター選択
// ---------------------------------------------------------------------------

/// Ruby `COLOR_TABLE`。添字が [`RTT_CATEGORIES`] の分野に対応する。
static COLOR_TABLE: &[&str] = &["W", "U", "V", "G", "R", "B"];

/// Ruby `RTT` の `s_format:`（`Skill#to_s` の書式）。
const RTT_SKILL_FORMAT: &str = "%<category_name>sの%<row_dice>d：%<skill_name>s";

/// Ruby `/^CM(\w)(\d+)$/i`。
///
/// Ruby の `\w` はASCIIの `[a-zA-Z0-9_]` だけを指す。`regex` クレートで `(?i)` を
/// 付けると Unicode ケースフォールディングが効いて `K`(U+212A) 等まで拾うので、
/// `(?i)` を使わず `CM` の大小を明示して書く。
fn cm_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[Cc][Mm]([0-9A-Za-z_])(\d+)$").expect("valid regex"))
}

/// Ruby `#get_monster`。
///
/// 色記号が `COLOR_TABLE` に無い、または番号が 2〜12 の外なら `nil`。
fn get_monster(command: &str) -> Option<String> {
    let m = cm_pattern().captures(command)?;

    let cat = COLOR_TABLE.iter().position(|c| *c == &m[1])?;
    // `\d+` にマッチした部分なので、桁あふれは `between?(2, 12)` で必ず落ちる。
    let row_dice: i64 = m[2].parse().unwrap_or(i64::MAX);
    if !(2..=12).contains(&row_dice) {
        return None;
    }

    // Ruby: RTT.categories[cat].skills[row_dice - 2]
    // Ruby の `Category` は自分の1始まりの位置を `category_dice` として持つ。
    let skill = RTT_CATEGORIES[cat].skill_at(cat as i64 + 1, row_dice)?;
    Some(format!(
        "モンスター選択 ＞ {}",
        skill.format_with(RTT_SKILL_FORMAT)
    ))
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `RTT` の分野「白」の特技（2D6の2〜12）。
static RTT_SKILLS_WHITE: &[&str] = &[
    "白竜",
    "僧侶",
    "格闘家",
    "斧使い",
    "剣士",
    "槍士",
    "歩兵",
    "弓兵",
    "砲兵",
    "天使",
    "軍神",
];

/// Ruby `RTT` の分野「青」の特技（2D6の2〜12）。
static RTT_SKILLS_BLUE: &[&str] = &[
    "水竜",
    "魚",
    "魚人",
    "イカ",
    "蟹",
    "探偵",
    "海賊",
    "魔術師",
    "使い魔",
    "雲",
    "水精霊",
];

/// Ruby `RTT` の分野「緑」の特技（2D6の2〜12）。
static RTT_SKILLS_GREEN: &[&str] = &[
    "緑竜",
    "ワーム",
    "鳥人",
    "鳥",
    "獣",
    "獣人",
    "エルフ",
    "妖精",
    "昆虫",
    "植物",
    "森精霊",
];

/// Ruby `RTT` の分野「金」の特技（2D6の2〜12）。
static RTT_SKILLS_GOLD: &[&str] = &[
    "金竜",
    "宝石",
    "岩石",
    "鋼",
    "錬金術師",
    "魔法生物",
    "ドワーフ",
    "機械",
    "運命",
    "女神",
    "土精霊",
];

/// Ruby `RTT` の分野「赤」の特技（2D6の2〜12）。
static RTT_SKILLS_RED: &[&str] = &[
    "火竜",
    "竜人",
    "恐竜",
    "戦車",
    "蛮族",
    "小鬼",
    "大鬼",
    "巨人",
    "雷",
    "炎",
    "火精霊",
];

/// Ruby `RTT` の分野「黒」の特技（2D6の2〜12）。
static RTT_SKILLS_BLACK: &[&str] = &[
    "黒竜",
    "闇騎士",
    "怪物",
    "忍者",
    "妖怪",
    "蝙蝠",
    "吸血鬼",
    "不死者",
    "幽霊",
    "悪魔",
    "邪神",
];

/// Ruby `RTT` に渡す分野の一覧（1D6の1〜6）。
static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("白", RTT_SKILLS_WHITE),
    SaiFicCategory::new("青", RTT_SKILLS_BLUE),
    SaiFicCategory::new("緑", RTT_SKILLS_GREEN),
    SaiFicCategory::new("金", RTT_SKILLS_GOLD),
    SaiFicCategory::new("赤", RTT_SKILLS_RED),
    SaiFicCategory::new("黒", RTT_SKILLS_BLACK),
];

/// Ruby `TABLES["BFT"]`（バトルフィールド表 / 1D6）。
static TABLE_BFT: Table = Table::from_dice(
    "バトルフィールド表",
    1,
    6,
    &[
        "ハイ・アンティ/戦闘フェイズの終了時、勝者は通常習得できるモンスターカード一つに加えて、もう一つ敗者からモンスターカードを選んで手に入れることができる。//通常よりも多くのカードを賭けの対象にするルール。",
        "バーニング/ラウンドの終了時、すべてのキャラクターは【LP】現在値を3点失う。//マグマの近くや極寒の地など体力を削られるような過酷な環境で行われるルール。",
        "ノーマル/特に影響なし。//通常のルール。",
        "ハード/すべてのキャラクターの判定にプラス1の修正を加える。また、ラウンド終了時にすべてのキャラクターはモンスターカードを一つ選んで破壊状態（ルールブックP187参照）にしなければならない。//風の強い場所や水中など、カードを扱いにくい環境でのルール。",
        "スピード/モンスターカードのリスクが1高いものとして扱われる。また、判定に失敗した場合、速度から振り落とされて1D6のダメージを受ける。//バイクやローラーボードなどを使って行われる高速カードバトルルール。",
        "デスルール/戦闘フェイズでも死亡判定が発声する。また、戦闘不能になったキャラクターは即座に死亡判定を行う。ただし、攻撃を行った側がデスルールを使用しないことを選択すれば、死亡判定は発生しない。//モンスターによって実際のダメージを与える、死の危険性があるルール。",
    ],
);

/// Ruby `TABLES["CDT"]`（崩壊運命表 / 1D6）。
static TABLE_CDT: Table = Table::from_dice(
    "崩壊運命表",
    1,
    6,
    &[
        "レジェンドカードがあなたを崩壊する大地に呼び寄せた。暴虐な振る舞いをするダークランカーを倒すことをレジェンドカードは望んでいる。",
        "あなたはひょんなことから人を助けた。すると、あなたはいつの間にか救世主と呼ばれる存在になっていた、救世主であるあなたに人々は懇願する。ダークランカーを倒してくれと。",
        "あなたの住むところはダークランカーの力が及ばない楽園であった。しかし、楽園はダークランカー一味の襲撃にあい、あなただけが生き残ってしまった。楽園を出たあなたは戦いを決意する。",
        "世の中は変わった。だが、愛する人（もしくは愛する物や家族）は健在だ。あなたは愛する人を護るためにも、ダークランカーを倒すべく動き始めた。",
        "あなたはこの世界が好きだ。それはどんな理由でもよい。しかし、ダークランカーが持つダークカードはこの世界を壊す。ならば、倒してこの世界を守らねばならない。",
        "崩壊していく大地。泣き叫ぶ人々の声。あなたはこの状況を作ったのが、あなたの身内であると知る。ダークカードの手から身内を救うためにも、あなたはカードを手にとった。",
    ],
);

/// Ruby `TABLES["CST"]`（街中場所表 / 1D6）。
static TABLE_CST: Table = Table::from_dice(
    "街中場所表",
    1,
    6,
    &[
        "カードショップ/ソウルカードを遊ぶ者たちが集まる場所。プレイスペースもあれば、カードの販売もしている。",
        "ビル街/ビルが立ち並ぶ街。ビジネスマンが忙しなく動き、チェーン店が多く見られる。",
        "駅前/人が集まる駅前。電車から降りてくる人は多く、今日も人と人がすれ違う。",
        "食事処/レストランから大衆食堂、喫茶店やバーなど、食事は人の活力であり、カードランカーにも元気は必要だ。",
        "道路/長く広い道路。車と人が通過していく場所だが、時おりトラブルを抱えたカードランカー同士が戦っている。",
        "プール/都会にあるプール。都会の生活に疲れた人々が集まる場所。時おり、ソウルカードの戦いも見られる。",
    ],
);

/// Ruby `TABLES["DT"]`（運命表 / 1D6）。
static TABLE_DT: Table = Table::from_dice(
    "運命表",
    1,
    6,
    &[
        "あなたが欲しているカードはダークランカーが持っているかもしれないという情報を掴んだ。ダークランカーを倒し、アンティルールでカードを手に入れなければならない。",
        "ダークランカーとなった人物とあなたはカード仲間であったが、ある日見たその人物はダークカードの力にとり憑かれて豹変していた。あなたは仲間をカードによって救うため、戦いを決意した。",
        "ダークランカーはあなたの仲間や身内、大切なモノを傷つけた（壊した）。あなたの大切なものを傷つけたダークランカー、許しはしない。",
        "あなたの持つレジェンドカードが、ダークランカーもしくは他のレジェンドカードが出現することを察知した。レジェンドカードに導かれるまま、キミはダークランカー（レジェンドカード）を探し始めた。",
        "カードランカーの組織やソウルカードの安定を願う人からそのダークランカーを倒すように依頼を受けた、あなたはその仕事を受ける価値があると思った。そう思った理由は報酬でもいいし、あなたの流儀でもよい。",
        "ダークランカーとあなたは偶然にも出会ってしまった。ダークランカーは危険な存在だ。見てしまった以上、放っておくわけにはいかない。",
    ],
);

/// Ruby `TABLES["GDT"]`（学園運命表 / 1D6）。
static TABLE_GDT: Table = Table::from_dice(
    "学園運命表",
    1,
    6,
    &[
        "あなたが過ごしているクラスや寮、部活が潰されそうになった。その裏にはダークランカーの影響があるらしい。",
        "学園の偉い人から、カードランカーであるあなたに調査依頼が入った。どうやらダークランカーが学園に干渉しているとのこと。",
        "学園内のカードが奪われた。ダークランカーの影響だろう。大切にされていたカードを取り戻すために、あなたは立ち上がった。",
        "学内に邪悪な影響を受けたカードが入り込んでいた。おそらく、ダークランカーの仕業に違いない。",
        "ダークランカーによって被害を受けた生徒があなたに相談してきた。あなたはその生徒のためにもダークランカーの調査に乗り出した。",
        "ダークランカーの影響を受け、授業や部活動はまともにできなくなってしまった。あなたは元の学校生活を再開させるためにも、調査を始めた。",
    ],
);

/// Ruby `TABLES["OST"]`（郊外場所表 / 1D6）。
static TABLE_OST: Table = Table::from_dice(
    "郊外場所表",
    1,
    6,
    &[
        "カードショップ/ソウルカードを遊ぶ者たちが集まる場所。少し治安と客層が悪いが、賞金稼ぎも集まる。",
        "荒野/動植物も少なく、ピリピリとした雰囲気のある場所。",
        "遺跡/古代の遺跡。レジェンドカードやモンスターカードはこうした場所に発生したり、隠されていたりすることが多い。",
        "平原/どこまでも続く平原。動物も温厚であり、生い茂る草花が柔らかな印象を与える場所だ。",
        "山岳/険しい道が続く山。カードの精霊たちが生息していることもあるが、カード山賊団には気をつけねばならない。",
        "海川/海や川。山と同じくカードの精霊たちが住んでいる場所だ。安らげる場所でもあり、休憩している人がソウルカードをしている。",
    ],
);

/// Ruby `TABLES["GST"]`（学園場所表 / 1D6）。
static TABLE_GST: Table = Table::from_dice(
    "学園場所表",
    1,
    6,
    &[
        "購買/学生にとっては学園内で唯一買い物ができる場所。パンの他に、カードパックが売っている。",
        "グラウンド／体育館/運動するのに適した広い空間だが、同時にソウルカードをやるのにもうってつけの場所である。",
        "屋上/校舎の屋上は一部の生徒には人気のスポットだ。今日も強い風が彼らを迎えている。",
        "教室/日が昇っている間は、学生たちの声で賑やかな場所。夕暮れからは少し物哀しく、寂しい。",
        "校舎裏/学校の中でも珍しく人目につかない場所。不良たちがソウルカードをやっている姿が見られる。",
        "部活棟/部活をやる者のために用意された場所。しかし、サボってソウルカードをやっているところも。",
    ],
);

/// Ruby `TABLES["ST"]`（場所表 / 1D6）。
static TABLE_ST: Table = Table::from_dice(
    "場所表",
    1,
    6,
    &[
        "カード系/ショップや大会の会場など、ソウルカードに関係がある場所。カードランカーたちも集まってくる。",
        "自然/公園や山など、自然の息吹が感じられる場所。耳を澄ませばカードの声も聞こえるかもしれない。",
        "神秘/古代の施設や、神社・教会などの神秘的な場所。レジェンドカードが隠されているかもしれない。",
        "安息/自宅など、安らげる空間。そこはあなたが安らげる場所であり、思い出の地なのかもしれない。",
        "街中/人々が住む街中。何気なく落ちているカードの中には、価値があるものもあるかも。",
        "水辺/プールや海岸など、水が近くに存在する場所。ひとまず、ここでひと息つけそうだ。",
    ],
);

/// Ruby `TABLES["TDT"]`（大会運命表 / 1D6）。
static TABLE_TDT: Table = Table::from_dice(
    "大会運命表",
    1,
    6,
    &[
        "あなたは友人と共に大会に出場した。しかし、友人はダークランカーによって倒されてしまった。",
        "あなたは大会の商品を狙い、大会に出場した。だが、ダークランカーもそれを狙っているらしい。",
        "あなたは大会の運営者から、大会に関わっているダークランカーの撃破を依頼された。",
        "あなたはカードの導くままに、大会に関わってくるダークランカーの出現を察知した。",
        "あなたは大会の一選手として戦っていた。だが、謎の刺客によって襲われた。きっとダークランカーの仕業に違いない。",
        "あなたは大会に出場し、優勝候補と言われているカードランカーだ。だが、そんなキミをダークランカーは襲った。",
    ],
);

/// Ruby `TABLES["WT"]`（変調表 / 1D6）。
static TABLE_WT: Table = Table::from_dice(
    "変調表",
    1,
    6,
    &[
        "猛毒/ラウンド終了時に【LP】の現在値を3点失う。また【LP】の現在値を回復できない。",
        "炎上/ラウンド終了時に、モンスターカードを一つ選び破壊状態にしなければならない。既に破壊状態になっているものは選べない。",
        "妨害/攻撃判定にマイナス2の修正を受ける。",
        "捕縛/ブロック判定にマイナス2の修正を受ける。",
        "召喚制限/「タイプ：補助」のモンスターカードを使用できない。",
        "暗闇/「タイプ：支援」のモンスターカードを使用できない。",
    ],
);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[
    ("BFT", &TABLE_BFT),
    ("CDT", &TABLE_CDT),
    ("CST", &TABLE_CST),
    ("DT", &TABLE_DT),
    ("GDT", &TABLE_GDT),
    ("OST", &TABLE_OST),
    ("GST", &TABLE_GST),
    ("ST", &TABLE_ST),
    ("TDT", &TABLE_TDT),
    ("WT", &TABLE_WT),
];

/// Ruby `RTT`（`SaiFicSkillTable.new(..., rtt: "RM", rtt_format:, s_format:)`）。
///
/// `rct` / `rttn` の別名は指定されていないので、`RTT` / `RCT` / `RTT1`〜`RTT6` と
/// 別名の `RM` が引ける。`rct_format` と `rttn_format` は既定のまま。
static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES)
    .with_commands(Some("RM"), None, &[])
    .with_formats(SaiFicFormats {
        rtt: "ランダムモンスター選択(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
        rct: DEFAULT_RCT_FORMAT,
        rttn: DEFAULT_RTTN_FORMAT,
        skill: RTT_SKILL_FORMAT,
    });

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::CardRanker`（ID: `CardRanker`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardRanker;

impl GameSystem for CardRanker {
    fn id(&self) -> &'static str {
        "CardRanker"
    }

    fn name(&self) -> &'static str {
        "カードランカー"
    }

    fn sort_key(&self) -> &'static str {
        "かあとらんかあ"
    }

    fn help_message(&self) -> &'static str {
        r"ランダムでモンスターカードを選ぶ (RM) (RTTn n：色番号、省略可能)
ランダム分野表 RCT
特定のモンスターカードを選ぶ (CMxy　x：色、y：番号）
　白：W、青：U、緑：V、金：G、赤：R、黒：B
　例）CMW2→白の2：白竜　CMG12→金の12：土精霊
場所表 (ST)
街中場所表 (CST)
郊外場所表 (OST)
学園場所表 (GST)
運命表 (DT)
大会運命表 (TDT)
学園運命表 (GDT)
崩壊運命表 (CDT)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTT[1-6]?",
            "RCT",
            "RM",
            "CM",
            "BFT",
            "CDT",
            "CST",
            "DT",
            "GDT",
            "OST",
            "GST",
            "ST",
            "TDT",
            "WT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#initialize`: `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `#initialize`: `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `CardRanker#result_2d6`。
    fn result_2d6_with_randomizer(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
        rng: &mut Randomizer,
    ) -> Result<Option<CheckOutcome>, EvalError> {
        check_result_2d6(total, dice_total, cmp_op, target, rng)
    }

    /// Ruby `CardRanker#eval_game_system_specific_command`。
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
            .join("test/data/CardRanker.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/CardRanker.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/CardRanker.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("CardRanker.toml must parse");
        assert_eq!(
            data.tests.len(),
            44,
            "case count in test/data/CardRanker.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "CardRanker",
                "unexpected game system in CardRanker.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("CardRanker"), &tc.input, &mut src) {
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
                    "FAIL CardRanker:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} CardRanker cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// TOMLに無い経路の固定。
    ///
    /// - `RTT1`〜`RTT6`（分野指定のランダム特技表。書式は既定のまま）
    /// - `result_2d6` は `>=` 以外では `nil`（＝ `Base#result_ndx` に委ねる）
    /// - `CM` の番号が範囲外・色記号が未知なら `nil`
    #[test]
    fn rttn_and_non_ge_paths() {
        let mut src = SeededRandomizer::new(vec![(3, 6), (4, 6)]);
        let result = eval_command(&GameSystemId::new("CardRanker"), "RTT4", &mut src)
            .expect("RTT4 must not error")
            .expect("RTT4 must produce output");
        assert_eq!(result.text, "金分野ランダム特技表(7) ＞ 金の7：魔法生物");

        // `<=` は Ruby の `result_2d6` が nil を返すので、通常の成功/失敗判定になる。
        let mut src = SeededRandomizer::new(vec![(6, 6), (6, 6)]);
        let result = eval_command(&GameSystemId::new("CardRanker"), "2D6<=7", &mut src)
            .expect("2D6<=7 must not error")
            .expect("2D6<=7 must produce output");
        assert_eq!(result.text, "(2D6<=7) ＞ 12[6,6] ＞ 12 ＞ 失敗");
        assert!(!result.critical, "critical must not be set for <=");

        for command in ["CMW13", "CMW0", "CMZ5"] {
            let mut src = SeededRandomizer::new(vec![]);
            assert!(
                eval_command(&GameSystemId::new("CardRanker"), command, &mut src)
                    .expect("must not error")
                    .is_none(),
                "{command} must be nil"
            );
        }
    }
}
