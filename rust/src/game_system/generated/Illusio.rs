//! P4で手書き移植した `lib/bcdice/game_system/Illusio.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Illusio#eval_game_system_specific_command`（判定 `[n]IL(BNo)[P]`）と `check_roll`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::Illusio`（ID: `Illusio`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Illusio;

impl GameSystem for Illusio {
    fn id(&self) -> &'static str {
        "Illusio"
    }

    fn name(&self) -> &'static str {
        "晃天のイルージオ"
    }

    fn sort_key(&self) -> &'static str {
        "こうてんのいるうしお"
    }

    fn help_message(&self) -> &'static str {
        r"判定：[n]IL(BNo)[P]

[]内のコマンドは省略可能。
「n」でダイス数を指定。省略時は「1」。
(BNo)でブロックナンバーを指定。「236」のように記述。順不同可。
コマンド末に「P」を指定で、(BNo)のパリィ判定。（一応、複数指定可）

【書式例】
・6IL236 → 6dでブロックナンバー「2,3,6」の判定。
・IL4512 → 1dでブロックナンバー「1,2,4,5」の判定。
・2IL1P → 2dでパリィナンバー「1」の判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?IL([1-6]{0,6})(P)?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Illusio#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Illusio#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_check_roll(command, rng)
    }
}

/// Ruby `/(\d+)?IL([1-6]{0,6})(P)?$/i`。
///
/// 先頭は固定されていないので、末尾までが合えばコマンドの途中からでもマッチする
/// （原典どおり）。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)?IL([1-6]{0,6})(P)?$").expect("valid regex"))
}

/// Ruby `Illusio#eval_game_system_specific_command` 本体。
fn eval_check_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: return nil unless m
    let Some(captures) = command_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: (m[1] || 1).to_i
    let dice_count = captures.get(1).map_or(1, |m| to_i(m.as_str()));
    // Ruby: (m[2] || "").each_char.map(&:to_i).uniq.sort
    let block_no = block_numbers(captures.get(2).map_or("", |m| m.as_str()));
    // Ruby: !m[3].nil?
    let is_parry = captures.get(3).is_some();

    Ok(Some(SpecificCommandOutput::text(check_roll(
        dice_count, &block_no, is_parry, rng,
    )?)))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 桁あふれするダイス数は Ruby でも `roll_barabara` の上限（`UPPER_LIMIT_RANDS`）に
/// 引っかかって `TooManyRandsError` になるので、飽和させても挙動は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `(m[2] || "").each_char.map(&:to_i).uniq.sort`。
///
/// 文字クラスが `[1-6]` なので各文字は必ず1桁の数字になる。
/// Ruby の `uniq` は「出現順を保った重複除去」だが、直後に `sort` するので
/// 「ソートしてから隣接除去」と同値。
fn block_numbers(digits: &str) -> Vec<i64> {
    let mut numbers: Vec<i64> = digits.bytes().map(|b| i64::from(b - b'0')).collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

/// Ruby `Illusio#check_roll`。
fn check_roll(
    dice_count: i64,
    block_no: &[i64],
    is_parry: bool,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut dice_array = rng.roll_barabara(dice_count, 6)?;
    dice_array.sort_unstable();
    let dice_text = join_comma(&dice_array);

    let mut result_array: Vec<String> = Vec::with_capacity(dice_array.len());
    let mut success = 0i64;
    for dice in &dice_array {
        // Ruby: block_no.count(i) > 0
        if block_no.contains(dice) {
            result_array.push("×".to_owned());
        } else {
            result_array.push(dice.to_string());
            success += 1;
        }
    }

    let block_text = join_comma(block_no);
    let block_text2 = if is_parry { "Parry" } else { "Block" };
    let result_text = result_array.join(",");

    let result =
        format!("{dice_count}D6({block_text2}:{block_text}) ＞ {dice_text} ＞ {result_text} ＞ ");
    if !is_parry {
        return Ok(format!("{result}成功数：{success}"));
    }

    if success < dice_count {
        // 「非ダメージ」は原典の誤記（「被ダメージ」）だが、出力文字列なのでそのまま残す。
        Ok(format!("{result}パリィ成立！　次の非ダメージ2倍。"))
    } else {
        Ok(format!("{result}成功数：{success}　パリィ失敗"))
    }
}

/// Ruby `Array#join(',')`。
fn join_comma(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Illusio", "Illusio.toml", 9);
    }
}
