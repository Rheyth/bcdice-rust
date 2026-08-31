//! P4で手書き移植した `lib/bcdice/game_system/Liminal.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Liminal#resolute_action`（技能判定 `LMx+b>=t`）
//! - `Liminal#resolute_initiative`（イニシアティヴ判定 `LIx+b>=t`）
//!
//! 原典は2つの判定がほぼ同じコードの写しになっている。ここでは
//! [`resolute`] に束ね、唯一の差（技能レベル0のときの難易度+2）を引数で切り替える。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Liminal`（ID: `Liminal`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liminal;

impl GameSystem for Liminal {
    fn id(&self) -> &'static str {
        "Liminal"
    }

    fn name(&self) -> &'static str {
        "リミナル"
    }

    fn sort_key(&self) -> &'static str {
        "りみなる"
    }

    fn help_message(&self) -> &'static str {
        r"■技能判定　LMx+b>=t+m   x:技能レベル b:ボーナス t:難易度 m:敵の技能レベル(対抗判定)

例)LM2>=8:  技能レベル2,難易度8で技能判定し、その結果を表示。(クリティカル成功も表示)
   LM3+2>=9:技能レベル3,ボーナス+2,難易度9で技能判定し、その結果を表示。( 〃 )
   LM0>=8:  技能なし,難易度8で技能判定する。(難易度+2は自動的に足されます)

■イニシアティヴ判定　LIx+b>=t+m   x:認識力レベル b:ボーナス t:難易度 m:敵の認識力レベル
例)LI2>=8+2:  認識力レベル2,難易度8,敵認識力レベル2で技能判定し、その結果を表示。
   LI0>=8+2:  認識力なし,難易度8,敵認識力レベル2で技能判定する。(難易度加算なし)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["LI", "LM"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Liminal#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: resolute_action(command) || resolute_initiative(command)
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if let Some(result) = resolute_initiative(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(None)
    }
}

/// Ruby `Liminal#resolute_action`（技能判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("LM", round_type: @round_type)（Base の既定 :floor）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["LM"], RoundType::Floor)
            .has_suffix_number()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge)])
    });

    // Ruby: difficulty += 2 if skill_level == 0
    resolute(parser, "LM", true, command, rng)
}

/// Ruby `Liminal#resolute_initiative`（イニシアティヴ判定）。
fn resolute_initiative(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["LI"], RoundType::Floor)
            .has_suffix_number()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge)])
    });

    // イニシアティヴ判定には技能レベル0の難易度加算が無い
    resolute(parser, "LI", false, command, rng)
}

/// 2つの判定に共通の本体。
///
/// `bump_difficulty_without_skill` が Ruby側の唯一の差
/// （`resolute_action` のみ `difficulty += 2 if skill_level == 0`）。
fn resolute(
    parser: &Parser,
    label: &str,
    bump_difficulty_without_skill: bool,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `has_suffix_number` / `restrict_cmp_op_to(:>=)` なので、
    // パースに成功した時点で技能レベルと難易度は必ずある。
    let skill_level = parsed
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let bonus = parsed.modify_number;
    let mut difficulty = parsed.target_number.unwrap_or(crate::Int::from(0));

    let dice = rng.roll_barabara(2, 6)?;
    let dice_total: i64 = dice.iter().sum();
    let total = dice_total + skill_level + bonus.clone();
    if bump_difficulty_without_skill && skill_level == 0 {
        difficulty += 2;
    }

    // Ruby の代入順（condition → critical → ファンブルで打ち消し）をそのまま保つ。
    let mut result = EvalResult::new();
    result.set_condition(total >= difficulty);
    result.critical = total >= difficulty.clone() + 5;
    if dice_total == 2 {
        result.fumble = true;
        result.critical = false;
        result.set_condition(false);
    }

    let judgement = if result.fumble {
        "1ゾロ"
    } else if result.critical {
        "クリティカル"
    } else if result.success {
        "成功"
    } else {
        "失敗"
    };

    result.text = [
        format!(
            "({label}{skill_level}{}>={difficulty})",
            with_symbol(sat_i64(&bonus))
        ),
        format!(
            "{dice_total}[{}]{}",
            join_dice(&dice),
            with_symbol(crate::randomizer::sat_i64(&(skill_level + bonus)))
        ),
        total.to_string(),
        judgement.to_owned(),
    ]
    .join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `Liminal#with_symbol`。
///
/// `Format.modifier` と違い **0でも `"+0"` を返す**（`Format.modifier(0)` は空文字列）。
fn with_symbol(number: i64) -> String {
    if number >= 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `dice.join(',')`。
fn join_dice(dice: &[i64]) -> String {
    dice.iter()
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
            .join("test/data/Liminal.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Liminal.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Liminal.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Liminal.toml must parse");
        assert_eq!(data.tests.len(), 10, "case count in test/data/Liminal.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Liminal",
                "unexpected game system in Liminal.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Liminal"), &tc.input, &mut src) {
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

            let allowed_surplus = 0;
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Liminal:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Liminal cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
