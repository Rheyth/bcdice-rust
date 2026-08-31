//! 加算ダイス。Ruby `lib/bcdice/common_command/add_dice*` の移植。
//!
//! - `add_dice.rb` … エントリポイント
//! - `add_dice/parser.y` … Racc文法
//! - `add_dice/node.rb` … 構文木
//! - `add_dice/randomizer.rb` … ダイスロールの記録

use num_traits::{ToPrimitive, Zero};

use crate::common_command::lexer::{self, Cursor, Tok};
use crate::eval::{EvalError, EvalResult};
use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::randomizer::{sat_i64, Randomizer};
use crate::Int;

/// Ruby `AddDice.eval(command, game_system, randomizer)`。
pub fn eval(
    command: &str,
    game_system: &dyn GameSystem,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(mut cmd) => cmd.eval(game_system, rng).map(Some),
        None => Ok(None),
    }
}

/// 出目のフィルタ。Ruby `Node::DiceRollWithFilter::Filter`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// 大きな出目から複数個取る（`KH`）
    KeepHighest,
    /// 小さな出目から複数個取る（`KL`）
    KeepLowest,
    /// 大きな出目から複数個除く（`DH`）
    DropHighest,
    /// 小さな出目から複数個除く（`DL`）
    DropLowest,
}

impl Filter {
    /// Ruby `Filter#abbr`。
    pub fn abbr(self) -> &'static str {
        match self {
            Filter::KeepHighest => "KH",
            Filter::KeepLowest => "KL",
            Filter::DropHighest => "DH",
            Filter::DropLowest => "DL",
        }
    }

    /// Ruby `Filter#apply`。`sorted_values` は昇順ソート済みの出目。
    ///
    /// Rubyの `Array#take(n)` / `#drop(n)` は負数で ArgumentError になるが、
    /// TOMLテストに該当ケースはない。ここでは0として扱う。
    fn apply(self, sorted_values: &[i64], n: &Int) -> Vec<i64> {
        let len = sorted_values.len();
        let n_usize = if n <= &Int::ZERO {
            0
        } else {
            n.to_usize().unwrap_or(len).min(len)
        };
        match self {
            Filter::KeepHighest => sorted_values.iter().rev().take(n_usize).copied().collect(),
            Filter::KeepLowest => sorted_values.iter().take(n_usize).copied().collect(),
            Filter::DropHighest => sorted_values.iter().rev().skip(n_usize).copied().collect(),
            Filter::DropLowest => sorted_values.iter().skip(n_usize).copied().collect(),
        }
    }
}

/// 除算の端数処理。Ruby `AddDice::Node::DivideWith*`。
///
/// 記号が `Arithmetic` 側と異なる（切り上げが `U`）ので別の型にしてある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivideKind {
    /// `DivideWithGameSystemDefault`（記号なし）
    GameSystemDefault,
    /// `DivideWithRoundingUp`（記号 `U`）
    RoundingUp,
    /// `DivideWithRoundingOff`（記号 `R`）
    RoundingOff,
    /// `DivideWithRoundingDown`（記号 `F`）
    RoundingDown,
}

impl DivideKind {
    fn rounding_method(self) -> &'static str {
        match self {
            DivideKind::GameSystemDefault => "",
            DivideKind::RoundingUp => "U",
            DivideKind::RoundingOff => "R",
            DivideKind::RoundingDown => "F",
        }
    }

    /// Ruby `DivideBase#calc`。**除数0のときは1を返す**（Arithmetic側と違う）。
    fn calc(
        self,
        lhs: Int,
        rhs: Int,
        round_type: crate::enums::RoundType,
    ) -> Result<Int, EvalError> {
        use crate::arithmetic::{ceil_div, floor_div, round_div};
        use crate::enums::RoundType;

        if rhs.is_zero() {
            return Ok(1.into());
        }
        Ok(match self {
            DivideKind::GameSystemDefault => match round_type {
                RoundType::Ceil => ceil_div(lhs, rhs)?,
                RoundType::Round => round_div(lhs, rhs)?,
                RoundType::Floor => floor_div(lhs, rhs),
            },
            DivideKind::RoundingUp => ceil_div(lhs, rhs)?,
            DivideKind::RoundingOff => round_div(lhs, rhs)?,
            DivideKind::RoundingDown => floor_div(lhs, rhs),
        })
    }
}

