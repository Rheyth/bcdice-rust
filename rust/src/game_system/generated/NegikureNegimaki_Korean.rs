//! P4で手書き移植した `lib/bcdice/game_system/NegikureNegimaki_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `NegikureNegimaki` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::NegikureNegimaki`] の関数をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。

use super::NegikureNegimaki::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr` の定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    result_level: "성공 레벨%{success_level}/요구%{required_level}",
    success_level: "성공 레벨%{success_level}",
    damage: "일반 피해%{normal_damage}/직격 피해%{direct_damage}",
    guts_loss: "거츠 감소%{guts_loss}",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::NegikureNegimaki_Korean`（ID: `NegikureNegimaki:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegikureNegimaki_Korean;

impl GameSystem for NegikureNegimaki_Korean {
    fn id(&self) -> &'static str {
        "NegikureNegimaki:Korean"
    }

    fn name(&self) -> &'static str {
        "네지쿠레 네지마키"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:네지쿠레 네지마키"
    }

    fn help_message(&self) -> &'static str {
        r"■ 행위 판정
nNNx#y: n개의 D6을 굴려, x 이상의 주사위 결과값의 개수를 성공 레벨로 판정.
n: 주사위 수（생략 시 1）
x: 난이도（생략 시 4）
y: 요구 성공 레벨（생략 시 1, 0은 1로 처리）

■ 전투 판정（공격 판정）
nNAx#y: n개의 D6을 굴려, x 이상을 성공으로 간주. y 이상의 성공은 직격 피해가 된다.
n: 주사위 수（생략 시 1）
x: 난이도（생략 시 4）
y: 크리티컬 값（생략 시 6, 0은 1로 처리）
일반 피해 = 성공 레벨 - 직격 피해
직격 피해 = 성공한 눈 중 y 이상의 개수
거츠 감소 = 주사위 결과값 1의 개수

■ 스트라이크 판정
nNS: n개의 D6을 굴려, 주사위 결과값 1의 개수만큼 거츠 감소를 산출한다
n: 주사위 수（생략 시 1）
거츠 감소가 0이면 성공, 1 이상이면 실패
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*NN\d*(#\d+)?", r"\d*NA\d*(#\d+)?", r"\d*NS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `NegikureNegimaki#eval_game_system_specific_command`（`ko_kr` の定型文で）。
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
        crate::game_system::test_support::assert_toml_cases_strict(
            "NegikureNegimaki:Korean",
            "NegikureNegimaki_Korean.toml",
            26,
        );
    }
}
