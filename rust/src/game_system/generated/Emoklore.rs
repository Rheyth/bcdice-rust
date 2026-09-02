//! P4で手書き移植した `lib/bcdice/game_system/Emoklore.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Emoklore#eval_game_system_specific_command`（`xDM<=y` / `sDAa+z` の振り分け）
//! - `#roll_dm` / `#roll_da` / `#dice_roll` / `#compare_result`
//!
//! # 定型文
//!
//! Ruby側は `I18n.t("Emoklore.…", locale:)` で `i18n/Emoklore/ja_jp.yml` から引く。
//! Rust側は同じ値を `static` として直接持ち、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`Emoklore_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `Emoklore_Korean < Emoklore` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// ロケールごとの定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の定型文。`Emoklore` と `Emoklore_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `Emoklore.success_count`（`%{count}` を成功数で置換する）
    pub(crate) success_count: &'static str,
    /// i18n `Emoklore.double`
    pub(crate) double: &'static str,
    /// i18n `Emoklore.triple`
    pub(crate) triple: &'static str,
    /// i18n `Emoklore.miracle`
    pub(crate) miracle: &'static str,
    /// i18n `Emoklore.catastrophe`
    pub(crate) catastrophe: &'static str,
    /// i18n `Emoklore.dice_count_zero`
    pub(crate) dice_count_zero: &'static str,
    /// i18n `fumble`
    pub(crate) fumble: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
}

/// Ruby `Emoklore::CRITICAL_VALUE`。
const CRITICAL_VALUE: i64 = 1;
/// Ruby `Emoklore::FUMBLE_VALUE`。
const FUMBLE_VALUE: i64 = 10;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `case command when %r{^[-+*/\d]*DM<=[-+*/\d]+}`（振り分け用。末尾は固定しない）。
fn dm_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A[-+*/\d]*DM<=[-+*/\d]+").expect("valid regex"))
}

/// Ruby `case command when /^(B|\d*)DA\d+(\+)?\d*/`（振り分け用）。
fn da_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(B|\d*)DA\d+(\+)?\d*").expect("valid regex"))
}

/// Ruby `%r{^([-+*/\d]+)?DM<=([-+*/\d]+)(E(-?\d+))?$}`。
fn dm_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A([-+*/\d]+)?DM<=([-+*/\d]+)(E(-?\d+))?\z").expect("valid regex")
    })
}

/// Ruby `/^(B|\d+)?DA(\d+)(\+\d+)?$/`。
fn da_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(B|\d+)?DA(\d+)(\+\d+)?\z").expect("valid regex"))
}

/// Ruby `%r{[-+*/]}`。
fn has_operator(s: &str) -> bool {
    s.contains(['-', '+', '*', '/'])
}

/// Ruby `String#to_i`（符号＋先頭の数字列。空なら 0）。`i64` 範囲外は `i64::MAX` に飽和。
fn ruby_to_i(s: &str) -> i64 {
    str_helpers::ruby_to_i(s)
}

/// i18n の `%{count}` 置換。
fn interpolate_count(template: &str, count: i64) -> String {
    template.replace("%{count}", &count.to_string())
}

/// Ruby `Array#to_s`（`inspect`）。`[3, 4, 6, 7]` の形。
fn inspect(values: &[i64]) -> String {
    let body = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

/// Ruby `Emoklore#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if dm_dispatch_pattern().is_match(command) {
        return Ok(roll_dm(sys, command, rng)?.map(SpecificCommandOutput::result));
    }
    if da_dispatch_pattern().is_match(command) {
        return Ok(roll_da(sys, command, rng)?.map(SpecificCommandOutput::result));
    }
    Ok(None)
}

/// Ruby `Emoklore#dice_roll`（ダイスロールの共通処理）。
fn dice_roll(
    sys: &SystemTables,
    num_dice: i64,
    success_threshold: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let values = rng.roll_barabara(num_dice, 10)?;

    // Ruby: 1の目は critical と success で二重に数えられ、10の目は success（判定値が
    // 10以上のとき）と fumble で相殺される。原典の数え方をそのまま写す。
    let critical = values.iter().filter(|v| **v == CRITICAL_VALUE).count() as i64;
    let success = values.iter().filter(|v| **v <= success_threshold).count() as i64;
    let fumble = values.iter().filter(|v| **v == FUMBLE_VALUE).count() as i64;

    let success_value = critical + success - fumble;
    let mut result = compare_result(sys, success_value);

    // Ruby: "#{values} ＞ #{success_value} ＞ #{success_count} #{result.text}"
    //       成功数と結果文言の区切りは半角スペース1つ。
    result.text = format!(
        "{} ＞ {success_value} ＞ {} {}",
        inspect(&values),
        interpolate_count(sys.success_count, success_value),
        result.text
    );
    Ok(result)
}

/// Ruby `Emoklore#compare_result`。
fn compare_result(sys: &SystemTables, success: i64) -> EvalResult {
    if success < 0 {
        EvalResult::fumble(sys.fumble)
    } else if success == 0 {
        EvalResult::failure(sys.failure)
    } else if success == 1 {
        EvalResult::success(sys.success)
    } else if success == 2 {
        EvalResult::critical(sys.double)
    } else if success == 3 {
        EvalResult::critical(sys.triple)
    } else if success <= 9 {
        EvalResult::critical(sys.miracle)
    } else {
        EvalResult::critical(sys.catastrophe)
    }
}

