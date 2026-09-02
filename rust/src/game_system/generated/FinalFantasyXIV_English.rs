//! `lib/bcdice/game_system/FinalFantasyXIV_English.rb` の移植。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

use super::FinalFantasyXIV::{eval_specific_command, SystemTables};

static EN_SYSTEM: SystemTables = SystemTables {
    critical: "Critical",
    direct_hit: "Direct Hit",
    normal_hit: "Only Base Effect",
    success: "Success",
    failure: "Failure",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalFantasyXIV_English;

impl GameSystem for FinalFantasyXIV_English {
    fn id(&self) -> &'static str {
        "FinalFantasyXIV:English"
    }

    fn name(&self) -> &'static str {
        "FINAL FANTSY XIV TTRPG(English)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:English:FINAL FANTASY XIV TTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"Ability Checks nAB+m>=CR
  Perform a d20 ability check. If a die count is specified, the highest roll is adopted.
  n: die count(optional)
  m: modifiy number(optional)
  CR: Challenge Ratting(optional)
  Base Effect only, Direct hit and Critical are automatically evaluated.
  Example: AB, AB+5, AB+5>=14, 2AB+5>=14
Making checks nDC+m>=CR
  Same as ability check.
  Success and Failure ar automatically evaluated.
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*AB", r"\d*DC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&EN_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "FinalFantasyXIV:English",
            "FinalFantasyXIV_English.toml",
            20,
        );
    }
}
