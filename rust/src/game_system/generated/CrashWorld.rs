//! P4で手書き移植した `lib/bcdice/game_system/CrashWorld.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `CrashWorld#eval_game_system_specific_command` → `get_crash_world_roll`（判定 `CWn`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::CrashWorld`（ID: `CrashWorld`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashWorld;

impl GameSystem for CrashWorld {
    fn id(&self) -> &'static str {
        "CrashWorld"
    }

    fn name(&self) -> &'static str {
        "墜落世界"
    }

    fn sort_key(&self) -> &'static str {
        "ついらくせかい"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 CWn
初期目標値n (必須)
例・CW8
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CW"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `CrashWorld#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /CW(\d+)/i
        let Some(captures) = command_pattern().captures(command) else {
            return Ok(None);
        };

        let target = to_i(&captures[1]);
        Ok(Some(SpecificCommandOutput::text(get_crash_world_roll(
            target, rng,
        )?)))
    }
}

/// Ruby `/CW(\d+)/i`。前後を固定していないので部分一致でよい（原典どおり）。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)CW(\d+)").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 目標値が12以上の場合、Ruby も本移植も「必ず成功が続く」ため
/// 乱数を使い切る（テストでは注入乱数の枯渇、本番では無限ループ）まで回る。
/// 飽和させてもこの分岐は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `CrashWorld#getCrashWorldRoll`。
fn get_crash_world_roll(mut target: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut output = String::from("(");
    let mut is_end = false;
    let mut successness = 0i64;
    let mut num = 0i64;

    while !is_end {
        num = rng.roll_once(12)?;

        // 振った数字を出力へ書き足す
        if output == "(" {
            output = format!("({num}");
        } else {
            output = format!("{output}, {num}");
        }

        if num <= target || num == 11 {
            // 成功/クリティカル(11)。 次回の目標値を変更して継続
            target = num;
            successness += 1;
        } else if num == 12 {
            // ファンブルなら終了。
            is_end = true;
        } else {
            // target < num < 11で終了
            is_end = true;
        }
    }

    if num == 12 {
        // ファンブルの時、成功度は0
        successness = 0;
    }

    output = format!("{output})  成功度 : {successness}");

    if num == 12 {
        output = format!("{output} ファンブル");
    }

    Ok(output)
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
            .join("test/data/CrashWorld.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/CrashWorld.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/CrashWorld.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("CrashWorld.toml must parse");
        assert_eq!(
            data.tests.len(),
            11,
            "case count in test/data/CrashWorld.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "CrashWorld",
                "unexpected game system in CrashWorld.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("CrashWorld"), &tc.input, &mut src) {
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
                    "FAIL CrashWorld:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} CrashWorld cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
