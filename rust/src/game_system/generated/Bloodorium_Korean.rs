//! P4で手書き移植した `lib/bcdice/game_system/Bloodorium_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Bloodorium` を継承し `@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Bloodorium`] のものをそのまま使い、ここには
//! `ko_kr` ロケールの文言だけを置く。
//!
//! 文言は `i18n/Bloodorium/ko_kr.yml` と `i18n/ko_kr.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use super::Bloodorium::dicecheck;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `Bloodorium.triumph`（`i18n/Bloodorium/ko_kr.yml`）。
const TRIUMPH_KO_KR: &str = "《트라이엄프》(*%{triumph})";

/// i18n `success`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の成功文言。
const GLOBAL_SUCCESS: &str = "성공";
/// i18n `failure`（`i18n/ko_kr.yml`）。`Base#result_ndx` が使う汎用の失敗文言。
const GLOBAL_FAILURE: &str = "실패";

/// Ruby `BCDice::GameSystem::Bloodorium_Korean`（ID: `Bloodorium:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloodorium_Korean;

impl GameSystem for Bloodorium_Korean {
    fn id(&self) -> &'static str {
        "Bloodorium:Korean"
    }

    fn name(&self) -> &'static str {
        "블러도리움"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:블러도리움"
    }

    fn help_message(&self) -> &'static str {
        r"・주사위 체크 xDC+y
　【주사위 체크】를 실행한다.《트라이엄프》를 결과에 자동 반영한다.
　x: 주사위 수
　y: 결과에 대한 수정값 (생략 가능)
"
    }

    /// Ruby `register_prefix_from_super_class()`（`Bloodorium` と同じ接頭辞）。
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@locale = :ko_kr` により `Base#result_ndx` の `translate` が
    /// `i18n/ko_kr.yml` を引くようになる分。
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

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(dicecheck(command, TRIUMPH_KO_KR, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/Bloodorium_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Bloodorium:Korean",
            "Bloodorium_Korean.toml",
            10,
        );
    }

    /// `@locale = :ko_kr` により汎用コマンドの成否文言も韓国語になること。
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
            let result = eval_command(&GameSystemId::new("Bloodorium:Korean"), input, &mut src)
                .expect("eval")
                .expect("some output");
            assert_eq!(result.text, expected, "input: {input}");
        }
    }
}
