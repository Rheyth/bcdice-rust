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
use crate::dice_table::{D66GridTable, RollableTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
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

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 判定値は `roll_barabara` の上限、技能値は出目との比較にしか使わないので、
/// 飽和させても分岐は変わらない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
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

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/AngelGear.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/AngelGear.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/AngelGear.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("AngelGear.toml must parse");
        assert_eq!(
            data.tests.len(),
            12,
            "case count in test/data/AngelGear.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "AngelGear",
                "unexpected game system in AngelGear.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("AngelGear"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL AngelGear:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} AngelGear cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
