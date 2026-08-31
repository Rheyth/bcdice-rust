//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:ko_kr` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `ko_kr` ロケールの文言一式。
static KO_KR: Locale = Locale {
    success: "성공",
    failure: "실패",
    critical: "크리티컬",
    special: "스페셜",
    critical_special: "크리티컬/스페셜",
    fumble: "펌블",
    partial_success: "부분적 성공",
    automatic_success: "자동성공",
    automatic_failure: "자동실패",
    broken: "고장",
    broken_number: "고장 수치",
};

/// Ruby `BCDice::GameSystem::Cthulhu_Korean`（ID: `Cthulhu:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_Korean;

impl GameSystem for Cthulhu_Korean {
    fn id(&self) -> &'static str {
        "Cthulhu:Korean"
    }

    fn name(&self) -> &'static str {
        "크툴루"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:크툴루"
    }

    fn help_message(&self) -> &'static str {
        r"c=크리티컬치 ／ f=펌블치 ／ s=스페셜

1d100<=n    c・f・s 모두 오프（단순하게 수치만을 뽑아낼 때 사용）

・cfs이 붙는 판정의 커맨드

CC	 1d100 판정을 행함 c=1、f=100
CCB  위와 동일、c=5、f=96

예：CC<=80  （기능치 80로 행휘판정. 1%룰으로 cf적용）
예：CCB<=55 （기능치 55로 행휘판정. 5%룰으로 cf적용）

・경우의 수 판정에 대해서

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・저항 판정에 대해서
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※고장 넘버 판정

・CC(x) c=1、f=100
x=고장 넘버. 주사위 눈x이상이 나온 후에, 펌블이 동시에 발생했을 경우. 모두 출력한다. （텍스트 「펌블＆고장」）
펌블이 아닌 경우, 성공・실패에 관련되지 않고 「고장」만을 출력한다. （성공・실패를 출력하지 않고 덧쓰기한 것을 출력하는 형태）

・CCB(x) c=5、f=96
위와 동일
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&KO_KR, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :ko_kr`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_KR, command, rng)
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
            .join("test/data/Cthulhu_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Cthulhu_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Cthulhu_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Cthulhu_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            100,
            "case count in test/data/Cthulhu_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Cthulhu:Korean",
                "unexpected game system in Cthulhu_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Cthulhu:Korean"), &tc.input, &mut src) {
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
                    "FAIL Cthulhu:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Cthulhu:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
