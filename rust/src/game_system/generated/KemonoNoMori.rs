//! P4で手書き移植した `lib/bcdice/game_system/KemonoNoMori.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `check_1D12`（行為判定 `KAx±y` / 継続判定 `KCx±y`）
//! - `get_trap_result`（罠動作チェック+獲物表 `CTR`）
//! - `get_escape_experience_table_result`（逃走体験表 `EET`）
//! - `TABLES`（各種表20件の `RangeTable` と `WST` の `Table`）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("KemonoNoMori.…", locale:)` で `i18n/KemonoNoMori/ja_jp.yml` から
//! 表と定型文を作る。Rust側は同じ値を `static` として直接持つ。データ部分
//! （`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`KemonoNoMori_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `KemonoNoMori_Korean < KemonoNoMori` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::{self};
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::KemonoNoMori`（ID: `KemonoNoMori`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KemonoNoMori;

impl GameSystem for KemonoNoMori {
    fn id(&self) -> &'static str {
        "KemonoNoMori"
    }

    fn name(&self) -> &'static str {
        "獸ノ森"
    }

    fn sort_key(&self) -> &'static str {
        "けもののもり"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定(成功度自動算出)(P119): KAx[±y]
・継続判定(成功度+1固定): KCx[±y]
   x=目標値
   y=目標値への修正(任意) x+y-z のように複数指定可能
     例1）KA7+3 → 目標値7にプラス3の修正を加えた行為判定
     例2）KC6 → 目標値6の継続判定
・罠動作チェック+獲物表(P163): CTR
   罠ごとに1D12を振り、12が出た場合には生き物が罠を動作させ、その影響を受けている。
・各種表（基本ルールブック）
  ・大失敗表(P120): FT
  ・能力値ランダム決定表(P121): RST
  ・ランダム所要時間表(P122): RTT
  ・ランダム消耗表(P122): RET
  ・ランダム天気表(P128): RWT
  ・ランダム天気持続表(P128): RWDT
  ・ランダム遮蔽物表（屋外）(P140): ROMT
  ・ランダム遮蔽物表（屋内）(P140): RIMT
  ・逃走体験表(P144): EET
  ・食材採集表(P157): GFT
  ・水採集表(P157): GWT
  ・白の魔石効果表(P186): WST
・部位ダメージ関連の表（参照先ページはリプレイ&データブック「嚙神ノ宴」のもの）
  ・人間部位表(P216): HPT
  ・部位ダメージ段階表(P217): PDT
  ・四足動物部位表(P225): QPT
  ・無足動物部位表(P225): APT
  ・二足動物部位表(P226): TPT
  ・鳥部位表(P226): BPT
  ・頭足動物部位表(P227): CPT
  ・昆虫部位表(P227): IPT
  ・蜘蛛部位表(P228): SPT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "K[AC]", "CTR", "FT", "RST", "RTT", "RET", "RWT", "RWDT", "ROMT", "RIMT", "EET", "GFT",
            "GWT", "WST", "HPT", "PDT", "QPT", "APT", "TPT", "BPT", "CPT", "IPT", "SPT",
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
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// Ruby `TABLES` の値。`RangeTable`（20件）と `Table`（`WST`）が混在する。
///
/// どちらも `to_s` は `"表名(値) ＞ 内容"` なので、`roll_tables` からは文字列で揃えて扱う。
pub(crate) enum TableRef {
    /// Ruby `DiceTable::RangeTable`
    Range(&'static RangeTable),
    /// Ruby `DiceTable::Table`
    Plain(&'static Table),
}

impl TableRef {
    /// Ruby `table.roll(@randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            TableRef::Range(table) => Ok(table.roll(rng)?.to_string()),
            TableRef::Plain(table) => Ok(table.roll(rng)?.to_string()),
        }
    }
}

/// i18n `KemonoNoMori.trap_activated_*` の整形（`%{check_num}` と `%{chase_num}` を埋める）。
pub(crate) type TrapFormatter = fn(check_num: i64, chase_num: i64) -> String;

