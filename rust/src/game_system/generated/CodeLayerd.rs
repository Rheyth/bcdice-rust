//! P4で手書き移植した `lib/bcdice/game_system/CodeLayerd.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command` → `check_roll`（`nCL+x@m[c]+x2>=t`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::CodeLayerd`（ID: `CodeLayerd`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeLayerd;

impl GameSystem for CodeLayerd {
    fn id(&self) -> &'static str {
        "CodeLayerd"
    }

    fn name(&self) -> &'static str {
        "コード：レイヤード"
    }

    fn sort_key(&self) -> &'static str {
        "こおとれいやあと"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[+-]?\d*CL"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sides_implicit_d = 10`。
    fn sides_implicit_d(&self) -> i64 {
        10
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let Some(m) = check_pattern().captures(command) else {
            return Ok(None);
        };

        let base = m.get(1).map_or(1, |x| to_i(x.as_str()));
        let modifier1 = m.get(2).map_or(0, |x| to_i(x.as_str()));
        let target = m.get(4).map_or(6, |x| to_i(x.as_str()));
        let critical_target = m.get(6).map_or(1, |x| to_i(x.as_str()));
        let modifier2 = m.get(7).map_or(0, |x| to_i(x.as_str()));
        let diff = m.get(9).map(|x| to_i(x.as_str()));

        let result = check_roll(
            command,
            base,
            target,
            critical_target,
            diff,
            modifier1.saturating_add(modifier2),
            rng,
        )?;
        Ok(Some(SpecificCommandOutput::result(result)))
    }
}

const HELP_MESSAGE: &str = r"・行為判定（nCL@m[c]+x または nCL+x@m[c]） クリティカル・ファンブル判定あり
  (ダイス数)CL+(修正値)@(判定値)[(クリティカル値)]+(修正値2)

  @m,[c],+xは省略可能。(@6[1]として処理)
  n個のD10でmを判定値、cをクリティカル値とした行為判定を行う。
  nが0以下のときはクリティカルしない1CL判定を行う。(1CL[0]と同一)
  例）
  7CL>=5 ：サイコロ7個で判定値6のロールを行い、目標値5に対して判定
  4CL@7  ：サイコロ4個で判定値7のロールを行い達成値を出す
  4CL+2@7 または 4CL@7+2  ：サイコロ4個で判定値7のロールを行い達成値を出し、修正値2を足す。
  4CL[2] ：サイコロ4個でクリティカル値2のロールを行う。
  0CL : 1CL[0]と同じ判定

  デフォルトダイス：10面
";

/// Ruby `/([+-]?\d+)?CL([+-]\d+)?(@(\d))?(\[(\d+)\])?([+-]\d+)?(>=(\d+))?/i`。
///
/// 原典は**アンカーを持たない**。先頭一致は `prefixes_pattern` 側で保証済みで、
/// 末尾を縛らないことで `7CL@10`（`@\d` が1桁しか食わず `0` が余る）のような
/// 入力も原典どおり受理される。
fn check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)([+-]?\d+)?CL([+-]\d+)?(@(\d))?(\[(\d+)\])?([+-]\d+)?(>=(\d+))?")
            .expect("valid regex")
    })
}

/// Ruby `String#to_i` 相当（桁あふれは飽和させる）。
fn to_i(text: &str) -> i64 {
    let text = text.strip_prefix('+').unwrap_or(text);
    text.parse::<i64>().unwrap_or_else(|_| {
        if text.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `CodeLayerd#check_roll`。
fn check_roll(
    command: &str,
    base: i64,
    target: i64,
    critical_target: i64,
    diff: Option<i64>,
    modify_number: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    // クリティカルしない1D
    let (base, critical_target) = if base <= 0 {
        (1, 0)
    } else {
        (base, critical_target)
    };

    let mut result = EvalResult::new();

    let target = if target > 10 { 10 } else { target };
    let mut dice_list = rng.roll_barabara(base, 10)?;
    dice_list.sort_unstable();
    let success_count = dice_list.iter().filter(|&&x| x <= target).count() as i64;
    let critical_count = dice_list.iter().filter(|&&x| x <= critical_target).count() as i64;
    result.critical = critical_count > 0;
    let success_total = success_count
        .saturating_add(critical_count)
        .saturating_add(modify_number);

    let mod_text = modifier(&crate::Int::from(modify_number));

    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // (10d10+5)
    let mut text = format!("{command} ＞ ({base}d10{mod_text}) ＞ [{dice_text}]{mod_text} ＞ ");
    if target != 6 {
        text.push_str(&format!("判定値[{target}] "));
    }
    if critical_target != 1 {
        text.push_str(&format!("クリティカル値[{critical_target}] "));
    }
    text.push_str(&format!("達成値[{success_count}]"));

    if success_count <= 0 {
        result.fumble = true;
        result.failure = true;
        result.text = format!("{text} ＞ ファンブル！");
        return Ok(result);
    }

    if result.critical {
        text.push_str(&format!("+クリティカル[{critical_count}]"));
    }
    text.push_str(&mod_text);
    if result.critical || modify_number != 0 {
        text.push_str(&format!("=[{success_total}]"));
    }

    match diff {
        None => result.text = format!("{text} ＞ {success_total}"),
        Some(diff) if success_total >= diff => {
            result.text = format!("{text} ＞ 成功");
            result.success = true;
        }
        Some(_) => {
            result.text = format!("{text} ＞ 失敗");
            result.failure = true;
        }
    }

    Ok(result)
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
            .join("test/data/CodeLayerd.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/CodeLayerd.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/CodeLayerd.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("CodeLayerd.toml must parse");
        assert_eq!(
            data.tests.len(),
            23,
            "case count in test/data/CodeLayerd.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "CodeLayerd",
                "unexpected game system in CodeLayerd.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("CodeLayerd"), &tc.input, &mut src) {
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
                    "FAIL CodeLayerd:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} CodeLayerd cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
