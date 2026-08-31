//! P4で手書き移植した `lib/bcdice/game_system/TokyoGhostResearch.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#tgr_opening_table`（導入表 `OP`）
//! - `#tgr_common_trouble_table`（一般トラブル表 `TB`）
//! - `#getCheckResult`（`TK` 系。原典のバグをそのまま再現する。下記参照）

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `#tgr_opening_table`（導入表 1D10）。
static OPENING_TABLE: Table = Table::from_dice(
    "導入表",
    1,
    10,
    &[
        "【病休中断】体調不良または怪我で療養中だったが強制召喚された。",
        "【忙殺中】別の業務で忙殺中であった。",
        "【出張帰り】遠方での仕事から戻ったばかり。",
        "【休暇取り消し】休暇中だったが呼び戻された。",
        "【平常運転】いつもどおりの仕事中だった。",
        "【休暇明け】十分に休養をとったあとで、心身ともに充実している。",
        "【人生の岐路】人生の岐路にまさに差し掛かったところであった。",
        "【同窓会】かつての同級生に会い、差を実感したばかりだった。",
        "【転職活動中】転職を考えて求人サイトを見ているところだった。",
        "【サボリ中】仕事をサボっているところに呼び出しがあった。",
    ],
);

/// Ruby `#tgr_common_trouble_table`（一般トラブル表 1D10）。
static COMMON_TROUBLE_TABLE: Table = Table::from_dice(
    "一般トラブル表",
    1,
    10,
    &[
        "トラブルが生じたが、間一髪、危機を脱した。【ダメージなし】",
        "どうにかタスクを処理したが、非常に疲労してしまった。【肉体ダメージ1点】",
        "タスク処理の過程で負傷してしまった。【肉体ダメージ1点】",
        "恐怖や混乱、ストレスなどで精神の均衡を崩してしまった。【精神ダメージ1点】",
        "過去のトラウマなどを思い出し、気分が沈んでしまった。【精神ダメージ1点】",
        "自身の信用をキズつけたり、汚名を背負ってしまった。【環境ダメージ1点】",
        "会社や上司の不興を買ってしまった。【環境ダメージ1点】",
        "疲労困憊で動くこともままならない。【肉体ダメージ1点＋精神ダメージ1点】",
        "負傷したうえ、会社に損害を与えてしまった。【肉体ダメージ1点＋環境ダメージ1点】",
        "上司から厳しく叱責され、まずい立場になった。【精神ダメージ1点＋環境ダメージ1点】",
    ],
);

pub struct TokyoGhostResearch;

impl GameSystem for TokyoGhostResearch {
    fn id(&self) -> &'static str {
        "TokyoGhostResearch"
    }

    fn name(&self) -> &'static str {
        "東京ゴーストリサーチ"
    }

    fn sort_key(&self) -> &'static str {
        "とうきようこおすとりさあち"
    }

    fn help_message(&self) -> &'static str {
        r"判定
・タスク処理は目標値以上の値で成功となります。
  1d10>={目標値}
  例：目標値「5」の場合、5～0で成功
各種表
  ・導入表  OP
  ・一般トラブル表  TB
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["OP", "TB", "TK?"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command.upcase when /TK/i
        if command.contains("TK") {
            // Ruby `#getCheckResult` は `/TK?<=(\d+)/i` に1つしかグループが無いのに
            // `Regexp.last_match(2)` を読むため `nil.to_i` → `diff = 0` となり、
            // `if diff > 0` に入らず常に空文字列を返す（原典のバグ）。
            // 空文字列は `dice_command` が nil に畳むので、共通コマンドへ抜ける。
            return Ok(Some(SpecificCommandOutput::text(String::new())));
        }

        let table = match command {
            "OP" => &OPENING_TABLE,
            "TB" => &COMMON_TROUBLE_TABLE,
            _ => return Ok(None),
        };

        Ok(Some(SpecificCommandOutput::text(
            table.roll(rng)?.to_string(),
        )))
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
            .join("test/data/TokyoGhostResearch.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TokyoGhostResearch.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TokyoGhostResearch.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TokyoGhostResearch.toml must parse");
        assert_eq!(
            data.tests.len(),
            27,
            "case count in test/data/TokyoGhostResearch.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TokyoGhostResearch",
                "unexpected game system in TokyoGhostResearch.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("TokyoGhostResearch"),
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
                    "FAIL TokyoGhostResearch:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TokyoGhostResearch cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
