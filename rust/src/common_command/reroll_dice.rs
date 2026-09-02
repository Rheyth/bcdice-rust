use crate::arithmetic::{self, Node, ParenMode};
use crate::common_command::lexer::{self, Cursor, Tok};
use crate::eval::{EvalError, EvalResult};
use crate::format;
use crate::game_system::GameSystem;
use crate::normalize::CmpOp;
use crate::randomizer::{sat_i64, Randomizer};
use crate::Int;

/// 振り足しループの上限。Ruby `RerollDice::REROLL_LIMIT`。
/// 本家 `reroll_dice/node.rb:83` の `while !dice_queue.empty? && loop_count < REROLL_LIMIT`
/// と同一構造（ループ上限到達時の無言打ち切りは本家由来の挙動）。
pub const REROLL_LIMIT: usize = 10000;

/// Ruby `RerollDice.eval(command, game_system, randomizer)`。
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

/// ダイス表記。Ruby `RerollDice::Node::Notation`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notation {
    times: Node,
    sides: Node,
}

/// Ruby `RerollDice::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    notations: Vec<Notation>,
    cmp_op: Option<CmpOp>,
    target_number: Option<Node>,
    reroll_cmp_op: Option<CmpOp>,
    reroll_threshold: Option<Node>,
    /// Ruby `@lexer.source`（空白で切り詰めた入力）。エラーメッセージに使う。
    source: String,
}

/// 振り足し条件。Ruby `Node::RerollCondition`。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RerollCondition {
    cmp_op: CmpOp,
    threshold: Option<Int>,
}

impl RerollCondition {
    /// Ruby `#valid?(sides)`。
    fn valid(&self, sides: &Int) -> bool {
        let Some(threshold) = &self.threshold else {
            return false;
        };
        let one = Int::from(1);
        match self.cmp_op {
            CmpOp::Le => threshold < sides,
            CmpOp::Lt => threshold <= sides,
            CmpOp::Ge => threshold > &one,
            CmpOp::Gt => threshold >= &one,
            CmpOp::Ne => threshold >= &one && threshold <= sides,
            CmpOp::Eq => true,
        }
    }

    /// Ruby `#reroll?(value)`。
    fn reroll(&self, value: i64) -> bool {
        match &self.threshold {
            Some(t) => self.cmp_op.apply(&Int::from(value), t),
            None => false,
        }
    }
}

