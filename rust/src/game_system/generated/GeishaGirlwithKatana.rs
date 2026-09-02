//! P4で手書き移植した `lib/bcdice/game_system/GeishaGirlwithKatana.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`GK` / `GK#n` の判定と `GL` のチョムバ）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::GeishaGirlwithKatana`（ID: `GeishaGirlwithKatana`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeishaGirlwithKatana;

impl GameSystem for GeishaGirlwithKatana {
    fn id(&self) -> &'static str {
        "GeishaGirlwithKatana"
    }

    fn name(&self) -> &'static str {
        "ゲイシャ・ガール・ウィズ・カタナ"
    }

    fn sort_key(&self) -> &'static str {
        "けいしやかあるういすかたな"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["GK", "GL"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: /^GL$/i =~ command
        if command == "GL" {
            return Ok(Some(SpecificCommandOutput::text(chomba_result_text())));
        }

        // Ruby: unless /^GK(#(\d+))?$/i =~ command
        let Some(m) = gk_pattern().captures(command) else {
            return Ok(None);
        };
        let chomba_counter = m.get(2).map(|x| to_i(x.as_str()));

        if is_chomba(chomba_counter, rng)? {
            return Ok(Some(SpecificCommandOutput::text(chomba_result_text())));
        }

        let mut dice_list = rng.roll_barabara(3, 6)?;
        dice_list.sort_unstable();

        if let Some(yaku) = yaku(&dice_list) {
            return Ok(Some(SpecificCommandOutput::text(result_text_by_dice(
                &dice_list,
                &format!("【役】{yaku}"),
            ))));
        }

        let (deme, zorome) = deme_zorome(&dice_list);
        if deme == 0 {
            return Ok(Some(SpecificCommandOutput::text(result_text_by_dice(
                &dice_list, "失敗",
            ))));
        }

        let yp = if zorome == 1 { " YPが1増加" } else { "" };
        Ok(Some(SpecificCommandOutput::text(result_text_by_dice(
            &dice_list,
            &format!("達成値{deme}{yp}"),
        ))))
    }
}

const HELP_MESSAGE: &str = r"・判定 (GK#n)
  役やチョムバを含めて1回分のダイスロールを判定します。
　役は　（通常判定）／（戦闘時）　の順で両方出力されます。
  GK のみの場合5%の確率でチョムバます。
  GK#3 の様に #n をつけることによってチョムバの確率をn%にすることができます。
　例）GK　GK#10
・隠しコマンド (GL)
  必ずチョムバします。GMが空気を読んでチョムバさせたいときや、
  GKコマンドを打ち間違えてチョムバするを想定してます。
　例）GL
";

/// Ruby `/^GK(#(\d+))?$/i`。
fn gk_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^GK(#(\d+))?$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当（桁あふれは飽和させる）。
fn to_i(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `GeishaGirlwithKatana#isChomba`。
fn is_chomba(chomba_counter: Option<i64>, rng: &mut Randomizer) -> Result<bool, EvalError> {
    let chomba_counter = chomba_counter.unwrap_or(5);
    let chomba = rng.roll_once(100)?;
    Ok(chomba <= chomba_counter)
}

/// Ruby `GeishaGirlwithKatana#getChombaResultText`。
fn chomba_result_text() -> String {
    result_text("チョムバ！！")
}

/// Ruby `GeishaGirlwithKatana#getYaku`。
fn yaku(dice_list: &[i64]) -> Option<&'static str> {
    match dice_list {
        [1, 2, 3] => Some("自動失敗／自分の装甲効果無しでダメージを受けてしまう"),
        [4, 5, 6] => Some("自動成功／敵の装甲を無視してダメージを与える"),
        [1, 1, 1] => Some("10倍成功 YPが10増加／10倍ダメージ YPが10増加"),
        [2, 2, 2] => Some("2倍成功／2倍ダメージ"),
        [3, 3, 3] => Some("3倍成功／3倍ダメージ"),
        [4, 4, 4] => Some("4倍成功／4倍ダメージ"),
        [5, 5, 5] => Some("5倍成功／5倍ダメージ"),
        [6, 6, 6] => Some("6倍成功／6倍ダメージ"),
        _ => None,
    }
}

/// Ruby `GeishaGirlwithKatana#getDemeZorome`。
fn deme_zorome(dice_list: &[i64]) -> (i64, i64) {
    if dice_list[0] == dice_list[1] {
        (dice_list[2], dice_list[0])
    } else if dice_list[1] == dice_list[2] {
        (dice_list[0], dice_list[1])
    } else {
        (0, 0)
    }
}

/// Ruby `GeishaGirlwithKatana#getResultTextByDice`。
fn result_text_by_dice(dice_list: &[i64], result: &str) -> String {
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    result_text(&format!("{dice_text} ＞ {result}"))
}

/// Ruby `GeishaGirlwithKatana#getResultText`。
fn result_text(result: &str) -> String {
    format!("(3B6) ＞ {result}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GeishaGirlwithKatana",
            "GeishaGirlwithKatana.toml",
            22,
        );
    }
}
