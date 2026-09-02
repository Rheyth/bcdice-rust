//! P4で手書き移植した `lib/bcdice/game_system/TrinitySeven.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TrinitySeven#eval_game_system_specific_command` → `roll_hit` / `roll_damage` / `roll_name`
//! - `TrinitySeven#result_1d100`
//! - `NAME1` / `NAME2`（名字表）

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::TrinitySeven`（ID: `TrinitySeven`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrinitySeven;

impl GameSystem for TrinitySeven {
    fn id(&self) -> &'static str {
        "TrinitySeven"
    }

    fn name(&self) -> &'static str {
        "トリニティセブンRPG"
    }

    fn sort_key(&self) -> &'static str {
        "とりにていせふんRPG"
    }

    fn help_message(&self) -> &'static str {
        r#"クリティカルが変動した命中及び、7の出目がある場合のダメージ計算が行なえます。
なお、通常の判定としても利用できます。

・発動/命中　［TR(±c*)<=(x)±(y*) 又は TR<=(x) など］*は必須ではない項目です。
"TR(クリティカルの修正値*)<=(発動/命中)±(発動/命中の修正値*)"
加算減算のみ修正値も付けられます。 ［修正値］は必須ではありません。
例）TR<=50 TR<=60+20 TR7<=40 TR-7<=80 TR+10<=80+20

・ダメージ計算　［(x)DM(c*)±(y*) 又は (x)DM(c*) 又は (x)DM±(y*)］*は必須ではない項目です。
"(ダイス数)DM(7の出目の数*)+(修正*)"
加算減算のみ修正値も付けられます。 ［7の出目の数］および［修正値］は必須ではありません。
例）6DM2+1 5DM2 4DM 3DM+3
後から7の出目に変更する場合はC(7*6＋5)のように入力して計算してください。

・名前表　[TRNAME]
名字と名前を出します。PCや突然現れたNPCの名付けにどうぞ。

"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DM", "TR", "TRNAME"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `TrinitySeven#result_1d100`。
    fn result_1d100(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        if dice_total >= 96 {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "ファンブル",
            ))))
        } else if dice_total <= 7 {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "クリティカル",
            ))))
        } else {
            None
        }
    }

    /// Ruby `TrinitySeven#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `TrinitySeven#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_hit(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(text) = roll_damage(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_name(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(None)
}

/// Ruby `String#to_i`（先頭の符号付き数字。無ければ 0）。
fn ruby_to_i(text: &str) -> i64 {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let neg = if bytes.first() == Some(&b'-') {
        i = 1;
        true
    } else if bytes.first() == Some(&b'+') {
        i = 1;
        false
    } else {
        false
    };
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if start == i {
        return 0;
    }
    let n: i64 = text[start..i]
        .parse()
        .unwrap_or(if neg { i64::MIN } else { i64::MAX });
    if neg {
        n.saturating_neg()
    } else {
        n
    }
}

/// Ruby `TrinitySeven#roll_hit`。
fn roll_hit(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&[r"TR\d*"], RoundType::Floor).restrict_cmp_op_to(&[Some(CmpOp::Le)]);
    let Some(mut cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let modify: I =
        I::from(ruby_to_i(cmd.command.get(2..).unwrap_or(""))) + cmd.modify_number.clone();
    let critical = 7 + modify.clone();
    let Some(target) = cmd.target_number.clone() else {
        return Ok(None);
    };

    let total = rng.roll_once(100)?;
    let mut result = get_hit_roll_result(
        total,
        crate::randomizer::sat_i64(&target),
        crate::randomizer::sat_i64(&critical),
    );

    cmd.command = "TR".to_owned();
    cmd.modify_number = modify;
    result.text = format!(
        "({}) ＞ {total} ＞ {}",
        cmd.to_s(SuffixPosition::AfterCommand),
        result.text
    );
    Ok(Some(result))
}

