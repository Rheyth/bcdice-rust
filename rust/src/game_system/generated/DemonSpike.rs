//! P4で手書き移植した `lib/bcdice/game_system/DemonSpike.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DemonSpike#eval_game_system_specific_command` → `roll_action` / `roll_step`
//!   （行為判定 `xDS+y`）

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::DemonSpike`（ID: `DemonSpike`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemonSpike;

impl GameSystem for DemonSpike {
    fn id(&self) -> &'static str {
        "DemonSpike"
    }

    fn name(&self) -> &'static str {
        "デモンスパイク"
    }

    fn sort_key(&self) -> &'static str {
        "てもんすはいく"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定 xDS+y
　行為判定を行い、達成値、成否、成功度を出力する。
　x: ダイス数（省略：2）
　y: 能力値やスパイク能力による達成値の修正（省略可）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*DS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DemonSpike#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_action(command, rng)?.map(SpecificCommandOutput::result))
    }
}

/// Ruby `DemonSpike#roll_step` の戻り値 `{dice_list:, dice_sum:}`。
struct Step {
    /// 降順に並べた出目
    dice_list: Vec<i64>,
    /// 上位2つの和（2〜10に丸めたもの）
    dice_sum: i64,
}

/// Ruby `DemonSpike#roll_action`（行為判定）。
fn roll_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("DS", round_type: @round_type)
    //       `@round_type` は Base の既定（:floor）のまま。
    //       `restrict_cmp_op_to(nil)` は可変長引数なので許可リストが `[nil]` になる。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["DS"], RoundType::Floor)
            .enable_prefix_number()
            .restrict_cmp_op_to(&[None])
    });

    let Some(mut parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: parsed.prefix_number ||= 2（`parsed` の表示にもこの既定値が使われる）
    let times = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    parsed.prefix_number = Some(crate::Int::from(times));
    if times < 2 {
        return Ok(None);
    }

    // Ruby: 出目の合計が10（＝上限）の間、振り足す
    let mut step_list: Vec<Step> = Vec::new();
    let mut step = roll_step(times, rng)?;
    loop {
        let dice_sum = step.dice_sum;
        step_list.push(step);
        if dice_sum != 10 {
            break;
        }
        step = roll_step(times, rng)?;
    }

    let is_fumble = step_list.first().is_some_and(|s| s.dice_sum == 2);
    let total = if is_fumble {
        0
    } else {
        step_list
            .iter()
            .map(|s| s.dice_sum)
            .sum::<i64>()
            .saturating_add(crate::randomizer::sat_i64(&parsed.modify_number))
    };
    // Ruby `Integer#/` は床除算（修正値が大きく負なら total も負になりうる）
    let success_level = (total).div_euclid(10);
    let is_success = total >= 10;

    let res = if is_success {
        format!("成功, 成功度{success_level}")
    } else if is_fumble {
        "自動的失敗".to_owned()
    } else {
        "失敗".to_owned()
    };

    let mut sequence: Vec<String> = Vec::with_capacity(step_list.len() + 3);
    sequence.push(format!("({})", parsed.to_s(SuffixPosition::AfterCommand)));
    for s in &step_list {
        sequence.push(format!(
            "{}[{}]",
            s.dice_sum,
            s.dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    sequence.push(total.to_string());
    sequence.push(res);

    let mut result = EvalResult::new();
    // Ruby: r.condition = is_success（success/failure を同時に決める）
    result.set_condition(is_success);
    result.critical = step_list.len() > 1;
    result.fumble = is_fumble;
    result.text = sequence.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `DemonSpike#roll_step`。
fn roll_step(times: i64, rng: &mut Randomizer) -> Result<Step, EvalError> {
    let mut dice_list = rng.roll_barabara(times, 6)?;
    // Ruby: .sort.reverse
    dice_list.sort_unstable();
    dice_list.reverse();

    // Ruby: (dice_list[0] + dice_list[1]).clamp(2, 10)
    // 呼び出し元が `times >= 2` を保証しているので通常は必ず2個以上あるが、
    // `roll_barabara` の個数上限（201個以上）を超えると空配列が返り、Ruby は
    // NoMethodError でクラッシュする。ここは0扱いにして落とさない。
    let first = dice_list.first().copied().unwrap_or(0);
    let second = dice_list.get(1).copied().unwrap_or(0);
    let dice_sum = (first + second).clamp(2, 10);

    Ok(Step {
        dice_list,
        dice_sum,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DemonSpike",
            "DemonSpike.toml",
            12,
        );
    }
}