/// 1ロケール分の表と定型文。`KemonoNoMori` と `KemonoNoMori_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, TableRef)],
    /// i18n `KemonoNoMori.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `KemonoNoMori.critical`（`%{success_degree}` を埋める）
    pub(crate) critical: fn(success_degree: i64) -> String,
    /// i18n `KemonoNoMori.success`（`%{success_degree}` を埋める）
    pub(crate) success: fn(success_degree: i64) -> String,
    /// i18n `failure`（共通のロケールファイル）
    pub(crate) failure: &'static str,
    /// i18n `KemonoNoMori.trap_not_activated`
    pub(crate) trap_not_activated: fn(check_num: i64) -> String,
    /// i18n `KemonoNoMori.trap_activated_small`（獲物表 1〜4）
    pub(crate) trap_activated_small: TrapFormatter,
    /// i18n `KemonoNoMori.trap_activated_large`（獲物表 5〜8）
    pub(crate) trap_activated_large: TrapFormatter,
    /// i18n `KemonoNoMori.trap_activated_human`（獲物表 9〜12）
    pub(crate) trap_activated_human: TrapFormatter,
    /// i18n `KemonoNoMori.reappear`（`%{hours}` を埋める）
    pub(crate) reappear: fn(hours: i64) -> String,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `KemonoNoMori#eval_game_system_specific_command`。
///
/// Ruby は `case command when /KA\d[-+\d]*/ …` なので、いずれの分岐も**部分一致**で判定する。
pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if action_judge_pattern().is_match(command) {
        return Ok(check_1d12(tables, command, true, rng)?.map(SpecificCommandOutput::result));
    }
    if continuous_judge_pattern().is_match(command) {
        return Ok(check_1d12(tables, command, false, rng)?.map(SpecificCommandOutput::result));
    }
    if command == "CTR" {
        return Ok(Some(SpecificCommandOutput::result(get_trap_result(
            tables, rng,
        )?)));
    }
    if command == "EET" {
        return Ok(get_escape_experience_table_result(tables, command, rng)?
            .map(SpecificCommandOutput::result));
    }
    Ok(roll_tables(tables, command, rng)?.map(SpecificCommandOutput::text))
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
    Ok(Some(table.roll_text(rng)?))
}

/// Ruby `case command when /KA\d[-+\d]*/`（行為判定）。
fn action_judge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"KA\d[-+\d]*").expect("valid regex"))
}

/// Ruby `case command when /KC\d[-+\d]*/`（継続判定）。
fn continuous_judge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"KC\d[-+\d]*").expect("valid regex"))
}

/// Ruby `check_1D12` が目標値を取り出す正規表現。
fn target_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"K[AC](\d[-+\d]*)").expect("valid regex"))
}

