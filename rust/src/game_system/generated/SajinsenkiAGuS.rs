//! P4で手書き移植した `lib/bcdice/game_system/SajinsenkiAGuS.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#roll_ippan`（一般判定 `nAG+x`）
//! - `#roll_hit_check` → `#roll_om` / `#roll_nm`（命中判定 `OM` / `NM`）と `#calc_hr`
//! - `TABLES`（クリティカル表 `CR`）
//!
//! # 桁あふれ
//!
//! Ruby側は Bignum なので `10 + level + x` などが溢れない。Rust の `i64` では
//! `-99999999999999999999AG` のような入力で溢れるので、加減算と `abs` は
//! 飽和演算にしてある（本家がクラッシュしない入力でパニックしないため）。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::command_parser::{Parsed, Parser};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SajinsenkiAGuS;

impl GameSystem for SajinsenkiAGuS {
    fn id(&self) -> &'static str {
        "SajinsenkiAGuS"
    }

    fn name(&self) -> &'static str {
        "砂塵戦機アーガス"
    }

    fn sort_key(&self) -> &'static str {
        "さしんせんきああかす"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SajinsenkiAGuS#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, self.round_type(), rng)
    }
}

static HELP_MESSAGE: &str = r"・一般判定Lv（チャンス出目0→判定0） nAG+x
　　　nは習得レベル、Lv0の場合nの省略可能。xは判定値修正（数式による修正可）、省略した場合はレベル修正0
　　　例）AG:習得レベル0の一般技能、1AG+1:習得レベル1・判定値修正+1の技能、AG+2-1：習得レベル0・判定値修正2-1の技能、(1-1)AG：習得レベル1・レベル修正-1の技能

・適正距離での命中判定（チャンス出目0→判定0、HR算出）OM+y@z
　　　yは命中補正値（数式可）、zはクリティカル値。クリティカル値省略時は0
　　　HRの算出時には、HRが大きくなる場合に出目0を10に読み替えます。
　　　例）OM+18-6@2:命中補正値+18-6でクリティカル値2、適正距離の判定

・非適正距離での命中判定（チャンス出目0→判定0、HR算出）NM+y@z
　　　yは命中補正値（数式可）、zはクリティカル値。クリティカル値省略時は0
　　　HRの算出時には、HRが大きくなる場合に出目0を10に読み替えます。
　　　例）NM+4-3:命中補正値+4-3で非適正距離の判定

・クリティカル表 CR

※通常の1D10などの10面ダイスにおいて出目10の読み替えはしません。コマンドのみです。

";

static PREFIXES: &[&str] = &[r"-?\d*AG", "OM", "NM", "CR"];

