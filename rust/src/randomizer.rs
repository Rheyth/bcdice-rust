//! 乱数器。Ruby `BCDice::Randomizer`（lib/bcdice/randomizer.rb）の移植。
//!
//! Ruby版との構造対応:
//! - `Randomizer`（`#random` を private に持つ本体）→ [`Randomizer`]
//! - `RandomizerMock`（test/randomizer_mock.rb、`rands` 注入）→ [`SeededRandomizer`]
//!
//! Ruby版は `#random(sides)` をサブクラスで差し替えてモック化するが、Rustでは
//! 乱数源を [`RandSource`] トレイトに分離し、[`Randomizer`] がそれを借用する形にした。
//! 出目系列は Ruby版と同一のAPI（roll_once / roll_barabara / roll_sum / roll_index /
//! roll_tens_d10 / roll_d9 / roll_d66）と上限値を守る。

use num_traits::ToPrimitive;

use crate::dice_table::apply_d66_sort;
use crate::enums::D66SortType;
use crate::eval::EvalError;

/// 1回のダイスロールで振れる個数の上限。Ruby `UPPER_LIMIT_DICE_TIMES`。
pub const UPPER_LIMIT_DICE_TIMES: i64 = 200;
/// ダイスの面数の上限。Ruby `UPPER_LIMIT_DICE_SIDES`。
pub const UPPER_LIMIT_DICE_SIDES: i64 = 10000;
/// 1コマンドで振れるダイスの総数の上限。Ruby `UPPER_LIMIT_RANDS`。
pub const UPPER_LIMIT_RANDS: usize = 10000;

/// `crate::Int` を `i64` に飽和変換する。
///
/// times/sides が範囲外（`i64::MAX` 超や負値など）のときの Ruby の挙動
/// （空配列、TooManyRands、または無効出目・エラー）と観測的に等価になる根拠:
/// ダイスロールの個数や面数は `0 < times <= UPPER_LIMIT_DICE_TIMES` や
/// `0 < sides <= UPPER_LIMIT_DICE_SIDES` のように有界な上限値でクランプ・判定されるため、
/// `i64` の範囲外の極端に巨大な値は飽和して `i64::MAX` となっても上限チェックで等しく弾かれ、
/// 負値は飽和して `i64::MIN` となっても 0 以下のチェックで等しく弾かれるため、
/// Ruby の挙動と観測的に同一となる。
pub fn sat_i64(v: &crate::Int) -> i64 {
    v.to_i64().unwrap_or_else(|| {
        if v > &crate::Int::ZERO {
            i64::MAX
        } else {
            i64::MIN
        }
    })
}

/// 乱数源トレイト。`random(sides)` は 1..=sides の整数を返す。
///
/// Ruby版の private `#random(sides)` をオーバーライドしてモック化する構造を、
/// トレイト分離で再現したもの。
pub trait RandSource {
    /// 1以上 `sides` 以下の整数を返す。
    ///
    /// 注入済み乱数を使うモック実装では、列が尽きた場合や面数が食い違う場合に
    /// `Err` を返す（Ruby側のモックは `raise` する）。
    fn random(&mut self, sides: i64) -> Result<i64, EvalError>;
}

/// TOML `rands` を注入する乱数源。test/randomizer_mock.rb 相当。
///
/// 事前に記録された `(value, sides)` 列を順に取り出す。取り出し時の面数チェックは
/// Rubyモックと同様に不一致でエラーにする（TOMLテストの `rands` は出現順に記録される）。
#[derive(Debug, Clone, Default)]
pub struct SeededRandomizer {
    rands: std::collections::VecDeque<(i64, i64)>,
}

impl SeededRandomizer {
    /// `(value, sides)` の列から生成。TOMLの `rands` 配列の順序どおりに並べること。
    pub fn new(rands: impl IntoIterator<Item = (i64, i64)>) -> Self {
        Self {
            rands: rands.into_iter().collect(),
        }
    }

    /// まだ消費していない注入済み出目が残っているか。
    ///
    /// テスト終了時に `true` だった場合、期待より少ないダイスしか振られていない
    /// （TOMLと実装のどちらかがズレている）ことを意味する。
    pub fn is_empty(&self) -> bool {
        self.rands.is_empty()
    }

    /// 残っている注入済み出目の数。
    pub fn remaining(&self) -> usize {
        self.rands.len()
    }
}

