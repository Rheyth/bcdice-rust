//! P4で手書き移植した `lib/bcdice/game_system/AngelGear.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `AngelGear#eval_game_system_specific_command` → `resolute_action`（判定 `nAG[s][±a]`）
//! - `TABLES`（感情表 `ET`）と `Base#roll_tables`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::D66GridTable;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::AngelGear`（ID: `AngelGear`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AngelGear;

impl GameSystem for AngelGear {
    fn id(&self) -> &'static str {
        "AngelGear"
    }

    fn name(&self) -> &'static str {
        "エンゼルギア 天使大戦TRPG The 2nd Editon"
    }

    fn sort_key(&self) -> &'static str {
        "えんせるきあ2"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　nAG[s][±a]
[]内は省略可能。
n:判定値
s:技能値
a:修正
（例）
12AG 10AG3±20

・感情表　ET
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AG", "ET"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AngelGear#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `AngelGear#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: if (m = /^(\d+)AG(\d+)?(([+-]\d+)*)$/.match(command))
        if let Some(captures) = action_pattern().captures(command) {
            let num_dice = to_i(&captures[1]);
            // Ruby: m[2]&.to_i（技能値の指定が無ければ nil）
            let skill_value = captures.get(2).map(|m| to_i(m.as_str()));
            let modify = captures.get(3).map_or("", |m| m.as_str());

            let result = resolute_action(num_dice, skill_value, modify, command, rng)?;
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        // Ruby: else roll_tables(command, TABLES)
        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `/^(\d+)AG(\d+)?(([+-]\d+)*)$/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)AG(\d+)?(([+-]\d+)*)$").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `AngelGear#resolute_action`。
fn resolute_action(
    num_dice: i64,
    skill_value: Option<i64>,
    modify: &str,
    command: &str,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let mut dice = rng.roll_barabara(num_dice, 6)?;
    dice.sort_unstable();
    let dice_text = dice
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut modify_n = 0i64;
    let mut success = 0i64;
    // 技能値の指定が無いと成功数も修正値も0のまま。修正値だけ書いても無視される
    // （原典が `if skill_value` の中で両方を計算しているため）。
    if let Some(skill_value) = skill_value {
        success = dice.iter().filter(|v| **v <= skill_value).count() as i64;
        if !modify.is_empty() {
            // 修正値は `(([+-]\d+)*)` に一致した文字列なので必ず評価できる。
            modify_n = arithmetic::eval(modify, RoundType::Floor)?
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0);
        }
    }

    let total = success.saturating_add(modify_n);
    // Ruby: gospel = '(福音発生)' if success + modify_n >= 100（それ以外は nil ＝ 空文字列）
    let gospel = if total >= 100 { "(福音発生)" } else { "" };

    // Ruby: format('%+d', modify_n) は0でも符号を出す（`Format.modifier` とは異なる）
    let output =
        format!("({command}) ＞ {success}[{dice_text}]{modify_n:+} ＞ 成功数: {total}{gospel}");

    if total >= 100 {
        Ok(EvalResult::critical(output))
    } else if total > 0 {
        Ok(EvalResult::success(output))
    } else {
        Ok(EvalResult::failure(output))
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    table_helpers::roll_table(command, TABLES, rng)
}

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &D66GridTable)] = &[("ET", &EMOTION_TABLE)];

/// Ruby `TABLES['ET']`（感情表）。
static EMOTION_TABLE: D66GridTable = D66GridTable::new("感情表", EMOTION_TABLE_ITEMS);

/// 感情表の項目（行=左のダイス、列=右のダイス）。
static EMOTION_TABLE_ITEMS: &[&[&str]] = &[
    &[
        "好奇心（好奇心）",
        "憧れ（あこがれ）",
        "尊敬（そんけい）",
        "仲間意識（なかまいしき）",
        "母性愛（ぼせいあい）",
        "感心（かんしん）",
    ],
    &[
        "純愛（じゅんあい）",
        "友情（ゆうじょう）",
        "同情（どうじょう）",
        "父性愛（ふせいあい）",
        "幸福感（こうふくかん）",
        "信頼（しんらい）",
    ],
    &[
        "競争心（きょうそうしん）",
        "親近感（しんきんかん）",
        "まごころ",
        "好意（こうい）",
        "有為（ゆうい）",
        "崇拝（すうはい）",
    ],
    &[
        "大嫌い（だいきらい）",
        "妬み（ねたみ）",
        "侮蔑（ぶべつ）",
        "腐れ縁（くされえん）",
        "恐怖（きょうふ）",
        "劣等感（れっとうかん）",
    ],
    &[
        "偏愛（へんあい）",
        "寂しさ（さびしさ）",
        "憐憫（れんびん）",
        "闘争心（とうそうしん）",
        "食傷（しょくしょう）",
        "嘘つき（うそつき）",
    ],
    &[
        "甘え（あまえ）",
        "苛立ち（いらだち）",
        "下心（したごころ）",
        "憎悪（ぞうお）",
        "疑惑（ぎわく）",
        "支配（しはい）",
    ],
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "AngelGear",
            "AngelGear.toml",
            12,
        );
    }
}
