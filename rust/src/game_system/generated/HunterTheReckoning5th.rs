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
use crate::game_system::{GameSystem, SpecificCommandOutput};
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

    let dice_text = join_dice(&dice_list);
    let success_dice = dice_list.iter().filter(|&&x| x >= 6).count() as i64;
    let ten_dice = dice_list.iter().filter(|&&x| x == 10).count() as i64;
    let botch_dice = dice_list.iter().filter(|&&x| x == 1).count() as i64;

    Ok((dice_text, success_dice, ten_dice, botch_dice))
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice: &[i64]) -> String {
    dice.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/HunterTheReckoning5th.toml");
        path.exists().then_some(path)
    }

    /// `test/data/HunterTheReckoning5th.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/HunterTheReckoning5th.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("HunterTheReckoning5th.toml must parse");
        assert_eq!(
            data.tests.len(),
            66,
            "case count in test/data/HunterTheReckoning5th.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "HunterTheReckoning5th",
                "unexpected game system in HunterTheReckoning5th.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("HunterTheReckoning5th"),
                &tc.input,
                &mut src,
            ) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL HunterTheReckoning5th:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} HunterTheReckoning5th cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
