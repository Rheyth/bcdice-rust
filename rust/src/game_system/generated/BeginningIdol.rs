//! P4で手書き移植した `lib/bcdice/game_system/BeginningIdol.rb` と
//! `lib/bcdice/game_system/beginning_idol/*.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `BeginningIdol#result_nd6`（2以下でファンブル、12以上でスペシャル）
//! - `roll_attack`（`n[S]A[r][+m/-m]`）/ `roll_burst`（`nnC`）/ `roll_performance`（`[r]PDn[+m/-m]`）
//! - `beginning_idol/*.rb` の表クラス群（`ChainTable` / `ChainD66Table` / `BadStatusTable` /
//!   `ItemTable` / `D6TwiceTable` / `MySkillNameTable` / `RandomEventTable` /
//!   `WorkWithChanceTable` / `WithAbnormality` / `SkillTable` / `SkillGetTable`）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("BeginningIdol.…", locale:)` で `i18n/BeginningIdol/ja_jp.yml` から表を作る。
//! Rust側は同じ値を `static` として直接持つ。データ部分（`JA_` 接頭辞の `static` 群）は
//! 同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差のあるロジックは [`SystemTables`] に束ね、
//! `BeginningIdol_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::{NoExpand, Regex};

use crate::dice_table::sai_fic_skill_table::{
    DEFAULT_RCT_FORMAT, DEFAULT_RTTN_FORMAT, DEFAULT_RTT_FORMAT, DEFAULT_SKILL_FORMAT,
};
use crate::dice_table::{
    D66Table, RollResult, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkill,
    SaiFicSkillTable, Table, TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// 表の部品
// ---------------------------------------------------------------------------

/// 引くと文字列を返す表。
///
/// Ruby の `ChainTable` / `ChainD66Table` は項目に対して
/// `item.respond_to?(:roll) ? item.roll(randomizer) : item` を評価して `join("\n")` する。
/// 連鎖先には `DiceTable::Table` のように `RollResult` を返すものと、`ItemTable` のように
/// 文字列を返すものが混在するため、`to_s` 後の文字列で揃えたトレイトにしてある。
pub(crate) trait RollText: Sync {
    /// Ruby `#roll(randomizer)` の結果を `to_s` したもの。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError>;
}

impl RollText for Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng)?.to_string())
    }
}

impl RollText for D66Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng)?.to_string())
    }
}

/// 連鎖する表の項目。Ruby の `Array<String, #roll>` の要素に対応する。
pub(crate) enum Node {
    /// 文字列の項目。
    Text(&'static str),
    /// 連鎖先の表。
    Table(&'static dyn RollText),
}

impl Node {
    fn resolve(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        match self {
            Node::Text(text) => Ok((*text).to_string()),
            Node::Table(table) => table.roll_text(rng),
        }
    }
}

/// Ruby `chosen.map { ... }.join("\n")`。
fn resolve_nodes(nodes: &[Node], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut parts = Vec::with_capacity(nodes.len());
    for node in nodes {
        parts.push(node.resolve(rng)?);
    }
    Ok(parts.join("\n"))
}

/// 遅延コンパイルする正規表現。i18n から来る表現なのでソースは実行時にしか判明しない。
pub(crate) struct LazyRegex {
    source: &'static str,
    cell: OnceLock<Regex>,
}

impl LazyRegex {
    pub(crate) const fn new(source: &'static str) -> Self {
        Self {
            source,
            cell: OnceLock::new(),
        }
    }

    /// # Panics
    ///
    /// i18n の正規表現が `regex` クレートで解釈できない場合（Ruby の `RegexpError` 相当）。
    fn get(&self) -> &Regex {
        self.cell
            .get_or_init(|| Regex::new(self.source).expect("valid i18n regexp"))
    }
}

/// Ruby `BeginningIdol::ChainTable`。項目が配列で、要素ごとに連鎖しうる。
pub(crate) struct BiChainTable {
    name: &'static str,
    times: i64,
    sides: i64,
    items: &'static [&'static [Node]],
}

impl BiChainTable {
    pub(crate) const fn new(
        name: &'static str,
        times: i64,
        sides: i64,
        items: &'static [&'static [Node]],
    ) -> Self {
        Self {
            name,
            times,
            sides,
            items,
        }
    }
}

impl RollText for BiChainTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_sum(self.times, self.sides)?;
        let index = value - self.times;
        let body = match usize::try_from(index).ok().and_then(|i| self.items.get(i)) {
            Some(nodes) => resolve_nodes(nodes, rng)?,
            None => String::new(),
        };
        Ok(format!("{}({}) ＞ {}", self.name, value, body))
    }
}

/// Ruby `BeginningIdol::ChainD66Table`。D66は常に昇順に並べ替える。
pub(crate) struct BiChainD66Table {
    name: &'static str,
    items: &'static [(i64, &'static [Node])],
}

impl BiChainD66Table {
    pub(crate) const fn new(name: &'static str, items: &'static [(i64, &'static [Node])]) -> Self {
        Self { name, items }
    }
}

impl RollText for BiChainD66Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = roll_d66_asc(rng)?;
        let body = match self.items.iter().find(|(key, _)| *key == value) {
            Some((_, nodes)) => resolve_nodes(nodes, rng)?,
            None => String::new(),
        };
        Ok(format!("{}({}) ＞ {}", self.name, value, body))
    }
}

/// Ruby `BeginningIdol::D6TwiceTable`（無茶ぶり表）。2D6を並べ替えずに使う。
pub(crate) struct D6TwiceTable {
    name: &'static str,
    items1: &'static [&'static str],
    items2: &'static [&'static str],
}

impl D6TwiceTable {
    pub(crate) const fn new(
        name: &'static str,
        items1: &'static [&'static str],
        items2: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            items1,
            items2,
        }
    }
}

impl RollText for D6TwiceTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let dice = rng.roll_barabara(2, 6)?;
        let value1 = dice.first().copied().unwrap_or(0);
        let value2 = dice.get(1).copied().unwrap_or(0);
        Ok(format!(
            "{}[{},{}] ＞ {}{}",
            self.name,
            value1,
            value2,
            item_at(self.items1, value1 - 1),
            item_at(self.items2, value2 - 1),
        ))
    }
}

/// Ruby `BeginningIdol::BadStatusTable`（変調表 `BT[n]`）。
pub(crate) struct BadStatusTable {
    name: &'static str,
    prefix_format: &'static str,
    items: &'static [&'static str],
}

impl BadStatusTable {
    pub(crate) const fn new(
        name: &'static str,
        prefix_format: &'static str,
        items: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            prefix_format,
            items,
        }
    }

    /// Ruby `#roll_command`: `/^BT(\d+)?$/`。
    fn roll_command(
        &self,
        rng: &mut Randomizer,
        command: &str,
    ) -> Result<Option<String>, EvalError> {
        let Some(counts) = parse_count_command(command, "BT") else {
            return Ok(None);
        };
        self.roll(rng, counts)
    }

    /// Ruby `#roll(randomizer, roll_counts)`。
    fn roll(&self, rng: &mut Randomizer, counts: i64) -> Result<Option<String>, EvalError> {
        if counts <= 0 {
            return Ok(None);
        }

        let mut dice_list = rng.roll_barabara(counts, 6)?;
        dice_list.sort_unstable();
        // Ruby `Array#uniq` は出現順を保った重複除去。昇順なので出目の昇順になる。
        let mut index_list: Vec<i64> = Vec::new();
        for dice in &dice_list {
            if !index_list.contains(dice) {
                index_list.push(*dice);
            }
        }

        let prefix = if index_list.len() > 1 {
            let count = index_list.len().to_string();
            format!(
                "{}\n",
                format_named(self.prefix_format, &[("count_bad_status", &count)])
            )
        } else {
            String::new()
        };
        let body = index_list
            .iter()
            .map(|i| item_at(self.items, i - 1))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Some(format!(
            "{} ＞ [{}] ＞ {prefix}{body}",
            self.name,
            join_i64(&dice_list)
        )))
    }
}

/// Ruby `BeginningIdol::ItemTable`（アイテム表 `IT[n]`）。
pub(crate) struct ItemTable {
    name: &'static str,
    emph: &'static str,
    counting: &'static str,
    sep: &'static str,
    items: &'static [&'static str],
}

impl ItemTable {
    pub(crate) const fn new(
        name: &'static str,
        emph: &'static str,
        counting: &'static str,
        sep: &'static str,
        items: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            emph,
            counting,
            sep,
            items,
        }
    }

    /// Ruby `#roll_command`: `/^IT(\d+)?$/`。
    fn roll_command(
        &self,
        rng: &mut Randomizer,
        command: &str,
    ) -> Result<Option<String>, EvalError> {
        let Some(counts) = parse_count_command(command, "IT") else {
            return Ok(None);
        };
        self.roll(rng, counts)
    }

    /// Ruby `#roll(randomizer, roll_counts)`。
    fn roll(&self, rng: &mut Randomizer, counts: i64) -> Result<Option<String>, EvalError> {
        // Ruby: return nil if roll_counts == 0
        if counts == 0 {
            return Ok(None);
        }

        let mut dice_list = rng.roll_barabara(counts, 6)?;
        dice_list.sort_unstable();
        // Ruby `group_by(&:itself)` は出現順にグループを並べる。
        // dice_list は昇順なので、隣接まとめと同じ順序・同じ内容になる。
        let mut groups: Vec<(i64, usize)> = Vec::new();
        for dice in &dice_list {
            match groups.last_mut() {
                Some((value, count)) if value == dice => *count += 1,
                _ => groups.push((*dice, 1)),
            }
        }

        let mut parts = Vec::with_capacity(groups.len());
        for (dice, count) in &groups {
            let mut item = item_at(self.items, dice - 1).to_string();
            if groups.len() != 1 {
                item = format_named(self.emph, &[("item", &item)]);
            }
            if dice_list.len() != groups.len() {
                item = format_named(
                    self.counting,
                    &[("item", &item), ("count", &count.to_string())],
                );
            }
            parts.push(item);
        }

        Ok(Some(format!(
            "{} ＞ [{}] ＞ {}",
            self.name,
            join_i64(&dice_list),
            parts.join(self.sep)
        )))
    }
}

impl RollText for ItemTable {
    /// Ruby `ItemTable#roll(randomizer)`（`roll_counts` 既定値1）。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng, 1)?.unwrap_or_default())
    }
}

/// [`MySkillNameTable`] が連鎖させる表。Ruby では `Table` と `D66Table` が混ざる。
pub(crate) enum SubTable {
    /// Ruby `DiceTable::Table`（称号表）
    Plain(&'static Table),
    /// Ruby `DiceTable::D66Table`
    D66(&'static D66Table),
}

impl SubTable {
    fn roll(&self, rng: &mut Randomizer) -> Result<RollResult, EvalError> {
        match self {
            SubTable::Plain(table) => table.roll(rng),
            SubTable::D66(table) => table.roll(rng),
        }
    }
}

/// Ruby `BeginningIdol::MySkillNameTable`（マイスキル名決定表 `MS`）。
pub(crate) struct MySkillNameTable {
    name: &'static str,
    formats: &'static [&'static str],
    chains: &'static [&'static [SubTable]],
}

impl MySkillNameTable {
    pub(crate) const fn new(
        name: &'static str,
        formats: &'static [&'static str],
        chains: &'static [&'static [SubTable]],
    ) -> Self {
        Self {
            name,
            formats,
            chains,
        }
    }
}

impl RollText for MySkillNameTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let index = rng.roll_once(6)?;
        let slot = usize::try_from(index - 1).ok();
        let Some(chain) = slot.and_then(|i| self.chains.get(i)) else {
            return Ok(String::new());
        };

        let mut chosen = Vec::with_capacity(chain.len());
        for table in *chain {
            chosen.push(table.roll(rng)?);
        }

        let dice = chosen
            .iter()
            .map(|result| format!("{}{}", result.table_name(), result.value()))
            .collect::<Vec<_>>()
            .join(",");
        let bodies: Vec<&str> = chosen.iter().map(|result| result.last_body()).collect();
        let skill_name_format = slot
            .and_then(|i| self.formats.get(i))
            .copied()
            .unwrap_or_default();

        Ok(format!(
            "{} ＞ [{index},{dice}] ＞ {}",
            self.name,
            format_positional(skill_name_format, &bodies)
        ))
    }
}

/// Ruby `BeginningIdol::RandomEventTable`（ランダムイベント `RE`）。
pub(crate) struct RandomEventTable {
    name: &'static str,
    format: &'static str,
    on_name: &'static str,
    on_items: &'static [(i64, &'static str, i64)],
    off_name: &'static str,
    off_items: &'static [(i64, &'static str, i64)],
}

impl RandomEventTable {
    pub(crate) const fn new(
        name: &'static str,
        format: &'static str,
        on_name: &'static str,
        on_items: &'static [(i64, &'static str, i64)],
        off_name: &'static str,
        off_items: &'static [(i64, &'static str, i64)],
    ) -> Self {
        Self {
            name,
            format,
            on_name,
            on_items,
            off_name,
            off_items,
        }
    }
}

impl RollText for RandomEventTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let first_index = rng.roll_once(6)?;
        let d66_index = rng.roll_d66(D66SortType::NoSort)?;

        // Ruby: first_index.even? ? on_event : off_event
        let (table_name, items) = if first_index % 2 == 0 {
            (self.on_name, self.on_items)
        } else {
            (self.off_name, self.off_items)
        };

        let (event, page) = items
            .iter()
            .find(|(key, _, _)| *key == d66_index)
            .map_or(("", 0), |(_, event, page)| (*event, *page));
        let body = format_named(
            self.format,
            &[("event", event), ("page", &page.to_string())],
        );

        Ok(format!(
            "{} ＞ (1D6) ＞ {first_index}\n{table_name} ＞ [{d66_index}] ＞ {body}",
            self.name
        ))
    }
}

/// Ruby `BeginningIdol::WorkWithChanceTable`（地方アイドル仕事表 `LO[n]`）。
pub(crate) struct WorkWithChanceTable {
    table: D66Table,
    regexp: LazyRegex,
    off_text: &'static str,
}

impl WorkWithChanceTable {
    pub(crate) const fn new(table: D66Table, regexp: &'static str, off_text: &'static str) -> Self {
        Self {
            table,
            regexp: LazyRegex::new(regexp),
            off_text,
        }
    }

    /// Ruby `#roll_command`: `/^LO([1-6]{1,2})?$/`。
    fn roll_command(
        &self,
        rng: &mut Randomizer,
        command: &str,
    ) -> Result<Option<String>, EvalError> {
        let Some(rest) = command.strip_prefix("LO") else {
            return Ok(None);
        };
        let chance = if rest.is_empty() {
            None
        } else if rest.len() <= 2 && rest.bytes().all(|b| (b'1'..=b'6').contains(&b)) {
            rest.parse::<i64>().ok()
        } else {
            return Ok(None);
        };

        Ok(Some(self.roll(rng, chance)?))
    }

    /// Ruby `#roll(randomizer, chance)`。
    fn roll(&self, rng: &mut Randomizer, chance: Option<i64>) -> Result<String, EvalError> {
        let chosen = self.table.roll(rng)?;
        let Some(chance) = chance else {
            return Ok(chosen.to_string());
        };

        let body = chosen.last_body();
        let regexp = self.regexp.get();
        let Some(captures) = regexp.captures(body) else {
            return Ok(chosen.to_string());
        };

        let value: i64 = captures
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let new_body = if value >= chance {
            self.off_text.to_string()
        } else {
            regexp.replace(body, NoExpand("")).into_owned()
        };

        Ok(format!(
            "{}({}) ＞ {new_body}",
            chosen.table_name(),
            chosen.value()
        ))
    }
}

/// Ruby `BeginningIdol::WithAbnormality`。表の結果に含まれる
/// 「変調がランダムに◯つ発生する。」を変調表の結果へ置き換える。
pub(crate) struct Abnormality {
    regexp: LazyRegex,
    num_map: &'static [&'static str],
    bad_status_table: &'static BadStatusTable,
}

impl Abnormality {
    pub(crate) const fn new(
        regexp: &'static str,
        num_map: &'static [&'static str],
        bad_status_table: &'static BadStatusTable,
    ) -> Self {
        Self {
            regexp: LazyRegex::new(regexp),
            num_map,
            bad_status_table,
        }
    }

    /// Ruby `#replace_abnormality(chosen, randomizer)` + `RollResult#to_s`。
    fn replace(&self, chosen: &RollResult, rng: &mut Randomizer) -> Result<String, EvalError> {
        let body = chosen.last_body();
        let regexp = self.regexp.get();
        let Some(captures) = regexp.captures(body) else {
            return Ok(chosen.to_string());
        };

        // Ruby `#kanji_to_i`: num_map の位置 + 1
        let kanji = captures.get(1).map_or("", |m| m.as_str());
        let Some(count) = self.num_map.iter().position(|n| *n == kanji) else {
            return Ok(chosen.to_string());
        };

        let Some(text) = self.bad_status_table.roll(rng, count as i64 + 1)? else {
            return Ok(chosen.to_string());
        };
        let new_body = regexp.replace(body, NoExpand(&text));

        Ok(format!(
            "{}({}) ＞ {new_body}",
            chosen.table_name(),
            chosen.value()
        ))
    }
}

/// [`AbnormalityTable`] の土台。Ruby の `D66WithAbnormality` / `TableWithAbnormality`。
pub(crate) enum AbnormalitySource {
    /// Ruby `D66WithAbnormality`
    D66(D66Table),
    /// Ruby `TableWithAbnormality`
    Plain(Table),
}

/// Ruby `D66WithAbnormality` / `TableWithAbnormality`。
pub(crate) struct AbnormalityTable {
    source: AbnormalitySource,
    abnormality: &'static Abnormality,
}

impl AbnormalityTable {
    pub(crate) const fn new(source: AbnormalitySource, abnormality: &'static Abnormality) -> Self {
        Self {
            source,
            abnormality,
        }
    }
}

impl RollText for AbnormalityTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let chosen = match &self.source {
            AbnormalitySource::D66(table) => table.roll(rng)?,
            AbnormalitySource::Plain(table) => table.roll(rng)?,
        };
        self.abnormality.replace(&chosen, rng)
    }
}

/// Ruby `BeginningIdol::SkillTable < DiceTable::SaiFicSkillTable`。
pub(crate) struct SkillTable {
    inner: SaiFicSkillTable,
}

impl SkillTable {
    pub(crate) const fn new(inner: SaiFicSkillTable) -> Self {
        Self { inner }
    }

    fn roll_command(
        &self,
        rng: &mut Randomizer,
        command: &str,
    ) -> Result<Option<String>, EvalError> {
        self.inner.roll_command(rng, command)
    }

    fn roll_skill(&self, rng: &mut Randomizer) -> Result<Option<SaiFicSkill>, EvalError> {
        self.inner.roll_skill(rng)
    }
}

impl RollText for SkillTable {
    /// Ruby `SkillTable#roll`: `roll_command(randomizer, "RTT")`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll_command(rng, "RTT")?.unwrap_or_default())
    }
}

/// Ruby `BeginningIdol::SkillHometown`。出身分野の特技表（`AT6`）を引く。
pub(crate) struct SkillHometown {
    skill_table: &'static SkillTable,
}

impl SkillHometown {
    pub(crate) const fn new(skill_table: &'static SkillTable) -> Self {
        Self { skill_table }
    }
}

impl RollText for SkillHometown {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self
            .skill_table
            .roll_command(rng, "AT6")?
            .unwrap_or_default())
    }
}

/// Ruby `BeginningIdol::SkillGetTable`（アイドルスキル修得表 `SGT` / `RS`）。
///
/// 引いた項目が「◯分野が出たら振り直し」だった場合、対象外の分野が出るまで特技表を引き続ける。
pub(crate) struct SkillGetTable {
    table: Table,
    skill_table: &'static SkillTable,
    reroll_reg: LazyRegex,
    reroll: &'static str,
    secondary_name: &'static str,
    /// Ruby `Skill#to_s` の書式（`SaiFicSkillTable` の `s_format`）。
    skill_format: &'static str,
}

impl SkillGetTable {
    pub(crate) const fn new(
        table: Table,
        skill_table: &'static SkillTable,
        reroll_reg: &'static str,
        reroll: &'static str,
        secondary_name: &'static str,
        skill_format: &'static str,
    ) -> Self {
        Self {
            table,
            skill_table,
            reroll_reg: LazyRegex::new(reroll_reg),
            reroll,
            secondary_name,
            skill_format,
        }
    }
}

impl RollText for SkillGetTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let chosen = self.table.roll(rng)?;
        let body = chosen.last_body();
        let Some(captures) = self.reroll_reg.get().captures(body) else {
            return Ok(chosen.to_string());
        };

        // Ruby `m.captures`。nil のグループは `include?` に一致しないので落としてよい。
        let reroll_category: Vec<&str> = captures
            .iter()
            .skip(1)
            .flatten()
            .map(|m| m.as_str())
            .collect();

        let mut new_body = format!("{body}\n");
        while let Some(skill) = self.skill_table.roll_skill(rng)? {
            new_body.push_str(&format!(
                "{} ＞ [{},{}] ＞ {}",
                self.secondary_name,
                skill.category_dice,
                skill.row_dice,
                skill.format_with(self.skill_format)
            ));
            if !reroll_category.contains(&skill.category_name) {
                break;
            }
            new_body.push_str(&format!(" ＞ {}\n", self.reroll));
        }

        Ok(format!(
            "{}({}) ＞ {new_body}",
            chosen.table_name(),
            chosen.value()
        ))
    }
}

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`BeginningIdol` と `BeginningIdol_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    pub(crate) skill_table: &'static SkillTable,
    pub(crate) item_table: &'static ItemTable,
    pub(crate) bad_status_table: &'static BadStatusTable,
    pub(crate) local_work_table: &'static WorkWithChanceTable,
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
    pub(crate) tables: &'static [(&'static str, &'static dyn RollText)],
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
    /// i18n `BeginningIdol.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `BeginningIdol.special`
    pub(crate) special: &'static str,
    /// i18n `BeginningIdol.burst.name`
    pub(crate) burst_name: &'static str,
    /// i18n `BeginningIdol.burst.burst`
    pub(crate) burst_burst: &'static str,
    /// i18n `BeginningIdol.burst.critical_success`
    pub(crate) burst_critical_success: &'static str,
    /// i18n `BeginningIdol.burst.success`
    pub(crate) burst_success: &'static str,
    /// i18n `BeginningIdol.attack.name`
    pub(crate) attack_name: &'static str,
    /// i18n `BeginningIdol.attack.damage`
    pub(crate) attack_damage: &'static str,
    /// i18n `BeginningIdol.PD.paformance`（原典どおりのつづり）
    pub(crate) pd_paformance: &'static str,
    /// i18n `BeginningIdol.PD.symphony`
    pub(crate) pd_symphony: &'static str,
    /// i18n `BeginningIdol.PD.miracle`
    pub(crate) pd_miracle: &'static str,
    /// i18n `BeginningIdol.PD.perfect_miracle`
    pub(crate) pd_perfect_miracle: &'static str,
    /// i18n `BeginningIdol.PD.miracle_synchro`
    pub(crate) pd_miracle_synchro: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `BeginningIdol#result_nd6`。
pub(crate) fn check_result_nd6(
    tables: &SystemTables,
    total: crate::Int,
    dice_total: i64,
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    // Ruby: return nil if target == '?'
    let Target::Number(target) = target else {
        return None;
    };
    // Ruby: return nil unless cmp_op == :>=
    if cmp_op != CmpOp::Ge {
        return None;
    }

    let result = if dice_total <= 2 {
        EvalResult::fumble(tables.fumble)
    } else if dice_total >= 12 {
        EvalResult::critical(tables.special)
    } else if total >= target {
        EvalResult::success(tables.success)
    } else {
        EvalResult::failure(tables.failure)
    };

    Some(CheckOutcome::Result(Box::new(result)))
}

/// Ruby `BeginningIdol#eval_game_system_specific_command`（`||` の連鎖）。
pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let text = match roll_attack(tables, command, rng)? {
        Some(text) => Some(text),
        None => match roll_burst(tables, command, rng)? {
            Some(text) => Some(text),
            None => match roll_performance(tables, command, rng)? {
                Some(text) => Some(text),
                None => match tables.skill_table.roll_command(rng, command)? {
                    Some(text) => Some(text),
                    None => match tables.item_table.roll_command(rng, command)? {
                        Some(text) => Some(text),
                        None => match tables.bad_status_table.roll_command(rng, command)? {
                            Some(text) => Some(text),
                            None => match tables.local_work_table.roll_command(rng, command)? {
                                Some(text) => Some(text),
                                None => roll_tables(tables, command, rng)?,
                            },
                        },
                    },
                },
            },
        },
    };

    Ok(text.map(SpecificCommandOutput::text))
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

