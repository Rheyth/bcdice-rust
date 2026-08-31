//! P4で手書き移植した `lib/bcdice/game_system/TheUnofficialHollowKnightRPG.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#ability_roll`（能力値ロール `[n]AD[+b][#r][>=t]`）
//! - `#initiative_roll`（イニシアチブロール `[n]INTI[+b][#r]`）
//! - `#number_with_sign_from_int` / `#number_with_reroll_from_int`
//!
//! 区切りは他システムと違い **半角** の `" > "`（Ruby原典どおり）。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::TheUnofficialHollowKnightRPG`（ID: `TheUnofficialHollowKnightRPG`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheUnofficialHollowKnightRPG;

impl GameSystem for TheUnofficialHollowKnightRPG {
    fn id(&self) -> &'static str {
        "TheUnofficialHollowKnightRPG"
    }

    fn name(&self) -> &'static str {
        "The Unofficial Hollow Knight RPG"
    }

    fn sort_key(&self) -> &'static str {
        "しあんおふいしやるほろうないとRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・能力値判定　[n]AD[+b][#r][>=t]
　n: 能力値。小数可。省略不可。
　b: ボーナス、ペナルティダイス。省略可。
　r: 追加リロールダイス数。省略可。
　t: 目標値。>=含めて省略可。
　成功数を判定。
　例）1AD, 2.5AD, 1.5AD+1, 2AD#1, 2.5AD+2#2>=4

・イニシアチブ　[n]INTI[+b][#r]
　n: イニシアチブに使う能力値。省略不可。
  b: ボーナス、ペナルティダイス。省略可。
  r: 追加リロールダイス数。省略可。
　振り直しを行ったうえでイニシアチブ値を計算。
　例）1INTI, 2.5INTI, 1.5INTI+1, 2INTI#1, 2.5INTI+2#2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"(\d+\.?\d*)?AD([+-](\d+))?(#(\d+))?(>=(\d+))?",
            r"(\d+\.?\d*)?(INTI|inti)([+-](\d+))?(#(\d+))?",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#eval_game_system_specific_command`: `ability_roll(command) || initiative_roll(command)`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = ability_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        if let Some(text) = initiative_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(None)
    }
}

/// Ruby `#number_with_sign_from_int`。
fn number_with_sign_from_int(number: i64) -> String {
    if number == 0 {
        String::new()
    } else if number > 0 {
        format!("+{}", number.unsigned_abs())
    } else {
        format!("-{}", number.unsigned_abs())
    }
}

