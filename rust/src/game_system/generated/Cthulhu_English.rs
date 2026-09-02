//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_English.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:en_us` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `en_us` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/en_us.yml` と `i18n/en_us.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `en_us` ロケールの文言一式。
static EN_US: Locale = Locale {
    success: "Success",
    failure: "Failure",
    critical: "Critical Success",
    special: "Special",
    critical_special: "Critical Success/Special",
    fumble: "Fumble",
    partial_success: "Partial Success",
    automatic_success: "Automatic Success",
    automatic_failure: "Automatic Failure",
    broken: "Malfunction",
    broken_number: "Malfunction Number",
};

/// Ruby `BCDice::GameSystem::Cthulhu_English`（ID: `Cthulhu:English`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_English;

impl GameSystem for Cthulhu_English {
    fn id(&self) -> &'static str {
        "Cthulhu:English"
    }

    fn name(&self) -> &'static str {
        "Call of Cthulhu"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:English:Call of Cthulhu"
    }

    fn help_message(&self) -> &'static str {
        r#"c=Critical Rate ／ f=Fumble Rate ／ s=Special

1d100<=n    c・f・s AllOff（Does Simple Numeric Comparison Only）

・Roll Command that determines cfs

CC	 Does a 1d100 roll c=1、f=100
CCB  Same as above、c=5、f=96

Ex：CC<=80  （Rolls using 80 as skill value with 1% cf rule applied）
Ex：CCB<=55 （Rolls using 55 as skill value with 5% cf rule applied）

・About Roll Combination

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・About Opposed Rolls
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※Malfunction Number Determination

・CC(x) c=1、f=100
x=Malfunction Number. Outputs（text "Fumble&Malfunction"）together, when roll result is equal or above x, and fumble happens simultaneously.
If not a fumble, outputs text "Malfunction" regardless of success/failure（Outputs the overwritten result, not outputting success/failure）

・CCB(x) c=5、f=96
Same as above
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`en_us` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:en_us`）を見るため
    /// `Success` / `Failure` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&EN_US, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :en_us`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&EN_US, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Cthulhu:English",
            "Cthulhu_English.toml",
            105,
        );
    }
}