/// 二項演算子（除算以外）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
}

impl BinOp {
    fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
        }
    }

    fn apply(self, lhs: Int, rhs: Int) -> Int {
        match self {
            BinOp::Add => lhs + rhs,
            BinOp::Sub => lhs - rhs,
            BinOp::Mul => lhs * rhs,
        }
    }
}

/// 加算ロールの構文木。Ruby `AddDice::Node::*`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    BinaryOp {
        lhs: Box<Node>,
        op: BinOp,
        rhs: Box<Node>,
    },
    Divide {
        lhs: Box<Node>,
        rhs: Box<Node>,
        kind: DivideKind,
    },
    Negate(Box<Node>),
    DiceRoll {
        times: Box<Node>,
        sides: Box<Node>,
        text: Option<String>,
    },
    /// 面数省略のダイスロール。Ruby `ImplicitSidesDiceRoll`（`DiceRoll` のサブクラス）。
    ImplicitSidesDiceRoll {
        times: Box<Node>,
        text: Option<String>,
    },
    DiceRollWithFilter {
        times: Box<Node>,
        /// `None` は Ruby側の `:implicit`（面数省略）。
        sides: Option<Box<Node>>,
        n_filtering: Box<Node>,
        filter: Filter,
        text: Option<String>,
    },
    Parenthesis(Box<Node>),
    Number(Int),
}

impl Node {
    /// Ruby `#include_dice?`。
    pub fn include_dice(&self) -> bool {
        match self {
            Node::BinaryOp { lhs, rhs, .. } | Node::Divide { lhs, rhs, .. } => {
                lhs.include_dice() || rhs.include_dice()
            }
            Node::Negate(body) | Node::Parenthesis(body) => body.include_dice(),
            Node::DiceRoll { .. }
            | Node::ImplicitSidesDiceRoll { .. }
            | Node::DiceRollWithFilter { .. } => true,
            Node::Number(_) => false,
        }
    }

    /// Ruby `@lhs.is_a?(Node::DiceRoll)`。
    ///
    /// `ImplicitSidesDiceRoll` は `DiceRoll` のサブクラスなので **true**、
    /// `DiceRollWithFilter` は別クラスなので false。
    /// （`dummyBot.toml` の `201D` が `(201D6) ＞ 0` になる根拠）
    fn is_dice_roll_class(&self) -> bool {
        matches!(
            self,
            Node::DiceRoll { .. } | Node::ImplicitSidesDiceRoll { .. }
        )
    }

