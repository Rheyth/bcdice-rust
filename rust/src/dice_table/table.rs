//! Ruby `BCDice::DiceTable::Table`（lib/bcdice/dice_table/table.rb）の移植。

use super::{parse_dice_notation, RollResult, RollableTable};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 出目の合計をそのまま項目の添字に使う表。
///
/// 例: `"2D6"` の表なら `items[0]` が合計2、`items[10]` が合計12に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Table {
    name: &'static str,
    times: i64,
    sides: i64,
    items: &'static [&'static str],
}

impl Table {
    /// ダイスの個数と面数を直接指定して構築する（`const` で表を書くための入口）。
    pub const fn from_dice(
        name: &'static str,
        times: i64,
        sides: i64,
        items: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            times,
            sides,
            items,
        }
    }

    /// Ruby `Table.new(name, type, items)`。`type` は `"1D6"` のような文字列。
    ///
    /// # Panics
    ///
    /// `type` が `nDm` 形式を含まない場合にパニックする
    /// （Ruby は `ArgumentError, "Unexpected table type: ..."` を投げる）。
    pub fn new(name: &'static str, dice_type: &str, items: &'static [&'static str]) -> Self {
        let (times, sides) = parse_dice_notation(dice_type)
            .unwrap_or_else(|| panic!("Unexpected table type: {dice_type}"));
        Self::from_dice(name, times, sides, items)
    }

    /// 表の名前。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 振るダイスの個数。
    pub fn times(&self) -> i64 {
        self.times
    }

    /// 振るダイスの面数。
    pub fn sides(&self) -> i64 {
        self.sides
    }

    /// Ruby `#choice(value)`。出目の合計に対応する項目を選ぶ。
    ///
    /// 添字が範囲外の場合、本体は空文字列になる（mod.rs のdoc参照）。
    pub fn choice(&self, value: i64) -> RollResult {
        let index = value - self.times;
        let body = usize::try_from(index)
            .ok()
            .and_then(|i| self.items.get(i))
            .copied()
            .unwrap_or("");
        RollResult::text(self.name, value, body)
    }
}

impl RollableTable for Table {
    /// Ruby `#roll(bcdice)`: `roll_sum(times, sides)` の結果で `choice` する。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let value = rng.roll_sum(self.times, self.sides)?;
        Ok(self.choice(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    static ITEMS: &[&str] = &["2の項目", "3の項目", "4の項目"];
    static TABLE: Table = Table::from_dice("テスト表", 2, 2, ITEMS);

    fn roll_with(table: &Table, rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn chooses_item_by_sum() {
        assert_eq!(
            roll_with(&TABLE, &[(1, 2), (1, 2)]),
            "テスト表(2) ＞ 2の項目"
        );
        assert_eq!(
            roll_with(&TABLE, &[(2, 2), (1, 2)]),
            "テスト表(3) ＞ 3の項目"
        );
        assert_eq!(
            roll_with(&TABLE, &[(2, 2), (2, 2)]),
            "テスト表(4) ＞ 4の項目"
        );
    }

    #[test]
    fn parses_dice_type_string() {
        let t = Table::new("表", "2D6", ITEMS);
        assert_eq!((t.times(), t.sides()), (2, 6));
        assert_eq!(t.name(), "表");
    }

    #[test]
    #[should_panic(expected = "Unexpected table type")]
    fn invalid_dice_type_panics() {
        Table::new("表", "D6", ITEMS);
    }

    #[test]
    fn out_of_range_index_gives_empty_body() {
        // Ruby は nil を返して "表(値) ＞ " になる。負の添字も回り込ませない。
        assert_eq!(TABLE.choice(9).to_string(), "テスト表(9) ＞ ");
        assert_eq!(TABLE.choice(0).to_string(), "テスト表(0) ＞ ");
    }
}
