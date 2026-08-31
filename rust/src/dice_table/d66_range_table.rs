//! Ruby `BCDice::DiceTable::D66RangeTable`（lib/bcdice/dice_table/d66_range_table.rb）の移植。

use super::{RangeInc, RollResult, RollableTable};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 項目を範囲（`11..16` など）で指定するD66表。出目の入れ替えは行わない。
#[derive(Debug, Clone, Copy)]
pub struct D66RangeTable {
    name: &'static str,
    items: &'static [(RangeInc, &'static str)],
}

impl D66RangeTable {
    /// Ruby `D66RangeTable.new(name, items)`。
    pub const fn new(name: &'static str, items: &'static [(RangeInc, &'static str)]) -> Self {
        Self { name, items }
    }

    /// 表の名前。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// キーに対応する項目を返す。Ruby `@items.find { |row| row[0].include?(key) }`。
    ///
    /// Ruby は該当なしの場合 `nil[1]` で NoMethodError になるが、
    /// ここでは空文字列にする（mod.rs のdoc参照）。
    pub fn fetch(&self, key: i64) -> &'static str {
        self.items
            .iter()
            .find(|(range, _)| range.includes(key))
            .map(|(_, content)| *content)
            .unwrap_or("")
    }
}

impl RollableTable for D66RangeTable {
    /// Ruby `#roll(randomizer)`: `roll_once(6)` を2回振り、`d1*10+d2` を引く。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let dice1 = rng.roll_once(6)?;
        let dice2 = rng.roll_once(6)?;
        let key = dice1 * 10 + dice2;
        Ok(RollResult::text(self.name, key, self.fetch(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    static ITEMS: &[(RangeInc, &str)] = &[
        (RangeInc::new(11, 26), "前半"),
        (RangeInc::new(31, 46), "中盤"),
        (RangeInc::new(51, 66), "後半"),
    ];
    static TABLE: D66RangeTable = D66RangeTable::new("範囲D66表", ITEMS);

    fn roll_with(rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = TABLE.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn picks_item_by_range() {
        assert_eq!(roll_with(&[(1, 6), (1, 6)]), "範囲D66表(11) ＞ 前半");
        assert_eq!(roll_with(&[(4, 6), (2, 6)]), "範囲D66表(42) ＞ 中盤");
        assert_eq!(roll_with(&[(6, 6), (6, 6)]), "範囲D66表(66) ＞ 後半");
    }

    #[test]
    fn does_not_swap_dice() {
        // D66RangeTable は入れ替えを行わない（61 は 16 にならない）
        assert_eq!(roll_with(&[(6, 6), (1, 6)]), "範囲D66表(61) ＞ 後半");
    }

    #[test]
    fn unmatched_key_gives_empty_body() {
        assert_eq!(TABLE.fetch(27), "");
    }
}
