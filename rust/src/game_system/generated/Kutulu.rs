//! P4で手書き移植した `lib/bcdice/game_system/Kutulu.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Kutulu#resolute_action`（アクティヴ能力の判定 `nKU`）
//! - `Kutulu#resolute_competition`（対抗判定 `nKR`）
//!
//! `KutuluRevised` は Ruby でも `Base` を直接継承した別クラスなので、
//! こちらの関数は共有せずあちらに複製してある（原典どおり）。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Kutulu`（ID: `Kutulu`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kutulu;

impl GameSystem for Kutulu {
    fn id(&self) -> &'static str {
        "Kutulu"
    }

    fn name(&self) -> &'static str {
        "Kutulu"
    }

    fn sort_key(&self) -> &'static str {
        "くとうるう"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　nKU        n: ダイス数

例)3KU: ダイスを3個振って、その結果を表示(ギリギリでの成功も表示)

■対抗判定　nKR        n: ダイス数

例)2KR: ダイスを2個振って、その結果を表示。対抗判定用の3桁の数字も出力。(大きい方が勝利)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\dK[UR]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Kutulu#eval_game_system_specific_command`。
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

/// Ruby `/(\d)KU/`（アンカーなし）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d)KU").expect("valid regex"))
}

/// Ruby `/(\d)KR/`（アンカーなし）。
fn competition_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d)KR").expect("valid regex"))
}

/// Ruby `Kutulu#resolute_action`（アクティヴ能力の判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    // 1桁の数字なのでパースは必ず成功する。
    let num_dices: i64 = m[1].parse().expect("single digit");

    let mut dices = rng.roll_barabara(num_dices, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);

    let mut output = format!("({num_dices}KU) ＞ {dice_text}");

    let success_num = dices.iter().filter(|&&val| val >= 4).count();
    let counts_4 = dices.iter().filter(|&&val| val == 4).count();
    if success_num > 0 {
        output.push_str(&format!(" ＞ 成功数{success_num}"));
        if success_num == 1 && counts_4 == 1 {
            output.push_str(" ＞ *ギリギリでの成功");
        }
        Ok(Some(EvalResult::success(output)))
    } else {
        output.push_str(" ＞ 失敗");
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `Kutulu#resolute_competition`（対抗判定用出力）。
fn resolute_competition(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = competition_pattern().captures(command) else {
        return Ok(None);
    };

    let num_dices: i64 = m[1].parse().expect("single digit");

    let mut dices = rng.roll_barabara(num_dices, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);

    let counts_6 = dices.iter().filter(|&&val| val == 6).count();
    let counts_5 = dices.iter().filter(|&&val| val == 5).count();
    let success_num = dices.iter().filter(|&&val| val >= 4).count();
    // Ruby: format("(%d%d%d)", success_num, counts_6, counts_5)
    let com_text = format!("({success_num}{counts_6}{counts_5})");

    let output = format!("({num_dices}KR) ＞ {dice_text} ＞ {com_text}");

    if success_num > 0 {
        Ok(Some(EvalResult::success(output)))
    } else {
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `dices.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
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
            .join("test/data/Kutulu.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Kutulu.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Kutulu.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Kutulu.toml must parse");
        assert_eq!(data.tests.len(), 15, "case count in test/data/Kutulu.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Kutulu",
                "unexpected game system in Kutulu.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Kutulu"), &tc.input, &mut src) {
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
                    "FAIL Kutulu:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Kutulu cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
