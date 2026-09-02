//! P4で手書き移植した `lib/bcdice/game_system/TrailOfCthulhu.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TrailOfCthulhu#resolute_action`（技能判定 `TCb[>=t]`）
//! - `TrailOfCthulhu#roll_mythos_madness_table`（神話的狂気表 `MMT[a,b]`）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `TrailOfCthulhu::MITHOS_MADDNESS`（神話的狂気表）。
static MITHOS_MADDNESS: &[&str] = &[
    "1:強迫性障害",
    "2:恐怖症",
    "3:誇大妄想狂",
    "4:殺人狂",
    "5:恣意的記憶喪失",
    "6:多重人格障害",
    "7:偏執症",
    "8:妄想症",
];

/// Ruby `BCDice::GameSystem::TrailOfCthulhu`（ID: `TrailOfCthulhu`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailOfCthulhu;

impl GameSystem for TrailOfCthulhu {
    fn id(&self) -> &'static str {
        "TrailOfCthulhu"
    }

    fn name(&self) -> &'static str {
        "トレイル・オブ・クトゥルー"
    }

    fn sort_key(&self) -> &'static str {
        "とれいるおふくとうるう"
    }

    fn help_message(&self) -> &'static str {
        r"■技能判定　TCb[>=t]   b:消費プール・ポイント t:難易度(省略可能)

例)TC2>=5:消費プール・ポイント2,難易度5で技能判定し、その結果を表示する。
   TC>=3: 難易度3で技能判定し、その結果を表示する。
   TC:    難易度指定せずに技能判定する。
   TC3:   消費プール・ポイント3,難易度指定せずに技能判定する。

■神話的狂気表　MMT[a,b]   a,b:除外する神話的狂気(省略時は全神話的狂気を表示する)

例)MMT[1,8]: 神話的狂気のうち、1番と8番を除外してロールし、神話的狂気を決定する。
   MMT2,6:   神話的狂気のうち、2番と6番を除外してロールし、神話的狂気を決定する。
   MMT:      神話的狂気を1番から8番まで列挙する。

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["TC", "MMT"]
    }

    crate::impl_prefixes_pattern!();

    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `TrailOfCthulhu#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: resolute_action(command) || roll_mythos_madness_table(command)
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if let Some(result) = roll_mythos_madness_table(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(None)
    }
}

/// Ruby `/^TC([+\d]*)(>=(\d+))?/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^TC([+\d]*)(>=(\d+))?").expect("valid regex"))
}

/// Ruby `TrailOfCthulhu#resolute_action`（技能判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let bonus_src = m.get(1).map_or("", |g| g.as_str());
    let bonus = if bonus_src.is_empty() {
        I::ZERO
    } else {
        // Ruby: Arithmetic.eval(m[1], @round_type)。式にならなければ nil。
        match arithmetic::eval(bonus_src, RoundType::Ceil)? {
            Some(v) => v,
            None => return Ok(None),
        }
    };

    // Ruby: m[3].to_i（未指定なら nil.to_i == 0）
    let difficulty: I = I::from(m.get(3).map_or(0, |g| ruby_to_i(g.as_str())));

    let dice = rng.roll_once(6)?;
    let total = dice + bonus.clone();

    let mut result = EvalResult::new();
    let sequence = if difficulty > I::ZERO {
        result.set_condition(total >= difficulty);
        vec![
            format!("(TC{bonus}>={difficulty})"),
            format!("{dice}+{bonus}"),
            total.to_string(),
            if result.success { "成功" } else { "失敗" }.to_owned(),
        ]
    } else {
        vec![
            format!("(TC{bonus})"),
            format!("{dice}+{bonus}"),
            total.to_string(),
        ]
    };

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `/^MMT(\[?([1-8],[1-8])\]?)?/`。
fn mythos_madness_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^MMT(\[?([1-8],[1-8])\]?)?").expect("valid regex"))
}

/// Ruby `TrailOfCthulhu#roll_mythos_madness_table`（神話的狂気表）。
fn roll_mythos_madness_table(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = mythos_madness_pattern().captures(command) else {
        return Ok(None);
    };

    let sequence;
    let result_text;

    if m.get(1).is_some() {
        // Ruby: m[2].split(',')。`([1-8],[1-8])` にマッチしているので必ず2要素。
        let exclusion: Vec<&str> = m.get(2).map_or("", |g| g.as_str()).split(',').collect();
        if exclusion.len() != 2 {
            return Ok(None);
        }

        sequence = format!("(MMT[{}])", exclusion.join(","));
        let excluded: Vec<i64> = exclusion.iter().map(|s| ruby_to_i(s)).collect();

        loop {
            let idx = rng.roll_once(8)?;
            if idx != excluded[0] && idx != excluded[1] {
                // Ruby: MITHOS_MADDNESS[idx - 1]
                result_text = usize::try_from(idx - 1)
                    .ok()
                    .and_then(|i| MITHOS_MADDNESS.get(i))
                    .copied()
                    .unwrap_or("")
                    .to_owned();
                break;
            }
        }
    } else {
        sequence = "(MMT)".to_owned();
        // Ruby: 1..8 を順に集めて ", " で連結する
        result_text = MITHOS_MADDNESS.join(", ");
    }

    Ok(Some(EvalResult::with_text(format!(
        "{sequence} ＞ {result_text}"
    ))))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn ruby_to_i(s: &str) -> i64 {
    str_helpers::leading_digits_to_i_max(s)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    /// 余った注入乱数を許すケース（`(1始まりのケース番号, 残り個数)`）。
    ///
    /// Ruby本家の `RandomizerMock`（test/randomizer_mock.rb）は余りを検査しないので、
    /// TOMLには「Rubyもダイスを振る前に nil を返すコマンド」にもダイスが書かれている。
    /// ケース6 `TC+` は `Arithmetic.eval("+")` が nil を返して即 return するため、
    /// Ruby側も1個も振らない。TOMLは期待値の正本なので書き換えない。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[
        (6, 1), // TC+ （Ruby も振らずに nil を返す）
    ];

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/TrailOfCthulhu.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TrailOfCthulhu.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TrailOfCthulhu.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TrailOfCthulhu.toml must parse");
        assert_eq!(
            data.tests.len(),
            10,
            "case count in test/data/TrailOfCthulhu.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TrailOfCthulhu",
                "unexpected game system in TrailOfCthulhu.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("TrailOfCthulhu"), &tc.input, &mut src) {
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

            let allowed_surplus = SURPLUS_RANDS_ALLOWED
                .iter()
                .find(|(case, _)| *case == i + 1)
                .map_or(0, |(_, remaining)| *remaining);
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL TrailOfCthulhu:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TrailOfCthulhu cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
