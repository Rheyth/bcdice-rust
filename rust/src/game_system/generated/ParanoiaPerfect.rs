//! P4で手書き移植した `lib/bcdice/game_system/ParanoiaPerfect.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ParanoiaPerfect#get_node_dice_roll`（ノードダイス判定 `NDx,y`）
//! - `ParanoiaPerfect#generate_roll_results`（コンピュータダイスの表示）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::ParanoiaPerfect`（ID: `ParanoiaPerfect`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParanoiaPerfect;

impl GameSystem for ParanoiaPerfect {
    fn id(&self) -> &'static str {
        "ParanoiaPerfect"
    }

    fn name(&self) -> &'static str {
        "パラノイア・パーフェクト エディション"
    }

    fn sort_key(&self) -> &'static str {
        "はらのいあはあふえくとえていしよん"
    }

    fn help_message(&self) -> &'static str {
        r"※コマンドは入力内容の前方一致で検出しています。
・通常の判定　NDx,y
　x：ノードダイスの数.マイナスも可.
　y: 反逆スターの数.省略可.省略時0
　ノードダイスの絶対値 + 1個(コンピュータダイス)のダイスがロールされる.
例）ND2　ND-3　ND2,1　ND-3,2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ND"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ParanoiaPerfect#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        get_node_dice_roll(command, rng)
    }
}

/// Ruby `/^ND((-)?\d+)(,(\d+))?$/i`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn node_dice_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^ND((-)?[0-9]+)(,([0-9]+))?$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当（桁あふれは飽和させる）。
///
/// Ruby は Bignum になるが、そこまで大きい値は `roll_barabara` の本数上限に
/// 引っかかって `TooManyRandsError` になる。飽和させても同じ経路へ落ちる。
fn to_i_saturating(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or({
        if text.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `ParanoiaPerfect#get_node_dice_roll`。
fn get_node_dice_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = node_dice_pattern().captures(command) else {
        return Ok(None);
    };

    let parameter_num = to_i_saturating(&m[1]);
    // Ruby: m[4].to_i（nil.to_i は 0）
    let traitorous_star = m.get(4).map_or(0, |g| to_i_saturating(g.as_str()));
    let dice_count = parameter_num.saturating_abs().saturating_add(1);

    let dices = rng.roll_barabara(dice_count, 6)?;

    let mut success_rate = dices.iter().filter(|d| **d >= 5).count() as i64;
    if parameter_num < 0 {
        success_rate -= dices.iter().filter(|d| **d < 5).count() as i64;
    }

    let (results, computer_dice_message) = generate_roll_results(traitorous_star, &dices);

    Ok(Some(SpecificCommandOutput::text(format!(
        "({command}) ＞ [{}] ＞ 成功度{success_rate}{computer_dice_message}",
        results.join(", ")
    ))))
}

/// Ruby `ParanoiaPerfect#generate_roll_results`。
///
/// 最後のダイス（コンピュータダイス）が `6 - 反逆スター` 以上なら
/// 出目に `C` を付けて `(Computer)` を添える。
fn generate_roll_results(traitorous_star: i64, dices: &[i64]) -> (Vec<String>, &'static str) {
    let mut results: Vec<String> = dices.iter().map(|d| d.to_string()).collect();

    // Ruby: last_die = results[-1].to_i（空配列なら nil.to_i で 0）
    let Some(last_die) = dices.last().copied() else {
        return (results, "");
    };

    if last_die >= 6 - traitorous_star {
        let last = results.len() - 1;
        results[last] = format!("{last_die}C");
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
            .join("test/data/ParanoiaPerfect.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ParanoiaPerfect.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ParanoiaPerfect.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ParanoiaPerfect.toml must parse");
        assert_eq!(
            data.tests.len(),
            15,
            "case count in test/data/ParanoiaPerfect.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ParanoiaPerfect",
                "unexpected game system in ParanoiaPerfect.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ParanoiaPerfect"), &tc.input, &mut src) {
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
                    "FAIL ParanoiaPerfect:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ParanoiaPerfect cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