    /// Ruby `#eval(game_system, randomizer)`。
    ///
    /// `rng` が `None` の呼び出し（Rubyの `eval(game_system, nil)`）は、文法の
    /// `include_dice?` チェックでダイスを含まないと保証された部分木にのみ行われる。
    fn eval(
        &mut self,
        gs: &dyn GameSystem,
        rng: Option<&mut AddDiceRandomizer<'_, '_>>,
    ) -> Result<Int, EvalError> {
        match self {
            Node::BinaryOp { lhs, op, rhs } => {
                let mut rng = rng;
                let l = lhs.eval(gs, reborrow(&mut rng))?;
                let r = rhs.eval(gs, reborrow(&mut rng))?;
                Ok(op.apply(l, r))
            }
            Node::Divide { lhs, rhs, kind } => {
                let mut rng = rng;
                let l = lhs.eval(gs, reborrow(&mut rng))?;
                let r = rhs.eval(gs, reborrow(&mut rng))?;
                kind.calc(l, r, gs.round_type())
            }
            Node::Negate(body) => Ok(-body.eval(gs, rng)?),
            Node::Parenthesis(inner) => inner.eval(gs, rng),
            Node::Number(v) => Ok(v.clone()),
            Node::DiceRoll { times, sides, text } => {
                let t = times.eval(gs, None)?;
                let s = sides.eval(gs, None)?;
                let rng = require_rng(rng)?;
                let dice_list = rng.roll(t, s)?;
                let total: Int = dice_list.iter().map(|&v| Int::from(v)).sum();
                *text = Some(format!("{total}[{}]", join_i64(&dice_list)));
                Ok(total)
            }
            Node::ImplicitSidesDiceRoll { times, text } => {
                let t = times.eval(gs, None)?;
                let s = Int::from(gs.sides_implicit_d());
                let rng = require_rng(rng)?;
                let dice_list = rng.roll(t, s)?;
                let total: Int = dice_list.iter().map(|&v| Int::from(v)).sum();
                *text = Some(format!("{total}[{}]", join_i64(&dice_list)));
                Ok(total)
            }
            Node::DiceRollWithFilter {
                times,
                sides,
                n_filtering,
                filter,
                text,
            } => {
                let t = times.eval(gs, None)?;
                let s = match sides {
                    Some(node) => node.eval(gs, None)?,
                    None => Int::from(gs.sides_implicit_d()),
                };
                let n = n_filtering.eval(gs, None)?;
                let rng = require_rng(rng)?;
                let mut sorted_values = rng.roll(t, s)?;
                sorted_values.sort_unstable();
                let total: Int = filter
                    .apply(&sorted_values, &n)
                    .iter()
                    .map(|&v| Int::from(v))
                    .sum();
                *text = Some(format!("{total}[{}]", join_i64(&sorted_values)));
                Ok(total)
            }
        }
    }

    /// Ruby `#expr(game_system)`。ダイス部分を `2D6` 形式で再構築する
    /// （回数・面数を再評価するがロールはしない）。
    fn expr(&mut self, gs: &dyn GameSystem) -> Result<String, EvalError> {
        Ok(match self {
            Node::BinaryOp { lhs, op, rhs } => {
                format!("{}{}{}", lhs.expr(gs)?, op.as_str(), rhs.expr(gs)?)
            }
            Node::Divide { lhs, rhs, kind } => format!(
                "{}/{}{}",
                lhs.expr(gs)?,
                rhs.expr(gs)?,
                kind.rounding_method()
            ),
            Node::Negate(body) => format!("-{}", body.expr(gs)?),
            Node::Parenthesis(inner) => format!("({})", inner.expr(gs)?),
            Node::Number(v) => v.to_string(),
            Node::DiceRoll { times, sides, .. } => {
                format!("{}D{}", times.eval(gs, None)?, sides.eval(gs, None)?)
            }
            Node::ImplicitSidesDiceRoll { times, .. } => {
                format!("{}D{}", times.eval(gs, None)?, gs.sides_implicit_d())
            }
            Node::DiceRollWithFilter {
                times,
                sides,
                n_filtering,
                filter,
                ..
            } => {
                let t = times.eval(gs, None)?;
                let s = match sides {
                    Some(node) => node.eval(gs, None)?,
                    None => gs.sides_implicit_d().into(),
                };
                let n = n_filtering.eval(gs, None)?;
                format!("{}D{}{}{}", t, s, filter.abbr(), n)
            }
        })
    }

    /// Ruby `#output`。ダイスロード済みの結果を含むメッセージ表現。
    ///
    /// ダイスノードの `@text` は評価前は `nil`。`Command#eval` が必ず先に
    /// 評価してから参照するため、ここでは空文字列にフォールバックしている。
    fn output(&self) -> String {
        match self {
            Node::BinaryOp { lhs, op, rhs } => {
                format!("{}{}{}", lhs.output(), op.as_str(), rhs.output())
            }
            Node::Divide { lhs, rhs, kind } => format!(
                "{}/{}{}",
                lhs.output(),
                rhs.output(),
                kind.rounding_method()
            ),
            Node::Negate(body) => format!("-{}", body.output()),
            Node::Parenthesis(inner) => format!("({})", inner.output()),
            Node::Number(v) => v.to_string(),
            Node::DiceRoll { text, .. }
            | Node::ImplicitSidesDiceRoll { text, .. }
            | Node::DiceRollWithFilter { text, .. } => text.clone().unwrap_or_default(),
        }
    }
}

