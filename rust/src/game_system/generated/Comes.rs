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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Comes", "Comes.toml", 2);
    }
}
