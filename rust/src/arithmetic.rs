//! 四則演算の構文木と評価器。
//!
//! Ruby `lib/bcdice/arithmetic/node.rb` / `lib/bcdice/arithmetic/parser.y` /
//! `lib/bcdice/arithmetic.rb` の移植。
//!
//! # 文法エンジンの選定
//!
//! lalrpop 等のパーサジェネレータは使わず、**手書きの再帰下降＋左結合ループ**で
//! Racc文法の受理言語を再現する。理由:
//!
//! - BCDiceのRacc文法は `expect 2`（`Command::Parser`）のように意図的な
//!   shift/reduce コンフリクトとyaccの既定解決（shift優先）に依存している。
//!   lalrpop向けに文法を非コンフリクト化すると、Raccの解決結果と一致する保証がなくなる。
//! - 各文法は数十規則と小さく、LALRの状態遷移を手で追って再現できる規模である。
//!
//! そのため各パーサは「1つでも構文エラーがあれば全体を `None`」とし、
//! **末尾までトークンを消費しきったこと（`$end` 到達）を必ず検査する**。
//! Ruby側の `rescue ParseError -> nil` と同一の挙動になる。

use num_traits::{FromPrimitive, ToPrimitive, Zero};

use crate::common_command::lexer::{self, Cursor, Tok};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::Int;

/// 二項演算子（除算を除く）。Ruby側は `:+` `:-` `:*` のシンボル。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
}

impl ArithOp {
    /// Rubyの `"#{@op}"`（Symbol#to_s）相当。
    pub fn as_str(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
        }
    }

    pub(crate) fn apply(self, lhs: Int, rhs: Int) -> Int {
        match self {
            ArithOp::Add => lhs + rhs,
            ArithOp::Sub => lhs - rhs,
            ArithOp::Mul => lhs * rhs,
        }
    }
}

/// 除算ノードの端数処理方法。Ruby `Arithmetic::Node::DivideWith*` の各クラス。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivideKind {
    /// `DivideWithGameSystemDefault`（記号なし）
    GameSystemDefault,
    /// `DivideWithCeil`（記号 `C`）
    Ceil,
    /// `DivideWithRound`（記号 `R`）
    Round,
    /// `DivideWithFloor`（記号 `F`）
    Floor,
}

impl DivideKind {
    /// Ruby `ROUNDING_METHOD`。`Arithmetic` 側の切り上げ記号は `"C"`
    /// （`AddDice` 側は `"U"` なので注意）。
    pub fn rounding_method(self) -> &'static str {
        match self {
            DivideKind::GameSystemDefault => "",
            DivideKind::Ceil => "C",
            DivideKind::Round => "R",
            DivideKind::Floor => "F",
        }
    }
}

/// Ruby `Integer#/`（床除算）。Rustの `/` は0方向丸めなので一致しない。
///
/// `i64::div_euclid` は剰余を非負にする除算なので**別物**（`7 / -2` は
/// Rubyでは `-4`、`div_euclid` では `-3`）。ここは自前で床方向に補正する。
pub(crate) fn floor_div(dividend: Int, divisor: Int) -> Int {
    let q = &dividend / &divisor;
    let rem = &dividend % &divisor;
    if !rem.is_zero() && ((dividend < Int::ZERO) != (divisor < Int::ZERO)) {
        q - 1
    } else {
        q
    }
}

