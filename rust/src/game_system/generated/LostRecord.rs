//! P4で手書き移植した `lib/bcdice/game_system/LostRecord.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `@d66_sort_type = D66SortType::ASC`（小さい目が十の位）
//!
//! 固有コマンドも判定フックも持たない。

use crate::enums::D66SortType;
use crate::game_system::GameSystem;

/// Ruby `BCDice::GameSystem::LostRecord`（ID: `LostRecord`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostRecord;

impl GameSystem for LostRecord {
    fn id(&self) -> &'static str {
        "LostRecord"
    }

    fn name(&self) -> &'static str {
        "ロストレコード"
    }

    fn sort_key(&self) -> &'static str {
        "ろすとれこおと"
    }

    fn help_message(&self) -> &'static str {
        r"※このダイスボットは部屋のシステム名表示用となります。
D66を振った時、小さい目が十の位になります。
"
    }

    /// Ruby `LostRecord#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "LostRecord",
            "LostRecord.toml",
            2,
        );
    }
}
