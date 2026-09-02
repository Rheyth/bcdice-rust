//! P4で手書き移植した `lib/bcdice/game_system/NeonUnderRealm.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NeonUnderRealm#eval_game_system_specific_command`（判定 `[M]NU[N][±K][@L][±K']`）
//!   と補助メソッド `parse_command` / `eval_dice_count` / `build_expr_text`

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

const DEFAULT_THRESHOLD: i64 = 5;
const MIN_THRESHOLD: i64 = 1;
const MAX_THRESHOLD: i64 = 10;
const DEFAULT_KIAI_THRESHOLD: i64 = 0;
const MIN_KIAI_THRESHOLD: i64 = 0;
const MAX_KIAI_THRESHOLD: i64 = 5;
const DEFAULT_MODIFIER: i64 = 0;

/// Ruby `NeonUnderRealm#parse_command` の正規表現。
///
/// `/\A(?<m_expr>\d+(?:[+-]\d+)*)NU(?<n>\d+)?(?<k>[+-]\d+)?(?:@(?<l>\d+))?(?<j>[+-]\d+)?\z/`
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\A(?P<m_expr>\d+(?:[+-]\d+)*)NU(?P<n>\d+)?(?P<k>[+-]\d+)?(?:@(?P<l>\d+))?(?P<j>[+-]\d+)?\z",
        )
        .expect("valid regex")
    })
}

/// Ruby `parse_command` の戻り値 `[m_expr, n_str, k_str, l_str]`。
struct ParsedCommand<'a> {
    m_expr: &'a str,
    n_str: Option<&'a str>,
    k_str: Option<&'a str>,
    l_str: Option<&'a str>,
}

/// Ruby `NeonUnderRealm#parse_command`。
fn parse_command(command: &str) -> Option<ParsedCommand<'_>> {
    let m = command_pattern().captures(command)?;
    let m_expr = m.name("m_expr")?.as_str();
    // 修正値が K を優先
    let k = m.name("k").or_else(|| m.name("j")).map(|x| x.as_str());
    Some(ParsedCommand {
        m_expr,
        n_str: m.name("n").map(|x| x.as_str()),
        k_str: k,
        l_str: m.name("l").map(|x| x.as_str()),
    })
}

/// Ruby `NeonUnderRealm#eval_dice_count`。
///
/// `BCDice::Arithmetic.eval(source, @round_type) rescue StandardError -> nil`。
/// 評価時のエラーはすべて `None` に畳む。
fn eval_dice_count(source: &str) -> Option<i64> {
    arithmetic::eval(source, RoundType::Floor)
        .ok()
        .flatten()
        .as_ref()
        .map(crate::randomizer::sat_i64)
}

