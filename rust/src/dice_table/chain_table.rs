//! Ruby `BCDice::DiceTable::ChainTable`（lib/bcdice/dice_table/chain_table.rb）の移植。

use super::{parse_dice_notation, RollBody, RollResult, RollableTable, TableItem};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 項目が別の表になりうる（連鎖する）表。
#[derive(Debug, Clone, Copy)]
pub struct ChainTable {
    name: &'static str,
    times: i64,
    sides: i64,
    items: &'static [TableItem],
}

impl ChainTable {
    /// ダイスの個数と面数を直接指定して構築する。
    pub const fn from_dice(
        name: &'static str,
        times: i64,
        sides: i64,
        items: &'static [TableItem],
    ) -> Self {
        Self {
            name,
            times,
            sides,
            items,
        }
    }

    /// Ruby `ChainTable.new(name, type, items)`。
    ///
    /// # Panics
    ///
    /// `type` が `nDm` 形式を含まない場合にパニックする（Ruby の `ArgumentError`）。
    pub fn new(name: &'static str, dice_type: &str, items: &'static [TableItem]) -> Self {
        let (times, sides) = parse_dice_notation(dice_type)
            .unwrap_or_else(|| panic!("Unexpected table type: {dice_type}"));
        Self::from_dice(name, times, sides, items)
    }

    /// 表の名前。
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl RollableTable for ChainTable {
    /// Ruby `#roll(randomizer)`。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let value = rng.roll_sum(self.times, self.sides)?;
        let index = value - self.times;
        let body = match usize::try_from(index).ok().and_then(|i| self.items.get(i)) {
            Some(item) => item.resolve(rng)?,
            None => RollBody::Text(""),
        };
        Ok(RollResult::new(self.name, value, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dice_table::Table;
    use crate::randomizer::SeededRandomizer;

    static INNER: Table = Table::from_dice("内側表", 1, 2, &["内側1", "内側2"]);
    static ITEMS: &[TableItem] = &[TableItem::Text("直接の項目"), TableItem::Table(&INNER)];
    static CHAIN: ChainTable = ChainTable::from_dice("連鎖表", 1, 2, ITEMS);

    fn roll_with(rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = CHAIN.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn text_item_is_returned_directly() {
        assert_eq!(roll_with(&[(1, 2)]), "連鎖表(1) ＞ 直接の項目");
    }

    #[test]
    fn table_item_is_rolled_and_nested() {
        // 外側で2 → 2番目の項目（内側表）を引く → 内側で1
        assert_eq!(
            roll_with(&[(2, 2), (1, 2)]),
            "連鎖表(2) ＞ 内側表(1) ＞ 内側1"
        );
    }
}
