//! trait経由の評価インフラを検証するためのダミーゲームシステム。
//!
//! Ruby本家に対応するシステムは無い。P3-Batch1 の完了条件
//! 「ダミーのダイスボット1つを trait で登録し、TOMLハーネスから呼べること」を
//! 満たすための最小実装で、以下を1つのシステムで通す:
//!
//! - `prefixes` / `prefixes_pattern` によるゲームシステム固有コマンドの振り分け
//!   （Ruby `Base#dice_command`）
//! - `eval_game_system_specific_command` の3通りの戻り値
//!   （文字列 / `Result` / `"1"`＝該当なし）
//! - [`DiceTable`](crate::dice_table) をゲームシステムから `static` で使う形
//! - 既定値以外の設定（`sort_barabara_dice`）が共通コマンドまで届くこと
//!
//! 全336システムの登録完了に伴いレジストリからは外れている
//! （`game_system_class("DummySystem")` は `None` を返す）。
//! 本ファイルは上記の評価パスを検証するフィクスチャとして残しており、
//! 単体テストから直接 `eval_raw` 経由で使う。

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// ダミーのダイスボット。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DummySystem;

/// 固有コマンド `DUMT` が引く表。
static DUMMY_TABLE: Table = Table::from_dice(
    "ダミー表",
    1,
    6,
    &[
        "ダミー1",
        "ダミー2",
        "ダミー3",
        "ダミー4",
        "ダミー5",
        "ダミー6",
    ],
);

impl GameSystem for DummySystem {
    fn id(&self) -> &'static str {
        "DummySystem"
    }

    fn name(&self) -> &'static str {
        "ダミーシステム"
    }

    fn sort_key(&self) -> &'static str {
        "たみいしすてむ"
    }

    fn help_message(&self) -> &'static str {
        "\
DUMT ：ダミー表を引く（1D6）
DUMC ：ダミー判定（常に成功する Result を返す）
DUM  ：該当なし（Ruby の \"1\" と同じく nil に畳まれる）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["DUM"]
    }

    crate::impl_prefixes_pattern!();

    /// 既定値以外の設定が共通コマンドまで届くことを確かめるために立てている。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        match command {
            // 文字列を返す経路（Ruby: roll_tables の `table.roll(@randomizer).to_s`）
            "DUMT" => Ok(Some(SpecificCommandOutput::text(
                DUMMY_TABLE.roll(rng)?.to_string(),
            ))),
            // Result を返す経路（success/failure フラグを立てられる）
            "DUMC" => Ok(Some(SpecificCommandOutput::result(EvalResult::success(
                "ダミー判定 ＞ 成功",
            )))),
            // Ruby が「表引きに失敗した」ことを示す番兵。dice_command が nil に畳む
            "DUM" => Ok(Some(SpecificCommandOutput::text("1"))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::eval_raw;
    use crate::game_system::game_system_class;
    use crate::randomizer::SeededRandomizer;

    fn eval(input: &str, rands: &[(i64, i64)]) -> Option<EvalResult> {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let result = {
            let mut rng = Randomizer::new(&mut src);
            eval_raw(&DummySystem, input, &mut rng).expect("no eval error")
        };
        assert!(src.is_empty(), "unconsumed rands for {input:?}");
        result
    }

    #[test]
    fn prefix_command_returning_text_is_evaluated() {
        let result = eval("DUMT", &[(3, 6)]).expect("recognized");
        assert_eq!(result.text, "ダミー表(3) ＞ ダミー3");
        assert!(!result.secret);
    }

    #[test]
    fn prefix_command_is_upcased_before_matching() {
        // Ruby: command = command.upcase if @enabled_upcase_input
        let result = eval("dumt", &[(6, 6)]).expect("recognized");
        assert_eq!(result.text, "ダミー表(6) ＞ ダミー6");
    }

    #[test]
    fn leading_s_marks_secret_and_is_stripped() {
        let result = eval("SDUMT", &[(1, 6)]).expect("recognized");
        assert_eq!(result.text, "ダミー表(1) ＞ ダミー1");
        assert!(result.secret);
    }

    #[test]
    fn result_output_keeps_flags_and_ors_secret() {
        let result = eval("DUMC", &[]).expect("recognized");
        assert_eq!(result.text, "ダミー判定 ＞ 成功");
        assert!(result.success);
        assert!(!result.secret);

        let secret = eval("SDUMC", &[]).expect("recognized");
        assert!(secret.success && secret.secret);
    }

    #[test]
    fn sentinel_one_is_folded_to_nil() {
        // Ruby: return nil if output.nil? || output.empty? || output == "1"
        assert!(eval("DUM", &[]).is_none());
    }

    #[test]
    fn unknown_command_with_prefix_falls_through() {
        // 接頭辞にはマッチするが固有コマンドではない → 共通コマンドも該当なし
        assert!(eval("DUMX", &[]).is_none());
    }

    #[test]
    fn common_commands_still_work_with_system_settings() {
        // sort_barabara_dice = true が共通コマンドに効いている
        let result = eval("3B6", &[(5, 6), (1, 6), (3, 6)]).expect("recognized");
        assert_eq!(result.text, "(3B6) ＞ 1,3,5");
    }

    #[test]
    fn dice_bot_is_unaffected_by_dummy_prefixes() {
        // prefixes_pattern のキャッシュがシステム間で共有されていないことの確認。
        // 共有されていると DiceBot が DummySystem のパターンで判定されてしまう。
        let dice_bot = game_system_class("DiceBot").expect("registered");
        assert!(dice_bot.prefixes_pattern().is_none());

        let mut src = SeededRandomizer::new(Vec::new());
        let mut rng = Randomizer::new(&mut src);
        assert!(eval_raw(dice_bot, "DUMT", &mut rng)
            .expect("no eval error")
            .is_none());
    }
}
