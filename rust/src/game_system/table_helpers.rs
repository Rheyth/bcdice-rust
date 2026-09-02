//! Ruby `Base#roll_tables` の共通実装。
//!
//! 生成コード各ファイルにコピペされていた「コマンド名で表を引く」処理を集約する。
//! Ruby の `Base#roll_tables(command, tables)` は
//! `tables[command]&.roll(randomizer)&.to_s` に相当し、未定義コマンドは `nil`（`Ok(None)`）。
//!
//! 生成コード側でこの形のローカル関数を新設することは禁止（R3 移植規約）。
//! 具体型（`&Table` / `&D66GridTable` 等のスライス）もトレイトオブジェクト
//! （`&dyn RollableTable`）と同じ関数で扱えるよう、ジェネリクスで受ける。

use crate::dice_table::{RangeTable, RollableTable};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// Ruby `Base#roll_tables(command, tables)`。表を引けたら `RollResult#to_s` を返す。
///
/// `T` は [`RollableTable`] を実装する具体型（`Table` 等）か
/// トレイトオブジェクト型（`dyn RollableTable`）。
pub(crate) fn roll_table<T: ?Sized + RollableTable>(
    command: &str,
    tables: &[(&str, &T)],
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `Base#roll_tables(command, tables)` の [`RangeTable`] 版。
///
/// `RangeTable` は `RollableTable` を実装しない（結果型が `RangeRollResult`）ため
/// 切り分けている。既定の整形は `{name}({sum}) ＞ {content}`。
pub(crate) fn roll_range_table(
    command: &str,
    tables: &[(&str, &RangeTable)],
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}
