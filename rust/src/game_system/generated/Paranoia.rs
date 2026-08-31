//! P4で手書き移植した `lib/bcdice/game_system/Paranoia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Paranoia#eval_game_system_specific_command` と `#getaRoll`（`geta`）
//!
//! `@enabled_upcase_input = false` なので、コマンドは小文字のまま
//! `eval_game_system_specific_command` に渡り、そのまま出力へ埋め込まれる。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `case command when /geta/i`。アンカーが無いので部分一致でよい。
fn geta_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)geta").expect("valid regex"))
}

/// Ruby `Paranoia#eval_game_system_specific_command`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let result = if geta_pattern().is_match(command) {
        geta_roll(rng)?
    } else {
        String::new()
    };

    // Ruby: return nil if result.empty?
    if result.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!("{command} ＞ {result}")))
}

/// Ruby `Paranoia#getaRoll`。
fn geta_roll(rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut result = String::new();

    let dice = rng.roll_once(2)?;

    result.push_str("幸福ですか？ ＞ ");

    let geta_string = match dice {
        1 => "幸福です",
        2 => "幸福ではありません",
        _ => "",
    };

    result.push_str(geta_string);

    Ok(result)
}

/// Ruby `BCDice::GameSystem::Paranoia`（ID: `Paranoia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paranoia;

impl GameSystem for Paranoia {
    fn id(&self) -> &'static str {
        "Paranoia"
    }

    fn name(&self) -> &'static str {
        "パラノイア"
    }

    fn sort_key(&self) -> &'static str {
        "はらのいあ"
    }

    fn help_message(&self) -> &'static str {
        r"※「パラノイア」は完璧なゲームであるため特殊なダイスコマンドを必要としません。
※このダイスボットは部屋のシステム名表示用となります。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["geta"]
    }

    crate::impl_prefixes_pattern!();

    fn enabled_upcase_input(&self) -> bool {
        false
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific_command(command, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/Paranoia.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Paranoia.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Paranoia.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Paranoia.toml must parse");
        assert_eq!(data.tests.len(), 9, "case count in test/data/Paranoia.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Paranoia",
                "unexpected game system in Paranoia.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Paranoia"), &tc.input, &mut src) {
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
                    "FAIL Paranoia:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Paranoia cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
