//! よくある形式のコマンドのパースを補助する汎用パーサ。
//!
//! Ruby `lib/bcdice/command/parser.y` / `lib/bcdice/command/lexer.rb` /
//! `lib/bcdice/command/parsed.rb` の移植。P4のゲームシステム移植で使う。
//!
//! # `expect 2` の再現
//!
//! 原典の文法は
//!
//! ```text
//! expr: notation option modifier target      (規則1)
//!     | notation modifier option target      (規則2)
//!     | notation option target               (規則3)
//! option: /* none */ | option AT unary | option SHARP unary | option DOLLAR unary
//! modifier: PLUS mul | MINUS mul | modifier PLUS mul | modifier MINUS mul
//! ```
//!
//! で `expect 2` が宣言されている。この2件のコンフリクトは
//! **`notation` 直後の先読みが `PLUS` / `MINUS` のときの shift/reduce 衝突**である:
//!
//! - reduce: `option -> ε`（規則1・規則3のため。`modifier` の FIRST は
//!   `{PLUS, MINUS}` なので、この2トークンが `option -> ε` の先読み集合に入る）
//! - shift: `modifier -> . PLUS mul` / `. MINUS mul`（規則2のため）
//!
//! yacc/Racc の既定解決は **shift 優先**なので、`notation` の直後に `+`/`-` が来ると
//! 規則2（`notation modifier option target`）の経路に入り、空の `option` を
//! 前置で還元することはない。
//!
//! この解決が安全である最強の根拠は、**規則1と規則2のアクションが
//! どちらも `parsed(notation, option, modifier, target)` で意味的に同一**という点にある。
//! すなわち「前置 option が空のときにどちらの規則を選ぶか」は出力に影響しない。
//! だからこそ本家は `expect 2` でコンフリクトを許容できている。
//!
//! ここから、受理される語は次のとおりと決まる（オプション列は修正値の前か後ろの
//! **片側だけ**に置ける）:
//!
//! 1. `notation` を読む
//! 2. オプション列（`@`/`#`/`$` の連続）を読む
//! 3. 次が `+`/`-` なら修正値を読む
//! 4. 修正値があり、かつ (2) が空だった場合に限り、もう一度オプション列を読める
//!    （＝規則2）。(2) が空でなければ規則1なので、修正値の後のオプションは構文エラー
//! 5. 目標値（空 / `CMP_OP add` / `CMP_OP ?`）を読む
//! 6. 入力を読み切っていること（`$end`）を検査する

use crate::arithmetic::{self, Node, ParenMode};
use crate::common_command::lexer::{
    digits_to_int, scan_cmp_op, scan_digits, upcase_char, Cursor, Tok,
};
use crate::enums::RoundType;
use crate::format;
use crate::normalize::{self, CmpOp};
use regex::Regex;

/// `Parsed#to_s` でのクリティカル値などの表示位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixPosition {
    /// `:after_command`
    AfterCommand,
    /// `:after_modify_number`
    AfterModifyNumber,
    /// `:after_target_number`
    AfterTargetNumber,
}

/// パース結果。Ruby `BCDice::Command::Parsed`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub command: String,
    pub prefix_number: Option<crate::Int>,
    pub suffix_number: Option<crate::Int>,
    pub critical: Option<crate::Int>,
    pub fumble: Option<crate::Int>,
    pub dollar: Option<crate::Int>,
    pub modify_number: crate::Int,
    pub cmp_op: Option<CmpOp>,
    pub target_number: Option<crate::Int>,
    pub question_target: bool,
}

