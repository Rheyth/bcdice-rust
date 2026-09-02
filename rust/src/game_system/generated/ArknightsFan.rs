//! P4で手書き移植した `lib/bcdice/game_system/ArknightsFan.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ArknightsFan#eval_ad`（能力値判定 `nADm<=x`）
//! - `#eval_ab` / `#roll_ab` / `#roll_ab_withtype`（攻撃/防御判定 `nABm<=x`、役職付き）
//! - `#eval_orp` / `#roll_orp`（鉱石病判定 `ORPx@y+Dd+Tt`）
//! - `#eval_worsening` / `#eval_addiction`（`--WORSENING` / `--ADDICTION`）
//! - `#check_roll`（クリティカル/エラーと成功失敗の6通り優先）

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::ArknightsFan`（ID: `ArknightsFan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArknightsFan;

impl GameSystem for ArknightsFan {
    fn id(&self) -> &'static str {
        "ArknightsFan"
    }

    fn name(&self) -> &'static str {
        "アークナイツTRPG by daaaper"
    }

    fn sort_key(&self) -> &'static str {
        "ああくないつTRPGはいてえはあ"
    }

    fn help_message(&self) -> &'static str {
        r"■ 能力値判定 (nADm<=x)
  nDmのダイスロールをして、出目が x 以下であれば成功。
  出目が91以上でエラー。
  出目が10以下でクリティカル。

■ 攻撃/防御判定 (nABm<=x)
  nBmのダイスロールをして、
    出目が x 以下であれば成功数+1。
    出目が91以上でエラー。成功数-1。
    出目が10以下でクリティカル。成功数+1。
  上記による成功数をカウント。

■ 役職効果付き攻撃判定 (nABm<=x--役職名h)
  h: 健康状態(0: 健康、1: 中等症、2: 重症)
  nBmのダイスロールをして、
    出目が x 以下であれば成功数+1。
    出目が91以上でエラー。成功数-1。
    出目が10以下でクリティカル。成功数+1。
  上記による成功数をカウントした上で、以下の役職名による成功数増加効果を適応。
    狙撃（SNI）: 健康(h=0)かつ成功数1以上のとき、成功数+1。
  健康状態hを省略した場合、健康(h=0)として扱われる。

■ 鉱石病判定 (ORPx@y+Dd+Tt)
  x: 生理的耐性、y: 上昇後侵食度、d: ダイス補正、t: 判定値補正
  生理的耐性xのOPが侵食度yに上昇した際の鉱石病判定を、ダイス数補正d、判定値補正tで行う。
  ダイス数補正と判定値補正は省略可能。例えば ORP60@25 は ORP60@25+D0+T0 と同義。
  また、ダイス数補正と判定値補正は逆順でも可。例えば ORP60@25+T10+D2 も可。

■ 増悪判定（--WORSENING）
  症状を「末梢神経障害」「内臓機能不全」「精神症状」からランダムに選択。
  継続ラウンド数を1d6+1で判定。

■ 中毒判定（--ADDICTION）
  症状を「脳神経障害」「多臓器不全」「急性精神反応」からランダムに選択。

■ 判定の省略表記
  nADm、nABm、nABmにおいて、
    n（ダイス個数）を省略した場合、1として扱われる。
    m（ダイス種類）を省略した場合、100として扱われる。
  例えば、AD<=90は1AD100<=90として解釈される。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"[-+*/\d]*AD\d*",
            r"[-+*/\d]*AB\d*",
            "ORP",
            "--WORSENING",
            "--ADDICTION",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn sides_implicit_d(&self) -> i64 {
        100
    }

    /// Ruby `ArknightsFan#eval_game_system_specific_command`。
    ///
    /// `eval_ad || eval_ab || eval_orp || eval_worsening || eval_addiction`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = eval_ad(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = eval_ab(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = eval_orp(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = eval_worsening(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(eval_addiction(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `ArknightsFan::Status`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Critical,
    Success,
    Failure,
    Error,
}

/// Ruby `ArknightsFan::STATUS_NAME`。
fn status_name(status: Status) -> &'static str {
    match status {
        Status::Critical => "クリティカル！",
        Status::Success => "成功",
        Status::Failure => "失敗",
        Status::Error => "エラー",
    }
}

/// Ruby `check_roll` のクリティカル/エラー軸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CriError {
    Critical,
    Error,
    Neutral,
}

