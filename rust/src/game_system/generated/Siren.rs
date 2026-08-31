//! P4で手書き移植した `lib/bcdice/game_system/Siren.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Siren#check_action`（判定 `SL+a<=b±c`）
//! - `Siren#check_training`（育成 `TR$a<=b`）

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Siren`（ID: `Siren`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Siren;

impl GameSystem for Siren {
    fn id(&self) -> &'static str {
        "Siren"
    }

    fn name(&self) -> &'static str {
        "終末アイドル育成TRPG セイレーン"
    }

    fn sort_key(&self) -> &'static str {
        "せいれえん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定: SL+a<=b±c
  a=達成値への修正(0の場合は省略)
  b=能力値
  c=判定への修正(0の場合は省略、複数可)
例)判定修正-10の装備を装着しながら【技術：60】〈兵器：2〉で判定する場合。
SL+2<=60+40-10

・育成: TR$a<=b
  a=育成した回数
  b=ヘルス
例）ヘルスの現在値が60で2回目の【身体】の育成を行う場合。
TR$2<=60
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SL", "TR"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Siren#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /^SL/ ... when /^TR/ ... else return nil
        //       （`/i` 無しだが `dice_command` が大文字化済みの文字列を渡す）
        if command.starts_with("SL") {
            return check_action(command, rng);
        }
        if command.starts_with("TR") {
            return check_training(command, rng);
        }
        Ok(None)
    }
}

/// Ruby `dig10` / `dig1` の算出。
///
/// 十の位・一の位がそれぞれ0なら10として扱う（1D100の00を100と読む）。
fn digits(dice: i64) -> (i64, i64) {
    let dig10 = dice / 10;
    let dig1 = dice % 10;
    (
        if dig10 == 0 { 10 } else { dig10 },
        if dig1 == 0 { 10 } else { dig1 },
    )
}

/// Ruby `Siren#check_action`（判定）。
fn check_action(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('SL', round_type: @round_type).restrict_cmp_op_to(:<=)
    //       `@round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["SL"], RoundType::Floor).restrict_cmp_op_to(&[Some(CmpOp::Le)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `restrict_cmp_op_to(:<=)` が目標値省略を許さないので、必ず値がある。
    let Some(target) = parsed.target_number else {
        return Ok(None);
    };

    let dice = rng.roll_once(100)?;

    if dice > crate::randomizer::sat_i64(&target) {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
            format!("(1D100<={target}) ＞ {dice} ＞ 失敗"),
        ))));
    }

    let (dig10, dig1) = digits(dice);
    let achievement_value = dig10 + dig1 + parsed.modify_number;
    Ok(Some(SpecificCommandOutput::result(EvalResult::success(
        format!("(1D100<={target}) ＞ {dice} ＞ 成功(達成値：{achievement_value})"),
    ))))
}

/// Ruby `Siren#check_training`（育成）。
fn check_training(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('TR', round_type: @round_type)
    //         .restrict_cmp_op_to(:<=).enable_dollar.disable_modifier
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["TR"], RoundType::Floor)
            .restrict_cmp_op_to(&[Some(CmpOp::Le)])
            .enable_dollar()
            .disable_modifier()
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: count = parsed.dollar; return nil if count.nil?
    let Some(count) = parsed.dollar else {
        return Ok(None);
    };

    let Some(target) = parsed.target_number else {
        return Ok(None);
    };

    let dice = rng.roll_once(100)?;

    let (dig10, dig1) = digits(dice);
    let achievement_value = dig10 + dig1;

    if dice > crate::randomizer::sat_i64(&target) {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
            format!(
                "(1D100<={target}) ＞ {dice} ＞ 失敗(能力値減少：10 / ヘルス減少：{achievement_value})"
            ),
        ))));
    }

    Ok(Some(SpecificCommandOutput::result(EvalResult::success(
        format!(
            "(1D100<={target}) ＞ {dice} ＞ 成功(能力値上昇：{} / ヘルス減少：{achievement_value})",
            count * 5 + achievement_value
        ),
    ))))
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
            .join("test/data/Siren.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Siren.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Siren.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Siren.toml must parse");
        assert_eq!(data.tests.len(), 19, "case count in test/data/Siren.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Siren",
                "unexpected game system in Siren.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Siren"), &tc.input, &mut src) {
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
                    "FAIL Siren:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Siren cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
