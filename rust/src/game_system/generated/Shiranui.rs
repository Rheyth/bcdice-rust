//! P4で手書き移植した `lib/bcdice/game_system/Shiranui.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Shiranui#roll_infinite_d66`（∞D66 `x+∞D66` / `x+ID66`）と
//!   `Shiranui::InifiniteD66Step`
//! - `TABLES`（おみくじ `OMKJ`）

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Shiranui`（ID: `Shiranui`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shiranui;

impl GameSystem for Shiranui {
    fn id(&self) -> &'static str {
        "Shiranui"
    }

    fn name(&self) -> &'static str {
        "不知火"
    }

    fn sort_key(&self) -> &'static str {
        "しらぬい"
    }

    fn help_message(&self) -> &'static str {
        r"■∞D66ダイスロール
「 ∞D66 」または「 ID66 」
（ ID は Infinite D の略です）

□行動力や攻撃力の指定
「 x+∞D66 」または「 x+ID66 」
（ x は行動力や攻撃力）

□鬼火の使用について
鬼火を使用する∞D66は、ダイスボットでサポートしていません。

■おみくじを引く
OMKJ
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+\+)?(∞|I)D66", "OMKJ"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Shiranui#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Shiranui#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(m) = infinite_d66_pattern().captures(command) {
            // Ruby: fixed_score = m[1]&.to_i（m[1] は "5+" だが `to_i` で 5 になる）
            let fixed_score = m.get(2).map(|g| to_i_saturating(g.as_str()));
            return Ok(Some(SpecificCommandOutput::result(roll_infinite_d66(
                fixed_score,
                rng,
            )?)));
        }

        // Ruby: roll_tables(command, TABLES)
        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `INFINITE_D66_ROLL_REG = /^((\d+)\+)?(∞|I)D66$/i`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn infinite_d66_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(([0-9]+)\+)?(∞|I)D66$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当（桁あふれは飽和させる）。
fn to_i_saturating(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or(i64::MAX)
}

// OMKJ: おみくじ (1D6) 6 items
static OMKJ_ITEMS: &[&str] = &[
    "大凶［御利益１］――このみくじにあたる人は、凶運から逃れることができぬ者なり。まさに凶運にその身をゆだねてこそ、浮かぶ瀬もあれ。……これより上演中に演者が振る［∞Ｄ66］で初めて⚀⚀が出たら、御利益を使っても振り直しができない。",
    "凶［御利益２］――このみくじにあたる人は、吉兆を逃す定めにある。まさに、天の与うるを取らざれば反ってその咎めを受く。……これより上演中に演者が振る［∞Ｄ66］で初めて⚅⚅が出たら、強制的に１回の振り直しをする。",
    "小吉［御利益３］――このみくじにあたる人は、神使の機嫌を損ねている。神使が何に怒り、何に苛立っているのかは、まさに神のみぞ知る。……神使の機嫌が突然、悪くなる。これより上演中に神使は何かと理由をつけてはシラヌイの前から立ち去ろうとする。",
    "中吉［御利益４］――このみくじにあたる人は、神使の機嫌を良くすることを行った者なり。神使が何に喜び、なぜ機嫌が良いのか、まさに神のみぞ知る。……神使の機嫌がすこぶる良くなる。これより上演中に神使は上機嫌となり、シラヌイに何かにつけて話しかけてくれる。",
    "吉［御利益５］――このみくじにあたる人は、悪運を幸運へと変える道を進む者なり。まさに禍福は糾える縄の如し。……これより上演中に演者が振る［∞Ｄ66］で初めて⚀⚀が出たら、御利益を消費することなく、１回の振り直しをする。",
    "大吉［御利益６］――このみくじにあたる人は、思いもよらぬ幸運に巡り合う者なり。まさに、暗き道より出て、気づけば月の光あり。……これより上演中に演者が振る［∞Ｄ66］で１回だけ、サイコロの出目を⚅⚅に変えてよい。",
];
static OMKJ_TABLE: Table = Table::from_dice("おみくじ", 1, 6, OMKJ_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[("OMKJ", &OMKJ_TABLE)];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `Shiranui::InifiniteD66Step`（∞D66の1回分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InifiniteD66Step {
    /// 昇順にソート済みの2個の出目
    dices: [i64; 2],
}

impl InifiniteD66Step {
    /// Ruby `#score`。
    fn score(&self) -> i64 {
        if self.repdigit() {
            // ゾロ目の場合
            let digit = self.dices[0];
            if digit == 1 {
                // 1 のゾロ目なら 0 となる
                0
            } else {
                // 1 以外のゾロ目なら、数字の 10 倍となる
                digit * 10
            }
        } else {
            // ゾロ目でない場合は、 D66 様式で値を算出する
            self.dices[0] * 10 + self.dices[1]
        }
    }

    /// Ruby `#repdigit?`。
    fn repdigit(&self) -> bool {
        self.dices[0] == self.dices[1]
    }

    /// Ruby `#to_continue_diceroll?`（ダイスロールを継続する必要があるか）。
    fn to_continue_diceroll(self) -> bool {
        self.repdigit() && self.dices[0] != 1
    }

    /// Ruby `#to_s`。
    fn to_text(self) -> String {
        format!("[{},{}]", self.dices[0], self.dices[1])
    }
}