/// Ruby `ArknightsFan#check_roll`。
///
/// 成功判定とクリティカル/エラーは独立で、次の6通り:
/// - 成功+Critical → CRITICAL
/// - 成功+Neutral → SUCCESS
/// - 成功+Error → SUCCESS（成功時はエラーを適用しない）
/// - 失敗+Critical → FAILURE（失敗時はクリティカルを適用しない）
/// - 失敗+Neutral → FAILURE
/// - 失敗+Error → ERROR
fn check_roll(roll_result: i64, target: &I) -> Status {
    let target = crate::randomizer::sat_i64(target);
    let success = roll_result <= target;
    let crierror = if roll_result <= 10 {
        CriError::Critical
    } else if roll_result >= 91 {
        CriError::Error
    } else {
        CriError::Neutral
    };

    match (success, crierror) {
        (true, CriError::Critical) => Status::Critical,
        (true, CriError::Neutral | CriError::Error) => Status::Success,
        (false, CriError::Critical | CriError::Neutral) => Status::Failure,
        (false, CriError::Error) => Status::Error,
    }
}

/// Ruby `Arithmetic.eval(source, @round_type)`（Base 既定 `:floor`）。
/// パース失敗・ゼロ除算は `Ok(None)`（コマンド未成立）。
fn eval_floor(source: &str) -> Result<Option<I>, EvalError> {
    arithmetic::eval(source, RoundType::Floor)
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `@randomizer.roll_barabara(times, sides).sort`。
fn roll_sorted(rng: &mut Randomizer, times: i64, sides: i64) -> Result<Vec<i64>, EvalError> {
    let mut dice_list = rng.roll_barabara(times, sides)?;
    dice_list.sort_unstable();
    Ok(dice_list)
}

/// 回数（空なら 1）と面数（空なら 100）。
fn parse_times_and_sides(times: &str, sides: &str) -> Result<Option<(i64, i64)>, EvalError> {
    let times = if times.is_empty() {
        1
    } else {
        let Some(times) = eval_floor(times)? else {
            return Ok(None);
        };
        crate::randomizer::sat_i64(&times)
    };
    let sides = if sides.is_empty() {
        100
    } else {
        sides.parse().unwrap_or(0)
    };
    Ok(Some((times, sides)))
}

/// Ruby `%r{^([-+*/\d]*)AD(\d*)<=([-+*/\d]+)$}`。
fn ad_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([-+*/\d]*)AD(\d*)<=([-+*/\d]+)$").expect("valid regex"))
}

/// Ruby `%r{^([-+*/\d]*)AB(\d*)<=([-+*/\d]+)(?:--([^\d\s]+)([0-2])?)?$}`。
fn ab_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([-+*/\d]*)AB(\d*)<=([-+*/\d]+)(?:--([^\d\s]+)([0-2])?)?$")
            .expect("valid regex")
    })
}

/// Ruby 第1正規表現（`+D` のあと `+T`）。
///
/// `^ORP(?<END>...)@(?<ORP>...)(?:\+D(?<DICE>...))?(?:\+T(?<TGT>...))?$`
fn orp_pattern_dt() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^ORP(?<END>[-+*/\d]+)@(?<ORP>[-+*/\d]+)(?:\+D(?<DICE>[-+*/\d]+))?(?:\+T(?<TGT>[-+*/\d]+))?$",
        )
        .expect("valid regex")
    })
}

/// Ruby 第2正規表現（`+T` のあと `+D`）。D補正とT補正が逆順でも対応する。
fn orp_pattern_td() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^ORP(?<END>[-+*/\d]+)@(?<ORP>[-+*/\d]+)(?:\+T(?<TGT>[-+*/\d]+))?(?:\+D(?<DICE>[-+*/\d]+))?$",
        )
        .expect("valid regex")
    })
}

/// Ruby `ArknightsFan#eval_ad`。
fn eval_ad(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = ad_pattern().captures(command) else {
        return Ok(None);
    };

    let Some((times, sides)) = parse_times_and_sides(&m[1], &m[2])? else {
        return Ok(None);
    };
    let Some(target) = eval_floor(&m[3])? else {
        return Ok(None);
    };

    roll_ad(command, times, sides, &target, rng).map(Some)
}

/// Ruby `ArknightsFan#roll_ad`。
fn roll_ad(
    command: &str,
    times: i64,
    sides: i64,
    target: &I,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let dice_list = roll_sorted(rng, times, sides)?;
    let total: i64 = dice_list.iter().sum();
    let result = check_roll(total, target);

    let result_text = if times == 1 {
        format!(
            "({command}) ＞ {} ＞ {}",
            join_dice(&dice_list),
            status_name(result)
        )
    } else {
        format!(
            "({command}) ＞ {total}[{}] ＞ {}",
            join_dice(&dice_list),
            status_name(result)
        )
    };

    Ok(match result {
        Status::Critical => EvalResult::critical(result_text),
        Status::Success => EvalResult::success(result_text),
        Status::Failure => EvalResult::failure(result_text),
        Status::Error => EvalResult::fumble(result_text),
    })
}

