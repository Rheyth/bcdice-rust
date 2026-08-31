//! P4で手書き移植した `lib/bcdice/game_system/SwordWorld2_5.rb`。
//!
//! 成長 / 超越判定 / 防御ファンブル表 / 絡み効果表は `SwordWorld2_0` に委譲し、
//! レーティング表は `sword_world/rating_parser.y` の `version: :v2_5` 差分
//! （`$~+N` / `ad N` / `sz N` / `#N` / `OH` / `DB`）を含めてここで扱う。
//! ドルイド物理魔法表（`Dru`）/ ビブリオマンサー応急行使判定（`Bib`）/
//! アビスカース表（`ABT` / `AABT`）は本システム固有。

use super::SwordWorld::{check_result_2d6, SystemText as SwordWorldText};
use super::SwordWorld2_0::{
    self, eval_non_rating_command, parse_rating_modifier, rating_value, roll_dice, take_number,
    Non2dRoll, SystemText as Sw20Text,
};
use crate::arithmetic::{self, Node, ParenMode};
use crate::command_parser::Parser;
use crate::common_command::lexer::{self, Cursor, Tok};
use crate::dice_table::{D66GridTable, D66ParityTable, RollableTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// `i18n/SwordWorld2_5/*.yml` の文言と表。`base` は `SwordWorld2_0` 側の文言。
pub(crate) struct SystemText {
    pub(crate) base: &'static Sw20Text,
    pub(crate) special_failure: &'static str,
    pub(crate) abyss_curse: &'static D66GridTable,
    pub(crate) abyss_curse_category: &'static D66ParityTable,
    pub(crate) abyss_curse_attr: &'static D66ParityTable,
    pub(crate) additional_abyss_curse: &'static D66GridTable,
}

static JA_ABYSS_CURSE_ITEMS: &[&[&str]] = &[
    &[
        "「自傷の」　装備時　この武具を装備中に装備者がクリティカルを発生させた時、装備者のHPが5点減少する。",
        "「嘆きの」　装備時　近くに敵がいたり、長い緊張状態が続くと涙が止まらなくなる。戦闘中なら「射程：術者（自身）」「射程：接触」以外の効果で対象を選べなくなる。",
        "「優しき」　装備時　敵に同情してしまう。敵対するキャラクターを対象にする場合、対象のHPが1点以上減少しているなら命中力判定、魔法行使判定に-2のペナルティ修正を受ける。",
        "「差別の」　装備時　特定の分類に対して与える物理ダメージ・魔法ダメージが2点減少する。分類は「分類決定表」で無作為に決定する。",
        "「脆弱な」　装備時　魔法ダメージを受けるたび、そのダメージが+1点される。",
        "「無謀な」　装備時　防護点が2点減少する（最低0）。",
    ],
    &[
        "「重い」　装備時　強化した武具の必要筋力が+2される。威力、防護点などは変化なし。",
        "「難しい」　装備時　いかなる威力表使用時でも、③④欄の数値が威力に関係なく「0」になる（自動失敗ではない）。",
        "「軟弱な」　装備時　精神抵抗力判定に-1のペナルティ修正を受ける。",
        "「病弱な」　装備時　生命抵抗力判定に-1のペナルティ修正を受ける。",
        "「過敏な」　装備時　特定の属性から受ける物理ダメージ、魔法ダメージが2点上昇する。属性は「属性決定表」で無作為に決定する。",
        "「陽気な」　装備時　精神抵抗判定に失敗するたび、笑いが止まらなくなる。次の手番終了時まで行動判定（『Ⅰ』123頁）に-1のペナルティ修正を受ける。この効果は累積する。",
    ],
    &[
        "「たどたどしい」　装備時　話をする時に言葉に詰まったり、言い間違えたりしやすくなる。魔法行使判定に-1のペナルティ修正を受ける。",
        "「代弁する」　装備時　自身の会話は、そのまま武具が魔法文明語の聞き取りづらい声で話す。装備中は魔法文明語以外の言語で会話は行えず、妖精魔法、魔動機術を行使できなくなる。",
        "「施しは受けない」　装備時　戦闘中、「抵抗：任意」の効果を受け入れた場合、次の手番開始時まで生命抵抗力、精神抵抗力に-2のペナルティ修正を受ける。",
        "「死に近い」　携行時　常に生死判定に「冒険者レベル」と同じ値のペナルティ修正を受ける。",
        "「おしゃれな」　携行時　その武具を常に華美に飾りたくなる。収入を得るたび、その1割以上をこの武具の装飾に費やさなければならない（効果などに変化はない）。",
        "「マナを吸う」　携行時　魔法や練技など、自身の意思でMPを消費する効果を使用する場合、すべてのMP消費が1点上昇する。",
    ],
    &[
        "「鈍重な」　携行時　移動力が半分（端数切り上げ）になる。",
        "「定まらない」　携行時　戦闘中の手番開始時に1dし、出目が「1」なら《ターゲッティング》とそれを前提とした戦闘特技を習得していないものとして扱う。",
        "「錯乱の」　携行時　戦闘中の手番開始時に1dし、出目が「1」なら近接攻撃を含む「射程：接触」の対象に効果を使用する際、対象は同じ位置（エリア、座標）の全てのキャラクター（敵味方含む）から無作為に選ばれる。",
        "「足絡みの」　携行時　戦闘中の手番開始時に1dし、出目が「1」ならその場で即座に転倒する。手番中には起き上がれない。",
        "「滑り落ちる」　携行時　戦闘中の手番開始時に1dし、出目が「1」なら手に装備または保持しているものをすべてその場に落とす（その手番の主動作で拾う事は可能）。",
        "「悪臭放つ」　携行時　強い悪臭を放つ。所持しているだけで他のキャラクターに不快感を与え、隠密判定に-2のペナルティ修正を受ける。さらに冒険者ランク（『Ⅱ』137頁）が1段階低いものとして扱われる。",
    ],
    &[
        "「醜悪な」　携行時　武具の見た目が悪く、魅力がない。売却する際、基本取引価格の4分の1の価格で売却する。さらに冒険者ランク（『Ⅱ』137頁）が1段階低いものとして扱われる。",
        "「唸る」　携行時　その武具から常に羽虫が飛び交うような音が響く。隠密判定、危険感知判定に-4のペナルティ修正を受ける。",
        "「ふやけた」　携行時　水を吸ったようにふやけた質感をしている。追加ダメージ-1（武器）、防護点-1（鎧、盾）。病気属性の効果に対する生命抵抗力、精神抵抗力判定に-4のペナルティ修正を受ける。",
        "「古傷の」　携行時　HPを回復する効果（休息による回復を含む）を受けた場合、その回復量が1点減少する。",
        "「まばゆい」　携行時　光などを弾いて強く輝く。自身は常に視界が悪い事による-1のペナルティ修正を受ける。",
        "「栄光なき」　携行時　行為判定で自動成功した際、自動成功とは扱わず、2dを振り直し、その後の出目に従う。この効果は1日に1回のみ発揮される。",
    ],
    &[
        "「正直者の」　携行時　嘘、方便がすぐばれるようになる。真偽判定の対象となる場合、-4のペナルティ修正を受ける。",
        "「乗り物酔いの」　携行時　揺れに弱くなる。自身の足以外の手段で10分以上移動した後、1時間、行動判定に-1のペナルティ修正を受ける。",
        "「碧を厭う」　携行時　自然の中では落ち着かなくなる。自然環境（『Ⅰ』108頁）では行動判定に-1のペナルティ修正を受ける。",
        "「我慢できない」　携行時　セッション中に1日の始まりを迎えるたび、趣味や嗜好品などに「冒険者レベル×10」ガメルを出費しなければならない。趣味や嗜好品が消費できない環境であれば、翌日の朝まで最大HP、最大MPが「冒険者レベル」点減少する。",
        "「つきまとう」　携行時　この武具が気がつけば身の回りにある。この武具以外での命中力判定、魔法行使判定（武器）、回避力判定（鎧、盾）に-4のペナルティ修正を受ける。",
        "「のろまな」　携行時　戦闘開始処理の「戦闘準備」をいっさい行えなくなる。",
    ],
];
static JA_ABYSS_CURSE_TABLE: D66GridTable =
    D66GridTable::new("アビスカース表", JA_ABYSS_CURSE_ITEMS);

static JA_ABYSS_CURSE_CATEGORY_ODD: &[&str] = &[
    "蛮族",
    "動物",
    "植物",
    "アンデッド",
    "魔法生物",
    "「蛮族」「動物」「植物」「アンデッド」「魔法生物」から任意",
];
static JA_ABYSS_CURSE_CATEGORY_EVEN: &[&str] = &[
    "魔動機",
    "幻獣",
    "妖精",
    "魔神",
    "人族",
    "「魔動機」「幻獣」「妖精」「魔神」「人族」から任意",
];
static JA_ABYSS_CURSE_CATEGORY_TABLE: D66ParityTable = D66ParityTable::new(
    "アビスカース分類決定表",
    JA_ABYSS_CURSE_CATEGORY_ODD,
    JA_ABYSS_CURSE_CATEGORY_EVEN,
);

static JA_ABYSS_CURSE_ATTR_ODD: &[&str] = &["土", "水・氷", "炎", "風", "雷", "純粋エネルギー"];
static JA_ABYSS_CURSE_ATTR_EVEN: &[&str] = &["断空", "衝撃", "毒", "病気", "呪い", "精神効果"];
static JA_ABYSS_CURSE_ATTR_TABLE: D66ParityTable = D66ParityTable::new(
    "アビスカース属性決定表",
    JA_ABYSS_CURSE_ATTR_ODD,
    JA_ABYSS_CURSE_ATTR_EVEN,
);

static JA_ADDITIONAL_ABYSS_CURSE_ITEMS: &[&[&str]] = &[
    &[
        "「衰退の」　携行時　戦闘準備のタイミングで「1d」を3回振り、対応する能力値が1点ずつ減少する。同じ出目の場合、その回数だけ能力値が1点減少する。「生命力」「精神力」が減少した場合、HP、MPの最大値が現在値ごと減少する。この効果でいずれかの能力値が0以下になった場合、生死判定を行わずに死亡する。減少した能力値は戦闘終了時に元に戻る。",
        "「怠惰の」　携行時　戦闘準備のタイミングで「1d」を1回振り、対応する能力値が3点減少する。「生命力」「精神力」が減少した場合、HP、MPの最大値が現在値ごと減少する。 この効果でいずれかの能力値が0以下になった場合、生死判定を行わずに死亡する。減少した能力値は戦闘終了時に元に戻る。",
        "「慌てる」　携行時　戦闘準備と1ラウンド目先攻時の行動判定に-2のペナルティ修正を受ける。",
        "「喉が詰まる」　携行時　戦争準備と1ラウンド目先攻時に会話と発声が行えなくなる。",
        "「無駄遣いの」　携行時　〈魔晶石〉を使用するとき、本来の消費に加えて1点余分に〈魔晶石〉のMPを消費しなければならなくなる。自身のMPと併用する場合でも同様。",
        "「空腹の」　携行時　すぐに空腹になる。通常の3倍の食事を必要とする。保存食であれば1日に3日分を消費しなければならず、食事に必要な費用は3倍となる。これらを消費できない場合、空腹によるペナルティ修正を受ける。",
    ],
    &[
        "「疲れが取れない」　携行時　長い時間の睡眠を必要とする。HP、MPを回復するために必要な睡眠時間が、倍の時間（6時間、12時間）必要になる。",
        "「薬物が効きにくい」　携行時　薬草・ポーション類は、1回の使用で本来の2倍の量を消費しなければ効果が現れない。 2倍の量を使用する際でも、所要時間に変化はない。",
        "「死体漁りの」　携行時　戦闘終了時、戦利品を取得できる機会がある場合、必ずそれを行う。戦利品決定の2dの出目が常に-1される。",
        "「従わない」　携行時　仲間キャラクターが使用する鼓胞の効果を受けない(必ず拒否する)。",
        "「一服を取る」　携行時　戦闘終了時、1時間経過するまでその場で休憩を求めるようになる。 休憩しなかった場合、1時間、行動判定に-1のペナルティ修正を受ける。",
        "「余裕を見せる」　携行時　HP現在値が最大値の80%を超えている場合、行動判定に-1のペナルティ修正を受ける。",
    ],
    &[
        "「息が荒い」　携行時　練技使用時、 効果時間が本来の3分の2になる（10秒単位、端数切り上げ）",
        "「音痴の」　携行時　バード技能を基準とするすべての行為判定に-2のペナルティ修正を受ける。",
        "「信頼しきれない」　携行時　手番開始時に「1d」し、出目が「1」なら、10秒 (1ラウンド)の間、自身の騎獣の行為判定に-2のペナルティ修正を受ける。",
        "「手から零れる」　携行時　賦術使用時「1d」し、出目が「1」なら、消費したカードと同じ量のカードを追加で消費しなければならない。消費できない場合、術は効果を発揮しない(本来の消費カードは失われる)。",
        "「喉が詰まる」　携行時時　その武具を常に華美に飾りたくなる。収入を得るたび、その1割以上をこの武具の装飾に費やさなければならない（効果などに変化はない）。",
        "「天地荒ぶる」　携行時　手番開始時に「1d-2」し、出目に等しいだけの「陣気」を失う(最低O)。",
    ],
    &[
        "「失敗を嘲る」　携行時　自身の周囲30m内で仲間キャラクターが行為判定を行い、自動失敗した場合、10分の間笑いが止まらなくなる。発声が必要な動作に-1のペナルティ修正を受け、隠密判定に-4のペナルティ修正を受ける。",
        "「学ばない」　携行時　自動失敗時に取得できる経験点が50点ではなく10点になる。",
        "「完璧主義な」　携行時　戦闘中の手番開始時に1dし、出目が「1」なら近接攻撃を含む「射程：接触」の対象に効果を使用する際、対象は同じ位置（エリア、座標）の全てのキャラクター（敵味方含む）から無作為に選ばれる。",
        "「華美を嫌う」　携行時　「観察判定パッケージ」の判定を行う際、通常の2倍の時間を必要とする。",
        "「昏睡の」　携行時　このキャラクターの睡眠は達成値20の「精神効果」として扱い、6時間経過しなければ解除されない(弱ではない)。",
        "「烙印を受ける」　携行時　このキャラクターが「毒」「病気」「呪い」「精神効果」属性の効果を受けた場合、その効果の達成値を+4する(解除しにくくなる)。",
    ],
    &[
        "「散漫な」　携行時　「異常感知判定」「危険感知判定」「罠回避判定」に-4のペナルティ修正を受ける。",
        "「命を削る」　装備時　ラウンド終了時、「ラウンド数」と同じ点数の確定ダメージをHPに受ける。",
        "「マナを削る」　装備時　ラウンド終了時、「ラウンド数」と同じ点数の確定ダメージをMPに受ける。",
        "「誇示する」　装備時　自身の手番でいずれかのキャラクターのHPを1点以上減少させた場合、10秒 (1ラウンド) の間、回避力判定と生命・精神抵抗力判定に-1のペナルティ修正を受ける。",
        "「踏ん張りがきかない」　装備時　1回のダメージ処理でHPが10点以上減少した場合、その場に転倒する。",
        "「身を晒す」　装備時　「形状:貫通」「形状:突破」の効果範囲に存在する場合、1dを振らず、必ず効果を受ける。",
    ],
    &[
        "「いたぶる」　装備時　複数の部位を持つ敵キャラクターを対象に取る場合、コア部位以外のHPが1以上あるなら、必ずコア部位を除外し、コア部位以外を対象に取らなければならない。",
        "「残心の」　装備時　いずれかのキャラクターのHPを0以下にした場合、10秒(1ラウンド)の間、回避力と生命・精神抵抗力に-2のペナルティ修正を受ける。",
        "「マナが漏れやすい」　装備時　いずれかの魔法の対象となったとき、本来の効果に加えて、MPが1点減少する。「抵抗:消滅」や「抵抗:任意」の効果に抵抗した場合、この効果は受けない。",
        "「退けたがる」　装備時　戦闘中、自身が手番で、自身に最も近い敵キャラクターのHPを「1」点以上減少できなかった場合、手番終了時に「2d」点の確定ダメージを受ける。",
        "「手加減する」　装備時　戦闘中、HPが最大値の半分以下の敵キャラクターに対する行動判定に-2のペナルティ修正を受ける。",
        "「調子が悪い」　装備時　行為判定、威力表の使用などで「2d」したときの出目が「7」の場合、すべて「6」として扱う。出目を変更する効果などを適用する場合、適用された最終的な出目が「7」だったなら、この効果を改めて適用する。",
    ],
];
/// 追加アビスカース表。`zh_hans.yml` に無いので簡体字版もこれへ fallback する。
pub(crate) static JA_ADDITIONAL_ABYSS_CURSE_TABLE: D66GridTable =
    D66GridTable::new("追加アビスカース表", JA_ADDITIONAL_ABYSS_CURSE_ITEMS);

pub(crate) static JA_TEXT: SystemText = SystemText {
    base: &SwordWorld2_0::JA_TEXT,
    special_failure: "特殊失敗",
    abyss_curse: &JA_ABYSS_CURSE_TABLE,
    abyss_curse_category: &JA_ABYSS_CURSE_CATEGORY_TABLE,
    abyss_curse_attr: &JA_ABYSS_CURSE_ATTR_TABLE,
    additional_abyss_curse: &JA_ADDITIONAL_ABYSS_CURSE_TABLE,
};

/// `$N` / `$+N` / `$~+N`。最初の出目にだけ適用する修正（同時指定不可）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstAdjust {
    /// `$N`: 出目を N に差し替える
    To(i64),
    /// `$+N` / `$-N`: 出目に N を加える
    Modify(i64),
    /// `$~+N`: 出目が10以下のときだけ N を加える（魔女の火用、v2.5 のみ）
    ModifySsp(i64),
}

