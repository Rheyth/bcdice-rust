//! P4で手書き移植した `lib/bcdice/game_system/FullMetalPanic_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `FullMetalPanic` を継承し、`set_aliases_for_srs_roll('MG', 'FP')` を
//! 再宣言したうえで `@locale` を `:ko_kr` に変えるだけなので、
//! 判定の実装は [`super::FullMetalPanic`] のものをそのまま使い、
//! ここには `ko_kr` ロケールの定型文だけを置く。
//!
//! 文言は `i18n/SRS/ko_kr.yml`（`auto_success` / `auto_failure`）と
//! `i18n/ko_kr.yml`（`success` / `failure`）から写したもので、値は1文字も変えていない。

use super::FullMetalPanic::{eval_specific_command, SystemTables};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// `ko_kr` ロケールの設定と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    notations: &["2D6", "MG", "FP"],
    auto_success: "자동 성공",
    auto_failure: "자동 실패",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::FullMetalPanic_Korean`（ID: `FullMetalPanic:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullMetalPanic_Korean;

impl GameSystem for FullMetalPanic_Korean {
    fn id(&self) -> &'static str {
        "FullMetalPanic:Korean"
    }

    fn name(&self) -> &'static str {
        "풀 메탈 패닉! RPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:풀 메탈 패닉! RPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　・通常判定：2D6+m@c#f>=t または 2D6+m>=t[c,f]
　　修正値m、目標値t、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]ごと省略可、@c・#fの指定は順不同）。
　　クリティカル値、ファンブル値の既定値は、それぞれ12、2です。
　　自動成功、自動失敗、成功、失敗を自動表示します。

　　例) 2d6>=10　　　　　修正値0、目標値10で判定
　　例) 2d6+2>=10　　　　修正値+2、目標値10で判定
　　例) 2d6+2>=10[11]　　↑をクリティカル値11で判定
　　例) 2d6+2@11>=10 　　↑をクリティカル値11で判定
　　例) 2d6+2>=10[12,4]　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4>=10 　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2>=10[,4]　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) 2d6+2#4>=10　　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) MG+2>=10　　　　 2d6+2>=10と同じ（MGが2D6のショートカットコマンド）
　　例) FP+2>=10　　　　 2d6+2>=10と同じ（FPが2D6のショートカットコマンド）

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) MG　　　　　 2d6[]と同じ（MGが2D6のショートカットコマンド）
　　例) MG+2@12#4　　2d6+2@12#4と同じ（MGが2D6のショートカットコマンド）
　　例) FP　　　　　 2d6[]と同じ（FPが2D6のショートカットコマンド）
　　例) FP+2@12#4　　2d6+2@12#4と同じ（FPが2D6のショートカットコマンド）

・D66ダイスあり（入れ替えなし)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6", "MG", "FP"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SRS#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `SRS#initialize` の `@d66_sort_type = D66SortType::NO_SORT`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 接頭辞（`2D6` / `MG` / `FP`）に一致しない加算ダイス判定がこの経路を通る。
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

    /// Ruby `SRS#eval_game_system_specific_command`。
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

    /// `test/data/FullMetalPanic_Korean.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "FullMetalPanic:Korean",
            "FullMetalPanic_Korean.toml",
            37,
        );
    }

    /// SRS判定を通らない加算ダイス判定が `ko_kr` の定型文になること。
    ///
    /// Ruby は `Base#result_ndx` の `translate("success")` が `@locale = :ko_kr` を見るため
    /// `성공` / `실패` になる。TOMLにこの経路のケースが無いのでここで固定する。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            // 接頭辞（2D6 / MG / FP）に一致しないので SRS 判定を通らない
            (
                "3D6>=10",
                vec![(4, 6), (5, 6), (6, 6)],
                "(3D6>=10) ＞ 15[4,5,6] ＞ 15 ＞ 성공",
            ),
            (
                "3D6>=16",
                vec![(4, 6), (5, 6), (6, 6)],
                "(3D6>=16) ＞ 15[4,5,6] ＞ 15 ＞ 실패",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("FullMetalPanic:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
