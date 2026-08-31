//! P4で手書き移植した `lib/bcdice/game_system/Karukami.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Karukami#roll_ub`（行為判定・ダメージ算出 `xUB+y@c>=t`）

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Karukami`（ID: `Karukami`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Karukami;

impl GameSystem for Karukami {
    fn id(&self) -> &'static str {
        "Karukami"
    }

    fn name(&self) -> &'static str {
        "カルカミ"
    }

    fn sort_key(&self) -> &'static str {
        "かるかみ"
    }

    fn help_message(&self) -> &'static str {
        r"■ 行為判定、ダメージ算出 (xUB+y@c>=t)
  6面ダイスをx個ダイスロールし、クリティカル値以上の出目が出たら振り足して合計値を算出します。
  x: ダイス数
  y: 修正値（省略可）
  c: クリティカル値（省略可）
  t: 目標値値（省略可）
  例）2UB, 2UB>=7, 3UB+1@5, 3UB+1@5<10
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+UB"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Karukami#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_ub(command, rng)
    }
}

/// Ruby `Karukami#roll_ub`。
fn roll_ub(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Command::Parser.new("UB", round_type: @round_type).has_prefix_number.enable_critical
    let parser = Parser::new(&["UB"], RoundType::Floor)
        .has_prefix_number()
        .enable_critical();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let command_text = parsed.to_s(SuffixPosition::AfterCommand);

    // Ruby: critical = parsed.critical || 6（`@0` は 0 のまま。Rubyでは 0 も真）
    let critical = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(6);
    if critical <= 1 {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "({command_text}) ＞ クリティカル値は2以上としてください"
        ))));
    }

    let mut list_list: Vec<Vec<i64>> = Vec::new();
    let mut criticals: i64 = 0;
    // has_prefix_number なので必ず埋まる。
    let mut stack: I = parsed.prefix_number.expect("has_prefix_number");
    while stack > I::ZERO {
        let dice_list = rng.roll_barabara(sat_i64(&stack), 6)?;
        stack = I::from(dice_list.iter().filter(|&&x| x >= critical).count() as i64);
        criticals += sat_i64(&stack);
        list_list.push(dice_list);
    }

    let mut total: I = I::from(list_list.iter().flatten().sum::<i64>()) + parsed.modify_number;

    // Ruby: list_list.first.all?(1)
    // ダイス数が0以下だと Ruby は `nil.all?` で NoMethodError になるが、
    // ここでは「ファンブルではない」に畳んでいる（TOMLに該当ケースはない）。
    let is_fumble = list_list
        .first()
        .is_some_and(|list| list.iter().all(|&x| x == 1));

    let mut result = if is_fumble {
        total = I::ZERO;
        EvalResult::fumble("ファンブル")
    } else if parsed.cmp_op.is_none() {
        // Ruby `Result.new()` の text は nil で、後段の compact で落ちる
        EvalResult::new()
    } else {
        let cmp_op = parsed.cmp_op.expect("checked above");
        let target_number = parsed.target_number.expect("cmp_op implies target_number");
        if cmp_op.apply(&total, &target_number) {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        }
    };
    result.critical = criticals > 0;

    // Ruby: sequence.compact.join(" ＞ ")
    // `result.text` が空になるのは `Result.new()`（Rubyでは nil）の枝だけなので、
    // 空文字列を落とすことが `compact` と一致する。
    let mut sequence: Vec<String> = Vec::with_capacity(list_list.len() + 4);
    sequence.push(format!("({command_text})"));
    for list in &list_list {
        sequence.push(format!(
            "[{}]",
            list.iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    sequence.push(total.to_string());
    if result.critical {
        sequence.push(format!("{criticals}クリティカル"));
    }
    if !result.text.is_empty() {
        sequence.push(result.text.clone());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
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
            .join("test/data/Karukami.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Karukami.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Karukami.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Karukami.toml must parse");
        assert_eq!(
            data.tests.len(),
            15,
            "case count in test/data/Karukami.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Karukami",
                "unexpected game system in Karukami.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Karukami"), &tc.input, &mut src) {
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
                    "FAIL Karukami:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Karukami cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
