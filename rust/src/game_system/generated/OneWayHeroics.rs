//! P4で手書き移植した `lib/bcdice/game_system/OneWayHeroics.rb` と
//! `lib/bcdice/game_system/one_way_heroics/*.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `OneWayHeroics#eval_game_system_specific_command`
//!   （`RETx` / `RETPx` / `DNGNx` / `DNGNPx` / `aJDx+y,z` / 各種表）
//! - `#getRollDiceCommandResult` / `#rollJudgeDice` / `#getJudgeReusltText`
//! - `one_way_heroics/tables.rb` の `TABLES` と補助クラス
//!   （`GoldFlow` / `StatusDown` / `MoveToTable`）
//! - `one_way_heroics/dungeon_table.rb` の `DungeonTable`
//! - `one_way_heroics/random_event_table.rb` の `RandomEventTable` /
//!   `BranchByElapsedDays` / `BranchByDayParity` / `MoveToTableWithDay`
//!
//! # 表データ
//!
//! Ruby側の表はクラス定数として組み立てられる（i18n未対応）。Rust側も同じ値を
//! `static` として持つ。データ部分（`T_` 接頭辞の `static` 群と `DUNGEON_TABLE*` /
//! `RANDOM_EVENT_TABLE*`）は上記rbから機械的に書き出したもので、値は1文字も変えていない。
//!
//! # 表の型
//!
//! `MoveToTable` などの項目は「振ると**文字列**を返す」ものなので、
//! 共有の [`crate::dice_table::ChainTable`] / [`crate::dice_table::D66Table`]
//! （項目が `RollResult` を返す前提）には載らない。ここでは Ruby と同じく
//! 「`to_s` 後の文字列」で揃えた専用の表型を持つ。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// 表の部品
// ---------------------------------------------------------------------------

/// 表の項目。Ruby の `Array<String, #roll>` に対応する。
///
/// `GoldFlow` / `StatusDown` / `MoveToTable` は Ruby ではクラスだが、
/// 状態を持たない値なので enum のヴァリアントとして持つ（`static` に直接書ける）。
enum Item {
    /// 文字列の項目
    Text(&'static str),
    /// Ruby `GoldFlow`
    GoldFlow { times: i64, action: &'static str },
    /// Ruby `StatusDown`
    StatusDown { status: &'static str, times: i64 },
    /// Ruby `MoveToTable`
    MoveToTable {
        text: &'static str,
        table_key: &'static str,
    },
}

impl Item {
    /// Ruby `chosen = chosen.roll(randomizer) if chosen.respond_to?(:roll)`。
    fn resolve(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        match *self {
            Item::Text(text) => Ok(text.to_owned()),
            Item::GoldFlow { times, action } => {
                // Ruby `GoldFlow#roll`
                let dice_list = rng.roll_barabara(times, 6)?;
                let dice_total: i64 = dice_list.iter().sum();
                let gold = dice_total * 100;
                Ok([
                    format!("{times}D6に100を掛け、それだけの【所持金】を{action}"),
                    format!("{times}D6[{}]*100", dice_text::join_dice(&dice_list)),
                    format!("【所持金】{gold} を{action}"),
                ]
                .join(" ＞ "))
            }
            Item::StatusDown { status, times } => {
                // Ruby `StatusDown#roll`
                let dice_list = rng.roll_barabara(times, 6)?;
                let total: i64 = dice_list.iter().sum();
                Ok([
                    format!("{status}が{times}D6減少する"),
                    format!("{times}D6[{}]", dice_text::join_dice(&dice_list)),
                    format!("{status}が {total} 減少する"),
                ]
                .join(" ＞ "))
            }
            Item::MoveToTable { text, table_key } => move_to_table(text, table_key, rng),
        }
    }
}

/// Ruby `MoveToTable#roll`。
fn move_to_table(
    text: &'static str,
    table_key: &'static str,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let table = table_by_key(table_key)?;
    Ok(format!(
        "{text} ＞\n {table_key} ＞ {}",
        table.roll_text(rng)?
    ))
}

/// Ruby `TABLES[key]`。
///
/// Ruby は未登録キーだと `nil.roll` でクラッシュする。`TABLES` に無いキーを
/// 参照している表はないので、ここに来たら移植のバグとして表面化させる。
fn table_by_key(key: &str) -> Result<&'static TableRef, EvalError> {
    TABLES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, t)| t)
        .ok_or(EvalError::Internal("OneWayHeroics: unknown table key"))
}

/// Ruby `TABLES` の値。`DiceTable::Table` / `ChainTable` / `D66Table` /
/// `RangeTable` が混在するので、`to_s` 後の文字列で揃えて扱う。
enum TableRef {
    /// Ruby `DiceTable::Table`
    Plain(&'static Table),
    /// Ruby `DiceTable::RangeTable`
    Range(&'static RangeTable),
    /// Ruby `DiceTable::ChainTable`
    Chain(&'static ChainTable),
    /// Ruby `DiceTable::D66Table`
    D66(&'static D66Table),
}

impl TableRef {
    /// Ruby `table.roll(randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            TableRef::Plain(table) => Ok(table.roll(rng)?.to_string()),
            TableRef::Range(table) => Ok(table.roll(rng)?.to_string()),
            TableRef::Chain(table) => table.roll_text(rng),
            TableRef::D66(table) => table.roll_text(rng),
        }
    }
}

/// Ruby `DiceTable::ChainTable` 相当（項目が文字列を返す版）。
struct ChainTable {
    name: &'static str,
    times: i64,
    sides: i64,
    items: &'static [Item],
}

impl ChainTable {
    /// Ruby `ChainTable#roll(randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_sum(self.times, self.sides)?;
        let index = value - self.times;
        let body = match usize::try_from(index).ok().and_then(|i| self.items.get(i)) {
            Some(item) => item.resolve(rng)?,
            // Ruby: @items[index] が nil のとき "表名(値) ＞ " になる
            None => String::new(),
        };
        Ok(format!("{}({value}) ＞ {body}", self.name))
    }
}

/// Ruby `DiceTable::D66Table` 相当（項目が文字列を返す版）。
///
/// このゲームシステムの D66 表はすべて `D66SortType::ASC` なので、
/// 入れ替えは昇順固定で書いてある。
struct D66Table {
    name: &'static str,
    items: &'static [(i64, Item)],
}

impl D66Table {
    /// Ruby `D66Table#roll(randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let dice = rng.roll_barabara(2, 6)?;
        let (a, b) = (
            dice.first().copied().unwrap_or(0),
            dice.get(1).copied().unwrap_or(0),
        );
        // Ruby: D66SortType::ASC（小さい方が十の位）
        let key = a.min(b) * 10 + a.max(b);

        let body = match self.items.iter().find(|(k, _)| *k == key) {
            Some((_, item)) => item.resolve(rng)?,
            None => String::new(),
        };
        Ok(format!("{}({key}) ＞ {body}", self.name))
    }
}

// ---------------------------------------------------------------------------
// 日数つきの表
// ---------------------------------------------------------------------------

/// Ruby `OneWayHeroics::DungeonTable`。
struct DungeonTable {
    name: &'static str,
    key: &'static str,
    times: i64,
    sides: i64,
    items: &'static [&'static str],
}

impl DungeonTable {
    /// Ruby `DungeonTable#roll_with_day`。
    fn roll_with_day(&self, day: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
        let mut value = rng.roll_sum(self.times, self.sides)?;
        // Ruby: value += @times if day >= 4
        if day >= 4 {
            value += self.times;
        }

        let index = value - self.times;
        let body = usize::try_from(index)
            .ok()
            .and_then(|i| self.items.get(i))
            .copied()
            .unwrap_or("");
        Ok(format!("{}({value}) ＞ {body}", self.name))
    }
}

