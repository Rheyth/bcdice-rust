//! P4で手書き移植した `lib/bcdice/game_system/ChaosFlare.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#result_2d6`（2D6のファンブル判定と差分値）
//! - `#eval_game_system_specific_command` → `roll_fate_table`（`FT`）と `cf_roll`（`nCF`）

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::int_helpers::int_saturating_sub;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::ChaosFlare`（ID: `ChaosFlare`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChaosFlare;

impl GameSystem for ChaosFlare {
    fn id(&self) -> &'static str {
        "ChaosFlare"
    }

    fn name(&self) -> &'static str {
        "カオスフレア"
    }

    fn sort_key(&self) -> &'static str {
        "かおすふれあ"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*CF", "FT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ChaosFlare#result_2d6`。ゲーム別成功度判定(2D6)。
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Ge {
            return None;
        }

        let mut sequence: Vec<String> = Vec::new();
        let mut result = EvalResult::new();
        let mut total = total;

        if dice_total <= 2 {
            total = int_saturating_sub(&total, &I::from(20));
            sequence.push("ファンブル(-20)".to_string());
            result.fumble = true;
        }

        // Ruby: target != '?'
        if let Target::Number(target) = target {
            if total >= target {
                sequence.push("成功".to_string());
                result.success = true;
            } else {
                sequence.push("失敗".to_string());
                result.failure = true;
            }

            let difference = int_saturating_sub(&total, &target);
            if difference != I::ZERO {
                sequence.push(format!("差分値{difference}"));
            }
        }

        if sequence.is_empty() {
            return Some(CheckOutcome::Nothing);
        }

        result.text = sequence.join(" ＞ ");
        Some(CheckOutcome::Result(Box::new(result)))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let text = if command.starts_with("FT") {
            roll_fate_table(command, rng)?
        } else {
            cf_roll(command, rng)?
        };
        Ok(text.map(SpecificCommandOutput::text))
    }
}

const HELP_MESSAGE: &str = r"判定
CF
  書式: [ダイスの数]CF[修正値][@クリティカル値][#ファンブル値][>=目標値]
    CF以外は全て省略可能
  例:
  - CF 2D6,クリティカル値12,ファンブル値2で判定
  - CF+10@10 修正値+10,クリティカル値10で判定
  - CF+10#3 修正値+10,ファンブル値3で判定
  - CF+10>=10 目標値を指定した場合、差分値も出力する
  - 3CF+10@10#3>=10 3D6での判定
  - CF@9#3+8>=10

2D6
  ファンブル値2で判定する。クリティカルの判定は行われない。
  目標値が設定された場合、差分値を出力する。
  - 2D6+4>=10

各種表
  FT: 因縁表
  FTx: 数値を指定すると因果表の値を出力する
  - FT -> 11から66の間でランダム決定
  - FT23 -> 23の項目を出力
  - FT0
  - FT7
";

/// Ruby `/^FT(\d+)?/`。
fn fate_table_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^FT(\d+)?").expect("valid regex"))
}

/// Ruby `ChaosFlare#roll_fate_table`（因縁表）。
fn roll_fate_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = fate_table_pattern().captures(command) else {
        return Ok(None);
    };

    let (dice1, dice2) = match m.get(1) {
        Some(num) => {
            let num = to_i(num.as_str());
            if num == 0 || num == 7 {
                let index = num as usize;
                return Ok(Some(format!("因果表({num}) ＞ {}", FATE_TABLE[index][0])));
            }

            let dice1 = num / 10;
            let dice2 = num % 10;
            if !(1..=6).contains(&dice1) || !(1..=6).contains(&dice2) {
                return Ok(None);
            }
            (dice1, dice2)
        }
        None => (rng.roll_once(6)?, rng.roll_once(6)?),
    };

    let row = FATE_TABLE[dice1 as usize];
    // Ruby: index2 = (dice2 / 2) - 1。dice2 == 1 では -1 になり、
    // Rubyの負添字で行の末尾要素（dice2 == 6 と同じ項目）を引く。
    let index2 = dice2 / 2 - 1;
    let index2 = if index2 < 0 {
        row.len() as i64 + index2
    } else {
        index2
    };

    Ok(Some(format!(
        "因果表({dice1}{dice2}) ＞ {}",
        row[index2 as usize]
    )))
}

/// Ruby `ChaosFlare#cf_roll`（カオスフレア専用コマンド）。
fn cf_roll(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let parser = Parser::new(&[r"\d*CF"], RoundType::Floor)
        .enable_critical()
        .enable_fumble();

    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let times = if cmd.command == "CF" {
        2
    } else {
        to_i(&cmd.command)
    };
    let critical = cmd
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(12);
    let fumble = cmd
        .fumble
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);

    if times < 0 || !matches!(cmd.cmp_op, None | Some(CmpOp::Ge)) {
        return Ok(None);
    }

    let dice_list = rng.roll_barabara(times, 6)?;
    let dice_total = dice_list.iter().fold(0i64, |a, b| a.saturating_add(*b));
    let dice_list_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let is_critical = dice_total >= critical;
    let is_fumble = dice_total <= fumble;

    let total = if is_critical {
        30
    } else if is_fumble {
        -20
    } else {
        dice_total
    };
    let total = total.saturating_add(crate::randomizer::sat_i64(&cmd.modify_number));

    let mut sequence = vec![
        format!("({})", cmd.to_s(SuffixPosition::AfterModifyNumber)),
        format!("{dice_total}[{dice_list_text}]"),
        total.to_string(),
    ];
    if total < 0 {
        sequence.push("0".to_string());
    }
    if is_critical {
        sequence.push("クリティカル".to_string());
    }
    if is_fumble {
        sequence.push("ファンブル".to_string());
    }
    if let Some(target_number) = cmd.target_number {
        sequence.push(format!(
            "差分値 {}",
            difference(total, crate::randomizer::sat_i64(&target_number))
        ));
    }

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `ChaosFlare#difference`。
fn difference(total: i64, target_number: i64) -> i64 {
    if total < 0 {
        target_number.saturating_neg()
    } else {
        total.saturating_sub(target_number)
    }
}

/// Ruby `String#to_i` 相当（先頭の数字列だけを読み、桁あふれは飽和させる）。
fn to_i(text: &str) -> i64 {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `ChaosFlare::FATE_TABLE`。
static FATE_TABLE: [&[&str]; 8] = [
    &["腐れ縁"],
    &["純愛", "親近感", "庇護"],
    &["信頼", "感服", "共感"],
    &["友情", "尊敬", "慕情"],
    &["好敵手", "期待", "借り"],
    &["興味", "憎悪", "悲しみ"],
    &["恐怖", "執着", "利用"],
    &["任意"],
];

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
            .join("test/data/ChaosFlare.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ChaosFlare.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ChaosFlare.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ChaosFlare.toml must parse");
        assert_eq!(
            data.tests.len(),
            22,
            "case count in test/data/ChaosFlare.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ChaosFlare",
                "unexpected game system in ChaosFlare.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ChaosFlare"), &tc.input, &mut src) {
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
                    "FAIL ChaosFlare:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ChaosFlare cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
