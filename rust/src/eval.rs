//! コマンド評価の入り口。Ruby `BCDice::Base#eval`（lib/bcdice/base.rb）に対応する。

use crate::common_command;
use crate::game_system::{game_system_class, GameSystem, GameSystemId, SpecificCommandOutput};
use crate::preprocessor;
use crate::randomizer::{RandSource, Randomizer};

pub use crate::result::EvalResult;

/// 評価時のエラー。
///
/// Ruby側で「rescueされずに伝播する例外」に相当するものを列挙する。
/// パースエラー（`Racc::ParseError`）は各パーサ内で `nil` に畳まれるので含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// コマンドとして解釈できなかった（Ruby側で eval が nil を返した場合に相当）。
    UnrecognizedCommand,
    /// ゲームシステム固有コマンド（`eval_game_system_specific_command`）が未実装。
    ///
    /// P3-Batch2 が生成したメタデータスタブは、接頭辞にマッチした入力に対して
    /// これを返す。既定の `Ok(None)` にしてしまうと、未実装の固有コマンドが黙って
    /// 汎用コマンドへフォールスルーして誤った出力を返すため。P4で個別移植して解消する。
    NotImplemented,
    /// このゲームシステムがレジストリに登録されていない（P3の後続バッチで解消する）。
    SystemNotImplemented,
    /// Ruby `BCDice::TooManyRandsError`。
    TooManyRands,
    /// Ruby `ZeroDivisionError`。
    ZeroDivision,
    /// Ruby `FloatDomainError`（`(x.to_f/0).ceil` など）。
    FloatDomain,
    /// 空白のみ（または空文字）の入力。
    ///
    /// Ruby は `Preprocessor#trim_after_whitespace` が `nil` を返し、続く
    /// `replace_parentheses` の `nil.gsub` で `NoMethodError` を送出する
    /// （lib/bcdice/preprocessor.rb:37,45・本家のバグ）。`eval` が nil を返すのか
    /// 例外になるのかは呼び出し側から観測できる差なので、例外側（`error: "Other"`）
    /// として再現する。差分ファズの degenerate 5件がこれ。
    BlankInput,
    /// 注入乱数（`SeededRandomizer`）の枯渇・面数不一致。
    /// Ruby側の `RandomizerMock#random` の `raise` に相当する。
    RandSource(String),
    /// 文法上到達しないはずの状態に入った（移植のバグ）。
    /// 握り潰さずハーネスのfail理由として表面化させる。
    Internal(&'static str),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UnrecognizedCommand => f.write_str("command was not recognized"),
            EvalError::NotImplemented => {
                f.write_str("game system specific command not implemented (P4)")
            }
            EvalError::SystemNotImplemented => f.write_str("game system not implemented yet (P3)"),
            EvalError::TooManyRands => f.write_str("TooManyRandsError"),
            EvalError::ZeroDivision => f.write_str("ZeroDivisionError"),
            EvalError::FloatDomain => f.write_str("FloatDomainError"),
            EvalError::BlankInput => f.write_str("input is blank (Ruby raises NoMethodError)"),
            EvalError::RandSource(msg) => write!(f, "rand source error: {msg}"),
            EvalError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

/// ゲームシステムIDとコマンドを受け取り、評価する。
///
/// `randomizer` には TOML `rands` を注入した `SeededRandomizer` を渡す。
/// 戻り値の `Ok(None)` は Ruby側で eval が nil を返した場合（コマンド未解釈）に相当する。
pub fn eval_command(
    system: &GameSystemId,
    command: &str,
    randomizer: &mut dyn RandSource,
) -> Result<Option<EvalResult>, EvalError> {
    let game_system = game_system_class(system.as_str()).ok_or(EvalError::SystemNotImplemented)?;
    let mut rng = Randomizer::new(randomizer);
    eval_raw(game_system, command, &mut rng)
}

/// Ruby `Base#eval` 本体。
///
/// `Repeat` コマンドが `game_system.class.new(trailer)` で自分自身を再入するため、
/// ランダマイザを共有したまま再帰的に呼べるようにしてある。
pub fn eval_raw(
    game_system: &dyn GameSystem,
    raw_input: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    // Ruby は Preprocessor.process の中で NoMethodError になる。前処理より前に弾く
    if preprocessor::is_blank(raw_input) {
        return Err(EvalError::BlankInput);
    }

    let command = preprocessor::process(raw_input, game_system)?;

    // Ruby: dice_command(command) || eval_common_command(@raw_input)
    if let Some(result) = dice_command(game_system, &command, rng)? {
        return Ok(Some(result));
    }

    // Ruby側は result.rands / result.detailed_rands をここで詰めるが、
    // ハーネスは注入乱数の消費で検証するため省略している（result.rs のdoc参照）。
    common_command::eval_common_command(game_system, raw_input, rng)
}

/// Ruby `Base#dice_command`。ゲームシステム固有コマンドを試す。
///
/// 接頭辞が未登録（Ruby側の `prefixes_pattern` が `/(?!)/`）のゲームシステムでは
/// 常に `None` を返す。DiceBotはこの経路。
fn dice_command(
    game_system: &dyn GameSystem,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: command = command.upcase if @enabled_upcase_input
    let command = if game_system.enabled_upcase_input() {
        std::borrow::Cow::Owned(command.to_uppercase())
    } else {
        std::borrow::Cow::Borrowed(command)
    };

    let Some(pattern) = game_system.prefixes_pattern() else {
        return Ok(None);
    };
    let Some(captures) = pattern.captures(&command) else {
        return Ok(None);
    };

    // Ruby: secret = !m[1].nil?; command = command[1..-1] if secret
    let secret = captures.get(1).is_some();
    // 先頭の 'S' はASCII1バイトなので、この範囲指定は常に文字境界。
    let command = if secret { &command[1..] } else { &command[..] };

    match game_system.eval_game_system_specific_command(command, rng)? {
        Some(SpecificCommandOutput::Result(mut result)) => {
            // Ruby: output.secret = output.secret? || secret
            result.secret = result.secret || secret;
            Ok(Some(*result))
        }
        // Ruby: return nil if output.nil? || output.empty? || output == "1"
        Some(SpecificCommandOutput::Text(text)) if !text.is_empty() && text != "1" => {
            Ok(Some(EvalResult {
                text,
                secret,
                ..EvalResult::default()
            }))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::randomizer::SeededRandomizer;

    fn eval_dice_bot(input: &str) -> Result<Option<EvalResult>, EvalError> {
        let system = game_system_class("DiceBot").expect("DiceBot is implemented");
        let mut src = SeededRandomizer::new(std::iter::empty());
        let mut rng = Randomizer::new(&mut src);
        eval_raw(system, input, &mut rng)
    }

    /// Ruby の `Base#eval("")` は Preprocessor 内で NoMethodError を送出する。
    /// 「nil を返す」ではなく「例外になる」ことを再現する
    /// （差分ファズ degenerate 5件の解消・reports/fuzz_known_diffs.md 節1参照）。
    #[test]
    fn blank_input_is_an_error_like_ruby() {
        for blank in ["", " ", "   ", "\t", "\n"] {
            assert_eq!(
                eval_dice_bot(blank),
                Err(EvalError::BlankInput),
                "blank input must error: {blank:?}"
            );
        }
    }

    /// 空白以外の未解釈コマンドは従来どおり nil（`Ok(None)`）。
    /// U+3000（全角空白）は Ruby の `strip` にも `\s` にも含まれないため対象外。
    #[test]
    fn unrecognized_command_is_still_none() {
        assert_eq!(eval_dice_bot("foo"), Ok(None));
        assert_eq!(eval_dice_bot("　"), Ok(None));
    }
}
