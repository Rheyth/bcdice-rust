//! P4で手書き移植した `lib/bcdice/game_system/KinAriel.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KinAriel#resolute_action`（判定 `KA<=t`）
//! - `KinAriel#resolute_competition`（対抗判定 `VS<=t`）と `get_roll_result`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::KinAriel`（ID: `KinAriel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KinAriel;

impl GameSystem for KinAriel {
    fn id(&self) -> &'static str {
        "KinAriel"
    }

    fn name(&self) -> &'static str {
        "キナリエル"
    }

    fn sort_key(&self) -> &'static str {
        "きなりえる"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　KA<=t            t: 目標値

例)KA<=50: 目標値50で結果を表示(クリティカル、ファンブル、成功、失敗)

■対抗判定　VS<=t        t: 目標値

例)VS<=50: 目標値50で最大5回振って、その結果を表示。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["KA", "VS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `KinAriel#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: resolute_action(command) || resolute_competition(command)
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = resolute_competition(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `/KA<=(\d+)/`。前後を固定していないので部分一致でよい（原典どおり）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"KA<=(\d+)").expect("valid regex"))
}

/// Ruby `/VS<=(\d+)/`。前後を固定していないので部分一致でよい（原典どおり）。
fn competition_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"VS<=(\d+)").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `KinAriel#resolute_action`（通常判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let target = to_i(&captures[1]);
    let dice = rng.roll_once(100)?;

    let mut output = format!("(KA<={target}) ＞ [{dice}]");

    // クリティカル判定は成功した場合の中、ファンブル判定は失敗した場合の中にある
    // （原典の入れ子構造をそのまま保つ）。
    if dice <= target {
        if dice <= 5 {
            output.push_str(" ＞ クリティカル");
            Ok(Some(EvalResult::critical(output)))
        } else {
            output.push_str(" ＞ 成功");
            Ok(Some(EvalResult::success(output)))
        }
    } else if dice >= 96 {
        output.push_str(" ＞ ファンブル");
        Ok(Some(EvalResult::fumble(output)))
    } else {
        output.push_str(" ＞ 失敗");
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `KinAriel#resolute_competition`（対抗判定）。
fn resolute_competition(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = competition_pattern().captures(command) else {
        return Ok(None);
    };

    let target = to_i(&captures[1]);
    let mut output = format!("(VS<={target}) ＞ ");
    let mut dice_arr: Vec<i64> = Vec::new();
    // Ruby: result = Result.new（5回とも回るので、この初期値は出力に出ない）
    let mut result = EvalResult::new();

    for _ in 0..5 {
        let dice = rng.roll_once(100)?;
        dice_arr.push(dice);
        result = get_roll_result(dice, target);

        // Result.fumble は failure も立てるので、この判定順は入れ替えられない。
        let suffix = if result.critical {
            "クリティカル"
        } else if result.fumble {
            "ファンブル"
        } else if result.failure {
            "失敗"
        } else {
            continue;
        };

        output.push_str(&format!(
            "[{}] ＞ {}回目で{}",
            join_comma(&dice_arr),
            dice_arr.len(),
            suffix
        ));
        result.text = output;
        return Ok(Some(result));
    }

    output.push_str(&format!(
        "[{}] ＞ {}回成功",
        join_comma(&dice_arr),
        dice_arr.len()
    ));
    result.text = output;
    Ok(Some(result))
}

/// Ruby `KinAriel#get_roll_result`。
fn get_roll_result(dice: i64, target: i64) -> EvalResult {
    if dice <= target {
        if dice <= 5 {
            EvalResult::critical("")
        } else {
            EvalResult::success("")
        }
    } else if dice >= 96 {
        EvalResult::fumble("")
    } else {
        EvalResult::failure("")
    }
}

/// Ruby `Array#join(",")`。
fn join_comma(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
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
            .join("test/data/KinAriel.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/KinAriel.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/KinAriel.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KinAriel.toml must parse");
        assert_eq!(
            data.tests.len(),
            11,
            "case count in test/data/KinAriel.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KinAriel",
                "unexpected game system in KinAriel.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KinAriel"), &tc.input, &mut src) {
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
                    "FAIL KinAriel:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KinAriel cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
