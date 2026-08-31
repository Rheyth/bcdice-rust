//! BigInt（`crate::Int`）用の補助演算。
//!
//! Ruby の `Integer` は多倍長で飽和・ラップしないため、B18（多倍長整数化）で
//! `i64` から `crate::Int`（`num_bigint::BigInt`）へ移行した経路のうち、
//! 旧 `i64` 組込みメソッドに依存していた箇所の代替を提供する。
//!
//! - [`int_saturating_sub`]:
//!   飽和演算（`i64` 範囲で飽和。Ruby との一致が不要な内部的な回数・面数などに使用）
//! - [`int_rem_euclid`]: Ruby `Integer#%`（正の除数に対するユークリッド剰余）
//! - [`int_clamp`]: Ruby `Integer#clamp`
//! - [`IntHelperOps::abs_int`]: Ruby `Integer#abs`

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

/// 飽和減算（`i64` 範囲で飽和）。
pub(crate) fn int_saturating_sub(a: &crate::Int, b: &crate::Int) -> crate::Int {
    match (a.to_i64(), b.to_i64()) {
        (Some(x), Some(y)) => crate::Int::from(x.saturating_sub(y)),
        _ => {
            // i64 範囲外: 符号で飽和値を決める
            let neg = b > &crate::Int::ZERO;
            if neg {
                crate::Int::from(i64::MIN)
            } else {
                crate::Int::from(i64::MAX)
            }
        }
    }
}

/// Ruby `Integer#%` 相当（除数は正のリテラル想定）。
pub(crate) fn int_rem_euclid(a: &crate::Int, divisor: i64) -> crate::Int {
    let d = crate::Int::from(divisor);
    let r = a % &d;
    // a が負でも除数が正なので結果は [0, d) に正規化する
    if r < crate::Int::ZERO {
        r + d
    } else {
        r
    }
}

/// Ruby `Integer#clamp` 相当。
pub(crate) fn int_clamp(v: &crate::Int, lo: &crate::Int, hi: &crate::Int) -> crate::Int {
    if v < lo {
        lo.clone()
    } else if v > hi {
        hi.clone()
    } else {
        v.clone()
    }
}

/// `BigInt` に `abs_int()` を生やすトレイト（`abs` は `num_traits::Signed` と衝突回避のため別名）。
pub(crate) trait IntHelperOps {
    fn abs_int(&self) -> crate::Int;
}

impl IntHelperOps for BigInt {
    fn abs_int(&self) -> crate::Int {
        if self.is_zero() {
            crate::Int::ZERO
        } else {
            self.abs()
        }
    }
}
