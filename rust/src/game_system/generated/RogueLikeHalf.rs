//! P4で手書き移植した `lib/bcdice/game_system/RogueLikeHalf.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `RogueLikeHalf#resolute_action`（判定ロール `RH+x>=t`）
//! - `RogueLikeHalf#resolute_d33`（`D33+x`）
//! - `RogueLikeHalf#roll_table_command` / `get_another_table_result` / `get_table_index`
//!   （宝物表 `NTT+x`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::Table;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::RogueLikeHalf`（ID: `RogueLikeHalf`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RogueLikeHalf;

impl GameSystem for RogueLikeHalf {
    fn id(&self) -> &'static str {
        "RogueLikeHalf"
    }

    fn name(&self) -> &'static str {
        "ローグライクハーフ"
    }

    fn sort_key(&self) -> &'static str {
        "ろおくらいくはあふ"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　RH+x>=t        x:技量点 t:達成値(威力)

例)RH+1>=5: ダイスを1個振って、技量点1,達成値5の結果を表示(クリティカル・ファンブルも表示)

■D33　D33+x        x:修正値

例)D33: 3面ダイスを2個振って、その結果を表示。

■宝物表　NTT+x     x:修正値
"
    }

    /// Ruby `register_prefix('RH', 'D33', TABLES.keys)`。
    fn prefixes(&self) -> &'static [&'static str] {
        &["RH", "D33", "NTT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `RogueLikeHalf#eval_game_system_specific_command`。
///
/// Ruby: `resolute_action(command) || resolute_d33(command) || roll_table_command(command)`
/// 最後の `roll_table_command` は該当なしのとき `[]` / `""` を返すが、
/// どちらも `Base#dice_command` の `output.empty?` で `nil` に畳まれる。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = resolute_action(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = resolute_d33(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(Some(SpecificCommandOutput::text(roll_table_command(
        command, rng,
    )?)))
}

/// Ruby `RogueLikeHalf#with_symbol`。
fn with_symbol(number: i64) -> String {
    if number == 0 {
        "+0".to_owned()
    } else if number > 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `RogueLikeHalf#get_result_of_action`。
fn get_result_of_action(total: i64, die: i64, target: i64) -> EvalResult {
    if die == 6 {
        EvalResult::critical("クリティカル")
    } else if die == 1 {
        EvalResult::fumble("ファンブル")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `/RH([+-]\d)*(>=(\d+))?/`。修正値は**1桁ぶんしか拾わない**（原典どおり）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"RH([+-]\d)*(>=(\d+))?").expect("valid regex"))
}

/// Ruby `RogueLikeHalf#resolute_action`（判定ロール）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1] ? Arithmetic.eval(m[1], @round_type) : 0
    // `[+-]\d` は必ず評価できるので `Arithmetic.eval` が nil を返す枝には入らない。
    let modify = match m.get(1) {
        Some(mo) => arithmetic::eval(mo.as_str(), RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };
    // Ruby: m[3].to_i（nil なら 0）
    let mut target = m
        .get(3)
        .map_or(0, |mo| mo.as_str().parse().unwrap_or(i64::MAX));
    if target == 0 {
        target = 4;
    }

    let die = rng.roll_once(6)?;
    let die_text = die.to_string();
    let total = die + modify;

    let mut result = get_result_of_action(total, die, target);

    let command_text = format!("(RH{}>={target})", with_symbol(modify));
    let sequence = [
        command_text,
        format!("[{die_text}]{}", with_symbol(modify)),
        total.to_string(),
        result.text.clone(),
    ];

    result.text = sequence.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `/D33([+-]\d+)*/`。
fn d33_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"D33([+-]\d+)*").expect("valid regex"))
}