fn reborrow<'x, 'a, 'r>(
    rng: &'x mut Option<&mut AddDiceRandomizer<'a, 'r>>,
) -> Option<&'x mut AddDiceRandomizer<'a, 'r>> {
    rng.as_mut().map(|r| &mut **r)
}

fn require_rng<'x, 'a, 'r>(
    rng: Option<&'x mut AddDiceRandomizer<'a, 'r>>,
) -> Result<&'x mut AddDiceRandomizer<'a, 'r>, EvalError> {
    rng.ok_or(EvalError::Internal(
        "AddDice: dice node evaluated without randomizer",
    ))
}

fn join_i64(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// 目標値のノード。Ruby文法の `target: add | QUESTION`。
#[derive(Debug, Clone, PartialEq, Eq)]
enum TargetNode {
    Expr(Node),
    /// Ruby `Node::UndecidedTarget`（`"?"`）
    Undecided,
}

impl TargetNode {
    fn include_dice(&self) -> bool {
        match self {
            TargetNode::Expr(node) => node.include_dice(),
            TargetNode::Undecided => false,
        }
    }
}

/// ダイスロールを記録するランダマイザ。Ruby `AddDice::Randomizer`。
pub struct AddDiceRandomizer<'a, 'r> {
    source: &'a mut Randomizer<'r>,
    game_system: &'a dyn GameSystem,
    /// `(sides, value)` の列。Ruby `RandResult = Struct.new(:sides, :value)`。
    rand_results: Vec<(i64, i64)>,
}

impl<'a, 'r> AddDiceRandomizer<'a, 'r> {
    pub fn new(source: &'a mut Randomizer<'r>, game_system: &'a dyn GameSystem) -> Self {
        Self {
            source,
            game_system,
            rand_results: Vec::new(),
        }
    }

    pub fn rand_results(&self) -> &[(i64, i64)] {
        &self.rand_results
    }

    /// Ruby `AddDice::Randomizer#roll`。
    ///
    /// `Array.new(times) { ... }` は Ruby では `times` が負のとき ArgumentError に
    /// なるが、TOMLテストに該当ケースはない。ここでは空配列として扱う。
    pub fn roll(&mut self, times: Int, sides: Int) -> Result<Vec<i64>, EvalError> {
        let times_i64 = sat_i64(&times);
        let sides_i64 = sat_i64(&sides);
        let mut dice_list = if sides == 66.into() {
            let mut v = Vec::new();
            for _ in 0..times_i64.max(0) {
                v.push(self.source.roll_d66(self.game_system.d66_sort_type())?);
            }
            v
        } else if sides == 9.into() && self.game_system.enabled_d9() {
            let mut v = Vec::new();
            for _ in 0..times_i64.max(0) {
                v.push(self.source.roll_d9()?);
            }
            v
        } else {
            self.source.roll_barabara(times_i64, sides_i64)?
        };

        if self.game_system.sort_add_dice() {
            dice_list.sort_unstable();
        }

        self.rand_results
            .extend(dice_list.iter().map(|v| (sides_i64, *v)));

        Ok(dice_list)
    }
}

/// 加算ロールコマンド。Ruby `AddDice::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    lhs: Node,
    cmp_op: Option<CmpOp>,
    rhs: Option<TargetNode>,
}