/// Ruby `check_1D12(command, is_action_judge)`。
fn check_1d12(
    tables: &SystemTables,
    command: &str,
    is_action_judge: bool,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = target_pattern().captures(command) else {
        return Ok(None);
    };

    // 修正込みの目標値を計算。Ruby: ArithmeticEvaluator.eval(m[1])
    let target_total = arithmetic::eval(&m[1], RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    // 行為判定の成功度は [目標値の10の位の数+1]、継続判定の成功度は固定で+1。
    // Ruby の `Integer#/` は床除算なので、目標値が負になる入力でも切り捨て方向を合わせる。
    let success_degree = if is_action_judge {
        (target_total).div_euclid(10) + 1
    } else {
        1
    };

    let dice_total = rng.roll_once(12)?;
    let head = format!("(1D12<={target_total}) ＞ {dice_total} ＞ ");

    let result = if dice_total == 12 {
        EvalResult::fumble(format!("{head}{}", tables.fumble))
    } else if dice_total == 11 {
        EvalResult::critical(format!("{head}{}", (tables.critical)(success_degree)))
    } else if dice_total <= target_total {
        EvalResult::success(format!("{head}{}", (tables.success)(success_degree)))
    } else {
        EvalResult::failure(format!("{head}{}", tables.failure))
    };

    Ok(Some(result))
}

/// Ruby `get_trap_result`（罠動作チェック+獲物表 `CTR`）。
fn get_trap_result(tables: &SystemTables, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let tra_check_num = rng.roll_once(12)?;
    if tra_check_num != 12 {
        return Ok(EvalResult::with_text((tables.trap_not_activated)(
            tra_check_num,
        )));
    }

    let chase_num = rng.roll_once(12)?;
    // Ruby: case chase_num when 1..4 / 5..8 / 9..12。
    // `roll_once(12)` は 1〜12 しか返さないので、それ以外は Ruby でも chase_key が nil に
    // なって例外になる枝。ここでは 9..12 と同じ扱いにまとめている。
    let format_trap = match chase_num {
        1..=4 => tables.trap_activated_small,
        5..=8 => tables.trap_activated_large,
        _ => tables.trap_activated_human,
    };

    Ok(EvalResult::with_text(format_trap(tra_check_num, chase_num)))
}

/// Ruby `get_escape_experience_table_result`（逃走体験表 `EET`）。
///
/// 表を引いてから再登場までの時間を1D12で振るので、消費する乱数は「1D12 → 1D12」の順。
fn get_escape_experience_table_result(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    // 呼び出し元が `command == "EET"` を確かめているので、表は必ず見つかる。
    let Some(escape_experience) = roll_tables(tables, command, rng)? else {
        return Ok(None);
    };
    let escape_duration = rng.roll_once(12)?;

    Ok(Some(EvalResult::with_text(format!(
        "{escape_experience} ({})",
        (tables.reappear)(escape_duration)
    ))))
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの定型文
// ---------------------------------------------------------------------------

/// i18n `ja_jp.KemonoNoMori.critical`。
fn ja_critical(success_degree: i64) -> String {
    format!("大成功（成功度+{success_degree}, 次の継続判定の目標値を10に変更）")
}

/// i18n `ja_jp.KemonoNoMori.success`。
fn ja_success(success_degree: i64) -> String {
    format!("成功（成功度+{success_degree}）")
}

/// i18n `ja_jp.KemonoNoMori.trap_not_activated`。
fn ja_trap_not_activated(check_num: i64) -> String {
    format!("罠動作チェック(1D12) ＞ {check_num} ＞ 罠は動作していなかった")
}

/// i18n `ja_jp.KemonoNoMori.trap_activated_small`。
fn ja_trap_activated_small(check_num: i64, chase_num: i64) -> String {
    format!("罠動作チェック(1D12) ＞ {check_num} ＞ 罠が動作していた！ ＞ 獲物表({chase_num}) ＞ 小型動物が罠にかかっていた")
}

/// i18n `ja_jp.KemonoNoMori.trap_activated_large`。
fn ja_trap_activated_large(check_num: i64, chase_num: i64) -> String {
    format!("罠動作チェック(1D12) ＞ {check_num} ＞ 罠が動作していた！ ＞ 獲物表({chase_num}) ＞ 大型動物が罠にかかっていた")
}

/// i18n `ja_jp.KemonoNoMori.trap_activated_human`。
fn ja_trap_activated_human(check_num: i64, chase_num: i64) -> String {
    format!("罠動作チェック(1D12) ＞ {check_num} ＞ 罠が動作していた！ ＞ 獲物表({chase_num}) ＞ 人間の放浪者が罠にかかっていた")
}

/// i18n `ja_jp.KemonoNoMori.reappear`。
fn ja_reappear(hours: i64) -> String {
    format!("再登場: {hours}時間後")
}

// ---------------------------------------------------------------------------
// 表データ（i18n/KemonoNoMori/ja_jp.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `KemonoNoMori.table.FT.items`。
static JA_FT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "【余裕】が3点減少する（最低0まで）"),
    (RangeInc::new(4, 5), "ランダムな荷物1個が落ちて行方不明になる（大失敗したエリアのアイテム調査で見つけることが可能）"),
    (RangeInc::new(6, 7), "ランダムな荷物1個が破壊される"),
    (RangeInc::new(8, 9), "ランダム天気表(RWT)を使用し、結果をターンの終了まで適用する"),
    (RangeInc::new(10, 10), "ランダムな準備している小道具1個が破壊される"),
    (RangeInc::new(11, 11), "着装している防具が破壊される"),
    (RangeInc::new(12, 12), "準備している武器が破壊される"),
];

