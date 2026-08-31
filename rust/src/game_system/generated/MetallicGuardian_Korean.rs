//! P4で手書き移植した `lib/bcdice/game_system/MetallicGuardian_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `MetallicGuardian` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::MetallicGuardian`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 文言は `i18n/SRS/ko_kr.yml`（`auto_success` / `auto_failure`）と
//! `i18n/ko_kr.yml`（`success` / `failure`）から写したもので、値は1文字も変えていない。

use super::MetallicGuardian::{eval_specific_command, SystemTables};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// `ko_kr` ロケールの設定と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    notations: &["2D6", "MG"],
    auto_success: "자동 성공",
    auto_failure: "자동 실패",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::MetallicGuardian_Korean`（ID: `MetallicGuardian:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetallicGuardian_Korean;

impl GameSystem for MetallicGuardian_Korean {
    fn id(&self) -> &'static str {
        "MetallicGuardian:Korean"
    }

    fn name(&self) -> &'static str {
        "메탈릭 가디언 RPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:메탈릭 가디언 RPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　・通常判定：2D6+m@c#f>=t または 2D6+m>=t[c,f]
　　修正値m、目標値t、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]ごと省略可、@c・#fの指定は順不同）。
　　クリティカル値、ファンブル値の既定値は、それぞれ12、2です。
　　自動成功、自動失敗、成功、失敗を自動表示します。

　　例) 2d6>=10　　　　　修正値0、目標値10で判定
　　例) 2d6+2>=10　　　　修正値+2、目標値10で判定
　　例) 2d6+2>=10[11]　　↑をクリティカル値11で判定
　　例) 2d6+2@11>=10 　　↑をクリティカル値11で判定
　　例) 2d6+2>=10[12,4]　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4>=10 　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2>=10[,4]　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) 2d6+2#4>=10　　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) MG+2>=10　　　　 2d6+2>=10と同じ（MGが2D6のショートカットコマンド）

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) MG　　　　　 2d6[]と同じ（MGが2D6のショートカットコマンド）
　　例) MG+2@12#4　　2d6+2@12#4と同じ（MGが2D6のショートカットコマンド）

・D66ダイスあり（入れ替えなし)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6", "MG"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SRS#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `SRS#initialize` の `@d66_sort_type = D66SortType::NO_SORT`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    /// Ruby `SRS#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
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
            .join("test/data/MetallicGuardian_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MetallicGuardian_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MetallicGuardian_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MetallicGuardian_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            19,
            "case count in test/data/MetallicGuardian_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MetallicGuardian:Korean",
                "unexpected game system in MetallicGuardian_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("MetallicGuardian:Korean"),
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
                    "FAIL MetallicGuardian:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MetallicGuardian:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
