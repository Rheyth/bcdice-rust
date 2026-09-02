//! P4で手書き移植した `lib/bcdice/game_system/GoldenSkyStories.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GoldenSkyStories#eval_game_system_specific_command`（下駄占い `GETA`）
//! - `#getaRoll`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `case command when /geta/i`（部分一致・大文字小文字を無視）。
fn geta_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)geta").expect("valid regex"))
}

/// Ruby `GoldenSkyStories#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if !geta_pattern().is_match(command) {
        // Ruby: result が '' のままなので nil
        return Ok(None);
    }

    // Ruby: getaRoll は定型文を必ず含むので `result.empty?` にはならない
    let result = geta_roll(rng)?;
    Ok(Some(SpecificCommandOutput::text(format!(
        "{command} ＞ {result}"
    ))))
}

/// Ruby `GoldenSkyStories#getaRoll`。
fn geta_roll(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice = rng.roll_once(7)?;

    // Ruby: case dice ... else '' （1〜7以外は空文字列のまま）
    let geta_string = match dice {
        1 => "裏：あめ",
        2 => "表：はれ",
        3 => "裏：あめ",
        4 => "表：はれ",
        5 => "裏：あめ",
        6 => "表：はれ",
        7 => "横：くもり",
        _ => "",
    };

    Ok(format!("下駄占い ＞ {geta_string}"))
}

/// Ruby `BCDice::GameSystem::GoldenSkyStories`（ID: `GoldenSkyStories`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenSkyStories;

impl GameSystem for GoldenSkyStories {
    fn id(&self) -> &'static str {
        "GoldenSkyStories"
    }

    fn name(&self) -> &'static str {
        "ゆうやけこやけ"
    }

    fn sort_key(&self) -> &'static str {
        "ゆうやけこやけ"
    }

    fn help_message(&self) -> &'static str {
        r"※「ゆうやけこやけ」はダイスロールを使用しないシステムです。
※このダイスボットは部屋のシステム名表示用となります。

・下駄占い (GETA)
  あーしたてんきになーれ
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["geta"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `GoldenSkyStories#initialize` の `@enabled_upcase_input = false`。
    fn enabled_upcase_input(&self) -> bool {
        false
    }

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
            "GoldenSkyStories",
            "GoldenSkyStories.toml",
            6,
        );
    }
}