/// Ruby `SwordWorld::RatingParsed`（v2.5 で使う全項目）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RatingCommand {
    rate: i64,
    critical: i64,
    modifier: i64,
    kept_modify: i64,
    first_to: i64,
    first_modify: i64,
    first_modify_ssp: i64,
    rateup: i64,
    add_damage: i64,
    non_2d_roll: Non2dRoll,
    set_zero_val: i64,
    modifier_after_half: Option<i64>,
    modifier_after_one_and_a_half: Option<i64>,
    modifier_after_double: Option<i64>,
}

impl RatingCommand {
    /// Ruby `RatingParsed#min_critical`。
    fn min_critical(&self) -> i64 {
        match self.non_2d_roll {
            Non2dRoll::SemiFixed(value) if value > 1 => value
                .saturating_add(self.kept_modify)
                .saturating_add(2)
                .clamp(3, 13),
            _ => 3,
        }
    }

    /// Ruby `RatingParsed#to_s`。
    fn label(&self) -> String {
        let mut output = format!("KeyNo.{}", self.rate);
        if self.critical < 13 {
            output.push_str(&format!("c[{}]", self.critical));
        }
        if self.first_modify != 0 {
            output.push_str(&format!(
                "m[{}]",
                format::modifier(&self.first_modify.into())
            ));
        }
        if self.first_modify_ssp != 0 {
            output.push_str(&format!(
                "m[~{}]",
                format::modifier(&self.first_modify_ssp.into())
            ));
        }
        if self.first_to != 0 {
            output.push_str(&format!("m[{}]", self.first_to));
        }
        if self.rateup != 0 {
            output.push_str(&format!("r[{}]", self.rateup));
        }
        if self.add_damage != 0 {
            output.push_str(&format!("ad[{}]", self.add_damage));
        }
        match self.non_2d_roll {
            Non2dRoll::GreatestFortune => output.push_str("gf"),
            Non2dRoll::SemiFixed(value) => output.push_str(&format!("sf[{value}]")),
            Non2dRoll::TmpFixed(value) => output.push_str(&format!("tf[{value}]")),
            Non2dRoll::None => {}
        }
        if self.set_zero_val != 0 {
            output.push_str(&format!("sz[{}]", self.set_zero_val));
        }
        if self.kept_modify != 0 {
            output.push_str(&format!(
                "a[{}]",
                format::modifier(&self.kept_modify.into())
            ));
        }
        output.push_str(&format::modifier(&self.modifier.into()));
        output
    }
}

