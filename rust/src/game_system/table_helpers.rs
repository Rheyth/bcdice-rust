//! Ruby `Base#roll_tables` の共通実装。
//!
//! 生成コード各ファイルにコピペされていた「コマンド名で表を引く」処理を集約する。
//! Ruby の `Base#roll_tables(command, tables)` は
//! `tables[command]&.roll(randomizer)&.to_s` に相当し、未定義コマンドは `nil`（`Ok(None)`）。
//!
//! 引数の持ち方（`TABLES` 直参照 / スライス / ロケール別 `SystemTables`）は
//! 呼び出し側ごとに異なるため、配列スライス版を基本に各ラッパを提供する。
//!
//! 生成コード側でこの形のローカル関数を新設することは禁止（R3 移植規約）。

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// Ruby `Base#roll_tables(command, tables)`。表を引けたら `RollResult#to_s` を返す。
///
/// `RollableTable` を実装する表（`Table` / `D66Table` / `ChainTable` 等）用。
pub(crate) fn roll_rollable_table(
    command: &str,
    tables: &[(&str, &dyn RollableTable)],
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// [`roll_rollable_table`] の `Table` 専用版。
///
/// `TABLES` が `&[(&str, &Table)]` のシステム（`&Table` は `&dyn RollableTable` に
/// 自動アップキャストされないため、型合わせのための明示的な多重定義）。
pub(crate) fn roll_plain_table(
    command: &str,
    tables: &[(&str, &Table)],
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}
