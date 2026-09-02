//! P4で手書き移植した `lib/bcdice/game_system/Arianrhod.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@sort_add_dice = true`（`@d66_sort_type = D66SortType::NO_SORT` はトレイト既定値）
//! - `Arianrhod#result_nd6`（全1ファンブル / 6が2個以上でクリティカル）
//!
//! `Arianrhod_Korean` が `ko_kr` の定型文を差し替えられるよう、判定は
//! [`Messages`] を受け取る関数に切り出してある。

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// 1ロケール分の定型文。`Arianrhod` と `Arianrhod_Korean` はこれだけが違う。
pub(crate) struct Messages {
    /// i18n `fumble`
    pub fumble: &'static str,
    /// i18n `Arianrhod.critical`（`%{dice}` を置換する）
    pub critical: &'static str,
    /// i18n `success`
    pub success: &'static str,
    /// i18n `failure`
    pub failure: &'static str,
}

/// i18n `ja_jp`（`i18n/Arianrhod/ja_jp.yml` と `i18n/ja_jp.yml`）。
static JA_MESSAGES: Messages = Messages {
    fumble: "ファンブル",
    critical: "クリティカル(+%{dice}D6)",
    success: "成功",
    failure: "失敗",
};

/// Ruby `Arianrhod#result_nd6`。
pub(crate) fn result_nd6_impl(
    messages: &Messages,
    total: i64,
    dice_list: &[i64],
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    let n_max = dice_list.iter().filter(|&&d| d == 6).count();

    if dice_list.iter().filter(|&&d| d == 1).count() == dice_list.len() {
        // 全部１の目ならファンブル
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            messages.fumble,
        ))))
    } else if n_max >= 2 {
        // ２個以上６の目があったらクリティカル
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            messages.critical.replace("%{dice}", &n_max.to_string()),
        ))))
    } else if cmp_op != CmpOp::Ge {
        None
    } else {
        match target {
            Target::Question => None,
            Target::Number(target) => {
                if total >= crate::randomizer::sat_i64(&target) {
                    Some(CheckOutcome::Result(Box::new(EvalResult::success(
                        messages.success,
                    ))))
                } else {
                    Some(CheckOutcome::Result(Box::new(EvalResult::failure(
                        messages.failure,
                    ))))
                }
            }
        }
    }
}

/// Ruby `BCDice::GameSystem::Arianrhod`（ID: `Arianrhod`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arianrhod;

impl GameSystem for Arianrhod {
    fn id(&self) -> &'static str {
        "Arianrhod"
    }

    fn name(&self) -> &'static str {
        "アリアンロッドRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ありあんろつとRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・クリティカル、ファンブルの自動判定を行います。(クリティカル時の追加ダメージも表示されます)
・D66ダイスあり
"
    }

    /// Ruby `Arianrhod#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Arianrhod#result_nd6`。
    fn result_nd6(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd6_impl(
            &JA_MESSAGES,
            crate::randomizer::sat_i64(&total),
            value_list,
            cmp_op,
            target,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Arianrhod",
            "Arianrhod.toml",
            27,
        );
    }
}
