//! P4で手書き移植した `lib/bcdice/game_system/SevenFortressMobius.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `NightWizard` を継承し、`initialize` で `@nw_command` を
//! `"SFM"` に変えるだけなので、判定の実装は [`super::NightWizard`] のものを
//! そのまま使い、ここには判定コマンドの語の設定だけを置く。

use std::sync::OnceLock;

use regex::Regex;

use super::NightWizard::{build_nw_pattern, eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `SevenFortressMobius#initialize` の `@nw_command = "SFM"`。
fn sfm_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| build_nw_pattern("SFM"))
}

/// `SevenFortressMobius` の設定一式。
static SFM_SYSTEM: SystemTables = SystemTables {
    nw_command: "SFM",
    nw_pattern: sfm_pattern,
};

/// Ruby `BCDice::GameSystem::SevenFortressMobius`（ID: `SevenFortressMobius`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SevenFortressMobius;

impl GameSystem for SevenFortressMobius {
    fn id(&self) -> &'static str {
        "SevenFortressMobius"
    }

    fn name(&self) -> &'static str {
        "セブン＝フォートレス メビウス"
    }

    fn sort_key(&self) -> &'static str {
        "せふんふおおとれすめひうす"
    }

    fn help_message(&self) -> &'static str {
        r#"・判定用コマンド　(nSFM+m@x#y)
　"(基本値)SFM(常時および常時に準じる特技等及び状態異常（省略可）)@(クリティカル値)#(ファンブル値)（常時以外の特技等及び味方の支援効果等の影響（省略可））"でロールします。
　Rコマンド(2R6m[n,m]c[x]f[y]>=t tは目標値)に読替されます。
　クリティカル値、ファンブル値が無い場合は1や13などのあり得ない数値を入れてください。
　例）12SFM-5@7#2　　1SFM　　50SFM+5@7,10#2,5　50SFM-5+10@7,10#2,5+15+25
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"([-+]?\d+)?SFM", "2R6"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&SFM_SYSTEM, command, rng)
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
            .join("test/data/SevenFortressMobius.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/SevenFortressMobius.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/SevenFortressMobius.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("SevenFortressMobius.toml must parse");
        assert_eq!(
            data.tests.len(),
            4,
            "case count in test/data/SevenFortressMobius.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "SevenFortressMobius",
                "unexpected game system in SevenFortressMobius.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("SevenFortressMobius"),
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
                    "FAIL SevenFortressMobius:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} SevenFortressMobius cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
