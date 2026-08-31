//! P4で手書き移植した `lib/bcdice/game_system/OrgaRain.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `OrgaRain#eval_game_system_specific_command` と `#check_roll`（判定 `[n]OR(count)`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `/(\d+)?OR(\d{0,6})$/i`。
///
/// Rubyの `$` は行末にもマッチするが、`Preprocessor` が最初の空白より前しか
/// 残さないため入力に改行は無く、Rustの「文字列末尾」と同じ意味になる。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)?OR(\d{0,6})$").expect("valid regex"))
}

/// Ruby `OrgaRain#eval_game_system_specific_command`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: (m[1] || 1).to_i
    let dice_count = m.get(1).map_or(1, |x| to_i(x.as_str()));
    // Ruby: (m[2] || "").each_char.map(&:to_i).sort
    let mut count_no: Vec<i64> = m
        .get(2)
        .map_or("", |x| x.as_str())
        .chars()
        .map(|c| i64::from(c.to_digit(10).unwrap_or(0)))
        .collect();
    count_no.sort_unstable();

    Ok(Some(check_roll(dice_count, &count_no, rng)?))
}

/// Ruby `OrgaRain#check_roll`。
fn check_roll(
    dice_count: i64,
    count_no: &[i64],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut dice_array = rng.roll_barabara(dice_count, 10)?;
    dice_array.sort_unstable();
    let dice_text = join_values(&dice_array);

    let mut result_array: Vec<String> = Vec::new();
    let mut success = 0i64;
    // Ruby: dice_array.map { |x| x == 10 ? 0 : x }（10は命数0として扱う）
    for i in dice_array.iter().map(|&x| if x == 10 { 0 } else { x }) {
        let multiple = count_no.iter().filter(|&&c| c == i).count() as i64;
        if multiple > 0 {
            result_array.push(format!("{i}(x{multiple})"));
            success += multiple;
        } else {
            result_array.push("×".to_owned());
        }
    }

    let count_text = join_values(count_no);
    let result_text = result_array.join(",");

    Ok(format!(
        "{dice_count}D10(命数：{count_text}) ＞ {dice_text} ＞ {result_text} ＞ 成功数：{success}"
    ))
}

/// Ruby `String#to_i`。i64に収まらない値は飽和させる（Rubyでは Bignum）。
fn to_i(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `Array#join(',')`。
fn join_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::OrgaRain`（ID: `OrgaRain`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrgaRain;

impl GameSystem for OrgaRain {
    fn id(&self) -> &'static str {
        "OrgaRain"
    }

    fn name(&self) -> &'static str {
        "在りて遍くオルガレイン"
    }

    fn sort_key(&self) -> &'static str {
        "ありてあまねくおるかれいん"
    }

    fn help_message(&self) -> &'static str {
        r"判定：[n]OR(count)

[]内のコマンドは省略可能。
「n」でダイス数を指定。省略時は「1」。
(count)で命数を指定。「3111」のように記述。最大6つ。順不同可。

【書式例】
・5OR6042 → 5dで命数「0,2,4,6」の判定
・6OR33333 → 6dで命数「3,3,3,3,3」の判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?OR(\d{0,6})?"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
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
            .join("test/data/OrgaRain.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/OrgaRain.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/OrgaRain.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("OrgaRain.toml must parse");
        assert_eq!(data.tests.len(), 7, "case count in test/data/OrgaRain.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "OrgaRain",
                "unexpected game system in OrgaRain.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("OrgaRain"), &tc.input, &mut src) {
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
                    "FAIL OrgaRain:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} OrgaRain cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