/// Ruby `BranchByDay` の分岐先。文字列か `DungeonTable`。
enum BranchTarget {
    /// Ruby の文字列項目。ASCIIのみなら `TABLES` のキー、そうでなければそのまま出力する。
    Text(&'static str),
    /// Ruby `DungeonTable`（`roll_with_day` を持つ）
    Dungeon(&'static DungeonTable),
}

/// Ruby `BranchByElapsedDays` / `BranchByDayParity` の分岐条件。
enum BranchKind {
    /// Ruby `BranchByElapsedDays`（出目が日数を超えるか）
    ElapsedDays,
    /// Ruby `BranchByDayParity`（出目の偶奇）
    DayParity,
}

/// Ruby `BranchByDay` とそのサブクラス。
struct Branch {
    kind: BranchKind,
    text: &'static str,
    less_than_equal: BranchTarget,
    greater: BranchTarget,
}

impl Branch {
    /// Ruby `BranchByDay#choice`。
    fn choice(&self, value: i64, day: i64) -> &BranchTarget {
        let greater = match self.kind {
            BranchKind::ElapsedDays => value > day,
            BranchKind::DayParity => value % 2 != 0,
        };
        if greater {
            &self.greater
        } else {
            &self.less_than_equal
        }
    }

    /// Ruby `BranchByDay#branch_result`。
    fn branch_result(&self, value: i64, day: i64) -> String {
        match self.kind {
            BranchKind::ElapsedDays => {
                if value > day {
                    format!("日数[{day}]を超えている")
                } else {
                    format!("日数[{day}]以下")
                }
            }
            BranchKind::DayParity => {
                if value % 2 != 0 {
                    "奇数".to_owned()
                } else {
                    "偶数".to_owned()
                }
            }
        }
    }

    /// Ruby `BranchByDay#roll_with_day`。
    fn roll_with_day(&self, day: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_once(6)?;

        let chosen = match self.choice(value, day) {
            // Ruby: chosen.respond_to?(:roll_with_day)
            BranchTarget::Dungeon(table) => {
                format!("{}{day} ＞ {}", table.key, table.roll_with_day(day, rng)?)
            }
            // Ruby: chosen.ascii_only?（表のキーなら引く）
            BranchTarget::Text(text) if text.is_ascii() => {
                format!("{text} ＞ {}", table_by_key(text)?.roll_text(rng)?)
            }
            BranchTarget::Text(text) => (*text).to_owned(),
        };

        Ok(format!(
            "{} ＞\n 1D6 ＞ {value} ＞ {} ＞\n {chosen}",
            self.text,
            self.branch_result(value, day)
        ))
    }
}

/// Ruby `RandomEventTable` の項目。
enum EventItem {
    /// 文字列の項目
    Text(&'static str),
    /// Ruby `BranchByElapsedDays` / `BranchByDayParity`
    Branch(Branch),
    /// Ruby `MoveToTable`（`roll` を持つので `respond_to?(:roll)` の枝）
    MoveToTable {
        text: &'static str,
        table_key: &'static str,
    },
    /// Ruby `MoveToTableWithDay`
    MoveToTableWithDay {
        text: &'static str,
        table: &'static DungeonTable,
    },
}

impl EventItem {
    /// Ruby `RandomEventTable#roll_with_day` の項目解決部分。
    ///
    /// 原典は `respond_to?(:roll)`（`MoveToTable`）→ `respond_to?(:roll_with_day)` の順に見る。
    fn resolve(&self, day: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            EventItem::Text(text) => Ok((*text).to_owned()),
            EventItem::MoveToTable { text, table_key } => move_to_table(text, table_key, rng),
            EventItem::Branch(branch) => branch.roll_with_day(day, rng),
            // Ruby `MoveToTableWithDay#roll_with_day`
            EventItem::MoveToTableWithDay { text, table } => Ok(format!(
                "{text} ＞\n {}{day} ＞ {}",
                table.key,
                table.roll_with_day(day, rng)?
            )),
        }
    }
}

/// Ruby `OneWayHeroics::RandomEventTable`。
struct RandomEventTable {
    name: &'static str,
    times: i64,
    sides: i64,
    items: &'static [EventItem],
}

impl RandomEventTable {
    /// Ruby `RandomEventTable#roll_with_day`。
    fn roll_with_day(&self, day: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_sum(self.times, self.sides)?;
        // Ruby: index = value - 1
        let index = value - 1;
        let body = match usize::try_from(index).ok().and_then(|i| self.items.get(i)) {
            Some(item) => item.resolve(day, rng)?,
            None => String::new(),
        };
        Ok(format!("{}({value}) ＞ {body}", self.name))
    }
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `/^RET(\d+)$/` など、日数つき表コマンドの正規表現。
fn day_command_pattern(prefix: &'static str) -> Regex {
    Regex::new(&format!(r"\A{prefix}(\d+)\z")).expect("valid regex")
}

/// Ruby `/^(\d*)JD(\d*)(\+(\d*))?(,(\d+))?$/`。
fn jd_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(\d*)JD(\d*)(\+(\d*))?(,(\d+))?\z").expect("valid regex"))
}

/// Ruby `/^\d*JD/`（振り分け用。末尾は固定しない）。
fn jd_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A\d*JD").expect("valid regex"))
}

/// Ruby `OneWayHeroics#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby の `case` と同じ順序で見る（`RET` は `RETP` より先に判定される）。
    static DAY_COMMANDS: OnceLock<Vec<(Regex, DayTable)>> = OnceLock::new();
    let day_commands = DAY_COMMANDS.get_or_init(|| {
        vec![
            (
                day_command_pattern("RET"),
                DayTable::Event(&RANDOM_EVENT_TABLE),
            ),
            (
                day_command_pattern("RETP"),
                DayTable::Event(&RANDOM_EVENT_TABLE_PLUS),
            ),
            (
                day_command_pattern("DNGN"),
                DayTable::Dungeon(&DUNGEON_TABLE),
            ),
            (
                day_command_pattern("DNGNP"),
                DayTable::Dungeon(&DUNGEON_TABLE_PLUS),
            ),
        ]
    });

    for (pattern, table) in day_commands {
        if let Some(m) = pattern.captures(command) {
            let day = to_i(&m[1]);
            return Ok(Some(SpecificCommandOutput::text(
                table.roll_with_day(day, rng)?,
            )));
        }
    }

    if jd_dispatch_pattern().is_match(command) {
        return Ok(get_roll_dice_command_result(command, rng)?.map(SpecificCommandOutput::text));
    }

    // Ruby: roll_tables(command, TABLES)
    let Ok(table) = table_by_key(command) else {
        return Ok(None);
    };
    Ok(Some(SpecificCommandOutput::text(table.roll_text(rng)?)))
}

/// 日数つきで引く表（`RETx` / `DNGNx` の振り分け先）。
enum DayTable {
    Event(&'static RandomEventTable),
    Dungeon(&'static DungeonTable),
}

impl DayTable {
    fn roll_with_day(&self, day: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            DayTable::Event(table) => table.roll_with_day(day, rng),
            DayTable::Dungeon(table) => table.roll_with_day(day, rng),
        }
    }
}

/// Ruby `OneWayHeroics#getRollDiceCommandResult`（判定 `aJDx+y,z`）。
fn get_roll_dice_command_result(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(m) = jd_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: diceCount = 2 if diceCount.empty?
    let dice_count_str = &m[1];
    let dice_count = if dice_count_str.is_empty() {
        2
    } else {
        to_i(dice_count_str)
    };
    if dice_count < 2 {
        return Ok(None);
    }

    let ability = to_i(&m[2]);
    let target = m.get(6).map(|x| to_i(x.as_str()));

    // Ruby: modifyText = m[3] || ""; modifyText = "+1" if modifyText == "+"
    let modify_text = match m.get(3).map(|x| x.as_str()) {
        Some("+") => "+1",
        Some(other) => other,
        None => "",
    };
    // Ruby: modifyText.to_i（"" は 0、"+11" は 11）
    let modify_value = ruby_to_i(modify_text);

    let (dice, dice_text) = roll_judge_dice(dice_count, rng)?;
    let total = dice.saturating_add(ability).saturating_add(modify_value);

    let mut text = command.to_owned();
    text.push_str(&format!(
        " ＞ {dice_count}D6[{dice_text}]+{ability}{modify_text}"
    ));
    text.push_str(&format!(" ＞ {total}"));

    let result = get_judge_reuslt_text(dice, total, target);
    if !result.is_empty() {
        text.push_str(&format!(" ＞ {result}"));
    }

    Ok(Some(text))
}

/// Ruby `OneWayHeroics#rollJudgeDice`。戻り値は `[total, diceText]`。
fn roll_judge_dice(dice_count: i64, rng: &mut Randomizer) -> Result<(i64, String), EvalError> {
    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let dice: i64 = dice_list.iter().sum();
    let dice_text = dice_text::join_dice(&dice_list);

    if dice_count == 2 {
        return Ok((dice, dice_text));
    }

    // Ruby: sort! して reverse!（降順）してから上位2個を使う
    let mut sorted = dice_list;
    sorted.sort_unstable();
    sorted.reverse();

    let total = sorted[0] + sorted[1];
    let text = format!("{dice_text}→{},{}", sorted[0], sorted[1]);
    Ok((total, text))
}

/// Ruby `OneWayHeroics#getJudgeReusltText`。
///
/// 2/12の判定は「選抜後の2個の合計」に対して行う（能力値・修正値は含めない）。
fn get_judge_reuslt_text(dice: i64, total: i64, target: Option<i64>) -> &'static str {
    if dice == 2 {
        return "ファンブル";
    }
    if dice == 12 {
        return "スペシャル";
    }

    let Some(target) = target else {
        return "";
    };

    if total >= target {
        "成功"
    } else {
        "失敗"
    }
}

/// Ruby の `String#to_i`（ここに来るのは `\d+` にマッチした文字列だけ）。
///
/// 桁あふれは Ruby だと Bignum になるので、`i64` に収まらない場合は飽和させる。
/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby の `String#to_i`（先頭の符号付き数字だけを読み、無ければ0）。
fn ruby_to_i(s: &str) -> i64 {
    let body = s.strip_prefix('+').unwrap_or(s);
    let digits: String = body.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return 0;
    }
    digits.parse().unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// Ruby `DUNGEON_TABLE` の項目。