/// Ruby `#number_with_reroll_from_int`。
fn number_with_reroll_from_int(number: i64) -> String {
    if number == 0 {
        String::new()
    } else if number > 0 {
        format!("#{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `String#to_f`（マッチしなかった場合は `nil.to_f` = `0.0`）。
///
/// 捕獲する形は `\d+\.?\d*` なので、Rust の `f64` パーサでそのまま解釈できる。
fn capture_to_f(c: Option<regex::Match<'_>>) -> f64 {
    c.and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Ruby `String#to_i`（マッチしなかった場合・空文字列は `0`）。
///
/// 捕獲する形は `\d*` / `[+-]\d+` なので、符号つきもそのまま解釈できる。
/// 桁あふれは Ruby だと Bignum になるので、`i64` に収まらない場合は飽和させる。
fn capture_to_i(c: Option<regex::Match<'_>>) -> i64 {
    let Some(s) = c.map(|m| m.as_str()) else {
        return 0;
    };
    if s.is_empty() {
        return 0;
    }
    s.parse::<i64>().unwrap_or(if s.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `Float#to_i`（0方向への切り捨て）。
///
/// Rust の `as` キャストは範囲外を飽和させるので、Ruby の巨大値でも panic しない。
fn float_to_i(value: f64) -> i64 {
    value as i64
}

/// Ruby `Float#to_s`。
///
/// Rust の `{}` は最短往復表現を出すが、整数値のとき小数点以下を落とす（`1.0` → `"1"`）
/// ので、Ruby と同じく `.0` を補う。`1e16` 以上で Ruby が指数表記
/// （`"1.5e+16"`）に切り替わる差は、そこまで大きいダイス数だと
/// `roll_barabara` が先に `TooManyRands` を返すため出力経路に現れない。
fn ruby_float_to_s(value: f64) -> String {
    let s = format!("{value}");
    if s.contains('.') {
        s
    } else {
        format!("{s}.0")
    }
}

/// Ruby `/\.[1-9]+/ =~ str`（小数点の直後に `1`〜`9` が来るか）。
fn has_nonzero_fraction(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(2)
        .any(|w| w[0] == b'.' && (b'1'..=b'9').contains(&w[1]))
}

/// `[1,2,3]` の中身部分を作る。
fn join_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `#ability_roll`（能力値ロール）。
fn ability_roll(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(\d+\.?\d*)?AD([+-](\d+))?(#(\d*))?(>=(\d+))?").expect("valid regex")
    });
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let num_of_die = capture_to_f(m.get(1));
    // Ruby の捕獲は `([+-](\d+))` の**内側**なので、符号は落ちて常に非負になる。
    let bonus = capture_to_i(m.get(3));
    let mut reroll = capture_to_i(m.get(5));
    let mut difficulty = capture_to_i(m.get(7));

    let num_of_die_i = float_to_i(num_of_die);
    let mut dice_command = if has_nonzero_fraction(&ruby_float_to_s(num_of_die)) {
        // Ruby: 文字列補間が先に評価され、そのあとで reroll が増える。
        let c = format!(
            "{}AD{}{}",
            ruby_float_to_s(num_of_die),
            number_with_sign_from_int(bonus),
            number_with_reroll_from_int(reroll)
        );
        reroll += 1;
        c
    } else {
        format!(
            "{}AD{}{}",
            num_of_die_i,
            number_with_sign_from_int(bonus),
            number_with_reroll_from_int(reroll)
        )
    };

    if difficulty == 0 {
        difficulty = 5;
    } else {
        dice_command += &format!(">={difficulty}");
    }

    let values = rng.roll_barabara(num_of_die_i.saturating_add(bonus), 6)?;
    let mut result = values.iter().filter(|&&v| v >= difficulty).count() as i64;
    let failed_roll = num_of_die_i.saturating_sub(result);

    let mut rolled_text = format!("[{}]", join_values(&values));

    let reroll_values = if reroll == 1 {
        vec![rng.roll_once(6)?]
    } else if reroll > 1 {
        rng.roll_barabara(reroll, 6)?
    } else {
        Vec::new()
    };

    let mut reroll_result = reroll_values.iter().filter(|&&v| v >= difficulty).count() as i64;
    if failed_roll < reroll_result {
        reroll_result = failed_roll;
    }
    result += reroll_result;

    if !reroll_values.is_empty() {
        rolled_text += &format!(" Reroll [{}]", join_values(&reroll_values));
    }

    Ok(Some(format!(
        "({dice_command}) > {rolled_text} > {result}成功"
    )))
}

/// Ruby `#initiative_roll`（イニシアチブロール）。
fn initiative_roll(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(\d+\.?\d*)?(INTI|inti)([+-](\d+))?(#(\d+))?").expect("valid regex")
    });
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let grace = capture_to_f(m.get(1));
    // こちらの捕獲は `([+-](\d+))` の**外側**なので符号を含む。
    let bonus = capture_to_i(m.get(3));
    let mut reroll = capture_to_i(m.get(6));

    let dice_command = if has_nonzero_fraction(&ruby_float_to_s(grace)) {
        let c = format!(
            "({}INTI{}{})",
            ruby_float_to_s(grace),
            number_with_sign_from_int(bonus),
            number_with_reroll_from_int(reroll)
        );
        reroll += 1;
        c
    } else {
        format!(
            "({}INTI{}{})",
            float_to_i(grace),
            number_with_sign_from_int(bonus),
            number_with_reroll_from_int(reroll)
        )
    };

    // Ruby: `roll_barabara(grace + bonus, 6)`。grace は Float なので
    // `Array.new` が 0方向へ切り捨てる。
    let values = rng.roll_barabara(float_to_i(grace + bonus as f64), 6)?;

    let mut revalue = if reroll == 0 {
        Vec::new()
    } else {
        rng.roll_barabara(reroll, 6)?
    };
    revalue.sort_unstable();

    let mut result: i64 = 0;
    // Ruby は `"["` から積んで最後に `chop` するので、`values` が空だと `"]"` だけになる。
    // `join` に畳むと `"[]"` になって乖離するため、原典どおりに書く。
    let mut res_text = String::from("[");
    for value in &values {
        if revalue.is_empty() {
            res_text += &value.to_string();
            result += value;
        } else {
            let mut is_min = false;
            for index in 0..revalue.len() {
                let re_value = revalue[index];
                // Ruby: next unless re > value（リロール分が上回ったときだけ置換）
                if re_value <= *value {
                    continue;
                }
                res_text += &format!("{value}<<{re_value}");
                result += re_value;
                revalue.remove(index);
                is_min = true;
                break;
            }
            if !is_min {
                res_text += &value.to_string();
                result += value;
            }
        }

        res_text.push(',');
    }
    res_text.pop();
    res_text.push(']');

    Ok(Some(format!("{dice_command} > {res_text} > {result}")))
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
            .join("test/data/TheUnofficialHollowKnightRPG.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TheUnofficialHollowKnightRPG.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TheUnofficialHollowKnightRPG.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TheUnofficialHollowKnightRPG.toml must parse");
        assert_eq!(
            data.tests.len(),
            22,
            "case count in test/data/TheUnofficialHollowKnightRPG.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TheUnofficialHollowKnightRPG",
                "unexpected game system in TheUnofficialHollowKnightRPG.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("TheUnofficialHollowKnightRPG"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL TheUnofficialHollowKnightRPG:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TheUnofficialHollowKnightRPG cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
