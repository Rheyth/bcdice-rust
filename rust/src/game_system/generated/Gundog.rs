//! P4で手書き移植した `lib/bcdice/game_system/Gundog.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@enabled_d9 = true`（`nD9` / `roll_d9`）
//! - `Gundog#result_1d100`（1D100の成功度判定）
//!
//! `GundogZero` / `GundogRevised` は Ruby 側で本クラスを継承する。
//! `GundogZero.rs` は本ファイルがスタブの頃に親由来の判定を取り込済みなので、
//! 判定ロジックはそちらにも残してある。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::Gundog`（ID: `Gundog`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gundog;

impl GameSystem for Gundog {
    fn id(&self) -> &'static str {
        "Gundog"
    }

    fn name(&self) -> &'static str {
        "ガンドッグ"
    }

    fn sort_key(&self) -> &'static str {
        "かんとつく"
    }

    fn help_message(&self) -> &'static str {
        r"失敗、成功、クリティカル、ファンブルとロールの達成値の自動判定を行います。
nD9ロールも対応。
"
    }

    /// Ruby `Gundog#initialize` の `@enabled_d9 = true`。
    fn enabled_d9(&self) -> bool {
        true
    }

    /// Ruby `Gundog#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_1d100_impl(crate::randomizer::sat_i64(&total), cmp_op, target)
    }
}

/// Ruby `Gundog#result_1d100` 本体。
fn result_1d100_impl(total: i64, cmp_op: CmpOp, target: Target) -> Option<CheckOutcome> {
    // Ruby: return nil unless cmp_op == :<=
    if cmp_op != CmpOp::Le {
        return None;
    }

    // 目標値 `?` の判定は `total >= 100` と `total <= 1` の**後**に来る。
    // 先頭に出すと `1D100<=?` のファンブル／絶対成功が拾えなくなる。
    if total >= 100 {
        return Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            "ファンブル",
        ))));
    }
    if total <= 1 {
        return Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            "絶対成功(達成値1+SL)",
        ))));
    }

    // Ruby: elsif target == "?" -> Result.nothing
    // `nil`（＝次のフックへ進む）ではなく `:nothing`（＝以降を打ち切って nil）。
    let Target::Number(target) = target else {
        return Some(CheckOutcome::Nothing);
    };

    if total > crate::randomizer::sat_i64(&target) {
        return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
    }

    // ここに来る total は 2..=99 なので、Ruby側の
    // `dig10 = 0 if dig10 >= 10` / `dig1 = 0 if dig1 >= 10` は到達しない。
    let dig10 = total / 10;
    let dig1 = total - dig10 * 10;

    let result = if dig1 <= 0 {
        EvalResult::critical("クリティカル(達成値20+SL)")
    } else {
        EvalResult::success(format!("成功(達成値{}+SL)", dig10 + dig1))
    };
    Some(CheckOutcome::Result(Box::new(result)))
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
            .join("test/data/Gundog.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Gundog.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Gundog.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Gundog.toml must parse");
        assert_eq!(data.tests.len(), 11, "case count in test/data/Gundog.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Gundog",
                "unexpected game system in Gundog.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Gundog"), &tc.input, &mut src) {
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
                    "FAIL Gundog:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Gundog cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
