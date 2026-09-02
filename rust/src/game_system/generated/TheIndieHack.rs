//! P4で手書き移植した `lib/bcdice/game_system/TheIndieHack.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TheIndieHack#resolute_action`（判定 `cIH+a`）と `#get_success_level`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `/([+-]?\d)?IH([+-]\d)?/`。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([+-]?\d)?IH([+-]\d)?").expect("valid regex"))
}

/// Ruby `TheIndieHack#resolute_action`。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1].to_i / m[2].to_i（nil.to_i は 0）。出力には**生の文字列**の方を使う。
    let cl_text = m.get(1).map_or("", |x| x.as_str());
    let abilities_text = m.get(2).map_or("", |x| x.as_str());
    let cl = to_i(cl_text);
    let abilities = to_i(abilities_text);

    let mut dices = rng.roll_barabara(2, 6)?;
    let dice_text = dice_text::join_dice(&dices);
    // `roll_barabara(2, 6)` は必ず2個返す（上限超過は Err）。
    if dices.len() < 2 {
        return Ok(None);
    }
    dices[0] += cl;
    dices[1] += abilities;
    let dice_text2 = dice_text::join_dice(&dices);

    let diff = dices[1] - dices[0];
    let side = if diff == 0 {
        "両"
    } else if diff < 0 {
        "ライト"
    } else {
        "ダーク"
    };
    let level = get_success_level(diff);

    let output = if dice_text == dice_text2 {
        format!("(IH) ＞ {dice_text} ＞ {side}{level}")
    } else {
        format!("({cl_text}IH{abilities_text}) ＞ [{dice_text}] ＞ {dice_text2} ＞ {side}{level}")
    };

    let result = if diff > 0 {
        EvalResult::success(output)
    } else if diff < 0 {
        EvalResult::failure(output)
    } else {
        // Ruby: Result.new.tap { |result| result.text = output }（フラグは立たない）
        EvalResult::with_text(output)
    };

    Ok(Some(result))
}

/// Ruby `TheIndieHack#get_success_level`。
fn get_success_level(die_difference: i64) -> &'static str {
    match die_difference.abs() {
        0 => "陣営がそれぞれ確定描写を1つ追加します",
        1 => "陣営が確定描写を1つ追加しますが、味方によって追加されたネガティブな確定描写を1つ受けます",
        2 => "陣営が確定描写を1つ追加します",
        3 => "陣営が確定描写を1つ追加し、さらに場面描写を1つ追加します",
        4 => "陣営が確定描写を1つ追加し、さらにその陣営の味方ひとりも確定描写を1つ追加します",
        _ => "陣営が確定描写を2つ追加します",
    }
}

/// Ruby `String#to_i`（`nil.to_i == 0`、先頭の `+` / `-` を解釈する）。
fn to_i(source: &str) -> i64 {
    source.parse().unwrap_or(0)
}

/// Ruby `BCDice::GameSystem::TheIndieHack`（ID: `TheIndieHack`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheIndieHack;

impl GameSystem for TheIndieHack {
    fn id(&self) -> &'static str {
        "TheIndieHack"
    }

    fn name(&self) -> &'static str {
        "The Indie Hack"
    }

    fn sort_key(&self) -> &'static str {
        "しいんていはつく"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　cIH+a        c:CL  a:能力値

例)IH: ライトダイスとダークダイスを1個ずつ振って、その結果を表示
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"([+-]?\d)?IH"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(resolute_action(command, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TheIndieHack",
            "TheIndieHack.toml",
            8,
        );
    }
}