impl Parsed {
    /// Ruby `Parsed#to_s(suffix_position)`。
    ///
    /// 比較演算子は `Format.comparison_operator` ではなく **Symbol#to_s** が
    /// そのまま連結される（`:==` は `"=="`、`:!=` は `"!="`）ので注意。
    pub fn to_s(&self, suffix_position: SuffixPosition) -> String {
        let prefix = opt_str(self.prefix_number.as_ref());
        let suffix = opt_str(self.suffix_number.as_ref());
        let c = self
            .critical
            .as_ref()
            .map(|v| format!("@{v}"))
            .unwrap_or_default();
        let f = self
            .fumble
            .as_ref()
            .map(|v| format!("#{v}"))
            .unwrap_or_default();
        let d = self
            .dollar
            .as_ref()
            .map(|v| format!("${v}"))
            .unwrap_or_default();
        let m = format::modifier(&self.modify_number);
        let cmp = self
            .cmp_op
            .map(|op| op.symbol_str().to_string())
            .unwrap_or_default();
        let target = if self.question_target {
            "?".to_string()
        } else {
            opt_str(self.target_number.as_ref())
        };

        match suffix_position {
            SuffixPosition::AfterCommand => {
                format!("{prefix}{}{suffix}{c}{f}{d}{m}{cmp}{target}", self.command)
            }
            SuffixPosition::AfterModifyNumber => {
                format!("{prefix}{}{suffix}{m}{c}{f}{d}{cmp}{target}", self.command)
            }
            SuffixPosition::AfterTargetNumber => {
                format!("{prefix}{}{suffix}{m}{cmp}{target}{c}{f}{d}", self.command)
            }
        }
    }
}

fn opt_str(v: Option<&crate::Int>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// Ruby `BCDice::Command::Lexer` の字句解析。
///
/// 走査順は原典どおり:
/// 1. コマンド表記（先頭一致）。String指定は `Regexp.new` されるので正規表現扱い
/// 2. `\d+` → NUMBER
/// 3. `[<>!=]+` → 正規化。失敗したら ILLEGAL
/// 4. 1文字取り出して大文字化 → 記号表か、それ以外は文字トークン
fn lex(source: &str, notations: &[Regex]) -> Vec<Tok> {
    let source = crate::common_command::lexer::first_word(source);
    let mut tokens = Vec::new();
    let mut rest = source;

    'outer: while !rest.is_empty() {
        for re in notations {
            // StringScanner#scan は現在位置に**アンカーされた**マッチなので、
            // 「rest に対する最左マッチの開始位置が0であること」で再現する。
            if let Some(m) = re.find(rest) {
                if m.start() == 0 && !m.as_str().is_empty() {
                    tokens.push(Tok::Notation(m.as_str().to_string()));
                    rest = &rest[m.end()..];
                    continue 'outer;
                }
            }
        }

        if let Some(d) = scan_digits(rest) {
            tokens.push(Tok::Number(digits_to_int(d)));
            rest = &rest[d.len()..];
            continue;
        }
        if let Some(op) = scan_cmp_op(rest) {
            tokens.push(match normalize::comparison_operator(op) {
                Some(cmp) => Tok::CmpOp(Some(cmp)),
                None => Tok::Illegal,
            });
            rest = &rest[op.len()..];
            continue;
        }

        let c = rest.chars().next().expect("rest is not empty");
        rest = &rest[c.len_utf8()..];
        let upper = upcase_char(c);
        tokens.push(match upper.as_str() {
            "+" => Tok::Plus,
            "-" => Tok::Minus,
            "*" => Tok::Asterisk,
            "/" => Tok::Slash,
            "(" => Tok::ParenL,
            ")" => Tok::ParenR,
            "?" => Tok::Question,
            "@" => Tok::At,
            "#" => Tok::Sharp,
            "$" => Tok::Dollar,
            _ => Tok::Sym(upper),
        });
    }

    tokens
}

/// 汎用コマンドパーサ。Ruby `BCDice::Command::Parser`。
///
/// # Examples
///
/// ```
/// use bcdice::command_parser::Parser;
/// use bcdice::enums::RoundType;
/// use bcdice::normalize::CmpOp;
/// use num_bigint::BigInt;
///
/// let parser = Parser::new(&["MC"], RoundType::Floor).enable_critical();
/// let parsed = parser.parse("MC+2*3@30<=10/2-3").unwrap();
/// assert_eq!(parsed.command, "MC");
/// assert_eq!(parsed.modify_number, BigInt::from(6));
/// assert_eq!(parsed.critical, Some(BigInt::from(30)));
/// assert_eq!(parsed.cmp_op, Some(CmpOp::Le));
/// assert_eq!(parsed.target_number, Some(BigInt::from(2)));
/// ```
#[derive(Debug, Clone)]
pub struct Parser {
    notations: Vec<Regex>,
    round_type: RoundType,
    prefix_number: bool,
    suffix_number: bool,
    need_prefix_number: bool,
    need_suffix_number: bool,
    modifier: bool,
    critical: bool,
    fumble: bool,
    dollar: bool,
    allowed_cmp_op: Vec<Option<CmpOp>>,
    question_target: bool,
}

