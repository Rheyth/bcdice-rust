//! P4で手書き移植した `lib/bcdice/game_system/ShinMegamiTenseiKakuseihen.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ShinMegamiTenseiKakuseihen#check_1D100`（スワップ／通常／逆スワップ判定）
//!
//! 原典は deprecated の `check_1D100`（文字列を返す旧フック）で、
//! `Deprecated::Checker#check_result_legacy` が `Result.new(text)` に包む。
//! 成功・失敗フラグは立たない。Rust側は同じ文字列を `result_1d100` から返す。

use crate::game_system::int_helpers::int_rem_euclid;
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::ShinMegamiTenseiKakuseihen`（ID: `ShinMegamiTenseiKakuseihen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShinMegamiTenseiKakuseihen;

impl GameSystem for ShinMegamiTenseiKakuseihen {
    fn id(&self) -> &'static str {
        "ShinMegamiTenseiKakuseihen"
    }

    fn name(&self) -> &'static str {
        "真・女神転生TRPG 覚醒篇"
    }

    fn sort_key(&self) -> &'static str {
        "しんめかみてんせいTRPGかくせいへん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
1D100<=(目標値) でスワップ・通常・逆スワップ判定を判定。
威力ダイスは nU6[6] (nはダイス個数)でロール可能です。
"
    }

    /// Ruby `ShinMegamiTenseiKakuseihen#check_1D100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return '' if target == '?'  （空文字は legacy checker が nil 扱い）
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return '' unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        let (dice1, dice2) = split_tens(dice_total);
        let total1: I = I::from(dice1 * 10 + dice2);
        let total2: I = I::from(dice2 * 10 + dice1);
        let is_repdigit = dice1 == dice2;

        // Ruby: Result.new(ret.delete_prefix(" ＞ ")) 相当。フラグは立たない。
        let text = format!(
            "スワップ{}／通常{}／逆スワップ{}",
            check_result_text(
                sat_i64(&target),
                total1.clone().min(total2.clone()),
                is_repdigit
            ),
            check_result_text(
                crate::randomizer::sat_i64(&target),
                int_rem_euclid(&total, 100),
                is_repdigit
            ),
            check_result_text(sat_i64(&target), total1.max(total2), is_repdigit),
        );

        Some(CheckOutcome::Result(Box::new(EvalResult::with_text(text))))
    }
}

/// Ruby `ShinMegamiTenseiKakuseihen#split_tens`。
///
/// 戻り値の1要素目は十の位、2要素目は一の位（Ruby側の変数名 `ones`/`tens` とは逆）。
fn split_tens(value: i64) -> (i64, i64) {
    let value = value.rem_euclid(100);
    let ones = value / 10;
    let tens = value % 10;
    (ones, tens)
}

/// Ruby `ShinMegamiTenseiKakuseihen#getCheckResultText`。
fn check_result_text(diff: i64, total: crate::Int, is_repdigit: bool) -> String {
    format!(
        "({total:02}){}",
        check_result(diff, total.clone(), is_repdigit)
    )
}

/// Ruby `ShinMegamiTenseiKakuseihen#getCheckResult`。
fn check_result(diff: i64, total: crate::Int, is_repdigit: bool) -> &'static str {
    if diff >= crate::randomizer::sat_i64(&total) {
        success_result(is_repdigit)
    } else {
        fail_result(is_repdigit)
    }
}

/// Ruby `ShinMegamiTenseiKakuseihen#getSuccessResult`。
fn success_result(is_repdigit: bool) -> &'static str {
    if is_repdigit {
        "絶対成功"
    } else {
        "成功"
    }
}

/// Ruby `ShinMegamiTenseiKakuseihen#getFailResult`。
fn fail_result(is_repdigit: bool) -> &'static str {
    if is_repdigit {
        "絶対失敗"
    } else {
        "失敗"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ShinMegamiTenseiKakuseihen",
            "ShinMegamiTenseiKakuseihen.toml",
            12,
        );
    }
}
