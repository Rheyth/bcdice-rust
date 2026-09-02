//! P4で手書き移植した `lib/bcdice/game_system/OrgaRain.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `OrgaRain#eval_game_system_specific_command` と `#check_roll`（判定 `[n]OR(count)`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `/(\d+)?OR(\d{0,6})$/i`。
///
/// Rubyの `$` は行末にもマッチするが、`Preprocessor` が最初の空白より前しか
/// 残さないため入力に改行は無く、Rustの「文字列末尾」と同じ意味になる。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)?OR(\d{0,6})$").expect("valid regex"))
}

/// Ruby `OrgaRain#eval_game_system_specific_command`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: (m[1] || 1).to_i
    let dice_count = m.get(1).map_or(1, |x| to_i(x.as_str()));
    // Ruby: (m[2] || "").each_char.map(&:to_i).sort
    let mut count_no: Vec<i64> = m
        .get(2)
        .map_or("", |x| x.as_str())
        .chars()
        .map(|c| i64::from(c.to_digit(10).unwrap_or(0)))
        .collect();
    count_no.sort_unstable();

    Ok(Some(check_roll(dice_count, &count_no, rng)?))
}

/// Ruby `OrgaRain#check_roll`。
fn check_roll(
    dice_count: i64,
    count_no: &[i64],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut dice_array = rng.roll_barabara(dice_count, 10)?;
    dice_array.sort_unstable();
    let dice_text = join_values(&dice_array);

    let mut result_array: Vec<String> = Vec::new();
    let mut success = 0i64;
    // Ruby: dice_array.map { |x| x == 10 ? 0 : x }（10は命数0として扱う）
    for i in dice_array.iter().map(|&x| if x == 10 { 0 } else { x }) {
        let multiple = count_no.iter().filter(|&&c| c == i).count() as i64;
        if multiple > 0 {
            result_array.push(format!("{i}(x{multiple})"));
            success += multiple;
        } else {
            result_array.push("×".to_owned());
        }
    }

    let count_text = join_values(count_no);
    let result_text = result_array.join(",");

    Ok(format!(
        "{dice_count}D10(命数：{count_text}) ＞ {dice_text} ＞ {result_text} ＞ 成功数：{success}"
    ))
}

/// Ruby `String#to_i`。i64に収まらない値は飽和させる（Rubyでは Bignum）。
fn to_i(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `Array#join(',')`。
fn join_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::OrgaRain`（ID: `OrgaRain`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrgaRain;

impl GameSystem for OrgaRain {
    fn id(&self) -> &'static str {
        "OrgaRain"
    }

    fn name(&self) -> &'static str {
        "在りて遍くオルガレイン"
    }

    fn sort_key(&self) -> &'static str {
        "ありてあまねくおるかれいん"
    }

    fn help_message(&self) -> &'static str {
        r"判定：[n]OR(count)

[]内のコマンドは省略可能。
「n」でダイス数を指定。省略時は「1」。
(count)で命数を指定。「3111」のように記述。最大6つ。順不同可。

【書式例】
・5OR6042 → 5dで命数「0,2,4,6」の判定
・6OR33333 → 6dで命数「3,3,3,3,3」の判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?OR(\d{0,6})?"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific_command(command, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("OrgaRain", "OrgaRain.toml", 7);
    }
}
