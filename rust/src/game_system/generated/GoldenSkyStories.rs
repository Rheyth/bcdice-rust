//! P4で手書き移植した `lib/bcdice/game_system/GoldenSkyStories.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GoldenSkyStories#eval_game_system_specific_command`（下駄占い `GETA`）
//! - `#getaRoll`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `case command when /geta/i`（部分一致・大文字小文字を無視）。
fn geta_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)geta").expect("valid regex"))
}

/// Ruby `GoldenSkyStories#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if !geta_pattern().is_match(command) {
        // Ruby: result が '' のままなので nil
        return Ok(None);
    }

    // Ruby: getaRoll は定型文を必ず含むので `result.empty?` にはならない
    let result = geta_roll(rng)?;
    Ok(Some(SpecificCommandOutput::text(format!(
        "{command} ＞ {result}"
    ))))
}

/// Ruby `GoldenSkyStories#getaRoll`。
fn geta_roll(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice = rng.roll_once(7)?;

    // Ruby: case dice ... else '' （1〜7以外は空文字列のまま）
    let geta_string = match dice {
        1 => "裏：あめ",
        2 => "表：はれ",
        3 => "裏：あめ",
        4 => "表：はれ",
        5 => "裏：あめ",
        6 => "表：はれ",
        7 => "横：くもり",
        _ => "",
    };

    Ok(format!("下駄占い ＞ {geta_string}"))
}

/// Ruby `BCDice::GameSystem::GoldenSkyStories`（ID: `GoldenSkyStories`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenSkyStories;

impl GameSystem for GoldenSkyStories {
    fn id(&self) -> &'static str {
        "GoldenSkyStories"
    }

    fn name(&self) -> &'static str {
        "ゆうやけこやけ"
    }

    fn sort_key(&self) -> &'static str {
        "ゆうやけこやけ"
    }

    fn help_message(&self) -> &'static str {
        r"※「ゆうやけこやけ」はダイスロールを使用しないシステムです。
※このダイスボットは部屋のシステム名表示用となります。

・下駄占い (GETA)
  あーしたてんきになーれ
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["geta"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `GoldenSkyStories#initialize` の `@enabled_upcase_input = false`。
    fn enabled_upcase_input(&self) -> bool {
        false
    }

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
            .join("test/data/GoldenSkyStories.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/GoldenSkyStories.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/GoldenSkyStories.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("GoldenSkyStories.toml must parse");
        assert_eq!(
            data.tests.len(),
            6,
            "case count in test/data/GoldenSkyStories.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "GoldenSkyStories",
                "unexpected game system in GoldenSkyStories.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("GoldenSkyStories"), &tc.input, &mut src) {
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
                    "FAIL GoldenSkyStories:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} GoldenSkyStories cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
