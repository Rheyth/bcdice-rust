//! P4で手書き移植した `lib/bcdice/game_system/ZettaiReido.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ZettaiReido#roll2DR`（判定 `m-2DR+n>=x`）
//! - `ZettaiReido#roll2DarkDice` / `#changeDiceToDarkDice`（闇のダイスとDP）
//! - `ZettaiReido#getModInfo` / `#getDiffInfo` / `#getResult`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::ZettaiReido`（ID: `ZettaiReido`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZettaiReido;

impl GameSystem for ZettaiReido {
    fn id(&self) -> &'static str {
        "ZettaiReido"
    }

    fn name(&self) -> &'static str {
        "絶対隷奴"
    }

    fn sort_key(&self) -> &'static str {
        "せつたいれいと"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
m-2DR+n>=x
m(基本能力),n(修正値),x(目標値)
DPの取得の有無も表示されます。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+\-2DR"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ZettaiReido#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let Some(m) = command_pattern().captures(command) else {
            return Ok(None);
        };

        let base_avility = to_i_saturating(&m[1]);
        let mod_text = m.get(2).map_or("", |g| g.as_str());
        let diff_value = m.get(4).map(|g| g.as_str());

        Ok(Some(SpecificCommandOutput::result(roll_2dr(
            base_avility,
            mod_text,
            diff_value,
            rng,
        )?)))
    }
}

/// Ruby `/^(\d+)-2DR([+\-\d]*)(>=(\d+))?$/i`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^([0-9]+)-2DR([+\-0-9]*)(>=([0-9]+))?$").expect("valid regex")
    })
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i_saturating(text: &str) -> i64 {
    str_helpers::to_i_max(text)
}

/// Ruby `ZettaiReido#roll2DR`。
fn roll_2dr(
    base_avility: i64,
    mod_text: &str,
    diff_value: Option<&str>,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let (dice_total, dice_text, dark_point) = roll_2dark_dice(rng)?;

    let (mod_value, mod_text) = get_mod_info(mod_text)?;
    let (diff, diff_text) = get_diff_info(diff_value);

    let base_command_text = format!("({base_avility}-2DR{mod_text}{diff_text})");
    let dice_command_text = format!("{base_avility}-{dice_total}[{dice_text}]{mod_text}");
    let total = base_avility
        .saturating_sub(dice_total)
        .saturating_add(mod_value);

    let mut result = get_result(dice_total, total, diff);

    // Ruby: darkPointText = "#{darkPoint}DP" if darkPoint > 0（偽なら nil のまま）
    let dark_point_text = (dark_point > 0).then(|| format!("{dark_point}DP"));

    // Ruby: [...].compact.join(" ＞ ")
    let mut parts = vec![
        base_command_text,
        dice_command_text,
        total.to_string(),
        result.text,
    ];
    if let Some(text) = dark_point_text {
        parts.push(text);
    }
    result.text = parts.join(" ＞ ");

    Ok(result)
}

/// Ruby `ZettaiReido#roll2DarkDice`。戻り値は `[闇のダイスの合計, 出目の表示, DP]`。
fn roll_2dark_dice(rng: &mut Randomizer) -> Result<(i64, String, i64), EvalError> {
    let dice1 = rng.roll_once(6)?;
    let dice2 = rng.roll_once(6)?;

    let (dark_dice1, dark_point1) = change_dice_to_dark_dice(dice1);
    let (dark_dice2, dark_point2) = change_dice_to_dark_dice(dice2);

    let mut dark_point = dark_point1 + dark_point2;
    if dark_point == 2 {
        dark_point = 4;
    }

    Ok((
        dark_dice1 + dark_dice2,
        format!("{dark_dice1},{dark_dice2}"),
        dark_point,
    ))
}

/// Ruby `ZettaiReido#changeDiceToDarkDice`。6は0扱いになりDPを1点得る。
fn change_dice_to_dark_dice(dice: i64) -> (i64, i64) {
    if dice == 6 {
        (0, 1)
    } else {
        (dice, 0)
    }
}

/// Ruby `ZettaiReido#getModInfo`。
///
/// Ruby `ArithmeticEvaluator.eval` は不正な式（空文字列を含む）で0を返す。
fn get_mod_info(mod_text: &str) -> Result<(i64, String), EvalError> {
    let value = arithmetic::eval(mod_text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let text = if value < 0 {
        value.to_string()
    } else if value > 0 {
        format!("+{value}")
    } else {
        String::new()
    };

    Ok((value, text))
}

/// Ruby `ZettaiReido#getDiffInfo`。
fn get_diff_info(diff_value: Option<&str>) -> (Option<i64>, String) {
    match diff_value {
        None => (None, String::new()),
        Some(text) => {
            let value = to_i_saturating(text);
            (Some(value), format!(">={value}"))
        }
    }
}

/// Ruby `ZettaiReido#getResult`。
fn get_result(dice_total: i64, total: i64, diff: Option<i64>) -> EvalResult {
    if dice_total == 0 {
        return EvalResult::critical("クリティカル");
    }

    if dice_total == 10 {
        return EvalResult::fumble("ファンブル");
    }

    // Ruby: diff = 0 if diff.nil?
    let success_level = total.saturating_sub(diff.unwrap_or(0));
    if success_level >= 0 {
        EvalResult::success(format!("{success_level} 成功"))
    } else {
        EvalResult::failure("失敗")
    }
}

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
            .join("test/data/ZettaiReido.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ZettaiReido.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ZettaiReido.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ZettaiReido.toml must parse");
        assert_eq!(
            data.tests.len(),
            15,
            "case count in test/data/ZettaiReido.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ZettaiReido",
                "unexpected game system in ZettaiReido.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ZettaiReido"), &tc.input, &mut src) {
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
                    "FAIL ZettaiReido:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ZettaiReido cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