static DUNGEON_TABLE_ITEMS: &[&str] = &[
    "犬小屋（１５５ページ）",
    "犬小屋（１５５ページ）",
    "「ダンジョン遭遇表」（１５３ページ）へ移動。小型ダンジョンだ。",
    "「ダンジョン遭遇表」（１５３ページ）へ移動。小型ダンジョンだ。",
    "「ダンジョン遭遇表」（１５３ページ）へ移動。ここは中型ダンジョンなので、モンスターが出現した場合、数が1体増加する。さらにイベントの経験値が1増加する。",
    "「ダンジョン遭遇表」（１５３ページ）へ移動。ここは大型ダンジョンなので、モンスターが出現した場合、数が2体増加する。さらにイベントの経験値が2増加する。",
    "牢獄遭遇表へ移動（１５４ページ）。牢獄つきダンジョン。",
];
/// Ruby `DUNGEON_TABLE`（ダンジョン表 / 1D6）。
static DUNGEON_TABLE: DungeonTable = DungeonTable {
    name: "ダンジョン表",
    key: "DNGN",
    times: 1,
    sides: 6,
    items: DUNGEON_TABLE_ITEMS,
};

/// Ruby `DUNGEON_TABLE_PLUS` の項目。
static DUNGEON_TABLE_PLUS_ITEMS: &[&str] = &[
    "犬小屋（基本１５５ページ）",
    "犬小屋（基本１５５ページ）",
    "犬小屋（基本１５５ページ）",
    "犬小屋（基本１５５ページ）",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。小型ダンジョンだ。",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。小型ダンジョンだ。",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。ここは中型ダンジョンのため、モンスターが出現した場合、数が１体増加する。またイベントの【経験値】が１増加する。",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。ここは大型ダンジョンのため、モンスターが出現した場合、数が２体増加する。またイベントの【経験値】が２増加する。",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。近くに寄っただけで吸い込まれる罠のダンジョンだ。「ダンジョン遭遇表」を使用したあと、中央にあるモニュメントに触れて転移して出るか、【鉄格子】と戦闘して出るか選択する。転移した場合は闇の目の前に出てしまい、全力ダッシュで【ＳＴ】を１Ｄ６消費する。【鉄格子】との戦闘では逃走を選択できない。",
    "「ダンジョン遭遇表」（基本１５３ページ）へ移動。水浸しのダンジョンで、「ダンジョン遭遇表」を使用した直後に【ＳＴ】が３減少する。「水泳」",
    "水路に囲まれた水上遺跡だ。なかに入るなら【ＳＴ】を４消費（「水泳」）してから「ダンジョン遭遇表」（基本１５３ページ）へ移動。イベントの判定に成功すると追加で【豪華な宝箱】が１つ出現し、戦闘か開錠を試みられる。",
    "「牢獄遭遇表」（基本１５４ページ）へ移動。牢獄つきダンジョンだ。",
    "砂の遺跡にたどりつき、「牢獄遭遇表」（基本１５４ページ）へ移動。モンスターが出現した場合、数が２体増加する。またイベントの【経験値】が２増加する。イベントの判定に成功すると追加で【珍しい箱】が１つ出現し、戦闘か開錠を試みられる。",
];
/// Ruby `DUNGEON_TABLE_PLUS`（ダンジョン表プラス / 2D6）。
static DUNGEON_TABLE_PLUS: DungeonTable = DungeonTable {
    name: "ダンジョン表プラス",
    key: "DNGNP",
    times: 2,
    sides: 6,
    items: DUNGEON_TABLE_PLUS_ITEMS,
};

/// Ruby `RANDOM_EVENT_TABLE` の項目。
static RANDOM_EVENT_TABLE_ITEMS: &[EventItem] = &[
    EventItem::Branch(Branch {
        kind: BranchKind::ElapsedDays,
        text: "さらに１Ｄ６を振る。現在ＰＣがいるエリアの【日数】以下なら「施設表」へ移動。【日数】を超えていれば「ダンジョン表」（１５３ページ）へ移動。",
        less_than_equal: BranchTarget::Text("FCLT"),
        greater: BranchTarget::Dungeon(&DUNGEON_TABLE),
    }),
    EventItem::Branch(Branch {
        kind: BranchKind::ElapsedDays,
        text: "さらに１Ｄ６を振る。現在ＰＣがいるエリアの【日数】以下なら「世界の旅表」（１５７ページ）へ移動。【日数】を超えていれば「野外遭遇表(OUTENC)」（１５５ページ）へ移動。",
        less_than_equal: BranchTarget::Text("「世界の旅表」（１５７ページ）へ。"),
        greater: BranchTarget::Text("OUTENC"),
    }),
    EventItem::MoveToTable { text: "「施設表」へ移動。", table_key: "FCLT" },
    EventItem::Text("「世界の旅表」（１５７ページ）へ移動。"),
    EventItem::MoveToTable { text: "「野外遭遇表」（１５５ページ）へ移動。", table_key: "OUTENC" },
    EventItem::MoveToTableWithDay { text: "「ダンジョン表」（１５２ページ）へ移動。", table: &DUNGEON_TABLE },
];
/// Ruby `RANDOM_EVENT_TABLE`（ランダムイベント表 / 1D6）。
static RANDOM_EVENT_TABLE: RandomEventTable = RandomEventTable {
    name: "ランダムイベント表",
    times: 1,
    sides: 6,
    items: RANDOM_EVENT_TABLE_ITEMS,
};

/// Ruby `RANDOM_EVENT_TABLE_PLUS` の項目。
static RANDOM_EVENT_TABLE_PLUS_ITEMS: &[EventItem] = &[
    EventItem::Branch(Branch {
        kind: BranchKind::ElapsedDays,
        text: "さらに1D6を振る。現在PCがいるエリアの【日数】以下なら施設表プラス（０２２ページ）へ移動。【経過日数】を超えていればダンジョン表プラス（０２５ページ）へ移動",
        less_than_equal: BranchTarget::Text("FCLTP"),
        greater: BranchTarget::Dungeon(&DUNGEON_TABLE_PLUS),
    }),
    EventItem::Branch(Branch {
        kind: BranchKind::ElapsedDays,
        text: "さらに1D6を振る。現在PCがいるエリアの【日数】以下なら世界の旅表（基本１５７ページ）へ移動。【経過日数】を超えていれば野外遭遇表（基本１５５ページ）へ移動",
        less_than_equal: BranchTarget::Text("「世界の旅表」（１５７ページ）へ。"),
        greater: BranchTarget::Text("OUTENC"),
    }),
    EventItem::Branch(Branch {
        kind: BranchKind::ElapsedDays,
        text: "さらに1D6を振る。現在PCがいるエリアの【日数】以下なら世界の旅表２（０２８ページ）へ移動。【経過日数】を超えていれば野外遭遇表プラス（０２５ページ）へ移動",
        less_than_equal: BranchTarget::Text("世界の旅表２（０２８ページ）へ。"),
        greater: BranchTarget::Text("OUTENCP"),
    }),
    EventItem::Branch(Branch {
        kind: BranchKind::DayParity,
        text: "さらに1D6を振る。奇数なら世界の旅表（基本１５７ページ）へ移動。偶数なら世界の旅表２（０２８ページ）へ移動",
        less_than_equal: BranchTarget::Text("世界の旅表（基本１５７ページ）へ。"),
        greater: BranchTarget::Text("世界の旅表２（０２８ページ）へ。"),
    }),
    EventItem::MoveToTable { text: "施設表プラスへ移動（０２２ページ）", table_key: "FCLTP" },
    EventItem::MoveToTableWithDay { text: "ダンジョン表プラスへ移動（０２５ページ）", table: &DUNGEON_TABLE_PLUS },
];
/// Ruby `RANDOM_EVENT_TABLE_PLUS`（ランダムイベント表プラス / 1D6）。
static RANDOM_EVENT_TABLE_PLUS: RandomEventTable = RandomEventTable {
    name: "ランダムイベント表プラス",
    times: 1,
    sides: 6,
    items: RANDOM_EVENT_TABLE_PLUS_ITEMS,
};

// ここから下の表データは `lib/bcdice/game_system/one_way_heroics/tables.rb` から
// 機械的に書き出したもの（値は1文字も変えていない）。

/// Ruby `TABLES["FT"]`（ファンブル表 / 1D6）の項目。
static T_FT_ITEMS: &[Item] = &[
    Item::Text("装備以外のアイテムのうちプレイヤー指定の１つを失う"),
    Item::Text("装備のうちプレイヤー指定の１つを失う"),
    Item::GoldFlow {
        times: 1,
        action: "失う",
    },
    Item::GoldFlow {
        times: 1,
        action: "拾う",
    },
    Item::Text("【経験値】２を獲得する"),
    Item::Text("【経験値】４を獲得する"),
];
/// Ruby `TABLES["FT"]`。
static T_FT: ChainTable = ChainTable {
    name: "ファンブル表",
    times: 1,
    sides: 6,
    items: T_FT_ITEMS,
};

