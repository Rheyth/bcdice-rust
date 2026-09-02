//! P4で手書き移植した `lib/bcdice/game_system/ConvictorDrive.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ConvictorDrive#roll_command`（`xCD@z>=y` の判定）
//! - `TABLES`（技能ランク表 `SLT` / 遅延イベント表 `DCT`）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::Table;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{dice_text, table_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `TABLES["SLT"]` の項目。
static SKILL_RANK_ITEMS: &[&str] = &[
    "ランク外",
    "E-",
    "E",
    "E+",
    "D-",
    "D",
    "D+",
    "C-",
    "C",
    "C+",
    "B-",
    "B",
    "B+",
    "A-",
    "A",
    "A+",
    "S-",
    "S",
    "S+",
];

/// Ruby `TABLES["DCT"]` の項目。
static DELAY_EVENT_ITEMS: &[&str] = &[
    "状況遅延Ⅰ（全員の初期リソースを-1する）",
    "状況遅延Ⅱ（全員の初期リソースを-1する）",
    "状況遅延Ⅲ（全員の初期リソースを-2する）",
    "武装を許すⅠ（ボスの攻撃ダイスを+1dする）",
    "武装を許すⅡ（脅威度4以下のエネミーの攻撃ダイスを2体まで+1dする）",
    "武装を許すⅢ（脅威度3以下のエネミーの攻撃ダイスを1体+2dする）",
    "緊急出撃Ⅰ（ランダムなPCのHPを-1する）",
    "緊急出撃Ⅱ（ランダムなPCのHPを-1する）",
    "緊急出撃Ⅲ（ランダムなPC2人のHPを-1する）",
    "絶望（ダイスを二度振り、二つ適用する）",
];

/// Ruby `TABLES["SLT"]`（`DiceTable::Table.new("技能ランク表", "2D10", ...)`）。
static SKILL_RANK_TABLE: Table = Table::from_dice("技能ランク表", 2, 10, SKILL_RANK_ITEMS);

/// Ruby `TABLES["DCT"]`（`DiceTable::Table.new("遅延イベント表", "1D10", ...)`）。
static DELAY_EVENT_TABLE: Table = Table::from_dice("遅延イベント表", 1, 10, DELAY_EVENT_ITEMS);

/// Ruby `TABLES`。`roll_tables` が引くコマンド名 → 表。
static TABLES: &[(&str, &Table)] = &[("SLT", &SKILL_RANK_TABLE), ("DCT", &DELAY_EVENT_TABLE)];

/// Ruby `BCDice::GameSystem::ConvictorDrive`（ID: `ConvictorDrive`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvictorDrive;

impl GameSystem for ConvictorDrive {
    fn id(&self) -> &'static str {
        "ConvictorDrive"
    }

    fn name(&self) -> &'static str {
        "コンヴィクター・ドライブ"
    }

    fn sort_key(&self) -> &'static str {
        "こんういくたあとらいふ"
    }

    fn help_message(&self) -> &'static str {
        r"xCD@z>=y: x個の10面ダイスで目標値y（省略時5）、クリティカルラインz（省略時10）の判定を行う。
SLT: 技能レベル表を振る
DCT: 遅延イベント表を振る
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["[-+*0-9()]*CD", "SLT", "DCT"]
    }

    crate::impl_prefixes_pattern!();

    fn sides_implicit_d(&self) -> i64 {
        10
    }

    /// Ruby `ConvictorDrive#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_command(command) || table_helpers::roll_table(command, TABLES, TABLES)
        if let Some(result) = roll_command(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if let Some(text) = table_helpers::roll_table(command, TABLES, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        Ok(None)
    }
}

/// Ruby `ConvictorDrive#roll_command`。
fn roll_command(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('CD', round_type: round_type)
    //       `round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["CD"], RoundType::Floor)
            .has_prefix_number()
            .enable_critical()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge), None])
    });
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // `has_prefix_number` なのでパースに成功した時点で prefix_number は必ずある。
    let dice_list = rng.roll_barabara(
        cmd.prefix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        10,
    )?;
    let target_num = cmd.target_number.clone().unwrap_or(crate::Int::from(5));
    // Ruby: cmd.critical&.clamp(target_num, 10) || 10
    let critical = cmd
        .critical
        .as_ref()
        .map_or(10, |c| clamp_critical(sat_i64(c), sat_i64(&target_num)));

    let succeed_num = dice_list
        .iter()
        .filter(|&&x| x >= crate::randomizer::sat_i64(&target_num))
        .count();
    let critical_num = dice_list.iter().filter(|&&x| x >= critical).count();

    let mut parts = vec![
        cmd.to_s(SuffixPosition::AfterCommand),
        dice_text::join_dice(&dice_list),
    ];
    if critical_num > 0 {
        parts.push(format!("クリティカル数{critical_num}"));
    }
    // Ruby: 成功数はクリティカル分を二重に数える（原典どおり）
    parts.push(format!("成功数{}", succeed_num + critical_num));

    Ok(Some(EvalResult {
        text: parts.join(" ＞ "),
        success: succeed_num > 0,
        critical: critical_num > 0,
        ..EvalResult::default()
    }))
}

/// Ruby `critical.clamp(target, 10)`。
///
/// Ruby は `target > 10` だと `ArgumentError`（min > max）で落ちる。TOMLに該当ケースが
/// 無く、`i64::clamp` も同条件でパニックするので、下限側だけを優先して panic を避ける。
/// `target <= 10` の範囲では Ruby の `clamp` と完全に一致する。
fn clamp_critical(critical: i64, target: i64) -> i64 {
    if critical < target {
        target
    } else {
        critical.min(10)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ConvictorDrive",
            "ConvictorDrive.toml",
            10,
        );
    }
}
