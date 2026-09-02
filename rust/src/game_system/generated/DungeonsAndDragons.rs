//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 固有コマンドも判定フックも持たない（部屋のシステム名表示用）。
//! 成功／失敗は `Base#result_ndx` の既定実装に任せる。

use crate::game_system::GameSystem;

/// Ruby `BCDice::GameSystem::DungeonsAndDragons`（ID: `DungeonsAndDragons`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons;

impl GameSystem for DungeonsAndDragons {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons"
    }

    fn name(&self) -> &'static str {
        "ダンジョンズ＆ドラゴンズ"
    }

    fn sort_key(&self) -> &'static str {
        "たんしよんすあんととらこんす"
    }

    fn help_message(&self) -> &'static str {
        r"※このダイスボットは部屋のシステム名表示用となります。
"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DungeonsAndDragons",
            "DungeonsAndDragons.toml",
            8,
        );
    }
}
