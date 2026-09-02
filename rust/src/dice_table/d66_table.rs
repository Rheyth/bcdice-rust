//! Ruby `BCDice::DiceTable::D66Table`（lib/bcdice/dice_table/d66_table.rb）の移植。

use super::{roll_d66_key, RollBody, RollResult, RollableTable, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// D66を振り、出目を入れ替えてから項目を引く表。
///
/// Ruby側の項目は `Hash`（キーは `11`〜`66` の数値）。本移植は
/// `&'static [(i64, TableItem)]` の連想リストで持つ。
#[derive(Debug, Clone, Copy)]
pub struct D66Table {
    name: &'static str,
    sort_type: D66SortType,
    items: &'static [(i64, TableItem)],
}

impl D66Table {
    /// Ruby `D66Table.new(name, sort_type, items)`。
    pub const fn new(
        name: &'static str,
        sort_type: D66SortType,
        items: &'static [(i64, TableItem)],
    ) -> Self {
        Self {
            name,
            sort_type,
            items,
        }
    }

    /// キーに対応する項目を返す。Ruby の `Hash#[]` 相当（未登録なら `None`）。
    fn find(&self, key: i64) -> Option<&'static TableItem> {
        self.items.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Ruby `#choice(key)`。ダイスを振らずにキーで項目を引く。
    ///
    /// Ruby はネストした表を引き直さない（`@items[key]` をそのまま body にする）ため、
    /// ここでもネストは解決せず、表だった場合は空文字列にする。
    pub fn choice(&self, key: i64) -> RollResult {
        let body = match self.find(key) {
            Some(TableItem::Text(t)) => t,
            _ => "",
        };
        RollResult::text(self.name, key, body)
    }
}

impl RollableTable for D66Table {
    /// Ruby `#roll(randomizer)`: 2D66 → 入れ替え → キー参照。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let key = roll_d66_key(rng, self.sort_type)?;

        let body = match self.find(key) {
            Some(item) => item.resolve(rng)?,
            None => RollBody::Text(""),
        };
        Ok(RollResult::new(self.name, key, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dice_table::Table;
    use crate::randomizer::SeededRandomizer;

    static INNER: Table = Table::from_dice("内側表", 1, 2, &["内側1", "内側2"]);
    static ITEMS: &[(i64, TableItem)] = &[
        (11, TableItem::Text("ゾロ目1")),
        (16, TableItem::Text("1と6")),
        (61, TableItem::Text("6と1")),
        (66, TableItem::Table(&INNER)),
    ];

    fn roll_with(table: &D66Table, rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn no_sort_keeps_dice_order() {
        let t = D66Table::new("表", D66SortType::NoSort, ITEMS);
        assert_eq!(roll_with(&t, &[(6, 6), (1, 6)]), "表(61) ＞ 6と1");
        assert_eq!(roll_with(&t, &[(1, 6), (6, 6)]), "表(16) ＞ 1と6");
    }

    #[test]
    fn asc_and_desc_swap_dice() {
        let asc = D66Table::new("表", D66SortType::Asc, ITEMS);
        assert_eq!(roll_with(&asc, &[(6, 6), (1, 6)]), "表(16) ＞ 1と6");

        let desc = D66Table::new("表", D66SortType::Desc, ITEMS);
        assert_eq!(roll_with(&desc, &[(1, 6), (6, 6)]), "表(61) ＞ 6と1");
    }

    #[test]
    fn nested_table_is_rolled() {
        let t = D66Table::new("表", D66SortType::NoSort, ITEMS);
        assert_eq!(
            roll_with(&t, &[(6, 6), (6, 6), (2, 2)]),
            "表(66) ＞ 内側表(2) ＞ 内側2"
        );
    }

    #[test]
    fn missing_key_gives_empty_body() {
        let t = D66Table::new("表", D66SortType::NoSort, ITEMS);
        assert_eq!(roll_with(&t, &[(3, 6), (4, 6)]), "表(34) ＞ ");
    }

    #[test]
    fn choice_does_not_roll_nested_table() {
        // Ruby の #choice はネストを解決しない
        let t = D66Table::new("表", D66SortType::NoSort, ITEMS);
        assert_eq!(t.choice(11).to_string(), "表(11) ＞ ゾロ目1");
        assert_eq!(t.choice(66).to_string(), "表(66) ＞ ");
    }
}
