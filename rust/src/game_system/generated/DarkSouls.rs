//! P4で手書き移植した `lib/bcdice/game_system/DarkSouls.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（行為判定 `[n]DS[a±b][@t]` / 能動判定 `[n]ADS...`）
//! - `#checkRoll` / `#getValue` / `#getValueText`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::DarkSouls`（ID: `DarkSouls`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkSouls;

impl GameSystem for DarkSouls {
    fn id(&self) -> &'static str {
        "DarkSouls"
    }

    fn name(&self) -> &'static str {
        "ダークソウルTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "たあくそうるTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定：[n]DS[a±b][@t]　　[]内のコマンドは省略可
・能動判定：[n]ADS[a±b][@t]　　FP消費を判定
　n：ダイス数。省略時は「2」
　a±b：修正値。「1+2-1」のように、複数定可
　@t：目標値。省略時は達成値を、指定時は判定の正否を表示
例）DS → 2D6の達成値を表示
　　1DS → 1D6の達成値を表示
　　ADS+2-2 → 2D6+2の達成値を表示（能動判定）
　　DS+2@10 → 2D6+2で目標値10の判定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?(A)?DS([-+\d]*)(@\d+)?"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        static RE: OnceLock<Regex> = OnceLock::new();
        // Ruby は `command.upcase` に対して `/i` 付きでマッチさせる。
        // 呼び出し元（`Base#dice_command`）で既に大文字化されているので、
        // ここでは `(?i)` を付けるだけで等価。
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)(\d+)?(A)?DS([-+\d]*)(@(\d+))?$").expect("valid regex")
        });

        let Some(m) = re.captures(command) else {
            return Ok(None);
        };

        // Ruby: `(m[1] || 2).to_i` — 省略時のダイス数は 2。
        let dice_count = m.get(1).map_or(2, |c| to_i(c.as_str()));
        let is_active = m.get(2).is_some();
        let modify = get_value(m.get(3).map_or("", |c| c.as_str()))?;
        let target = m.get(5).map_or(0, |c| to_i(c.as_str()));

        let output = check_roll(dice_count, is_active, modify, target, rng)?;
        Ok(Some(SpecificCommandOutput::text(output)))
    }
}

/// Ruby `String#to_i`（ここに来るのは `\d+` にマッチした文字列だけ）。
///
/// 桁あふれは Ruby だと Bignum になるので、`i64` に収まらない場合は飽和させる。
fn to_i(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `#getValue`: `ArithmeticEvaluator.eval(text)`（不正な式・`nil` は `0`）。
fn get_value(text: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `#getValueText`。
fn get_value_text(value: i64) -> String {
    if value == 0 {
        String::new()
    } else if value < 0 {
        value.to_string()
    } else {
        format!("+{value}")
    }
}

/// Ruby `#checkRoll`。
fn check_roll(
    dice_count: i64,
    is_active: bool,
    modify: i64,
    target: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let dice: i64 = dice_list.iter().sum();
    let dice_text = dice_list
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let success_value = dice + modify;
    let modify_text = get_value_text(modify);
    let target_text = if target == 0 {
        String::new()
    } else {
        format!(">={target}")
    };

    // Ruby: `focusText` は能動判定かつ 1 の目が出たときだけ代入され、
    // それ以外では `nil`（`nil.to_s == ""`）。
    let mut focus_text = String::new();
    if is_active {
        let focus_damage = dice_list.iter().filter(|&&i| i == 1).count();
        if focus_damage > 0 {
            focus_text = format!("（FP{}消費）", "■".repeat(focus_damage));
        }
    }

    let mut result = format!("({dice_count}D6{modify_text}{target_text})");
    result += &format!(" ＞ {dice}({dice_text}){modify_text}");
    result += &format!(" ＞ {success_value}{target_text}");

    if target > 0 {
        if success_value >= target {
            result += " ＞ 【成功】";
        } else {
            result += " ＞ 【失敗】";
        }
    }

    result += &focus_text;
    Ok(result)
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
            .join("test/data/DarkSouls.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DarkSouls.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/DarkSouls.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DarkSouls.toml must parse");
        assert_eq!(
            data.tests.len(),
            20,
            "case count in test/data/DarkSouls.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DarkSouls",
                "unexpected game system in DarkSouls.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("DarkSouls"), &tc.input, &mut src) {
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
                    "FAIL DarkSouls:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DarkSouls cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
