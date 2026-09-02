//! P4で手書き移植した `lib/bcdice/game_system/MagicPunk_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `MagicPunk` を継承し `@locale = :ko_kr` にするだけなので、
//! 判定は [`super::MagicPunk`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く
//! （`i18n/MagicPunk/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない）。

use super::MagicPunk::{roll_mp, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_SYSTEM: SystemTables = SystemTables {
    bad_beat: "실패(BB)",
    jackpot: "성공(JP)",
    success: "성공(%<value>d)",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::MagicPunk_Korean`（ID: `MagicPunk:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicPunk_Korean;

impl GameSystem for MagicPunk_Korean {
    fn id(&self) -> &'static str {
        "MagicPunk:Korean"
    }

    fn name(&self) -> &'static str {
        "매직펑크TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:매직펑크TRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 판정 (nMPm)
nD20을 굴려, m 이하의 눈이 있으면 성공.
m과 같은 눈이 있으면 잭팟(자동 성공).
모든 눈이 1이면 배드 비트(자동 실패).

■ 챌린지 판정 (nMPmCx)
통상 판정에 더해, 챌린지 값 x 이상의 눈이 필요.

■ 주사위 수 0개 (0MPmCx)
수정치 등으로 주사위 수가 0개가 된 경우 2d20을 굴림.
두 개의 눈 중 더 나쁜 쪽의 결과를 적용.
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"^\d*MP\d+"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_mp(&KO_SYSTEM, command, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MagicPunk:Korean",
            "MagicPunk_Korean.toml",
            14,
        );
    }
}