/// Ruby `roll_attack`: `/^(\d+)(S?)A([1-6]*)([+-]\d+)?$/`。
fn roll_attack(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let pattern =
        RE.get_or_init(|| Regex::new(r"^(\d+)(S?)A([1-6]*)([+-]\d+)?$").expect("valid regex"));

    let Some(captures) = pattern.captures(command) else {
        return Ok(None);
    };

    let counts = parse_i64(&captures[1]);
    if counts <= 0 {
        return Ok(None);
    }

    let sure = !captures[2].is_empty();
    let remove = digits(&captures[3]);
    let adjust = captures.get(4).map_or(0, |m| parse_i64(m.as_str()));
    let adjust_str = modifier(&crate::Int::from(adjust));

    let mut dice = rng.roll_barabara(counts, 6)?;
    dice.sort_unstable();
    let dice_str = join_i64(&dice);

    // Ruby `Array#-` は該当する出目をすべて取り除く。
    dice.retain(|value| !remove.contains(value));

    let mut text = format!("{} ＞ [{dice_str}]{adjust_str} ＞ ", tables.attack_name);

    if dice.len() as i64 != counts && !dice.is_empty() {
        text.push_str(&format!("[{}]{adjust_str} ＞ ", join_i64(&dice)));
    }

    // dice は昇順のままなので、隣接比較で `uniq` との個数一致を判定できる。
    let all_uniq = dice.windows(2).all(|pair| pair[0] != pair[1]);
    if sure || all_uniq {
        let total = (dice.iter().sum::<i64>() + adjust).max(0);
        text.push_str(&format_named(
            tables.attack_damage,
            &[("total", &total.to_string())],
        ));
    } else {
        text.push_str(tables.failure);
    }

    Ok(Some(text))
}

/// Ruby `roll_burst`: `/^(\d{2})C$/`（アイドル熱湯風呂）。
fn roll_burst(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let pattern = RE.get_or_init(|| Regex::new(r"^(\d{2})C$").expect("valid regex"));

    let Some(captures) = pattern.captures(command) else {
        return Ok(None);
    };

    let degrees = parse_i64(&captures[1]);
    if !(45..=55).contains(&degrees) {
        return Ok(None);
    }

    let counts = if degrees <= 49 {
        3
    } else if degrees <= 52 {
        4
    } else if degrees <= 54 {
        5
    } else {
        6
    };

    let mut dice_list = rng.roll_barabara(counts, 6)?;
    dice_list.sort_unstable();
    let total = dice_list.iter().sum::<i64>() + degrees;

    let result = if total >= 80 {
        tables.burst_burst
    } else if total >= 75 {
        tables.burst_critical_success
    } else if total >= 65 {
        tables.burst_success
    } else {
        tables.failure
    };

    Ok(Some(format!(
        "{} ＞ {degrees}+[{}] ＞ {total} ＞ {result}",
        tables.burst_name,
        join_i64(&dice_list)
    )))
}

/// Ruby `roll_performance`: `/^([1-7]*)PD(\d+)([+-]\d+)?$/`。
fn roll_performance(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let pattern =
        RE.get_or_init(|| Regex::new(r"^([1-7]*)PD(\d+)([+-]\d+)?$").expect("valid regex"));

    let Some(captures) = pattern.captures(command) else {
        return Ok(None);
    };

    let counts = parse_i64(&captures[2]);
    if counts <= 0 {
        return Ok(None);
    }

    let mut carry = digits(&captures[1]);
    carry.sort_unstable();
    let modifier_value = captures.get(3).map_or(0, |m| parse_i64(m.as_str()));

    let mut dice_list = rng.roll_barabara(counts, 6)?;
    dice_list.sort_unstable();

    let mut all_dice = dice_list.clone();
    all_dice.extend_from_slice(&carry);
    all_dice.sort_unstable();
    let filtered = select_uniqs(&all_dice);

    let title = if carry.is_empty() {
        tables.pd_paformance
    } else {
        tables.pd_symphony
    };

    let result = if carry.is_empty() {
        result_performance(tables, &filtered, modifier_value, &all_dice)
    } else {
        result_symphony(tables, &filtered, modifier_value)
    };

    let dice_part = if carry.is_empty() {
        format!(
            "[{}]{}",
            join_i64(&dice_list),
            modifier(&crate::Int::from(modifier_value))
        )
    } else {
        format!(
            "[{}],[{}]{}",
            join_i64(&dice_list),
            join_i64(&carry),
            modifier(&crate::Int::from(modifier_value))
        )
    };

    Ok(Some([title.to_string(), dice_part, result].join(" ＞ ")))
}

/// Ruby `select_uniqs`。1度しか出ていない出目だけを昇順で返す。
fn select_uniqs(dice_list: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = dice_list
        .iter()
        .filter(|value| dice_list.iter().filter(|other| other == value).count() == 1)
        .copied()
        .collect();
    out.sort_unstable();
    out
}

/// Ruby `result_performance`。
fn result_performance(
    tables: &SystemTables,
    list: &[i64],
    modifier_value: i64,
    all_list: &[i64],
) -> String {
    if list.is_empty() {
        format_named(
            tables.pd_miracle,
            &[("value", &(modifier_value + 10).to_string())],
        )
    } else if list == [1, 2, 3, 4, 5, 6] {
        format_named(
            tables.pd_perfect_miracle,
            &[("value", &(modifier_value + 30).to_string())],
        )
    } else if list.len() != all_list.len() {
        format!(
            "[{}]{} ＞ {}",
            join_i64(list),
            modifier(&crate::Int::from(modifier_value)),
            list.iter().sum::<i64>() + modifier_value
        )
    } else {
        (list.iter().sum::<i64>() + modifier_value).to_string()
    }
}

/// Ruby `result_symphony`。
fn result_symphony(tables: &SystemTables, list: &[i64], modifier_value: i64) -> String {
    if list.is_empty() {
        return format_named(
            tables.pd_miracle_synchro,
            &[("value", &(modifier_value + 15).to_string())],
        );
    }

    let tail = if list == [1, 2, 3, 4, 5, 6] {
        format_named(
            tables.pd_perfect_miracle,
            &[("value", &(modifier_value + 30).to_string())],
        )
    } else {
        (list.iter().sum::<i64>() + modifier_value).to_string()
    };

    format!(
        "[{}]{} ＞ {tail}",
        join_i64(list),
        modifier(&crate::Int::from(modifier_value))
    )
}

// ---------------------------------------------------------------------------
// 小物
// ---------------------------------------------------------------------------

/// Ruby `randomizer.roll_barabara(2, 6).sort` から D66 の値を作る。
fn roll_d66_asc(rng: &mut Randomizer) -> Result<i64, EvalError> {
    let dice = rng.roll_barabara(2, 6)?;
    let mut pair = [
        dice.first().copied().unwrap_or(0),
        dice.get(1).copied().unwrap_or(0),
    ];
    pair.sort_unstable();
    Ok(pair[0] * 10 + pair[1])
}

/// Ruby `/^{prefix}(\d+)?$/` のコマンド解釈。数値部の省略は Ruby の `|| 1` と同じ。
fn parse_count_command(command: &str, prefix: &str) -> Option<i64> {
    let rest = command.strip_prefix(prefix)?;
    if rest.is_empty() {
        return Some(1);
    }
    if !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(parse_i64(rest))
}

/// Ruby `String#to_i`。桁あふれは Ruby では多倍長になるが、
/// ダイス個数上限（200）を超えた時点で結果は変わらないので飽和させる。
fn parse_i64(text: &str) -> i64 {
    text.parse().unwrap_or(i64::MAX)
}

/// Ruby `str.each_char.map(&:to_i)`（`[1-7]` のみが来る）。
fn digits(text: &str) -> Vec<i64> {
    text.chars()
        .filter_map(|c| c.to_digit(10).map(i64::from))
        .collect()
}

/// 添字が範囲外なら空文字列（Ruby の `items[i]` が nil のとき `to_s` は `""`）。
fn item_at(items: &'static [&'static str], index: i64) -> &'static str {
    usize::try_from(index)
        .ok()
        .and_then(|i| items.get(i))
        .copied()
        .unwrap_or_default()
}

/// Ruby `array.join(",")`。
fn join_i64(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `format(template, name: value)` の `%{name}` 参照だけを解釈する。
///
/// i18n 側で BeginningIdol が使う書式は `%{total}` / `%{value}` / `%{item}` /
/// `%{count}` / `%{count_bad_status}` / `%{event}` / `%{page}` の7種で、
/// いずれも幅・型指定を伴わない（`%<name>d` 形式は `SaiFicSkillTable` 側にしかない）。
fn format_named(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("%{{{name}}}"), value);
    }
    out
}

/// Ruby `format(template, *args)` の `%s` 参照だけを順に埋める（マイスキル名決定表）。
fn format_positional(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut args = args.iter();
    while let Some(pos) = rest.find("%s") {
        out.push_str(&rest[..pos]);
        out.push_str(args.next().copied().unwrap_or_default());
        rest = &rest[pos + "%s".len()..];
    }
    out.push_str(rest);
    out
}

static JA_SKILL_CATEGORY1: &[&str] = &[
    "～125", "131", "136", "141", "146", "156", "166", "171", "176", "180", "190～",
];

static JA_SKILL_CATEGORY2: &[&str] = &[
    "エスニック",
    "ダーク",
    "セクシー",
    "フェミニン",
    "キュート",
    "プレーン",
    "パッション",
    "ポップ",
    "バーニング",
    "クール",
    "スター",
];

static JA_SKILL_CATEGORY3: &[&str] = &[
    "異国文化",
    "スタイル",
    "集中力",
    "胆力",
    "体力",
    "笑顔",
    "運動神経",
    "気配り",
    "学力",
    "セレブ",
    "演技力",
];

static JA_SKILL_CATEGORY4: &[&str] = &[
    "中二病",
    "ミステリアス",
    "マイペース",
    "軟派",
    "語尾",
    "キャラ分野の空白",
    "元気",
    "硬派",
    "物腰丁寧",
    "どじ",
    "ばか",
];

static JA_SKILL_CATEGORY5: &[&str] = &[
    "オカルト",
    "ペット",
    "スポーツ",
    "おしゃれ",
    "料理",
    "趣味分野の空白",
    "ショッピング",
    "ダンス",
    "ゲーム",
    "音楽",
    "アイドル",
];

static JA_SKILL_CATEGORY6: &[&str] = &[
    "沖縄",
    "九州地方",
    "四国地方",
    "中国地方",
    "近畿地方",
    "中部地方",
    "関東地方",
    "北陸地方",
    "東北地方",
    "北海道",
    "海外",
];

static JA_SKILL_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("身長", JA_SKILL_CATEGORY1),
    SaiFicCategory::new("属性", JA_SKILL_CATEGORY2),
    SaiFicCategory::new("才能", JA_SKILL_CATEGORY3),
    SaiFicCategory::new("キャラ", JA_SKILL_CATEGORY4),
    SaiFicCategory::new("趣味", JA_SKILL_CATEGORY5),
    SaiFicCategory::new("出身", JA_SKILL_CATEGORY6),
];

/// Ruby `translate_skill_table(:ja_jp)`。`rtt:`/`rttn:` は `AT`/`AT1`〜`AT6`。
static JA_SKILL_TABLE: SkillTable = SkillTable::new(
    SaiFicSkillTable::new(JA_SKILL_CATEGORIES)
        .with_commands(
            Some("AT"),
            None,
            &["AT1", "AT2", "AT3", "AT4", "AT5", "AT6"],
        )
        .with_formats(SaiFicFormats {
            rtt: DEFAULT_RTT_FORMAT,
            rct: DEFAULT_RCT_FORMAT,
            rttn: DEFAULT_RTTN_FORMAT,
            skill: DEFAULT_SKILL_FORMAT,
        }),
);

static JA_ITEM_TABLE_ITEMS: &[&str] = &[
    "スタミナドリンク",
    "トレーニングウェア",
    "ドリーミングシューズ",
    "キャラアイテム",
    "お菓子",
    "差し入れ",
];

/// Ruby `ItemTable.new(:ja_jp)`。
static JA_ITEM_TABLE: ItemTable = ItemTable::new(
    "アイテム",
    "「%{item}」",
    "%{item}%{count}つ",
    "と",
    JA_ITEM_TABLE_ITEMS,
);

static JA_BAD_STATUS_ITEMS: &[&str] = &[
    "「不穏な空気」　PCの【メンタル】が減少するとき、減少する数値が1点上昇する",
    "「微妙な距離感」　【理解度】が上昇しなくなる",
    "「ガラスの心」　PCのファンブル値が1点上昇する",
    "「怪我」　幕間のとき、プロデューサーは「回想」しか行えない",
    "「信じきれない」　PC全員の【理解度】を1点低いものとして扱う",
    "「すれ違い」　PCはアイテムの使用と、リザルトフェイズに「おねがい」をすることができなくなる",
];

/// Ruby `BadStatusTable.new(:ja_jp)`。
static JA_BAD_STATUS_TABLE: BadStatusTable = BadStatusTable::new(
    "変調",
    "以下の%{count_bad_status}つが発生する。",
    JA_BAD_STATUS_ITEMS,
);

static JA_LOCAL_WORK_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("握手会をすることになった。遠方から自分たち目当てにやって来るお客さんも多数見える。チャンスが5以下ならオフ。\n特技 : 《アイドル／趣味12》")),
    (14, TableItem::Text("ミニコンサートが全国放送で小さく紹介される。ちょっとだけ、外見が注目されたみたいだ。チャンスが4以下ならオフ。\n特技 : 《スタイル／才能3》")),
    (15, TableItem::Text("地元ラジオ局で自分たちの番組が始まる。チャンスが3以下ならオフ。\n特技 : 《キャラ分野の空白／趣味7》")),
    (16, TableItem::Text("地元のテレビ局にゲスト出演。うまく自分たちを紹介できるだろうか？　チャンスが3以下ならオフ。\n特技 : 好きな出身分野の特技")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("街頭でティッシュ配りの手伝いをする。笑顔を忘れずに、がんばろう。\n特技 : 《笑顔／才能7》")),
    (24, TableItem::Text("地元のお手伝いの一環として、害虫退治に駆り出された。なぜ、こんなことに。\n特技 : 《胆力／才能5》")),
    (25, TableItem::Text("畑仕事のお手伝いをすることになった。とりあえず、体力が要求される。\n特技 : 《体力／才能6》")),
    (26, TableItem::Text("ショッピングモールのお手伝いをすることになった。うまくお手伝いができれば、繁盛するかも。\n特技 : 《ショッピング／趣味8》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("インターネットラジオに出演。声とトークで。地域のことを伝えていこう。\n特技 : 《異国文化／才能2》")),
    (35, TableItem::Text("地元のテレビ局の取材が入る。テーマは、地方でがんばっている人たちだ。\n特技 : 《元気／キャラ8》")),
    (36, TableItem::Text("デパートで風船を配るお手伝い。子どもたち相手のお仕事は、ちょっと大変です。\n特技 : 《気配り／才能9》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("着ぐるみを着て、市民と交流。暑くてつらい仕事だけど、大切な交流の時間です。\n特技 : 《バーニング／属性10》")),
    (46, TableItem::Text("観光地の物販コーナーで地元の特産品を売るお手伝い。地方アイドル的に、大切なお仕事。\n特技 : 好きな出身分野の特技")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("動画サイトのチャンネルで、自分たちの宣伝を行なうことに。世界中に発信！\n特技 : 《スター／属性12》")),
    (66, TableItem::Text("オフ")),
];

/// Ruby `translate_local_work_table(:ja_jp)`（`LO[n]`）。
static JA_LOCAL_WORK_TABLE: WorkWithChanceTable = WorkWithChanceTable::new(
    D66Table::new("地方アイドル仕事表", D66SortType::Asc, JA_LOCAL_WORK_ITEMS),
    "チャンスが(\\d{1,2})以下ならオフ。",
    "オフ",
);

static JA_ABNORMALITY_NUM_MAP: &[&str] = &["一", "二", "三"];

/// Ruby `WithAbnormality`（`変調がランダムにN つ発生する。` の置換）。
static JA_ABNORMALITY: Abnormality = Abnormality::new(
    "変調がランダムに(一|二|三)つ発生する。",
    JA_ABNORMALITY_NUM_MAP,
    &JA_BAD_STATUS_TABLE,
);

static JA_DT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("１２＆８８\n自分の【パフォーマンス値】が決定したとき、その値を2点上昇する。")),
    (12, TableItem::Text("Glow Up Princess\nパフォーマンスを行うとき、サイコロを追加で1つ振れる。")),
    (13, TableItem::Text("しずく\nライブフェイズ開始時に、【メンタル】が5点上昇する。")),
    (14, TableItem::Text("Pop☆Sweet\n自分の【メンタル】が上昇するとき、さらに1点上昇する。")),
    (15, TableItem::Text("Ttype\n一芸突破をしても【メンタル】が減少しない。また、一芸突破をした時、達成値が1点上昇する。")),
    (16, TableItem::Text("Vampire Story\nパフォーマンスの【パフォーマンス値】が10以上の場合、自分の【メンタル】が3点上昇する。")),
    (22, TableItem::Text("Pure Mermaid\n【ビジュアル】の演目は、指定特技を《スタイル》に変更できる。指定特技が《スタイル》の演目では、【パフォーマンス値】が2点上昇する。")),
    (23, TableItem::Text("I'm cute\nライブフェイズ開始時に【メンタル】が1点上昇する。幕間の開始時に能力値を1つ選ぶ。選ばれた能力値は、このライブフェイズの間、1点上昇する。")),
    (24, TableItem::Text("No.1 Girl\n【パフォーマンス値】が決定するとき、【メンタル】を1点減少することで、【パフォーマンス値】が3点上昇する。")),
    (25, TableItem::Text("Final Romance\n【ビジュアル】のパフォーマンスを行うとき、キャラクターを1人選ぶ。選んだキャラクターの自分に対する【理解度】と同じだけ、【パフォーマンス値】が上昇する。")),
    (26, TableItem::Text("Prism Line\nパフォーマンス1回につき、1度だけパフォーマンスに使用したサイコロ1つを振り直すことができる。")),
    (33, TableItem::Text("さーばんとさーびす\nシンフォニーを行うたびに、そのパフォーマンスの【パフォーマンス値】が3点上昇する。")),
    (34, TableItem::Text("Travel Bag\n幕間に自分の【理解度】チェック1つを外すことができる。")),
    (35, TableItem::Text("JewelC\n開幕演目と幕間にアイテムを1つ選んで獲得する。")),
    (36, TableItem::Text("Sweet Girl\nパフォーマンスを行ったPCは、【メンタル】を2点上昇する。")),
    (44, TableItem::Text("Satisfaction West\nミラクル、ミラクルシンクロ、パーフェクトミラクルが発生したとき【パフォーマンス値】を5点上昇する。")),
    (45, TableItem::Text("Under Big Ben\n使用能力が【ボイス】のパフォーマンスの【パフォーマンス値】が10以上の場合、自分に対する【理解度】チェック1つを外すことができる。")),
    (46, TableItem::Text("PIERO\n一芸突破の達成値が2点上昇する。")),
    (55, TableItem::Text("甘々娘々\n使用能力が【ビジュアル】のパフォーマンスを行うとき、【パフォーマンス値】が3点上昇する。")),
    (56, TableItem::Text("花鳥風月\nシンフォニーを行うとき、振るサイコロの数を1つ増やす、もしくは1つ減らすことができる。")),
    (66, TableItem::Text("Jingle Bells\nリザルトフェイズに以下の効果が発生する。リザルトフェイズに、【獲得ファン人数】が1D6点上昇する。また、PC全員は、条件を満たしていなくても「お願い」をすることができる。")),
];

/// Ruby `CostumeTable` (`DT`)。
static JA_DT: D66Table = D66Table::new("衣装(チャレンジガールズ)", D66SortType::Asc, JA_DT_ITEMS);

static JA_DT_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("１２＆８８")),
    (12, TableItem::Text("Glow Up Princess")),
    (13, TableItem::Text("しずく")),
    (14, TableItem::Text("Pop☆Sweet")),
    (15, TableItem::Text("Ttype")),
    (16, TableItem::Text("Vampire Story")),
    (22, TableItem::Text("Pure Mermaid")),
    (23, TableItem::Text("I'm cute")),
    (24, TableItem::Text("No.1 Girl")),
    (25, TableItem::Text("Final Romance")),
    (26, TableItem::Text("Prism Line")),
    (33, TableItem::Text("さーばんとさーびす")),
    (34, TableItem::Text("Travel Bag")),
    (35, TableItem::Text("JewelC")),
    (36, TableItem::Text("Sweet Girl")),
    (44, TableItem::Text("Satisfaction West")),
    (45, TableItem::Text("Under Big Ben")),
    (46, TableItem::Text("PIERO")),
    (55, TableItem::Text("甘々娘々")),
    (56, TableItem::Text("花鳥風月")),
    (66, TableItem::Text("Jingle Bells")),
];

/// Ruby `CostumeTable#brand_only` (`DT`)。
static JA_DT_BRAND: D66Table = D66Table::new(
    "衣装(チャレンジガールズ)",
    D66SortType::Asc,
    JA_DT_BRAND_ITEMS,
);

static JA_RC_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("Angel kiss\nパフォーマンスを行うとき、1の目が出たサイコロは取り除かれない。シンフォニーを行ったとき、1の目が出たサイコロをすべて取り除く。")),
    (12, TableItem::Text("Pirate ship\n指定特技が属性分野の演目を行うとき、その指定特技を《セクシー／属性4》に変更することができる。")),
    (13, TableItem::Text("ロードトゥプリンス\nミラクル、ミラクルシンクロ、パーフェクトミラクルが発生したとき、そのキャラクターは【メンタル】が10点上昇する。")),
    (14, TableItem::Text("Princess Guardian\n自分以外のキャラクターの【メンタル】が0点になったときに、《気配り／才能9》で判定を行うことができる。この判定には、個性特技は使用できない。成功すると、そのキャラクターは、【獲得ファン人数】が半分にならない。")),
    (15, TableItem::Text("Starlight TourS\nライブフェイズの間、演目を1つ選んで、指定特技を《スター／属性12》に変更できる。")),
    (16, TableItem::Text("花鳥風月・裏\nライブフェイズ中、一度だけ場に残っているすべてのサイコロの目を反転（1なら6に、2なら5に、3なら4に）することができる。")),
    (22, TableItem::Text("しくらま\n判定に使用したサイコロの目が7の場合、【メンタル】が7点上昇する。")),
    (23, TableItem::Text("Chime\nミラクル、ミラクルシンクロ、パーフェクトミラクルが発生したとき、そのキャラクターはランダムにアイテムを1つ獲得する。")),
    (24, TableItem::Text("砂上の光\nシンフォニーを行ったとき、シンフォニーを受けたキャラクターの【メンタル】が5点上昇する。")),
    (25, TableItem::Text("Air by me\n幕間の開始時に、【メンタル】が5点上昇する。")),
    (26, TableItem::Text("戦国ストリート\n演目の使用能力が【フィジカル】のとき、【パフォーマンス値】が2点上昇する。また、指定特技が《ダンス／趣味9》の場合、【パフォーマンス値】が2点上昇する。")),
    (33, TableItem::Text("Wild man\n一芸突破の達成値が2点上昇する。ただし、スペシャルは発生しなくなる。")),
    (34, TableItem::Text("Gray Stand\n【獲得ファン人数】が減少したとき、減少した値の半分（端数切り捨て）と同じ値だけ、【獲得ファン人数】が上昇する。")),
    (35, TableItem::Text("トイ ARM\n演目の開始時に、2D6を振る。その結果が11以上の場合、この演目では【メンタル】が減少しない。")),
    (36, TableItem::Text("white plan\nファンブルが発生しても変調を受けない。")),
    (44, TableItem::Text("SINOBI\n開幕演目を行うとき、出ないことを選択することができる。")),
    (45, TableItem::Text("V-X\nミラクルが発生したときの【パフォーマンス値】を15にできる。")),
    (46, TableItem::Text("ドラゴンナックル\n幕間より後、PCが行うパフォーマンスの【パフォーマンス値】が4点上昇する。")),
    (55, TableItem::Text("Halloween Magic\n後半PPによって【メンタル】が減少するとき、その値を4点減少する（最低0）。")),
    (56, TableItem::Text("Satisfaction East\n【獲得ファン人数】が減少したとき、【メンタル】を20点にすることができる。")),
    (66, TableItem::Text("Devil kiss\nパフォーマンスを行うとき、6の目が出たサイコロは取り除かれない。シンフォニーを行ったとき、6の目が出たサイコロをすべて取り除く。")),
];

