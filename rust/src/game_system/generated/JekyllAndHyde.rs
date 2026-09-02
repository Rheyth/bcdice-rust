//! P4で手書き移植した `lib/bcdice/game_system/JekyllAndHyde.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 親 `DesperateRun#eval_game_system_specific_command`
//!   （`check_roll` → `ddc_table` → `roll_tables` の順で試す）
//! - `JekyllAndHyde::TABLES`（`GOALT` の目標決定表）
//!
//! # 親クラスの扱い
//!
//! Ruby側は `DesperateRun` を継承するが、Rust側の
//! [`super::DesperateRun`] はまだスタブなので、親の評価ロジック
//! （`#check_roll` / `#ddc_table`）はこのファイルへ取り込んである。
//! 親が移植されたら整理する前提。
//!
//! 表は `table_helpers::roll_table(command, self.class::TABLES)` で `JekyllAndHyde::TABLES` を引くので、
//! 親の `ACT` / `ITEMT` などは対象外（登録済み接頭辞も `RC` / `DDC` / `GOALT` だけ）。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::Table;
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// Ruby `TABLES["GOALT"]` の項目。
static GOALT_ITEMS: &[&str] = &[
    "「主人格の目的達成」",
    "「主人格の目的阻害」",
    "「主人格のハッピーエンド（目的達成しなくてもよい）」",
    "「主人格のバッドエンド（目的達成していてもよい）」",
    "「自分の人格が目的を決定できる」",
    "「主人格の目的達成」「主人格の目的阻害」「主人格のハッピーエンド（目的達成しなくてもよい）」「主人格のバッドエンド（目的達成していてもよい）」「自分の人格が目的を決定できる」のどれかを自由に選べる",
];

/// Ruby `TABLES["GOALT"]`（目標決定表）。
static GOALT_TABLE: Table = Table::from_dice("目標決定表", 1, 6, GOALT_ITEMS);

/// Ruby `JekyllAndHyde::TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[("GOALT", &GOALT_TABLE)];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Command::Parser.new(/RC\d+/, round_type: round_type).restrict_cmp_op_to(nil)`。
///
/// `round_type` は `Base` の既定（`RoundType::Floor`）。`DesperateRun` は上書きしない。
fn parser() -> &'static Parser {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    PARSER.get_or_init(|| Parser::new(&[r"RC\d+"], RoundType::Floor).restrict_cmp_op_to(&[None]))
}

/// Ruby `DesperateRun#check_roll(string)`。
fn check_roll(string: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(cmd) = parser().parse(string) else {
        return Ok(None);
    };

    let dice = rng.roll_barabara(2, 6)?;
    let (d1, d2) = (dice[0], dice[1]);
    let dice_total = d1 + d2;
    let total = d1 + d2 + cmd.modify_number.clone();
    // notation が `RC\d+` なので `[2..]` は必ず十進数字列。
    let target: i64 = cmd.command[2..].parse().unwrap_or(0);

    let modifier_str = if cmd.modify_number != I::ZERO {
        format!("　修正値：{}", cmd.modify_number)
    } else {
        String::new()
    };

    let mut result = if d1 == d2 {
        EvalResult::critical("ゾロ目！【Critical】")
    } else if dice_total == 7 {
        EvalResult::fumble("ダイスの出目が表裏！【Fumble】")
    } else if total >= crate::Int::from(target) {
        EvalResult::success(format!("{total}、難易度以上！【Success】"))
    } else {
        EvalResult::failure(format!("{total}、難易度未満！【Miss】"))
    };

    result.text = format!(
        "判定　難易度：{target}{modifier_str} ＞ 出目：{d1}、{d2} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `DesperateRun#ddc_table(command)`。
fn ddc_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "DDC" {
        return Ok(None);
    }

    let dice = rng.roll_barabara(2, 6)?;
    let (d1, d2) = (dice[0], dice[1]);

    let (smaller, larger) = if d1 <= d2 { (d1, d2) } else { (d2, d1) };
    let difference = larger - smaller;

    Ok(Some(format!(
        "難易度決定 ＞ 出目：{d1}、{d2} ＞ {larger}-{smaller}={difference} ＞ 難易度{}",
        5 + difference
    )))
}

/// Ruby `BCDice::GameSystem::JekyllAndHyde`（ID: `JekyllAndHyde`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JekyllAndHyde;

impl GameSystem for JekyllAndHyde {
    fn id(&self) -> &'static str {
        "JekyllAndHyde"
    }

    fn name(&self) -> &'static str {
        "ジキルとハイドとグリトグラ"
    }

    fn sort_key(&self) -> &'static str {
        "しきるとはいととくりとくら"
    }

    fn help_message(&self) -> &'static str {
        r"・難易度算出コマンド　DDC
・判定コマンド　RCx　or　RCx+y　or　RCx-y（x＝難易度、y=修正値（省略可能））
・目標決定表　GOALT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["RC", "DDC", "GOALT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DesperateRun#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `DesperateRun#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `DesperateRun#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = ddc_table(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "JekyllAndHyde",
            "JekyllAndHyde.toml",
            20,
        );
    }
}
