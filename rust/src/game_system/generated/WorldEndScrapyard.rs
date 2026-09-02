//! P4で手書き移植した `lib/bcdice/game_system/WorldEndScrapyard.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `WorldEndScrapyard#resolute_action`（技能判定 `WES<=t`）
//! - `roll_roc_table`（変異表 `HEN` / 発掘表 `HAK`。`C` 付きは全項目を列挙）
//! - `roll_search_table`（探索表 `TANx+5`）
//! - `TABLES`（`KAN` / `MOK` / `BAS` / `TOJ`）
//!
//! 表データは `lib/bcdice/game_system/WorldEndScrapyard.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::WorldEndScrapyard`（ID: `WorldEndScrapyard`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldEndScrapyard;

impl GameSystem for WorldEndScrapyard {
    fn id(&self) -> &'static str {
        "WorldEndScrapyard"
    }

    fn name(&self) -> &'static str {
        "World End scrapyard"
    }

    fn sort_key(&self) -> &'static str {
        "わあるとえんとすくらつふやあと"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    /// Ruby `register_prefix("WES", "HEN", "HAK", "KAN", "MOK", "BAS", "TOJ", "TAN")`。
    fn prefixes(&self) -> &'static [&'static str] {
        &["WES", "HEN", "HAK", "KAN", "MOK", "BAS", "TOJ", "TAN"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `WorldEndScrapyard#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = resolute_action(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_roc_table(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_search_table(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `/^WES<=((\d+)([+]\d+)?)/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^WES<=((\d+)([+]\d+)?)").expect("valid regex"))
}

/// Ruby `WorldEndScrapyard#resolute_action`（技能判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1].empty? ? nil : Arithmetic.eval(m[1], @round_type)
    // `(\d+)([+]\d+)?` は必ず1文字以上なので `empty?` の枝には入らない。
    let expr = m.get(1).expect("group 1 always participates").as_str();
    let Some(target) = arithmetic::eval(expr, RoundType::Ceil)? else {
        return Ok(None);
    };

    let dice = rng.roll_once(100)?;

    let mut result = EvalResult::new();
    result.set_condition(dice <= crate::randomizer::sat_i64(&target));
    let diff = target.clone() - dice;

    let sequence = [
        format!("(WES<={target})"),
        dice.to_string(),
        if result.success { "成功" } else { "失敗" }.to_owned(),
        format!("差分値{diff}"),
    ];

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `/^(HEN|HAK)(C)?/`。
fn roc_table_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(HEN|HAK)(C)?").expect("valid regex"))
}

/// Ruby `WorldEndScrapyard#roll_roc_table`（変異表 / 発掘表）。
fn roll_roc_table(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = roc_table_pattern().captures(command) else {
        return Ok(None);
    };

    let cmd = m.get(1).expect("group 1 always participates").as_str();
    // Ruby: 該当しない場合は sequence が空・target_table が [] のままだが、
    // 正規表現が HEN / HAK のどちらかにしかマッチしないのでここは必ず埋まる。
    let (label, target_table): (&str, &[&str]) = match cmd {
        "HEN" => ("変異表", VARIATION_TABLE),
        _ => ("発掘表", EXCAVATION_TABLE),
    };

    let result_text = if m.get(2).is_none() {
        target_table[(rng.roll_once(10)? - 1) as usize].to_owned()
    } else {
        // Ruby: `[0..9].each { |i| roc_table_all.push(target_table[i]) }` は
        // Range 1個の配列を回すので `target_table[0..9]`（先頭10件の部分配列）が
        // 1要素として入り、`join("\n")` で平坦化されて全項目が並ぶ。
        target_table[0..10].join("\n")
    };

    let mut result = EvalResult::new();
    result.text = [label.to_owned(), result_text].join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `/^(TAN)([1-5])([+]5)?/`。
fn search_table_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(TAN)([1-5])([+]5)?").expect("valid regex"))
}

