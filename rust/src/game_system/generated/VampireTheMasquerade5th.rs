//! P4で手書き移植した `lib/bcdice/game_system/VampireTheMasquerade5th.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `VampireTheMasquerade5th#eval_game_system_specific_command`
//!   （判定コマンド `nVMFx+x` / `nVMIxHx`）と、その下請けの
//!   `get_dice_pools` / `get_roll_result` / `get_critical_success` / `make_dice_roll`
//!
//! コマンドの形は `WerewolfTheApocalypse5th` と同型だが、`get_roll_result` の構造が
//! 異なる（達成数を無条件に先頭へ出す・成功数を書き換えない・
//! 凄惨なるクリティカル／獣の過ちの分岐を持つ）ので、共通化せず原典どおりに分けて持つ。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::VampireTheMasquerade5th`（ID: `VampireTheMasquerade5th`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VampireTheMasquerade5th;

/// Ruby `DIFFICULTY_INDEX`。難易度のキャプチャ番号。
const DIFFICULTY_INDEX: usize = 1;
/// Ruby `DICE_POOL_HUNGER_DICE_NO_INCLUDED_INDEX`。VMF のダイスプール。
const DICE_POOL_HUNGER_DICE_NO_INCLUDED_INDEX: usize = 5;
/// Ruby `HUNGER_DICE_NO_INCLUDED_INDEX`。VMF のハンガーダイス。
const HUNGER_DICE_NO_INCLUDED_INDEX: usize = 7;
/// Ruby `COMMAND_HUNGER_DICE_INCLUDED_INDEX`。内数指定側のコマンド名（`VMI`）。
const COMMAND_HUNGER_DICE_INCLUDED_INDEX: usize = 9;
/// Ruby `DICE_POOL_HUNGER_DICE_INCLUDED_INDEX`。VMI のダイスプール。
const DICE_POOL_HUNGER_DICE_INCLUDED_INDEX: usize = 10;
/// Ruby `HUNGER_DICE_INCLUDED_INDEX`。VMI のハンガーダイス。
const HUNGER_DICE_INCLUDED_INDEX: usize = 12;

/// Ruby `NOT_CHECK_SUCCESS`。判定成功にかかわるチェックを行わない難易度。
const NOT_CHECK_SUCCESS: i64 = -1;

impl GameSystem for VampireTheMasquerade5th {
    fn id(&self) -> &'static str {
        "VampireTheMasquerade5th"
    }

    fn name(&self) -> &'static str {
        "ヴァンパイア：ザ・マスカレード第５版"
    }

    fn sort_key(&self) -> &'static str {
        "うあんはいあさますかれえと5"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(nVMFx+x または nVMIxHx)
  VMFコマンドはハンガーダイスとダイスプールを個別に指定する。
  VMIコマンドはハンガーダイスをダイスプールの内数として指定する。

    例：難易度2、9ダイスプールでハンガーダイス3個の場合、それぞれ以下のようなコマンドとなる。
    2VMF6+3
    2VMI9H3

  難易度指定：達成数のカウント、判定成功と失敗、クリティカル処理、クリティカル成功、完全失敗のチェックを行う
             （ハンガーダイスがある場合）凄惨なるクリティカルと獣の過ちチェックを行う
  例) (難易度)VMF(通常ダイス)+(ハンガーダイス)
      (難易度)VMF(通常ダイス)
      (難易度)VMI(通常ダイス)H(ハンガーダイス)
      (難易度)VMI(通常ダイス)

  難易度省略：達成数のカウント、判定失敗、クリティカル処理、完全失敗、（ハンガーダイスがある場合）獣の過ちチェックを行う
              判定成功、凄惨なるクリティカルのチェックを行わない
              クリティカル成功、（ハンガーダイスがある場合）獣の過ち、凄惨なるクリティカルのヒントを出力
  例) VMF(通常ダイス)+(ハンガーダイス)
      VMF(通常ダイス)
      VMI(通常ダイス)H(ハンガーダイス)
      VMI(通常ダイス)

  難易度0指定：クリティカル処理と達成数のカウントを行い、全てのチェックを行わない
  例) 0VMF(通常ダイス)+(ハンガーダイス)
      0VMF(通常ダイス)
      0VMI(通常ダイス)+(ハンガーダイス)
      0VMI(通常ダイス)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*(VMF|(VMI\d*(H\d?)?))"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `VampireTheMasquerade5th#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let Some(m) = command_pattern().captures(command) else {
            // Ruby: return '' （`dice_command` が nil に畳む）
            return Ok(Some(SpecificCommandOutput::text("")));
        };

        let (dice_pool, hunger_dice_pool) = get_dice_pools(&m);
        if hunger_dice_pool > 5 {
            return Ok(Some(SpecificCommandOutput::text(
                "ハンガーダイス指定は5ダイスが最大です。",
            )));
        }

        // Ruby: `dice_text, success_dice, ten_dice, = make_dice_roll(dice_pool)`
        // 第4要素（1の個数）は捨てられる。獣の過ちはハンガーダイス側だけで決まる。
        let normal = make_dice_roll(dice_pool, rng)?;
        let mut success_dice = normal.success_dice;
        let mut ten_dice = normal.ten_dice;

        let result_text = format!("({dice_pool}D10");
        let hunger_ten_dice;
        let hunger_botch_dice;
        let result_text = if hunger_dice_pool >= 0 {
            let hunger = make_dice_roll(hunger_dice_pool, rng)?;

            hunger_ten_dice = hunger.ten_dice;
            hunger_botch_dice = hunger.botch_dice;
            ten_dice += hunger_ten_dice;
            success_dice += hunger.success_dice;

            format!(
                "{result_text}+{hunger_dice_pool}D10) ＞ [{}]+[{}] ",
                normal.dice_text, hunger.dice_text
            )
        } else {
            hunger_ten_dice = 0;
            hunger_botch_dice = 0;
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
            hunger_ten_dice,
            hunger_botch_dice,
            difficulty,
        )))
    }
}

