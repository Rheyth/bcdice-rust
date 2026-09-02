//! P4で手書き移植した `lib/bcdice/game_system/WerewolfTheApocalypse5th.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `WerewolfTheApocalypse5th#eval_game_system_specific_command`
//!   （判定コマンド `nWAFx+x` / `nWAIxRx`）と、その下請けの
//!   `get_dice_pools` / `get_roll_result` / `get_critical_success` / `make_dice_roll`

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::WerewolfTheApocalypse5th`（ID: `WerewolfTheApocalypse5th`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WerewolfTheApocalypse5th;

/// Ruby `DIFFICULTY_INDEX`。難易度のキャプチャ番号。
const DIFFICULTY_INDEX: usize = 1;
/// Ruby `DICE_POOL_RAGE_DICE_NO_INCLUDED_INDEX`。WAF のダイスプール。
const DICE_POOL_RAGE_DICE_NO_INCLUDED_INDEX: usize = 5;
/// Ruby `RAGE_DICE_NO_INCLUDED_INDEX`。WAF の Rageダイス。
const RAGE_DICE_NO_INCLUDED_INDEX: usize = 7;
/// Ruby `COMMAND_RAGE_DICE_INCLUDED_INDEX`。内数指定側のコマンド名（`WAI`）。
const COMMAND_RAGE_DICE_INCLUDED_INDEX: usize = 9;
/// Ruby `DICE_POOL_RAGE_DICE_INCLUDED_INDEX`。WAI のダイスプール。
const DICE_POOL_RAGE_DICE_INCLUDED_INDEX: usize = 10;
/// Ruby `RAGE_DICE_INCLUDED_INDEX`。WAI の Rageダイス。
const RAGE_DICE_INCLUDED_INDEX: usize = 12;

/// Ruby `NOT_CHECK_SUCCESS`。判定成功にかかわるチェックを行わない難易度。
const NOT_CHECK_SUCCESS: i64 = -1;

impl GameSystem for WerewolfTheApocalypse5th {
    fn id(&self) -> &'static str {
        "WerewolfTheApocalypse5th"
    }

    fn name(&self) -> &'static str {
        "Werewolf: The Apocalypse 5th Edition"
    }

    fn sort_key(&self) -> &'static str {
        "わあうふるしあほかりふす5"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(nWAFx+x または nWAIxRx)
  WAFコマンドはRageダイスとダイスプールを個別に指定する。
  WAIコマンドはRageダイスをダイスプールの内数として指定する。

    例：難易度2、9ダイスプールでRageダイス3個の場合、それぞれ以下のようなコマンドとなる。
    2WAF6+3
    2WAI9R3

  難易度指定：達成数のカウント、判定成功と失敗、（Rageダイスがある場合）Brutal outcome、クリティカル処理、完全失敗/Total Failure、クリティカル成功のチェックを行う
  例) (難易度)WAF(通常ダイス)+(Rageダイス)
      (難易度)WAF(通常ダイス)
      (難易度)WAI(通常ダイス)R(Rageダイス)
      (難易度)WAI(通常ダイス)

  難易度省略：達成数のカウント、判定失敗、（Rageダイスがある場合）Brutal outcome、クリティカル処理、完全失敗のチェックを行う
              判定成功チェックを行わない
              クリティカル成功/Critical Winのヒントを出力
  例) WAF(通常ダイス)+(Rageダイス)
      WAF(通常ダイス)
      WAI(通常ダイス)R(Rageダイス)
      WAI(通常ダイス)

  難易度0指定：クリティカル処理と達成数のカウントを行い、全てのチェックを行わない
  例) 0WAF(通常ダイス)+(Rageダイス)
      0WAF(通常ダイス)
      0WAI(通常ダイス)+(Rageダイス)
      0WAI(通常ダイス)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*(WAF|(WAI\d*(R\d?)?))"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `WerewolfTheApocalypse5th#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let Some(m) = command_pattern().captures(command) else {
            // Ruby: return '' （`dice_command` が nil に畳む）
            return Ok(Some(SpecificCommandOutput::text("")));
        };

        let (dice_pool, rage_dice_pool) = get_dice_pools(&m);
        if rage_dice_pool > 5 {
            return Ok(Some(SpecificCommandOutput::text(
                "5を超えるRageダイス指定はできません。",
            )));
        }

        // Ruby: `dice_text, success_dice, ten_dice, = make_dice_roll(dice_pool)`
        // 第4要素（1と2の個数）は捨てられる。Brutal outcome は Rageダイス側だけで決まる。
        let normal = make_dice_roll(dice_pool, rng)?;
        let mut success_dice = normal.success_dice;
        let mut ten_dice = normal.ten_dice;

        let result_text = format!("({dice_pool}D10");
        let brutal_outcome;
        let result_text = if rage_dice_pool >= 0 {
            let rage = make_dice_roll(rage_dice_pool, rng)?;

            brutal_outcome = rage.brutal_result_dice / 2;
            ten_dice += rage.ten_dice;
            success_dice += rage.success_dice;

            format!(
                "{result_text}+{rage_dice_pool}D10) ＞ [{}]+[{}] ",
                normal.dice_text, rage.dice_text
            )
        } else {
            // Ruby はここで `rage_ten_dice = 0` も置くが、`get_roll_result` 側が
            // `_rage_ten_dice` として捨てるので移植先では持たない。
            brutal_outcome = 0;
            format!("{result_text}) ＞ [{}] ", normal.dice_text)
        };

        success_dice += get_critical_success(ten_dice);

        let difficulty = m
            .get(DIFFICULTY_INDEX)
            .map_or(NOT_CHECK_SUCCESS, |x| to_i(x.as_str()));

        Ok(Some(get_roll_result(
            result_text,
            success_dice,
            ten_dice,
            brutal_outcome,
            difficulty,
        )))
    }
}