/// Ruby `WorldEndScrapyard#roll_search_table`（探索表）。
fn roll_search_table(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = search_table_pattern().captures(command) else {
        return Ok(None);
    };

    let level: i64 = m
        .get(2)
        .expect("group 2 always participates")
        .as_str()
        .parse()
        .expect("[1-5] is a valid number");
    // Ruby: m[3].nil? ? 0 : Arithmetic.eval(m[3], @round_type)
    let modifier = match m.get(3) {
        Some(mo) => arithmetic::eval(mo.as_str(), RoundType::Ceil)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };
    let idx = level * 5 + rng.roll_once(10)? + modifier;
    // idx は 6..=40 の範囲に収まる（level 1..=5, 1D10, modifier 0 or 5）。
    let result_text = SEARCH_TABLE[(idx - 6) as usize];

    let mut result = EvalResult::new();
    result.text = ["探索表", result_text].join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `HELP_MESSAGE`。
const HELP_MESSAGE: &str = r"■判定　WES<=t   t:成功率({能力値}+{悪運})
能力値は(筋力・敏捷・器用・知識・魅力)のいずれか。
成功・失敗・差分値を表示する。

例)WES<=20+10:能力値20,悪運10で判定し、その結果を表示する。
   WES<=30:   悪運30で判定し、その結果を表示する。

■各種表
・《変異表》           HEN HENC(任意選択,RoCのC)
・《発掘表》           HAK HAKC(  同上  )
・《PC/NPC関係性表》   KAN
・《シナリオの目的表》 MOK
・《シナリオの場所表》 BAS
・《登場NPC表》        TOJ
・《探索表》           TANx+b (x:レベル,1～5 b:修正+5)
";

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/WorldEndScrapyard.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `VARIATION_TABLE`（変異表）。
static VARIATION_TABLE: &[&str] = &[
    "0:[回復不能]状態になります。",
    "1:[異形]状態になります。",
    "2:[大型]状態になります([小柄]状態だった場合でも)",
    "3: 特に効果はありませんが『外見』を自由に変更させても構いません。",
    "4:《悪運》以外の《能力値》を【2つ】を『ランダム』で選んで、その《能力値》の数値を入れ替えます。",
    "5:《悪運》以外の《能力値》を【2つ】を『任意』で選んで、その《能力値》の数値を入れ替えます。",
    "6: 特に効果はありません。",
    "7:[小柄]状態になります([大型]状態だった場合でも)",
    "8:《悪運》以外の《能力値》【5つ】の数値を【10点単位】で再分配します。",
    "9:[異形]状態を打ち消します。",
];

/// Ruby `EXCAVATION_TABLE`（発掘表）。
static EXCAVATION_TABLE: &[&str] = &[
    "0: なにも発見できなかった。",
    "1:《悪運》を【レベル×3点】減少させる。",
    "2: なにも発見できなかった。",
    "3:《悪運》を【レベル×3点】上昇させる。",
    "4: 何かの『物資・材料?』を発見する(内容はGM/PCで相談する)",
    "5:《煙草》3本分、または《弾薬》3発分のどちらか【1つ】を得る。",
    "6: 壊れた『日用品?』を発見する(内容はGM/PCの相談で決める)",
    "7:《消費アイテム》をランダムで【1つ】を得る。",
    "8: 壊れた『家具類?』を発見する(内容はGM/PCの相談で決める)",
    "9:《装備アイテム》をランダムで【1つ】を得る。",
];

