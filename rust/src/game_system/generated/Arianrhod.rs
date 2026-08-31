//! P4で手書き移植した `lib/bcdice/game_system/Arianrhod.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@sort_add_dice = true`（`@d66_sort_type = D66SortType::NO_SORT` はトレイト既定値）
//! - `Arianrhod#result_nd6`（全1ファンブル / 6が2個以上でクリティカル）
//!
//! `Arianrhod_Korean` が `ko_kr` の定型文を差し替えられるよう、判定は
//! [`Messages`] を受け取る関数に切り出してある。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// 1ロケール分の定型文。`Arianrhod` と `Arianrhod_Korean` はこれだけが違う。
pub(crate) struct Messages {
    /// i18n `fumble`
    pub fumble: &'static str,
    /// i18n `Arianrhod.critical`（`%{dice}` を置換する）
    pub critical: &'static str,
    /// i18n `success`
    pub success: &'static str,
    /// i18n `failure`
    pub failure: &'static str,
}

/// i18n `ja_jp`（`i18n/Arianrhod/ja_jp.yml` と `i18n/ja_jp.yml`）。
static JA_MESSAGES: Messages = Messages {
    fumble: "ファンブル",
    critical: "クリティカル(+%{dice}D6)",
    success: "成功",
    failure: "失敗",
};

/// Ruby `Arianrhod#result_nd6`。
pub(crate) fn result_nd6_impl(
    messages: &Messages,
    total: i64,
    dice_list: &[i64],
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    let n_max = dice_list.iter().filter(|&&d| d == 6).count();

    if dice_list.iter().filter(|&&d| d == 1).count() == dice_list.len() {
        // 全部１の目ならファンブル
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            messages.fumble,
        ))))
    } else if n_max >= 2 {
        // ２個以上６の目があったらクリティカル
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            messages.critical.replace("%{dice}", &n_max.to_string()),
        ))))
    } else if cmp_op != CmpOp::Ge {
        None
    } else {
        match target {
            Target::Question => None,
            Target::Number(target) => {
                if total >= crate::randomizer::sat_i64(&target) {
                    Some(CheckOutcome::Result(Box::new(EvalResult::success(
                        messages.success,
                    ))))
                } else {
                    Some(CheckOutcome::Result(Box::new(EvalResult::failure(
                        messages.failure,
                    ))))
                }
            }
        }
    }
}

/// Ruby `BCDice::GameSystem::Arianrhod`（ID: `Arianrhod`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arianrhod;

impl GameSystem for Arianrhod {
    fn id(&self) -> &'static str {
        "Arianrhod"
    }

    fn name(&self) -> &'static str {
        "アリアンロッドRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ありあんろつとRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・クリティカル、ファンブルの自動判定を行います。(クリティカル時の追加ダメージも表示されます)
・D66ダイスあり
"
    }

    /// Ruby `Arianrhod#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Arianrhod#result_nd6`。
    fn result_nd6(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd6_impl(
            &JA_MESSAGES,
            crate::randomizer::sat_i64(&total),
            value_list,
            cmp_op,
            target,
        )
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
            .join("test/data/Arianrhod.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Arianrhod.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Arianrhod.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Arianrhod.toml must parse");
        assert_eq!(
            data.tests.len(),
            27,
            "case count in test/data/Arianrhod.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Arianrhod",
                "unexpected game system in Arianrhod.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Arianrhod"), &tc.input, &mut src) {
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
                    "FAIL Arianrhod:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Arianrhod cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
