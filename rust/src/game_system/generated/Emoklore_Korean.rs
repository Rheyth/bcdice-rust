//! P4で手書き移植した `lib/bcdice/game_system/Emoklore_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Emoklore` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Emoklore`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 文言は `i18n/Emoklore/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure` /
//! `fumble`）から写したもので、値は1文字も変えていない
//! （`success_count` の末尾のカンマもロケール側の定義どおり）。

use super::Emoklore::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `ko_kr` ロケールの定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    success_count: "성공 수 %{count},",
    double: "더블",
    triple: "트리플",
    miracle: "미라클",
    catastrophe: "카타스트로프",
    dice_count_zero: "주사위 개수가 0 이하 ＞ 확정 실패",
    fumble: "펌블",
    failure: "실패",
    success: "성공",
};

/// Ruby `BCDice::GameSystem::Emoklore_Korean`（ID: `Emoklore:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emoklore_Korean;

impl GameSystem for Emoklore_Korean {
    fn id(&self) -> &'static str {
        "Emoklore:Korean"
    }

    fn name(&self) -> &'static str {
        "에모크로아TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:에모크로아TRPG"
    }

    fn help_message(&self) -> &'static str {
        r#"・기능치 판정（xDM<=y / xDM<=yEz）
  "(개수)DM<=(판정치)"로 판정합니다.
  주사위의 개수는 생략 가능하며, 생략 시 1개로 설정됩니다.
  주사위 개수와 판정치에는 사칙연산（+-*/）을 사용할 수 있습니다.
  수식 끝에 Ez를 붙이면 주사위 수에 z를 더합니다. E-z로 빼기도 가능합니다.
  ex）2DM<=5　DM<=8　2DM<=3+2
      2+2DM<=5 → 주사위 4개로 판정치 5
      2DM<=5E2 → 주사위 2+2 = 주사위 4개로 판정치 5
      3DM<=5E-1 → 주사위 3-1 = 주사위 2개로 판정치 5
    ※주사위 수가 0 이하가 되는 경우 확정 실패

・기능치 판정（sDAa+z)
  "(기능 레벨)DA(능력치)+(주사위 보너스)"로 판정합니다.
  주사위 보너스의 개수는 생략 가능하며, 생략 시 0개로 설정됩니다.
  기능 레벨에는 1~3의 수치를 입력합니다. 기본 기능으로 판정하려면 기능 레벨에"b"를 입력하세요.
  주사위 개수는 기능 레벨과 주사위 보너스 개수에 따라 결정되며, s+z개의 주사위를 굴립니다. (s="b"인 경우 s=1)
  판정치는 s+a 입니다.（s="b"인 경우에는 s=0）
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[-+*/\d]*DM<=", r"(B|\d*)DA"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// `1D10>=5` のような汎用の加算ダイス判定がこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(KO_SYSTEM.success))
        } else {
            Some(EvalResult::failure(KO_SYSTEM.failure))
        }
    }

    /// Ruby `Emoklore#eval_game_system_specific_command`。
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
            "Emoklore:Korean",
            "Emoklore_Korean.toml",
            33,
        );
    }
}
