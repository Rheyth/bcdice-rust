//! P4で手書き移植した `lib/bcdice/game_system/Alsetto_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Alsetto` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Alsetto`] の関数をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。

use super::Alsetto::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// i18n `ko_kr` の定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    damage: "%{total_damage} 대미지",
    success_count: "성공 수 : %{success_count}",
    triumph: " / %{critical_count} 트라이엄프",
};

/// Ruby `BCDice::GameSystem::Alsetto_Korean`（ID: `Alsetto:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alsetto_Korean;

impl GameSystem for Alsetto_Korean {
    fn id(&self) -> &'static str {
        "Alsetto:Korean"
    }

    fn name(&self) -> &'static str {
        "시편의 알세토"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:시편의 알세토"
    }

    fn help_message(&self) -> &'static str {
        r"・성공 판정：nAL[m]　　　　・트라이엄프 없음：nALC[m]
・명중 판정：nAL[m]*p　　　・트라이엄프 없음：nALC[m]*p
・명중 판정(건슬링거의 근원시)：nALG[m]*p
[] 내부는 생략 가능.

AL 커맨드는 트라이엄프 수만큼, 자동으로 추가 주사위 굴림 처리를 수행합니다.
「n」으로 주사위 수를 지정.
「m」으로 목표치를 지정. 생략 시에는 기본값인 「3」이 사용됩니다.
「p」로 공격력을 지정. 「*」 대신 「x」도 사용 가능.
공격력을 지정하면 명중 판정이 되며, 성공수가 아닌 대미지를 결과로 표시합니다.

ALC 커맨드는 트라이엄프 없이 성공수, 대미지를 결과로 표시합니다.
ALG 커맨드는 「2 이하」에서 트라이엄프 처리를 수행합니다.

【사용 예시】
・5AL → 5d6에서 목표치 3.
・5ALC → 5d6에서 목표치 3. 트라이엄프 없음.
・6AL2 → 6d6에서 목표치 2.
・4AL*5 → 4d6에서 목표치 3, 공격력 5의 명중 판정.
・7AL2x10 → 7d6에서 목표치 2, 공격력 10의 명중 판정.
・8ALC4x5 → 8d6에서 목표치 4, 공격력 5, 트라이엄프 없는 명중 판정.
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AL[CG]?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Alsetto#eval_game_system_specific_command`（`ko_kr` の定型文で）。
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
            "Alsetto:Korean",
            "Alsetto_Korean.toml",
            25,
        );
    }
}
