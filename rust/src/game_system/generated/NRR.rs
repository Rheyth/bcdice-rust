//! P4で手書き移植した `lib/bcdice/game_system/NRR.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NRR#roll_nr`（判定 `xNR6` / `xNR8` / `xNR10` / `xNR12`）
//! - 判定表（`DISADVANTAGE` / `NORMAL` / `ADVANTAGE` / `EXTRA`）と
//!   `ICON` / `RESULT_LABEL`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::NRR`（ID: `NRR`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NRR;

impl GameSystem for NRR {
    fn id(&self) -> &'static str {
        "NRR"
    }

    fn name(&self) -> &'static str {
        "nRR"
    }

    fn sort_key(&self) -> &'static str {
        "えぬああるあある"
    }

    fn help_message(&self) -> &'static str {
        r"▪️判定
・ノーマルダイス　NR8
・有利ダイス　NR10
・不利ダイス　NR6
・Exダイス　NR12

ダイスの個数を指定しての判定ができます。
例：有利ダイス2個で判定　2NR10

▪️判定結果とシンボル
⭕：成功
❌：失敗
✨：クリティカル（大成功）
💀：ファンブル（大失敗）
🌈：ミラクル（奇跡）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*NR(6|8|10|12)"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `NRR#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_nr(command, rng)?.map(SpecificCommandOutput::result))
    }
}

/// 判定結果の段階。Ruby側は `:fumble` などのシンボル。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Fumble,
    Failure,
    Success,
    Critical,
    Miracle,
}

impl Level {
    /// Ruby `ICON`。`success` の絵文字だけ異体字セレクタ（U+FE0F）が付く。
    fn icon(self) -> &'static str {
        match self {
            Level::Fumble => "💀",
            Level::Failure => "❌",
            Level::Success => "⭕️",
            Level::Critical => "✨",
            Level::Miracle => "🌈",
        }
    }

    /// Ruby `RESULT_LABEL`。
    fn label(self) -> &'static str {
        match self {
            Level::Fumble => "ファンブル（大失敗）",
            Level::Failure => "失敗",
            Level::Success => "成功",
            Level::Critical => "クリティカル（大成功）",
            Level::Miracle => "ミラクル（奇跡）",
        }
    }

    /// Ruby `SUCCESSES.include?(level)`。
    fn is_success(self) -> bool {
        matches!(self, Level::Success | Level::Critical | Level::Miracle)
    }

    /// Ruby `CRITICALS.include?(level)`。
    fn is_critical(self) -> bool {
        matches!(self, Level::Critical | Level::Miracle)
    }
}

/// Ruby `LEVELS`（複数ダイス時の集計の並び順）。
static LEVELS: &[Level] = &[
    Level::Fumble,
    Level::Failure,
    Level::Success,
    Level::Critical,
    Level::Miracle,
];

/// Ruby `DISADVANTAGE`（不利ダイス `NR6`）。
static DISADVANTAGE: &[Level] = &[
    Level::Fumble,
    Level::Failure,
    Level::Failure,
    Level::Failure,
    Level::Success,
    Level::Success,
];

/// Ruby `NORMAL`（ノーマルダイス `NR8`）。
static NORMAL: &[Level] = &[
    Level::Fumble,
    Level::Failure,
    Level::Failure,
    Level::Failure,
    Level::Success,
    Level::Success,
    Level::Success,
    Level::Critical,
];

/// Ruby `ADVANTAGE`（有利ダイス `NR10`）。
static ADVANTAGE: &[Level] = &[
    Level::Fumble,
    Level::Failure,
    Level::Failure,
    Level::Success,
    Level::Success,
    Level::Success,
    Level::Success,
    Level::Success,
    Level::Critical,
    Level::Critical,
];

/// Ruby `EXTRA`（Exダイス `NR12`）。
static EXTRA: &[Level] = &[
    Level::Fumble,
    Level::Fumble,
    Level::Failure,
    Level::Failure,
    Level::Success,
    Level::Success,
    Level::Critical,
    Level::Critical,
    Level::Critical,
    Level::Critical,
    Level::Miracle,
    Level::Miracle,
];
/// Ruby `/^(\d+)?NR(6|8|10|12)$/`（大文字小文字を区別しない指定はない）。
fn roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)?NR(6|8|10|12)$").expect("valid regex"))
}

/// Ruby `NRR#roll_nr`。
fn roll_nr(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = roll_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1]&.to_i || 1
    // 桁あふれする入力は Ruby では Bignum になり roll_barabara が TooManyRandsError を
    // 上げる。i64 に収まらない場合も同じ経路へ落ちるように飽和させる。
    let times: i64 = match m.get(1) {
        Some(digits) => digits.as_str().parse().unwrap_or(i64::MAX),
        None => 1,
    };

    let sides_text = m.get(2).expect("group 2 always matches").as_str();
    let table: &'static [Level] = match sides_text {
        "6" => DISADVANTAGE,
        "8" => NORMAL,
        "10" => ADVANTAGE,
        _ => EXTRA,
    };

    let values = rng.roll_barabara(times, table.len() as i64)?;
    let mut result = EvalResult::new();
    let text = if times == 1 {
        let level = table[(values[0] - 1) as usize];
        result.set_condition(level.is_success());
        result.fumble = level == Level::Fumble;
        result.critical = level.is_critical();

        format!("{} {}", level.icon(), level.label())
    } else {
        let levels: Vec<Level> = values.iter().map(|&v| table[(v - 1) as usize]).collect();

        // Ruby: LEVELS.map { count == 0 ? nil : "#{ICON[l]} #{count}" }.compact.join(", ")
        LEVELS
            .iter()
            .filter_map(|level| {
                let count = levels.iter().filter(|l| *l == level).count();
                (count != 0).then(|| format!("{} {count}", level.icon()))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    // Ruby: times_str = times == 1 ? nil : times（nil は空文字列に補間される）
    let times_str = if times == 1 {
        String::new()
    } else {
        times.to_string()
    };
    result.text = format!(
        "({times_str}NR{sides_text}) ＞ {} ＞ {text}",
        values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("NRR", "NRR.toml", 15);
    }
}
