//! P4で手書き移植した `lib/bcdice/game_system/Gundog.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@enabled_d9 = true`（`nD9` / `roll_d9`）
//! - `Gundog#result_1d100`（1D100の成功度判定）
//!
//! `GundogZero` / `GundogRevised` は Ruby 側で本クラスを継承する。
//! `GundogZero.rs` は本ファイルがスタブの頃に親由来の判定を取り込済みなので、
//! 判定ロジックはそちらにも残してある。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::Gundog`（ID: `Gundog`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gundog;

impl GameSystem for Gundog {
    fn id(&self) -> &'static str {
        "Gundog"
    }

    fn name(&self) -> &'static str {
        "ガンドッグ"
    }

    fn sort_key(&self) -> &'static str {
        "かんとつく"
    }

    fn help_message(&self) -> &'static str {
        r"失敗、成功、クリティカル、ファンブルとロールの達成値の自動判定を行います。
nD9ロールも対応。
"
    }

    /// Ruby `Gundog#initialize` の `@enabled_d9 = true`。
    fn enabled_d9(&self) -> bool {
        true
    }

    /// Ruby `Gundog#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_1d100_impl(crate::randomizer::sat_i64(&total), cmp_op, target)
    }
}

/// Ruby `Gundog#result_1d100` 本体。
fn result_1d100_impl(total: i64, cmp_op: CmpOp, target: Target) -> Option<CheckOutcome> {
    // Ruby: return nil unless cmp_op == :<=
    if cmp_op != CmpOp::Le {
        return None;
    }

    // 目標値 `?` の判定は `total >= 100` と `total <= 1` の**後**に来る。
    // 先頭に出すと `1D100<=?` のファンブル／絶対成功が拾えなくなる。
    if total >= 100 {
        return Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            "ファンブル",
        ))));
    }
    if total <= 1 {
        return Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            "絶対成功(達成値1+SL)",
        ))));
    }

    // Ruby: elsif target == "?" -> Result.nothing
    // `nil`（＝次のフックへ進む）ではなく `:nothing`（＝以降を打ち切って nil）。
    let Target::Number(target) = target else {
        return Some(CheckOutcome::Nothing);
    };

    if total > crate::randomizer::sat_i64(&target) {
        return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
    }

    // ここに来る total は 2..=99 なので、Ruby側の
    // `dig10 = 0 if dig10 >= 10` / `dig1 = 0 if dig1 >= 10` は到達しない。
    let dig10 = total / 10;
    let dig1 = total - dig10 * 10;

    let result = if dig1 <= 0 {
        EvalResult::critical("クリティカル(達成値20+SL)")
    } else {
        EvalResult::success(format!("成功(達成値{}+SL)", dig10 + dig1))
    };
    Some(CheckOutcome::Result(Box::new(result)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Gundog", "Gundog.toml", 11);
    }
}
