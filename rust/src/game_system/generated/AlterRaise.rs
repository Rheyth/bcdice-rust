//! P4で手書き移植した `lib/bcdice/game_system/AlterRaise.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`case command.upcase` の `when` 9本）
//! - `#get_emancipation_table` / `#get_AlterRaise_1d6_table_result` /
//!   `#get_AlterRaise_d66_table_result` と各表
//! - `Base#get_table_by_number`（該当なしの既定値 `"1"` も含む）
//!
//! # ROC指定の閾値が表ごとに違う
//!
//! 原典は「出目の指定（ROC）を振らずに使うか」の判定を表の種類ごとに変えている。
//! 共通化すると壊れるので、そのまま3つの関数に分けてある。
//!
//! - 解放判定表（2D6）: `roc > 1`（`EMA1` は**振る**）
//! - 1D6の表: `roc > 0`（`PER1` は振らない）
//! - D66の表: `roc > 10` なら2桁を分解、`roc > 0` なら十の位だけ指定して一の位を振る

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlterRaise;

impl GameSystem for AlterRaise {
    fn id(&self) -> &'static str {
        "AlterRaise"
    }

    fn name(&self) -> &'static str {
        "アルトレイズ"
    }

    fn sort_key(&self) -> &'static str {
        "あるとれいす"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AlterRaise#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

static HELP_MESSAGE: &str = r"◆解放判定：EMA[x]

[x]で達成値を指定してください。省略時はダイスロールします。


【各種表】
◆性格傾向表：PER[n]　　　　　　 ◆場所表：LOC[ab]
◆平穏・経験表：QUI[ab]　　　　　◆喜び・経験表：DEL[ab]
◆心の傷・経験表：TRA[ab]　　　　◆シーン演出表：SCE[n]
◆スタンス表：STA[n]　　　　　　 ◆感情表：EMO[ab]

[]内のコマンドを省略でダイスロール、指定でROC結果を表示します。
[n]は「1D6」、[ab]は「D66」の出目を指定してください。

【書式例】
PER3：性格傾向表の「3」をROC
LOC52：場所表の「52」をROC
QUI：平穏・経験表をダイスロール
";

static PREFIXES: &[&str] = &[
    "EMA", "PER", "LOC", "QUI", "DEL", "TRA", "SCE", "STA", "EMO",
];