/// Ruby `TABLES["DC"]`（魔王追撃表 / 1D6）の項目。
static T_DC_ITEMS: &[Item] = &[
    Item::Text("装備以外のアイテムのうちＧＭ指定の１つを失う"),
    Item::Text("装備のうちＧＭ指定の１つを失う"),
    Item::GoldFlow {
        times: 2,
        action: "失う",
    },
    Item::StatusDown {
        status: "【ＬＩＦＥ】",
        times: 1,
    },
    Item::StatusDown {
        status: "【ＳＴ】",
        times: 1,
    },
    Item::StatusDown {
        status: "【ＬＩＦＥ】",
        times: 2,
    },
];
/// Ruby `TABLES["DC"]`。
static T_DC: ChainTable = ChainTable {
    name: "魔王追撃表",
    times: 1,
    sides: 6,
    items: T_DC_ITEMS,
};

/// Ruby `TABLES["PR"]`（進行ルート表 / 1D6）の項目。
static T_PR_ITEMS: &[&str] = &[
    "少し荒れた地形が続く。【日数】から【筋力】を引いただけ【ＳＴ】が減少する（最低０）",
    "穏やかな地形が続く。【日数】から【敏捷】を引いただけ【ＳＴ】が減少する（最低０）",
    "険しい岩山だ。【日数】に１を足して【生命】を引いただけ【ＳＴ】が減少する（最低０）「登山」",
    "山で迷った。【日数】に２を足して【知力】を引いただけ【ＳＴ】が減少する（最低０）「登山」",
    "川を泳ぐ。【日数】に１を足して【意志】を引いただけ【ＳＴ】が減少する（最低０）「水泳」",
    "広い川を船で渡る。【日数】に２を足して【魅力】を引いただけ【ＳＴ】が減少する（最低０）「水泳」",
];
/// Ruby `TABLES["PR"]`。
static T_PR: Table = Table::from_dice("進行ルート表", 1, 6, T_PR_ITEMS);

/// Ruby `TABLES["TT"]`（会話テーマ表 / 1D6）の項目。
static T_TT_ITEMS: &[&str] = &[
    "身体の悩みごとについて話す。【筋力】で判定。",
    "仕事の悩みごとについて話す。【敏捷】で判定。",
    "家族の悩みごとについて話す。【生命】で判定。",
    "勇者としてこれでいいのか的悩みごとを話す。【知力】で判定。",
    "友人関係の悩みごとを話す。【意志】で判定。",
    "恋の悩みごとを話す。【魅力】で判定。",
];
/// Ruby `TABLES["TT"]`。
static T_TT: Table = Table::from_dice("会話テーマ表", 1, 6, T_TT_ITEMS);

/// Ruby `TABLES["EC"]`（逃走判定表 / 1D6）の項目。
static T_EC_ITEMS: &[&str] = &[
    "崖を登れば逃げられそうだ。【筋力】を使用する。",
    "障害物はない。走るしかない。【敏捷】を使用する。",
    "しつこく追われる。【生命】を使用する。",
    "隠れられる地形がある。【知力】を使用する。",
    "背中を向ける勇気が出るか？　【意志】を使用す",
    "もう人徳しか頼れない。【魅力】を使用する。",
];
/// Ruby `TABLES["EC"]`。
static T_EC: Table = Table::from_dice("逃走判定表", 1, 6, T_EC_ITEMS);

/// Ruby `TABLES["RNPC"]`（ランダムNPC特徴表 / 2D6）の項目。
static T_RNPC_ITEMS: &[&str] = &[
    "【物持ちの】",
    "【目のいい】",
    "【弱そうな】",
    "【宝石好きな】",
    "【エッチな】",
    "【ケチな】",
    "【変態の】",
    "【金持ちの】",
    "【強そうな】",
    "【目の悪い】",
    "【すばやい】",
];
/// Ruby `TABLES["RNPC"]`。
static T_RNPC: Table = Table::from_dice("ランダムNPC特徴表", 2, 6, T_RNPC_ITEMS);

/// Ruby `TABLES["SCT"]`（偵察表 / 1D6）の項目。
static T_SCT_ITEMS: &[&str] = &[
    "山に突き当たる。「登山」判定：【筋力】　ジャッジ：山を登る描写。",
    "川を流れ下る。「水泳」判定：【敏捷】　ジャッジ：川でピンチに陥る描写。",
    "広い湖だ……。「水泳」判定：【生命】　ジャッジ：湖面を泳ぐ描写。",
    "山の楽なルートを探そう。「登山」判定：【知力】　ジャッジ：山の豆知識。",
    "迫る闇から恐怖のあまり目を離せない。判定：【意志】　ジャッジ：勇者としての決意。",
    "任意のＮＰＣに会って情報を聞く。判定：【魅力】　ジャッジ：相手を立てる会話。",
];
/// Ruby `TABLES["SCT"]`。
static T_SCT: Table = Table::from_dice("偵察表", 1, 6, T_SCT_ITEMS);

/// Ruby `TABLES["FCLT"]`（施設表 / 2D6）の項目。
static T_FCLT_ITEMS: &[&str] = &[
    "聖なる神殿（１５２ページ）。",
    "魔王の力を封じた神殿（１５２ページ）。",
    "耳長たちの村（１５２ページ）。",
    "「村遭遇表」へ移動。大きな街なので村遭遇表を２回使用し、好きな結果を選べる。",
    "「村遭遇表」へ移動。小さな村だ。",
    "エリアの地形が「雪原」なら雪国の小屋（１５２ページ）。エリアの地形が「山岳」なら山小屋（１５２ページ）。それ以外の地形なら「村遭遇表」へ移動。この村は「石の小屋」だ。",
    "村遭遇表」へ移動。小さな村だ。",
    "村遭遇表」へ移動。大きな街なので村遭遇表を２回使用し、好きな結果を選べる。",
    "滅びた石の小屋（１５２ページ）。",
    "滅びた小さな村（１５２ページ）。",
    "闇ギルド（１５２ページ）。",
];
/// Ruby `TABLES["FCLT"]`。
static T_FCLT: Table = Table::from_dice("施設表", 2, 6, T_FCLT_ITEMS);

/// Ruby `TABLES["FCLTP"]`（施設表プラス / D66昇順）の項目。
static T_FCLTP_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("聖なる神殿（基本１５２ページ）")),
    (12, Item::Text("魔王の力を封じた神殿（基本１５２ページ）")),
    (13, Item::Text("耳長たちの村（基本１５２ページ）判定成功時に【耳長の軽い弓】【耳長の杖】を購入可能")),
    (14, Item::Text("村遭遇表へ移動（基本１５１ページ）大きな街なので村遭遇表を2回振り、好きな結果を選べる")),
    (15, Item::Text("村遭遇表へ移動（基本１５１ページ）小さな村")),
    (16, Item::Text("エリアの地形が雪原なら雪国の小屋（基本１５２ページ）エリアの地形が山岳なら山小屋（基本１５２ページ）それ以外の地形なら石の小屋、村遭遇表へ移動（基本１５１ページ）")),
    (22, Item::Text("村遭遇表へ移動（基本１５１ページ）小さな村")),
    (23, Item::Text("村遭遇表へ移動（基本１５１ページ）大きな街なので村遭遇表を2回振り、好きな結果を選べる")),
    (24, Item::Text("滅びた石の小屋（基本１５２ページ）")),
    (25, Item::Text("滅びた小さな村（基本１５２ページ）")),
    (26, Item::Text("闇ギルド（基本１５２ページ）判定成功時に一度だけ【闇ギルド袋屋】に３０００シルバ支払い【所持重量】を１増加することができる。")),
    (33, Item::Text("小さな店遭遇表プラスへ移動（０２３ページ）")),
    (34, Item::Text("酒場遭遇表プラスへ移動")),
    (35, Item::Text("酒場遭遇表プラスへ移動")),
    (36, Item::Text("錬金おばばの家（０２４ページ）")),
    (44, Item::Text("鍛冶屋の家（０２４ページ）")),
    (45, Item::Text("半獣人の隠れ家（０２４ページ）")),
    (46, Item::Text("罪人の街（０２４ページ）")),
    (55, Item::Text("封印の街（０２４ページ）")),
    (56, Item::Text("水上の街（０２４ページ）")),
    (66, Item::Text("人魚の集落（０２４ページ）")),
];
/// Ruby `TABLES["FCLTP"]`。
static T_FCLTP: D66Table = D66Table {
    name: "施設表プラス",
    items: T_FCLTP_ITEMS,
};

/// Ruby `TABLES["OUTENC"]`（野外遭遇表 / 1D6）の項目。
static T_OUTENC_ITEMS: &[Item] = &[
    Item::MoveToTable { text: "エリアの地形ごとの野外モンスター表へ移動。モンスターのうち１体にランダムな特徴がつく。モンスター特徴表（１５６ページ）を使用する。", table_key: "MONFT" },
    Item::Text("エリアの地形ごとの野外モンスター表へ移動"),
    Item::Text("エリアの地形ごとの野外モンスター表へ移動"),
    Item::Text("アンデッドの群れ（１５６ページ）"),
    Item::Text("盗賊の群れ（１５６ページ）"),
    Item::MoveToTable { text: "希少動物表（基本１５６ページ）へ移動", table_key: "RANI" },
];
/// Ruby `TABLES["OUTENC"]`。
static T_OUTENC: ChainTable = ChainTable {
    name: "野外遭遇表",
    times: 1,
    sides: 6,
    items: T_OUTENC_ITEMS,
};

