//! P4で手書き移植した `lib/bcdice/game_system/HarnMaster.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `HarnMaster#result_1d100`（1D100の致命的失敗・決定的成功判定）
//! - `HarnMaster#getCheckShockResult`（ショック判定 `SHKx,y`）
//! - `HarnMaster#getStrikeLocationHuman`（人型用命中部位表 `SLH` / `SLHU` / `SLHD`）
//!
//! # 原典との差異
//!
//! Ruby の `eval_game_system_specific_command` は `rescue StandardError => e` で
//! 例外メッセージを出力文字列として返す。ここでは `TooManyRandsError`（`SHK1,10001` など）を
//! そのまま [`EvalError`] として伝播させる。TOMLに該当ケースがなく、
//! 本移植では例外メッセージの文言を再現する意味がないため
//! （`Chill` と同じ方針）。`raise "unknow atak type"` の枝は、
//! `@enabled_upcase_input` により `type` が `"U"` / `"D"` / なし のいずれかにしかならないので
//! 原典でも到達しない。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::HarnMaster`（ID: `HarnMaster`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnMaster;

impl GameSystem for HarnMaster {
    fn id(&self) -> &'static str {
        "HarnMaster"
    }

    fn name(&self) -> &'static str {
        "ハーンマスター"
    }

    fn sort_key(&self) -> &'static str {
        "はあんますたあ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　1D100<=XX の判定時に致命的失敗・決定的成功を判定
・ショック判定（SHKx）
　例）SHK13,3
・人型用　中段命中部位表 (SLH)／上段命中部位 (SLHU)／上段命中部位 (SLHD)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SHK", "SLH", "SLHU", "SLHD"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `HarnMaster#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == '?'
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        // Ruby `Integer#%` と Rust の `%` は符号が違うが、`== 0` の判定は一致する。
        let result = if total <= target {
            if total % I::from(5) == I::ZERO {
                EvalResult::critical("決定的成功")
            } else {
                EvalResult::success("成功")
            }
        } else if total % I::from(5) == I::ZERO {
            EvalResult::fumble("致命的失敗")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }

    /// Ruby `HarnMaster#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(m) = shock_pattern().captures(command) {
            // Ruby: `\d*` は空文字列にマッチしうる（`"".to_i == 0`）
            let toughness: i64 = m[1].parse().unwrap_or(0);
            let damage: i64 = m[2].parse().unwrap_or(i64::MAX);
            let text = check_shock_result(damage, toughness, rng)?;
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        if let Some(m) = strike_location_pattern().captures(command) {
            let strike_type = m.get(1).map(|g| g.as_str());
            let text = strike_location_human(strike_type, rng)?;
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        // Ruby: else -> nil
        Ok(None)
    }
}

/// Ruby `/^SHK(\d*),(\d+)/i`。
fn shock_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^SHK(\d*),(\d+)").expect("valid regex"))
}

/// Ruby `/SLH(U|D)?/i`（アンカーなし）。
fn strike_location_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)SLH(U|D)?").expect("valid regex"))
}

/// Ruby `HarnMaster#getCheckShockResult`。
fn check_shock_result(
    damage: i64,
    toughness: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_list = rng.roll_barabara(damage, 6)?;
    let dice: i64 = dice_list.iter().sum();
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let result = if dice <= toughness {
        "成功"
    } else {
        "失敗"
    };

    Ok(format!(
        "ショック判定(ダメージ:{damage}, 耐久力:{toughness}) ＞ ({dice}[{dice_text}]) ＞ {result}"
    ))
}

