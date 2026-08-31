//! P4で手書き移植した `lib/bcdice/game_system/Magius.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Magius#resolute_ability_action`（能力値判定 `MA+x>=t`）
//! - `Magius#resolute_skill_action`（技能値判定 `MS+x>=t`）
//!
//! Ruby側で `Magius_3rdNewTokyoCity < Magius` は
//! `get_result_of_ability_action` / `get_result_of_skill_action` の2つだけを上書きする。
//! Rust側はこの2フックを [`SystemRules`] に束ね、コマンド解釈とダイスの振り方は
//! [`eval_specific_command`] を共有する。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `Magius` の判定結果フック。`Magius_3rdNewTokyoCity` はここだけが違う。
pub(crate) struct SystemRules {
    /// Ruby `get_result_of_ability_action(total, dice_add, target)`
    pub(crate) ability_result: fn(i64, i64, i64) -> EvalResult,
    /// Ruby `get_result_of_skill_action(total, dice_add, target)`
    pub(crate) skill_result: fn(i64, i64, i64) -> EvalResult,
}

/// Ruby `Magius` 本体のフック。
static JA_RULES: SystemRules = SystemRules {
    ability_result: result_of_ability_action,
    skill_result: result_of_skill_action,
};

/// Ruby `Magius#get_result_of_ability_action`。
fn result_of_ability_action(total: i64, _dice_add: i64, target: i64) -> EvalResult {
    if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `Magius#get_result_of_skill_action`。
fn result_of_skill_action(total: i64, _dice_add: i64, target: i64) -> EvalResult {
    if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `Magius#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    rules: &SystemRules,
    round_type: RoundType,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: resolute_ability_action(command) || resolute_skill_action(command)
    if let Some(result) = resolute_ability_action(rules, round_type, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = resolute_skill_action(rules, round_type, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(None)
}

/// Ruby `/MA([+-]\d+)*>=(\d+)/`。
///
/// `([+-]\d+)*` は繰り返しなので、Rubyと同じく **最後の**修正値だけが `m[1]` に入る
/// （`MA+1+2>=7` なら `"+2"`）。
fn ability_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"MA([+-]\d+)*>=(\d+)").expect("valid regex"))
}

/// Ruby `/MS([+-]\d+)*>=(\d+)/`。
fn skill_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"MS([+-]\d+)*>=(\d+)").expect("valid regex"))
}

/// Ruby `Magius#resolute_ability_action`（2D6のうち全部を使う能力値判定）。
fn resolute_ability_action(
    rules: &SystemRules,
    round_type: RoundType,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = ability_pattern().captures(command) else {
        return Ok(None);
    };

    let modify = parse_modifier(m.get(1).map(|x| x.as_str()), round_type)?;
    let target = parse_target(&m[2]);

    let mut dices = rng.roll_barabara(2, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);
    let dice_add: i64 = dices.iter().sum();
    let total = dice_add.saturating_add(modify);

    let result = (rules.ability_result)(total, dice_add, target);

    Ok(Some(finish(result, command, &dice_text, modify, total)))
}

/// Ruby `Magius#resolute_skill_action`（3D6のうち上位2つを使う技能値判定）。
fn resolute_skill_action(
    rules: &SystemRules,
    round_type: RoundType,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = skill_pattern().captures(command) else {
        return Ok(None);
    };

    let modify = parse_modifier(m.get(1).map(|x| x.as_str()), round_type)?;
    let target = parse_target(&m[2]);

    let mut dices = rng.roll_barabara(3, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);
    // Ruby: dices[1].to_i + dices[2].to_i（昇順ソート後の上位2つ）。
    // ダイスが振れなかった場合の `nil.to_i` は 0 になる。
    let dice_add: i64 = dices.get(1).copied().unwrap_or(0) + dices.get(2).copied().unwrap_or(0);
    let total = dice_add.saturating_add(modify);

    let result = (rules.skill_result)(total, dice_add, target);

    Ok(Some(finish(result, command, &dice_text, modify, total)))
}

/// Ruby `modify = m[1] ? Arithmetic.eval(m[1], @round_type) : 0`。
///
/// `m[1]` は `[+-]\d+` なので `Arithmetic.eval` が `nil` を返すことはない
/// （Ruby側は `nil` だと直後の加算で NoMethodError になる）。
fn parse_modifier(source: Option<&str>, round_type: RoundType) -> Result<i64, EvalError> {
    match source {
        Some(s) => Ok(arithmetic::eval(s, round_type)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)),
        None => Ok(0),
    }
}

/// Ruby `m[2].to_i`。i64に収まらない目標値は飽和させる（Rubyでは Bignum）。
fn parse_target(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby の `sequence.join(" ＞ ")`（`sequence` は `compact` 済みだが nil は入らない）。
///
/// `with_symbol` は `Format.modifier` と同一の実装なので後者を使う。
fn finish(
    mut result: EvalResult,
    command: &str,
    dice_text: &str,
    modify: i64,
    total: i64,
) -> EvalResult {
    let text = std::mem::take(&mut result.text);
    result.text = format!(
        "({command}) ＞ [{dice_text}]{} ＞ {total} ＞ {text}",
        modifier(&crate::Int::from(modify))
    );
    result
}

/// Ruby `dices.join(",")`。
fn join_dice(dices: &[i64]) -> String {
    dices
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::Magius`（ID: `Magius`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Magius;

impl GameSystem for Magius {
    fn id(&self) -> &'static str {
        "Magius"
    }

    fn name(&self) -> &'static str {
        "MAGIUS"
    }

    fn sort_key(&self) -> &'static str {
        "まきうす"
    }

    fn help_message(&self) -> &'static str {
        r"■能力値判定　MA+x>=t        x:修正値 t:目標値
例)MA>=7: ダイスを2個振って、その結果を表示

■技能値判定　MS+x>=t        x:修正値 t:目標値
例)MS>=7: ダイスを3個振って、そのうち上位2つを採用し、結果を表示

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
        eval_specific_command(&JA_RULES, self.round_type(), command, rng)
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
            .join("test/data/Magius.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Magius.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Magius.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Magius.toml must parse");
        assert_eq!(data.tests.len(), 6, "case count in test/data/Magius.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Magius",
                "unexpected game system in Magius.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Magius"), &tc.input, &mut src) {
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
                    "FAIL Magius:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Magius cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
