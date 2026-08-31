//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_English.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:en_us` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `en_us` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/en_us.yml` と `i18n/en_us.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `en_us` ロケールの文言一式。
static EN_US: Locale = Locale {
    success: "Success",
    failure: "Failure",
    critical: "Critical Success",
    special: "Special",
    critical_special: "Critical Success/Special",
    fumble: "Fumble",
    partial_success: "Partial Success",
    automatic_success: "Automatic Success",
    automatic_failure: "Automatic Failure",
    broken: "Malfunction",
    broken_number: "Malfunction Number",
};

/// Ruby `BCDice::GameSystem::Cthulhu_English`（ID: `Cthulhu:English`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_English;

impl GameSystem for Cthulhu_English {
    fn id(&self) -> &'static str {
        "Cthulhu:English"
    }

    fn name(&self) -> &'static str {
        "Call of Cthulhu"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:English:Call of Cthulhu"
    }

    fn help_message(&self) -> &'static str {
        r#"c=Critical Rate ／ f=Fumble Rate ／ s=Special

1d100<=n    c・f・s AllOff（Does Simple Numeric Comparison Only）

・Roll Command that determines cfs

CC	 Does a 1d100 roll c=1、f=100
CCB  Same as above、c=5、f=96

Ex：CC<=80  （Rolls using 80 as skill value with 1% cf rule applied）
Ex：CCB<=55 （Rolls using 55 as skill value with 5% cf rule applied）

・About Roll Combination

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・About Opposed Rolls
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※Malfunction Number Determination

・CC(x) c=1、f=100
x=Malfunction Number. Outputs（text "Fumble&Malfunction"）together, when roll result is equal or above x, and fumble happens simultaneously.
If not a fumble, outputs text "Malfunction" regardless of success/failure（Outputs the overwritten result, not outputting success/failure）

・CCB(x) c=5、f=96
Same as above
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`en_us` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:en_us`）を見るため
    /// `Success` / `Failure` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&EN_US, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :en_us`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&EN_US, command, rng)
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
            .join("test/data/Cthulhu_English.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Cthulhu_English.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Cthulhu_English.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Cthulhu_English.toml must parse");
        assert_eq!(
            data.tests.len(),
            105,
            "case count in test/data/Cthulhu_English.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Cthulhu:English",
                "unexpected game system in Cthulhu_English.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Cthulhu:English"), &tc.input, &mut src) {
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
                    "FAIL Cthulhu:English:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Cthulhu:English cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