impl Parser {
    /// Ruby `Parser#initialize(*notations, round_type:)`。
    ///
    /// `notations` は正規表現パターン文字列（Ruby側の String 指定は
    /// `Regexp.new` されるので、どちらも正規表現として解釈される）。
    ///
    /// # Panics
    /// パターンが正規表現としてコンパイルできない場合。
    pub fn new(notations: &[&str], round_type: RoundType) -> Self {
        Self {
            notations: notations
                .iter()
                .map(|n| Regex::new(n).expect("notation must be a valid regexp"))
                .collect(),
            round_type,
            prefix_number: false,
            suffix_number: false,
            need_prefix_number: false,
            need_suffix_number: false,
            modifier: true,
            critical: false,
            fumble: false,
            dollar: false,
            allowed_cmp_op: vec![
                None,
                Some(CmpOp::Ge),
                Some(CmpOp::Gt),
                Some(CmpOp::Le),
                Some(CmpOp::Lt),
                Some(CmpOp::Eq),
                Some(CmpOp::Ne),
            ],
            question_target: false,
        }
    }

    /// 修正値は受け付けないようにする。Ruby `#disable_modifier`。
    pub fn disable_modifier(mut self) -> Self {
        self.modifier = false;
        self
    }

    /// リテラルの前に数値を許可する。Ruby `#enable_prefix_number`。
    pub fn enable_prefix_number(mut self) -> Self {
        self.prefix_number = true;
        self
    }

    /// リテラルの後ろに数値を許可する。Ruby `#enable_suffix_number`。
    pub fn enable_suffix_number(mut self) -> Self {
        self.suffix_number = true;
        self
    }

    /// リテラルの前に数値が必要であると設定する。Ruby `#has_prefix_number`。
    pub fn has_prefix_number(mut self) -> Self {
        self.prefix_number = true;
        self.need_prefix_number = true;
        self
    }

    /// リテラルの後ろに数値が必要であると設定する。Ruby `#has_suffix_number`。
    pub fn has_suffix_number(mut self) -> Self {
        self.suffix_number = true;
        self.need_suffix_number = true;
        self
    }

    /// `@` によるクリティカル値の指定を許可する。Ruby `#enable_critical`。
    pub fn enable_critical(mut self) -> Self {
        self.critical = true;
        self
    }

    /// `#` によるファンブル値の指定を許可する。Ruby `#enable_fumble`。
    pub fn enable_fumble(mut self) -> Self {
        self.fumble = true;
        self
    }

    /// `$` による値の指定を許可する。Ruby `#enable_dollar`。
    pub fn enable_dollar(mut self) -> Self {
        self.dollar = true;
        self
    }

    /// 使用できる比較演算子を制限する。Ruby `#restrict_cmp_op_to`。
    ///
    /// 目標値未入力を許可する場合には `None` を含めること。
    pub fn restrict_cmp_op_to(mut self, ops: &[Option<CmpOp>]) -> Self {
        self.allowed_cmp_op = ops.to_vec();
        self
    }

    /// 目標値 `"?"` の指定を許可する。Ruby `#enable_question_target`。
    pub fn enable_question_target(mut self) -> Self {
        self.question_target = true;
        self
    }

