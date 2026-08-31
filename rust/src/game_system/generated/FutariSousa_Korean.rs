//! `lib/bcdice/game_system/FutariSousa_Korean.rb` の手書き移植。

use std::sync::OnceLock;

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

use super::FutariSousa::{eval_specific_command, ruby_help, SystemText, PREFIXES};

static KO_KR: SystemText = SystemText {
    yaml: include_str!("../../../../i18n/FutariSousa/ko_kr.yml"),
    success: "성공",
    failure: "실패",
    dt_fumble: "펌블（상태 이상을 받고, 조수의 마음고생이 1점 상승）",
    dt_special: "스페셜（조수의 여유를 1점 획득）",
    as_fumble: "펌블（상태 이상을 받고, 조수의 마음고생이 1점 상승）",
    as_special: "스페셜（여유 2점과, 탐정의 조수를 향한 감정을 획득）",
    as_success: "성공（여유 1점과, 탐정의 조수를 향한 감정을 획득）",
    consume_shrd_text_rand: false,
};

fn help_message() -> &'static str {
    static HELP: OnceLock<String> = OnceLock::new();
    HELP.get_or_init(|| {
        ruby_help(
            include_str!("../../../../lib/bcdice/game_system/FutariSousa_Korean.rb"),
            "MESSAGETEXT",
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct FutariSousa_Korean;

impl GameSystem for FutariSousa_Korean {
    fn id(&self) -> &'static str {
        "FutariSousa:Korean"
    }
    fn name(&self) -> &'static str {
        "둘이서 수사"
    }
    fn sort_key(&self) -> &'static str {
        "国際化:Korean:둘이서 수사"
    }
    fn help_message(&self) -> &'static str {
        help_message()
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }
    crate::impl_prefixes_pattern!();
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_KR, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        super::super::TokyoNova::assert_toml_cases(
            "FutariSousa:Korean",
            "FutariSousa_Korean.toml",
            144,
        );
    }
}
