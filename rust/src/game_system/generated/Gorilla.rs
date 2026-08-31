//! P4で手書き移植した `lib/bcdice/game_system/Gorilla.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Gorilla#change_text`（`G` を `2D6` に展開するショートカット）
//! - `Gorilla#result_2d6`（出目 `[5,5]` のゴリティカル）

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::Gorilla`（ID: `Gorilla`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gorilla;

impl GameSystem for Gorilla {
    fn id(&self) -> &'static str {
        "Gorilla"
    }

    fn name(&self) -> &'static str {
        "ゴリラTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "こりらTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"2D6ロール時のゴリティカル自動判定を行います。

G = 2D6のショートカット

例) G>=7 : 2D6して7以上なら成功
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["G"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Gorilla#change_text`。
    ///
    /// Ruby: `string.gsub(/^(S)?G/i) { "#{Regexp.last_match(1)}2D6" }`
    /// 先頭の `S`（シークレットロール）は残したまま `G` だけ `2D6` にする。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        // Rustの `$1` は「グループ12」と読まれ得るので `${1}` と明示する。
        shortcut_pattern().replace_all(text, "${1}2D6")
    }

    /// Ruby `Gorilla#result_2d6`。
    ///
    /// `[5,5]` 以外は `nil` を返して `Base#result_ndx` の成功/失敗判定へ落とす。
    fn result_2d6(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        if value_list == [5, 5] {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "ゴリティカル（自動的成功）",
            ))))
        } else {
            None
        }
    }
}

/// Ruby `/^(S)?G/i`。
fn shortcut_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?mi)^(S)?G").expect("valid regex"))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    /// 余った注入乱数を許すケース（`(1始まりのケース番号, 残り個数)`）。
    ///
    /// Ruby本家の `RandomizerMock` は余りを検査しないので、TOMLには
    /// 「Ruby側もダイスを振る前に nil を返すコマンド」にもダイスが書かれている。
    /// ケース89 (`DMG>10 比較演算子の不正`) は比較演算子 `>` が
    /// `restrict_cmp_op_to(:>=)` により不正で、Ruby も1個も振らない
    /// （Docker Ruby 3.2 実測: result=nil, rands unconsumed）。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[];

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/Gorilla.toml");
        path.exists().then_some(path)
    }

    /// `test/data/Gorilla.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Gorilla.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Gorilla.toml must parse");
        assert_eq!(data.tests.len(), 15, "case count in test/data/Gorilla.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Gorilla",
                "unexpected game system in Gorilla.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Gorilla"), &tc.input, &mut src) {
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

            let allowed_surplus = SURPLUS_RANDS_ALLOWED
                .iter()
                .find(|(case, _)| *case == i + 1)
                .map_or(0, |(_, remaining)| *remaining);
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Gorilla:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Gorilla cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
