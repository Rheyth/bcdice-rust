//! P4で手書き移植した `lib/bcdice/game_system/SevenFortressMobius.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `NightWizard` を継承し、`initialize` で `@nw_command` を
//! `"SFM"` に変えるだけなので、判定の実装は [`super::NightWizard`] のものを
//! そのまま使い、ここには判定コマンドの語の設定だけを置く。

use std::sync::OnceLock;

use regex::Regex;

use super::NightWizard::{build_nw_pattern, eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `SevenFortressMobius#initialize` の `@nw_command = "SFM"`。
fn sfm_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| build_nw_pattern("SFM"))
}

/// `SevenFortressMobius` の設定一式。
static SFM_SYSTEM: SystemTables = SystemTables {
    nw_command: "SFM",
    nw_pattern: sfm_pattern,
};

/// Ruby `BCDice::GameSystem::SevenFortressMobius`（ID: `SevenFortressMobius`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SevenFortressMobius;

impl GameSystem for SevenFortressMobius {
    fn id(&self) -> &'static str {
        "SevenFortressMobius"
    }

    fn name(&self) -> &'static str {
        "セブン＝フォートレス メビウス"
    }

    fn sort_key(&self) -> &'static str {
        "せふんふおおとれすめひうす"
    }

    fn help_message(&self) -> &'static str {
        r#"・判定用コマンド　(nSFM+m@x#y)
　"(基本値)SFM(常時および常時に準じる特技等及び状態異常（省略可）)@(クリティカル値)#(ファンブル値)（常時以外の特技等及び味方の支援効果等の影響（省略可））"でロールします。
　Rコマンド(2R6m[n,m]c[x]f[y]>=t tは目標値)に読替されます。
　クリティカル値、ファンブル値が無い場合は1や13などのあり得ない数値を入れてください。
　例）12SFM-5@7#2　　1SFM　　50SFM+5@7,10#2,5　50SFM-5+10@7,10#2,5+15+25
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"([-+]?\d+)?SFM", "2R6"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&SFM_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "SevenFortressMobius",
            "SevenFortressMobius.toml",
            4,
        );
    }
}
