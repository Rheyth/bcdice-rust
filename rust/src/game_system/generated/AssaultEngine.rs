//! P4で手書き移植した `lib/bcdice/game_system/AssaultEngine.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `AssaultEngine#eval_game_system_specific_command`（`AEt` / `nAEt` / `AESt`）
//! - `judge` / `return_result` / `format00`

use crate::command_parser::Parser;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::AssaultEngine`（ID: `AssaultEngine`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssaultEngine;

impl GameSystem for AssaultEngine {
    fn id(&self) -> &'static str {
        "AssaultEngine"
    }

    fn name(&self) -> &'static str {
        "アサルトエンジン"
    }

    fn sort_key(&self) -> &'static str {
        "あさるとえんしん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 AEt (t目標値)
    例: AE45 （目標値45）
・リロール nAEt (nロール前の値、t目標値)
    例: 76AE45 (目標値45で、76を振り直す)

・スワップ（t目標値） エネミーブックP11
    例: AES45 （目標値45、スワップ表示あり）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*AE"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AssaultEngine#eval_game_system_specific_command`。
    ///
    /// Ruby `initialize` の `@round_type = RoundType::FLOOR` は `Base` の既定値と同じ
    /// なので、アクセサの上書きは不要。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let parser = Parser::new(&["AES?"], self.round_type())
            .enable_prefix_number()
            .has_suffix_number();
        let Some(cmd) = parser.parse(command) else {
            return Ok(None);
        };

        // Ruby: has_suffix_number なので suffix_number は必ず存在する
        let mut target = cmd
            .suffix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0);
        if target >= 100 {
            target = 99;
        }

        let result = if cmd.command.contains("AES") {
            // SWAP初回
            let total = rng.roll_once(100)? % 100; // 0-99
            let swap = (total % 10) * 10 + (total / 10);
            let r1 = judge(target, total);
            let r2 = judge(target, swap);
            let text = format!(
                "(AES{}) ＞ {} / スワップ{}",
                format00(target),
                r1.text,
                r2.text
            );
            return_result(&r1, &r2, text)
        } else if cmd.prefix_number.is_none() {
            // 初回ロール
            let total = rng.roll_once(100)? % 100; // 0-99
            let mut r = judge(target, total);
            r.text = format!("(AE{}) ＞ {}", format00(target), r.text);
            r
        } else {
            // リロール
            let now = cmd
                .prefix_number
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0);
            let die = rng.roll_once(10)? % 10; // 0-9
            let new1 = judge(target, (now / 10 * 10) + die); // 1の位を振り直す
            let new2 = judge(target, now % 10 + die * 10); // 10の位を振り直す

            let text = format!(
                "({}AE{}) ＞ {die} ＞ {} / {}",
                format00(now),
                format00(target),
                new1.text,
                new2.text
            );
            return_result(&new1, &new2, text)
        };

        Ok(Some(SpecificCommandOutput::result(result)))
    }
}

/// Ruby `AssaultEngine#format00`（`format("%02d", dice)`）。
fn format00(dice: i64) -> String {
    format!("{dice:02}")
}

/// Ruby `AssaultEngine#return_result`。
fn return_result(result1: &EvalResult, result2: &EvalResult, text: String) -> EvalResult {
    if result1.critical || result2.critical {
        EvalResult::critical(text)
    } else if result1.success || result2.success {
        EvalResult::success(text)
    } else if result1.fumble && result2.fumble {
        EvalResult::fumble(text)
    } else {
        EvalResult::failure(text)
    }
}

/// Ruby `AssaultEngine#judge`。
fn judge(target: i64, total: i64) -> EvalResult {
    let double = (total / 10) == (total % 10);
    let total_text = format00(total);
    if total <= target {
        if double {
            EvalResult::critical(format!("({total_text})クリティカル"))
        } else {
            EvalResult::success(format!("({total_text})成功"))
        }
    } else if double {
        EvalResult::fumble(format!("({total_text})ファンブル"))
    } else {
        EvalResult::failure(format!("({total_text})失敗"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "AssaultEngine",
            "AssaultEngine.toml",
            18,
        );
    }
}