    /// Ruby `Parser#parse(source)`。
    ///
    /// Ruby側は `rescue ParseError, ZeroDivisionError -> nil`。
    /// FloatDomainError（`@1/0C` 等）はRubyでは rescue されずクラッシュするが、
    /// ここでは `None`（パース失敗）に畳んでいる。本家のクラッシュを再現する意味が
    /// 無く、呼び出し側（P4のゲームシステム）の契約が `Parsed | nil` であるため。
    pub fn parse(&self, source: &str) -> Option<Parsed> {
        let tokens = lex(source, &self.notations);
        let mut cur = Cursor::new(&tokens);

        // 1. notation
        let notation = self.parse_notation(&mut cur)?;

        // 2. オプション列
        let mut options = Options::default();
        let had_leading_options = self.parse_options(&mut cur, &mut options)?;

        // 3. 修正値
        let modifier = if matches!(cur.peek(), Some(Tok::Plus) | Some(Tok::Minus)) {
            Some(self.parse_modifier(&mut cur)?)
        } else {
            None
        };

        // 4. 規則2（前置オプションが空だったときだけ、修正値の後ろにオプションを置ける）
        if modifier.is_some() && !had_leading_options {
            self.parse_options(&mut cur, &mut options)?;
        }

        // 規則1・規則2の `raise ParseError unless @modifier`
        if modifier.is_some() && !self.modifier {
            return None;
        }

        // 5. 目標値
        let target = self.parse_target(&mut cur)?;

        // 6. $end
        if !cur.at_eof() {
            return None;
        }

        // Ruby `#parsed`
        let round_type = self.round_type;
        let eval_opt = |node: &Option<Node>| -> Option<Option<crate::Int>> {
            match node {
                Some(n) => match n.eval(round_type) {
                    Ok(v) => Some(Some(v)),
                    // Ruby: rescue ZeroDivisionError -> nil、FloatDomainも同様に扱う
                    Err(_) => None,
                },
                None => Some(None),
            }
        };

        let prefix_number = eval_opt(&notation.prefix)?;
        let suffix_number = eval_opt(&notation.suffix)?;
        let critical = eval_opt(&options.critical)?;
        let fumble = eval_opt(&options.fumble)?;
        let dollar = eval_opt(&options.dollar)?;
        let modify_number = match modifier {
            Some(node) => node.eval(round_type).ok()?,
            // 規則3: Arithmetic::Node::Number.new(0)
            None => 0.into(),
        };

        let (question_target, target_number) = match target.target {
            Some(TargetValue::Question) => (true, Some(0.into())),
            Some(TargetValue::Expr(node)) => (false, Some(node.eval(round_type).ok()?)),
            None => (false, None),
        };

        Some(Parsed {
            command: notation.command,
            prefix_number,
            suffix_number,
            critical,
            fumble,
            dollar,
            modify_number,
            cmp_op: target.cmp_op,
            target_number,
            question_target,
        })
    }

    /// `notation` 規則一式。
    fn parse_notation(&self, cur: &mut Cursor) -> Option<Notation> {
        // notation: NOTATION [term] | term NOTATION [term]
        let prefix = if let Some(Tok::Notation(_)) = cur.peek() {
            None
        } else {
            Some(arithmetic::parse_term(cur, ParenMode::Drop)?)
        };

        let command = match cur.peek() {
            Some(Tok::Notation(s)) => {
                let s = s.clone();
                cur.advance();
                s
            }
            _ => return None,
        };

        // NOTATION の直後に term が来たら必ず suffix として読む（他に還元先がない）
        let suffix = if cur.peek_starts_term() {
            Some(arithmetic::parse_term(cur, ParenMode::Drop)?)
        } else {
            None
        };

        // 各規則のアクションにある検査
        match (prefix.is_some(), suffix.is_some()) {
            (true, true) => {
                if !(self.prefix_number && self.suffix_number) {
                    return None;
                }
            }
            (true, false) => {
                if !self.prefix_number || self.need_suffix_number {
                    return None;
                }
            }
            (false, true) => {
                if !self.suffix_number || self.need_prefix_number {
                    return None;
                }
            }
            (false, false) => {
                if self.need_prefix_number || self.need_suffix_number {
                    return None;
                }
            }
        }

        Some(Notation {
            command,
            prefix,
            suffix,
        })
    }

    /// `option` 規則。1つ以上読めたら `Some(true)`。
    ///
    /// 各アクションの `raise ParseError unless @critical && option[:critical].nil?`
    /// （有効化されていない／既に指定済みならエラー）を再現する。
    fn parse_options(&self, cur: &mut Cursor, options: &mut Options) -> Option<bool> {
        let mut found = false;
        loop {
            let slot = match cur.peek() {
                Some(Tok::At) => OptionSlot::Critical,
                Some(Tok::Sharp) => OptionSlot::Fumble,
                Some(Tok::Dollar) => OptionSlot::Dollar,
                _ => return Some(found),
            };
            cur.advance();

            let (enabled, target) = match slot {
                OptionSlot::Critical => (self.critical, &mut options.critical),
                OptionSlot::Fumble => (self.fumble, &mut options.fumble),
                OptionSlot::Dollar => (self.dollar, &mut options.dollar),
            };
            if !enabled || target.is_some() {
                return None;
            }

            *target = Some(arithmetic::parse_unary(cur, ParenMode::Drop)?);
            found = true;
        }
    }