/// Ruby の判定コマンド正規表現。
///
/// Ruby: `/\A(\d+)?(((VMF)(-?\d+)(\+(\d+))?)|((VMI)(-?\d+)(H(\d+))?))$/`
///
/// Rubyの `$` は行末にもマッチするが、`Preprocessor` が最初の空白（改行を含む）より
/// 前しか残さないため、末尾アンカーは `\z` と等価になる。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(\d+)?(((VMF)(-?\d+)(\+(\d+))?)|((VMI)(-?\d+)(H(\d+))?))\z")
            .expect("valid regex")
    })
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(s: &str) -> i64 {
    str_helpers::to_i_signed_saturating(s)
}

/// Ruby `VampireTheMasquerade5th#get_dice_pools`。
///
/// 戻り値は `(dice_pool, hunger_dice_pool)`。ハンガーダイス未指定は `-1`。
fn get_dice_pools(m: &Captures<'_>) -> (i64, i64) {
    let hunger_dice_included_command = m
        .get(COMMAND_HUNGER_DICE_INCLUDED_INDEX)
        .map(|x| x.as_str());
    if hunger_dice_included_command == Some("VMI") {
        // Hunger Diceを内数処理するの場合
        let mut hunger_dice_pool = m
            .get(HUNGER_DICE_INCLUDED_INDEX)
            .map_or(-1, |x| to_i(x.as_str()));
        // Ruby `nil.to_i` は 0
        let mut dice_pool_value = m
            .get(DICE_POOL_HUNGER_DICE_INCLUDED_INDEX)
            .map_or(0, |x| to_i(x.as_str()));
        if dice_pool_value <= 0 {
            // ダイスプールが0のとき、最低保証ダイスプールであるダイスプール1にする
            dice_pool_value = 1;
        }
        // Ruby: `dice_pool_value - (hunger_dice_pool < 0 ? 0 : hunger_dice_pool)`
        let mut dice_pool = dice_pool_value - hunger_dice_pool.max(0);
        if dice_pool_value > 0 && hunger_dice_pool >= dice_pool_value {
            // 1 以上のダイスプール、かつ、ハンガーダイスがダイスプール以上のとき、
            // ダイスプールが全てハンガーダイスになる。
            dice_pool = 0;
            hunger_dice_pool = dice_pool_value;
        }
        (dice_pool, hunger_dice_pool)
    } else {
        // Hunger DiceがPLによる内数指定の場合
        let hunger_dice_pool = m
            .get(HUNGER_DICE_NO_INCLUDED_INDEX)
            .map_or(-1, |x| to_i(x.as_str()));
        let mut dice_pool = m
            .get(DICE_POOL_HUNGER_DICE_NO_INCLUDED_INDEX)
            .map_or(0, |x| to_i(x.as_str()));
        if dice_pool <= 0 && hunger_dice_pool <= 0 {
            // ダイスプールとハンガーダイスどちらも0指定のとき、最低保証ダイスプールである1ダイスプールにする
            dice_pool = 1;
        }
        (dice_pool, hunger_dice_pool)
    }
}