/// Ruby `TrinitySeven#get_hit_roll_result`。
fn get_hit_roll_result(total: i64, target: i64, critical: i64) -> EvalResult {
    if total >= 96 {
        EvalResult::fumble("ファンブル")
    } else if total <= critical {
        EvalResult::critical("クリティカル")
    } else if total <= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `TrinitySeven#roll_damage`。
fn roll_damage(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let parser = Parser::new(&[r"\d+DM\d*"], RoundType::Floor).restrict_cmp_op_to(&[None]);
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let (dice_count_s, critical_s) = cmd
        .command
        .split_once("DM")
        .unwrap_or((cmd.command.as_str(), ""));
    let dice_count = ruby_to_i(dice_count_s);
    let critical = ruby_to_i(critical_s);
    let modify = cmd.modify_number.clone();

    let mut dice_list = rng.roll_barabara(dice_count, 6)?;
    dice_list.sort_unstable();
    let dice_text = join_dice(&dice_list);

    let (total, additional) = get_roll_damage_result(
        dice_count,
        critical,
        &mut dice_list,
        crate::randomizer::sat_i64(&modify),
    );
    let additional_text = additional
        .as_ref()
        .map(|list| format!("→[{}]", join_dice(list)))
        .unwrap_or_default();

    Ok(Some(format!(
        "({}) ＞ [{dice_text}]{additional_text}{} ＞ {total}",
        cmd.to_s(SuffixPosition::AfterCommand),
        modifier(&modify)
    )))
}

/// Ruby `TrinitySeven#get_roll_damage_result`。
fn get_roll_damage_result(
    dice_count: i64,
    mut critical: i64,
    dice_list: &mut Vec<i64>,
    modify: i64,
) -> (i64, Option<Vec<i64>>) {
    if critical <= 0 {
        let total: i64 = dice_list.iter().sum::<i64>() + modify;
        return (total, None);
    }

    if critical > dice_count {
        critical = dice_count;
    }

    let mut rest_dice = dice_list.clone();
    for _ in 0..critical {
        if !rest_dice.is_empty() {
            rest_dice.remove(0);
        }
        if !dice_list.is_empty() {
            dice_list.remove(0);
        }
        dice_list.push(7);
    }

    let max = rest_dice.pop().unwrap_or(1);
    let rest_sum: i64 = rest_dice.iter().sum();
    let power = pow7(critical);
    let total = max
        .saturating_mul(power)
        .saturating_add(rest_sum)
        .saturating_add(modify);
    (total, Some(dice_list.clone()))
}

/// Ruby `7**critical`。
fn pow7(exp: i64) -> i64 {
    match u32::try_from(exp) {
        Ok(e) => 7i64.checked_pow(e).unwrap_or(i64::MAX),
        Err(_) => i64::MAX,
    }
}

fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `TrinitySeven#roll_name`。
fn roll_name(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "TRNAME" {
        return Ok(None);
    }
    let first_name = NAME1.roll(rng)?.last_body();
    let second_name = NAME2.roll(rng)?.last_body();
    Ok(Some(format!("{first_name} , {second_name}")))
}

/// Ruby `TrinitySeven::NAME1`。
static NAME1_ITEMS: &[&str] = &[
    "春日",
    "浅見",
    "風間",
    "神無月",
    "倉田",
    "不動",
    "山奈",
    "シャルロック",
    "霧隠",
    "果心",
    "今井",
    "長瀬",
    "明智",
    "風祭",
    "志貫",
    "一文字",
    "月夜野",
    "桜田門",
    "果瀬",
    "九十九",
    "速水",
    "片桐",
    "葉月",
    "ウィンザー",
    "時雨里",
    "神城",
    "水際",
    "一ノ江",
    "仁藤",
    "北千住",
    "西村",
    "諏訪",
    "藤宮",
    "御代",
    "橘",
    "霧生",
    "白石",
    "椎名",
    "綾小路",
    "二条",
    "光明寺",
    "春秋",
    "雪見",
    "刀条院",
    "ランカスター",
    "ハクア",
    "エルタニア",
    "ハーネス",
    "アウグストゥス",
    "椎名町",
    "鍵守",
    "茜ヶ崎",
    "鎮宮",
    "美柳",
    "鎖々塚",
    "櫻ノ杜",
    "鏡ヶ守",
    "輝井",
    "南陽",
    "雪乃城",
    "六角屋",
    "鈴々",
    "東三条",
    "朱雀院",
    "青龍院",
    "白虎院",
    "玄武院",
    "麒麟院",
    "リーシュタット",
    "サンクチュアリ",
    "六実",
    "須藤",
    "ミレニアム",
    "七里",
    "三枝",
    "八殿",
    "藤里",
    "久宝",
    "東",
    "赤西",
    "神ヶ崎",
    "グランシア",
    "ダークブーレード",
    "天光寺",
    "月見里",
    "璃宮",
    "藤見澤",
    "赤聖",
    "姫宮",
    "華ノ宮",
    "\"天才\"",
    "\"達人\"",
    "\"賢者\"",
    "\"疾風\"",
    "\"海の\"",
    "\"最強\"",
    "\"凶器\"",
    "\"灼熱\"",
    "\"人間兵器\"",
    "\"魔王\"",
];
static NAME1: Table = Table::from_dice("名字表", 1, 100, NAME1_ITEMS);

/// Ruby `TrinitySeven::NAME2`。
static NAME2_ITEMS: &[&str] = &[
    "アラタ/聖",
    "アビィス/リリス",
    "ルーグ/レヴィ",
    "ラスト/アリン",
    "ソラ/ユイ",
    "イーリアス/アキオ",
    "アカーシャ/ミラ",
    "アリエス/リーゼロッテ",
    "ムラサメ/シャルム",
    "龍貴/竜姫",
    "英樹/春菜",
    "準一/湊",
    "急司郎/光理",
    "夕也/愛奈",
    "晴彦/アキ",
    "疾風/ヤシロ",
    "カガリ/灯花",
    "次郎/優都",
    "春太郎/静理",
    "ジン/時雨",
    "イオリ/伊織",
    "ユウヒ/優姫",
    "サツキ/翠名",
    "シュライ/サクラ",
    "ミナヅキ/姫乃",
    "カエデ/優樹菜",
    "ハル/フユ",
    "ドール/瑞江",
    "ニトゥレスト/キリカ",
    "スカー/綾瀬",
    "真夏/小夏",
    "光一/ののか",
    "彩/翠",
    "トウカ/柊花",
    "命/ミコト",
    "司/つかさ",
    "ゆとり/なごみ",
    "冬彦/観月",
    "カレン/華恋",
    "清次郎/亜矢",
    "サード/夢子",
    "ボックス/詩子",
    "ヘリオス/カエデ",
    "ゲート/京香",
    "オンリー/パトリシア",
    "ザッハーク/アーリ",
    "ラスタバン/ラスティ",
    "桜花/燁澄",
    "計都/リヴィア",
    "カルヴァリオ/香夜",
    "悠人/夜々子",
    "太子/羽菜",
    "夕立/夕凪",
    "アルフ/愛美",
    "ファロス/灯利",
    "スプートニク/詩姫",
    "アーネスト/累",
    "ナイン/カグヤ",
    "クリア/ヒマワリ",
    "ウォーカー/オリビア",
    "ダーク/クオン",
    "ウェイヴ/凛",
    "ルーン/マリエ",
    "エンギ/セイギ",
    "シラヌイ/ミライ",
    "ブライン/キズナ",
    "クロウ/カナタ",
    "スレイヤー/ヒカル",
    "レス/ミリアリア",
    "ミフユ/サリエル",
    "鳴央/音央",
    "モンジ/理亜",
    "パルデモントゥム/スナオ",
    "ミシェル/詩穂",
    "フレンズ/サン",
    "サトリ/識",
    "ロード/唯花",
    "クロノス/久宝",
    "フィラデルフィア/冬海",
    "ティンダロス/美星",
    "勇弥/ユーリス",
    "エイト/アンジェラ",
    "サタン/ルシエル",
    "エース/小波",
    "セージ/胡蝶",
    "忍/千之",
    "重吾/キリコ",
    "マイケル/ミホシ",
    "カズマ/鶴香",
    "ヤマト/エリシエル",
    "歴史上の人物の名前（信長、ジャンヌなど）",
    "スポーツ選手の名前（ベッカム、沙保里など）",
    "学者の名前（ソクラテス、エレナなど）",
    "アイドルの名前（タクヤ、聖子など）",
    "土地、国、町の名前（イングランド、ワシントンなど）",
    "モンスターの名前（ドラゴン、ラミアなど）",
    "武器防具の名前（ソード、メイルなど）",
    "自然現象の名前（カザンハリケーンなど）",
    "機械の名前（洗濯機、テレビなど）",
    "目についた物の名前（シャーペン、メガネなど）",
];
static NAME2: Table = Table::from_dice("名字表", 1, 100, NAME2_ITEMS);

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TrinitySeven",
            "TrinitySeven.toml",
            57,
        );
    }
}