/// Ruby `(dividend.to_f / divisor).ceil`。
///
/// 本家 `node.rb` と同様に **常に to_f 経路**で計算する。`Int`（BigInt）を
/// `to_f64()` で f64 へ丸めてから除算・`ceil` するため、2^53 超の入力では
/// Ruby と同一の丸め落ちが発生する（等価性優先・意図的な挙動）。
///
/// 除数0のときRubyは `Float::INFINITY.ceil` で FloatDomainError を送出する。
/// また、f64 への変換結果が無限大のとき（`10^400` 等の f64 最大超入力）も
/// 本家同様 FloatDomainError 相当を返す。
pub(crate) fn ceil_div(dividend: Int, divisor: Int) -> Result<Int, EvalError> {
    let d = dividend.to_f64().ok_or(EvalError::FloatDomain)?;
    let v = divisor.to_f64().ok_or(EvalError::FloatDomain)?;
    let q = d / v;
    if q.is_infinite() || q.is_nan() {
        return Err(EvalError::FloatDomain);
    }
    // f64 の .ceil() は巨大な値で self をそのまま返すので、
    // BigInt への再変換で丸め結果を再現する。
    let c = q.ceil();
    Int::from_f64(c).ok_or(EvalError::FloatDomain)
}

/// Ruby `(dividend.to_f / divisor).round`。
///
/// 本家 `node.rb` と同様に **常に to_f 経路**で計算する。`Int`（BigInt）を
/// `to_f64()` で f64 へ丸めてから除算・`round` するため、2^53 超の入力では
/// Ruby と同一の丸め落ちが発生する（等価性優先・意図的な挙動）。
/// f64 の `round()` は half away from zero なので本家と一致する。
///
/// 除数0のときRubyは `Float::INFINITY.round` で FloatDomainError を送出する。
/// また、f64 への変換結果が無限大のとき（`10^400` 等の f64 最大超入力）も
/// 本家同様 FloatDomainError 相当を返す。
pub(crate) fn round_div(dividend: Int, divisor: Int) -> Result<Int, EvalError> {
    let d = dividend.to_f64().ok_or(EvalError::FloatDomain)?;
    let v = divisor.to_f64().ok_or(EvalError::FloatDomain)?;
    let q = d / v;
    if q.is_infinite() || q.is_nan() {
        return Err(EvalError::FloatDomain);
    }
    // f64 の .round() は巨大な値で self をそのまま返すので、
    // BigInt への再変換で丸め結果を再現する。
    let r = q.round();
    Int::from_f64(r).ok_or(EvalError::FloatDomain)
}

/// Ruby `Integer#/` そのもの。除数0で ZeroDivisionError。
pub(crate) fn ruby_div(dividend: Int, divisor: Int) -> Result<Int, EvalError> {
    if divisor.is_zero() {
        return Err(EvalError::ZeroDivision);
    }
    Ok(floor_div(dividend, divisor))
}

/// 四則演算の構文木。Ruby `Arithmetic::Node::*`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    BinaryOp {
        lhs: Box<Node>,
        op: ArithOp,
        rhs: Box<Node>,
    },
    Divide {
        lhs: Box<Node>,
        rhs: Box<Node>,
        kind: DivideKind,
    },
    Negative(Box<Node>),
    Parenthesis(Box<Node>),
    Number(Int),
}

impl Node {
    /// ノードを評価する。Ruby `#eval(round_type)`。
    pub fn eval(&self, round_type: RoundType) -> Result<Int, EvalError> {
        match self {
            Node::BinaryOp { lhs, op, rhs } => {
                let l = lhs.eval(round_type)?;
                let r = rhs.eval(round_type)?;
                Ok(op.apply(l, r))
            }
            Node::Divide { lhs, rhs, kind } => {
                let l = lhs.eval(round_type)?;
                let r = rhs.eval(round_type)?;
                // Arithmetic側の DivideBase は AddDice と違い除数0を特別扱いしない。
                match kind {
                    DivideKind::GameSystemDefault => match round_type {
                        RoundType::Ceil => ceil_div(l, r),
                        RoundType::Round => round_div(l, r),
                        RoundType::Floor => ruby_div(l, r),
                    },
                    DivideKind::Ceil => ceil_div(l, r),
                    DivideKind::Round => round_div(l, r),
                    DivideKind::Floor => ruby_div(l, r),
                }
            }
            Node::Negative(body) => Ok(-body.eval(round_type)?),
            Node::Parenthesis(expr) => expr.eval(round_type),
            Node::Number(v) => Ok(v.clone()),
        }
    }

