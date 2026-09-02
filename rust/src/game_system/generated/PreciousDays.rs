//! P4で手書き移植した `lib/bcdice/game_system/PreciousDays.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `PreciousDays#roll_pd`（判定 `nPD+m>=x`）
//! - `TABLES`（プライズ表3種 `PIT`/`PCT`/`PFT`・第一印象表 `FIT`・師匠の呼び名表 `TCT`）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない
//! （`PFT` の一部項目の先頭空白や、`貴族` の説明文が `動物` と同じになっている
//! 原典のデータバグもそのまま複製している）。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::{D66RangeTable, RangeInc, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `TABLES` の「プライズ表(物品)」の項目。
static PIT_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(11, 13),
        "旅行カバン ＞ 師匠との思い出のつまったカバン。",
    ),
    (
        RangeInc::new(14, 16),
        "リボンタイ ＞ 立ち寄った街で、師匠が選んでくれたお揃いのリボンタイ。",
    ),
    (
        RangeInc::new(21, 23),
        "キレイな石 ＞ 遺跡や川原などで見つけた、美しい天然石。",
    ),
    (
        RangeInc::new(24, 26),
        "手編みのマフラー ＞ 滞在先で手に入れた、感謝の気持ちのこもった編み物。",
    ),
    (
        RangeInc::new(31, 33),
        "魔術書の写本 ＞ たまたま手に入れた魔術書(写本)。",
    ),
    (
        RangeInc::new(34, 36),
        "師匠の落とし物 ＞ 師匠がうっかり落とした、小さなペンダントやアクセサリーなど。",
    ),
    (
        RangeInc::new(41, 43),
        "不思議な木の実 ＞ 森の散策中に発見した珍しい木の実。",
    ),
    (
        RangeInc::new(44, 46),
        "錆びた懐中時計 ＞ 古代遺跡などで見つかる懐中時計(故障している)。",
    ),
    (
        RangeInc::new(51, 53),
        "手書きの楽譜 ＞ 吟遊詩人の書いた、楽譜。",
    ),
    (
        RangeInc::new(54, 56),
        "特注の薬草セット ＞ ちょっとした傷や発熱に処方できる薬草のセット。",
    ),
    (
        RangeInc::new(61, 63),
        "紋章入りのバッジ ＞ そのシナリオで手に入れた、紋章のついたバッジ。",
    ),
    (
        RangeInc::new(64, 66),
        "使い古した手袋 ＞ 修行でボロボロになった、手放せない手袋。",
    ),
];

/// Ruby `TABLES` の「プライズ表(自身の変化)」の項目。
static PCT_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(11, 13),
        "背筋が伸びる ＞ 師匠との旅や修行を通じ、姿勢が正され、 威厳が増した。",
    ),
    (
        RangeInc::new(14, 16),
        "表情が大人びる ＞ 困難を乗り越えた経験から、表情に落ち着きと聡明さが加わった。",
    ),
    (
        RangeInc::new(21, 23),
        "ちいさな傷 ＞ いつの間にかついていた、小さな傷跡。",
    ),
    (
        RangeInc::new(24, 26),
        "集中力の向上 ＞ 周囲に動じない集中力が身についた。",
    ),
    (
        RangeInc::new(31, 33),
        "日焼けした肌 ＞ 健康的に日焼けした。",
    ),
    (
        RangeInc::new(34, 36),
        "魔力の流れ ＞ 体内のマナの流れを意識できるようになり、魔術の適性が向上した。",
    ),
    (
        RangeInc::new(41, 43),
        "強靭な足腰 ＞ 長い修行の旅を続けた結果、疲れにくい強靭な足腰になった。",
    ),
    (
        RangeInc::new(44, 46),
        "握力の増強 ＞ 物を握る力が強くなった。",
    ),
    (
        RangeInc::new(51, 53),
        "優美な指先 ＞ 繊細な詠唱や魔法薬の調合により、指先が優美になった。",
    ),
    (
        RangeInc::new(54, 56),
        "良い香り ＞ なにやら自分から良い香りが漂うになった。",
    ),
    (
        RangeInc::new(61, 63),
        "早起き体質 ＞ なにやら、自然と早起きできるようになった。",
    ),
    (
        RangeInc::new(64, 66),
        "共感能力の成長 ＞ 人々との交流を通じて、他者の感情を深く理解できるようになった。",
    ),
];

