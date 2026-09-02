//! P4で手書き移植した `lib/bcdice/game_system/NSSQ.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#roll_sq`（判定 `xSQ±y>=z`）
//! - `#damage_roll` / `#critical_damage_roll`（ダメージロール `xDR(C)(+)y`）
//! - `#heal_roll`（回復ロール `xHRy`）
//! - `#collecting_roll` / `#result_collecting`（採集ロール `[TSG]C±z`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `#roll_sq` の正規表現 `/(\d+)SQ([+\-\d]+)?(([>=]+)(\d+))?/i`。
fn sq_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)SQ([+\-\d]+)?(([>=]+)(\d+))?").expect("valid regex"))
}

/// Ruby `#damage_roll` の正規表現 `/(\d+)DR(C)?(\+)?(\d+)/i`。
fn dr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)DR(C)?(\+)?(\d+)").expect("valid regex"))
}

/// Ruby `#heal_roll` の正規表現 `/^(\d+)HR(\d+)?$/i`。
fn hr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)HR(\d+)?$").expect("valid regex"))
}

/// Ruby `#collecting_roll` の正規表現 `/([TSG])C([+\-\d]+)?/i`。
fn collect_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([TSG])C([+\-\d]+)?").expect("valid regex"))
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（`nil` と不正な式は 0）。
fn arith(expr: Option<&str>) -> Result<i64, EvalError> {
    match expr {
        None => Ok(0),
        Some(s) => Ok(arithmetic::eval(s, RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)),
    }
}

/// Ruby `Array#join(",")`。
fn join(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `Array#count(value)`。
fn count(dice_list: &[i64], value: i64) -> i64 {
    dice_list.iter().filter(|&&d| d == value).count() as i64
}

/// Ruby `#damage`。耐性を超えた出目の数を数える。
fn damage(dice_list: &[i64], resist: i64) -> i64 {
    dice_list.iter().filter(|&&x| x > resist).count() as i64
}

/// 判定結果の種別。Ruby は `Result` を先に作って `text` に判定文を入れているが、
/// Rust では判定文とフラグを分けて持ち、最後に生成子を選ぶ。
enum SqOutcome {
    Critical,
    Fumble,
    Success,
    Failure,
    Plain,
}

/// Ruby `NSSQ#roll_sq`。
fn roll_sq(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = sq_re().captures(command) else {
        return Ok(None);
    };

    let dice_count: i64 = m[1].parse().unwrap_or(0);
    let modifier = arith(m.get(2).map(|c| c.as_str()))?;
    let target: Option<i64> = m.get(5).and_then(|c| c.as_str().parse().ok());

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let mut sorted = dice_list.clone();
    sorted.sort_unstable();
    sorted.reverse();
    let largest_two: Vec<i64> = sorted.into_iter().take(2).collect();
    let total: i64 = largest_two.iter().sum::<i64>() + modifier;
    let num_1 = count(&dice_list, 1);

    let outcome = if largest_two == [6, 6] {
        SqOutcome::Critical
    } else if largest_two == [1, 1] {
        SqOutcome::Fumble
    } else if target.is_some_and(|t| total >= t) {
        SqOutcome::Success
    } else if target.is_some_and(|t| total < t) {
        SqOutcome::Failure
    } else {
        SqOutcome::Plain
    };

    let result_text = match outcome {
        SqOutcome::Critical => " ＞ 絶対成功！",
        SqOutcome::Fumble => " ＞ 絶対失敗！",
        SqOutcome::Success => " ＞ 成功",
        SqOutcome::Failure => " ＞ 失敗",
        // Ruby: Result.new は text が nil なので文字列補間では空文字列になる
        SqOutcome::Plain => "",
    };

    // ダイス数が2個の場合は1の出目の数だけ【FP】を獲得できる
    let fp_result = if dice_count == 2 && num_1 >= 1 {
        format!(" (【FP】{num_1}獲得)")
    } else {
        String::new()
    };

    let text = [
        format!("({command})"),
        format!(
            "[{}]{}",
            join(&dice_list),
            format::modifier(&crate::Int::from(modifier))
        ),
        format!("{total}[{}]{result_text}{fp_result}", join(&largest_two)),
    ]
    .join(" ＞ ");

    Ok(Some(match outcome {
        SqOutcome::Critical => EvalResult::critical(text),
        SqOutcome::Fumble => EvalResult::fumble(text),
        SqOutcome::Success => EvalResult::success(text),
        SqOutcome::Failure => EvalResult::failure(text),
        SqOutcome::Plain => EvalResult::with_text(text),
    }))
}

/// Ruby `NSSQ#damage_roll`。
fn damage_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = dr_re().captures(command) else {
        return Ok(None);
    };

    let dice_count: i64 = m[1].parse().unwrap_or(0);
    let critical_up = m.get(2).is_some(); // 強化効果 クリティカルアップ
    let increase_critical_dice = m.get(3).is_some();
    let resist: i64 = m[4].parse().unwrap_or(0);

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let normal_damage = damage(&dice_list, resist);

    let mut text = format!("({command}) ＞ [{}]{resist}", join(&dice_list));

    let critical_target = if critical_up { 1 } else { 2 };

    if count(&dice_list, 6) - count(&dice_list, 1) >= critical_target {
        text += &critical_damage_roll(increase_critical_dice, resist, normal_damage, rng)?;
        Ok(Some(EvalResult::critical(text)))
    } else {
        text += &format!(" ＞ {normal_damage}ダメージ");
        Ok(Some(if normal_damage > 0 {
            EvalResult::success(text)
        } else {
            EvalResult::failure(text)
        }))
    }
}