    /// メッセージへの出力。Ruby `#output`。
    pub fn output(&self) -> String {
        match self {
            Node::BinaryOp { lhs, op, rhs } => {
                format!("{}{}{}", lhs.output(), op.as_str(), rhs.output())
            }
            Node::Divide { lhs, rhs, kind } => {
                format!(
                    "{}/{}{}",
                    lhs.output(),
                    rhs.output(),
                    kind.rounding_method()
                )
            }
            Node::Negative(body) => format!("-{}", body.output()),
            Node::Parenthesis(expr) => format!("({})", expr.output()),
            Node::Number(v) => v.to_string(),
        }
    }

    /// `Parenthesis` ノードか。Calcの出力整形で使う。
    pub fn is_parenthesis(&self) -> bool {
        matches!(self, Node::Parenthesis(_))
    }
}

/// `term: PARENL add PARENR` をどう扱うか。
///
/// Ruby側の `arithmetic/parser.y` および barabara / tally / reroll / upper の
/// 各parser.yは `result = val[1]`（カッコを捨てる）だが、`calc/parser.y` だけは
/// `Arithmetic::Node::Parenthesis.new(val[1])` で包む。出力文字列が変わるので区別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParenMode {
    /// カッコを捨てる（`arithmetic` / `barabara` / `tally` / `reroll` / `upper`）
    Drop,
    /// `Parenthesis` ノードで包む（`calc`）
    Keep,
}

/// `add` を解析する。Ruby文法の `add: add PLUS mul | add MINUS mul | mul`。
pub fn parse_add(cur: &mut Cursor, mode: ParenMode) -> Option<Node> {
    let mut lhs = parse_mul(cur, mode)?;
    loop {
        if cur.accept(&Tok::Plus) {
            let rhs = parse_mul(cur, mode)?;
            lhs = Node::BinaryOp {
                lhs: Box::new(lhs),
                op: ArithOp::Add,
                rhs: Box::new(rhs),
            };
        } else if cur.accept(&Tok::Minus) {
            let rhs = parse_mul(cur, mode)?;
            lhs = Node::BinaryOp {
                lhs: Box::new(lhs),
                op: ArithOp::Sub,
                rhs: Box::new(rhs),
            };
        } else {
            return Some(lhs);
        }
    }
}

/// `mul` を解析する。`mul: mul ASTERISK unary | mul SLASH unary round_type | unary`。
pub fn parse_mul(cur: &mut Cursor, mode: ParenMode) -> Option<Node> {
    let first = parse_unary(cur, mode)?;
    parse_mul_from(cur, mode, first)
}

