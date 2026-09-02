//! `lib/bcdice/game_system/TokyoNova.rb` の手書き移植。

use crate::game_system::GameSystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokyoNova;

impl GameSystem for TokyoNova {
    fn id(&self) -> &'static str {
        "TokyoNova"
    }

    fn name(&self) -> &'static str {
        "トーキョーN◎VA"
    }

    fn sort_key(&self) -> &'static str {
        "とおきよおのは"
    }

    fn help_message(&self) -> &'static str {
        "※このダイスボットは部屋のシステム名表示用となります。\n"
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/TokyoNova.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TokyoNova",
            "TokyoNova.toml",
            8,
        );
    }
}
