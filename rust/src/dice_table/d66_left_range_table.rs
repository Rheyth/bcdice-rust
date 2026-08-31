//! Ruby `BCDice::DiceTable::D66LeftRangeTable`
//! （lib/bcdice/dice_table/d66_left_range_table.rb）の移植。

use super::{apply_d66_sort, roll_barabara_2d6, RangeInc, RollResult, RollableTable};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 左側（十の位）だけを範囲で指定するD66表。
///
/// Ruby は構築時に `{左*10+右 => 項目}` のHashへ展開してから `D66Table` として引く。
/// 本移植は展開せず、引くときに「左の範囲を探す → 右で添字を引く」で同じキーに到達する。
#[derive(Debug, Clone, Copy)]
pub struct D66LeftRangeTable {
    name: &'static str,
    sort_type: D66SortType,
    items: &'static [(RangeInc, &'static [&'static str])],
}

impl D66LeftRangeTable {
    /// Ruby `D66LeftRangeTable.new(name, sort_type, items)`。
    pub const fn new(
        name: &'static str,
        sort_type: D66SortType,
        items: &'static [(RangeInc, &'static [&'static str])],
    ) -> Self {
        Self {
            name,
            sort_type,
            items,
        }
    }

    /// 表の名前。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// キー（`11`〜`66`）に対応する項目を返す。該当なしは空文字列。
    pub fn fetch(&self, key: i64) -> &'static str {
        let left = key / 10;
        let right = key % 10;
        self.items
            .iter()
            .find(|(range, _)| range.includes(left))
            .and_then(|(_, right_items)| {
                usize::try_from(right - 1)
                    .ok()
                    .and_then(|i| right_items.get(i))
            })
            .copied()
            .unwrap_or("")
    }
}

impl RollableTable for D66LeftRangeTable {
    /// Ruby `D66Table#roll` を継承: `roll_barabara(2, 6)` → 入れ替え → キー参照。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let (a, b) = roll_barabara_2d6(rng)?;
        let (a, b) = apply_d66_sort(self.sort_type, a, b);
        let key = a * 10 + b;
        Ok(RollResult::text(self.name, key, self.fetch(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    static LOW: &[&str] = &["低1", "低2", "低3", "低4", "低5", "低6"];
    static HIGH: &[&str] = &["高1", "高2", "高3", "高4", "高5", "高6"];
    static ITEMS: &[(RangeInc, &[&str])] =
        &[(RangeInc::new(1, 3), LOW), (RangeInc::new(4, 6), HIGH)];

    fn roll_with(table: &D66LeftRangeTable, rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn left_dice_selects_range_and_right_dice_indexes() {
        let t = D66LeftRangeTable::new("表", D66SortType::NoSort, ITEMS);
        assert_eq!(roll_with(&t, &[(1, 6), (1, 6)]), "表(11) ＞ 低1");
        assert_eq!(roll_with(&t, &[(3, 6), (6, 6)]), "表(36) ＞ 低6");
        assert_eq!(roll_with(&t, &[(4, 6), (2, 6)]), "表(42) ＞ 高2");
    }

    #[test]
    fn sort_type_is_applied_before_lookup() {
        let t = D66LeftRangeTable::new("表", D66SortType::Asc, ITEMS);
        // 6,1 → 昇順で 16
        assert_eq!(roll_with(&t, &[(6, 6), (1, 6)]), "表(16) ＞ 低6");
    }

    #[test]
    fn expansion_matches_ruby_key_mapping() {
        // Ruby: range.each { |left| right_items.each_with_index { |item, i| key = left*10 + i+1 } }
        let t = D66LeftRangeTable::new("表", D66SortType::NoSort, ITEMS);
        for left in 1..=6i64 {
            for right in 1..=6i64 {
                let expected = if left <= 3 { LOW } else { HIGH }[(right - 1) as usize];
                assert_eq!(t.fetch(left * 10 + right), expected);
            }
        }
    }

    #[test]
    fn missing_right_item_gives_empty_body() {
        static SHORT: &[&str] = &["だけ"];
        static SHORT_ITEMS: &[(RangeInc, &[&str])] = &[(RangeInc::new(1, 6), SHORT)];
        let t = D66LeftRangeTable::new("表", D66SortType::NoSort, SHORT_ITEMS);
        assert_eq!(t.fetch(11), "だけ");
        assert_eq!(t.fetch(12), "");
    }
}
