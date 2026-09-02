//! P4で手書き移植した `lib/bcdice/game_system/RokumonSekai2.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `RokumonSekai2#replace_text`（`aRSm<=t` → `3R6m<=t[a]`）
//! - `#eval_game_system_specific_command` / `#rokumon2_roll` / `#rokumon2_suc_rank`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::RokumonSekai2`（ID: `RokumonSekai2`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RokumonSekai2;

impl GameSystem for RokumonSekai2 {
    fn id(&self) -> &'static str {
        "RokumonSekai2"
    }

    fn name(&self) -> &'static str {
        "六門世界RPG セカンドエディション"
    }

    fn sort_key(&self) -> &'static str {
        "ろくもんせかいRPG2"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
aRSm<=t
能力値a,修正値m,目標値tで判定ロールを行います。
Rコマンド(3R6m<=t[a])に読み替えます。
成功度、評価、ボーナスダイスを自動表示します。
　例) 3RS+1<=9　3R6+1<=9[3]
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+RS", "3R6"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `RokumonSekai2#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let string = replace_text(command);
        let Some(m) = roll_pattern().captures(&string) else {
            return Ok(None);
        };

        let mod_text = m.get(1).map(|g| g.as_str()).unwrap_or("");
        let target = to_i(&m[2]);
        let abl = to_i(&m[3]);

        // Ruby `ArithmeticEvaluator.eval(modText)`（空・不正は 0）
        let modifier = if mod_text.is_empty() {
            0
        } else {
            arithmetic::eval(mod_text, RoundType::Floor)?
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0)
        };

        let (dstr, suc, sum) = rokumon2_roll(modifier, target, abl, rng)?;
        let mut output = format!("{sum}[{dstr}] ＞ {suc} ＞ 評価{}", rokumon2_suc_rank(suc));
        if suc != 0 {
            output.push_str(&format!("(+{suc}d6)"));
        }
        Ok(Some(SpecificCommandOutput::text(format!(
            "({string}) ＞ {output}"
        ))))
    }
}

/// Ruby `RokumonSekai2#replace_text`。
fn replace_text(string: &str) -> String {
    let with_mod = rs_mod_pattern().replace_all(string, |c: &regex::Captures| {
        format!("3R6{}<={}[{}]", &c[2], &c[3], &c[1])
    });
    rs_plain_pattern()
        .replace_all(&with_mod, |c: &regex::Captures| {
            format!("3R6<={}[{}]", &c[2], &c[1])
        })
        .into_owned()
}

/// Ruby `/(\d+)RS([+-][+\-\d]+)<=(\d+)/i`。
fn rs_mod_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9]+)RS([+-][+\-0-9]+)<=([0-9]+)").expect("valid regex"))
}

/// Ruby `/(\d+)RS<=(\d+)/i`。
fn rs_plain_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9]+)RS<=([0-9]+)").expect("valid regex"))
}

/// Ruby `/3R6([+\-\d]*)<=(\d+)\[(\d+)\]/i`。
fn roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)3R6([+\-0-9]*)<=([0-9]+)\[([0-9]+)\]").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` 範囲外は符号方向に飽和。
fn to_i(text: &str) -> i64 {
    str_helpers::to_i_signed_saturating(text)
}

/// Ruby `RokumonSekai2#rokumon2_roll`。
fn rokumon2_roll(
    modifier: i64,
    target: i64,
    abl: i64,
    rng: &mut Randomizer,
) -> Result<(String, i64, i64), EvalError> {
    let times = 3i64.saturating_add(modifier.saturating_abs());
    let mut dice = rng.roll_barabara(times, 6)?;
    dice.sort_unstable();
    let dicestr = dice
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let drop = usize::try_from(modifier.saturating_abs()).unwrap_or(usize::MAX);
    let drop = drop.min(dice.len());
    if modifier < 0 {
        dice.drain(0..drop);
    } else {
        dice.truncate(dice.len() - drop);
    }

    let mut suc = 0i64;
    let mut cnt5 = 0i64;
    let mut cnt2 = 0i64;
    let mut sum = 0i64;
    for die1 in &dice {
        if *die1 >= 5 {
            cnt5 += 1;
        }
        if *die1 <= 2 {
            cnt2 += 1;
        }
        if *die1 <= abl {
            suc += 1;
        }
        sum += *die1;
    }

    if sum < target {
        suc += 2;
    } else if sum == target {
        suc += 1;
    }

    if cnt5 >= 3 {
        suc = 0;
    }
    if cnt2 >= 3 {
        suc = 5;
    }

    Ok((dicestr, suc, sum))
}

/// Ruby `RokumonSekai2#rokumon2_suc_rank`。
fn rokumon2_suc_rank(suc: i64) -> &'static str {
    const RANKS: [&str; 6] = ["E", "D", "C", "B", "A", "S"];
    usize::try_from(suc)
        .ok()
        .and_then(|i| RANKS.get(i).copied())
        .unwrap_or("")
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
            .join("test/data/RokumonSekai2.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/RokumonSekai2.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/RokumonSekai2.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("RokumonSekai2.toml must parse");
        assert_eq!(
            data.tests.len(),
            50,
            "case count in test/data/RokumonSekai2.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "RokumonSekai2",
                "unexpected game system in RokumonSekai2.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("RokumonSekai2"), &tc.input, &mut src) {
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
                    "FAIL RokumonSekai2:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} RokumonSekai2 cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