/// Ruby `String#to_i`（`"+2"` / `"-3"` / `"7"`）。桁あふれは i64 に飽和させる。
fn to_i(s: &str) -> i64 {
    let digits = s.strip_prefix('+').unwrap_or(s);
    digits.parse().unwrap_or(if digits.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `NeonUnderRealm#build_expr_text`。
///
/// 「MB10」表記に寄せる：`xB10<=n±k@l[...]`
fn build_expr_text(dice_count: i64, n: i64, k: i64, l: i64, dice_list: &[i64]) -> String {
    let k_text = modifier(&crate::Int::from(k));
    let l_text = if l > 0 {
        format!("@{l}")
    } else {
        String::new()
    };
    format!(
        "{dice_count}B10<={n}{k_text}{l_text}[{}]",
        dice_text::join_dice(dice_list)
    )
}

/// Ruby `NeonUnderRealm#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(parsed) = parse_command(command) else {
        return Ok(None);
    };

    let Some(dice_count) = eval_dice_count(parsed.m_expr) else {
        return Ok(None);
    };
    if dice_count < 1 {
        return Ok(None);
    }

    let threshold = parsed.n_str.map_or(DEFAULT_THRESHOLD, to_i);
    if !(MIN_THRESHOLD..=MAX_THRESHOLD).contains(&threshold) {
        return Ok(None);
    }

    let modifier = parsed.k_str.map_or(DEFAULT_MODIFIER, to_i);

    let kiai_threshold = parsed.l_str.map_or(DEFAULT_KIAI_THRESHOLD, to_i);
    if !(MIN_KIAI_THRESHOLD..=MAX_KIAI_THRESHOLD).contains(&kiai_threshold) {
        return Ok(None);
    }

    let mut dice_list = rng.roll_barabara(dice_count, 10)?;
    dice_list.sort_unstable();

    let raw_success = dice_list.iter().filter(|&&x| x <= threshold).count() as i64;
    let achieved = raw_success.saturating_add(modifier).max(0);

    let effect = dice_count - raw_success;
    let kiai = if kiai_threshold > 0 {
        dice_list.iter().filter(|&&x| x <= kiai_threshold).count() as i64
    } else {
        0
    };

    let expr_text = build_expr_text(dice_count, threshold, modifier, kiai_threshold, &dice_list);

    let mut result_text_parts = vec![
        format!("達成値：{achieved}（成功数：{raw_success}）"),
        format!("効果値：{effect}"),
        format!("気合：{kiai}"),
    ];

    // 素成功0なら失敗。または達成値0なら失敗。成功表示は不要。失敗表示はこのケースのみ。
    if raw_success == 0 || achieved == 0 {
        result_text_parts.push("失敗".to_owned());
    }

    let text = [
        format!("({})", command.to_uppercase()),
        expr_text,
        result_text_parts.join(" / "),
    ]
    .join(" ＞ ");

    let mut result = EvalResult::with_text(text);

    // 成功条件：達成値>=1 かつ 素成功>=1
    let is_success = achieved >= 1 && raw_success >= 1;
    result.success = is_success;
    result.failure = !is_success;

    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::NeonUnderRealm`（ID: `NeonUnderRealm`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeonUnderRealm;

impl GameSystem for NeonUnderRealm {
    fn id(&self) -> &'static str {
        "NeonUnderRealm"
    }

    fn name(&self) -> &'static str {
        "光都暗域〈ネオン・アンダーレルム〉"
    }

    fn sort_key(&self) -> &'static str {
        "ねおんあんたあれるむ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定（D10の出目が「目標値以下」を成功として数える）
  [M]NU[N][±K][@L][±K']

  M：判定ダイス数（省略不可）。「10+5+3-2」のような加減算を許可
  N：目標値（1～10）。省略時は 5
  K, K'：達成値への補正。両方指定された場合はKを採用し、K'は無視される。（省略可）
  L：気合の閾値（0～5）。省略時は 0（気合は常に0扱い）

  ※Nが1～10の範囲外、またはLが0～5の範囲外の場合はコマンドとして処理しません（出力しません）
  ※達成値 = max(0, 素の成功数 + K)
  ※効果値 = ダイス数 - 素の成功数
  ※素の成功数が0なら判定は失敗（達成値が補正で1以上でも失敗扱い）
  ※成功は表示しません。素の成功数0または達成値0のときのみ「失敗」を表示します。

例）
  4NU
  4NU7+2
  10+5+3-2NU5-1@2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+([+\-]\d+)*NU"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific_command(command, rng)?.map(SpecificCommandOutput::result))
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
            .join("test/data/NeonUnderRealm.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NeonUnderRealm.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/NeonUnderRealm.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NeonUnderRealm.toml must parse");
        assert_eq!(
            data.tests.len(),
            9,
            "case count in test/data/NeonUnderRealm.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NeonUnderRealm",
                "unexpected game system in NeonUnderRealm.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("NeonUnderRealm"), &tc.input, &mut src) {
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
                    "FAIL NeonUnderRealm:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NeonUnderRealm cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
