//! P4で手書き移植した `lib/bcdice/game_system/ChroniclesOfDarkness2e.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`CDx@cWdAs`）と `#get_arguments`
//! - `#make_dice_roll` / `#roll_dice_pool`（振り足し・チャンスロール）
//! - `#get_roll_result`（成功数・自動成功・武器修正・Exceptional Success の判定）
//!
//! # 一致しないコマンドは空文字列
//!
//! Ruby の `eval_game_system_specific_command` は正規表現に一致しないとき `nil` ではなく
//! `''` を返す。`Base#dice_command` が `output.empty?` を `nil` に畳むので結果は同じ。
//! ここでも [`SpecificCommandOutput::text`] に空文字列を返して原典の形をそのまま保つ。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `EXCEPTIONAL_SUCCESS_THRESHOLD`。Exceptional Successの閾値。
const EXCEPTIONAL_SUCCESS_THRESHOLD: i64 = 5;

/// Ruby `/\A(CD)(-?\d+)(@([8-9]|10))?(W(\d+))?(A(\d+))?$/`。
///
/// Ruby の `\d` はASCII数字だけを指す。`regex` クレートの `\d` は既定でUnicode対応
/// （全角数字などまで拾う）なので、明示的に `[0-9]` と書く。
fn cd_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(CD)(-?[0-9]+)(@([8-9]|10))?(W([0-9]+))?(A([0-9]+))?$").expect("valid regex")
    })
}

