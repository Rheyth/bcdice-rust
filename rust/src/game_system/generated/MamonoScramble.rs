//! P4で手書き移植した `lib/bcdice/game_system/MamonoScramble.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MamonoScramble#roll_ability`（判定 `xMS<=t`）
//! - `TABLES`（アクシデント表 `ACC`）

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::MamonoScramble`（ID: `MamonoScramble`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MamonoScramble;

impl GameSystem for MamonoScramble {
    fn id(&self) -> &'static str {
        "MamonoScramble"
    }

    fn name(&self) -> &'static str {
        "マモノスクランブル"
    }

    fn sort_key(&self) -> &'static str {
        "まものすくらんふる"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 xMS<=t
　[判定]を行う。成否と[マリョク]の上昇量を表示する。
　x: ダイス数
　t: 能力値（目標値）

・アクシデント表 ACC
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+MS", "ACC"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `@sides_implicit_d = 12`。
    fn sides_implicit_d(&self) -> i64 {
        12
    }

    /// Ruby `MamonoScramble#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_ability(command) || roll_tables(command, TABLES)
        if let Some(result) = roll_ability(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(None)
    }
}

/// Ruby `MamonoScramble#roll_ability`（判定 `xMS<=t`）。
fn roll_ability(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: Command::Parser.new("MS", round_type: @round_type)
    //         .has_prefix_number.disable_modifier.restrict_cmp_op_to(:<=)
    let parser = Parser::new(&["MS"], RoundType::Ceil)
        .has_prefix_number()
        .disable_modifier()
        .restrict_cmp_op_to(&[Some(CmpOp::Le)]);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // has_prefix_number / restrict_cmp_op_to(:<=) によりどちらも必ず埋まる。
    let prefix_number = parsed.prefix_number.as_ref().expect("has_prefix_number");
    let target_number = parsed
        .target_number
        .as_ref()
        .expect("cmp_op is restricted to <=");

    let mut dice_list = rng.roll_barabara(crate::randomizer::sat_i64(prefix_number), 12)?;
    dice_list.sort_unstable();

    let count_success = dice_list
        .iter()
        .filter(|&&v| v <= crate::randomizer::sat_i64(target_number))
        .count();
    let count_one = dice_list.iter().filter(|&&v| v == 1).count();
    let is_critical = count_one > 0;
    let has_twelve = dice_list.contains(&12);

    let maryoku = if has_twelve && !is_critical {
        0
    } else {
        count_success + count_one
    };

    let joined = dice_list
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let judgement = if count_success > 0 {
        format!("成功, [マリョク]が{maryoku}上がる")
    } else {
        "失敗".to_owned()
    };

    let mut result = EvalResult::new();
    result.text = format!(
        "({}) ＞ [{joined}] ＞ {judgement}",
        parsed.to_s(SuffixPosition::AfterCommand)
    );
    result.set_condition(count_success > 0);
    result.critical = result.success && is_critical;

    Ok(Some(result))
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "ACC" {
        return Ok(None);
    }
    Ok(Some(ACCIDENT_TABLE.roll(rng)?.to_string()))
}

/// Ruby `TABLES["ACC"]` の項目。
static ACCIDENT_ITEMS: &[&str] = &[
    "思わぬ対立：[判定]で10〜12の出目を1個でも出した場合、【耐久値】を2点減らす。",
    "都市の迷宮化：[判定]に【社会】を使用できない。",
    "不穏な天気：特別な効果は発生しない。",
    "突然の雷雨：エリアの[特性]に[雨]や[水たまり]などを足してもいい。",
    "関係ない危機：[判定]に失敗したPCの【耐久値】を2点減らす。",
    "からりと晴天：エリアの[特性]に[強い日光]や[日だまり]などを足してもいい。",
    "謎のお祭り：[判定]で1〜3の出目を1個でも出した場合、【耐久値】を2点回復する。",
    "すごい人ごみ：エリアの[特性]に[野次馬]や[観光客]などを足してもいい。",
    "マリョク乱気流：[判定]に【異質】を使用できない。",
    "魔術テロ事件：GMが1Dをロールする。出目が1〜3なら【身体】、出目が4〜6なら【異質】、出目が7〜9なら【社会】が[判定]で使えない。10〜12は何も起きない。",
    "マリョク低気圧：[判定]に【身体】を使用できない。",
    "平穏な時間：特別な効果は発生しない。",
];

/// Ruby `TABLES["ACC"]`（`1D12`）。
static ACCIDENT_TABLE: Table = Table::from_dice("アクシデント表", 1, 12, ACCIDENT_ITEMS);

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
            .join("test/data/MamonoScramble.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MamonoScramble.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MamonoScramble.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MamonoScramble.toml must parse");
        assert_eq!(
            data.tests.len(),
            14,
            "case count in test/data/MamonoScramble.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MamonoScramble",
                "unexpected game system in MamonoScramble.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("MamonoScramble"), &tc.input, &mut src) {
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
                    "FAIL MamonoScramble:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MamonoScramble cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