impl Command {
    /// Ruby `Node::Command#eval`。
    pub fn eval(
        &mut self,
        gs: &dyn GameSystem,
        rng: &mut Randomizer,
    ) -> Result<EvalResult, EvalError> {
        let mut add_rng = AddDiceRandomizer::new(rng, gs);
        let total = self.lhs.eval(gs, Some(&mut add_rng))?;

        // Ruby: unless randomizer.rand_results.size <= 1 && @lhs.is_a?(Node::DiceRoll)
        let interrim_expr = if add_rng.rand_results().len() <= 1 && self.lhs.is_dice_roll_class() {
            None
        } else {
            Some(self.lhs.output())
        };

        let rand_results = add_rng.rand_results().to_vec();

        let check = match (self.cmp_op, &self.rhs) {
            (Some(op), Some(rhs)) => {
                let target = match rhs {
                    TargetNode::Undecided => Target::Question,
                    TargetNode::Expr(node) => {
                        // ダイスを含まないことは文法で保証済み
                        Target::Number(node.clone().eval(gs, None)?)
                    }
                };
                // Ruby も `check_result` の中では `AddDice::Randomizer` ではなく
                // ゲームシステムの `@randomizer` を使う（ここで振ったダイスは
                // `rand_results` に入らない）。`add_rng` の借用はここで既に切れている。
                gs.check_result(total.clone(), &rand_results, op, target, rng)?
            }
            _ => None,
        };

        // Ruby: sequence の `result&.text` は Result.new のとき nil（compactで落ちる）
        let text_part = check.as_ref().map(|r| r.text.clone());
        let mut result = check.unwrap_or_default();

        let expr = self.expr(gs)?;
        let mut sequence = vec![format!("({expr})")];
        if let Some(i) = interrim_expr {
            sequence.push(i);
        }
        sequence.push(total.to_string());
        if let Some(t) = text_part {
            sequence.push(t);
        }

        result.secret = self.secret;
        result.text = sequence.join(" ＞ ");
        Ok(result)
    }

    /// Ruby `Node::Command#expr`。
    fn expr(&mut self, gs: &dyn GameSystem) -> Result<String, EvalError> {
        let lhs = self.lhs.expr(gs)?;
        let cmp_op_text = match self.cmp_op {
            // Ruby `#cmp_op_text`
            Some(CmpOp::Ne) => "<>".to_string(),
            Some(CmpOp::Eq) => "=".to_string(),
            Some(other) => other.symbol_str().to_string(),
            None => String::new(),
        };
        let rhs_text = match &self.rhs {
            None => String::new(),
            Some(TargetNode::Undecided) => "?".to_string(),
            Some(TargetNode::Expr(node)) => node.clone().eval(gs, None)?.to_string(),
        };
        Ok(format!("{lhs}{cmp_op_text}{rhs_text}"))
    }
}

// ---------------------------------------------------------------------------
// パーサ（add_dice/parser.y の再帰下降移植）
// ---------------------------------------------------------------------------

/// Ruby `AddDice::Parser.parse(source)`。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    // secret: /* none */ | S
    let secret = cur.accept_sym("S");

    let lhs = parse_add(&mut cur)?;

    let (cmp_op, rhs) = match cur.peek() {
        Some(Tok::CmpOp(op)) => {
            let op = *op;
            cur.advance();
            let rhs = parse_target(&mut cur)?;
            // command: secret add CMP_OP target
            //   raise ParseError if !lhs.include_dice? || rhs.include_dice? || cmp_op.nil?
            if rhs.include_dice() {
                return None;
            }
            (Some(op?), Some(rhs))
        }
        // command: secret add
        _ => (None, None),
    };

    if !lhs.include_dice() {
        return None;
    }
    if !cur.at_eof() {
        return None;
    }

    Some(Command {
        secret,
        lhs,
        cmp_op,
        rhs,
    })
}

/// `target: add | QUESTION`。
fn parse_target(cur: &mut Cursor) -> Option<TargetNode> {
    if cur.accept(&Tok::Question) {
        Some(TargetNode::Undecided)
    } else {
        parse_add(cur).map(TargetNode::Expr)
    }
}

/// `add: add PLUS mul | add MINUS mul | mul`。
fn parse_add(cur: &mut Cursor) -> Option<Node> {
    let mut lhs = parse_mul(cur)?;
    loop {
        let op = if cur.accept(&Tok::Plus) {
            BinOp::Add
        } else if cur.accept(&Tok::Minus) {
            BinOp::Sub
        } else {
            return Some(lhs);
        };
        let rhs = parse_mul(cur)?;
        let (op, rhs) = expand_negate(op, rhs);
        lhs = Node::BinaryOp {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        };
    }
}