/// Ruby `ArknightsFan#eval_ab`。
fn eval_ab(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = ab_pattern().captures(command) else {
        return Ok(None);
    };

    let Some((times, sides)) = parse_times_and_sides(&m[1], &m[2])? else {
        return Ok(None);
    };
    let Some(target) = eval_floor(&m[3])? else {
        return Ok(None);
    };

    let type_name = m.get(4).map(|g| g.as_str());
    let type_status = if let Some(status) = m.get(5) {
        status.as_str().parse().unwrap_or(0)
    } else if type_name == Some("SNIPER") {
        // スプレッドシート版キャラシの後方互換
        1
    } else {
        0
    };

    if let Some(type_name) = type_name {
        roll_ab_withtype(command, times, sides, &target, type_name, type_status, rng).map(Some)
    } else {
        roll_ab(command, times, sides, &target, rng).map(Some)
    }
}

/// Ruby `ArknightsFan#process_ab`。戻り値は `(success, critical, error)`。
fn process_ab(dice_list: &[i64], target: &I) -> (i64, i64, i64) {
    let mut success_count = 0;
    let mut critical_count = 0;
    let mut error_count = 0;

    for &value in dice_list {
        match check_roll(value, target) {
            Status::Critical => {
                critical_count += 1;
                success_count += 1;
            }
            Status::Success => success_count += 1,
            Status::Failure => {}
            Status::Error => error_count += 1,
        }
    }

    (success_count, critical_count, error_count)
}

/// Ruby `Result.new.tap { r.condition = ...; r.critical = ...; r.fumble = ... }`。
///
/// `condition=` は success/failure だけを立て、critical/fumble は独立。
/// `EvalResult::success` は result_count==0 でも success を強制するので使わない。
fn ab_eval_result(
    text: String,
    result_count: i64,
    critical_count: i64,
    error_count: i64,
) -> EvalResult {
    let mut r = EvalResult::with_text(text);
    r.set_condition(result_count > 0);
    r.critical = critical_count > 0;
    r.fumble = error_count > 0;
    r
}

/// Ruby `ArknightsFan#roll_ab`。
fn roll_ab(
    command: &str,
    times: i64,
    sides: i64,
    target: &I,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let dice_list = roll_sorted(rng, times, sides)?;

    let (success_count, critical_count, error_count) = process_ab(&dice_list, target);
    let result_count = success_count + critical_count - error_count;

    let result_text = format!(
        "({command}) ＞ [{}] ＞ {success_count}+{critical_count}C-{error_count}E ＞ 成功数{result_count}",
        join_dice(&dice_list)
    );
    Ok(ab_eval_result(
        result_text,
        result_count,
        critical_count,
        error_count,
    ))
}

/// Ruby `ArknightsFan#roll_ab_withtype`。
fn roll_ab_withtype(
    command: &str,
    times: i64,
    sides: i64,
    target: &I,
    type_name: &str,
    type_status: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let dice_list = roll_sorted(rng, times, sides)?;

    let (success_count, critical_count, error_count) = process_ab(&dice_list, target);
    let mut result_count = success_count + critical_count - error_count;

    let result_mod = match type_name {
        "SNI" => Some(i64::from(type_status == 0 && result_count > 0)),
        "SNIPER" => Some(i64::from(type_status != 0 && result_count > 0)),
        _ => None,
    };

    let result_text = if let Some(result_mod) = result_mod {
        result_count += result_mod;
        format!(
            "({command}) ＞ [{}] ＞ {success_count}+{critical_count}C-{error_count}E+{result_mod}({type_name}) ＞ 成功数{result_count}",
            join_dice(&dice_list)
        )
    } else {
        format!(
            "({command}) ＞ [{}] ＞ {success_count}+{critical_count}C-{error_count}E ＞ 成功数{result_count}",
            join_dice(&dice_list)
        )
    };

    Ok(ab_eval_result(
        result_text,
        result_count,
        critical_count,
        error_count,
    ))
}

/// Ruby `times_mod = !m[3].nil? ? Arithmetic.eval(m[:DICE]) : 0` と
/// `target_mod = !m[4].nil? ? Arithmetic.eval(m[:TGT]) : 0`。
///
/// 番号付きグループの有無で評価する。第1正規表現は g3=DICE / g4=TGT、
/// 第2正規表現は g3=TGT / g4=DICE。両方あるときは named 経由で両補正が乗る。
fn orp_mod(m: &Captures, numbered: usize, name: &str) -> Result<Option<i64>, EvalError> {
    if m.get(numbered).is_none() {
        return Ok(Some(0));
    }
    let Some(value) = m.name(name) else {
        return Ok(None);
    };
    let Some(v) = eval_floor(value.as_str())? else {
        return Ok(None);
    };
    Ok(Some(crate::randomizer::sat_i64(&v)))
}