/// Ruby `VampireTheMasquerade5th#get_roll_result`。
fn get_roll_result(
    result_text: String,
    success_dice: i64,
    ten_dice: i64,
    hunger_ten_dice: i64,
    hunger_botch_dice: i64,
    difficulty: i64,
) -> SpecificCommandOutput {
    let mut result_text = format!("{result_text} 達成数={success_dice}");
    let is_critical = ten_dice >= 2;

    if difficulty > 0 {
        result_text = format!("{result_text} 難易度={difficulty}");

        if success_dice >= difficulty {
            result_text = format!("{result_text} 上回り={}", success_dice - difficulty);

            if hunger_ten_dice > 0 && is_critical {
                return SpecificCommandOutput::result(EvalResult::critical(format!(
                    "{result_text}：判定成功! [凄惨なるクリティカル/Messy Critical]"
                )));
            } else if is_critical {
                return SpecificCommandOutput::result(EvalResult::critical(format!(
                    "{result_text}：判定成功! [クリティカル成功/Critical Win]"
                )));
            }

            return SpecificCommandOutput::result(EvalResult::success(format!(
                "{result_text}：判定成功!"
            )));
        }

        if hunger_botch_dice > 0 {
            return SpecificCommandOutput::result(EvalResult::fumble(format!(
                "{result_text}：判定失敗! [獣の過ち/Bestial Failure]"
            )));
        }
        if success_dice == 0 {
            return SpecificCommandOutput::result(EvalResult::fumble(format!(
                "{result_text}：判定失敗! [完全失敗/Total Failure]"
            )));
        }

        return SpecificCommandOutput::result(EvalResult::failure(format!(
            "{result_text}：判定失敗!"
        )));
    } else if difficulty < 0 {
        if success_dice == 0 {
            if hunger_botch_dice > 0 {
                return SpecificCommandOutput::result(EvalResult::fumble(format!(
                    "{result_text}：判定失敗! [獣の過ち/Bestial Failure]"
                )));
            }

            return SpecificCommandOutput::result(EvalResult::fumble(format!(
                "{result_text}：判定失敗! [完全失敗/Total Failure]"
            )));
        }

        if hunger_botch_dice > 0 {
            result_text = format!("{result_text}\n　判定失敗なら [獣の過ち/Bestial Failure]");
        }
        if hunger_ten_dice > 0 && is_critical {
            result_text =
                format!("{result_text}\n　判定成功なら [凄惨なるクリティカル/Messy Critical]");
        } else if is_critical {
            result_text = format!("{result_text}\n　判定成功なら [クリティカル成功/Critical Win]");
        }
        return SpecificCommandOutput::text(result_text);
    }

    // 難易度0指定(=全ての判定チェックを行わない)
    SpecificCommandOutput::text(result_text)
}

/// Ruby `VampireTheMasquerade5th#get_critical_success`。10の目2個毎に追加2成功。
fn get_critical_success(ten_dice: i64) -> i64 {
    // Ruby `Integer#/` は床除算だが、`ten_dice` は非負なので通常の整数除算と一致する。
    (ten_dice / 2) * 2
}

/// Ruby `VampireTheMasquerade5th#make_dice_roll` の戻り値。
struct DiceRoll {
    /// Ruby `dice_text`。出目をカンマ区切りにしたもの。
    dice_text: String,
    /// Ruby `success_dice`。6以上の個数。
    success_dice: i64,
    /// Ruby `ten_dice`。10の個数。
    ten_dice: i64,
    /// Ruby `botch_dice`。1の個数。
    botch_dice: i64,
}

/// Ruby `VampireTheMasquerade5th#make_dice_roll`。
fn make_dice_roll(dice_pool: i64, rng: &mut Randomizer) -> Result<DiceRoll, EvalError> {
    let dice_list = rng.roll_barabara(dice_pool, 10)?;

    let dice_text = dice_list
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let success_dice = dice_list.iter().filter(|&&x| x >= 6).count() as i64;
    let ten_dice = dice_list.iter().filter(|&&x| x == 10).count() as i64;
    let botch_dice = dice_list.iter().filter(|&&x| x == 1).count() as i64;

    Ok(DiceRoll {
        dice_text,
        success_dice,
        ten_dice,
        botch_dice,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    /// 注入乱数が余るケース（1始まりのケース番号 → 余る本数）。
    ///
    /// upstream の TOML は「最低保証ダイスプールへの切り上げ」
    /// 「ハンガーダイスが5を超えた場合の早期return」「コマンドが正規表現に非マッチ」により、
    /// 実際に振る本数より多い `rands` を持つ。Ruby の `RandomizerMock` は余りを許す
    /// （枯渇したときだけ raise する）ので、原典どおりに移植するとこれらのケースは
    /// 乱数を使い切らない。ここだけ余りを許容するが、本数まで厳密に一致させる
    /// （ダイスを振る本数がずれれば必ず失敗する）。それ以外の全ケースでは
    /// `rust/tests/toml_harness.rs::run_case` と同じく余り0本を要求する。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[
        (1, 2),
        (19, 2),
        (22, 1),
        (34, 2),
        (43, 2),
        (59, 2),
        (83, 2),
        (100, 2),
        (103, 1),
        (115, 2),
        (124, 2),
        (140, 2),
        (164, 6),
        (165, 5),
        (168, 6),
        (170, 6),
    ];

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/VampireTheMasquerade5th.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/VampireTheMasquerade5th.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数の消費本数）。乱数は既定で余り0本を要求し、
    /// `SURPLUS_RANDS_ALLOWED` のケースだけ余る本数を厳密一致で固定する。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/VampireTheMasquerade5th.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("VampireTheMasquerade5th.toml must parse");
        assert_eq!(
            data.tests.len(),
            183,
            "case count in test/data/VampireTheMasquerade5th.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "VampireTheMasquerade5th",
                "unexpected game system in VampireTheMasquerade5th.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("VampireTheMasquerade5th"),
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

            let allowed_surplus = SURPLUS_RANDS_ALLOWED
                .iter()
                .find(|(case, _)| *case == i + 1)
                .map_or(0, |(_, remaining)| *remaining);
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL VampireTheMasquerade5th:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} VampireTheMasquerade5th cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