/// Ruby `SEARCH_TABLE`（探索表）。
static SEARCH_TABLE: &[&str] = &[
    "06:エネミー:レベル1『スカベンジャー』PC数×1体と遭遇する。",
    "07:なにもなし。",
    "08:エネミー:レベル1『下水ネズミの群れ』PC数×2体と遭遇する。",
    "09:なにもなし。",
    "10:《対抗判定:魅力/成功率35》を行い、勝利時に《発掘表》を1回振る。",
    "11:なにもなし。",
    "12:《行為判定:魅力》の成功時、次回《探索表》の結果に【+5】を加える。",
    "13:なにもなし。",
    "14:《悪運》を【PCのレベル×3点】上昇させる。",
    "15:行商人から【1レベル】までの《消費/装備アイテム》を交換・購入できる。",
    "16:エネミー:レベル2『粗暴な自警団』PC数×1体と遭遇する。",
    "17:なにもなし。",
    "18:エネミー:レベル2『バイオゾンビ』PC数×2体と遭遇する。",
    "19:なにもなし。",
    "20:《対抗判定:知識/成功率40》を行い、勝利時に《発掘表》を1回振る。",
    "21:なにもなし。",
    "22:《行為判定:知識》の成功時、次回《探索表》の結果に【+5】を加える。",
    "23:なにもなし。",
    "24:《悪運》を【PCのレベル×3点】上昇させる。",
    "25:行商人から【3レベル】までの《消費/装備アイテム》を交換・購入できる。",
    "26:エネミー:レベル3『ピットスパイダー』PC数×1体と遭遇する。",
    "27:なにもなし。",
    "28:エネミー:レベル3『バイオグール』PC数×2体と遭遇する。",
    "29:なにもなし。",
    "30:《対抗判定:器用/成功率45》を行い、勝利時に《発掘表》を1回振る。",
    "31:なにもなし。",
    "32:《行為判定:器用》の成功時、次回《探索表》の結果に【+5】を加える。",
    "33:なにもなし。",
    "34:《悪運》を【PCのレベル×3点】上昇させる。",
    "35:行商人から【5レベル】までの《消費/装備アイテム》を交換・購入できる。",
    "36:エネミー:レベル4『暴走オートマトン』PC数×1体と遭遇する。",
    "37:なにもなし。",
    "38:エネミー:レベル4『ネクロソルジャー』PC数×2体と遭遇する。",
    "39:なにもなし。",
    "40:《対抗判定:敏捷/成功率50》を行い、勝利時に《発掘表》を1回振る。",
];

/// Ruby `TABLES["KAN"]`（PC/NPC関係性表）。
static KAN_ITEMS: &[RangeTableItem] = &[
    (RangeInc::single(1), "物心付いた時からの長い幼馴染"),
    (RangeInc::new(2, 3), "旅立った頃からの始まりの仲間"),
    (RangeInc::new(4, 5), "なにかどうしようもない腐れ縁"),
    (RangeInc::new(6, 7), "既に互いの腕を信用できる同志"),
    (RangeInc::new(8, 9), "未だ相手を良く知らない同業者"),
    (RangeInc::single(10), "いま出会ったばかりの仕事相手"),
];
static KAN: RangeTable = RangeTable::from_dice("PC/NPC関係性表", 1, 10, KAN_ITEMS);

/// Ruby `TABLES["MOK"]`（シナリオの目的表）。
static MOK_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 6), "目的の物品を入手する"),
    (RangeInc::new(7, 12), "目的の情報を入手する"),
    (RangeInc::new(13, 18), "目的の人物と接触する"),
    (RangeInc::new(19, 24), "目的の人物を防衛する"),
    (RangeInc::new(25, 30), "目的の人物を救出する"),
    (RangeInc::new(31, 37), "場所を調査・探索する"),
    (RangeInc::new(38, 44), "場所を通過・突破する"),
    (RangeInc::new(45, 51), "場所を制圧・破壊する"),
    (RangeInc::new(52, 58), "場所から脱出する"),
    (RangeInc::new(59, 65), "場所を防衛する"),
    (RangeInc::new(66, 73), "敵目標を討伐する"),
    (RangeInc::new(74, 80), "敵目標を捕獲する"),
    (RangeInc::new(81, 87), "敵目標と和解する"),
    (RangeInc::new(88, 94), "敵目標を追跡する"),
    (RangeInc::new(95, 99), "敵目標から逃走する"),
    (RangeInc::single(100), "その他(GMの裁量で決めて良い)"),
];
static MOK: RangeTable = RangeTable::from_dice("シナリオの目的表", 1, 100, MOK_ITEMS);