/// Ruby `TABLES["FT"]`（`1D12`）。
static JA_FT: RangeTable = RangeTable::from_dice("大失敗表", 1, 12, JA_FT_ITEMS);

/// i18n `KemonoNoMori.table.RST.items`。
static JA_RST_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "【移動】"),
    (RangeInc::new(3, 4), "【格闘】"),
    (RangeInc::new(5, 6), "【射撃】"),
    (RangeInc::new(7, 8), "【製作】"),
    (RangeInc::new(9, 10), "【察知】"),
    (RangeInc::new(11, 12), "【自制】"),
];

/// Ruby `TABLES["RST"]`（`1D12`）。
static JA_RST: RangeTable = RangeTable::from_dice("能力値ランダム決定表", 1, 12, JA_RST_ITEMS);

/// i18n `KemonoNoMori.table.RTT.items`。
static JA_RTT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "2"),
    (RangeInc::new(4, 6), "3"),
    (RangeInc::new(7, 9), "4"),
    (RangeInc::new(10, 12), "5"),
];

/// Ruby `TABLES["RTT"]`（`1D12`）。
static JA_RTT: RangeTable = RangeTable::from_dice("ランダム所要時間表", 1, 12, JA_RTT_ITEMS);

/// i18n `KemonoNoMori.table.RET.items`。
static JA_RET_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "0"),
    (RangeInc::new(4, 6), "1"),
    (RangeInc::new(7, 9), "2"),
    (RangeInc::new(10, 12), "4"),
];

/// Ruby `TABLES["RET"]`（`1D12`）。
static JA_RET: RangeTable = RangeTable::from_dice("ランダム消耗表", 1, 12, JA_RET_ITEMS);

/// i18n `KemonoNoMori.table.RWT.items`。
static JA_RWT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "濃霧"),
    (RangeInc::new(3, 4), "大雨"),
    (RangeInc::new(5, 6), "雷雨"),
    (RangeInc::new(7, 8), "強風"),
    (RangeInc::new(9, 10), "酷暑"),
    (RangeInc::new(11, 12), "極寒"),
];

/// Ruby `TABLES["RWT"]`（`1D12`）。
static JA_RWT: RangeTable = RangeTable::from_dice("ランダム天気表", 1, 12, JA_RWT_ITEMS);

/// i18n `KemonoNoMori.table.RWDT.items`。
static JA_RWDT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "1ターン"),
    (RangeInc::new(3, 4), "3ターン"),
    (RangeInc::new(5, 6), "6ターン"),
    (RangeInc::new(7, 8), "24ターン"),
    (RangeInc::new(9, 10), "72ターン"),
    (RangeInc::new(11, 12), "156ターン"),
];

/// Ruby `TABLES["RWDT"]`（`1D12`）。
static JA_RWDT: RangeTable = RangeTable::from_dice("ランダム天気持続表", 1, 12, JA_RWDT_ITEMS);

/// i18n `KemonoNoMori.table.ROMT.items`。
static JA_ROMT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "【藪】耐久度3,軽減値1,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-1の修正を付加"),
    (RangeInc::new(3, 5), "【木】耐久度5,軽減値2,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-1の修正を付加"),
    (RangeInc::new(6, 8), "【大木】耐久度7,軽減値3,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-2の修正を付加"),
    (RangeInc::new(9, 10), "【岩】耐久度6,軽減値4,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-1の修正を付加/コンタクト内で行われる格闘攻撃のダメージ+1"),
    (RangeInc::new(11, 12), "【岩壁】耐久度8,軽減値4,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-2の修正を付加/コンタクト内で行われる格闘攻撃のダメージ+2"),
];

