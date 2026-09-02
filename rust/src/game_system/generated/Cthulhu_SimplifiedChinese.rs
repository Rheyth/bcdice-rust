//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_SimplifiedChinese.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:zh_hans` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `zh_hans` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/zh_hans.yml` と `i18n/zh_hans.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `zh_hans` ロケールの文言一式。
static ZH_HANS: Locale = Locale {
    success: "成功",
    failure: "失败",
    critical: "大成功",
    special: "极难成功",
    critical_special: "大成功/极难成功",
    fumble: "大失败",
    partial_success: "部分成功",
    automatic_success: "自动成功",
    automatic_failure: "自动失败",
    broken: "故障",
    broken_number: "故障率",
};

/// Ruby `BCDice::GameSystem::Cthulhu_SimplifiedChinese`（ID: `Cthulhu:SimplifiedChinese`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_SimplifiedChinese;

impl GameSystem for Cthulhu_SimplifiedChinese {
    fn id(&self) -> &'static str {
        "Cthulhu:SimplifiedChinese"
    }

    fn name(&self) -> &'static str {
        "克苏鲁的呼唤 第六版"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Simplified Chinese:克苏鲁的呼唤 第六版"
    }

    fn help_message(&self) -> &'static str {
        r"c=大成功值 ／ f=大失败值 ／ s=极难成功

1d100<=n    c・f・s全部关闭（只进行数值比较判定）

・带cfs判定的判定指令

CC	 掷1d100骰 c=1、f=100
CCB  同上，c=5、f=96

例：CC<=80  （以80技能値进行行为判定。并以1%的标准使用cf的值）
例：CCB<=55 （以55技能値进行行为判定。并以5%的标准使用cf的值）

・关于组合骰

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・关于对抗骰
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※故障值判定

・CC(x) c=1、f=100
x=故障值。骰点在x以上并且发生大失败时，会和大失败一起显示（文本为「大失败＆故障」）
没有发生大失败时，与成功或失败无关，文斗都会显示为「故障」（不显示成功或失败的情况下进行覆盖显示）

・CCB(x) c=5、f=96
同上

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`zh_hans` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:zh_hans`）を見るため
    /// `成功` / `失败` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&ZH_HANS, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :zh_hans`）。
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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Cthulhu:SimplifiedChinese",
            "Cthulhu_SimplifiedChinese.toml",
            103,
        );
    }
}