/// `\d+` にマッチした部分を `i64` にする。Ruby の `String#to_i` は多倍長なので
/// 桁あふれしないが、Rustでは飽和させる（実用上到達しない経路）。
fn to_i(s: &str) -> i64 {
    s.parse::<i64>().unwrap_or(if s.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = cd_pattern().captures(command) else {
        // Ruby: return ''（`dice_command` が nil に畳む）
        return Ok(Some(SpecificCommandOutput::text("")));
    };

    // 引数分解（Ruby `#get_arguments`）
    let dice_pool = to_i(&m[2]);
    let again_number = m.get(4).map_or(10, |g| to_i(g.as_str()));
    let weapon_modifier = m.get(6).map_or(0, |g| to_i(g.as_str()));
    let auto_success = m.get(8).map_or(0, |g| to_i(g.as_str()));

    // ダイスロール
    let roll = make_dice_roll(dice_pool, again_number, rng)?;

    // 結果判定
    Ok(Some(SpecificCommandOutput::result(get_roll_result(
        dice_pool,
        &roll,
        weapon_modifier,
        auto_success,
    ))))
}

/// [`roll_dice_pool`] の戻り値。Ruby の `success_number, dice_text, dramatic_failure`。
struct DicePoolRoll {
    success_number: i64,
    dice_text: String,
    dramatic_failure: bool,
}

/// Ruby `#make_dice_roll`。ダイスプールが0以下ならチャンスロール。
fn make_dice_roll(
    dice_pool: i64,
    again_number: i64,
    rng: &mut Randomizer,
) -> Result<DicePoolRoll, EvalError> {
    if dice_pool <= 0 {
        roll_dice_pool(1, again_number, true, rng)
    } else {
        roll_dice_pool(dice_pool, again_number, false, rng)
    }
}

/// Ruby `#roll_dice_pool`。
///
/// チャンスロールでは `again_dice` が0のままなので振り足しは起きない
/// （原典コメント: "The chance die only counts as a success if you roll a 10,
/// which you do not reroll."）。
fn roll_dice_pool(
    mut dice_pool: i64,
    again_number: i64,
    chance_roll: bool,
    rng: &mut Randomizer,
) -> Result<DicePoolRoll, EvalError> {
    let mut success_number: i64 = 0;
    let mut again_dice: i64 = 0;
    let mut dice_text = String::new();

    loop {
        let dice_list = rng.roll_barabara(dice_pool, 10)?;
        let joined = dice_list
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        dice_text.push_str(&format!("[{joined}] "));

        if chance_roll {
            if dice_list.contains(&1) {
                // Dramatic Failure発生
                return Ok(DicePoolRoll {
                    success_number: 0,
                    dice_text,
                    dramatic_failure: true,
                });
            } else if dice_list.contains(&10) {
                // チャンスダイスは10のみ成功
                success_number = 1;
            } else {
                success_number = 0;
            }
        } else {
            success_number += dice_list.iter().filter(|x| **x >= 8).count() as i64;
            again_dice = dice_list.iter().filter(|x| **x >= again_number).count() as i64;
        }

        if again_dice > 0 {
            // 振り足しが存在するなら再判定
            dice_pool = again_dice;
        } else {
            break;
        }
    }

    Ok(DicePoolRoll {
        success_number,
        dice_text,
        dramatic_failure: false,
    })
}

/// Ruby `#get_roll_result`。
fn get_roll_result(
    dice_pool: i64,
    roll: &DicePoolRoll,
    weapon_modifier: i64,
    auto_success: i64,
) -> EvalResult {
    let mut result_text = if dice_pool <= 0 {
        "Chance Roll(1D10) ＞ ".to_string()
    } else {
        format!("({dice_pool}D10) ＞ ")
    };
    result_text.push_str(&roll.dice_text);
    result_text.push_str(&format!("success={} ", roll.success_number));

    let total_success = roll.success_number.saturating_add(auto_success);

    // CofD2e rulebook p21 より、チャンスダイスで "1" が出ていたら
    // 自動成功の有無に関わらず Dramatic Failure。
    if roll.dramatic_failure {
        return EvalResult::fumble(format!("{result_text}Dramatic Failure!"));
    }

    if total_success > 0 {
        if auto_success > 0 {
            // 成功数の合算が発生するときだけトータル成功数を表示
            result_text.push_str(&format!(
                "auto_success={auto_success} total_success={total_success} "
            ));
        }
        if weapon_modifier != 0 {
            // 成功数が1でもあれば、武器修正とダメージを表示
            let damage = total_success.saturating_add(weapon_modifier);
            result_text.push_str(&format!(
                "weapon_modifier={weapon_modifier} damage={damage} "
            ));
        }
        if total_success >= EXCEPTIONAL_SUCCESS_THRESHOLD {
            // 5成功以上は Exceptional Success
            EvalResult::critical(format!("{result_text}Exceptional Success!"))
        } else {
            EvalResult::success(format!("{result_text}Success!"))
        }
    } else {
        EvalResult::failure(format!("{result_text}Failure!"))
    }
}

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::ChroniclesOfDarkness2e`（ID: `ChroniclesOfDarkness2e`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChroniclesOfDarkness2e;

impl GameSystem for ChroniclesOfDarkness2e {
    fn id(&self) -> &'static str {
        "ChroniclesOfDarkness2e"
    }

    fn name(&self) -> &'static str {
        "Chronicles of Darkness 2nd Edtion"
    }

    fn sort_key(&self) -> &'static str {
        "くろにくるすおふたあくねす2"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(CDx@cWdAs)
    x:ダイスプール(0以下でChance Roll)
    c:振り足し値(省略可、省略時は10)。8-10の値を取る。Chance Rollには適用されない。
    d:武器ダメージ修正(省略可)。判定による成功数が1以上のときにダメージとして修正値を加算。
    s:自動成功数(省略可)。成功数に加算される。

    例1：6ダイスプール、10 again
    CD6
    CD6@10

    例2：9ダイスプール、9の振り足し(9 again)、自動成功1
    CD9@9A1

    例3：0ダイスプール(Chance Roll)、8の振り足し(8 again, 適用されない)、自動成功1、武器修正+2
    CD0@8W2A1

    例4：-1ダイスプール(Chance Roll)
    CD(4-5)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(CD\d*)"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ChroniclesOfDarkness2e#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/ChroniclesOfDarkness2e.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ChroniclesOfDarkness2e",
            "ChroniclesOfDarkness2e.toml",
            43,
        );
    }

    /// TOMLに無い経路の固定。
    ///
    /// - 接頭辞 `(CD\d*)` には一致するが判定コマンドの書式に合わない入力は `nil`
    ///   （Ruby の `return ''` → `Base#dice_command` が畳む）
    /// - 振り足し値は `8`〜`10` のみ受け付ける
    /// - チャンスロールは `@` 指定があっても振り足さない
    #[test]
    fn empty_output_and_again_bounds() {
        for command in ["CD", "CD6@7", "CD6@11", "CD6X1", "CD6W3A"] {
            let mut src = SeededRandomizer::new(vec![]);
            assert!(
                eval_command(
                    &GameSystemId::new("ChroniclesOfDarkness2e"),
                    command,
                    &mut src
                )
                .expect("must not error")
                .is_none(),
                "{command} must be nil"
            );
        }

        // 8-again でも 10 が出たチャンスダイスは振り足さない。
        let mut src = SeededRandomizer::new(vec![(10, 10)]);
        let result = eval_command(
            &GameSystemId::new("ChroniclesOfDarkness2e"),
            "CD0@8W2A1",
            &mut src,
        )
        .expect("CD0@8W2A1 must not error")
        .expect("CD0@8W2A1 must produce output");
        assert_eq!(
            result.text,
            "Chance Roll(1D10) ＞ [10] success=1 auto_success=1 total_success=2 \
             weapon_modifier=2 damage=4 Success!"
        );
    }
}
