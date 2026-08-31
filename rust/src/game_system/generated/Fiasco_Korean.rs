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
    use std::path::Path;

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    #[test]
    fn all_toml_cases_pass() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data/Fiasco_Korean.toml");
        if !path.exists() {
            return;
        }
        let data = TestDataFile::load(&path).expect("Fiasco_Korean.toml must parse");
        assert_eq!(data.tests.len(), 20);
        for (index, case) in data.tests.iter().enumerate() {
            let mut rng =
                SeededRandomizer::new(case.rands.iter().map(|rand| (rand.value, rand.sides)));
            let result = eval_command(&GameSystemId::new("Fiasco:Korean"), &case.input, &mut rng)
                .unwrap_or_else(|error| panic!("case {}: {error}", index + 1))
                .unwrap_or_else(|| panic!("case {} returned nil", index + 1));
            assert_eq!(result.text, case.output, "case {}", index + 1);
            assert_eq!(
                (
                    result.secret,
                    result.success,
                    result.failure,
                    result.critical,
                    result.fumble
                ),
                (
                    case.secret,
                    case.success,
                    case.failure,
                    case.critical,
                    case.fumble
                ),
                "case {} flags",
                index + 1
            );
            let expected_remaining = if [9, 10, 19, 20].contains(&(index + 1)) {
                6 // Ruby also returns before rolling on duplicate colors.
            } else {
                0
            };
            assert_eq!(
                rng.remaining(),
                expected_remaining,
                "case {} remaining random values",
                index + 1
            );
        }
    }
}
