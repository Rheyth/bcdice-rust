use crate::arithmetic::{self, Node, ParenMode};
use crate::common_command::lexer::{self, Cursor, Tok};
use crate::eval::{EvalError, EvalResult};
use crate::format;
use crate::game_system::GameSystem;
use crate::normalize::CmpOp;
use crate::randomizer::{sat_i64, Randomizer};
use crate::Int;

/// Ruby `BarabaraDice.eval(command, game_system, randomizer)`。
pub fn eval(
    command: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(cmd) => cmd.eval(game_system, rng).map(Some),
        None => Ok(None),
    }
}

/// ダイス表記のノード。Ruby `BarabaraDice::Node::Notation`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notation {
    times: Node,
    sides: Node,
}

/// Ruby `BarabaraDice::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    notations: Vec<Notation>,
    cmp_op: Option<CmpOp>,
    target_number: Option<Node>,
}

impl Command {
    /// Ruby `Node::Command#eval`。
    pub fn eval(&self, gs: &dyn GameSystem, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        let round_type = gs.round_type();

        // (times, sides) に評価した Dice の列
        let mut dice: Vec<(Int, Int)> = Vec::with_capacity(self.notations.len());
        for n in &self.notations {
            dice.push((n.times.eval(round_type)?, n.sides.eval(round_type)?));
        }

        let cmp_op = self.cmp_op.or(gs.default_cmp_op());
        // Ruby: @target_number&.eval(round_type) || game_system.default_target_number()
        // （0はtruthyなので `||` は「nilのときだけ既定値」）
        let target_number: Option<Int> = match &self.target_number {
            Some(node) => Some(node.eval(round_type)?),
            None => None,
        }
        .or_else(|| gs.default_target_number().map(Int::from));

        let mut dice_list_list = Vec::with_capacity(dice.len());
        for (times, sides) in &dice {
            dice_list_list.push(rng.roll_barabara(sat_i64(times), sat_i64(sides))?);
        }
        if gs.sort_barabara_dice() {
            for list in &mut dice_list_list {
                list.sort_unstable();
            }
        }

        let dice_list: Vec<i64> = dice_list_list.concat();
        let count_of_1 = dice_list.iter().filter(|d| **d == 1).count();

        let success_num = match cmp_op {
            Some(op) => {
                let target = target_number.as_ref().ok_or(EvalError::Internal(
                    "BarabaraDice: cmp_op without target number",
                ))?;
                dice_list
                    .iter()
                    .filter(|d| op.apply(&Int::from(**d), target))
                    .count() as i64
            }
            None => 0,
        };

        let notation_text = dice
            .iter()
            .map(|(t, s)| format!("{t}B{s}"))
            .collect::<Vec<_>>()
            .join("+");
        let target_text = target_number
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();

        let mut sequence = vec![
            format!(
                "({notation_text}{}{target_text})",
                format::comparison_operator(cmp_op)
            ),
            dice_list
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ];
        if cmp_op.is_some() {
            sequence.push(format!("成功数{success_num}"));
        }
        if let Some(t) = gs.grich_text(count_of_1, dice_list.len(), success_num) {
            sequence.push(t);
        }

        Ok(EvalResult {
            text: sequence.join(" ＞ "),
            secret: self.secret,
            ..EvalResult::default()
        })
    }
}

/// Ruby `BarabaraDice::Parser.parse(source)`。
///
/// 文法は `expr: secret notations target` / `notations: notations PLUS dice | dice` /
/// `dice: term B term`。`notations` の後の `PLUS` は必ず `dice` を導くので、
/// UpperDiceのような先読み分岐は不要。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    let secret = cur.accept_sym("S");

    let mut notations = vec![parse_dice(&mut cur)?];
    while cur.accept(&Tok::Plus) {
        notations.push(parse_dice(&mut cur)?);
    }

    let (cmp_op, target_number) = parse_target(&mut cur)?;

    cur.at_eof().then_some(Command {
        secret,
        notations,
        cmp_op,
        target_number,
    })
}

/// `dice: term B term`。
fn parse_dice(cur: &mut Cursor) -> Option<Notation> {
    let times = arithmetic::parse_term(cur, ParenMode::Drop)?;
    if !cur.accept_sym("B") {
        return None;
    }
    let sides = arithmetic::parse_term(cur, ParenMode::Drop)?;
    Some(Notation { times, sides })
}

/// `target: /* none */ | CMP_OP add`。
///
/// `raise ParseError unless cmp_op` があるので、正規化に失敗した比較演算子は
/// 構文エラーとして扱う。
///
/// barabara / reroll / upper の3つのparser.yはこの規則が字句まで同一なので、
/// ここに置いたものを共有する。
pub(crate) fn parse_target(cur: &mut Cursor) -> Option<(Option<CmpOp>, Option<Node>)> {
    match cur.peek() {
        Some(Tok::CmpOp(op)) => {
            let op = (*op)?;
            cur.advance();
            let target = arithmetic::parse_add(cur, ParenMode::Drop)?;
            Some((Some(op), Some(target)))
        }
        _ => Some((None, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_notations() {
        assert!(parse("2b6+4b10>3").is_some());
        assert!(parse("(1*2)b(2+4)").is_some());
        assert!(parse("2b6+1").is_none());
        assert!(parse("2b6x").is_none());
    }
}
