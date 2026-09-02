//! P4で手書き移植した `lib/bcdice/game_system/ShadowRun4.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ShadowRun4#grich_text`（B/Rコマンド時のグリッチ判定）
//!
//! 設定値（`@sort_add_dice` / `@sort_barabara_dice` / `@reroll_dice_reroll_threshold` /
//! `@default_cmp_op` / `@default_target_number`）はスタブが持っている値と一致する。

use crate::game_system::GameSystem;
use crate::normalize::CmpOp;

/// Ruby `BCDice::GameSystem::ShadowRun4`（ID: `ShadowRun4`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowRun4;

impl GameSystem for ShadowRun4 {
    fn id(&self) -> &'static str {
        "ShadowRun4"
    }

    fn name(&self) -> &'static str {
        "シャドウラン 4th Edition"
    }

    fn sort_key(&self) -> &'static str {
        "しやとうらん4"
    }

    fn help_message(&self) -> &'static str {
        r"個数振り足しロール(xRn)の境界値を6にセット、バラバラロール(xBn)の目標値を5以上にセットします。
BコマンドとRコマンド時に、グリッチの表示を行います。
"
    }

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@reroll_dice_reroll_threshold = 6`。
    fn reroll_dice_reroll_threshold(&self) -> Option<i64> {
        Some(6)
    }

    /// Ruby `initialize` の `@default_cmp_op = :>=`。
    fn default_cmp_op(&self) -> Option<CmpOp> {
        Some(CmpOp::Ge)
    }

    /// Ruby `initialize` の `@default_target_number = 5`。
    fn default_target_number(&self) -> Option<i64> {
        Some(5)
    }

    /// Ruby `ShadowRun4#grich_text`。
    fn grich_text(
        &self,
        count_one: usize,
        dice_total_count: usize,
        count_success: i64,
    ) -> Option<String> {
        // Ruby: dice_cnt_total_half = (1.0 * dice_cnt_total / 2)
        let dice_cnt_total_half = dice_total_count as f64 / 2.0;

        // Ruby: unless numberSpot1 >= dice_cnt_total_half -> nil
        // 両辺とも有限値なので `!(a >= b)` は `a < b` と等しい。
        if (count_one as f64) < dice_cnt_total_half {
            return None;
        }

        // グリッチ！
        if count_success == 0 {
            Some("クリティカルグリッチ".to_owned())
        } else {
            Some("グリッチ".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ShadowRun4",
            "ShadowRun4.toml",
            36,
        );
    }
}