/// Ruby `Emoklore#roll_dm`（技能判定 `xDM<=y`）。
fn roll_dm(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = dm_pattern().captures(command) else {
        return Ok(None);
    };

    let base_dice_str = m.get(1).map(|x| x.as_str());
    let threshold_str = &m[2];
    // Ruby: modifier = m[4]&.to_i（`E0` は 0 だが nil ではないので真値）
    let modifier = m.get(4).map(|x| ruby_to_i(x.as_str()));

    let base_dice = match base_dice_str {
        Some(s) => arithmetic::eval(s, RoundType::Floor)?,
        None => Some(I::ONE),
    };
    let success_threshold = arithmetic::eval(threshold_str, RoundType::Floor)?;
    // Ruby: return nil unless base_dice && success_threshold（0は真値なので通る）
    let (Some(base_dice), Some(success_threshold)) = (base_dice, success_threshold) else {
        return Ok(None);
    };

    let num_dice = match modifier {
        Some(modifier) => base_dice.clone() + modifier,
        None => base_dice.clone(),
    };

    // Ruby: ダイス数が0以下の場合は確定失敗
    if num_dice <= I::ZERO {
        return Ok(Some(EvalResult::fumble(format!(
            "({command}) ＞ {}",
            sys.dice_count_zero
        ))));
    }

    let mut result = dice_roll(
        sys,
        crate::randomizer::sat_i64(&num_dice),
        crate::randomizer::sat_i64(&success_threshold),
        rng,
    )?;

    // Ruby: 算術式やダイスボーナスがある場合は展開形を表示
    let has_arithmetic = base_dice_str.is_some_and(has_operator) || has_operator(threshold_str);
    let values_changed = base_dice_str.is_some_and(|s| s != base_dice.to_string())
        || threshold_str != success_threshold.to_string();
    result.text = if modifier.is_some() || (has_arithmetic && values_changed) {
        format!(
            "({command}) ＞ ({num_dice}DM<={success_threshold}) ＞ {}",
            result.text
        )
    } else {
        format!("({num_dice}DM<={success_threshold}) ＞ {}", result.text)
    };
    Ok(Some(result))
}

/// Ruby `Emoklore#roll_da`（取得技能判定 `sDAa+z`）。
fn roll_da(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = da_pattern().captures(command) else {
        return Ok(None);
    };

    let level = m.get(1).map(|x| x.as_str());
    // Ruby: bonus = m[3].to_i（"+5" → 5、nil → 0）
    let bonus = m.get(3).map_or(0, |x| ruby_to_i(x.as_str()));
    // Ruby: (m[1] == "B" ? 1 : (m[1]&.to_i || 1)) + bonus
    let base = match level {
        Some("B") => 1,
        Some(s) => ruby_to_i(s),
        None => 1,
    };
    let num_dice = base.saturating_add(bonus);
    // Ruby: m[1].to_i + m[2].to_i（"B".to_i も nil.to_i も 0）
    let success_threshold = level.map_or(0, ruby_to_i).saturating_add(ruby_to_i(&m[2]));

    if num_dice <= 0 {
        return Ok(Some(EvalResult::fumble(format!(
            "({command}) ＞ {}",
            sys.dice_count_zero
        ))));
    }

    let mut result = dice_roll(sys, num_dice, success_threshold, rng)?;
    result.text = format!(
        "({command}) ＞ ({num_dice}DM<={success_threshold}) ＞ {}",
        result.text
    );
    Ok(Some(result))
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの定型文
// ---------------------------------------------------------------------------

/// `ja_jp` ロケールの定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    success_count: "成功数%{count}",
    double: "ダブル",
    triple: "トリプル",
    miracle: "ミラクル",
    catastrophe: "カタストロフ",
    dice_count_zero: "ダイス数が0以下 ＞ 確定失敗",
    fumble: "ファンブル",
    failure: "失敗",
    success: "成功",
};

/// Ruby `BCDice::GameSystem::Emoklore`（ID: `Emoklore`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emoklore;

impl GameSystem for Emoklore {
    fn id(&self) -> &'static str {
        "Emoklore"
    }

    fn name(&self) -> &'static str {
        "エモクロアTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "えもくろあTRPG"
    }

    fn help_message(&self) -> &'static str {
        r#"・技能値判定（xDM<=y / xDM<=yEz）
  "(個数)DM<=(判定値)"で指定します。
  ダイスの個数は省略可能で、省略した場合1個になります。
  個数や判定値には四則演算（+-*/）を使用できます。
  末尾にEzを付けるとダイス数にzを加算します。E-zで減算も可能です。
  例）2DM<=5 DM<=8 2+2DM<=5 → 4個で判定値5
      2DM<=5E2 → 4個で判定値5 / 3DM<=5E-1 → 2個で判定値5
  ※ダイス数が0以下になる場合は確定失敗

・技能値判定（sDAa+z)
  "(技能レベル)DA(能力値)+(ダイスボーナス)"で指定します。
  ダイスボーナスの個数は省略可能で、省略した場合0になります。
  技能レベルは1～3の数値、またはベース技能の場合"b"が入ります。
  ダイスの個数は技能レベルとダイスボーナスの個数により決定し、s+z個のダイスを振ります。（s="b"の場合はs=1）
  判定値はs+aとなります。（s="b"の場合はs=0）
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[-+*/\d]*DM<=", r"(B|\d*)DA"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Emoklore#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Emoklore", "Emoklore.toml", 33);
    }
}
