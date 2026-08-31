//! P4で手書き移植した `lib/bcdice/game_system/Ainecadette.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Ainecadette#roll_action`（先輩 `nAI` / 後輩 `nCA` の行為判定）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `SUCCESS_THRESHOLD`（成功の目標値）。
const SUCCESS_THRESHOLD: i64 = 4;

/// Ruby `SPECIAL_DICE`（スペシャルとなる出目）。
const SPECIAL_DICE: i64 = 6;

/// Ruby `BCDice::GameSystem::Ainecadette`（ID: `Ainecadette`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ainecadette;

impl GameSystem for Ainecadette {
    fn id(&self) -> &'static str {
        "Ainecadette"
    }

    fn name(&self) -> &'static str {
        "エネカデット"
    }

    fn sort_key(&self) -> &'static str {
        "えねかてつと"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定
- 先輩 (AI) 10面ダイスを2つ振って判定します。『有利』なら【3AI】、『不利』なら【1AI】を使います。
- 後輩 (CA) 6面ダイスを2つ振って判定します。『有利』なら【3CA】、『不利』なら【1CA】を使います。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?AI", r"(\d+)?CA"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_action(command, rng)
    }
}

/// Ruby `roll_action` のコマンド抽出（`/^(\d+)?(AI|CA)$/`）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)?(AI|CA)$").expect("valid regex"))
}

/// Ruby `Ainecadette#roll_action`。
fn roll_action(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: return nil unless m
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let is_senpai = &m[2] == "AI";

    // Ruby: times = m[1]&.to_i || 2
    // 桁あふれする入力は Ruby だと Bignum のまま roll_barabara に渡り、
    // 個数上限（200）を超えて空配列になる。i64 に収まらない場合も同じ経路へ落とす。
    let times: i64 = match m.get(1) {
        Some(digits) => digits.as_str().parse().unwrap_or(i64::MAX),
        None => 2,
    };
    let sides = if is_senpai { 10 } else { 6 };
    // Ruby: return nil if times <= 0
    if times <= 0 {
        return Ok(None);
    }

    let dice_list = rng.roll_barabara(times, sides)?;
    // Ruby `Array#max`。空配列（個数上限超え）では nil になり、続く `max <= 1` が
    // NoMethodError でクラッシュする。ここでは 0 として扱う。
    let max = dice_list.iter().copied().max().unwrap_or(0);

    let mut result = if max <= 1 {
        EvalResult::fumble("ファンブル（もやもやカウンターを2個獲得）")
    } else if dice_list.contains(&SPECIAL_DICE) {
        let me = if is_senpai { "先輩" } else { "後輩" };
        let target = if is_senpai { "後輩" } else { "先輩" };
        EvalResult::critical(format!(
            "スペシャル（絆カウンターを1個獲得し、{target}は{me}への感情を1つ獲得）"
        ))
    } else if max >= SUCCESS_THRESHOLD {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };

    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    result.text = format!("({command}) ＞ [{dice_str}] ＞ {}", result.text);

    Ok(Some(SpecificCommandOutput::result(result)))
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
            .join("test/data/Ainecadette.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Ainecadette.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Ainecadette.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Ainecadette.toml must parse");
        assert_eq!(
            data.tests.len(),
            19,
            "case count in test/data/Ainecadette.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Ainecadette",
                "unexpected game system in Ainecadette.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Ainecadette"), &tc.input, &mut src) {
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
                    "FAIL Ainecadette:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Ainecadette cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
