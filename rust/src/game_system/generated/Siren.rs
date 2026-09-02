//! P4で手書き移植した `lib/bcdice/game_system/Siren.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Siren#check_action`（判定 `SL+a<=b±c`）
//! - `Siren#check_training`（育成 `TR$a<=b`）

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Siren`（ID: `Siren`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Siren;

impl GameSystem for Siren {
    fn id(&self) -> &'static str {
        "Siren"
    }

    fn name(&self) -> &'static str {
        "終末アイドル育成TRPG セイレーン"
    }

    fn sort_key(&self) -> &'static str {
        "せいれえん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定: SL+a<=b±c
  a=達成値への修正(0の場合は省略)
  b=能力値
  c=判定への修正(0の場合は省略、複数可)
例)判定修正-10の装備を装着しながら【技術：60】〈兵器：2〉で判定する場合。
SL+2<=60+40-10

・育成: TR$a<=b
  a=育成した回数
  b=ヘルス
例）ヘルスの現在値が60で2回目の【身体】の育成を行う場合。
TR$2<=60
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SL", "TR"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Siren#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /^SL/ ... when /^TR/ ... else return nil
        //       （`/i` 無しだが `dice_command` が大文字化済みの文字列を渡す）
        if command.starts_with("SL") {
            return check_action(command, rng);
        }
        if command.starts_with("TR") {
            return check_training(command, rng);
        }
        Ok(None)
    }
}

/// Ruby `dig10` / `dig1` の算出。
///
/// 十の位・一の位がそれぞれ0なら10として扱う（1D100の00を100と読む）。
fn digits(dice: i64) -> (i64, i64) {
    let dig10 = dice / 10;
    let dig1 = dice % 10;
    (
        if dig10 == 0 { 10 } else { dig10 },
        if dig1 == 0 { 10 } else { dig1 },
    )
}

/// Ruby `Siren#check_action`（判定）。
fn check_action(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('SL', round_type: @round_type).restrict_cmp_op_to(:<=)
    //       `@round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["SL"], RoundType::Floor).restrict_cmp_op_to(&[Some(CmpOp::Le)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `restrict_cmp_op_to(:<=)` が目標値省略を許さないので、必ず値がある。
    let Some(target) = parsed.target_number else {
        return Ok(None);
    };

    let dice = rng.roll_once(100)?;

    if dice > crate::randomizer::sat_i64(&target) {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
            format!("(1D100<={target}) ＞ {dice} ＞ 失敗"),
        ))));
    }

    let (dig10, dig1) = digits(dice);
    let achievement_value = dig10 + dig1 + parsed.modify_number;
    Ok(Some(SpecificCommandOutput::result(EvalResult::success(
        format!("(1D100<={target}) ＞ {dice} ＞ 成功(達成値：{achievement_value})"),
    ))))
}

/// Ruby `Siren#check_training`（育成）。
fn check_training(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('TR', round_type: @round_type)
    //         .restrict_cmp_op_to(:<=).enable_dollar.disable_modifier
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["TR"], RoundType::Floor)
            .restrict_cmp_op_to(&[Some(CmpOp::Le)])
            .enable_dollar()
            .disable_modifier()
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: count = parsed.dollar; return nil if count.nil?
    let Some(count) = parsed.dollar else {
        return Ok(None);
    };

    let Some(target) = parsed.target_number else {
        return Ok(None);
    };

    let dice = rng.roll_once(100)?;

    let (dig10, dig1) = digits(dice);
    let achievement_value = dig10 + dig1;

    if dice > crate::randomizer::sat_i64(&target) {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
            format!(
                "(1D100<={target}) ＞ {dice} ＞ 失敗(能力値減少：10 / ヘルス減少：{achievement_value})"
            ),
        ))));
    }

    Ok(Some(SpecificCommandOutput::result(EvalResult::success(
        format!(
            "(1D100<={target}) ＞ {dice} ＞ 成功(能力値上昇：{} / ヘルス減少：{achievement_value})",
            count * 5 + achievement_value
        ),
    ))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Siren", "Siren.toml", 19);
    }
}
