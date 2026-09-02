//! P4で手書き移植した `lib/bcdice/game_system/Revulture_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Revulture` を継承し、`@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::Revulture`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 文言は `i18n/Revulture/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure`）から
//! 写したもので、値は1文字も変えていない。

use super::Revulture::{eval_specific_command, SystemTables};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `ko_kr.success`。`Revulture` の [`SystemTables`] は判定コマンド用の文言しか
/// 持たないので、`Base#result_ndx` が使う分だけここに置く。
const KO_SUCCESS: &str = "성공";
/// i18n `ko_kr.failure`。
const KO_FAILURE: &str = "실패";

/// `ko_kr` ロケールの定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    no_dice: "주사위가 0개 입니다.",
    no_border: "목표값이 지정되지 않아 추가 대미지를 계산할 수 없습니다.",
    critical: "크리티컬 %<count>d",
    hit_count: "히트 수 %<count>d",
    damage: "대미지 %<count>d",
};

/// Ruby `BCDice::GameSystem::Revulture_Korean`（ID: `Revulture:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revulture_Korean;

impl GameSystem for Revulture_Korean {
    fn id(&self) -> &'static str {
        "Revulture:Korean"
    }

    fn name(&self) -> &'static str {
        "광쇄의 리벌처"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:광쇄의 리벌처"
    }

    fn help_message(&self) -> &'static str {
        r"■공격 판정（ xAT, xATK, xATTACK ）
x: 주사위 수（덧셈 + 과 나눗셈 / 사용 가능）
예시） 3AT, 4ATK, 5+6ATTACK, 15/2AT

□공격 판정 목표값 포함（ xAT<=y, xATK<=y, xATTACK<=y ）
x: 주사위 수（덧셈 + 과 나눗셈 / 사용 가능）
y: 목표값（ 1 이상 6 이하. 덧셈 + 사용 가능）
예시） 3AT<=4, 3AT<=2+1

□공격 판정　목표값＆추가 대미지 포함（ xAT<=y[>=a:+b], xATK<=y[>=a:+b], xATTACK<=y[z] ）
x: 주사위 수（덧셈 + 과 나눗셈 / 사용 가능）
y: 목표값（ 1 이상 6 이하. 덧셈 + 사용 가능）
z: 추가 대미지 규칙（자세한 내용은 아래 참고）（※여러 개를 동시에 사용 가능）

▽추가 대미지 규칙 [a:+b]
a: 히트 수가 a 라면
　=a　（히트 수가 a와 동일）
　>=a　（히트 수가 a 이상）
b: 대미지를 b 점 추가

예시） 3AT<=4[>=2:+3] #룰 북 p056「그레인그랜트 AR(グレングラントAR)」
예시） 2AT<=4[=1:+5][>=2:+8] #룰 북 p067「파보르 드래곤 브레스(ファーボル・ドラゴンブレス)」
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+([+\/]\d+)*AT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞（`xAT`）に一致しない加算ダイス判定がこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(KO_SUCCESS))
        } else {
            Some(EvalResult::failure(KO_FAILURE))
        }
    }

    /// Ruby `Revulture#eval_game_system_specific_command`。
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

    /// `test/data/Revulture_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Revulture:Korean",
            "Revulture_Korean.toml",
            38,
        );
    }

    /// アタック判定を通らない加算ダイス判定が `ko_kr` の定型文になること。
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
            let result = eval_command(&GameSystemId::new("Revulture:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