    /// `modifier: PLUS mul | MINUS mul | modifier PLUS mul | modifier MINUS mul`。
    fn parse_modifier(&self, cur: &mut Cursor) -> Option<Node> {
        let mut node = if cur.accept(&Tok::Plus) {
            arithmetic::parse_mul(cur, ParenMode::Drop)?
        } else if cur.accept(&Tok::Minus) {
            Node::Negative(Box::new(arithmetic::parse_mul(cur, ParenMode::Drop)?))
        } else {
            return None;
        };

        loop {
            let op = if cur.accept(&Tok::Plus) {
                arithmetic::ArithOp::Add
            } else if cur.accept(&Tok::Minus) {
                arithmetic::ArithOp::Sub
            } else {
                return Some(node);
            };
            let rhs = arithmetic::parse_mul(cur, ParenMode::Drop)?;
            node = Node::BinaryOp {
                lhs: Box::new(node),
                op,
                rhs: Box::new(rhs),
            };
        }
    }

    /// `target: /* none */ | CMP_OP add | CMP_OP QUESTION`。
    fn parse_target(&self, cur: &mut Cursor) -> Option<TargetPart> {
        let cmp_op = match cur.peek() {
            Some(Tok::CmpOp(op)) => {
                let op = *op;
                cur.advance();
                op
            }
            _ => {
                // target: /* none */
                if !self.allowed_cmp_op.contains(&None) {
                    return None;
                }
                return Some(TargetPart {
                    cmp_op: None,
                    target: None,
                });
            }
        };

        if !self.allowed_cmp_op.contains(&cmp_op) {
            return None;
        }

        if cur.accept(&Tok::Question) {
            if !self.question_target {
                return None;
            }
            return Some(TargetPart {
                cmp_op,
                target: Some(TargetValue::Question),
            });
        }

        let node = arithmetic::parse_add(cur, ParenMode::Drop)?;
        Some(TargetPart {
            cmp_op,
            target: Some(TargetValue::Expr(node)),
        })
    }
}

struct Notation {
    command: String,
    prefix: Option<Node>,
    suffix: Option<Node>,
}

#[derive(Default)]
struct Options {
    critical: Option<Node>,
    fumble: Option<Node>,
    dollar: Option<Node>,
}

enum OptionSlot {
    Critical,
    Fumble,
    Dollar,
}

enum TargetValue {
    Expr(Node),
    Question,
}

struct TargetPart {
    cmp_op: Option<CmpOp>,
    target: Option<TargetValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mc_parser() -> Parser {
        Parser::new(&["MC"], RoundType::Floor).enable_critical()
    }

    #[test]
    fn doc_example_literal_by_string() {
        // parser.y 冒頭の doc 例。
        // doc には `parsed.cmp_op #=> #>=` とあるが、字句解析（`<=` → `:<=`）と
        // target 規則から導出される正しい値は `:<=`。docの誤記と判断した。
        let parsed = mc_parser().parse("MC+2*3@30<=10/2-3").unwrap();
        assert_eq!(parsed.command, "MC");
        assert_eq!(parsed.modify_number, 6.into());
        assert_eq!(parsed.critical, Some(30.into()));
        assert_eq!(parsed.cmp_op, Some(CmpOp::Le));
        assert_eq!(parsed.target_number, Some(2.into()));
        assert!(!parsed.question_target);
    }

    #[test]
    fn doc_example_literal_by_regexp() {
        // doc例2は `round_type:` が必須キーワード引数なので FLOOR を補う。
        let parser = Parser::new(&[r"RE\d+"], RoundType::Floor);
        let parsed = parser.parse("RE44+20").unwrap();
        assert_eq!(parsed.command, "RE44");
        assert_eq!(parsed.modify_number, 20.into());
    }

