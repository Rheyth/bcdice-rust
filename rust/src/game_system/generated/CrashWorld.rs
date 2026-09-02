//! P4で手書き移植した `lib/bcdice/game_system/CrashWorld.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `CrashWorld#eval_game_system_specific_command` → `get_crash_world_roll`（判定 `CWn`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::CrashWorld`（ID: `CrashWorld`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashWorld;

impl GameSystem for CrashWorld {
    fn id(&self) -> &'static str {
        "CrashWorld"
    }

    fn name(&self) -> &'static str {
        "墜落世界"
    }

    fn sort_key(&self) -> &'static str {
        "ついらくせかい"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 CWn
初期目標値n (必須)
例・CW8
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CW"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `CrashWorld#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command when /CW(\d+)/i
        let Some(captures) = command_pattern().captures(command) else {
            return Ok(None);
        };

        let target = to_i(&captures[1]);
        Ok(Some(SpecificCommandOutput::text(get_crash_world_roll(
            target, rng,
        )?)))
    }
}

/// Ruby `/CW(\d+)/i`。前後を固定していないので部分一致でよい（原典どおり）。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)CW(\d+)").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 目標値が12以上の場合、Ruby も本移植も「必ず成功が続く」ため
/// 乱数を使い切る（テストでは注入乱数の枯渇、本番では無限ループ）まで回る。
/// 飽和させてもこの分岐は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `CrashWorld#getCrashWorldRoll`。
fn get_crash_world_roll(mut target: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut output = String::from("(");
    let mut is_end = false;
    let mut successness = 0i64;
    let mut num = 0i64;

    while !is_end {
        num = rng.roll_once(12)?;

        // 振った数字を出力へ書き足す
        if output == "(" {
            output = format!("({num}");
        } else {
            output = format!("{output}, {num}");
        }

        if num <= target || num == 11 {
            // 成功/クリティカル(11)。 次回の目標値を変更して継続
            target = num;
            successness += 1;
        } else if num == 12 {
            // ファンブルなら終了。
            is_end = true;
        } else {
            // target < num < 11で終了
            is_end = true;
        }
    }

    if num == 12 {
        // ファンブルの時、成功度は0
        successness = 0;
    }

    output = format!("{output})  成功度 : {successness}");

    if num == 12 {
        output = format!("{output} ファンブル");
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "CrashWorld",
            "CrashWorld.toml",
            11,
        );
    }
}
