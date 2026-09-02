//! 汎用コマンド群。Ruby `lib/bcdice/common_command*` の移植。
//!
//! `COMMANDS` の順序（`lib/bcdice/common_command.rb`）がそのまま試行順になる。
//! いずれかが結果を返した時点で打ち切る。

pub mod add_dice;
pub mod barabara_dice;
pub mod calc;
pub mod choice;
pub mod d66_dice;
pub mod lexer;
pub mod repeat;
pub mod reroll_dice;
mod scanner;
pub mod tally_dice;
pub mod upper_dice;
pub mod version;

use crate::eval::{EvalError, EvalResult};
use crate::game_system::GameSystem;
use crate::randomizer::Randomizer;

/// Ruby `Base#eval_common_command(command)`。
///
/// 渡される `command` は**前処理前の生入力**（`Base#eval` が `@raw_input` を渡す）。
/// `change_text` はここでもう一度適用される（原典どおり）。
pub fn eval_common_command(
    game_system: &dyn GameSystem,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let command = game_system.change_text(command);
    let command = command.as_ref();

    // COMMANDS = [AddDice, BarabaraDice, TallyDice, Calc, Choice,
    //             D66Dice, Repeat, RerollDice, UpperDice, Version]
    if let Some(r) = add_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = barabara_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = tally_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = calc::eval(command, game_system)? {
        return Ok(Some(r));
    }
    if let Some(r) = choice::eval(command, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = d66_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = repeat::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = reroll_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = upper_dice::eval(command, game_system, rng)? {
        return Ok(Some(r));
    }
    if let Some(r) = version::eval(command) {
        return Ok(Some(r));
    }

    Ok(None)
}
