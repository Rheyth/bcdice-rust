//! P4で手書き移植した `lib/bcdice/game_system/Ventangle_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Ventangle` を継承し `@locale = :ko_kr` にするだけなので、
//! 判定は [`super::Ventangle`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの定型文（`i18n/Ventangle/ko_kr.yml`）だけを置く。

use super::Ventangle::{eval_specific_command, SystemTexts};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_TEXTS: SystemTexts = SystemTexts {
    special: "스페셜",
    level_gap: "갭 보너스(%<gap>d)",
    fumble: "펌블",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::Ventangle_Korean`（ID: `Ventangle:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ventangle_Korean;

impl GameSystem for Ventangle_Korean {
    fn id(&self) -> &'static str {
        "Ventangle:Korean"
    }

    fn name(&self) -> &'static str {
        "벤탱글"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:벤탱글"
    }

    fn help_message(&self) -> &'static str {
        r"기본 양식 VTn@s#f$g>=T n=주사위 개수（생략 시 2） s=스페셜치（생략 시 12） f=펌블치（생략 시 2） g=레벨 갭 판정치（생략 가능） T=목표치（생략 가능）

예시：
VT        기본 스페셜치, 펌블치로 판정
VT@10#3   스페셜치 10, 펌블치 3으로 판정
VT3@10#3  어드밴티지 1점을 사용해 스페셜치 10, 펌블치 3 판정을 주사위 3개로 판정

VT>=5         기본 스페셜치, 펌블치로 목표치 5 판정
VT@10#3>=5    스페셜치 10, 펌블치 3으로 목표치 5 판정
VT@10#3$5>=5  스페셜치 10, 펌블치 3으로 목표치 5 판정. 이때 달성치가 목표치보다 5이상 큰 경우, 갭 보너스를 표시
VT3@10#3>=5   어드밴티지 1점을 사용해 스페셜치 10, 펌블치 3, 목표치 5 판정을 주사위 3개로 판정
VT3@10#3$4>=5 어드밴티지 1점을 사용해 스페셜치 10, 펌블치 3, 목표치 5 판정을 주사위 3개로 판정. 이때 달성치가 목표치보다 4이상 큰 경우, 갭 보너스를 표시
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["VT"]
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

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Ventangle:Korean",
            "Ventangle_Korean.toml",
            47,
        );
    }
}