impl Command {
    /// Ruby `Node::Command#eval`。
    pub fn eval(&self, gs: &dyn GameSystem, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        let round_type = gs.round_type();
        let cmp_op = self.cmp_op.or(gs.default_cmp_op());
        // Ruby: @reroll_cmp_op || cmp_op || :>=
        let reroll_cmp_op = self.reroll_cmp_op.or(cmp_op).unwrap_or(CmpOp::Ge);

        let target_number: Option<Int> = match &self.target_number {
            Some(node) => Some(node.eval(round_type)?),
            None => None,
        }
        .or_else(|| gs.default_target_number().map(Int::from));

        let reroll_threshold: Option<Int> = match &self.reroll_threshold {
            Some(node) => Some(node.eval(round_type)?),
            None => None,
        }
        .or_else(|| gs.reroll_dice_reroll_threshold().map(Int::from))
        .or_else(|| target_number.clone());

        let reroll_condition = RerollCondition {
            cmp_op: reroll_cmp_op,
            threshold: reroll_threshold,
        };

        let mut dice_queue: std::collections::VecDeque<(Int, Int)> =
            std::collections::VecDeque::new();
        for n in &self.notations {
            dice_queue.push_back((n.times.eval(round_type)?, n.sides.eval(round_type)?));
        }

        if !dice_queue
            .iter()
            .all(|(_, sides)| reroll_condition.valid(sides))
        {
            return Ok(self.result_with_text(format!(
                "{} ＞ 条件が間違っています。2R6>=5 あるいは 2R6[5] のように振り足し目標値を指定してください。",
                self.source
            )));
        }

        let dice_list_list = roll(dice_queue, rng, &reroll_condition, gs.sort_barabara_dice())?;

        let dice_list: Vec<i64> = dice_list_list.concat();

        // 振り足し分は出目1の個数をカウントしない
        let one_count = dice_list_list
            .iter()
            .take(self.notations.len())
            .flatten()
            .filter(|v| **v == 1)
            .count();

        let success_count = match cmp_op {
            Some(op) => {
                let target = target_number.as_ref().ok_or(EvalError::Internal(
                    "RerollDice: cmp_op without target number",
                ))?;
                dice_list
                    .iter()
                    .filter(|v| op.apply(&Int::from(**v), target))
                    .count() as i64
            }
            None => 0,
        };

        let mut sequence = vec![
            self.expr(
                round_type,
                &reroll_condition,
                cmp_op,
                target_number.as_ref(),
            )?,
            dice_list_list
                .iter()
                .map(|list| {
                    list.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>()
                .join(" + "),
            format!("成功数{success_count}"),
        ];
        if let Some(t) = gs.grich_text(one_count, dice_list.len(), success_count) {
            sequence.push(t);
        }

        Ok(self.result_with_text(sequence.join(" ＞ ")))
    }

    /// Ruby `#expr`。
    fn expr(
        &self,
        round_type: crate::enums::RoundType,
        reroll_condition: &RerollCondition,
        cmp_op: Option<CmpOp>,
        target_number: Option<&Int>,
    ) -> Result<String, EvalError> {
        let mut notation = Vec::with_capacity(self.notations.len());
        for n in &self.notations {
            notation.push(format!(
                "{}R{}",
                n.times.eval(round_type)?,
                n.sides.eval(round_type)?
            ));
        }

        // Ruby: reroll_condition.cmp_op == cmp_op なら記号を出さない
        let reroll_cmp_op_text = if Some(reroll_condition.cmp_op) == cmp_op {
            ""
        } else {
            format::comparison_operator(Some(reroll_condition.cmp_op))
        };
        let threshold_text = reroll_condition
            .threshold
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let cmp_op_text = format::comparison_operator(cmp_op);
        let target_text = target_number.map(|t| t.to_string()).unwrap_or_default();

        Ok(format!(
            "({}[{reroll_cmp_op_text}{threshold_text}]{cmp_op_text}{target_text})",
            notation.join("+")
        ))
    }

    fn result_with_text(&self, text: String) -> EvalResult {
        EvalResult {
            text,
            secret: self.secret,
            ..EvalResult::default()
        }
    }
}

/// Ruby `Node::Command#roll`。
fn roll(
    mut dice_queue: std::collections::VecDeque<(Int, Int)>,
    rng: &mut Randomizer,
    reroll_condition: &RerollCondition,
    sort: bool,
) -> Result<Vec<Vec<i64>>, EvalError> {
    let mut dice_list_list = Vec::new();
    let mut loop_count = 0usize;

    while loop_count < REROLL_LIMIT {
        let Some((times, sides)) = dice_queue.pop_front() else {
            break;
        };
        loop_count += 1;

        let mut dice_list = rng.roll_barabara(sat_i64(&times), sat_i64(&sides))?;
        if sort {
            dice_list.sort_unstable();
        }

        let reroll_count = dice_list
            .iter()
            .filter(|v| reroll_condition.reroll(**v))
            .count();
        dice_list_list.push(dice_list);

        if reroll_count > 0 {
            dice_queue.push_back((Int::from(reroll_count), sides));
        }
    }

    Ok(dice_list_list)
}

/// Ruby `RerollDice::Parser.parse(source)`。
///
/// 文法:
/// ```text
/// expr: secret notations target
///     | secret notations bracket target
///     | secret notations target at
/// ```
/// `notations` 直後の `BRACKETL` は2番目の規則にしか現れないので分岐は決定的。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    let secret = cur.accept_sym("S");

    let mut notations = Vec::new();
    loop {
        let (times, sides) = super::barabara_dice::parse_dice(&mut cur, "R")?;
        notations.push(Notation { times, sides });
        if !cur.accept(&Tok::Plus) {
            break;
        }
    }

    let mut reroll_cmp_op = None;
    let mut reroll_threshold = None;

    if cur.peek() == Some(&Tok::BracketL) {
        // bracket: BRACKETL add BRACKETR | BRACKETL CMP_OP add BRACKETR
        cur.advance();
        if let Some(Tok::CmpOp(op)) = cur.peek() {
            let op = (*op)?;
            cur.advance();
            reroll_cmp_op = Some(op);
        }
        reroll_threshold = Some(arithmetic::parse_add(&mut cur, ParenMode::Drop)?);
        if !cur.accept(&Tok::BracketR) {
            return None;
        }

        let (cmp_op, target_number) = super::barabara_dice::parse_target(&mut cur)?;
        return cur.at_eof().then_some(Command {
            secret,
            notations,
            cmp_op,
            target_number,
            reroll_cmp_op,
            reroll_threshold,
            source: lexed.source.clone(),
        });
    }

    let (cmp_op, target_number) = super::barabara_dice::parse_target(&mut cur)?;

    if cur.accept(&Tok::At) {
        // at: AT add | AT CMP_OP add
        if let Some(Tok::CmpOp(op)) = cur.peek() {
            let op = (*op)?;
            cur.advance();
            reroll_cmp_op = Some(op);
        }
        reroll_threshold = Some(arithmetic::parse_add(&mut cur, ParenMode::Drop)?);
    }

    cur.at_eof().then_some(Command {
        secret,
        notations,
        cmp_op,
        target_number,
        reroll_cmp_op,
        reroll_threshold,
        source: lexed.source.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reroll_notations() {
        assert!(parse("2R6>=3").is_some());
        assert!(parse("2R4+2R6[>4]>=4").is_some());
        assert!(parse("2R4+2R6>=4@<=2").is_some());
        assert!(parse("2R6[3]").is_some());
        assert!(parse("2R6[3]>=4@2").is_none());
        assert!(parse("2U6").is_none());
    }

    #[test]
    fn source_is_trimmed_input() {
        let cmd = parse("2R6<=7 無限に振り足ししてしまう").unwrap();
        assert_eq!(cmd.source, "2R6<=7");
    }
}
