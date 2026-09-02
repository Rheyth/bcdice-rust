//! P4で手書き移植した `lib/bcdice/game_system/Sengensyou.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Sengensyou#eval_game_system_specific_command`（命中判定・回避判定 `SGS`）

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `Sengensyou#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Command::Parser.new('SGS', round_type: @round_type).restrict_cmp_op_to(nil)
    let parser = Parser::new(&["SGS"], RoundType::Floor).restrict_cmp_op_to(&[None]);
    let Some(command) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_list = rng.roll_barabara(3, 6)?;
    let dice_total: i64 = dice_list.iter().sum();
    let is_critical = dice_total >= 16;
    let is_fumble = dice_total <= 5;

    let modify_number = command.modify_number;
    let mut sequence = vec![
        format!("(3D6{})", modifier(&modify_number)),
        format!(
            "{dice_total}[{}]",
            dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
    ];
    // Ruby: modify_text は修正値が0のとき nil（＝`.compact` で落ちる）
    if modify_number != I::ZERO {
        sequence.push(format!("{dice_total}{}", modifier(&modify_number)));
    }
    sequence.push((dice_total + modify_number).to_string());
    if is_critical {
        sequence.push("クリティカル".to_owned());
    } else if is_fumble {
        sequence.push("ファンブル".to_owned());
    }

    // Ruby: `r.critical = ` / `r.fumble = ` は成功・失敗のフラグを立てない。
    // `Result.critical` / `Result.fumble` を使うと success / failure まで立つので使わない。
    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.critical = is_critical;
    result.fumble = is_fumble;

    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `BCDice::GameSystem::Sengensyou`（ID: `Sengensyou`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sengensyou;

impl GameSystem for Sengensyou {
    fn id(&self) -> &'static str {
        "Sengensyou"
    }

    fn name(&self) -> &'static str {
        "千幻抄"
    }

    fn sort_key(&self) -> &'static str {
        "せんけんしよう"
    }

    fn help_message(&self) -> &'static str {
        r"・SGS　命中判定・回避判定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SGS"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Sengensyou",
            "Sengensyou.toml",
            7,
        );
    }
}