/// Ruby `TABLES["OUTENCP"]`（野外遭遇表プラス / 1D6）の項目。
static T_OUTENCP_ITEMS: &[Item] = &[
    Item::MoveToTable { text: "エリアの地形ごとの野外モンスター表プラスへ移動。モンスターのうち1体にランダムな特徴がつく。モンスター特徴表プラス（０２７ページ）を使用する。", table_key: "MONFTP" },
    Item::Text("エリアの地形ごとの野外モンスター表プラスへ移動し、出現したモンスターとの戦闘が発生する"),
    Item::Text("スライムモンスター表プラス（０２７ページ）へ移動"),
    Item::Text("アンデッドの群れ（基本１５６ページ）"),
    Item::Text("盗賊の群れ（基本１５６ページ）"),
    Item::MoveToTable { text: "希少動物表（基本１５６ページ）へ移動", table_key: "RANI" },
];
/// Ruby `TABLES["OUTENCP"]`。
static T_OUTENCP: ChainTable = ChainTable {
    name: "野外遭遇表プラス",
    times: 1,
    sides: 6,
    items: T_OUTENCP_ITEMS,
};

/// Ruby `TABLES["MONFT"]`（モンスター特徴表 / D66昇順）の項目。
static T_MONFT_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【エッチな】")),
    (12, Item::Text("【変態の】")),
    (13, Item::Text("【弱そうな】")),
    (14, Item::Text("【目のいい】")),
    (15, Item::Text("【目の悪い】")),
    (16, Item::Text("【強そうな】")),
    (22, Item::Text("【強そうな】")),
    (23, Item::Text("【宝石好きな】")),
    (24, Item::Text("【幻の】")),
    (25, Item::Text("【違法な】")),
    (26, Item::Text("【イカした】")),
    (33, Item::Text("【物持ちの】")),
    (34, Item::Text("【炎を吐く】")),
    (35, Item::Text("【必中の】")),
    (36, Item::Text("【すばやい】")),
    (44, Item::Text("【やたら硬い】")),
    (45, Item::Text("【名の知れた】")),
    (46, Item::Text("【凶悪な】")),
    (55, Item::Text("【賞金首の】")),
    (56, Item::Text("【古代種の】")),
    (66, Item::Text("【最強の】")),
];
/// Ruby `TABLES["MONFT"]`。
static T_MONFT: D66Table = D66Table {
    name: "モンスター特徴表",
    items: T_MONFT_ITEMS,
};

/// Ruby `TABLES["MONFTP"]`（モンスター特徴表プラス / D66昇順）の項目。
static T_MONFTP_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【エッチな】（基本１７８ページ）")),
    (12, Item::Text("【変態の】（基本１７８ページ）")),
    (13, Item::Text("【目のいい】（基本１７８ページ）")),
    (14, Item::Text("【目の悪い】（基本１７８ページ）")),
    (15, Item::Text("【強そうな】（基本１７８ページ）")),
    (16, Item::Text("【宝石好きな】（基本１７８ページ）")),
    (22, Item::Text("【幻の】（基本１７８ページ）")),
    (23, Item::Text("【違法な】（基本１７８ページ）")),
    (24, Item::Text("【イカした】（基本１７８ページ）")),
    (25, Item::Text("【物持ちの】（基本１７８ページ）")),
    (26, Item::Text("【炎を吐く】（基本１７８ページ）")),
    (33, Item::Text("【やたら硬い】（基本１７８ページ）")),
    (34, Item::Text("【古代種の】（基本１７８ページ）")),
    (35, Item::Text("【最強の】（基本１７８ページ）")),
    (36, Item::Text("【異国風の】（０４７ページ）")),
    (44, Item::Text("【毛深い】（０４７ページ）")),
    (45, Item::Text("【耐火の】（０４７ページ）")),
    (46, Item::Text("【耐雷の】（０４７ページ） ")),
    (55, Item::Text("【浮遊の】（０４７ページ）")),
    (56, Item::Text("【臭い】（０４７ページ）")),
    (66, Item::Text("【恐怖の】（０４７ページ）")),
];
/// Ruby `TABLES["MONFTP"]`。
static T_MONFTP: D66Table = D66Table {
    name: "モンスター特徴表プラス",
    items: T_MONFTP_ITEMS,
};

/// Ruby `TABLES["RANI"]`（希少動物表 / 1D6）の項目。
static T_RANI_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "【『緑の森』隊長】1体と遭遇する。今回のセッションで【雪ウサギ】【山岳ゴート】【遺跡白馬】【草原カワウソ】【砂漠キツネ】のいずれかを倒したことがあれば、戦闘が発生する。戦闘にならなかった場合はなごやかに別れる。"),
    (RangeInc::new(2, 3), "【『緑の森』団員】1体と遭遇する。今回のセッションで【雪ウサギ】【山岳ゴート】【遺跡白馬】【草原カワウソ】【砂漠キツネ】のいずれかを倒したことがあれば、戦闘が発生する。戦闘にならなかった場合はなごやかに別れる。"),
    (RangeInc::new(4, 6), "地形によって異なる希少動物が1体出現する。雪原なら【雪ウサギ】、山岳なら【山岳ゴート】、遺跡なら【遺跡白馬】、草原なら【草原カワウソ】、砂漠と荒野は【砂漠キツネ】。それ以外は【緑の森団員】となる。戦闘を挑んでもいいし、見送ってもいい。"),
];
/// Ruby `TABLES["RANI"]`。
static T_RANI: RangeTable = RangeTable::from_dice("希少動物表", 1, 6, T_RANI_ITEMS);

/// Ruby `TABLES["DROP"]`（ドロップアイテム表 / 1D6）の項目。
static T_DROP_ITEMS: &[Item] = &[
    Item::MoveToTable {
        text: "武器ドロップ表へ移動",
        table_key: "DROPWP",
    },
    Item::MoveToTable {
        text: "武器ドロップ表へ移動",
        table_key: "DROPWP",
    },
    Item::MoveToTable {
        text: "防具ドロップ表へ移動",
        table_key: "DROPAR",
    },
    Item::MoveToTable {
        text: "食品ドロップ表へ移動",
        table_key: "DROPFD",
    },
    Item::MoveToTable {
        text: "巻物ドロップ表へ移動",
        table_key: "DROPSC",
    },
    Item::MoveToTable {
        text: "その他ドロップ表へ移動",
        table_key: "DROPOT",
    },
];
/// Ruby `TABLES["DROP"]`。
static T_DROP: ChainTable = ChainTable {
    name: "ドロップアイテム表",
    times: 1,
    sides: 6,
    items: T_DROP_ITEMS,
};

/// Ruby `TABLES["DROPWP"]`（武器ドロップ表 / D66昇順）の項目。
static T_DROPWP_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【さびた小剣】")),
    (12, Item::Text("【さびた長剣】")),
    (13, Item::Text("【さびた大剣】")),
    (14, Item::Text("【長い棒】")),
    (15, Item::Text("【ダガー】")),
    (16, Item::Text("【木こりの大斧】")),
    (22, Item::Text("【ショートブレイド】")),
    (23, Item::Text("【木の杖】")),
    (24, Item::Text("【狩人の弓】")),
    (25, Item::Text("【レイピア】")),
    (26, Item::Text("【携帯弓】")),
    (33, Item::Text("【ロングブレイド】")),
    (34, Item::Text("【スレンドスピア】")),
    (35, Item::Text("【バトルアックス】")),
    (36, Item::Text("【軍用剛弓】")),
    (44, Item::Text("【グランドブレイド】")),
    (45, Item::Text("【祈りの杖】")),
    (46, Item::Text("【ヘビィボウガン】")),
    (55, Item::Text("【シルバーランス】")),
    (56, Item::Text("【イーグルブレイド】")),
    (66, Item::Text("【クレセントアクス】")),
];
/// Ruby `TABLES["DROPWP"]`。
static T_DROPWP: D66Table = D66Table {
    name: "武器ドロップ表",
    items: T_DROPWP_ITEMS,
};

/// Ruby `TABLES["DROPAR"]`（防具ドロップ表 / D66昇順）の項目。
static T_DROPAR_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【旅人の服】")),
    (12, Item::Text("【旅人の服】")),
    (13, Item::Text("【旅人の服】")),
    (14, Item::Text("【レザーシールド】")),
    (15, Item::Text("【レザーシールド】")),
    (16, Item::Text("【騎士のコート】")),
    (22, Item::Text("【騎士のコート】")),
    (23, Item::Text("【スケイルシールド】")),
    (24, Item::Text("【スケイルシールド】")),
    (25, Item::Text("【レザーベスト】")),
    (26, Item::Text("【レザーベスト】")),
    (33, Item::Text("【ヘビィシールド】")),
    (34, Item::Text("【チェインクロス】")),
    (35, Item::Text("【チェインクロス】")),
    (36, Item::Text("【試練の腕輪】")),
    (44, Item::Text("【精霊のローブ】")),
    (45, Item::Text("【必殺の腕輪】")),
    (46, Item::Text("【ギガントプレート】")),
    (55, Item::Text("【破壊の腕輪】")),
    (56, Item::Text("【理力の腕輪】")),
    (66, Item::Text("【加速の腕輪】")),
];
/// Ruby `TABLES["DROPAR"]`。
static T_DROPAR: D66Table = D66Table {
    name: "防具ドロップ表",
    items: T_DROPAR_ITEMS,
};

