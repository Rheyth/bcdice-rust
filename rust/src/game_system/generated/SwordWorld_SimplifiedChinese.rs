//! `SwordWorld` の簡体字中国語バリアント。

use super::SwordWorld::{check_result_2d6, eval_specific_command, SystemText};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::CheckOutcome;

/// `i18n/zh_hans.yml` と `i18n/SwordWorld/zh_hans.yml` の文言。
static ZH_HANS: SystemText = SystemText {
    success: "成功",
    failure: "失败",
    critical: "自动成功",
    fumble: "自动失败",
    keynumber_exceeds: "威力最大为100",
    infinite_critical: "请输入3以上的C值",
    round_suffix: "暴击",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwordWorld_SimplifiedChinese;

impl GameSystem for SwordWorld_SimplifiedChinese {
    fn id(&self) -> &'static str {
        "SwordWorld:SimplifiedChinese"
    }

    fn name(&self) -> &'static str {
        "剑世界"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Simplified Chinese:剑世界"
    }

    fn help_message(&self) -> &'static str {
        r"・SW　威力表　(Kx[c]+m$f) (x:威力值, c:暴击值, m:加值, f:骰子出目修正)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["H?K"]
    }

    crate::impl_prefixes_pattern!();

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(
            &ZH_HANS,
            total,
            crate::Int::from(dice_total),
            cmp_op,
            target,
        )
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&ZH_HANS, command, rng)
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/SwordWorld_SimplifiedChinese.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "SwordWorld:SimplifiedChinese",
            "SwordWorld_SimplifiedChinese.toml",
            230,
        );
    }
}
