//! Ruby `BCDice::DiceTable::D66GridTable` / `D66HalfGridTable` / `D66OneThirdTable`
//! （lib/bcdice/dice_table/d66_{grid,half_grid,one_third}_table.rb）の移植。
//!
//! Rubyでは `D66HalfGridTable` / `D66OneThirdTable` は行を複製して
//! `D66GridTable` に渡す継承関係にあるが、本移植は複製せず
//! 「行選択関数を持つ単一構造体 [`D66RowSplitTable`]」に統一した（同値）。
//! 行選択は左ダイス（1〜6）から行番号（0始まり）を選ぶ関数 `fn(i64) -> usize` で行う。

use super::{roll_d66_key_no_sort, RollResult, RollableTable};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// D66を振って6×6マスの表を引く。
#[derive(Debug, Clone, Copy)]
pub struct D66GridTable {
    name: &'static str,
    items: &'static [&'static [&'static str]],
}

impl D66GridTable {
    /// Ruby `D66GridTable.new(name, items)`。`items` は6行×6列。
    pub const fn new(name: &'static str, items: &'static [&'static [&'static str]]) -> Self {
        Self { name, items }
    }
}

impl RollableTable for D66GridTable {
    /// Ruby `#roll(randomizer)`: `roll_once(6)` を2回振り `items[d1-1][d2-1]`。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let key = roll_d66_key_no_sort(rng)?;
        let (d1, d2) = (key / 10, key % 10);
        let body = grid_cell(self.items, d1, d2);
        Ok(RollResult::text(self.name, key, body))
    }
}

/// 行を2分割するD66表。Ruby `D66HalfGridTable` / `D66OneThirdTable` の統一形。
///
/// Rubyは `[rows[0]] * n0 + [rows[1]] * n1 + ...` のように6行へ複製してから
/// `D66GridTable` として引くが、本移植は複製せず左ダイスから行を選ぶ
/// （左ダイス `d1` に対し `row_index(d1)` 行を引く。結果は同じ）。
#[derive(Debug, Clone, Copy)]
pub struct D66RowSplitTable {
    name: &'static str,
    /// 分割後のユニークな行（複製前のもの）。未使用スロットは空行。
    rows: [&'static [&'static str]; 3],
    /// 左ダイス（1〜6）から行番号（0始まり）を選ぶ関数。
    row_index: fn(i64) -> usize,
}

impl D66RowSplitTable {
    /// Ruby `D66HalfGridTable.new(name, items_1_2_3, items_4_5_6)` 相当。
    /// 左ダイス1〜3で `rows[0]`、4〜6で `rows[1]` を引く。
    pub const fn half_grid(
        name: &'static str,
        items_1_2_3: &'static [&'static str],
        items_4_5_6: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            rows: [items_1_2_3, items_4_5_6, &[]],
            row_index: |d1| if d1 <= 3 { 0 } else { 1 },
        }
    }

    /// Ruby `D66OneThirdTable.new(name, items_1_2, items_3_4, items_5_6)` 相当。
    /// 左ダイス1〜2 / 3〜4 / 5〜6 で行を切り替える。
    pub const fn one_third(
        name: &'static str,
        items_1_2: &'static [&'static str],
        items_3_4: &'static [&'static str],
        items_5_6: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            rows: [items_1_2, items_3_4, items_5_6],
            row_index: |d1| ((d1 - 1) / 2) as usize,
        }
    }

    /// 左ダイスに対応する行を返す（範囲外は空行扱いで空文字列）。
    fn row(&self, d1: i64) -> &'static [&'static str] {
        let index = (self.row_index)(d1);
        self.rows.get(index).copied().unwrap_or(&[])
    }
}

impl RollableTable for D66RowSplitTable {
    /// Ruby `#roll(randomizer)`: 2D66を振り、左ダイスで行を選んで `row[d2-1]`。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let key = roll_d66_key_no_sort(rng)?;
        let (d1, d2) = (key / 10, key % 10);
        Ok(RollResult::text(self.name, key, row_cell(self.row(d1), d2)))
    }
}

