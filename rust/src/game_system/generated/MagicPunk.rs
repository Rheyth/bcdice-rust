//! P4で手書き移植した `lib/bcdice/game_system/MagicPunk.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MagicPunk#roll_mp`（判定 `nMPm` / チャレンジ判定 `nMPmCx` / ダイス数0 `0MPmCx`）
//!
//! 定型文は `i18n/MagicPunk/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `MagicPunk_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// 1ロケール分の定型文。`MagicPunk` と `MagicPunk_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `MagicPunk.bad_beat`
    pub(crate) bad_beat: &'static str,
    /// i18n `MagicPunk.jackpot`
    pub(crate) jackpot: &'static str,
    /// i18n `MagicPunk.success`（`%<value>d` を含む書式文字列）
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

static JA_SYSTEM: SystemTables = SystemTables {
    bad_beat: "失敗(BB)",
    jackpot: "成功(JP)",
    success: "成功(%<value>d)",
    failure: "失敗",
};

/// Ruby `/^(\d*)MP(\d+)(C?)(\d*)$/`。
fn mp_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d*)MP(\d+)(C?)(\d*)$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当。桁あふれは i64 に飽和させる
/// （Ruby は多倍長になるが、ダイス数ならどのみち振り切れずにエラーになる）。
fn to_i(digits: &str) -> i64 {
    if digits.is_empty() {
        0
    } else {
        digits.parse().unwrap_or(i64::MAX)
    }
}

/// Ruby `MagicPunk#roll_mp`。
pub(crate) fn roll_mp(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = mp_pattern().captures(command) else {
        return Ok(None);
    };

    // 構文解析
    let dices = if m[1].is_empty() { 1 } else { to_i(&m[1]) };
    let spec = to_i(&m[2]);
    let opt1 = &m[3];
    let arg1 = to_i(&m[4]);
    // ダイス数0モードフラグ
    let is_zero = dices == 0;
    // チャレンジ値
    let challenge = if opt1 == "C" { arg1 } else { 0 };
    // ダイスロール
    let dice_list = rng.roll_barabara(if is_zero { 2 } else { dices }, 20)?;

    // 通常は1つ成功なら成功、0ダイス時はすべて成功したとき成功
    // 通常はすべて失敗なら失敗、0ダイス時は1つ失敗したら失敗
    let check_method = |f: &dyn Fn(i64) -> bool| -> bool {
        if is_zero {
            dice_list.iter().all(|&d| f(d))
        } else {
            dice_list.iter().any(|&d| f(d))
        }
    };
    let fail_method = |f: &dyn Fn(i64) -> bool| -> bool {
        if is_zero {
            dice_list.iter().any(|&d| f(d))
        } else {
            dice_list.iter().all(|&d| f(d))
        }
    };

    let mut check = check_method(&|d| d <= spec && challenge <= d); // 通常判定
    let mut is_jp = check_method(&|d| d == spec); // ジャックポット判定
    let is_bb = fail_method(&|d| d == 1); // バッドビート判定

    let result = if is_bb {
        // 自動失敗優先
        is_jp = false;
        check = false;
        sys.bad_beat.to_owned()
    } else if is_jp {
        check = true;
        sys.jackpot.to_owned()
    } else if check {
        let selected = dice_list.iter().copied().filter(|&d| d <= spec);
        let value = if is_zero {
            selected.min()
        } else {
            selected.max()
        };
        // Ruby: `check` が真なら spec 以下の目が必ず1つはあるので nil にはならない
        let value = value.map(|v| v.to_string()).unwrap_or_default();
        sys.success.replace("%<value>d", &value)
    } else {
        sys.failure.to_owned()
    };

    let mut r = EvalResult::with_text(format!(
        "({dices}MP{spec}C{challenge}) > [{}] > {result}",
        join_dice(&dice_list)
    ));
    r.fumble = is_bb;
    r.critical = is_jp;
    r.set_condition(check);
    Ok(Some(r))
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::MagicPunk`（ID: `MagicPunk`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicPunk;

impl GameSystem for MagicPunk {
    fn id(&self) -> &'static str {
        "MagicPunk"
    }

    fn name(&self) -> &'static str {
        "マジックパンクTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ましつくはんくTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (nMPm)
nD20のダイスロールをして、m以下の目があれば成功。
mと同じ目があればジャックポット(自動成功)。
すべての目が1ならバッドビート(自動失敗)。
■ チャレンジ判定 (nMPmCx)
通常の判定に加えてチャレンジ値x以上の目が必要になる。
■ ダイス数0 (0MPmCx)
修正によりダイス数が0になった場合は2d20のダイスロールを行う。
2つの目からより悪い結果になる方を採用する。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"^\d*MP\d+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `MagicPunk#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_mp(&JA_SYSTEM, command, rng)?.map(SpecificCommandOutput::result))
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
            .join("test/data/MagicPunk.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MagicPunk.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MagicPunk.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MagicPunk.toml must parse");
        assert_eq!(
            data.tests.len(),
            14,
            "case count in test/data/MagicPunk.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MagicPunk",
                "unexpected game system in MagicPunk.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("MagicPunk"), &tc.input, &mut src) {
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
                    "FAIL MagicPunk:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MagicPunk cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