impl RandSource for SeededRandomizer {
    fn random(&mut self, sides: i64) -> Result<i64, EvalError> {
        let (value, expected_sides) = match self.rands.pop_front() {
            Some(pair) => pair,
            None => {
                return Err(EvalError::RandSource(format!(
                    "rands is empty (requested sides={sides})"
                )))
            }
        };

        if sides != expected_sides {
            return Err(EvalError::RandSource(format!(
                "unexpected sides at [{value}/{expected_sides}], \
                 side (given {sides}, expected {expected_sides})"
            )));
        }

        Ok(value)
    }
}

/// 実行したダイスロールの詳細。Ruby `Randomizer::DetailedRandResult`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailedRandResult {
    pub kind: RandKind,
    pub sides: i64,
    pub value: i64,
}

/// [`DetailedRandResult`] の種別。Ruby側はシンボル（`:normal` など）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandKind {
    Normal,
    TensD10,
    D9,
}

/// 乱数生成器本体。Ruby `BCDice::Randomizer`。
pub struct Randomizer<'a> {
    source: &'a mut dyn RandSource,
    rand_results: Vec<(i64, i64)>,
    detailed_rand_results: Vec<DetailedRandResult>,
}

impl std::fmt::Debug for Randomizer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Randomizer")
            .field("rand_results", &self.rand_results)
            .finish_non_exhaustive()
    }
}

impl<'a> Randomizer<'a> {
    /// 乱数源を借用して生成する。
    pub fn new(source: &'a mut dyn RandSource) -> Self {
        Self {
            source,
            rand_results: Vec::new(),
            detailed_rand_results: Vec::new(),
        }
    }

    /// ダイスの出目一覧（`(value, sides)` の列）。Ruby `#rand_results`。
    pub fn rand_results(&self) -> &[(i64, i64)] {
        &self.rand_results
    }

    /// ダイスロールの詳細一覧。Ruby `#detailed_rand_results`。
    pub fn detailed_rand_results(&self) -> &[DetailedRandResult] {
        &self.detailed_rand_results
    }

    /// 複数個のダイスを振る。Ruby `#roll_barabara`。
    pub fn roll_barabara(&mut self, times: i64, sides: i64) -> Result<Vec<i64>, EvalError> {
        // Ruby: @rand_results.size + times > UPPER_LIMIT_RANDS -> raise TooManyRandsError
        if (self.rand_results.len() as i64).saturating_add(times) > UPPER_LIMIT_RANDS as i64 {
            return Err(EvalError::TooManyRands);
        }

        if times <= 0 || times > UPPER_LIMIT_DICE_TIMES {
            return Ok(Vec::new());
        }

        let mut out = Vec::with_capacity(times as usize);
        for _ in 0..times {
            out.push(self.roll_once(sides)?);
        }
        Ok(out)
    }

    /// 複数個のダイスを振って合計を求める。Ruby `#roll_sum`。
    pub fn roll_sum(&mut self, times: i64, sides: i64) -> Result<i64, EvalError> {
        Ok(self
            .roll_barabara(times, sides)?
            .iter()
            .fold(0i64, |a, b| a.wrapping_add(*b)))
    }

    /// 1回だけダイスロールを行う。Ruby `#roll_once`。
    pub fn roll_once(&mut self, sides: i64) -> Result<i64, EvalError> {
        if sides <= 0 || sides > UPPER_LIMIT_DICE_SIDES {
            return Ok(0);
        }

        let dice = self.rand_inner(sides)?;
        self.push_to_detail(RandKind::Normal, sides, dice);

        Ok(dice)
    }

    /// ダイス表などでindexを参照する用のダイスロール。Ruby `#roll_index`。
    pub fn roll_index(&mut self, sides: i64) -> Result<i64, EvalError> {
        Ok(self.roll_once(sides)? - 1)
    }

    /// 十の位をd10を使って決定するためのダイスロール。Ruby `#roll_tens_d10`。
    pub fn roll_tens_d10(&mut self) -> Result<i64, EvalError> {
        let mut dice = self.rand_inner(10)?;
        if dice == 10 {
            dice = 0;
        }

        let ret = dice * 10;
        self.push_to_detail(RandKind::TensD10, 10, ret);
        Ok(ret)
    }

    /// d10を0~9として扱うダイスロール。Ruby `#roll_d9`。
    pub fn roll_d9(&mut self) -> Result<i64, EvalError> {
        let dice = self.rand_inner(10)? - 1;
        self.push_to_detail(RandKind::D9, 10, dice);
        Ok(dice)
    }

    /// D66のダイスロールを行う。Ruby `#roll_d66`。
    ///
    /// 入れ替え処理は `dice_table::apply_d66_sort` と共用（C-9の二重化解消）。
    /// `roll_once(6)`×2 の乱数消費順序・記録形式は不変。
    pub fn roll_d66(&mut self, sort_type: D66SortType) -> Result<i64, EvalError> {
        let (d1, d2) = (self.roll_once(6)?, self.roll_once(6)?);
        let (d1, d2) = apply_d66_sort(sort_type, d1, d2);
        Ok(d1 * 10 + d2)
    }