    #[test]
    fn option_can_appear_after_modifier_only_when_absent_before() {
        // 規則2: notation modifier option target
        let parsed = mc_parser().parse("MC+2@30<=10").unwrap();
        assert_eq!(parsed.modify_number, 2.into());
        assert_eq!(parsed.critical, Some(30.into()));

        // 規則1: notation option modifier target
        let parsed = mc_parser().parse("MC@30+2<=10").unwrap();
        assert_eq!(parsed.modify_number, 2.into());
        assert_eq!(parsed.critical, Some(30.into()));

        // 修正値の前後に分けて置くことはできない
        let parser = mc_parser().enable_fumble();
        assert!(parser.parse("MC@30+2#5").is_none());
        assert!(parser.parse("MC@30#5+2").is_some());
        assert!(parser.parse("MC+2@30#5").is_some());
    }

    #[test]
    fn rejects_disabled_options_and_duplicates() {
        // クリティカル未許可
        assert!(Parser::new(&["MC"], RoundType::Floor)
            .parse("MC@30")
            .is_none());
        // 重複指定
        assert!(mc_parser().parse("MC@30@40").is_none());
    }

    #[test]
    fn modifier_can_be_disabled() {
        let parser = Parser::new(&["MC"], RoundType::Floor).disable_modifier();
        assert!(parser.parse("MC+2").is_none());
        assert_eq!(parser.parse("MC").unwrap().modify_number, 0.into());
    }

    #[test]
    fn prefix_and_suffix_numbers() {
        let parser = Parser::new(&["MC"], RoundType::Floor).has_prefix_number();
        assert_eq!(parser.parse("2MC").unwrap().prefix_number, Some(2.into()));
        assert!(parser.parse("MC").is_none());
        // suffix は許可されていない
        assert!(parser.parse("2MC3").is_none());

        let parser = Parser::new(&["MC"], RoundType::Floor)
            .enable_prefix_number()
            .has_suffix_number();
        assert_eq!(parser.parse("MC3").unwrap().suffix_number, Some(3.into()));
        let parsed = parser.parse("2MC3").unwrap();
        assert_eq!(parsed.prefix_number, Some(2.into()));
        assert_eq!(parsed.suffix_number, Some(3.into()));
        assert!(parser.parse("MC").is_none());
    }

    #[test]
    fn restricts_comparison_operators() {
        let parser = Parser::new(&["MC"], RoundType::Floor).restrict_cmp_op_to(&[Some(CmpOp::Le)]);
        assert!(parser.parse("MC<=10").is_some());
        assert!(parser.parse("MC>=10").is_none());
        // nil を許可していないので目標値省略も不可
        assert!(parser.parse("MC").is_none());
    }

    #[test]
    fn question_target() {
        let parser = Parser::new(&["MC"], RoundType::Floor);
        assert!(parser.parse("MC<=?").is_none());

        let parser = parser.enable_question_target();
        let parsed = parser.parse("MC<=?").unwrap();
        assert!(parsed.question_target);
        assert_eq!(parsed.target_number, Some(0.into()));
        assert_eq!(parsed.cmp_op, Some(CmpOp::Le));
    }

    #[test]
    fn requires_end_of_input() {
        assert!(mc_parser().parse("MC<=10x").is_none());
        // 空白以降は切り捨てられる
        assert_eq!(
            mc_parser().parse("MC<=10 コメント").unwrap().target_number,
            Some(10.into())
        );
    }

    #[test]
    fn zero_division_becomes_nil() {
        assert!(mc_parser().parse("MC<=10/0").is_none());
    }

    #[test]
    fn parsed_to_s() {
        let parsed = mc_parser().enable_fumble().parse("MC@30#5+2<=10").unwrap();
        assert_eq!(parsed.to_s(SuffixPosition::AfterCommand), "MC@30#5+2<=10");
        assert_eq!(
            parsed.to_s(SuffixPosition::AfterModifyNumber),
            "MC+2@30#5<=10"
        );
        assert_eq!(
            parsed.to_s(SuffixPosition::AfterTargetNumber),
            "MC+2<=10@30#5"
        );
    }
}
