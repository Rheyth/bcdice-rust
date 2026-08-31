//! P4で手書き移植した `lib/bcdice/game_system/ShadowRun4.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ShadowRun4#grich_text`（B/Rコマンド時のグリッチ判定）
//!
//! 設定値（`@sort_add_dice` / `@sort_barabara_dice` / `@reroll_dice_reroll_threshold` /
//! `@default_cmp_op` / `@default_target_number`）はスタブが持っている値と一致する。

use crate::game_system::GameSystem;
use crate::normalize::CmpOp;

/// Ruby `BCDice::GameSystem::ShadowRun4`（ID: `ShadowRun4`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowRun4;

impl GameSystem for ShadowRun4 {
    fn id(&self) -> &'static str {
        "ShadowRun4"
    }

    fn name(&self) -> &'static str {
        "シャドウラン 4th Edition"
    }

    fn sort_key(&self) -> &'static str {
        "しやとうらん4"
    }

    fn help_message(&self) -> &'static str {
        r"個数振り足しロール(xRn)の境界値を6にセット、バラバラロール(xBn)の目標値を5以上にセットします。
BコマンドとRコマンド時に、グリッチの表示を行います。
"
    }

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@reroll_dice_reroll_threshold = 6`。
    fn reroll_dice_reroll_threshold(&self) -> Option<i64> {
        Some(6)
    }

    /// Ruby `initialize` の `@default_cmp_op = :>=`。
    fn default_cmp_op(&self) -> Option<CmpOp> {
        Some(CmpOp::Ge)
    }

    /// Ruby `initialize` の `@default_target_number = 5`。
    fn default_target_number(&self) -> Option<i64> {
        Some(5)
    }

    /// Ruby `ShadowRun4#grich_text`。
    fn grich_text(
        &self,
        count_one: usize,
        dice_total_count: usize,
        count_success: i64,
    ) -> Option<String> {
        // Ruby: dice_cnt_total_half = (1.0 * dice_cnt_total / 2)
        let dice_cnt_total_half = dice_total_count as f64 / 2.0;

        // Ruby: unless numberSpot1 >= dice_cnt_total_half -> nil
        // 両辺とも有限値なので `!(a >= b)` は `a < b` と等しい。
        if (count_one as f64) < dice_cnt_total_half {
            return None;
        }

        // グリッチ！
        if count_success == 0 {
            Some("クリティカルグリッチ".to_owned())
        } else {
            Some("グリッチ".to_owned())
        }
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
            .join("test/data/ShadowRun4.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ShadowRun4.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ShadowRun4.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ShadowRun4.toml must parse");
        assert_eq!(
            data.tests.len(),
            36,
            "case count in test/data/ShadowRun4.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ShadowRun4",
                "unexpected game system in ShadowRun4.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ShadowRun4"), &tc.input, &mut src) {
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
                    "FAIL ShadowRun4:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ShadowRun4 cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