/// Ruby `#expand_negate`。加減算の右辺が負数である場合に加減算を逆転させる。
fn expand_negate(op: BinOp, rhs: Node) -> (BinOp, Node) {
    if let Node::Negate(body) = rhs {
        match op {
            BinOp::Add => return (BinOp::Sub, *body),
            BinOp::Sub => return (BinOp::Add, *body),
            BinOp::Mul => return (BinOp::Mul, Node::Negate(body)),
        }
    }
    (op, rhs)
}

/// `mul: mul ASTERISK unary | mul SLASH unary round_type | unary`。
fn parse_mul(cur: &mut Cursor) -> Option<Node> {
    let mut lhs = parse_unary(cur)?;
    loop {
        if cur.accept(&Tok::Asterisk) {
            let rhs = parse_unary(cur)?;
            lhs = Node::BinaryOp {
                lhs: Box::new(lhs),
                op: BinOp::Mul,
                rhs: Box::new(rhs),
            };
        } else if cur.accept(&Tok::Slash) {
            let rhs = parse_unary(cur)?;
            let kind = parse_round_type(cur);
            lhs = Node::Divide {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                kind,
            };
        } else {
            return Some(lhs);
        }
    }
}

/// `round_type: /* none */ | F | U | C | R`。
fn parse_round_type(cur: &mut Cursor) -> DivideKind {
    if cur.accept_sym("F") {
        DivideKind::RoundingDown
    } else if cur.accept_sym("U") || cur.accept_sym("C") {
        DivideKind::RoundingUp
    } else if cur.accept_sym("R") {
        DivideKind::RoundingOff
    } else {
        DivideKind::GameSystemDefault
    }
}

/// `unary: PLUS unary | MINUS unary | dice`。
///
/// MINUS の際、対象が既に `Negate` なら二重否定を解除する（原典どおり）。
fn parse_unary(cur: &mut Cursor) -> Option<Node> {
    if cur.accept(&Tok::Plus) {
        parse_unary(cur)
    } else if cur.accept(&Tok::Minus) {
        let body = parse_unary(cur)?;
        Some(match body {
            Node::Negate(inner) => *inner,
            other => Node::Negate(Box::new(other)),
        })
    } else {
        parse_dice(cur)
    }
}

/// `dice` 規則一式。
///
/// LALRでの分岐条件:
/// - `term D` の後、`NUMBER`/`PARENL` なら面数の `term`、`K`/`D`/`M` なら
///   面数省略で `filter`、それ以外なら `dice: term D`（面数省略ダイス）。
///   （FOLLOW(dice) と FIRST(filter) は互いに素なのでコンフリクトしない）
/// - 先頭が `D` なら `dice: D term [filter]`（回数1）。
fn parse_dice(cur: &mut Cursor) -> Option<Node> {
    if cur.accept_sym("D") {
        // dice: D term | D term filter
        let sides = parse_term(cur)?;
        if sides.include_dice() {
            return None;
        }
        // raise ParseError if sides.instance_of?(Node::Number) && sides.literal == 66
        if matches!(sides, Node::Number(ref n) if n == &66.into()) {
            return None;
        }

        if peek_starts_filter(cur) {
            let (filter, n_filtering) = parse_filter(cur)?;
            if n_filtering.include_dice() {
                return None;
            }
            return Some(Node::DiceRollWithFilter {
                times: Box::new(Node::Number(1.into())),
                sides: Some(Box::new(sides)),
                n_filtering: Box::new(n_filtering),
                filter,
                text: None,
            });
        }

        return Some(Node::DiceRoll {
            times: Box::new(Node::Number(1.into())),
            sides: Box::new(sides),
            text: None,
        });
    }

    let times = parse_term(cur)?;
    if !cur.accept_sym("D") {
        // dice: term
        return Some(times);
    }

    if cur.peek_starts_term() {
        let sides = parse_term(cur)?;
        if peek_starts_filter(cur) {
            // dice: term D explicit_or_implicit_sides filter （面数あり）
            let (filter, n_filtering) = parse_filter(cur)?;
            if times.include_dice() || sides.include_dice() || n_filtering.include_dice() {
                return None;
            }
            return Some(Node::DiceRollWithFilter {
                times: Box::new(times),
                sides: Some(Box::new(sides)),
                n_filtering: Box::new(n_filtering),
                filter,
                text: None,
            });
        }
        // dice: term D term
        if times.include_dice() || sides.include_dice() {
            return None;
        }
        return Some(Node::DiceRoll {
            times: Box::new(times),
            sides: Box::new(sides),
            text: None,
        });
    }

    if peek_starts_filter(cur) {
        // dice: term D explicit_or_implicit_sides filter （面数省略）
        let (filter, n_filtering) = parse_filter(cur)?;
        if times.include_dice() || n_filtering.include_dice() {
            return None;
        }
        return Some(Node::DiceRollWithFilter {
            times: Box::new(times),
            sides: None,
            n_filtering: Box::new(n_filtering),
            filter,
            text: None,
        });
    }

    // dice: term D
    if times.include_dice() {
        return None;
    }
    Some(Node::ImplicitSidesDiceRoll {
        times: Box::new(times),
        text: None,
    })
}