/// Ruby `TABLES["BAS"]`（シナリオの場所表）。
static BAS_ITEMS: &[RangeTableItem] = &[
    (
        RangeInc::new(1, 7),
        "小規模コミュニティ〈シヴィラツィオ〉が主体。",
    ),
    (
        RangeInc::new(8, 9),
        "小規模コミュニティ〈ペルディトゥス〉小種族区分が混在している。",
    ),
    (
        RangeInc::single(10),
        "小規模コミュニティ〈ペルディトゥス：トロウル〉が主体。",
    ),
    (
        RangeInc::single(11),
        "小規模コミュニティ〈ペルディトゥス：オーガ〉が主体。",
    ),
    (
        RangeInc::single(12),
        "小規模コミュニティ〈ペルディトゥス：ゴブ〉が主体。",
    ),
    (
        RangeInc::single(13),
        "小規模コミュニティ〈ペルディトゥス：アドラー〉が主体。",
    ),
    (
        RangeInc::single(14),
        "小規模コミュニティ〈ペルディトゥス：ドヴェルク〉が主体。",
    ),
    (
        RangeInc::new(15, 19),
        "小規模コミュニティ〈フラグメンテ・マキナ〉が主体。",
    ),
    (
        RangeInc::new(20, 24),
        "小規模コミュニティ〈サバイバーズ〉が主体。",
    ),
    (
        RangeInc::new(25, 29),
        "小規模コミュニティ〈チルドレンズ〉が主体。",
    ),
    (
        RangeInc::new(30, 35),
        "大規模コミュニティ（すべての種族が混在している）",
    ),
    (
        RangeInc::new(36, 38),
        "廃墟〈シヴィラツィオ〉の住居跡など。",
    ),
    (
        RangeInc::new(39, 41),
        "廃墟〈ペルディトゥス〉の森林や洞窟。",
    ),
    (
        RangeInc::new(42, 44),
        "廃墟〈フラグメンテ・マキナ〉の機械施設。",
    ),
    (
        RangeInc::new(45, 47),
        "廃墟〈サバイバーズ〉のバラックやキャンプ跡。",
    ),
    (
        RangeInc::new(48, 50),
        "廃墟〈チルドレンズ〉のスラム跡や下水道など。",
    ),
    (
        RangeInc::single(51),
        "遺跡《テックレベル》２（神域の神殿など）",
    ),
    (
        RangeInc::new(52, 56),
        "遺跡《テックレベル》３（古代の城塞など）",
    ),
    (
        RangeInc::new(57, 64),
        "遺跡《テックレベル》５（廃ビルや研究所など）",
    ),
    (
        RangeInc::new(65, 69),
        "遺跡《テックレベル》７（壊れた軌道エレベータなど）",
    ),
    (
        RangeInc::single(70),
        "遺跡《テックレベル》８（墜された恒星間宇宙艦など）",
    ),
    (RangeInc::new(71, 80), "荒野"),
    (RangeInc::new(81, 86), "森林"),
    (RangeInc::new(87, 92), "沼沢"),
    (
        RangeInc::new(93, 99),
        "汚染領域（放射能や高密度魔力や生物汚染で汚染されている）",
    ),
    (RangeInc::single(100), "その他(GMの裁量で決めて良い)"),
];
static BAS: RangeTable = RangeTable::from_dice("シナリオの場所表", 1, 100, BAS_ITEMS);