/// Ruby `TABLES` の「プライズ表(友人)」の項目。
static PFT_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(11, 13),
        "登場したNPC ＞ そのシナリオで交流したNPC。",
    ),
    (
        RangeInc::new(14, 16),
        "妖精 ＞ 森や泉などで出会った、無害な妖精。",
    ),
    (
        RangeInc::new(21, 23),
        "ライバル ＞ 魔術師大会などで出会った同世代の魔術師。",
    ),
    (
        RangeInc::new(24, 26),
        " 心優しい村人 ＞ 旅の途中で親切にしてくれた村の住民。",
    ),
    (
        RangeInc::new(31, 33),
        " 放浪の吟遊詩人 ＞ PCの旅の物語を歌にしてくれることを約束した詩人。",
    ),
    (
        RangeInc::new(34, 36),
        "旅の料理人 ＞ 旅の道中で食料を分け合い、共に野営をした旅の料理人。",
    ),
    (
        RangeInc::new(41, 43),
        "塔の受付嬢 ＞ 魔術師協会の塔で、 出会った受付の担当者。",
    ),
    (
        RangeInc::new(44, 46),
        "元・野盗 ＞ 過去の過ちを悔い改めた元野盗の改心者。",
    ),
    (
        RangeInc::new(51, 53),
        "動物 ＞ PCと仲よくなった小さな動物。",
    ),
    (
        RangeInc::new(54, 56),
        "貴族 ＞ PCと仲よくなった小さな動物。",
    ),
    (
        RangeInc::new(61, 63),
        "古代語の先生 ＞ 図書館などで古代語の知識を教えてくれた、年上の学者。",
    ),
    (
        RangeInc::new(64, 66),
        "年下の仲間 ＞ PCを慕ってくれるようになった年下の魔術師見習い。",
    ),
];

/// Ruby `TABLES` の「第一印象表」の項目。
static FIT_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(11, 12),
        "神秘的 ＞ 現実離れした、夢の中にいるような神秘的な雰囲気をまとっている。",
    ),
    (
        RangeInc::new(13, 14),
        "変わっている ＞ 一般的な魔術師のイメージとはかけ離れた、風変わりな人のような気がする。",
    ),
    (
        RangeInc::new(15, 16),
        "優しい ＞ その表情や声に穏やかさや優しさを感じた。",
    ),
    (
        RangeInc::new(21, 22),
        "恐ろしい ＞ 底知れぬ恐怖を感じた。見た目ではなく魔術師として底知れなさを感じる。",
    ),
    (
        RangeInc::new(23, 24),
        "完璧 ＞ 一切の隙が無く、完璧な魔術師に見えた。",
    ),
    (
        RangeInc::new(25, 26),
        "孤独 ＞ 目の前にいるというのに、まるで遠方にいるかのような距離を感じた。",
    ),
    (
        RangeInc::new(31, 32),
        "美しい ＞ その姿やたたずまいに、息をのむほどの美しさを感じた。",
    ),
    (
        RangeInc::new(33, 34),
        "疲れている ＞ まるで何度も繰り返し生を受けているかのような疲労を感じた。",
    ),
    (
        RangeInc::new(35, 36),
        "天才 ＞ ひと目で魔術師としての才能の違いが分かるのだと、初めて知った。",
    ),
    (
        RangeInc::new(41, 42),
        "厳格 ＞ 怖そうというのとは違う、厳しく真面目そうな人のように思えた。",
    ),
    (
        RangeInc::new(43, 44),
        "謎めいている ＞ ミステリアスな印象を与える、不思議な人だった。",
    ),
    (
        RangeInc::new(45, 46),
        "暖かい ＞ 不思議な温もりを感じる、見た人を安心させる人だった。",
    ),
    (
        RangeInc::new(51, 52),
        "憧れる ＞ ひと目見てこの人のようになりたい、なるんだという気持ちにになった。",
    ),
    (
        RangeInc::new(53, 54),
        "印象が無い ＞ 不思議と見た目も立ち居振る舞いも印象に残らない人だった。",
    ),
    (
        RangeInc::new(55, 56),
        "隠している ＞ 何かを隠している。そんな不思議な印象を感じる人だった。",
    ),
    (
        RangeInc::new(61, 62),
        "頼りない ＞ 一見すると、大丈夫かなという印象を与える人だった。",
    ),
    (
        RangeInc::new(63, 64),
        "年齢不詳 ＞ 若くも老人のようにも見える。見た目で年齢を推し量れない人だった。",
    ),
    (
        RangeInc::new(65, 66),
        "普通の人 ＞ 平凡というか、特別な印象を与えない、不思議な人だった。",
    ),
];

/// Ruby `TABLES["TCT"]`（師匠の呼び名表）の項目。
static TCT_ITEMS: &[&str] = &["師匠", "先生", "マスター", "親方", "ボス", "コーチ"];