/// FIRST(filter) = { K, D, M }。
fn peek_starts_filter(cur: &Cursor) -> bool {
    cur.peek_is_sym("K") || cur.peek_is_sym("D") || cur.peek_is_sym("M")
}

/// `filter: filter_type term | filter_type_with_shorthand`。
///
/// 略記（`MAX` / `MIN`）は `filter_type_with_shorthand` 経由なので個数の `term` を
/// 取らない（`3D6MAX2` は構文エラー）。
fn parse_filter(cur: &mut Cursor) -> Option<(Filter, Node)> {
    if cur.accept_sym("M") {
        // filter_shorthand: M A X | M I N
        let filter = if cur.accept_sym("A") {
            if !cur.accept_sym("X") {
                return None;
            }
            Filter::KeepHighest
        } else if cur.accept_sym("I") {
            if !cur.accept_sym("N") {
                return None;
            }
            Filter::KeepLowest
        } else {
            return None;
        };
        return Some((filter, Node::Number(1.into())));
    }

    let filter = if cur.accept_sym("K") {
        if cur.accept_sym("H") {
            Filter::KeepHighest
        } else if cur.accept_sym("L") {
            Filter::KeepLowest
        } else {
            return None;
        }
    } else if cur.accept_sym("D") {
        if cur.accept_sym("H") {
            Filter::DropHighest
        } else if cur.accept_sym("L") {
            Filter::DropLowest
        } else {
            return None;
        }
    } else {
        return None;
    };

    if cur.peek_starts_term() {
        let n = parse_term(cur)?;
        Some((filter, n))
    } else {
        Some((filter, Node::Number(1.into())))
    }
}

/// `term: PARENL add PARENR | NUMBER`。カッコは `Parenthesis` で保持する。
fn parse_term(cur: &mut Cursor) -> Option<Node> {
    if cur.accept(&Tok::ParenL) {
        let inner = parse_add(cur)?;
        if !cur.accept(&Tok::ParenR) {
            return None;
        }
        Some(Node::Parenthesis(Box::new(inner)))
    } else if let Some(Tok::Number(n)) = cur.peek() {
        let n = n.clone();
        cur.advance();
        Some(Node::Number(n))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_dice_expressions() {
        assert!(parse("134").is_none());
        assert!(parse("1+2*3/4").is_none());
    }

    #[test]
    fn rejects_d66_without_times() {
        assert!(parse("D66").is_none());
        assert!(parse("D66A").is_none());
        // 回数指定があれば通る
        assert!(parse("2D66").is_some());
    }

    #[test]
    fn rejects_trailing_tokens() {
        assert!(parse("2D6>12?a").is_none());
        assert!(parse("1D6/3x").is_none());
        assert!(parse("3D6MAX2").is_none());
    }

    #[test]
    fn parses_filters() {
        assert!(parse("5D10KH3").is_some());
        assert!(parse("5DKH").is_some());
        assert!(parse("5DDH3").is_some());
        assert!(parse("3D6MAX").is_some());
        assert!(parse("3D6MIN").is_some());
        assert!(parse("5DK").is_none());
    }
}