/// 先頭の `unary` を外部で解析済みの場合の `mul`。
///
/// UpperDiceの `notations PLUS dice` と `modifier_expr PLUS mul` の分岐で、
/// LALRと同じく「`term` を読んでから次トークンで判定する」ために使う。
pub fn parse_mul_from(cur: &mut Cursor, mode: ParenMode, first: Node) -> Option<Node> {
    let mut lhs = first;
    loop {
        if cur.accept(&Tok::Asterisk) {
            let rhs = parse_unary(cur, mode)?;
            lhs = Node::BinaryOp {
                lhs: Box::new(lhs),
                op: ArithOp::Mul,
                rhs: Box::new(rhs),
            };
        } else if cur.accept(&Tok::Slash) {
            let rhs = parse_unary(cur, mode)?;
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

/// `round_type: /* none */ | U | C | R | F` を解析する。
///
/// `U` `C` `R` `F` が `mul` の後続になりうるのはこの位置だけなので、
/// LALRでもコンフリクトなくshiftされる（FOLLOW(mul) にこれらは含まれない）。
pub fn parse_round_type(cur: &mut Cursor) -> DivideKind {
    if cur.accept_sym("U") || cur.accept_sym("C") {
        DivideKind::Ceil
    } else if cur.accept_sym("R") {
        DivideKind::Round
    } else if cur.accept_sym("F") {
        DivideKind::Floor
    } else {
        DivideKind::GameSystemDefault
    }
}

/// `unary: PLUS unary | MINUS unary | term`。
pub fn parse_unary(cur: &mut Cursor, mode: ParenMode) -> Option<Node> {
    if cur.accept(&Tok::Plus) {
        parse_unary(cur, mode)
    } else if cur.accept(&Tok::Minus) {
        Some(Node::Negative(Box::new(parse_unary(cur, mode)?)))
    } else {
        parse_term(cur, mode)
    }
}

/// `term: PARENL add PARENR | NUMBER`。
pub fn parse_term(cur: &mut Cursor, mode: ParenMode) -> Option<Node> {
    if cur.accept(&Tok::ParenL) {
        let inner = parse_add(cur, mode)?;
        if !cur.accept(&Tok::ParenR) {
            return None;
        }
        Some(match mode {
            ParenMode::Drop => inner,
            ParenMode::Keep => Node::Parenthesis(Box::new(inner)),
        })
    } else if let Some(Tok::Number(n)) = cur.peek() {
        let n = n.clone();
        cur.advance();
        Some(Node::Number(n))
    } else {
        None
    }
}

/// Ruby `Arithmetic::Parser.parse(source)`。
pub fn parse(source: &str) -> Option<Node> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);
    let node = parse_add(&mut cur, ParenMode::Drop)?;
    cur.at_eof().then_some(node)
}

/// Ruby `Arithmetic.eval(source, round_type)`。
///
/// Ruby側は `rescue ZeroDivisionError -> nil` のみを行うので、ここでも
/// ゼロ除算だけを `Ok(None)` に畳み、それ以外（`/0C` などの FloatDomainError 相当）は
/// Ruby同様にそのまま呼び出し元へ伝播させる。
/// 「パースできない式」も `Ok(None)`。
pub fn eval(source: &str, round_type: RoundType) -> Result<Option<Int>, EvalError> {
    let node = match parse(source) {
        Some(node) => node,
        None => return Ok(None),
    };

    match node.eval(round_type) {
        Ok(v) => Ok(Some(v)),
        Err(EvalError::ZeroDivision) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(src: &str) -> Option<Int> {
        eval(src, RoundType::Floor).expect("no propagating error in these cases")
    }

    #[test]
    fn floor_div_matches_ruby() {
        // Ruby: -7 / 2 == -4, 7 / -2 == -4, -7 / -2 == 3
        assert_eq!(floor_div((-7).into(), 2.into()), (-4).into());
        assert_eq!(floor_div(7.into(), (-2).into()), (-4).into());
        assert_eq!(floor_div((-7).into(), (-2).into()), 3.into());
        assert_eq!(floor_div(7.into(), 2.into()), 3.into());
        assert_eq!(floor_div(4.into(), (-3).into()), (-2).into());
    }

    #[test]
    fn evaluates_basic_expressions() {
        assert_eq!(ev("1+2"), Some(3.into()));
        assert_eq!(ev("10/2+(5*2)-(3+1)"), Some(11.into()));
        assert_eq!(ev("-1---1"), Some((-2).into()));
        assert_eq!(ev("3*4"), Some(12.into()));
        assert_eq!(ev("1/0"), None); // ZeroDivisionError -> nil
        assert_eq!(ev("1+"), None); // ParseError -> nil
        assert_eq!(ev("1/2R"), Some(1.into()));
        assert_eq!(ev("1/3R"), Some(0.into()));
        assert_eq!(ev("1/2C"), Some(1.into()));
        assert_eq!(ev("1/2F"), Some(0.into()));
    }

    #[test]
    fn output_reproduces_source_shape() {
        let node = parse("1+4*3/2").unwrap();
        assert_eq!(node.output(), "1+4*3/2");
        let node = parse("1/2R").unwrap();
        assert_eq!(node.output(), "1/2R");
    }

    #[test]
    fn requires_end_of_input() {
        assert!(parse("1+2)").is_none());
        assert!(parse("1+2X").is_none());
    }

    /// Ruby実測値（`ruby -e 'p((2**53+1).to_f)'` 等）との一致を確認する。
    /// BigInt→f64 の丸め落ち（2^53超）も本家どおりに再現する。
    #[test]
    fn ceil_round_div_matches_ruby_float_path() {
        // Ruby: (9007199254740993.to_f / 1).ceil == 9007199254740992（2^53+1はf64で2^53に丸め落ち）
        assert_eq!(
            ceil_div(Int::from(9007199254740993i128), Int::from(1)),
            Ok(Int::from(9007199254740992i128))
        );
        assert_eq!(
            round_div(Int::from(9007199254740993i128), Int::from(1)),
            Ok(Int::from(9007199254740992i128))
        );
        // Ruby: (10**18).to_f / 7 → 1.4285714285714286e+17 → 142857142857142864
        // （正確な整数演算だと 142857142857142858 になり、差6が出る）
        let d = Int::from(10).pow(18);
        assert_eq!(
            ceil_div(d.clone(), Int::from(7)),
            Ok(Int::from(142857142857142864i128))
        );
        assert_eq!(
            round_div(d, Int::from(7)),
            Ok(Int::from(142857142857142864i128))
        );
        // Ruby: (9223372036854775809.to_f / 1).ceil == 9223372036854775808
        // （旧実装のBigInt正確演算では 9223372036854775809 になり等価性が破れていた）
        let n = Int::parse_bytes(b"9223372036854775809", 10).unwrap();
        assert_eq!(
            ceil_div(n.clone(), Int::from(1)),
            Ok(Int::parse_bytes(b"9223372036854775808", 10).unwrap())
        );
        assert_eq!(
            round_div(n, Int::from(1)),
            Ok(Int::parse_bytes(b"9223372036854775808", 10).unwrap())
        );
        // i64範囲内の通常ケース（既存挙動の維持）
        assert_eq!(ceil_div(Int::from(7), Int::from(2)), Ok(Int::from(4)));
        assert_eq!(round_div(Int::from(7), Int::from(2)), Ok(Int::from(4)));
        assert_eq!(round_div(Int::from(5), Int::from(2)), Ok(Int::from(3)));
        assert_eq!(round_div(Int::from(-5), Int::from(2)), Ok(Int::from(-3)));
        assert_eq!(ceil_div(Int::from(-7), Int::from(2)), Ok(Int::from(-3)));
    }

    /// f64最大超の入力は本家どおり FloatDomainError（Infinity.ceil がクラッシュ）。
    #[test]
    fn float_domain_error_on_overflow() {
        // 10^400 は f64 (最大約1.8e308) に変換できない → to_f が Infinity → ceil が FloatDomainError
        let big = Int::from(10).pow(400);
        assert_eq!(
            ceil_div(big.clone(), Int::from(1)),
            Err(EvalError::FloatDomain)
        );
        assert_eq!(round_div(big, Int::from(1)), Err(EvalError::FloatDomain));
        // 除数0: Float::INFINITY.ceil で FloatDomainError（本家どおり）
        assert_eq!(
            ceil_div(Int::from(1), Int::from(0)),
            Err(EvalError::FloatDomain)
        );
        assert_eq!(
            round_div(Int::from(1), Int::from(0)),
            Err(EvalError::FloatDomain)
        );
    }

    /// 評価器経由でも巨大数の丸めが本家どおりになること。
    #[test]
    fn eval_bignum_div_matches_ruby() {
        // C9223372036854775809/1 → Ruby: 9223372036854775808（f64丸め落ち）
        let r = eval("9223372036854775809/1", RoundType::Ceil).unwrap();
        assert_eq!(
            r,
            Some(Int::parse_bytes(b"9223372036854775808", 10).unwrap())
        );
        // f64最大超 → FloatDomainError が伝播（ZeroDivision とは別の経路）
        let src = format!("{}/1", Int::from(10).pow(400));
        assert!(matches!(
            eval(&src, RoundType::Ceil),
            Err(EvalError::FloatDomain)
        ));
    }
}