/// Ruby `ArknightsFan#eval_orp`。
fn eval_orp(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let m = orp_pattern_dt()
        .captures(command)
        .or_else(|| orp_pattern_td().captures(command));
    let Some(m) = m else {
        return Ok(None);
    };

    let Some(endurance) = eval_floor(&m["END"])? else {
        return Ok(None);
    };
    let endurance = crate::randomizer::sat_i64(&endurance);
    let Some(oripathy) = eval_floor(&m["ORP"])? else {
        return Ok(None);
    };
    let oripathy = crate::randomizer::sat_i64(&oripathy);
    let Some(times_mod) = orp_mod(&m, 3, "DICE")? else {
        return Ok(None);
    };
    let Some(target_mod) = orp_mod(&m, 4, "TGT")? else {
        return Ok(None);
    };

    roll_orp(command, endurance, oripathy, times_mod, target_mod, rng).map(Some)
}

/// Ruby `ENDURANCE_LEVEL_TABLE`。最後の `Float::INFINITY` は「どれにも入らなければ 4」。
const ENDURANCE_LEVEL_TABLE: [i64; 4] = [20, 40, 70, 90];
/// Ruby `ORP_TIMES_TABLE`。
const ORP_TIMES_TABLE: [i64; 5] = [1, 2, 2, 3, 4];

/// Ruby `ArknightsFan#roll_orp`。
fn roll_orp(
    command: &str,
    endurance: i64,
    oripathy: i64,
    times_mod: i64,
    target_mod: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let endurance_level = ENDURANCE_LEVEL_TABLE
        .iter()
        .position(|&n| endurance <= n)
        .unwrap_or(4);
    let original_times = ORP_TIMES_TABLE[endurance_level];
    let times = original_times + times_mod;

    if oripathy <= 20 {
        return Ok(EvalResult::with_text(format!(
            "({command}) ＞ 鉱石病判定が発生しない侵食度です。侵食度は21以上を指定してください。"
        )));
    }

    // Ruby: `(oripathy / 20.0).ceil - 1`
    let oripathy_stage = (oripathy as f64 / 20.0).ceil() as i64 - 1;
    let original_target = (80 - oripathy_stage * 20) - (oripathy - oripathy_stage * 20) * 5;
    let target = original_target + target_mod;

    let times_mod_text = if times_mod > 0 {
        format!("+{times_mod}")
    } else {
        String::new()
    };
    let target_mod_text = if target_mod > 0 {
        format!("+{target_mod}")
    } else {
        String::new()
    };
    let dice_and_target_text = format!(
        "ダイス数{original_times}{times_mod_text}、判定値{original_target}{target_mod_text}"
    );

    let mut result_texts = vec![
        format!("({command})"),
        dice_and_target_text,
        format!("{times}B100<={target}"),
    ];

    if target <= 0 {
        result_texts.push("自動失敗！".to_owned());
        return Ok(EvalResult::failure(result_texts.join(" ＞ ")));
    }

    let dice_list = roll_sorted(rng, times, 100)?;
    let success_count = dice_list.iter().filter(|n| **n <= target).count() as i64;

    result_texts.push(format!("[{}]", join_dice(&dice_list)));
    result_texts.push(format!("成功数{success_count}"));
    if success_count > 0 {
        result_texts.push("成功".to_owned());
        Ok(EvalResult::success(result_texts.join(" ＞ ")))
    } else {
        result_texts.push("失敗".to_owned());
        Ok(EvalResult::failure(result_texts.join(" ＞ ")))
    }
}

/// Ruby `WORSENING_TABLE`。
const WORSENING_TABLE: [&str; 3] = ["末梢神経障害", "内臓機能不全", "精神症状"];

/// Ruby `ArknightsFan#eval_worsening`。文字列を返す（`Result` ではない）。
fn eval_worsening(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "--WORSENING" {
        return Ok(None);
    }

    let value = rng.roll_once(3)?;
    let chosen = WORSENING_TABLE[(value - 1) as usize];
    let elapse = rng.roll_once(6)? + 1;

    Ok(Some(format!("--WORSENING ＞ {chosen}: {elapse} rounds")))
}

/// Ruby `ADDICTION_TABLE`（ヘルプ文の「急性精神反応」ではなく表の「急性精神症状」）。
const ADDICTION_TABLE: [&str; 3] = ["脳神経障害", "多臓器不全", "急性精神症状"];

/// Ruby `ArknightsFan#eval_addiction`。文字列を返す（`Result` ではない）。
fn eval_addiction(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "--ADDICTION" {
        return Ok(None);
    }

    let value = rng.roll_once(3)?;
    let chosen = ADDICTION_TABLE[(value - 1) as usize];

    Ok(Some(format!("--ADDICTION ＞ {chosen}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ArknightsFan",
            "ArknightsFan.toml",
            95,
        );
    }
}