/// Ruby `NSSQ#critical_damage_roll`。
fn critical_damage_roll(
    increase_critical_dice: bool,
    resist: i64,
    normal_damage: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_count = if increase_critical_dice { 8 } else { 4 };

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let critical_damage = damage(&dice_list, resist);
    Ok(format!(
        " ＞ クリティカルヒット！ ＞ ({dice_count}DR{resist}) ＞ [{}]{resist} ＞ {}ダメージ",
        join(&dice_list),
        normal_damage + critical_damage
    ))
}

/// Ruby `NSSQ#heal_roll`。
fn heal_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = hr_re().captures(command) else {
        return Ok(None);
    };

    let dice_count: i64 = m[1].parse().unwrap_or(0);
    let resist: i64 = m.get(2).and_then(|c| c.as_str().parse().ok()).unwrap_or(3);

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let heal_amount = damage(&dice_list, resist);
    let text = format!(
        "({command}) ＞ [{}]{resist} ＞ {heal_amount}回復",
        join(&dice_list)
    );

    Ok(Some(if heal_amount > 0 {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }))
}

/// Ruby `NSSQ#collecting_roll`。
fn collecting_roll(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = collect_re().captures(command) else {
        return Ok(None);
    };

    let modifier = arith(m.get(2).map(|c| c.as_str()))?;

    let aatto_param = match m[1].to_uppercase().as_str() {
        "T" => 3,
        "S" => 4,
        "G" => 5,
        _ => return Err(EvalError::Internal("NSSQ: unexpected collecting type")),
    };

    let roll_times = aatto_param - 2 + modifier;
    if roll_times <= 0 {
        return Ok(None);
    }

    let mut results: Vec<String> = Vec::new();
    for i in 0..roll_times {
        let dice_list = rng.roll_barabara(2, 6)?;
        let dice: i64 = dice_list.iter().sum();

        results.push(format!(
            "({command}) ＞ {dice}[{}]: {}",
            join(&dice_list),
            result_collecting(i, dice, aatto_param)
        ));
    }

    Ok(Some(results.join("\n")))
}

/// Ruby `NSSQ#result_collecting`。
fn result_collecting(i: i64, dice: i64, aatto: i64) -> &'static str {
    if dice <= aatto && aatto - 2 > i {
        "！ああっと！"
    } else if aatto - 2 <= i {
        "成功（追加分）"
    } else {
        "成功"
    }
}

pub struct NSSQ;

impl GameSystem for NSSQ {
    fn id(&self) -> &'static str {
        "NSSQ"
    }

    fn name(&self) -> &'static str {
        "SRSじゃない世界樹の迷宮TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "えすああるえすしやないせかいしゆのめいきゆうTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (xSQ±y>=z)
  xD6の判定。3つ以上振ったとき、出目の高い2つを表示します。絶対成功、絶対失敗も計算します。
  2つのサイコロを使用して出目に1があった場合は、FPの獲得も表示します。3つ以上使用した場合は表示しません。
  ±y: yに修正値を入力。±の計算に対応。省略可能。
  z: 目標値。省略可能。

■ ダメージロール (xDR(C)(+)y)
  xD6のダメージロール。クリティカルヒットの自動判定を行います。Cを付けるとクリティカルアップ状態で計算できます。+を付けるとクリティカルヒット時のダイスが8個になります。
  x: xに振るダイス数を入力。
  y: yに耐性を入力。
  例) 5DR3 5DRC4 5DRC+4

■ 回復ロール (xHRy)
  xD6の回復ロール。クリティカルヒットが発生しません。
  x: xに振るダイス数を入力。
  y: yに耐性を入力。省略した場合3。
  例) 2HR 10HR2

■ 採集ロール (TC±z,SC±z,GC±z)
  少しだけ(T)、そこそこ(S)、ガッツリ(G)採取採掘伐採を行います。
  z: zに追加でロールする回数を入力。省略可能。
  例) TC SC+1 GC-1
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+SQ[\+\-\d]*",
            r"\d+DR(C)?(\+)?\d+",
            "[TSG]C",
            r"\d+HR\d*",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_sq(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = damage_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = heal_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = collecting_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("NSSQ", "NSSQ.toml", 28);
    }
}
