//! P4で手書き移植した `lib/bcdice/game_system/Aoharubaan.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Aoharubaan#roll_judge`（`1D6+m>=t` の判定）
//! - `TABLES`（カレカノ反応表 `KR` / `KReaction`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Aoharubaan`（ID: `Aoharubaan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aoharubaan;

impl GameSystem for Aoharubaan {
    fn id(&self) -> &'static str {
        "Aoharubaan"
    }

    fn name(&self) -> &'static str {
        "あおはるばーんっ"
    }

    fn sort_key(&self) -> &'static str {
        "あおはるはあんつ"
    }

    fn help_message(&self) -> &'static str {
        r"カレカノ反応表（ KR, KReaction ）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(1d6?|d6)(\+\d+)?(>=|=>)(\d+)", "KR", "KREACTION"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Aoharubaan#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: command = ALIAS[command] || command
        let command = ALIAS
            .iter()
            .find(|(from, _)| *from == command)
            .map(|(_, to)| *to)
            .unwrap_or(command);

        if let Some(m) = judge_roll_pattern().captures(command) {
            let result = roll_judge(m.get(2).map(|x| x.as_str()), &m[4], rng)?;
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `JUDGE_ROLL_REG`（`/^(1d6?|d6)(\+\d+)?(>=|=>)(\d+)$/i`）。
fn judge_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(1d6?|d6)(\+\d+)?(>=|=>)(\d+)$").expect("valid regex"))
}

/// Ruby `Aoharubaan#roll_judge`。
fn roll_judge(
    modifier_expression: Option<&str>,
    border_expression: &str,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    // Ruby: modifier_expression ? Arithmetic.eval(modifier_expression, RoundType::FLOOR) : nil
    let modifier = match modifier_expression {
        Some(expr) => arithmetic::eval(expr, RoundType::Floor)?,
        None => None,
    };
    // Ruby: border_expression.to_i
    let border: i64 = border_expression.parse().unwrap_or(i64::MAX);

    let command_text = make_command_text(modifier.as_ref().map(crate::randomizer::sat_i64), border);

    let dice = rng.roll_once(6)?;
    // Ruby: dice + modifier.to_i（nil.to_i は 0）
    let score = dice + modifier.clone().unwrap_or_default();

    let is_success = score >= crate::Int::from(border); // 「成功」か？
    let is_right = is_success && score == crate::Int::from(border); // 「ピタリ賞」か？
    let is_excellent = is_success && score >= crate::Int::from(7); // 「限界突破」か？

    let mut result_elements: Vec<&str> = Vec::new();
    result_elements.push(if is_success { "成功" } else { "失敗" });
    if is_right {
        result_elements.push("ピタリ賞");
    }
    if is_excellent {
        result_elements.push("限界突破");
    }

    let mut message_elements: Vec<String> = Vec::new();
    message_elements.push(command_text);
    if let Some(modifier) = modifier {
        message_elements.push(format!("{dice}+{modifier}"));
    }
    message_elements.push(score.to_string());
    message_elements.push(result_elements.join(" ＆ "));

    let mut result = EvalResult::with_text(message_elements.join(" ＞ "));
    result.set_condition(is_success);
    result.critical = is_right || is_excellent;
    Ok(result)
}

/// Ruby `Aoharubaan#make_command_text`。
fn make_command_text(modifier: Option<i64>, border: i64) -> String {
    let mut command = "1D6".to_owned();
    if let Some(modifier) = modifier {
        command = format!("{command}+{modifier}");
    }
    command = format!("{command}>={border}");
    format!("({command})")
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `ALIAS`（キー・値ともに `upcase` 済み）。
static ALIAS: &[(&str, &str)] = &[("KR", "KREACTION")];

/// Ruby `TABLES["KREACTION"]`（カレカノ反応表）の項目。
static K_REACTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "何となく素っ気ない気がする。"),
    (RangeInc::new(3, 4), "いつもと変わらない安心感。"),
    (
        RangeInc::new(5, 6),
        "何故だかすごくデレてきた！　嬉しくて〈テンション〉１回復。",
    ),
];

/// Ruby `DiceTable::RangeTable.new("カレカノ反応表", "1D6", …)`。
static K_REACTION: RangeTable = RangeTable::from_dice("カレカノ反応表", 1, 6, K_REACTION_ITEMS);

/// Ruby `TABLES`（キーは `transform_keys(&:upcase)` 済み）。
static TABLES: &[(&str, &RangeTable)] = &[("KREACTION", &K_REACTION)];

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
            .join("test/data/Aoharubaan.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Aoharubaan.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Aoharubaan.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Aoharubaan.toml must parse");
        assert_eq!(
            data.tests.len(),
            13,
            "case count in test/data/Aoharubaan.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Aoharubaan",
                "unexpected game system in Aoharubaan.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Aoharubaan"), &tc.input, &mut src) {
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
                    "FAIL Aoharubaan:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Aoharubaan cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