/// Ruby `RogueLikeHalf#resolute_d33`（D33ロール）。
fn resolute_d33(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = d33_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1] ? Arithmetic.eval(m[1], @round_type) : 0
    let modify = match m.get(1) {
        Some(mo) => arithmetic::eval(mo.as_str(), RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };

    let dice = rng.roll_barabara(2, 3)?;
    let dice_text = dice
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("");
    // Ruby: `dice_total = 12 if > 12` → `dice_total = 4 if < 4` の順。
    // 上限を先に切っても下限が上限を超えないので `clamp(4, 12)` と等価。
    let dice_total = (dice[0] * 3 + dice[1] + modify).clamp(4, 12);
    // Ruby `Integer#divmod`。ここでは dice_total が 4..12 なので通常の除算と同じ。
    let mut quot = dice_total / 3;
    let mut rem = dice_total % 3;
    if rem == 0 {
        quot -= 1;
        rem = 3;
    }
    let total = quot * 10 + rem;

    let sequence = if modify != 0 {
        vec![
            format!("({command})"),
            format!("{dice_text}{}", with_symbol(modify)),
            total.to_string(),
        ]
    } else {
        vec![format!("({command})"), dice_text]
    };

    Ok(Some(EvalResult::with_text(sequence.join(" ＞ "))))
}

/// Ruby `/([A-Z]+)(([+]|-)(\d+))?/`。
fn table_command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([A-Z]+)(([+]|-)(\d+))?").expect("valid regex"))
}

/// Ruby `RogueLikeHalf#roll_table_command`。
///
/// 該当なしのときは空文字列（Ruby は `[]` または `""`）を返す。
fn roll_table_command(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    // Ruby: command = command.upcase（`dice_command` で大文字化済みだが原典どおり）
    let command = command.to_uppercase();
    let Some(m) = table_command_pattern().captures(&command) else {
        return Ok(String::new());
    };

    let name = m.get(1).expect("group 1 always participates").as_str();
    let operator = m.get(3).map(|mo| mo.as_str());
    // Ruby: m[4].to_i（nil なら 0）
    let value = m
        .get(4)
        .map_or(0, |mo| mo.as_str().parse().unwrap_or(i64::MAX));

    get_another_table_result(name, operator, value, rng)
}

/// Ruby `RogueLikeHalf#get_another_table_result`。
fn get_another_table_result(
    table_name: &str,
    operator: Option<&str>,
    value: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == table_name) else {
        return Ok(String::new());
    };

    let index = get_table_index(operator, value, 1, 6, rng)?;

    let info = table.choice(index);
    Ok(format!(
        "{}:{}:{}",
        info.table_name(),
        info.value(),
        info.body()
    ))
}

/// Ruby `RogueLikeHalf#get_table_index`。
fn get_table_index(
    operator: Option<&str>,
    value: i64,
    dice_count: i64,
    dice_type: i64,
    rng: &mut Randomizer,
) -> Result<i64, EvalError> {
    let modify = match operator {
        Some("+") => value,
        Some("-") => -value,
        _ => 0,
    };

    let mut index = rng.roll_sum(dice_count, dice_type)?;
    index += modify;

    index = index.max(dice_count);
    index = index.min(dice_count * dice_type + 1);

    Ok(index)
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/RogueLikeHalf.rb から書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `TABLES["NTT"]`（宝物表）。
static NTT_ITEMS: &[&str] = &[
    "金貨１枚",
    "１ｄ６枚の金貨",
    "２ｄ６枚の金貨（下限は金貨５枚）",
    "１個のアクセサリー（１ｄ６×１ｄ６枚の金貨と同等の価値）",
    "１個の宝石・小（１ｄ６×５枚の金貨と同等の価値。下限は金貨１５枚の価値）",
    "１個の宝石・大（２ｄ６×５枚の金貨と同等の価値。下限は金貨３０枚の価値）",
    "【魔法の宝物表】でダイスロールを行うこと。",
];
static NTT: Table = Table::from_dice("宝物表", 1, 6, NTT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[("NTT", &NTT)];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "RogueLikeHalf",
            "RogueLikeHalf.toml",
            16,
        );
    }
}