/// Ruby `HarnMaster#getStrikeLocationHuman`。
///
/// `strike_type` は `Regexp.last_match(1)`（`"U"` / `"D"` / なし）。
fn strike_location_human(
    strike_type: Option<&str>,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let (type_name, table): (&str, &[TableItem]) = match strike_type {
        Some("U") => ("命中部位(人型 上段)", UPPER_TABLE),
        Some("D") => ("命中部位(人型 下段)", DOWN_TABLE),
        // Ruby: when nil。それ以外は raise だが、入力が大文字化されるため到達しない。
        _ => ("命中部位(人型 中段)", NORMAL_TABLE),
    };

    let number = rng.roll_once(100)?;
    let part = get_table_by_number(number, table);
    let part = get_location_side(part, number);
    let part = get_face_location(&part, rng)?;

    Ok(format!("{type_name} ＞ ({number}){part}"))
}

/// Ruby `HarnMaster#getLocationSide`。
///
/// 先頭が `*` の項目だけ、出目の偶奇で「左」「右」に置き換える（最初の1個のみ）。
fn get_location_side(part: &str, number: i64) -> String {
    // Ruby: unless part =~ /^\*/ -> return part
    if !part.starts_with('*') {
        return part.to_owned();
    }

    // Ruby: number.odd? ? "左" : "右"
    let side = if number % 2 != 0 { "左" } else { "右" };
    // Ruby: part.sub(/\*/, side) は最初の1個だけ置換する
    part.replacen('*', side, 1)
}

/// Ruby `HarnMaster#getFaceLocation`。
///
/// 末尾が `+` の項目（「顔+」）だけ、追加で顔面部位表を振る。
fn get_face_location(part: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    // Ruby: unless part =~ /\+$/ -> return part
    let Some(head) = part.strip_suffix('+') else {
        return Ok(part.to_owned());
    };

    let number = rng.roll_once(100)?;
    let face_location = get_table_by_number(number, FACE_TABLE);
    let face_location = get_location_side(face_location, number);

    // Ruby: part.sub(/\+$/, " ＞ (#{number})#{faceLocation}")
    Ok(format!("{head} ＞ ({number}){face_location}"))
}

/// 表の項目。Ruby の `[上限, 内容]`。
type TableItem = (i64, &'static str);

/// Ruby `Base#get_table_by_number(index, table, default = "1")`。
fn get_table_by_number(index: i64, table: &[TableItem]) -> &'static str {
    for &(number, text) in table {
        if number >= index {
            return text;
        }
    }
    "1"
}

/// Ruby `HarnMaster#getFaceLocation` 内の顔面部位表。
static FACE_TABLE: &[TableItem] = &[
    (15, "顎"),
    (30, "*目"),
    (64, "*頬"),
    (80, "鼻"),
    (90, "*耳"),
    (100, "口"),
];

/// Ruby `HarnMaster#getStrikeLocationHumanUpperTable`。
static UPPER_TABLE: &[TableItem] = &[
    (15, "頭部"),
    (30, "顔+"),
    (45, "首"),
    (57, "*肩"),
    (69, "*上腕"),
    (73, "*肘"),
    (81, "*前腕"),
    (85, "*手"),
    (95, "胸部"),
    (100, "腹部"),
];

/// Ruby `HarnMaster#getStrikeLocationHumanNormalTable`。
static NORMAL_TABLE: &[TableItem] = &[
    (5, "頭部"),
    (10, "顔+"),
    (15, "首"),
    (27, "*肩"),
    (33, "*上腕"),
    (35, "*肘"),
    (39, "*前腕"),
    (43, "*手"),
    (60, "胸部"),
    (70, "腹部"),
    (74, "股間"),
    (80, "*臀部"),
    (88, "*腿"),
    (90, "*膝"),
    (96, "*脛"),
    (100, "*足"),
];

/// Ruby `HarnMaster#getStrikeLocationHumanDownTable`。
static DOWN_TABLE: &[TableItem] = &[
    (6, "*前腕"),
    (12, "*手"),
    (19, "胸部"),
    (29, "腹部"),
    (35, "股間"),
    (49, "*臀部"),
    (70, "*腿"),
    (78, "*膝"),
    (92, "*脛"),
    (100, "*足"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "HarnMaster",
            "HarnMaster.toml",
            18,
        );
    }
}
