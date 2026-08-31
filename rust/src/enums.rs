//! ゲームシステム設定で使う列挙型。Ruby `lib/bcdice/enum.rb` の移植。
//!
//! Ruby側は `module D66SortType` に `:no_sort` などのシンボル定数を置いているが、
//! Rustでは型で表現する。モジュール名は `enum` がRustの予約語のため `enums` とした。

/// D66のダイス入れ替え方法。Ruby `BCDice::D66SortType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D66SortType {
    /// 入れ替えない（`:no_sort`）
    NoSort,
    /// 一の位が大きな出目になるよう、昇順にソートする（`:asc`）
    Asc,
    /// 一の位が小さな出目になるよう、降順にソートする（`:desc`）
    Desc,
}

/// 割り算をした後の端数の扱い。Ruby `BCDice::RoundType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundType {
    /// 切り上げ（`:ceil`）
    Ceil,
    /// 切り捨て（`:floor`）
    Floor,
    /// 四捨五入（`:round`）
    Round,
}
