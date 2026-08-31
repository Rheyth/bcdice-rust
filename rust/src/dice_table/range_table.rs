//! Ruby `BCDice::DiceTable::RangeTable`（lib/bcdice/dice_table/range_table.rb）の移植。

use super::{parse_dice_notation_anchored, RangeInc};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 表の項目。`(出目の合計の範囲, 内容)`。
pub type RangeTableItem = (RangeInc, &'static str);

/// 結果の整形処理。Ruby の `initialize` に渡すブロックに対応する。
pub type RangeTableFormatter = fn(&RangeTable, &RangeRollResult) -> String;

/// 表を振った結果。Ruby `RangeTable::RollResult`（`Struct.new(:sum, :values, :content, :formatted)`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeRollResult {
    /// 出目の合計
    pub sum: i64,
    /// 出目の配列
    pub values: Vec<i64>,
    /// 選ばれた項目の内容
    pub content: &'static str,
    /// 整形された結果（Ruby `#to_s` と同じ）
    pub formatted: String,
}

impl std::fmt::Display for RangeRollResult {
    /// Ruby `alias_method :to_s, :formatted`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.formatted)
    }
}

/// 各項目が出目の合計の範囲を持つ表。
///
/// Ruby は構築時に項目を `range.min` でソートし、最小値・最大値のカバーと
/// 隙間/重なりの有無を検査する。`&'static` スライスは並べ替えられないため、
/// 検査は [`RangeTable::validate`] に切り出した（テストから呼ぶことを想定）。
#[derive(Debug, Clone, Copy)]
pub struct RangeTable {
    name: &'static str,
    num_of_dice: i64,
    num_of_sides: i64,
    items: &'static [RangeTableItem],
    formatter: Option<RangeTableFormatter>,
}

/// [`RangeTable::validate`] が返す不整合。Ruby が構築時に投げる例外に対応する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeTableError {
    /// 項目が1つもない。
    Empty,
    /// 出目の合計の最小値がカバーされていない（Ruby `RangeError`）。
    MinNotCovered { min_sum: i64, first: RangeInc },
    /// 出目の合計の最大値がカバーされていない（Ruby `RangeError`）。
    MaxNotCovered { max_sum: i64, last: RangeInc },
    /// 範囲が重なっている（Ruby `RangeError: Range overlap`）。
    Overlap { first: RangeInc, second: RangeInc },
    /// 範囲に隙間がある（Ruby `RangeError: Range gap`）。
    Gap { first: RangeInc, second: RangeInc },
}

impl std::fmt::Display for RangeTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RangeTableError::Empty => f.write_str("no items"),
            RangeTableError::MinNotCovered { min_sum, first } => {
                write!(f, "min value ({min_sum}) is not covered: {first}")
            }
            RangeTableError::MaxNotCovered { max_sum, last } => {
                write!(f, "max value ({max_sum}) is not covered: {last}")
            }
            RangeTableError::Overlap { first, second } => {
                write!(f, "Range overlap: {first} and {second}")
            }
            RangeTableError::Gap { first, second } => {
                write!(f, "Range gap: {first} and {second}")
            }
        }
    }
}

/// Ruby `RangeTable::DEFAULT_FORMATTER`。
fn default_formatter(table: &RangeTable, result: &RangeRollResult) -> String {
    format!("{}({}) ＞ {}", table.name, result.sum, result.content)
}

impl RangeTable {
    /// ダイスの個数と面数を直接指定して構築する。
    pub const fn from_dice(
        name: &'static str,
        num_of_dice: i64,
        num_of_sides: i64,
        items: &'static [RangeTableItem],
    ) -> Self {
        Self {
            name,
            num_of_dice,
            num_of_sides,
            items,
            formatter: None,
        }
    }

    /// Ruby `RangeTable.new(name, dice_roll_method, items)`。
    ///
    /// # Panics
    ///
    /// `dice_roll_method` が `\A\d+D\d+\z` に一致しない場合にパニックする
    /// （Ruby は `ArgumentError, "...: invalid dice roll method: ..."` を投げる）。
    pub fn new(
        name: &'static str,
        dice_roll_method: &str,
        items: &'static [RangeTableItem],
    ) -> Self {
        let (num_of_dice, num_of_sides) = parse_dice_notation_anchored(dice_roll_method)
            .unwrap_or_else(|| panic!("{name}: invalid dice roll method: {dice_roll_method}"));
        Self::from_dice(name, num_of_dice, num_of_sides, items)
    }