/// Ruby `TABLES["DROPHW"]`（聖武具ドロップ表 / 2D6）の項目。
static T_DROPHW_ITEMS: &[&str] = &[
    "【紅き太陽の剣】",
    "【紅き太陽の剣】",
    "【聖剣カレドヴルフ】 ",
    "【聖斧エルサーベス】 ",
    "【水霊のマント】",
    "【大地の鎧】",
    "【大気の盾】",
    "【聖弓ル・アルシャ】",
    "【聖槍ヴァルキウス】",
    "【聖なる月の剣】",
    "【聖なる月の剣】",
];
/// Ruby `TABLES["DROPHW"]`。
static T_DROPHW: Table = Table::from_dice("聖武具ドロップ表", 2, 6, T_DROPHW_ITEMS);

/// Ruby `TABLES["DROPFD"]`（食品ドロップ表 / D66昇順）の項目。
static T_DROPFD_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【枯れた草】")),
    (12, Item::Text("【こげた草】")),
    (13, Item::Text("【サボテンの肉】")),
    (14, Item::Text("【動物の肉】")),
    (15, Item::Text("【癒しの草】、地形が火山なら【こげた草】")),
    (
        16,
        Item::Text("【癒しの草】、地形が火山なら【こげた草】、地形が雪原なら【スノークリスタ草】"),
    ),
    (22, Item::Text("【スタミナ草】、地形が火山なら【こげた草】")),
    (
        23,
        Item::Text(
            "【スタミナ草】、地形が火山なら【こげた草】、地形が雪原なら【スノークリスタ草】",
        ),
    ),
    (24, Item::Text("【触手の草】、地形が火山なら【こげた草】")),
    (
        25,
        Item::Text("【触手の草】、地形が火山なら【こげた草】、地形が雪原なら【スノークリスタ草】"),
    ),
    (26, Item::Text("【スタミナのアンプル】")),
    (33, Item::Text("【癒しのアンプル】")),
    (34, Item::Text("【癒しのアンプル】")),
    (35, Item::Text("【ナユタの実】、地形が火山なら【こげた草】")),
    (36, Item::Text("【ナユタの実】、地形が火山なら【こげた草】")),
    (44, Item::Text("【火炎のアンプル】")),
    (45, Item::Text("【強酸のアンプル】")),
    (46, Item::Text("【とぶクスリ】")),
    (55, Item::Text("【竜炎のアンプル】")),
    (56, Item::Text("【おいしいお弁当】")),
    (66, Item::Text("【自然治癒のアンプル】")),
];
/// Ruby `TABLES["DROPFD"]`。
static T_DROPFD: D66Table = D66Table {
    name: "食品ドロップ表",
    items: T_DROPFD_ITEMS,
};

/// Ruby `TABLES["DROPSC"]`（巻物ドロップ表 / D66昇順）の項目。
static T_DROPSC_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【石壁の巻物】")),
    (12, Item::Text("【石壁の巻物】")),
    (13, Item::Text("【周辺の地図】")),
    (14, Item::Text("【周辺の地図】")),
    (15, Item::Text("【周辺の地図】")),
    (16, Item::Text("【火炎付与の巻物】")),
    (22, Item::Text("【混乱の巻物】")),
    (23, Item::Text("【剣の巻物】")),
    (24, Item::Text("【剣の巻物】")),
    (25, Item::Text("【鎧の巻物】")),
    (26, Item::Text("【鎧の巻物】")),
    (33, Item::Text("【応急修理の巻物】")),
    (34, Item::Text("【応急修理の巻物】")),
    (35, Item::Text("【移動不能付与の巻物】")),
    (36, Item::Text("【移動不能付与の巻物】")),
    (44, Item::Text("【宝の地図】")),
    (45, Item::Text("【宝の地図】")),
    (46, Item::Text("【召喚の巻物】")),
    (55, Item::Text("【剣の王の巻物】")),
    (56, Item::Text("【守りの神の巻物】")),
    (66, Item::Text("【高度修復の巻物】")),
];
/// Ruby `TABLES["DROPSC"]`。
static T_DROPSC: D66Table = D66Table {
    name: "巻物ドロップ表",
    items: T_DROPSC_ITEMS,
};

/// Ruby `TABLES["DROPOT"]`（その他ドロップ表 / D66昇順）の項目。
static T_DROPOT_ITEMS: &[(i64, Item)] = &[
    (
        11,
        Item::Text("【大きな石】、地形が火山なら【くすんだ宝石】"),
    ),
    (
        12,
        Item::Text("【大きな石】、地形が火山なら【くすんだ宝石】"),
    ),
    (13, Item::Text("【大きな石】、地形が火山なら【美しい宝石】")),
    (14, Item::Text("【木製の矢】")),
    (15, Item::Text("【理力の矢】")),
    (16, Item::Text("【鉄製の矢】")),
    (22, Item::Text("【投げナイフ】")),
    (23, Item::Text("【爆弾矢】")),
    (24, Item::Text("【くすんだ宝石】")),
    (25, Item::Text("【盾修復キット】")),
    (26, Item::Text("【上質の研ぎ石】")),
    (33, Item::Text("【エルザイト爆弾】")),
    (34, Item::Text("【セーブクリスタル】")),
    (35, Item::Text("【試練の腕輪】")),
    (36, Item::Text("【必殺の腕輪】")),
    (44, Item::Text("【破壊の腕輪】")),
    (45, Item::Text("【理力の腕輪】")),
    (46, Item::Text("【加速の腕輪】")),
    (55, Item::Text("【美しい宝石】")),
    (56, Item::Text("【封印のカギ】")),
    (66, Item::Text("【闇ギルド会員証】")),
];
/// Ruby `TABLES["DROPOT"]`。
static T_DROPOT: D66Table = D66Table {
    name: "その他ドロップ表",
    items: T_DROPOT_ITEMS,
};

/// Ruby `TABLES["DROPP"]`（ドロップアイテム表プラス / D66昇順）の項目。
static T_DROPP_ITEMS: &[(i64, Item)] = &[
    (
        11,
        Item::MoveToTable {
            text: "武器ドロップ表",
            table_key: "DROPWP",
        },
    ),
    (
        12,
        Item::MoveToTable {
            text: "武器ドロップ表",
            table_key: "DROPWP",
        },
    ),
    (
        13,
        Item::MoveToTable {
            text: "武器ドロップ表2",
            table_key: "DROPWP2",
        },
    ),
    (
        14,
        Item::MoveToTable {
            text: "武器ドロップ表2",
            table_key: "DROPWP2",
        },
    ),
    (
        15,
        Item::MoveToTable {
            text: "防具ドロップ表",
            table_key: "DROPAR",
        },
    ),
    (
        16,
        Item::MoveToTable {
            text: "防具ドロップ表",
            table_key: "DROPAR",
        },
    ),
    (
        22,
        Item::MoveToTable {
            text: "防具ドロップ表2",
            table_key: "DROPAR2",
        },
    ),
    (
        23,
        Item::MoveToTable {
            text: "防具ドロップ表2",
            table_key: "DROPAR2",
        },
    ),
    (
        24,
        Item::MoveToTable {
            text: "食品ドロップ表",
            table_key: "DROPFD",
        },
    ),
    (
        25,
        Item::MoveToTable {
            text: "食品ドロップ表",
            table_key: "DROPFD",
        },
    ),
    (
        26,
        Item::MoveToTable {
            text: "食品ドロップ表2",
            table_key: "DROPFD2",
        },
    ),
    (
        33,
        Item::MoveToTable {
            text: "食品ドロップ表2",
            table_key: "DROPFD2",
        },
    ),
    (
        34,
        Item::MoveToTable {
            text: "薬品ドロップ表プラス",
            table_key: "DROPDRP",
        },
    ),
    (
        35,
        Item::MoveToTable {
            text: "薬品ドロップ表プラス",
            table_key: "DROPDRP",
        },
    ),
    (
        36,
        Item::MoveToTable {
            text: "巻物ドロップ表",
            table_key: "DROPSC",
        },
    ),
    (
        44,
        Item::MoveToTable {
            text: "巻物ドロップ表",
            table_key: "DROPSC",
        },
    ),
    (
        45,
        Item::MoveToTable {
            text: "巻物ドロップ表2",
            table_key: "DROPSC2",
        },
    ),
    (
        46,
        Item::MoveToTable {
            text: "巻物ドロップ表2",
            table_key: "DROPSC2",
        },
    ),
    (
        55,
        Item::MoveToTable {
            text: "その他ドロップ表",
            table_key: "DROPOT",
        },
    ),
    (
        56,
        Item::MoveToTable {
            text: "その他ドロップ表",
            table_key: "DROPOT",
        },
    ),
    (
        66,
        Item::MoveToTable {
            text: "その他ドロップ表2",
            table_key: "DROPOT2",
        },
    ),
];
/// Ruby `TABLES["DROPP"]`。
static T_DROPP: D66Table = D66Table {
    name: "ドロップアイテム表プラス",
    items: T_DROPP_ITEMS,
};