/// Ruby `SwordWorld::RatingOptions`。パース途中の未評価オプション。
struct RatingOptions {
    modifier: Option<Node>,
    critical: Option<Node>,
    first_adjust: Option<FirstAdjust>,
    half: Option<Node>,
    one_and_a_half: Option<Node>,
    double: Option<Node>,
    rateup: Option<Node>,
    add_damage: Option<Node>,
    non_2d_roll: Non2dRoll,
    set_zero_val: Option<i64>,
    kept_modify: Option<Node>,
}

/// `H` / `OH` / `DB` の直後。`+` `-` 数字 `(` が続けばその項を「後修正」として読み、
/// 続かなければ修正 0 とする（Racc の shift 優先どおり）。
fn parse_optional_unary(cursor: &mut Cursor<'_>) -> Option<Node> {
    if matches!(cursor.peek(), Some(Tok::Plus) | Some(Tok::Minus)) || cursor.peek_starts_term() {
        arithmetic::parse_unary(cursor, ParenMode::Drop)
    } else {
        Some(Node::Number(0.into()))
    }
}

/// `DOLLAR NUMBER | DOLLAR PLUS NUMBER | DOLLAR MINUS NUMBER | DOLLAR TILDE PLUS NUMBER`。
fn parse_first_adjust(cursor: &mut Cursor<'_>) -> Option<FirstAdjust> {
    if cursor.accept(&Tok::Plus) {
        Some(FirstAdjust::Modify(take_number(cursor)?))
    } else if cursor.accept(&Tok::Minus) {
        Some(FirstAdjust::Modify(take_number(cursor)?.saturating_neg()))
    } else if cursor.accept_sym("~") {
        if !cursor.accept(&Tok::Plus) {
            return None;
        }
        Some(FirstAdjust::ModifySsp(take_number(cursor)?))
    } else {
        Some(FirstAdjust::To(take_number(cursor)?))
    }
}

