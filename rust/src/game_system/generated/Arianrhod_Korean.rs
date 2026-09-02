//! P4で手書き移植した `lib/bcdice/game_system/Arianrhod_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Arianrhod` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Arianrhod`] のものをそのまま使い、ここには `ko_kr`
//! ロケールの定型文だけを置く。
//!
//! 文言は `i18n/Arianrhod/ko_kr.yml` と `i18n/ko_kr.yml` から写したもので、
//! 値は1文字も変えていない。
//!
//! 接頭辞の空文字列は Ruby `register_prefix_from_super_class` が親の `nil` を
//! 1件登録してしまう既知の挙動（スタブの `prefixes: [""]`）を維持する。

use super::Arianrhod::{result_nd6_impl, Messages};
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// i18n `ko_kr`（`i18n/Arianrhod/ko_kr.yml` と `i18n/ko_kr.yml`）。
static KO_MESSAGES: Messages = Messages {
    fumble: "펌블",
    critical: "크리티컬(+%{dice}D6)",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::Arianrhod_Korean`（ID: `Arianrhod:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arianrhod_Korean;

impl GameSystem for Arianrhod_Korean {
    fn id(&self) -> &'static str {
        "Arianrhod:Korean"
    }

    fn name(&self) -> &'static str {
        "아리안로드RPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:아리안로드RPG"
    }

    fn help_message(&self) -> &'static str {
        r"・크리티컬, 펌블의 자동판정을 행합니다.(크리티컬 시의 추가 대미지도 표시됩니다)
・D66 다이스 있음
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[""]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Arianrhod#initialize` の `@sort_add_dice = true`（継承）。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Arianrhod#result_nd6`（`ko_kr` の定型文で）。
    fn result_nd6(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd6_impl(
            &KO_MESSAGES,
            crate::randomizer::sat_i64(&total),
            value_list,
            cmp_op,
            target,
        )
    }

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(KO_MESSAGES.success))
        } else {
            Some(EvalResult::failure(KO_MESSAGES.failure))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Arianrhod:Korean",
            "Arianrhod_Korean.toml",
            27,
        );
    }
}
