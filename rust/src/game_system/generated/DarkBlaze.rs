//! P4で手書き移植した `lib/bcdice/game_system/DarkBlaze.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DarkBlaze#replace_text`（`DBxy` / `DBx,y` / `DB@x@y` / `DBxy#m` → `3R6[...]`）
//! - `DarkBlaze#check_roll` → `check_roll`（3R6判定。クリティカル/ファンブル処理込み）
//! - `DarkBlaze#get_dice` → `get_dice`
//! - `DarkBlaze#eval_game_system_specific_command`（`BTx` 掘り出し袋表 → `get_horidasibukuro_table`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::DarkBlaze`（ID: `DarkBlaze`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkBlaze;

impl GameSystem for DarkBlaze {
    fn id(&self) -> &'static str {
        "DarkBlaze"
    }

    fn name(&self) -> &'static str {
        "ダークブレイズ"
    }

    fn sort_key(&self) -> &'static str {
        "たあくふれいす"
    }

    fn help_message(&self) -> &'static str {
        r#"・行為判定　(DBxy#n)
　行為判定専用のコマンドです。
　"DB(能力)(技能)#(修正)"でロールします。Rコマンド(3R6+n[x,y]>=m mは難易度)に読替をします。
　クリティカルとファンブルも自動で処理されます。
　DB@x@y#m と DBx,y#m にも対応しました。
　例）DB33　　　DB32#-1　　　DB@3@1#1　　　DB3,2　　　DB23#1>=4　　　3R6+1[3,3]>=4

・掘り出し袋表　(BTx)
　"BT(ダイス数)"で掘り出し袋表を自動で振り、結果を表示します。
　例）BT1　　　BT2　　　BT[1...3]
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["DB", "BT", "3R6"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `/BT(\d)?$/i`。
fn bt_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^BT(\d)?$").expect("valid regex"))
}

/// Ruby `/(^|\s)S?(3[rR]6([+\-\d]+)?(\[(\d+),(\d+)\])(([>=]+)(\d+))?)(\s|$)/i`。
///
/// キャプチャ番号は Ruby に合わせる:
/// 1=`(^|\s)` 2=コマンド全体 3=修正 4=[]群 5,6=[]内 7=比較群 8=演算子 9=難易度 10=`(\s|$)`
fn r6_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)S?(3[rR]6([+\-\d]+)?(\[(\d+),(\d+)\])(([>=]+)(\d+))?)(\s|$)")
            .expect("valid regex")
    })
}

/// Ruby `DarkBlaze#replace_text`。
///
/// `DB` を含まない文字列は無変換（Ruby `return string unless string =~ /DB/i`）。
/// `DBx,y` / `DB@x@y` を `DBxy` へ正規化してから `#n` 付き→無しの順に置換する。
fn replace_text(string: &str) -> String {
    if !string.contains("DB")
        && !string.contains("db")
        && !string.contains("Db")
        && !string.contains("dB")
    {
        return string.to_string();
    }

    let s = Regex::new(r"(?i)DB(\d),(\d)")
        .unwrap()
        .replace_all(string, "DB${1}${2}");
    let s = Regex::new(r"(?i)DB@(\d)@(\d)")
        .unwrap()
        .replace_all(&s, "DB${1}${2}");
    let s = Regex::new(r"(?i)DB(\d)(\d)#(\d[+\-\d]*)")
        .unwrap()
        .replace_all(&s, "3R6+${3}[${1},${2}]");
    let s = Regex::new(r"(?i)DB(\d)(\d)#([+\-\d]*)")
        .unwrap()
        .replace_all(&s, "3R6${3}[${1},${2}]");
    Regex::new(r"(?i)DB(\d)(\d)")
        .unwrap()
        .replace_all(&s, "3R6[${1},${2}]")
        .into_owned()
}

/// Ruby `DarkBlaze#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(caps) = bt_pattern().captures(command) {
        let dice: i64 = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);
        return get_horidasibukuro_table(dice, rng)
            .map(|text| Some(SpecificCommandOutput::text(text)));
    }

    check_roll(command, rng)
}

