//! P4で手書き移植した `lib/bcdice/game_system/HunterTheReckoning5th.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `HunterTheReckoning5th#eval_game_system_specific_command`（`nHRFx+x` 判定）
//! - `#get_roll_result` / `#get_critical_success` / `#make_dice_roll`

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `HunterTheReckoning5th::DIFFICULTY_INDEX`。
const DIFFICULTY_INDEX: usize = 1;
/// Ruby `HunterTheReckoning5th::DICE_POOL_INDEX`。
const DICE_POOL_INDEX: usize = 3;
/// Ruby `HunterTheReckoning5th::DESPERATION_DICE_INDEX`。
const DESPERATION_DICE_INDEX: usize = 5;

/// Ruby `HunterTheReckoning5th::NOT_CHECK_SUCCESS`。
/// 判定成功にかかわるチェックを行わない（判定失敗に関わるチェックは行う）。
const NOT_CHECK_SUCCESS: i64 = -1;

/// Ruby `BCDice::GameSystem::HunterTheReckoning5th`（ID: `HunterTheReckoning5th`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HunterTheReckoning5th;

impl GameSystem for HunterTheReckoning5th {
    fn id(&self) -> &'static str {
        "HunterTheReckoning5th"
    }

    fn name(&self) -> &'static str {
        "Hunter: The Reckoning 5th Edition"
    }

    fn sort_key(&self) -> &'static str {
        "はんあたされこにんく5"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(nHRFx+x)
  注意：難易度は必要達成数を表す

  難易度指定：達成数のカウント、判定成功と失敗、クリティカル処理、クリティカル成功、完全失敗のチェックを行う
             （Desperationダイスがある場合）OverreachとDespairの発生チェックを行う
  例) (難易度)HRF(通常ダイス)+(Desperationダイス)
      (難易度)HRF(通常ダイス)

  難易度省略：達成数のカウント、判定失敗、クリティカル処理、完全失敗、（Desperationダイスがある場合）Despairチェックを行う
              判定成功、Overreachのチェックを行わない
              クリティカル成功、（Desperationダイスがある場合）Despair、Overreachのヒントを出力
  例) HRF(通常ダイス)+(Desperationダイス)
      HRF(通常ダイス)

  難易度0指定：全てのチェックを行わない
  例) 0HRF(通常ダイス)+(Desperationダイス)
      0HRF(通常ダイス)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*HRF"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `HunterTheReckoning5th#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: /\A(\d+)?(HRF)(-?\d+)(\+(\d+))?$/
        let re = Regex::new(r"\A(\d+)?(HRF)(-?\d+)(\+(\d+))?\z").expect("valid regex");
        let Some(m) = re.captures(command) else {
            // Ruby: return ''
            return Ok(Some(SpecificCommandOutput::text(String::new())));
        };

        // Ruby: m[DICE_POOL_INDEX].to_i（正規表現で `\d+` が必ずマッチする）
        let mut dice_pool: i64 = m[DICE_POOL_INDEX].parse().unwrap_or(i64::MAX);
        if dice_pool <= 0 {
            // ダイスプールが0以下のとき、最低保証ダイスプールであるダイスプール1にする
            dice_pool = 1;
        }
        let (dice_text, mut success_dice, mut ten_dice, _botch) = make_dice_roll(dice_pool, rng)?;
        let mut result_text = format!("({dice_pool}D10");

        let desperaton_dice_pool: Option<i64> = m
            .get(DESPERATION_DICE_INDEX)
            .map(|x| x.as_str().parse().unwrap_or(i64::MAX));
        let (desperaton_ten_dice, desperaton_botch_dice);
        match desperaton_dice_pool {
            Some(pool) => {
                if pool > 5 {
                    return Ok(Some(SpecificCommandOutput::text(
                        "Desperationダイス指定は5ダイスが最大です。",
                    )));
                }

                let (desperaton_dice_text, desperaton_success_dice, d_ten, d_botch) =
                    make_dice_roll(pool, rng)?;

                ten_dice += d_ten;
                success_dice += desperaton_success_dice;
                desperaton_ten_dice = d_ten;
                desperaton_botch_dice = d_botch;

                result_text =
                    format!("{result_text}+{pool}D10) ＞ [{dice_text}]+[{desperaton_dice_text}] ");
            }
            None => {
                desperaton_ten_dice = 0;
                desperaton_botch_dice = 0;
                result_text = format!("{result_text}) ＞ [{dice_text}] ");
            }
        }

        success_dice += get_critical_success(ten_dice);

        let difficulty: i64 = m
            .get(DIFFICULTY_INDEX)
            .map(|x| x.as_str().parse().unwrap_or(i64::MAX))
            .unwrap_or(NOT_CHECK_SUCCESS);

        get_roll_result(
            result_text,
            success_dice,
            ten_dice,
            desperaton_ten_dice,
            desperaton_botch_dice,
            difficulty,
        )
    }
}