/// Ruby `rating_parser.y` の `option` 規則（v2.5）。
///
/// `option` は左再帰で、修正値もオプション列の一要素として現れる（修正値の前後どちらにも
/// 他のオプションを置ける）。`sz` 以外の各オプションは一度しか指定できず、重複は構文エラー。
/// `gf` / `sf` / `tf` は互いに排他、`$N` / `$+N` / `$~+N` も互いに排他。
fn parse_rating_options(cursor: &mut Cursor<'_>) -> Option<RatingOptions> {
    let mut options = RatingOptions {
        modifier: None,
        critical: None,
        first_adjust: None,
        half: None,
        one_and_a_half: None,
        double: None,
        rateup: None,
        add_damage: None,
        non_2d_roll: Non2dRoll::None,
        set_zero_val: None,
        kept_modify: None,
    };

    loop {
        if matches!(cursor.peek(), Some(Tok::Plus) | Some(Tok::Minus)) {
            if options.modifier.is_some() {
                return None;
            }
            options.modifier = Some(parse_rating_modifier(cursor)?);
            continue;
        }
        if cursor.accept(&Tok::BracketL) {
            if options.critical.is_some() {
                return None;
            }
            options.critical = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            if !cursor.accept(&Tok::BracketR) {
                return None;
            }
            continue;
        }
        if cursor.accept(&Tok::At) {
            if options.critical.is_some() {
                return None;
            }
            options.critical = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            continue;
        }
        if cursor.accept_sym("$") {
            if options.first_adjust.is_some() {
                return None;
            }
            options.first_adjust = Some(parse_first_adjust(cursor)?);
            continue;
        }
        if cursor.accept_sym("H") {
            if options.half.is_some() {
                return None;
            }
            options.half = Some(parse_optional_unary(cursor)?);
            continue;
        }
        if cursor.accept_sym("O") {
            if !cursor.accept_sym("H") || options.one_and_a_half.is_some() {
                return None;
            }
            options.one_and_a_half = Some(parse_optional_unary(cursor)?);
            continue;
        }
        if cursor.accept_sym("D") {
            if !cursor.accept_sym("B") || options.double.is_some() {
                return None;
            }
            options.double = Some(parse_optional_unary(cursor)?);
            continue;
        }
        if cursor.accept_sym("R") {
            if options.rateup.is_some() {
                return None;
            }
            options.rateup = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            continue;
        }
        if cursor.accept_sym("A") {
            if !cursor.accept_sym("D") || options.add_damage.is_some() {
                return None;
            }
            options.add_damage = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            continue;
        }
        if cursor.accept_sym("G") {
            if !cursor.accept_sym("F") || options.non_2d_roll != Non2dRoll::None {
                return None;
            }
            options.non_2d_roll = Non2dRoll::GreatestFortune;
            continue;
        }
        if cursor.accept_sym("S") {
            if cursor.accept_sym("F") {
                if options.non_2d_roll != Non2dRoll::None {
                    return None;
                }
                options.non_2d_roll = Non2dRoll::SemiFixed(take_number(cursor)?.clamp(1, 6));
            } else if cursor.accept_sym("Z") {
                // `sz` だけは重複指定が構文エラーにならず、後の指定で上書きされる
                options.set_zero_val = Some(take_number(cursor)?);
            } else {
                return None;
            }
            continue;
        }
        if cursor.accept_sym("T") {
            if !cursor.accept_sym("F") || options.non_2d_roll != Non2dRoll::None {
                return None;
            }
            options.non_2d_roll = Non2dRoll::TmpFixed(take_number(cursor)?.clamp(1, 6));
            continue;
        }
        if cursor.accept_sym("#") {
            if options.kept_modify.is_some() {
                return None;
            }
            options.kept_modify = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            continue;
        }
        return Some(options);
    }
}

