//! P4で手書き移植した `lib/bcdice/game_system/Magius_3rdNewTokyoCity.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Magius` を継承し、`get_result_of_ability_action` /
//! `get_result_of_skill_action` の2つだけを上書きする（ゾロ目 12/2 を絶対成功・絶対失敗にする）。
//! コマンド解釈とダイスの振り方は [`super::Magius`] の実装をそのまま使う。

use super::Magius::{eval_specific_command, SystemRules};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `Magius_3rdNewTokyoCity` が上書きする判定結果フック。
static RULES: SystemRules = SystemRules {
    ability_result: result_of_ability_action,
    skill_result: result_of_skill_action,
};

/// Ruby `Magius_3rdNewTokyoCity#get_result_of_ability_action`。
fn result_of_ability_action(total: i64, dice_add: i64, target: i64) -> EvalResult {
    if dice_add == 12 {
        EvalResult::critical("絶対成功")
    } else if dice_add == 2 {
        EvalResult::fumble("絶対失敗")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `Magius_3rdNewTokyoCity#get_result_of_skill_action`。
fn result_of_skill_action(total: i64, dice_add: i64, target: i64) -> EvalResult {
    if dice_add == 12 {
        EvalResult::critical("絶対成功")
    } else if dice_add == 2 {
        EvalResult::fumble("絶対失敗")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `BCDice::GameSystem::Magius_3rdNewTokyoCity`（ID: `Magius_3rdNewTokyoCity`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Magius_3rdNewTokyoCity;

impl GameSystem for Magius_3rdNewTokyoCity {
    fn id(&self) -> &'static str {
        "Magius_3rdNewTokyoCity"
    }

    fn name(&self) -> &'static str {
        "MAGIUS:新世紀エヴァンゲリオンRPG 決戦！第3新東京市"
    }

    fn sort_key(&self) -> &'static str {
        "まきうすしんせいきえうあんけりおんRPGけつせんたい3しんとうきようし"
    }

    fn help_message(&self) -> &'static str {
        r"■能力値判定　MA+x>=t        x:修正値 t:目標値
例)MA>=7: ダイスを2個振って、その結果(成功,失敗,絶対成功,絶対失敗)を表示

■技能値判定　MS+x>=t        x:修正値 t:目標値
例)MS>=7: ダイスを3個振って、そのうち上位2つを採用し、結果(成功,失敗,絶対成功,絶対失敗)を表示

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["M[AS]"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&RULES, self.round_type(), command, rng)
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
            .join("test/data/Magius_3rdNewTokyoCity.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Magius_3rdNewTokyoCity.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Magius_3rdNewTokyoCity.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Magius_3rdNewTokyoCity.toml must parse");
        assert_eq!(
            data.tests.len(),
            10,
            "case count in test/data/Magius_3rdNewTokyoCity.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Magius_3rdNewTokyoCity",
                "unexpected game system in Magius_3rdNewTokyoCity.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("Magius_3rdNewTokyoCity"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL Magius_3rdNewTokyoCity:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Magius_3rdNewTokyoCity cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
