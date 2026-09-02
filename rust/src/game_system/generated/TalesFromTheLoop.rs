//! P4で手書き移植した `lib/bcdice/game_system/TalesFromTheLoop.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TalesFromTheLoop#eval_game_system_specific_command`（判定コマンド `nTFLx±y`）
//! - `#make_dice_roll` / `#make_dice_roll_text`

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::TalesFromTheLoop`（ID: `TalesFromTheLoop`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TalesFromTheLoop;

impl GameSystem for TalesFromTheLoop {
    fn id(&self) -> &'static str {
        "TalesFromTheLoop"
    }

    fn name(&self) -> &'static str {
        "ザ・ループTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "さるうふTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定コマンド(nTFLx-x+x or nTFLx+x-x)
  (必要成功数)TFL(判定ダイス数)+/-(修正ダイス数)

※ 必要成功数と修正ダイス数は省略可能

例1) 必要成功数1、判定ダイスは能力値3
      1TFL3

例2）必要成功数不明、あるいはダイスボットの成功判定を使わない、判定ダイスは能力値3+技能1で4、アイテムの修正+1
      TFL4+1

例3）必要成功数1、判定ダイスは能力値2+技能1で3、コンディションにチェックが2つ、アイテムの修正+1
      1TFL3-2+1
     あるいは以下のようにカッコ書きで内訳を詳細に記述することも可能。
      1TFL(2+1)-(1+1)+1
     修正ダイスのプラスとマイナスは逆でもよい。
      1TFL(2+1)+1-(1+1)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?(TFL)"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `TalesFromTheLoop#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `Command::Parser.new(/\d*TFL\d+/, round_type: round_type)`。
///
/// 比較演算子・目標値は原典が制限していないので既定のまま
/// （パースはできるが `make_dice_roll_text` は目標値を見ない）。
fn parser() -> &'static Parser {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // `round_type` は `Base` の既定（`RoundType::Floor`）。
    PARSER.get_or_init(|| Parser::new(&[r"\d*TFL\d+"], RoundType::Floor))
}

/// Ruby `TalesFromTheLoop#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(parsed) = parser().parse(command) else {
        return Ok(None);
    };

    // Ruby: difficulty, dice_pool = parsed.command.split("TFL", 2).map(&:to_i)
    // notation が `\d*TFL\d+` なので "TFL" は必ず含まれ、2要素に割れる。
    let mut parts = parsed.command.splitn(2, "TFL");
    let difficulty = parts.next().map_or(0, to_i);
    let mut dice_pool = parts.next().map_or(0, to_i);

    dice_pool = dice_pool.saturating_add(crate::randomizer::sat_i64(&parsed.modify_number));
    if dice_pool <= 0 {
        dice_pool = 1;
    }

    let (ability_dice_text, success_dice) = make_dice_roll(dice_pool, rng)?;

    Ok(Some(make_dice_roll_text(
        difficulty,
        dice_pool,
        &ability_dice_text,
        success_dice,
    )))
}

/// Ruby `String#to_i`（先頭の十進数だけを読み、無ければ 0）。
///
/// ここに来る文字列は `\d*TFL\d+` の一部なので符号や空白は現れない。
/// 桁あふれは Ruby だと Bignum になるが、`i64` に飽和させておけば
/// ダイス個数側は `roll_barabara` の上限（`TooManyRandsError`）へ落ちる。
fn to_i(digits: &str) -> i64 {
    if digits.is_empty() {
        // Ruby: "".to_i == 0
        return 0;
    }
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `TalesFromTheLoop#make_dice_roll`。
///
/// 6の目の数が成功数。
fn make_dice_roll(dice_pool: i64, rng: &mut Randomizer) -> Result<(String, i64), EvalError> {
    let dice_list = rng.roll_barabara(dice_pool, 6)?;
    let success_dice = dice_list.iter().filter(|d| **d == 6).count() as i64;

    let joined = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    Ok((format!("[{joined}]"), success_dice))
}

/// Ruby `TalesFromTheLoop#make_dice_roll_text`。
///
/// 必要成功数（`difficulty`）が 0（＝コマンドで省略）のときだけ `Result` ではなく
/// 文字列を返す。`reroll_command` は Ruby だと `if` の中でだけ代入される
/// ローカル変数なので、振り直せない場合は `nil` ＝ 空文字列として連結される。
fn make_dice_roll_text(
    difficulty: i64,
    dice_pool: i64,
    ability_dice_text: &str,
    success_dice: i64,
) -> SpecificCommandOutput {
    let mut dice_count_text =
        format!("({dice_pool}D6) ＞ {ability_dice_text} 成功数:{success_dice}");
    let push_dice = dice_pool - success_dice;

    let mut reroll_command = String::new();
    if push_dice > 0 {
        dice_count_text = format!("{dice_count_text} 振り直し可能:{push_dice}");
        reroll_command = format!("\n振り直しコマンド: TFL{push_dice}");
    }

    if difficulty <= 0 {
        return SpecificCommandOutput::text(format!("{dice_count_text}{reroll_command}"));
    }

    let result = if success_dice >= difficulty {
        EvalResult::success(format!("{dice_count_text} 成功！{reroll_command}"))
    } else {
        EvalResult::failure(format!("{dice_count_text} 失敗！{reroll_command}"))
    };
    SpecificCommandOutput::result(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TalesFromTheLoop",
            "TalesFromTheLoop.toml",
            42,
        );
    }
}