/// Ruby `TABLES["TOJ"]`（登場NPC表）。
static TOJ_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "勇敢な小動物（子犬など）"),
    (RangeInc::new(3, 4), "温厚な乗輓用生物（ロバなど）"),
    (RangeInc::new(5, 6), "何もかも諦めた無気力な孤児"),
    (RangeInc::new(7, 8), "汚染で奇病を抱える賢い子供"),
    (RangeInc::new(9, 10), "トラウマで言葉を失った無口な子供"),
    (RangeInc::new(11, 12), "暴力しか知らない戦闘狂の野生少年"),
    (RangeInc::new(13, 14), "昔の歌を歌い続ける夢見がちな少女"),
    (RangeInc::new(15, 16), "仲間を守るために戦う熱血漢の少年"),
    (RangeInc::new(17, 18), "物資を溜め込む利己主義な銭ゲバ少女"),
    (RangeInc::new(19, 20), "盗みが得意で小生意気なスラムの少年"),
    (
        RangeInc::new(21, 22),
        "人を裏切ることに躊躇しない狡猾な少女",
    ),
    (
        RangeInc::new(23, 24),
        "機械いじりが得意な天才メカニック少年",
    ),
    (
        RangeInc::new(25, 26),
        "大人を信用せず一人で生き延びる孤独な少女",
    ),
    (
        RangeInc::new(27, 28),
        "早く大人にならざるを得なかった冷徹な少年兵",
    ),
    (
        RangeInc::new(29, 30),
        "過去の文明に憧れ、古い本を読み漁る知識少女",
    ),
    (RangeInc::new(31, 32), "復讐に燃える元軍人"),
    (RangeInc::new(33, 34), "薬物中毒の荒くれ戦士"),
    (RangeInc::new(35, 36), "冷酷だが仲間思いの傭兵"),
    (RangeInc::new(37, 38), "汚染地帯に潜む孤独な狩人"),
    (RangeInc::new(39, 40), "カルト教団の狂信な宣教師"),
    (RangeInc::new(41, 42), "医術を独学で身につけた流れ者"),
    (RangeInc::new(43, 44), "金儲けと裏切りが得意な詐欺師"),
    (RangeInc::new(45, 46), "植物栽培に没頭する農業主義者"),
    (RangeInc::new(47, 48), "家族を探し続ける高潔な放浪者"),
    (RangeInc::new(49, 50), "話題を売りさばく胡散臭い情報屋"),
    (RangeInc::new(51, 52), "コミュニティを守る厳格すぎる保安官"),
    (RangeInc::new(53, 54), "機械やロボットを操る天才エンジニア"),
    (RangeInc::new(55, 56), "奴隷売買を生業とする冷血な奴隷商人"),
    (RangeInc::new(57, 58), "倫理を失ったマッドサイエンティスト"),
    (RangeInc::new(59, 60), "ロックで人々を鼓舞する放浪の演奏者"),
    (
        RangeInc::new(61, 62),
        "酒とギャンブルに溺れる自暴自棄な中年",
    ),
    (
        RangeInc::new(63, 64),
        "汚染地帯を探索するトレジャーハンター",
    ),
    (
        RangeInc::new(65, 66),
        "美しい外見で人を操るサイコパスな美女",
    ),
    (
        RangeInc::new(67, 68),
        "過去の罪に苛まれる贖罪意識の強い戦士",
    ),
    (
        RangeInc::new(69, 70),
        "コミュニティ周辺を操る豪腕フィクサー",
    ),
    (RangeInc::new(71, 72), "戦争の記憶を引きずる老兵"),
    (RangeInc::new(73, 74), "何も信じず虚無的な哲学者"),
    (RangeInc::new(75, 76), "権力欲が強く独裁的な集落支配者"),
    (RangeInc::new(77, 78), "過去の文明の知識を伝える語り部"),
    (RangeInc::new(79, 80), "薬草や民間療法に詳しい薬師の老婆"),
    (RangeInc::new(81, 82), "隠し物資を山ほど持つ強欲な蓄財家"),
    (RangeInc::new(83, 84), "落ちぶれたかつてのカリスマ的預言者"),
    (RangeInc::new(85, 86), "最後の正義を信じる老いた保安官代理"),
    (RangeInc::new(87, 88), "コミュニティの厳しくも公正な指導者"),
    (RangeInc::new(89, 90), "疲れ果てたが諦めきれない不屈の老婆"),
    (
        RangeInc::new(91, 92),
        "汚染に体を蝕まれながらも戦う頑固親父",
    ),
    (
        RangeInc::new(93, 94),
        "最高の銃を作る頑固なガンスミスの職人",
    ),
    (
        RangeInc::new(95, 96),
        "交易路を熟知した熟練のキャラバン隊長",
    ),
    (
        RangeInc::new(97, 98),
        "情報データベースの無機質な対話型ＡＩ",
    ),
    (
        RangeInc::new(99, 100),
        "古い時空に存在していた高慢な魔法知性",
    ),
];
static TOJ: RangeTable = RangeTable::from_dice("登場NPC表", 1, 100, TOJ_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &RangeTable)] =
    &[("KAN", &KAN), ("MOK", &MOK), ("BAS", &BAS), ("TOJ", &TOJ)];

#[cfg(test)]
mod tests {

    use crate::dice_table::RangeTable;

    /// Ruby は `RangeTable.new` の時点で範囲の網羅性を検査する。
    /// Rust側は `static` を並べ替えられないので検査を分離してある（dice_table/mod.rs 参照）。
    #[test]
    fn range_tables_cover_their_dice_range() {
        for (name, table) in [
            ("KAN", &super::KAN as &RangeTable),
            ("MOK", &super::MOK),
            ("BAS", &super::BAS),
            ("TOJ", &super::TOJ),
        ] {
            table
                .validate()
                .unwrap_or_else(|e| panic!("{name} is not a valid RangeTable: {e}"));
        }
    }

    /// `test/data/WorldEndScrapyard.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WorldEndScrapyard",
            "WorldEndScrapyard.toml",
            19,
        );
    }
}
