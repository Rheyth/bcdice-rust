//! P4で手書き移植した `lib/bcdice/game_system/Paranoia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Paranoia#eval_game_system_specific_command` と `#getaRoll`（`geta`）
//!
//! `@enabled_upcase_input = false` なので、コマンドは小文字のまま
//! `eval_game_system_specific_command` に渡り、そのまま出力へ埋め込まれる。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `case command when /geta/i`。アンカーが無いので部分一致でよい。
fn geta_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)geta").expect("valid regex"))
}

/// Ruby `Paranoia#eval_game_system_specific_command`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let result = if geta_pattern().is_match(command) {
        geta_roll(rng)?
    } else {
        String::new()
    };

    // Ruby: return nil if result.empty?
    if result.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!("{command} ＞ {result}")))
}

/// Ruby `Paranoia#getaRoll`。
fn geta_roll(rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut result = String::new();

    let dice = rng.roll_once(2)?;

    result.push_str("幸福ですか？ ＞ ");

    let geta_string = match dice {
        1 => "幸福です",
        2 => "幸福ではありません",
        _ => "",
    };

    result.push_str(geta_string);

    Ok(result)
}

/// Ruby `BCDice::GameSystem::Paranoia`（ID: `Paranoia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paranoia;

impl GameSystem for Paranoia {
    fn id(&self) -> &'static str {
        "Paranoia"
    }

    fn name(&self) -> &'static str {
        "パラノイア"
    }

    fn sort_key(&self) -> &'static str {
        "はらのいあ"
    }

    fn help_message(&self) -> &'static str {
        r"※「パラノイア」は完璧なゲームであるため特殊なダイスコマンドを必要としません。
※このダイスボットは部屋のシステム名表示用となります。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["geta"]
    }

    crate::impl_prefixes_pattern!();

    fn enabled_upcase_input(&self) -> bool {
        false
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
        crate::game_system::test_support::assert_toml_cases_strict("Paranoia", "Paranoia.toml", 9);
    }
}
