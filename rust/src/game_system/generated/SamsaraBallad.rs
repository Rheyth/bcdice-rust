//! P4で手書き移植した `lib/bcdice/game_system/SamsaraBallad.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `SamsaraBallad#eval_game_system_specific_command`
//!   （`SB` の通常D100ロール / `SBS` のスワップロール）
//! - `#compare` / `#fumble_?` / `#critical_?`（F値・C値による判定）

use std::sync::OnceLock;

use crate::command_parser::{Parsed, Parser};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::SamsaraBallad`（ID: `SamsaraBallad`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamsaraBallad;

impl GameSystem for SamsaraBallad {
    fn id(&self) -> &'static str {
        "SamsaraBallad"
    }

    fn name(&self) -> &'static str {
        "サンサーラ・バラッド"
    }

    fn sort_key(&self) -> &'static str {
        "さんさあらはらつと"
    }

    fn help_message(&self) -> &'static str {
        r"SB	 通常のD100ロールを行う
SBS	 スワップロールでD100ロールを行う
SB#x@y	 F値をx、C値をyとして通常のD100ロールを行う
SBS#x@y	 F値をx、C値をyとしてスワップロールでD100ロールを行う

例：
SB<=85 通常の技能で成功率85%の判定
SBS<=70 習熟を得た技能で成功率70%の判定
SBS#3@7<=80 習熟を得た技能で、F値3、C値7で成功率80%の攻撃判定
SB<57 通常の技能で、能動側の達成値が57の受動判定
SBS<70 習熟を得た技能で、能動側の達成値が70の受動判定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SBS?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SamsaraBallad#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `SamsaraBallad#eval_game_system_specific_command` 本体。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        // Ruby: Command::Parser.new('SBS', 'SB', round_type: round_type)
        //         .enable_critical.enable_fumble.restrict_cmp_op_to(nil, :<=, :<)
        Parser::new(&["SBS", "SB"], RoundType::Floor)
            .enable_critical()
            .enable_fumble()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Le), Some(CmpOp::Lt)])
    });
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let (places_text, total) = if cmd.command == "SB" {
        (None, rng.roll_once(100)?)
    } else {
        let a = rng.roll_once(10)?;
        let b = rng.roll_once(10)?;
        let places_text = format!("{a},{b}");
        // Ruby: places = [a, b].map { |n| n == 10 ? 0 : n }.sort
        let mut places = [a, b].map(|n| if n == 10 { 0 } else { n });
        places.sort_unstable();

        let mut total = places[0] * 10 + places[1];
        if total == 0 {
            total = 100;
        }
        (Some(places_text), total)
    };

    let mut result = compare(total, &cmd);

    let result_str = if result.failure {
        Some("失敗")
    } else if result.success {
        Some("成功")
    } else {
        None
    };

    let additional_str = if result.fumble {
        Some("ファンブル")
    } else if result.critical {
        Some("クリティカル")
    } else {
        None
    };

    // Ruby: "(D100#{cmd.cmp_op}#{cmd.target_number})"（nil は空文字列になる）
    let cmp_op = cmd.cmp_op.map(CmpOp::symbol_str).unwrap_or_default();
    let target_number = cmd.target_number.map(|n| n.to_string()).unwrap_or_default();

    let mut sequence = vec![format!("(D100{cmp_op}{target_number})")];
    sequence.extend(places_text);
    sequence.push(total.to_string());
    sequence.extend(result_str.map(str::to_owned));
    sequence.extend(additional_str.map(str::to_owned));

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `SamsaraBallad#compare`。テキストは呼び出し側で組み立てる。
fn compare(total: i64, cmd: &Parsed) -> EvalResult {
    match (cmd.cmp_op, cmd.target_number.clone()) {
        // Ruby: [:<=, :<].include?(cmd.cmp_op)
        // （目標値は比較演算子と対でしか入らないので、ここで一緒に取り出す）
        (Some(cmp_op @ (CmpOp::Le | CmpOp::Lt)), Some(target_number)) => {
            if !cmp_op.apply(&crate::Int::from(total), &target_number) {
                EvalResult::failure("")
            } else if is_fumble(total, cmd.fumble.as_ref().map(crate::randomizer::sat_i64)) {
                // Ruby: Result.new.tap { |r| r.success = true; r.fumble = true }
                EvalResult {
                    success: true,
                    fumble: true,
                    ..EvalResult::new()
                }
            } else if is_critical(total, cmd.critical.as_ref().map(crate::randomizer::sat_i64)) {
                EvalResult::critical("")
            } else {
                EvalResult::success("")
            }
        }
        _ => {
            if is_fumble(total, cmd.fumble.as_ref().map(crate::randomizer::sat_i64)) {
                // Ruby: ファンブル優先
                EvalResult {
                    fumble: true,
                    ..EvalResult::new()
                }
            } else if is_critical(total, cmd.critical.as_ref().map(crate::randomizer::sat_i64)) {
                EvalResult {
                    critical: true,
                    ..EvalResult::new()
                }
            } else {
                EvalResult::new()
            }
        }
    }
}

/// Ruby `SamsaraBallad#fumble_?`: `fumble && (total % 10 <= fumble)`。
fn is_fumble(total: i64, fumble: Option<i64>) -> bool {
    fumble.is_some_and(|fumble| total % 10 <= fumble)
}

/// Ruby `SamsaraBallad#critical_?`: `critical && (total % 10 >= critical)`。
fn is_critical(total: i64, critical: Option<i64>) -> bool {
    critical.is_some_and(|critical| total % 10 >= critical)
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
            .join("test/data/SamsaraBallad.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/SamsaraBallad.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/SamsaraBallad.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("SamsaraBallad.toml must parse");
        assert_eq!(
            data.tests.len(),
            55,
            "case count in test/data/SamsaraBallad.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "SamsaraBallad",
                "unexpected game system in SamsaraBallad.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("SamsaraBallad"), &tc.input, &mut src) {
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
                    "FAIL SamsaraBallad:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} SamsaraBallad cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
