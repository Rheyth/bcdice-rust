//! P4で手書き移植した `lib/bcdice/game_system/Sengensyou.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Sengensyou#eval_game_system_specific_command`（命中判定・回避判定 `SGS`）

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `Sengensyou#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Command::Parser.new('SGS', round_type: @round_type).restrict_cmp_op_to(nil)
    let parser = Parser::new(&["SGS"], RoundType::Floor).restrict_cmp_op_to(&[None]);
    let Some(command) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_list = rng.roll_barabara(3, 6)?;
    let dice_total: i64 = dice_list.iter().sum();
    let is_critical = dice_total >= 16;
    let is_fumble = dice_total <= 5;

    let modify_number = command.modify_number;
    let mut sequence = vec![
        format!("(3D6{})", modifier(&modify_number)),
        format!(
            "{dice_total}[{}]",
            dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
    ];
    // Ruby: modify_text は修正値が0のとき nil（＝`.compact` で落ちる）
    if modify_number != I::ZERO {
        sequence.push(format!("{dice_total}{}", modifier(&modify_number)));
    }
    sequence.push((dice_total + modify_number).to_string());
    if is_critical {
        sequence.push("クリティカル".to_owned());
    } else if is_fumble {
        sequence.push("ファンブル".to_owned());
    }

    // Ruby: `r.critical = ` / `r.fumble = ` は成功・失敗のフラグを立てない。
    // `Result.critical` / `Result.fumble` を使うと success / failure まで立つので使わない。
    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.critical = is_critical;
    result.fumble = is_fumble;

    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `BCDice::GameSystem::Sengensyou`（ID: `Sengensyou`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sengensyou;

impl GameSystem for Sengensyou {
    fn id(&self) -> &'static str {
        "Sengensyou"
    }

    fn name(&self) -> &'static str {
        "千幻抄"
    }

    fn sort_key(&self) -> &'static str {
        "せんけんしよう"
    }

    fn help_message(&self) -> &'static str {
        r"・SGS　命中判定・回避判定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SGS"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
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
            .join("test/data/Sengensyou.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Sengensyou.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Sengensyou.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Sengensyou.toml must parse");
        assert_eq!(
            data.tests.len(),
            7,
            "case count in test/data/Sengensyou.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Sengensyou",
                "unexpected game system in Sengensyou.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Sengensyou"), &tc.input, &mut src) {
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
                    "FAIL Sengensyou:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Sengensyou cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
