//! P4で手書き移植した `lib/bcdice/game_system/Comes.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Comes#eval_game_system_specific_command` → `Base#roll_tables` と `TABLES`（`PT`）
//!
//! # 表データ
//!
//! Ruby側は `DiceTable::Table` をクラス定数として直接書いている（i18n未対応）。
//! Rust側も同じ値を `static` として持ち、値は1文字も変えていない。

use crate::dice_table::{RollableTable, Table};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `TABLES['PT']` の項目。
static PT_ITEMS: &[&str] = &[
    "恐ろしい目に合う。『恐怖』を与える。",
    "今見ているものを理解できない。『混乱』を与える。",
    "我を忘れて見とれてしまう。『魅了』を与える。",
    "思わぬ遠回りをしてしまう。『疲労』を与える。",
    "大きな失態を演じてしまう。『負傷』を与える。",
    "別の困難が立ちはだかる。新たに判定を行わせる。",
];

/// Ruby `TABLES['PT']`（判定ペナルティ表）。
static PT_TABLE: Table = Table::from_dice("判定ペナルティ表", 1, 6, PT_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[("PT", &PT_TABLE)];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `BCDice::GameSystem::Comes`（ID: `Comes`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comes;

impl GameSystem for Comes {
    fn id(&self) -> &'static str {
        "Comes"
    }

    fn name(&self) -> &'static str {
        "カムズ"
    }

    fn sort_key(&self) -> &'static str {
        "かむす"
    }

    fn help_message(&self) -> &'static str {
        r"・各種表
　判定ペナルティ表 PT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["PT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Comes#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Comes#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Comes#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
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
            .join("test/data/Comes.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Comes.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Comes.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Comes.toml must parse");
        assert_eq!(data.tests.len(), 2, "case count in test/data/Comes.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Comes",
                "unexpected game system in Comes.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Comes"), &tc.input, &mut src) {
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
                    "FAIL Comes:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Comes cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
