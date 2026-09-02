//! P4で手書き移植した `lib/bcdice/game_system/ShadowRun5.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ShadowRun5#eval_game_system_specific_command`（リミット付きバラバラロール `xB6@l`）
//! - `ShadowRun5#grich_text`（グリッチ判定）
//!
//! 設定値（`@sort_add_dice` / `@sort_barabara_dice` / `@reroll_dice_reroll_threshold` /
//! `@default_cmp_op` / `@default_target_number`）は親クラス `ShadowRun4#initialize` と
//! 同じ値を `ShadowRun5#initialize` が再代入しているもので、スタブが持っている値と一致する。
//! グリッチ判定だけは親の `>=` に対して `>` に変わっているので、ここで上書きする。

use std::sync::OnceLock;

use regex::Regex;

use crate::common_command::barabara_dice;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::ShadowRun5`（ID: `ShadowRun5`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowRun5;

impl GameSystem for ShadowRun5 {
    fn id(&self) -> &'static str {
        "ShadowRun5"
    }

    fn name(&self) -> &'static str {
        "シャドウラン 5th Edition"
    }

    fn sort_key(&self) -> &'static str {
        "しやとうらん5"
    }

    fn help_message(&self) -> &'static str {
        r"個数振り足しロール(xRn)の境界値を6にセット、バラバラロール(xBn)の目標値を5以上にセットします。
バラバラロール(xBn)のみ、リミットをセットできます。リミットの指定は(xBn@l)のように指定します。(省略可)
BコマンドとRコマンド時に、グリッチの表示を行います。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)B6@(\d+)"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@reroll_dice_reroll_threshold = 6`。
    fn reroll_dice_reroll_threshold(&self) -> Option<i64> {
        Some(6)
    }

    /// Ruby `initialize` の `@default_cmp_op = :>=`。
    fn default_cmp_op(&self) -> Option<CmpOp> {
        Some(CmpOp::Ge)
    }

    /// Ruby `initialize` の `@default_target_number = 5`。
    fn default_target_number(&self) -> Option<i64> {
        Some(5)
    }

    /// Ruby `ShadowRun5#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_limited_roll(self, command, rng)
    }

    /// Ruby `ShadowRun5#grich_text`。
    ///
    /// 親クラス `ShadowRun4#grich_text` は `>=` で判定するが、5thは `>`。
    fn grich_text(
        &self,
        count_one: usize,
        dice_total_count: usize,
        count_success: i64,
    ) -> Option<String> {
        // Ruby: dice_cnt_total_half = dice_cnt_total.to_f / 2
        let dice_cnt_total_half = dice_total_count as f64 / 2.0;

        // Ruby: unless numberSpot1 > dice_cnt_total_half -> nil
        // 両辺とも有限値なので `!(a > b)` は `a <= b` と等しい。
        if (count_one as f64) <= dice_cnt_total_half {
            return None;
        }

        // グリッチ！
        if count_success == 0 {
            Some("クリティカルグリッチ".to_owned())
        } else {
            Some("グリッチ".to_owned())
        }
    }
}

/// Ruby `/(\d+B6)@(\d+)/`。
fn limit_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+B6)@(\d+)").expect("valid regex"))
}

/// Ruby `/成功数(\d+)/`。
fn success_count_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"成功数(\d+)").expect("valid regex"))
}

/// Ruby `ShadowRun5#eval_game_system_specific_command`。
///
/// `xB6@l` を「リミット `l` 付きのバラバラロール」として評価する。
/// 本体のロールは共通コマンド `BarabaraDice` へ丸投げし、その出力文字列を
/// 置換して整える（原典どおり）。
fn eval_limited_roll(
    game_system: &dyn GameSystem,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // 接頭辞 `^(S)?((\d+)B6@(\d+))` を通ってきた入力なので必ずマッチする。
    // Ruby側はマッチしなければ `m[1]` で NoMethodError になる経路。
    let Some(captures) = limit_pattern().captures(command) else {
        return Err(EvalError::Internal("ShadowRun5: command has no xB6@l"));
    };

    let b_dice = &captures[1];
    // Ruby: m[2].to_i（多倍長）。i64に収まらない指定は飽和させる
    // （成功数は振ったダイス数以下なので、飽和しても「超過なし」の枝は変わらない）。
    let limit: i64 = captures[2].parse().unwrap_or(i64::MAX);

    // Ruby: BarabaraDice.eval(b_dice, self, @randomizer).text
    // `self` を渡すので sort_barabara_dice / default_cmp_op / default_target_number /
    // grich_text がこのゲームシステムのものになる。
    let Some(before) = barabara_dice::eval(b_dice, game_system, rng)? else {
        return Err(EvalError::Internal(
            "ShadowRun5: xB6 is not a barabara roll",
        ));
    };
    let output_before_limited = before.text;

    // Ruby: /成功数(\d+)/.match(...) の m[1]。
    // `@default_cmp_op` が常に `:>=` なので成功数は必ず出力に含まれる。
    let Some(m) = success_count_pattern().captures(&output_before_limited) else {
        return Err(EvalError::Internal("ShadowRun5: no 成功数 in roll output"));
    };
    let before_suc_cnt: i64 = m[1].parse().unwrap_or(i64::MAX);

    let mut output = if before_suc_cnt > limit {
        let after_suc_cnt = limit;
        let over_suc_cnt = before_suc_cnt - limit;
        let replaced = success_count_pattern()
            .replace_all(&output_before_limited, format!("成功数{after_suc_cnt}"))
            .into_owned();
        format!("{replaced}(リミット超過{over_suc_cnt})")
    } else {
        output_before_limited
    };

    // Ruby: output.gsub('B', 'B6') → gsub('6>=5', "[6]Limit[#{limit}]>=5")
    // 2段階の単純置換。1段目で `(12B6>=5)` が `(12B66>=5)` になり、
    // 2段目でその `6>=5` が `[6]Limit[l]>=5` に置き換わる。
    output = output.replace('B', "B6");
    output = output.replace("6>=5", &format!("[6]Limit[{limit}]>=5"));

    Ok(Some(SpecificCommandOutput::text(output)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ShadowRun5",
            "ShadowRun5.toml",
            46,
        );
    }
}
