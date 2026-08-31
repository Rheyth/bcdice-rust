use crate::arithmetic::{self, Node, ParenMode};
use crate::common_command::lexer::{self, Cursor};
use crate::eval::{EvalError, EvalResult};
use crate::game_system::GameSystem;
use crate::randomizer::{sat_i64, Randomizer};
use crate::Int;

/// 最大面数。Ruby `Node::Command::MAX_SIDES`。
pub const MAX_SIDES: i64 = 20;

/// Ruby `TallyDice.eval(command, game_system, randomizer)`。
pub fn eval(
    command: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(cmd) => cmd.eval(game_system, rng),
        None => Ok(None),
    }
}

/// Ruby `TallyDice::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    times: Node,
    sides: Node,
    show_zeros: bool,
}

impl Command {
    /// Ruby `Node::Command#eval`。ダイスとして無効なら `nil`。
    pub fn eval(
        &self,
        gs: &dyn GameSystem,
        rng: &mut Randomizer,
    ) -> Result<Option<EvalResult>, EvalError> {
        let times = self.times.eval(gs.round_type())?;
        let sides = self.sides.eval(gs.round_type())?;

        // Dice#valid? : @times > 0 && @sides > 0
        if times <= Int::ZERO || sides <= Int::ZERO {
            return Ok(None);
        }

        let dice_text = format!("{times}T{}{sides}", if self.show_zeros { "Z" } else { "Y" });

        if sides > Int::from(MAX_SIDES) {
            // Ruby: Result.new(text) なので secret は設定されない
            return Ok(Some(EvalResult::with_text(format!(
                "({dice_text}) ＞ 面数は1以上、{MAX_SIDES}以下としてください"
            ))));
        }

        let times_i64 = sat_i64(&times);
        let sides_i64 = sat_i64(&sides);

        let values = rng.roll_barabara(times_i64, sides_i64)?;

        let values_str = {
            let mut v = values.clone();
            if gs.sort_barabara_dice() {
                v.sort_unstable();
            }
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };

        let values_count_strs: Vec<String> = (1..=sides_i64)
            .filter_map(|v| {
                let count = values.iter().filter(|x| **x == v).count();
                if count == 0 && !self.show_zeros {
                    None
                } else {
                    Some(format!("[{v}]×{count}"))
                }
            })
            .collect();

        let sequence = [
            format!("({dice_text})"),
            values_str,
            values_count_strs.join(", "),
        ];

        Ok(Some(EvalResult {
            text: sequence.join(" ＞ "),
            secret: self.secret,
            ..EvalResult::default()
        }))
    }
}

/// Ruby `TallyDice::Parser.parse(source)`。
///
/// 文法は `expr: secret notation` / `notation: term T show_zeros term` /
/// `show_zeros: Y | Z`。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    let secret = cur.accept_sym("S");

    let times = arithmetic::parse_term(&mut cur, ParenMode::Drop)?;
    if !cur.accept_sym("T") {
        return None;
    }
    let show_zeros = if cur.accept_sym("Y") {
        false
    } else if cur.accept_sym("Z") {
        true
    } else {
        return None;
    };
    let sides = arithmetic::parse_term(&mut cur, ParenMode::Drop)?;

    cur.at_eof().then_some(Command {
        secret,
        times,
        sides,
        show_zeros,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tally_notation() {
        assert!(parse("20TY6").is_some());
        assert!(parse("20ty6 小文字").is_some());
        assert!(parse("(1+2*3)TZ(1+5/2C)").is_some());
        assert!(parse("20TX6").is_none());
        assert!(parse("20T6").is_none());
    }
}
