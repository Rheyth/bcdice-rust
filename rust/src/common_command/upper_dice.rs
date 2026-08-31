use num_traits::ToPrimitive;

use crate::arithmetic::{self, ArithOp, Node, ParenMode};
use crate::common_command::lexer::{self, Cursor, Tok};
use crate::eval::{EvalError, EvalResult};
use crate::format;
use crate::game_system::GameSystem;
use crate::normalize::CmpOp;
use crate::randomizer::{sat_i64, Randomizer};
use crate::Int;

/// Ruby `UpperDice.eval(command, game_system, randomizer)`。
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

/// ダイス表記。Ruby `UpperDice::Node::Notation`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notation {
    roll_times: Node,
    sides: Node,
}

/// Ruby `UpperDice::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    notations: Vec<Notation>,
    modifier: Node,
    cmp_op: Option<CmpOp>,
    target_number: Option<Node>,
    reroll_threshold: Option<Node>,
}

/// 1本のダイスの振り足し結果。Ruby の `{sum:, list:}` に相当。
struct RollEntry {
    sum: Int,
    list: Vec<i64>,
}

impl Command {
    /// Ruby `Node::Command#eval`。
    pub fn eval(&self, gs: &dyn GameSystem, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        let round_type = gs.round_type();

        let mut dice_list: Vec<(Int, Int)> = Vec::with_capacity(self.notations.len());
        for n in &self.notations {
            dice_list.push((n.roll_times.eval(round_type)?, n.sides.eval(round_type)?));
        }

        let reroll_threshold: Int = match &self.reroll_threshold {
            Some(node) => Some(node.eval(round_type)?),
            None => None,
        }
        .or_else(|| gs.upper_dice_reroll_threshold().map(Int::from))
        .unwrap_or_else(|| 0.into());

        let modifier: Int = self.modifier.eval(round_type)?;
        let target_number: Option<Int> = match &self.target_number {
            Some(node) => Some(node.eval(round_type)?),
            None => None,
        };

        let expr = self.expr(
            &dice_list,
            &reroll_threshold,
            &modifier,
            target_number.as_ref(),
        );

        if reroll_threshold <= 1.into() {
            return Ok(
                self.result_with_text(format!("({expr}) ＞ 無限ロールの条件がまちがっています"))
            );
        }

        let mut roll_list: Vec<RollEntry> = Vec::new();
        for (roll_times, sides) in &dice_list {
            let n = if roll_times <= &Int::ZERO {
                0
            } else {
                roll_times.to_usize().unwrap_or(usize::MAX)
            };
            let mut entries: Vec<RollEntry> = Vec::new();
            // Ruby の `Array.new(n) { ... }` は配列を先に確保するので、巨大な n は
            // ブロックを1回も回さずに NoMemoryError になる。ここも同じく先に確保し、
            // 確保できない場合はエラーにする（面数0のダイスは乱数を消費しないため、
            // 確保せずに回すと事実上停止しないループになりうる）。
            entries
                .try_reserve(n)
                .map_err(|_| EvalError::Internal("UpperDice: roll times too large"))?;
            for _ in 0..n {
                let list = roll_ones(rng, sat_i64(sides), &reroll_threshold)?;
                let sum: Int = list.iter().map(|&v| Int::from(v)).sum();
                entries.push(RollEntry { sum, list });
            }
            if gs.sort_barabara_dice() {
                entries.sort_by_key(|e| e.sum.clone());
            }
            roll_list.extend(entries);
        }

        let result = match self.cmp_op {
            Some(op) => {
                let target = target_number.as_ref().ok_or(EvalError::Internal(
                    "UpperDice: cmp_op without target number",
                ))?;
                let success_count = roll_list
                    .iter()
                    .filter(|e| op.apply(&(&e.sum + &modifier), target))
                    .count();
                format!("成功数{success_count}")
            }
            None => {
                // Ruby `#result_max_sum`
                let total: Int = roll_list.iter().map(|e| e.sum.clone()).sum::<Int>() + &modifier;
                // Ruby の `[].max` は nil なので、空なら空文字列になる
                let max = roll_list
                    .iter()
                    .map(|e| &e.sum + &modifier)
                    .max()
                    .map(|m| m.to_string())
                    .unwrap_or_default();
                format!("{max}/{total}(最大/合計)")
            }
        };

        let sequence = [
            format!("({expr})"),
            interlim_expr(&roll_list, &modifier),
            result,
        ];

        Ok(self.result_with_text(sequence.join(" ＞ ")))
    }

    /// Ruby `#expr`。
    fn expr(
        &self,
        dice_list: &[(Int, Int)],
        reroll_threshold: &Int,
        modifier: &Int,
        target_number: Option<&Int>,
    ) -> String {
        let notation = dice_list
            .iter()
            .map(|(t, s)| format!("{t}U{s}"))
            .collect::<Vec<_>>()
            .join("+");
        format!(
            "{notation}[{reroll_threshold}]{}{}{}",
            format::modifier(modifier),
            format::comparison_operator(self.cmp_op),
            target_number.map(|t| t.to_string()).unwrap_or_default()
        )
    }

    fn result_with_text(&self, text: String) -> EvalResult {
        EvalResult {
            text,
            secret: self.secret,
            ..EvalResult::default()
        }
    }
}