/// `Some(node)` を切り上げで評価する。評価できない式（ゼロ除算）は Ruby と同じくパース失敗扱い。
fn eval_option(node: &Option<Node>) -> Option<Option<i64>> {
    match node {
        Some(node) => Some(Some(crate::randomizer::sat_i64(
            &node.eval(RoundType::Ceil).ok()?,
        ))),
        None => Some(None),
    }
}

/// Ruby `RatingParser.new(version: :v2_5).parse`。
fn parse_rating(source: &str) -> Option<RatingCommand> {
    let lexed = lexer::lex(source);
    let mut cursor = Cursor::new(&lexed.tokens);

    let prefix_half = cursor.accept_sym("H");
    let prefix_one_and_a_half = !prefix_half && cursor.accept_sym("O");
    let prefix_double = !prefix_half && !prefix_one_and_a_half && cursor.accept_sym("D");
    if (prefix_one_and_a_half && !cursor.accept_sym("H"))
        || (prefix_double && !cursor.accept_sym("B"))
        || !cursor.accept_sym("K")
    {
        return None;
    }
    let rate = take_number(&mut cursor)?;
    let mut options = parse_rating_options(&mut cursor)?;
    if !cursor.at_eof() {
        return None;
    }

    // 先頭の H / OH / DB は末尾指定と同じ扱いになり、別種の倍率との併用は構文エラー
    if prefix_half {
        if options.one_and_a_half.is_some() || options.double.is_some() {
            return None;
        }
        options.half.get_or_insert(Node::Number(0.into()));
    } else if prefix_one_and_a_half {
        if options.half.is_some() || options.double.is_some() {
            return None;
        }
        options.one_and_a_half.get_or_insert(Node::Number(0.into()));
    } else if prefix_double {
        if options.half.is_some() || options.one_and_a_half.is_some() {
            return None;
        }
        options.double.get_or_insert(Node::Number(0.into()));
    }

    let modifier_after_half = eval_option(&options.half)?;
    let modifier_after_one_and_a_half = eval_option(&options.one_and_a_half)?;
    let modifier_after_double = eval_option(&options.double)?;
    let critical = match eval_option(&options.critical)? {
        Some(value) => value.clamp(0, 13),
        None if modifier_after_half.is_some() || modifier_after_one_and_a_half.is_some() => 13,
        None => 10,
    };
    let (first_to, first_modify, first_modify_ssp) = match options.first_adjust {
        Some(FirstAdjust::To(value)) => (value, 0, 0),
        Some(FirstAdjust::Modify(value)) => (0, value, 0),
        Some(FirstAdjust::ModifySsp(value)) => (0, 0, value),
        None => (0, 0, 0),
    };

    Some(RatingCommand {
        rate,
        critical,
        modifier: eval_option(&options.modifier)?.unwrap_or(0),
        kept_modify: eval_option(&options.kept_modify)?.unwrap_or(0),
        first_to,
        first_modify,
        first_modify_ssp,
        rateup: eval_option(&options.rateup)?.unwrap_or(0),
        add_damage: eval_option(&options.add_damage)?.unwrap_or(0),
        non_2d_roll: options.non_2d_roll,
        set_zero_val: options.set_zero_val.unwrap_or(0),
        modifier_after_half,
        modifier_after_one_and_a_half,
        modifier_after_double,
    })
}

