//! P4で手書き移植した `lib/bcdice/game_system/RecordOfLodossWar.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `RecordOfLodossWar#eval_game_system_specific_command`（判定 `LW<=t` / 回避判定 `LWD<=t`）

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;

/// Ruby `RecordOfLodossWar#eval_game_system_specific_command`。
fn eval_specific_command(
    round_type: RoundType,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    // Ruby: Command::Parser.new("LWD", "LW", ...) の並びが重要。
    // "LW" を先に置くと "LWD" が "LW" + "D" に分かれてパースに失敗する。
    let parser =
        Parser::new(&["LWD", "LW"], round_type).restrict_cmp_op_to(&[None, Some(CmpOp::Le)]);

    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: ![nil, :<=].include?(cmd.cmp_op) → nil
    if !matches!(cmd.cmp_op, None | Some(CmpOp::Le)) {
        return Ok(None);
    }

    let auto_failure = if cmd.command == "LWD" { 51 } else { 91 };
    // Ruby: (cmd.target_number.to_f / 10).ceil。目標値未指定なら `nil.to_f == 0.0`。
    let target_number = cmd.target_number.clone().unwrap_or(crate::Int::from(0));
    /* TODO-B18-CHECK */
    let critical = ((crate::randomizer::sat_i64(&target_number) as f64) / 10.0).ceil() as i64;

    let dice_value = rng.roll_once(100)?;

    let result = if dice_value >= auto_failure {
        Some(format!("自動失敗({auto_failure})"))
    } else if dice_value <= critical {
        Some(format!("大成功({critical})"))
    } else if dice_value <= 10 {
        Some("自動成功".to_owned())
    } else if cmd.cmp_op.is_some() {
        // 比較演算子があれば目標値も必ず伴う（`?` 目標値は許可していない）。
        Some(
            if dice_value <= crate::randomizer::sat_i64(&target_number) {
                "成功"
            } else {
                "失敗"
            }
            .to_owned(),
        )
    } else {
        None
    };

    // Ruby: "(1D100#{cmd.cmp_op}#{cmd.target_number})" は Symbol#to_s の連結
    //       （`Format.comparison_operator` ではない）。
    let cmp_op_text = cmd.cmp_op.map_or("", |op| op.symbol_str());
    let target_text = cmd
        .target_number
        .as_ref()
        .map_or_else(String::new, |t| t.to_string());

    // Ruby: sequence.compact.join(" ＞ ")
    let mut sequence = vec![
        format!("(1D100{cmp_op_text}{target_text})"),
        dice_value.to_string(),
    ];
    if let Some(result) = result {
        sequence.push(result);
    }

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `BCDice::GameSystem::RecordOfLodossWar`（ID: `RecordOfLodossWar`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordOfLodossWar;

impl GameSystem for RecordOfLodossWar {
    fn id(&self) -> &'static str {
        "RecordOfLodossWar"
    }

    fn name(&self) -> &'static str {
        "ロードス島戦記RPG"
    }

    fn sort_key(&self) -> &'static str {
        "ろおとすとうせんきRPG"
    }

    fn help_message(&self) -> &'static str {
        r"●判定
　LW<=(目標値)で判定。
　達成値が目標値の1/10(端数切り上げ)以下であれば大成功。1～10であれば自動成功。
　91～100であれば自動失敗となります。

●回避判定
　LWD<=(目標値)で回避判定。この時出目が51以上で自動失敗となります。

　判定と回避判定は、どちらもコマンドだけの場合、出目の表示と自動成功と自動失敗の判定のみを行います。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["LW"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(
            eval_specific_command(self.round_type(), command, rng)?
                .map(SpecificCommandOutput::text),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "RecordOfLodossWar",
            "RecordOfLodossWar.toml",
            9,
        );
    }
}
