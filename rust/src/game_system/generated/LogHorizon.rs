//! P4で手書き移植した `lib/bcdice/game_system/LogHorizon.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 判定 `xLH±y>=z`（`getCheckRollDiceCommandResult` / `result_text`）
//! - 消耗表ロール `CTx±y`（`roll_consumption`）と消耗表 `tCTx±y$z`
//!   （`roll_consumption_table` / `ConsumptionTable`）
//! - 財宝表ロール `TRSx±y`（`roll_treasure`）、財宝表 `tTRSx±y$`
//!   （`roll_treasure_table` / `TreasureTable` / `HeroineTreasureTable`）、
//!   拡張ルール財宝表 `tTRSEx±y$`（`roll_treasure_table_b2` / `ExpansionTreasureTable`）
//! - ロデ研の新発明 `IATt`、アキバの街のトラブル `TIAS`、廃棄児 `ABDC`、楽器種別表 `MIIx`、
//!   イースタル探索表 `ESTLx±y$z`、`TABLES`（`PTAG` / `KOYU` / `MGR1`〜`3` / `HLOC` / `PCNM`）
//!
//! # Rubyのネストクラスの扱い
//!
//! 原典は表を `ConsumptionTable` / `TreasureTable` / `HeroineTreasureTable` /
//! `ExpansionTreasureTable` というネストクラスで表し、`fix_dice_value` で
//! 状態（固定ダイス値）を持たせてから `roll` している。Rust側は
//!
//! - i18n から来る **データ** → [`ConsumptionTableData`] / [`TreasureTableData`] などの `static`
//! - `fix_dice_value` の状態 → `roll` 系関数の `fixed_dice` 引数
//! - サブクラスによる `pick_item` の差分 → [`TreasureKind`] による分岐
//!
//! に落とし、状態を持つオブジェクトを作らない形にした。
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("LogHorizon.…", locale:)` で `i18n/LogHorizon/ja_jp.yml` から表を作る。
//! Rust側は同じ値を `static` として直接持つ。データ部分（`JA_` 接頭辞の `static` 群）は
//! 同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`LogHorizon_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `LogHorizon_Korean < LogHorizon` なのに対応する）。
//! `ko_kr` に無いキー（`TRSE` / `ESTL` / `TRS.below_lower_limit` など）は Ruby の
//! `I18n.fallbacks.defaults = [:ja_jp]` で ja_jp が使われるので、Korean側は
//! このモジュールの `JA_` な `static` をそのまま指す。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::{self};
use crate::command_parser::{Parsed, Parser, SuffixPosition};
use crate::dice_table::{D66Table, RollableTable, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// 表データの型
// ---------------------------------------------------------------------------

/// 消耗表。i18n `LogHorizon.CT.*`（Ruby `LogHorizon::ConsumptionTable` が持つデータ）。
pub(crate) struct ConsumptionTableData {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// CR帯ごとの副表。各副表は「出目0〜7」に対応する8項目。
    pub(crate) items: &'static [&'static [&'static str]],
}

/// 財宝表。i18n `LogHorizon.TRS.*` / `LogHorizon.TRSE.*`
/// （Ruby `LogHorizon::TreasureTable` が持つデータ）。
pub(crate) struct TreasureTableData {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// `items[0]` に対応するキー。Ruby側は `Hash{Integer => String}` だが、
    /// 本家の全表がキー連番なので配列＋先頭キーで持つ。
    pub(crate) first_key: i64,
    /// 表の項目。
    pub(crate) items: &'static [&'static str],
}

impl TreasureTableData {
    /// Ruby `@items[index]`。未登録のキーは `nil` ＝ 文字列補間で空文字列になる。
    fn get(&self, index: i64) -> &'static str {
        index
            .checked_sub(self.first_key)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| self.items.get(i))
            .copied()
            .unwrap_or("")
    }
}

/// 名前つきの1D6表。i18n `LogHorizon.IAT.{A,B,L,T}`。
pub(crate) struct NamedItems {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// 出目1〜6に対応する6項目。
    pub(crate) items: &'static [&'static str],
}

/// Ruby `roll_random_table` が引く表。i18n `LogHorizon.TIAS` / `LogHorizon.ABDC`。
pub(crate) struct RandomTableData {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// 桁ごとの1D6表。
    pub(crate) tables: &'static [&'static [&'static str]],
}

/// 楽器種別表。i18n `LogHorizon.MII`。
pub(crate) struct MusicalInstrumentData {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// 楽器の種類（1〜6）。
    pub(crate) type_list: &'static [&'static str],
    /// 種類ごとの1D6表。
    pub(crate) items: &'static [&'static [&'static str]],
}

/// イースタル探索表。i18n `LogHorizon.ESTL`。
pub(crate) struct EastalData {
    /// 表の名前。
    pub(crate) name: &'static str,
    /// `items[0]` に対応するキー。
    pub(crate) first_key: i64,
    /// 表の項目。YAMLのブロックスカラー（`|`）なので末尾に改行が1つ残る。
    pub(crate) items: &'static [&'static str],
}

impl EastalData {
    /// Ruby `table[total]`。
    fn get(&self, index: i64) -> &'static str {
        index
            .checked_sub(self.first_key)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| self.items.get(i))
            .copied()
            .unwrap_or("")
    }
}

/// 財宝表の種別。Ruby側の3クラス（`pick_item` の差分だけが違う）に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreasureKind {
    /// Ruby `TreasureTable`
    Normal,
    /// Ruby `HeroineTreasureTable`
    Heroine,
    /// Ruby `ExpansionTreasureTable`
    Expansion,
}

/// 1ロケール分の表と定型文。`LogHorizon` と `LogHorizon_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `LogHorizon.LH.critical`
    pub(crate) lh_critical: &'static str,
    /// i18n `LogHorizon.LH.fumble`
    pub(crate) lh_fumble: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
    /// Ruby `construct_consumption_table`（`(P|E|G|C|ES|CS)` → 消耗表）
    pub(crate) ct: &'static [(&'static str, &'static ConsumptionTableData)],
    /// Ruby `construct_treasure_table`（`[CMIHG]TRS` → 財宝表）
    pub(crate) trs: &'static [(&'static str, &'static TreasureTableData)],
    /// Ruby `roll_treasure_table_b2`（`[CMIO]TRSE` → 拡張ルール財宝表）
    pub(crate) trse: &'static [(&'static str, &'static TreasureTableData)],
    /// i18n `LogHorizon.TRS.below_lower_limit`（`%{value}` を含む）
    pub(crate) below_lower_limit: &'static str,
    /// i18n `LogHorizon.TRS.exceed_upper_limit`（`%{value}` を含む）
    pub(crate) exceed_upper_limit: &'static str,
    /// i18n `LogHorizon.TRS.need_cr`（`%{command}` を含む）
    pub(crate) need_cr: &'static str,
    /// i18n `LogHorizon.IAT.name`
    pub(crate) iat_name: &'static str,
    /// i18n `LogHorizon.IAT.A`
    pub(crate) iat_a: &'static NamedItems,
    /// i18n `LogHorizon.IAT.B`
    pub(crate) iat_b: &'static NamedItems,
    /// i18n `LogHorizon.IAT.L`
    pub(crate) iat_l: &'static NamedItems,
    /// i18n `LogHorizon.IAT.T`
    pub(crate) iat_t: &'static NamedItems,
    /// i18n `LogHorizon.TIAS`
    pub(crate) tias: &'static RandomTableData,
    /// i18n `LogHorizon.ABDC`
    pub(crate) abdc: &'static RandomTableData,
    /// i18n `LogHorizon.MII`
    pub(crate) mii: &'static MusicalInstrumentData,
    /// i18n `LogHorizon.ESTL`
    pub(crate) estl: &'static EastalData,
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static D66Table)],
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `LogHorizon#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = check_roll(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(text) = roll_consumption(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_consumption_table(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_treasure(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_treasure_table(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_treasure_table_b2(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_invention_attribute(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    // Ruby `getTroubleInAkibaStreetDiceCommandResult`
    if command == "TIAS" {
        return Ok(Some(SpecificCommandOutput::text(roll_random_table(
            tables.tias,
            rng,
        )?)));
    }
    // Ruby `getAbandonedChildDiceCommandResult`
    if command == "ABDC" {
        return Ok(Some(SpecificCommandOutput::text(roll_random_table(
            tables.abdc,
            rng,
        )?)));
    }
    if let Some(text) = roll_musical_instrument_type(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_eastal_exploration_table(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_tables(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }

    Ok(None)
}

/// Ruby `LogHorizon#getCheckRollDiceCommandResult`（判定 `xLH±y>=z`）。
fn check_roll(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new(/\d+LH/, round_type: round_type).restrict_cmp_op_to(nil, :>=)
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+LH"], RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: parsed.command.to_i（"3LH" → 3）
    let dice_count = ruby_to_i(&parsed.command);

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let dice_total: i64 = dice_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));
    let total = dice_total.wrapping_add(crate::randomizer::sat_i64(&parsed.modify_number));

    let mut result = result_text(tables, dice_count, &dice_list, total, &parsed);

    let mut sequence = vec![
        format!("({})", parsed.to_s(SuffixPosition::AfterCommand)),
        format!(
            "{dice_total}[{}]{}",
            join_dice(&dice_list),
            modifier(&parsed.modify_number)
        ),
        total.to_string(),
    ];
    // Ruby: [...].compact —— `Result.new` の text だけが nil で、他の枝の文言は必ず非空。
    if !result.text.is_empty() {
        sequence.push(std::mem::take(&mut result.text));
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `LogHorizon#result_text`。
fn result_text(
    tables: &SystemTables,
    dice_count: i64,
    dice_list: &[i64],
    total: i64,
    parsed: &Parsed,
) -> EvalResult {
    if dice_list.iter().filter(|d| **d == 6).count() >= 2 {
        EvalResult::critical(tables.lh_critical)
    } else if count_i64(dice_list, 1) >= dice_count {
        EvalResult::fumble(tables.lh_fumble)
    } else if parsed.cmp_op.is_none() {
        EvalResult::new()
    } else {
        // 比較演算子があるときは目標値も必ずある（`?` は `enable_question_target` を
        // 呼んでいないのでパース時に弾かれる）。
        let target = parsed.target_number.clone().unwrap_or(crate::Int::from(0));
        if total >= crate::randomizer::sat_i64(&target) {
            EvalResult::success(tables.success)
        } else {
            EvalResult::failure(tables.failure)
        }
    }
}

/// Ruby `dice_list.count(value)`（`i64` へ飽和させて比較する）。
fn count_i64(dice_list: &[i64], value: i64) -> i64 {
    i64::try_from(dice_list.iter().filter(|d| **d == value).count()).unwrap_or(i64::MAX)
}

/// Ruby `LogHorizon#roll_consumption`（消耗表ロール `CTx±y`）。
fn roll_consumption(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^CT\d*([+\-\d]+)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    let modify_number = arith_eval(captures.get(1).map(|m| m.as_str()))?;
    let formatted_modifier = modifier(&crate::Int::from(modify_number));
    let dice = rng.roll_once(6)?;

    // Ruby: interim_expr は修正値の表記が空でないときだけ作られ、compact で落ちる
    let mut sequence = vec![format!("(1D6{formatted_modifier})")];
    if !formatted_modifier.is_empty() {
        sequence.push(format!("{dice}{formatted_modifier}"));
    }
    sequence.push(dice.wrapping_add(modify_number).to_string());

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `LogHorizon#roll_consumption_table`（消耗表 `tCTx±y$z`）。
fn roll_consumption_table(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Ruby: /(P|E|G|C|ES|CS)CT(\d+)?([+\-\d]+)?(?:\$(\d+))?/（先頭・末尾のアンカー無し）
    let re = RE.get_or_init(|| {
        Regex::new(r"(P|E|G|C|ES|CS)CT(\d+)?([+\-\d]+)?(?:\$(\d+))?").expect("valid regex")
    });
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby `construct_consumption_table`。正規表現が6種類のいずれかを保証するので
    // 見つからない枝には到達しない（Ruby側は nil[:name] で NoMethodError になる）。
    let Some((_, table)) = tables.ct.iter().find(|(key, _)| *key == &captures[1]) else {
        return Ok(None);
    };

    let cr = captures.get(2).map_or(0, |m| ruby_to_i(m.as_str()));
    let modify_number = arith_eval(captures.get(3).map(|m| m.as_str()))?;
    // Ruby: table.fix_dice_value(m[4].to_i) if m[4]
    let fixed_dice = captures.get(4).map(|m| ruby_to_i(m.as_str()));

    Ok(Some(consumption_table_roll(
        table,
        cr,
        modify_number,
        fixed_dice,
        rng,
    )?))
}

/// Ruby `LogHorizon::ConsumptionTable#roll`。
fn consumption_table_roll(
    table: &ConsumptionTableData,
    cr: i64,
    modify_number: i64,
    fixed_dice: Option<i64>,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    // Ruby: ((cr - 1) / 5).clamp(0, @tables.size - 1)（Integer#/ は床除算）
    let last_index = i64::try_from(table.items.len().saturating_sub(1)).unwrap_or(i64::MAX);
    let table_index = (cr.wrapping_sub(1)).div_euclid(5).clamp(0, last_index);
    let items: &[&str] = usize::try_from(table_index)
        .ok()
        .and_then(|i| table.items.get(i))
        .copied()
        .unwrap_or(&[]);

    // Ruby: @dice_value ||= randomizer.roll_once(6)
    // （`fix_dice_value(0)` の 0 は Ruby では真なので、固定値0もそのまま残る）
    let dice_value = match fixed_dice {
        Some(value) => value,
        None => rng.roll_once(6)?,
    };
    let total = dice_value.wrapping_add(modify_number);
    let chosen = items
        .get(total.clamp(0, 7) as usize)
        .copied()
        .unwrap_or_default();

    Ok(format!("{}({total}[{dice_value}]) ＞ {chosen}", table.name))
}

/// Ruby `LogHorizon#roll_treasure`（財宝表ロール `TRSx±y`）。
fn roll_treasure(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^TRS(\d+)?([+\-\d]+)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    let character_rank = captures.get(1).map_or(0, |m| ruby_to_i(m.as_str()));
    let modify_number = arith_eval(captures.get(2).map(|m| m.as_str()))?;

    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_total: i64 = dice_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));
    let bonus = character_rank.wrapping_mul(5).wrapping_add(modify_number);
    let total = dice_total.wrapping_add(bonus);

    Ok(Some(format!(
        "(2D6+{character_rank}*5{}) ＞ {dice_total}[{}]{} ＞ {total}",
        modifier(&crate::Int::from(modify_number)),
        join_dice(&dice_list),
        modifier(&crate::Int::from(bonus))
    )))
}

/// Ruby `LogHorizon#roll_treasure_table`（財宝表 `tTRSx±y$`）。
fn roll_treasure_table(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"^([CMIHG]TRS)(\d+)?([+\-\d]+)?(\$)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    let type_name = &captures[1];
    let Some((_, table)) = tables.trs.iter().find(|(key, _)| *key == type_name) else {
        return Ok(None);
    };
    // Ruby `construct_treasure_table`: HTRS だけ HeroineTreasureTable
    let kind = if type_name == "HTRS" {
        TreasureKind::Heroine
    } else {
        TreasureKind::Normal
    };

    let character_rank = captures.get(2).map_or(0, |m| ruby_to_i(m.as_str()));
    let modify_number = arith_eval(captures.get(3).map(|m| m.as_str()))?;
    if character_rank == 0 && modify_number == 0 {
        return Ok(Some(tables.need_cr.replace("%{command}", command)));
    }

    // Ruby: table.fix_dice_value(7) if m[4]（プライズ1回分）
    let fixed_dice = captures.get(4).map(|_| 7);

    Ok(Some(treasure_table_roll(
        tables,
        table,
        kind,
        character_rank,
        modify_number,
        fixed_dice,
        rng,
    )?))
}

/// Ruby `LogHorizon#roll_treasure_table_b2`（拡張ルール財宝表 `tTRSEx±y$`）。
fn roll_treasure_table_b2(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"^([CMIO]TRSE)(\d+)?([+\-\d]+)?(\$)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    let type_name = &captures[1];
    let Some((_, table)) = tables.trse.iter().find(|(key, _)| *key == type_name) else {
        return Ok(None);
    };

    let character_rank = captures.get(2).map_or(0, |m| ruby_to_i(m.as_str()));
    let modify_number = arith_eval(captures.get(3).map(|m| m.as_str()))?;
    if character_rank == 0 && modify_number == 0 {
        return Ok(Some(tables.need_cr.replace("%{command}", command)));
    }

    let fixed_dice = captures.get(4).map(|_| 7);

    Ok(Some(treasure_table_roll(
        tables,
        table,
        TreasureKind::Expansion,
        character_rank,
        modify_number,
        fixed_dice,
        rng,
    )?))
}

/// Ruby `LogHorizon::TreasureTable#roll`。
fn treasure_table_roll(
    tables: &SystemTables,
    table: &TreasureTableData,
    kind: TreasureKind,
    cr: i64,
    modify_number: i64,
    fixed_dice: Option<i64>,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    // Ruby: `fix_dice_value(dice)` は `@dice_list = [dice]`
    let mut dice_list: Option<Vec<i64>> = fixed_dice.map(|dice| vec![dice]);

    let index = if cr == 0 && modify_number != 0 {
        // modifierの値のみ設定されている場合には、その値の項目をダイスロールせずに参照する
        modify_number
    } else {
        // Ruby: @dice_list ||= randomizer.roll_barabara(2, 6)
        if dice_list.is_none() {
            dice_list = Some(rng.roll_barabara(2, 6)?);
        }
        let dice_total: i64 = dice_list
            .iter()
            .flatten()
            .fold(0i64, |a, b| a.wrapping_add(*b));
        dice_total
            .wrapping_add(cr.wrapping_mul(5))
            .wrapping_add(modify_number)
    };

    let chosen = pick_item(tables, table, kind, index);
    // Ruby: dice_str = "[…]" if @dice_list（＝振らなくても固定値があれば付く）
    let dice_str = dice_list
        .as_ref()
        .map(|list| format!("[{}]", join_dice(list)))
        .unwrap_or_default();

    Ok(format!("{}({index}{dice_str}) ＞ {chosen}", table.name))
}

/// Ruby `TreasureTable#pick_item` と、そのサブクラス2つの上書き。
fn pick_item(
    tables: &SystemTables,
    table: &TreasureTableData,
    kind: TreasureKind,
    index: i64,
) -> String {
    match kind {
        TreasureKind::Normal => {
            if index <= 6 {
                below_lower_limit(tables, 6)
            } else if index <= 62 {
                table.get(index).to_owned()
            } else if index <= 72 {
                format!("{}&80G", table.get(index - 10))
            } else if index <= 82 {
                format!("{}&160G", table.get(index - 20))
            } else if index <= 87 {
                format!("{}&260G", table.get(index - 30))
            } else {
                exceed_upper_limit(tables, 88)
            }
        }
        TreasureKind::Heroine => {
            if index <= 6 {
                below_lower_limit(tables, 6)
            } else if index <= 53 {
                table.get(index).to_owned()
            } else {
                exceed_upper_limit(tables, 54)
            }
        }
        TreasureKind::Expansion => {
            if index <= 6 {
                below_lower_limit(tables, 6)
            } else if index <= 162 {
                table.get(index).to_owned()
            } else if index <= 172 {
                format!("{}&200G", table.get(index - 10))
            } else if index <= 182 {
                format!("{}&400G", table.get(index - 20))
            } else if index <= 187 {
                format!("{}&600G", table.get(index - 30))
            } else {
                exceed_upper_limit(tables, 188)
            }
        }
    }
}

/// i18n `LogHorizon.TRS.below_lower_limit`（`%{value}以下の出目は未定義です`）。
fn below_lower_limit(tables: &SystemTables, value: i64) -> String {
    tables
        .below_lower_limit
        .replace("%{value}", &value.to_string())
}

/// i18n `LogHorizon.TRS.exceed_upper_limit`（`%{value}以上の出目は未定義です`）。
fn exceed_upper_limit(tables: &SystemTables, value: i64) -> String {
    tables
        .exceed_upper_limit
        .replace("%{value}", &value.to_string())
}

/// Ruby `LogHorizon#getInventionAttributeTextDiceCommandResult`（ロデ研の新発明 `IATt`）。
fn roll_invention_attribute(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"IAT([ABMDLT]*)").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: Regexp.last_match(1) && Regexp.last_match(1) != '' ? Regexp.last_match(1) : 'MDLT'
    let matched = captures.get(1).map_or("", |m| m.as_str());
    let table_indicate_string = if matched.is_empty() { "MDLT" } else { matched };
    let is_single = table_indicate_string.chars().count() == 1;

    let mut numbers: Vec<String> = Vec::new();
    let mut result: Vec<String> = Vec::new();

    for c in table_indicate_string.chars() {
        let dice_result = rng.roll_once(6)?;
        numbers.push(dice_result.to_string());

        let table = match c {
            'A' | 'M' => tables.iat_a,
            'B' | 'D' => tables.iat_b,
            'L' => tables.iat_l,
            // 文字クラスが `[ABMDLT]` なので残るのは 'T' だけ
            _ => tables.iat_t,
        };
        let chosen = index_1based(table.items, dice_result);

        result.push(if is_single {
            format!("{}：{chosen}", table.name)
        } else {
            chosen.to_owned()
        });
    }

    Ok(Some(format!(
        "{}([{}]) ＞ {}",
        tables.iat_name,
        numbers.join(","),
        result.join(" ")
    )))
}

/// Ruby `LogHorizon#roll_random_table`（`TIAS` / `ABDC`）。
fn roll_random_table(table: &RandomTableData, rng: &mut Randomizer) -> Result<String, EvalError> {
    let times = i64::try_from(table.tables.len()).unwrap_or(i64::MAX);
    let dice_list = rng.roll_barabara(times, 6)?;
    let result: Vec<&str> = table
        .tables
        .iter()
        .zip(&dice_list)
        .map(|(items, n)| index_1based(items, *n))
        .collect();

    Ok(format!(
        "{}([{}]) ＞ {}",
        table.name,
        join_dice(&dice_list),
        result.join(" ")
    ))
}

/// Ruby `LogHorizon#getMusicalInstrumentTypeDiceCommandResult`（楽器種別表 `MIIx`）。
fn roll_musical_instrument_type(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"MII(\d?)").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: is_roll = !(Regexp.last_match(1) && Regexp.last_match(1) != '')
    let matched = captures.get(1).map_or("", |m| m.as_str());
    let is_roll = matched.is_empty();
    let instrument_type = if is_roll {
        rng.roll_once(6)?
    } else {
        ruby_to_i(matched)
    };

    if !(1..=6).contains(&instrument_type) {
        return Ok(None);
    }

    let type_name = index_1based(tables.mii.type_list, instrument_type);
    let dice = rng.roll_once(6)?;
    let result = usize::try_from(instrument_type - 1)
        .ok()
        .and_then(|i| tables.mii.items.get(i))
        .map_or("", |items| index_1based(items, dice));

    let type_str = if is_roll {
        format!("({instrument_type})")
    } else {
        String::new()
    };

    Ok(Some(format!(
        "{}{type_str} ＞ {type_name}({dice}) ＞ {result}",
        tables.mii.name
    )))
}

/// Ruby `LogHorizon#roll_eastal_exploration_table`（イースタル探索表 `ESTLx±y$z`）。
fn roll_eastal_exploration_table(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"ESTL(\d+)?([+\-\d]+)?(?:\$(\d+))?").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };
    // Ruby: return nil if m[1].nil? && m[2].nil? && m[3].nil?
    if captures.get(1).is_none() && captures.get(2).is_none() && captures.get(3).is_none() {
        return Ok(None);
    }

    let character_rank = captures.get(1).map_or(0, |m| ruby_to_i(m.as_str()));
    let modify_number = arith_eval(captures.get(2).map(|m| m.as_str()))?;
    let fixed_dice_value = captures.get(3).map(|m| ruby_to_i(m.as_str()));

    let dice_list = if let Some(value) = fixed_dice_value {
        vec![value]
    } else if character_rank == 0 {
        Vec::new()
    } else {
        rng.roll_barabara(2, 6)?
    };

    let dice_str = if dice_list.is_empty() {
        String::new()
    } else {
        format!("[{}]", join_dice(&dice_list))
    };
    let dice_total: i64 = dice_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));
    let total = dice_total
        .wrapping_add(character_rank.wrapping_mul(5))
        .wrapping_add(modify_number)
        .clamp(7, 162);

    let chosen = ruby_chomp(tables.estl.get(total));

    Ok(Some(format!(
        "{}({total}{dice_str})\n{chosen}",
        tables.estl.name
    )))
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = tables.tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

