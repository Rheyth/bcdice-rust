//! P4で手書き移植した `lib/bcdice/game_system/Bloodorium.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#dicecheck`（ダイスチェック `xDC+y`）と `#total_expr`
//!
//! `translate('Bloodorium.triumph')` は `i18n/Bloodorium/{ja_jp,ko_kr}.yml` の値を
//! 静的データとして持ち、ロケール差だけを引数で受ける
//! （`ko_kr` 版は [`super::Bloodorium_Korean`]）。

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// i18n `Bloodorium.triumph`（`i18n/Bloodorium/ja_jp.yml`）。
pub const TRIUMPH_JA_JP: &str = "《トライアンフ》(*%{triumph})";

/// Ruby `BCDice::GameSystem::Bloodorium`（ID: `Bloodorium`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloodorium;

impl GameSystem for Bloodorium {
    fn id(&self) -> &'static str {
        "Bloodorium"
    }

    fn name(&self) -> &'static str {
        "ブラドリウム"
    }

    fn sort_key(&self) -> &'static str {
        "ふらとりうむ"
    }

    fn help_message(&self) -> &'static str {
        r"・ダイスチェック xDC+y
　【ダイスチェック】を行う。《トライアンフ》を結果に自動反映する。
　x: ダイス数
　y: 結果への修正値 （省略可）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(dicecheck(command, TRIUMPH_JA_JP, rng)?.map(SpecificCommandOutput::result))
    }
}

/// Ruby `#dicecheck`（ダイスチェック）。
///
/// `triumph_message` は `translate('Bloodorium.triumph', triumph:)` に対応する
/// ロケール別の文言（`%{triumph}` を出目の重複数で置換する）。
pub fn dicecheck(
    command: &str,
    triumph_message: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["DC"], RoundType::Floor)
        .has_prefix_number()
        .restrict_cmp_op_to(&[None]);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let mut dice_list = rng.roll_barabara(
        parsed
            .prefix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        6,
    )?;
    dice_list.sort_unstable();

    // Ruby は `[].max` が `nil` になり `nil * triumph` で NoMethodError を投げる
    // （`0DC` は接頭辞 `\d+DC` にマッチするのでここへ到達しうる）。
    // Rustでは panic させず「出力なし」に畳む。
    let Some(&dice_value) = dice_list.iter().max() else {
        return Ok(None);
    };

    // Ruby: `group_by(&:itself).transform_values(&:length).values.max`
    // （出目ごとの個数の最大値）。dice_list はソート済みなので連なりを数えれば同じ。
    let triumph = max_run_length(&dice_list);

    let total = dice_value * triumph + parsed.modify_number.clone();

    let mut sequence: Vec<String> = Vec::new();
    sequence.push(format!("({})", parsed.to_s(SuffixPosition::AfterCommand)));
    sequence.push(format!(
        "[{}]{}",
        dice_list
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
        format::modifier(&parsed.modify_number)
    ));
    if triumph > 1 {
        sequence.push(triumph_message.replace("%{triumph}", &triumph.to_string()));
    }
    if total != crate::Int::from(dice_value) {
        sequence.push(total_expr(
            dice_value,
            triumph,
            crate::randomizer::sat_i64(&parsed.modify_number),
        ));
    }
    sequence.push(total.to_string());

    Ok(Some(EvalResult {
        text: sequence.join(" ＞ "),
        // Ruby: `r.critical = triumph > 1`（`Result.critical` と違い success は立たない）
        critical: triumph > 1,
        ..EvalResult::default()
    }))
}

/// ソート済みの列で、同じ値が連続する最大長を返す。
fn max_run_length(sorted: &[i64]) -> i64 {
    let mut max: i64 = 0;
    let mut run: i64 = 0;
    let mut prev: Option<i64> = None;
    for &v in sorted {
        run = if prev == Some(v) { run + 1 } else { 1 };
        prev = Some(v);
        max = max.max(run);
    }
    max
}

/// Ruby `#total_expr`。
fn total_expr(dice_value: i64, triumph: i64, modify_number: i64) -> String {
    let formated_triumph = if triumph > 1 {
        format!("*{triumph}")
    } else {
        String::new()
    };

    format!(
        "{dice_value}{formated_triumph}{}",
        format::modifier(&crate::Int::from(modify_number))
    )
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
            .join("test/data/Bloodorium.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Bloodorium.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Bloodorium.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Bloodorium.toml must parse");
        assert_eq!(
            data.tests.len(),
            10,
            "case count in test/data/Bloodorium.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Bloodorium",
                "unexpected game system in Bloodorium.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Bloodorium"), &tc.input, &mut src) {
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
                    "FAIL Bloodorium:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Bloodorium cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// TOMLに無い分岐。
    #[test]
    fn branches_not_covered_by_toml() {
        fn eval(input: &str, rands: Vec<(i64, i64)>) -> Option<String> {
            let mut src = SeededRandomizer::new(rands);
            let out = eval_command(&GameSystemId::new("Bloodorium"), input, &mut src)
                .expect("must not error");
            assert!(src.is_empty(), "unconsumed rands for {input}");
            out.map(|r| r.text)
        }

        // `0DC` は接頭辞 `\d+DC` にマッチするので固有コマンドまで到達する。
        // Ruby は `[].max` が `nil` になり `nil * triumph` で NoMethodError を投げるが、
        // ここでは panic させず「出力なし」に畳んでいる。
        assert_eq!(eval("0DC", vec![]), None);
        assert_eq!(eval("0DC+1", vec![]), None);
    }
}