/// Ruby `SwordWorld#rating`（`rollDice` は `SwordWorld2_0` 版）。
fn rating(
    text: &SystemText,
    source: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(command) = parse_rating(source) else {
        return Ok(None);
    };
    let base = text.base;
    if command.rate > 100 {
        let message = base.keynumber_exceeds.replace("%{keyMax}", "100");
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            message,
        ))));
    }

    let min_critical = command.min_critical();
    if command.critical < min_critical {
        let message = base
            .infinite_critical
            .replace("%{min_critical}", &min_critical.to_string());
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            message,
        ))));
    }

    let mut dice_texts = Vec::new();
    let mut dice_totals = Vec::new();
    let mut rate_results = Vec::new();
    let mut rating_total = 0_i64;
    let mut dice_only_total = 0_i64;
    let mut round = 0_i64;
    let mut first_to = command.first_to;
    let mut first_modify = command.first_modify;
    let mut first_modify_ssp = command.first_modify_ssp;

    loop {
        let (mut raw, dice_text) = roll_dice(command.non_2d_roll, round, rng)?;
        let mut dice = raw;

        if first_to != 0 {
            raw = first_to;
            dice = first_to;
            first_to = 0;
        } else if first_modify != 0 {
            dice = dice.saturating_add(first_modify);
            first_modify = 0;
        } else if first_modify_ssp != 0 {
            if raw <= 10 {
                dice = dice.saturating_add(first_modify_ssp);
            }
            first_modify_ssp = 0;
        }

        dice_texts.push(dice_text);
        // 出目がピンゾロの時にはそこで終了
        if raw <= 2 {
            dice_totals.push(raw.to_string());
            rate_results.push("**".to_string());
            round += 1;
            break;
        }

        if command.kept_modify != 0 && dice != 2 {
            dice = dice.saturating_add(command.kept_modify);
        }
        let dice = dice.clamp(2, 12);
        let current_key = command
            .rate
            .saturating_add(round.saturating_mul(command.rateup))
            .clamp(0, 100);
        let rate_value = if dice > command.set_zero_val {
            rating_value(current_key, dice)
        } else {
            0
        };
        rating_total = rating_total.saturating_add(rate_value);
        dice_only_total = dice_only_total.saturating_add(dice);
        dice_totals.push(dice.to_string());
        rate_results.push(if dice > 2 {
            rate_value.to_string()
        } else {
            "**".to_string()
        });
        round += 1;

        if dice < command.critical {
            break;
        }
    }

    let critical_count = (round - 1).max(0);
    let additional_damage = command.add_damage.saturating_mul(critical_count);
    let mut sequence = vec![format!(
        "2D:[{}]={}",
        dice_texts.join(" "),
        dice_totals.join(",")
    )];
    let mut result = EvalResult::new();

    if dice_only_total <= 2 {
        sequence.push(rate_results.join(","));
        sequence.push(base.common.fumble.to_string());
        result.fumble = true;
    } else {
        // rate回数が1回で、修正値がない時には途中式と最終結果が一致するので、途中式を省略する
        if rate_results.len() > 1 || command.modifier != 0 {
            let mut calculation = format!(
                "{}{}",
                rate_results.join(","),
                format::modifier(&command.modifier.into())
            );
            if additional_damage != 0 {
                calculation.push_str(&format!("+{}*{}", command.add_damage, critical_count));
            }
            if let Some(after) = command.modifier_after_half {
                calculation = format!("({calculation})/2{}", format::modifier(&after.into()));
            } else if let Some(after) = command.modifier_after_one_and_a_half {
                calculation = format!("({calculation})*1.5{}", format::modifier(&after.into()));
            } else if let Some(after) = command.modifier_after_double {
                calculation = format!("({calculation})*2{}", format::modifier(&after.into()));
            }
            sequence.push(calculation);
        } else if let Some(after) = command.modifier_after_half {
            sequence.push(format!(
                "{}/2{}",
                rate_results[0],
                format::modifier(&after.into())
            ));
        } else if let Some(after) = command.modifier_after_one_and_a_half {
            sequence.push(format!(
                "{}*1.5{}",
                rate_results[0],
                format::modifier(&after.into())
            ));
        } else if let Some(after) = command.modifier_after_double {
            sequence.push(format!(
                "{}*2{}",
                rate_results[0],
                format::modifier(&after.into())
            ));
        }

        if round > 1 {
            sequence.push(format!("{}{}", round - 1, base.round_suffix));
        }

        let mut total = rating_total
            .saturating_add(command.modifier)
            .saturating_add(additional_damage);
        if let Some(after) = command.modifier_after_half {
            total = (total + 1).div_euclid(2).saturating_add(after);
        } else if let Some(after) = command.modifier_after_one_and_a_half {
            // Ruby: (total * 1.5).ceil
            total = (total.saturating_mul(3) + 1)
                .div_euclid(2)
                .saturating_add(after);
        } else if let Some(after) = command.modifier_after_double {
            total = total.saturating_mul(2).saturating_add(after);
        }
        sequence.push(total.to_string());
        result.critical = round > 1;
    }

    result.text = format!("{} ＞ {}", command.label(), sequence.join(" ＞ "));
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `String#capitalize`（先頭だけ大文字、残りは小文字）。
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

