//! P4で手書き移植した `lib/bcdice/game_system/CthulhuTech.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `CthulhuTech::Test`（行為判定 `nD10+m>=d`）と `CthulhuTech::Contest`
//!   （対抗判定 `nD10+m>d`・ダメージロール表示つき）
//! - `#calculate_roll_result`（最大出目／ゾロ目の和／ストレートの和の最大値）
//! - `#sum_of_largest_straight`

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::CthulhuTech`（ID: `CthulhuTech`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CthulhuTech;

impl GameSystem for CthulhuTech {
    fn id(&self) -> &'static str {
        "CthulhuTech"
    }

    fn name(&self) -> &'static str {
        "クトゥルフテック"
    }

    fn sort_key(&self) -> &'static str {
        "くとうるふてつく"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定（test）：nD10+m>=d
　n個のダイスを使用して、修正値m、難易度dで行為判定（test）を行います。
　修正値mは省略可能、複数指定可能（例：+2-4）です。
　成功、失敗、クリティカル、ファンブルを自動判定します。
　例）2D10>=12　4D10+2>=28　5D10+2-4>=32

・対抗判定（contest）：nD10+m>d
　行為判定と同様ですが、防御側有利のため「>=」ではなく「>」を入力します。
　ダメージダイスも表示します。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+D10"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `CthulhuTech#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: TEST_RE = /\A(\d+)D10((?:[-+]\d+)+)?(>=?)(\d+)\z/
        let re = Regex::new(r"\A(\d+)D10((?:[-+]\d+)+)?(>=?)(\d+)\z").expect("valid regex");
        let Some(m) = re.captures(command) else {
            return Ok(None);
        };

        // Ruby: num = m[1].to_i
        let num: i64 = m[1].parse().unwrap_or(i64::MAX);
        // Ruby: modifier = m[2] ? ArithmeticEvaluator.eval(m[2]) : 0
        let mod_value = match m.get(2) {
            Some(x) => arithmetic::eval(x.as_str(), RoundType::Floor)?
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0),
            None => 0,
        };
        // Ruby: node_class = m[3] == '>' ? Contest : Test
        let is_contest = &m[3] == ">";
        // Ruby: difficulty = m[4].to_i
        let difficulty: i64 = m[4].parse().unwrap_or(i64::MAX);

        let text = execute(num, mod_value, difficulty, is_contest, rng)?;
        Ok(Some(SpecificCommandOutput::text(text)))
    }
}

/// Ruby `CthulhuTech::Test#execute`（`Contest` は `result_str` だけ上書き）。
fn execute(
    num: i64,
    modifier_value: i64,
    difficulty: i64,
    is_contest: bool,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_values = rng.roll_barabara(num, 10)?;

    // ファンブル：出目の半分（小数点以下切り上げ）以上が1の場合
    let fumble = dice_values.iter().filter(|&&x| x == 1).count() >= dice_values.len().div_ceil(2);

    let mut sorted_dice_values = dice_values;
    sorted_dice_values.sort_unstable();
    let roll_result = calculate_roll_result(&sorted_dice_values);
    let test_value = roll_result + modifier_value;

    let diff = test_value - difficulty;

    // Ruby: success = !fumble && diff.send(COMPARE_OP, 0)
    // （Contest は `>`、Test は `>=`）
    let success = !fumble && if is_contest { diff > 0 } else { diff >= 0 };

    let critical = diff >= 10;

    // Ruby: expression()
    let compare_op = if is_contest { ">" } else { ">=" };
    let expression = format!(
        "({num}D10{}{compare_op}{difficulty})",
        modifier(&crate::Int::from(modifier_value))
    );

    // Ruby: test_value_expression(sorted_dice_values, roll_result)
    let dice_str = join_dice(&sorted_dice_values);
    let test_value_expression = format!(
        "{roll_result}[{dice_str}]{}",
        modifier(&crate::Int::from(modifier_value))
    );

    let result_str = result_str(success, fumble, critical, diff, is_contest);

    let output_parts = [
        expression,
        test_value_expression,
        test_value.to_string(),
        result_str,
    ];
    Ok(output_parts.join(" ＞ "))
}

/// Ruby `CthulhuTech::Test#result_str`（`Contest#result_str` を含む）。
fn result_str(success: bool, fumble: bool, critical: bool, diff: i64, is_contest: bool) -> String {
    let formatted = if fumble {
        "ファンブル"
    } else if critical {
        "クリティカル"
    } else if success {
        "成功"
    } else {
        "失敗"
    };

    if is_contest && success {
        // Ruby Contest: damage_roll_num = (diff / 5.0).ceil
        let damage_roll_num = (diff as f64 / 5.0).ceil() as i64;
        format!("{formatted}（ダメージ：{damage_roll_num}D10）")
    } else {
        formatted.to_owned()
    }
}

/// Ruby `CthulhuTech::Test#calculate_roll_result`。
///
/// 以下のうち最大のものを返す。
///
/// * 出目の最大値
/// * ゾロ目の和の最大値
/// * ストレート（昇順で連続する3個以上の値）の和の最大値
fn calculate_roll_result(sorted_dice_values: &[i64]) -> i64 {
    let highest_single_roll = sorted_dice_values.last().copied().unwrap_or(0);

    // Ruby: group_by(&:itself).values.map(&:sum).max
    let sum_of_highest_set_of_multiples = grouped_sums(sorted_dice_values)
        .into_iter()
        .max()
        .unwrap_or(0);

    let candidates = [
        highest_single_roll,
        sum_of_highest_set_of_multiples,
        sum_of_largest_straight(sorted_dice_values),
    ];

    candidates.into_iter().max().unwrap_or(0)
}

/// Ruby `group_by(&:itself).values.map(&:sum)`。
fn grouped_sums(values: &[i64]) -> Vec<i64> {
    let mut sums = Vec::new();
    let mut iter = values.iter().copied().peekable();
    while let Some(value) = iter.next() {
        let mut sum = value;
        while iter.peek() == Some(&value) {
            sum += iter.next().unwrap();
        }
        sums.push(sum);
    }
    sums
}

/// Ruby `CthulhuTech::Test#sum_of_largest_straight`。
///
/// ストレートとは、昇順で3個以上連続した値のこと。
fn sum_of_largest_straight(sorted_dice_values: &[i64]) -> i64 {
    // 出目が3個未満ならば、ストレートは存在しない
    if sorted_dice_values.len() < 3 {
        return 0;
    }

    // ストレートの和の最大値
    let mut max_sum = 0;

    // 連続した値の数
    let mut n_consecutive_values = 0;
    // 連続した値の和
    let mut sum = 0;
    // 直前の値（初期値を負の値にして、最初の値と連続にならないようにする）
    let mut last = -1;

    for &value in sorted_dice_values
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
    {
        // 値が連続でなければ、状態を初期化する（現在の値を連続1個目とする）
        if value - last > 1 {
            n_consecutive_values = 1;
            sum = value;
            last = value;
            continue;
        }

        // 連続した値なので溜める
        n_consecutive_values += 1;
        sum += value;
        last = value;

        // ストレートならば、和の最大値を更新する
        if n_consecutive_values >= 3 && sum > max_sum {
            max_sum = sum;
        }
    }

    max_sum
}

/// Ruby `dice_values.join(',')`。
fn join_dice(dice: &[i64]) -> String {
    dice.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "CthulhuTech",
            "CthulhuTech.toml",
            68,
        );
    }
}
