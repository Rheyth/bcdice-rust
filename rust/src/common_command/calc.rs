//! 計算コマンド `C(...)`。Ruby `lib/bcdice/common_command/calc.rb` の移植。
//!
//! P3側の実装（arithmetic共有ノード＋GameSystem経由のround_type）を正とし、
//! P1系のテストをP3 APIに合わせて維持した統合版。

use crate::arithmetic::{self, Node, ParenMode};
use crate::common_command::lexer::{self, Cursor};
use crate::enums::RoundType;
use crate::eval::{EvalError, EvalResult};
use crate::game_system::GameSystem;

/// Ruby `Calc.eval(command, game_system, _randomizer)`。
pub fn eval(command: &str, game_system: &dyn GameSystem) -> Result<Option<EvalResult>, EvalError> {
    match parse(command) {
        Some(cmd) => cmd.eval(game_system.round_type()).map(Some),
        None => Ok(None),
    }
}

/// Ruby `Calc::Node::Command`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    secret: bool,
    expr: Node,
}

impl Command {
    /// Ruby `Node::Command#eval(round_type)`。
    pub fn eval(&self, round_type: RoundType) -> Result<EvalResult, EvalError> {
        let value = match self.expr.eval(round_type) {
            Ok(v) => v.to_string(),
            // Ruby: rescue ZeroDivisionError
            Err(EvalError::ZeroDivision) => {
                "ゼロ除算が発生したため計算できませんでした".to_string()
            }
            // FloatDomainError（`c1/0C` 等）はRuby側でも rescue されずクラッシュする。
            // ここでは握り潰さずエラーとして伝播させる。
            Err(e) => return Err(e),
        };

        let output = if self.expr.is_parenthesis() {
            self.expr.output()
        } else {
            format!("({})", self.expr.output())
        };

        Ok(EvalResult {
            text: format!("c{output} ＞ {value}"),
            secret: self.secret,
            ..EvalResult::default()
        })
    }
}

/// Ruby `Calc::Parser.parse(source)`。
///
/// 文法は `expr: secret C add`。`term` は **`Parenthesis` で包む**
/// （他のcommon_commandの文法はカッコを捨てる）。
pub fn parse(source: &str) -> Option<Command> {
    let lexed = lexer::lex(source);
    let mut cur = Cursor::new(&lexed.tokens);

    let secret = cur.accept_sym("S");
    if !cur.accept_sym("C") {
        return None;
    }
    let expr = arithmetic::parse_add(&mut cur, ParenMode::Keep)?;
    cur.at_eof().then_some(Command { secret, expr })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(src: &str) -> Option<String> {
        let cmd = parse(src)?;
        Some(cmd.eval(RoundType::Floor).unwrap().text)
    }

    #[test]
    fn formats_output() {
        assert_eq!(text("C(1+2)").as_deref(), Some("c(1+2) ＞ 3"));
        assert_eq!(text("c1+4*3/2").as_deref(), Some("c(1+4*3/2) ＞ 7"));
        assert_eq!(text("c((10+10)*9)").as_deref(), Some("c((10+10)*9) ＞ 180"));
    }

    #[test]
    fn uses_the_game_system_round_type_for_unsuffixed_division() {
        // calc.rb が game_system.round_type を渡すのに対応する
        let cmd = parse("C(5/2)").unwrap();
        assert_eq!(cmd.eval(RoundType::Ceil).unwrap().text, "c(5/2) ＞ 3");
        assert_eq!(cmd.eval(RoundType::Floor).unwrap().text, "c(5/2) ＞ 2");
    }

    #[test]
    fn reports_zero_division() {
        assert_eq!(
            text("C(1/0)").as_deref(),
            Some("c(1/0) ＞ ゼロ除算が発生したため計算できませんでした")
        );
    }

    #[test]
    fn rejects_invalid_expression() {
        assert!(parse("c1+4*").is_none());
        assert!(parse("2d6").is_none());
    }
}