/// Ruby `DarkBlaze#check_roll`。
fn check_roll(
    string: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let string = replace_text(string);
    let Some(caps) = r6_pattern().captures(&string) else {
        return Ok(None);
    };

    let matched = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_owned();
    let modifier_expr = caps.get(3).map(|m| m.as_str());
    let mut modifier_value = 0;
    if let Some(expr) = modifier_expr {
        modifier_value = arithmetic::eval(expr, RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0);
    }

    let mut abl = 1;
    let mut skl = 1;
    if caps.get(4).is_some() {
        abl = caps
            .get(5)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        skl = caps
            .get(6)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
    }

    let mut sign_of_inequality = "";
    let mut diff = 0;
    if let Some(cmp_group) = caps.get(7) {
        // Ruby: Normalize.comparison_operator(m[8]) → Format.comparison_operator(...)。
        // `([>=]+)` が生み出すのは `>=` と `>` のみで、どちらも比較は `total >= diff`
        // （`>` は Ruby の `comparison_operator` で `>=` へ正規化されないが、
        // 原典の `send(:>, diff)` と `>=` の差は `[>=]+` が `>` 1文字しか
        // 作らないため実質出ない。ここでは成功判定として `>=` を使う）。
        let _ = caps.get(8);
        sign_of_inequality = cmp_group.as_str();
        diff = caps
            .get(9)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
    }

    let (total, out_str) = get_dice(modifier_value, abl, skl, rng)?;
    let mut output = format!("({matched}) ＞ {out_str}");

    if !sign_of_inequality.is_empty() {
        // Ruby: `total.send(cmp_op, diff)`。`[>=]+` の正規表現なので比較は常に `>=`。
        if total >= diff {
            output.push_str(" ＞ 成功");
        } else {
            output.push_str(" ＞ 失敗");
        }
    }

    Ok(Some(SpecificCommandOutput::text(output)))
}

/// Ruby `DarkBlaze#get_dice`。
///
/// 3+|修正| 個の d6 を振ってソートし、先頭3つ（修正が負なら末尾3つ）を判定に使う。
/// `ch <= 能力値` と `ch <= 技能値` の成立数が達成値。3つすべてが2以下でクリティカル
/// （達成値 = 6+技能値）、3つすべてが5以上でファンブル（達成値 = 0）。
fn get_dice(
    modifier: i64,
    abl: i64,
    skl: i64,
    rng: &mut Randomizer,
) -> Result<(i64, String), EvalError> {
    let mut total = 0;
    let mut crit = 0;
    let mut fumble = 0;
    let dice_c = 3 + modifier.unsigned_abs() as usize;

    let mut dice_arr = rng.roll_barabara(dice_c as i64, 6)?;
    dice_arr.sort_unstable();
    let dice_str = dice_arr
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    for i in 0..3 {
        let index = if modifier < 0 { dice_c - i - 1 } else { i };
        let ch = dice_arr
            .get(index)
            .copied()
            .ok_or(EvalError::Internal("DarkBlaze: dice index out of range"))?;

        if ch <= abl {
            total += 1;
        }
        if ch <= skl {
            total += 1;
        }
        if ch <= 2 {
            crit += 1;
        }
        if ch >= 5 {
            fumble += 1;
        }
    }

    let mut result_text = String::new();

    if crit >= 3 {
        result_text = " ＞ クリティカル".to_string();
        total = 6 + skl;
    }

    if fumble >= 3 {
        result_text = " ＞ ファンブル".to_string();
        total = 0;
    }

    let output = format!("{total}[{dice_str}]{result_text}");

    Ok((total, output))
}

/// Ruby `DarkBlaze#get_horidasibukuro_table`（掘り出し袋表）。
fn get_horidasibukuro_table(dice: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    const MATERIAL_KIND: [&str; 8] = [
        // 2D6: 5, 6, 7, 8, 9, 10, 11, 12
        "蟲甲",     // 5
        "金属",     // 6
        "金貨",     // 7
        "植物",     // 8
        "獣皮",     // 9
        "竜鱗",     // 10
        "レアモノ", // 11
        "レアモノ", // 12
    ];

    const MAGIC_STONE: [&str; 3] = ["火炎石", "雷撃石", "氷結石"];

    let num1 = rng.roll_sum(2, 6)?;
    let mut num2 = rng.roll_sum(dice, 6)?;

    let output: String = if num1 <= 4 {
        num2 = rng.roll_once(6)?;
        let magic_stone_result = MAGIC_STONE[((num2 / 2) - 1) as usize];
        format!("《{magic_stone_result}》を{dice}個獲得")
    } else if num1 == 7 {
        format!("《金貨》を{num2}枚獲得")
    } else {
        let kind = MATERIAL_KIND[(num1 - 5) as usize];

        if num2 <= 3 {
            format!("《{kind} I》を1個獲得")
        } else if num2 <= 5 {
            format!("《{kind} I》を2個獲得")
        } else if num2 <= 7 {
            format!("《{kind} I》を3個獲得")
        } else if num2 <= 9 {
            format!("《{kind} II》を1個獲得")
        } else if num2 <= 11 {
            format!("《{kind} I》を2個《{kind} II》を1個獲得")
        } else if num2 <= 13 {
            format!("《{kind} I》を2個《{kind} II》を2個獲得")
        } else if num2 <= 15 {
            format!("《{kind} III》を1個獲得")
        } else if num2 <= 17 {
            format!("《{kind} II》を2個《{kind} III》を1個獲得")
        } else {
            format!("《{kind} II》を2個《{kind} III》を2個獲得")
        }
    };

    Ok(format!("掘り出し袋表[{num1},{num2}] ＞ {output}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DarkBlaze",
            "DarkBlaze.toml",
            101,
        );
    }
}