/// Ruby `CostumeTable` (`RC`)。
static JA_RC: D66Table = D66Table::new("衣装(ロードトゥプリンス)", D66SortType::Asc, JA_RC_ITEMS);

static JA_RC_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("Angel kiss")),
    (12, TableItem::Text("Pirate ship")),
    (13, TableItem::Text("ロードトゥプリンス")),
    (14, TableItem::Text("Princess Guardian")),
    (15, TableItem::Text("Starlight TourS")),
    (16, TableItem::Text("花鳥風月・裏")),
    (22, TableItem::Text("しくらま")),
    (23, TableItem::Text("Chime")),
    (24, TableItem::Text("砂上の光")),
    (25, TableItem::Text("Air by me")),
    (26, TableItem::Text("戦国ストリート")),
    (33, TableItem::Text("Wild man")),
    (34, TableItem::Text("Gray Stand")),
    (35, TableItem::Text("トイ ARM")),
    (36, TableItem::Text("white plan")),
    (44, TableItem::Text("SINOBI")),
    (45, TableItem::Text("V-X")),
    (46, TableItem::Text("ドラゴンナックル")),
    (55, TableItem::Text("Halloween Magic")),
    (56, TableItem::Text("Satisfaction East")),
    (66, TableItem::Text("Devil kiss")),
];

/// Ruby `CostumeTable#brand_only` (`RC`)。
static JA_RC_BRAND: D66Table = D66Table::new(
    "衣装(ロードトゥプリンス)",
    D66SortType::Asc,
    JA_RC_BRAND_ITEMS,
);

static JA_FC_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("常峰製作所\n第一演目では、【メンタル】が減少しない。")),
    (12, TableItem::Text("フォーチュンスター\n最終演目の【パフォーマンス値】が「【メンタル】÷2（端数切り捨て）」点上昇する。")),
    (13, TableItem::Text("ファイタースケイル\n【メンタル】が5点以下の場合、【パフォーマンス値】が1D6点上昇する。")),
    (14, TableItem::Text("Blood Scissors\n自分以外のキャラクター一人の【メンタル】を5点減少するか、プロデューサーに変調「怪我」を与えることで、自分の【メンタル】が5点上昇する。この効果は、プロデューサーが既に「怪我」の変調を受けていると、使用できない。")),
    (15, TableItem::Text("蒸気式演技服\n判定を行うとき、【メンタル】を1点消費することで、判定の達成値が1点上昇する。")),
    (16, TableItem::Text("ウェイトスター\n「スタミナドリンク」によって、他のキャラクターの【メンタル】を上昇する場合、さらに4点上昇する。")),
    (22, TableItem::Text("Little Stage\n判定のサイコロやパフォーマンスで「1」の出目が1つ以上出た場合、【思い出】を1つ獲得する。")),
    (23, TableItem::Text("Check It\n開幕演目前に、最終演目以外の好きな演目を指定する。指定された演目では、自分の【メンタル】が減少しない。")),
    (24, TableItem::Text("12 Sword\nアイドル戦闘ルールを使用しているとき、与えるダメージが3点上昇し、上昇する【獲得ファン人数】が5点上昇する。")),
    (25, TableItem::Text("Magi Magic\nパフォーマンスや自分が行うシンフォニーでサイコロを取り除くたびに、【メンタル】が2点上昇する。")),
    (26, TableItem::Text("Jokers\n最終演目に行う一芸突破の目標値が3点減少する。")),
    (33, TableItem::Text("Papillon Club\n自分以外のキャラクターがタイプが「補助」のアイドルスキルを使用するたびに、【メンタル】が3点上昇する。")),
    (34, TableItem::Text("ネイキッドチャレンジ\n開幕演目開始時に、【メンタル】が5点減少する。このライブフェイズの間、好きな能力値が3点上昇する。")),
    (35, TableItem::Text("Cold Vivit\n好きなギャップを1つ埋める。このギャップは、ライブフェイズ終了時に元に戻る。")),
    (36, TableItem::Text("対魔絶伏\n特別な演目では、【メンタル】が減少しない。")),
    (44, TableItem::Text("Rescue Power\n演目の判定でファンブルが発生した場合、好きな能力値でパフォーマンスを行うことができる。")),
    (45, TableItem::Text("アニマルエンジン\n幕間の終了時に、好きな動物からの【理解度】が2点上昇する。")),
    (46, TableItem::Text("ふわふわキッチン\n好きなときに、「お菓子」を一つ消費することで、好きなキャラクターの【メンタル】が1D6点上昇できる。また、幕間に「お菓子」を1つ獲得する。")),
    (55, TableItem::Text("Night Talk\n幕間で「信用」を行ったとき、上昇する【メンタル】が10点になる。")),
    (56, TableItem::Text("ティーチングタイム\n自分以外のキャラクターを1人指定する。このライブフェイズの間、指定されたPCの能力値が1点上昇する。")),
    (66, TableItem::Text("See Diver\n演目名に「海」「水」「泡」「湖」「風呂」を含む演目を行った場合、【獲得ファン人数】が1D6点上昇する。")),
];

/// Ruby `CostumeTable` (`FC`)。
static JA_FC: D66Table = D66Table::new("衣装(フォーチュンスターズ)", D66SortType::Asc, JA_FC_ITEMS);

static JA_FC_BRAND_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("常峰製作所")),
    (12, TableItem::Text("フォーチュンスター")),
    (13, TableItem::Text("ファイタースケイル")),
    (14, TableItem::Text("Blood Scissors")),
    (15, TableItem::Text("蒸気式演技服")),
    (16, TableItem::Text("ウェイトスター")),
    (22, TableItem::Text("Little Stage")),
    (23, TableItem::Text("Check It")),
    (24, TableItem::Text("12 Sword")),
    (25, TableItem::Text("Magi Magic")),
    (26, TableItem::Text("Jokers")),
    (33, TableItem::Text("Papillon Club")),
    (34, TableItem::Text("ネイキッドチャレンジ")),
    (35, TableItem::Text("Cold Vivit")),
    (36, TableItem::Text("対魔絶伏")),
    (44, TableItem::Text("Rescue Power")),
    (45, TableItem::Text("アニマルエンジン")),
    (46, TableItem::Text("ふわふわキッチン")),
    (55, TableItem::Text("Night Talk")),
    (56, TableItem::Text("ティーチングタイム")),
    (66, TableItem::Text("See Diver")),
];

/// Ruby `CostumeTable#brand_only` (`FC`)。
static JA_FC_BRAND: D66Table = D66Table::new(
    "衣装(フォーチュンスターズ)",
    D66SortType::Asc,
    JA_FC_BRAND_ITEMS,
);

/// Ruby `bland`（アクセサリーブランド決定表 `ACB`）。
static JA_ACB: BiChainTable = BiChainTable::new(
    "アクセサリーブランド決定表",
    1,
    6,
    &[
        &[
            Node::Text("『チャレンジガールズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_DT_BRAND),
        ],
        &[
            Node::Text("『チャレンジガールズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_DT_BRAND),
        ],
        &[
            Node::Text("『ロードトゥプリンス』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_RC_BRAND),
        ],
        &[
            Node::Text("『ロードトゥプリンス』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_RC_BRAND),
        ],
        &[
            Node::Text("『フォーチュンスターズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_FC_BRAND),
        ],
        &[
            Node::Text("『フォーチュンスターズ』の衣装ブランドからランダムに決定する。"),
            Node::Table(&JA_FC_BRAND),
        ],
    ],
);

static JA_RARE_SKILL_TABLE_ITEMS: &[&str] = &[
    "【秘めたる素質】を修得する。",
    "【王者の風格】を修得する。",
    "【万能アイドル】を修得する。",
    "【最強の負けず嫌い】を修得する。",
    "【超絶無敵コーデ】を修得する。",
    "【強く正しく美しく】を修得する。",
];

/// Ruby `rare_skill_table`（`GG` の 23/24/25 から連鎖）。
static JA_RARE_SKILL_TABLE: Table =
    Table::from_dice("レアアイドルスキル修得表", 1, 6, JA_RARE_SKILL_TABLE_ITEMS);

/// Ruby `tn`（夜語りシチュエーション表 `TN`。4番目に特技表が連鎖する）。
static JA_TN: BiChainTable = BiChainTable::new(
    "夜語りシチュエーション表",
    1,
    6,
    &[
        &[Node::Text("みんなが寝静まった寝室。二人だけのお話。"), Node::Text("特技 : 好きな特技")],
        &[Node::Text("夜の街を歩きながら、【背景】をぽつぽつと語り出す。"), Node::Text("特技 : シーンプレイヤーの個性特技")],
        &[Node::Text("「好きなもの」を探しに出かけた帰り道。"), Node::Text("特技 : シーンプレイヤーが修得している趣味分野の特技")],
        &[Node::Text("「嫌いなもの」から逃げてきて、二人きりになってしまった。"), Node::Table(&JA_SKILL_TABLE)],
        &[Node::Text("暗い道を往くとき、ふとしたきっかけで、「身体的特徴」に触れてしまう。"), Node::Text("特技 : シーンプレイヤーが修得している身長分野の特技")],
        &[Node::Text("「ファッション特徴」の話に夢中になっていたら、いつの間にか二人になっていたことに気づく。"), Node::Text("特技 : シーンプレイヤーが修得している属性分野の特技")],
    ],
);

/// Ruby `cg`（コモン成長表 `CG`。4・5番目にアイテム表が連鎖する）。
static JA_CG: BiChainTable = BiChainTable::new(
    "コモン成長表",
    1,
    6,
    &[
        &[Node::Text("【メンタル】が2点上昇する。")],
        &[Node::Text("【メンタル】が4点上昇する。")],
        &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")],
        &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&JA_ITEM_TABLE)],
        &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&JA_ITEM_TABLE)],
        &[Node::Text("【獲得ファン人数】が3点上昇する。")],
    ],
);

/// Ruby `gg`（ゴールド成長表 `GG`。23/24/25 はレアアイドルスキル、56 はアイテムが連鎖する）。
static JA_GG: BiChainD66Table = BiChainD66Table::new(
    "ゴールド成長表",
    &[
        (11, &[Node::Text("好きなアイドルスキルを一つ選んで修得する。")]),
        (12, &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (13, &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (14, &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (15, &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (16, &[Node::Text("『チャレンジガールズ』か『ロードトゥプリンス』のアイドルスキル修得表を使ってアイドルスキルを一つ修得する。")]),
        (22, &[Node::Text("好きなアイドルスキルを一つ選んで修得する。")]),
        (23, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&JA_RARE_SKILL_TABLE)]),
        (24, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&JA_RARE_SKILL_TABLE)]),
        (25, &[Node::Text("レアアイドルスキル修得表を使ってアイドルスキルを一つ修得する。"), Node::Table(&JA_RARE_SKILL_TABLE)]),
        (26, &[Node::Text("好きな能力値一つが1点上昇する。")]),
        (33, &[Node::Text("好きな能力値一つが2点上昇する。")]),
        (34, &[Node::Text("【ボイス】が1点上昇する。")]),
        (35, &[Node::Text("【フィジカル】が1点上昇する。")]),
        (36, &[Node::Text("【ビジュアル】が1点上昇する。")]),
        (44, &[Node::Text("個性特技を別の特技に変更することができる。")]),
        (45, &[Node::Text("好きな能力値二つが1点上昇する。")]),
        (46, &[Node::Text("すべての能力値が1点上昇する。")]),
        (55, &[Node::Text("【メンタル】が10点上昇する。")]),
        (56, &[Node::Text("アイテムをランダムに一つ獲得する。"), Node::Table(&JA_ITEM_TABLE)]),
        (66, &[Node::Text("個性特技の目標値が1点減少する。")]),
    ],
);

/// Ruby `SkillHometown`（`HA` の 22 から出身分野の特技表を引く）。
static JA_SKILL_HOMETOWN: SkillHometown = SkillHometown::new(&JA_SKILL_TABLE);

/// Ruby `ha`（ハプニング表 `HA`。22 に出身分野の特技表が連鎖する）。
static JA_HA: BiChainD66Table = BiChainD66Table::new(
    "ハプニング表",
    &[
        (11, &[Node::Text("ハプニングなし")]),
        (12, &[Node::Text("ハプニングなし")]),
        (13, &[Node::Text("ハプニングなし")]),
        (14, &[Node::Text("ハプニングなし")]),
        (15, &[Node::Text("ハプニングなし")]),
        (16, &[Node::Text("ハプニングなし")]),
        (22, &[Node::Text("パートナープレイヤーに、地方からオファーが来た。その土地独特の文化を学んで、パートナープレイヤーに伝えよう。"), Node::Table(&JA_SKILL_HOMETOWN)]),
        (23, &[Node::Text("グラビア撮影だが、用意された衣装のサイズがパートナープレイヤーに合わなかった。何とかして、衣装を合わせなければいけない。"), Node::Text("特技 : パートナープレイヤーが修得している身長分野の特技")]),
        (24, &[Node::Text("ダンス撮影中。パートナープレイヤーのダンスに迷いが見えた。何かアドバイスをして、迷いを取り払いたい。"), Node::Text("特技 : 《ダンス／趣味9》")]),
        (25, &[Node::Text("歌の仕事だが、パートナープレイヤーの歌がどこかぎこちない。うまく本来の歌を取り戻させよう。"), Node::Text("特技 : パートナープレイヤーが修得している属性分野の特技")]),
        (26, &[Node::Text("体力を消費する仕事の最中に、パートナープレイヤーが倒れてしまった！　急いで処置をしなければ！"), Node::Text("特技 : 《気配り／才能9》")]),
        (33, &[Node::Text("パートナープレイヤーにマイナースポーツのCMが回ってきたが、知らない様子だ。ルールを教えよう。"), Node::Text("特技 : 《スポーツ／趣味4》")]),
        (34, &[Node::Text("パートナープレイヤーのキャラに合わない仕事が舞い込んだ。演技力で乗り切ってほしい。"), Node::Text("特技 : 《演技力／才能12》")]),
        (35, &[Node::Text("パートナープレイヤーが風邪をひいてしまう。次の仕事までに、なんとか治してもらわなければ。"), Node::Text("特技 : 《元気／キャラ8》")]),
        (36, &[Node::Text("パートナープレイヤーの属性らしくない衣装が来てしまった。うまくアレンジできればいいけど。"), Node::Text("特技 : 《おしゃれ／趣味5》")]),
        (44, &[Node::Text("パートナープレイヤーのテンションが低い。テンションを上げるようなことを言おう。"), Node::Text("特技 : 《バーニング／属性10》")]),
        (45, &[Node::Text("パートナープレイヤーの仕事に必要な小道具が足りなくなった。調達しよう。"), Node::Text("特技 : 《ショッピング／趣味8》")]),
        (46, &[Node::Text("パートナープレイヤーに外国から仕事が舞い込んできた。外国の文化に合わせた仕事をしなければ。"), Node::Text("特技 : 《異国文化／才能2》")]),
        (55, &[Node::Text("パートナープレイヤーに大会社からの仕事のオファーがやって来る。プレッシャーに負けないように後押ししよう。"), Node::Text("特技 : 《胆力／才能5》")]),
        (56, &[Node::Text("パートナープレイヤーと他のアイドルグループとのコラボイベントが行われる。そのアイドルの情報を集めてこよう。"), Node::Text("特技 : 《アイドル／趣味12》")]),
        (66, &[Node::Text("パートナープレイヤーの周りで、幽霊騒ぎが起こる。安心させるためにも、調査に乗り出そう。"), Node::Text("特技 : 《オカルト／趣味2》")]),
    ],
);

static JA_ACT_HEAD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アイマスク")),
    (12, TableItem::Text("うさみみ")),
    (13, TableItem::Text("ねこみみ")),
    (14, TableItem::Text("サングラス")),
    (15, TableItem::Text("ピアス")),
    (16, TableItem::Text("シュシュ")),
    (22, TableItem::Text("仮面")),
    (23, TableItem::Text("ティアラ")),
    (24, TableItem::Text("エクステ")),
    (25, TableItem::Text("バンダナ")),
    (26, TableItem::Text("ヘアバンド")),
    (33, TableItem::Text("インカム")),
    (34, TableItem::Text("イヤリング")),
    (35, TableItem::Text("ホワイトプリム")),
    (36, TableItem::Text("ゴーグル")),
    (44, TableItem::Text("つけひげ")),
    (45, TableItem::Text("ヘッドホン")),
    (46, TableItem::Text("耳あて")),
    (55, TableItem::Text("トナカイの角")),
    (56, TableItem::Text("花飾り")),
    (66, TableItem::Text("かんざし")),
];

static JA_ACT_HEAD: D66Table =
    D66Table::new("頭アクセサリー表", D66SortType::Asc, JA_ACT_HEAD_ITEMS);

static JA_ACT_HAT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヘルメット")),
    (12, TableItem::Text("麦わら帽子")),
    (13, TableItem::Text("サンタ帽子")),
    (14, TableItem::Text("花冠")),
    (15, TableItem::Text("学帽")),
    (16, TableItem::Text("ハンチング帽")),
    (22, TableItem::Text("シルクハット")),
    (23, TableItem::Text("テンガロンハット")),
    (24, TableItem::Text("ナイトキャップ")),
    (25, TableItem::Text("ロシア帽")),
    (26, TableItem::Text("ベレー帽")),
    (33, TableItem::Text("コック帽")),
    (34, TableItem::Text("パーティコーン")),
    (35, TableItem::Text("とんがり帽子")),
    (36, TableItem::Text("キャップ")),
    (44, TableItem::Text("ナースキャップ")),
    (45, TableItem::Text("カンカン帽")),
    (46, TableItem::Text("ハット帽")),
    (55, TableItem::Text("ターバン")),
    (56, TableItem::Text("セーラーキャップ")),
    (66, TableItem::Text("中共帽子")),
];

static JA_ACT_HAT: D66Table =
    D66Table::new("帽子アクセサリー表", D66SortType::Asc, JA_ACT_HAT_ITEMS);

static JA_ACT_BODY_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("首かけ指輪")),
    (12, TableItem::Text("カウベル")),
    (13, TableItem::Text("ネックレス")),
    (14, TableItem::Text("蝶ネクタイ")),
    (15, TableItem::Text("メガホン")),
    (16, TableItem::Text("ペンダント")),
    (22, TableItem::Text("ブローチ")),
    (23, TableItem::Text("金の首輪")),
    (24, TableItem::Text("チョーカー")),
    (25, TableItem::Text("南京錠")),
    (26, TableItem::Text("タスキ")),
    (33, TableItem::Text("エプロン")),
    (34, TableItem::Text("名札")),
    (35, TableItem::Text("階級章")),
    (36, TableItem::Text("胸当て")),
    (44, TableItem::Text("ベルト")),
    (45, TableItem::Text("ポシェット")),
    (46, TableItem::Text("マフラー")),
    (55, TableItem::Text("首かけカメラ")),
    (56, TableItem::Text("リボン")),
    (66, TableItem::Text("肩パット")),
];

static JA_ACT_BODY: D66Table =
    D66Table::new("胴アクセサリー表", D66SortType::Asc, JA_ACT_BODY_ITEMS);

static JA_ACT_ARM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("動物の手")),
    (12, TableItem::Text("宝石の腕輪")),
    (13, TableItem::Text("動物のマペット")),
    (14, TableItem::Text("グローブ")),
    (15, TableItem::Text("指ぬきグローブ")),
    (16, TableItem::Text("リストバンド")),
    (22, TableItem::Text("鍋掴み")),
    (23, TableItem::Text("手袋")),
    (24, TableItem::Text("長手袋")),
    (25, TableItem::Text("腕章")),
    (26, TableItem::Text("腕時計")),
    (33, TableItem::Text("ドリル")),
    (34, TableItem::Text("楽器")),
    (35, TableItem::Text("マフ")),
    (36, TableItem::Text("カフス")),
    (44, TableItem::Text("ボクシンググローブ")),
    (45, TableItem::Text("シルバーアクセサリー")),
    (46, TableItem::Text("ゴールドアクセサリー")),
    (55, TableItem::Text("ぬいぐるみ")),
    (56, TableItem::Text("ミサンガ")),
    (66, TableItem::Text("手甲")),
];

static JA_ACT_ARM: D66Table = D66Table::new("腕アクセサリー表", D66SortType::Asc, JA_ACT_ARM_ITEMS);

static JA_ACT_FOOT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("革靴")),
    (12, TableItem::Text("ブーツ")),
    (13, TableItem::Text("スポーツシューズ")),
    (14, TableItem::Text("スキー靴")),
    (15, TableItem::Text("アンクル")),
    (16, TableItem::Text("バスケットシューズ")),
    (22, TableItem::Text("スリッパ")),
    (23, TableItem::Text("ミサンガ")),
    (24, TableItem::Text("動物の足")),
    (25, TableItem::Text("作業靴")),
    (26, TableItem::Text("ルーズウォーマー")),
    (33, TableItem::Text("ニーパッド")),
    (34, TableItem::Text("ガーターリング")),
    (35, TableItem::Text("ポーチ")),
    (36, TableItem::Text("ローラースケート")),
    (44, TableItem::Text("へんなタイツ")),
    (45, TableItem::Text("白タイツ")),
    (46, TableItem::Text("網タイツ")),
    (55, TableItem::Text("ガラスの靴")),
    (56, TableItem::Text("グリープ")),
    (66, TableItem::Text("ベル")),
];

static JA_ACT_FOOT: D66Table =
    D66Table::new("足アクセサリー表", D66SortType::Asc, JA_ACT_FOOT_ITEMS);

static JA_ACT_OTHER_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ボンボン")),
    (12, TableItem::Text("マント")),
    (13, TableItem::Text("蝶の羽")),
    (14, TableItem::Text("天使の羽")),
    (15, TableItem::Text("悪魔の羽")),
    (16, TableItem::Text("猫のしっぽ")),
    (22, TableItem::Text("トレンチコート")),
    (23, TableItem::Text("ばんそうこう")),
    (24, TableItem::Text("パラソル")),
    (25, TableItem::Text("ステッキ")),
    (26, TableItem::Text("タトゥーシール")),
    (33, TableItem::Text("バーコード")),
    (34, TableItem::Text("バレーボール")),
    (35, TableItem::Text("大きなリボン")),
    (36, TableItem::Text("鎖")),
    (44, TableItem::Text("キャラクターグッズ")),
    (45, TableItem::Text("イメージカラーのネイル")),
    (46, TableItem::Text("メガネ")),
    (55, TableItem::Text("旗")),
    (56, TableItem::Text("ジャケット")),
    (66, TableItem::Text("サスペンダー")),
];

static JA_ACT_OTHER: D66Table =
    D66Table::new("その他アクセサリー表", D66SortType::Asc, JA_ACT_OTHER_ITEMS);