/// `[` から `]` までの整数列を読む（`Dru[2,3,4]` / `Bib[5]` の中身）。
fn bracketed_numbers(command: &str, prefix: &str) -> Option<Vec<i64>> {
    let rest = command.strip_prefix(prefix)?;
    let end = rest.find(']')?;
    rest[..end]
        .split(',')
        .map(|digits| {
            (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                .then(|| digits.parse().ok())
                .flatten()
        })
        .collect()
}

fn dice_list_text(dice: &[i64]) -> String {
    dice.iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `SwordWorld2_5#druid_dice`。`Dru[2〜6の値,7〜9の値,10〜12の値]+修正`。
fn druid_dice(
    command: &str,
    power_list: &[i64],
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let parser = Parser::new(&[r"(?i)DRU\[\d+,\d+,\d+\]"], RoundType::Ceil);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let dice = rng.roll_barabara(2, 6)?;
    let dice_total = dice.iter().sum::<i64>();
    let power = match dice_total {
        ..=6 => power_list[0],
        7..=9 => power_list[1],
        _ => power_list[2],
    };
    let total: crate::Int = crate::Int::from(power) + &parsed.modify_number;
    let modifier = format::modifier(&parsed.modify_number);
    let sequence = [
        format!("({}{modifier})", capitalize(&parsed.command)),
        format!("2D[{}]={dice_total}", dice_list_text(&dice)),
        format!("{power}{modifier}"),
        total.to_string(),
    ];
    Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
}

/// Ruby `SwordWorld#result_2d6`。比較演算子や目標値が無くても出目 12 / 2 は判定する。
fn result_2d6(
    text: &SwordWorldText,
    total: crate::Int,
    dice_total: i64,
    cmp_op: Option<CmpOp>,
    target: Option<&crate::Int>,
) -> Option<EvalResult> {
    if dice_total >= 12 {
        return Some(EvalResult::critical(text.critical));
    }
    if dice_total <= 2 {
        return Some(EvalResult::fumble(text.fumble));
    }
    if cmp_op != Some(CmpOp::Ge) {
        return None;
    }
    let target = target?;
    Some(if &total >= target {
        EvalResult::success(text.success)
    } else {
        EvalResult::failure(text.failure)
    })
}

/// Ruby `SwordWorld2_5#biblio_emergency_dice`。`Bib[特殊失敗値]+修正>=目標値`。
fn biblio_emergency_dice(
    text: &SystemText,
    command: &str,
    failure_num: i64,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let parser = Parser::new(&[r"(?i)BIB\[\d+\]"], RoundType::Ceil);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let dice = rng.roll_barabara(2, 6)?;
    let dice_total = dice.iter().sum::<i64>();
    let total: crate::Int = crate::Int::from(dice_total) + &parsed.modify_number;

    let mut result = if dice_total == failure_num {
        EvalResult::fumble(text.special_failure)
    } else {
        result_2d6(
            &text.base.common,
            total.clone(),
            dice_total,
            parsed.cmp_op,
            parsed.target_number.as_ref(),
        )
        .unwrap_or_default()
    };

    let modifier = format::modifier(&parsed.modify_number);
    let mut sequence = vec![
        format!(
            "({}{modifier}{}{})",
            capitalize(&parsed.command),
            parsed.cmp_op.map(CmpOp::symbol_str).unwrap_or_default(),
            parsed
                .target_number
                .map(|target| target.to_string())
                .unwrap_or_default()
        ),
        format!("{dice_total}[{}]{modifier}", dice_list_text(&dice)),
        total.to_string(),
    ];
    if !result.text.is_empty() {
        sequence.push(result.text.clone());
    }
    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `SwordWorld2_5#get_abyss_curse_table`。
///
/// 「差別の」（14）は分類決定表、「過敏な」（25）は属性決定表を続けて振り、改行でつなぐ。
fn abyss_curse_table(text: &SystemText, rng: &mut Randomizer) -> Result<String, EvalError> {
    let table_result = text.abyss_curse.roll(rng)?;
    let additional = match table_result.value() {
        14 => Some(text.abyss_curse_category.roll(rng)?),
        25 => Some(text.abyss_curse_attr.roll(rng)?),
        _ => None,
    };
    let mut lines = vec![table_result.to_string()];
    if let Some(additional) = additional {
        lines.push(additional.to_string());
    }
    Ok(lines.join("\n"))
}

/// Ruby `SwordWorld2_5#eval_game_system_specific_command`。
///
/// `Dru` / `Bib` / `ABT` / `AABT` を先に判定し、それ以外は `SwordWorld2_0` の
/// 成長・超越判定・各表を経て v2.5 のレーティング表へ進む。
pub(crate) fn eval_specific_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(power_list) = bracketed_numbers(command, "DRU[").filter(|list| list.len() == 3) {
        return druid_dice(command, &power_list, rng);
    }
    if let Some(failure_num) = bracketed_numbers(command, "BIB[").filter(|list| list.len() == 1) {
        return biblio_emergency_dice(text, command, failure_num[0], rng);
    }
    match command {
        "ABT" => {
            return Ok(Some(SpecificCommandOutput::text(abyss_curse_table(
                text, rng,
            )?)))
        }
        "AABT" => {
            return Ok(Some(SpecificCommandOutput::text(
                text.additional_abyss_curse.roll(rng)?.to_string(),
            )))
        }
        _ => {}
    }
    if let Some(result) = eval_non_rating_command(text.base, command, rng) {
        return result;
    }
    rating(text, command, rng)
}

/// Ruby `BCDice::GameSystem::SwordWorld2_5`（ID: `SwordWorld2.5`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwordWorld2_5;

impl GameSystem for SwordWorld2_5 {
    fn id(&self) -> &'static str {
        "SwordWorld2.5"
    }
    fn name(&self) -> &'static str {
        "ソード・ワールド2.5"
    }
    fn sort_key(&self) -> &'static str {
        "そおとわあると2.5"
    }
    fn help_message(&self) -> &'static str {
        r#"自動的成功、成功、失敗、自動的失敗の自動判定を行います。

・レーティング表　(Kx)
　"Kキーナンバー+ボーナス"の形で記入します。
　ボーナスの部分に「K20+K30」のようにレーティングを取ることは出来ません。
　また、ボーナスは複数取ることが出来ます。
　レーティング表もダイスロールと同様に、他のプレイヤーに隠れてロールすることも可能です。
　例）K20　　　K10+5　　　k30　　　k10+10　　　Sk10-1　　　k10+5+2

・クリティカル値の設定
　クリティカル値は"[クリティカル値]"で指定します。
　指定しない場合はクリティカル値10とします。
　クリティカル処理が必要ないときは13などとしてください。(防御時などの対応)
　またタイプの軽減化のために末尾に「@クリティカル値」でも処理するようにしました。
　例）K20[10]　　　K10+5[9]　　　k30[10]　　　k10[9]+10　　　k10-5@9

・レーティング表の半減 (HKx, KxH+N)
　レーティング表の先頭または末尾に"H"をつけると、レーティング表を振って最終結果を半減させます。
　末尾につけた場合、直後に修正をつけることで、半減後の加減算を行うことができます。
　この際、複数の項による修正にはカッコで囲うことが必要です（カッコがないとパースに失敗します）
　クリティカル値を指定しない場合、クリティカルなしと扱われます。
　例）HK20　　K20h　　HK10-5@9　　K10-5@9H　　K20gfH　　K20+8H+2　　K20+8H+(1+1)

