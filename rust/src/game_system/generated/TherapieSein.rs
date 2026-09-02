//! P4で手書き移植した `lib/bcdice/game_system/TherapieSein.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TherapieSein#eval_game_system_specific_command`（一般判定 `TS` / 戦闘判定 `OP`）と
//!   `checkRoll` / `getValueText`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::TherapieSein`（ID: `TherapieSein`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TherapieSein;

impl GameSystem for TherapieSein {
    fn id(&self) -> &'static str {
        "TherapieSein"
    }

    fn name(&self) -> &'static str {
        "セラフィザイン"
    }

    fn sort_key(&self) -> &'static str {
        "せらふいさいん"
    }

    fn help_message(&self) -> &'static str {
        r"・一般判定：TS[n][±m][@t]　　[]内のコマンドは省略可能。クリティカル無。
・戦闘判定：OP[n][±m][@t]　　[]内のコマンドは省略可能。クリティカル有。

「n」で能力値修正などを指定。
「±m」で達成値への修正値を追加指定。+5+1-3のように、複数指定も可能です。
「@t」で目標値を指定。省略時は達成値のみ表示、指定時は判定の正否を追加表示。

【書式例】
・TS → ダイスの合計値を達成値として表示。
・TS4 → ダイス合計+4を達成値表示。
・TS4-1 → ダイス合計+4-1（計+3）を達成値表示。
・TS2+1@10 → ダイス合計+2+1（計+3）の達成値と、判定の成否を表示。
・OP4+3+1 → ダイス合計+4+3+1（計+8）を達成値＆クリティカル表示。
・OP3@12 → ダイス合計+3の達成値＆クリティカル、判定の成否を表示。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["TS", "OP"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `TherapieSein#eval_game_system_specific_command`。
    ///
    /// 原典は `command.upcase` してから `case` に掛けるが、`Base#dice_command` が
    /// `@enabled_upcase_input` に従って既に大文字化しているので二重適用にあたる。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: when /(TS|OP)(\d+)?(([+-]\d+)*)(@(\d+))?$/i
        let Some(captures) = command_pattern().captures(command) else {
            return Ok(None);
        };

        let has_critical = &captures[1] == "OP";
        // Ruby: (Regexp.last_match(6) || 0).to_i
        let target = captures.get(6).map_or(0, |m| to_i(m.as_str()));
        // Ruby: (Regexp.last_match(2) || 0).to_i
        let mut modify = captures.get(2).map_or(0, |m| to_i(m.as_str()));
        let modify_add_string = captures.get(3).map_or("", |m| m.as_str());

        // Ruby: modifyAddString.scan(/[+-]\d+/).each { |i| modify += i.to_i }
        for m in modifier_pattern().find_iter(modify_add_string) {
            modify = modify.saturating_add(to_i(m.as_str()));
        }

        Ok(Some(SpecificCommandOutput::text(check_roll(
            has_critical,
            modify,
            target,
            rng,
        )?)))
    }
}

/// Ruby `/(TS|OP)(\d+)?(([+-]\d+)*)(@(\d+))?$/i`。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(TS|OP)(\d+)?(([+-]\d+)*)(@(\d+))?$").expect("valid regex"))
}

/// Ruby `/[+-]\d+/`（`String#scan` 用）。
fn modifier_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[+-]\d+").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `TherapieSein#checkRoll`。
fn check_roll(
    has_critical: bool,
    modify: i64,
    target: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_list = rng.roll_barabara(2, 6)?;
    let dice: i64 = dice_list.iter().sum();
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let success_value = dice.saturating_add(modify);

    let modify_text = get_value_text(modify);
    // Ruby: target == 0 ? '' : ">=#{target}"
    let target_text = if target == 0 {
        String::new()
    } else {
        format!(">={target}")
    };

    let mut result = format!("(2D6{modify_text}{target_text})");
    result.push_str(&format!(" ＞ {dice}({dice_text}){modify_text}"));

    if has_critical && dice == 12 {
        result.push_str(" ＞ クリティカル！");
        return Ok(result);
    }

    result.push_str(&format!(" ＞ {success_value}{target_text}"));

    if target == 0 {
        return Ok(result);
    }

    if success_value >= target {
        result.push_str(" ＞ 【成功】");
    } else {
        result.push_str(" ＞ 【失敗】");
    }

    Ok(result)
}

/// Ruby `TherapieSein#getValueText`。
fn get_value_text(value: i64) -> String {
    if value == 0 {
        return String::new();
    }
    if value < 0 {
        return value.to_string();
    }
    // Ruby: "\+#{value}"（ダブルクォート内の `\+` はただの `+`）
    format!("+{value}")
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
            .join("test/data/TherapieSein.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TherapieSein.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TherapieSein.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TherapieSein.toml must parse");
        assert_eq!(
            data.tests.len(),
            12,
            "case count in test/data/TherapieSein.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TherapieSein",
                "unexpected game system in TherapieSein.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("TherapieSein"), &tc.input, &mut src) {
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
                    "FAIL TherapieSein:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TherapieSein cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
