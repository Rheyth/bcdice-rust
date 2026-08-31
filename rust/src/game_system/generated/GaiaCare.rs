//! P4で手書き移植した `lib/bcdice/game_system/GaiaCare.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Emoklore` を継承し、`register_prefix_from_super_class` で親の
//! 接頭辞を引き継ぐだけ（`@locale` も `:ja_jp` のまま）なので、判定の実装は
//! [`super::Emoklore`] のものをそのまま使い、ここにはメタデータだけを置く。
//! `Base#result_ndx`（`1D10>=5` などの汎用判定）も親と同じ既定実装で足りる。

use super::Emoklore::{eval_specific_command, JA_SYSTEM};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::GaiaCare`（ID: `GaiaCare`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaiaCare;

impl GameSystem for GaiaCare {
    fn id(&self) -> &'static str {
        "GaiaCare"
    }

    fn name(&self) -> &'static str {
        "ガイアケアTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "かいあけあTRPG"
    }

    fn help_message(&self) -> &'static str {
        r#"・技能値判定（xDM<=y / xDM<=yEz）
  "(個数)DM<=(判定値)"で指定します。
  ダイスの個数は省略可能で、省略した場合1個になります。
  個数や判定値には四則演算（+-*/）を使用できます。
  末尾にEzを付けるとダイス数にzを加算します。E-zで減算も可能です。
  例）2DM<=5 DM<=8 2+2DM<=5 → 4個で判定値5
      2DM<=5E2 → 4個で判定値5 / 3DM<=5E-1 → 2個で判定値5
  ※ダイス数が0以下になる場合は確定失敗

・技能値判定（sDAa+z)
  "(技能レベル)DA(能力値)+(ダイスボーナス)"で指定します。
  ダイスボーナスの個数は省略可能で、省略した場合0になります。
  技能レベルは1～3の数値、またはベース技能の場合"b"が入ります。
  ダイスの個数は技能レベルとダイスボーナスの個数により決定し、s+z個のダイスを振ります。（s="b"の場合はs=1）
  判定値はs+aとなります。（s="b"の場合はs=0）
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[-+*/\d]*DM<=", r"(B|\d*)DA"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
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
            .join("test/data/GaiaCare.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/GaiaCare.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/GaiaCare.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("GaiaCare.toml must parse");
        assert_eq!(data.tests.len(), 6, "case count in test/data/GaiaCare.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "GaiaCare",
                "unexpected game system in GaiaCare.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("GaiaCare"), &tc.input, &mut src) {
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
                    "FAIL GaiaCare:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} GaiaCare cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
