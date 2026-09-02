//! P4で手書き移植した `lib/bcdice/game_system/Irisbane_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Irisbane` を継承し、`TABLES` を `translate_tables(:ko_kr)` に差し替えて
//! `@locale` を `:ko_kr` に変えるだけなので、判定の実装は [`super::Irisbane`] の
//! ものをそのまま使い、ここには `ko_kr` ロケールの表と定型文だけを置く。
//!
//! 表と文言は `i18n/Irisbane/ko_kr.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use super::Irisbane::{eval_specific_command, SystemTables};
use crate::dice_table::{D66LeftRangeTable, RangeInc};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `success`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の成功文言。
const GLOBAL_SUCCESS: &str = "성공";
/// i18n `failure`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の失敗文言。
const GLOBAL_FAILURE: &str = "실패";

/// `1..3` の行。
static KO_SCENE_SITUATION_ROW0: &[&str] = &[
    "【일상】아무것도 변하지 않는 일상의 한 장면. 변화하기 쉬운 세계에서는 그것이 매우 소중하다.",
    "【준비】무언가를 하기 위한 채비를 하는 한 장면. 정보 수집, 쇼핑 원정, 해야 할 일은 가득하다.",
    "【취미】자신의 시간을 유효하게 활용하는 한 장면. 쫓기지 않는 만큼 마음이 가볍고 상쾌하다.",
    "【카페】한숨 돌리며 기호품을 즐기는 한 장면. 온화한 공기는 그러나 왕왕 변하기 쉽다.",
    "【단련】몸을 단련하고 마음을 기르는 수련의 한 장면. 자신만 괜찮다면 그 방법도 무엇이든 좋다.",
    "【직무】역할에 따라 일에 힘쓰는 한 장면. 목적이 무엇이든 해야 할 일에는 변함이 없다.",
];
/// `4..6` の行。
static KO_SCENE_SITUATION_ROW1: &[&str] = &[
    "【이동】어딘가에서 어딘가로 향하는 한 장면. 나아가고 있다면 수단도 목적지도 상관없다.",
    "【묘전】고인이 잠든 장소를 찾는 한 장면. 함께 잠드는 일만은 없도록.",
    "【조작】무언가를 다루며 소원을 이루는 한 장면. 운전이든 무엇이든 딴 곳을 보는 것은 주의가 필요하다.",
    "【식사】무언가를 양식으로 삼아 자신의 힘을 비축하는 한 장면. 행동하면 소모된다. 배고프면 아무것도 안 된다.",
    "【휴식】나날의 틈새, 쉬어 가는 한 장면. '아무것도 하지 않는다'는 것도 훌륭한 행동이다.",
    "【몽환】현실에 존재하지 않는 무언가에 빠지는 한 장면. 시간대에 상관없이 언젠가는 반드시 깨어날 것이다.",
];
static KO_SCENE_SITUATION_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 3), KO_SCENE_SITUATION_ROW0),
    (RangeInc::new(4, 6), KO_SCENE_SITUATION_ROW1),
];
static KO_SCENE_SITUATION: D66LeftRangeTable =
    D66LeftRangeTable::new("시추에이션", D66SortType::NoSort, KO_SCENE_SITUATION_ITEMS);
/// `ko_kr` ロケールの表と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: &[("SCENESITUATION", &KO_SCENE_SITUATION)],
    zero_dice_count: "판정 수가 0입니다",
    success_dice_count: "성공 주사위 수 %{count}",
    attack_power: "× 공격력 %{power}",
    damage: "대미지 %{damage}",
    damage_with_mod: "대미지 %{damage}%{operator}%{mod_value}",
};

/// Ruby `BCDice::GameSystem::Irisbane_Korean`（ID: `Irisbane:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Irisbane_Korean;

impl GameSystem for Irisbane_Korean {
    fn id(&self) -> &'static str {
        "Irisbane:Korean"
    }

    fn name(&self) -> &'static str {
        "눈 돌리지 않는 이리스베인"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:눈 돌리지 않는 이리스베인"
    }

    fn help_message(&self) -> &'static str {
        r"■공격 판정（ ATTACKx@y<=z ）
x: 공격력
y: 판정 수
z: 목표값
（※ ATTACK 은 ATK 또는 AT 로 줄여 쓸 수 있습니다）
예） ATTACK2@3<=5
예） ATK10@2<=4
예） AT8@3<=2

위 x y z 에는 각각 사칙연산을 지정할 수 있습니다.
예） ATTACK2+7@3*2<=5-1

□공격 판정의 데미지 증감（ ATTACKx@y<=z[+a]  ATTACKx@y<=z[-a]）
말미에 [+a] 또는 [-a] 를 지정하면 최종 데미지를 증감할 수 있습니다.
a: 증감량
예） ATTACK2@3<=5[+10]
예） ATK10@2<=4[-8]
예） AT8@3<=2[-8+5]

■시추에이션（p115）
SceneSituation, SSi
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AT(TACK|K)?", "SCENESITUATION", "SSI"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 親クラスは `result_1d100` などを一切上書きしないので、目標値つきの汎用コマンド
    /// （`2D6>=7` など）はすべてこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(GLOBAL_SUCCESS))
        } else {
            Some(EvalResult::failure(GLOBAL_FAILURE))
        }
    }

    /// Ruby `Irisbane#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Irisbane#initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `Irisbane#eval_game_system_specific_command`。
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

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/Irisbane_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Irisbane:Korean",
            "Irisbane_Korean.toml",
            41,
        );
    }

    /// 汎用コマンドの成功／失敗が `ko_kr` の定型文になること。
    ///
    /// Ruby は `Base#result_ndx` の `translate("success")` が `@locale = :ko_kr` を見るため
    /// `성공` / `실패` になる。TOMLにこの経路のケースが無いのでここで固定する。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            (
                "2D6>=7",
                vec![(4, 6), (5, 6)],
                "(2D6>=7) ＞ 9[4,5] ＞ 9 ＞ 성공",
            ),
            (
                "2D6>=10",
                vec![(4, 6), (5, 6)],
                "(2D6>=10) ＞ 9[4,5] ＞ 9 ＞ 실패",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("Irisbane:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
