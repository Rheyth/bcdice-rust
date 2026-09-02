//! P4で手書き移植した `lib/bcdice/game_system/SharedFantasia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `SharedFantasia#change_text`（`SF` / `ST` → `2D6` の書き換え）
//! - `SharedFantasia#result_2d6`（自動成功・自動失敗・劇的成功・致命的失敗）
//!
//! # `SF` / `ST` 接頭辞について
//!
//! `register_prefix` には `SF` と `ST` があるが、`change_text` が前処理で
//! `2D6` に書き換えてしまうため、この2つが `dice_command` 側で一致することはない
//! （実際の判定は共通コマンドの加算ロールが行う）。原典どおりの構造を保つため
//! 接頭辞はそのまま残してある。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `/S[FT]/i`。
///
/// `(?i)` は使わない（`regex` クレートの `(?i)` はUnicodeケースフォールディングになり、
/// `K`(U+212A) 等まで拾ってしまう）ので大小を明示して書く。
fn sf_st_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[Ss][FfTt]").expect("valid regex"))
}

/// Ruby `SharedFantasia#change_text`（`gsub(/S[FT]/i, "2D6")`）。
fn change_text_impl(text: &str) -> Cow<'_, str> {
    sf_st_pattern().replace_all(text, "2D6")
}

/// Ruby `BCDice::GameSystem::SharedFantasia`（ID: `SharedFantasia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedFantasia;

impl GameSystem for SharedFantasia {
    fn id(&self) -> &'static str {
        "SharedFantasia"
    }

    fn name(&self) -> &'static str {
        "Shared†Fantasia"
    }

    fn sort_key(&self) -> &'static str {
        "しえああとふあんたしあ"
    }

    fn help_message(&self) -> &'static str {
        r"2D6の成功判定に 自動成功、自動失敗、致命的失敗、劇的成功 の判定があります。

SF/ST = 2D6のショートカット

例) SF+4>=9 : 2D6して4を足した値が9以上なら成功
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SF", "ST"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SharedFantasia#change_text`。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        change_text_impl(text)
    }

    /// Ruby `SharedFantasia#result_2d6`。
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == '?'
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        // Ruby: return nil unless [:>=, :>].include?(cmp_op)
        if cmp_op != CmpOp::Ge && cmp_op != CmpOp::Gt {
            return None;
        }

        let critical = dice_total == 12;
        let fumble = dice_total == 2;

        // Ruby: totalValueBonus = (cmp_op == :>= ? 1 : 0)
        let total_value_bonus = i64::from(cmp_op == CmpOp::Ge);

        let result = if (total + total_value_bonus) > target {
            if critical {
                EvalResult::critical("自動成功(劇的成功)")
            } else if fumble {
                EvalResult::failure("自動失敗")
            } else {
                EvalResult::success("成功")
            }
        } else if critical {
            EvalResult::success("自動成功")
        } else if fumble {
            EvalResult::fumble("自動失敗(致命的失敗)")
        } else {
            EvalResult::failure("失敗")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "SharedFantasia",
            "SharedFantasia.toml",
            14,
        );
    }
}