/// Ruby `translate_accessories_table`（`ACT`）。
static JA_ACT: BiChainTable = BiChainTable::new(
    "アクセサリー種別決定表",
    1,
    6,
    &[
        &[
            Node::Text("頭アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_HEAD),
        ],
        &[
            Node::Text("帽子アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_HAT),
        ],
        &[
            Node::Text("胴アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_BODY),
        ],
        &[
            Node::Text("腕アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_ARM),
        ],
        &[
            Node::Text("足アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_FOOT),
        ],
        &[
            Node::Text("その他アクセサリー表を使用する。"),
            Node::Table(&JA_ACT_OTHER),
        ],
    ],
);

static JA_MS_ARTICLE_ITEMS: &[&str] = &[
    "アイドル",
    "ガール／ボーイ",
    "パラダイス",
    "プリンセス／プリンス",
    "スタイル",
    "クイーン／キング",
];

static JA_MS_ARTICLE: Table = Table::from_dice("称号表", 1, 6, JA_MS_ARTICLE_ITEMS);

static JA_MS_DESCRIBE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ビギニング")),
    (12, TableItem::Text("パワフル")),
    (13, TableItem::Text("ビューティフル")),
    (14, TableItem::Text("エターナル")),
    (15, TableItem::Text("きらめき")),
    (16, TableItem::Text("シャイニング")),
    (22, TableItem::Text("パーフェクト")),
    (23, TableItem::Text("1000%")),
    (24, TableItem::Text("フレッシュ")),
    (25, TableItem::Text("ドキドキ")),
    (26, TableItem::Text("ワイルド")),
    (33, TableItem::Text("ロイヤル")),
    (34, TableItem::Text("ときめき")),
    (35, TableItem::Text("ふわふわ")),
    (36, TableItem::Text("スタイリッシュ")),
    (44, TableItem::Text("小悪魔")),
    (45, TableItem::Text("スーパー")),
    (46, TableItem::Text("ウルトラ")),
    (55, TableItem::Text("ハイパー")),
    (56, TableItem::Text("ダイナマイト")),
    (66, TableItem::Text("アルティメット")),
];

static JA_MS_DESCRIBE: D66Table = D66Table::new("形容表", D66SortType::Asc, JA_MS_DESCRIBE_ITEMS);

static JA_MS_SCENE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("マーメイド")),
    (12, TableItem::Text("ドリーム")),
    (13, TableItem::Text("ピュア")),
    (14, TableItem::Text("アニマル")),
    (15, TableItem::Text("サンシャイン")),
    (16, TableItem::Text("ムーンライト")),
    (22, TableItem::Text("かわいい／かっこいい")),
    (23, TableItem::Text("フューチャリング")),
    (24, TableItem::Text("ライジング")),
    (25, TableItem::Text("バーニング")),
    (26, TableItem::Text("スターライト")),
    (33, TableItem::Text("ボンバー")),
    (34, TableItem::Text("レインボー")),
    (35, TableItem::Text("フローズン")),
    (36, TableItem::Text("ヒート")),
    (44, TableItem::Text("ダーク")),
    (45, TableItem::Text("ぴかぴか")),
    (46, TableItem::Text("サンライズ")),
    (55, TableItem::Text("スターダスト")),
    (56, TableItem::Text("オーロラ")),
    (66, TableItem::Text("ギャラクシー")),
];

static JA_MS_SCENE: D66Table = D66Table::new("情景表", D66SortType::Asc, JA_MS_SCENE_ITEMS);

static JA_MS_MATERIAL_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バスケット")),
    (12, TableItem::Text("エクスプレス")),
    (13, TableItem::Text("エアプレーン")),
    (14, TableItem::Text("ロケット")),
    (15, TableItem::Text("ハリケーン")),
    (16, TableItem::Text("バイク")),
    (22, TableItem::Text("タイガー")),
    (23, TableItem::Text("ドルフィン")),
    (24, TableItem::Text("ドッグ")),
    (25, TableItem::Text("キャット")),
    (26, TableItem::Text("バニー")),
    (33, TableItem::Text("ドラゴン")),
    (34, TableItem::Text("ソード")),
    (35, TableItem::Text("ランス")),
    (36, TableItem::Text("パラソル")),
    (44, TableItem::Text("ローズ")),
    (45, TableItem::Text("ロータス")),
    (46, TableItem::Text("コスモス")),
    (55, TableItem::Text("キャンディ")),
    (56, TableItem::Text("ハート")),
    (66, TableItem::Text("フェニックス")),
];

static JA_MS_MATERIAL: D66Table =
    D66Table::new("マテリアル表", D66SortType::Asc, JA_MS_MATERIAL_ITEMS);

static JA_MS_ACTION_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("スパイラル")),
    (12, TableItem::Text("フライ")),
    (13, TableItem::Text("シャワー")),
    (14, TableItem::Text("ダイブ")),
    (15, TableItem::Text("イリュージョン")),
    (16, TableItem::Text("ラッシュ")),
    (22, TableItem::Text("ターン")),
    (23, TableItem::Text("ラブ")),
    (24, TableItem::Text("ハグ")),
    (25, TableItem::Text("ダッシュ")),
    (26, TableItem::Text("シュート")),
    (33, TableItem::Text("ダイビング")),
    (34, TableItem::Text("クロス")),
    (35, TableItem::Text("トリック")),
    (36, TableItem::Text("ビーム")),
    (44, TableItem::Text("スラッシュ")),
    (45, TableItem::Text("ボイス")),
    (46, TableItem::Text("ドライブ")),
    (55, TableItem::Text("くるくる")),
    (56, TableItem::Text("ジャンプ")),
    (66, TableItem::Text("アクション")),
];

static JA_MS_ACTION: D66Table = D66Table::new("アクション表", D66SortType::Asc, JA_MS_ACTION_ITEMS);

static JA_MS_FORMATS: &[&str] = &[
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%s＋%s＋%s",
    "%sもしくは%s＋%s＋PCの名前",
    "%sもしくは%s＋%s＋PCの名前",
];

/// Ruby `MySkillNameTable.new(:ja_jp)`（`MS`）。
static JA_MS: MySkillNameTable = MySkillNameTable::new(
    "マイスキル名決定表",
    JA_MS_FORMATS,
    &[
        &[
            SubTable::D66(&JA_MS_DESCRIBE),
            SubTable::D66(&JA_MS_SCENE),
            SubTable::D66(&JA_MS_MATERIAL),
        ],
        &[
            SubTable::D66(&JA_MS_DESCRIBE),
            SubTable::D66(&JA_MS_SCENE),
            SubTable::D66(&JA_MS_ACTION),
        ],
        &[
            SubTable::D66(&JA_MS_DESCRIBE),
            SubTable::D66(&JA_MS_MATERIAL),
            SubTable::D66(&JA_MS_ACTION),
        ],
        &[
            SubTable::D66(&JA_MS_SCENE),
            SubTable::D66(&JA_MS_MATERIAL),
            SubTable::D66(&JA_MS_ACTION),
        ],
        &[
            SubTable::D66(&JA_MS_DESCRIBE),
            SubTable::D66(&JA_MS_SCENE),
            SubTable::Plain(&JA_MS_ARTICLE),
        ],
        &[
            SubTable::D66(&JA_MS_MATERIAL),
            SubTable::D66(&JA_MS_ACTION),
            SubTable::Plain(&JA_MS_ARTICLE),
        ],
    ],
);

static JA_RE_ON_EVENT_ITEMS: &[(i64, &str, i64)] = &[
    (11, "雨女は誰？", 96),
    (12, "千客万来☆アイドル喫茶", 97),
    (13, "フチドル", 98),
    (14, "生放送は踊る", 99),
    (15, "貸し切りプールの誘惑", 100),
    (16, "ケーオンストリート！", 101),
    (21, "アイドル×アニメ×ドリーマー！", 102),
    (22, "一日警察署長、緊急出動!?", 103),
    (23, "アイドルフィン！", 104),
    (24, "「カラオケ採点ガチバトル☆」", 105),
    (25, "「大正乙女ろまんてぃっく」", 106),
    (26, "鳩時計ラジオ", 107),
    (31, "「ガチ学院」ＣＭ", 108),
    (32, "「カラフルアイスクリーム」モデル", 109),
    (33, "忙しすぎる毎日", 110),
    (34, "悩める新人デザイナー", 112),
    (35, "「スクール☆ライフ」", 113),
    (36, "魔法のように", 114),
    (41, "食レポとその後", 115),
    (42, "ソロライブ！", 116),
    (43, "お昼の放送", 117),
    (44, "文化祭！", 118),
    (45, "商店街を救え！", 120),
    (46, "二つの仕事", 121),
    (51, "温泉にて", 122),
    (52, "アイドル探偵と豪華客船", 124),
    (53, "のうぎょう", 125),
    (54, "コント撮影", 127),
    (55, "アイドルＶＳサメ", 128),
    (56, "駅前で歌う", 130),
    (61, "街の清掃ボランティア", 131),
    (62, "ミニユニット活動", 132),
    (63, "カブトムシ狩り", 134),
    (64, "ポスター作り", 135),
    (65, "メロディ", 136),
    (66, "さいてい新聞部の取材", 138),
];

static JA_RE_OFF_EVENT_ITEMS: &[(i64, &str, i64)] = &[
    (11, "アイドル、未知との遭遇", 139),
    (12, "神様おねがい！", 140),
    (13, "プチ合宿の罠!?", 141),
    (14, "どこかで会ったような……", 142),
    (15, "アイデンティティがっ！", 143),
    (16, "ホリダシ×オオソウジ", 144),
    (21, "エンドレス!?　握手会", 146),
    (22, "不安な路線変更", 147),
    (23, "全力ねこレース", 148),
    (24, "恐怖の再テスト！", 149),
    (25, "たくさんのファンレター", 150),
    (26, "夕暮れの帰り道。", 152),
    (31, "どきどき♪　調理実習", 153),
    (32, "超アイドル衣装？", 154),
    (33, "おもいでの修学旅行", 156),
    (34, "アルバイト！", 158),
    (35, "ドライブしよう！", 159),
    (36, "ファミレス攻防戦", 160),
    (41, "総合練習", 162),
    (42, "歌声はお腹から", 164),
    (43, "メイクレッスン基本から", 165),
    (44, "怪我", 166),
    (45, "エゴサ", 168),
    (46, "喫茶店でひと息", 169),
    (51, "天体観測ツアー", 170),
    (52, "謎のコーチ", 172),
    (53, "屋上にて", 174),
    (54, "クラスメイトより", 176),
    (55, "最強アイドル伝", 177),
    (56, "イメチェンしよう", 178),
    (61, "郊外ショッピング施設", 179),
    (62, "お見舞い", 180),
    (63, "ライブを観よう！", 181),
    (64, "頂を目指す", 182),
    (65, "重いコンダラ", 183),
    (66, "アイドル改造計画", 184),
];

/// Ruby `RandomEventTable.new(:ja_jp)`（`RE`）。
static JA_RE: RandomEventTable = RandomEventTable::new(
    "ランダムイベント",
    "%{event}（『ビギニングロード』%{page}ページ）",
    "オンイベント表",
    JA_RE_ON_EVENT_ITEMS,
    "オフイベント表",
    JA_RE_OFF_EVENT_ITEMS,
);