static PIT_TABLE: D66RangeTable = D66RangeTable::new("プライズ表(物品)", PIT_ITEMS);
static PCT_TABLE: D66RangeTable = D66RangeTable::new("プライズ表(自身の変化)", PCT_ITEMS);
static PFT_TABLE: D66RangeTable = D66RangeTable::new("プライズ表(友人)", PFT_ITEMS);
static FIT_TABLE: D66RangeTable = D66RangeTable::new("第一印象表", FIT_ITEMS);
static TCT_TABLE: Table = Table::from_dice("師匠の呼び名表", 1, 6, TCT_ITEMS);

/// Ruby `TABLES` の値。`D66RangeTable` と `Table` が混在する。
///
/// どちらも `to_s` は `"表名(値) ＞ 内容"` なので、`roll_tables` からは文字列で揃えて扱う。
enum TableRef {
    /// Ruby `DiceTable::D66RangeTable`
    D66Range(&'static D66RangeTable),
    /// Ruby `DiceTable::Table`
    Plain(&'static Table),
}

impl TableRef {
    /// Ruby `table.roll(@randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            TableRef::D66Range(table) => Ok(table.roll(rng)?.to_string()),
            TableRef::Plain(table) => Ok(table.roll(rng)?.to_string()),
        }
    }
}

/// Ruby `TABLES`。`roll_tables` が引くコマンド名 → 表。
static TABLES: &[(&str, TableRef)] = &[
    ("PIT", TableRef::D66Range(&PIT_TABLE)),
    ("PCT", TableRef::D66Range(&PCT_TABLE)),
    ("PFT", TableRef::D66Range(&PFT_TABLE)),
    ("FIT", TableRef::D66Range(&FIT_TABLE)),
    ("TCT", TableRef::Plain(&TCT_TABLE)),
];

/// Ruby `BCDice::GameSystem::PreciousDays`（ID: `PreciousDays`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreciousDays;

impl GameSystem for PreciousDays {
    fn id(&self) -> &'static str {
        "PreciousDays"
    }

    fn name(&self) -> &'static str {
        "プレシャスデイズ"
    }

    fn sort_key(&self) -> &'static str {
        "ふれしやすていす"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (nPD+m>=x)
  nD6のダイスロールをして、その合計が x を超えていたら成功。
  出目6が2個以上あればクリティカル。出目が全て1ならファンブル。
  n: ダイス数(省略時 2)
  m: 修正値(省略可)
  x: 目標値(省略可)
  例) PD, PD+5>=6, 3PD>=10

■ 表
- 第一印象表 FIT
- プライズ表(物品) PIT
- プライズ表(自身の変化) PCT
- プライズ表(友人) PFT
- 師匠の呼び名表 TCT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*PD", "PIT", "PCT", "PFT", "FIT", "TCT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `PreciousDays#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_pd(command) || roll_tables(command, TABLES)
        if let Some(result) = roll_pd(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        Ok(None)
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll_text(rng)?))
}

/// Ruby `PreciousDays#roll_pd`。
fn roll_pd(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("PD", round_type: round_type)（Base の既定 :floor）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["PD"], RoundType::Floor)
            .enable_prefix_number()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge), None])
    });
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: times = cmd.prefix_number; times ||= 2
    let times = cmd
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    if times <= 0 {
        return Ok(None);
    }

    // Ruby: roll_barabara(times, 6).sort.reverse（降順）
    let mut dice_list = rng.roll_barabara(times, 6)?;
    dice_list.sort_unstable();
    dice_list.reverse();

    let dice_total: i64 = dice_list.iter().sum();
    let total = dice_total + cmd.modify_number.clone();

    let mut result = if dice_list.iter().filter(|&&d| d == 6).count() >= 2 {
        EvalResult::critical("クリティカル")
    } else if dice_list.iter().filter(|&&d| d == 1).count() as i64 == times {
        EvalResult::fumble("ファンブル")
    } else if cmd.cmp_op.is_none() {
        // Ruby: Result.new（text は nil なので下の compact で落ちる）
        EvalResult::new()
    } else if total >= cmd.target_number.clone().unwrap_or(crate::Int::from(0)) {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };

    // Ruby: "(#{command})" は素のコマンド文字列（`cmd.to_s` ではない）
    let modify_str = modifier(&cmd.modify_number);
    let mut sequence = vec![
        format!("({command})"),
        format!(
            "{dice_total}[{}]{modify_str}",
            dice_text::join_dice(&dice_list)
        ),
        total.to_string(),
    ];
    // Ruby: result.text が nil のケースは `.compact` で消える
    if !result.text.is_empty() {
        sequence.push(result.text.clone());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "PreciousDays",
            "PreciousDays.toml",
            10,
        );
    }
}
