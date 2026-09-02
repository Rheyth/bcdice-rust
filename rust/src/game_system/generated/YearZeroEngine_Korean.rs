//! P4で手書き移植した `lib/bcdice/game_system/YearZeroEngine_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `YearZeroEngine` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! コマンド解釈・判定は [`super::YearZeroEngine`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 定型文は `i18n/YearZeroEngine/ko_kr.yml` から書き写したもので、値は1文字も変えていない。

use super::YearZeroEngine::{eval_specific_command, SystemStrings};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::YearZeroEngine_Korean`（ID: `YearZeroEngine:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearZeroEngine_Korean;

impl GameSystem for YearZeroEngine_Korean {
    fn id(&self) -> &'static str {
        "YearZeroEngine:Korean"
    }

    fn name(&self) -> &'static str {
        "이어 제로 엔진(Year Zero Engine)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:이어 제로 엔진(Year Zero Engine)"
    }

    fn help_message(&self) -> &'static str {
        r"・다이스 풀 판정 커맨드(nYZEx+x+x+m)
  (난이도)YZE(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)+(수정치)  # (6만 셈함)
  (난이도)YZE(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)-(수정치)  # (6만 셈함)

・다이스 풀 판정 커맨드(nMYZx+x+x)
  (난이도)MYZ(능력 주사위 수)+(기능 주사위 수)+(아이템 주사위 수)  # (1과 6을 세어 푸시 가능 수 표시)
  (난이도)MYZ(능력 주사위 수)-(기능 주사위 수)+(아이템 주사위 수)  # (1과 6을 세어 푸시 가능 수 표시, 기능 마이너스 지정)

  ※ 난이도, 기능 주사위 수, 아이템 주사위 수는 생략 가능

・스텝 다이스 판정 커맨드(nYZSx+x+m+f)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)   # (1, 6을 세어 푸시 가능 수 표시)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)   # (1, 6을 세어 푸시 가능 수 표시)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)A  # (1, 6을 세어 푸시 가능 수 표시, 유리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)A  # (1, 6을 세어 푸시 가능 수 표시, 유리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)+(수정치)D  # (1, 6을 세어 푸시 가능 수 표시, 불리)
  (난이도)YZS(능력 주사위 면 수)+(기능 주사위 면 수)-(수정치)D  # (1, 6을 세어 푸시 가능 수 표시, 불리)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?(YZE|MYZ|YZS)"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_STRINGS, command, rng)
    }
}

/// `ko_kr` ロケールの定型文（`i18n/YearZeroEngine/ko_kr.yml`）。
static KO_STRINGS: SystemStrings = SystemStrings {
    success_count: "성공 수",
    difficulty: "난이도",
    success_msg: "판정 성공!",
    failure_msg: "판정 실패!",
    roll_one: "주사위 눈 1",
    ability: "능력",
    skill: "기능",
    item: "아이템",
    pushable: "푸시 가능",
    dice: "주사위",
};

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "YearZeroEngine:Korean",
            "YearZeroEngine_Korean.toml",
            90,
        );
    }
}
