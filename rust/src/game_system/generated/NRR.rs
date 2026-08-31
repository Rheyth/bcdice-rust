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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/NRR.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NRR.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/NRR.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NRR.toml must parse");
        assert_eq!(data.tests.len(), 15, "case count in test/data/NRR.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "NRR", "unexpected game system in NRR.toml");

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("NRR"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL NRR:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NRR cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
