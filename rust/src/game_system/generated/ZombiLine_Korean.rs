//! P4で手書き移植した `lib/bcdice/game_system/ZombiLine_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `ZombiLine` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! 判定・表の引き方は [`super::ZombiLine`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データと定型文だけを置く
//! （`i18n/ZombiLine/ko_kr.yml` から機械的に書き出したもので、値は1文字も変えていない）。

use super::ZombiLine::{eval_specific_command, RollText, SystemTables};
use crate::dice_table::{RangeInc, RangeTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static KO_SST_ITEMS: &[&str] = &[
    "[분노] 가장 가까이 있는 적을 공격(성공률 +20%)합니다. 가까이 적이 없는 경우, 누군가의 스트레스를 +1시킵니다. / 설명: 머리에 피가 몰려 누군가에게 화를 터트립니다.",
    "[도피] 낙하하더라도 적에게서 도망치려 이동합니다. 주변에 적이 없는 경우, 현실도피를 하려 합니다. / 설명: 견딜 수 없게 되어 도망칩니다.",
    "[환상] 전투 중에는 「행동방치(전AP)」를 합니다. 전투 이외의 경우, 환상을 보며 웃음을 흘립니다. / 설명: 자신이 꿈꾸는 환상을 봅니다.",
    "[절규] 전투 중에는 비명을 지르며 「주목을 모은다(2AP)」를 합니다. 전투 이외의 경우, 무의미한 소리를 지릅니다. / 설명: 생각없이 비명을 지릅니다.",
    "[자해] 스스로 【상처】를 입습니다. 전투 중에는 「자해행위(1AP)」로 스스로에게 【상처】를 입힙니다. / 설명: 생각없이 자신에게 상처를 입힙니다.",
    "[불안] 누군가의 스트레스를 1 올립니다. 근처에 아무도 없는 경우, 울음을 터트립니다. / 설명: 불안해져 쓸데없는 말을 지껄입니다.",
    "[기피] 그 자리에서 가장 가까이 있는 대상에게 「돌(1AP)」을 던집니다. 그게 불가능한 경우, 【넘어짐】상태가 됩니다. / 설명: 혐오감으로 모든 것을 거부합니다.",
    "[폭주] 가장 가까이 있는 적에게 공격을 가합니다. 가까이 적이 없는 경우 주변의 의견을 듣지 않고 제멋대로 행동을 합니다. / 설명: 냉정을 찾지 못하고 제멋대로 행동합니다.",
    "[혼란] 가까이 있는 무작위 대상에게 격투로 공격을 가합니다. 그게 불가능한 경우 「행동방치(전AP)」를 합니다. / 설명: 세상 모든 것이 적으로 보여 공격합니다.",
    "[개안] 스트레스가 0까지 떨어집니다. 당신은 교조가 되어 교의를 한가지 정해 「포교」할 수 있습니다. 다음 증상이 나올 때까지 효과가 지속됩니다. / 설명: 좀비투성이 세상의 진리를 보았습니다.",
];
static KO_SST: Table = Table::from_dice("스트레스 증상표", 1, 10, KO_SST_ITEMS);

static KO_IT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 50), "신선 식재료"),
    (RangeInc::new(51, 80), "수상한 식재료"),
    (RangeInc::new(81, 100), "위험한 식재료"),
];
static KO_IT: RangeTable = RangeTable::from_dice("식재료 표", 1, 100, KO_IT_ITEMS);

static KO_TABLES: &[(&str, &dyn RollText)] = &[("SST", &KO_SST), ("IT", &KO_IT)];

static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    success_critical: "성공(크리티컬)",
    success_fumble: "성공(펌블)",
    success: "성공",
    failure_fumble: "실패(펌블)",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::ZombiLine_Korean`（ID: `ZombiLine:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZombiLine_Korean;

impl GameSystem for ZombiLine_Korean {
    fn id(&self) -> &'static str {
        "ZombiLine:Korean"
    }

    fn name(&self) -> &'static str {
        "좀비 라인"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:좀비 라인"
    }

    fn help_message(&self) -> &'static str {
        r"■ 판정 (xZL<=y)
　x：주사위 개수(생략 시 1)
　y：성공률

■ 각종 표
　스트레스 증상표 SST
　식재료 표 IT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*ZL", "SST", "IT"]
    }

    crate::impl_prefixes_pattern!();

    fn sides_implicit_d(&self) -> i64 {
        10
    }

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
            "ZombiLine:Korean",
            "ZombiLine_Korean.toml",
            23,
        );
    }
}
