//! P4で手書き移植した `lib/bcdice/game_system/FateCoreSystem.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command` → `roll_df`（ファッジダイス判定 `xDF+y>=t`）

use std::cmp::Ordering;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::FateCoreSystem`（ID: `FateCoreSystem`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FateCoreSystem;

impl GameSystem for FateCoreSystem {
    fn id(&self) -> &'static str {
        "FateCoreSystem"
    }

    fn name(&self) -> &'static str {
        "Fate Core System"
    }

    fn sort_key(&self) -> &'static str {
        "ふえいとこあしすてむ"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*DF"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_df(command, rng)?.map(SpecificCommandOutput::result))
    }
}

const HELP_MESSAGE: &str = r"■ ファッジダイスによる判定 (xDF+y>=t)
  ファッジダイスをx個ダイスロールし、結果を判定します。
  x: ダイス数(省略時4)
  y: 修正値（省略可）
  t: 目標値（省略可）
  例）4DF, 4DF>=3, 4DF+1>=3, DF, DF>=3, DF+1>=3
";

/// Ruby `FateCoreSystem#roll_df`。
fn roll_df(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["DF"], RoundType::Floor)
        .enable_prefix_number()
        .restrict_cmp_op_to(&[Some(CmpOp::Ge), None]);

    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: dice_x = 4; dice_x = parsed.prefix_number if parsed.prefix_number
    let dice_x = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(4);
    let dice_list = roll_fate_dice(dice_x, rng)?;
    let total = dice_list
        .iter()
        .fold(0i64, |a, b| a.saturating_add(*b))
        .saturating_add(crate::randomizer::sat_i64(&parsed.modify_number));

    let fate_dice_list: String = dice_list
        .iter()
        .map(|i| match i.cmp(&0) {
            Ordering::Equal => "[ ]",
            Ordering::Greater => "[+]",
            Ordering::Less => "[-]",
        })
        .collect();

    let outcome = outcome(
        total,
        parsed
            .target_number
            .as_ref()
            .map(crate::randomizer::sat_i64),
    );
    let mut result = outcome.to_result();

    let mut sequence = vec![
        format!("({})", parsed.to_s(SuffixPosition::AfterCommand)),
        format!("{fate_dice_list}{}", modifier(&parsed.modify_number)),
        result_ladder(total),
    ];
    if let Some(text) = outcome.text() {
        sequence.push(text.to_string());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `FateCoreSystem#roll_fate_dice`。
fn roll_fate_dice(times: i64, rng: &mut Randomizer) -> Result<Vec<i64>, EvalError> {
    Ok(rng
        .roll_barabara(times, 3)?
        .into_iter()
        .map(|i| i - 2)
        .collect())
}

/// Ruby `FateCoreSystem#result_ladder`。
fn result_ladder(total: i64) -> String {
    let ladder = match total.clamp(-2, 8) {
        8 => "Legendary",
        7 => "Epic",
        6 => "Fantastic",
        5 => "Superb",
        4 => "Great",
        3 => "Good",
        2 => "Fair",
        1 => "Average",
        0 => "Mediocre",
        -1 => "Poor",
        _ => "Terrible",
    };

    format!("{ladder}({total:+})")
}

/// Ruby `FateCoreSystem#outcome` が返す `Result` の種別。
///
/// Ruby は目標値が無い場合 `Result.new`（`text` が nil）を返し、呼び出し側の
/// `compact` で判定テキストだけが落ちる。[`Outcome::Undecided`] がその nil に対応する
/// （`EvalResult#text` は空文字列なので、Rust側では型で区別する）。
enum Outcome {
    /// 目標値なし。Ruby `Result.new`。
    Undecided,
    /// Ruby `Result.success(text)`。
    Success(&'static str),
    /// Ruby `Result.critical(text)`。
    Critical(&'static str),
    /// Ruby `Result.failure(text)`。
    Failure(&'static str),
}

impl Outcome {
    /// Ruby `Result#text`（`Result.new` のときは nil）。
    fn text(&self) -> Option<&'static str> {
        match self {
            Outcome::Undecided => None,
            Outcome::Success(text) | Outcome::Critical(text) | Outcome::Failure(text) => Some(text),
        }
    }

    fn to_result(&self) -> EvalResult {
        match self {
            Outcome::Undecided => EvalResult::new(),
            Outcome::Success(text) => EvalResult::success(*text),
            Outcome::Critical(text) => EvalResult::critical(*text),
            Outcome::Failure(text) => EvalResult::failure(*text),
        }
    }
}

/// Ruby `FateCoreSystem#outcome`。
fn outcome(total: i64, target: Option<i64>) -> Outcome {
    let Some(target) = target else {
        return Outcome::Undecided;
    };

    if total == target {
        Outcome::Success("Tie(+0)")
    } else if total == target.saturating_add(1) {
        Outcome::Success("Succeed(+1)")
    } else if total >= target.saturating_add(3) {
        Outcome::Critical("Succeed with Style")
    } else if total >= target {
        Outcome::Success("Succeed")
    } else {
        Outcome::Failure("Fail")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "FateCoreSystem",
            "FateCoreSystem.toml",
            22,
        );
    }
}
