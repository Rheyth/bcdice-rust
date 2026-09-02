//! P4で手書き移植した `lib/bcdice/game_system/WorldsEndFrontline_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `WorldsEndFrontline` を継承し、`@locale` を `:ko_kr` に変えるだけで
//! `eval_game_system_specific_command` の上書きは無い。したがってコマンド解釈・判定は
//! [`super::WorldsEndFrontline`] の実装をそのまま使い、ここには `ko_kr` ロケールの
//! 定型文（`i18n/Bloodorium/ko_kr.yml`）だけを置く。

use super::WorldsEndFrontline::{eval_specific_command, SystemTexts, HELP_MESSAGE};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::WorldsEndFrontline_Korean`（ID: `WorldsEndFrontline:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldsEndFrontline_Korean;

impl GameSystem for WorldsEndFrontline_Korean {
    fn id(&self) -> &'static str {
        "WorldsEndFrontline:Korean"
    }

    fn name(&self) -> &'static str {
        "월드 엔드 프론트라인"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:월드 엔드 프론트라인"
    }

    /// Ruby は `HELP_MESSAGE` を上書きしないので、親から引き継いだ日本語の文面のまま。
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    /// Ruby `register_prefix_from_super_class()`。
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_TEXTS, command, rng)
    }
}

/// i18n `i18n/Bloodorium/ko_kr.yml`（`"《트라이엄프》(*%{triumph})"`）。
static KO_TEXTS: SystemTexts = SystemTexts {
    triumph_before: "《트라이엄프》(*",
    triumph_after: ")",
};

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WorldsEndFrontline:Korean",
            "WorldsEndFrontline_Korean.toml",
            10,
        );
    }
}