/// Ruby `Dice#roll_ones`。閾値以上の出目が出る限り振り足す。
fn roll_ones(
    rng: &mut Randomizer,
    sides: i64,
    reroll_threshold: &Int,
) -> Result<Vec<i64>, EvalError> {
    let mut dice_list = Vec::new();
    loop {
        let value = rng.roll_once(sides)?;
        dice_list.push(value);
        if &Int::from(value) < reroll_threshold {
            return Ok(dice_list);
        }
    }
}

/// Ruby `#interlim_expr`。
fn interlim_expr(roll_list: &[RollEntry], modifier: &Int) -> String {
    let dice = roll_list
        .iter()
        .map(|e| {
            if e.list.len() == 1 {
                e.sum.to_string()
            } else {
                format!(
                    "{}[{}]",
                    e.sum,
                    e.list
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",");

    format!("{dice}{}", format::modifier(modifier))
}

/// Ruby `UpperDice::Parser.parse(source)`。
///
/// 文法:
/// ```text
/// expr: secret notations modifier target
///     | secret notations bracket modifier target
///     | secret notations modifier target at
/// notations: notations PLUS dice | dice
/// dice: term U term
/// modifier: /* none */ | modifier_expr
/// modifier_expr: PLUS mul | MINUS mul | modifier_expr PLUS mul | modifier_expr MINUS mul
/// ```
///
/// `notations` の後の `PLUS` は「次のダイス」か「修正値の開始」かが曖昧に見えるが、
/// LALRでは `PLUS` をshiftした後に `term` を読み、**次のトークンが `U` かどうか**で
/// 決まる（`dice: term . U term` のshiftと `unary: term .` のreduceで、この状態の
/// reduce先読み集合に `U` が入らないため衝突しない）。ここでも同じ順序で判定する。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    let secret = cur.accept_sym("S");

    let mut notations = vec![parse_dice(&mut cur)?];

    // notations の続きと、修正値の先頭 unary の先読み
    let mut modifier: Option<Node> = None;
    loop {
        if !cur.accept(&Tok::Plus) {
            break;
        }
        if cur.peek_starts_term() {
            let term = arithmetic::parse_term(&mut cur, ParenMode::Drop)?;
            if cur.accept_sym("U") {
                let sides = arithmetic::parse_term(&mut cur, ParenMode::Drop)?;
                notations.push(Notation {
                    roll_times: term,
                    sides,
                });
                continue;
            }
            // `PLUS mul` の mul の先頭 unary が term だった
            modifier = Some(arithmetic::parse_mul_from(&mut cur, ParenMode::Drop, term)?);
            break;
        }
        // `PLUS mul`（mul は PLUS/MINUS 始まりの unary など）
        modifier = Some(arithmetic::parse_mul(&mut cur, ParenMode::Drop)?);
        break;
    }

    // bracket は modifier より前にしか現れない（規則2）
    let mut reroll_threshold = None;
    if modifier.is_none() && cur.accept(&Tok::BracketL) {
        reroll_threshold = Some(arithmetic::parse_add(&mut cur, ParenMode::Drop)?);
        if !cur.accept(&Tok::BracketR) {
            return None;
        }
    }
    let has_bracket = reroll_threshold.is_some();

    // modifier: /* none */ | modifier_expr
    if modifier.is_none() {
        if cur.accept(&Tok::Plus) {
            modifier = Some(arithmetic::parse_mul(&mut cur, ParenMode::Drop)?);
        } else if cur.accept(&Tok::Minus) {
            modifier = Some(Node::Negative(Box::new(arithmetic::parse_mul(
                &mut cur,
                ParenMode::Drop,
            )?)));
        }
    }
    // modifier_expr PLUS mul | modifier_expr MINUS mul
    while modifier.is_some() {
        let op = if cur.accept(&Tok::Plus) {
            ArithOp::Add
        } else if cur.accept(&Tok::Minus) {
            ArithOp::Sub
        } else {
            break;
        };
        let rhs = arithmetic::parse_mul(&mut cur, ParenMode::Drop)?;
        modifier = Some(Node::BinaryOp {
            lhs: Box::new(modifier.take().expect("checked by while condition")),
            op,
            rhs: Box::new(rhs),
        });
    }

    let (cmp_op, target_number) = super::barabara_dice::parse_target(&mut cur)?;

    // at は規則3（bracketなし）にしか現れない
    if !has_bracket && cur.accept(&Tok::At) {
        reroll_threshold = Some(arithmetic::parse_add(&mut cur, ParenMode::Drop)?);
    }

    cur.at_eof().then_some(Command {
        secret,
        notations,
        // Ruby: modifier の既定値は Arithmetic::Node::Number.new(0)
        modifier: modifier.unwrap_or(Node::Number(0.into())),
        cmp_op,
        target_number,
        reroll_threshold,
    })
}

/// `dice: term U term`。
fn parse_dice(cur: &mut Cursor) -> Option<Notation> {
    let roll_times = arithmetic::parse_term(cur, ParenMode::Drop)?;
    if !cur.accept_sym("U") {
        return None;
    }
    let sides = arithmetic::parse_term(cur, ParenMode::Drop)?;
    Some(Notation { roll_times, sides })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upper_dice() {
        assert!(parse("4U6[6]").is_some());
        assert!(parse("4U6[6]>=10").is_some());
        assert!(parse("4U6>=10@6").is_some());
        assert!(parse("2U6+2U4[4]+6>=8").is_some());
        assert!(parse("4U6[6]+1+2+10").is_some());
        // 角カッコと@の両指定はパースエラー
        assert!(parse("4U6[6]+11>=21@4").is_none());
        assert!(parse("4U6+2U").is_none());
    }
}
