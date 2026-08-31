//! ダイス表。Ruby `lib/bcdice/dice_table/*.rb` の移植。
//!
//! # 設計
//!
//! Rubyの表はクラス定数として（＝ロード時に1度だけ）組み立てられ、`roll(randomizer)` で
//! 引かれる。Rustでも同じく **ゲームシステム側の `static` / `const` として持てる形** にし、
//! 項目は `&'static str` のスライスで保持する。ダイスは [`Randomizer`] を借用して振る。
//!
//! そのため各表の構築子は原則 `const fn` にしてある。
//! `"1D6"` のような文字列でダイス指定を渡す Ruby 互換の構築子（`new`）も併せて用意し、
//! 書式が不正なら Ruby の `ArgumentError` に対応してパニックする。
//!
//! # Ruby との差異（意図的なもの）
//!
//! - **範囲外の項目参照**: Rubyは `@items[index]` が `nil` を返し、`to_s` が
//!   `"表名(値) ＞ "` になる。加えて Ruby は**負のindexで末尾から回り込む**。
//!   本移植は一律「該当なし＝空文字列」に統一し、回り込みは再現しない
//!   （回り込みに依存した表はBCDice本家にも存在しない）。
//! - **`RangeTable` の項目ソート**: Ruby は構築時に `sort_by { range.min }` して
//!   最小値カバー・最大値カバー・隙間/重なりを検査する。`&'static` スライスは
//!   構築時に並べ替えられないため、検査は [`RangeTable::validate`] に切り出した
//!   （`fetch` は Ruby の `find` と同じく最初に一致した項目を返すので、
//!   項目に重なりがなければ並び順によらず結果は一致する）。
//! - **`D66HalfGridTable` / `D66OneThirdTable`**: Ruby は 3行/2行の表を6行へ複製して
//!   `D66GridTable` に渡す。本移植は複製せず、左ダイスから行を選ぶ実装にした（同値）。

pub mod chain_table;
pub mod d66_grid_table;
pub mod d66_left_range_table;
pub mod d66_parity_table;
pub mod d66_range_table;
pub mod d66_table;
pub mod range_table;
pub mod roll_result;
pub mod sai_fic_skill_table;
pub mod table;

pub use chain_table::ChainTable;
pub use d66_grid_table::{D66GridTable, D66HalfGridTable, D66OneThirdTable};
pub use d66_left_range_table::D66LeftRangeTable;
pub use d66_parity_table::D66ParityTable;
pub use d66_range_table::D66RangeTable;
pub use d66_table::D66Table;
pub use range_table::{RangeRollResult, RangeTable, RangeTableError};
pub use roll_result::{RollBody, RollResult};
pub use sai_fic_skill_table::{SaiFicCategory, SaiFicFormats, SaiFicSkill, SaiFicSkillTable};
pub use table::Table;

use crate::eval::EvalError;
use crate::randomizer::Randomizer;

/// 振れる表。Ruby の `chosen.respond_to?(:roll)` による分岐に対応する。
///
/// ネスト可能な表（[`ChainTable`] / [`D66Table`] の項目）はこのトレイト経由で引かれる。
/// [`RangeTable`] は結果型が異なる（[`RangeRollResult`]）ため実装しない。
/// BCDice本家にも `RangeTable` をネストしている表は無い。
pub trait RollableTable: Sync {
    /// 表を振る。Ruby `#roll(randomizer)`。
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError>;
}

/// ネスト可能な表の項目。Ruby の `Array<String, #roll>` に対応する。
#[derive(Clone, Copy)]
pub enum TableItem {
    /// 文字列の項目。
    Text(&'static str),
    /// 別の表（引くと結果がネストする）。
    Table(&'static dyn RollableTable),
}

impl std::fmt::Debug for TableItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableItem::Text(t) => f.debug_tuple("Text").field(t).finish(),
            TableItem::Table(_) => f.write_str("Table(..)"),
        }
    }
}

impl TableItem {
    /// 項目を結果本体に変換する。文字列ならそのまま、表なら引いてネストさせる。
    ///
    /// Ruby: `chosen = chosen.roll(randomizer) if chosen.respond_to?(:roll)`
    fn resolve(&self, rng: &mut Randomizer) -> Result<RollBody, EvalError> {
        match self {
            TableItem::Text(t) => Ok(RollBody::Text(t)),
            TableItem::Table(table) => Ok(RollBody::Nested(Box::new(table.roll(rng)?))),
        }
    }
}

/// 整数の閉区間。Ruby の `Range`（`1..3`）と `Integer` の両方に対応する。
///
/// Ruby側は `[1..3, "内容"]` とも `[5, "内容"]` とも書けるので、
/// 単一値は [`RangeInc::single`] で表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeInc {
    /// 下限（含む）
    pub start: i64,
    /// 上限（含む）
    pub end: i64,
}

