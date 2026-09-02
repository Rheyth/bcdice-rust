//! P4で手書き移植した `lib/bcdice/game_system/ShadowRun.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@sort_add_dice = true` / `@sort_barabara_dice = true`
//! - `@upper_dice_reroll_threshold = 6`（上方無限ロール `xUn` の境界値）
//!
//! 固有コマンドも判定フックも持たない。

use crate::game_system::GameSystem;

/// Ruby `BCDice::GameSystem::ShadowRun`（ID: `ShadowRun`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowRun;

impl GameSystem for ShadowRun {
    fn id(&self) -> &'static str {
        "ShadowRun"
    }

    fn name(&self) -> &'static str {
        "シャドウラン"
    }

    fn sort_key(&self) -> &'static str {
        "しやとうらん"
    }

    fn help_message(&self) -> &'static str {
        r"上方無限ロール(xUn)の境界値を6にセットします。
"
    }

    /// Ruby `ShadowRun#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `ShadowRun#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `ShadowRun#initialize` の `@upper_dice_reroll_threshold = 6`。
    fn upper_dice_reroll_threshold(&self) -> Option<i64> {
        Some(6)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ShadowRun",
            "ShadowRun.toml",
            29,
        );
    }
}