/// Ruby `TABLES["ROMT"]`（`1D12`）。
static JA_ROMT: RangeTable = RangeTable::from_dice("ランダム遮蔽物表(屋外)", 1, 12, JA_ROMT_ITEMS);

/// i18n `KemonoNoMori.table.RIMT.items`。
static JA_RIMT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 4), "【木材の壁】耐久度4,軽減値2,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-1の修正を付加"),
    (RangeInc::new(5, 8), "【木材の扉】耐久度4,軽減値2,特殊効果:コンタクト内のキャラクターに対する射撃攻撃判定に-1、接触判定と突撃判定に-2の修正を付加"),
    (RangeInc::new(9, 12), "【木製家具】耐久度3,軽減値2,特殊効果:コンタクト内で行われる格闘攻撃のダメージ+1"),
];

/// Ruby `TABLES["RIMT"]`（`1D12`）。
static JA_RIMT: RangeTable = RangeTable::from_dice("ランダム遮蔽物表(屋内)", 1, 12, JA_RIMT_ITEMS);

/// i18n `KemonoNoMori.table.EET.items`。
static JA_EET_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "【余裕】が0になる"),
    (RangeInc::new(4, 6), "任意の【絆】を合計2点減少する"),
    (
        RangeInc::new(7, 9),
        "全ての荷物を失う（逃走したエリアに配置され、調査で発見可能）",
    ),
    (
        RangeInc::new(10, 12),
        "全ての武器と防具と小道具と荷物を失う（逃走したエリアに配置され、調査で発見可能）",
    ),
];

/// Ruby `TABLES["EET"]`（`1D12`）。
static JA_EET: RangeTable = RangeTable::from_dice("逃走体験表", 1, 12, JA_EET_ITEMS);

/// i18n `KemonoNoMori.table.GFT.items`。
static JA_GFT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "食べられる根（栄養価:2）"),
    (RangeInc::new(3, 5), "食べられる草（栄養価:3）"),
    (RangeInc::new(6, 8), "食べられる実（栄養価:5）"),
    (RangeInc::new(9, 10), "小型動物（栄養価:10）"),
    (RangeInc::new(11, 11), "大型動物（栄養価:40）"),
    (RangeInc::new(12, 12), "気持ち悪い虫（栄養価:1）"),
];

/// Ruby `TABLES["GFT"]`（`1D12`）。
static JA_GFT: RangeTable = RangeTable::from_dice("食材採集表", 1, 12, JA_GFT_ITEMS);

/// i18n `KemonoNoMori.table.GWT.items`。
static JA_GWT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 6), "汚水"),
    (RangeInc::new(7, 11), "飲料水"),
    (RangeInc::new(12, 12), "毒水"),
];

/// Ruby `TABLES["GWT"]`（`1D12`）。
static JA_GWT: RangeTable = RangeTable::from_dice("水採集表", 1, 12, JA_GWT_ITEMS);

/// i18n `KemonoNoMori.table.WST.items`。
static JA_WST_ITEMS: &[&str] = &[
    "役に立たないものの色を変える",
    "役に立たないものを大きくする",
    "役に立たないものを小さくする",
    "役に立たないものを保存する",
    "役に立たないものを復元する",
    "役に立たないものを召喚する",
    "役に立たないものを動かす",
    "役に立たないものを増やす",
    "役に立たないものを貼り付ける",
    "役に立たないものを作り出す",
    "小型動物を召喚する",
    "大型動物を召喚する",
];

/// Ruby `TABLES["WST"]`（`1D12`）。
static JA_WST: Table = Table::from_dice("白の魔石効果表", 1, 12, JA_WST_ITEMS);

