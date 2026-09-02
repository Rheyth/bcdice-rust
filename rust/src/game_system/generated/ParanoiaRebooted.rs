//! P4で手書き移植した `lib/bcdice/game_system/ParanoiaRebooted.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ParanoiaRebooted#get_node_dice_roll`（ノードダイス判定 `NDx`）
//! - `ParanoiaRebooted#get_mutant_power_roll`（ミュータントパワー判定 `MPx`）
//! - `ParanoiaRebooted#generate_roll_results`（コンピュータダイスの表示）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::ParanoiaRebooted`（ID: `ParanoiaRebooted`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParanoiaRebooted;

impl GameSystem for ParanoiaRebooted {
    fn id(&self) -> &'static str {
        "ParanoiaRebooted"
    }

    fn name(&self) -> &'static str {
        "パラノイア リブーテッド"
    }

    fn sort_key(&self) -> &'static str {
        "はらのいありふうてつと"
    }

    fn help_message(&self) -> &'static str {
        r"※コマンドは入力内容の前方一致で検出しています。
・通常の判定　NDx
　x：ノードダイスの数.マイナスも可.
　ノードダイスの絶対値 + 1個(コンピュータダイス)のダイスがロールされる.
例）ND2　ND-3

・ミュータントパワー判定　MPx
  x：ノードダイスの数.
　ノードダイスの値 + 1個(コンピュータダイス)のダイスがロールされる.
例）MP2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ND", "MP"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ParanoiaRebooted#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /^ND/i ... when /^MP/i ... else nil
        if starts_with_ignore_ascii_case(command, "ND") {
            return Ok(Some(get_node_dice_roll(command, rng)?));
        }
        if starts_with_ignore_ascii_case(command, "MP") {
            return Ok(Some(get_mutant_power_roll(command, rng)?));
        }
        Ok(None)
    }
}

/// Ruby `/^ND/i` のような「先頭固定・大文字小文字無視」の判定。
fn starts_with_ignore_ascii_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Ruby `/^ND((-)?\d+)/i`。Rubyの `\d` はASCII限定なので `[0-9]` を使う。
fn node_dice_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^ND((-)?[0-9]+)").expect("valid regex"))
}

/// Ruby `/^MP(\d+)/i`。
fn mutant_power_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^MP([0-9]+)").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i_saturating(text: &str) -> i64 {
    str_helpers::to_i_signed_saturating(text)
}

/// Ruby `ParanoiaRebooted#get_node_dice_roll`。
fn get_node_dice_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<SpecificCommandOutput, EvalError> {
    let Some(m) = node_dice_pattern().captures(command) else {
        // Ruby: return ''（`dice_command` が nil に畳む）
        return Ok(SpecificCommandOutput::text(""));
    };

    let parameter_num = to_i_saturating(&m[1]);
    let dice_count = parameter_num.saturating_abs().saturating_add(1);

    let dices = rng.roll_barabara(dice_count, 6)?;

    let mut success_rate = dices.iter().filter(|d| **d >= 5).count() as i64;
    if parameter_num < 0 {
        success_rate -= dices.iter().filter(|d| **d < 5).count() as i64;
    }

    let (results, computer_dice_message) = generate_roll_results(&dices);

    Ok(SpecificCommandOutput::text(format!(
        "({command}) ＞ [{}] ＞ 成功度{success_rate}{computer_dice_message}",
        results.join(", ")
    )))
}

/// Ruby `ParanoiaRebooted#get_mutant_power_roll`。
fn get_mutant_power_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<SpecificCommandOutput, EvalError> {
    let Some(m) = mutant_power_pattern().captures(command) else {
        // Ruby: return ''（`dice_command` が nil に畳む）
        return Ok(SpecificCommandOutput::text(""));
    };

    let parameter_num = to_i_saturating(&m[1]);
    let dice_count = parameter_num.saturating_abs().saturating_add(1);

    let dices = rng.roll_barabara(dice_count, 6)?;

    let failure_rate = dices.iter().filter(|d| **d == 1).count();
    let message = if failure_rate == 0 {
        "成功".to_owned()
    } else {
        format!("失敗({failure_rate})")
    };

    let (results, computer_dice_message) = generate_roll_results(&dices);

    Ok(SpecificCommandOutput::text(format!(
        "({command}) ＞ [{}] ＞ {message}{computer_dice_message}",
        results.join(", ")
    )))
}

/// Ruby `ParanoiaRebooted#generate_roll_results`。
///
/// 最後のダイス（コンピュータダイス）が6なら出目を `C` に置き換えて
/// `(Computer)` を添える。
fn generate_roll_results(dices: &[i64]) -> (Vec<String>, &'static str) {
    let mut results: Vec<String> = dices.iter().map(|d| d.to_string()).collect();

    // Ruby: results[-1].to_i == 6（空配列なら nil.to_i で 0 なので偽）
    if dices.last() == Some(&6) {
        let last = results.len() - 1;
        results[last] = "C".to_owned();
        return (results, "(Computer)");
    }

    (results, "")
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
            .join("test/data/ParanoiaRebooted.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ParanoiaRebooted.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ParanoiaRebooted.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ParanoiaRebooted.toml must parse");
        assert_eq!(
            data.tests.len(),
            13,
            "case count in test/data/ParanoiaRebooted.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ParanoiaRebooted",
                "unexpected game system in ParanoiaRebooted.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ParanoiaRebooted"), &tc.input, &mut src) {
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
                    "FAIL ParanoiaRebooted:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ParanoiaRebooted cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
