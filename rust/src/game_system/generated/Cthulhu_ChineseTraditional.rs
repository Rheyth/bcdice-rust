//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_ChineseTraditional.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:zh_hant` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `zh_hant` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/zh_hant.yml` と `i18n/zh_hant.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `zh_hant` ロケールの文言一式。
static ZH_HANT: Locale = Locale {
    success: "成功",
    failure: "失敗",
    critical: "決定性的成功",
    special: "特殊",
    critical_special: "決定性的成功/特殊",
    fumble: "致命性失敗",
    partial_success: "部分性成功",
    automatic_success: "自動成功",
    automatic_failure: "自動失敗",
    broken: "故障",
    broken_number: "故障率",
};

/// Ruby `BCDice::GameSystem::Cthulhu_ChineseTraditional`（ID: `Cthulhu:ChineseTraditional`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_ChineseTraditional;

impl GameSystem for Cthulhu_ChineseTraditional {
    fn id(&self) -> &'static str {
        "Cthulhu:ChineseTraditional"
    }

    fn name(&self) -> &'static str {
        "克蘇魯神話"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Chinese Traditional:克蘇魯神話"
    }

    fn help_message(&self) -> &'static str {
        r"c=爆擊率 ／ f=大失敗值 ／ s=特殊

1d100<=n    c・f・s全關閉（只進行單純數值比較判定）

・cfs付註判定指令

CC	 1d100擲骰 c=1、f=100
CCB  同上、c=5、f=96

例：CC<=80  （以技能值80來判定。cf適用於1%規則）
例：CCB<=55 （以技能值55來判定。cf適用於5%規則）

・關於組合骰組

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・關於對抗骰
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※故障率判定

・CC(x) c=1、f=100
x=故障率。擲出骰值x以上時、需在大失敗發生同時輸出（參照「大失敗＆故障」）
沒有大失敗時，無論成功或失敗只需參考[故障]來輸出(並非成功或失敗來輸出，而是覆蓋上去並對其輸出)

・CCB(x) c=5、f=96
同上

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`zh_hant` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:zh_hant`）を見るため
    /// `成功` / `失敗` になる。値は `ja_jp` と同じだが、参照するYAMLが違うので
    /// 他のバリアントと同じ形で明示的に上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&ZH_HANT, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :zh_hant`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&ZH_HANT, command, rng)
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
            .join("test/data/Cthulhu_ChineseTraditional.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Cthulhu_ChineseTraditional.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Cthulhu_ChineseTraditional.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Cthulhu_ChineseTraditional.toml must parse");
        assert_eq!(
            data.tests.len(),
            100,
            "case count in test/data/Cthulhu_ChineseTraditional.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Cthulhu:ChineseTraditional",
                "unexpected game system in Cthulhu_ChineseTraditional.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("Cthulhu:ChineseTraditional"),
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
                    "FAIL Cthulhu:ChineseTraditional:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Cthulhu:ChineseTraditional cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
