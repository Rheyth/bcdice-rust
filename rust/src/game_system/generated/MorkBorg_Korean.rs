//! P4で手書き移植した `lib/bcdice/game_system/MorkBorg_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `MorkBorg` を継承し、`TABLES` を `translate_tables(:ko_kr)` に差し替えて
//! `@locale` を `:ko_kr` に変えるだけなので、判定の実装は [`super::MorkBorg`] の
//! ものをそのまま使い、ここには `ko_kr` ロケールの表と定型文だけを置く。
//!
//! 表と文言は `i18n/MorkBorg/ko_kr.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use super::MorkBorg::{eval_specific_command, SystemTables};
use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `success`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の成功文言。
const GLOBAL_SUCCESS: &str = "성공";
/// i18n `failure`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の失敗文言。
const GLOBAL_FAILURE: &str = "실패";

/// i18n `MorkBorg.ERT.items`。
static KO_ERT_ITEMS: &[&str] = &[
    "죽인다!",
    "죽인다!",
    "격앙",
    "격앙",
    "격앙",
    "무관심",
    "무관심",
    "대체로 우호적",
    "대체로 우호적",
    "협조적",
    "협조적",
];
/// i18n `MorkBorg.ERT`（예상치 못한 반응표 / 2D6）。
static KO_ERT: Table = Table::from_dice("예상치 못한 반응표", 2, 6, KO_ERT_ITEMS);

/// i18n `MorkBorg.BRO.items`。
static KO_BRO_ITEMS: &[&str] = &[
    "d4 라운드 동안 기절하며, d4 HP를 회복하고 깨어난다.",
    "d6을 굴린다: 1~5 = 팔다리 골절 또는 절단. 6 = 한쪽 눈을 잃음. d4 라운드 동안 행동 불능이 되며, 그 후 d4 HP를 회복하고 다시 움직일 수 있게 된다.",
    "과다출혈: 처치하지 않으면 d2 시간 이내에 사망한다. 첫 1시간 동안은 모든 판정이 DR16이 되며, 마지막 1시간 동안은 DR18이 된다.",
    "죽는다.",
];
/// i18n `MorkBorg.BRO`（빈사표 / 1D4）。
static KO_BRO: Table = Table::from_dice("빈사표", 1, 4, KO_BRO_ITEMS);

/// `ko_kr` ロケールの表と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: &[("ERT", &KO_ERT), ("BRO", &KO_BRO)],
    fumble: "펌블",
    critical: "크리티컬",
    success: "성공",
    failure: "실패",
    pcs_go_first: "PC 선공",
    enemies_go_first: "적 선공",
    maintain: "유지되었다",
    flee: "(도주)",
    surrender: "(항복)",
};

/// Ruby `BCDice::GameSystem::MorkBorg_Korean`（ID: `MorkBorg:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorkBorg_Korean;

impl GameSystem for MorkBorg_Korean {
    fn id(&self) -> &'static str {
        "MorkBorg:Korean"
    }

    fn name(&self) -> &'static str {
        "모크 보그(MÖRK BORG)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:모크 보그(MÖRK BORG)"
    }

    fn help_message(&self) -> &'static str {
        r"■판정　sDRt        s: 능력치(생략 시:0) t:목표값

예)+3DR12: 능력치+3, DR12로 1d20을 굴려서 결과 표시(크리티컬·펌블도 표시)

■이니셔티브　sINS s: 능력치(생략 시:0. 개별 이니셔티브를 사용하는 경우)

예)INS: 1d6을 굴려서 이니셔티브 결과 표시(PC 선공을 성공으로 표시)

■모럴　sMORt s: 능력치(생략 시:0) t:상대 크리처의 모럴 값

예)MOR8: 2d6을 굴려서 모럴 판정 결과 표시(모럴 붕괴를 성공으로 표시)


■각종 표

・조우 반응표 Reaction (ERT)
・파손(빈사표) Broken (BRO)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"([+-]?\d+)?DR[\d]+",
            r"([+-]?\d+)?INS",
            r"([+-]?\d+)?MOR",
            "ERT",
            "BRO",
        ]
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

    /// Ruby `MorkBorg#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
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

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/MorkBorg_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MorkBorg:Korean",
            "MorkBorg_Korean.toml",
            42,
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
            let result = eval_command(&GameSystemId::new("MorkBorg:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
