//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons5_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DungeonsAndDragons5` を継承し、`@locale` を `:ko_kr` に変えて
//! `register_prefix_from_super_class` するだけで、判定の実装は上書きしない。
//! そのため評価は [`super::DungeonsAndDragons5::eval_specific_command`] をそのまま使い、
//! ここには `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/ko_kr.yml` から写したもので、値は1文字も変えていない。

use super::DungeonsAndDragons5::{eval_specific_command, Translations};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr`（`i18n/ko_kr.yml`）。
static KO_KR: Translations = Translations {
    critical: "크리티컬",
    fumble: "펌블",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::DungeonsAndDragons5_Korean`（ID: `DungeonsAndDragons5:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons5_Korean;

impl GameSystem for DungeonsAndDragons5_Korean {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons5:Korean"
    }

    fn name(&self) -> &'static str {
        "던전 앤 드래곤 5판"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:던전 앤 드래곤 5판"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AT", "AR", "2H"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng, &KO_KR)
    }
}

/// Ruby `HELP_MESSAGE` 定数。
const HELP_MESSAGE: &str = r"・명중 굴림　AT[x][@c][>=t][y]
　x: +- 수정치 (생략 가능)
　c: 크리티컬 수치 (생략 가능)
　t: 목표 AC (>= 포함, 생략 가능)
　y: 유리(A), 불리(D) (생략 가능)
　B: 브레스나 가이던스 등의 보너스 (생략 가능)
　※보충 설명: B만 입력하면 +1d4를, B[1D4+1D8] 와 같이 입력하면 []안의 주사위를 추가로 굴립니다.


　펌블/실패/성공/크리티컬을 자동으로 판정합니다.
　예시）AT AT>=10 AT+5>=18 AT-3>=16 ATA AT>=10A AT+3>=18A AT-3>=16 ATD AT>=10D AT+5>=18D AT-5>=16D
　    AT@19 AT+5@18 AT-2@19>=15

・능력 판정　AR[x][>=t][y]
　명중 굴림과 동일. 성공/실패 결과를 자동 판정합니다.
　예시）AR AR>=10 AR+5>=18 AR-3>=16 ARA AR>=10A AR+3>=18A AR-3>=16 ARD AR>=10D AR+5>=18D AR-5>=16D

・대형 무기 전투술 대미지 계산(베이직 룰북 32p)　2HnDx[m]
　n: 주사위 개수
　x: 주사위 면수(1d6의 6, 1d8의 8 등)
　m: +- 수정치 (생략 가능)
　팔라딘과 파이터의 무기를 양손으로 사용할 경우, 대미지 주사위에서 1 또는 2가 나오면 다시 굴립니다.
　예시)2H3D6 2H1D10+3 2H2D8-1
";

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DungeonsAndDragons5:Korean",
            "DungeonsAndDragons5_Korean.toml",
            74,
        );
    }
}
