//! P4で手書き移植した `lib/bcdice/game_system/ToshiakiHolyGrailWar.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#roll_f`（`Fx+y-z@a>=t` の判定。補正値からダイス個数と面数を決める）
//! - `#positive_modifier_penalty` / `#negative_modifier_bonus`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `Command::Parser.new(/F(\d+)(\+\d+)*(-\d+)*/, round_type: RoundType::CEIL)`
/// `.disable_modifier.enable_critical`。
fn parser() -> &'static Parser {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    PARSER.get_or_init(|| {
        Parser::new(&[r"F(\d+)(\+\d+)*(-\d+)*"], RoundType::Ceil)
            .disable_modifier()
            .enable_critical()
    })
}

/// Ruby `cmd.command.match(/^F(\d+)((?:\+\d+)+)?((?:-\d+)+)?$/)`。
fn re_command() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^F(\d+)((?:\+\d+)+)?((?:-\d+)+)?$").expect("valid regexp"))
}

/// Ruby `#positive_modifier_penalty(modifier)`。
///
/// `modifier` は `(\+\d+)+` 由来なので常に非負。Rubyの整数除算（切り捨て）と
/// Rustの `/`（0方向への切り捨て）は非負では一致する。
fn positive_modifier_penalty(modifier: i64) -> i64 {
    if modifier <= 10 {
        0
    } else {
        modifier / 10
    }
}

/// Ruby `#negative_modifier_bonus(modifier)`。
fn negative_modifier_bonus(modifier: i64) -> i64 {
    if modifier <= -5 {
        1
    } else {
        0
    }
}

/// Ruby `#roll_f(command)`。
fn roll_f(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(cmd) = parser().parse(command) else {
        return Ok(None);
    };

    let Some(m) = re_command().captures(&cmd.command) else {
        return Ok(None);
    };

    // notation が `F(\d+)…` なので数値部は必ず十進数字列。
    let status: i64 = m[1].parse().unwrap_or(0);
    // Ruby: m[2] ? Arithmetic.eval(m[2], RoundType::CEIL) : 0
    // `+n` の連なりしか来ないので `Arithmetic.eval` が nil を返すことはない。
    let positive_modifier = match m.get(2) {
        Some(s) => arithmetic::eval(s.as_str(), RoundType::Ceil)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };
    let negative_modifier = match m.get(3) {
        Some(s) => arithmetic::eval(s.as_str(), RoundType::Ceil)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };
    let side_bonus = cmd
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let times = (status + positive_modifier + negative_modifier).max(0);
    let sides = (6 - positive_modifier_penalty(positive_modifier)
        + negative_modifier_bonus(negative_modifier)
        + side_bonus)
        .clamp(2, 10);

    let list = rng.roll_barabara(times, sides)?;
    let total: i64 = list.iter().sum();

    // Ruby: cmd.cmp_op.nil? -> Result.new（text は nil なので後段の compact で落ちる）
    let mut result = match cmd.cmp_op {
        None => EvalResult::new(),
        Some(cmp_op) => {
            // 文法上、比較演算子があれば目標値も必ずある。
            if cmp_op.apply(
                &I::from(total),
                &cmd.target_number.clone().unwrap_or(I::from(0)),
            ) {
                EvalResult::success("成功")
            } else {
                EvalResult::failure("失敗")
            }
        }
    };

    // Ruby: "(#{times}D#{sides}#{cmd.cmp_op}#{cmd.target_number})"
    // 比較演算子は Format.comparison_operator ではなく Symbol#to_s がそのまま入る。
    let cmp_op_str = cmd.cmp_op.map(|op| op.symbol_str()).unwrap_or("");
    let target_str = cmd
        .target_number
        .as_ref()
        .map(|t| t.to_string())
        .unwrap_or_default();
    let dice_list = list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut sequence = vec![
        cmd.to_s(SuffixPosition::AfterCommand),
        format!("({times}D{sides}{cmp_op_str}{target_str})"),
        format!("{total}[{dice_list}]"),
        total.to_string(),
    ];
    // Ruby の `.compact`: `Result.new` の text は nil なので連結されない。
    if cmd.cmp_op.is_some() {
        sequence.push(result.text.clone());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::ToshiakiHolyGrailWar`（ID: `ToshiakiHolyGrailWar`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToshiakiHolyGrailWar;

impl GameSystem for ToshiakiHolyGrailWar {
    fn id(&self) -> &'static str {
        "ToshiakiHolyGrailWar"
    }

    fn name(&self) -> &'static str {
        "としあきの聖杯戦争TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "としあきのせいはいせんそうTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (Fx+y-z@a>=t)
  補正値ペナルティを自動計算してダイスの面数を決定しダイスロールを実行します。
  ダイス面数は2以上、10以下の範囲に制限されます。
  x: ステータス
  y: 補正値 (任意)
  z: マイナス補正値 (任意)
  a: ダイス面数の増量 (任意)
  t: 目標値 (任意)
  例)
    F8+11, F8+11-5, F8+11-5@1, F8+11+9-3-2@-1, F8+11-5>=50, F8
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["F"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ToshiakiHolyGrailWar#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_f(command, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ToshiakiHolyGrailWar",
            "ToshiakiHolyGrailWar.toml",
            20,
        );
    }
}
