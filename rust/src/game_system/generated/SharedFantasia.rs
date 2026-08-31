//! P4で手書き移植した `lib/bcdice/game_system/SharedFantasia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `SharedFantasia#change_text`（`SF` / `ST` → `2D6` の書き換え）
//! - `SharedFantasia#result_2d6`（自動成功・自動失敗・劇的成功・致命的失敗）
//!
//! # `SF` / `ST` 接頭辞について
//!
//! `register_prefix` には `SF` と `ST` があるが、`change_text` が前処理で
//! `2D6` に書き換えてしまうため、この2つが `dice_command` 側で一致することはない
//! （実際の判定は共通コマンドの加算ロールが行う）。原典どおりの構造を保つため
//! 接頭辞はそのまま残してある。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `/S[FT]/i`。
///
/// `(?i)` は使わない（`regex` クレートの `(?i)` はUnicodeケースフォールディングになり、
/// `K`(U+212A) 等まで拾ってしまう）ので大小を明示して書く。
fn sf_st_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[Ss][FfTt]").expect("valid regex"))
}

/// Ruby `SharedFantasia#change_text`（`gsub(/S[FT]/i, "2D6")`）。
fn change_text_impl(text: &str) -> Cow<'_, str> {
    sf_st_pattern().replace_all(text, "2D6")
}

/// Ruby `BCDice::GameSystem::SharedFantasia`（ID: `SharedFantasia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedFantasia;

impl GameSystem for SharedFantasia {
    fn id(&self) -> &'static str {
        "SharedFantasia"
    }

    fn name(&self) -> &'static str {
        "Shared†Fantasia"
    }

    fn sort_key(&self) -> &'static str {
        "しえああとふあんたしあ"
    }

    fn help_message(&self) -> &'static str {
        r"2D6の成功判定に 自動成功、自動失敗、致命的失敗、劇的成功 の判定があります。

SF/ST = 2D6のショートカット

例) SF+4>=9 : 2D6して4を足した値が9以上なら成功
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SF", "ST"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SharedFantasia#change_text`。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        change_text_impl(text)
    }

    /// Ruby `SharedFantasia#result_2d6`。
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == '?'
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return nil unless [:>=, :>].include?(cmp_op)
        if cmp_op != CmpOp::Ge && cmp_op != CmpOp::Gt {
            return None;
        }

        let critical = dice_total == 12;
        let fumble = dice_total == 2;

        // Ruby: totalValueBonus = (cmp_op == :>= ? 1 : 0)
        let total_value_bonus = i64::from(cmp_op == CmpOp::Ge);

        let result = if (total + total_value_bonus) > target {
            if critical {
                EvalResult::critical("自動成功(劇的成功)")
            } else if fumble {
                EvalResult::failure("自動失敗")
            } else {
                EvalResult::success("成功")
            }
        } else if critical {
            EvalResult::success("自動成功")
        } else if fumble {
            EvalResult::fumble("自動失敗(致命的失敗)")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
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
            .join("test/data/SharedFantasia.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/SharedFantasia.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/SharedFantasia.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("SharedFantasia.toml must parse");
        assert_eq!(
            data.tests.len(),
            14,
            "case count in test/data/SharedFantasia.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "SharedFantasia",
                "unexpected game system in SharedFantasia.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("SharedFantasia"), &tc.input, &mut src) {
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
                    "FAIL SharedFantasia:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} SharedFantasia cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
