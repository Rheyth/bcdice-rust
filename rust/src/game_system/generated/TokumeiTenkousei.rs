//! P4で手書き移植した `lib/bcdice/game_system/TokumeiTenkousei.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`xD6+y>=n` の判定。ゾロ目で自動振り足し）
//! - `#same_all_dice?` / `#interim_expr` / `#epp`

use std::sync::OnceLock;

use crate::command_parser::{Parsed, Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokumeiTenkousei;

impl GameSystem for TokumeiTenkousei {
    fn id(&self) -> &'static str {
        "TokumeiTenkousei"
    }

    fn name(&self) -> &'static str {
        "特命転攻生"
    }

    fn sort_key(&self) -> &'static str {
        "とくめいてんこうせい"
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

    /// Ruby `TokumeiTenkousei#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

static HELP_MESSAGE: &str = r#"・判定 (xD6+y>=n)
　ゾロ目での自動振り足し
　1の出目に応じてEPPの獲得量を表示
　目標値 "?" には未対応
"#;

static PREFIXES: &[&str] = &[r"\d+D6"];

/// Ruby `#eval_game_system_specific_command`。
fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby は `Command::Parser.new(/\d+D6/, round_type: round_type)`。
    // このシステムは `round_type` を上書きしないので `Base` 既定の `Floor` で固定できる。
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| Parser::new(&[r"\d+D6"], RoundType::Floor));

    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby は比較演算子が無いと `result` が nil のまま `result.text` を呼んで
    // NoMethodError になる（`3D6` 単体で到達する）。本移植は本家のクラッシュを
    // 再現しない方針（Postman.rs と同じ扱い）なので、ダイスを振る前に畳む。
    // ここで振ってから畳むと、フォールスルー先の共通コマンドが注入乱数を
    // 二重に消費してしまう。
    let (Some(cmp_op), Some(target_number)) = (cmd.cmp_op, cmd.target_number.clone()) else {
        return Ok(None);
    };

    let times = leading_i64(&cmd.command);

    let mut dice_list = roll_sorted(rng, times)?;
    let mut dice_list_list = vec![dice_list.clone()];
    while same_all_dice(&dice_list) {
        dice_list = roll_sorted(rng, times)?;
        dice_list_list.push(dice_list.clone());
    }

    let dice_list_flatten: Vec<i64> = dice_list_list.concat();
    let dice_total = dice_list_flatten
        .iter()
        .fold(0i64, |a, b| a.saturating_add(*b));
    let count_one = dice_list_flatten.iter().filter(|v| **v == 1).count() as i64;

    let total = dice_total.saturating_add(crate::randomizer::sat_i64(&cmd.modify_number));

    let mut result = if cmp_op.apply(&crate::Int::from(total), &target_number) {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };

    let mut sequence = vec![format!("({})", cmd.to_s(SuffixPosition::AfterCommand))];
    if let Some(expr) = interim_expr(&cmd, &dice_list_list, dice_total) {
        sequence.push(expr);
    }
    sequence.push(total.to_string());
    sequence.push(result.text.clone());
    if let Some(epp) = epp(count_one) {
        sequence.push(epp);
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `@randomizer.roll_barabara(times, 6).sort`。
fn roll_sorted(rng: &mut Randomizer, times: i64) -> Result<Vec<i64>, EvalError> {
    let mut dice_list = rng.roll_barabara(times, 6)?;
    dice_list.sort_unstable();
    Ok(dice_list)
}

/// Ruby `#same_all_dice?`。出目が全て同じか。
fn same_all_dice(dice_list: &[i64]) -> bool {
    dice_list.len() > 1 && dice_list.iter().all(|v| *v == dice_list[0])
}

/// Ruby `#interim_expr`。
fn interim_expr(cmd: &Parsed, dice_list_list: &[Vec<i64>], dice_total: i64) -> Option<String> {
    if dice_list_list.iter().map(Vec::len).sum::<usize>() == 1 && cmd.modify_number == I::ZERO {
        return None;
    }

    let dice_list = dice_list_list
        .iter()
        .map(|ds| format!("[{}]", join(ds)))
        .collect::<Vec<_>>()
        .concat();
    let modifier = format::modifier(&cmd.modify_number);

    Some(format!("{dice_total}{dice_list}{modifier}"))
}

/// Ruby `#epp`。エキストラパワーポイント獲得。
fn epp(count_one: i64) -> Option<String> {
    (count_one > 0).then(|| format!("{}EPP獲得", count_one * 5))
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `String#to_i`（`"10D6"` → `10`）。
///
/// 桁あふれする入力は Ruby だと Bignum になり、`roll_barabara` の個数上限で
/// エラーになる。i64 に収まらない場合も同じ経路へ落とす。
fn leading_i64(text: &str) -> i64 {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        0
    } else {
        text[..end].parse().unwrap_or(i64::MAX)
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
            .join("test/data/TokumeiTenkousei.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TokumeiTenkousei.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/TokumeiTenkousei.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TokumeiTenkousei.toml must parse");
        assert_eq!(
            data.tests.len(),
            26,
            "case count in test/data/TokumeiTenkousei.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TokumeiTenkousei",
                "unexpected game system in TokumeiTenkousei.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("TokumeiTenkousei"), &tc.input, &mut src) {
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
                    "FAIL TokumeiTenkousei:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TokumeiTenkousei cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