/// 表の引き方。Ruby の `get_*_table` が呼び分けている3種類。
enum TableKind {
    /// Ruby `#get_emancipation_table`（2D6・7以上で追加テキスト）
    Emancipation,
    /// Ruby `#get_AlterRaise_1d6_table_result`
    D6(&'static str, &'static [(i64, &'static str)]),
    /// Ruby `#get_AlterRaise_d66_table_result`
    D66(&'static str, &'static [(i64, &'static str)]),
}

/// Ruby の `case command.upcase` の `when` 節。**並び順に意味がある**
/// （各 `when` は `/KEY(\d+)?$/i` で先頭は非アンカーなので、先に書いた方が勝つ）。
static COMMANDS: &[(&str, TableKind)] = &[
    ("EMA", TableKind::Emancipation),
    ("PER", TableKind::D6("性格傾向表", PERSONALITY_TABLE)),
    ("LOC", TableKind::D66("場所表", LOCATION_TABLE)),
    ("QUI", TableKind::D66("平穏・経験表", QUIET_TABLE)),
    ("DEL", TableKind::D66("喜び・経験表", DELIGHT_TABLE)),
    ("TRA", TableKind::D66("心の傷・経験表", TRAUMA_TABLE)),
    ("SCE", TableKind::D6("シーン演出表", SCENE_PRODUCTION_TABLE)),
    ("STA", TableKind::D6("スタンス表", STANCE_TABLE)),
    ("EMO", TableKind::D66("感情表", EMOTION_TABLE)),
];

/// [`COMMANDS`] と同じ並びの `/KEY(\d+)?$/i`。
fn command_patterns() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        COMMANDS
            .iter()
            .map(|(keyword, _)| Regex::new(&format!(r"(?i){keyword}(\d+)?$")).expect("valid regex"))
            .collect()
    })
}

/// Ruby `#eval_game_system_specific_command`。
///
/// Ruby は `command.upcase` してから `case` に掛けるが、呼び出し元の
/// `Base#dice_command` が既に大文字化しているので、ここでは受け取った文字列をそのまま使う。
fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    for (re, (_, kind)) in command_patterns().iter().zip(COMMANDS) {
        let Some(m) = re.captures(command) else {
            continue;
        };

        // Ruby: roc = (Regexp.last_match(1) || 0).to_i
        let roc = m.get(1).map_or(0, |v| to_i(v.as_str()));
        let text = match kind {
            TableKind::Emancipation => get_emancipation_table(roc, rng)?,
            TableKind::D6(name, table) => get_1d6_table_result(name, table, roc, rng)?,
            TableKind::D66(name, table) => get_d66_table_result(name, table, roc, rng)?,
        };
        return Ok(Some(SpecificCommandOutput::text(text)));
    }

    Ok(None)
}

/// Ruby `#get_emancipation_table`。
fn get_emancipation_table(roc: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let name = "解放判定表";

    let (dice, dice_text) = if roc > 1 {
        // Ruby: dice = roc; dice = 12 if dice > 12
        (roc.min(12), String::new())
    } else {
        let dice_list = rng.roll_barabara(2, 6)?;
        let dice = dice_list.iter().sum::<i64>();
        (dice, format!("({})", join(&dice_list)))
    };

    let mut table_text = get_table_by_number(dice, EMANCIPATION_TABLE).to_owned();
    if dice >= 7 {
        // Ruby側のコメント: 「''だと\nは文字列扱いに。」＝ここは実際の改行
        table_text.push_str("\n【達成値7以上】GM：攻撃ルーチン1つを開示（番号はペアPLが指定）　PL：戦闘開始時のアクセルレベル+1");
    }

    Ok(format!("{name} ＞ {dice}{dice_text}：{table_text}"))
}

/// Ruby `#get_AlterRaise_1d6_table_result`。
fn get_1d6_table_result(
    name: &str,
    table: &[(i64, &'static str)],
    roc: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice = if roc > 0 {
        // Ruby: dice = roc; dice = 6 if dice > 6
        roc.min(6)
    } else {
        rng.roll_once(6)?
    };

    let table_text = get_table_by_number(dice, table);
    Ok(format!("{name} ＞ {dice}：{table_text}"))
}

/// Ruby `#get_AlterRaise_d66_table_result`。
fn get_d66_table_result(
    name: &str,
    table: &[(i64, &'static str)],
    roc: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let (dice1, dice2) = if roc > 10 {
        // Ruby: diceText = roc.to_s; dice1 = diceText[0, 1].to_i; dice2 = diceText[1, 1].to_i
        let dice_text = roc.to_string();
        // dice1 に下限クリップは無い（原典どおり）
        (
            digit_at(&dice_text, 0).min(6),
            digit_at(&dice_text, 1).clamp(1, 6),
        )
    } else if roc > 0 {
        let dice1 = roc.min(6);
        let dice2 = rng.roll_once(6)?;
        (dice1, dice2)
    } else {
        let dice1 = rng.roll_once(6)?;
        let dice2 = rng.roll_once(6)?;
        (dice1, dice2)
    };

    let dice = dice1 * 10 + dice2;
    let dice_text = format!("{dice1},{dice2}");
    let table_text = get_table_by_number(dice, table);

    Ok(format!("{name} ＞ {dice_text}：{table_text}"))
}

/// Ruby `Base#get_table_by_number(index, table, default = "1")`。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    for (number, text) in table {
        if *number >= index {
            return text;
        }
    }
    "1"
}

/// Ruby `String#[i, 1].to_i`（範囲外は `""` → `0`）。
fn digit_at(text: &str, index: usize) -> i64 {
    text.chars()
        .nth(index)
        .and_then(|c| c.to_digit(10))
        .map_or(0, i64::from)
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `String#to_i`。桁あふれは Ruby だと Bignum になるので i64 の上端へ飽和させる。
fn to_i(text: &str) -> i64 {
    if text.is_empty() {
        0
    } else {
        text.parse().unwrap_or(i64::MAX)
    }
}

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// Ruby `#get_emancipation_table` の `table`。
static EMANCIPATION_TABLE: &[(i64, &str)] = &[
    (2, "激闘。今回の端末は想定をはるかに上回る脅威だった。幾本もの太刀筋と永遠のような時間の果てに、君たちは勝利した。深手を負ったが、ギリギリ致命傷ではない。"),
    (4, "辛勝。今回の端末は想定以上の大物だった。刃と牙のせめぎ合いの果て、君たちは辛くも勝利した。"),
    (6, "勝利。今回の端末は、おおむね想定される程度の個体であった。多少の傷は負ったが、君たちは問題なく勝利できた。"),
    (8, "快勝。今回の端末には、危うげも無く勝利できた。君とペアのコンビネーションの賜物だろう。かすり傷を負ったが、勲章のようなものだ。"),
    (10, "圧勝。今回の端末は、君たちの敵ではなかった。君とペアの剣撃は瞬く間に端末を寸断し、粒子の光に還元した。"),
    (12, "刹那。端末をその切っ先に捉えた刹那、君たちの前で粒子の光が舞う。それ以上何も起こることはなく、世界は色を取り戻した。"),
];

/// Ruby `#get_personality_table` の `table`。
static PERSONALITY_TABLE: &[(i64, &str)] = &[
    (1, "挑戦"),
    (2, "調和"),
    (3, "感性"),
    (4, "信念"),
    (5, "論理"),
    (6, "思慮"),
];

/// Ruby `#get_location_table` の `table`。
static LOCATION_TABLE: &[(i64, &str)] = &[
    (13, "教室"),
    (16, "部室"),
    (23, "商店街"),
    (26, "田舎"),
    (33, "都会"),
    (36, "駅"),
    (43, "バイト"),
    (46, "ステージ"),
    (53, "図書館"),
    (56, "病院"),
    (63, "自然"),
    (66, "家"),
];

/// Ruby `#get_quiet_table` の `table`。
static QUIET_TABLE: &[(i64, &str)] = &[
    (13, "友達"),
    (16, "幼馴染み"),
    (23, "両親"),
    (26, "兄弟"),
    (33, "親戚"),
    (36, "理解者"),
    (43, "友人"),
    (46, "仲間"),
    (53, "趣味"),
    (56, "練習"),
    (63, "一人"),
    (66, "お気に入り"),
];

/// Ruby `#get_delight_table` の `table`。
static DELIGHT_TABLE: &[(i64, &str)] = &[
    (13, "勝利"),
    (16, "優勝"),
    (23, "出会い"),
    (26, "理解"),
    (33, "幸運"),
    (36, "プレゼント"),
    (43, "成就"),
    (46, "成長"),
    (53, "創造"),
    (56, "好転"),
    (63, "証明"),
    (66, "生還"),
];

/// Ruby `#get_trauma_table` の `table`。
static TRAUMA_TABLE: &[(i64, &str)] = &[
    (13, "敗北"),
    (16, "仲違い"),
    (23, "失恋"),
    (26, "無理解"),
    (33, "無力"),
    (36, "孤独"),
    (43, "別離"),
    (46, "死別"),
    (53, "損壊"),
    (56, "喪失"),
    (63, "病"),
    (66, "事故"),
];

/// Ruby `#get_scene_production_table` の `table`。
static SCENE_PRODUCTION_TABLE: &[(i64, &str)] = &[
    (1, "相談。君は相手に相談したいことがあった。"),
    (2, "遊び。君は相手と遊びたかった。"),
    (3, "案内。君は自身のアリウス・パーソナルを案内したかった。"),
    (4, "勝負。君は相手と何らかの勝負をしたかった。"),
    (5, "お願い。君は相手にお願いしたいことがあった。"),
    (6, "扉を開く前に。アクセルダイブ・ゲートをくぐる前に、君は相手に話したいことがあった。（＊ダイブした後のシーンも演出すること）"),
];

/// Ruby `#get_stance_table` の `table`。
static STANCE_TABLE: &[(i64, &str)] = &[
    (1, "友人"),
    (2, "恋愛"),
    (3, "師事"),
    (4, "ライバル"),
    (5, "家族"),
    (6, "守護"),
];

/// Ruby `#get_emotion_table` の `table`。
static EMOTION_TABLE: &[(i64, &str)] = &[
    (13, "勇気"),
    (16, "怒り"),
    (23, "悲しみ"),
    (26, "喜び"),
    (33, "驚き"),
    (36, "恐れ"),
    (43, "安らぎ"),
    (46, "誠意"),
    (53, "庇護"),
    (56, "謝意"),
    (63, "信頼"),
    (66, "好意"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "AlterRaise",
            "AlterRaise.toml",
            23,
        );
    }
}