/// Ruby `TABLES["DROPDRP"]`（薬品ドロップ表プラス / D66昇順）の項目。
static T_DROPDRP_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【燃料油のビン】")),
    (12, Item::Text("【燃料油のビン】")),
    (13, Item::Text("【燃料油のビン】")),
    (14, Item::Text("【弱体の薬】")),
    (15, Item::Text("【弱体の薬】")),
    (16, Item::Text("【弱体の薬】")),
    (22, Item::Text("【成長の薬】")),
    (23, Item::Text("【ベルセルクアンプル】")),
    (24, Item::Text("【ベルセルクアンプル】")),
    (25, Item::Text("【浮遊の薬】")),
    (26, Item::Text("【浮遊の薬】")),
    (33, Item::Text("【反動解消の薬】")),
    (34, Item::Text("【反動解消の薬】")),
    (35, Item::Text("【癒しの大ボトル】")),
    (36, Item::Text("【癒しの大ボトル】")),
    (44, Item::Text("【超元気のアンプル】")),
    (45, Item::Text("【超元気のアンプル】")),
    (46, Item::Text("【薬命酒】")),
    (55, Item::Text("【薬命酒】")),
    (56, Item::Text("【洗脳のクスリ】")),
    (66, Item::Text("【洗脳のクスリ】")),
];
/// Ruby `TABLES["DROPDRP"]`。
static T_DROPDRP: D66Table = D66Table {
    name: "薬品ドロップ表プラス",
    items: T_DROPDRP_ITEMS,
};

/// Ruby `TABLES["DROPSC2"]`（巻物ドロップ表2 / D66昇順）の項目。
static T_DROPSC2_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【火炎波の巻物】")),
    (12, Item::Text("【悟りの巻物】")),
    (13, Item::Text("【理盾の巻物】")),
    (14, Item::Text("【泉の巻物】")),
    (15, Item::Text("【雷神の巻物】")),
    (16, Item::Text("【超激震の巻物】")),
    (22, Item::Text("【闇を阻む巻物】")),
    (23, Item::Text("【引きこもりの巻物】")),
    (24, Item::Text("【鋼鉄の巻物】")),
    (25, Item::Text("【回廊の巻物】")),
    (26, Item::Text("【騎士団の巻物】")),
    (33, Item::Text("【水泳能力の巻物】")),
    (34, Item::Text("【浮遊能力の巻物】")),
    (35, Item::Text("【治癒の書】")),
    (36, Item::Text("【浮遊の書】")),
    (44, Item::Text("【突風の書】")),
    (45, Item::Text("【睡眠の書】")),
    (46, Item::Text("【火炎の書】")),
    (55, Item::Text("【鋼鉄の書】")),
    (56, Item::Text("【加速の書】")),
    (66, Item::Text("【闇払いの書】")),
];
/// Ruby `TABLES["DROPSC2"]`。
static T_DROPSC2: D66Table = D66Table {
    name: "巻物ドロップ表2",
    items: T_DROPSC2_ITEMS,
};

/// Ruby `TABLES["DROPWP2"]`（武器ドロップ表2 / D66昇順）の項目。
static T_DROPWP2_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【さびた巨大斧】")),
    (12, Item::Text("【さびた巨大斧】")),
    (13, Item::Text("【モコモコのバトン】")),
    (14, Item::Text("【モコモコのバトン】")),
    (15, Item::Text("【ベルセルクアクス】")),
    (16, Item::Text("【ベルセルクアクス】")),
    (22, Item::Text("【クナイ】")),
    (23, Item::Text("【クナイ】")),
    (24, Item::Text("【術殺槍】")),
    (25, Item::Text("【ウィンドスピア】")),
    (26, Item::Text("【ウィンドスピア】")),
    (33, Item::Text("【つるはし】")),
    (34, Item::Text("【つるはし】")),
    (35, Item::Text("【理力の剣】")),
    (36, Item::Text("【蒼い短刀】")),
    (44, Item::Text("【クリムゾンクロウ】")),
    (45, Item::Text("【ナユタの杖】")),
    (46, Item::Text("【ナユタの杖】")),
    (55, Item::Text("【一撃斧】")),
    (56, Item::Text("【ファイアブランド】")),
    (66, Item::Text("【ソードクロスボウ】")),
];
/// Ruby `TABLES["DROPWP2"]`。
static T_DROPWP2: D66Table = D66Table {
    name: "武器ドロップ表2",
    items: T_DROPWP2_ITEMS,
};

/// Ruby `TABLES["DROPAR2"]`（防具ドロップ表2 / D66昇順）の項目。
static T_DROPAR2_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【ボロボロの服】")),
    (12, Item::Text("【ボロボロの服】")),
    (13, Item::Text("【穴だらけの鎧】")),
    (14, Item::Text("【穴だらけの鎧】")),
    (15, Item::Text("【木製の追加装甲】")),
    (16, Item::Text("【木製の追加装甲】")),
    (22, Item::Text("【ガラスの鎧】")),
    (23, Item::Text("【ガラスの鎧】")),
    (24, Item::Text("【鉄板の追加装甲】")),
    (25, Item::Text("【鉄板の追加装甲】")),
    (26, Item::Text("【太陽のランタン】")),
    (33, Item::Text("【耐火服】")),
    (34, Item::Text("【獣の革のバッグ】")),
    (35, Item::Text("【重量ブーツ】")),
    (36, Item::Text("【冒険者のブーツ】")),
    (44, Item::Text("【ラバーブーツ】")),
    (45, Item::Text("【風のマント】")),
    (46, Item::Text("【狩人の服】")),
    (55, Item::Text("【ドラゴンスケイル】")),
    (56, Item::Text("【不育の腕輪】")),
    (66, Item::Text("【竜革の大きなバッグ】")),
];
/// Ruby `TABLES["DROPAR2"]`。
static T_DROPAR2: D66Table = D66Table {
    name: "防具ドロップ表2",
    items: T_DROPAR2_ITEMS,
};

/// Ruby `TABLES["DROPHWP"]`（聖武具ドロップ表プラス / D66昇順）の項目。
static T_DROPHWP_ITEMS: &[(i64, Item)] = &[
    (11, Item::Text("【大気の盾】")),
    (23, Item::Text("【聖剣カレドヴルフ】")),
    (36, Item::Text("【紅蓮の書】")),
    (12, Item::Text("【大気の盾】")),
    (24, Item::Text("【聖斧エルサーベス】")),
    (44, Item::Text("【聖弓ル・アルシャ】")),
    (13, Item::Text("【大地の鎧】")),
    (25, Item::Text("【聖斧エルサーベス】")),
    (45, Item::Text("【聖弓ル・アルシャ】")),
    (14, Item::Text("【大地の鎧】")),
    (26, Item::Text("【聖槍ヴァルキウス】")),
    (46, Item::Text("【聖なる月の剣】")),
    (15, Item::Text("【水霊のマント】")),
    (33, Item::Text("【聖槍ヴァルキウス】")),
    (55, Item::Text("【紅き太陽の剣】")),
    (16, Item::Text("【水霊のマント】")),
    (34, Item::Text("【聖槍ヴァルキウス】")),
    (56, Item::Text("【嵐の聖剣】")),
    (22, Item::Text("【聖剣カレドヴルフ】")),
    (35, Item::Text("【紅蓮の書】")),
    (66, Item::Text("【超重の聖斧】")),
];
/// Ruby `TABLES["DROPHWP"]`。
static T_DROPHWP: D66Table = D66Table {
    name: "聖武具ドロップ表プラス",
    items: T_DROPHWP_ITEMS,
};

/// Ruby `TABLES["DROPFD2"]`（食品ドロップ表2 / 1D6）の項目。
static T_DROPFD2_ITEMS: &[&str] = &[
    "【解毒の草】、地形が火 山なら【こげた草】、地 形が海岸なら【おいし い海藻】",
    "【気付けの草】、地形が 火山なら【こげた草】、 地形が海岸なら【おい しい海藻】",
    "【夜目の草】",
    "【力が湧く草】",
    "【集中の草】",
    "【牛乳】",
];
/// Ruby `TABLES["DROPFD2"]`。
static T_DROPFD2: Table = Table::from_dice("食品ドロップ表2", 1, 6, T_DROPFD2_ITEMS);

/// Ruby `TABLES["DROPOT2"]`（その他 ドロップ表2 / 2D6）の項目。
static T_DROPOT2_ITEMS: &[&str] = &[
    "【五連の矢】",
    "【炎の矢】",
    "【聖なる投げ刃】",
    "【物体破壊爆弾】",
    "【閃光弾】",
    "【聖なる短剣の破片】",
    "【閃光弾】",
    "【旋風の投げ刃】",
    "【スーパーエルザイト 爆弾】",
    "【炎の矢】",
    "【五連の矢】",
];
/// Ruby `TABLES["DROPOT2"]`。
static T_DROPOT2: Table = Table::from_dice("その他 ドロップ表2", 2, 6, T_DROPOT2_ITEMS);