/// `items[dice1 - 1][dice2 - 1]`。範囲外は空文字列（mod.rs のdoc参照）。
fn grid_cell(items: &'static [&'static [&'static str]], dice1: i64, dice2: i64) -> &'static str {
    usize::try_from(dice1 - 1)
        .ok()
        .and_then(|i| items.get(i))
        .map(|row| row_cell(row, dice2))
        .unwrap_or("")
}

/// `row[dice2 - 1]`。範囲外は空文字列。
fn row_cell(row: &'static [&'static str], dice2: i64) -> &'static str {
    usize::try_from(dice2 - 1)
        .ok()
        .and_then(|i| row.get(i))
        .copied()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    fn roll_with(table: &dyn RollableTable, rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    static ROW1: &[&str] = &["11", "12", "13", "14", "15", "16"];
    static ROW2: &[&str] = &["21", "22", "23", "24", "25", "26"];
    static ROW3: &[&str] = &["31", "32", "33", "34", "35", "36"];
    static ROW4: &[&str] = &["41", "42", "43", "44", "45", "46"];
    static ROW5: &[&str] = &["51", "52", "53", "54", "55", "56"];
    static ROW6: &[&str] = &["61", "62", "63", "64", "65", "66"];
    static GRID: &[&[&str]] = &[ROW1, ROW2, ROW3, ROW4, ROW5, ROW6];

    #[test]
    fn grid_table_indexes_by_both_dice() {
        let t = D66GridTable::new("表", GRID);
        assert_eq!(roll_with(&t, &[(1, 6), (1, 6)]), "表(11) ＞ 11");
        assert_eq!(roll_with(&t, &[(4, 6), (2, 6)]), "表(42) ＞ 42");
        assert_eq!(roll_with(&t, &[(6, 6), (6, 6)]), "表(66) ＞ 66");
    }

    #[test]
    fn half_grid_table_splits_at_three() {
        // Ruby の [a,a,a,b,b,b] 複製と同値であることを全36通りで確認する
        let t = D66RowSplitTable::half_grid("表", ROW1, ROW4);
        static EXPANDED: &[&[&str]] = &[ROW1, ROW1, ROW1, ROW4, ROW4, ROW4];
        let expanded = EXPANDED;
        let grid = D66GridTable::new("表", expanded);
        for d1 in 1..=6 {
            for d2 in 1..=6 {
                let rands = [(d1, 6), (d2, 6)];
                assert_eq!(roll_with(&t, &rands), roll_with(&grid, &rands));
            }
        }
        assert_eq!(roll_with(&t, &[(3, 6), (5, 6)]), "表(35) ＞ 15");
        assert_eq!(roll_with(&t, &[(4, 6), (5, 6)]), "表(45) ＞ 45");
    }

    #[test]
    fn one_third_table_splits_in_three() {
        let t = D66RowSplitTable::one_third("表", ROW1, ROW3, ROW5);
        static EXPANDED: &[&[&str]] = &[ROW1, ROW1, ROW3, ROW3, ROW5, ROW5];
        let expanded = EXPANDED;
        let grid = D66GridTable::new("表", expanded);
        for d1 in 1..=6 {
            for d2 in 1..=6 {
                let rands = [(d1, 6), (d2, 6)];
                assert_eq!(roll_with(&t, &rands), roll_with(&grid, &rands));
            }
        }
        assert_eq!(roll_with(&t, &[(2, 6), (1, 6)]), "表(21) ＞ 11");
        assert_eq!(roll_with(&t, &[(5, 6), (1, 6)]), "表(51) ＞ 51");
    }

    #[test]
    fn out_of_range_row_gives_empty_body() {
        static SHORT: &[&[&str]] = &[ROW1];
        let short = SHORT;
        let t = D66GridTable::new("表", short);
        assert_eq!(roll_with(&t, &[(2, 6), (1, 6)]), "表(21) ＞ ");
    }
}
