//! P4で手書き移植した `lib/bcdice/game_system/Raisondetre.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`RD` / `DD` の振り分け）
//! - `#checkRoll`（判定 `[判定値]RD[技能][@目標値]`）
//! - `#checkDamage`（ダメージロール `[ダイス数]DD[装甲]`）
//!
//! # `checkRoll` と `checkDamage` の非対称性
//!
//! 原典は似た処理を2つ持つが、ソートの位置が違うので共通化していない。
//!
//! - `checkRoll`: `diceArray.sort.join` は**コピーを整列**して表示するだけで、
//!   `diceArray` 自体は「10→0 の読み替え」と「補正の減算」を経た未整列のまま
//!   ファンブル判定・`count(0)`・`reverse` に使われる。
//! - `checkDamage`: `map { 10 -> 0 }.sort` した配列をそのまま使い、
//!   `criticalCount` は補正を引く**前**に数える。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raisondetre;

impl GameSystem for Raisondetre {
    fn id(&self) -> &'static str {
        "Raisondetre"
    }

    fn name(&self) -> &'static str {
        "叛逆レゾンデートル"
    }

    fn sort_key(&self) -> &'static str {
        "はんきやくれそんてとおる"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Raisondetre#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

static HELP_MESSAGE: &str = r"判定：[判定値]RD[技能][@目標値]
ダメージロール：[ダイス数]DD[装甲]

[]内のコマンドは省略可能。
「判定値」で判定に使用するダイス数を指定。省略時は「1」。0以下も指定可。
「技能」で有効なダイス数を指定。省略時は「1」。
達成値はクリティカルを含めて、「最も高くなる」ように計算します。
「@目標値」指定で、判定の成否を追加表示します。

ダメージロールは[装甲]指定で、有効なダイス数と0の出目の数を表示します。
[装甲]省略時は、ダイス結果のみ表示します。（複数の対象への攻撃時用）

【書式例】
・RD → 1Dで達成値を表示。
・2RD1@8 → 2D（1個選択）で目標値8の判定。
・-3RD → 1Dでダイスペナルティ-4の判定。
・4DD2 → 4Dで装甲2のダメージロール。
";

static PREFIXES: &[&str] = &[r"(-)?(\d+)?RD", r"(-)?(\d+)?DD"];

/// Ruby `#eval_game_system_specific_command`。
fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static RD_RE: OnceLock<Regex> = OnceLock::new();
    static DD_RE: OnceLock<Regex> = OnceLock::new();

    let rd_re =
        RD_RE.get_or_init(|| Regex::new(r"(?i)(-)?(\d+)?RD(\d+)?(@(\d+))?$").expect("valid regex"));
    let dd_re = DD_RE
        .get_or_init(|| Regex::new(r"(?i)(-)?(\d+)?DD([1-9])?([+-]\d+)?$").expect("valid regex"));

    if let Some(m) = rd_re.captures(command) {
        let dice_count = dice_count_of(&m);
        // Ruby: choiceCount = (Regexp.last_match(3) || 1).to_i
        let choice_count = m.get(3).map_or(1, |v| to_i(v.as_str()));
        // Ruby: target = (Regexp.last_match(5) || 0).to_i
        let target = m.get(5).map_or(0, |v| to_i(v.as_str()));

        return Ok(Some(SpecificCommandOutput::text(check_roll(
            dice_count,
            choice_count,
            target,
            rng,
        )?)));
    }

    if let Some(m) = dd_re.captures(command) {
        let dice_count = dice_count_of(&m);
        // Ruby: armor = (Regexp.last_match(3) || 0).to_i
        let mut armor = m.get(3).map_or(0, |v| to_i(v.as_str()));
        if armor > 0 {
            armor = armor.saturating_add(m.get(4).map_or(0, |v| to_i(v.as_str())));
            armor = armor.clamp(1, 9);
        }

        return Ok(Some(SpecificCommandOutput::text(check_damage(
            dice_count, armor, rng,
        )?)));
    }

    Ok(None)
}

/// Ruby の共通部分:
/// `diceCount = (Regexp.last_match(2) || 1).to_i; diceCount *= -1 unless Regexp.last_match(1).nil?`
fn dice_count_of(m: &Captures<'_>) -> i64 {
    let dice_count = m.get(2).map_or(1, |v| to_i(v.as_str()));
    if m.get(1).is_some() {
        -dice_count
    } else {
        dice_count
    }
}