/// Ruby `TABLES["DROPRAREBOX2"]`（珍しい箱ドロップ表2 / 2D6）の項目。
static T_DROPRAREBOX2_ITEMS: &[&str] = &[
    "聖武具ドロップ表プラ スへ",
    "【耐久力の結晶】",
    "【偉大な筋力の結晶】",
    "【偉大な敏捷の結晶】",
    "【偉大な生命の結晶】",
    "【竜鱗の追加装甲】",
    "【偉大な魅力の結晶】",
    "【偉大な意志の結晶】",
    "【偉大な知力の結晶】",
    "【スタミナの結晶】",
    "【闇払いの書】",
];
/// Ruby `TABLES["DROPRAREBOX2"]`。
static T_DROPRAREBOX2: Table = Table::from_dice("珍しい箱ドロップ表2", 2, 6, T_DROPRAREBOX2_ITEMS);

/// Ruby `TABLES["KNGFTP"]`（王特徴表プラス / 1D6）の項目。
static T_KNGFTP_ITEMS: &[&str] = &[
    "【力の王の】（０４７ページ）",
    "【力の王の】（０４７ページ）",
    "【疾風の王の】（０４７ページ）",
    "【疾風の王の】（０４７ページ）",
    "【炎の王の】（０４７ページ）",
    "【絶望の王の】（０４７ページ）",
];
/// Ruby `TABLES["KNGFTP"]`。
static T_KNGFTP: Table = Table::from_dice("王特徴表プラス", 1, 6, T_KNGFTP_ITEMS);

/// Ruby `OneWayHeroics::TABLES`。
static TABLES: &[(&str, TableRef)] = &[
    ("FT", TableRef::Chain(&T_FT)),
    ("DC", TableRef::Chain(&T_DC)),
    ("PR", TableRef::Plain(&T_PR)),
    ("TT", TableRef::Plain(&T_TT)),
    ("EC", TableRef::Plain(&T_EC)),
    ("RNPC", TableRef::Plain(&T_RNPC)),
    ("SCT", TableRef::Plain(&T_SCT)),
    ("FCLT", TableRef::Plain(&T_FCLT)),
    ("FCLTP", TableRef::D66(&T_FCLTP)),
    ("OUTENC", TableRef::Chain(&T_OUTENC)),
    ("OUTENCP", TableRef::Chain(&T_OUTENCP)),
    ("MONFT", TableRef::D66(&T_MONFT)),
    ("MONFTP", TableRef::D66(&T_MONFTP)),
    ("RANI", TableRef::Range(&T_RANI)),
    ("DROP", TableRef::Chain(&T_DROP)),
    ("DROPWP", TableRef::D66(&T_DROPWP)),
    ("DROPAR", TableRef::D66(&T_DROPAR)),
    ("DROPHW", TableRef::Plain(&T_DROPHW)),
    ("DROPFD", TableRef::D66(&T_DROPFD)),
    ("DROPSC", TableRef::D66(&T_DROPSC)),
    ("DROPOT", TableRef::D66(&T_DROPOT)),
    ("DROPP", TableRef::D66(&T_DROPP)),
    ("DROPDRP", TableRef::D66(&T_DROPDRP)),
    ("DROPSC2", TableRef::D66(&T_DROPSC2)),
    ("DROPWP2", TableRef::D66(&T_DROPWP2)),
    ("DROPAR2", TableRef::D66(&T_DROPAR2)),
    ("DROPHWP", TableRef::D66(&T_DROPHWP)),
    ("DROPFD2", TableRef::Plain(&T_DROPFD2)),
    ("DROPOT2", TableRef::Plain(&T_DROPOT2)),
    ("DROPRAREBOX2", TableRef::Plain(&T_DROPRAREBOX2)),
    ("KNGFTP", TableRef::Plain(&T_KNGFTP)),
];

/// Ruby `BCDice::GameSystem::OneWayHeroics`（ID: `OneWayHeroics`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneWayHeroics;

impl GameSystem for OneWayHeroics {
    fn id(&self) -> &'static str {
        "OneWayHeroics"
    }

    fn name(&self) -> &'static str {
        "片道勇者TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "かたみちゆうしやTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　aJDx+y,z
　a:ダイス数（省略時2個)、x:能力値、
　y:修正値（省略可。「＋」のみなら＋１）、z:目標値（省略可）
　例１）JD2+1,8 or JD2+,8　：能力値２、修正＋１、目標値８
　例２）JD3,10 能力値３、修正なし、目標値10
　例３）3JD4+ ダイス3個から2個選択、能力値４、修正なし、目標値なし
・ファンブル表 FT／魔王追撃表   DC／進行ルート表 PR／会話テーマ表 TT
逃走判定表   EC／ランダムNPC特徴表 RNPC／偵察表 SCT
施設表　FCLT／施設表プラス　FCLTP／希少動物表 RANI／王特徴表プラス KNGFTP
野外遭遇表 OUTENC／野外遭遇表プラス OUTENCP
モンスター特徴表 MONFT／モンスター特徴表プラス MONFTP
ドロップアイテム表 DROP／ドロップアイテム表プラス DROPP
武器ドロップ表 DROPWP／武器ドロップ表2 DROPWP2
防具ドロップ表 DROPAR／防具ドロップ表2 DROPAR2
聖武具ドロップ表 DROPHW／聖武具ドロップ表プラス DROPHWP
食品ドロップ表 DROPFD／食品ドロップ表2 DROPFD2
巻物ドロップ表 DROPSC／巻物ドロップ表2 DROPSC2
その他ドロップ表 DROPOT／その他 ドロップ表2 DROPOT2
薬品ドロップ表プラス DROPDRP／珍しい箱ドロップ表2 DROPRAREBOX2
・ランダムイベント表 RETx（x：現在の日数）、ランダムイベント表プラス RETPx
　例）RET3、RETP4
・ダンジョン表 DNGNx（x：現在の日数）、ダンジョン表プラス DNGNPx
　例）DNGN3、DNGNP4
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d*JD",
            "RETP?",
            "DNGNP?",
            "FT",
            "DC",
            "PR",
            "TT",
            "EC",
            "RNPC",
            "SCT",
            "FCLT",
            "FCLTP",
            "OUTENC",
            "OUTENCP",
            "MONFT",
            "MONFTP",
            "RANI",
            "DROP",
            "DROPWP",
            "DROPAR",
            "DROPHW",
            "DROPFD",
            "DROPSC",
            "DROPOT",
            "DROPP",
            "DROPDRP",
            "DROPSC2",
            "DROPWP2",
            "DROPAR2",
            "DROPHWP",
            "DROPFD2",
            "DROPOT2",
            "DROPRAREBOX2",
            "KNGFTP",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `OneWayHeroics#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `OneWayHeroics#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::{GameSystem, GameSystemId};

    use super::OneWayHeroics;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/OneWayHeroics.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "OneWayHeroics",
            "OneWayHeroics.toml",
            76,
        );
    }

    /// `TABLES` のキーが接頭辞の登録と一致すること。
    ///
    /// Ruby: `register_prefix('\d*JD', 'RETP?', 'DNGNP?', TABLES.keys)`。
    /// スタブ由来の `prefixes` と移植した `TABLES` がずれていないことを押さえる。
    #[test]
    fn table_keys_match_registered_prefixes() {
        let keys: Vec<&str> = super::TABLES.iter().map(|(k, _)| *k).collect();
        let prefixes: Vec<&str> = OneWayHeroics
            .prefixes()
            .iter()
            .copied()
            .filter(|p| !matches!(*p, r"\d*JD" | "RETP?" | "DNGNP?"))
            .collect();
        assert_eq!(
            keys, prefixes,
            "TABLES.keys と register_prefix の並びが一致すること"
        );
    }

    /// 希少動物表（`RangeTable`）が出目の全域を隙間なく覆っていること。
    ///
    /// Ruby は `RangeTable#initialize` が構築時に検査して `RangeError` を投げる。
    #[test]
    fn range_table_covers_all_values() {
        super::T_RANI
            .validate()
            .expect("希少動物表の範囲が正しいこと");
    }

    /// 全31表が最小値・最大値のどちらでも本体を返すこと。
    ///
    /// TOMLは一部の表しか引かないので、項目数の過不足（`表名(値) ＞ ` で終わる）を
    /// ここで検出する。ネストする表も含めて全て1D6なので、同じ出目を並べて注入できる。
    #[test]
    fn every_table_has_an_item_at_both_ends() {
        for (key, _) in super::TABLES {
            for value in [1, 6] {
                let mut src = SeededRandomizer::new(vec![(value, 6); 20]);
                let result = eval_command(&GameSystemId::new("OneWayHeroics"), key, &mut src)
                    .unwrap_or_else(|e| panic!("{key} (出目{value}): {e}"))
                    .unwrap_or_else(|| panic!("{key} (出目{value}): nil"));
                assert!(
                    !result.text.ends_with("＞ ") && !result.text.ends_with('＞'),
                    "{key} (出目{value}) の項目が空: {:?}",
                    result.text
                );
            }
        }
    }
}
