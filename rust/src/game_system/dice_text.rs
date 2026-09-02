//! 出目列の文字列整形（Ruby `Array#join`）の共通実装。
//!
//! 生成コード各ファイルにコピペされていた `join_dice` を集約する
//! （正規化8種のうち `","` 区切りが大半。`", "` とカスタム区切りは別関数）。
//!
//! 生成コード側でこの形のローカル関数を新設することは禁止（R3 移植規約）。

/// Ruby `dice_list.join(",")`。
pub(crate) fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `list.join(", ")`（空白区切り。OracleEngine 等）。
pub(crate) fn join_dice_with_comma_space(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ruby `dice.join(sep)`（区切り文字を指定）。
pub(crate) fn join_dice_with_sep(dice: &[i64], sep: &str) -> String {
    dice.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(sep)
}