// ---------------------------------------------------------------------------
// 補助
// ---------------------------------------------------------------------------

/// Ruby `String#to_i`（先頭の十進数だけを読み、無ければ 0）。
///
/// ここに来る文字列は `\d+` の一部か `\d+LH` なので符号や空白は現れない。
fn ruby_to_i(s: &str) -> i64 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return 0;
    }
    // 桁あふれは Ruby だと Bignum になる。i64 に収まらない場合は飽和させる。
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（`nil` と不正な式は 0）。
fn arith_eval(expr: Option<&str>) -> Result<i64, EvalError> {
    match expr {
        None => Ok(0),
        Some(source) => Ok(arithmetic::eval(source, RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)),
    }
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `items[dice - 1]`（範囲外は `nil` ＝ 空文字列）。
fn index_1based(items: &[&'static str], dice: i64) -> &'static str {
    usize::try_from(dice - 1)
        .ok()
        .and_then(|i| items.get(i))
        .copied()
        .unwrap_or("")
}

/// Ruby `String#chomp`（末尾の `"\r\n"` / `"\n"` / `"\r"` を1つだけ落とす）。
fn ruby_chomp(s: &str) -> &str {
    if let Some(rest) = s.strip_suffix("\r\n") {
        rest
    } else if let Some(rest) = s.strip_suffix('\n') {
        rest
    } else if let Some(rest) = s.strip_suffix('\r') {
        rest
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::LogHorizon`（ID: `LogHorizon`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogHorizon;

impl GameSystem for LogHorizon {
    fn id(&self) -> &'static str {
        "LogHorizon"
    }

    fn name(&self) -> &'static str {
        "ログ・ホライズンTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ろくほらいすんTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (xLH±y>=z)
　xD6の判定。クリティカル、ファンブルの自動判定を行います。
　x：xに振るダイス数を入力。
　±y：yに修正値を入力。±の計算に対応。省略可能。
　>=z：zに目標値を入力。±の計算に対応。省略可能。
　例） 3LH　2LH>=8　3LH+1>=10

■ 消耗表 (tCTx±y$z)
　PCT 体力／ECT 気力／GCT 物品／CCT 金銭
　x:CRを指定。
　±y:修正値。＋と－の計算に対応。省略可能。
　$z：＄を付けるとダイス目を z 固定。表の特定の値参照用に。省略可能。
　例） PCT1　ECT2+1　GCT3-1　CCT3$5

■ 消耗表ロール (CTx±y)
　消耗表ロールを行い、出目を決定する。
　x：CRを指定。指定できますが、無視されます。省略可能
　±y：修正値。＋と－の計算に対応。省略可能。

■ 財宝表 (tTRSx±y$)
　LHZB1記載の財宝表
　CTRS 金銭／MTRS 魔法素材／ITRS 換金アイテム／※HTRS ヒロイン／GTRS ゴブリン財宝表
　x：CRを指定。省略時はダイス値 0 固定で修正値の表参照。《ゴールドフィンガー》使用時など。
　±y：修正値。＋と－の計算に対応。省略可能。
　$：＄を付けると財宝表のダイス目を7固定（1回分のプライズ用）。省略可能。
　例） CTRS1　MTRS2+1　ITRS3-1　ITRS+27　CTRS3$

■ 財宝表（拡張ルールブック） (tTRSEx±y$)
　LHZB2記載の財宝表
　CTRSE 金銭／MTRSE 魔法素材／ITRSE 換金アイテム／OTRSE そのほか
　記法は財宝表と同様

■ 財宝表ロール (TRSx±y)
　財宝表ロールを行い、出目を決定する。
　x：CRを指定。省略時はCR 0として扱う
　±y：修正値。＋と－の計算に対応。省略可能。

■ イースタル探索表 (ESTLx±y$z)
　x：CRを指定。省略時はダイス値 0 固定で修正値の表参照。
　±y：修正値。＋と－の計算に対応。省略可能。
　$z：＄を付けるとダイス目を z 固定。特定CRの表参照用に。省略可能。
　例） ESTL1　ESTL+15　ESTL2+1$5　ESTL2-1$5

■ プレフィックスドマジックアイテム効果表 (MGRx)
　xはMGを指定。(LHZB1用)

■ 楽器種別表† (MIIx)
　xは楽器の種類(1～6を指定)、省略可能
　1 打楽器１／2 鍵盤楽器／3 弦楽器１／4 弦楽器２／5 管楽器１／6 管楽器２

■ 特殊消耗表☆ (tSCTx±y$z)
　消耗表と同様、ただしCRは省略可能。
　ESCT ロデ研は爆発だ！／CSCT アルヴの呪いじゃ！

■ ロデ研の新発明ランダム決定表※ (IATt)
　IATA 特徴A(メリット)／IATB 特徴B(デメリット)／IATL 見た目／IATT 種類
　tを省略すると全て表示。tにA/B/L/Tを任意の順で連結可能
　例）IAT　IATALT  IATABBLT  IATABL

■ 表
　・パーソナリティタグ表 (PTAG)
　・交友表 (KOYU)
　・攻撃命中箇所ランダム決定表※ (HLOC)
　・PC名ランダム決定表※ (PCNM)
　・アキバの街で遭遇するトラブルランダム決定表※ (TIAS)
　・廃棄児ランダム決定表※ (ABDC)

†印は☆印は「イントゥ・ザ・セルデシア さらなるビルドの羽ばたき（１）」より、
☆印はセルデシア・ガゼット「できるかな66」Vol.1より、
※印は「実録・七面体工房スタッフ座談会(夏の陣)」より。利用法などはそちら参照。
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+LH", r"\w+CT", "CT", r"\w+TRS", "TRS", "IAT", "TIAS", "ABDC", "MII", "ESTL",
            "PTAG", "KOYU", "MGR1", "MGR2", "MGR3", "HLOC", "PCNM",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

// ---------------------------------------------------------------------------
// 表データ（i18n/LogHorizon/ja_jp.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `LogHorizon.CT.PCT`。
static JA_CT_PCT: ConsumptionTableData = ConsumptionTableData {
    name: "体力消耗表",
    items: &[
        &[
            "消耗なし",
            "[疲労:5]を受ける",
            "[疲労:8]を受ける",
            "[疲労:10]を受ける",
            "[疲労:13]を受ける",
            "[疲労:15]を受ける",
            "[疲労:18]を受ける",
            "[疲労:20]を受ける",
        ],
        &[
            "消耗なし",
            "[疲労:10]を受ける",
            "[疲労:15]を受ける",
            "[疲労:20]を受ける",
            "[疲労:25]を受ける",
            "[疲労:30]を受ける",
            "[疲労:35]を受ける",
            "[疲労:40]を受ける",
        ],
        &[
            "消耗なし",
            "[疲労:25]を受ける",
            "[疲労:35]を受ける",
            "[疲労:45]を受ける",
            "[疲労:55]を受ける",
            "[疲労:65]を受ける",
            "[疲労:65]を受け、【因果力】を1点失う",
            "[疲労:65]を受け、【因果力】を2点失う",
        ],
        &[
            "消耗なし",
            "[疲労:40]を受ける",
            "[疲労:60]を受ける",
            "[疲労:80]を受ける",
            "[疲労:80]を受け、【因果力】を1点失う",
            "[疲労:80]を受け、【因果力】を2点失う",
            "[疲労:90]を受け、【因果力】を2点失う",
            "[疲労:90]を受け、【因果力】を3点失う",
        ],
        &[
            "消耗なし",
            "[疲労:60]を受ける",
            "[疲労:85]を受ける",
            "[疲労:110]を受ける",
            "[疲労:100]を受け、【因果力】を1点失う",
            "[疲労:120]を受け、【因果力】を1点失う",
            "[疲労:100]を受け、【因果力】を2点失う",
            "[疲労:100]を受け、【因果力】を3点失う",
        ],
        &[
            "消耗なし",
            "[疲労:80]を受ける",
            "[疲労:120]を受ける",
            "[疲労:120]を受け、【因果力】を1点失う",
            "[疲労:120]を受け、【因果力】を1点失う",
            "[疲労:120]を受け、【因果力】を2点失う",
            "[疲労:120]を受け、【因果力】を2点失う",
            "[疲労:120]を受け、【因果力】を3点失う",
        ],
    ],
};

/// i18n `LogHorizon.CT.ECT`。
static JA_CT_ECT: ConsumptionTableData = ConsumptionTableData {
    name: "気力消耗表",
    items: &[
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
        ],
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を3点失う",
        ],
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を3点失う",
            "【因果力】を4点失う",
        ],
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を3点失う",
            "【因果力】を1点失い、特技喪失する",
            "【因果力】を2点失い、特技喪失する",
        ],
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を2点失う",
            "【因果力】を3点失う",
            "【因果力】を1点失い、特技喪失する",
            "【因果力】を2点失い、特技喪失する",
            "【因果力】を3点失い、特技喪失する",
        ],
        &[
            "消耗なし",
            "【因果力】を1点失う",
            "【因果力】を2点失う",
            "【因果力】を3点失う",
            "【因果力】を1点失い、特技喪失する",
            "【因果力】を4点失う",
            "【因果力】を2点失い、特技喪失する",
            "【因果力】を3点失い、特技喪失する",
        ],
    ],
};

/// i18n `LogHorizon.CT.GCT`。
static JA_CT_GCT: ConsumptionTableData = ConsumptionTableData {
    name: "物品消耗表",
    items: &[
        &[
            "消耗なし",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
        ],
        &[
            "消耗なし",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを4個失う",
        ],
        &[
            "消耗なし",
            "[消耗品]アイテムを1個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを3個失い、[圧迫:2]を受ける",
        ],
        &[
            "消耗なし",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを4個失う",
            "[消耗品]アイテムを4個失い、[圧迫:1]を受ける",
        ],
        &[
            "消耗なし",
            "[消耗品]アイテムを2個失う",
            "[消耗品]アイテムを2個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを4個失う",
            "[消耗品]アイテムを4個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを4個失い、[圧迫:2]を受ける",
        ],
        &[
            "消耗なし",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失う",
            "[消耗品]アイテムを3個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを4個失う",
            "[消耗品]アイテムを4個失い、[圧迫:1]を受ける",
            "[消耗品]アイテムを4個失い、[圧迫:2]を受ける",
            "[消耗品]アイテムを4個失い、[圧迫:3]を受ける",
        ],
    ],
};

/// i18n `LogHorizon.CT.CCT`。
static JA_CT_CCT: ConsumptionTableData = ConsumptionTableData {
    name: "金銭消耗表",
    items: &[
        &[
            "消耗なし",
            "所持金を10G失う",
            "所持金を15G失う",
            "所持金を20G失う",
            "所持金を25G失う",
            "所持金を30G失う",
            "所持金を35G失う",
            "所持金を40G失う",
        ],
        &[
            "消耗なし",
            "所持金を25G失う",
            "所持金を35G失う",
            "所持金を50G失う",
            "所持金を60G失う",
            "所持金を75G失う",
            "所持金を90G失う",
            "所持金を100G失う",
        ],
        &[
            "消耗なし",
            "所持金を50G失う",
            "所持金を80G失う",
            "所持金を110G失う",
            "所持金を140G失う",
            "所持金を160G失う",
            "所持金を190G失う",
            "所持金を220G失う",
        ],
        &[
            "消耗なし",
            "所持金を100G失う",
            "所持金を150G失う",
            "所持金を200G失う",
            "所持金を250G失う",
            "所持金を300G失う",
            "所持金を350G失う",
            "所持金を400G失う",
        ],
        &[
            "消耗なし",
            "所持金を160G失う",
            "所持金を240G失う",
            "所持金を320G失う",
            "所持金を400G失う",
            "所持金を480G失う",
            "所持金を560G失う",
            "所持金を640G失う",
        ],
        &[
            "消耗なし",
            "所持金を210G失う",
            "所持金を320G失う",
            "所持金を430G失う",
            "所持金を540G失う",
            "所持金を650G失う",
            "所持金を760G失う",
            "所持金を870G失う",
        ],
    ],
};

/// i18n `LogHorizon.CT.ESCT`。
static JA_CT_ESCT: ConsumptionTableData = ConsumptionTableData {
    name: "特殊消耗表：ロデ研は爆発だ！",
    items: &[
        &[
            "……しかし特に何も起こらない！：効果なし。",
            "キミの頭髪が爆発した！　見事なアフロヘアーだ：シナリオ終了まで［頭部］タグのついた装備不可。",
            "芸術は爆発である：所持しているアイテムがランダムに１つ、芸術品になり［換金］アイテム化する",
            "反応起爆装甲：防具スロットに装備しているアイテムがチョバムアーマーになる。次にあなたが【ＨＰ】ダメージを受けた時、そのダメージを無効化し、そのアイテムを失う。",
            "山羊スライムが爆発的に増殖する：［所持品］スロットを全て山羊スライム（［取引不可］、価格５０）で埋める。",
            "キミのリアルが爆発する：コネクションを１つ、シナリオ終了時まで失う。",
            "工場が爆発する：［消耗品］タグを持つアイテムを購入することができなくなる。",
            "ボスエネミーが爆発する：シナリオはクライマックスを迎えることなく終了する。お疲れ様でした。",
        ],
    ],
};

/// i18n `LogHorizon.CT.CSCT`。
static JA_CT_CSCT: ConsumptionTableData = ConsumptionTableData {
    name: "特殊消耗表：アルヴの呪いじゃ！",
    items: &[&[
        "「祝ってやる！」祝われる：あなたの【因果力】＋１。",
        "空腹の呪い：すぐさま食料アイテムひとつを食べる。空腹のせいでとてもおいしい！",
        "無職の呪い：サブ職が強制的に〈ニート〉に変更させられる。",
        "種族変更の呪い：ランダムでダイスを振って別の種族に変わる。種族特技が使用不能になる。",
        "バリアフリーの呪い：［軽減］［障壁］状態になることができなくなる。",
        "集中力を乱す囁きの呪い：フォームが崩れて［構え］タグを持つ特技が使用不能になる。",
        "盲目の呪い：あなたの所持するコネクションのうちランダムに一つの関係が「熱愛」にかわる。",
        "やる気が萎える呪い：あーもーまじやる気しねえ、何もやる気しねえ。【因果力】使用不可。",
    ]],
};

/// Ruby `construct_consumption_table` の分岐（`(P|E|G|C|ES|CS)CT` の接頭辞 → 表）。
static JA_CT: &[(&str, &ConsumptionTableData)] = &[
    ("P", &JA_CT_PCT),
    ("E", &JA_CT_ECT),
    ("G", &JA_CT_GCT),
    ("C", &JA_CT_CCT),
    ("ES", &JA_CT_ESCT),
    ("CS", &JA_CT_CSCT),
];

/// i18n `LogHorizon.TRS.CTRS`。
static JA_TRS_CTRS: TreasureTableData = TreasureTableData {
    name: "金銭財宝表",
    first_key: 7,
    items: &[
        "39G", "40G", "42G", "43G", "45G", "46G", "48G", "50G", "52G", "54G", "57G", "59G", "61G",
        "64G", "67G", "70G", "72G", "75G", "79G", "82G", "85G", "89G", "92G", "96G", "100G",
        "104G", "108G", "112G", "117G", "121G", "126G", "130G", "135G", "140G", "145G", "150G",
        "155G", "161G", "166G", "172G", "178G", "183G", "189G", "195G", "201G", "208G", "214G",
        "221G", "227G", "234G", "241G", "248G", "255G", "262G", "269G", "277G",
    ],
};

/// i18n `LogHorizon.TRS.MTRS`。
static JA_TRS_MTRS: TreasureTableData = TreasureTableData {
    name: "魔法素材財宝表",
    first_key: 7,
    items: &[
        "魔触媒2[魔触媒2](20G)x2",
        "魔触媒2[魔触媒2](20G)x2",
        "魔触媒1[魔触媒1](15G)x3",
        "魔触媒1[魔触媒1](15G)x3",
        "魔触媒1[魔触媒1](15G)x3",
        "腐銀の小片[コア素材](30G)&鉱石のサンプル[換金](15G)",
        "新緑の若芽[コア素材](30G)&小さな花の種[換金](20G)",
        "命の葉[コア素材](40G)&強靭なツタ[換金](10G)",
        "鋭い牙[コア素材](40G)&使い込まれたナイフ[換金](10G)",
        "魔触媒3[魔触媒3](25G)x2",
        "古びた髑髏[コア素材](40G)&黒い詩集[換金](15G)",
        "黒睡蓮の花弁[コア素材](40G)&水キセル[換金](20G)",
        "純白の羽根[コア素材](40G)&小さなゴーグル[換金](20G)",
        "真紅の爪[コア素材](50G)&小さな鏡[換金](15G)",
        "赤熱の小爪[コア素材](50G)&細工物の暖炉[換金](20G)",
        "自戒の茨[コア素材](50G)&焦げた竜革[換金](20G)",
        "流星のかけら[コア素材](50G)&質素な指輪[換金](10G)x2",
        "聖なる繊維[コア素材](50G)&傷ついたメダル[換金](25G)",
        "折れたシャフト[コア素材](60G)&ガラクタの山[換金](5G)x4",
        "巨人の髭[コア素材](50G)&小さな櫛[換金](20G)",
        "精密な歯車[コア素材](60G)&工具鋼の破片[換金](25G)",
        "とがった爪[コア素材](60G)&刺繍の飾り帯[換金](30G)",
        "触媒のフラスコ[コア素材](60G)&小さな肖像画[換金](30G)",
        "液化魔晶[コア素材](60G)&精密な人形[換金](35G)",
        "魔触媒5[魔触媒5](40G)&魔触媒4[魔触媒4](30G)x2",
        "やわらかい石[コア素材](80G)&小型の蒸留器[換金](20G)",
        "謎めいた毛皮[コア素材](80G)&奇妙な頭骨[換金](30G)",
        "星屑の銀糸[コア素材](80G)&錫引きの星見盤[換金](30G)",
        "魔触媒5[魔触媒5](40G)x3",
        "小型錬成陣[コア素材](100G)&奇妙なオブジェ[換金](20G)",
        "魔触媒6[魔触媒6](50G)&魔触媒5[魔触媒5](40G)x2",
        "呪紋の種[コア素材](100G)&砕けた宝石[換金](30G)",
        "銀の円環[コア素材](100G)&純白のコサージュ[換金](30G)",
        "常眠りの種子[コア素材](100G)&庭師の飾り紐[換金](40G)",
        "飴色の粘液[コア素材](120G)&極彩色の粒[換金](5G)x5",
        "魔触媒6[魔触媒6](50G)x3",
        "銀色の甲殻[コア素材](120G)&乏しいつぼみ[換金](10G)x4",
        "魔触媒7[魔触媒7](60G)&魔触媒6[魔触媒6](50G)x2",
        "大きな魔力結晶[コア素材](120G)&妖精のラクガキ[換金](40G)",
        "拳士の魄片[コア素材](140G)&白紙の巻物[換金](40G)",
        "魔触媒7[魔触媒7](60G)x3",
        "蒼い鉱石[コア素材](140G)&天然ガラスの塊[換金](25G)x2",
        "巨大な風切り羽[コア素材](140G)&分厚い卵殻[換金](50G)",
        "魔触媒8[魔触媒8](70G)x2&魔触媒7[魔触媒7](60G)",
        "絶えない火種[コア素材](180G)&漆黒のスス[換金](20G)",
        "魔触媒8[魔触媒8](70G)x3",
        "魔導錠[コア素材](180G)&つくりかけの錠前[換金](10G)x3",
        "七色の透明捻子[コア素材](180G)&開かない細工箱[換金](40G)",
        "偏属性魔法結晶[コア素材](220G)&レポートの束[換金](10G)",
        "魔触媒9[魔触媒9](90G)&魔触媒8[魔触媒8](70G)x2",
        "魔触媒9[魔触媒9](90G)&魔触媒8[魔触媒8](70G)x2",
        "派手な羽根飾り[コア素材](220G)&空想動物図鑑[換金](30G)",
        "砂のバラ[コア素材](220G)&棘のような水晶[換金](10G)x5",
        "魔触媒9[魔触媒9](90G)x3",
        "頑丈な胃袋[コア素材](220G)&奇妙な標本[換金](50G)",
        "魔触媒10[魔触媒10](110G)&魔触媒9[魔触媒9](90G)x2",
    ],
};

/// i18n `LogHorizon.TRS.ITRS`。
static JA_TRS_ITRS: TreasureTableData = TreasureTableData {
    name: "換金アイテム財宝表",
    first_key: 7,
    items: &[
        "陶器の絵付きマグカップ[換金](40G)",
        "木製の騎士像[換金](40G)",
        "小さな風景画[換金](50G)",
        "奇妙な抽象画[換金](50G)",
        "夜会の仮面[換金](50G)",
        "錫の食器セット[換金](50G)",
        "古い詩集[換金](50G)",
        "鮮やかな刺繍のハンカチ[換金](50G)",
        "陶器の大皿[換金](60G)",
        "絵巻物[換金](60G)",
        "陶器の水盤[換金](60G)",
        "細工物のイヤリング[換金](60G)",
        "淑女の肖像画[換金](70G)",
        "小さな宝石箱[換金](70G)",
        "真鍮の燭台[換金](70G)",
        "貴族の古い日記[換金](70G)",
        "騎士の肖像画[換金](80G)",
        "塗り下駄[換金](80G)",
        "精緻なゲーム盤[換金](80G)",
        "精緻な静物画[換金](90G)",
        "樫の椅子[換金](90G)",
        "古いビスクドール[換金](90G)",
        "きらびやかな仮面[換金](100G)",
        "ウサギのぬいぐるみ[換金](100G)",
        "真鍮の子鬼像[換金](100G)",
        "地方の歴史書[換金](110G)",
        "夜会の手袋[換金](110G)",
        "ハイヒール[換金](120G)",
        "おとぎ話の本[換金](120G)",
        "少女の肖像画[換金](130G)",
        "小さなコケシ[換金](130G)",
        "藤の椅子[換金](130G)",
        "イヌとネコのパペット[換金](140G)",
        "掛け軸[換金](G140)",
        "クマのぬいぐるみ[換金](150G)",
        "ネコのかぶりもの[換金](150G)",
        "学術書[換金](160G)",
        "タカの剥製[換金](170G)",
        "大理石の賢者像[換金](170G)",
        "刺繍をあしらったクッション[換金](180G)",
        "イヌのかぶりもの[換金](180G)",
        "樫の大テーブル[換金](190G)",
        "彫金の指輪[換金](190G)",
        "花鳥画の掛け軸[換金](200G)",
        "最上質の毛布[換金](210G)",
        "上質の白粉[換金](210G)",
        "磁器の茶器[換金](220G)",
        "簡素なティアラ[換金](230G)",
        "彫金のイヤリング[換金](230G)",
        "豪華なネックレス[換金](240G)",
        "上質の香水[換金](250G)",
        "山水画の掛け軸[換金](250G)",
        "手の込んだドレス/礼服[換金](260G)",
        "陶器の絵皿[換金](270G)",
        "キツネのぬいぐるみ[換金](270G)",
        "古い歴史書[換金](280G)",
    ],
};

/// i18n `LogHorizon.TRS.HTRS`。
static JA_TRS_HTRS: TreasureTableData = TreasureTableData {
    name: "ヒロイン財宝表",
    first_key: 7,
    items: &[
        "行き倒れのみすぼらしいやせっぽちの幼女",
        "何よりお金が大事な御団子頭のチャイナ娘",
        "おネダり上手キャミソ肩だしセーター妖術師",
        "小さな背中で皆を護るカットジーンズ武闘家",
        "イカ帽子をかぶったドジっ娘侵略者",
        "呆れるほどポジティブな自称名探偵",
        "肝心な所でドジ踏む猫毛ポニテ殴り施療神官",
        "卵かけご飯の好きな貧乳神祇官",
        "光物大好きガメツイビッグリボン二刀細剣士",
        "オークから命からがら逃げてきた剣の乙女",
        "あなたのことを盲目的に賛美する自称妹",
        "アピールするのに認識されない神祇官",
        "明るくポジティブなピンクのリボンの少女",
        "虚乳の魔法が使えないコンプレックス魔女",
        "食欲に一切躊躇しないむっちり駄肉アイドル",
        "泣きホクロが魅惑の悪戯お姉さん盗剣士",
        "執着心の強い若作りで陽気な年増神話生物",
        "詩歌を愛する平安引きこもり姫",
        "病弱だが誇り高い没落貴族のお嬢様",
        "元気いっぱい！小柄な狼牙族の野生児少女",
        "借金のかたで段ボールハウスなリコピン",
        "帰国子女の巫女服風魔砲少女デース！",
        "ついてくるだけは出来るゾンビ娘",
        "夢にひたむきな緑髪の超時空アイドル候補生",
        "すぐ股間を蹴るギザギザ歯のヤンキー娘",
        "使い減りしない盾",
        "FXで有り金を全部とかしたうつろな先輩",
        "二の腕ぷにぷにだが家計簿を付けられる魔王",
        "その胸は豊満であった記憶喪失の女ニンジャ",
        "よくお裾わけをくれるおさげ髪のパン屋の娘",
        "「汚いは褒め言葉」の目線つき女暗殺者",
        "ボタンが弾けそうで涙目のギルド窓口看板娘",
        "何でもオカルトにしてパニックになる少女",
        "戦闘の度に大食いしてしまう悩める弓術少女",
        "些細なことですぐ絆されちゃう金髪少女",
        "一緒に帰るのが恥ずかしいピンク髪幼馴染",
        "ぽんぽんがペインでトイレ常駐なエルフ少女",
        "見つめると赤面する前髪法儀族少女",
        "すぐビビッてテンパる小動物系少女",
        "背が高く紳士でキザなヅカ系王子様騎士女",
        "竹を割ったような性格の筋肉ドワーフ女",
        "いつもあなたを夢に見ているポニテ少女",
        "先陣先駆け夜討ち朝駆けお寝坊姉御武士",
        "何にでも首を突っ込む旅ガラスのロリババア",
        "事あるごとに踊る薄着南国褐色娘",
        "彼女はあなたの初恋の人に似ている",
    ],
};

/// i18n `LogHorizon.TRS.GTRS`。
static JA_TRS_GTRS: TreasureTableData = TreasureTableData {
    name: "ゴブリン財宝表",
    first_key: 7,
    items: &[
        "ライトメイス(40G)",
        "百科事典(40G)",
        "42G",
        "古びた髑髏[コア素材](40G)",
        "加速の巻物(初級)(45G)",
        "46G",
        "48G",
        "50G",
        "ガラス玉[換金](60G)",
        "血塗られた刃[コア素材](50G)",
        "歪んだ金皿[換金](60G)",
        "59G",
        "毛皮の敷物[換金](60G)",
        "とがった爪[コア素材](60G)",
        "67G",
        "傷だらけの象牙像[換金](70G)",
        "72G",
        "ライトランス[破損](75G)",
        "骨のネックレス[換金](80G)",
        "砂金混じりの石[換金](80G)",
        "85G",
        "香木の小片[換金](90G)",
        "奇妙なお面[換金](90G)",
        "96G",
        "シミター(100G)",
        "ウッデンラウンド(100G)",
        "風鳴りの鈴[コア素材](80G)&鉄の陣笠[コア素材](30G)",
        "112G",
        "山羊スライム(大)[換金](120G)",
        "121G",
        "高級桧材[換金](125G)",
        "130G",
        "水晶のチェス駒[換金](135G)",
        "140G",
        "テント(キャンプ用)(150G)",
        "怪しい丸薬[コア素材](30G)x2&100G",
        "謎めいた毛皮[コア素材](80G)&怨念の鍔[コア素材](80G)",
        "儀式の骨剣[換金](160G)",
        "古ぼけたコイン[換金](165G)",
        "狼牙棒(170G)",
        "法理回路[コア素材](60G)&120G",
        "大きな魔力結晶[コア素材](120G)&60G",
        "189G",
        "上等な樽酒[換金](200G)",
        "201G",
        "白の指輪(210G)",
        "ヒスイの首飾り[換金](210G)",
        "金のゴブリン像[換金](220G)",
        "サーベル(230G)",
        "真鉄の刀身[コア素材](120G)&120G",
        "汚れた青磁の壺[換金](240G)",
        "とんすとんの焼き串盛り合わせ(120G)x2",
        "ゴブリン王の勲章[換金](255G)",
        "垣間見の巻物(中級)(130G)x2",
        "269G",
        "白狼の毛皮[換金](280G)",
    ],
};

/// Ruby `construct_treasure_table`（`[CMIHG]TRS` → 表）。
static JA_TRS: &[(&str, &TreasureTableData)] = &[
    ("CTRS", &JA_TRS_CTRS),
    ("MTRS", &JA_TRS_MTRS),
    ("ITRS", &JA_TRS_ITRS),
    ("HTRS", &JA_TRS_HTRS),
    ("GTRS", &JA_TRS_GTRS),
];

/// i18n `LogHorizon.TRSE.CTRSE`。
static JA_TRSE_CTRSE: TreasureTableData = TreasureTableData {
    name: "金銭財宝表（拡張）",
    first_key: 7,
    items: &[
        "35G", "40G", "40G", "40G", "45G", "45G", "45G", "50G", "50G", "50G", "55G", "55G", "60G",
        "60G", "65G", "70G", "70G", "75G", "75G", "80G", "85G", "85G", "90G", "95G", "100G",
        "100G", "105G", "110G", "115G", "120G", "125G", "130G", "135G", "140G", "145G", "150G",
        "155G", "160G", "165G", "170G", "175G", "180G", "185G", "195G", "200G", "205G", "210G",
        "220G", "225G", "230G", "240G", "245G", "255G", "260G", "265G", "275G", "280G", "290G",
        "300G", "300G", "310G", "320G", "330G", "340G", "340G", "350G", "360G", "370G", "380G",
        "390G", "400G", "410G", "420G", "430G", "440G", "450G", "460G", "460G", "480G", "490G",
        "500G", "510G", "520G", "530G", "540G", "550G", "560G", "570G", "580G", "590G", "610G",
        "620G", "630G", "640G", "650G", "660G", "680G", "690G", "700G", "710G", "730G", "740G",
        "750G", "760G", "780G", "790G", "800G", "820G", "830G", "840G", "860G", "870G", "890G",
        "900G", "910G", "930G", "940G", "960G", "970G", "990G", "1000G", "1020G", "1030G", "1050G",
        "1060G", "1080G", "1090G", "1110G", "1130G", "1140G", "1160G", "1170G", "1190G", "1210G",
        "1220G", "1240G", "1260G", "1270G", "1290G", "1310G", "1330G", "1340G", "1360G", "1380G",
        "1400G", "1410G", "1430G", "1450G", "1470G", "1490G", "1500G", "1520G", "1540G", "1560G",
        "1580G", "1600G",
    ],
};

/// i18n `LogHorizon.TRSE.MTRSE`。
static JA_TRSE_MTRSE: TreasureTableData = TreasureTableData {
    name: "魔法素材財宝表（拡張）",
    first_key: 7,
    items: &[
        "魔触媒2[魔触媒2](20G)&魔触媒1[魔触媒1](15G)",
        "新緑の若芽[コア素材](30G)",
        "魔触媒2[魔触媒2](20G)&魔触媒1[魔触媒1](15G)",
        "怪しい丸薬[コア素材](30G)",
        "魔触媒2[魔触媒2](20G)&魔触媒1[魔触媒1](15G)",
        "滑らかな被膜[コア素材](30G)",
        "魔触媒2[魔触媒2](20G)&魔触媒1[魔触媒1](15G)",
        "呪法シリンダー[コア素材](30G)",
        "魔触媒3[魔触媒3](25G)&魔触媒2[魔触媒2](20G)",
        "反魔水銀[コア素材](40G)",
        "魔触媒3[魔触媒3](25G)&魔触媒2[魔触媒2](20G)",
        "鋭い牙[コア素材](40G)",
        "魔触媒3[魔触媒3](25G)&魔触媒2[魔触媒2](20G)",
        "自戒の茨[コア素材](50G)",
        "魔触媒4[魔触媒4](30G)&魔触媒3[魔触媒3](25G)",
        "流星のかけら[コア素材](50G)",
        "魔触媒4[魔触媒4](30G)&魔触媒3[魔触媒3](25G)",
        "聖なる繊維[コア素材](50G)",
        "魔触媒5[魔触媒5](40G)&魔触媒4[魔触媒4](30G)",
        "巨人の髭[コア素材](60G)",
        "魔触媒5[魔触媒5](40G)&魔触媒4[魔触媒4](30G)",
        "精密な歯車[コア素材](60G)",
        "魔触媒5[魔触媒5](40G)&魔触媒4[魔触媒4](30G)",
        "謎めいた毛皮[コア素材](80G)",
        "魔触媒6[魔触媒6](50G)&魔触媒5[魔触媒5](40G)",
        "雄々しい角[コア素材](80G)",
        "魔触媒6[魔触媒6](50G)&魔触媒5[魔触媒5](40G)",
        "つややかな繭[コア素材](80G)",
        "魔触媒7[魔触媒7](60G)&魔触媒6[魔触媒6](50G)",
        "小型錬成陣[コア素材](100G)",
        "魔触媒7[魔触媒7](60G)&魔触媒6[魔触媒6](50G)",
        "銀の円環[コア素材](100G)",
        "魔触媒7[魔触媒7](60G)&魔触媒6[魔触媒6](50G)",
        "仙桃果[コア素材](120G)",
        "魔触媒8[魔触媒8](70G)&魔触媒7[魔触媒7](60G)",
        "動力ケーブル[コア素材](120G)",
        "魔触媒8[魔触媒8](70G)&魔触媒7[魔触媒7](60G)",
        "流星歯車[コア素材](120G)",
        "魔触媒9[魔触媒9](90G)&魔触媒8[魔触媒8](70G)",
        "茨のトゲ[コア素材](140G)",
        "魔触媒9[魔触媒9](90G)&魔触媒8[魔触媒8](70G)",
        "砕けた剛剣[コア素材](140G)",
        "魔触媒9[魔触媒9](90G)&魔触媒8[魔触媒8](70G)",
        "絶えない火種[コア素材](180G)",
        "魔触媒10[魔触媒10](110G)&魔触媒9[魔触媒9](90G)",
        "魔導錠[コア素材](180G)",
        "魔触媒10[魔触媒10](110G)&魔触媒9[魔触媒9](90G)",
        "青ざめた盾鱗[コア素材](180G)",
        "魔触媒10[魔触媒10](110G)x2",
        "破軍の戦帯[コア素材](220G)",
        "魔触媒11[魔触媒11](120G)&魔触媒10[魔触媒10](110G)",
        "派手な羽根飾り[コア素材](220G)",
        "魔触媒11[魔触媒11](120G)&魔触媒10[魔触媒10](110G)",
        "神隠しの古枝[コア素材](240G)",
        "魔触媒11[魔触媒11](120G)x2",
        "呪紋水晶[コア素材](240G)",
        "魔触媒12[魔触媒12](150G)&魔触媒11[魔触媒11](120G)",
        "謎めいたフラスコ[コア素材](240G)",
        "魔触媒12[魔触媒12](150G)&魔触媒11[魔触媒11](120G)",
        "未熟な竜玉[コア素材](300G)",
        "魔触媒12[魔触媒12](150G)x2",
        "陰陽の水銀[コア素材](300G)",
        "魔触媒13[魔触媒13](170G)&魔触媒12[魔触媒12](150G)",
        "金剛骨[コア素材](340G)",
        "魔触媒13[魔触媒13](170G)x2",
        "アルヴの研磨剤[コア素材](340G)",
        "魔触媒14[魔触媒14](190G)&魔触媒13[魔触媒13](170G)",
        "不死王の心臓[コア素材](340G)",
        "魔触媒14[魔触媒14](190G)x2",
        "緑小鬼の大軍旗[コア素材](380G)",
        "魔触媒14[魔触媒14](190G)x2",
        "深層アダマン鉱石[コア素材](380G)",
        "魔触媒15[魔触媒15](220G)&魔触媒14[魔触媒14](190G)",
        "漆黒の眼球[コア素材](440G)",
        "魔触媒15[魔触媒15](220G)&魔触媒14[魔触媒14](190G)",
        "雪桃の実[コア素材](440G)",
        "魔触媒15[魔触媒15](220G)x2",
        "四つ葉のアンク[コア素材](440G)",
        "魔触媒16[魔触媒16](250G)&魔触媒15[魔触媒15](220G)",
        "金色のたてがみ[コア素材](500G)",
        "魔触媒16[魔触媒16](250G)&魔触媒15[魔触媒15](220G)",
        "銀の車輪[コア素材](500G)",
        "魔触媒16[魔触媒16](250G)x2",
        "封緑樹の堅枝[コア素材](560G)",
        "魔触媒17[魔触媒17](280G)&魔触媒16[魔触媒16](250G)",
        "千年王樹の種子[コア素材](560G)",
        "魔触媒17[魔触媒17](280G)x2",
        "巨鳥の鎖骨[コア素材](560G)",
        "魔触媒17[魔触媒17](280G)x2",
        "水棲緑鬼の王卵[コア素材](620G)",
        "魔触媒18[魔触媒18](310G)&魔触媒17[魔触媒17](280G)",
        "太陽の飾り鋲[コア素材](620G)",
        "魔触媒18[魔触媒18](310G)x2",
        "火精霊の心核[コア素材](680G)",
        "魔触媒19[魔触媒19](340G)&魔触媒18[魔触媒18](310G)",
        "獄炎狼の房毛[コア素材](680G)",
        "魔触媒19[魔触媒19](340G)x2",
        "島クジラの巨骨[コア素材](680G)",
        "魔触媒19[魔触媒19](340G)x2",
        "原初の炎[コア素材](740G)",
        "魔触媒20[魔触媒20](370G)&魔触媒19[魔触媒19](340G)",
        "魔石草の種[コア素材](740G)",
        "魔触媒20[魔触媒20](370G)x2",
        "稠密立方魔晶[コア素材](820G)",
        "魔触媒21[魔触媒21](410G)&魔触媒20[魔触媒20](370G)",
        "清掃用女中人形九号[コア素材](820G)",
        "魔触媒21[魔触媒21](410G)&魔触媒20[魔触媒20](370G)",
        "飛びイルカの心臓[コア素材](820G)",
        "魔触媒21[魔触媒21](410G)x2",
        "奈落皇蛇の額鱗[コア素材](900G)",
        "魔触媒22[魔触媒22](450G)&魔触媒21[魔触媒21](410G)",
        "ブリザードダイヤ[コア素材](900G)",
        "魔触媒22[魔触媒22](450G)&魔触媒21[魔触媒21](410G)",
        "火炎向日葵の花びら[コア素材](980G)",
        "魔触媒22[魔触媒22](450G)x2",
        "紅蓮蝙蝠の鮮血[コア素材](980G)",
        "魔触媒23[魔触媒23](490G)&魔触媒22[魔触媒22](450G)",
        "落日祈祷書[コア素材](980G)",
        "魔触媒24[魔触媒24](530G)&黒檀の馬具[換金](440G)",
        "辰砂の組紐[コア素材](1060G)",
        "魔触媒23[魔触媒23](490G)x2",
        "黴臭い皮の剣帯[コア素材](1060G)",
        "魔触媒24[魔触媒24](530G)&魔触媒23[魔触媒23](490G)",
        "玻璃蜥蜴の抜け殻[コア素材](1160G)",
        "魔触媒24[魔触媒24](530G)x2",
        "火炎巨人の尺骨[コア素材](1160G)",
        "魔触媒24[魔触媒24](530G)x2",
        "高熱練炭[コア素材](1160G)",
        "魔触媒26[魔触媒26](620G)&青銅の装置[換金](510G)",
        "すり切れた羽衣[コア素材](1240G)",
        "魔触媒25[魔触媒25](580G)x2",
        "亡霊のため息[コア素材](1240G)",
        "魔触媒25[魔触媒25](580G)x2",
        "宵闇色のヒゲ[コア素材](1340G)",
        "魔触媒27[魔触媒27](670G)&月長石の腕輪[換金](550G)",
        "コアトルの毒腺[コア素材](1340G)",
        "魔触媒26[魔触媒26](620G)x2",
        "玄亀の甲羅[コア素材](1340G)",
        "魔触媒28[魔触媒28](720G)&宝玉のネックレス[換金](570G)",
        "山大鹿の角[コア素材](1440G)",
        "魔触媒28[魔触媒28](720G)&消えない燭台[換金](600G)",
        "煉獄クモの燃糸[コア素材](1540G)",
        "魔触媒27[魔触媒27](670G)x2",
        "碧氷晶の単眼球[コア素材](1540G)",
        "魔触媒29[魔触媒29](770G)&琥珀のイヤリング[換金](630G)",
        "衝撃竜の牙[コア素材](1540G)",
        "魔触媒29[魔触媒29](770G)&茨を模したティアラ[換金](650G)",
        "鉄鼠の自在尾[コア素材](1540G)",
        "魔触媒30[魔触媒30](820G)&石榴石の腕輪[換金](650G)",
        "人工生命の素[コア素材](1640G)",
        "魔触媒30[魔触媒30](820G)&輝くメガネ[換金](680G)",
        "翡翠甲虫の刃翅[コア素材](1640G)",
        "魔触媒29[魔触媒29](770G)x2",
        "無限渦流の煤[コア素材](1640G)",
        "魔触媒30[魔触媒30](820G)&スライム入り試験管[換金](750G)",
        "陽喰い鳥の嘴[コア素材](1640G)",
    ],
};

/// i18n `LogHorizon.TRSE.ITRSE`。
static JA_TRSE_ITRSE: TreasureTableData = TreasureTableData {
    name: "換金アイテム財宝表（拡張）",
    first_key: 7,
    items: &[
        "破れた書籍[換金](30G)",
        "古ぼけたぬいぐるみ[換金](40G)",
        "あやしい書籍[換金](40G)",
        "分厚い書籍[換金](40G)",
        "子供向けの書籍[換金](40G)",
        "犬のぬいぐるみ[換金](40G)",
        "使い込まれたティーセット[換金](40G)",
        "クマのぬいぐるみ[換金](50G)",
        "白磁のティーセット[換金](50G)",
        "藍色の染料[換金](50G)",
        "毛糸の帽子[換金](50G)",
        "恐竜のぬいぐるみ[換金](50G)",
        "豪華な日記[換金](60G)",
        "乳白色の角[換金](60G)",
        "羽根付き帽子[換金](60G)",
        "魔法の書籍[換金](70G)",
        "木製の食器[換金](70G)",
        "猛獣の毛皮[換金](70G)",
        "魔法使いの帽子[換金](70G)",
        "貴族の絵本[換金](80G)",
        "陶製の食器[換金](80G)",
        "磨かれたティーセット[換金](80G)",
        "しなやかな毛皮[換金](90G)",
        "巨大な角[換金](90G)",
        "貴重な書籍[換金](100G)",
        "銀の食器[換金](100G)",
        "魔獣の毛皮[換金](100G)",
        "司祭の帽子[換金](110G)",
        "野獣の牙[換金](110G)",
        "巨人のスプーン[換金](120G)",
        "枝分かれした角[換金](120G)",
        "猫耳帽子[換金](130G)",
        "木綿のドレス[換金](130G)",
        "水晶の角[換金](140G)",
        "琥珀の円盤[換金](140G)",
        "幻獣の牙[換金](150G)",
        "古代のドレス[換金](150G)",
        "王朝風のティーセット[換金](160G)",
        "傷ひとつない毛皮[換金](160G)",
        "絹のドレス[換金](170G)",
        "魔法のぬいぐるみ[換金](170G)",
        "神獣の毛皮[換金](180G)",
        "水晶の食器[換金](180G)",
        "石の呪具[換金](190G)",
        "禍々しい仮面[換金](200G)",
        "ねず耳カチューシャ[換金](200G)",
        "竜の牙[換金](210G)",
        "スライム？の剥製[換金](220G)",
        "銀のネジ[換金](220G)",
        "大猫の剥製[換金](230G)",
        "舞踏会の仮面[換金](240G)",
        "重厚な牙[換金](240G)",
        "宝石細工の仮面[換金](250G)",
        "サテンのドレス[換金](260G)",
        "光る角[換金](260G)",
        "魔法のドレス[換金](270G)",
        "道化師の仮面[換金](280G)",
        "アルヴのティーセット[換金](290G)",
        "燃えない毛皮[換金](300G)",
        "雪狼の剥製[換金](300G)",
        "籐製の椅子[換金](310G)",
        "赤い石板[換金](320G)",
        "黒獅子の剥製[換金](330G)",
        "三角の椅子[換金](340G)",
        "子供用紳士服[換金](340G)",
        "アルヴの食器[換金](350G)",
        "ほつれた紳士服[換金](360G)",
        "上等な紳士服[換金](370G)",
        "寄木細工の文箱[換金](380G)",
        "白磁の燭台[換金](390G)",
        "黄金の牙[換金](400G)",
        "白木の文箱[換金](410G)",
        "青銅の燭台[換金](420G)",
        "輝く燭台[換金](430G)",
        "王様の椅子[換金](440G)",
        "古風な紳士服[換金](450G)",
        "思い出のドレス[換金](460G)",
        "古代の種[換金](460G)",
        "小さな宝石箱[換金](480G)",
        "ひびわれた姿見[換金](490G)",
        "楓の書棚[換金](500G)",
        "神秘的な仮面[換金](510G)",
        "トネリコの書見台[換金](520G)",
        "ビロードの紳士服[換金](530G)",
        "磨きこまれた姿見[換金](540G)",
        "大きな宝石箱[換金](550G)",
        "白檀の文箱[換金](560G)",
        "白樺の姿見[換金](570G)",
        "焼き物の像[換金](580G)",
        "彫刻付き姿見[換金](590G)",
        "粘土の像[換金](610G)",
        "銀の燭台[換金](620G)",
        "鳳凰の剥製[換金](630G)",
        "呪われた像[換金](640G)",
        "魔王の椅子[換金](650G)",
        "神域の椅子[換金](660G)",
        "満杯の宝石箱[換金](680G)",
        "大理石の貨幣[換金](690G)",
        "絵の具のセット[換金](700G)",
        "染料のセット[換金](710G)",
        "木製の貨幣[換金](730G)",
        "貝殻の貨幣[換金](740G)",
        "覆い付き書見台[換金](750G)",
        "手作りのゲーム盤[換金](760G)",
        "祭祀用の像[換金](780G)",
        "真鍮製のゲーム盤[換金](790G)",
        "螺鈿の文箱[換金](800G)",
        "青銅の書見台[換金](820G)",
        "香木の小像[換金](830G)",
        "魔法使いの像[換金](840G)",
        "鍵のかかった宝石箱[換金](860G)",
        "いかさまゲーム盤[換金](870G)",
        "魔法の紳士服[換金](890G)",
        "化粧品のセット[換金](900G)",
        "古代の姫の小像[換金](910G)",
        "翡翠の小像[換金](930G)",
        "真鍮の鉱石[換金](940G)",
        "真鉄の塊[換金](960G)",
        "銀の塊[換金](970G)",
        "古代の貨幣[換金](990G)",
        "瑪瑙の文箱[換金](1000G)",
        "銀細工の書見台[換金](1020G)",
        "粗末な馬具[換金](1030G)",
        "あふれた宝石箱[換金](1050G)",
        "異文明の貨幣[換金](1060G)",
        "透明な文箱[換金](1080G)",
        "香水のセット[換金](1090G)",
        "鳥が象られたティアラ[換金](1110G)",
        "金細工の燭台[換金](1130G)",
        "重厚な馬具[換金](1140G)",
        "香辛料のセット[換金](1160G)",
        "太陽のティアラ[換金](1170G)",
        "豪華な姿見[換金](1190G)",
        "月のティアラ[換金](1210G)",
        "水晶のメガネ[換金](1220G)",
        "魔法銀の塊[換金](1240G)",
        "硝子のメガネ[換金](1260G)",
        "金細工のゲーム盤[換金](1270G)",
        "金剛石のメガネ[換金](1290G)",
        "品のあるネックレス[換金](1310G)",
        "植物の標本[換金](1330G)",
        "くすんだ首輪[換金](1340G)",
        "空色鋼の塊[換金](1360G)",
        "騎士の像[換金](1380G)",
        "ちぎれたネックレス[換金](1400G)",
        "漆黒の呪具[換金](1410G)",
        "精巧な馬具[換金](1430G)",
        "飾り気のないティアラ[換金](1450G)",
        "動物の標本[換金](1470G)",
        "人造人間の標本[換金](1490G)",
        "しゃべる姿見[換金](1500G)",
        "精巧なゲーム盤[換金](1520G)",
        "アルヴの貨幣[換金](1540G)",
        "豪奢なネックレス[換金](1560G)",
        "猫目石の腕輪[換金](1580G)",
        "藍玉の腕輪[換金](1600G)",
    ],
};

/// i18n `LogHorizon.TRSE.OTRSE`。
static JA_TRSE_OTRSE: TreasureTableData = TreasureTableData {
    name: "そのほか財宝表（拡張）",
    first_key: 7,
    items: &[
        "百科事典(40G)",
        "ライトメイス(40G)",
        "蘇生の宝珠(初級)(40G)",
        "季節の野菜ポトフ(20G)x2",
        "荷馬車の笛(45G)",
        "モンスター図鑑(初級)(40G)&お好みサンドイッチ(10G)",
        "ファインダガー(50G)",
        "パッデドアーマー[破損](55G)",
        "鉄壁符(初級)(45G)",
        "ショートボウ[破損](55G)",
        "マーシャルアーツドレス[破損](60G)",
        "ケルピーの鱗(60G)",
        "クレセントバーガー(30G)x2",
        "シェフの気まぐれピザ(60G)",
        "スモールシールド(70G)",
        "ウィローバンド[破損](30G)&暗闇透視薬(初級)(40G)",
        "スモールシールド[破損](35G)&魔精の小瓶(初級)(40G)",
        "鉢がね(80G)",
        "ベリーのタルト(40G)x2",
        "防水バッグ[破損](35G)&水中活動の巻物(初級)(50G)",
        "スティレット(90G)",
        "飛行の巻物(初級)(50G)&強酸毒(初級)(40G)",
        "スプリントメイル[破損](100G)",
        "異次元バッグ(初級)[破損](45G)&おにぎり(50G)",
        "ウッデンラウンド(100G)",
        "野太刀[破損](110G)",
        "保護の巻物(初級)(70G)&消耗毒(初級)(50G)",
        "ライトクロスボウ[破損](115G)",
        "良質な楽器(110G)",
        "ハンターフード[破損](60G)&猛毒薬(中級)(60G)",
        "とんすとんの焼き串盛り合わせ(120G)",
        "クィルブイリ[破損](135G)",
        "ケープ(140G)",
        "トートバッグ(140G)",
        "三つ目鳥の卵炒飯(70G)&衰弱毒(中級)(80G)",
        "範囲隠密の巻物(初級)(150G)",
        "アイアンシールド(150G)",
        "クイックカード(初級)(170G)",
        "イーグルカード(初級)(170G)",
        "和弓[破損](170G)",
        "エストック[破損](170G)",
        "食料バッグ(170G)",
        "トポト親方のコロッケ(90G)&拘束毒(中級)(100G)",
        "モーニングスター(200G)",
        "シーダーのスタッフ[破損](200G)",
        "カイトシールド(220G)",
        "怪鳥肉の水炊き(210G)",
        "チェインシャツ[破損](245G)",
        "大弓[破損](240G)",
        "範囲飛行の巻物(初級)(210G)",
        "ポーションホルダー(中級)(240G)",
        "アウトレイジ[破損](240G)",
        "定番カレー(130G)x2",
        "蘇生の宝珠(中級)(240G)",
        "招命の宝珠(中級)(120G)&焼きそばパン(150G)",
        "異次元バッグ(中級)(280G)",
        "名工の楽器(280G)",
        "月狼の牙[破損](180G)&解毒薬(中級)(130G)",
        "魚鱗の宝珠(中級)(280G)",
        "防水ポーチ(320G)",
        "収納シールド(320G)",
        "火炎のイヤリング(330G)",
        "絹の帽子(350G)",
        "幻翼の宝珠(中級)(320G)",
        "バグナウ[破損](340G)",
        "ロングボウ[破損](370G)",
        "ショテル[破損](370G)",
        "デッキシールド[破損](185G)&チキンボックス(200G)",
        "コルセスカ[破損](365G)",
        "サイプレスのスタッフ[破損](425G)",
        "チェーンシックル[破損](230G)&苺のふんわりカップケーキ(230G)",
        "ブロードアックス(410G)",
        "クロスシールド(470G)",
        "モンスター図鑑(上級)(400G)",
        "羽毛のケープ(400G)",
        "鉄身のブーツ(400G)",
        "看破の宝珠(中級)(420G)",
        "アドベンチャーザック(480G)",
        "隠密の巻物(中級)(460G)",
        "ファルクス[破損](500G)",
        "ポーションホルダー(上級)(540G)",
        "カタール[破損](500G)",
        "名匠の楽器(540G)",
        "技能薬(上級)(480G)",
        "火炎のタリスマン(620G)",
        "からすの手提げ袋(600G)",
        "範囲隠密の巻物(中級)(570G)",
        "トラベラーハット(650G)",
        "高位保護の巻物(中級)(520G)",
        "長柄槍[破損](600G)",
        "範囲飛行の巻物(中級)(570G)",
        "クイックカード(中級)(620G)",
        "トラベラーハット(650G)",
        "剛手甲[破損](650G)",
        "掃討の巻物(特級)(570G)",
        "エスパドン[破損](700G)",
        "異次元バッグ(上級)(740G)",
        "まごころミートパイ(410G)x2",
        "李満弓[破損](700G)",
        "招命の宝珠(上級)(395G)x2",
        "識別の巻物(上級)(690G)",
        "リネンクロス[破損](850G)",
        "出前のおかもち(820G)",
        "回復薬(上級)(720G)",
        "蘇生の宝珠(上級)(790G)",
        "再生薬(上級)(790G)",
        "エレメンタルカード(上級)(750G)",
        "極上の楽器(900G)",
        "踏破の巻物(上級)(750G)",
        "垣間見の巻物(特級)(750G)",
        "鉄壁符(上級)(820G)",
        "魚鱗の宝珠(上級)(860G)",
        "神奉刀[破損](850G)",
        "ヘヴィナックル[破損](950G)",
        "巧指符(特級)(1000G)",
        "幻翼の宝珠(上級)(930G)",
        "火炎のベルト(1000G)",
        "シルキーベレー(1100G)",
        "ヒーリングワード(上級)(970G)",
        "技能薬(特級)(930G)",
        "クレセントアックス[破損](1100G)",
        "ウォーハンマー(1100G)",
        "ミスルトゥのスタッフ[破損](1150G)",
        "範囲保護の巻物(上級)(970G)",
        "浄眼の巻物(上級)(970G)",
        "マジカルカード(上級)(1000G)",
        "現身符(上級)(1000G)",
        "ジュエリーポーチ(1200G)",
        "耐魔の軟膏(上級)(1000G)",
        "鉄身の軟膏(上級)(1000G)",
        "範囲飛行の巻物(上級)(1100G)",
        "隠密の巻物(上級)(1100G)",
        "魔性の楽器(1300G)",
        "良業物[破損](1150G)",
        "範囲付呪の巻物(上級)(1100G)",
        "ジャマダハル[破損](1300G)",
        "マスターパック(1400G)",
        "ドライハンダー[破損](1450G)",
        "グレイトヘルム[破損](1050G)",
        "高位保護の巻物(上級)(1200G)",
        "範囲隠密の巻物(上級)(1200G)",
        "ローズサークレット(1600G)",
        "火炎のメダリオン(1600G)",
        "ファインジャベリン[破損](1200G)",
        "カッツバルゲル[破損](1250G)",
        "紫の指輪[破損](1300G)",
        "異次元バッグ(特急)(1600G)",
        "五人張[破損](1750G)",
        "皇国技芸寮打物[破損](1400G)",
        "ヘヴィランス[破損](1500G)",
        "竜手甲[破損](1650G)",
        "ポロニアのワンド[破損](1700G)",
        "デスサイズ[破損](1700G)",
        "フランシスカ(1700G)",
        "伝説の楽器(1700G)",
        "ウルミン(1900G)",
    ],
};

/// Ruby `roll_treasure_table_b2`（`[CMIO]TRSE` → 表）。
pub(crate) static JA_TRSE: &[(&str, &TreasureTableData)] = &[
    ("CTRSE", &JA_TRSE_CTRSE),
    ("MTRSE", &JA_TRSE_MTRSE),
    ("ITRSE", &JA_TRSE_ITRSE),
    ("OTRSE", &JA_TRSE_OTRSE),
];

/// i18n `LogHorizon.IAT.A`。
static JA_IAT_A: NamedItems = NamedItems {
    name: "特徴A(メリット)",
    items: &[
        "光って回って音が鳴る",
        "何かに目覚める",
        "硬くて強い",
        "すごく速い",
        "大量生産できる",
        "よい手触り",
    ],
};

/// i18n `LogHorizon.IAT.B`。
static JA_IAT_B: NamedItems = NamedItems {
    name: "特徴B(デメリット)",
    items: &[
        "中毒性のある",
        "凄まじく重い",
        "ひどい臭いの",
        "壊れやすい",
        "マニュアルが超厚い",
        "捨てても戻ってくる",
    ],
};

/// i18n `LogHorizon.IAT.L`。
static JA_IAT_L: NamedItems = NamedItems {
    name: "見た目",
    items: &[
        "美しい",
        "素朴な",
        "風流な",
        "斬新な",
        "名状しがたき",
        "命を刈り取る形をしている",
    ],
};

/// i18n `LogHorizon.IAT.T`。
static JA_IAT_T: NamedItems = NamedItems {
    name: "発明品の種類",
    items: &["武器", "防具", "アクセサリ", "食料", "薬品", "乗り物"],
};

/// i18n `LogHorizon.TIAS`。
static JA_TIAS: RandomTableData = RandomTableData {
    name: "アキバの街で遭遇するトラブル",
    tables: &[
        &[
            "〈記録の地平線〉が",
            "〈三日月同盟〉が",
            "〈Ｄ．Ｄ．Ｄ〉が",
            "〈黒剣騎士団〉が",
            "〈西風の旅団〉が",
            "〈ロデリック商会〉が",
        ],
        &[
            "ギルド会館で",
            "くいだおれ横丁で",
            "街の真ん中で",
            "水楓の館で",
            "公衆浴場で",
            "下水道で",
        ],
        &[
            "理不尽な",
            "ささやかな",
            "甘い",
            "不自然な",
            "危険な",
            "頭の悪い",
        ],
        &["喧嘩", "恋愛沙汰", "いたずら", "陰謀", "事故", "落し物"],
    ],
};

/// i18n `LogHorizon.ABDC`。
static JA_ABDC: RandomTableData = RandomTableData {
    name: "廃棄児",
    tables: &[
        &[
            "名前：花の名前",
            "名前：星の名前",
            "名前：色の名前",
            "名前：石の名前",
            "名前：動物の名前",
            "名前：番号",
        ],
        &[
            "住居：廃ビル",
            "住居：道端",
            "住居：居候",
            "住居：木の上",
            "住居：公園",
            "住居：下水道",
        ],
        &[
            "特技：探し物",
            "特技：料理",
            "特技：歌",
            "特技：商売",
            "特技：手先が器用",
            "特技：あざといポーズ",
        ],
        &[
            "体型：痩せている",
            "体型：ちびっこ",
            "体型：発育良好",
            "体型：背が高い",
            "体型：ぷにぷに",
            "体型：ガチムチ",
        ],
        &[
            "好きな食べ物：果物",
            "好きな食べ物：お肉",
            "好きな食べ物：お野菜",
            "好きな食べ物：お魚",
            "好きな食べ物：お菓子",
            "好きな食べ物：虫",
        ],
        &[
            "一人称：ぼく/わたし",
            "一人称：オイラ/アタシ",
            "一人称：俺/私",
            "一人称：拙者/わらわ",
            "一人称：自分の名前",
            "一人称：ミー",
        ],
    ],
};

/// i18n `LogHorizon.MII`。
static JA_MII: MusicalInstrumentData = MusicalInstrumentData {
    name: "楽器種別表",
    type_list: &[
        "打楽器１",
        "鍵盤楽器",
        "弦楽器１",
        "弦楽器２",
        "管楽器１",
        "管楽器２",
    ],
    items: &[
        &[
            "カスタネット",
            "マラカス",
            "シンバル",
            "トライアングル",
            "太鼓",
            "ドラム",
        ],
        &[
            "木琴",
            "鉄琴",
            "ハーモニウム",
            "ハープシコード",
            "ピアノ",
            "クラヴィコード",
        ],
        &[
            "ハープ",
            "リュート",
            "ギター",
            "バイオリン",
            "チェロ",
            "リラ",
        ],
        &["琵琶", "和琴", "胡琴", "三味線", "シタール", "ダルシマー"],
        &[
            "トランペット",
            "ホルン",
            "トロンボーン",
            "チューバ",
            "フルート",
            "クラリネット",
        ],
        &[
            "リコーダー",
            "オカリナ",
            "オーボエ",
            "ハーモニカ",
            "アコーディオン",
            "尺八",
        ],
    ],
};

/// i18n `LogHorizon.ESTL`。項目は YAML のブロックスカラー（末尾に改行が1つ残る）を
/// そのまま持ち、Ruby `table[total].chomp` に対応する [`ruby_chomp`] で落とす。
pub(crate) static JA_ESTL: EastalData = EastalData {
    name: "イースタル探索表",
    first_key: 7,
    items: &[
        "香りに誘われて：\n　小さな街道沿いで、商隊が〈走り茸〉にまとわりつかれている。\n　奴らを追い払うと、商隊のリーダーがお礼にと人数分の「干した〈走り松茸〉[換金](40G)」をくれる。\n　追われていた理由はこれだったらしい。\n",
        "不幸な出会い：\n　街道を歩いていると、横合いの茂みから〈小牙竜鬼〉の一団が飛び出してきた。\n　突然のことでとても驚いたけど、向こうも〈冒険者〉がいるとは思っていなかったらしい。\n　一瞬の気まずい沈黙を経て、なし崩し的に乱戦が始まった。\n　PCは全員[疲労:10]を受ける。\n",
        "釣果はいかが？：\n　〈大地人〉の村近くの川で釣りが盛んなようだ。\n　挑戦するならば長靴の中に(2D*5)Gを見つけることができる。\n　この釣り場、ハズレなんじゃない？\n",
        "近道だと思ったんだけど：\n　「この雑木林を抜けた方が早いよ」なんて誰が言い出したのやら。\n　近道に踏み込んだら、見事に道に迷ってしまった。\n　半日を余計に消費してしまう。\n　なんか疲れた……。\n　PCは全員【因果力】1点を失う。\n",
        "おすすめの名産品：\n　キミたちは〈大地人〉の村に出くわした。\n　〈円卓会議〉と契約している農村で、〈冒険者〉にも好意的に接してくれる。\n　素朴だけど手厚いもてなしが心にしみた。\n　村の食堂でなにか食べるのなら10Gを支払い【因果力】に+1する。\n",
        "思わぬ報酬：\n　畑を開墾していた〈大地人〉農夫が、切り株を掘り起こせずに途方に暮れていた。\n　手助けするならPCは全員[疲労:10]を受ける。\n　みんなで力を合わせて切り株を掘り起こすと、そこにはなんと「鉄の陣笠[コア素材](30G)」が埋まっていた！\n　キミたちは「鉄の陣笠」とお礼にもらった「お好みサンドイッチ(『LHZB1』P218)」2個を受け取る。\n",
        "きりきり舞い：\n　野原の向こう側でつむじ風が舞っている、と思ったらどんどん速度を上げてこっちに向かってきた！\n　これはもう小さな竜巻だ。\n　PCは全員「難易度:9」の[耐久判定]を行なう。\n　失敗したPCは、風に耐えきれずにアイテムを吹き飛ばされる。\n　[消耗品]アイテムを1個失う。\n",
        "小さな祝福：\n　雑木林の中を進んでいくとキラキラする光が集まってくる。\n　PCのなかに〈森呪遣い〉もしくは〈召喚術師〉がいるのならば小さな植物の妖精たちが手を振っているのがわかる。\n　自分たちに気づいたことに喜んだ妖精たちは〈森呪遣い〉と〈召喚術師〉にささやかな祝福をしてくれた。\n　【因果力】1点を得る。\n",
        "だいなしピクニック：\n　林の中の広場でお弁当を広げて小休止。\n　モンスターの気配もなく、ちょっとしたピクニック気分……と思いきや、にわかに空がかき曇り、雷鳴とどろく大雨が降りだした！\n　キミたちはあわてて木陰に駆けこむ。\n　PCは全員1Dを振ること。\n　3以下のPCは一番高価な[消耗品]アイテム1つを失う。\n",
        "キャラバンクルーズ：\n　林の木陰で休憩している〈第８商店街〉の商隊と出会った。\n　ギルドマスターのカラシン(『LHZB1』P351)みたいだ。\n　PCのうち希望者は気さくな彼とコネクションを取得することができる。\n　関係は「友情」または「雇用関係」となる(PCが選択)。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "花畑にご用心：\n　花畑を渡る街道で〈暗殺蜜蜂〉の群れに出くわした。\n　追い払いながら縄張りから撤退する。\n　PCは全員[疲労:10]を受けるが、彼らが落とした「黒いローヤルゼリー[換金](70G)」1つを得る。\n",
        "おとりはまかせろ！：\n　キミたちは〈緑小鬼〉に襲われていた〈大地人〉の荷馬車を逃がすため、おとりと護衛に分かれて行動を開始した。\n　おとりをかって出るPCを1人、相談して決めること。\n　そのPCは[疲労:20]を受け【因果力】1点を失うが、他のPCたちと荷馬車は安全に撤退することができる。\n",
        "住処にお宝？：\n　〈フクロウ熊〉を撃退したキミたちは全員[疲労:10]を受ける。\n　もしかしたら近くに巣があるかもしれない。\n　代表者1名は「難易度:11」の[知覚判定]を行なう。\n　この判定は[偵察]タグがついているように扱う。\n　〔判定成功〕\n　　「壊れた武具[換金](90G)」を見つけた。\n　〔判定失敗〕\n　　何も得られない。\n　〔サブ職:狩人、辺境巡視〕\n　　判定に+2。\n",
        "触らぬ狼に祟りなし：\n　遠くに狼の群れを発見した。\n　まだこちらには気づいてないようだが、このまま進めば鉢合わせすることになるだろう。\n　避けるのならPCの代表者1名は「難易度:10」の[解析判定]を行なうこと。\n　〔判定成功〕\n　　遠回り成功。\n　　PCは全員[疲労:10]を受ける。\n　〔判定失敗〕\n　　気づかれてしまった！\n　　撃退したものの、PCは全員[疲労:25]を受ける。\n　〔サブ職:斥候、遊牧民〕\n　　判定に+2。\n",
        "ベリー摘み競争：\n　雑木林でベリーの茂みを見つけたキミたちは、誰が一番多く摘めるか競争を始めた。\n　PCは全員2Dを振ること。\n　一番低い出目のPCは、一番出目が高いPCにコネクションを取得する。\n　関係は「尊敬」か「ライバル」となる(PCが選択)。\n　このコネクションはアフタープレイで消滅するが、ログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "村の形見：\n　滅びた〈大地人〉の村が眼前に広がる。\n　せめてもの弔いに何かを持ち帰って役立てよう。\n　PCは【因果力】を任意の数だけ消費してもよい。\n　各自消費した【因果力】と同数のダイスを振り、全員の出目を合計すること。\n　3以上なら「赤熱の小爪[コア素材](50G)」1つを得る。\n　8以上なら加えてさらに「小さな宝石箱[換金](70G)」1つを得る。\n　13以上なら加えてさらに「ミニポーチ(『LHZB1』P217)」1つを得る。\n",
        "ぬかるみの強行軍：\n　折からの雨でひどく街道がぬかるんでいて、〈冒険者〉でも歩くには少々骨が折れる。\n　PCは全員「難易度:10」の[耐久判定]を行なう。\n　〔判定成功〕\n　　黙々と足を進め、無事目的地に到着した。\n　〔判定失敗〕\n　　疲れに加え少し足首をひねってしまったようだ。\n　　[疲労:20]を受ける。\n　〔サブ職:辺境巡視、配達屋〕\n　　判定に+2。\n",
        "ハニーハント：\n　雑木林の中で、巨大蜂の巣を見つけた。\n　たっぷりのハチミツを手に入れるチャンスだ！\n　挑戦するならば、PCの代表者1名は「難易度:10」の[操作判定]を行なう。\n　〔判定成功〕\n　　PCたちは「黄金色のハチミツ[換金](120G)」を得る。\n　　-2(累積する、最大-10)を受ければもう一度チャレンジできる。\n　〔判定失敗〕\n　　怒ったハチの群れに追いかけられPC全員は[疲労:10]を受ける。\n　〔サブ職:採取人、農家〕\n　　判定に+2。\n",
        "闇の群雲：\n　キミたちは〈闇精霊〉の「夜渡り」に遭遇する。\n　[暗視]状態、もしくは[魔法]プロップの影響を受けないPCは被害を受けない。\n　それ以外のPCは全員「難易度:10」の[抵抗判定]を行なう。\n　いまが夜なら(GM判断)「難易度:12」になる。\n　〔判定失敗〕\n　　【因果力】1点を失う。\n　〔サブ職:刻印呪師、魔法学者〕\n　　この判定に+2。\n",
        "風のいたずら：\n　金色に波打つ麦畑で、一際強い突風が吹いた。\n　PCは全員2Dを振ること。\n　一番低い出目のPCは装備や性別に関係なく、シナリオ終了まで[ぱんちら]タグが与えられる。\n　このタグを持っているPCが何らかの判定でファンブルした時、特別にタグを[ぱんもろ]に変化させることでその判定を振り直してもよい。\n",
        "古代の獣：\n　荒野を探索中、なかば地面にうずもれた巨大生物の骨格を見つけた。\n　キミたちはなにか素材が取れないか採取を試みる。\n　1Dを振ること(【因果力】1点で振り直してもよい)。\n　5か6が出たPCは「とがった爪[コア素材](60G)」もしくは「巨大な肋骨[換金](80G)」のどちらかを選んで得る。\n",
        "死者の窪地：\n　丘陵地帯の窪地は不浄な死者がむらがっていた。\n　風に揺れるのはすべて〈蠢く墓標〉だったのだ！\n　おびただしい数の〈動く骸骨〉が迫る中での撤退戦の末、キミたち全員は[疲労:20]を受ける。\n",
        "足跡は語る：\n　丘陵を行くキミたちはいくつもの足跡を発見した。\n　PCの代表者1名は[知識判定]を行なう。\n　〔判定成功〕\n　　足跡から〈醜豚鬼〉の小部隊の存在を看破したキミたちは奇襲を仕掛けることができる。\n　　仕掛ければ成功して120Gを入手する。\n　〔判定失敗もしくは奇襲しない〕\n　　足跡は意味不明で数が多すぎる。\n　　キミたちそっとその場を離れた。\n　〔サブ職:学者、追跡者〕\n　　判定に+2。\n",
        "キリングゾーン：\n　キミたちは悪辣な〈小牙竜鬼〉たちが仕掛けたトラップ地帯に入り込んでしまった。\n　罠を解除しつつ抜け出すしかないようだ。\n　PCの代表者1名は「難易度:10」の[解除判定]を行なう。\n　〔判定成功〕\n　　最低限の被害で脱出できた。\n　　PC全員は[疲労:10]を受ける。\n　〔判定失敗〕\n　　いくつかのトラップにはまりPC全員は[疲労:30]を受ける。\n　〔達成値:14〕\n　　罠を突破して逆襲してやった！\n　　100Gを得る。\n　〔サブ職:機工師、罠師〕\n　　判定に+2。\n",
        "果物天国：\n　通りかかった果樹園で〈大地人〉の農夫に果物の収穫を手伝ってほしいと頼まれた。\n　手伝うのならばPCは全員「難易度:10」の[解除判定]を行なう。\n　それぞれ難易度をいくつ上回ったかを記録し、その合計値が10以上ならば収穫は大成功だ。\n　喜んだ農夫は「かご一杯の果物(シェフの気まぐれピザ相当。『LHZB1』P218)」x2個をキミたちに持たせてくれる。\n",
        "岩陰からこんにちわ：\n　岩陰から不意打ちを仕掛けてきた〈蜷局竜〉をなんとか撃退。\n　キミたちは戦利品として「風鳴りの鈴[コア素材](80G)」を手に入れたが、少なくない被害を受けた。\n　PC全員は1～6の数字2つを宣言し、代表者1名は1Dを振ること。\n　出目と同じ数字を宣言したPCは[疲労:25]を受ける。\n",
        "野火から逃げろ！：\n　前方から焦げ臭い風が吹いてきた。\n　野火だ！\n　逃げようにも丘の合間を抜ける風が、炎を複雑に押し広げているようだ。\n　PCの代表者1名は「難易度:10」の[解析判定]を行なう。\n　〔判定成功〕\n　　風と地形を読み切り、PCは損害を免れた。\n　〔判定失敗〕\n　　何とか炎は免れたものの、思い切り煙を吸い込んでしまった。\n　　PCは全員[疲労:45]を受ける。\n　〔サブ職:生還者、星詠み〕\n　　判定に+2。\n",
        "穴があったら探検だ：\n　丘の中腹に小規模なダンジョンを発見した。\n　探索する場合は1日を消費してPC全員は1Dを振る。\n　1人でも6が出た場合、戦利品として「小さな宝箱[換金](60G)」を3個手に入れた。\n　ホクホクだ。\n　そうでない場合、隅々まで探索したが全員が[疲労:20]を受けたのみだった。\n　誰かが探ったあとみたい。\n　〔サブ職:蒐集家、設計士〕\n　　2Dを振ってもよい。\n",
        "一寸先は穴：\n　一面の田園地帯が広がっている。\n　景色に気をとられていたキミたちは、足元にポッカリ開いた堆肥の穴に気づかない！\n　PCは全員1Dを振ること。\n　出目が1～2ならば見事に穴にはまり込んでしまった！\n　しばらく臭いが取れそうにない。\n　被害者はコネクション1つ(被害者が選択)をして、シナリオ終了時まで関係を「くさい……」に変更する。\n",
        "木漏れ日と水辺：\n　森の中できれいな泉を見つけた。\n　希望者はここで水浴びをすることも可能だ。\n　旅の汚れを落として気分もリフレッシュ、爽快な気分で冒険を続けられそうだ。\n　探索表のイベントで何らかの判定を行なう場合、その判定に+1Dする。\n　この効果はPCたち全員で共有し一度効果を受けるかシーン終了時まで持続する。\n",
        "黒く光る宝物：\n　果樹園の木の上に巨大な鳥の巣を発見した。\n　巣の主が留守のうちに内部を調べてみると、散乱した羽毛や獣の骨の陰から「幻想の黒真珠[コア素材](100G)」を発見する。\n　いいのかな？\n　持っていくなら入手してもよい。\n",
        "猪突猛進：\n　起伏の激しい丘陵を進んでいると、前方から〈猛猪〉の群れが激しい地響きを伴って迫ってくる！\n　PC全員は「難易度:11」の[運動判定]を行なう。\n　一人でも失敗すれば全員が判定失敗として扱おう。\n　〔判定成功〕\n　　キミたちは一目散に逃げきった。\n　〔判定失敗〕\n　　逃げた先には、今度ははぐれ〈巨石兵士〉だ！？\n　　延々逃げ回ってPC全員はそれぞれ[疲労:1D*10]を受ける。\n　〔サブ職:軽業師、傭兵〕\n　　全員の判定に+1。\n",
        "へんてこ葡萄は蜜の味？：\n　日当たりのよい斜面に野性のブドウがたわわに実っている。\n　採取するPC全員は[知識判定]を行なう。\n　〔判定成功〕\n　　「貴腐ブドウ[換金](110G)」を手に入れる。\n　〔クリティカル〕\n　　さらに「常眠りの種子[コア素材](100G)」を得る。\n　〔サブ職:農家、採取人〕\n　　判定に+1D。\n",
        "ここはどこ？：\n　灌木の生い茂る丘陵地帯、視界は悪いし歩きにくいことこの上ない。\n　どっちに向かっているかすらおぼつかない。\n　[飛行]状態のPCが1人もいない場合、回り道を強いられPC全員は【因果力】1点失うか、1日の時間をロスする。\n",
        "狩りの邪魔：\n　澄んだ湧水を汲もうとしたキミたちに驚いて、立派な牡鹿が森の中に逃げていった。\n　すると近くの草むらから〈大地人〉の狩人がカンカンに怒って飛び出してきた。\n　どうやら彼の獲物を追い払ってしまったらしい。\n　PCは全員2Dを振ること。\n　一番低い出目のPCはこのNPCとコネクションを取得する。\n　関係は「恐縮」となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "激闘の報酬：\n　キミたちは廃墟に住み着いた〈暗蛇竜〉を退治した。\n　遺跡が崩落するほど激しい戦いだったが、首尾は上々だ。\n　PCは【因果力】を任意の数だけ消費してもよい。\n　各自消費した【因果力】と同数のダイスを振り、全員の出目を合計すること。\n　ダイスの合計値が3以上ならPCたちは「ねじくれた角[コア素材](120G)」1つを得る。\n　8以上でさらに300Gを得る。\n　13以上でさらに「暗竜牙【魔触媒7】(60G)」5つを得る。\n",
        "竜巻に追われて：\n　少しずつ風の強くなる中、キミたちが荒廃した高速道路を旅していると、背後から竜巻が追ってきていることに気付く。\n　勝てる相手じゃないから逃げださなければ！\n　PCは全員「難易度:12」の[運動判定]を行なう。\n　PCが[飛行]状態の場合は+2を得る。\n　〔判定成功〕\n　　PCは[疲労:20]を受ける。\n　　なんとか逃げ切れたようだ。\n　〔判定失敗〕\n　　PCは[疲労:50]を受け、【因果力】1点も失う。\n　〔サブ職:軽業師、生還者〕\n　　判定に+2。\n",
        "毒を浄化せよ：\n　キミたちは〈緑小鬼〉の集団が泉に毒を投げ込んでいるのを目撃する。\n　村を滅ぼそうとする陰謀か！？\n　戦うのならば[疲労:20]を受ける。\n　BSを解除できるアイテムか特技があるのならば、泉を清浄化しておこう……。\n",
        "させるか！：\n　道中、襲いかかってきた〈時計仕掛の蠍〉をキミたちは排除したはずだった。\n　とどめを刺したはずの一体が一瞬だけ息を吹き返し橙色の光線を放つ！\n　PCの代表者1名は[疲労:60]を受けなくてはならない。\n　その際に受ける[疲労]の強度を[軽減(光輝)]の強度ぶん減らしてもよい。\n",
        "ない、ない、あれがない！：\n　森を歩いているとふと荷物整理を思い出した。\n　PC全員は2Dを振ること。\n　一番低い出目のPCは忘れ物に気づいた。\n　何を忘れたかはキミが決定してよい。\n　慌てたキミは仲間に相談する。\n　キミが望むなら、仲間にコネクションを得てもよい。\n　関係は「相棒」か「恩人」となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "川底に眠る：\n　キミたちが川辺を歩いていると、川底で何かがキラリと光った。\n　なにかお宝があるかもしれない。\n　PCはここで半日過ごし[疲労:20]を受けるたびに1人1Dを振っても良い。\n　〔1D:1～2〕\n　　「水晶の砂[換金](20G)」を入手する。\n　〔1D:3～5〕\n　　「水晶の塊[換金](40G)」を入手する。\n　〔1D:6〕\n　　「蒼い鉱石[コア素材](140G)」を入手する。\n",
        "沼より出ずるもの：\n　のどかな沼沢地帯を歩いてたキミたちは、沼からはい出てきた〈破裂の粘体〉に囲まれてしまっていた。\n　この包囲網から逃れるには協力が必要だ。\n　全員1Dを振る(【因果力】1点消費でさらに+1Dしてもよい)。\n　全員のダイスの合計が13以上なら、無事突破できる。\n　12以下の場合PCは全員[疲労:40]を受ける。\n",
        "山の恵み：\n　豊かな沢の周りで山の幸を見つけよう！\n　PCは全員「難易度:12」の[知識判定]を行なう。\n　〔判定成功〕\n　　「大きな川魚(トポト親方のコロッケ相当。『LHZB1』P218)」を得る。\n　〔判定失敗〕\n　　「食べられるかもしれない水草[換金](1G)」2個を得る。\n　〔サブ職:料理人、食闘士〕\n　　判定に+2。\n",
        "幻の先に：\n　のどかな沼沢地を進んでいると、突如毒の沼にはまり込んでしまった。\n　〈幻霊〉の幻覚による誘い込みの罠だったらしい！\n　強度10以上の[軽減(邪毒もしくは精神)]を持たないPCは[疲労:50]を受ける。\n",
        "オークと一緒：\n　キミたちが崩れかけた高速道路を歩いていると、向こうから両手に骨付き肉を持った〈醜豚鬼〉の一団が踊りながら迫ってきた！\n　フレッシュミートゥ！\n　PCは全員2Dを振ること。\n　一番低い出目のPCの足元が崩れて〈醜豚鬼〉もろとも転落、そのPCには[尊い犠牲]タグが与えられる。\n　このタグはシナリオ終了まで解除されない。\n",
        "物言わぬ番人：\n　森を横切る小川のほとりに、苔むした瑠璃色の機械人形が壊れてうずくまっている。\n　この機械人形の名前はなんだろう？\n　「難易度:14」の[知識判定]に成功し、1人でもかすれた文字が読めるなら「七色の透明捻子[コア素材](180G)」1つを得る。\n",
        "鉄砲水！：\n　谷間で突然の大雨に見舞われる。\n　キミたちは鉄砲水の予兆に気付くだろうか？\n　PCの代表者1名は「難易度:14」の[知覚判定]を行なう。\n　〔判定成功〕\n　　予兆を察知して早々に避難したキミたちの足元を濁流が駆け抜けてゆく。\n　　危ないところだった。\n　〔判定失敗〕\n　　地響きが聞こえてきたときはもう手遅れだった。\n　　PCは全員[疲労:50]を受ける。\n　〔サブ職:探検家、生還者〕\n　　判定に+2。\n",
        "廃橋の戦い：\n　高速道路の廃墟を進んでいると、〈鋼尾翼竜〉の群れに襲われた！\n　キミたちは勝利を納め、265Gを得るが、不安定な足場のため[疲労:30]を受ける。\n　[飛行]状態や[天然]もしくは[地形]プロップへの耐性があるPCは疲労は受けない。\n",
        "転落注意：\n　谷あいの道を歩いていると、突然足元が崩れ始めた！\n　[飛行]状態でない、最も【行動力】が低いPCは崩落に巻き込まれて斜面を滑り落ちてしまう。\n　ダメージは大したことなかったけど、ついてない……。\n　滑落したPCは【因果力】1点を失うこと。\n",
        "今夜はごちそう？：\n　葦の茂る沼沢地で〈大地人〉の兄妹が罠を仕掛けている。\n　鵞鳥を夕飯にしたいらしい。\n　1日かけて猟を手伝う場合、GMは兄弟にかわって2Dを振ること(サブ職が狩人、罠師のPCがいれば+1Dだ)。\n　8以上ならば、狩りは成功。\n　兄妹の家で料理を食べてPCたちは[疲労]をすべて回復する。\n　7以下ならば腹ペコだ。\n　兄妹の家で「タイミング:レストタイム」の行動が可能。\n",
        "獅子の魔獣：\n　キミたちは手負いの〈人面獅子〉を廃墟の住処まで追いつめ、ついに討ち取った。\n　この偉業でPCは全員【因果力】1点を得る。\n　またPCの代表者1名は「難易度:15」の[知覚判定]を行なう。\n　判定に成功したなら、〈人面獅子〉の住処から「金色の毛玉[コア素材](220G)」を得る。\n　〔サブ職:狩人、追跡者〕\n　　判定に+2。\n",
        "森の奇襲：\n　キミたちが木漏れ日にきらめくせせらぎで旅の疲れを癒していたところに〈醜豚鬼の遊撃兵〉が奇襲をしかけてきた！\n　とっさに武器を取りなんとか〈醜豚鬼の遊撃兵〉を蹴散らしたが、損害は決して小さくはなかった。\n　PCは全員[疲労:40]を受ける。\n",
        "お肉入手のチャンス：\n　キミたちは水飲み場にやってきた。\n　足跡からすればここには動物が来そうな予感。\n　食料ゲットのチャンスだ！\n　半日のあいだ隠れて狙撃をするのならば狩りが可能だ。\n　[射撃攻撃]可能な武器を持つ代表者1名は「難易度:15」の[命中判定]を行なう。\n　〔判定成功〕\n　　PCたちは大きなイノシシを倒した。\n　　「魅惑のボタン肉(定番カレー相当。『LHZB1』P218)3個を得る。\n　〔判定失敗〕\n　　何も得られない。\n　〔サブ職:狩人、砲撃士〕\n　　判定に+2。\n",
        "迷惑な怪魚：\n　大河を渡っているキミたちの後ろから巨大なモンスターが襲い掛かる。\n　ボートは転覆してしまうし、泳いで逃げるしかない。\n　[水棲]タグを持たないPCは慣れない水中戦闘に巻き込まれて[疲労:60]を受ける。\n",
        "曙光：\n　山の稜線から差し込む朝日が、草の葉に溜まった朝露に反射する。\n　小鳥たちはさえずり、冷え込んだ夜の空気が追い払われていくのがわかる。\n　今朝のごはん当番は誰だっけ？\n　立候補したPC1名(メニューを発表すること！)には【因果力】1点が与えられる。\n　さあ、今日も一日頑張ろう！\n",
        "吸血の森：\n　〈吸血ヒル〉の群生地である深い森の中を探索するキミたち。\n　PCは全員「難易度:13」の[耐久判定]を行なう。\n　成功した人数がパーティーの3名以上いれば、ヒルを振り払いながらキミたちは探索をなしとげる。\n　「錆びた聖印[コア素材](240G)」と2D*60Gを得る。\n　そうでなければキミたちは探索もそこそこに退却し、全員[疲労:20]を受ける。\n",
        "落雷注意：\n　キミたちの頭上で雷雲が不気味に湧き上がり、雷鳴もゴロゴロと鳴り響いている。\n　そして不幸なことに、さえぎる物のない高原において雷はキミたちに狙いを定めたようだ！\n　[高位保護]を持たないPCは[疲労:80]を受ける。\n　〔サブ職:探検家、辺境巡視〕\n　　[疲労:80]ではなく[疲労:60]で済む。\n",
        "ギブアンドテイク：\n　街道沿いに残る廃墟でキャンプする〈冒険者〉グループと遭遇した。\n　どうやら彼らには回復職のメンバーが欠けているようだ。\n　〈回復職〉PCが治療を申し出ると、彼らは治療のお礼に「定番カレー(『LHZB1』P218)」1個を分けてくれる。\n　もし特技喪失を受けるならさらに1個入手してよい。\n",
        "悪魔の一族1：\n　大きな切り株に腰かけて一休み。\n　降り注ぐ木漏れ日が気持ちいい……と思ってたらカバンがない！？\n　少し離れたところでクロマミ族の連中がカバンをひっくり返してる！\n　PCは全員《異常探知》をすること。\n　最も達成値の低いPCは[消耗品]アイテムを2個失う。\n　〔ファンブル〕\n　　さらに[圧迫:2]を受ける。\n　〔サブ職:交易商人、配達屋〕\n　　判定に+2。\n",
        "旅は道連れ：\n　キミたちは森の入り口で立ち往生している。\n　ヤーマという行商人の馬車を発見した。\n　護衛がいないため森を通り抜けるのが心配らしい。\n　もし一日の護衛を申し出るならヤーマからは大変感謝され、謝礼として300G渡される。\n　また、PCが望むならばヤーマとのコネクションを取得してもよい。\n　関係は「取引」となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "勝負好きの〈妖精巨人〉：\n　高原の環状列石のそばを通りかかったキミたちに、〈妖精巨人〉が力比べを挑んできた。\n　勝負を受けるなら、PCの代表者が「難易度:14」の[耐久判定]、[知覚判定]、[知識判定]を1回ずつ行なう(それぞれの判定は別のPCが行なってもよい)。\n　2回以上成功すれば勝負に勝ち、「神鉄の組紐[コア素材](300G)」を入手する。\n　3回成功できたならさらにPC全員が【因果力】1点を入手する。\n　3回とも失敗した場合はPC全員が[疲労:35]を受ける。\n",
        "不気味に響く音：\n　薄暗い森で〈魔狂蝙蝠〉の襲撃を受けた。\n　撃退はしたが強力な超音波攻撃で頭がガンガンする……。\n　PCは全員「難易度:16」の[耐久判定]を行なう。\n　〔判定成功〕\n　　【因果力】1点を失う。\n　〔判定失敗〕\n　　気分が悪くなってきた。\n　　【因果力】2点を失う。\n　〔自身:高位保護〕\n　　【因果力】を失わなくてよい。\n",
        "手負いの獣にご用心：\n　森の小道を往くキミたちに、木々をへし折る重い音と地響きが近づいてくる。\n　なんと、手負いの〈魔熊〉が半狂乱となって飛び込んできた！\n　キミたちはなんとか〈魔熊〉の奇襲を撃退し、「傷ついた毛皮[換金](350G)」を手に入れたが被害も大きい。\n　「戦士職」もしくは「武器攻撃職」のPCは[疲労:45]を得る。\n",
        "闇の連絡通路：\n　キミたちは遺跡と化したパーキングエリアを繋ぐトンネルを前にしている。\n　ここを通れば随分と近道ができるが、トンネルは闇に閉ざされ中をうかがい知ることができない。\n　キミたちはこのトンネルに向かってもよいし、迂回してもよい。\n　トンネルを通るならば[暗視]を持たないPC全員は特技喪失を受ける。\n　迂回するならばPC全員は[疲労:20]を受けて、もう一度この表を振らなければならない。\n",
        "星月夜にて：\n　高原を旅するキミたちは満天の星の下で野営を行なう。\n　1Dを振って一番高い出目のPCが野営当番だ。\n　任意のPCひとりを見張りの相棒に指名し、今回の旅について話し合ってみよう(実時間で5分程度)。\n　あなたと相棒はお互いにコネクションを取るか、「関係変化表(『LHZB2』P59)」で1回ロールしてもよい。\n",
        "最後の御奉公：\n　多数のメイド型ゴーレムが眠るかつてのパーキングエリアを思わせる遺跡で、キミたちはかろうじて機能を保っていた一体を発見した。\n　彼女はキミたちにきしむ腕で「刻印の銀板[コア素材](340G)」を託すと、静かにその活動を停止した。\n",
        "冷たい霧に抱かれて：\n　灌木の生い茂る斜面を登っていると、急に濃霧がたちこめてきた。\n　霧はキミたちの体温を急速に奪っていく。\n　強度20以上の[軽減(冷気)]を持たないPCは[疲労:40]を受ける。\n",
        "鹿肉パーティーだ！：\n　早朝の高原で、〈剣角鹿〉の大きな群れを発見した。\n　半日かけて狩りを行なうならば「新鮮な鹿肉[換金]45G」を2D+5個入手できる。\n　〔サブ職:料理人〕\n　　入手した「新鮮な鹿肉」を1個消費するごとにPC1人の[疲労]を-20してもよい。\n",
        "手痛い報復：\n　清々しい風が吹き抜ける林を歩いていると、うっかり〈歩行毒樹〉の根を踏んで反撃を食らってしまった。\n　大したダメージは受けなかったけど、傷口がひどく腫れてとにかく痛い！\n　PCは全員2Dを振ること。\n　一番低い出目のPCは【因果力】1点を失う。\n",
        "かぐわしい？におい：\n　キミたちは森でとある食虫植物の群生地に飛び込んでしまった。\n　大したダメージこそ受けなかったが、虫を引き寄せるべとべとが装備にこびりついて離れない。\n　これは困った！\n　PCは全員2Dを振ること。\n　一番低い出目のPCは[べとべと]タグが与えられる。\n　このタグはシナリオ終了まで解除されない。\n",
        "誰も知らない花園：\n　険しい斜面を登り切った先には、一面の花畑が広がっていた。\n　その鮮やかさにキミたちは見ほれてしまう。\n　PCは全員1Dを振ること。\n　〔1D:1～4〕\n　　「きれいな花[換金](5G)」を好きなだけ摘んでもよい。\n　〔1D:5～6〕\n　　「うごめく花弁[コア素材](380G)」を手に入れる。\n",
        "夜明け前の攻防：\n　夜明け前の薄暗がりから〈怨霊〉の大群が襲いかかってきた。\n　夜明けまで持ちこたえねば！\n　「回復職」と「魔法攻撃職」のPCは激戦のあまり【因果力】2点を失う。\n",
        "何が出るかな……？：\n　瓦礫に半ばうずもれた遺跡でアルヴのものと思われる機械を発見した。\n　コインを入れるとランダムに品物が出てくるらしい。\n　挑戦したいPCは100Gを消費して、1Dを振ること。\n　なお、この機械は合計4回挑戦したところで完全に壊れてしまう。\n　〔1D:1〕\n　　ハズレ！\n　　何も出てこないぞ。\n　〔1D:2〕\n　　「鉄身の軟膏(初級)(『LHZB1』P219)」を入手。\n　〔1D:3～4〕\n　　「回復薬(中級)(『LHZB2』P146)」を入手。\n　〔1D:5〕\n　　「幻翼の宝珠(中級)(『LHZB2』P159)」を入手。\n　〔1D:6〕\n　　「保護の巻物(中級)(『LHZB2』P156)」を入手。\n",
        "茨の森：\n　キミたちは毒の棘を持つ灌木が生い茂る森に迷い込んだ。\n　茎や葉の棘が露出した肌を容赦なく引っ掻いていく。\n　[中鎧]を装備したPCは[疲労:30]を受ける。\n　[軽鎧]もしくは鎧を装備していないPCは[疲労:30]と特技喪失を受ける。\n",
        "朝まで歌合戦：\n　浜辺を行くキミたちの耳にきれいな歌声が聞こえてきた。\n　歌声の主を探すと、なんと岩場で〈人魚〉と〈翼持つ歌姫〉が歌の勝負をしている！？\n　ギャラリーに気づいた彼女たちはさらに熱心に歌い始めた。\n　キミたちはそそくさと立ち去ってもいいし、彼女らが満足するまで歌を聴いていてもいい。\n　そうした場合(一応美少女？の)祝福で【因果力】1点を得るが、[疲労:20]も受けてしまう。\n",
        "ロードランナー：\n　狭い山道の向こうから、すごい勢いで〈時計仕掛の駝鳥〉が駆けてくる！\n　PC代表者は「難易度:17」を目標に[回避判定]をしてもよい(そのまま受け止めてもよい)。\n　失敗した(受け止めた)場合3D*20点の物理ダメージを受けてしまうが、キミが[戦闘不能]にならなければ〈時計仕掛の駝鳥〉も砕けて「真鋼のワイヤー[コア素材](440G)」を得る。\n　回避に成功するか[戦闘不能]になった場合、〈時計仕掛の駝鳥〉は走り去る。\n",
        "森を舐める猛火：\n　山道を歩いていると、風に乗って焦げ臭いにおいが漂ってきた。\n　風上を見れば真っ赤な炎の舌が斜面を駆け上ってくるではないか！\n　[高位保護]を持たないPCは炎と煙に巻かれ[疲労:80]を受ける。\n",
        "岩石林に咲くバラ：\n　キミたちは石の花が咲くという、石化ガスのたなびく石柱の林を訪れた。\n　探検するなら[高位保護]を持つPCにかぎり【因果力】を任意の数だけ消費してもよい。\n　各自消費した【因果力】と同数のダイスを振り、全員の出目を合計すること。\n　ダイスの合計値が3以上ならPCたちは「雲母のバラ[換金](480G)」1つを得る。\n　8以上でさらに「雲母のバラ[換金](480G)」3つを得る。\n　13以上でさらに「紅玉のバラ[換金](2000G)」1つを得る。\n",
        "迫りくる海蛇竜：\n　浜辺で漁師たちが逃げまどっている。\n　〈海蛇竜〉が波打ち際で噛みつこうとしているのだ。\n　救助するならヘイトを集めてかばうしかない。\n　「戦士職」のPCは【因果力】2点を失う。\n",
        "浜辺で小休止：\n　周囲の砂浜には椰子の木がたくさん生えている。\n　木陰に入って椰子の実ジュースを飲んでいると、なんだかちょっぴりリゾート気分だ。\n　十分に休憩したら、冒険を再開しよう！\n　この探索表の出来事で次に何らかの判定をする場合ロールに+1D。\n　この効果はひとりでも全員でも次の判定のみ、一度効果を受けるかシーン終了時まで持続する。\n",
        "わんぱくイルカと〈冒険者〉：\n　目の前に広がる海で、何匹ものイルカが水面を蹴立てて遊んでいる。\n　キミたちに気づくと岸辺まで近づいてきた。\n　人懐っこい彼らはどうやらキミたちに遊んでほしいらしい。\n　もしも一緒に遊んでやるならPC全員は[疲労:40]を受けるが、遊んでくれたお礼にイルカたちが背中に乗せて送って行ってくれる。\n　次の探索表ロールを1回振りなおしてもよい。\n　〔自身:水棲〕\n　　受ける[疲労]の強度は20になる。\n",
        "夢のリゾート計画：\n　海岸線を見渡せる崖の上に、打ち捨てられた白いコテージを発見した。\n　風雨にさらされ傷んでいるが、修理すればまだまだ使えそうだ。\n　機会があったらキミたちの別荘にしてみるのもよいかもしれない。\n　キミたちはこの建物の場所を忘れないよう、しっかりと地図に書き込んだ。\n",
        "助けをよぶ声：\n　深い森の中を歩いていると、どこからともなく助けを呼ぶささやき声が聞こえてきた。\n　PC全員は[知覚判定]を行なうこと。\n　最も達成値の高いPCは、石の下から芽吹いたばかりの小さな双葉と、生まれたばかりの〈樹の精霊〉を発見する。\n　キミが石をどけてやると精霊は大いに感謝し、キミの[疲労]の強度を-60してくれる。\n　〔クリティカル〕\n　　精霊はさらに「芽吹きの甘露(苺のふんわりカップケーキ(『LHZB2』P145)相当)」をプレゼントしてくれた。\n　〔サブ職:農家、薔薇園の姫君〕\n　　判定を2回振って好きなほうを選べる。\n",
        "草むらかき分けて：\n　キミたちが進む深い森の中は、背丈ほどもありそうな下生えが生い茂っている。\n　徒歩では視界も通らず、草を刈り、道を切り開きながらじりじりと進むしかない……。\n　PCは全員[疲労:60]を受ける。\n　〔自身:騎乗、飛行〕\n　　[疲労]を受けなくてもよい。\n",
        "もうかりまっか？：\n　海沿いの道で〈大地人〉の行商人と出会った。\n　せっかくだから何か買って、顔を繋いでおくのもいいかもしれない。\n　PCのうち希望者はログチケット:アザーゲット1枚を消費してこの行商人とコネクションを取得してもよい。\n　関係は「ビジネス」か「敬意」となる(PCが選択)。\n　またコネクションとは無関係に一般的なアイテムを売買できる。\n",
        "世捨て人の工房：\n　寂れた高原を旅するキミたちは古い庵を発見する。\n　中は無人だったが、なんと小さな魔法工房がしつらえられていた！\n　【因果力】を1点消費したPCは自身が取得している《ファーマシスト》、《スクライブスクロール》、《インビュー》のうち1つを即座に実行してもよい。\n",
        "電光石火！？：\n　山の天気は不安定だ。\n　先ほどまでの晴天が嘘のように激しい雷雨が吹き荒れている。\n　うわっ、すぐ近くに落ちた！？\n　PCは全員「難易度:15」の[回避判定]を行なう。\n　強度20以上の[軽減(電撃)]を持つPCはこの判定に自動成功してよい。\n　〔判定成功〕\n　　回避成功！\n　　危うく直撃するところだった。\n　〔判定失敗〕\n　　避ける間もなく雷に打たれてしまい[疲労:100]を受ける。\n",
        "砂の中の金貨：\n　キミたちは苦戦の末、砂浜で遭遇した〈巨大蟻地獄〉を退治した。\n　早速巣穴の中を探索だ！\n　PCの代表者1名は「難易度:18」の[知覚判定]を行なう。\n　この判定は[偵察]タグがついているかのように扱う。\n　〔判定成功〕\n　　キミたちは2D*120Gを手に入れた。\n　　代表者のサブ職が〈採掘師〉か〈賭博師〉ならば、2Dの出目に+1すること。\n　〔判定失敗〕\n　　何も得られない。\n",
        "ローリングストーン：\n　針葉樹に覆われた高山の道を歩くキミたちの耳に「カラカラ」と不吉な音がする。\n　視線を上げれば落石に気が付く！\n　全員1Dを振ること(【因果力】1点で+1Dしてもよい)。\n　〔1D:1～3〕\n　　大きな石がキミに直撃！\n　　死ぬほど痛い上に【因果力】2点を失う。\n　〔1D:4～〕\n　　危うく難を逃れた。\n",
        "イースタルの夕日：\n　真っ赤な夕日が金色に輝く波間に溶けていく。\n　潮騒だけが響く中、この世界でも変わらぬ姿を見せる夕日の中でキミたちは話し合う。\n　PCは全員2Dを振ること。\n　一番低い出目のPCは、一番高い出目のPCにコネクションを取得する。\n　関係は「親愛」か「友情」となる(PCが選択)。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "大きな樹の迷宮：\n　大森林の中央にそびえる巨大樹の根元に薄暗い穴を発見。\n　探るのならば、代表者1名が「難易度:16」の[知覚判定]を行なう。\n　〔判定成功〕\n　　複雑に絡み合った通路の奥で「賢者の石[コア素材](620G)」と2D*100Gを得る。\n　〔判定失敗〕\n　　迷ってしまった！\n　　1日経過してしまう上に[疲労:50]を受ける。\n　〔サブ職:探検家、採掘師〕\n　　判定に+2。\n",
        "前門の鬼、後門の竜：\n　深い山懐に抱かれた廃神殿には〈陽炎悪鬼〉の大群が住み着いていた。\n　多勢に無勢、キミたちは速やかに撤退を開始する。\n　PCの代表者1名は「難易度:18」の[運動判定]を行なう。\n　この判定は[偵察]タグがついているかのように扱う。\n　〔判定成功〕\n　　追跡を振り切る。\n　　損害はない。\n　〔判定失敗〕\n　　〈陽炎悪鬼〉を振り切ったと思ったら、前から〈翡翠竜〉が！\n　　PCは全員[疲労:60]を受け【因果力】1点を失う。\n　〔サブ職:生還者、追跡者〕\n　　判定に+2。\n",
        "古代の工房：\n　山奥で古代の工房を発見した。\n　【因果力】1点を消費したPCは、自身が取得している《クロースメイク》、《ウッドクラフト》、《アーマーフォージ》、《ファーマシスト》を即座に1回実行してもよい。\n　アルヴの工房は性能抜群だ。\n",
        "揺れるつり橋：\n　目の前には霧深い谷と、朽ちて落ちかけのつり橋が揺れている。\n　どうにか向こう岸に渡らねばならない。\n　PCは全員「難易度:18」の[運動判定]を行なう。\n　〔判定成功〕\n　　無事につり橋を渡りきることができた。\n　〔判定失敗〕\n　　足を滑らせてしまった！\n　　落下こそ免れたが、渡りきるのにひどく苦労してしまった。\n　　[疲労:100]を受ける。\n　〔自身:飛行〕\n　　判定は自動成功。\n　〔サブ職:軽業師、船乗り〕\n　　判定に+2。\n",
        "真っ白なキミ：\n　キミたちは深い雪山を進む。\n　周囲の木々に降り積もった雪が、風にあおられ頭上から降り注いできた！\n　PCは全員2Dを振ること。\n　一番低い出目のPCは不運にも頭から雪まみれになってしまい、[雪だるマン]タグが与えられる。\n　このタグはシナリオ終了まで解除されない。\n",
        "打ち捨てられた砦：\n　風化した砦の廃墟をキミたちは発見した。\n　内部を探索したい場合1日を必要とする。\n　探索した場合不思議な魔方陣を発見し、PC全員の[疲労]の強度が-50される。\n　また、「夜明け色の軟玉[コア素材](680G)」を得る。\n",
        "白き魔物：\n　天候が急変し、ブリザードのような雪と風が吹き荒れる。\n　身体は凍りつきそうに冷え、視界は真っ白で何も見えない。\n　このままでは身動きが取れなくなってしまう。\n　強度20以上の[軽減(冷気)]と[暗視]の両方を持たないPCは[疲労:100]を受ける。\n",
        "強襲の雪獣：\n　〈雪トロール〉の群れが襲撃してきた！\n　PCの代表者1名は「難易度:18」の[知識判定]を行なう。\n　〔判定成功〕\n　　キミの指示で敵の弱点を突き、危なげなく勝利を収めた！\n　　【因果力】3点を得る。\n　〔判定失敗〕\n　　敵の情報がつかめず、苦戦を強いられた。\n　　PCは全員[疲労:60]を受ける。\n　〔サブ職:学者、軍師〕\n　　判定に+2。\n",
        "久しぶりのお客様：\n　野営場所を探すキミたちは、廃屋の前で〈家事妖精〉がぽつりと佇んでいるのを見つけた。\n　キミたちがここで一夜を過ごすことにするなら、彼女は久しぶりに訪れたお客様を精一杯もてなしてくれる。\n　この時キミたちは[食料]アイテムを提供してもよい。\n　翌朝には彼女の姿は消えているが、[食料]アイテムを提供したPCの手にはいつの間にかキラキラ光る小石が握られている。\n　【因果力】1点を得ること。\n",
        "〈大地人〉の特訓：\n　夜になって一夜の宿と思い立ち寄った〈大地人〉の小さな砦でキミたちは手厚い歓待を受けた。\n　キミたちは、出発を1日遅らせて兵士たちの訓練を手伝ってもよい。\n　訓練を買って出るPCは、特技喪失する代わりに【因果力】3点を得る。\n　とても厳しい訓練を立派にやり遂げた兵士たちの歓声を背に、キミたちは再び旅路につく。\n",
        "鳥妖の巣窟：\n　廃ビルに巣くう〈女面妖鳥〉がキミたちに対して集中攻撃を仕掛けてきたが、果敢に戦い見事に撃破。\n　戦利品として「人面鳥の羽根[コア素材](740G)」を手に入れた。\n　PC全員は1～6の好きな数字を申告すること。\n　代表者1名が1Dを振って、申告と同じ数字が出たPCは、戦闘のダメージで[疲労:80]を得る。\n　それ以外のPCは【因果力】1点を得る。\n",
        "野営地の攻防：\n　野営地に押し寄せたアンデッド〈反魂悪党〉が思いのほか手ごわかった。\n　奴らは闇に紛れて不意打ちを仕掛けてきたのだ。\n　PCは全員1Dを振ること。\n　〔1D:1～4〕\n　　PCは[疲労:100]を受ける。\n　〔1D:5以上〕\n　　PCは[疲労:35]を受ける。\n　〔自身:暗視〕\n　　出目に+2。\n",
        "今夜は天ぷらだ：\n　幅5mほどの名も知れぬ川をみつけた。\n　通り過ぎてもいいのだが、釣りをしてもよい。\n　釣りをする場合半日の時間を失うが「若葉アユ[換金](100G)」を1D匹釣り上げる。\n",
        "吹雪の行軍：\n　山の天気は変わりやすい。\n　さっきまで青空が見えていたのに、今や地吹雪で足元すら見えないありさまだ。\n　[高位保護]を持たないPCは[疲労:100]を受ける。\n",
        "ムーンライトフィーバー：\n　キミたちは道中で打ち棄てられた廃城で一晩を明かす。\n　ふと気が付くと月明りの下で輪になって妖精たちが踊っているようだ。\n　つられて踊りの輪に加わる場合、参加者は「難易度:16」の[運動判定]もしくは[操作判定]を行なう。\n　参加者に〈吟遊詩人〉がいるならば全員の判定に+2。\n　〔判定成功〕\n　　妖精たちはキミに大喝采と「回復薬(上級)(『LHZB2』P146)」1つを贈る。\n　〔判定失敗〕\n　　妖精たちは納得がいかない様子だ。\n　　キミは夜が明けるまで踊り、演奏を続けさせられた。\n　　[疲労:50]を受ける。\n",
        "幽霊屋敷へようこそ！：\n　一夜の屋根にと求めた山中の古い屋敷はホラームード。\n　全員2Dを振る。\n　一番出目が小さかった人は夜中にトイレに行きたくなり、仕方なく暗闇の中へ……。\n　残ったPCの中で希望者は、トイレに行ったPCを驚かしてもよい。\n　その場合トイレに行ったPCから関係「仇敵」のコネクションを得る。\n　驚かさずに付き添うのならば「関係変化表(『LHZB2』P59)」で1回ロールしてもよい。\n",
        "突然の雪崩！：\n　キミたちが穏やかな天候の下、雪深い山道を進んでいたその時、地鳴りと共に突然の大雪崩が襲いかかってきた！\n　PCの代表者1名は雪崩の方向を見極めるため「難易度:16」の[解析判定]を行なう。\n　〔判定成功〕\n　　[飛行]状態のPCは無事に逃げおおせたが、それ以外のPCは[疲労:30]を受ける。\n　〔判定失敗〕\n　　PC全員は[疲労:70]を受け特技喪失する。\n　〔サブ職:生還者、辺境巡視〕\n　　判定に+2。\n",
        "ノーストリリアの遺産：\n　街道沿いに遺跡らしい地下への入り口を発見する。\n　内部を探索する場合は半日を消費する。\n　探索した場合、通路の奥にはアルヴとの戦争当時のものらしき回復の魔方陣が残っていた。\n　〈猫人族〉、〈狼牙族〉、〈狐尾族〉、〈法儀族〉のPCは、魔方陣の効果で[軽減(光輝、邪毒、精神):30]を得る。\n　この軽減はOSとして扱いシナリオ終了時まで持続する。\n",
        "雪原の罠：\n　雪原を進むキミたちの足元が突如崩れ始める。\n　クレバスだ！？\n　最も【行動力】が低いPCが不運にも逃げ遅れ、クレバスの底に飲み込まれかける。\n　クレバスに落ちて[消耗品]タグを持つアイテムすべてを失うか、踏みとどまり【因果力】2点を失うか選ぶこと。\n",
        "珍味、食べる？：\n　キミたちは旅の途中で立ち寄った城砦で、任期を終えた兵士たちの宴にでくわした。\n　そこでひときわ異彩を放っていたのが……魚の塩漬けだ！\n　とにかく臭い！\n　だが彼らはとても美味しそうに平らげている。\n　勇気を出して口にしたPCは、[疲労]の強度が-100される代わりに、セッション終了時まで[超臭い]タグを得る。\n",
        "かつての星の里：\n　街道から少し離れた山腹に、キミたちは天文台の廃墟を発見した。\n　建物はかなり傷んではいるが、きちんと手を入れてやれば十分住むことはできそうだ。\n　ひょっとしたら、再び星を見ることができるかも知れない！\n　しかし残念ながら、今ここでできることは一夜の雨露をしのぐことぐらいだろう。\n",
        "追いすがる吹雪：\n　深い山中で〈氷雪巨人〉に遭遇してしまい、一時間近くも追い回されることになった。\n　吹雪のブレスがキミたちを何度も襲う。\n　強度30以上の[軽減(冷気)]を持たないPCは[疲労:80]と特技喪失を受ける。\n",
        "鎧袖一触！：\n　山道で〈大地人〉の隊商を襲う〈灰斑犬鬼〉の群れを発見した。\n　数こそ多かったが歴戦の〈冒険者〉であるキミたちの敵ではない！\n　残らず退治してやると、隊商の商人たちはたいそう喜んでいた。\n　900Gの謝礼をもらった。\n",
        "止まったら死ぬ！？：\n　周囲が突然の地響きに包まれ、斜面の上に積もった雪が崩落を起こした。\n　雪崩だ！\n　難を逃れるため、PCは全員「難易度:17」の[運動判定]を行なう。\n　〔判定失敗〕\n　　逃げ遅れたキミはあっという間に雪の奔流に飲み込まれてしまった。\n　　抜け出すために[疲労:120]を受ける。\n　〔自身:騎乗〕\n　　判定に+1Dする。\n　〔サブ職:探検家、辺境巡視〕\n　　判定に+2。\n",
        "〈冒険者〉の川流れ：\n　キミたちは目の前に広がる氷混じりの大河を渡る。\n　PCは全員2Dを振ること。\n　一番低い出目のPCは渡河の最中に足を滑らせ流されてしまうが、間一髪で他のPC(流されたPCが指定してよい)に救われる。\n　流されたPCは助けてくれた相手に対するコネクションを取得すること。\n　関係は「恩」か「友情」(助けたほうが選んでよい)となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "サルベージ：\n　川底に何か光るものが見えた気がする。\n　PCの代表者1名は「難易度:17」の[知覚判定]を行なう。\n　〔判定成功〕\n　　川底の砂に半ば埋もれた小さな宝石箱を発見！\n　　「ブリザードダイヤ[コア素材](980G)」と「色とりどりの宝石[換金](300G)」x5を得る。\n　〔判定失敗〕\n　　何も見つけられなかった。\n　〔サブ職:海賊、漁師〕\n　　判定に+2。\n",
        "ガスが濃くなってきたな：\n　渓谷を進んでいると周囲に毒々しい煙が立ち込めてきた。\n　これは……火山性の毒ガスだ！？\n　強度30以上の[軽減(邪毒)]を持たないPCは[疲労:100]を受ける。\n　〔サブ職:薬師、毒使い〕\n　　素早くガスの正体を見抜き、被害を受けなかった。\n",
        "樹海の白狐：\n　樹海の奥深くを歩いていると突然狐に似た精霊が現れた。\n　面白い話をするか、[食料]アイテムを与えると、代わりに「エメラルドの首飾り[換金](500G)」1D個をプレゼントされる。\n　〔サブ職:ちんどん屋、料理人〕\n　　さらに「金のあぶらげ[換金](1000G)」をプレゼントされる。\n",
        "吊るされた〈冒険者〉：\n　薄暗い密林でキミたちは足元に違和感を覚える。\n　しかし時すでに遅く、うごめくツタによって逆さ吊りにされ、所持品をあたりにぶちまけてしまった！\n　PC全員は[運動判定]を行なうこと。\n　この時[暗視]状態でないPCは、この判定に-1Dされる。\n　最も達成値の低いPCは[消耗品]アイテムを2個失い、[圧迫:1]を受ける。\n　〔ファンブル〕\n　　さらに[圧迫:1]を受ける。\n　〔サブ職:斥候、罠師〕\n　　この判定に+1D。\n",
        "儀式の生贄：\n　樹海を進むキミたちは〈蜥蜴人〉の集落に出くわし、運悪く生贄にされかけた〈狐耳族〉の少女を救出する。\n　信心深い少女はPCの代表者(PCが相談して選択)を「イキガミさま」と崇拝する。\n　代表PCは[神さま]タグを得る。\n　このタグはシナリオ終了まで解除されない。\n",
        "大自然のおこぼれ：\n　キミたちは密林で〈魔熊〉が〈地獄蜂〉の巣を襲い、ゆうゆうと食事をしているところに出くわした。\n　姿を隠すためにPCは全員「難易度:21」の[運動判定]を行なう。\n　〔全員が判定成功〕\n　　「地獄蜂の毒針[コア素材](1060G)」を得る。\n　〔1人でも判定失敗〕\n　　〈魔熊〉に気付かれて追いかけ回された。\n　　何も入手できない。\n",
        "息詰まる戦い：\n　火山が近いのだろうか？\n　ガスがもくもくと立ち込める渓谷を進むキミたちは、襲ってくるモンスターを倒しながら先を急ぐ。\n　PCは全員1Dを振ること。\n　出目が1～2の場合、戦闘中に毒ガスを吸い込んでしまい[疲労:80]を受ける。\n",
        "妖精のキッチン：\n　樹海を進むキミたちの目の前に、突如ファンシーな外見の建物が姿を現した。\n　中からはいい匂いが漂ってくる……\n　窓から覗き込んでみると、そこは〈厨房妖精〉が料理を作る妖精食堂だった！\n　もし、中に入ってみるなら「ニンゲンのお客様」と歓迎され、ひとりにつき「魔触媒」を1個消費することで不思議な妖精のお菓子を食べることができる。\n　それまでに受けていた[疲労]はすべて解除される。\n",
        "天然の迷宮：\n　キミたちは樹海を進む。\n　深く複雑な緑の迷宮は方向感覚を狂わせる！\n　PCは全員「難易度:18」の[知覚判定]を行なう。\n　2人以上判定に失敗した場合、方角を見失ってしまう！\n　全員[疲労:80]を受ける。\n　〔ファンブル〕\n　　シナリオ終了まで[方向音痴]タグを得る。\n　〔サブ職:探検家、地図屋〕\n　　判定に+2。\n",
        "謎の〈古来種〉：\n　樹海のはるか奥でキミたちは正体不明の〈古来種〉と出会った。\n　彼女はとてもとても退屈しており、PC(何人でもよい)は今までの冒険のエピソードを、実時間で5分以内にまとめて語ってもよい。\n　その場合、彼女はたいそう喜び褒美として「万年楓の小枝[魔触媒30][譲渡不可](820G)」を授ける。\n　望むPCは一時的なコネクションを取得してもよい。\n　関係は「友情」か「親愛」となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "モンスター退治の後は：\n　キミたちは激しい戦闘の末、火口にほど近い谷間に巣食う〈一眼巨人〉の一団を退治した。\n　PCの代表者1名は「CR*5+2D」を振る。\n　このロールは財宝表であるかのように特技やアイテムの効果を受けてよい。\n　130以上ならばキミたちは立派な宝箱を発見した！\n　「極上の玉鋼[換金](500G)」が収められていた。\n　140以上ならばさらに「氷不死鳥の羽根[コア素材](1160G)」を発見する。\n",
        "襲いくる水流：\n　上流で大雨が降ったのか、大河の水は氾濫し、濁流となってキミたちに襲い掛かってきた！\n　PCは全員「難易度:18」の[耐久判定]を行なう。\n　〔自身:[水棲]or[飛行]〕\n　　判定に自動成功してよい。\n　〔判定失敗〕\n　　水流に大きく流されてしまい[疲労:80]と[圧迫:1]を受ける。\n",
        "たまには鉱夫もいいよね：\n　火山に連なる渓谷は、良質の鉱床が露出しており、採掘にはもってこいだ。\n　この場所で1日を費やすならば、つるはしやハンマーを振るい、それぞれ「血色鉱石[換金](80G)」*1D個を得る。\n　〔サブ職:採掘師、採取人〕\n　　入手個数に+2してよい。\n",
        "バッドドリップ！？：\n　足元も見えない深い樹林を切り開いて進むキミたちは、うっかりカラフルな毒蛇の尻尾を踏みつけてしまった！\n　PCは全員2Dを振ること。\n　一番低い出目のPCは怒った蛇に噛み付かれてしまう。\n　毒が回って頭がクラクラしてきた。\n　【因果力】2点を失う。\n",
        "大冒険の舞台：\n　高台から密林を見下ろすと、極彩色の鳥が群れを成して眼下を横切っていくのが見えた。\n　この雄大な自然は、キミたちの挑戦を待ち受けている。\n　さあ、冒険を続けようじゃないか！\n　この探索表の出来事で次に何らかの判定をする場合ロールに+1D。\n　この効果はひとりでも全員でも次の判定のみ、一度効果を受けるかシーン終了時まで持続する。\n",
        "氷河の源：\n　キミたちは巨大な氷河を遡るうちに、氷の中に閉じ込められた魔法の輝きを見つける。\n　[火炎]タグのついた武器、もしくは魔法攻撃を持つPCがいるのならば運試しに1Dを振ること(1人1回まで)。\n　1～4なら「太古の氷[換金](100G)」を得るが、5以上なら「永久氷河の核[コア素材](1240G)」を得る。\n",
        "捉えがたき強敵：\n　火山から吹き出す白煙に紛れて〈煙霧精霊〉の群れが忍び寄る。\n　戦いは乱戦から始まり、キミたちの勝利で終わったものの、実に厳しい戦いだった。\n　PC全員は【因果力】1点を失う。\n　さらにPCは全員1Dを振ること。\n　強度30以上の[軽減(邪毒)]を持つPCは出目に+2。\n　〔1D:1～4〕\n　　PCは特技喪失する。\n　〔1D:5以上〕\n　　特に損害はない。\n",
        "小さな親切大きな宝石：\n　樹海の草木に埋もれた古い祭壇を見つけた。\n　祭壇を掃除して、お供えをしてもよい。\n　お供えにしてささげたアイテムの価格合計が300G以上なら、どこからともなく\n　「そ、そんな高価なものをささげないで！」\n　という言葉が聞こえてきて、頭の上から「大粒のルビー[換金](1200G)」が降ってくる。\n",
        "密林の魔女：\n　キミたちは密林の奥で不思議な小屋を発見した。\n　するとドアがひとりでに開き、\n　「何をグズグズしてるんだい！水汲みに薪割りかまどの掃除、やることは山積みだよ！」\n　としゃがれた老婆の怒声が飛び出してくる。\n　なんて尊大な！\n　しかし素直に従うならば、[疲労:300]を作業したPCで自由に分配すること。\n　その後老婆はPCが所持するアイテム1つに、マジックグレード(1D/2)のプレフィックスド効果(任意)を付与してくれる。\n　〔サブ職:メイド、エルダーメイド〕\n　　各PCが受ける[疲労]の強度を-10する。\n",
        "黒剣のお墨付き：\n　山上湖に巣くった〈火竜〉と戦う〈黒剣騎士団〉をキミたちは発見した。\n　彼らに加勢してモンスターとの戦闘に参加してもよい。\n　参加する場合半日を消費するが、希望者は「やるじゃねえか！」と感心した団長のアイザックとのコネクションを得る。\n　関係は「友情」となる。\n　このコネクションはアフタープレイで消滅するがログチケット:アザーゲット1枚を消費すれば維持してもよい。\n",
        "アレだけは苦手なんだよね……。：\n　野営の明かりはいろんなことを思い起こさせる。\n　PC全員は苦手なエネミー(ゴキブリ、人面魚、幽霊、ムカデなどなど)や現象を話そう。\n　全員自分を再確認して[弱点(告白した対象からの攻撃):20]を得る。\n　これはOSとして扱いシナリオ終了時まで持続する。\n",
        "悪意の嵐：\n　太古の戦場で突然の嵐に見舞われた。\n　暴風、落雷、大粒の雹と雨、そして怨嗟のうめき声。\n　PCの代表者1名は「難易度:22」の[解析判定]を行なう。\n　この判定は[偵察]タグがついているかのように扱う。\n　〔判定成功〕\n　　避難場所を見つけた。\n　　PCは全員[疲労:40]を受ける。\n　〔判定失敗〕\n　　避難場所が見つからない。\n　　怨嗟に蝕まれPCは全員[疲労:100]と特技喪失を受ける。\n",
        "仕上げは魔触媒：\n　岩陰に作りかけの魔方陣を発見した。\n　魔力を足してやれば力を発揮しそうだ。\n　希望者は自分のCR以上のランクを持つ「魔触媒」1つをささげること。\n　ささげた希望者は、自身の「タイミング:メジャーアクション」の特技のSRを+1できる(最大SRは超えない)。\n　この効果はOSとして扱いシナリオ終了時まで持続する。\n",
        "沈む！沈む！？：\n　じめじめとぬかるんだ、陰気な湿地帯を進むキミたちだが、突然ひざまで泥に埋まってしまう。\n　しまった、底なし沼だ！？\n　[飛行]状態ではないPCは【因果力】1点を失う。\n　[重鎧]もしくは[中鎧]を装備しているPCはさらに【因果力】1点を失う。\n",
        "あばれる恐竜：\n　シダの生い茂る沼地で突然二足歩行の恐竜の群れが現れる。\n　逃げずに調教を試みるPCは「難易度:27」の[交渉判定]を行なう。\n　この時[疲労:20]を受けるごとに+1のボーナスを得られる(最大+4まで)。\n　〔判定成功〕\n　　「剣二足竜の角笛(『LHZB2』P161)」を得る。\n　〔判定失敗〕\n　　恐竜たちは去っていく。\n",
        "一夜の勝負：\n　キミたちが野営をしていると、タキシード姿の青白い〈亡霊〉が姿を現した。\n　身構えるキミたちに〈亡霊〉はダイスで賭けをしようと持ちかけてくる。\n　もしも勝負を受けるなら代表者は【因果力】1点を支払い奇数か偶数を指定すること。\n　そののちGMは2Dを振ること。\n　的中したならば「財宝表:金銭」を1回ロールしてもよい。\n　4回勝負を行なうと〈亡霊〉は満足したのか朝日の中に消えていく。\n　勝負を受けず、〈亡霊〉を排除するならばPC全員は[疲労:100]を受ける。\n",
        "不浄なる戦場：\n　キミたちはアルヴの怨念が満ちると伝えられる古戦場で、呪われた〈骸骨巨人〉と激しい遭遇戦を繰り広げた。\n　戦いにはなんとか勝利し「無骨な白骨[換金](400G)」を1D個手に入れたが、かなりの消耗を強いられることになってしまう。\n　PCは全員[疲労:120]と特技喪失を受ける。\n",
        "飢えた〈魔狂鼠〉：\n　キミたちは黒い濁流のような、恐ろしい数の〈魔狂鼠〉の群れに遭遇した。\n　[食料]タグを持つアイテムすべてを投げ捨てて必死で逃げれば彼らを避けることができる。\n　食料を守るためには戦うしかない。\n　その場合は[疲労:120]を受け、さらに【因果力】1点を失う。\n",
        "怨念の森：\n　陰鬱な森がキミたちに牙を剥いた。\n　足元の腐葉土は瘴気を放つ泥濘へと変じ、ぬめるような霧が魂魄を削り取ってゆく。\n　[軽減(邪毒)]もしくは[高位保護]を持たないPCは【因果力】3点を失う。\n",
        "お憑かれ様：\n　アンデッドのうろつく古戦場で、キミたちは不注意から小さな祠を倒してしまった。\n　PCは全員2Dを振ること。\n　一番低い出目のPCは猛烈な寒気とともに[亡霊憑き]タグを得る。\n　このタグはシナリオ終了まで解除されず、タグを持つPCは判定でファンブルをするたびに[疲労:20]を受ける。\n",
        "夜間飛行：\n　キミたちは湿地帯で野営することになった。\n　PCは全員2Dを振ること。\n　一番高い出目のPCが見張りをしていると、月明かりの下、小さな赤いクモの子供たちが何百匹も風に乗って飛んでいく、幻想的な光景を目にした。\n　[疲労]が-30される。\n　また「煉獄クモの燃糸[コア素材](1540G)」を入手する。\n",
        "沸き立つ沼：\n　硫黄の臭いが立ち込める泥沼が突如沸騰し始めた！\n　毒ガスと灼熱の泥がキミたちに襲い掛かる。\n　[高位保護]もしくは[天然]プロップの影響を受けないPC以外は1Dを振る。\n　この時【因果力】1点を支払って1Dではなく2Dを振ってもよい。\n　出目の合計が2以下のPCは[戦闘不能]となる。\n　3～5ならば[疲労:100]を受ける。\n　6以上なら被害はない。\n",
        "ハイパードロップタイム！：\n　ひょんなことからキミたちは大量のエネミーに追い回され、苦戦の末全滅させた。\n　PCは【因果力】を任意の数だけ消費してもよい。\n　各自消費した【因果力】と同数のダイスを振り、全員の出目を合計すること。\n　ダイスの合計値が3以上ならPCたちは1500Gを得る。\n　8以上でさらに「古代のメダル[コア素材](1540G)」と「魔凝結[魔触媒29](770G)」2つを得る。\n　13以上でさらに「ダイヤの原石[換金](2300G)」1つを得る。\n",
        "心霊スポット？：\n　静かに水を湛えた湖のそばを通った時、急な悪寒に襲われた。\n　湖面に目をやると、水面下にひしめく亡者の群れと目が合ってしまった。\n　呪詛の視線がキミたちを捉える。\n　PCは全員1Dを振ること。\n　[高位保護]を持つPCは出目に+2。\n　〔1D:1～3〕\n　　ごっそり気力を削られる。\n　　【因果力】2点を失う。\n　〔1D:4以上〕\n　　視線に力を込めて睨み返す。\n　　やっぱ気合いだな。\n",
        "武人の誉れ：\n　荒野の只中、数多の折れた武器や朽ちた鎧でできた玉座に、古めかしい甲冑をまとったアルヴの亡霊が座している。\n　彼はキミたちに「勝者には名誉を、敗者には死を」と告げ、一騎打ちを申し込んできた。\n　断るならば、彼の姿は玉座と共に掻き消える。\n　PCの代表者1名は「難易度25」の[耐久判定]を行なう。\n　判定に失敗した場合[疲労:300]を受ける。\n　判定後、判定したPCが[戦闘不能]状態でなければ、彼は「人斬りのギガントエストック[M2]([人間]に対しダメージ+40。『LHZB2』P136&P165)」を残し、光の粒子となって天に昇る。\n",
        "見定める視線：\n　旅の道中で、キミたちは何度もモンスターの襲撃を受けた。\n　熟練の冒険者であるキミたちは危なげなくそれらを撃破し、合計で2D*200Gの戦利品を得た。\n　しかし、その様子を敵意に満ちた視線がずっと監視していたことには気づかなかったようだ。\n　このシナリオのクライマックスにおいて、GMは【因果力】(PC人数*2)点ぶんのGM用EXパワーを追加で使用できる。\n",
        "突然の闇夜：\n　沼に潜む〈蛙竜〉に出会ったキミたちは、フォッグブレスで突然の暗黒に突き落とされる。\n　PCは全員1Dを振ること。\n　[水棲]、[飛行]、[暗視]を持っているのならば1つにつき出目に+1を得られる。\n　〔1D:1～4〕\n　　逃亡中に焼け付くような毒液を受けて[疲労:120]を受けてしまう。\n　〔1D:5～〕\n　　光を閉ざす霧の中を逃走して[疲労:60]を受ける。\n",
        "幻の金鉱：\n　キミたちは瘴気を漂わせる金鉱を見つけた。\n　この金鉱に潜って金を探したいPCは2Dを振ること。\n　出た出目がいままでPCたち全員でこの金鉱を探った数と等しいか小さければ「金鉱石[換金]700G」を得る。\n　出目が大きい場合、PCたち全員は[疲労:80]を受けて気絶し丸一日が経過する。\n　目が覚めた時、金鉱は消えている。\n",
        "幻惑草の群生地：\n　粘つくような湿気の原生林の中で、甘い香りを放つ花畑にでくわした。\n　PCは全員「難易度:20」の[耐久判定]を行なう。\n　強度30以上の[軽減(精神)]を持つPCは判定に+1D。\n　〔判定成功〕\n　　PCは意識をしっかり保って花畑を抜ける。\n　〔判定失敗〕\n　　PCは幻覚を見て仲間のひとり(失敗したPCが選択)に斬りかかり、その仲間に[疲労:100]を与えてしまう。\n　　この効果は失敗したPC以外であれば《かばう》で代わりに受けることができる。\n　　また失敗したPCは特技喪失を受ける。\n",
        "硫黄の谷間：\n　黄褐色の霧が立ちこめる荒々しい谷間にたどり着く。\n　キミたちは呼吸を我慢するようにしてこの危険な谷をくぐり抜ける。\n　PCは全員1Dを振ること。\n　1が出たPCは濁った水たまりに動物の死体を見つけて気持ちが悪くなってしまい、シナリオ終了時まで[弱点(精神):20](OS扱い)を得る。\n",
        "火竜との戦い：\n　火炎をまとい大空を支配する恐ろしい火竜との遭遇戦となった。\n　PCは全員1Dを振る。\n　自発的に特技喪失をしてもよく、その場合追加で+1Dしてもよい。\n　ダイスの合計値が(PC人数*4+2)以上ならば勝利して「赤色魔力ダイオード[コア素材](1640G)」を得る。\n　未満であった場合はPCは全員【因果力】1点を失い[疲労:50]を受ける。\n",
    ],
};

/// i18n `LogHorizon.table.PTAG`。
static JA_TABLE_PTAG: D66Table = D66Table::new(
    "パーソナリティタグ表",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("[おませさん]")),
        (12, TableItem::Text("[好奇心旺盛]")),
        (13, TableItem::Text("[寂しがりや]")),
        (14, TableItem::Text("[生真面目]")),
        (15, TableItem::Text("[食いしん坊]")),
        (16, TableItem::Text("[やんちゃ]または[おてんば]")),
        (21, TableItem::Text("[お人よし]")),
        (22, TableItem::Text("[情熱家]")),
        (23, TableItem::Text("[世話好き]")),
        (24, TableItem::Text("[理知的]")),
        (25, TableItem::Text("[頑固者]")),
        (26, TableItem::Text("[兄貴肌]または[姉御肌]")),
        (31, TableItem::Text("[義理堅い]")),
        (32, TableItem::Text("[気まぐれ]")),
        (33, TableItem::Text("[職人気質]")),
        (34, TableItem::Text("[熱血漢]")),
        (35, TableItem::Text("[努力家]")),
        (36, TableItem::Text("[男好き]または[女好き]")),
        (41, TableItem::Text("[家庭的]")),
        (42, TableItem::Text("[負けず嫌い]")),
        (43, TableItem::Text("[純真]")),
        (44, TableItem::Text("[朴念仁]")),
        (45, TableItem::Text("[慈悲深い]")),
        (46, TableItem::Text("[マイペース]")),
        (51, TableItem::Text("[楽天家]")),
        (52, TableItem::Text("[仲間思い]")),
        (53, TableItem::Text("[誇り高い]")),
        (54, TableItem::Text("[社交的]")),
        (55, TableItem::Text("[冷静沈着]")),
        (56, TableItem::Text("[ロマンチスト]")),
        (61, TableItem::Text("[学者肌]")),
        (62, TableItem::Text("[内向的]")),
        (63, TableItem::Text("[苦労人]")),
        (64, TableItem::Text("[派手好き]")),
        (65, TableItem::Text("[勇猛果敢]")),
        (66, TableItem::Text("[ミステリアス]")),
    ],
);

/// i18n `LogHorizon.table.KOYU`。
static JA_TABLE_KOYU: D66Table = D66Table::new(
    "交友表",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("交友対象への庇護\nあなたは、交友対象を庇護したいと思っている。守ってあげられる強さがあなたにはある。")),
        (12, TableItem::Text("交友対象への親愛\nあなたは、交友対象を好ましく思っている。その気持ちを彼に伝えているとは限らない。")),
        (13, TableItem::Text("交友対象との義兄弟\nあなたは、交友対象と兄弟同然の間柄だ。もちろん、血は繋がっていないが、そんなことは些細なことだ。")),
        (14, TableItem::Text("交友対象の英雄\nあなたは、交友対象から英雄視されている。それは例えあなたが否定しても変わらない。")),
        (15, TableItem::Text("交友対象への尊敬\nあなたは、交友対象を尊敬している。彼の技術、心の強さ。それがなんであっても彼への敬意は変わらない。")),
        (16, TableItem::Text("交友対象の相棒\nあなたは、交友対象を相棒だと思っている。彼なら自分と一緒に歩んでくれるだろう。")),
        (21, TableItem::Text("交友対象への庇護\nあなたは、交友対象を庇護したいと思っている。守ってあげられる強さがあなたにはある。")),
        (22, TableItem::Text("交友対象への親愛\nあなたは、交友対象を好ましく思っている。その気持ちを彼に伝えているとは限らない。")),
        (23, TableItem::Text("交友対象との義兄弟\nあなたは、交友対象と兄弟同然の間柄だ。もちろん、血は繋がっていないが、そんなことは些細なことだ。")),
        (24, TableItem::Text("交友対象の英雄\nあなたは、交友対象から英雄視されている。それは例えあなたが否定しても変わらない。")),
        (25, TableItem::Text("交友対象への尊敬\nあなたは、交友対象を尊敬している。彼の技術、心の強さ。それがなんであっても彼への敬意は変わらない。")),
        (26, TableItem::Text("交友対象の相棒\nあなたは、交友対象を相棒だと思っている。彼なら自分と一緒に歩んでくれるだろう。")),
        (31, TableItem::Text("交友対象の恩\nあなたは交友対象に恩を受けた。今度は自分がその恩に報いる番だ。")),
        (32, TableItem::Text("交友対象のライバル\nあなたは、交友対象をライバルだと思っている。それは一方的なものかもしれないし、切磋琢磨する間柄かもしれない。")),
        (33, TableItem::Text("交友対象への興味\nあなたは、交友対象に対して興味を持っている。彼を見ているのが面白く、彼の行動を見届けてみたい。")),
        (34, TableItem::Text("交友対象との友情\nあなたは、彼を友人だと思っている。それはどこに行っても変わらない確かなものだ。")),
        (35, TableItem::Text("交友対象との同志\nあなたは、交友対象の同志である。同好の士であったり、同じ目的に向かう者だったりするだろう。")),
        (36, TableItem::Text("交友対象への理解\nあなたは、交友対象を理解したいと思っている。彼はあなたと違う新しい視点を見せてくれる。")),
        (41, TableItem::Text("交友対象の恩\nあなたは交友対象に恩を受けた。今度は自分がその恩に報いる番だ。")),
        (42, TableItem::Text("交友対象のライバル\nあなたは、交友対象をライバルだと思っている。それは一方的なものかもしれないし、切磋琢磨する間柄かもしれない。")),
        (43, TableItem::Text("交友対象への興味\nあなたは、交友対象に対して興味を持っている。彼を見ているのが面白く、彼の行動を見届けてみたい。")),
        (44, TableItem::Text("交友対象との友情\nあなたは、彼を友人だと思っている。それはどこに行っても変わらない確かなものだ。")),
        (45, TableItem::Text("交友対象との同志\nあなたは、交友対象の同志である。同好の士であったり、同じ目的に向かう者だったりするだろう。")),
        (46, TableItem::Text("交友対象への理解\nあなたは、交友対象を理解したいと思っている。彼はあなたと違う新しい視点を見せてくれる。")),
        (51, TableItem::Text("交友対象への尽力\nあなたは、交友対象に尽くしたいと思っている。それは彼の人柄かもしれないし、あなたの拘りかもしれない。")),
        (52, TableItem::Text("交友対象との師弟\nあなたは、交友対象との師弟関係を結んでいる。どちらが師でも構わないがいろいろ教えられることがあるだろう。")),
        (53, TableItem::Text("交友対象との雇用関係\nあなたは、交友対象と雇用関係にある。あなたと彼は仕事を通じて互いの力量を認め合う仲だ。")),
        (54, TableItem::Text("交友対象の隣人\nあなたは交友対象の近隣に住んでいる。毎日挨拶を交わす程度かもしれないし、一緒に夕食を食べる仲かもしれない。")),
        (55, TableItem::Text("交友対象との取引\nあなたは交友対象と商売をしている。互いに利のある取引ができる相手だ。")),
        (56, TableItem::Text("交友対象の家族\nあなたは交友対象と一緒に暮らしている。同じ家に誰かがいると寂しくはないだろう。")),
        (61, TableItem::Text("交友対象への尽力\nあなたは、交友対象に尽くしたいと思っている。それは彼の人柄かもしれないし、あなたの拘りかもしれない。")),
        (62, TableItem::Text("交友対象との師弟\nあなたは、交友対象との師弟関係を結んでいる。どちらが師でも構わないがいろいろ教えられることがあるだろう。")),
        (63, TableItem::Text("交友対象との雇用関係\nあなたは、交友対象と雇用関係にある。あなたと彼は仕事を通じて互いの力量を認め合う仲だ。")),
        (64, TableItem::Text("交友対象の隣人\nあなたは交友対象の近隣に住んでいる。毎日挨拶を交わす程度かもしれないし、一緒に夕食を食べる仲かもしれない。")),
        (65, TableItem::Text("交友対象との取引\nあなたは交友対象と商売をしている。互いに利のある取引ができる相手だ。")),
        (66, TableItem::Text("交友対象の家族\nあなたは交友対象と一緒に暮らしている。同じ家に誰かがいると寂しくはないだろう。")),
    ],
);

/// i18n `LogHorizon.table.MGR1`。
static JA_TABLE_MGR1: D66Table = D66Table::new(
    "プレフィックスドアイテム効果表（マジックグレード１）",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("接頭語：気合の　対応タグ：すべての武器\nアイテム効果：この武器の【攻撃力】に＋１する。")),
        (12, TableItem::Text("接頭語：秘術の　対応タグ：[杖][魔石]\nアイテム効果：このアイテムの【魔力】を＋１する。")),
        (13, TableItem::Text("接頭語：一撃の　対応タグ：[白兵攻撃]可能な武器\nアイテム効果：〔起動：ダメージロール〕この武器による[白兵攻撃]のダメージロールに＋７する。シナリオ１回使用可能。")),
        (14, TableItem::Text("接頭語：狙撃の　対応タグ：[弓][投擲]\nアイテム効果：〔起動：行動〕この武器による[射撃攻撃]と同時に使用する。攻撃の射程を＋２Ｓｑする。シナリオ１回使用可能。")),
        (15, TableItem::Text("接頭語：必殺の　対応タグ：すべての武器\nアイテム効果：〔起動：判定直後〕この武器による[武器攻撃]による[命中判定]のダイスに６の出目が１つ以上あれば、判定をクリティカルにする。シナリオ１回使用可能。")),
        (16, TableItem::Text("接頭語：火炎の　対応タグ：すべての武器、楽器\nアイテム効果：アイテムに[火炎]タグを追加する。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「冷気の」「電撃の」のように変更すること）")),
        (21, TableItem::Text("接頭語：気合の　対応タグ：すべての武器\nアイテム効果：この武器の【攻撃力】に＋１する。")),
        (22, TableItem::Text("接頭語：秘術の　対応タグ：[杖][魔石]\nアイテム効果：このアイテムの【魔力】を＋１する。")),
        (23, TableItem::Text("接頭語：一撃の　対応タグ：[白兵攻撃]可能な武器\nアイテム効果：〔起動：ダメージロール〕この武器による[白兵攻撃]のダメージロールに＋７する。シナリオ１回使用可能。")),
        (24, TableItem::Text("接頭語：狙撃の　対応タグ：[弓][投擲]\nアイテム効果：〔起動：行動〕この武器による[射撃攻撃]と同時に使用する。攻撃の射程を＋２Ｓｑする。シナリオ１回使用可能。")),
        (25, TableItem::Text("接頭語：必殺の　対応タグ：すべての武器\nアイテム効果：〔起動：判定直後〕この武器による[武器攻撃]による[命中判定]のダイスに６の出目が１つ以上あれば、判定をクリティカルにする。シナリオ１回使用可能。")),
        (26, TableItem::Text("接頭語：火炎の　対応タグ：すべての武器、楽器\nアイテム効果：アイテムに[火炎]タグを追加する。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「冷気の」「電撃の」のように変更すること）")),
        (31, TableItem::Text("接頭語：炎使いの　対応タグ：[杖][魔石][腕部]\nアイテム効果：〔起動：ダメージロール〕あなたが行う[火炎]タグを持つ攻撃のダメージロールに＋７する。シナリオ１回使用可能。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「氷使いの」「雷使いの」のように変更すること）")),
        (32, TableItem::Text("接頭語：鉄身の　対応タグ：[盾][重鎧][中鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近距離からの攻撃）：３]を得る。")),
        (33, TableItem::Text("接頭語：矢除けの　対応タグ：[盾][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近以外からの攻撃）：３]を得る。")),
        (34, TableItem::Text("接頭語：火除けの　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（火炎）：１０]を得る。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「氷除けの」「雷除けの」のように変更すること）")),
        (35, TableItem::Text("接頭語：根性の　対応タグ：[重鎧][頭部]\nアイテム効果：〔起動：本文〕あなたがBSを受けた直後に使用する。直前に受けたBSから１つ選んで解除する。シナリオ１回使用可能。")),
        (36, TableItem::Text("接頭語：癒しの　対応タグ：[腕部]\nアイテム効果：あなたの【回復力】を＋２する。")),
        (41, TableItem::Text("接頭語：炎使いの　対応タグ：[杖][魔石][腕部]\nアイテム効果：〔起動：ダメージロール〕あなたが行う[火炎]タグを持つ攻撃のダメージロールに＋７する。シナリオ１回使用可能。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「氷使いの」「雷使いの」のように変更すること）")),
        (42, TableItem::Text("接頭語：鉄身の　対応タグ：[盾][重鎧][中鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近距離からの攻撃）：３]を得る。")),
        (43, TableItem::Text("接頭語：矢除けの　対応タグ：[盾][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近以外からの攻撃）：３]を得る。")),
        (44, TableItem::Text("接頭語：火除けの　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（火炎）：１０]を得る。（この効果を選んだ時、タグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「氷除けの」「雷除けの」のように変更すること）")),
        (45, TableItem::Text("接頭語：根性の　対応タグ：[重鎧][頭部]\nアイテム効果：〔起動：本文〕あなたがBSを受けた直後に使用する。直前に受けたBSから１つ選んで解除する。シナリオ１回使用可能。")),
        (46, TableItem::Text("接頭語：癒しの　対応タグ：[腕部]\nアイテム効果：あなたの【回復力】を＋２する。")),
        (51, TableItem::Text("接頭語：スナネズミの　対応タグ：[腕部][外套]\nアイテム効果：〔起動：クリンナップ〕あなたの【ヘイト】をー２する。シナリオ１回使用可能。")),
        (52, TableItem::Text("接頭語：フクロウの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：インスタント〕あなたは[暗視]タグを得る。この効果はCSとして扱う。シナリオ１回使用可能。")),
        (53, TableItem::Text("接頭語：ヤマイワナの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：インスタント〕あなたは[水棲]タグを得る。この効果はCSとして扱う。シナリオ１回使用可能。")),
        (54, TableItem::Text("接頭語：目利きの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたがドロップ品ロールを行った直後に使用する。そのロールを振り直す。シナリオ１回使用可能。")),
        (55, TableItem::Text("接頭語：宝探しの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが財宝表ロールを行った直後に使用する。そのロールを振り直す。シナリオ１回使用可能。")),
        (56, TableItem::Text("接頭語：早変わりの　対応タグ：[鞄]\nアイテム効果：〔起動：インスタント〕あなたは即座に《装備の変更》を行う。シーン１回使用可能。")),
        (61, TableItem::Text("接頭語：スナネズミの　対応タグ：[腕部][外套]\nアイテム効果：〔起動：クリンナップ〕あなたの【ヘイト】をー２する。シナリオ１回使用可能。")),
        (62, TableItem::Text("接頭語：フクロウの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：インスタント〕あなたは[暗視]タグを得る。この効果はCSとして扱う。シナリオ１回使用可能。")),
        (63, TableItem::Text("接頭語：ヤマイワナの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：インスタント〕あなたは[水棲]タグを得る。この効果はCSとして扱う。シナリオ１回使用可能。")),
        (64, TableItem::Text("接頭語：目利きの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたがドロップ品ロールを行った直後に使用する。そのロールを振り直す。シナリオ１回使用可能。")),
        (65, TableItem::Text("接頭語：宝探しの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが財宝表ロールを行った直後に使用する。そのロールを振り直す。シナリオ１回使用可能。")),
        (66, TableItem::Text("接頭語：早変わりの　対応タグ：[鞄]\nアイテム効果：〔起動：インスタント〕あなたは即座に《装備の変更》を行う。シーン１回使用可能。")),
    ],
);

/// i18n `LogHorizon.table.MGR2`。
static JA_TABLE_MGR2: D66Table = D66Table::new(
    "プレフィックスドアイテム効果表（マジックグレード２）",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("接頭語：怒りの　対応タグ：[両手]武器\nアイテム効果：〔起動：ダメージロール〕この武器による[武器攻撃]のダメージロールに＋[あなたの【ヘイト】]する。シーン１回使用可能。")),
        (12, TableItem::Text("接頭語：連撃の　対応タグ：[片手]武器\nアイテム効果：〔起動：判定直後〕この武器による[武器攻撃]の[命中判定]を振り直す。シーン１回使用可能。")),
        (13, TableItem::Text("接頭語：鋭刃の　対応タグ：[剣][刀][槍]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (14, TableItem::Text("接頭語：痛撃の　対応タグ：[槌斧][格闘][鞭][杖]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (15, TableItem::Text("接頭語：魔弾の　対応タグ：[弓][投擲]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (16, TableItem::Text("接頭語：理力の　対応タグ：すべての武器、[魔石][楽器]\nアイテム効果：あなたの[魔法攻撃][特殊攻撃]のダメージロールに＋１Ｄする。この効果は複数累積しない。")),
        (21, TableItem::Text("接頭語：怒りの　対応タグ：[両手]武器\nアイテム効果：〔起動：ダメージロール〕この武器による[武器攻撃]のダメージロールに＋[あなたの【ヘイト】]する。シーン１回使用可能。")),
        (22, TableItem::Text("接頭語：連撃の　対応タグ：[片手]武器\nアイテム効果：〔起動：判定直後〕この武器による[武器攻撃]の[命中判定]を振り直す。シーン１回使用可能。")),
        (23, TableItem::Text("接頭語：鋭刃の　対応タグ：[剣][刀][槍]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (24, TableItem::Text("接頭語：痛撃の　対応タグ：[槌斧][格闘][鞭][杖]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (25, TableItem::Text("接頭語：魔弾の　対応タグ：[弓][投擲]\nアイテム効果：この武器による[武器攻撃]のダメージロールに＋１Ｄする。")),
        (26, TableItem::Text("接頭語：理力の　対応タグ：すべての武器、[魔石][楽器]\nアイテム効果：あなたの[魔法攻撃][特殊攻撃]のダメージロールに＋１Ｄする。この効果は複数累積しない。")),
        (31, TableItem::Text("接頭語：鬼殺しの　対応タグ：すべての武器\nアイテム効果：この武器による[人型]への[武器攻撃]のダメージロールに＋１Ｄする。（この効果を選んだ時、種族のタグを[自然][精霊][幻獣][不死][人造][人間]のいずれかに変更してもよい。その場合は接頭語も「精霊殺しの」「幻獣殺しの」のように変更すること）")),
        (32, TableItem::Text("接頭語：堅守の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：この防具の【物理防御力】に＋４する。")),
        (33, TableItem::Text("接頭語：抗魔の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：この防具の【魔法防御力】に＋４する。")),
        (34, TableItem::Text("接頭語：防壁の　対応タグ：[盾][頭部]\nアイテム効果：あなたはシーン開始時に[障壁：１０]を得る。")),
        (35, TableItem::Text("接頭語：忍耐の　対応タグ：[盾][頭部]\nアイテム効果：〔起動：本文〕あなたが強制的な移動を受けた時に使用する。その移動距離をー１Ｓｑする。")),
        (36, TableItem::Text("接頭語：護法の　対応タグ：[腕部]\nアイテム効果：あなたが与える[障壁]の強度は＋５される。")),
        (41, TableItem::Text("接頭語：鬼殺しの　対応タグ：すべての武器\nアイテム効果：この武器による[人型]への[武器攻撃]のダメージロールに＋１Ｄする。（この効果を選んだ時、種族のタグを[自然][精霊][幻獣][不死][人造][人間]のいずれかに変更してもよい。その場合は接頭語も「精霊殺しの」「幻獣殺しの」のように変更すること）")),
        (42, TableItem::Text("接頭語：堅守の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：この防具の【物理防御力】に＋４する。")),
        (43, TableItem::Text("接頭語：抗魔の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：この防具の【魔法防御力】に＋４する。")),
        (44, TableItem::Text("接頭語：防壁の　対応タグ：[盾][頭部]\nアイテム効果：あなたはシーン開始時に[障壁：１０]を得る。")),
        (45, TableItem::Text("接頭語：忍耐の　対応タグ：[盾][頭部]\nアイテム効果：〔起動：本文〕あなたが強制的な移動を受けた時に使用する。その移動距離をー１Ｓｑする。")),
        (46, TableItem::Text("接頭語：護法の　対応タグ：[腕部]\nアイテム効果：あなたが与える[障壁]の強度は＋５される。")),
        (51, TableItem::Text("接頭語：脈動の　対応タグ：[腕部]\nアイテム効果：あなたが与える[再生]の強度は＋３される。")),
        (52, TableItem::Text("接頭語：疾走の　対応タグ：[脚部][外套]\nアイテム効果：〔起動：行動〕あなたが《ラン》《ダッシュ》を行う時に使用する。その移動距離に＋１Ｓｑする。シーン１回使用可能。")),
        (53, TableItem::Text("接頭語：瞬速の　対応タグ：[脚部][外套]\nアイテム効果：このアイテムの[行動修正]に＋３する。")),
        (54, TableItem::Text("接頭語：逆境の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが[戦闘不能]になった直後に使用できる。あなたは【因果力】１点を得る。シーン１回使用可能。")),
        (55, TableItem::Text("接頭語：森渡りの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：あなたはシーン開始時に[軽減（[天然]プロップ、ギミックからのダメージ）：１５]を得る。（この効果を選んだ時、タグを[魔法][機械]のいずれかに変えてもよい。その場合は、接頭語も「魔渡りの」「罠渡りの」のように変更すること）")),
        (56, TableItem::Text("接頭語：旅人の　対応タグ：[鞄]\nアイテム効果：このアイテムに[所持品スロット]４個を追加する。")),
        (61, TableItem::Text("接頭語：脈動の　対応タグ：[腕部]\nアイテム効果：あなたが与える[再生]の強度は＋３される。")),
        (62, TableItem::Text("接頭語：疾走の　対応タグ：[脚部][外套]\nアイテム効果：〔起動：行動〕あなたが《ラン》《ダッシュ》を行う時に使用する。その移動距離に＋１Ｓｑする。シーン１回使用可能。")),
        (63, TableItem::Text("接頭語：瞬速の　対応タグ：[脚部][外套]\nアイテム効果：このアイテムの[行動修正]に＋３する。")),
        (64, TableItem::Text("接頭語：逆境の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが[戦闘不能]になった直後に使用できる。あなたは【因果力】１点を得る。シーン１回使用可能。")),
        (65, TableItem::Text("接頭語：森渡りの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：あなたはシーン開始時に[軽減（[天然]プロップ、ギミックからのダメージ）：１５]を得る。（この効果を選んだ時、タグを[魔法][機械]のいずれかに変えてもよい。その場合は、接頭語も「魔渡りの」「罠渡りの」のように変更すること）")),
        (66, TableItem::Text("接頭語：旅人の　対応タグ：[鞄]\nアイテム効果：このアイテムに[所持品スロット]４個を追加する。")),
    ],
);

/// i18n `LogHorizon.table.MGR3`。
static JA_TABLE_MGR3: D66Table = D66Table::new(
    "プレフィックスドアイテム効果表（マジックグレード３）",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("接頭語：気迫の　対応タグ：すべての武器\nアイテム効果：この武器の【攻撃力】に＋３する。")),
        (12, TableItem::Text("接頭語：神秘の　対応タグ：[杖][魔石]\nアイテム効果：このアイテムの【魔力】に＋３する。")),
        (13, TableItem::Text("接頭語：遠当ての　対応タグ：[弓][投擲]\nアイテム効果：この武器の射程に＋１Ｓｑする。")),
        (14, TableItem::Text("接頭語：吸血の　対応タグ：[白兵攻撃]可能な武器\nアイテム効果：この武器による[白兵攻撃]でダメージを与えた時、あなたの【HP】は５点回復する。")),
        (15, TableItem::Text("接頭語：衝撃の　対応タグ：[片手]武器\nアイテム効果：この武器による《基本武器攻撃》でダメージを与えた時、攻撃の対象に[放心]を与える。")),
        (16, TableItem::Text("接頭語：怒号の　対応タグ：[両手]武器\nアイテム効果：この武器による《基本武器攻撃》でダメージを与えた時、攻撃の対象に[萎縮]を与える。")),
        (21, TableItem::Text("接頭語：気迫の　対応タグ：すべての武器\nアイテム効果：この武器の【攻撃力】に＋３する。")),
        (22, TableItem::Text("接頭語：神秘の　対応タグ：[杖][魔石]\nアイテム効果：このアイテムの【魔力】に＋３する。")),
        (23, TableItem::Text("接頭語：遠当ての　対応タグ：[弓][投擲]\nアイテム効果：この武器の射程に＋１Ｓｑする。")),
        (24, TableItem::Text("接頭語：吸血の　対応タグ：[白兵攻撃]可能な武器\nアイテム効果：この武器による[白兵攻撃]でダメージを与えた時、あなたの【HP】は５点回復する。")),
        (25, TableItem::Text("接頭語：衝撃の　対応タグ：[片手]武器\nアイテム効果：この武器による《基本武器攻撃》でダメージを与えた時、攻撃の対象に[放心]を与える。")),
        (26, TableItem::Text("接頭語：怒号の　対応タグ：[両手]武器\nアイテム効果：この武器による《基本武器攻撃》でダメージを与えた時、攻撃の対象に[萎縮]を与える。")),
        (31, TableItem::Text("接頭語：甲羅の　対応タグ：[盾][重鎧][中鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近距離からの攻撃）：１０]を得る。")),
        (32, TableItem::Text("接頭語：矢弾きの　対応タグ：[盾][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近以外からの攻撃）：１０]を得る。")),
        (33, TableItem::Text("接頭語：耐火の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（火炎）：２５]を得る。（この効果を選んだ時、軽減するタグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「耐冷の」「耐電の」のように変更すること）")),
        (34, TableItem::Text("接頭語：城砦の　対応タグ：[盾][頭部]\nアイテム効果：あなたはシーン開始時に【障壁：２０】を得る。")),
        (35, TableItem::Text("接頭語：物見の　対応タグ：[頭部]\nアイテム効果：あなたが行う[偵察]タグを持つ行動、および《異常探知》の判定に＋１Ｄする。")),
        (36, TableItem::Text("接頭語：快癒の　対応タグ：[腕部]\nアイテム効果：あなたの【回復力】に＋５する。")),
        (41, TableItem::Text("接頭語：甲羅の　対応タグ：[盾][重鎧][中鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近距離からの攻撃）：１０]を得る。")),
        (42, TableItem::Text("接頭語：矢弾きの　対応タグ：[盾][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（至近以外からの攻撃）：１０]を得る。")),
        (43, TableItem::Text("接頭語：耐火の　対応タグ：[重鎧][中鎧][軽鎧]\nアイテム効果：あなたはシーン開始時に[軽減（火炎）：２５]を得る。（この効果を選んだ時、軽減するタグを[冷気][電撃][邪毒][光輝][精神]のいずれかに変えてもよい。その場合は接頭語も「耐冷の」「耐電の」のように変更すること）")),
        (44, TableItem::Text("接頭語：城砦の　対応タグ：[盾][頭部]\nアイテム効果：あなたはシーン開始時に【障壁：２０】を得る。")),
        (45, TableItem::Text("接頭語：物見の　対応タグ：[頭部]\nアイテム効果：あなたが行う[偵察]タグを持つ行動、および《異常探知》の判定に＋１Ｄする。")),
        (46, TableItem::Text("接頭語：快癒の　対応タグ：[腕部]\nアイテム効果：あなたの【回復力】に＋５する。")),
        (51, TableItem::Text("接頭語：罠外しの　対応タグ：[腕部]\nアイテム効果：〔起動：判定直前〕あなたが《プロップ解除》を行う時に使用する。その判定はクリティカルとなる。シーン１回使用可能。")),
        (52, TableItem::Text("接頭語：不動の　対応タグ：[脚部][外套]\nアイテム効果：あなたは[阻止能力]を失わない。")),
        (53, TableItem::Text("接頭語：影走りの　対応タグ：[脚部][外套]\nアイテム効果：あなたが[隠密状態]の時に《ラン》を行っても、[隠密状態]は解除されない。")),
        (54, TableItem::Text("接頭語：深海の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：あなたは[暗視][水棲]タグを得る。")),
        (55, TableItem::Text("接頭語：金運の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが財宝表ロールまたはドロップ表ロールを行った直後に使用する。そのロールを振り直す。シーン１回使用可能。")),
        (56, TableItem::Text("接頭語：罠避けの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：ダメージ適用直前〕あなたに適用される予定のプロップおよび[ギミック]によるダメージを無効にする。シナリオ１回使用可能。")),
        (61, TableItem::Text("接頭語：罠外しの　対応タグ：[腕部]\nアイテム効果：〔起動：判定直前〕あなたが《プロップ解除》を行う時に使用する。その判定はクリティカルとなる。シーン１回使用可能。")),
        (62, TableItem::Text("接頭語：不動の　対応タグ：[脚部][外套]\nアイテム効果：あなたは[阻止能力]を失わない。")),
        (63, TableItem::Text("接頭語：影走りの　対応タグ：[脚部][外套]\nアイテム効果：あなたが[隠密状態]の時に《ラン》を行っても、[隠密状態]は解除されない。")),
        (64, TableItem::Text("接頭語：深海の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：あなたは[暗視][水棲]タグを得る。")),
        (65, TableItem::Text("接頭語：金運の　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：本文〕あなたが財宝表ロールまたはドロップ表ロールを行った直後に使用する。そのロールを振り直す。シーン１回使用可能。")),
        (66, TableItem::Text("接頭語：罠避けの　対応タグ：[補助装備][鞄][楽器]\nアイテム効果：〔起動：ダメージ適用直前〕あなたに適用される予定のプロップおよび[ギミック]によるダメージを無効にする。シナリオ１回使用可能。")),
    ],
);

/// i18n `LogHorizon.table.HLOC`。
static JA_TABLE_HLOC: D66Table = D66Table::new(
    "攻撃命中箇所",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("額")),
        (12, TableItem::Text("頬")),
        (13, TableItem::Text("鼻")),
        (14, TableItem::Text("顎")),
        (15, TableItem::Text("後頭部")),
        (16, TableItem::Text("首")),
        (21, TableItem::Text("耳")),
        (22, TableItem::Text("目")),
        (23, TableItem::Text("こめかみ")),
        (24, TableItem::Text("腕")),
        (25, TableItem::Text("肘")),
        (26, TableItem::Text("手")),
        (31, TableItem::Text("手の指")),
        (32, TableItem::Text("心臓")),
        (33, TableItem::Text("胃")),
        (34, TableItem::Text("肺")),
        (35, TableItem::Text("肋骨")),
        (36, TableItem::Text("肩")),
        (41, TableItem::Text("背")),
        (42, TableItem::Text("わき腹")),
        (43, TableItem::Text("腰")),
        (44, TableItem::Text("下腹")),
        (45, TableItem::Text("太もも")),
        (46, TableItem::Text("喉")),
        (51, TableItem::Text("ふくらはぎ")),
        (52, TableItem::Text("アキレス腱")),
        (53, TableItem::Text("かかと")),
        (54, TableItem::Text("すね")),
        (55, TableItem::Text("足の小指")),
        (56, TableItem::Text("膝")),
        (61, TableItem::Text("社会的信用")),
        (62, TableItem::Text("人間関係")),
        (63, TableItem::Text("初恋の思い出")),
        (64, TableItem::Text("完璧なはずの予測")),
        (65, TableItem::Text("心")),
        (66, TableItem::Text("眼鏡")),
    ],
);

/// i18n `LogHorizon.table.PCNM`。
static JA_TABLE_PCNM: D66Table = D66Table::new(
    "PC名",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("Kuraudo")),
        (12, TableItem::Text("Sefirosu")),
        (13, TableItem::Text("Kirito")),
        (14, TableItem::Text("Asuna")),
        (15, TableItem::Text("Leeroy Jenkins")),
        (16, TableItem::Text("Buront")),
        (21, TableItem::Text("水瀬陽夢")),
        (22, TableItem::Text("三宝寺吾郎")),
        (23, TableItem::Text("奈流麗夢")),
        (24, TableItem::Text("フランチャイズ竜崎")),
        (25, TableItem::Text("太宰治")),
        (26, TableItem::Text("ねざ")),
        (31, TableItem::Text("クロウ・リー")),
        (32, TableItem::Text("ダィテス")),
        (33, TableItem::Text("達也")),
        (34, TableItem::Text("深雪")),
        (35, TableItem::Text("スレイ")),
        (36, TableItem::Text("タカぼんさん")),
        (41, TableItem::Text("黒の錬金術士")),
        (42, TableItem::Text("†愛天使猫姫†")),
        (43, TableItem::Text("デス★ガン")),
        (44, TableItem::Text("卍漆黒の堕天使卍")),
        (
            45,
            TableItem::Text("光速の異名を持ち重力を自在に操る高貴なる騎士"),
        ),
        (46, TableItem::Text("新世界†英傑殺し")),
        (51, TableItem::Text("ろぐほら")),
        (52, TableItem::Text("ああああ")),
        (53, TableItem::Text("そうこ")),
        (54, TableItem::Text("ぎんこう")),
        (55, TableItem::Text("あずかり")),
        (56, TableItem::Text("もょもと")),
        (61, TableItem::Text("サスケ")),
        (62, TableItem::Text("ラファエロ")),
        (63, TableItem::Text("ドナテロ")),
        (64, TableItem::Text("ミケランジェロ")),
        (65, TableItem::Text("川内")),
        (66, TableItem::Text("フジキド")),
    ],
);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static JA_TABLES: &[(&str, &D66Table)] = &[
    ("PTAG", &JA_TABLE_PTAG),
    ("KOYU", &JA_TABLE_KOYU),
    ("MGR1", &JA_TABLE_MGR1),
    ("MGR2", &JA_TABLE_MGR2),
    ("MGR3", &JA_TABLE_MGR3),
    ("HLOC", &JA_TABLE_HLOC),
    ("PCNM", &JA_TABLE_PCNM),
];

/// i18n `LogHorizon.TRS.below_lower_limit`（`ko_kr` に訳が無くフォールバックされる）。
pub(crate) const JA_TRS_BELOW_LOWER_LIMIT: &str = "%{value}以下の出目は未定義です";

/// i18n `LogHorizon.TRS.exceed_upper_limit`（同上）。
pub(crate) const JA_TRS_EXCEED_UPPER_LIMIT: &str = "%{value}以上の出目は未定義です";

/// i18n `LogHorizon.TRS.need_cr`（同上）。
pub(crate) const JA_TRS_NEED_CR: &str = "%{command} ＞ CRを指定してください";

/// `ja_jp` ロケールの表と定型文。
static JA_SYSTEM: SystemTables = SystemTables {
    lh_critical: "クリティカル！",
    lh_fumble: "ファンブル！",
    success: "成功",
    failure: "失敗",
    ct: JA_CT,
    trs: JA_TRS,
    trse: JA_TRSE,
    below_lower_limit: JA_TRS_BELOW_LOWER_LIMIT,
    exceed_upper_limit: JA_TRS_EXCEED_UPPER_LIMIT,
    need_cr: JA_TRS_NEED_CR,
    iat_name: "ロデ研の新発明",
    iat_a: &JA_IAT_A,
    iat_b: &JA_IAT_B,
    iat_l: &JA_IAT_L,
    iat_t: &JA_IAT_T,
    tias: &JA_TIAS,
    abdc: &JA_ABDC,
    mii: &JA_MII,
    estl: &JA_ESTL,
    tables: JA_TABLES,
};

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "LogHorizon",
            "LogHorizon.toml",
            268,
        );
    }
}