/// Ruby の判定コマンド正規表現。
///
/// Ruby: `/\A(\d+)?(((WAF)(-?\d+)(\+(\d+))?)|((WAI)(-?\d+)(R(\d+))?))$/`
///
/// Rubyの `$` は行末にもマッチするが、`Preprocessor` が最初の空白（改行を含む）より
/// 前しか残さないため、末尾アンカーは `\z` と等価になる。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(\d+)?(((WAF)(-?\d+)(\+(\d+))?)|((WAI)(-?\d+)(R(\d+))?))\z")
            .expect("valid regex")
    })
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(s: &str) -> i64 {
    str_helpers::to_i_signed_saturating(s)
}

/// Ruby `WerewolfTheApocalypse5th#get_dice_pools`。
///
/// 戻り値は `(dice_pool, rage_dice_pool)`。Rageダイス未指定は `-1`。
fn get_dice_pools(m: &Captures<'_>) -> (i64, i64) {
    let rage_dice_included_command = m.get(COMMAND_RAGE_DICE_INCLUDED_INDEX).map(|x| x.as_str());
    if rage_dice_included_command == Some("WAI") {
        // Rage Diceを内数処理するの場合
        let mut rage_dice_pool = m
            .get(RAGE_DICE_INCLUDED_INDEX)
            .map_or(-1, |x| to_i(x.as_str()));
        // Ruby `nil.to_i` は 0
        let mut dice_pool_value = m
            .get(DICE_POOL_RAGE_DICE_INCLUDED_INDEX)
            .map_or(0, |x| to_i(x.as_str()));
        if dice_pool_value <= 0 {
            // ダイスプールが0のとき、最低保証ダイスプールであるダイスプール1にする
            dice_pool_value = 1;
        }
        // Ruby: `dice_pool_value - (rage_dice_pool < 0 ? 0 : rage_dice_pool)`
        let mut dice_pool = dice_pool_value - rage_dice_pool.max(0);
        if dice_pool_value > 0 && rage_dice_pool >= dice_pool_value {
            // 1 以上のダイスプール、かつ、Rageダイスがダイスプール以上のとき、
            // ダイスプールが全てRageダイスになる。
            dice_pool = 0;
            rage_dice_pool = dice_pool_value;
        }
        (dice_pool, rage_dice_pool)
    } else {
        // Rage DiceがPLによる内数指定の場合
        let rage_dice_pool = m
            .get(RAGE_DICE_NO_INCLUDED_INDEX)
            .map_or(-1, |x| to_i(x.as_str()));
        let mut dice_pool = m
            .get(DICE_POOL_RAGE_DICE_NO_INCLUDED_INDEX)
            .map_or(0, |x| to_i(x.as_str()));
        if dice_pool <= 0 && rage_dice_pool <= 0 {
            // ダイスプールとrageダイスどちらも0指定のとき、最低保証ダイスプールである1ダイスプールにする
            dice_pool = 1;
        }
        (dice_pool, rage_dice_pool)
    }
}

