//! P4で手書き移植した `lib/bcdice/game_system/Airgetlamh_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Airgetlamh` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Airgetlamh`] の関数をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。

use super::Airgetlamh::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr` の定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    damage: "%<count>d 대미지",
    success_count: "성공 수 : %<count>d",
    critical: "%<count>d 크리티컬",
};

/// Ruby `BCDice::GameSystem::Airgetlamh_Korean`（ID: `Airgetlamh:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Airgetlamh_Korean;

impl GameSystem for Airgetlamh_Korean {
    fn id(&self) -> &'static str {
        "Airgetlamh:Korean"
    }

    fn name(&self) -> &'static str {
        "붉은 고탑의 에어게트람"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:붉은 고탑의 에어게트람"
    }

    fn help_message(&self) -> &'static str {
        r"【Reg2.0『THE ANSWERER』～】
・조사 판정（성공 수 표시）：[n]AA[m]
・명중 판정（대미지 표시）：[n]AA[m]*p[+t][Cx]
【～Reg1.1『승화(昇華)』】
・조사 판정（성공 수 표시）：[n]AL[m]
・명중 판정（대미지 표시）：[n]AL[m]*p
----------------------------------------
[]안의 커맨드는 생략 가능.

「n」으로 주사위 수（공격 횟수）지정. 생략 시「2」.
「m」으로 목표값 지정. 생략 시「6」.
「p」으로 위력 지정.「*」는「x」로 대체 가능.
「+t」으로 크리티컬 트리거 지정. 생략 가능.
「Cx」으로 크리티컬 값 지정. 생략 시「1」, 최대값「3」,「0」은 크리티컬 없음.

공격력 지정으로 명중 판정이 되며, 성공 수가 아닌 대미지를 결과로 표시합니다.
크리티컬 히트 수만큼 자동으로 추가 굴림 처리를 합니다.
（AL 커맨드에서는 크리티컬 처리를 하지 않습니다）

【사용 예시】
・AL → 2d10으로 목표값 6의 조사 판정.
・5AA7*12 → 5d10으로 목표값 7, 위력 12의 명중 판정.
・AA7x28+5 → 2d10으로 목표값 7, 위력 28, 크리티컬 트리거 5의 명중 판정.
・9aa5*10C2 → 9d10으로 목표값 5, 위력 10, 크리티컬 값 2의 명중 판정.
・15AAx4c0 → 15d10으로 목표값 6, 위력 4, 크리티컬 없음의 명중 판정.
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*A[AL]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Airgetlamh#initialize` の `@sort_add_dice = true`（親から継承）。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Airgetlamh#eval_game_system_specific_command`（`ko_kr` の定型文で）。
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
            "Airgetlamh:Korean",
            "Airgetlamh_Korean.toml",
            29,
        );
    }
}
