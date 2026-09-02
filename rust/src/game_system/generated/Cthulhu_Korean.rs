//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Cthulhu` を継承し、`register_prefix_from_super_class` で接頭辞を引き継いで
//! `@locale` を `:ko_kr` に変えるだけ（判定メソッドの上書きは無い）なので、
//! 実装は [`super::Cthulhu`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Cthulhu/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Cthulhu::{eval_specific_command, result_ndx_localized, Locale};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `ko_kr` ロケールの文言一式。
static KO_KR: Locale = Locale {
    success: "성공",
    failure: "실패",
    critical: "크리티컬",
    special: "스페셜",
    critical_special: "크리티컬/스페셜",
    fumble: "펌블",
    partial_success: "부분적 성공",
    automatic_success: "자동성공",
    automatic_failure: "자동실패",
    broken: "고장",
    broken_number: "고장 수치",
};

/// Ruby `BCDice::GameSystem::Cthulhu_Korean`（ID: `Cthulhu:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu_Korean;

impl GameSystem for Cthulhu_Korean {
    fn id(&self) -> &'static str {
        "Cthulhu:Korean"
    }

    fn name(&self) -> &'static str {
        "크툴루"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:크툴루"
    }

    fn help_message(&self) -> &'static str {
        r"c=크리티컬치 ／ f=펌블치 ／ s=스페셜

1d100<=n    c・f・s 모두 오프（단순하게 수치만을 뽑아낼 때 사용）

・cfs이 붙는 판정의 커맨드

CC	 1d100 판정을 행함 c=1、f=100
CCB  위와 동일、c=5、f=96

예：CC<=80  （기능치 80로 행휘판정. 1%룰으로 cf적용）
예：CCB<=55 （기능치 55로 행휘판정. 5%룰으로 cf적용）

・경우의 수 판정에 대해서

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・저항 판정에 대해서
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※고장 넘버 판정

・CC(x) c=1、f=100
x=고장 넘버. 주사위 눈x이상이 나온 후에, 펌블이 동시에 발생했을 경우. 모두 출력한다. （텍스트 「펌블＆고장」）
펌블이 아닌 경우, 성공・실패에 관련되지 않고 「고장」만을 출력한다. （성공・실패를 출력하지 않고 덧쓰기한 것을 출력하는 형태）

・CCB(x) c=5、f=96
위와 동일
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞に一致しない `1D100<=70` などがこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&KO_KR, total, cmp_op, target)
    }

    /// Ruby `Cthulhu#eval_game_system_specific_command`（`@locale = :ko_kr`）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_KR, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Cthulhu:Korean",
            "Cthulhu_Korean.toml",
            100,
        );
    }
}