/// i18n `KemonoNoMori.table.HPT.items`。
static JA_HPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "右腕部"),
    (RangeInc::new(3, 4), "左腕部"),
    (RangeInc::new(5, 6), "右脚部"),
    (RangeInc::new(7, 8), "左脚部"),
    (RangeInc::new(9, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["HPT"]`（`1D12`）。
static JA_HPT: RangeTable = RangeTable::from_dice("人間部位表", 1, 12, JA_HPT_ITEMS);

/// i18n `KemonoNoMori.table.PDT.items`。
static JA_PDT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 6), "軽傷"),
    (RangeInc::new(7, 10), "重傷"),
    (RangeInc::new(11, 11), "破壊"),
    (RangeInc::new(12, 12), "喪失"),
];

/// Ruby `TABLES["PDT"]`（`1D12`）。
static JA_PDT: RangeTable = RangeTable::from_dice("部位ダメージ段階表", 1, 12, JA_PDT_ITEMS);

/// i18n `KemonoNoMori.table.QPT.items`。
static JA_QPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "異形"),
    (RangeInc::new(3, 3), "武器"),
    (RangeInc::new(4, 4), "右前脚部"),
    (RangeInc::new(5, 5), "左前脚部"),
    (RangeInc::new(6, 6), "右後脚部"),
    (RangeInc::new(7, 7), "左後脚部"),
    (RangeInc::new(8, 10), "胴部"),
    (RangeInc::new(11, 12), "頭部"),
];

/// Ruby `TABLES["QPT"]`（`1D12`）。
static JA_QPT: RangeTable = RangeTable::from_dice("四足動物部位表", 1, 12, JA_QPT_ITEMS);

/// i18n `KemonoNoMori.table.APT.items`。
static JA_APT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "異形"),
    (RangeInc::new(4, 6), "武器"),
    (RangeInc::new(7, 10), "胴部"),
    (RangeInc::new(11, 12), "頭部"),
];

/// Ruby `TABLES["APT"]`（`1D12`）。
static JA_APT: RangeTable = RangeTable::from_dice("無足動物部位表", 1, 12, JA_APT_ITEMS);

/// i18n `KemonoNoMori.table.TPT.items`。
static JA_TPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "異形"),
    (RangeInc::new(2, 2), "武器"),
    (RangeInc::new(3, 3), "右腕部"),
    (RangeInc::new(4, 4), "左腕部"),
    (RangeInc::new(5, 6), "右脚部"),
    (RangeInc::new(7, 8), "左脚部"),
    (RangeInc::new(9, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["TPT"]`（`1D12`）。
static JA_TPT: RangeTable = RangeTable::from_dice("二足動物部位表", 1, 12, JA_TPT_ITEMS);

/// i18n `KemonoNoMori.table.BPT.items`。
static JA_BPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "異形"),
    (RangeInc::new(2, 2), "武器"),
    (RangeInc::new(3, 4), "右翼(右腕部)"),
    (RangeInc::new(5, 6), "左翼(左腕部)"),
    (RangeInc::new(7, 7), "右脚部"),
    (RangeInc::new(8, 8), "左脚部"),
    (RangeInc::new(9, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["BPT"]`（`1D12`）。
static JA_BPT: RangeTable = RangeTable::from_dice("鳥部位表", 1, 12, JA_BPT_ITEMS);

/// i18n `KemonoNoMori.table.CPT.items`。
static JA_CPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "異形"),
    (RangeInc::new(2, 2), "武器"),
    (RangeInc::new(3, 3), "右腕部"),
    (RangeInc::new(4, 4), "左腕部"),
    (RangeInc::new(5, 7), "右脚部"),
    (RangeInc::new(8, 10), "左脚部"),
    (RangeInc::new(11, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["CPT"]`（`1D12`）。
static JA_CPT: RangeTable = RangeTable::from_dice("頭足動物部位表", 1, 12, JA_CPT_ITEMS);

/// i18n `KemonoNoMori.table.IPT.items`。
static JA_IPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "異形"),
    (RangeInc::new(3, 3), "武器"),
    (RangeInc::new(4, 4), "右前脚部"),
    (RangeInc::new(5, 5), "左前脚部"),
    (RangeInc::new(6, 6), "右中脚部"),
    (RangeInc::new(7, 7), "左中脚部"),
    (RangeInc::new(8, 8), "右後脚部"),
    (RangeInc::new(9, 9), "左後脚部"),
    (RangeInc::new(10, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["IPT"]`（`1D12`）。
static JA_IPT: RangeTable = RangeTable::from_dice("昆虫部位表", 1, 12, JA_IPT_ITEMS);

/// i18n `KemonoNoMori.table.SPT.items`。
static JA_SPT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "異形"),
    (RangeInc::new(2, 2), "武器"),
    (RangeInc::new(3, 3), "右第一脚部"),
    (RangeInc::new(4, 4), "左第一脚部"),
    (RangeInc::new(5, 5), "右第二脚部"),
    (RangeInc::new(6, 6), "左第二脚部"),
    (RangeInc::new(7, 7), "右第三脚部"),
    (RangeInc::new(8, 8), "左第三脚部"),
    (RangeInc::new(9, 9), "右第四脚部"),
    (RangeInc::new(10, 10), "左第四脚部"),
    (RangeInc::new(11, 11), "胴部"),
    (RangeInc::new(12, 12), "頭部"),
];

/// Ruby `TABLES["SPT"]`（`1D12`）。
static JA_SPT: RangeTable = RangeTable::from_dice("蜘蛛部位表", 1, 12, JA_SPT_ITEMS);

/// Ruby `TABLES`（`general_tables` → `field_tables` → `body_part_tables` のマージ順）。
static JA_TABLES: &[(&str, TableRef)] = &[
    ("FT", TableRef::Range(&JA_FT)),
    ("RST", TableRef::Range(&JA_RST)),
    ("RTT", TableRef::Range(&JA_RTT)),
    ("RET", TableRef::Range(&JA_RET)),
    ("RWT", TableRef::Range(&JA_RWT)),
    ("RWDT", TableRef::Range(&JA_RWDT)),
    ("ROMT", TableRef::Range(&JA_ROMT)),
    ("RIMT", TableRef::Range(&JA_RIMT)),
    ("EET", TableRef::Range(&JA_EET)),
    ("GFT", TableRef::Range(&JA_GFT)),
    ("GWT", TableRef::Range(&JA_GWT)),
    ("WST", TableRef::Plain(&JA_WST)),
    ("HPT", TableRef::Range(&JA_HPT)),
    ("PDT", TableRef::Range(&JA_PDT)),
    ("QPT", TableRef::Range(&JA_QPT)),
    ("APT", TableRef::Range(&JA_APT)),
    ("TPT", TableRef::Range(&JA_TPT)),
    ("BPT", TableRef::Range(&JA_BPT)),
    ("CPT", TableRef::Range(&JA_CPT)),
    ("IPT", TableRef::Range(&JA_IPT)),
    ("SPT", TableRef::Range(&JA_SPT)),
];

/// `ja_jp` ロケールの表と定型文。
static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    fumble: "大失敗",
    critical: ja_critical,
    success: ja_success,
    failure: "失敗",
    trap_not_activated: ja_trap_not_activated,
    trap_activated_small: ja_trap_activated_small,
    trap_activated_large: ja_trap_activated_large,
    trap_activated_human: ja_trap_activated_human,
    reappear: ja_reappear,
};

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/KemonoNoMori.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// Ruby `RangeTable#store` が構築時に行う検査（隙間・重なり・端の被覆）。
    #[test]
    fn range_tables_are_complete() {
        for (name, table) in JA_TABLES {
            let TableRef::Range(table) = table else {
                continue;
            };
            assert_eq!(table.validate(), Ok(()), "{name}");
        }
    }

    /// `test/data/KemonoNoMori.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/KemonoNoMori.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KemonoNoMori.toml must parse");
        assert_eq!(
            data.tests.len(),
            89,
            "case count in test/data/KemonoNoMori.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KemonoNoMori",
                "unexpected game system in KemonoNoMori.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KemonoNoMori"), &tc.input, &mut src) {
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
                    "FAIL KemonoNoMori:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KemonoNoMori cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
