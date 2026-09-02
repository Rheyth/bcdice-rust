//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_ChineseTraditional.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:zh_hant` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `zh_hant` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/zh_hant.yml` と `i18n/zh_hant.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `zh_hant` ロケールの文言一式。
static ZH_HANT: Locale = Locale {
    success: "成功",
    failure: "失敗",
    critical: "決定性的成功",
    special: "特殊",
    critical_special: "決定性的成功/特殊",
    fumble: "致命性失敗",
    partial_success: "部分性成功",
    automatic_success: "自動成功",
    automatic_failure: "自動失敗",
    broken: "故障",
    broken_number: "故障率",
};

/// Ruby `BCDice::GameSystem::Cthulhu_ChineseTraditional`（ID: `Cthulhu:ChineseTraditional`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_ChineseTraditional;

impl GameSystem for Cthulhu_ChineseTraditional {
    fn id(&self) -> &'static str {
        "Cthulhu:ChineseTraditional"
    }

    fn name(&self) -> &'static str {
        "克蘇魯神話"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Chinese Traditional:克蘇魯神話"
    }

    fn help_message(&self) -> &'static str {
        r"c=爆擊率 ／ f=大失敗值 ／ s=特殊

1d100<=n    c・f・s全關閉（只進行單純數值比較判定）

・cfs付註判定指令

CC	 1d100擲骰 c=1、f=100
CCB  同上、c=5、f=96

例：CC<=80  （以技能值80來判定。cf適用於1%規則）
例：CCB<=55 （以技能值55來判定。cf適用於5%規則）

・關於組合骰組

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・關於對抗骰
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※故障率判定

・CC(x) c=1、f=100
x=故障率。擲出骰值x以上時、需在大失敗發生同時輸出（參照「大失敗＆故障」）
沒有大失敗時，無論成功或失敗只需參考[故障]來輸出(並非成功或失敗來輸出，而是覆蓋上去並對其輸出)

・CCB(x) c=5、f=96
同上

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`zh_hant` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:zh_hant`）を見るため
    /// `成功` / `失敗` になる。値は `ja_jp` と同じだが、参照するYAMLが違うので
    /// 他のバリアントと同じ形で明示的に上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&ZH_HANT, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :zh_hant`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&ZH_HANT, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Cthulhu:ChineseTraditional",
            "Cthulhu_ChineseTraditional.toml",
            100,
        );
    }
}
