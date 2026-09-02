//! P4で手書き移植した `lib/bcdice/game_system/SRS_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `SRS` を継承し、`@locale` を `:ko_kr` に変えて `HELP_MESSAGE` を
//! 差し替えるだけで、判定の実装は上書きしない（`aliases` も空のまま）。
//! そのため評価は [`super::SRS::eval_specific_command`] をそのまま使い、
//! ここには `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/SRS/ko_kr.yml` と `i18n/ko_kr.yml` から写したもので、
//! 値は1文字も変えていない。

use super::SRS::{eval_specific_command, Translations};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr`（`i18n/SRS/ko_kr.yml` と `i18n/ko_kr.yml`）。
static KO_KR: Translations = Translations {
    auto_success: "자동 성공",
    auto_failure: "자동 실패",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::SRS_Korean`（ID: `SRS:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SRS_Korean;

impl GameSystem for SRS_Korean {
    fn id(&self) -> &'static str {
        "SRS:Korean"
    }

    fn name(&self) -> &'static str {
        "스탠다드 RPG 시스템(SRS)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:스탠다드 RPG 시스템(SRS)"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_add_dice = true`（`SRS#initialize` から継承）。
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng, &KO_KR)
    }
}

/// Ruby `HELP_MESSAGE` 定数（`DEFAULT_HELP_MESSAGE`）。
const HELP_MESSAGE: &str = r"・판정
　・일반판정: 2D6+m@c#f>=t 또는 2D6+m>=t[c,f]
　　수정치 m, 목표치 t, 크리티컬치 c, 펌블치 f로 판정합니다.
　　수정치, 크리티컬치, 펌블치는 생략 가능합니다([]째로 생략 가능, @c・#f 지정 순서는 상관없음).
　　크리티컬치, 펌블치의 기본값은 각각 12, 2입니다.
　　자동성공, 자동실패, 성공, 실패를 자동 표시합니다.

　　예) 2d6>=10　　　　　수정치 0, 목표치 10으로 판정
　　예) 2d6+2>=10　　　　수정치 +2, 목표치 10으로 판정
　　예) 2d6+2>=10[11]　　↑를 크리티컬치 11로 판정
　　예) 2d6+2@11>=10 　　↑를 크리티컬치 11로 판정
　　예) 2d6+2>=10[12,4]　↑를 크리티컬치 12, 펌블치 4로 판정
　　예) 2d6+2@12#4>=10 　↑를 크리티컬치 12, 펌블치 4로 판정
　　예) 2d6+2>=10[,4]　　↑를 크리티컬치 12, 펌블치 4로 판정 (크리티컬치 생략)
　　예) 2d6+2#4>=10　　　↑를 크리티컬치 12, 펌블치 4로 판정 (크리티컬치 생략)

　・크리티컬 및 펌블만 판정: 2D6+m@c#f 또는 2D6+m[c,f]
　　목표치를 지정하지 않고, 수정치 m, 크리티컬치 c, 펌블치 f로 판정합니다.
　　수정치, 크리티컬치, 펌블치는 생략 가능합니다([]는 생략 불가, @c・#f 지정 순서는 상관없음).
　　자동성공, 자동실패를 자동 표시합니다.

　　예) 2d6[]　　　　수정치 0, 크리티컬치 12, 펌블치 2로 판정
　　예) 2d6+2[11]　　수정치 +2, 크리티컬치 11, 펌블치 2로 판정
　　예) 2d6+2@11 　　수정치 +2, 크리티컬치 11, 펌블치 2로 판정
　　예) 2d6+2[12,4]　수정치 +2, 크리티컬치 12, 펌블치 4로 판정
　　예) 2d6+2@12#4 　수정치 +2, 크리티컬치 12, 펌블치 4로 판정

・D66 주사위 있음 (순서 교체 없음)
";

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "SRS:Korean",
            "SRS_Korean.toml",
            74,
        );
    }
}