/// Ruby `Shiranui#roll_infinite_d66`。
fn roll_infinite_d66(
    fixed_score: Option<i64>,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let mut steps: Vec<InifiniteD66Step> = Vec::new();

    // Ruby: while steps.empty? || steps.last.to_continue_diceroll?
    //       打ち切りは無いが、`roll_barabara` が乱数の総数上限で
    //       `TooManyRandsError` を上げるので必ず停止する。
    while steps.last().is_none_or(|s| s.to_continue_diceroll()) {
        // 個別の出目をあつかうので、 roll_d66 ではなく roll_barabara を使う
        let mut dices = rng.roll_barabara(2, 6)?;
        dices.sort_unstable();
        let [d0, d1] = dices[..] else {
            return Err(EvalError::Internal(
                "roll_barabara(2, 6) must return 2 dice",
            ));
        };
        steps.push(InifiniteD66Step { dices: [d0, d1] });
    }

    // Ruby: steps.first.score.zero?（「しくじり」か？）
    let is_failure = steps[0].score() == 0;
    let total = if is_failure {
        0
    } else {
        // Ruby: steps.sum(&:score) + fixed_score.to_i（nil.to_i は 0）
        steps
            .iter()
            .map(|s| s.score())
            .fold(0i64, |a, b| a.saturating_add(b))
            .saturating_add(fixed_score.unwrap_or(0))
    };

    let mut result_text = format!("({})", make_command_text(fixed_score));
    result_text.push_str(" ＞ ");
    result_text.push_str(
        &steps
            .iter()
            .map(|s| s.to_text())
            .collect::<Vec<_>>()
            .join(" ＞ "),
    );

    if is_failure {
        result_text.push_str(" ＞ しくじり");
    } else {
        if steps.len() > 1 || fixed_score.is_some() {
            result_text.push_str(" ＞ ");
            result_text.push_str(&score_expression_text(&steps, fixed_score));
        }
        result_text.push_str(" ＞ ");
        result_text.push_str(&total.to_string());
    }

    // Ruby: Result.new(text).tap { |r| r.critical = ...; r.failure = ...; r.fumble = ... }
    //       `success` は触らないので、`Result.critical` / `Result.fumble` は使えない。
    Ok(EvalResult {
        critical: steps.len() > 1,
        failure: is_failure,
        fumble: is_failure,
        ..EvalResult::with_text(result_text)
    })
}

/// Ruby `Shiranui.make_command_text`。
fn make_command_text(fixed_score: Option<i64>) -> String {
    match fixed_score {
        None => "∞D66".to_owned(),
        Some(v) => format!("{v}+∞D66"),
    }
}

/// Ruby `Shiranui.score_expression_text`。
fn score_expression_text(steps: &[InifiniteD66Step], fixed_score: Option<i64>) -> String {
    let text = steps
        .iter()
        .map(|s| s.score().to_string())
        .collect::<Vec<_>>()
        .join("+");

    match fixed_score {
        None => text,
        Some(v) => format!("{v}+({text})"),
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
            .join("test/data/Shiranui.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Shiranui.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Shiranui.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Shiranui.toml must parse");
        assert_eq!(
            data.tests.len(),
            15,
            "case count in test/data/Shiranui.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Shiranui",
                "unexpected game system in Shiranui.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Shiranui"), &tc.input, &mut src) {
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
                    "FAIL Shiranui:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Shiranui cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
