//! `lib/bcdice/game_system/Fiasco_Korean.rb` の移植。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

use super::Fiasco::{eval_specific_command, SystemTables};

static KO_SYSTEM: SystemTables = SystemTables {
    white: "흰색",
    black: "검은색",
    count_suffix: "개",
    duplicate_white: "흰색 지정(W)은 중복될 수 없습니다.",
    duplicate_black: "검은색 지정(B)은 중복될 수 없습니다.",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fiasco_Korean;

impl GameSystem for Fiasco_Korean {
    fn id(&self) -> &'static str {
        "Fiasco:Korean"
    }

    fn name(&self) -> &'static str {
        "피아스코"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:피아스코"
    }

    fn help_message(&self) -> &'static str {
        r"  ・판정 커맨드(FSx, WxBx)
    관계, 비틀기 요소용(FSx)：관계나 비틀기 요소를 위해 x개의 다이스를 굴려 나온 값별로 분류한다.
    흑백차이판정용(WxBx)    ：비틀기, 후기를 위해 흰 다이스(W지정)과 검은 다이스(B지정)으로 차이를 구한다.
      ※ W와B는 한 쪽만 지정(Bx, Wx), 앞뒤 바꿔 지정(WxBx,BxWx)도 가능
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["FS", "W", "B"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "Fiasco:Korean",
            "Fiasco_Korean.toml",
            20,
            &[(9, 6), (10, 6), (19, 6), (20, 6)],
        );
    }
}