・レーティング表の1.5倍 (OHKx, KxOH+N)
　レーティング表の先頭または末尾に"OH"をつけると、レーティング表を振って最終結果を1.5倍します。
　末尾につけた場合、直後に修正をつけることで、1.5倍後の加減算を行うことができます。
　この際、複数の項による修正にはカッコで囲うことが必要です（カッコがないとパースに失敗します）
　クリティカル値を指定しない場合、クリティカルなしと扱われます。
　例）OHK20　　K20oh　　OHK10-5@9　　K20+8OH+2　　K20+8OH+(1+1)

・レーティング表の2倍 (DBKx, KxDB+N)
　レーティング表の先頭または末尾に"DB"をつけると、レーティング表を振って最終結果を2倍します。
　末尾につけた場合、直後に修正をつけることで、2倍後の加減算を行うことができます。
　この際、複数の項による修正にはカッコで囲うことが必要です（カッコがないとパースに失敗します）
　クリティカル値を指定しない場合、クリティカル値10と扱われます。
　例）DBK20　　K20db　　DBK10-5@9　　K20+8DB+2　　K20+8DB+(1+1)

・ダイス目の修正（運命変転やクリティカルレイ、魔女の火用）
　末尾に「$修正値」でダイス目に修正がかかります。
　$＋１と修正表記ならダイス目に＋修正、＄９のように固定値ならダイス目をその出目に差し替え。
　$~＋１とチルダを追加して記述することで、出目10以下の場合のみダイス目に＋修正（魔女の火用）
　クリティカルした場合でも固定値や修正値の適用は最初の一回だけです。
　例）K20$+1　　　K10+5$9　　　k10-5@9$+2　　　k10[9]+10$9　　　k20+6$~+1

・ダイス目の修正（必殺攻撃用）
　「＃修正値」でダイス目に修正がかかります。
　クリティカルした場合でも修正値の適用は継続されます。
　例）K20#1　　　k10-5@9#2

・首切り刀用レーティング上昇 r5
　例）K20r5　K30+24@8R5　K40+24@8$12r5

・肉喰む顎用クリティカルごと追加ダメージ ad2
　例）K20ad2　K30+24@8D2　K40+24@8$12ad2

・グレイテストフォーチュンは末尾に gf
　例）K20gf　K30+24@8GF　K40+24@8$12r5gf

・威力表を1d+sfで参照 クリティカル後も継続 sf4
　例）k10sf4　k0+5sf4@13　k70+26sf3@9

・威力表を1d+tfで参照 クリティカル後は2dで参照 tf3
　例）k10tf3　k0+5tf4@13　k70+26tf3@9

・アビスカース「難しい」用に KxSZy 表記でy以下の出目の威力表を0に書き換え。
　例) k10SZ4　k0+5@13sz6　k40+26@9sz3

・超越判定用に2d6ロールに 2D6@10 書式でクリティカル値付与が可能に。
　例）2D6@10　2D6@10+11>=30

・成長　(Gr)
　末尾に数字を付加することで、複数回の成長をまとめて行えます。
　例）Gr3

・防御ファンブル表　(FT)
　防御ファンブル表を出すことができます。

・絡み効果表　(TT)
　絡み効果表を出すことができます。

・ドルイドの物理魔法用表　(Dru[2-6の値,7-9の値,10-12の値])
　例）Dru[0,3,6]+10-3

・ビブリオマンサーの応急行使判定用　(Bib[特殊失敗値])
　例）Bib[6]+8　Bib[9]+10-1>=17

・アビスカース表　(ABT)
　アビスカース表を出すことができます。

・追加アビスカース表(AABT)
　追加アビスカース表を出すことができます。
"#
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "H?K",
            "OHK",
            "DBK",
            "Gr",
            r"2D6?@\d+",
            "FT",
            "TT",
            "Dru",
            "Bib",
            "ABT",
            "AABT",
        ]
    }
    crate::impl_prefixes_pattern!();

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(
            &SwordWorld2_0::JA_TEXT.common,
            total,
            crate::Int::from(dice_total),
            cmp_op,
            target,
        )
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_TEXT, command, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_toml_cases_pass() {
        SwordWorld2_0::assert_toml_cases("SwordWorld2.5", "SwordWorld2_5.toml", 144);
    }

    #[test]
    fn parses_v2_5_options_on_both_sides_of_modifier() {
        let command = parse_rating("K20@10+5$9R2").expect("parse");
        assert_eq!(command.critical, 10);
        assert_eq!(command.modifier, 5);
        assert_eq!(command.first_to, 9);
        assert_eq!(command.rateup, 2);
        assert_eq!(command.label(), "KeyNo.20c[10]m[9]r[2]+5");
    }

    #[test]
    fn rejects_duplicate_or_conflicting_options() {
        assert!(
            parse_rating("K20+5+3").is_some(),
            "one modifier with two terms"
        );
        assert!(parse_rating("K20+5@10+3").is_none(), "second modifier");
        assert!(parse_rating("K20$+1$9").is_none(), "two first-roll adjusts");
        assert!(parse_rating("K20$~-1").is_none(), "$~ takes only +N");
        assert!(
            parse_rating("K20gfsf3").is_none(),
            "gf and sf are exclusive"
        );
        assert!(parse_rating("K20#1#2").is_none(), "two kept modifies");
        assert!(parse_rating("K20ad1ad2").is_none(), "two add damages");
        assert!(parse_rating("HK20db").is_none(), "H prefix with DB suffix");
        assert_eq!(
            parse_rating("K20sz3sz5").map(|c| c.set_zero_val),
            Some(5),
            "sz overrides"
        );
    }

    #[test]
    fn capitalizes_like_ruby() {
        assert_eq!(capitalize("DRU[0,3,6]"), "Dru[0,3,6]");
        assert_eq!(capitalize("bib[5]"), "Bib[5]");
        assert_eq!(capitalize(""), "");
    }
}