/// Ruby `HunterTheReckoning5th#get_roll_result`。
fn get_roll_result(
    result_text: String,
    success_dice: i64,
    ten_dice: i64,
    _desperaton_ten_dice: i64,
    desperaton_botch_dice: i64,
    difficulty: i64,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let mut result_text = format!("{result_text} 達成数={success_dice}");
    let is_critical = ten_dice >= 2;

    if difficulty > 0 {
        result_text = format!("{result_text} 難易度={difficulty}");

        if success_dice >= difficulty {
            result_text = format!("{result_text} 上回り={}", success_dice - difficulty);

            let desperation_result = if desperaton_botch_dice > 0 {
                " [Overreach or Despair?]"
            } else {
                ""
            };

            if is_critical {
                return Ok(Some(SpecificCommandOutput::result(EvalResult::critical(
                    format!("{result_text}：判定成功! [クリティカル成功/Critical Win]{desperation_result}"),
                ))));
            } else {
                return Ok(Some(SpecificCommandOutput::result(EvalResult::success(
                    format!("{result_text}：判定成功!{desperation_result}"),
                ))));
            }
        } else {
            if desperaton_botch_dice > 0 {
                return Ok(Some(SpecificCommandOutput::result(EvalResult::fumble(
                    format!("{result_text}：判定失敗! [Despair]"),
                ))));
            }
            if success_dice == 0 {
                return Ok(Some(SpecificCommandOutput::result(EvalResult::fumble(
                    format!("{result_text}：判定失敗! [完全失敗/Total Failure]"),
                ))));
            }

            return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
                format!("{result_text}：判定失敗!"),
            ))));
        }
    } else if difficulty < 0 {
        if success_dice == 0 {
            if desperaton_botch_dice > 0 {
                return Ok(Some(SpecificCommandOutput::result(EvalResult::fumble(
                    format!("{result_text}：判定失敗! [Despair]"),
                ))));
            }

            return Ok(Some(SpecificCommandOutput::result(EvalResult::fumble(
                format!("{result_text}：判定失敗! [完全失敗/Total Failure]"),
            ))));
        } else {
            let mut desperation_result = String::new();
            if desperaton_botch_dice > 0 {
                result_text = format!("{result_text}\n　判定失敗なら [Despair]");
                desperation_result = " [Overreach or Despair?]".to_owned();
            }

            if is_critical {
                result_text =
                    format!("{result_text}\n　判定成功なら [クリティカル成功/Critical Win]");
            } else if desperaton_botch_dice > 0 {
                result_text = format!("{result_text}\n　判定成功なら");
            }

            // Ruby: return "#{result_text}#{desperation_result}"（文字列返しは
            // `Base#dice_command` で空文字列以外はそのまま出力になる）
            return Ok(Some(SpecificCommandOutput::text(format!(
                "{result_text}{desperation_result}"
            ))));
        }
    }

    // 難易度0指定(=全ての判定チェックを行わない)
    // Ruby: return result_text.to_s
    Ok(Some(SpecificCommandOutput::text(result_text)))
}

/// Ruby `HunterTheReckoning5th#get_critical_success`。
///
/// 10の目が2個毎に追加2成功
fn get_critical_success(ten_dice: i64) -> i64 {
    (ten_dice / 2) * 2
}

/// Ruby `HunterTheReckoning5th#make_dice_roll`。
fn make_dice_roll(
    dice_pool: i64,
    rng: &mut Randomizer,
) -> Result<(String, i64, i64, i64), EvalError> {
    let dice_list = rng.roll_barabara(dice_pool, 10)?;

    let dice_text = dice_text::join_dice(&dice_list);
    let success_dice = dice_list.iter().filter(|&&x| x >= 6).count() as i64;
    let ten_dice = dice_list.iter().filter(|&&x| x == 10).count() as i64;
    let botch_dice = dice_list.iter().filter(|&&x| x == 1).count() as i64;

    Ok((dice_text, success_dice, ten_dice, botch_dice))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "HunterTheReckoning5th",
            "HunterTheReckoning5th.toml",
            66,
        );
    }
}