/// Ruby `WerewolfTheApocalypse5th#get_roll_result`。
///
/// Ruby の第4引数 `_rage_ten_dice` は使われないので受け取らない。
fn get_roll_result(
    result_text: String,
    success_dice: i64,
    ten_dice: i64,
    brutal_outcome: i64,
    difficulty: i64,
) -> SpecificCommandOutput {
    let is_critical = ten_dice >= 2;

    let mut success_dice = success_dice;
    let mut result_text = if brutal_outcome > 0 && difficulty != 0 {
        success_dice += 4;
        format!("{result_text} [Brutal outcome] 自動失敗、または 達成数={success_dice}")
    } else {
        format!("{result_text} 達成数={success_dice}")
    };

    if difficulty > 0 {
        result_text = format!("{result_text} 難易度={difficulty}");
        if success_dice >= difficulty {
            result_text = format!("{result_text} 上回り={}", success_dice - difficulty);

            let result_data = if is_critical {
                EvalResult::critical(format!(
                    "{result_text}：判定成功! [クリティカル成功/Critical Win]"
                ))
            } else {
                EvalResult::success(format!("{result_text}：判定成功!"))
            };
            // Ruby: `brutal_outcome > 0 ? result_data.text : result_data`
            // Brutal outcome 時は文字列を返すので、成功・クリティカルのフラグが落ちる。
            return if brutal_outcome > 0 {
                SpecificCommandOutput::text(result_data.text)
            } else {
                SpecificCommandOutput::result(result_data)
            };
        }

        return if success_dice == 0 {
            SpecificCommandOutput::result(EvalResult::fumble(format!(
                "{result_text}：判定失敗! [完全失敗/Total Failure]"
            )))
        } else {
            SpecificCommandOutput::result(EvalResult::failure(format!("{result_text}：判定失敗!")))
        };
    } else if difficulty < 0 {
        if success_dice == 0 {
            return SpecificCommandOutput::result(EvalResult::fumble(format!(
                "{result_text}：判定失敗! [完全失敗/Total Failure]"
            )));
        }

        if is_critical {
            result_text = format!("{result_text}\n　判定成功なら [クリティカル成功/Critical Win]");
        }
        return SpecificCommandOutput::text(result_text);
    }

    // 難易度0指定(=全ての判定チェックを行わない)
    SpecificCommandOutput::text(result_text)
}

/// Ruby `WerewolfTheApocalypse5th#get_critical_success`。10の目2個毎に追加2成功。
fn get_critical_success(ten_dice: i64) -> i64 {
    // Ruby `Integer#/` は床除算だが、`ten_dice` は非負なので通常の整数除算と一致する。
    (ten_dice / 2) * 2
}

/// Ruby `WerewolfTheApocalypse5th#make_dice_roll` の戻り値。
struct DiceRoll {
    /// Ruby `dice_text`。出目をカンマ区切りにしたもの。
    dice_text: String,
    /// Ruby `success_dice`。6以上の個数。
    success_dice: i64,
    /// Ruby `ten_dice`。10の個数。
    ten_dice: i64,
    /// Ruby `brutal_result_dice`。1と2の個数の合計。
    brutal_result_dice: i64,
}

/// Ruby `WerewolfTheApocalypse5th#make_dice_roll`。
fn make_dice_roll(dice_pool: i64, rng: &mut Randomizer) -> Result<DiceRoll, EvalError> {
    let dice_list = rng.roll_barabara(dice_pool, 10)?;

    let dice_text = dice_list
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let success_dice = dice_list.iter().filter(|&&x| x >= 6).count() as i64;
    let ten_dice = dice_list.iter().filter(|&&x| x == 10).count() as i64;
    // Ruby: `dice_list.count(1) + dice_list.count(2)`
    let brutal_result_dice = dice_list.iter().filter(|&&x| x == 1 || x == 2).count() as i64;

    Ok(DiceRoll {
        dice_text,
        success_dice,
        ten_dice,
        brutal_result_dice,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "WerewolfTheApocalypse5th",
            "WerewolfTheApocalypse5th.toml",
            200,
            &[
                (1, 2),
                (10, 2),
                (19, 2),
                (26, 2),
                (94, 2),
                (103, 2),
                (112, 2),
                (119, 2),
                (196, 2),
                (197, 6),
                (198, 6),
            ],
        );
    }
}
