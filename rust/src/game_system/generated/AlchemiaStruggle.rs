//! P4で手書き移植した `lib/bcdice/game_system/AlchemiaStruggle.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`ALIAS` 解決 → `xAS` / `xASy` / `xUL` / 表）
//! - `#try_roll_alchemia` / `#try_roll_uldice` / `#pick_maximum` と出力整形メソッド群
//! - `TABLES`（`CATALYST_TABLES` + `ARTICLE_TABLES` + `DRAMA_SEQUENCE_TABLES`）と `ALIAS`
//!
//! # 表データ
//!
//! `TABLE_` 接頭辞の `static` 群は `.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。Ruby は `transform_keys(&:upcase)` でキーを大文字化するので、
//! Rust側は最初から大文字のキーで持つ。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{D66ParityTable, D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{str_helpers, table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `ROLL_REG`（`/^(\d+)AS(\d+)?$/i`）。
fn as_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)AS(\d+)?$").expect("valid regex"))
}

/// Ruby `try_roll_uldice` の `/^(\d+)UL$/`（`/i` 無し）。
///
/// `Base#dice_command` が `upcase` 済みの文字列を渡すので、`/i` の有無で差は出ない。
fn ul_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)UL$").expect("valid regex"))
}

/// Ruby `AlchemiaStruggle#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: c = ALIAS[command] || command
    let c = resolve_alias(command);

    if let Some(text) = try_roll_alchemia(c, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = try_roll_uldice(c, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(table_helpers::roll_table(c, TABLES, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `ALIAS[command] || command`。
fn resolve_alias(command: &str) -> &str {
    ALIAS
        .iter()
        .find(|(alias, _)| *alias == command)
        .map_or(command, |(_, full)| *full)
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `#try_roll_alchemia`。
fn try_roll_alchemia(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = as_pattern().captures(command) else {
        return Ok(None);
    };

    let roll_dice_count = to_i(&m[1]);
    let rolled = rng.roll_barabara(roll_dice_count, 6)?;

    match m.get(2) {
        // ロールのみ（ピックなし）
        None => Ok(Some(make_roll_text(&rolled))),
        // ロールして最大値をピック
        Some(pick) => {
            let pick_dice_count = to_i(pick.as_str());
            let picked = pick_maximum(&rolled, pick_dice_count);
            Ok(Some(make_roll_and_pick_text(
                &rolled,
                pick_dice_count,
                &picked,
            )))
        }
    }
}

/// Ruby `#try_roll_uldice`。
fn try_roll_uldice(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = ul_pattern().captures(command) else {
        return Ok(None);
    };

    let roll_dice_count = to_i(&m[1]);
    let mut dice_list = rng.roll_barabara(roll_dice_count, 6)?;
    dice_list.sort_unstable();

    let dice_list_text = dice_list
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");

    // Ruby: dice_list.group_by(&:itself).map { |k, v| "No.#{k}: #{v.size}個" }.join(", ")
    // `group_by` はキーの初出順を保つ。ソート済みなので出目の昇順になる。
    let mut groups: Vec<(i64, usize)> = Vec::new();
    for value in &dice_list {
        match groups.last_mut() {
            Some((k, count)) if k == value => *count += 1,
            _ => groups.push((*value, 1)),
        }
    }
    let result = groups
        .iter()
        .map(|(k, count)| format!("No.{k}: {count}個"))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(Some(format!(
        "({roll_dice_count}D6) ＞ [{dice_list_text}] ＞ {result}"
    )))
}

/// Ruby `#pick_maximum`。
///
/// Ruby の `Array#pop(n)` は末尾 n 個を「元の並びのまま」返す。
fn pick_maximum(dice_list: &[i64], pick_dice_count: i64) -> Vec<i64> {
    if i64::try_from(dice_list.len()).is_ok_and(|len| len <= pick_dice_count) {
        return dice_list.to_vec();
    }

    let mut sorted = dice_list.to_vec();
    sorted.sort_unstable();
    // ここに来るのは pick_dice_count < dice_list.len() のときだけなので、
    // usize への変換は必ず成功する。
    let keep = usize::try_from(pick_dice_count).unwrap_or(0);
    sorted.split_off(sorted.len() - keep)
}

/// Ruby `#make_roll_text`。
fn make_roll_text(rolled_dice_list: &[i64]) -> String {
    format!(
        "({}D6) ＞ {}",
        rolled_dice_list.len(),
        make_dice_text(rolled_dice_list)
    )
}

/// Ruby `#make_roll_and_pick_text`。
///
/// 実際にピックできた数と要求されたピック数は一致しないケースが（ルール上）あるため、
/// `pick_dice_count` はパラメータとして受ける必要がある。
fn make_roll_and_pick_text(
    rolled_dice_list: &[i64],
    pick_dice_count: i64,
    picked_dice_list: &[i64],
) -> String {
    format!(
        "({}D6|>{pick_dice_count}D6) ＞ {} >> {} ＞ {}",
        rolled_dice_list.len(),
        make_dice_text(rolled_dice_list),
        make_dice_text(picked_dice_list),
        picked_dice_list
            .iter()
            .fold(0i64, |a, b| a.wrapping_add(*b))
    )
}

/// Ruby `#make_dice_text`。
fn make_dice_text(dice_list: &[i64]) -> String {
    let mut sorted = dice_list.to_vec();
    sorted.sort_unstable();
    format!(
        "[{}]",
        sorted
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `TABLES["CELEMENT"]`（奇跡の触媒（エレメント））。
static TABLE_CELEMENT: Table = Table::from_dice(
    "奇跡の触媒（エレメント）",
    1,
    6,
    &["ワンド", "水晶玉", "カード", "ステッキ", "手鏡", "宝石"],
);

/// Ruby `TABLES["CALCHEMIA"]`（奇跡の触媒（アルケミア））。
static TABLE_CALCHEMIA: Table = Table::from_dice(
    "奇跡の触媒（アルケミア）",
    1,
    6,
    &[
        "指輪",
        "ブレスレット",
        "イヤリング",
        "ネックレス",
        "ブローチ",
        "ヘアピン",
    ],
);

/// Ruby `TABLES["CINFORMANT"]`（奇跡の触媒（インフォーマント））。
static TABLE_CINFORMANT: Table = Table::from_dice(
    "奇跡の触媒（インフォーマント）",
    1,
    6,
    &[
        "スマートフォン",
        "タブレット",
        "ノートパソコン",
        "無線機（トランシーバー）",
        "ウェアラブルデバイス",
        "携帯ゲーム機",
    ],
);

/// Ruby `TABLES["CINNOCENCE"]`（奇跡の触媒（イノセンス））。
static TABLE_CINNOCENCE: Table = Table::from_dice(
    "奇跡の触媒（イノセンス）",
    1,
    6,
    &["手袋", "笛", "靴", "鈴", "拡声器", "弦楽器"],
);

/// Ruby `TABLES["CACQUIRED"]`（奇跡の触媒（アクワイヤード））。
static TABLE_CACQUIRED: Table = Table::from_dice(
    "奇跡の触媒（アクワイヤード）",
    1,
    6,
    &["ボタン", "音声", "モーション", "脳波", "記録媒体", "ＡＩ"],
);

/// Ruby `TABLES["ARTICLES"]`（携行品（Ｓサイズ））。
static TABLE_ARTICLES: D66Table = D66Table::new(
    "携行品（Ｓサイズ）",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("マッチ")),
        (12, TableItem::Text("ペットボトル")),
        (13, TableItem::Text("試験管")),
        (14, TableItem::Text("団扇")),
        (15, TableItem::Text("植物")),
        (16, TableItem::Text("ハンカチ")),
        (22, TableItem::Text("化粧用具")),
        (23, TableItem::Text("ベルト")),
        (24, TableItem::Text("タバコ")),
        (25, TableItem::Text("チェーン")),
        (26, TableItem::Text("電池")),
        (33, TableItem::Text("お菓子")),
        (34, TableItem::Text("針金")),
        (35, TableItem::Text("コイン")),
        (36, TableItem::Text("ナイフ")),
        (44, TableItem::Text("カトラリー")),
        (45, TableItem::Text("砂")),
        (46, TableItem::Text("スプレー")),
        (55, TableItem::Text("石")),
        (56, TableItem::Text("文房具")),
        (66, TableItem::Text("ペンライト")),
    ],
);

/// Ruby `TABLES["ARTICLEM"]`（携行品（Ｍサイズ））。
static TABLE_ARTICLEM: D66ParityTable = D66ParityTable::new(
    "携行品（Ｍサイズ）",
    &["本", "傘", "金属板", "花火", "エアガン", "包帯"],
    &["工具", "ジャケット", "ロープ", "人形", "軽食", "ガラス瓶"],
);

/// Ruby `TABLES["ARTICLEL"]`（携行品（Ｌサイズ））。
static TABLE_ARTICLEL: D66ParityTable = D66ParityTable::new(
    "携行品（Ｌサイズ）",
    &["木刀", "釣り具", "自転車", "バット", "寝袋", "丸太"],
    &[
        "物干し竿",
        "鍋",
        "スケートボード",
        "シャベル（スコップ）",
        "タンク",
        "脚立",
    ],
);

/// Ruby `TABLES["PCINFORMATION"]`（ＰＣ情報獲得表）。
static TABLE_PCINFORMATION: D66ParityTable = D66ParityTable::new(
    "ＰＣ情報獲得表",
    &[
        "前の場面の直後 ―― 直前にやり取りをしていた場所。聞きたいことを突きつける頃合いかもしれない。",
        "自分の拠点 ―― 自分の心身を休められる場所。こちらのペースに引き込み、ゆさぶりをかける。",
        "相手の拠点 ―― 相手が生活の基点にしている場所。相手のペースに呑まれないよう、慎重にいこう。",
        "自学派の拠点 ―― 自分が学派の仲間と共に使用する場所。仲間に手は出させず、あくまでプレッシャーを与えるだけにしてもらう。",
        "カフェ、バー ―― 厳かな空気に包まれた大人の場所。ここで声を荒げるのは紳士的ではない。",
        "路地裏 ―― 建物と建物の間や、人通りの少ない裏通り。多少手荒な手段に出ても目立ちはしないだろう。",
    ],
    &[
        "廃墟 ―― 廃ビル、廃工場のような人が立ち入らない場所。おあつらえ向きの場所を用意してやった。",
        "公共交通機関 ―― バス、電車など。昼夜問わず多くの人が利用する乗り物。敢えて人目に付く場所で詰め寄り、動揺を誘う。",
        "雑木林 ―― 草木が揺れる音、虫や鳥の鳴き声だけが聞こえる。そこに邪魔する者はいない。",
        "夜の公園 ―― 寝静まった街の公園。街灯に照らされない場所なら目立つこともないだろう。",
        "駐車場 ―― 立体、平面、地下を問わず車を停める場所。人の出入りの激しさに対し、そこに留まる人は少ない。目撃者も多くはないだろう。",
        "高架下 ―― 線路、道路の橋の下。響く騒音が自分たちの存在を薄めてくれる。",
    ],
);

/// Ruby `TABLES["REASON"]`（理由表）。
static TABLE_REASON: Table = Table::from_dice(
    "理由表",
    1,
    6,
    &[
        "不信感 ―― 行動や言動になにか釈然としない部分を感じる。白黒はっきりさせよう。",
        "好奇心 ―― 相手のことを知りたいと掻き立てられる。知りたい気持ちを抑えられない。",
        "庇護感 ―― 知古の姿を重ねて守りたくなってしまう。信頼関係を君と築くため、踏み込んだところまで知っておきたい。",
        "嫌悪感 ―― 理由はないけど気に食わない。情報のアドバンテージを握ることで優位に立てるはずだ。",
        "偏愛 ―― 愛ゆえに知りたくなってしまう。君の思考、目的、感情のすべてを手に入れたい。",
        "直感 ―― 根拠はないが、なにか隠している気がする。一か八か、勝負に出よう。",
    ],
);

/// Ruby `TABLES["ASSOCIATE"]`（交流表）。
static TABLE_ASSOCIATE: D66ParityTable = D66ParityTable::new(
    "交流表",
    &[
        "前の場面の直後 ―― 直前にやり取りをした場所。ちょっと一息つくものいいだろう。",
        "自分の拠点 ―― 自分の心身を休められる場所。一緒にくつろぎながら話をしよう。",
        "相手の拠点 ―― 相手が生活の基点にしている場所。ちょっとお邪魔させてもらえないだろうか？",
        "相手学派の拠点 ―― 相手が学派の仲間と共に使用する場所。若干の居心地悪さはあるが、好感を持ってもらうためには我慢も必要。",
        "食事処 ―― ファミレス、居酒屋など。人でにぎわう食事処。気軽に飲み食いできる空間で、話も弾むはず。",
        "アミューズメント施設 ―― カラオケ、ボーリング、ゲームセンターなどの娯楽施設。遊べば人となりがわかる。手っ取り早くいこう。",
    ],
    &[
        "お祭り ―― 老若男女が参加するイベント。非日常的な空気を楽しむことで、気分転換もできるだろう。",
        "昼間の公園 ―― 散歩する人や子連れの家族で溢れる公園。僕らにもああやって生きる道があったのだろうか。",
        "思い出の場所 ―― 自分にとって思い入れのある大事な場所。この人になら胸の内を明かしてもいい気分になった。",
        "スポーツ観戦 ―― 野球、サッカー、バスケなど。プロアマ問わず観戦する。手に汗握る展開を共に見届けよう。",
        "屋上 ―― 街と人を見下ろす眺めのいい場所。この景色を君は喜ぶだろうか、怖がるだろうか。",
        "ショッピング ―― 大型商業施設やショッピングストリートに向かう。互いの興味があるものを知るいい機会だ。",
    ],
);

/// Ruby `TABLES["CONTACT"]`（接触のきっかけ表）。
static TABLE_CONTACT: Table = Table::from_dice(
    "接触のきっかけ表",
    1,
    6,
    &[
        "体勢を崩す ―― 転びそうになったところを支える、支えられる。",
        "付着物をとる ―― 髪や服についているゴミ、汚れをとってあげる。",
        "思わず手が出る ―― 言葉より先に、強めに手が出てしまう。",
        "物ごしに触れる ―― 物を渡す、拾う際に指先同士がぶつかる。",
        "友好のサイン ―― 肩を組む、握手をする、ハグをするなど。",
        "ケアをしてあげる ―― 髪をとかす、肩をもむ、頭を撫でる。相手を労ってする行為全般。",
    ],
);

/// Ruby `TABLES`（`CATALYST_TABLES.merge(ARTICLE_TABLES).merge(DRAMA_SEQUENCE_TABLES)`）。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("CELEMENT", &TABLE_CELEMENT),
    ("CALCHEMIA", &TABLE_CALCHEMIA),
    ("CINFORMANT", &TABLE_CINFORMANT),
    ("CINNOCENCE", &TABLE_CINNOCENCE),
    ("CACQUIRED", &TABLE_CACQUIRED),
    ("ARTICLES", &TABLE_ARTICLES),
    ("ARTICLEM", &TABLE_ARTICLEM),
    ("ARTICLEL", &TABLE_ARTICLEL),
    ("PCINFORMATION", &TABLE_PCINFORMATION),
    ("REASON", &TABLE_REASON),
    ("ASSOCIATE", &TABLE_ASSOCIATE),
    ("CONTACT", &TABLE_CONTACT),
];

/// Ruby `ALIAS`（短縮コマンド → `TABLES` のキー）。
///
/// Ruby は表のキーから機械的に作る:
/// - 触媒: `key[0, 4]`
/// - 携行品: `key[0, 2] + key[-1]`
/// - ドラマシーン: `key[0, 3]`
static ALIAS: &[(&str, &str)] = &[
    ("CELE", "CELEMENT"),
    ("CALC", "CALCHEMIA"),
    ("CINF", "CINFORMANT"),
    ("CINN", "CINNOCENCE"),
    ("CACQ", "CACQUIRED"),
    ("ARS", "ARTICLES"),
    ("ARM", "ARTICLEM"),
    ("ARL", "ARTICLEL"),
    ("PCI", "PCINFORMATION"),
    ("REA", "REASON"),
    ("ASS", "ASSOCIATE"),
    ("CON", "CONTACT"),
];

/// Ruby `BCDice::GameSystem::AlchemiaStruggle`（ID: `AlchemiaStruggle`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlchemiaStruggle;

impl GameSystem for AlchemiaStruggle {
    fn id(&self) -> &'static str {
        "AlchemiaStruggle"
    }

    fn name(&self) -> &'static str {
        "アルケミア・ストラグル"
    }

    fn sort_key(&self) -> &'static str {
        "あるけみあすとらくる"
    }

    fn help_message(&self) -> &'static str {
        r"■ ダイスロール（ xAS ）
  xDをロールします。
  例） 5AS

■ ダイスロール＆最大になるようにピック（ xASy ）
  xDをロールし、そこから最大になるようにy個をピックします。
  例） 4AS3

■ ウルダイスの獲得（ xUL ）
  xDのウルダイスを振り、出た出目の個数をNo.ごとにカウントします。
  例） 6UL

■ 表
  ・奇跡の触媒
    ・エレメント (CELE, CElement)
    ・アルケミア (CALC, CAlchemia)
    ・インフォーマント (CINF, CInformant)
    ・イノセンス (CINN, CInnocence)
    ・アクワイヤード (CACQ, CAcquired)
  ・携行品
    ・Ｓサイズ (ARS, ArticleS)
    ・Ｍサイズ (ARM, ArticleM)
    ・Ｌサイズ (ARL, ArticleL)
  ・ＰＣ情報獲得表 (PCI, PCInformation)
  ・理由表 (REA, Reason)
  ・交流表 (ASS, Associate)
  ・接触のきっかけ表 (CON, Contact)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+AS",
            r"\d+UL",
            "CELE",
            "CALC",
            "CINF",
            "CINN",
            "CACQ",
            "ARS",
            "ARM",
            "ARL",
            "PCI",
            "REA",
            "ASS",
            "CON",
            "CELEMENT",
            "CALCHEMIA",
            "CINFORMANT",
            "CINNOCENCE",
            "CACQUIRED",
            "ARTICLES",
            "ARTICLEM",
            "ARTICLEL",
            "PCINFORMATION",
            "REASON",
            "ASSOCIATE",
            "CONTACT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#initialize` の `@sort_add_dice = true`（加算ダイスのソート有）。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `#initialize` の `@sort_barabara_dice = true`（バラバラダイスでソート有）。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `#initialize` の `@round_type = RoundType::CEIL`（割り算をした時の端数切り上げ）。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `AlchemiaStruggle#eval_game_system_specific_command`。
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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "AlchemiaStruggle",
            "AlchemiaStruggle.toml",
            44,
        );
    }
}