    /// 独自の整形処理を指定する。Ruby の `initialize` に渡すブロックに対応する。
    pub const fn with_formatter(mut self, formatter: RangeTableFormatter) -> Self {
        self.formatter = Some(formatter);
        self
    }

    /// Ruby `#name`。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Ruby `#num_of_dice`。
    pub fn num_of_dice(&self) -> i64 {
        self.num_of_dice
    }

    /// Ruby `#num_of_sides`。
    pub fn num_of_sides(&self) -> i64 {
        self.num_of_sides
    }

    /// Ruby `#fetch(value)`。指定された合計値に対応する項目の内容を返す。
    ///
    /// Ruby は該当なしで `RangeError` を投げるが、[`validate`](Self::validate) を通した
    /// 表なら到達しない。ここではパニックさせず `None` を返す。
    pub fn fetch(&self, value: i64) -> Option<&'static str> {
        self.items
            .iter()
            .find(|(range, _)| range.includes(value))
            .map(|(_, content)| *content)
    }

    /// 表を振る。Ruby `#roll(randomizer)`: `roll_barabara(num_of_dice, num_of_sides)`。
    pub fn roll(&self, rng: &mut Randomizer) -> Result<RangeRollResult, EvalError> {
        let values = rng.roll_barabara(self.num_of_dice, self.num_of_sides)?;
        let sum = values.iter().fold(0i64, |a, b| a.wrapping_add(*b));

        let mut result = RangeRollResult {
            sum,
            values,
            content: self.fetch(sum).unwrap_or(""),
            formatted: String::new(),
        };
        result.formatted = self.formatter.unwrap_or(default_formatter)(self, &result);
        Ok(result)
    }

    /// Ruby `#store` が構築時に行う検査。
    ///
    /// - 出目の合計の最小値（`num_of_dice`）がカバーされているか
    /// - 出目の合計の最大値（`num_of_dice * num_of_sides`）がカバーされているか
    /// - 範囲に隙間や重なりがないか
    pub fn validate(&self) -> Result<(), RangeTableError> {
        if self.items.is_empty() {
            return Err(RangeTableError::Empty);
        }

        // Ruby: items.sort_by { |r, _| r.min }
        let mut sorted: Vec<RangeInc> = self.items.iter().map(|(r, _)| *r).collect();
        sorted.sort_by_key(|r| r.min().unwrap_or(i64::MAX));

        let min_sum = self.num_of_dice;
        let first = sorted[0];
        if !first.includes(min_sum) {
            return Err(RangeTableError::MinNotCovered { min_sum, first });
        }

        let max_sum = self.num_of_dice * self.num_of_sides;
        let last = sorted[sorted.len() - 1];
        if !last.includes(max_sum) {
            return Err(RangeTableError::MaxNotCovered { max_sum, last });
        }

        // Ruby: sorted_items.each_cons(2)
        for pair in sorted.windows(2) {
            let (r1, r2) = (pair[0], pair[1]);
            let Some(max1) = r1.max() else { continue };
            if r2.includes(max1) {
                return Err(RangeTableError::Overlap {
                    first: r1,
                    second: r2,
                });
            }
            if !r2.includes(max1 + 1) {
                return Err(RangeTableError::Gap {
                    first: r1,
                    second: r2,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    /// Ruby doc の例（バトルテックの致命的命中表）。
    static CRITICAL_ITEMS: &[RangeTableItem] = &[
        (RangeInc::new(2, 7), "致命的命中はなかった"),
        (RangeInc::new(8, 9), "1箇所の致命的命中"),
        (RangeInc::new(10, 11), "2箇所の致命的命中"),
        (
            RangeInc::single(12),
            "その部位が吹き飛ぶ（腕、脚、頭）または3箇所の致命的命中（胴）",
        ),
    ];
    static CRITICAL_TABLE: RangeTable = RangeTable::from_dice("致命的命中表", 2, 6, CRITICAL_ITEMS);

    fn roll_with(table: &RangeTable, rands: &[(i64, i64)]) -> RangeRollResult {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = table.roll(&mut rng).expect("roll");
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn default_formatter_matches_ruby_doc() {
        assert_eq!(
            roll_with(&CRITICAL_TABLE, &[(3, 6), (4, 6)]).to_string(),
            "致命的命中表(7) ＞ 致命的命中はなかった"
        );
        assert_eq!(
            roll_with(&CRITICAL_TABLE, &[(4, 6), (4, 6)]).to_string(),
            "致命的命中表(8) ＞ 1箇所の致命的命中"
        );
        assert_eq!(
            roll_with(&CRITICAL_TABLE, &[(5, 6), (5, 6)]).to_string(),
            "致命的命中表(10) ＞ 2箇所の致命的命中"
        );
    }

    #[test]
    fn custom_formatter_receives_values() {
        // Ruby doc の例: "致命的命中発生? ＞ 11[5,6] ＞ 2箇所の致命的命中"
        fn formatter(_table: &RangeTable, result: &RangeRollResult) -> String {
            let values: Vec<String> = result.values.iter().map(|v| v.to_string()).collect();
            format!(
                "致命的命中発生? ＞ {}[{}] ＞ {}",
                result.sum,
                values.join(","),
                result.content
            )
        }
        let table = CRITICAL_TABLE.with_formatter(formatter);
        assert_eq!(
            roll_with(&table, &[(5, 6), (6, 6)]).to_string(),
            "致命的命中発生? ＞ 11[5,6] ＞ 2箇所の致命的命中"
        );
    }

    #[test]
    fn parses_dice_roll_method() {
        let t = RangeTable::new("表", "2D6", CRITICAL_ITEMS);
        assert_eq!((t.num_of_dice(), t.num_of_sides()), (2, 6));
    }

    #[test]
    #[should_panic(expected = "invalid dice roll method")]
    fn invalid_dice_roll_method_panics() {
        RangeTable::new("表", "2D6+1", CRITICAL_ITEMS);
    }

    #[test]
    fn validate_accepts_complete_table() {
        assert_eq!(CRITICAL_TABLE.validate(), Ok(()));
    }

    #[test]
    fn validate_detects_uncovered_min() {
        static ITEMS: &[RangeTableItem] = &[(RangeInc::new(3, 12), "a")];
        let t = RangeTable::from_dice("表", 2, 6, ITEMS);
        assert_eq!(
            t.validate(),
            Err(RangeTableError::MinNotCovered {
                min_sum: 2,
                first: RangeInc::new(3, 12)
            })
        );
    }

    #[test]
    fn validate_detects_uncovered_max() {
        static ITEMS: &[RangeTableItem] = &[(RangeInc::new(2, 11), "a")];
        let t = RangeTable::from_dice("表", 2, 6, ITEMS);
        assert_eq!(
            t.validate(),
            Err(RangeTableError::MaxNotCovered {
                max_sum: 12,
                last: RangeInc::new(2, 11)
            })
        );
    }

    #[test]
    fn validate_detects_gap_and_overlap() {
        static GAP: &[RangeTableItem] = &[(RangeInc::new(2, 6), "a"), (RangeInc::new(8, 12), "b")];
        assert_eq!(
            RangeTable::from_dice("表", 2, 6, GAP).validate(),
            Err(RangeTableError::Gap {
                first: RangeInc::new(2, 6),
                second: RangeInc::new(8, 12)
            })
        );

        static OVERLAP: &[RangeTableItem] =
            &[(RangeInc::new(2, 7), "a"), (RangeInc::new(7, 12), "b")];
        assert_eq!(
            RangeTable::from_dice("表", 2, 6, OVERLAP).validate(),
            Err(RangeTableError::Overlap {
                first: RangeInc::new(2, 7),
                second: RangeInc::new(7, 12)
            })
        );
    }

    #[test]
    fn item_order_does_not_change_lookup() {
        // Ruby は構築時にソートするが、重なりがなければ fetch の結果は順序によらない
        static SHUFFLED: &[RangeTableItem] = &[
            (RangeInc::single(12), "d"),
            (RangeInc::new(2, 7), "a"),
            (RangeInc::new(10, 11), "c"),
            (RangeInc::new(8, 9), "b"),
        ];
        let t = RangeTable::from_dice("表", 2, 6, SHUFFLED);
        assert_eq!(t.validate(), Ok(()));
        assert_eq!(t.fetch(2), Some("a"));
        assert_eq!(t.fetch(9), Some("b"));
        assert_eq!(t.fetch(11), Some("c"));
        assert_eq!(t.fetch(12), Some("d"));
    }
}
