//! Ruby `String#to_i` 系の共通実装。
//!
//! 生成コード各ファイルにコピペされていた `to_i` / `ruby_to_i` / `to_i_saturating` を
//! 挙動差ごとに集約する。**挙動差は統合せず別関数として保持する**（R3 規約）。
//!
//! Ruby の `String#to_i` は多倍長 (`Bignum`) になるため、`i64` に収まらない入力の
//! 取り扱いがシステムごとに3系統に分かれている。どの入力がどれに来るかは
//! 各システムの正規表現（コマンド書式）が決めるため、勝手に統一してはならない:
//!
//! - [`ruby_to_i`]: 桁あふれで `i64::MAX` 飽和（正方向のみ）。先頭の符号・空白をスキップ。
//! - [`ruby_to_i_signed_saturating`]: 桁あふれで符号方向に飽和（負数は `i64::MIN`）。
//!   `Ruby to_i` の意味的に一番近い形。`"-"` / `"+1a"` 等の部分パースも実施。
//! - 空文字列・数字なしは Ruby 同様 `0`。
//!
//! パーサ系の差異:
//! - `parse::<i64>()` 版: Rust の `str::parse`（空白不可・`+`/`-` 1個のみ・全体が数値）。
//! - 手書きパーサ版: 先頭から数字を読む（Ruby `to_i` 互換の部分パース）。

/// Ruby `String#to_i`（先頭の十進数を切り出す。無ければ 0）。
///
/// 桁あふれは `i64::MAX` に飽和させる（正方向のみ）。
/// 入力は各システムのコマンド正規表現が `\d+` 等に制限している前提の呼び出しが多い。
pub(crate) fn ruby_to_i(s: &str) -> i64 {
    // Ruby `to_i` は先頭の空白をスキップし、符号を1個だけ受け付ける
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return 0;
    }
    // Ruby の `to_i` は多倍長。i64 に収まらない入力は飽和させる。
    s[..end].parse().unwrap_or(i64::MAX)
}

/// Ruby `String#to_i`（符号方向に飽和）。
///
/// 負の入力が `i64` 範囲外なら `i64::MIN`、正なら `i64::MAX`。
pub(crate) fn ruby_to_i_signed_saturating(s: &str) -> i64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return 0;
    }
    s[..end].parse().unwrap_or_else(|_| {
        if s.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `String#to_i` 相当だが、正規表現が `\d+` に全マッチさせた文字列を
/// `str::parse` で受ける版（部分パースなし）。
///
/// 桁あふれは `i64::MAX` 飽和（正方向のみ）。`parse` 可能な範囲の負数は
/// そのまま負の値になる（元コードの挙動を保持）。
pub(crate) fn to_i_max(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// [`to_i_max`] の符号方向飽和版。
///
/// `-` で始まる桁あふれ入力は `i64::MIN`。空文字列は `parse` 失敗で
/// `i64::MAX` になる（`""` が来る呼び出しでは使わないこと——元コードの挙動を保持）。
pub(crate) fn to_i_signed_saturating(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(if digits.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `String#to_i`（先頭の十進数。空なら 0）で、正方向飽和。
///
/// `DoubleCross` / `LogHorizon` / `Paradiso` / `Satasupe` / `TrailOfCthulhu` /
/// `WitchQuest` / `Strave` / `OneWayHeroics`(+省略) / `Emoklore`(符号対応) など
/// 「先頭の数字列だけ切り出して parse」系の共通化。`+` 接頭辞を剥がす
/// [`ruby_to_i_leading_plus`] と組み合わせて使う。
///
/// この関数は符号を処理しない（数字でない先頭文字で打ち切る）。
pub(crate) fn leading_digits_to_i_max(s: &str) -> i64 {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return 0;
    }
    digits.parse().unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_to_i_basic() {
        assert_eq!(ruby_to_i("123"), 123);
        assert_eq!(ruby_to_i(""), 0);
        assert_eq!(ruby_to_i("abc"), 0);
        assert_eq!(ruby_to_i("-42"), -42);
        assert_eq!(ruby_to_i("+7"), 7);
        assert_eq!(ruby_to_i("  12x"), 12);
        assert_eq!(ruby_to_i("12ab"), 12);
    }

    #[test]
    fn ruby_to_i_saturates_positive() {
        assert_eq!(ruby_to_i("99999999999999999999"), i64::MAX);
    }

    #[test]
    fn ruby_to_i_signed_saturates_both_directions() {
        assert_eq!(
            ruby_to_i_signed_saturating("99999999999999999999"),
            i64::MAX
        );
        assert_eq!(
            ruby_to_i_signed_saturating("-99999999999999999999"),
            i64::MIN
        );
        assert_eq!(ruby_to_i_signed_saturating("-5"), -5);
        assert_eq!(ruby_to_i_signed_saturating(""), 0);
    }

    #[test]
    fn to_i_max_and_signed() {
        assert_eq!(to_i_max("42"), 42);
        assert_eq!(to_i_max("99999999999999999999"), i64::MAX);
        // 元コードの挙動: 負号付きでも parse は成功する（`-3` → -3）。
        // `to_i_max` が負の値を返すのは「`-999...`（範囲外）」のみ。
        assert_eq!(to_i_max("-3"), -3);
        assert_eq!(to_i_signed_saturating("-99999999999999999999"), i64::MIN);
        assert_eq!(to_i_signed_saturating("-3"), -3);
    }

    #[test]
    fn leading_digits() {
        assert_eq!(leading_digits_to_i_max("12x"), 12);
        assert_eq!(leading_digits_to_i_max(""), 0);
        assert_eq!(leading_digits_to_i_max("99999999999999999999"), i64::MAX);
    }
}
