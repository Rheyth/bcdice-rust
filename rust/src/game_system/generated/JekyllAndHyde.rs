//! P4で手書き移植した `lib/bcdice/game_system/JekyllAndHyde.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 親 `DesperateRun#eval_game_system_specific_command`
//!   （`check_roll` → `ddc_table` → `roll_tables` の順で試す）
//! - `JekyllAndHyde::TABLES`（`GOALT` の目標決定表）
//!
//! # 親クラスの扱い
//!
//! Ruby側は `DesperateRun` を継承するが、Rust側の
//! [`super::DesperateRun`] はまだスタブなので、親の評価ロジック
//! （`#check_roll` / `#ddc_table`）はこのファイルへ取り込んである。
//! 親が移植されたら整理する前提。
//!
//! 表は `roll_tables(command, self.class::TABLES)` で `JekyllAndHyde::TABLES` を引くので、
//! 親の `ACT` / `ITEMT` などは対象外（登録済み接頭辞も `RC` / `DDC` / `GOALT` だけ）。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::{RollableTable, Table};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// Ruby `TABLES["GOALT"]` の項目。
static GOALT_ITEMS: &[&str] = &[
    "「主人格の目的達成」",
    "「主人格の目的阻害」",
    "「主人格のハッピーエンド（目的達成しなくてもよい）」",
    "「主人格のバッドエンド（目的達成していてもよい）」",
    "「自分の人格が目的を決定できる」",
    "「主人格の目的達成」「主人格の目的阻害」「主人格のハッピーエンド（目的達成しなくてもよい）」「主人格のバッドエンド（目的達成していてもよい）」「自分の人格が目的を決定できる」のどれかを自由に選べる",
];

/// Ruby `TABLES["GOALT"]`（目標決定表）。
static GOALT_TABLE: Table = Table::from_dice("目標決定表", 1, 6, GOALT_ITEMS);

/// Ruby `JekyllAndHyde::TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[("GOALT", &GOALT_TABLE)];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Command::Parser.new(/RC\d+/, round_type: round_type).restrict_cmp_op_to(nil)`。
///
/// `round_type` は `Base` の既定（`RoundType::Floor`）。`DesperateRun` は上書きしない。
fn parser() -> &'static Parser {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    PARSER.get_or_init(|| Parser::new(&[r"RC\d+"], RoundType::Floor).restrict_cmp_op_to(&[None]))
}

/// Ruby `DesperateRun#check_roll(string)`。
fn check_roll(string: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(cmd) = parser().parse(string) else {
        return Ok(None);
    };

    let dice = rng.roll_barabara(2, 6)?;
    let (d1, d2) = (dice[0], dice[1]);
    let dice_total = d1 + d2;
    let total = d1 + d2 + cmd.modify_number.clone();
    // notation が `RC\d+` なので `[2..]` は必ず十進数字列。
    let target: i64 = cmd.command[2..].parse().unwrap_or(0);

    let modifier_str = if cmd.modify_number != I::ZERO {
        format!("　修正値：{}", cmd.modify_number)
    } else {
        String::new()
    };

    let mut result = if d1 == d2 {
        EvalResult::critical("ゾロ目！【Critical】")
    } else if dice_total == 7 {
        EvalResult::fumble("ダイスの出目が表裏！【Fumble】")
    } else if total >= crate::Int::from(target) {
        EvalResult::success(format!("{total}、難易度以上！【Success】"))
    } else {
        EvalResult::failure(format!("{total}、難易度未満！【Miss】"))
    };

    result.text = format!(
        "判定　難易度：{target}{modifier_str} ＞ 出目：{d1}、{d2} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `DesperateRun#ddc_table(command)`。
fn ddc_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "DDC" {
        return Ok(None);
    }

    let dice = rng.roll_barabara(2, 6)?;
    let (d1, d2) = (dice[0], dice[1]);

    let (smaller, larger) = if d1 <= d2 { (d1, d2) } else { (d2, d1) };
    let difference = larger - smaller;

    Ok(Some(format!(
        "難易度決定 ＞ 出目：{d1}、{d2} ＞ {larger}-{smaller}={difference} ＞ 難易度{}",
        5 + difference
    )))
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `BCDice::GameSystem::JekyllAndHyde`（ID: `JekyllAndHyde`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JekyllAndHyde;

impl GameSystem for JekyllAndHyde {
    fn id(&self) -> &'static str {
        "JekyllAndHyde"
    }

    fn name(&self) -> &'static str {
        "ジキルとハイドとグリトグラ"
    }

    fn sort_key(&self) -> &'static str {
        "しきるとはいととくりとくら"
    }

    fn help_message(&self) -> &'static str {
        r"・難易度算出コマンド　DDC
・判定コマンド　RCx　or　RCx+y　or　RCx-y（x＝難易度、y=修正値（省略可能））
・目標決定表　GOALT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["RC", "DDC", "GOALT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DesperateRun#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `DesperateRun#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `DesperateRun#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = ddc_table(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/JekyllAndHyde.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/JekyllAndHyde.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/JekyllAndHyde.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("JekyllAndHyde.toml must parse");
        assert_eq!(
            data.tests.len(),
            20,
            "case count in test/data/JekyllAndHyde.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "JekyllAndHyde",
                "unexpected game system in JekyllAndHyde.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("JekyllAndHyde"), &tc.input, &mut src) {
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
                    "FAIL JekyllAndHyde:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} JekyllAndHyde cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