static JA_SH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("思いがけず、ブランドもの衣装を買えてしまった。これを、うまく使えないだろうか？\nシーンプレイヤーとパートナープレイヤーは、好きなブランドの衣装チケットを一つ獲得する。")),
    (12, TableItem::Text("雑貨コーナーを回って、自分たちらしいアクセサリーを探す。\nシーンプレイヤーとパートナープレイヤーは、アイテム「キャラアイテム」を一つ獲得する。")),
    (13, TableItem::Text("お歳暮コーナーが盛況だった。今のトレンドはなんだろうか。\nシーンプレイヤーとパートナープレイヤーは、アイテム「差し入れ」を一つ獲得する。")),
    (14, TableItem::Text("お菓子売り場で、気になっていたお菓子のシリーズを買い漁る。\nシーンプレイヤーとパートナープレイヤーは、アイテム「お菓子」を一つ獲得する。")),
    (15, TableItem::Text("スポーツショップに立ち寄って、スポーツドリンクを買い貯める。いつか使うかもしれない。\nシーンプレイヤーとパートナープレイヤーは、アイテム「スナミナドリンク」を一つ獲得する。")),
    (16, TableItem::Text("スポーツウェアの展示会をやっていたので、見て回る。びびっと来たアイテムも見つかった。\nシーンプレイヤーとパートナープレイヤーは、アイテム「トレーニングウェア」を一つ獲得する。")),
    (22, TableItem::Text("特売品を買い漁る。さて、使えるものかな？\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに二つ獲得する。")),
    (23, TableItem::Text("シューズコーナーで、普段使いの靴を調達する。ダンスにちょうどいいのはどれかな？\nシーンプレイヤーとパートナープレイヤーは、アイテム「ドリーミングシューズ」を一つ獲得する。")),
    (24, TableItem::Text("ふらりと買い物にやって来て、目についたものをとりあえず買ってみる。ちょっと疲れてたかも。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (25, TableItem::Text("色々な洋服を見て回って、自分やパートナーに合ったコーディネートを考えてみる。\nシーンプレイヤーとパートナープレイヤーは、【ビジュアル】が1点上昇する。")),
    (26, TableItem::Text("ちょうど買いたかったものが、格安で売られていた。タイミングがよかったみたいだ。\nシーンプレイヤーとパートナープレイヤーは、好きなアイテムを一つ獲得する。")),
    (33, TableItem::Text("ショッピングモールを歩いているうちに、アイテムを落としてしまう。\nシーンプレイヤーとパートナープレイヤーは、アイテムをすべて失う。")),
    (34, TableItem::Text("ショッピングモールを歩いていると、声をかけられた。地元の人たちから、応援のメッセージをもらう。\nシーンプレイヤーとパートナープレイヤーは、【獲得ファン人数】が4点上昇する。")),
    (35, TableItem::Text("ショッピングモールでは何も買わなかったが、交わした会話はお互いを知るきっかけになった。\nシーンプレイヤーとパートナープレイヤーは、お互いに対する【理解度】が3点上昇する。")),
    (36, TableItem::Text("ベンチで一休みしながら、お互いの長所について話し合う。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (44, TableItem::Text("混雑中のフードコートで食事をしようとしたところ、一時間近く待たされる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が2点減少する。")),
    (45, TableItem::Text("フードコートで定番メニューを頼み、勝手知ったる味を楽しむ。やっぱり、知っている味がいい。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が2点上昇する。")),
    (46, TableItem::Text("フードコートで新しいメニューにチャレンジ！\nシーンプレイヤーとパートナープレイヤーは、1D6を振る。出目が奇数の場合、そのPCは【メンタル】が5点減少し、出目が偶数の場合、そのPCは【メンタル】が5点上昇する。")),
    (55, TableItem::Text("CDコーナーを探しているうちに、迷ってしまった。さて、ここはどこだろう？\n変調がランダムに一つ発生する。")),
    (56, TableItem::Text("カフェコーナーで一休み。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が5点上昇する。")),
    (66, TableItem::Text("家具や家電コーナーを回るうちに、自分たちの将来が不安になってきた。\n変調がランダムに二つ発生する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SH", ...)`。
static JA_SH: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new(
        "ショッピングモール散策表",
        D66SortType::Asc,
        JA_SH_ITEMS,
    )),
    &JA_ABNORMALITY,
);

static JA_MO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("山で迷っていたら、謎の仙人に助けられる。そのついでに、体をうまく動かす方法まで教わる。ありがとう！　謎の仙人！\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が10点上昇する。")),
    (12, TableItem::Text("山の幸を頂いて、幸福感に包まれる。うまい！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (13, TableItem::Text("星空の下で、二人の【背景】を語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (14, TableItem::Text("森林浴をして、体調を整える。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がすべて回復する。")),
    (15, TableItem::Text("山登りを繰り返して、足腰を鍛える。\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (16, TableItem::Text("二人で一緒に朝日を見て、訳も分からず感動する。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (22, TableItem::Text("山で迷った。\n変調がランダムに二つ発生する。")),
    (23, TableItem::Text("山奥に住んでいるという、伝説のファッションデザイナーに着こなしの秘密を教えてもらう。\nシーンプレイヤーとパートナープレイヤーは、好きなブランドの衣装チケットを一つ獲得する。")),
    (24, TableItem::Text("山奥に住んでいるという、伝説のレッスントレーナーに教えを乞いに足を延ばす。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (25, TableItem::Text("ふらっと立ち寄った地元の飲食店で郷土料理を食べる。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (26, TableItem::Text("山奥にある神社まで登って、お祈りをする。無事にライブが成功しますように。\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が5点上昇する。")),
    (33, TableItem::Text("虫にたかられて、嫌な思いをする。\n変調がランダムに一つ発生する。")),
    (34, TableItem::Text("仲間たちみんなとバーベキューをして楽しむ。\nシーンプレイヤーとパートナープレイヤーは、PC全員に対する【理解度】が3点上昇する。")),
    (35, TableItem::Text("キノコ狩りをする。\nシーンプレイヤーとパートナープレイヤーは、1D6を振る。その値が偶数だった場合、アイテム「スタミナドリンク」を一つ獲得する。奇数だった場合、【メンタル】が6点減少する。")),
    (36, TableItem::Text("避暑地の喫茶店で一杯飲みながら、お互いのアイドル論について語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (44, TableItem::Text("山を移動中に、落し物をしてしまう。\nシーンプレイヤーとパートナープレイヤーは、アイテムを一つ失う。")),
    (45, TableItem::Text("山小屋で一晩を過ごす。お互いの生活習慣が見えてきた。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (46, TableItem::Text("ハイキングをしながら、お互いの嫌いなものについて理解を深める。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (55, TableItem::Text("野生の鹿に襲われそうになったので、プロデューサーが盾になった。\n変調「怪我」が発生する。")),
    (56, TableItem::Text("高原の広々としたテニスコートでテニスを楽しむ。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (66, TableItem::Text("山道を歩き疲れて、足が棒になる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.MO", ...)`。
static JA_MO: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new("山散策表", D66SortType::Asc, JA_MO_ITEMS)),
    &JA_ABNORMALITY,
);

static JA_SEA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("浜辺で行われていたミスコンに強制的に参加させられる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が1D6点上昇する。")),
    (12, TableItem::Text("浜辺を散歩しながら、お互いの好きなものについて語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (13, TableItem::Text("とれたての魚を使った寿司を食べて満足する。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (14, TableItem::Text("浜辺を歩いていると、アイドルだと気づいた地元の人たちから声援が飛んでくる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が5点上昇する。")),
    (15, TableItem::Text("童心に返って、海に向かって走り出す。やったー海だー！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (16, TableItem::Text("水泳で体を鍛える。荒れやすい海は特訓にもってこいだ！\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (22, TableItem::Text("せっかくだから釣りをしてみる。何が釣れるかな？\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (23, TableItem::Text("二人で競い合いながら泳ぎまわる。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (24, TableItem::Text("ちょっとだけ日に焼けて、違う自分をセルフプロデュース。ちゃんと事務所の許可はとれてます！\nシーンプレイヤーとパートナープレイヤーの【ビジュアル】が1点上昇する。")),
    (25, TableItem::Text("砂浜にいい感じのタイヤがあったので、それを引っ張りながら走り込みをする。\nシーンプレイヤーとパートナープレイヤーの【フィジカル】が1点上昇する。")),
    (26, TableItem::Text("海に向かって叫んで、すっきりする。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇し、【ボイス】が1点上昇する。")),
    (33, TableItem::Text("しつこいナンパに誘われて、ちょっと意気消沈してしまう。\n変調がランダムに二つ発生する。")),
    (34, TableItem::Text("みんなで花火をして楽しむ。\nシーンプレイヤーとパートナープレイヤーは、PC全員に対する【理解度】が2点上昇する。")),
    (35, TableItem::Text("水着で決めるファンションをコーディネートして、浜辺の視線を一人占め。\nシーンプレイヤーとパートナープレイヤーは、【ビジュアル】が1点上昇する。")),
    (36, TableItem::Text("魚料理を満足いくまで食べたはいいものの、食べ過ぎじゃないかが心配。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇する。")),
    (44, TableItem::Text("穏やかな海を見ながら、これまでのことを思い返す。\n変調が一つ回復する。")),
    (45, TableItem::Text("水族館に立ち寄ったら、お土産に色々持たされる。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (46, TableItem::Text("水族館のイルカショーから、新しい技のヒントをもらう。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (55, TableItem::Text("海で遊んでいたら、溺れかける。\n変調がランダムに三つ発生する。")),
    (56, TableItem::Text("サーファーたちから、人を惹きつける技術について聞き出す。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (66, TableItem::Text("夜の海に出没するという幽霊らしきものを見かけてしまい、ぞっとする。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が5点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SEA", ...)`。
static JA_SEA: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new("海散策表", D66SortType::Asc, JA_SEA_ITEMS)),
    &JA_ABNORMALITY,
);

static JA_SPA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("地元のお祭りに遭遇！　一緒になって盛り上げる。\nシーンプレイヤーとパートナープレイヤーの【獲得ファン人数】が5点上昇する。")),
    (12, TableItem::Text("卓球台を使って、お互いの力を出し切る。何かが掴めたような気がする。\nシーンプレイヤーとパートナープレイヤーの【合宿ポイント】が5点上昇する。")),
    (13, TableItem::Text("お土産屋さんで、色々なものを買い込む。しかし、これは役に立つのだろうか。\nシーンプレイヤーとパートナープレイヤーは、アイテムをランダムに一つ獲得する。")),
    (14, TableItem::Text("温泉からあがった後に、ゆっくりと涼む。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がすべて回復する。")),
    (15, TableItem::Text("温泉街の名物料理を出してもらう。うまい！\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (16, TableItem::Text("浴衣で活気のある街並みを歩きながら、お互いの【背景】を語り合う。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が1D6点上昇する。")),
    (22, TableItem::Text("湯に浸かり過ぎて目が回る。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇し、変調がランダムに一つ発生する。")),
    (23, TableItem::Text("たくさんの温泉に次々浸かる。楽しいけど疲れた。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (24, TableItem::Text("湯船に浸かってリフレッシュ。ひとまずは一息入れましょう。\n変調がすべて回復し、シーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (25, TableItem::Text("ジャグジー風呂で肩こりや足のむくみを癒す。温泉地でもこういう施設はあるんだな。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が5点上昇する。")),
    (26, TableItem::Text("みんなやプロデューサーを連れて足湯に浸かる。いつもご苦労様。\n変調がすべて回復する。")),
    (33, TableItem::Text("のぼせる。\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
    (34, TableItem::Text("貸切風呂で、ゆったりとしながらお互いを理解をするための話し合いをする。\nシーンプレイヤーとパートナープレイヤーのお互いに対する【理解度】が3点上昇する。")),
    (35, TableItem::Text("マッサージ機を使って、日ごろの疲れをたたき出す。\n変調をすべて回復する。")),
    (36, TableItem::Text("お風呂の後の牛乳もうまい！\nシーンプレイヤーとパートナープレイヤーは、アイテム「スナミナドリンク」を一つ獲得する。")),
    (44, TableItem::Text("ついつい無駄遣いをしてしまう。てへへ。\n変調がランダムに一つ発生する。")),
    (45, TableItem::Text("屋台での観光客向けの料理に舌鼓をうつ。こういう味もありなのか。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が3点上昇する。")),
    (46, TableItem::Text("温泉街の女将さんたちに、人の心を豊かにする術を教えてもらう。\nシーンプレイヤーとパートナープレイヤーは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。")),
    (55, TableItem::Text("部屋の中でぼけっと過ごす。\nシーンプレイヤーとパートナープレイヤーの【メンタル】が1点上昇する。")),
    (56, TableItem::Text("観光客の人々と会話をして、自分たちの知名度を確認する。\nアイドルランク係数が「10」以上の場合、【獲得ファン人数】が4D6点上昇する。")),
    (66, TableItem::Text("複雑な地形と坂道で疲れ始める。目的の場所はまだ？\nシーンプレイヤーとパートナープレイヤーは、【メンタル】が3点減少する。")),
];

/// Ruby `D66WithAbnormality.from_i18n("BeginningIdol.tables.SPA", ...)`。
static JA_SPA: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::D66(D66Table::new(
        "温泉街散策表",
        D66SortType::Asc,
        JA_SPA_ITEMS,
    )),
    &JA_ABNORMALITY,
);

static JA_LN_ITEMS: &[&str] = &[
    "誰のことも信じられない。私は一人でやってみせる。\nPC全員の【理解度】すべてにチェックを入れる。センターPCは、アイドルスキル修得表を使ってアイドルスキルを一つ修得する。",
    "事件がきっかけで、事務所の空気が悪くなった。嫌な雰囲気。\n変調がランダムに三つ発生する。",
    "口調が荒くなり、きつい一言を仲間に言ってしまう。\nPC全員の【メンタル】が5点減少し、【理解度】すべてにチェックを入れる。",
    "ちょっとした注意がきっかけで、仲間と喧嘩をしてしまう。\nPC全員の【メンタル】が10点減少する。",
    "誰も、話をしない。気まずさと静寂が場を包んだ。このままでは、会場の空気も悪くなる。\n【目標動員数】が二倍になる。",
    "突然の強い雨に打たれる。ずぶぬれのところに一人。そんなところを誰かに目撃されてしまう。\nセンターPCの【獲得ファン人数】が半分になる。",
];

/// Ruby `TableWithAbnormality.from_i18n("BeginningIdol.tables.LN", ...)`。
static JA_LN: AbnormalityTable = AbnormalityTable::new(
    AbnormalitySource::Plain(Table::from_dice("孤独表", 1, 6, JA_LN_ITEMS)),
    &JA_ABNORMALITY,
);

static JA_SGT_ITEMS: &[&str] = &[
    "シーンプレイヤーが修得している才能分野の特技が指定特技のアイドルスキル",
    "シーンプレイヤーが修得しているキャラ分野の特技が指定特技のアイドルスキル",
    "シーンプレイヤーが修得している趣味分野の特技が指定特技のアイドルスキル",
    "ランダムに決定した特技が指定特技のアイドルスキル(身長分野、属性分野、出身分野が出たら振り直し)",
    "《メンタルアップ》《パフォーマンスアップ》《アイテムアップ》のうちいずれか1つ",
    "《メンタルアップ》《パフォーマンスアップ》《アイテムアップ》のうちいずれか1つ",
];

/// Ruby `SkillGetTable.from_i18n("BeginningIdol.tables.SGT", ...)`。
static JA_SGT: SkillGetTable = SkillGetTable::new(
    Table::from_dice(
        "アイドルスキル修得表(チャレンジガールズ)",
        1,
        6,
        JA_SGT_ITEMS,
    ),
    &JA_SKILL_TABLE,
    "(身長)分野、(属性|才能)分野、(出身)分野が出たら振り直し",
    "振り直し",
    "特技リスト",
    DEFAULT_SKILL_FORMAT,
);

static JA_RS_ITEMS: &[&str] = &[
    "シーンプレイヤーが修得している属性分野の特技が指定特技のアイドルスキル",
    "シーンプレイヤーが修得しているキャラ分野の特技が指定特技のアイドルスキル",
    "シーンプレイヤーが修得している趣味分野の特技が指定特技のアイドルスキル",
    "ランダムに決定した特技が指定特技のアイドルスキル(身長分野、才能分野、出身分野が出たら振り直し)",
    "《メンタルディフェンス》《判定アップ》《個性アップ》のうちいずれか1つ",
    "《メンタルディフェンス》《判定アップ》《個性アップ》のうちいずれか1つ",
];

/// Ruby `SkillGetTable.from_i18n("BeginningIdol.tables.RS", ...)`。
static JA_RS: SkillGetTable = SkillGetTable::new(
    Table::from_dice(
        "アイドルスキル修得表(ロードトゥプリンス)",
        1,
        6,
        JA_RS_ITEMS,
    ),
    &JA_SKILL_TABLE,
    "(身長)分野、(属性|才能)分野、(出身)分野が出たら振り直し",
    "振り直し",
    "特技リスト",
    DEFAULT_SKILL_FORMAT,
);

static JA_CBT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("変わった言葉遣い")),
    (12, TableItem::Text("口ぐせ")),
    (13, TableItem::Text("動物っぽい")),
    (14, TableItem::Text("和風")),
    (15, TableItem::Text("お調子者")),
    (16, TableItem::Text("計算高い")),
    (22, TableItem::Text("妹／姉キャラ")),
    (23, TableItem::Text("ポジティブ！")),
    (24, TableItem::Text("ネガティブ……")),
    (25, TableItem::Text("やんちゃ")),
    (26, TableItem::Text("年齢")),
    (33, TableItem::Text("きぐるみ")),
    (34, TableItem::Text("負けず嫌い")),
    (35, TableItem::Text("努力家")),
    (36, TableItem::Text("語りたがり")),
    (44, TableItem::Text("天然")),
    (45, TableItem::Text("物まね")),
    (46, TableItem::Text("特徴なし")),
    (55, TableItem::Text("直感")),
    (56, TableItem::Text("ピアノ")),
    (66, TableItem::Text("大切な人")),
];

static JA_CBT: D66Table = D66Table::new(
    "キャラ空白表(チャレンジガールズ)",
    D66SortType::Asc,
    JA_CBT_ITEMS,
);

static JA_RCB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("悩み多し")),
    (12, TableItem::Text("俺様")),
    (13, TableItem::Text("弟系")),
    (14, TableItem::Text("がんばり屋")),
    (15, TableItem::Text("物静か")),
    (16, TableItem::Text("不器用")),
    (22, TableItem::Text("二重人格")),
    (23, TableItem::Text("ラッキーボーイ")),
    (24, TableItem::Text("愛され系")),
    (25, TableItem::Text("小悪魔")),
    (26, TableItem::Text("のほほん")),
    (33, TableItem::Text("静かな狂気")),
    (34, TableItem::Text("肉体派")),
    (35, TableItem::Text("ポエマー")),
    (36, TableItem::Text("おせっかい")),
    (44, TableItem::Text("恋愛好き")),
    (45, TableItem::Text("おかん")),
    (46, TableItem::Text("批評家")),
    (55, TableItem::Text("孤高")),
    (56, TableItem::Text("兄貴分")),
    (66, TableItem::Text("女嫌い")),
];

static JA_RCB: D66Table = D66Table::new(
    "キャラ空白表(ロードトゥプリンス)",
    D66SortType::Asc,
    JA_RCB_ITEMS,
);

static JA_HBT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("無趣味")),
    (12, TableItem::Text("ティータイム")),
    (13, TableItem::Text("詩")),
    (14, TableItem::Text("資格修得")),
    (15, TableItem::Text("イラスト")),
    (16, TableItem::Text("ぬいぐるみ")),
    (22, TableItem::Text("睡眠")),
    (23, TableItem::Text("長電話")),
    (24, TableItem::Text("メール")),
    (25, TableItem::Text("昆虫採集")),
    (26, TableItem::Text("編み物")),
    (33, TableItem::Text("食事")),
    (34, TableItem::Text("散歩")),
    (35, TableItem::Text("天体観測")),
    (36, TableItem::Text("カフェ巡り")),
    (44, TableItem::Text("お風呂")),
    (45, TableItem::Text("小物コレクション")),
    (46, TableItem::Text("ガーデニング")),
    (55, TableItem::Text("登山")),
    (56, TableItem::Text("歴史マニア")),
    (66, TableItem::Text("家事")),
];

static JA_HBT: D66Table = D66Table::new(
    "趣味空白表(チャレンジガールズ)",
    D66SortType::Asc,
    JA_HBT_ITEMS,
);

static JA_RHB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("鉄道")),
    (12, TableItem::Text("華道")),
    (13, TableItem::Text("旅行")),
    (14, TableItem::Text("日曜大工")),
    (15, TableItem::Text("習字")),
    (16, TableItem::Text("俳句")),
    (22, TableItem::Text("食べ歩き")),
    (23, TableItem::Text("筋トレ")),
    (24, TableItem::Text("工作")),
    (25, TableItem::Text("資格修得")),
    (26, TableItem::Text("釣り")),
    (33, TableItem::Text("街歩き")),
    (34, TableItem::Text("ファッション")),
    (35, TableItem::Text("飼育")),
    (36, TableItem::Text("いたずら")),
    (44, TableItem::Text("街でナンパ")),
    (45, TableItem::Text("読書")),
    (46, TableItem::Text("家事全般")),
    (55, TableItem::Text("昆虫採集")),
    (56, TableItem::Text("アート")),
    (66, TableItem::Text("睡眠")),
];

static JA_RHB: D66Table = D66Table::new(
    "趣味空白表(ロードトゥプリンス)",
    D66SortType::Asc,
    JA_RHB_ITEMS,
);

static JA_RU_ITEMS: &[&str] = &[
    "激しいアクションで興味を持った人たちを呼び寄せる。\nPC全員の【獲得ファン人数】が5点上昇する。",
    "マスコットキャラクターから聞こえてはいけない音が聞こえてきて、次の瞬間には動かなくなってしまった。\nこのセッションの間、マスコットキャラクターが使用できなくなる。",
    "マスコットキャラクターが行方不明！　プロデューサーが代わりに着ぐるみを着たけれども、負担が大きかった。\n変調「怪我」が発生する。",
    "マスコットキャラクターが不適切な発言をしてしまい、連帯責任で謝罪することになってしまう。\nPC全員の【獲得ファン人数】が、それぞれ5点減少する。",
    "マスコットキャラクターが転んで起き上がれなくなってしまった！　みんなで力を合わせて助け起こそう。\nPC全員の【メンタル】が3点減少する。",
    "マスコットが突然PCに物申す。問題点を挙げて、鍛えてくれる。\nPC一人は、「アイドルスキル修得表」を使って、アイドルスキルを一つ修得する。",
];

static JA_RU: Table = Table::from_dice("マスコット暴走表", 1, 6, JA_RU_ITEMS);

static JA_SIP_ITEMS: &[&str] = &[
    "テレビ番組に出て、ライブの宣伝をする。",
    "ラジオに出演して、ライブの宣伝をする。",
    "動画を配信して、ライブの宣伝をする。",
    "ライブの宣伝のために、街でビラ配りをする。",
    "ライブに人を集めるために、派手なパフォーマンスを街中でする。",
    "ライブの宣伝のために、あちこちを走り回る。",
];

static JA_SIP: Table = Table::from_dice("かんたんパーソン表", 1, 6, JA_SIP_ITEMS);

static JA_BU_ITEMS: &[&str] = &[
    "熱い！　熱い！\n【メンタル】が2点減少する。",
    "慌てて浴槽から出ようとしたが、足を滑らせて浴槽に落ちる。ウケたはいいが、とても熱い。\n【メンタル】が1D6点減少し、【獲得ファン人数】が3D6点上昇する。",
    "温かい目で見守っていた仲間の手を力いっぱい引っ張り、浴槽に引きずり込む。\n自分以外のPCを一人選ぶ。選ばれたPCは、【メンタル】を3点減少させ、「バーストタイム」を行う。",
    "あまりの熱さに浴槽へ入り損ねていたら、仲間の一人に叩き落とされる。\n【メンタル】を2点減少してから、PCを一人選ぶ。選んだPCに対する【理解度】が3点上昇し、チェックを外す。",
    "思い切って氷を頭から浴びる。クールダウン完了！\n【メンタル】を2点減少させることで、もう一度「バーストタイム」を行うことができる。",
    "熱湯風呂に入るための着替えに手間取ってしまい、急かされてしまう。結果、満足に着替えができなかった。\nこのライブフェイズの間、衣装の効果が無効化される。",
];

static JA_BU: Table = Table::from_dice("バースト表", 1, 6, JA_BU_ITEMS);

static JA_HW_ITEMS: &[&str] = &[
    "誰もいない屋内。静寂が世界を包んでいる。嵐の前の静けさだ。",
    "話し声が絶えない夕暮れの帰り道。みんなが明るい声を上げる中、自分の周りだけがぽっかり穴が空いたかのように静かだ。",
    "曇り空になってきた。早く屋内に行かないと、雨でぬれてしまうかもしれない。",
    "ゲリラ豪雨だ。傘も持ってきていないので、激しい雨に打たれるしかない。ついてないな。",
    "夜空を雲が覆いつくしてしまっている。空を見上げても、星の輝きは見えない。",
    "屋内の電気がトラブルで点灯しないようだ。暗い世界は、気分まで滅入ってしまう。",
];

static JA_HW: Table = Table::from_dice("向かい風シーン表", 1, 6, JA_HW_ITEMS);

static JA_FL_ITEMS: &[&str] = &[
    "トレーニングルームで、皆が真剣な顔をしている。真面目な雰囲気が場を支配しており、軽い冗談も言えなさそう。",
    "いつものたまり場。なのに、今日に限って、騒がしさがどこかに行ってしまったようだ。",
    "誰も歩いていない夜道。人気もなく、不安を抱くような暗闇に、足音だけが響いている。",
    "強い風と強い雨が吹きつける事務所の中。外に出れば、吹き飛ばされてしまいそう。",
    "曇り空の下。そこにいるだけで、暗い気持ちになるような、どんよりとした天気。",
    "雨が降り続けている。雨は、ずっと続いている。いつになったら晴れるのだろう。",
];

static JA_FL: Table = Table::from_dice("駆け出しシーン表", 1, 6, JA_FL_ITEMS);

static JA_MSE_ITEMS: &[&str] = &[
    "演目を行ったときに使用できる。自分の【メンタル】が15点になる。この効果は、1回のセッションに1度まで使用できる。",
    "ミラクル・ミラクルシンクロ・パーフェクトミラクルを発生させたときに使用できる。【パフォーマンス値】が10点上昇する。この効果は、1回のセッションに1度まで使用できる。",
    "ファンブルではない判定の後に使用する。判定の達成値を12にする。この効果は、1回のセッションに1度まで使用できる。",
    "演目を行ったときに使用できる。自分以外のPC全員の【メンタル】が2D6点上昇する。この効果は、1回のセッションに1度まで使用できる。",
    "ライブフェイズ開始時に使用する。このフェイズの間、すべての判定の達成値にプラス2の修正がつく。この効果は、1回のセッションに1度まで使用できる。",
    "パフォーマンスのサイコロを振った後に使用する。サイコロ1つの出目を6にすることができる。この効果は、1回のセッションに1度まで使用できる。",
];

static JA_MSE: Table = Table::from_dice("マイスキル効果表", 1, 6, JA_MSE_ITEMS);

static JA_ST_ITEMS: &[&str] = &[
    "見事なパフォーマンスに、人々が感動する。",
    "その声に観客が聞き惚れる。",
    "一糸乱れぬダンスが決まる。",
    "宙に飛ばしたマイクを見事にキャッチする。",
    "トランポリンなどを使って、会場の天井近くまでジャンプ。",
    "観客と一体になって決めポーズ。",
];

static JA_ST: Table = Table::from_dice("演出表", 1, 6, JA_ST_ITEMS);

static JA_FST_ITEMS: &[&str] = &[
    "会場を覆っていた暗雲を退散させる。",
    "会場に花が咲く。",
    "炎の旋風が観客を燃え上がらせる。",
    "ハートの風船が会場中を飛び交う。",
    "羽を生やして会場を飛び回る。",
    "打ち上がった花火と共に決めポーズ。",
];

static JA_FST: Table = Table::from_dice("ファンタジー演出表", 1, 6, JA_FST_ITEMS);

static JA_BWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("先輩アイドルが司会を務めるバラエティ番組に出演！　どんなコーナーを任されるんだろう？\n特技 : 趣味分野からランダム")),
    (13, TableItem::Text("先輩アイドルと街歩き番組で共演。この街には何があるんだろう？\n特技 : 出身分野からランダム")),
    (14, TableItem::Text("先輩アイドルが音楽番組に出演。バックダンサーを務めることに。\n特技 : 《ダンス／趣味9》")),
    (15, TableItem::Text("先輩アイドルと一緒にグラビア撮影。負けないように目立とう。\n特技 : 《マイペース／キャラ4》")),
    (16, TableItem::Text("アイドル紹介番組で、先輩アイドルから紹介される。元気にいこう。\n特技 : 《元気／キャラ8》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("先輩アイドルのラジオ番組にゲスト出演。トークでうまく盛り上げられるかな？\n特技 : 《キャラ分野の空白／キャラ7》")),
    (24, TableItem::Text("同期アイドルたちと一緒に、大掛かりなアスレチックセットに挑戦！\n特技 : 《運動神経／才能8》")),
    (25, TableItem::Text("特別な衣装で行う演劇の仕事がやって来た。どんな衣装なんだろう？\n特技 : 属性分野からランダム")),
    (26, TableItem::Text("プロダクションの超大御所が出演する番組に呼ばれる。\n特技 : 《胆力／才能5》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("シーンプレイヤーのキャラクターを活かしたミニドラマが制作される。\n特技 : シーンプレイヤーの修得しているキャラ分野の特技")),
    (35, TableItem::Text("海外ロケを行うプロダクション制作の旅番組に出演。今日はどこまで行くんだろうか？\n特技 : 《海外／出身12》")),
    (36, TableItem::Text("プロダクション内の劇場で、シーンプレイヤーの「好きなもの」を使った演劇が始まる。\n特技 : シーンプレイヤーの修得している趣味分野の特技")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("ファッションブランドと提携したファッションショーに招待される。\n特技 : 《おしゃれ／趣味5》")),
    (46, TableItem::Text("アイドル雑誌の出版社から取材がやってきた。うまく対応しよう。\n特技 : 《物腰丁寧／キャラ10》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("シリーズもののドラマにちょっとした役で出演！\n特技 : 《演技力／才能12》")),
    (66, TableItem::Text("プロダクション内で総選挙が開始！　今回のテーマは……？\n特技 : ランダム")),
];

static JA_BWT: D66Table = D66Table::new(
    "大手芸能プロダクション仕事表",
    D66SortType::Asc,
    JA_BWT_ITEMS,
);

static JA_LWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("パートナープレイヤーの「身体的特徴」にびびっと来たTV曲からオファーが来る。\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (13, TableItem::Text("スタントマンなしのアクションドラマが製作開始！　オーディションを受けることに。\n特技 : 《運動神経／才能8》")),
    (14, TableItem::Text("歌番組のオーディションに挑戦！　たくさんのライバルの中から、選ばれることを目指す！\n特技 : 《集中力／キャラ4》")),
    (15, TableItem::Text("飲み屋に営業へ。アイドルにあまり興味なさそうな客層だけど……。\n特技 : 《マイペース／キャラ4》")),
    (16, TableItem::Text("結婚式のパンフレット写真を撮ることに。花嫁さんってどんな気分だろう？\n特技 : 《スタイル／才能3》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("CDデビューをかけて、バラエティ番組で他のアイドルと対決！\n特技 : キャラ分野からランダム")),
    (24, TableItem::Text("CDショップで、CDを手渡し販売。よろしくお願いします。\n特技 : 《元気／キャラ8》")),
    (25, TableItem::Text("ローカル番組に出演。この地方では、何が流行っているんだろう？\n特技 : 出身分野からランダム")),
    (26, TableItem::Text("劇の脇役を勝ち取るためにオーディションを受ける。平凡な女の子を演じるらしい。\n特技 : 《プレーン／属性7》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("パートナープレイヤーの「好きなもの」が題材のドラマが製作中。オーディションを受けよう。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (35, TableItem::Text("パートナープレイヤーの「嫌いなもの」を題材にしたドラマにオファーが……。\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (36, TableItem::Text("コーラスに欠員が出たアイドルライブの穴埋めとして選ばれる。主役のキャラに合わせないと。\n特技 : 属性分野からランダム")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("PCたちの事務所にレポートのカメラが入る。小さいけどがんばってます！\n特技 : ランダム")),
    (46, TableItem::Text("オフ")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("オフ")),
    (66, TableItem::Text("オフ")),
];

static JA_LWT: D66Table = D66Table::new(
    "弱小芸能プロダクション仕事表",
    D66SortType::Asc,
    JA_LWT_ITEMS,
);

static JA_TWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("シアター内でドラマを収録。テーマは、パートナープレイヤーの「好きなもの」だ。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (13, TableItem::Text("シアター内で売り出すCDを制作。テーマは、シーンプレイヤーの「好きなもの」だ。\n特技 : シーンプレイヤーが修得している趣味分野の特技")),
    (14, TableItem::Text("シアターの売店に駆り出される。直接お客さんと触れ合うチャンス！\n特技 : 《笑顔／才能7》")),
    (15, TableItem::Text("シアター内でグッズを販売。今の売れ線は何かな？\n特技 : キャラ分野からランダム")),
    (16, TableItem::Text("シアター内で握手会を開催！　来てくれたみんなに感謝を。\n特技 : 《気配り／才能9》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("今回の劇、シーンプレイヤーの【背景】を元にしたノンフィクションドラマ！\n特技 : 趣味分野からランダム")),
    (24, TableItem::Text("シアターを使って、バラエティの企画が開始。みんなを笑わせよう。\n特技 : 《ばか／キャラ12》")),
    (25, TableItem::Text("シアターに流れるミニラジオの収録。メンバーに無茶ぶりをするコーナーが見せ場。\n特技 : キャラ分野からランダム")),
    (26, TableItem::Text("シアターにファッションデザイナーを招いてファッションショー。うまく魅せていこう。\n特技 : 属性分野からランダム")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("シアター企画、1D6時間耐久ダンスが始まる……！\n特技 : 《体力／才能6》")),
    (35, TableItem::Text("シアター企画、パートナープレイヤーは「嫌いなもの」に何回挑戦できるか！\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (36, TableItem::Text("シアター企画、シーンプレイヤー対「嫌いなもの」！\n特技 : シーンプレイヤーが修得しているキャラ分野の特技")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("シアター企画、非常に高いゴールを使ったバスケット！　……本当にできるの？\n特技 : 《190～／身長12》")),
    (46, TableItem::Text("シアター企画、動物と触れ合うアイドルの姿を中継！\n特技 : 《ペット／趣味3》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("シアターの外で行う仕事をこなす。\n特技 : ランダム")),
    (66, TableItem::Text("オフ")),
];

static JA_TWT: D66Table = D66Table::new("ライブシアター仕事表", D66SortType::Asc, JA_TWT_ITEMS);

static JA_CWT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("先生に頼まれて、入部希望者たちの校内案内を務めることになった。\n特技 : 《物腰丁寧／キャラ10》")),
    (13, TableItem::Text("校内イベントの司会進行をすることになった。うまく盛り上げられるかな？\n特技 : 《ポップ／属性9》")),
    (14, TableItem::Text("校内放送に出演。全校生徒の前でも、緊張しないようにしないと。\n特技 : 《胆力／才能5》")),
    (15, TableItem::Text("部活の大会に応援をしに行くことに。長い試合は、体力勝負だ。\n特技 : 《体力／才能6》")),
    (16, TableItem::Text("アイドル部を取材する記者がやって来た。うまく自分たちの言葉を語れるかな？\n特技 : 《気配り／才能9》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("近くの幼稚園で歌を披露することになった。小さい子と目線を合わせないと……。\n特技 : 《～125／身長2》")),
    (24, TableItem::Text("メイド喫茶をすることになった。パートナープレイヤーの「好きなもの」が主なメニューだ。\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (25, TableItem::Text("校内のイベントを取材することになった。そこにはパートナープレイヤーの「嫌いなもの」が……。\n特技 : パートナープレイヤーが修得しているキャラ分野の特技")),
    (26, TableItem::Text("パートナープレイヤーの「ファッション特徴」を活かした、校内ファッションショー。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("地元の商店街からお店の手伝いを依頼される。町を盛り上げていこう。\n特技 : 《ショッピング／趣味8》")),
    (35, TableItem::Text("地元のイベントに出演。郷土愛が試される！\n特技 : プロデューサーが出身分野の特技から選ぶ")),
    (36, TableItem::Text("パートナープレイヤーの「身体的特徴」に惹かれた企業からオファーが来た！\n特技 : 《セクシー／属性4》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("オフ")),
    (46, TableItem::Text("オフ")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("オフ")),
    (66, TableItem::Text("オフ")),
];

static JA_CWT: D66Table = D66Table::new("アイドル部仕事表", D66SortType::Asc, JA_CWT_ITEMS);

static JA_SU_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("飲料水のコマーシャル。美味しそうに飲もう。\n特技 : 《演技力／才能12》")),
    (13, TableItem::Text("夏のパレードのコマーシャル。今回のテーマは、パートナープレイヤーの「好きなもの」。\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (14, TableItem::Text("海水浴場のコマーシャル。見ている人に、元気をおすそ分けできればいいな。\n特技 : 《ポップ／属性9》")),
    (15, TableItem::Text("避暑地のコマーシャル。涼しいところで、ゆったりと過ごしているところをお届け。\n特技 : 《マイペース／キャラ4》")),
    (16, TableItem::Text("虫避け商品のコマーシャル。虫に立ち向かう男らしさを求められる。\n特技 : 《硬派／キャラ9》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("水族館で元気なイルカたちとショーをする。\n特技 : 《元気／キャラ8》")),
    (24, TableItem::Text("少年野球の始球式を任される。球児たちに恥ずかしくないピッチングを魅せないと。\n特技 : 《スポーツ／趣味4》")),
    (25, TableItem::Text("夏のファッションを雑誌で発表することになった。この時期のコーデはこれ！\n特技 : 《おしゃれ／趣味5》")),
    (26, TableItem::Text("番組で夏野菜を美味しく食べる必要に迫られる。ゴーヤをアイドルらしく食べよう。\n特技 : 《沖縄／出身2》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("ビーチバレーで敵のアイドルと対決だ！　夏の暑さに負けるな！\n特技 : 《バーニング／属性10》")),
    (35, TableItem::Text("ビーチで他のアイドルとナンパ対決をすることになった。どちらがうまくこなせるかな？\n特技 : 《軟派／キャラ5》")),
    (36, TableItem::Text("夏のグルメ特集！　夏バテ防止のためにも、たくさん食べるところを魅せなければ。\n特技 : 《料理／趣味6》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("夏休みの子供たちと触れ合う番組に呼ばれる。子供たちの相手も大変だ。\n特技 : 《気配り／才能9》")),
    (46, TableItem::Text("夏の旅行番組。夏ならではの、地元の強みを伝えることに。\n特技 : シーンプレイヤーが修得している出身分野の特技")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("夏だからアツアツのお風呂に叩きこまれる。リアクションを撮りたいようだ。\n特技 : 《ばか／キャラ12》")),
    (66, TableItem::Text("オフ")),
];

static JA_SU: D66Table = D66Table::new("情熱の夏仕事表", D66SortType::Asc, JA_SU_ITEMS);

static JA_WI_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("クリスマスがテーマのコマーシャルソングを歌う。恋人たちに祝福を！\n特技 : 《パッション／属性8》")),
    (13, TableItem::Text("スキー場のコマーシャルムービーを撮ることに。うまく滑れるといいな。\n特技 : 《スポーツ／趣味4》")),
    (14, TableItem::Text("苦い失恋をテーマにしたコマーシャルムービーを撮ることになった。クールに決めよう。\n特技 : 《クール／属性11》")),
    (15, TableItem::Text("スケートリンクのコマーシャルムービーに出演。氷上で華麗にダンスを決めよう。\n特技 : 《ダンス／趣味9》")),
    (16, TableItem::Text("アイスのコマーシャルムービーに出演。寒いのを耐えろ！\n特技 : 《胆力／才能5》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("年末年始に流れる鉄道のコマーシャルムービーに起用される。清潔感のある演技が必要だ。\n特技 : 《プレーン／属性7》")),
    (24, TableItem::Text("温泉地のコマーシャル。温泉に必要なのは、やっぱりセクシーさかな？\n特技 : 《セクシー／属性4》")),
    (25, TableItem::Text("冬ファッションを雑誌で紹介することになった。冬コーデはこれで決まり！\n特技 : 《おしゃれ／趣味5》")),
    (26, TableItem::Text("雪かきの大変さを伝えるために、東北で雪かきを手伝う。これは重労働だ。\n特技 : 《東北地方／出身10》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("年末のバラエティ番組で、関西の芸人と一緒にコントをやることになった。ノリを合わせよう。\n特技 : 《近畿地方／出身6》")),
    (35, TableItem::Text("年末のフェスで、有名アイドルたちと共演！　スターに負けない迫力を出そう。\n特技 : 《スター／属性12》")),
    (36, TableItem::Text("冬の北海グルメを伝えるために、北海道に飛ぶ。果たして、寒さに耐えられるのか。\n特技 : 《北海道／出身11》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("冬のグルメ特集。鍋の作り方を教えます。\n特技 : 《料理／趣味6》")),
    (46, TableItem::Text("バレンタインに向けて、女の子たちにアドバイス。\n特技 : 《フェミニン／属性5》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("冬だからアツアツのお風呂に叩きこまれる。暖かいというか熱い！\n特技 : 《ばか／キャラ12》")),
    (66, TableItem::Text("オフ")),
];

static JA_WI: D66Table = D66Table::new("ぬくもりの冬仕事表", D66SortType::Asc, JA_WI_ITEMS);

static JA_NA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("渓流で釣り対決！　たくさん釣ったほうが勝ち！\n特技 : 《集中力／才能4》")),
    (13, TableItem::Text("海岸で釣りをすることに。釣った魚がおいしいほうが勝ち。\n特技 : 《胆力／才能5》")),
    (14, TableItem::Text("虫取りに挑戦。元気に戯れる図を撮りたいとのこと。\n特技 : 《元気／キャラ8》")),
    (15, TableItem::Text("キャンプを張ろう。みんな快適に寝泊りができるように、心配りが大事。\n特技 : 《気配り／才能9》")),
    (16, TableItem::Text("海で泳ぎの対決。自分のペースを守って、戦おう。\n特技 : 《マイペース／キャラ4》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("森でナンパ対決！　……動物にモテた方が勝ち！\n特技 : 《ペット／趣味3》")),
    (24, TableItem::Text("森で鬼ごっこをすることになった。相手の動きを読めば勝てる！\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (25, TableItem::Text("森の奥で、動物との戦いが始まった。や、やるしかない。\n特技 : 《運動神経／才能8》")),
    (26, TableItem::Text("オフ")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("料理ができる男をここでアピール！　飯盒炊爨（はんごうすいさん）に挑戦！\n特技 : 《料理／趣味6》")),
    (35, TableItem::Text("山登り対決。早く登るよりも、怪我をしないように気をつけたい。\n特技 : 《体力／才能6》")),
    (36, TableItem::Text("薪拾い。地味な絵面になってしまうので、退屈をさせないように魅せよう。\n特技 : 《キャラ分野の空白／キャラ7》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("視聴者が喜びそうなここだけの話をパートナープレイヤーにする。\n特技 : パートナープレイヤーが修得しているキャラ分野の特技")),
    (46, TableItem::Text("些細なことでパートナープレイヤーと喧嘩になる。どっちが強いか勝負だ！\n特技 : パートナープレイヤーが修得している才能分野の特技")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("ドラム缶風呂を用意することに。……熱いんだけど！\n特技 : 《バーニング／属性10》")),
    (66, TableItem::Text("オフ")),
];

static JA_NA: D66Table = D66Table::new("大自然仕事表", D66SortType::Asc, JA_NA_ITEMS);

static JA_GA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("学園が主催しているアイドル触れ合いイベントに出演。美しく振舞おう。\n特技 : 《スタイル／才能3》")),
    (23, TableItem::Text("学園のアイドルたちが出ているラジオに出演。先輩たちに負けないように、がんばろう。\n特技 : 《元気／キャラ8》")),
    (24, TableItem::Text("学園と提携しているブランドのファッションショーに登場。\n特技 : 《おしゃれ／趣味5》")),
    (25, TableItem::Text("学園を紹介するDVDに出演。うまく魅力を紹介できるかな？\n特技 : 《気配り／才能9》")),
    (26, TableItem::Text("学内オーディションに出場。勝ち抜けば、歌番組に出場できる！\n特技 : 《胆力／才能5》")),
    (33, TableItem::Text("学園が制作しているアイドルドラマに吸血鬼役で出演。恐ろし気な演技、できるかな？\n特技 : 《オカルト／趣味2》")),
    (34, TableItem::Text("学園が制作しているドラマに出演。演技の中で、どうやって個性を出していこうか。\n特技 : キャラ分野からランダム")),
    (35, TableItem::Text("学園主催のミニライブに出演。たくさんの出演者の中から、どうやって目立とう。\n特技 : キャラ分野からランダム")),
    (36, TableItem::Text("学園の先輩たちと共演するライブイベントに出演。\n特技 : 《胆力／才能5》")),
    (44, TableItem::Text("学園の紹介で、おいしい芋の紹介番組に出演。北海道に出発だ！\n特技 : 《北海道／出身11》")),
    (45, TableItem::Text("学内オーディションで、ポップなCMのイメージガールを決定。戦い抜こう。\n特技 : 《ポップ／属性9》")),
    (46, TableItem::Text("学内のミュージカルに出演することになった。自分たちの実力を舞台の上で発揮しよう。\n特技 : 《演技力／才能12》")),
    (55, TableItem::Text("市内の店舗を手伝うドキュメンタリー番組を撮ることに。お店を手伝おう。\n特技 : 《物腰丁寧／キャラ10》")),
    (56, TableItem::Text("市内のスタジオで収録されている朝の情報番組に出演。朝から元気に行こう。\n特技 : 《元気／キャラ8》")),
    (66, TableItem::Text("裏山を使った簡単なPV撮影！　山での撮影は体力が要求される。\n特技 : 《体力／才能6》")),
];

static JA_GA: D66Table = D66Table::new("聖デトワール女学園仕事表", D66SortType::Asc, JA_GA_ITEMS);

static JA_BA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("アカデミーの中でも、特に優秀な成績を収めた者を表彰する式が開催される。\n特技 : 《スター／属性12》")),
    (23, TableItem::Text("アカデミー主催の音楽祭に出演。楽器を演奏して、その姿で魅せよう。\n特技 : 《音楽／趣味11》")),
    (24, TableItem::Text("アカデミーが製作しているドラマに出演。脇役だけど、しっかりと存在感を出していこう。\n特技 : 《演技力／才能12》")),
    (25, TableItem::Text("番組の企画で、アカデミー生のアクションを見せることになった。\n特技 : 《運動神経／才能9》")),
    (26, TableItem::Text("番組の1コーナーで、アカデミー生が様々なスポーツに挑戦する必要があるらしい。がんばろう。\n特技 : 《スポーツ／趣味4》")),
    (33, TableItem::Text("先輩と一緒にバラエティ番組に出演。面白いリアクションを期待される。\n特技 : 《ばか／キャラ10》")),
    (34, TableItem::Text("先輩のライブでバックコーラスに参加。美しい声を添えよう。\n特技 : 《音楽／趣味11》")),
    (35, TableItem::Text("先輩のライブでバックダンサーとして出演。ダイナミックな演出に負けないようにしよう。\n特技 : 《ダンス／趣味9》")),
    (36, TableItem::Text("先輩が主演を務めているアニメ映画に脇役の声優として出演。光る演技を見せよう。\n特技 : 《演技力／才能12》")),
    (44, TableItem::Text("同級生と一緒に、漫画作品をモデルにしたミュージカルに出演。熱い気合を求められる。\n特技 : 《バーニング／属性10》")),
    (45, TableItem::Text("同級生と一緒にキャラ付けの強いビジュアル系バンドを組んで、試験のステージで発表。\n特技 : キャラ分野からランダム")),
    (46, TableItem::Text("同級生とファッションを競い合う、セルフプロデュース試験が始まる。\n特技 : 《おしゃれ／趣味5》")),
    (55, TableItem::Text("一般教養の試験が始まる。恐ろしいことに、その様子を生中継するらしい。\n特技 : 《学力／才能10》")),
    (56, TableItem::Text("アイドルの歴史を使った、クイズ試験が始まる。\n特技 : 《アイドル／趣味12》")),
    (66, TableItem::Text("試験のテーマは耽美。セクシーさを仲間と競い合おう。\n特技 : 《セクシー／属性4》")),
];

static JA_BA: D66Table = D66Table::new("アカデミー仕事表", D66SortType::Asc, JA_BA_ITEMS);

static JA_WT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("握手会が始まる。アイドルとして重要な場面、集中していこう。\n特技 : 《アイドル／趣味12》")),
    (13, TableItem::Text("パートナープレイヤーの「嫌いなもの」に取材をすることになった。大丈夫かな……？\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (14, TableItem::Text("シーンプレイヤーの「好きなもの」に関する番組の仕事だ！　テンション上がるー！\n特技 : シーンプレイヤーが修得している趣味分野の特技")),
    (15, TableItem::Text("パートナープレイヤーの「ファッション特徴」を活かす仕事がやって来た。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (16, TableItem::Text("クライアントから、もっとパートナープレイヤーの「個性特技」を推していこうよ、と提案を受ける。\n特技 : パートナープレイヤーの個性特技")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("学園ドラマを撮影！　二人のキャラ付けはどうなるのかな？\n特技 : キャラ分野からランダム")),
    (24, TableItem::Text("ドラマのアクションシーンを撮ることに。コンビネーションで切り抜けよう。\n特技 : 身長分野からランダム")),
    (25, TableItem::Text("感動系のドラマに出演。どんな能力が必要になるかな？\n特技 : 才能分野からランダム")),
    (26, TableItem::Text("趣味の番組に出演。こんな世界があるのか……。\n特技 : 趣味分野からランダム")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("アイドルだらけのバラエティ番組に参戦！　どのアイドルよりも高い点数を取るといいらしい！\n特技 : 《ばか／キャラ12》")),
    (35, TableItem::Text("スポーツ番組の賑やかしとして呼ばれた！　スポーツのこと、わかりますか？\n特技 : 《スポーツ／趣味4》")),
    (36, TableItem::Text("クイズ番組に二人でゲスト出演。力を合わせて勝ち抜くか、それとも自分らしさを重視するか……。\n特技 : 《学力／才能10》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("料理番組に出演。どんな料理を作ろうかな。\n特技 : 《料理／趣味8》")),
    (46, TableItem::Text("CDショップでサイン会を開催。ファンを喜ばせることができるかな？\n特技 : 《気配り／才能9》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("二人にグラビア撮影の仕事が来た。スタイルのよさを魅せるチャンス？\n特技 : 《スタイル／才能3》")),
    (66, TableItem::Text("オフ")),
];

static JA_WT: D66Table = D66Table::new("仕事表", D66SortType::Asc, JA_WT_ITEMS);

static JA_VA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("食べたものをリポートする番組に出演。どうすれば味を言葉で表現できるだろうか？\n特技 : 《料理／趣味6》")),
    (13, TableItem::Text("スポーツする番組に出演。どれだけ活躍できるかが試される。\n特技 : 《スポーツ／趣味4》")),
    (14, TableItem::Text("トーク番組に出演。二人の喋りをうまく魅せなければ。\n特技 : 《気配り／才能9》")),
    (15, TableItem::Text("猛獣の檻に入って、ボール遊びをすることに。度胸が肝心！\n特技 : 《胆力／才能5》")),
    (16, TableItem::Text("パートナープレイヤーの「好きなもの」を題材にした番組コーナーを任された。\n特技 : パートナープレイヤーの修得している趣味分野の特技")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("街歩き番組に出演。軽快なトークで、素人の人たちからうまい言葉を引き出そう。\n特技 : 《軟派／キャラ5》")),
    (24, TableItem::Text("釣り番組に出ることになったが、大物を釣るまで帰れないという。さて、動きの少ない釣りでどうやって目立つか……。\n特技 : 《キャラ分野の空白／キャラ7》")),
    (25, TableItem::Text("パートナープレイヤーの「嫌いなもの」を題材にした番組コーナーで、いじり役を任されることになった。\n特技 : パートナープレイヤーの修得しているキャラ分野の特技")),
    (26, TableItem::Text("クイズ番組に出演。問題とどう向き合うか、それが大事だ。\n特技 : 《学力／才能10》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("農作業体験番組に出演。クワを持って畑に出かけよう。\n特技 : 《体力／才能6》")),
    (35, TableItem::Text("工作体験番組に出演。いい物をスタジオにお届けできるよう、物づくりを真剣に行おう。\n特技 : 《集中力／才能4》")),
    (36, TableItem::Text("電子ゲームやアナログゲームを遊ぶ番組に出演。どんな風に盛り上げられるかな？\n特技 : 《ゲーム／趣味10》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("今日は漫才をするようだ。笑わせるには、何が必要だろうか。\n特技 : 《ばか／キャラ12》")),
    (46, TableItem::Text("シチュエーションコントに出演。大げさな演技が求められる。\n特技 : 《演技力／才能12》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("趣味的な番組に出ることになった。今日のテーマは何かな。\n特技 : 《趣味分野の空白／趣味7》")),
    (66, TableItem::Text("オフ")),
];

static JA_VA: D66Table = D66Table::new("バラエティ仕事表", D66SortType::Asc, JA_VA_ITEMS);

static JA_MU_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("シーンプレイヤーとパートナープレイヤーでミニライブをすることになった。うまく声を合わせよう。\n特技 : パートナープレイヤーの修得している属性分野の特技")),
    (13, TableItem::Text("歌番組で取り上げられる。どんな気持ちで歌ったのか、うまく表現して伝えよう。\n特技 : 《パッション／属性8》")),
    (14, TableItem::Text("パートナープレイヤーとハーモニーを重ねる歌がやってきた。二人の特徴をうまく声に乗せよう。\n特技 : パートナープレイヤーの修得している才能分野の特技")),
    (15, TableItem::Text("CDの手渡し販売が開始。心を込めて、笑顔で手渡しだ。\n特技 : 《笑顔／才能7》")),
    (16, TableItem::Text("レコーディングに音楽業界の大物が立ち会う。緊張せず、自分の実力を発揮しよう。\n特技 : 《胆力／才能5》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("パートナープレイヤーと話し合って歌詞を作ることになった。お互いのことをよく理解しよう。\n特技 : パートナープレイヤーの修得しているキャラ分野の特技")),
    (24, TableItem::Text("活動範囲を広げるために、色々な楽器に挑戦！\n特技 : 《音楽／趣味11》")),
    (25, TableItem::Text("高級そうなバーで歌うことに。高級感のあるファッションを身につけることを条件に歌うことを許される。\n特技 : 《セレブ／才能11》")),
    (26, TableItem::Text("中学生がターゲットのCDを販売することになった。心の中の中学生を解放するときが来た！\n特技 : 《中二病／キャラ2》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("子供向けの歌を作ることになった。視線を子供に近づけないと。\n特技 : 《～125／身長2》")),
    (35, TableItem::Text("結婚式の歌を任される。祝福の気持ちを込めて歌おう。\n特技 : 《気配り／才能9》")),
    (36, TableItem::Text("レースを盛り上げるアップテンポな曲を作ることに。\n特技 : 《元気／キャラ8》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("パートナープレイヤーの地元を象徴するような歌を作ることになった。地元の話を聞き出そう。\n特技 : パートナープレイヤーの修得している出身分野の特技")),
    (46, TableItem::Text("ホラームービーの主題歌を担当することになった。怖さを声で表現できるだろうか？\n特技 : 《オカルト／趣味2》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("作曲家の先生と打ち合わせ。自分のイメージをうまく伝えられるかな？\n特技 : 《物腰丁寧／キャラ10》")),
    (66, TableItem::Text("オフ")),
];

static JA_MU: D66Table = D66Table::new("音楽関係仕事表", D66SortType::Asc, JA_MU_ITEMS);

static JA_DR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("エキストラで出演。できるだけ目立たないように……。\n特技 : 《プレーン／属性7》")),
    (13, TableItem::Text("刑事ドラマに刑事役で出演。クールに決めよう。\n特技 : 《クール／属性11》")),
    (14, TableItem::Text("刑事ドラマに犯人役で出演。悪いことをはぐらかす演技が必要だ。\n特技 : 《ミステリアス／キャラ3》")),
    (15, TableItem::Text("医療ドラマに医者役で出演。臓器や血にひるまずがんばろう。\n特技 : 《胆力／才能5》")),
    (16, TableItem::Text("再現アニメの吹き替えに挑戦。できるだけ丁寧に。\n特技 : 《物腰丁寧／キャラ10》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("悪役に挑戦。暗い気持ちが必要だ。\n特技 : 《ダーク／属性3》")),
    (24, TableItem::Text("学園ドラマに悩める生徒役で出演。青春らしさをどこまで見せようか。\n特技 : 《中二病／キャラ2》")),
    (25, TableItem::Text("時代劇に出演。硬派に決めるシーンがやって来た。\n特技 : 《硬派／キャラ9》")),
    (26, TableItem::Text("特撮にヒーロー役で出演。熱い演技を見せるとき！\n特技 : 《バーニング／属性10》")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("共演者に超有名人が！　緊張せずにがんばろう。\n特技 : 《マイペース／キャラ4》")),
    (35, TableItem::Text("気難しい監督にいろいろ注意される。どう対応したものか……。\n特技 : 《スター／属性12》")),
    (36, TableItem::Text("パートナープレイヤーの【背景】を再現したミニドラマを撮るようだ。彼の過去をどう表現したものか。\n特技 : パートナープレイヤーの修得している属性分野の特技")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("アクションドラマに出演。アクションを決められるかどうかがカギだ。\n特技 : 《運動神経／才能8》")),
    (46, TableItem::Text("恋愛ドラマに出演。共演者をどきどきさせる演技が必要とのこと。\n特技 : 《セクシー／属性4》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("感動系ドラマに出演。泣くシーンは、ここ一番の見せ場だ！\n特技 : 《演技力／才能12》")),
    (66, TableItem::Text("オフ")),
];

static JA_DR: D66Table = D66Table::new("ドラマ関係仕事表", D66SortType::Asc, JA_DR_ITEMS);

static JA_VI_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("ビーチでグラビア撮影の仕事が入る。肉体美を見せるとき。\n特技 : 《セクシー／属性4》")),
    (13, TableItem::Text("ファッション誌の取材がやって来て、自分らしいファッションを見せてほしいと頼まれる。\n特技 : 《キャラ分野の空白／キャラ7》")),
    (14, TableItem::Text("地方ごとのファッションを取り入れるファッションショーが開幕。出演することに。\n特技 : 出身分野からランダム")),
    (15, TableItem::Text("ファッションショーにモデルとして登場。完璧なスタイルを見せつけろ。\n特技 : 《スタイル／才能3》")),
    (16, TableItem::Text("雑誌でシーンプレイヤーおすすめコーデを紹介するようだ。どんな組み合わせを街に発信しようか。\n特技 : 《おしゃれ／趣味5》")),
    (22, TableItem::Text("オフ")),
    (23, TableItem::Text("パートナープレイヤーの「身体的特徴」がちょっとした流行になった。乗るしかない、このビッグウェーブに。\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (24, TableItem::Text("深夜番組の1コーナーに、シーンプレイヤーの「身体的特徴」の特集が組まれるようだ。それに乗っかろう。\n特技 : シーンプレイヤーが修得している身長分野の特技")),
    (25, TableItem::Text("ニュース番組の1コーナーで、パートナープレイヤーの「ファッション特徴」が紹介される。うまくアシストしよう。\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (26, TableItem::Text("シーンプレイヤーの「ファッション特徴」を売っている企業から、CMに出てくれないかと頼まれる。\n特技 : シーンプレイヤーが修得している属性分野の特技")),
    (33, TableItem::Text("オフ")),
    (34, TableItem::Text("女性のファッションについて語る番組に出演。\n特技 : 《フェミニン／属性5》")),
    (35, TableItem::Text("高い身長向けの衣装が届いた。これを使ってうまく魅せられるのか？\n特技 : 《190～／身長12》")),
    (36, TableItem::Text("テレビ番組でアニメキャラのコスプレをすることに。自分らしく決めポーズ。\n特技 : 《趣味分野の空白／趣味7》")),
    (44, TableItem::Text("オフ")),
    (45, TableItem::Text("低身長向けコーデを作ったデザイナーから連絡が入る。それを見事に着こなしてほしいとのこと。\n特技 : 《146／身長6》")),
    (46, TableItem::Text("子供と共演するCMを撮ることになった。子供に愛されるファッションを考えよう。\n特技 : 《～125／身長2》")),
    (55, TableItem::Text("オフ")),
    (56, TableItem::Text("パートナープレイヤーを宣材用の撮影向けにコーディネートすることになった。さて、どうしてやろうか。\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (66, TableItem::Text("オフ")),
];

static JA_VI: D66Table = D66Table::new("ビジュアル関係仕事表", D66SortType::Asc, JA_VI_ITEMS);

static JA_SP_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("マラソン大会に参加することになった。とにかく、走ろう。\n特技 : 《体力／才能6》")),
    (23, TableItem::Text("サッカー選手たちにインタビュー！　サッカーの魅力を聞き出そう。\n特技 : 《スポーツ／趣味4》")),
    (24, TableItem::Text("野球の始球式をすることになった。自分らしく、キャラクターを前面に出して投げるのがコツ。\n特技 : 《キャラ分野の空白／キャラ7》")),
    (25, TableItem::Text("バスケットボールを体験！　その魅力を伝えよう。\n特技 : 《スポーツ／趣味4》")),
    (26, TableItem::Text("ラグビーのパワフルさを間近で体験。見ている人たちにも迫力を伝えないと。\n特技 : 《スポーツ／趣味4》")),
    (33, TableItem::Text("アメフトのハーフタイムショーの短い時間を任される。集中して魅力を出し切ろう。\n特技 : 《集中力／才能4》")),
    (34, TableItem::Text("チアリーディング（男性アイドルは応援団）でスポーツ選手たちの応援をすることに。みんながんばれ！\n特技 : 《キュート／属性6》")),
    (35, TableItem::Text("陸上競技を一通り体験！　それぞれの種目の見どころを発信しよう。\n特技 : 《運動神経／才能8》")),
    (36, TableItem::Text("水泳をする仕事がやってきた。競泳水着をカッコよく着こなそう。\n特技 : 《クール／属性11》")),
    (44, TableItem::Text("スポーツドリンクのCMだ。「生き返る」感じを出していこう。\n特技 : 《パッション／属性8》")),
    (45, TableItem::Text("運動靴のCM。パートナープレイヤーを力強く追い抜いて、速くなれることをアピール！\n特技 : 《バーニング／属性10》")),
    (46, TableItem::Text("ジャージや体操服のCMが入ってきた。うまく着こなして、運動着もおしゃれなことを証明しよう。\n特技 : 《おしゃれ／趣味5》")),
    (55, TableItem::Text("近々開催される大会の応援団長を任された。出場する選手たちにエールを送ろう！\n特技 : 《元気／キャラ8》")),
    (56, TableItem::Text("テニスの試合をパートナープレイヤーとやることになった。ダブルスでいこう。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (66, TableItem::Text("ゴルフコースを回りながら、プロから手ほどきを受けることに。丁寧な言葉遣いで、大人っぽく振舞おう。\n特技 : 《物腰丁寧／キャラ10》")),
];

static JA_SP: D66Table = D66Table::new("スポーツ仕事表", D66SortType::Asc, JA_SP_ITEMS);

static JA_CHR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("雪の積もる野外コンサートホールでミニライブ。苛酷な環境だけど、耐え抜かないと。\n特技 : 《体力／才能6》")),
    (23, TableItem::Text("ラジオの公開録音中に、クリスマスケーキ作りにチャレンジすることになった。うまく作れるかな？\n特技 : 《料理／趣味6》")),
    (24, TableItem::Text("アイドルが提案するクリスマスデート用のファッションを雑誌で紹介。勝てるコーデを考えてみよう。\n特技 : 《おしゃれ／趣味5》")),
    (25, TableItem::Text("ケーキ屋さんと提携して、クリスマスケーキの売り込みをすることに。\n特技 : 《ショッピング／趣味8》")),
    (26, TableItem::Text("地元の中学校のクリスマスイベントに登場。学生のみんなと一緒に盛り上がろう。\n特技 : 《パッション／属性8》")),
    (33, TableItem::Text("クリスマスに放映される特別ドラマにパートナープレイヤーの恋人役で出演。恋愛をうまく演じられるかな？\n特技 : 《演技力／才能12》")),
    (34, TableItem::Text("トークバラエティのクリスマス特番に呼ばれて収録を始める。本日のテーマは、「恋」について。\n特技 : 《軟派／キャラ5》")),
    (35, TableItem::Text("ラジオ番組で、家族と過ごすクリスマスの思い出について語ることになった。\n特技 : 《異国文化／才能2》")),
    (36, TableItem::Text("セクシーサンタグランプリというファッション大会に出ることになった。セクシーアイドルたちと対決だ！\n特技 : 《セクシー／属性4》")),
    (44, TableItem::Text("遊園地で行われるクリスマスイベントのCMを撮影することになった。楽しそうな笑顔を見せよう。\n特技 : 《笑顔／才能7》")),
    (45, TableItem::Text("サンタクロースの格好をして、小学生たちにプレゼントを配ることになった。オファーはかわいさ重視！\n特技 : 《キュート／属性6》")),
    (46, TableItem::Text("サンタクロースの格好をして、街でイベントをすることに。異国っぽさをうまく出せるかな？\n特技 : 《海外／出身12》")),
    (55, TableItem::Text("クリスマスをテーマにした写真集が発売。そのうちの何枚かを担当することに。\n特技 : 《スタイル／才能3》")),
    (56, TableItem::Text("新人アイドルたちが歌うクリスマスソングを収めたカバーアルバムが発売。自分たちも収録されています。\n特技 : 《音楽／趣味11》")),
    (66, TableItem::Text("アイドルとデートをした気分になれるDVDが発売。自分たちも、クリスマス編の収録を行った。\n特技 : 《アイドル／趣味12》")),
];

static JA_CHR: D66Table = D66Table::new("クリスマス仕事表", D66SortType::Asc, JA_CHR_ITEMS);

static JA_PAR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("パートナープレイヤーの【背景】に関する仕事がやって来る。こいつは何を見てきたんだ？\n特技 : パートナープレイヤーの個性特技")),
    (23, TableItem::Text("パートナープレイヤーの「好きなもの」に関する仕事がやって来る。場を盛り上げていこう。\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (24, TableItem::Text("パートナープレイヤーの「嫌いなもの」に関する仕事がやって来る。どうフォローしたものか。\n特技 : パートナープレイヤーが修得しているキャラ分野の特技")),
    (25, TableItem::Text("パートナープレイヤーの「身体的特徴」に関する仕事がやって来る。どこがいいのかを聞いてみる。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (26, TableItem::Text("パートナープレイヤーの「ファッション特徴」に関する仕事がやって来る。自分も真似をすることに。\n特技 : パートナープレイヤーが修得している身長分野の特技")),
    (33, TableItem::Text("パートナープレイヤーの個性特技に関する仕事がやって来る。合わせてみよう。\n特技 : パートナープレイヤーの個性特技")),
    (34, TableItem::Text("パートナープレイヤーの「身体的特徴」に関する仕事がやって来る。どこがいいのかを聞いてみる。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (35, TableItem::Text("パートナープレイヤーが修得している属性分野の特技に関する仕事がやって来る。\n特技 : パートナープレイヤーが修得している属性分野の特技")),
    (36, TableItem::Text("パートナープレイヤーが修得しているキャラ分野の特技に関する仕事がやって来る。\n特技 : パートナープレイヤーが修得しているキャラ分野の特技")),
    (44, TableItem::Text("シーンプレイヤーの【背景】を振り返らせるような仕事がやって来た。今はアイドルとしてそれをこなそう。\n特技 : シーンプレイヤーの個性特技")),
    (45, TableItem::Text("パートナープレイヤーが修得している趣味分野の特技に関する仕事がやって来る。\n特技 : パートナープレイヤーが修得している趣味分野の特技")),
    (46, TableItem::Text("パートナープレイヤーが修得している出身分野の特技に関する仕事がやって来る。\n特技 : パートナープレイヤーが修得している出身分野の特技")),
    (55, TableItem::Text("シーンプレイヤーの個性特技に関する仕事がやって来る。今こそ見せ場だ！\n特技 : シーンプレイヤーの個性特技")),
    (56, TableItem::Text("パートナープレイヤーが修得している才能分野の特技に関する仕事がやって来る。\n特技 : パートナープレイヤーが修得している才能分野の特技")),
    (66, TableItem::Text("シーンプレイヤーの「好きなもの」に関する仕事がやって来る。やったぜ！\n特技 : シーンプレイヤーの個性特技")),
];

static JA_PAR: D66Table = D66Table::new("パートナー関係仕事表", D66SortType::Asc, JA_PAR_ITEMS);

static JA_SW_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("おいし過ぎて止まらない様子を描く、ふわふわなお菓子のCMを行う。\n特技 : 《ポップ／属性9》")),
    (23, TableItem::Text("苦い恋模様を描く、ビターチョコレートのCM撮影を行う。\n特技 : 《ダーク／属性3》")),
    (24, TableItem::Text("甘い恋模様を描く、甘いチョコレートのCM撮影を行う。\n特技 : 《キュート／属性6》")),
    (25, TableItem::Text("家でポリポリ食べているところを描く、スナック菓子のCMを行う。\n特技 : 《プレーン／属性7》")),
    (26, TableItem::Text("青春の汗を流す様子を描く、甘い飲料水のCMを行う。\n特技 : 《バーニング／属性10》")),
    (33, TableItem::Text("チョコレートを食べて脳細胞を活性化させる探偵のドラマに出演する。\n特技 : 《集中力／才能4》")),
    (34, TableItem::Text("朝のシリアルを食べて元気いっぱいな様子を描く、CM撮影を行う。\n特技 : 《元気／キャラ8》")),
    (35, TableItem::Text("眠いときに目がすっきりと覚める様子を描く、刺激の強いお菓子のCM撮影を行う。\n特技 : 《パッション／属性8》")),
    (36, TableItem::Text("一本で栄養補給ができる様子を描く、健康補助食品なお菓子のCM撮影を行う。\n特技 : 《クール／属性11》")),
    (44, TableItem::Text("地元にある駄菓子屋さんのプロモーションを手伝う。\n特技 : 《ショッピング／趣味8》")),
    (45, TableItem::Text("料理番組で、市販のお菓子を使った新しいおやつの開発を任される。\n特技 : 《料理／趣味6》")),
    (46, TableItem::Text("お菓子の家を再現したアトラクション施設を宣伝する。\n特技 : 《フェミニン／属性5》")),
    (55, TableItem::Text("情報番組の1コーナーで、お勧めのケーキを紹介する。\n特技 : 《料理／趣味6》")),
    (56, TableItem::Text("お菓子をテーマにした、夢いっぱいの遊園地の宣伝を行う。\n特技 : 《笑顔／才能7》")),
    (66, TableItem::Text("チョコレートを使ったグラビア撮影をすることになる。\n特技 : 《スタイル／才能3》")),
];

static JA_SW: D66Table = D66Table::new("お菓子仕事表", D66SortType::Asc, JA_SW_ITEMS);

static JA_AN_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("ライオンの檻に、餌を届ける仕事がやって来る。勇気を出して踏み込もう。\n特技 : 《胆力／才能5》")),
    (23, TableItem::Text("ハムスターと戯れる絵を作る。\n特技 : 《ペット／趣味4》")),
    (24, TableItem::Text("牛の乳しぼり体験を動画にしよう。\n特技 : 《集中力／才能4》")),
    (25, TableItem::Text("かわいい猫の動画を撮るために、きまぐれな猫の機嫌をとりにいく。\n特技 : 《ペット／趣味4》")),
    (26, TableItem::Text("犬の散歩シーンを撮るためにも、犬と信頼関係を作る。\n特技 : 《ペット／趣味4》")),
    (33, TableItem::Text("公園の鳩に餌をあげるシーンの手伝いをする。\n特技 : 《マイペース／キャラ4》")),
    (34, TableItem::Text("象の上に乗って、コメントを述べる仕事。\n特技 : 《エスニック／属性2》")),
    (35, TableItem::Text("ぬるぬるしたどじょうを手づかみする絵を要求される。\n特技 : 《セクシー／属性4》")),
    (36, TableItem::Text("ウサギをなでる絵を作る仕事がやって来る。不安そうなウサギを安心させよう。\n特技 : 《ペット／趣味4》")),
    (44, TableItem::Text("奈良の鹿に餌をあげるドラマに出演。\n特技 : 《近畿地方／出身6》")),
    (45, TableItem::Text("馬に乗って、競馬場を駆ける映像を撮ることに。うまく乗りこなそう。\n特技 : 《セレブ／才能11》")),
    (46, TableItem::Text("水族館でペンギンたちと一緒に遊ぶシーンを撮影。\n特技 : 《キュート／属性6》")),
    (55, TableItem::Text("蛇を手づかみする番組企画が入る。\n特技 : 《胆力／才能5》")),
    (56, TableItem::Text("海に入って、魚や貝を見つける企画をすることになった。\n特技 : 《元気／キャラ8》")),
    (66, TableItem::Text("山奥で歩き回って、色々な昆虫を見つける仕事を行う。\n特技 : 《体力／才能6》")),
];

static JA_AN: D66Table = D66Table::new("動物仕事表", D66SortType::Asc, JA_AN_ITEMS);

static JA_MOV_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (22, TableItem::Text("ゾンビ映画にゾンビ役で出演。動く死体らしい演技を心がけよう。\n特技 : 《ダーク／属性3》")),
    (23, TableItem::Text("時代劇映画にサムライ役で出演。厚い忠義を見せよう。\n特技 : 《硬派／キャラ9》")),
    (24, TableItem::Text("西部劇映画にガンマン役で出演。静かに熱い役を演じよう。\n特技 : 《バーニング／属性10》")),
    (25, TableItem::Text("SF映画に未来のエンジニア役で出演。難しい言葉をクールに言い放とう。\n特技 : 《クール／属性11》")),
    (26, TableItem::Text("カンフー映画に若き拳法家役で出演。激しいアクションで敵と戦おう。\n特技 : 《運動神経／才能8》")),
    (33, TableItem::Text("恋愛映画に当て馬役として出演。フラれたあとに感情的になる演技が大事。\n特技 : 《演技力／才能12》")),
    (34, TableItem::Text("現代劇に中学二年生役として出演。現代の若者を演技で表現しよう。\n特技 : 《中二病／キャラ2》")),
    (35, TableItem::Text("特撮ヒーローにヒーロー役として出演。スーツアクターのアクションに、アフレコで魂を載せよう。\n特技 : 《演技力／才能12》")),
    (36, TableItem::Text("ホラー映画に主役として出演。悲鳴や動きで怖がっているところを見せよう。\n特技 : 《ダーク／属性3》")),
    (44, TableItem::Text("インド映画にダンサーとして出演。情熱的な踊りで映画を盛り上げよう。\n特技 : 《ダンス／趣味9》")),
    (45, TableItem::Text("ミステリー映画の犯人役として出演。怪しげな演技で人々を魅了しよう。\n特技 : 《ミステリアス／キャラ3》")),
    (46, TableItem::Text("戦争映画に出演。哀しみの叫びを上げよう。\n特技 : 《演技力／才能12》")),
    (55, TableItem::Text("ちょっとしたお色気シーンを撮ることに。\n特技 : 《セクシー／属性4》")),
    (56, TableItem::Text("ドキュメンタリー映画で、過去の偉人を演じることに。その人の一生をトレースしよう。\n特技 : 《演技力／才能12》")),
    (66, TableItem::Text("おバカな映画に、突き抜けたバカ役として出演。バカになれ！\n特技 : 《ばか／キャラ12》")),
];

static JA_MOV: D66Table = D66Table::new("映画仕事表", D66SortType::Asc, JA_MOV_ITEMS);

static JA_FA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("オフ")),
    (12, TableItem::Text("オフ")),
    (13, TableItem::Text("オフ")),
    (14, TableItem::Text("オフ")),
    (15, TableItem::Text("オフ")),
    (16, TableItem::Text("オフ")),
    (
        22,
        TableItem::Text(
            "ドラゴンと対峙しても、引かない勇気を見せるシーン。\n特技 : 《胆力／才能5》",
        ),
    ),
    (
        23,
        TableItem::Text("怪物の群れを魔法で一掃するシーン。\n特技 : 《ポップ／属性9》"),
    ),
    (
        24,
        TableItem::Text("剣を使って街を荒らす盗賊たちを成敗するシーン。\n特技 : 《硬派／キャラ9》"),
    ),
    (
        25,
        TableItem::Text("斧を振るって、動く植物を破壊するシーン。\n特技 : 《体力／才能6》"),
    ),
    (
        26,
        TableItem::Text(
            "仲間と一緒に槍で突いて、敵の兵士を追い返すシーン。\n特技 : 《気配り／才能9》",
        ),
    ),
    (
        33,
        TableItem::Text(
            "歌と踊りでファンタジーの住民たちを惹きつけるシーン。\n特技 : 《音楽／趣味11》",
        ),
    ),
    (
        34,
        TableItem::Text("酒場で芸を披露して、人気者になるシーン。\n特技 : 《軟派／キャラ5》"),
    ),
    (
        35,
        TableItem::Text(
            "無実の罪でとらえられ、牢屋から脱出するシーン。\n特技 : 《ミステリアス／キャラ3》",
        ),
    ),
    (
        36,
        TableItem::Text("突然現れた魔物の群れに襲われるシーン。\n特技 : 《どじ／キャラ11》"),
    ),
    (
        44,
        TableItem::Text("巨大な魔物に、みんなで立ち向かっていくシーン。\n特技 : 《気配り／才能9》"),
    ),
    (
        45,
        TableItem::Text("悪の魔法使いによって、呪いをかけられるシーン。\n特技 : 《ダーク／属性3》"),
    ),
    (
        46,
        TableItem::Text("新しい武器と防具を調達して、着こなすシーン。\n特技 : 《おしゃれ／趣味5》"),
    ),
    (
        55,
        TableItem::Text("一面の草原を駆け抜けるシーン。\n特技 : 《元気／キャラ8》"),
    ),
    (
        56,
        TableItem::Text("疲れている王様を元気づけてあげるシーン。\n特技 : 《パッション／属性8》"),
    ),
    (
        66,
        TableItem::Text("空を駆ける不思議な船に乗って、感動するシーン。\n特技 : 《笑顔／才能7》"),
    ),
];

static JA_FA: D66Table = D66Table::new("ファンタジー仕事表", D66SortType::Asc, JA_FA_ITEMS);

static JA_BVT_ITEMS: &[&str] = &[
    "自社内テレビスタジオ",
    "自社内ライブステージ",
    "自社番組",
    "イベント会場",
    "ショッピングセンター",
    "自社主催フェス",
];

static JA_BVT: Table = Table::from_dice("大手芸能プロダクション会場表", 1, 6, JA_BVT_ITEMS);

static JA_LVT_ITEMS: &[&str] = &[
    "地方のお祭り",
    "CDショップ前",
    "小劇場",
    "音楽番組",
    "ローカルテレビ",
    "芸能関係社共同開催フェス",
];

static JA_LVT: Table = Table::from_dice("弱小芸能プロダクション会場表", 1, 6, JA_LVT_ITEMS);

static JA_TVT_ITEMS: &[&str] = &[
    "ライブシアター",
    "ライブシアター",
    "ライブシアター",
    "ライブシアター",
    "ライブシアター",
    "ライブシアター",
];

static JA_TVT: Table = Table::from_dice("ライブシアター会場表", 1, 6, JA_TVT_ITEMS);

static JA_CVT_ITEMS: &[&str] = &[
    "運動場",
    "体育館",
    "屋上",
    "街中",
    "地元のイベント",
    "学園アイドル大会",
];

static JA_CVT: Table = Table::from_dice("アイドル部会場表", 1, 6, JA_CVT_ITEMS);

static JA_BST_ITEMS: &[&str] = &[
    "社内プロジェクトルーム",
    "社内カフェ",
    "社内プール",
    "社内レッスンルーム",
    "ショッピングセンター",
    "社内エステルーム",
];

static JA_BST: Table = Table::from_dice("大手芸能プロダクション場所表", 1, 6, JA_BST_ITEMS);

static JA_LST_ITEMS: &[&str] = &["給湯室", "客間", "居間", "屋上", "社長室", "近所の公演"];

static JA_LST: Table = Table::from_dice("弱小芸能プロダクション場所表", 1, 6, JA_LST_ITEMS);

static JA_TST_ITEMS: &[&str] = &[
    "私たちの舞台",
    "控室",
    "売店",
    "シアター前",
    "レッスンルーム",
    "舞台袖",
];

static JA_TST: Table = Table::from_dice("ライブシアター場所表", 1, 6, JA_TST_ITEMS);

static JA_CST_ITEMS: &[&str] = &["部室", "音楽室", "教室", "屋上", "運動場", "体育館"];

static JA_CST: Table = Table::from_dice("アイドル部場所表", 1, 6, JA_CST_ITEMS);

static JA_BPT_ITEMS: &[&str] = &[
    "先輩アイドルたちの期待",
    "後輩アイドルたちからの憧れ",
    "社長の視察",
    "同期アイドルたちとの競い合い",
    "大物ゲスト登場",
    "TV番組のプロデューサー",
];

static JA_BPT: Table = Table::from_dice(
    "大手芸能プロダクションプレッシャー種別決定表",
    1,
    6,
    JA_BPT_ITEMS,
);

static JA_LPT_ITEMS: &[&str] = &[
    "熱心にライブに来てくれるファン",
    "とても悪い条件でのステージ",
    "大手プロダクションの視察",
    "ドレスデザイナーの品定め",
    "新曲を提供したミュージシャン",
    "取材に来た芸能記者",
];

static JA_LPT: Table = Table::from_dice(
    "弱小芸能プロダクションプレッシャー種別決定表",
    1,
    6,
    JA_LPT_ITEMS,
);

static JA_TPT_ITEMS: &[&str] = &[
    "ライブシアターに毎日来てくれるファン",
    "ライブシアターで働くスタッフ",
    "シアター経営者の視察",
    "シアターに偶然立ち寄った大勢の観客",
    "並行して行われるイベントのファン",
    "ライバルライブシアターのアイドルユニット",
];

static JA_TPT: Table = Table::from_dice("ライブシアタープレッシャー種別決定表", 1, 6, JA_TPT_ITEMS);

static JA_CPT_ITEMS: &[&str] = &[
    "ライバルチーム「海原校」の挑戦",
    "最強チーム「Tiara's」の偵察",
    "PTAの視察",
    "PCの親",
    "仲の良い同級生",
    "ライバルチーム「聖国際女学園」の挑戦",
];

static JA_CPT: Table = Table::from_dice("アイドル部プレッシャー種別決定表", 1, 6, JA_CPT_ITEMS);

static JA_BIT_ITEMS: &[&str] = &[
    "栄養ドリンク",
    "自動販売機",
    "先輩アイドルのポスター",
    "先輩アイドルのCD",
    "アイドル雑誌",
    "台本",
];

static JA_BIT: Table = Table::from_dice("大手芸能プロダクション道具表", 1, 6, JA_BIT_ITEMS);

static JA_LIT_ITEMS: &[&str] = &[
    "セロハンテープ",
    "冷蔵庫",
    "百円玉",
    "ボロボロのソファー",
    "おにぎり",
    "キッチン",
];

static JA_LIT: Table = Table::from_dice("弱小芸能プロダクション道具表", 1, 6, JA_LIT_ITEMS);

static JA_TIT_ITEMS: &[&str] = &[
    "舞台の照明",
    "企画書",
    "PCのグッズ",
    "ホワイトボード",
    "ライブポスター",
    "うどん",
];

static JA_TIT: Table = Table::from_dice("ライブシアター道具表", 1, 6, JA_TIT_ITEMS);

static JA_CIT_ITEMS: &[&str] = &[
    "パソコン",
    "机",
    "鞄",
    "ハンディカメラ",
    "ジャージ",
    "投票箱",
];

static JA_CIT: Table = Table::from_dice("アイドル部道具表", 1, 6, JA_CIT_ITEMS);

static JA_CHO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("冒険だ／アドベンチャー\nこの演目の間、スペシャル値が1点減少。")),
    (12, TableItem::Text("世界中の愛／ラブ\nPC全員の【メンタル】が3点上昇。")),
    (13, TableItem::Text("今届けたい／待ちきれない\nこの演目の【パフォーマンス値】が1点上昇。")),
    (14, TableItem::Text("負けないぜ／勝ちたい\nこの演目のPPが5点減少（最低0）。")),
    (15, TableItem::Text("感謝の言葉／ありがとうと\n変調がすべて回復する。")),
    (16, TableItem::Text("キミとなら／あなたたちが\nこの演目の間、すべての【理解度】が2点高いものとして扱う。")),
    (22, TableItem::Text("仲間となら／仲間の\nPC全員は、自分以外のキャラクターに対する【理解度】が1点上昇。")),
    (23, TableItem::Text("マジで好き／MAX\nPC全員の【メンタル】が1D6点上昇。")),
    (24, TableItem::Text("死にたいぐらいに／闇に落ちて\nこの演目の間、スペシャル値とファンブル値が1点減少。")),
    (25, TableItem::Text("だけど／でも\nこの演目の間、一芸突破の目標値が5点になる。")),
    (26, TableItem::Text("キスよりも／囁くよりも\nこの演目の間、【ビジュアル】が1点上昇する。")),
    (33, TableItem::Text("一緒にいてほしい／ずっと\nアイドルクラスが「ほのぼの」のPC全員は【思い出】を1つ獲得。")),
    (34, TableItem::Text("走る／走れ\nこの演目の間、【フィジカル】が1点上昇する。")),
    (35, TableItem::Text("待ち焦がれる／いつまでも\nこの演目の間、シンフォニーを行ったとき、サイコロ1つの出目を1に変更できる。")),
    (36, TableItem::Text("真夜中の／真昼の\nミラクル・ミラクルシンクロが発生したとき、【パフォーマンス値】にプラス5。")),
    (44, TableItem::Text("笑おうぜ／笑顔で\nアイドルクラスが「コメディ」のPC全員は【獲得ファン人数】が［2D6に【ランク係数】をかけた数］点上昇。")),
    (45, TableItem::Text("パーティだ／お金でも\nPC全員は、アイテムを1つランダムに獲得する。")),
    (46, TableItem::Text("だから／それから\nこの演目の間、一芸突破以外の判定の達成値が1点上昇する。")),
    (55, TableItem::Text("オレだけを見ろ／独占したい\n一芸突破を行っても、【メンタル】が減少しない。")),
    (56, TableItem::Text("オレたちの歌／歌おう\nこの演目の間、【ボイス】が1点上昇する。")),
    (66, TableItem::Text("愛しています／好きだ\nアイドルクラスが「マジメ」のPC全員の【メンタル】が5点上昇。")),
];

static JA_CHO: D66Table = D66Table::new("サビ表", D66SortType::Asc, JA_CHO_ITEMS);

static JA_SCH_ITEMS: &[&str] = &[
    "夏は\nこの演目の間、PC全員の【パフォーマンス値】が1点上昇。",
    "熱いぜ！\nこの演目の間、PC全員のスペシャル値が1点減少。",
    "水着が\nこの演目の間、衣装の効果によって上昇する値にプラス1。",
    "乾いた喉\nこの演目の間、PC全員のファンブル値が3点上昇。",
    "潤う\n【メンタル】が5点以下のPC全員の【メンタル】が10点上昇。",
    "弾けて\nこの演目の間、パーフェクトミラクルの【パフォーマンス値】が15点上昇。",
];

static JA_SCH: Table = Table::from_dice("情熱の夏サビ表", 1, 6, JA_SCH_ITEMS);

static JA_WCH_ITEMS: &[&str] = &[
    "雪のような\nPC全員の【メンタル】が3点上昇。",
    "チョコレートに\nPC1人の【メンタル】が10点上昇。",
    "溶かしてあげる\nこの演目の間、PC全員の判定の達成値が1点上昇。",
    "特別な日\nこの演目の間、PC1人のスペシャル値が3点減少。",
    "吹雪が\nこの演目の間、ミラクルの【パフォーマンス値】が10点上昇。",
    "寒さも忘れて\nこの演目の間、PCはファンブルが発生しても、変調が発生しない。",
];

static JA_WCH: Table = Table::from_dice("ぬくもりの冬サビ表", 1, 6, JA_WCH_ITEMS);

static JA_NCH_ITEMS: &[&str] = &[
    "野生の\nPC全員の【メンタル】が1D6点上昇。",
    "パワーで\nこの演目の間、PC1人の【パフォーマンス値】が1D6点上昇。",
    "鍛えた体\nPC全員の【メンタル】が3点上昇。",
    "掴みとる\nこの演目の間、PC全員の【パフォーマンス値】が1点上昇。",
    "抱き留める\nこの演目の間、PC1人の【パフォーマンス値】が3点上昇。",
    "毎日が戦い\nPC1人の【獲得ファン人数】が［2D6に【ランク係数】をかけた数］点上昇。",
];

static JA_NCH: Table = Table::from_dice("大自然サビ表", 1, 6, JA_NCH_ITEMS);

static JA_GCH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("女の子だから／キュンキュンしてる\nPC全員の【メンタル】が1D6点上昇する。")),
    (12, TableItem::Text("見つめていたい／心の声\nこの演目の間、【パフォーマンス値】が2点上昇する。")),
    (13, TableItem::Text("私の気持ち／あなたへ届け\nこの演目の間、【協調値】が1点上昇する。")),
    (14, TableItem::Text("繋がりたい／夜を過ごしたい\nPC全員の【メンタル】が1D6点上昇する。")),
    (15, TableItem::Text("手と手を繋いで／みんなと一緒に\nこの演目の間、シンフォニーをするたびに、【メンタル】が5点上昇する。")),
    (16, TableItem::Text("ファッションで／おしゃれして\n衣装の効果が1点上昇する。")),
    (22, TableItem::Text("アイドルだけど／アイドルとして\nこの演目の間、意地判定の達成値が2点上昇する。")),
    (23, TableItem::Text("愛してる／好きです\nこの演目の間、【協調値】が1点上昇する。")),
    (24, TableItem::Text("恋したい／恋してる\nこの演目の間、【協調値】が1点上昇する。")),
    (25, TableItem::Text("LOVE／「大好き」\nこの演目の間、【協調値】が1点上昇する。")),
    (26, TableItem::Text("お母さんには秘密／ヴェールでかくして\nこの演目の間、【メンタル】が減少しない。")),
    (33, TableItem::Text("愛に溺れて／沈んでいく\nこの演目の間、シンフォニーをするたびに、【パフォーマンス値】が2点上昇する。")),
    (34, TableItem::Text("潰してほしい／壊して\nこの演目の間、判定に失敗したPCは【獲得ファン人数】が2D6点上昇する。")),
    (35, TableItem::Text("どんなに遠くに／離れても\nこの演目の間、すべてのギャップは埋まっているものとして扱う。")),
    (36, TableItem::Text("想いを届けて／胸に秘めた鼓動\nPC全員の【メンタル】が1D6点上昇する。")),
    (44, TableItem::Text("私のことが好きなら／一緒に死にたい\nこの演目の間、【メンタル】が0になっても、行動不能にならない。")),
    (45, TableItem::Text("（台詞）／（ピアノソロ）\nPC全員の【メンタル】が1D6点上昇する。")),
    (46, TableItem::Text("せーのっ／いくよー！\nこの演目の間、PCはパフォーマンスのサイコロすべてを一度だけ振り直すことができる。")),
    (55, TableItem::Text("あの日みたいに／あの子のこと忘れて\nこの演目の間、判定に失敗しても、判定のサイコロを一度だけ振り直すことができる。")),
    (56, TableItem::Text("歌を届けよう／声に想いを\nこの演目の間、【パフォーマンス値】が2点上昇する。")),
    (66, TableItem::Text("（ユニット名）／（PCの名前）\n好きな能力値が1点上昇する。")),
];

static JA_GCH: D66Table = D66Table::new("女性向けサビ表", D66SortType::Asc, JA_GCH_ITEMS);

static JA_PCH_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バトル／戦いに臨む\nこの演目の間、判定の達成値が2点上昇する。")),
    (12, TableItem::Text("宇宙に／銀河が\nこの演目の間、パフォーマンスのサイコロは取り除かれない。")),
    (13, TableItem::Text("空へ／天に向けて\nこの演目の判定に成功したPCは、【メンタル】が10点上昇する。")),
    (14, TableItem::Text("ぶち壊すぜ／むしゃくしゃして\nこの演目の間、一芸突破を行ったときの目標値が4になる。")),
    (15, TableItem::Text("バイクに乗って／ヘリで飛ばして\nPC全員は、アイテム「キャラアイテム」を1個獲得する。")),
    (16, TableItem::Text("アタック／殴りかかる\nこの演目の間、一芸突破を行ったときの判定の達成値が3点上昇する。")),
    (22, TableItem::Text("情熱／熱情\nこの演目の間、スペシャル値が1点減少。")),
    (23, TableItem::Text("走り切るのさ／星の輝き\nこの演目の間、PCの【メンタル】が減少しない。")),
    (24, TableItem::Text("心赴くまま／願いを込めて\nPC全員の【メンタル】が［自分からの【理解度】の合計］点上昇する。")),
    (25, TableItem::Text("高みへ／打ち破る\nこの演目の間、スペシャル値が1点減少。")),
    (26, TableItem::Text("イメージを／覚悟を\nこの演目の間、スペシャル値が1点減少。")),
    (33, TableItem::Text("弱気な自分に／暗闇裂く\nPC全員は、アイテム「ドリーミングシューズ」を1個獲得する。")),
    (34, TableItem::Text("衝動（リビドー）／強敵（ライバル）\nこの演目の間、スペシャル値が1点減少。")),
    (35, TableItem::Text("覚悟を決めて／クライマックス\nこの演目が最終演目の場合、判定の達成値が4点上昇する。")),
    (36, TableItem::Text("最高の力を／最弱脱ぎ捨て\nこの演目の間、スペシャル値が1点減少。")),
    (44, TableItem::Text("自我（エゴ）／瞬間（とき）\nこの演目の間、判定に失敗しても、判定のサイコロを一度だけ振り直すことができる。")),
    (45, TableItem::Text("（台詞）／（ギターソロ）\nこの演目の間、スペシャル値が1点減少。")),
    (46, TableItem::Text("Let's／try\nこの演目の間、判定の達成値が1点上昇する。")),
    (55, TableItem::Text("起死回生／負けたりしない\nPC全員の【メンタル】が1D6点上昇する。")),
    (56, TableItem::Text("共鳴していく／想いを束ねて\nこの演目の間、シンフォニーするたびに、【パフォーマンス値】が2点上昇する。")),
    (66, TableItem::Text("運命（デスティニー）／正義（ジャスティス）\nこの演目の間、スペシャル値が1点減少。")),
];

static JA_PCH: D66Table = D66Table::new("力強いサビ表", D66SortType::Asc, JA_PCH_ITEMS);

static JA_LUR_ITEMS1: &[&str] = &[
    "地元の商店街で",
    "マスコットキャラクターと",
    "地元のプールで",
    "地元の小学校で",
    "地元のショッピングモールで",
    "田んぼの真ん中で",
];

static JA_LUR_ITEMS2: &[&str] = &[
    "愛について叫ぶ",
    "民謡を歌う",
    "ファッションショー",
    "水着で宣伝",
    "ネット配信",
    "お祭り騒ぎ",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.LUR", ...)`。
static JA_LUR: D6TwiceTable =
    D6TwiceTable::new("地方アイドル無茶ぶり表", JA_LUR_ITEMS1, JA_LUR_ITEMS2);

static JA_SUR_ITEMS1: &[&str] = &[
    "海水浴場で",
    "偉い人の前で",
    "あの有名アイドルの前で",
    "仲間の前で",
    "カメラの前で",
    "一般客の前で",
];

static JA_SUR_ITEMS2: &[&str] = &[
    "かき氷いっき食い",
    "ナンパ",
    "スイカ割り",
    "カッコいいポーズ",
    "満面の笑顔",
    "喧嘩のふり",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.SUR", ...)`。
static JA_SUR: D6TwiceTable = D6TwiceTable::new("情熱の夏無茶ぶり表", JA_SUR_ITEMS1, JA_SUR_ITEMS2);

static JA_WUR_ITEMS1: &[&str] = &[
    "クリスマスツリーの前で",
    "子供たちの前で",
    "大雪の中で",
    "雪が降り始めた街で",
    "暖かい部屋の中で",
    "暖房が効きすぎの部屋の中で",
];

static JA_WUR_ITEMS2: &[&str] = &[
    "雪かき",
    "アイスを食べる",
    "薄着で登場",
    "歌ってください",
    "サンタのコスプレ",
    "おでんを急いで食べる",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.WUR", ...)`。
static JA_WUR: D6TwiceTable =
    D6TwiceTable::new("ぬくもりの冬無茶ぶり表", JA_WUR_ITEMS1, JA_WUR_ITEMS2);

static JA_NUR_ITEMS1: &[&str] = &[
    "斧を持って",
    "クワを持って",
    "釣竿を持って",
    "虫取り網を持って",
    "栄養ドリンクの宣伝をしながら",
    "命綱をつけて",
];

static JA_NUR_ITEMS2: &[&str] = &[
    "木を倒す",
    "畑を耕す",
    "昆虫採集",
    "大物を釣る",
    "一晩過ごす",
    "崖を登る",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.NUR", ...)`。
static JA_NUR: D6TwiceTable = D6TwiceTable::new("大自然無茶ぶり表", JA_NUR_ITEMS1, JA_NUR_ITEMS2);

static JA_GUR_ITEMS1: &[&str] = &[
    "裏山で",
    "食堂で",
    "先輩の前で",
    "全国放送で",
    "全校生徒の前で",
    "学園の様子を伝えるネット中継で",
];

static JA_GUR_ITEMS2: &[&str] = &[
    "歌を披露",
    "乗馬",
    "テニス",
    "「個性とは何か」を語る",
    "「アイドルとは何か」を語る",
    "「アイドルをやっていてよかった瞬間」を語る",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.GUR", ...)`。
static JA_GUR: D6TwiceTable =
    D6TwiceTable::new("聖デトワール女学園無茶ぶり表", JA_GUR_ITEMS1, JA_GUR_ITEMS2);

static JA_BUR_ITEMS1: &[&str] = &[
    "TVカメラの前で",
    "ライバルと一緒に",
    "試験で",
    "寮で",
    "幼年部で",
    "初等部で",
];

static JA_BUR_ITEMS2: &[&str] = &[
    "反省会",
    "ゲリラライブ",
    "宿題をこなす",
    "食事を作る",
    "自作の歌を披露",
    "自作のポエムを披露",
];

/// Ruby `D6TwiceTable.from_i18n("BeginningIdol.tables.BUR", ...)`。
static JA_BUR: D6TwiceTable =
    D6TwiceTable::new("アカデミー無茶ぶり表", JA_BUR_ITEMS1, JA_BUR_ITEMS2);

static JA_ACE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("好きな能力値が1点高いものとして扱う。")),
    (12, TableItem::Text("【ボイス】が1点高いものとして扱う。")),
    (13, TableItem::Text("【フィジカル】が1点高いものとして扱う。")),
    (14, TableItem::Text("【ビジュアル】が1点高いものとして扱う。")),
    (15, TableItem::Text("このアクセサリーを装備したとき、【メンタル】が5点上昇する。この効果は、1回のセッションに1度まで使用できる。")),
    (16, TableItem::Text("【パフォーマンス値】が2点上昇する。")),
    (22, TableItem::Text("このアクセサリーを装備したとき、「アイドルスキル修得表」を使って、ランダムにアイドルスキルを1つ修得する。リザルトフェイズにそのアイドルスキルは失われる。この効果は、1回のセッションに1度まで使用できる。")),
    (23, TableItem::Text("開幕演目と最終演目で行う判定の達成値が2点上昇する。")),
    (24, TableItem::Text("【協調値】が1点上昇する。")),
    (25, TableItem::Text("アイドルスキルの効果が1点上昇する。")),
    (26, TableItem::Text("意地判定の達成値が3点上昇する。")),
    (33, TableItem::Text("特殊な演目によって上昇する【獲得ファン人数】が3点上昇する。")),
    (34, TableItem::Text("一芸突破を行ったとき、判定の達成値が2点上昇する。")),
    (35, TableItem::Text("このアクセサリーを装備したとき、好きな特技を1つ選ぶ。選んだ特技は、ライブフェイズの間、修得しているものとして扱う。この効果は、1回のセッションに1度まで使用できる。")),
    (36, TableItem::Text("幕間での判定の達成値が2点上昇する。")),
    (44, TableItem::Text("思い出を使用したとき、【メンタル】が3点上昇する。")),
    (45, TableItem::Text("ミラクルが発生したときの【パフォーマンス値】が15点になる。")),
    (46, TableItem::Text("アイドルスキルを使用したときの判定の達成値が2点上昇する。")),
    (55, TableItem::Text("特別な演目を行っても、【メンタル】が減少しない。")),
    (56, TableItem::Text("最終演目での【メンタル】減少が半分（端数切り捨て）になる。")),
    (66, TableItem::Text("スペシャルが発生したとき、【メンタル】が10点上昇する。")),
];

static JA_ACE: D66Table = D66Table::new("アクセサリー効果表", D66SortType::Asc, JA_ACE_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表の対応）。
static JA_TABLES: &[(&str, &dyn RollText)] = &[
    ("DT", &JA_DT),
    ("RC", &JA_RC),
    ("FC", &JA_FC),
    ("ACB", &JA_ACB),
    ("TN", &JA_TN),
    ("CG", &JA_CG),
    ("GG", &JA_GG),
    ("HA", &JA_HA),
    ("CBT", &JA_CBT),
    ("RCB", &JA_RCB),
    ("HBT", &JA_HBT),
    ("RHB", &JA_RHB),
    ("RU", &JA_RU),
    ("SIP", &JA_SIP),
    ("BU", &JA_BU),
    ("HW", &JA_HW),
    ("FL", &JA_FL),
    ("MSE", &JA_MSE),
    ("ST", &JA_ST),
    ("FST", &JA_FST),
    ("BWT", &JA_BWT),
    ("LWT", &JA_LWT),
    ("TWT", &JA_TWT),
    ("CWT", &JA_CWT),
    ("SU", &JA_SU),
    ("WI", &JA_WI),
    ("NA", &JA_NA),
    ("GA", &JA_GA),
    ("BA", &JA_BA),
    ("WT", &JA_WT),
    ("VA", &JA_VA),
    ("MU", &JA_MU),
    ("DR", &JA_DR),
    ("VI", &JA_VI),
    ("SP", &JA_SP),
    ("CHR", &JA_CHR),
    ("PAR", &JA_PAR),
    ("SW", &JA_SW),
    ("AN", &JA_AN),
    ("MOV", &JA_MOV),
    ("FA", &JA_FA),
    ("BVT", &JA_BVT),
    ("LVT", &JA_LVT),
    ("TVT", &JA_TVT),
    ("CVT", &JA_CVT),
    ("BST", &JA_BST),
    ("LST", &JA_LST),
    ("TST", &JA_TST),
    ("CST", &JA_CST),
    ("BPT", &JA_BPT),
    ("LPT", &JA_LPT),
    ("TPT", &JA_TPT),
    ("CPT", &JA_CPT),
    ("BIT", &JA_BIT),
    ("LIT", &JA_LIT),
    ("TIT", &JA_TIT),
    ("CIT", &JA_CIT),
    ("CHO", &JA_CHO),
    ("SCH", &JA_SCH),
    ("WCH", &JA_WCH),
    ("NCH", &JA_NCH),
    ("GCH", &JA_GCH),
    ("PCH", &JA_PCH),
    ("LUR", &JA_LUR),
    ("SUR", &JA_SUR),
    ("WUR", &JA_WUR),
    ("NUR", &JA_NUR),
    ("GUR", &JA_GUR),
    ("BUR", &JA_BUR),
    ("ACE", &JA_ACE),
    ("ACT", &JA_ACT),
    ("MS", &JA_MS),
    ("RE", &JA_RE),
    ("SH", &JA_SH),
    ("MO", &JA_MO),
    ("SEA", &JA_SEA),
    ("SPA", &JA_SPA),
    ("LN", &JA_LN),
    ("SGT", &JA_SGT),
    ("RS", &JA_RS),
];

/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    skill_table: &JA_SKILL_TABLE,
    item_table: &JA_ITEM_TABLE,
    bad_status_table: &JA_BAD_STATUS_TABLE,
    local_work_table: &JA_LOCAL_WORK_TABLE,
    tables: JA_TABLES,
    success: "成功",
    failure: "失敗",
    fumble: "ファンブル(変調がランダムに1つ発生し、PCは【思い出】を1つ獲得する)",
    special: "スペシャル！(PCは【思い出】を1つ獲得する)",
    burst_name: "バーストタイム",
    burst_burst: "Burst!\n「バースト表」を使用する。",
    burst_critical_success: "大成功\n【獲得ファン人数】が2D6点上昇する。\nPC全員が挑戦者ではない場合、自分以外のPCを一人指名する。指名されたPCは、新たな挑戦者として、【メンタル】を減少させずに「バーストタイム」を行う。",
    burst_success: "成功\n【獲得ファン人数】が2D6点上昇する。",
    attack_name: "攻撃",
    attack_damage: "%{total}ダメージ",
    pd_paformance: "パフォーマンス",
    pd_symphony: "シンフォニー",
    pd_miracle: "【ミラクル】%{value}",
    pd_perfect_miracle: "【パーフェクトミラクル】%{value}",
    pd_miracle_synchro: "【ミラクルシンクロ】%{value}＋シンフォニーを行った人数",
};

/// Ruby `BCDice::GameSystem::BeginningIdol`（ID: `BeginningIdol`）。
///
/// 表とメッセージは `ja_jp` ロケール。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginningIdol;

impl GameSystem for BeginningIdol {
    fn id(&self) -> &'static str {
        "BeginningIdol"
    }

    fn name(&self) -> &'static str {
        "ビギニングアイドル"
    }

    fn sort_key(&self) -> &'static str {
        "ひきにんくあいとる"
    }

    fn help_message(&self) -> &'static str {
        r"これは、2015年に新書サイズで発売された『駆け出しアイドルRPG ビギニングアイドル チャレンジガールズ』およびそのサプリメントに対応したコマンドです。

・パフォーマンス　[r]PDn[+m/-m](r：場に残った出目　n：振る数　m：修正値)
・ワールドセッティング仕事表　BWT：大手芸能プロ　LWT：弱小芸能プロ
　TWT：ライブシアター　CWT：アイドル部　LO[n]：地方アイドル(n：チャンス)
　SU：情熱の夏　WI：ぬくもりの冬　NA：大自然　GA：女学園　BA：アカデミー
・仕事表　WT　VA：バラエティ　MU：音楽関係　DR：ドラマ関係
　VI：ビジュアル関係　SP：スポーツ　CHR：クリスマス　PAR：パートナー関係
　SW：お菓子　AN：動物　MOV：映画　FA：ファンタジー
・ランダムイベント　RE
・ハプニング表　HA
・特技リスト　AT[n](n：分野No.)
・アイドルスキル修得表　SGT：チャレンジガールズ　RS：ロードトゥプリンス
・変調　BT[n](n：発生数)
・アイテム　IT[n](n：獲得数)
・アクセサリー　ACT：種別決定　ACB：ブランド決定　ACE：効果表
・衣装　DT：チャレンジガールズ　RC：ロードトゥプリンス　FC:フォーチュンスターズ
・無茶ぶり表　LUR：地方アイドル　SUR：情熱の夏　WUR：ぬくもりの冬
　NUR：大自然　GUR：女学園　BUR：アカデミー
・センタールール　HW：向かい風シーン表　FL：駆け出しシーン表　LN：孤独表
　マイスキル【MS：名前決定　MSE：効果表】　演出表【ST　FST：ファンタジー】
・合宿ルール　散策表【SH：ショッピングモール　MO：山　SEA：海　SPA：温泉街】
　TN：夜語りシチュエーション表　成長表【CG：コモン　GG：ゴールド】
・サビ表　CHO　SCH：情熱の夏　WCH：ぬくもりの冬　NCH：大自然
　GCH：女性向け　PCH：力強い
・キャラ空白表　CBT：チャレンジガールズ　RCB：ロードトゥプリンス
・趣味空白表　HBT：チャレンジガールズ　RHB：ロードトゥプリンス
・マスコット暴走表　RU
・アイドル熱湯風呂　nC：バーストタイム(n：温度)　BU：バースト表
・攻撃　n[S]A[r][+m/-m](n：振る数　S：失敗しない　r：取り除く出目　m：修正値)
・かんたんパーソン表　SIP
・会場表
　BVT：大手芸能プロ　LVT：弱小芸能プロ　TVT：ライブシアター　CVT：アイドル部
・場所表
　BST：大手芸能プロ　LST：弱小芸能プロ　TST：ライブシアター　CST：アイドル部
・プレッシャー種別決定表
　BPT：大手芸能プロ　LPT：弱小芸能プロ　TPT：ライブシアター　CPT：アイドル部
・道具表
　BIT：大手芸能プロ　LIT：弱小芸能プロ　TIT：ライブシアター　CIT：アイドル部
[]内は省略可　D66入れ替えあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "LO",
            "DT",
            "RC",
            "FC",
            "ACB",
            "TN",
            "CG",
            "GG",
            "HA",
            "CBT",
            "RCB",
            "HBT",
            "RHB",
            "RU",
            "SIP",
            "BU",
            "HW",
            "FL",
            "MSE",
            "ST",
            "FST",
            "BWT",
            "LWT",
            "TWT",
            "CWT",
            "SU",
            "WI",
            "NA",
            "GA",
            "BA",
            "WT",
            "VA",
            "MU",
            "DR",
            "VI",
            "SP",
            "CHR",
            "PAR",
            "SW",
            "AN",
            "MOV",
            "FA",
            "BVT",
            "LVT",
            "TVT",
            "CVT",
            "BST",
            "LST",
            "TST",
            "CST",
            "BPT",
            "LPT",
            "TPT",
            "CPT",
            "BIT",
            "LIT",
            "TIT",
            "CIT",
            "CHO",
            "SCH",
            "WCH",
            "NCH",
            "GCH",
            "PCH",
            "LUR",
            "SUR",
            "WUR",
            "NUR",
            "GUR",
            "BUR",
            "ACE",
            "ACT",
            "MS",
            "RE",
            "SH",
            "MO",
            "SEA",
            "SPA",
            "LN",
            "SGT",
            "RS",
            "RTT[1-6]?",
            "RCT",
            "AT",
            "AT1",
            "AT2",
            "AT3",
            "AT4",
            "AT5",
            "AT6",
            "IT",
            "BT",
            r"\d{2}C",
            r"\d+S?A",
            "[1-7]*PD",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `BeginningIdol#result_nd6`。
    fn result_nd6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_nd6(&JA_SYSTEM, total, dice_total, cmp_op, target)
    }

    /// Ruby `BeginningIdol#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "BeginningIdol",
            "BeginningIdol.toml",
            201,
            &[
                (145, 16),
                (146, 16),
                (147, 16),
                (148, 16),
                (149, 16),
                (150, 16),
                (151, 16),
                (152, 16),
                (153, 16),
                (154, 16),
                (155, 16),
                (156, 16),
                (157, 16),
                (158, 16),
                (159, 16),
                (160, 16),
                (161, 16),
                (162, 16),
                (163, 16),
                (164, 16),
                (165, 16),
                (166, 16),
                (167, 16),
            ],
        );
    }
}
