//! P4で手書き移植した `lib/bcdice/game_system/Magius_3rdNewTokyoCity.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Magius` を継承し、`get_result_of_ability_action` /
//! `get_result_of_skill_action` の2つだけを上書きする（ゾロ目 12/2 を絶対成功・絶対失敗にする）。
//! コマンド解釈とダイスの振り方は [`super::Magius`] の実装をそのまま使う。

use super::Magius::{eval_specific_command, SystemRules};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `Magius_3rdNewTokyoCity` が上書きする判定結果フック。
static RULES: SystemRules = SystemRules {
    ability_result: result_of_ability_action,
    skill_result: result_of_skill_action,
};

/// Ruby `Magius_3rdNewTokyoCity#get_result_of_ability_action`。
fn result_of_ability_action(total: i64, dice_add: i64, target: i64) -> EvalResult {
    if dice_add == 12 {
        EvalResult::critical("絶対成功")
    } else if dice_add == 2 {
        EvalResult::fumble("絶対失敗")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `Magius_3rdNewTokyoCity#get_result_of_skill_action`。
fn result_of_skill_action(total: i64, dice_add: i64, target: i64) -> EvalResult {
    if dice_add == 12 {
        EvalResult::critical("絶対成功")
    } else if dice_add == 2 {
        EvalResult::fumble("絶対失敗")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `BCDice::GameSystem::Magius_3rdNewTokyoCity`（ID: `Magius_3rdNewTokyoCity`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Magius_3rdNewTokyoCity;

impl GameSystem for Magius_3rdNewTokyoCity {
    fn id(&self) -> &'static str {
        "Magius_3rdNewTokyoCity"
    }

    fn name(&self) -> &'static str {
        "MAGIUS:新世紀エヴァンゲリオンRPG 決戦！第3新東京市"
    }

    fn sort_key(&self) -> &'static str {
        "まきうすしんせいきえうあんけりおんRPGけつせんたい3しんとうきようし"
    }

    fn help_message(&self) -> &'static str {
        r"■能力値判定　MA+x>=t        x:修正値 t:目標値
例)MA>=7: ダイスを2個振って、その結果(成功,失敗,絶対成功,絶対失敗)を表示

■技能値判定　MS+x>=t        x:修正値 t:目標値
例)MS>=7: ダイスを3個振って、そのうち上位2つを採用し、結果(成功,失敗,絶対成功,絶対失敗)を表示

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["M[AS]"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&RULES, self.round_type(), command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Magius_3rdNewTokyoCity",
            "Magius_3rdNewTokyoCity.toml",
            10,
        );
    }
}