/// Ruby `eval_game_system_specific_command`:
/// `roll_ippan(command) || roll_hit_check(command) || roll_tables(command, TABLES)`。
fn eval_specific(
    command: &str,
    round_type: RoundType,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_ippan(command, round_type, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_hit_check(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(text) = roll_tables(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(None)
}

/// Ruby `#roll_ippan`（一般判定）。
fn roll_ippan(
    command: &str,
    round_type: RoundType,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(-?\d+)?AG((?:[-+]\d+)*)$").expect("valid regex"));

    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: level = m[1].to_i（nil.to_i == 0）
    let level = m.get(1).map_or(0, |v| to_i(v.as_str()));
    // Ruby: x = Arithmetic.eval(m[2], @round_type) || 0
    let x = arithmetic::eval(m.get(2).map_or("", |v| v.as_str()), round_type)?.unwrap_or(I::ZERO);
    let target = if level <= 0 {
        7i64.saturating_add(crate::randomizer::sat_i64(&x))
    } else {
        10i64
            .saturating_add(level)
            .saturating_add(crate::randomizer::sat_i64(&x))
    };

    let dice_list = roll_d10_with_zero(rng, 2)?;
    let total = sum(&dice_list);
    let success_level = 1 + dice_list.iter().filter(|v| **v <= level).count() as i64;

    let mut result = EvalResult::new();
    result.set_condition(total <= target);

    let mut sequence = vec![
        format!("(2D10<={target})"),
        format!("{total}[{}]", join(&dice_list)),
    ];
    if dice_list.contains(&0) {
        sequence.push("チャンス".to_owned());
    }
    sequence.push(if result.success {
        format!("成功(+{success_level})")
    } else {
        "失敗".to_owned()
    });

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `#roll_hit_check`（命中判定＆HR算出）。
///
/// Ruby は `Command::Parser.new("OM", "NM", round_type: @round_type)`。
/// このシステムは `round_type` を上書きしないので `Base` 既定の `Floor` で固定できる。
fn roll_hit_check(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser =
        PARSER.get_or_init(|| Parser::new(&["OM", "NM"], RoundType::Floor).enable_critical());

    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    if parsed.command == "OM" {
        Ok(Some(roll_om(&parsed, rng)?))
    } else if parsed.command == "NM" {
        Ok(Some(roll_nm(&parsed, rng)?))
    } else {
        Ok(None)
    }
}

/// Ruby `#roll_om`（適正距離での命中判定）。
fn roll_om(parsed: &Parsed, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let target = parsed.modify_number.clone();
    let critical = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    // Ruby: roll_barabara(2, 10).map { 10 -> 0 }.sort.reverse
    let mut dice_list = roll_d10_with_zero(rng, 2)?;
    dice_list.sort_unstable();
    dice_list.reverse();

    let total = sum(&dice_list);
    let criticals = dice_list.iter().filter(|v| **v <= critical).count();
    let hr = calc_hr(crate::randomizer::sat_i64(&target), &dice_list);

    let mut r = EvalResult::new();
    r.set_condition(total <= crate::randomizer::sat_i64(&target));
    r.critical = criticals >= 1;

    let mut sequence = vec![
        format!("(2D10<={target})"),
        format!("{total}[{}]", join(&dice_list)),
    ];
    // Ruby: チャンス判定は振った2個すべてを見る
    if dice_list.contains(&0) {
        sequence.push("チャンス".to_owned());
    }
    sequence.push(if r.success {
        format!("成功（HR={hr}、クリティカル{criticals}）")
    } else {
        "失敗".to_owned()
    });

    r.text = sequence.join(" ＞ ");
    Ok(r)
}

/// Ruby `#roll_nm`（非適正距離での命中判定）。
fn roll_nm(parsed: &Parsed, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let target = parsed.modify_number.clone();
    let critical = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    // Ruby: roll_barabara(3, 10).map { 10 -> 0 }.sort.reverse
    let mut dice_list = roll_d10_with_zero(rng, 3)?;
    dice_list.sort_unstable();
    dice_list.reverse();

    let chosen_dice_list: Vec<i64> = dice_list.iter().take(2).copied().collect();
    let total = sum(&chosen_dice_list);
    let criticals = chosen_dice_list.iter().filter(|v| **v <= critical).count();
    let hr = calc_hr(crate::randomizer::sat_i64(&target), &chosen_dice_list);

    let mut r = EvalResult::new();
    r.set_condition(total <= crate::randomizer::sat_i64(&target));
    r.critical = criticals >= 1;

    let mut sequence = vec![
        format!("(3D10<={target})"),
        format!(
            "{total}[{},{}&{}]",
            at(&dice_list, 0),
            at(&dice_list, 1),
            at(&dice_list, 2)
        ),
    ];
    // Ruby: チャンス判定は採用した2個だけを見る（`roll_om` との違い）
    if chosen_dice_list.contains(&0) {
        sequence.push("チャンス".to_owned());
    }
    sequence.push(if r.success {
        format!("成功（HR={hr}、クリティカル{criticals}）")
    } else {
        "失敗".to_owned()
    });

    r.text = sequence.join(" ＞ ");
    Ok(r)
}

/// Ruby `#calc_hr`。出目0を0のまま／10として読み替えた場合の大きい方を採る。
fn calc_hr(target: i64, chosen_dice_list: &[i64]) -> i64 {
    let total = sum(chosen_dice_list);
    let zeros = chosen_dice_list.iter().filter(|v| **v == 0).count() as i64;
    let a = target.saturating_sub(total).saturating_abs();
    let b = target
        .saturating_sub(total)
        .saturating_sub(zeros.saturating_mul(10))
        .saturating_abs();
    a.max(b)
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "CR" {
        return Ok(None);
    }
    Ok(Some(CRITICAL_TABLE.roll(rng)?.to_string()))
}

/// Ruby `roll_barabara(times, 10).map { |v| v == 10 ? 0 : v }`。
fn roll_d10_with_zero(rng: &mut Randomizer, times: i64) -> Result<Vec<i64>, EvalError> {
    Ok(rng
        .roll_barabara(times, 10)?
        .into_iter()
        .map(|v| if v == 10 { 0 } else { v })
        .collect())
}

fn sum(values: &[i64]) -> i64 {
    values.iter().fold(0i64, |a, b| a.saturating_add(*b))
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby の `dice_list[i]` を文字列補間したときの値（範囲外は `nil` → 空文字列）。
fn at(values: &[i64], index: usize) -> String {
    values.get(index).map(|v| v.to_string()).unwrap_or_default()
}

/// Ruby `String#to_i`。桁あふれは Ruby だと Bignum になるので i64 の端へ飽和させる。
fn to_i(text: &str) -> i64 {
    text.parse().unwrap_or(if text.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `TABLES["CR"]`（クリティカル表）。
static CRITICAL_TABLE: Table = Table::from_dice("クリティカル表", 1, 10, CRITICAL_ITEMS);

static CRITICAL_ITEMS: &[&str] = &[
    "1：「小破」ダメージ+［5］。耐久値-［1］",
    "2：「小破」ダメージ+［5］。耐久値-［1］",
    "3：「小破」ダメージ+［5］。耐久値-［1］",
    "4：「小破」ダメージ+［5］。耐久値-［1］",
    "5：「兵装」損壊を受けるごとに［1D10］を振り、出目に応じた部位の兵装とオプションが《脱落》",
    "6：「上体」攻撃系能力［白兵/ 火器/ 索敵］は各［- 損壊Lv］",
    "7：「脚部」行動系・防御系能力［Iv 値（イニシア値）/ 最大MP/ 回避］は各［- 損壊Lv］",
    "8：「搭乗者」搭乗者の〈最大HP〉および〈HP〉は［-（4 ×損壊Lv）］",
    "9：「搭乗者」搭乗者の〈最大HP〉および〈HP〉は［-（4 ×損壊Lv）］",
    "0：「小破」ダメージ+［5］。耐久値-［1］",
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "SajinsenkiAGuS",
            "SajinsenkiAGuS.toml",
            26,
        );
    }
}
