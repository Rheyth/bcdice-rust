//! P4で手書き移植した `lib/bcdice/game_system/Karukami.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Karukami#roll_ub`（行為判定・ダメージ算出 `xUB+y@c>=t`）

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Karukami`（ID: `Karukami`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Karukami;

impl GameSystem for Karukami {
    fn id(&self) -> &'static str {
        "Karukami"
    }

    fn name(&self) -> &'static str {
        "カルカミ"
    }

    fn sort_key(&self) -> &'static str {
        "かるかみ"
    }

    fn help_message(&self) -> &'static str {
        r"■ 行為判定、ダメージ算出 (xUB+y@c>=t)
  6面ダイスをx個ダイスロールし、クリティカル値以上の出目が出たら振り足して合計値を算出します。
  x: ダイス数
  y: 修正値（省略可）
  c: クリティカル値（省略可）
  t: 目標値値（省略可）
  例）2UB, 2UB>=7, 3UB+1@5, 3UB+1@5<10
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+UB"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Karukami#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_ub(command, rng)
    }
}

/// Ruby `Karukami#roll_ub`。
fn roll_ub(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Command::Parser.new("UB", round_type: @round_type).has_prefix_number.enable_critical
    let parser = Parser::new(&["UB"], RoundType::Floor)
        .has_prefix_number()
        .enable_critical();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let command_text = parsed.to_s(SuffixPosition::AfterCommand);

    // Ruby: critical = parsed.critical || 6（`@0` は 0 のまま。Rubyでは 0 も真）
    let critical = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(6);
    if critical <= 1 {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "({command_text}) ＞ クリティカル値は2以上としてください"
        ))));
    }

    let mut list_list: Vec<Vec<i64>> = Vec::new();
    let mut criticals: i64 = 0;
    // has_prefix_number なので必ず埋まる。
    let mut stack: I = parsed.prefix_number.expect("has_prefix_number");
    while stack > I::ZERO {
        let dice_list = rng.roll_barabara(sat_i64(&stack), 6)?;
        stack = I::from(dice_list.iter().filter(|&&x| x >= critical).count() as i64);
        criticals += sat_i64(&stack);
        list_list.push(dice_list);
    }

    let mut total: I = I::from(list_list.iter().flatten().sum::<i64>()) + parsed.modify_number;

    // Ruby: list_list.first.all?(1)
    // ダイス数が0以下だと Ruby は `nil.all?` で NoMethodError になるが、
    // ここでは「ファンブルではない」に畳んでいる（TOMLに該当ケースはない）。
    let is_fumble = list_list
        .first()
        .is_some_and(|list| list.iter().all(|&x| x == 1));

    let mut result = if is_fumble {
        total = I::ZERO;
        EvalResult::fumble("ファンブル")
    } else if parsed.cmp_op.is_none() {
        // Ruby `Result.new()` の text は nil で、後段の compact で落ちる
        EvalResult::new()
    } else {
        let cmp_op = parsed.cmp_op.expect("checked above");
        let target_number = parsed.target_number.expect("cmp_op implies target_number");
        if cmp_op.apply(&total, &target_number) {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        }
    };
    result.critical = criticals > 0;

    // Ruby: sequence.compact.join(" ＞ ")
    // `result.text` が空になるのは `Result.new()`（Rubyでは nil）の枝だけなので、
    // 空文字列を落とすことが `compact` と一致する。
    let mut sequence: Vec<String> = Vec::with_capacity(list_list.len() + 4);
    sequence.push(format!("({command_text})"));
    for list in &list_list {
        sequence.push(format!(
            "[{}]",
            list.iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    sequence.push(total.to_string());
    if result.critical {
        sequence.push(format!("{criticals}クリティカル"));
    }
    if !result.text.is_empty() {
        sequence.push(result.text.clone());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Karukami", "Karukami.toml", 15);
    }
}