impl RangeInc {
    /// Ruby `start..end`。
    pub const fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }

    /// Ruby の `Integer` 指定（`Range.new(x, x)` に強制変換される）。
    pub const fn single(value: i64) -> Self {
        Self {
            start: value,
            end: value,
        }
    }

    /// Ruby `Range#include?`。
    pub const fn includes(&self, value: i64) -> bool {
        self.start <= value && value <= self.end
    }

    /// Ruby `Range#min`。空区間（`start > end`）では `None`。
    pub const fn min(&self) -> Option<i64> {
        if self.start <= self.end {
            Some(self.start)
        } else {
            None
        }
    }

    /// Ruby `Range#max`。空区間（`start > end`）では `None`。
    pub const fn max(&self) -> Option<i64> {
        if self.start <= self.end {
            Some(self.end)
        } else {
            None
        }
    }
}

impl std::fmt::Display for RangeInc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}..{}", self.start, self.end)
        }
    }
}

/// `"1D6"` 形式のダイス指定をパースする。Ruby `/(\d+)D(\d+)/i`（非アンカー）。
///
/// Ruby の `Table` / `ChainTable` はアンカーなしの `match` を使うため、
/// 前後に余分な文字があっても最初に見つかった `nDm` を採用する。
fn parse_dice_notation(spec: &str) -> Option<(i64, i64)> {
    let bytes = spec.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let times_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i >= bytes.len() || !bytes[i].eq_ignore_ascii_case(&b'D') {
            continue;
        }
        i += 1;
        let sides_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == sides_start {
            continue;
        }
        let times = spec[times_start..sides_start - 1].parse().ok()?;
        let sides = spec[sides_start..i].parse().ok()?;
        return Some((times, sides));
    }
    None
}

/// `"1D6"` 形式のダイス指定をパースする。Ruby `RangeTable::DICE_ROLL_METHOD_RE`
/// （`/\A(\d+)D(\d+)\z/i`、アンカーあり）。
fn parse_dice_notation_anchored(spec: &str) -> Option<(i64, i64)> {
    let (head, tail) = spec.split_once(['D', 'd'])?;
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    if !head.bytes().all(|b| b.is_ascii_digit()) || !tail.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((head.parse().ok()?, tail.parse().ok()?))
}

/// Ruby `randomizer.roll_barabara(2, 6)` の結果を2つ組で受け取る。
///
/// 個数2は `roll_barabara` の有効範囲（1〜200）内なので必ず2要素返るが、
/// 添字アクセスでパニックしないよう `get` を使う。
fn roll_barabara_2d6(rng: &mut Randomizer) -> Result<(i64, i64), EvalError> {
    let dice = rng.roll_barabara(2, 6)?;
    Ok((
        dice.first().copied().unwrap_or(0),
        dice.get(1).copied().unwrap_or(0),
    ))
}

/// Ruby `D66SortType` に従って2つの出目を入れ替える。
fn apply_d66_sort(sort_type: crate::enums::D66SortType, a: i64, b: i64) -> (i64, i64) {
    use crate::enums::D66SortType;
    match sort_type {
        D66SortType::Asc => (a.min(b), a.max(b)),
        D66SortType::Desc => (a.max(b), a.min(b)),
        D66SortType::NoSort => (a, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dice_notation() {
        assert_eq!(parse_dice_notation("1D6"), Some((1, 6)));
        assert_eq!(parse_dice_notation("2d10"), Some((2, 10)));
        // Ruby の match はアンカーなしなので前後の文字を無視する
        assert_eq!(parse_dice_notation("表 2D6 表"), Some((2, 6)));
        assert_eq!(parse_dice_notation("D6"), None);
        assert_eq!(parse_dice_notation("1D"), None);
        assert_eq!(parse_dice_notation("なし"), None);
    }

    #[test]
    fn parses_dice_notation_anchored() {
        assert_eq!(super::parse_dice_notation_anchored("2D6"), Some((2, 6)));
        assert_eq!(super::parse_dice_notation_anchored("2d6"), Some((2, 6)));
        assert_eq!(super::parse_dice_notation_anchored(" 2D6"), None);
        assert_eq!(super::parse_dice_notation_anchored("2D6+1"), None);
    }

    #[test]
    fn range_inc_matches_ruby_range() {
        let r = RangeInc::new(2, 7);
        assert!(r.includes(2) && r.includes(7) && !r.includes(1) && !r.includes(8));
        assert_eq!(r.min(), Some(2));
        assert_eq!(r.max(), Some(7));
        assert_eq!(r.to_string(), "2..7");

        let s = RangeInc::single(12);
        assert!(s.includes(12) && !s.includes(11));
        assert_eq!(s.to_string(), "12");
    }
}
