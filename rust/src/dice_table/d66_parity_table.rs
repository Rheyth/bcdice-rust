//! Ruby `BCDice::DiceTable::D66ParityTable`（lib/bcdice/dice_table/d66_parity_table.rb）の移植。

use super::{RollResult, RollableTable};
use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 左ダイス（十の位）の偶奇で参照先の表を切り替えるD66表。
#[derive(Debug, Clone, Copy)]
pub struct D66ParityTable {
    name: &'static str,
    odd: &'static [&'static str],
    even: &'static [&'static str],
}

impl D66ParityTable {
    /// Ruby `D66ParityTable.new(name, odd, even)`。`odd` / `even` は6要素。
    pub const fn new(
        name: &'static str,
        odd: &'static [&'static str],
        even: &'static [&'static str],
    ) -> Self {
        Self { name, odd, even }
    }

    /// 表の名前。
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl RollableTable for D66ParityTable {
    /// Ruby `#roll(randomizer)`: `roll_once(6)` を2回振り、左の偶奇で表を選ぶ。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        let dice1 = rng.roll_once(6)?;
        let dice2 = rng.roll_once(6)?;

        // Ruby: dice1.odd? ? @odd : @even
        let second_table = if dice1 % 2 != 0 { self.odd } else { self.even };
        let body = usize::try_from(dice2 - 1)
            .ok()
            .and_then(|i| second_table.get(i))
            .copied()
            .unwrap_or("");

        Ok(RollResult::text(self.name, dice1 * 10 + dice2, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    static ODD: &[&str] = &["奇1", "奇2", "奇3", "奇4", "奇5", "奇6"];
    static EVEN: &[&str] = &["偶1", "偶2", "偶3", "偶4", "偶5", "偶6"];
    static TABLE: D66ParityTable = D66ParityTable::new("偶奇表", ODD, EVEN);

    fn roll_with(rands: &[(i64, i64)]) -> String {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let mut rng = Randomizer::new(&mut src);
        let out = TABLE.roll(&mut rng).expect("roll").to_string();
        assert!(src.is_empty(), "unconsumed rands");
        out
    }

    #[test]
    fn odd_left_dice_uses_odd_table() {
        assert_eq!(roll_with(&[(1, 6), (3, 6)]), "偶奇表(13) ＞ 奇3");
        assert_eq!(roll_with(&[(5, 6), (6, 6)]), "偶奇表(56) ＞ 奇6");
    }

    #[test]
    fn even_left_dice_uses_even_table() {
        assert_eq!(roll_with(&[(2, 6), (1, 6)]), "偶奇表(21) ＞ 偶1");
        assert_eq!(roll_with(&[(6, 6), (4, 6)]), "偶奇表(64) ＞ 偶4");
    }
}