    /// Ruby private `#rand_inner`。
    fn rand_inner(&mut self, sides: i64) -> Result<i64, EvalError> {
        if self.rand_results.len() >= UPPER_LIMIT_RANDS {
            return Err(EvalError::TooManyRands);
        }

        let dice = self.source.random(sides)?;
        self.rand_results.push((dice, sides));
        Ok(dice)
    }

    fn push_to_detail(&mut self, kind: RandKind, sides: i64, value: i64) {
        self.detailed_rand_results
            .push(DetailedRandResult { kind, sides, value });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_source_returns_values_in_order() {
        let mut src = SeededRandomizer::new([(4, 6), (1, 6)]);
        assert_eq!(src.random(6), Ok(4));
        assert_eq!(src.random(6), Ok(1));
        assert!(src.is_empty());
    }

    #[test]
    fn seeded_source_checks_sides() {
        let mut src = SeededRandomizer::new([(4, 6)]);
        assert!(src.random(10).is_err());
    }

    #[test]
    fn roll_once_returns_zero_for_out_of_range_sides() {
        let mut src = SeededRandomizer::new([]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_once(0), Ok(0));
        assert_eq!(r.roll_once(10001), Ok(0));
        assert!(r.rand_results().is_empty());
    }

    #[test]
    fn roll_barabara_returns_empty_over_dice_times_limit() {
        let mut src = SeededRandomizer::new([]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_barabara(201, 6), Ok(Vec::new()));
        assert_eq!(r.roll_barabara(0, 6), Ok(Vec::new()));
    }

    #[test]
    fn roll_d66_sorts() {
        let mut src = SeededRandomizer::new([(6, 6), (1, 6), (6, 6), (1, 6), (6, 6), (1, 6)]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_d66(D66SortType::NoSort), Ok(61));
        assert_eq!(r.roll_d66(D66SortType::Asc), Ok(16));
        assert_eq!(r.roll_d66(D66SortType::Desc), Ok(61));
    }

    #[test]
    fn roll_tens_d10_maps_ten_to_zero() {
        // Ruby: 出目10を0に読み替えてから10倍する。detailには乗算後の値が乗る。
        let mut src = SeededRandomizer::new([(10, 10), (3, 10)]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_tens_d10(), Ok(0));
        assert_eq!(r.roll_tens_d10(), Ok(30));
        assert_eq!(
            r.detailed_rand_results(),
            [
                DetailedRandResult {
                    kind: RandKind::TensD10,
                    sides: 10,
                    value: 0
                },
                DetailedRandResult {
                    kind: RandKind::TensD10,
                    sides: 10,
                    value: 30
                },
            ]
        );
        // rand_results には生の出目が入る
        assert_eq!(r.rand_results(), [(10, 10), (3, 10)]);
    }

    #[test]
    fn roll_d9_is_zero_based() {
        let mut src = SeededRandomizer::new([(10, 10), (1, 10)]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_d9(), Ok(9));
        assert_eq!(r.roll_d9(), Ok(0));
        assert_eq!(
            r.detailed_rand_results()[0],
            DetailedRandResult {
                kind: RandKind::D9,
                sides: 10,
                value: 9
            }
        );
    }

    #[test]
    fn roll_index_is_zero_based() {
        let mut src = SeededRandomizer::new([(1, 4), (4, 4)]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_index(4), Ok(0));
        assert_eq!(r.roll_index(4), Ok(3));
    }

    #[test]
    fn roll_sum_adds_up() {
        let mut src = SeededRandomizer::new([(5, 6), (3, 6)]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_sum(2, 6), Ok(8));
        // 個数上限を超えると空配列なので合計は0
        let mut src = SeededRandomizer::new([]);
        let mut r = Randomizer::new(&mut src);
        assert_eq!(r.roll_sum(201, 6), Ok(0));
    }

    #[test]
    fn too_many_rands() {
        struct Always;
        impl RandSource for Always {
            fn random(&mut self, _sides: i64) -> Result<i64, EvalError> {
                Ok(1)
            }
        }
        let mut src = Always;
        let mut r = Randomizer::new(&mut src);
        for _ in 0..50 {
            r.roll_barabara(200, 6).unwrap();
        }
        assert_eq!(r.rand_results().len(), UPPER_LIMIT_RANDS);
        assert_eq!(r.roll_barabara(1, 6), Err(EvalError::TooManyRands));
    }
}