/// Ruby `#checkRoll`。
fn check_roll(
    dice_count: i64,
    choice_count: i64,
    target: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let (correction, roll_count) = correction_and_roll_count(dice_count);

    let mut rolled = rng.roll_barabara(roll_count, 10)?;
    rolled.sort_unstable();
    let dice_text = join(&rolled);

    // Ruby: diceArray.map! { |x| x == 10 ? 0 : x }; diceArray.map! { |i| i - correction }
    let dice_array: Vec<i64> = rolled
        .iter()
        .map(|x| if *x == 10 { 0 } else { *x })
        .map(|i| i.saturating_sub(correction))
        .collect();

    // Ruby: diceText2 = diceArray.sort.join(',')（`sort` はコピーを返すので `dice_array` は未整列のまま）
    let dice_text2 = {
        let mut sorted = dice_array.clone();
        sorted.sort_unstable();
        join(&sorted)
    };

    let funble_count = dice_array.iter().filter(|i| **i <= 1).count() as i64;
    let is_funble = funble_count >= roll_count;

    let mut dice = 0i64;
    let mut success = 0i64;
    let mut critical = 0i64;
    let mut critical_count = 0i64;
    let mut choice_text = String::new();

    if !is_funble {
        critical_count = dice_array.iter().filter(|i| **i == 0).count() as i64;
        critical = critical_count.saturating_mul(10);

        let mut choice_array: Vec<i64> = dice_array.iter().rev().copied().collect();
        // Ruby `Array#delete(0)` は 0 を全て取り除く
        choice_array.retain(|v| *v != 0);
        // Ruby: choiceArray.slice(0..(choiceCount - 1))（choiceCount が 0 なら 0..-1 ＝ 全体）
        if choice_count > 0 {
            choice_array.truncate(choice_count as usize);
        }
        choice_text = join(&choice_array);
        // Ruby `inject(:+)` は空配列だと nil になり直後の加算でクラッシュするが、
        // 0 を全て取り除いて空になるのは全要素が 0（＝ファンブル）のときだけなので到達しない。
        dice = sum(&choice_array);
        success = dice.saturating_add(critical);
    }

    let mut result = format!("{roll_count}D10");
    if correction > 0 {
        result.push_str(&format!("-{correction}"));
    }
    result.push_str(&format!(" ＞ [{dice_text}] ＞ [{dice_text2}] ＞ "));

    if is_funble {
        result.push_str("達成値：0 (Funble)");
    } else {
        result.push_str(&format!("{dice}[{choice_text}]"));
        if critical > 0 {
            result.push_str(&format!("+{critical}"));
        }
        result.push_str(&format!("=達成値：{success}"));
        if critical > 0 {
            result.push_str(&format!(" ({critical_count}Critical)"));
        }
    }

    if target > 0 {
        result.push_str(&format!(">={target} "));
        result.push_str(if success >= target {
            "【成功】"
        } else {
            "【失敗】"
        });
    }

    Ok(result)
}

/// Ruby `#checkDamage`。
fn check_damage(dice_count: i64, armor: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let (correction, roll_count) = correction_and_roll_count(dice_count);

    let mut dice_list = rng.roll_barabara(roll_count, 10)?;
    dice_list.sort_unstable();
    let dice_text = join(&dice_list);

    // Ruby: diceArray = dice_list.map { |x| x == 10 ? 0 : x }.sort
    let mut dice_array: Vec<i64> = dice_list
        .iter()
        .map(|x| if *x == 10 { 0 } else { *x })
        .collect();
    dice_array.sort_unstable();
    // Ruby: criticalCount は補正を引く前に数える
    let critical_count = dice_array.iter().filter(|i| **i == 0).count();
    let dice_array: Vec<i64> = dice_array
        .iter()
        .map(|i| i.saturating_sub(correction))
        .collect();
    let dice_text2 = join(&dice_array);

    let mut result = format!("{roll_count}D10");
    if correction > 0 {
        result.push_str(&format!("-{correction}"));
    }
    result.push_str(&format!(" ＞ [{dice_text}] ＞ [{dice_text2}]"));

    if armor > 0 {
        let mut result_array: Vec<String> = Vec::new();
        let mut success = 0i64;

        for i in &dice_array {
            if *i >= armor {
                result_array.push(i.to_string());
                success += 1;
            } else {
                result_array.push("×".to_owned());
            }
        }

        result.push_str(&format!(
            " ＞ [{}]>={armor} 有効数：{success}",
            result_array.join(",")
        ));
    }

    result.push_str(&format!("　0={critical_count}個"));

    Ok(result)
}

/// Ruby の `checkRoll` / `checkDamage` 冒頭にある共通の分岐。
fn correction_and_roll_count(dice_count: i64) -> (i64, i64) {
    if dice_count <= 0 {
        // Ruby: correction = 1 + diceCount * -1
        (1i64.saturating_sub(dice_count), 1)
    } else {
        (0, dice_count)
    }
}

fn sum(values: &[i64]) -> i64 {
    values.iter().fold(0i64, |a, b| a.saturating_add(*b))
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `String#to_i`。桁あふれは Ruby だと Bignum になるので i64 の端へ飽和させる。
fn to_i(text: &str) -> i64 {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return 0;
    }
    let value: i64 = rest[..end].parse().unwrap_or(i64::MAX);
    if negative {
        -value
    } else {
        value
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
            .join("test/data/Raisondetre.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Raisondetre.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/Raisondetre.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Raisondetre.toml must parse");
        assert_eq!(
            data.tests.len(),
            25,
            "case count in test/data/Raisondetre.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Raisondetre",
                "unexpected game system in Raisondetre.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Raisondetre"), &tc.input, &mut src) {
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
                    "FAIL Raisondetre:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Raisondetre cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
