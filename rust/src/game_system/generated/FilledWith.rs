//! P4で手書き移植した `lib/bcdice/game_system/FilledWith.rb`
//! （と `lib/bcdice/game_system/filled_with/*.rb`）。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `FilledWith::FW`（判定 `3FW@x#y<=z` / `z-3FW@x#y`）
//! - `#roll_jump_table`（ナンバーワンくじ `LOTN` / `LOTP`、マジカルクッキング `COOKx`）
//! - 夢幻の迷宮用の各表（`TRAPd` / `TRSr±x` / `ROPd` / `RANDda` / `RENCda` / `REDde`）
//! - `TABLES`（【必殺技！】 `HST`）
//!
//! # 表データ
//!
//! 表データ（`static` 群）は Ruby のリテラルから機械的に書き出したもので、
//! 値は1文字も変えていない。`Row` の引数は Ruby 側では整数と文字列が混在するが、
//! `Kernel.format` の `%s` で埋め込まれる（`to_s` される）ので文字列で持つ。
//! くじ・クッキング表の `lambda { TABLES[n] }`（別の表へのジャンプ）は
//! [`JumpItem::Jump`] で表す。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{RangeInc, RollableTable, Table};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::{sat_i64, Randomizer};
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::FilledWith`（ID: `FilledWith`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilledWith;

impl GameSystem for FilledWith {
    fn id(&self) -> &'static str {
        "FilledWith"
    }

    fn name(&self) -> &'static str {
        "フィルトウィズ"
    }

    fn sort_key(&self) -> &'static str {
        "ふいるとういす"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 (3FW@x#y<=z or z-3FW@x#y)
 3個の6面ダイスを振る判定。
 @x:xにクリティカル値を入力。省略可。(省略時クリティカル値4)
 #y:yにファンブル値を入力。省略可(省略時ファンブル値17)
 <=z or z-:zに目標値を入力。±の計算に対応。省略可。
・【必殺技！】 (HST)
 ホムンクルス特技【必殺技！】表。
・マジカルクッキング (COOKx)
 マジカルクッキングのシェフのおすすめコース。
 xにクッキングレベルを入力。(1-8)
・ナンバーワンくじ (LOTN or LOTP)
 LOTNでノーマルくじ、LOTPでプレミアムくじ。(GURPS-FW版)
----------夢幻の迷宮用----------
・共通書式
 a:aに地形(1-6の数字)を入力。省略可。(省略時ランダム決定)
  (1:洞窟 2:遺跡 3:山岳 4:水辺 5:森林 6:墓場)
 d:dに難易度を入力。(E:初級 N:中級 H:上級 L:悪夢 X:伝説)
・ランダムイベント表 (RANDda)
・ランダムエンカウント表 (RENCda)
・エネミーデータ表 (REDde)
 エネミーデータ参照表。
 GMがシークレットダイスで参照するとPLに知られずにエネミーデータを参照可能。
 e:3桁のイベントダイスを入力(D666の結果)。
・トラップ表 (TRAPd)
・財宝表 (TRSr±x)
 r:rに財宝ランクを入力。
 ±x:xに財宝ランク修正値を入力。省略可。
・迷宮追加オプション表(ROPd)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "3FW",
            r"[\+\-\d]*-3FW",
            "LOT[NP]",
            "HST",
            "COOK[1-8]",
            "RAND",
            "RENC",
            "RED",
            "TRS",
            "TRAP[ENHLX]",
            "ROP[ENHLX]",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `FilledWith#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `FilledWith#eval_game_system_specific_command` 本体。
///
/// Ruby の `case command` は各 `when` が**アンカーなし**の正規表現なので、
/// ここでも部分一致で分岐し、各処理の中でアンカー付きの正規表現を改めて当てる。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // ダイスロールコマンド
    if let Some(fw) = FW::parse(command)? {
        return Ok(Some(SpecificCommandOutput::result(fw.roll(rng)?)));
    }

    static COOK_RE: OnceLock<Regex> = OnceLock::new();
    static TRAP_RE: OnceLock<Regex> = OnceLock::new();
    static ROP_RE: OnceLock<Regex> = OnceLock::new();
    let cook_re = COOK_RE.get_or_init(|| Regex::new(r"COOK([1-8])").expect("valid regex"));
    let trap_re = TRAP_RE.get_or_init(|| Regex::new(r"TRAP[ENHLX]").expect("valid regex"));
    let rop_re = ROP_RE.get_or_init(|| Regex::new(r"ROP[ENHLX]").expect("valid regex"));

    // 各種コマンド
    if command == "LOTN" {
        let result = roll_jump_table("ナンバーワンノーマルくじ", &LOT_NORMAL_TABLE_1, rng)?;
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if command == "LOTP" {
        let result = roll_jump_table("ナンバーワンプレミアムくじ", &LOT_PREMIUM_TABLE_1, rng)?;
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(m) = cook_re.captures(command) {
        let lv = to_i(&m[1]);
        // Ruby: COOK_TABLES[lv]（1〜8 は必ずある）
        let Some((_, table)) = COOK_TABLES.iter().find(|(key, _)| *key == lv) else {
            return Err(EvalError::Internal(
                "FilledWith: COOK_TABLES[lv] is missing",
            ));
        };
        let result = roll_jump_table("マジカルクッキング", table, rng)?;
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if trap_re.is_match(command) {
        return Ok(roll_trap_table(command, rng)?.map(SpecificCommandOutput::text));
    }
    if command.contains("TRS") {
        return Ok(get_tresure_result(command, rng)?.map(SpecificCommandOutput::text));
    }
    if command.contains("RAND") || command.contains("RENC") {
        return Ok(roll_random_event_table(command, rng)?.map(SpecificCommandOutput::text));
    }
    if command.contains("RED") {
        return Ok(fetch_enemy_data(command).map(SpecificCommandOutput::text));
    }
    if rop_re.is_match(command) {
        return Ok(roll_random_option_table(command, rng)?.map(SpecificCommandOutput::text));
    }

    Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `String#to_i`（符号付きの数字列）。桁あふれは符号側へ飽和させる。
fn to_i(source: &str) -> i64 {
    source.parse().unwrap_or(if source.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// 判定（3FW）
// ---------------------------------------------------------------------------

/// Ruby `FilledWith::FW`（パース済みの判定コマンド）。
struct FW {
    dice_count: i64,
    target: Option<i64>,
    critical: i64,
    fumble: i64,
}

impl FW {
    /// Ruby `FW.parse(command)`。判定コマンドでなければ `None`。
    ///
    /// 目標値の式は `Arithmetic.eval` で評価する（パースできなければ `nil`＝目標値なし）。
    fn parse(command: &str) -> Result<Option<Self>, EvalError> {
        static RE_TARGET_FIRST: OnceLock<Regex> = OnceLock::new();
        static RE_DICE_FIRST: OnceLock<Regex> = OnceLock::new();
        // Ruby: /^(\d[+\-\d]*)-(\d+)FW(?:@(\d+))?(?:\#(\d+))?$/
        let re_target_first = RE_TARGET_FIRST.get_or_init(|| {
            Regex::new(r"^(\d[+\-\d]*)-(\d+)FW(?:@(\d+))?(?:#(\d+))?$").expect("valid regex")
        });
        // Ruby: /(\d+)FW(?:@(\d+))?(?:\#(\d+))?(?:<=([+\-\d]+))?/（アンカーなし）
        let re_dice_first = RE_DICE_FIRST.get_or_init(|| {
            Regex::new(r"(\d+)FW(?:@(\d+))?(?:#(\d+))?(?:<=([+\-\d]+))?").expect("valid regex")
        });

        if let Some(m) = re_target_first.captures(command) {
            return Ok(Some(Self {
                dice_count: to_i(&m[2]),
                target: arithmetic::eval(&m[1], RoundType::Floor)?
                    .as_ref()
                    .map(crate::randomizer::sat_i64),
                critical: m.get(3).map_or(4, |g| to_i(g.as_str())),
                fumble: m.get(4).map_or(17, |g| to_i(g.as_str())),
            }));
        }

        if let Some(m) = re_dice_first.captures(command) {
            let target = match m.get(4) {
                Some(g) => arithmetic::eval(g.as_str(), RoundType::Floor)?,
                None => None,
            };
            return Ok(Some(Self {
                dice_count: to_i(&m[1]),
                target: target.as_ref().map(sat_i64),
                critical: m.get(2).map_or(4, |g| to_i(g.as_str())),
                fumble: m.get(3).map_or(17, |g| to_i(g.as_str())),
            }));
        }

        Ok(None)
    }

    /// Ruby `FW#roll`。
    fn roll(&self, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
        let dice_list = rng.roll_barabara(self.dice_count, 6)?;
        let dice: i64 = dice_list.iter().sum();
        let dice_str = join_dice(&dice_list);

        let mut res = self.result(dice);

        let mut sequence = vec![format!("({})", self.expr()), format!("{dice}[{dice_str}]")];
        // Ruby: `Result.new` の text は nil なので `.compact` で落ちる
        if !res.text.is_empty() {
            sequence.push(res.text.clone());
        }

        res.text = sequence.join(" ＞ ");

        Ok(res)
    }

    /// Ruby `FW#expr`。既定値（C値4・F値17）は省略する。
    fn expr(&self) -> String {
        let mut ret = format!("{}FW", self.dice_count);
        if self.critical != 4 {
            ret.push_str(&format!("@{}", self.critical));
        }
        if self.fumble != 17 {
            ret.push_str(&format!("#{}", self.fumble));
        }
        if let Some(target) = self.target {
            ret.push_str(&format!("<={target}"));
        }

        ret
    }

    /// Ruby `FW#result`。
    fn result(&self, total: i64) -> EvalResult {
        if total <= self.critical {
            EvalResult::critical("クリティカル！")
        } else if total >= self.fumble {
            EvalResult::fumble("ファンブル！")
        } else if let Some(target) = self.target {
            let success = target - total;
            if total <= target {
                EvalResult::success(format!("成功(成功度:{success})"))
            } else {
                EvalResult::failure(format!("失敗(失敗度:{success})"))
            }
        } else {
            EvalResult::new()
        }
    }
}

// ---------------------------------------------------------------------------
// ジャンプする項目を含む表（くじ・マジカルクッキング）
// ---------------------------------------------------------------------------

/// Ruby の `DiceTable::RangeTable`（1D6）の項目。文字列か、別の表へのジャンプ（`lambda`）。
enum JumpItem {
    /// 結果の文章。
    Text(&'static str),
    /// 次の繰り返しで参照する表。
    Jump(&'static JumpTable),
}

/// Ruby の `DiceTable::RangeTable.new(name, "1D6", items)` のうち、
/// 項目に `lambda { TABLES[n] }` を含みうるもの。
///
/// 表の名前は出力に使われない（`roll_jump_table` に渡された名前が使われる）ので持たない。
struct JumpTable {
    items: &'static [(RangeInc, JumpItem)],
}

impl JumpTable {
    const fn new(items: &'static [(RangeInc, JumpItem)]) -> Self {
        Self { items }
    }

    /// Ruby `RangeTable#roll(randomizer)`（1D6）。戻り値は `(出目, 項目)`。
    ///
    /// Ruby は構築時に 1〜6 が隙間なく覆われていることを検査するので、
    /// 該当する項目が無いことはない。
    fn roll(&self, rng: &mut Randomizer) -> Result<(i64, &'static JumpItem), EvalError> {
        let value = rng.roll_once(6)?;
        let item = self
            .items
            .iter()
            .find(|(range, _)| range.includes(value))
            .map(|(_, item)| item)
            .ok_or(EvalError::Internal("FilledWith: JumpTable has a gap"))?;
        Ok((value, item))
    }
}

/// Ruby `FilledWith#roll_jump_table(table_name, table)`。
///
/// ジャンプ先の表を引き続け、出目を連結した文字列（`values.join`）と結果の文章を返す。
fn roll_jump_table(
    table_name: &str,
    table: &'static JumpTable,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let mut table = table;
    let mut values = String::new();

    loop {
        let (value, item) = table.roll(rng)?;
        values.push_str(&value.to_string());

        match item {
            JumpItem::Text(content) => {
                return Ok(EvalResult::with_text(format_table_roll_result(
                    table_name, &values, content,
                )));
            }
            // 次の繰り返しで指定された表を参照する
            JumpItem::Jump(next) => table = next,
        }
    }
}

/// Ruby `FilledWith#format_table_roll_result`: 表を振った結果を独自の書式で整形する。
fn format_table_roll_result(table_name: &str, number: &str, result: &str) -> String {
    format!("{table_name}({number}):{result}")
}

// ---------------------------------------------------------------------------
// 夢幻の迷宮
// ---------------------------------------------------------------------------

/// Ruby `FilledWith::Difficulty`（難易度）。
struct Difficulty {
    /// Ruby `#index`: `DIFFICULTYS`（E/N/H/L/X）の中の位置。
    index: usize,
    /// Ruby `#name`。
    name: &'static str,
}

impl Difficulty {
    /// Ruby `Difficulty.new(sign)`。`[ENHLX]` 以外は `None`。
    fn from_sign(sign: &str) -> Option<Self> {
        let (index, name) = match sign {
            "E" => (0, "初級"),
            "N" => (1, "中級"),
            "H" => (2, "上級"),
            "L" => (3, "悪夢"),
            "X" => (4, "伝説"),
            _ => return None,
        };
        Some(Self { index, name })
    }
}

/// Ruby `FilledWith::Row`（難易度ごとに埋め込む値を持つ行）。
struct Row {
    /// Ruby `@body`: `%s` を含む書式文字列。
    body: &'static str,
    /// Ruby `@args`: `%s` ごとの、難易度順（E/N/H/L/X）の値。
    args: &'static [&'static [&'static str]],
}

impl Row {
    const fn new(body: &'static str, args: &'static [&'static [&'static str]]) -> Self {
        Self { body, args }
    }

    /// Ruby `Row#format(difficulty)`: `Kernel.format(@body, *args)`。
    ///
    /// 本文に含まれる書式指定は `%s` だけなので、順に置き換える。
    fn format(&self, difficulty: &Difficulty) -> String {
        let mut out = String::new();
        let mut parts = self.body.split("%s");
        if let Some(first) = parts.next() {
            out.push_str(first);
        }
        for (i, part) in parts.enumerate() {
            if let Some(arg) = self.args.get(i) {
                // Ruby: e[difficulty.index]（範囲外の nil は `%s` で空文字列になる）
                out.push_str(arg.get(difficulty.index).copied().unwrap_or(""));
            }
            out.push_str(part);
        }
        out
    }
}

/// Ruby `FilledWith#roll_trap_table`（夢幻の迷宮トラップ表 `TRAPd`）。
fn roll_trap_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^TRAP([ENHLX])$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };
    let Some(difficulty) = Difficulty::from_sign(&m[1]) else {
        return Ok(None);
    };

    let number = rng.roll_sum(3, 6)?;
    // Ruby: TRAP_TABLE[number - 3]（3D6 なので 3〜18 に収まる）
    let chosen = usize::try_from(number - 3)
        .ok()
        .and_then(|i| TRAP_TABLE.get(i))
        .ok_or(EvalError::Internal("FilledWith: TRAP_TABLE row is missing"))?;

    Ok(Some(format!(
        "トラップ表<{}>({number}):{}",
        difficulty.name,
        chosen.format(&difficulty)
    )))
}

/// Ruby `FilledWith::D66Table`（迷宮追加オプション表用のD66表）。
struct OptionTable {
    name: &'static str,
    rows: &'static [(i64, Row)],
}

impl OptionTable {
    const fn new(name: &'static str, rows: &'static [(i64, Row)]) -> Self {
        Self { name, rows }
    }

    /// Ruby `D66Table#roll(randomizer, difficality)`。
    fn roll(&self, rng: &mut Randomizer, difficulty: &Difficulty) -> Result<String, EvalError> {
        let value = rng.roll_d66(D66SortType::NoSort)?;
        let chosen = self
            .rows
            .iter()
            .find(|(key, _)| *key == value)
            .map(|(_, row)| row)
            .ok_or(EvalError::Internal(
                "FilledWith: OPTION_TABLE row is missing",
            ))?;

        Ok(format!(
            "{}<{}>({value}):{}",
            self.name,
            difficulty.name,
            chosen.format(difficulty)
        ))
    }
}

/// Ruby `FilledWith#roll_random_option_table`（夢幻の迷宮追加オプション表 `ROPd`）。
fn roll_random_option_table(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^ROP([ENHLX])$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };
    let Some(difficulty) = Difficulty::from_sign(&m[1]) else {
        return Ok(None);
    };

    Ok(Some(OPTION_TABLE.roll(rng, &difficulty)?))
}

/// Ruby `FilledWith::EventTable`（地形ごとのランダムイベント表）。
struct EventTable {
    area_name: &'static str,
    area_id: i64,
    rows: &'static [(i64, Row)],
}

impl EventTable {
    const fn new(area_name: &'static str, area_id: i64, rows: &'static [(i64, Row)]) -> Self {
        Self {
            area_name,
            area_id,
            rows,
        }
    }

    /// Ruby `EventTable#roll(randomizer, difficality, type:)`。
    ///
    /// `event_type` が指定されていれば十の位はそれに固定する（`RENC` は 4）。
    fn roll(
        &self,
        rng: &mut Randomizer,
        difficulty: &Difficulty,
        event_type: Option<i64>,
    ) -> Result<String, EvalError> {
        let tens = match event_type {
            Some(tens) => tens,
            None => rng.roll_once(6)?,
        };
        let ones = rng.roll_once(6)?;
        let value = tens * 10 + ones;

        let chosen = self
            .rows
            .iter()
            .find(|(key, _)| *key == value)
            .map(|(_, row)| row)
            .ok_or(EvalError::Internal(
                "FilledWith: EVENT_TABLES row is missing",
            ))?;

        Ok(format!(
            "ランダムイベント表({}{value}):{}<{}>:{}",
            self.area_id,
            self.area_name,
            difficulty.name,
            chosen.format(difficulty)
        ))
    }
}

/// Ruby `FilledWith#roll_random_event_table`
/// （夢幻の迷宮ランダムイベント表 `RANDda` / ランダムエンカウント表 `RENCda`）。
fn roll_random_event_table(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(RAND|RENC)([ENHLX])([1-6])?$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: type = m[1] == "RAND" ? nil : 4
    let event_type = if &m[1] == "RAND" { None } else { Some(4) };
    let Some(difficulty) = Difficulty::from_sign(&m[2]) else {
        return Ok(None);
    };
    // Ruby: area = m[3]&.to_i || @randomizer.roll_once(6)
    let area = match m.get(3) {
        Some(g) => to_i(g.as_str()),
        None => rng.roll_once(6)?,
    };

    // Ruby: EVENT_TABLES[area - 1]（地形は 1〜6）
    let table = usize::try_from(area - 1)
        .ok()
        .and_then(|i| EVENT_TABLES.get(i))
        .ok_or(EvalError::Internal(
            "FilledWith: EVENT_TABLES[area] is missing",
        ))?;

    Ok(Some(table.roll(rng, &difficulty, event_type)?))
}

/// Ruby `FilledWith#fetch_enemy_data`（夢幻の迷宮エネミーデータ表 `REDde`）。
fn fetch_enemy_data(command: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^RED([ENHLX])(256|265|465|665|666|[1-6]4[1-6])$").expect("valid regex")
    });
    let m = re.captures(command)?;

    let difficulty = Difficulty::from_sign(&m[1])?;
    let key = m.get(2)?.as_str();
    // Ruby: AREA_LIST[key[0].to_i - 1]（先頭の桁は 1〜6）
    let area_index = key.chars().next()?.to_digit(10)?.checked_sub(1)? as usize;
    let area_name = AREA_LIST.get(area_index).copied().unwrap_or("");

    // Ruby: ENEMY_DATA[key][difficulty.index]（正規表現が受け付ける鍵はすべて定義済み）
    let chosen = ENEMY_DATA
        .iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, data)| data.get(difficulty.index))
        .copied()?;

    Some(format!(
        "エネミーデータ表({key}):{area_name}<{}>:{chosen}",
        difficulty.name
    ))
}

/// Ruby `FilledWith#getTresureResult`（夢幻の迷宮財宝表 `TRSr±x`）。
fn get_tresure_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^TRS(\d+)([+-]\d)?$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: rank = m[1].to_i + m[2].to_i; rank = rank.clamp(0, 12)
    let rank = to_i(&m[1])
        .saturating_add(m.get(2).map_or(0, |g| to_i(g.as_str())))
        .clamp(0, 12);
    let table = &TRESURE_TABLES[rank as usize];

    Ok(Some(table.roll(rng)?.to_string()))
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/filled_with/*.rb と FilledWith.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `LOT_NORMAL_TABLES[1]`（ナンバーワンノーマルくじ（phase 1））。
static LOT_NORMAL_TABLE_1: JumpTable = JumpTable::new(&[
    (RangeInc::new(1, 3), JumpItem::Text("イレブンチキン")),
    (RangeInc::new(4, 5), JumpItem::Jump(&LOT_NORMAL_TABLE_2)),
    (RangeInc::single(6), JumpItem::Jump(&LOT_NORMAL_TABLE_3)),
]);

/// Ruby `LOT_NORMAL_TABLES[2]`（ナンバーワンノーマルくじ（phase 2））。
static LOT_NORMAL_TABLE_2: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("バロールたわし")),
    (RangeInc::single(2), JumpItem::Text("イグニスジッポ")),
    (
        RangeInc::single(3),
        JumpItem::Text("ヤコ仮面or梟の文鎮(選択可)"),
    ),
    (
        RangeInc::single(4),
        JumpItem::Text("ナレッジのハンモックorジンジャビースト"),
    ),
    (RangeInc::new(5, 6), JumpItem::Jump(&LOT_NORMAL_TABLE_3)),
]);

/// Ruby `LOT_NORMAL_TABLES[3]`（ナンバーワンノーマルくじ（phase 3））。
static LOT_NORMAL_TABLE_3: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("特性HPポーション")),
    (RangeInc::single(2), JumpItem::Text("特性MPポーション")),
    (RangeInc::single(3), JumpItem::Text("黒い甲冑")),
    (RangeInc::single(4), JumpItem::Text("天体望遠鏡")),
    (RangeInc::single(5), JumpItem::Text("金獅子の剥製")),
    (RangeInc::single(6), JumpItem::Jump(&LOT_NORMAL_TABLE_4)),
]);

/// Ruby `LOT_NORMAL_TABLES[4]`（ナンバーワンノーマルくじ（phase 4））。
static LOT_NORMAL_TABLE_4: JumpTable = JumpTable::new(&[
    (
        RangeInc::single(1),
        JumpItem::Text("特性スタミナポーション"),
    ),
    (RangeInc::single(2), JumpItem::Text("戦乙女の兜")),
    (RangeInc::single(3), JumpItem::Text("フェンリルの首輪")),
    (
        RangeInc::single(4),
        JumpItem::Text("フェニックスカーペット"),
    ),
    (RangeInc::single(5), JumpItem::Text("動くアダマンゴーレム")),
    (RangeInc::single(6), JumpItem::Jump(&LOT_NORMAL_TABLE_5)),
]);

/// Ruby `LOT_NORMAL_TABLES[5]`（ナンバーワンノーマルくじ（phase 5））。
static LOT_NORMAL_TABLE_5: JumpTable = JumpTable::new(&[
    (
        RangeInc::single(1),
        JumpItem::Text("キャンディークッション"),
    ),
    (RangeInc::single(2), JumpItem::Text("屑鉄の金床")),
    (RangeInc::single(3), JumpItem::Text("薪割り王の斧")),
    (RangeInc::single(4), JumpItem::Text("ロジエの水差し")),
    (RangeInc::single(5), JumpItem::Text("箱舟の模型")),
    (RangeInc::single(6), JumpItem::Jump(&LOT_PREMIUM_TABLE_5)),
]);

/// Ruby `LOT_PREMIUM_TABLES[1]`（ナンバーワンプレミアムくじ（phase 1））。
static LOT_PREMIUM_TABLE_1: JumpTable = JumpTable::new(&[
    (RangeInc::new(1, 3), JumpItem::Text("プレミアムチキン")),
    (RangeInc::single(4), JumpItem::Jump(&LOT_NORMAL_TABLE_3)),
    (RangeInc::new(5, 6), JumpItem::Jump(&LOT_PREMIUM_TABLE_2)),
]);

/// Ruby `LOT_PREMIUM_TABLES[2]`（ナンバーワンプレミアムくじ（phase 2））。
static LOT_PREMIUM_TABLE_2: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("親衛隊バッジ")),
    (RangeInc::single(2), JumpItem::Text("ハタモトチャブダイ")),
    (RangeInc::single(3), JumpItem::Text("星のコンパス")),
    (RangeInc::single(4), JumpItem::Text("白銀の甲冑")),
    (RangeInc::single(5), JumpItem::Jump(&LOT_NORMAL_TABLE_4)),
    (RangeInc::single(6), JumpItem::Jump(&LOT_PREMIUM_TABLE_3)),
]);

/// Ruby `LOT_PREMIUM_TABLES[3]`（ナンバーワンプレミアムくじ（phase 3））。
static LOT_PREMIUM_TABLE_3: JumpTable = JumpTable::new(&[
    (
        RangeInc::single(1),
        JumpItem::Text("特性クイックHPポーション"),
    ),
    (
        RangeInc::single(2),
        JumpItem::Text("特性クイックMPポーション"),
    ),
    (
        RangeInc::single(3),
        JumpItem::Text("特製クイックスタミナポーション"),
    ),
    (
        RangeInc::single(4),
        JumpItem::Text("火龍のフィギュアor氷龍のフィギュア(選択可)"),
    ),
    (RangeInc::single(5), JumpItem::Text("ヒメショーグンドレス")),
    (RangeInc::single(6), JumpItem::Jump(&LOT_PREMIUM_TABLE_4)),
]);

/// Ruby `LOT_PREMIUM_TABLES[4]`（ナンバーワンプレミアムくじ（phase 4））。
static LOT_PREMIUM_TABLE_4: JumpTable = JumpTable::new(&[
    (
        RangeInc::single(1),
        JumpItem::Text("クイックユグドラポーション"),
    ),
    (
        RangeInc::single(2),
        JumpItem::Text("銀河龍のフィギュア/ドラゴン"),
    ),
    (
        RangeInc::single(3),
        JumpItem::Text("銀河龍のフィギュア/魔族"),
    ),
    (RangeInc::single(4), JumpItem::Text("魔族チェスセット")),
    (RangeInc::single(5), JumpItem::Text("イグニスコンロ")),
    (RangeInc::single(6), JumpItem::Jump(&LOT_PREMIUM_TABLE_5)),
]);

/// Ruby `LOT_PREMIUM_TABLES[5]`（ナンバーワンプレミアムくじ（phase 5））。
static LOT_PREMIUM_TABLE_5: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("グレヴディバリウス")),
    (
        RangeInc::single(2),
        JumpItem::Text("天使の望遠鏡orデスの目覚まし時計(選択可)"),
    ),
    (RangeInc::single(3), JumpItem::Text("世界樹の蔦")),
    (RangeInc::single(4), JumpItem::Text("死神の飾りドレス")),
    (
        RangeInc::single(5),
        JumpItem::Text("ザバーニヤ等身大フィギュア"),
    ),
    (RangeInc::single(6), JumpItem::Jump(&LOT_PREMIUM_TABLE_6)),
]);

/// Ruby `LOT_PREMIUM_TABLES[6]`（ナンバーワンプレミアムくじ（phase 6））。
static LOT_PREMIUM_TABLE_6: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("イレブンチキン")),
    (
        RangeInc::single(2),
        JumpItem::Text("イレブンチキン(2ピース)"),
    ),
    (
        RangeInc::single(3),
        JumpItem::Text("イレブンチキン(3ピース)"),
    ),
    (
        RangeInc::single(4),
        JumpItem::Text("イレブンチキン(6ピース)"),
    ),
    (
        RangeInc::single(5),
        JumpItem::Text("イレブンチキン(12ピース)"),
    ),
    (RangeInc::single(6), JumpItem::Text("wish star")),
]);

/// Ruby `COOK_TABLES[1]`（マジカルクッキング（Lv 1））。
static COOK_TABLE_1: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("おべんとミートボール")),
    (RangeInc::single(2), JumpItem::Text("パリパリ小魚")),
    (RangeInc::single(3), JumpItem::Text("キャロットタルト")),
    (RangeInc::single(4), JumpItem::Text("おにぎり")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_2)),
]);

/// Ruby `COOK_TABLES[2]`（マジカルクッキング（Lv 2））。
static COOK_TABLE_2: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("カリカリミミズ肉")),
    (RangeInc::single(2), JumpItem::Text("竹つきチクワ")),
    (RangeInc::single(3), JumpItem::Text("トロピカルジュース")),
    (RangeInc::single(4), JumpItem::Text("イナリ寿司")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_3)),
]);

/// Ruby `COOK_TABLES[3]`（マジカルクッキング（Lv 3））。
static COOK_TABLE_3: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("ホットミートパイ")),
    (RangeInc::single(2), JumpItem::Text("魔界魚の目玉")),
    (RangeInc::single(3), JumpItem::Text("パンプキンプリン")),
    (RangeInc::single(4), JumpItem::Text("スタミナ丼")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_4)),
]);

/// Ruby `COOK_TABLES[4]`（マジカルクッキング（Lv 4））。
static COOK_TABLE_4: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("ジャンボ串焼き")),
    (RangeInc::single(2), JumpItem::Text("シルヴァまっしぐら")),
    (RangeInc::single(3), JumpItem::Text("フラウアイスクリーム")),
    (RangeInc::single(4), JumpItem::Text("ピクニックランチ")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_5)),
]);

/// Ruby `COOK_TABLES[5]`（マジカルクッキング（Lv 5））。
static COOK_TABLE_5: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("グラント風香草焼き")),
    (RangeInc::single(2), JumpItem::Text("エドマエスシ")),
    (RangeInc::single(3), JumpItem::Text("スターフルーツパフェ")),
    (RangeInc::single(4), JumpItem::Text("具沢山本格カレー")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_6)),
]);

/// Ruby `COOK_TABLES[6]`（マジカルクッキング（Lv 6））。
static COOK_TABLE_6: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("ドラゴンステーキ")),
    (RangeInc::single(2), JumpItem::Text("刺身盛り合わせ")),
    (RangeInc::single(3), JumpItem::Text("エデンのアップルパイ")),
    (RangeInc::single(4), JumpItem::Text("フォートレス炒飯")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_7)),
]);

/// Ruby `COOK_TABLES[7]`（マジカルクッキング（Lv 7））。
static COOK_TABLE_7: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("マツザカスペシャル")),
    (RangeInc::single(2), JumpItem::Text("オオトロスシ")),
    (RangeInc::single(3), JumpItem::Text("スノーホワイトボンブ")),
    (RangeInc::single(4), JumpItem::Text("よもつへぐい")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_8)),
]);

/// Ruby `COOK_TABLES[8]`（マジカルクッキング（Lv 8））。
static COOK_TABLE_8: JumpTable = JumpTable::new(&[
    (RangeInc::single(1), JumpItem::Text("超特大マンガ肉")),
    (RangeInc::single(2), JumpItem::Text("特上うな丼")),
    (RangeInc::single(3), JumpItem::Text("魔将樹のかき氷")),
    (RangeInc::single(4), JumpItem::Text("ヘブンズランチ")),
    (RangeInc::new(5, 6), JumpItem::Jump(&COOK_TABLE_9)),
]);

/// Ruby `COOK_TABLES[9]`（マジカルクッキング（Lv 9））。
static COOK_TABLE_9: JumpTable = JumpTable::new(&[
    (RangeInc::new(1, 3), JumpItem::Text("世界樹のサラダ")),
    (RangeInc::new(4, 6), JumpItem::Text("黄金のラダマン鍋")),
]);

/// Ruby `COOK_TABLES`（クッキングレベル → 表）。
static COOK_TABLES: &[(i64, &JumpTable)] = &[
    (1, &COOK_TABLE_1),
    (2, &COOK_TABLE_2),
    (3, &COOK_TABLE_3),
    (4, &COOK_TABLE_4),
    (5, &COOK_TABLE_5),
    (6, &COOK_TABLE_6),
    (7, &COOK_TABLE_7),
    (8, &COOK_TABLE_8),
    (9, &COOK_TABLE_9),
];

/// Ruby `TABLES["HST"]`（【必殺技！】）。
static TABLE_HST: Table = Table::from_dice(
    "【必殺技！】",
    1,
    6,
    &[
        "〔命中〕判定に出目[1,1,1]でクリティカル。更に致傷力に「SLv×20」のボーナスを得る。",
        "〔命中〕判定と致傷力に「SLv×10」のボーナスを得る。",
        "致傷力に「SLv×10」のボーナスを得る。",
        "攻撃が命中するとバッドステータス「転倒」を与える。",
        "通常攻撃。",
        "〔命中〕判定に[6,6,6]でファンブル。更に、使用者がバッドステータス「転倒」を受ける。",
    ],
);

/// Ruby `TABLES`。
static TABLES: &[(&str, &dyn RollableTable)] = &[("HST", &TABLE_HST)];

/// Ruby `TRAP_TABLE`（3D6 の合計 3〜18 に対応する16行）。
static TRAP_TABLE: [Row; 16] = [
    Row::new("トライディザスター:宝箱から広範囲に火炎・冷気・電撃が放たれる罠。PC全員に「%s」の「火炎」「冷気」「電撃」属性ダメージ。", &[&["3D6+3", "3D6+50", "3D6+70", "3D6+100", "300"]]),
    Row::new("ペトロブラスター:広範囲に石化光線を放つ罠。PC全員[抵抗-%s]判定を行い、失敗したPCはBS「石化」を受ける。", &[&["2", "4", "6", "8", "10"]]),
    Row::new("クロスボウストリーム:宝箱から矢の嵐が放たれる罠。PC全員に「%s」の「刺突」属性ダメージ。「ドッジ-%s」で〔回避〕が可能。", &[&["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"], &["4", "6", "8", "10", "20"]]),
    Row::new("フォーチュンイーター:PC全員の幸運を食らい、Ftを%s点減少させる。Ftが0の場合「%s」点の防護点無視ダメージ。", &[&["1", "2", "3", "4", "5"], &["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"]]),
    Row::new("スロット:解除に失敗しても害はないが、スロットが揃うまで開かない宝箱。スロットを1回まわすには%sGPが必要。行動を消費して[感覚-%s]判定に成功すればスロットは揃う。有利な特異点「ビビット反射」があれば判定に+4のボーナス。", &[&["100", "300", "600", "1000", "10000"], &["4", "6", "8", "10", "15"]]),
    Row::new("テレポーター:PC全員(とエンカウントしているエネミー)を転送して道に迷わせる。「財宝ランク」が1段階減少する。", &[]),
    Row::new("アイスコフィン:宝箱を開けようとしたキャラクターを氷漬けにする罠。対象1体に「%s」の「冷気」属性ダメージ。更にFPにも%s点の防護点無視ダメージ。", &[&["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"], &["5", "10", "15", "20", "30"]]),
    Row::new("クロスボウ:宝箱を開けようとしたキャラクターに強力な矢が放たれる罠。対象1体に「%s」の「刺突」属性ダメージ。「ドッジ-%s」", &[&["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"], &["4", "6", "8", "10", "20"]]),
    Row::new("毒針:宝箱を開けようとしたキャラクターに毒針を突き刺す罠対象1体に%s点の防護点無視ダメージ。更に[抵抗-%s]判定に失敗するとシナリオ終了まであらゆる判定に-2のペナルティ。", &[&["15", "30", "45", "60", "150"], &["4", "6", "8", "10", "15"]]),
    Row::new("アラーム:即座にその地形のエンカウント表を振って、それに対応したエネミーが出現する。出現したエネミーはそのターンから行動順に組み込まれる。出現するエネミー以外の記述は無視する。", &[]),
    Row::new("殺人鬼の斧:宝箱を開けようとしたキャラクターに斧が振り下ろされる罠。対象1体に「%s」の「打撃」「斬撃」属性ダメージ。「ドッジ-%s」か「シールド-%s」で〔回避〕が可能。", &[&["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"], &["4", "6", "8", "10", "20"], &["4", "6", "8", "10", "-20"]]),
    Row::new("死神:宝箱を開けようとしたキャラクターに死神を取り憑かせる罠。4ラウンド目が終了するまであらゆる判定に-3のペナルティを受け、4ラウンド目の終了と同時に「%s」の防護点無視ダメージ。", &[&["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"]]),
    Row::new("幻の宝:宝箱に偽の財宝を入れ、本物の財宝を入手させない罠。トラップが発動すると価値の無い偽の宝物「幻の宝」を入手してしまう。「幻の宝」はアイテム欄を3つ占有し、シナリオ終了まで捨てられない。アイテム欄に空きがない場合は、何かを捨てて誰かが必ず持たなくてはならない。", &[]),
    Row::new("エクスプロージョン:宝箱が大爆発を起こし、中身を粉々にしてしまう罠。宝箱の中身は消滅する。PC全員に「%s」の「打撃」「火炎」属性ダメージ。", &[&["3D6+10", "3D6+30", "3D6+50", "3D6+80", "200"]]),
    Row::new("レインボーポイズン:宝箱から七色の毒ガスが放たれる罠。PC全員に「%s」の防護点無視ダメージ。更にシナリオ終了まであらゆる判定に-2のペナルティ。[抵抗-%s]判定に成功すれば無効。", &[&["3D6+10", "3D6+30", "3D6+50", "3D6+80", "200"], &["4", "6", "8", "10", "15"]]),
    Row::new("デスクラウド:宝箱から致死性の毒ガスを放つ罠。PC全員を即死させる。[抵抗-%s]判定に成功すれば無効。", &[&["2", "4", "6", "8", "12"]]),
];

/// Ruby `OPTION_TABLE` の行（D66 のキー順）。
static OPTION_ROWS: &[(i64, Row)] = &[
    (11, Row::new("黄金の迷宮(財宝ランク+2):全てが黄金で彩られた迷宮。財宝ランクが大きく上昇する。", &[])),
    (12, Row::new("密林の迷宮(財宝ランク+1):密林の中にひっそりとたたずむ迷宮。分類が「魔獣」「獣人」「霊獣」のエネミーが行うあらゆる判定に+2のボーナス。", &[])),
    (13, Row::new("カラクリの迷宮:複雑なカラクリが周囲で絶え間なく動いている迷宮。分類「ギア」のエネミーが行うあらゆる判定に+2のボーナス。クリア時に「アタッチメント割引券」を全員が%s枚獲得。", &[&["1", "2", "3", "5", "10"]])),
    (14, Row::new("フラウの舞踏会:あちこちに花畑のある迷宮。フラウが発生するランダムイベントが発生した際、「この迷宮を制覇して、私達が舞踏会を開けるようにしてね」とお願いされ、クリア時の報酬に%sが追加される。", &[&["「キノコの帽子」(装飾品)", "「猛毒の花」(装飾品)", "「フルブロウン」(鎧)", "「緊急召喚の宝珠」(装飾品)", "魔将樹の大剣（剣）"]])),
    (15, Row::new("アズマ風の迷宮:風流なアズマ風の迷宮。武器に「刀」を持つエネミーが行うあらゆる判定に+2のボーナス。クリア時に「アタッチメント割引券」を全員が%s枚獲得。", &[&["1", "2", "3", "5", "10"]])),
    (16, Row::new("枯れた泉の迷宮:「全地形1-1」の回復の泉が全て枯れており、回復効果を得ることができない。「山岳1-6」の貴重な水源や、「水辺1-6」の毒の泉などはそのまま存在する。", &[])),
    (21, Row::new("天空への道(財宝ランク+1):上へ上へと果てしなく登っていく迷宮。空気が薄くなって疲労しやすくなる。【特技】特技などによるFP消費が全て+3。", &[])),
    (22, Row::new("灼熱焦土の迷宮(財宝ランク+1):とてつもなく暑く、あちこちで炎が燃え盛る迷宮。エネミーが行う「火炎」属性を含む攻撃の致傷力に+%sのボーナス。", &[&["10", "20", "30", "50", "100"]])),
    (23, Row::new("灼熱焦土の迷宮(財宝ランク+1):とてつもなく寒く、気温が氷点下の迷宮。エネミーが行う「冷気」属性を含む攻撃の致傷力に+%sのボーナス。", &[&["10", "20", "30", "50", "100"]])),
    (24, Row::new("盗賊王の迷宮:迷宮内での罠や鍵を解除する[感覚]判定に-3のペナルティ。4ラウンドまでに出現した宝箱の「財宝ランク」+1。", &[])),
    (25, Row::new("ミミック狂暴化:「全地形2-5」のミミックの致傷力に+%sのボーナス。ミミックを見破った場合に得られるGPが%sGP増加する。", &[&["20", "30", "50", "80", "150"], &["500", "1000", "3000", "5000", "20000"]])),
    (26, Row::new("トレジャーイーター狂暴化:「全地形2-6」のトレジャーイーターを見破る[知力]判定に-3のペナルティ。4ラウンドまでに出現した宝箱の「財宝ランク」+1。", &[])),
    (31, Row::new("暗闇の迷宮:どこもかしこも真っ暗な迷宮。「猫の目」などがなければ視覚に関する[感覚]判定に-5のペナルティ。", &[])),
    (32, Row::new("騒音の迷宮:常に大音量で謎の音楽(BGM)が鳴っている迷宮。聴覚に関する[感覚]判定に-5のペナルティ。", &[])),
    (33, Row::new("未知の怪物の迷宮(財宝ランク+1):エネミーの姿がシルエットのみになる迷宮。エネミーのデータがいかなる手段でも判明させられなくなる。(通常通り〔HP〕〔FP〕〔先制〕は判明する)", &[])),
    (34, Row::new("氾濫中の迷宮:大雨が降っており、川などが氾濫している迷宮。水泳を行う際の[敏捷]判定に-5のペナルティ。「森林3-6」の山火事イベントの効果は無視できる。", &[])),
    (35, Row::new("間抜けの迷宮(財宝ランク+1):頭がおかしくなりそうな極彩色の迷宮。[知力][意志]判定に-2のペナルティ。[知力]や[意志]そのものが下がるわけではない。", &[])),
    (36, Row::new("瘴気の迷宮(財宝ランク+1):生命力を奪う紫の霧で満ちた迷宮。〔HP〕の最大値に-%sのペナルティ。", &[&["10", "20", "30", "40", "50"]])),
    (41, Row::new("加速する迷宮:狂ったように針の動く時計が多数された迷宮。「CT:安息の日」以外の【特技】が「CT:なし」になる。", &[])),
    (42, Row::new("停滞する迷宮(財宝ランク+1):動かない時計が多数設置された迷宮。「CT:安息の日」以外のCTの存在する【特技】が「CT:シナリオ終了」になる。この効果はシナリオ終了まで持続する。", &[])),
    (43, Row::new("猛毒の迷宮(財宝ランク+1):見るからに毒々しい紫色の沼があちこちにある迷宮。エネミーが行う、名称に「毒」もしくは「ポイズン」が入る【特技】や、名称に「毒」もしくは「ポイズン」が入るトラップの致傷力に+%sのボーナス。", &[&["10", "20", "40", "50", "100"]])),
    (44, Row::new("死の迷宮(財宝ランク+2):死の運命から逃れることのできない、血まみれの迷宮。「生命保険証」の効果が適用されない。", &[])),
    (45, Row::new("幸運の迷宮:何者かの加護を感じる迷宮。PC全員のFtの最大値と現在値に+1のボーナス。この効果はシナリオ終了まで持続する。", &[])),
    (46, Row::new("不運の迷宮:PC全員のFt最大値と現在値に-1のペナルティ。この効果はシナリオ終了まで持続する。", &[])),
    (51, Row::new("レアメタルの迷宮:非常にレアなエネミー「レアメタルキャンディー」「レアメタルクラウン」が生息している迷宮。キャンディークラウン(CL40)、ゴールデンクラウン(CL177)から獲得できる通常ドロップのGPが5倍になる。", &[])),
    (52, Row::new("魔力の泉:PCとエネミーの双方が、〔FP〕を消費せずに【魔法】を使用できるようになる。最終的な消費〔FP〕が最大〔FP〕より大きい【魔法】は使用できない。", &[])),
    (53, Row::new("ブルーの迷宮:陰鬱な気分になり、他のキャラクターと関わる気力を失う。PC全員が不利な特異点「嫌な奴」を1段階得る。", &[])),
    (54, Row::new("レッドの迷宮:なぜか興奮して非常に好戦的になる。PC全員が不利な特異点「脳みそ筋肉」を得る。交戦中に「1:回復系」のイベントが発生しても戦闘を終了させることができない。", &[])),
    (55, Row::new("ピンクの迷宮:なんだか身近な異性(同性も?)が気になって仕方なくなる。PC全員が不利な特異点「英雄色を好む」を得る。魔族も戦闘意欲を失い、「分類:魔族」のエネミーが出現するイベントは無視する。", &[])),
    (56, Row::new("ハズレの迷宮(財宝ランク-1):ツギハギだらけの壁などでできた、ハリボテのような貧相な迷宮。宝箱の中身もなんだか貧相になる。", &[])),
    (61, Row::new("ラダマンティスの迷宮(財宝ランク+2):第一魔将ラダマンティスの像が入口に設置された迷宮。全てのエネミーが行うあらゆる判定に+2のボーナス。また、「遺跡6-6」のイベントのダメージ+%s。", &[&["20", "40", "60", "80", "150"]])),
    (62, Row::new("グレイヴディガーの迷宮(財宝ランク+2):第二魔将グレイヴディガーの像が入口に設置された迷宮。「分類:アンデッド」のエネミーが行うあらゆる判定に+5のボーナス。", &[])),
    (63, Row::new("ハイペリオンの迷宮(財宝ランク+2):第三魔将ハイペリオンの像が入口に設置された迷宮。全てのエネミーが「ターン開始」時に〔HP〕を全回復する。", &[])),
    (64, Row::new("ムスペルニヴルの迷宮(財宝ランク+2):勇ましくも美しい女性の像が設置された迷宮。エネミーが行う「火炎」もしくは「冷気」属性を含む攻撃の致傷力に+%sのボーナス。", &[&["20", "40", "60", "80", "150"]])),
    (65, Row::new("ウェルスの迷宮:人懐っこそうなアズマ風の青年が設置された迷宮。シナリオ上で第五魔将の正体が明らかに鳴っている場合のみ、PC全員のFtの最大値と現在値に+5のボーナス。この効果はシナリオ終了まで持続する。", &[])),
    (66, Row::new("バロールの迷宮(財宝ランク+2):第六魔将バロールの像が入口に設置された迷宮。「分類:ギア」のエネミーが行うあらゆる判定に+5のボーナス。", &[])),
];

/// Ruby `OPTION_TABLE`。
static OPTION_TABLE: OptionTable = OptionTable::new("迷宮追加オプション表", OPTION_ROWS);

/// Ruby `EVENT_TABLES[0]`（洞窟）の行。
static EVENT_ROWS_1: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("淡い光を放つ鉱石に包まれた部屋に出る。Ftが%s点回復する。", &[&["1", "2", "3", "4", "1"]])),
    (13, Row::new("天井が開いていて植物の育っている洞窟内庭園。[知力-%s]判定に成功すると、力の沸く木の実を見つけてPC全員の〔HP〕〔FP〕を%s点回復できる。有効：「自然と生きる」【賢人の知恵】", &[&["3", "6", "9", "12", "15"], &["20", "30", "40", "50", "100"]])),
    (14, Row::new("「きのこっきのこっげんきでろ～♪」洞窟内で栽培できる特殊なキノコを栽培する、キノコのフラウに出会う。100GP支払うことでキノコを1つ譲ってもらうことができ、〔HP〕が全回復してシナリオ終了まで〔HP〕の最大値に+%sのボーナスを得る。", &[&["10", "15", "20", "25", "30"]])),
    (15, Row::new("ギア技術を用いた強力な懐中電灯のおいてある休憩室。迷宮を出るまでPC全員に有利な特異点「猫の目」があるものとして扱う。迷宮を出ると懐中電灯は寿命が来たのか壊れてしまう。", &[])),
    (16, Row::new("不思議な光を放つ水が湧いている。PCの〔HP〕〔FP〕のいずれかが減少している場合、このイベントは拒否できない。また全快でも不利な特異点「好奇心は猫を殺す」を持つPCは飲みにいこうとする。飲んだキャラクターは1D6を振り、以下の様な効果が発生する。\n----------\n1:〔HP〕〔FP〕と「★死亡」以外のBS全回復\n2:シナリオ終了まで不利な特異点が全て消滅する\n3:シナリオ終了まで「真実を知る者」「お金持ち」などのキャラクターの地位や立場に関連するものを除いた有利な特異点が全て消滅する\n4:〔HP〕が1点になってしまう。〔HP〕が0以下の場合は〔HP〕〔FP〕が全回復\n5:シナリオ終了まで余分な獣っぽい耳や尻尾が生える。既に生えているキャラクターはそれが大きくなる。聴覚に関する[感覚]判定+4。\n6:あいやー！それ呪いの泉アルよ！シナリオ終了まで性別が男女入れ替わる。ロールプレイが面白かった場合、シナリオ終了時に「アタッチメント割引券」を与えても良い", &[])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("頑丈な岩壁に阻まれた隠し財宝の部屋を発見する。岩壁は「打撃」属性で合計%s点のダメージを与えると破壊され、財宝ランク%sのアイテムを入手可能。岩壁はどのヘクスからでも隣接扱いで攻撃可能。", &[&["100", "200", "300", "500", "1000"], &["2", "4", "6", "8", "10"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("洞窟内で暗がりの中に近道を発見!有利な特異点「猫の目」を持つキャラクターがいればクリア時の財宝ランク+1。", &[])),
    (32, Row::new("洞窟内で近道を発見!だが狭くて通れるかどうか…?有利な特異点「軟体動物」を持つキャラクターがいればクリア時の財宝ランク+1。", &[])),
    (33, Row::new("6つの分かれ道だ、どちらに進めばいいのだろうか…?1D6を振って1～3が出ればクリア時の財宝ランク+1。4～6が出ればクリア時の財宝ランク-1。有効:「非凡な直感」", &[])),
    (34, Row::new("足場の悪い道だ。うまく降りられればいいが…。[敏捷-%s]判定に成功すればクリア時の財宝ランク+1。失敗すると判定を行ったキャラクターに[3D+20]の防護点無視ダメージ。有効:「軟体動物」", &[&["3", "6", "9", "12", "20"]])),
    (35, Row::new("大きな岩でふさがれた近道。力づくでどかせてやる！[体力-%s]判定に成功すれば売り味の財宝ランク+1。失敗すると判定を行ったキャラクターは〔FP〕を5点消費する。", &[&["2", "4", "6", "8", "15"]])),
    (36, Row::new("6つの分かれ道だ、どうやら正解の道以外はエネミーの巣だが…。1D6を振って1が出ればクリア時の財宝ランク+1。2～6が出れば即座に洞窟のエンカウントイベントが発生する!有効:「非凡な直感」", &[])),
    (41, Row::new("水あめ状のボディに王冠をのせた、珍しいエネミーだ!エネミーとエンカウント。", &[])),
    (42, Row::new("獣の皮をかぶった魔族が狩った獲物の取り分でもめているようだ。彼らは興奮して争っているが、PCたちを見つけると向きを変えて襲い掛かってくる!エネミーとエンカウント。「マジカルクッキング」で持ち歩くタイプのものを作っていた場合、それを渡すことで戦闘を回避することもできる。", &[])),
    (43, Row::new("岩だと思って通りすぎようとしたら魔族だった!?エネミーとエンカウント。相手は洞窟で巧妙に擬態しており、〔先制〕に+10のボーナスを得る。有利な特異点「虫の知らせ」があれば〔先制〕ボーナスを無効化できる。", &[])),
    (44, Row::new("洞窟でトロッコに乗ったら暴走するギア魔族に追いかけられた!エネミーとエンカウント。トロッコに乗って逃げつつの戦闘になり、2ラウンド以内に倒せなければ轢かれてPC全員が「%s」の防護点無視ダメージを受ける。また、1ラウンドに1人誰かが行動を消費してトロッコを運転する必要があり、これに失敗すると即座に轢かれてしまう。轢かれると戦闘は終了する。PCにケイヴウォーカーがいればトロッコの運転を「支援ターン」で行える。", &[&["3D6+30", "3D6+30", "3D6+30", "3D6+100", "300"]])),
    (45, Row::new("炎の燃え盛る洞窟だ!運の悪いことに魔獣までいるようだ…。イベント詳細決定時に敵味方全員が「3D6+%s」の「火炎」属性ダメージを受ける。さらにエネミーとエンカウント。", &[&["10", "20", "30", "50", "150"]])),
    (46, Row::new("冷気に包まれた洞窟だ。更に氷の魔族や魔獣まで棲息している!イベント詳細決定時に敵味方全員が「3D+%s」の「冷気」属性ダメージを受ける。さらにエネミーとエンカウント。", &[&["10", "20", "30", "50", "150"]])),
    (51, Row::new("突然の落盤がエネミーを押しつぶす!戦闘中の場合、ランダムにエネミー1体を即死させ、ドロップ品も入手可能。それ以外の場合は特に何も起きない。「分類:ネームド」「分類:魔将」のエネミーには「3D+50」の防護点無視ダメージ。", &[])),
    (52, Row::new("採掘用のダイナマイトが打ち捨てられている。ケイヴウォーカー用の消耗品%sを持てるだけ持つことができる。", &[&["「ダイナマイト」", "「特性ダイナマイト」2個と「ダイナマイト」", "「特性ダイナマイト」4個と「ダイナマイト」", "「特性ダイナマイト」", "「ボンバーキング」(「電子の国のアリス」適用時）もしくは「特性ダイナマイト」（非適用時）"]])),
    (53, Row::new("洞窟内に有毒ガスが発生している!敵味方全員[抵抗-%s]で判定し、失敗したキャラクターは%s点の防護点無視ダメージを受ける。有効:【ガシンショウタン】", &[&["4", "6", "8", "10", "15"], &["15", "30", "45", "60", "100"]])),
    (54, Row::new("溶岩の流れる部屋だ、早くおさらばしたいところだが…。[敏捷-%s]判定に成功すれば何事も無く渡り切ることができる。失敗するとPC全員に「3D6+%s」の「火炎」属性ダメージ。PCにドラコニアンがいれば判定不要。有効:【野伏の歩み】", &[&["3", "6", "9", "12", "20"], &["20", "40", "60", "90", "200"]])),
    (55, Row::new("突然の落盤がPCのうちランダムに1人を押しつぶす!「3D6+%s」の防護点無視ダメージを受ける。「ドッジ-%s」で回避可能。有利な特異点「猫の目」か「虫の知らせ」があれば判定不要。", &[&["40", "60", "80", "120", "400"], &["4", "6", "8", "10", "20"]])),
    (56, Row::new("突然の地震が起きて洞窟全体が崩落を始める!このイベントが発生すると戦闘は強制終了する。PC全員[敏捷%s]で判定し、失敗したキャラクターは〔HP〕が0になる。〔HP〕が0以下だった場合は死亡してしまう。全員が判定に成功した場合、クリア時の財宝ランク+1。", &[&["", "-1", "-2", "-4", "-8"]])),
    (61, Row::new("プラネタリウムのように鉱石が輝く部屋。星の配置に関する謎かけになっている。行動を消費して[知力-%s]判定に成功するとこの謎を解くことができる。謎が解けると%sを入手可能。有効:【賢人の知恵】", &[&["6", "9", "12", "15", "30"], &["「うぃっしゅすたー」(大事なもの)か、10000GPの好きな方", "「うぃっしゅすたー」(大事なもの)2個か、20000GPの好きな方", "「うぃっしゅすたー」(大事なもの)3個か、30000GPの好きな方", "「うぃっしゅすたー」(大事なもの)5個か、50000GPの好きな方", "「wish star」（消耗品）"]])),
    (62, Row::new("地熱で温泉が湧いている!みんなで一休みできそうだ。このイベントが発生すると戦闘は強制終了する。PC全員の〔HP〕〔FP〕[Ft]が全回復する。また、シナリオ終了まで「ターン開始」時に〔HP〕が%s点ずつ回復する。", &[&["5", "10", "15", "20", "30"]])),
    (63, Row::new("地下渓流を発見する。どうやらここを進んでいくしかないようだ…。このラウンドではPCが「水中」にいるものとする。また、[敏捷-%s]判定に成功すると水中に沈んだ宝箱を発見する。中身は「%s」で、カギやトラップはかかっていない。行動を消費することで開けることができる。有効:【野伏の歩み】", &[&["6", "9", "12", "15", "25"], &["3D6x100GP", "(3D6+10)x100GP", "(3D6+30)x100GP", "3D6x1000GP", "3D6x10000GP"]])),
    (64, Row::new("とてつもない量のアダマン合金の原石を発見する!持ち帰り放題だ!「アダマン原石」は1つ%sGPで売却可能で、行動を消費することで自分のアイテム欄に好きなだけ入手可能。", &[&["300", "500", "1000", "2000", "5000"]])),
    (65, Row::new("地面から強力な重力で引きつけられる!敵味方全員が〔移動〕に-5のペナルティを受ける。さらに洞窟のエンカウントイベントが発生。「4-5」と「4-6」の部屋のダメージは適用しない。", &[])),
    (66, Row::new("怪しい光を放つ鉱石の部屋。鉱石は不気味な振動を放っており、ラウンド終了時にランダムなPC1人の武器が1つ壊れてシナリオ終了まで使用できなくなる。鉱石は「打撃」属性で合計%s点のダメージを与えると破壊される。鉱石はどのヘクスからでも隣接扱いで攻撃可能。", &[&["100", "200", "300", "500", "1000"]])),
];

/// Ruby `EVENT_TABLES[1]`（遺跡）の行。
static EVENT_ROWS_2: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("随分進んだし一休みしよう。ラウンド数が奇数の場合は〔FP〕が「%s」回復する。ラウンド数が偶数の場合は進んだように見せかけて迷っているだけだった。どっと疲れてしまい〔FP〕は回復せず、クリア時の財宝ランク-1。有利な特異点「精緻な記憶」があれば、偶数ラウンドでも〔FP〕が回復して財宝ランクも減少しない。", &[&["3D6", "3D6+5", "3D6+10", "3D6+15", "3D6+30"]])),
    (13, Row::new("どうやらここは古代の医務室らしい。なにやら怪しげな機械がたくさんあるが…。[知力-%s]判定に成功すればPC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。有効：【魔導工学のススメ】", &[&["3", "6", "9", "12", "18"]])),
    (14, Row::new("「きみたちの道に光あれ…お金？いらないよ」道標の光ゴケを栽培している、苔のフラウに出会う。以後「いくつかの道から正解を選ぶ」イベントが発生した場合、光ゴケがそのうちの1つに飛んでいき、必ず正解の道を選ぶことができる。次が最終イベントだった場合、クリア時の財宝ランク+1。", &[])),
    (15, Row::new("ダンジョンの見取り図を書いた部屋に出る。次のランダムイベントでは、好きな地形を選択してからイベント表を振ることができる。次が最終イベントだった場合、クリア時の財宝ランク+1。", &[])),
    (16, Row::new("「パワーアップ装置!」と書かれた怪しげな機械が置いてある。%sGPを支払って使用するとキャラクターを強化(?)可能。使用するかは自由だが、不利な特異点「好奇心は猫を殺す」を持っているキャラクターはGPがあるなら入ってしまう。使用したキャラクターは1D6を振り、以下のような効果が発生する。\n----------\n1:なんと、ランダムな[主能力]が1点上昇する!（永久）\n2:ただの日焼けサロンだ!健康的に焼け、シナリオ終了まで「火炎」に対する防護点+10。\n3:CLが1上昇する!やったね!【特技】はシナリオが終了してから習得する。\n4:装置は突然爆発する!ダメージは受けないが髪型がアフロに。装置は壊れる。\n5:巨大化してシナリオ終了まで〔HP〕の最大値が+50!〔HP〕〔FP〕も全回復。装置は壊れる。\n6:故障のせいかランダムな[主能力]が1点下降する…大ショック…（永久）\n\n[主能力]変動時は1D6を振ってどの能力が変動するか決定する。\n1:体力　2:敏捷　3:感覚　4:知力　5:意志　6:使用者が選ぶ", &[&["3000", "5000", "10000", "20000", "30000"]])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("部屋で脱出ゲームのような様々な仕掛けをとかないと開かない宝箱。行動を消費して[知力-%s]判定に成功すれば開けることができ、財宝ランク%sのアイテムを入手可能。判定に失敗すると宝箱は爆発してしまい、中身も失われる。有効:【盗賊の心得】", &[&["6", "9", "12", "15", "25"], &["2", "4", "6", "8", "10"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("鏡に映る怪人が自分の真似をしろとジェスチャーしている。[敏捷-%s]判定に成功するとうまく真似してパントマイムすることができ、鏡が消滅して隠し通路が開かれる。クリア時の財宝ランク+1。【華劇:○○】を習得したフラウがいれば判定不要。", &[&["6", "8", "10", "12", "16"]])),
    (32, Row::new("小さな魔将の像がフィルトウィズの地図上で乱雑に置かれている。[知力-%s]判定に成功すれば魔将の所在地を正しく配置することができ、隠し通路が開かれる。クリア時の財宝ランク+1。有効:【賢人の知恵】", &[&["6", "9", "12", "15", "20"]])),
    (33, Row::new("シュートの罠に引っかかるが、結果的に新たな道を発見できた!このイベントが発生すると戦闘は強制終了する。PC全員が「3D+10」の「打撃」属性ダメージを受けるが、クリア時の財宝ランク+1。", &[])),
    (34, Row::new("ラダマンティス崇拝者が創りだした巨大な甲羅のようなものが置かれた部屋。登っていけば近道することができそうだが、これはかなり骨が折れそうだ…。行動を消費して[敏捷-%s]判定を行い、成功すればクリア時の財宝ランク+1。判定に挑戦したキャラクターは次のターンまで〔回避〕判定を行えない。有効:「軟体動物」【野伏の歩み】", &[&["6", "9", "12", "15", "20"]])),
    (35, Row::new("後ろから重い物が転がってくる音がする。ローリングストーンだ!このイベントが発生すると戦闘は強制終了する。ラウンド終了時に全員の〔移動〕が%s以上であればクリア時の財宝ランク+1。〔移動〕が%s未満のPCは「%s」の防護点無視ダメージを受ける。", &[&["6", "8", "10", "12"], &["6", "8", "10", "12", "15"], &["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"]])),
    (36, Row::new("通路が複雑に入り組んでおり道に迷いそうだ…。2D6を振り、ゾロ目ならクリア時の財宝ランク+1。それ以外の場合はクリア時の財宝ランク-1。有効:「非凡な直感」", &[])),
    (41, Row::new("絶体絶命、モンスターハウスだ!危機感を煽るBGMも流れてくる。これらのエネミーを全滅させた際、最後に倒したエネミーからは必ず「レア」を入手できる。イベント決定後に逃亡を宣言しても良い。その場合クリア時の財宝ランク-2。", &[])),
    (42, Row::new("石像が並んでいる通路があるが…なんと石像は警備装置だった!エネミーとエンカウント。奇襲攻撃のため、エネミーからの攻撃の〔回避〕に-4のペナルティを受ける。【魔導工学のススメ】があれば警戒していたため、このペナルティは受けない。", &[])),
    (43, Row::new("魔族が障害物に身を隠しながら狙撃してきた!エネミーとエンカウント。障害物のため、エネミーは近接攻撃に対する防護点に+30のボーナス。", &[])),
    (44, Row::new("「汝が戦う理由とは何か?答えい人間!」ブツゼンにモンスターが仁王立ちしており、問答を行ってくる。[意思-6]判定に成功すると戦闘を回避することができ、ドロップ品も入手可能。判定に失敗するか戦闘を望んだ場合、エネミーとエンカウント。有効:【詩人の交渉術】", &[])),
    (45, Row::new("進路上に鎖に繋がれたモンスターを発見する。どうやら普通のモンスターより凶暴なようだ、戦うべきか避けるべきか…エネミーとエンカウント。エネミーはあらゆる致傷力に+%sのボーナスを得る。戦闘を避けた場合クリア時の財宝ランク-1。", &[&["10", "20", "30", "40", "100"]])),
    (46, Row::new("暴れゴーレムが襲い掛かってくる!暴走しているせいか凄まじいスピードだ!エネミーとエンカウント。このエネミーは〔移動〕〔先制〕に+10のボーナスを得ており、全員が行動終了した後、ラウンド終了前にもう1度ターンがまわってくる。", &[])),
    (51, Row::new("一斉に周囲の壁の射出口が開き、無数の矢が放たれる!PC全員に「%s」の「刺突」属性ダメージ。「ドッジ-%s」で〔回避〕が可能。有利な特異点「虫の知らせ」があれば〔回避〕に+4のボーナスを得る。", &[&["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"], &["4", "6", "8", "10", "20"]])),
    (52, Row::new("突然部屋に白いガスが充満して睡魔が襲ってくる!PC全員が「☆眠り」を受ける。[抵抗-%s]判定で無効化。", &[&["2", "4", "6", "8", "12"]])),
    (53, Row::new("フロアイミテーターの罠だ!突然部屋の入口が閉まり、君たちを消化し始める!「斬撃」属性で合計%s点のダメージを与えると入り口が開き脱出できるが、それまでPC全員は「ターン開始」時に%s点の防護点無視ダメージを受ける。ラウンドごとの地形変化やイベントは行われず、最終イベントにも進めなくなる。入り口はどのヘクスからでも隣接扱いで攻撃可能。誰も「斬撃」属性の攻撃ができないなら君たちの冒険はここまでだ。", &[&["100", "200", "300", "500", "1000"], &["10", "20", "30", "40", "60"]])),
    (54, Row::new("休憩室のようだ。寝心地のよさそうなふかふかのベッドまである。このイベントが発生すると戦闘は強制終了する。PC全員が[意思-%s]判定を行い、失敗したPCは「☆眠り」を受ける。この「☆眠り」は誰かが行動を消費して叩き起こさない限り、シナリオ終了まで持続する。全員が判定に失敗した場合、君たちの冒険はここまでだ。もう目覚めることはないが、せめていい夢を見られるといいだろう。ホムンクルスは「居眠り禁止」を厳命されているため、「☆眠り」を受けない。", &[&["2", "4", "6", "8", "12"]])),
    (55, Row::new("「ラダマンティスの蛇」の像と目が合ってしまった。どうやらダンジョンの監視者に見つかってしまったらしい!以後探索中にエネミーと遭遇した場合、CLの最も高いエネミーが1体増加する。このイベントの効果は累積せず、「分類：ネームド」「分類：魔将」のエネミーの数は変化しない。", &[])),
    (56, Row::new("怪しげな卵が置かれた部屋。君たちが近寄ると卵が割れて…?1D6を振り、その出目によって中から様々なものが出現する。\n----------\n1～2:これは財宝だ!財宝ランク%sのアイテムが入手可能。\n3～4:卵はそのまま大爆発!PC全員に「%s」の「打撃」「火炎」属性ダメージ。\n5～6:卵からドラゴンが!エネミーとエンカウント。", &[&["2", "4", "6", "8", "10"], &["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"]])),
    (61, Row::new("突然ダンジョンの明かりが消えてほぼ真っ暗になってしまう!次のラウンドに入るまであらゆる判定に-6のペナルティを受ける。有利な特異点「猫の目」があればペナルティを受けない。また「洞窟1-5の懐中電灯」や「遺跡1-4の光ゴケ」などがあれば無効。", &[])),
    (62, Row::new("どうやら古代の訓練施設のようだ。「パンチングマシーン」と書かれたものがあり、行動を消費すれば100GPを支払って挑戦可能。「格闘」武器でのダメージ判定を1回行い、与えたダメージによってファンファーレとともに景品が出る。1回挑戦すると、ガタが来ていたのか壊れてしまう。\n----------\nダメージ%s以上:財宝ランク%sのアイテム\nダメージ%s以上:財宝ランク%sのアイテム\nダメージ%s以上:財宝ランク%sのアイテム", &[&["20", "40", "60", "80", "200"], &["0", "2", "4", "6", "8"], &["30", "50", "70", "90", "400"], &["1", "3", "5", "7", "9"], &["40", "60", "80", "100", "600"], &["2", "4", "6", "8", "10"]])),
    (63, Row::new("謎の装置のせいで気になるあの子と体が入れ替わってしまった!?シナリオ終了までPCのうち2人の体や能力が入れ替わってしまう。入れ替わるキャラクターはランダムで決定するか、面白くなるようにGMが選ぶ。戦闘で扱うデータは今までどおりだが、入れ替わったキャラクターのロールプレイを行うこと。", &[])),
    (64, Row::new("やった、きんきらきんの宝物庫だ!「2-1」「2-2」「2-3」の宝箱が3つ同時に出現する。ただし、「2-3」の罠の解除に失敗すると、宝箱を開けようとしたキャラクターは黄金像になってしまう。めでたく財宝の仲間入りだ。「☆石化」と同様に扱うが、シナリオ終了までいかなる手段でも治せない。", &[])),
    (65, Row::new("キャンディークラウンハウスだ!エネミーとエンカウント。4ラウンド終了までに1体も倒せなかった場合、チャンスを逃した悔しさからPC全員が〔FP〕に「逃げたキャンディークラウンの数x5」の防護点無視ダメージを受ける。", &[])),
    (66, Row::new("なぜここに呼ばれたかお分かりになりますでしょうか?ラダマンティス崇拝者の作りだした審判の部屋に飛ばされてしまった!ラウンド終了時に部屋の中央のラダマンティス像から裁きの光が放たれ、PC全員が「%s」の防護点無視ダメージを受ける。%s点のダメージを与えれば破壊可能でダメージも受けない。ラダマンティス像はどのヘクスからでも隣接扱いで攻撃可能。破壊に成功すると財宝ランク%sのアイテムが入手可能。", &[&["3D6+50", "3D6+80", "3D6+110", "3D6+150", "400"], &["200", "400", "600", "1000", "2000"], &["3", "5", "7", "9", "11"]])),
];

/// Ruby `EVENT_TABLES[2]`（山岳）の行。
static EVENT_ROWS_3: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("休憩に丁度いい広場を見つけた。〔FP〕が3D6%s点回復する!だが、居心地が良すぎて離れづらくなってしまう。PC全員が[意思-%s]判定を行う。PCのうち半分以上が失敗した場合、次のラウンドもこのイベントが発生して、クリア時の財宝ランク-1。", &[&["", "+5", "+10", "+15", "+30"], &["2", "3", "4", "6", "9"]])),
    (13, Row::new("山小屋を発見し一休み。〔HP〕と〔FP〕が3D6%s点回復する。PCが4人の時にこのイベントが発生した場合、休憩中の暇つぶしに山小屋の四隅をぐるぐるまわって運動し始める。だが、後から妙なことに気が付きFtが1点減少する…", &[&["", "+5", "+10", "+15", "+30"]])),
    (14, Row::new("「へーっ、こんなとこまでよく来たね!これ、食べる?食べる?」キイチゴを栽培しているフラウに出会う。栄養満点で幸せになる。PC全員の〔HP〕と〔FP〕を10点、Ftを1点回復することができる。さらに、【魔法の料理人】を持つPCがいれば、消耗品「いちごジャム」をSLv個入手可能。\n----------\n名称:いちごジャム\n価格:非売品\n解説:〔HP〕と〔FP〕を10点、Ftを1点回復する。戦闘中には使用できない。", &[])),
    (15, Row::new("少し休憩できそうな洞窟を発見する。PC全員の〔FP〕を10点回復する。中には他のエクスプローラーが残していったと思われる道具もあった。「%s」を入手することができる。大雨が降っていたり、火災が起こったりしていた場合、その効果も消滅する。", &[&["高級HPポーション", "最高級HPポーション", "特性HPポーション", "うぃっしゅすたー", "うぃっしゅすたーx3"]])),
    (16, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。%s「★死亡」以外のあらゆるBSも回復する。しかし山間の貴重な水源のため亜人によって管理されており、使用するには%sGPが必要。PCのパーティーに管理している亜人と同じ種族がいるか、【詩人の交渉術%s】を持つキャラクターがいれば無料で使用可能。どの種族が管理しているかは1D6を振って決定する。\n----------\n1:ドラコニアン\n2:リザードとワイズマン\n3:フラウ\n4:シルヴァテイル\n5:ストームコーザー\n6:グラント", &[&["", "", "", "", "また、フォーチュンも最大値を越えて1増加する。"], &["500", "1000", "2000", "4000", "30000"], &["1", "2", "3", "4", "5"]])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("狭い谷底に宝箱があるのが見える。そして手元には岩とロープが。これはもうバンジージャンプで取ってくるしかない!行動を消費して[敏捷-%s][意思-%s]で判定を行い、両方に成功すれば財宝ランク%sのアイテムを入手できる。[敏捷]判定がファンブルだった場合ロープがちぎれてしまい、4ラウンド目が終了するまで戻ってこれない。有効:【野伏の歩み】([敏捷]判定のみ)", &[&["3", "6", "9", "12", "16"], &["3", "5", "7", "9", "12"], &["3", "5", "7", "9", "11"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("山歩きの達人だけが発見できるショートカットコース。%sを持つキャラクターがいればクリア時の財宝ランク+1。", &[&["有利な特異点「鷹の目」か【野伏の歩み】", "有利な特異点「鷹の目」２段階か【野伏の歩み2】", "有利な特異点「鷹の目」２段階か【野伏の歩み3】", "有利な特異点「鷹の目」２段階か【野伏の歩み4】", "有利な特異点「鷹の目」必須かつ【野伏の歩み4】"]])),
    (32, Row::new("鉱物を運んでいたのであろう、運搬用トロッコを発見する。うまく運転できれば素早く進行できそうだが…?[敏捷-%s]判定に成功すればクリア時の財宝ランク+1。判定に失敗すると脱線して放り出されてしまい、PC全員が「3D+10」の防護点無視ダメージを受ける。PCにケイヴウォーカーがいれば判定不要。", &[&["3", "6", "9", "12", "16"]])),
    (33, Row::new("険しい斜面が続く。これは体力勝負になりそうだ…。PC全員が[体力%s]判定を行い、全員が成功すればクリア時の財宝ランク+1。判定に失敗したキャラクターは〔FP〕を%s点失う。", &[&["", "-1", "-2", "-4", "-8"], &["5", "10", "15", "20", "30"]])),
    (34, Row::new("空を飛べればショートカットできそうな崖を発見する。なんらかの飛行手段のあるPCがいる場合、クリア時の財宝ランク+1。元々飛行できなくてもラウンド中に【フライト】などの【特技】を使用してラウンド終了時に「飛行状態」であれば良い。", &[])),
    (35, Row::new("遠くから山彦のような不思議な声が聞こえる。山彦と会話をすると迷いやすい道を教えてくれるが、大声で会話をしているのでエネミーに見つかりやすくなる。会話をするなら次のイベントは必ず山岳のエンカウントイベントになり、会話を諦めるならクリア時の財宝ランク-1。有利な特異点「天体時計」があれば会話を諦めてもデメリットはない。", &[])),
    (36, Row::new("急に突風が吹いてきて、体力のないものは吹き飛ばされてしまう!もっとも[体力]の低いPC1人が[体力-%s]判定を行い、失敗すると「%s」の防護点無視ダメージを受ける。さらに、救助に時間がかかるためクリア時の財宝ランク-1。PCにストームコーザーがいればイベントの効果を無視できる(演出は自由)。", &[&["2", "3", "4", "6", "9"], &["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"]])),
    (41, Row::new("高いところから多数の岩がごろごろと…いや、これは魔物だ!エネミーとエンカウント。高低差で勢いがついているため、エネミーは〔移動〕に+5、%sのボーナスを得る", &[&["物理攻撃の致傷力に+10", "物理攻撃の致傷力に+30", "物理攻撃の致傷力に+60", "物理攻撃の致傷力に+120", "あらゆる攻撃の致傷力に+100"]])),
    (42, Row::new("空中から羽をもった魔物があちこちから攻撃を仕掛けてきた!エネミーとエンカウント。エネミーのうち%s体は任意の「PC配置ヘクス」に設置される。", &[&["3", "3", "1", "1", "3"]])),
    (43, Row::new("「た、隊長、もうダメです!」「バカもん!この程度で根を上げるな!」登山訓練中の魔族の小隊と出くわす。だが相手は疲れているようだ?エネミーとエンカウント。〔FP〕は最大値から10点減少している。", &[])),
    (44, Row::new("山道の横から突然穴が空き、魔物と鉢合わせしてしまう!エネミーとエンカウント。PC全員が[感覚-3]判定を行い、失敗したキャラクターはそのラウンド中に行動することができない。(〔回避〕や[抵抗]の判定、自分のターン以外で行う【特技】は可能)有効:「狐の耳」", &[])),
    (45, Row::new("ケタケタ笑いながら空から襲撃される。エネミーとエンカウント。彼女達はとても飽きっぽいので2ラウンド行動した後、3ラウンド目の開始前に立ち去ってしまう。(ドロップは入手できない)", &[])),
    (46, Row::new("フェニックスがつがいで暮らす縄張りに足を踏み入れてしまった!エネミーとエンカウント。すぐに引き返せば戦闘を回避できるが、クリア時の財宝ランク-1。そのまま進むのであれば[意思-6]で判定を行い、成功すれば見逃してもらうことができる。失敗するとフェニックスは怒り狂い、襲ってくる。有効:【詩人の交渉術】", &[])),
    (51, Row::new("ジャーンジャーン!げぇ、落石だ!!上から魔族が岩を落とし始める。PC全員に「%s」点の「打撃」属性ダメージ。「ドッジ-%s」で〔回避〕が可能。", &[&["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"], &["3", "5", "7", "9", "15"]])),
    (52, Row::new("ファイトー!いっぱーつ!!足場が崩れ出す。PC全員が[敏捷-%s]判定を行い、失敗すると「%s」点の防護点無視ダメージを受ける。成功したPCは[体力]判定を行い、失敗したPC1人を支えることができる。この判定に失敗した場合、支えようとしたキャラクターも一緒にダメージを受ける。有効:【野伏の歩み】", &[&["3", "6", "9", "12", "20"], &["3D6+20", "3D6+40", "3D6+60", "3D6+90", "200"]])),
    (53, Row::new("滑りやすい岩を飛び移っていく道が続いている。敵味方全員が[敏捷-%s]で判定を行い、失敗したキャラクターは「☆転倒」を受ける。飛行可能なキャラクターはこの影響を受けない。有効:【野伏の歩み】", &[&["2", "2", "4", "6", "10"]])),
    (54, Row::new("先に進む道に『銀糸の甲殻』アルゲントゥムの巣がびっしり張られている。幸い主はいないようだが、巣が邪魔で移動がしにくい…敵味方全員が4ラウンド目まで飛行状態になれなくなり、〔移動〕にも-5のペナルティを受ける。不利な特異点「トラウマ:虫」などがあればそれも適用する。", &[])),
    (55, Row::new("美しい万年雪の中を進んでいたと思ったのも束の間、雪崩に遭遇してしまう!全員が「%s」点の「打撃」「冷気」属性ダメージを受け、クリア時の財宝ランク-2。[感覚-%s]の判定に成功すれば、すぐに洞窟に避難してやり過ごすことができる。洞窟の中には%sがあり、入手可能。有効:「狐の耳」【野伏の歩み】", &[&["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"], &["6", "9", "12", "15", "20"], &["スノーの結晶(装飾品/3000GP)", "紺碧の瞳(装飾品/10000GP)", "氷狼の爪(装飾品/25000GP)", "氷河龍の盾(盾/65000GP)", "コキュートスダガー(ナイフ/110000GP)"]])),
    (56, Row::new("���から魔族の大群が登ってくる!ジャーンジャーン!石を落として撃退だ!魔族の群れは射撃攻撃で合計%s点のダメージを与えれば撃退可能。複数の対象を攻撃できる【特技】を使用した場合は、ダメージを2回計算可能。ラウンド終了時までに撃退できなかった場合、PC全員が撃破に足りなかったダメージに等しい防護点無視ダメージを受ける。魔族の群れはどのヘクスからでも攻撃可能だが、射程が10ヘクス以上の攻撃でなければならない。石を落としても攻撃可能で、「3D+%s」点の「打撃」属性ダメージを与える。この攻撃は判定なしで自動的に命中する。", &[&["100", "200", "300", "500", "1200"], &["10", "20", "30", "50", "100"]])),
    (61, Row::new("急に大雨が降り出した、屋内に避難したいところだが…地形が「洞窟」か「遺跡」に変更されるまで、ラウンド終了時、〔FP〕に「%s」点の防護点無視ダメージを受ける。〔FP〕に受けるダメージはPCがそれぞれダイスを振って決定する。「水中行動」が可能なPCはこの効果を受けない。", &[&["1D6", "2D6", "3D6", "3D6+5", "3D6+15"]])),
    (62, Row::new("色とりどりの石が岩肌に…これはカラーストーンの原石だ![知力-%s]判定に成功すればカラーストーン原石を持ち帰ることができる。「カラーストーン原石」は1つ%sGPで売却可能で、行動を消費することで自分のアイテム欄に好きなだけ入手可能。有効:【魔導工学のススメ】", &[&["6", "9", "12", "15", "20"], &["500", "1000", "2000", "3000", "5000"]])),
    (63, Row::new("不思議な岩がある。1D6を振って1～2であれば「火炎」属性、3～4であれば「冷気」属性、5～6であれば「電撃」属性の攻撃で、4ラウンド目終了までに%s点のダメージを与えれば破壊可能。不思議な岩はどのヘクスからでも隣接扱いで攻撃可能で、破壊したあとには財宝ランク%sのアイテムが発見できる。", &[&["30", "60", "100", "200", "500"], &["2", "4", "6", "8", "10"]])),
    (64, Row::new("「こんなところで人間に会うなんて珍しいな、お前らも食っていくか?」ストームコーザーが獲物を取った後調理している。このイベントが発生するとエネミーは消滅する。マジカルクッキングLv4「ジャンボ串焼き」を食べさせてもらえる。【魔法の料理人】を習得したPCがいれば、Lv5「グラント風香草焼き」にパワーアップする。この料理の効果は、自分たちが「マジカルクッキング」で作った料理と累積する。", &[])),
    (65, Row::new("高山植物の群生地を発見する。ポーションの材料になるものもありそうだ?[知力-%s]判定に成功すれば、薬草を持ち帰ることができる。「高山の薬草」は1つ%sGPで売却可能で、行動を消費することで自分のアイテム欄に好きなだけ入手可能。【ポーションナレッジ】があれば判定は不要。有効:【賢人の知恵】「自然と生きる」", &[&["6", "9", "12", "15", "20"], &["500", "1000", "2000", "3000", "5000"]])),
    (66, Row::new("岩壁が大きく崩れていく!崩落に巻き込まれて入口付近まで戻されてしまい、クリア時の財宝ランク-2。だが、崩落した後に鈍く輝くアイテムが転がっている…?[感覚-%s]判定に成功すれば%sを入手可能。有効:「鷹の目」", &[&["6", "9", "12", "15", "20"], &["アダマンソード(剣/8000GP)", "アダマンスケイル(鎧/15000GP)", "アダマンプレート(鎧/30000GP)", "ヒュージーブレード(剣/45000GP)", "封印の手錠(盾/200000GP)"]])),
];

/// Ruby `EVENT_TABLES[3]`（水辺）の行。
static EVENT_ROWS_4: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("甲羅干しに適した暖かい小島を見つける。少しだけ休憩しよう。〔HP〕と〔FP〕が「3D%s」点回復する。ワイズマンのPCは全回復する。何か面白い寝言を言ったPCに「%s」が空から降ってくる。", &[&["", "+5", "+10", "+15", "+30"], &["HPポーション", "高級HPポーション", "最高級HPポーション", "うぃっしゅすたー", "蘇生ポーション"]])),
    (13, Row::new("魚の群れを発見!捕まえてたらふくいただくとしよう!【魔法の料理人】があればマジカルクッキングの「魔界魚の目玉」(Lv3)%sの効果を得られる。この効果はすでにマジカルクッキングを使用していても累積する。", &[&["", "、【魔法の料理人2】があれば「シルヴァまっしぐら」(Lv4)と「エドマエスシ」(Lv5)の全て", "、【魔法の料理人2】があれば「シルヴァまっしぐら」(Lv4)と「エドマエスシ」(Lv5)、【魔法の料理人3】があれば「刺身盛り合わせ」(Lv6)と「オオトロスシ」(Lv7)の全て", "、【魔法の料理人2】があれば「シルヴァまっしぐら」(Lv4)と「エドマエスシ」(Lv5)、【魔法の料理人3】があれば「刺身盛り合わせ」(Lv6)と「オオトロスシ」(Lv7)、【魔法の料理人4】があれば「特上うな丼」(Lv8)の全て", "、【魔法の料理人2】があれば「シルヴァまっしぐら」(Lv4)と「エドマエスシ」(Lv5)、【魔法の料理人3】があれば「刺身盛り合わせ」(Lv6)と「オオトロスシ」(Lv7)、【魔法の料理人4】があれば「特上うな丼」(Lv8)、【魔法の料理人５】で全員のフォーチュン+1（最大値をこえる）の全て"]])),
    (14, Row::new("「こんにちは、スーっとして気持ちいいよ。…ふふ、一緒に入ろうか?」ミントや菖蒲を育てているフラウに出会い、薬湯に浸かって〔FP〕が全回復する。さらに、「火炎」属性に対する防護点に+10のボーナス。", &[])),
    (15, Row::new("リザードの集落につく。〔HP〕の減ったキャラクターがいれば持て囃され、マジカルクッキング「カリカリミミズ肉」(Lv2)をごちそうしてもらい、この先の道案内も買って出てくれる。クリア時の財宝ランク+1。全員の〔HP〕が満タンの場合は、そっけない扱いをされ何も起こらない。この効果はすでにマジカルクッキングを使用していても累積する。", &[])),
    (16, Row::new("回復の泉に見せかけた毒の泉。パーティーメンバーの〔HP〕〔FP〕のいずれかが減少している場合このイベントは拒否できない。毒の水で「%s」点の防護点無視ダメージを受ける。〔HP〕〔FP〕が全快のキャラクターと「虫の知らせ」があるPCはこの効果を受けない。[知力-6]判定に成功すればこれが毒の泉だとわかる。有効:【野伏の歩み】【賢人の知恵】", &[&["3D6+10", "3D6+20", "3D6+30", "3D6+50", "100"]])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("水中の奥深くに宝箱があるのが見える。とってくるのは無理そうに見えるが…行動を消費して[敏捷-%s]判定を行い、成功すれば財宝ランク%sのアイテムを入手できる。有利な特異点「水中行動」があれば判定不要。有効:【野伏の歩み】", &[&["9", "12", "15", "18", "25"], &["3", "5", "7", "9", "11"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D6+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("進みやすい浅瀬が続き、すいすいと進める。クリア時の財宝ランク+1。有利な特異点「水中行動」や【ミズグモ】をもつPCがいればクリア時の財宝ランク+2。ただしグラントがいると浅瀬にもかかわらず溺れてしまい、財宝ランクは変化しない。(RP次第でグラントがいるデメリットをなくしても良い)", &[])),
    (32, Row::new("様々な漂流物が流れこむ船の墓場だ。お宝もありそうだが探索は大変そうだ…〔FP〕を3点消費する代わりに水辺の財宝系イベント「2-1～2-6」が発生する。この効果を望まない場合は特に何も起きない。", &[])),
    (33, Row::new("ヒャッハー、ウォータースライダーだ!巨大な植物の幹を滑って進むことができるが、かなり怖いので勇気が必要。PC全員が[意思%s]判定を行い、全員成功すればクリア時の財宝ランク+1。この判定に成功したPCは、失敗したPCを1人まで抱えて滑り落ちることで成功扱いにできる。", &[&["", "-1", "-2", "-4", "-8"]])),
    (34, Row::new("大蓮のような植物の群生地を渡っていく。重たい装備を身に着けていると沈んでしまいそうだ…[必要体力]が15以上の装備を所有しているPCがいるとクリア時の財宝ランク-1。そのPCに有利な特異点「水中行動」や【ミズグモ】があればこのデメリットは受けない。", &[])),
    (35, Row::new("水流が複雑になっており、思うように進めず戻されてしまう。[体力-%s]判定に失敗するとクリア時の財宝ランク-1。", &[&["4", "5", "6", "8", "12"]])),
    (36, Row::new("突然の激流にイカダが巻き込まれる!身を任せている場合じゃない!敵味方全員が[敏捷-%s]判定を行い、失敗したキャラクターは4ラウンド終了まで帰ってこれなくなる。全員が失敗してしまった場合、冒険は強制的に失敗となりダンジョンの入口に戻されてしまう。ダンジョン内で手に入れたアイテムも消耗品とGP以外は全て水に流されて消滅する。有効:【野伏の歩み】", &[&["2", "3", "4", "6", "10"]])),
    (41, Row::new("蟹の魔物がイカダをちょんぎろうとしている!早く倒さなくては!エネミーとエンカウント。1ラウンド以内に倒せなかった場合、イカダをちょんぎられてクリア時の財宝ランク-1。", &[])),
    (42, Row::new("アシガル潜水隊と遭遇。エネミーとエンカウント。エネミーは奇襲攻撃を仕掛けてくるため〔先制〕に+5のボーナスを得る。%s", &[&["[感覚-6]判定に成功すると水面に動く竹筒を発見可能で、その場合は〔先制〕へのボーナスは無効化可能。有効:「鷹の目」", "[感覚-8]判定に成功すると水面に動く竹筒を発見可能で、その場合は〔先制〕へのボーナスは無効化可能。有効:「鷹の目」", "[感覚-10]定に成功すると水面に動く竹筒を発見可能で、その場合は〔先制〕へのボーナスは無効化可能。有効:「鷹の目」", "[感覚-12]判定に成功すると水面に動く竹筒を発見可能で、その場合は〔先制〕へのボーナスは無効化可能。有効:「鷹の目」", ""]])),
    (43, Row::new("何か水面が揺れている…まさか、あれはニンジャ!?エネミーとエンカウント。エネミーは謎の光学迷彩ギアで姿を隠しており、〔回避〕に+%sのボーナス。", &[&["2", "2", "4", "6", "2"]])),
    (44, Row::new("水辺で花を育てている少女に出会う…が、少女は魔族だった!エネミーとエンカウント。彼女が育てている花の根元には無数の動物の死体が無残にも肥料にされている。戦闘開始時に[意思]判定を行い、失敗したキャラクターは戦慄して1ターン行動不可。〔回避〕[抵抗]の判定を行うことはできる。", &[])),
    (45, Row::new("高波にのった魔族の騎士が現れる!エネミーとエンカウント。エネミーは波で勢いがついているため、最初のターンは〔移動〕〔先制〕に+5のボーナス。", &[])),
    (46, Row::new("湖の上を渡っていると思ったら巨大なスライムの上だった!?エネミーとエンカウント。エネミーは巨大なため〔HP〕に+%sのボーナスを得ている。また、全てのヘクスに対して隣接しているものとする。(移動妨害も全てのヘクスで受ける)", &[&["50", "100", "150", "250", "1500"]])),
    (51, Row::new("美しい歌声が響き渡る、意識が引きずり込まれそうだ。その場にいる敵味方全員は[抵抗-%s]判定を行い、失敗すると1ターン「☆眠り」を受ける。有効:【音響補正装置】", &[&["2", "4", "6", "8", "12"]])),
    (52, Row::new("小舟で進んでいると、なにやら激しい水の音がする。この先は滝のようだ!PC全員が[移動-%s]判定を行い(「水中行動」があれば+5のボーナスを得る)、成功したキャラクターがいれば事なきを得る。更に滝の裏の洞窟から財宝ランク%sのアイテムを入手可能。〔移動〕判定に全員失敗した場合、全員が滝から転落して「%s」の防護点無視ダメージを受ける。有効:【野伏の歩み】", &[&["3", "4", "5", "7", "12"], &["3", "5", "7", "9", "11"], &["3D6+20", "3D6+30", "3D6+40", "3D6+60", "150"]])),
    (53, Row::new("船の墓場を進んでいると、強風でマストが倒れてくる!PC全員が「ドッジ-%s」判定を行い、失敗すると「%s」の防護点無視ダメージを受け、「☆転倒」を受ける。", &[&["2", "4", "6", "8", "15"], &["3D6+10", "3D6+20", "3D6+30", "3D6+50", "150"]])),
    (54, Row::new("ぺっぺっ、水がしょっぱい!濃い塩水がギアをダメにしてしまう。所有している武器についたアタッチメント1つがシナリオ終了まで消滅してしまう。どのアタッチメントが消滅するかは選択可能。【魔導工学のススメ%s】があれば応急処置を行い、この効果を受けない。また、「★特別アタッチメント」は消滅しない。", &[&["1", "1", "2", "3", "4"]])),
    (55, Row::new("とても寒く冷たい水辺で、体力をどんどん奪われてしまう。「冷気」属性に対する防護点が%s以下のキャラクターは、ラウンド終了時に〔FP〕を%s点失う。", &[&["10", "15", "20", "25", "50"], &["5", "10", "15", "20", "30"]])),
    (56, Row::new("突然間欠泉が噴き出してダメージを受ける!敵味方全員のうちランダムで1体に「%s」点の「火炎」属性ダメージと「☆転倒」を与える。このイベントが2ラウンド目で発生した場合、4ラウンド目のイベントはダイスを振らず「洞窟6-2」の温泉イベントになる。", &[&["3D6+40", "3D6+60", "3D6+80", "3D6+120", "400"]])),
    (61, Row::new("傷ついた%sを発見する。ラウンド終了までに〔HP〕を%s点まで回復させればなついて%sを入手可能。回復できなかった場合、%sは逃走する。", &[&["スワンプランナー", "スカイサーチャー", "グリマルキン", "ゴールデンコーン", "プチブレイザー"], &["30", "50", "70", "100", "400"], &["スワンプランナー(ペット/4000GP)", "スカイサーチャー(ペット/6000GP)", "グリマルキン(ペット/15000GP)", "ゴールデンコーン(ペット/30000GP)", "プチブレイザー（ペット/100000GP）"], &["スワンプランナー", "スカイサーチャー", "グリマルキン", "ゴールデンコーン", "プチブレイザー"]])),
    (62, Row::new("なにやら派手な魚を見つける。あれはもしやアズマに伝わる幻の魚「ニシキゴイ」か!?行動を消費して[敏捷-%s]判定に成功すれば見事捕まえ、アイテム欄に入手することができる。売却すると%sGPになる。この判定は4ラウンド目が終了するまで何度でも行うことができる。有効:【野伏の歩み】", &[&["8", "10", "12", "15", "25"], &["3000", "6000", "12000", "24000", "120000"]])),
    (63, Row::new("カエルの合唱が響き渡る。何やら意味がある歌声に聞こえるが…[知力-%s]判定に成功すると歌の意味を理解できる。カエル達は安全な道を教えてくれていたのだ!判定に成功した場合クリア時の財宝ランク+1。判定に失敗すると毒の沼に足を踏み入れ、PC全員が「%s」点の防護点無視ダメージを受ける。有効:「自然と生きる」【賢人の知恵】", &[&["6", "9", "12", "15", "25"], &["3D6+10", "3D6+20", "3D6+30", "3D6+50", "150"]])),
    (64, Row::new("老齢のグラントが岩に腰かけ釣りをしている。このイベントが発生するとエネミーは消滅する。「どうだね、よかったら釣り勝負をやっていかないか?」PC全員が行動を消費して3D6を振り、誰かが3か4を出せば見事勝利することができる。「見事じゃ、そこにあるガラクタはワシには必要ないものじゃから好きに持って行くがいい」と財宝を渡してくれる。財宝ランク%sのアイテムを2つ入手可能。", &[&["3", "5", "7", "9", "11"]])),
    (65, Row::new("沈没した「死者の方舟」だ。探せば何かありそうだが危険そうだ…1D6を振って以下の結果を適用すること。このイベントは無視することができ、その場合は何も起きない。\n----------\n1～2:…これは財宝だ!財宝ランク%sのアイテムと%sを入手。\n3～4:…マストが倒れてきた!「水辺5-3」と同様のイベントが発生する。\n5～6:…船に残っていたアンデッドが襲ってきた!エネミーとエンカウント。", &[&["3", "5", "7", "9", "11"], &["スケルトンメイル(鎧)", "死者の心臓(装飾品)", "死霊貴族の服(鎧)", "デュラハンの盾(盾)", "死者の止まらない歯車（装飾品）"]])),
    (66, Row::new("貝の群生地だ。貝の中にはキラリと光るものが…あれは真珠だ!行動を消費して[敏捷-%s]判定を行い、成功すれば1回ごとにアイテム「真珠」を入手可能。これは1つ%sGPで売却可能で、4ラウンド目が終了するまで何回でも入手可能。ただし、判定の成否に関わらず〔FP〕を2点消費する。「水中行動」があれば判定不要で〔FP〕も消費しない。有効:【野伏の歩み】", &[&["6", "9", "12", "15", "20"], &["1000", "2000", "4000", "8000", "20000"]])),
];

/// Ruby `EVENT_TABLES[4]`（森林）の行。
static EVENT_ROWS_5: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("鬱蒼とした森の奥に見事な佇まいの茶室を見つける。ラウンド中に【ソチャデスカ】を使用した際の回復量が2倍になる。茶室の中には友好的なカラクリフラウがいて、%sGPを支払ったPC全員の〔HP〕〔FP〕を「3D6+%s」点回復してくれる。", &[&["100", "200", "300", "500", "2000"], &["10", "20", "30", "50", "100"]])),
    (13, Row::new("おいしそうな果実の木だ!とてもおいしく、食べるとやたらとテンションが高くなる。〔HP〕〔FP〕が「3D6%s」点回復するが、シナリオ終了まで不利な特異点「脳みそ筋肉」を得たかのように猪突猛進型の性格になってしまう。[知力-%s]判定に成功すれば興奮作用のある種をどけてデメリットは受けない。有効:「自然と生きる」【賢人の知恵】", &[&["", "+5", "+10", "+15", "+30"], &["3", "6", "9", "12", "20"]])),
    (14, Row::new("「ひんやりおいしい森のアイスやさんですよ～。今なら私との握手券付き～♪」フラウが果物いっぱいのトロピカルなアイスクリームを売っている。マジカルクッキング「フラウアイスクリーム」(Lv4)を食べさせてもらえる。この料理の効果は、自分たちが「マジカルクッキング」で作った料理と累積する。", &[])),
    (15, Row::new("森のなかにひっそりとたたずむジンジャを発見して、ナインテイルのカンヌシかミコが旅の無事を祈ってくれる。PT全員に【不死なる炎】がかかり、最終イベントに入るまで、死亡判定に自動成功するようになる。", &[])),
    (16, Row::new("食人植物の花粉による回復の泉の幻影。PTメンバーの〔HP〕〔FP〕のいずれかが減少している場合このイベントは拒否できない。[知力-6]判定に失敗すると食人植物の群生地にふらふらと入り、PC全員が「%s」の「斬撃」属性ダメージを受ける。有利な特異点「自然と生きる」があれば、事前に気が付きダメージは受けない。有効:【賢人の知恵】", &[&["3D6+20", "3D6+30", "3D6+40", "3D6+60", "150"]])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("上から3本の蔓が垂れ下がっている。すごく引っ張りたくなる形状だ…行動を消費して引っ張ったキャラクターは1D6を振って以下のものを入手する。蔦を1つ引っ張ると他の蔦はスルスルっと上に消えていってしまう。有効:「非凡な直感」\n----------\n1～2:大当たり!財宝ランク%sの財宝\n3～4:小当たり!おいしい果物(%sと同等)\n5～6:「ハズレ」と書かれた硬い木の実が直撃!「%s」点の「打撃」属性ダメージを受ける", &[&["3", "5", "7", "9", "11"], &["HPポーション", "高級HPポーション", "最高級HPポーション", "奇跡のポーション", "蘇生ポーション"], &["3D6+10", "3D6+20", "3D6+30", "3D6+50", "150"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("この獣道を通れば近道になりそうだ!しかし狭くて動きにくい…クリア時の財宝ランクが+1されるが、ラウンド中の〔移動〕に-3のペナルティ。", &[])),
    (32, Row::new("「ホウホウホーウ。そっちの道は危ないぞ…」鳥たちが古い言葉で道案内をしてくれる。[知力-6]判定に成功すれば、クリア時の財宝ランク+1。失敗すると次のイベントは必ず「森林」のエンカウントイベントになる。このイベントが4ラウンド目だった場合、道に迷ってクリア時の財宝ランク-1。有効:「自然と生きる」【賢人の知恵】", &[])),
    (33, Row::new("時計をもった兎が忙しそうに走っている。追いかけていくと木の根本に落っこちて妙な場所についてしまった!4ラウンド目が終了するまで「迷宮オプション」を新たに1つ追加する。このイベントが4ラウンド目だった場合、近道になりクリア時の財宝ランク+1。", &[])),
    (34, Row::new("毒草の群生地だ!突っ切れば近道になりそうだが…気にせず直進すればシナリオ終了まで〔HP〕の最大値と現在値が%s点減少するが、クリア時の財宝ランク+1。迂回すると悪い効果は受けないがクリア時の財宝ランク-1。", &[&["10", "15", "20", "25", "40"]])),
    (35, Row::new("「花の香りのする方においで」と看板がある。[感覚-%s]判定に失敗すると遠回りになりクリア時の財宝ランク-1。有利な特徴「犬の鼻」が%s", &[&["6", "9", "12", "15", "20"], &["あれば判定不要。", "あれば判定不要。", "二段階あれば判定不要。一段階はボーナスのみ。", "二段階あれば判定不要。一段階はボーナスのみ。", "二段階あれば判定不要。一段階はボーナスのみ。"]])),
    (36, Row::new("おや、進行方向から動物たちが逃げてくるぞ?…大変だ、森林火災だ!引き返して火災から逃げるとクリア時の財宝ランク-2。構わず突っ切るなら4ラウンド目が終了するまで、ターン開始時に「%s」の「火炎」属性ダメージを受ける。", &[&["3D6+10", "3D6+15", "3D6+20", "3D6+30", "100"]])),
    (41, Row::new("森の盗賊団がこちらの懐を狙ってきた!エネミーとエンカウント。このエネミーからダメージを受けるたびに所持品のうち最も価格の安いものを奪われてしまう。4ラウンド目までに倒せば奪い返すことができる。", &[])),
    (42, Row::new("動物狩りを楽しんでいた魔族と偶然遭遇してしまう。エネミーとエンカウント。「分類:魔物」「分類:魔獣」のエネミーが登場していた場合、魔族たちにに狩られると思い逃亡する。", &[])),
    (43, Row::new("魔族の偵察隊と遭遇する。報告される前に倒さなくては!指定のエネミーを1ラウンド以内に倒せなかった場合、次のエンカウントが発生した際、新たに登場するエネミーは準備万端で待ち構えているため〔先制〕に+10、あらゆる致傷力に+%sのボーナス。", &[&["10", "20", "30", "40", "100"]])),
    (44, Row::new("森を支配する木の魔族の王に遭遇してしまった!エネミーとエンカウント。%s4ラウンド終了までに討伐に成功すると財宝ランク%sのアイテムを入手できる。", &[&["エネミーは〔HP〕に+100のボーナス。", "エネミーは〔HP〕に+150のボーナス。", "エネミーは〔HP〕に+200のボーナス。", "", "エネミーは〔HP〕に+200、あらゆる致傷力+200、分類「ネームド」と【始原の力】を追加。"], &["3", "5", "7", "9", "11"]])),
    (45, Row::new("フラウとその尖兵が道案内をしてくれる…と思ったらカラーコンタクトをつけてフラウのふりをした魔族とその尖兵だった!エネミーとエンカウント。尖兵は「PC配置ヘクス」に配置すること。PCはラウンド終了まで〔回避〕に-6のペナルティ。有利な特異点「背中の目」があれば〔回避〕へのペナルティは受けない。", &[])),
    (46, Row::new("迷彩服を着て軍事訓練中の第三師団精鋭コンバット部隊だ!エネミーとエンカウント。エネミーは〔先制〕に+4のボーナス。", &[])),
    (51, Row::new("良い香りが周囲に満ちていく。戦闘中だった場合、闘争心を失わせるキノコの香りで敵味方ともに剣を収め強制的に戦闘終了となる。戦闘中ではなかった場合は[知力-6]判定に失敗すると香りをまともにかいで闘争心を失ってしまい、シナリオ終了まであらゆる致傷力に-5のペナルティ。有効:「犬の鼻」「自然と生きる」【賢人の知恵】", &[])),
    (52, Row::new("突然上から木の蔓が誰かを吊り上げて首を絞めようとする!ランダムなPC1人は[感覚-%s]判定を行い、失敗すると「%s」点の防護点無視ダメージを受け、そのラウンド中は行動も〔回避〕も行えない。有利な特異点「自然と生きる」をもつPCが対象になった場合は何も起きない。有効:【野伏の歩み】", &[&["3", "4", "5", "7", "10"], &["3D6+30", "3D6+50", "3D6+70", "3D6+100", "300"]])),
    (53, Row::new("巧妙なベアトラップが仕掛けてある。ランダムなキャラクター1人(エネミーも含む)に「%s」点の「斬撃」属性ダメージを与え、〔移動〕〔回避〕を0にする。このトラップにかかったキャラクターか隣接したキャラクターが行動を消費して[感覚-%s]判定に成功すれば解除可能。有効:【盗賊の心得】【ロックスミス】", &[&["3D6+10", "3D6+20", "3D6+30", "3D6+50", "200"], &["6", "8", "10", "12", "20"]])),
    (54, Row::new("突然前も見えないほどのスコールに見舞われる!敵味方全員の攻撃や【特技】の射程が全て1になってしまう。森林「3-6」の山火事が起きていた場合、鎮火する。", &[])),
    (55, Row::new("ランダムなPC1人が巨大な食虫植物に喰われてしまう!幸いすぐに助け出すことができたのだが、鎧も服もボロボロにされてしまう(ｲﾔｰﾝ)喰われたPCが装備していた鎧はシナリオ終了まで使用不可能になる。", &[])),
    (56, Row::new("すさまじい悪臭を放つ巨大な花だ!あまりの悪臭にPCの装備していた「ペット」は全て気絶してシナリオ終了まで使用不可能になる。なぜかギア製のペットももれなく気絶する。有利な得意点「犬の鼻」をもつPCがいれば近寄る前に回避可能。", &[])),
    (61, Row::new("魔族の狩りで傷ついた狼を発見する。ラウンド終了までに〔HP〕を1点でも回復してやればふらふらと歩きだし、木の根元でここを掘れというように吠える。埋まっていたものは1D6を振って決定する。\n----------\n1:犬の糞(ゴミ)\n2:10GP\n3:財宝ランク%sのアイテム\n4:財宝ランク%sのアイテム\n5:財宝ランク%sのアイテム\n6:うぃっしゅすたー(大事なもの)", &[&["0", "2", "4", "6", "8"], &["1", "3", "5", "7", "9"], &["2", "4", "6", "8", "10"]])),
    (62, Row::new("少し開けた原っぱでフラウのダンスパーティーに誘われる。このイベントが発生するとエネミーは消滅する。行動を消費して[敏捷-%s]判定に成功してダンスを踊ると原っぱの中心からすくすくと大きなキノコが育ち、キノコの馬車でダンジョンのゴール付近まで一気にすっとばしてくれる。第4ラウンドが終了したものと扱い、クリア時の財宝ランク+2。", &[&["6", "8", "10", "12", "16"]])),
    (63, Row::new("フラウたちが「森ガールファッションコンテスト」を開催しており、その助っ人を依頼される。このイベントが発生するとエネミーは消滅する。[意思-%s]判定に成功すれば的確なアドバイスで助っ人を依頼したフラウを優勝に導くことができ、その謝礼として%sを入手できる。", &[&["6", "8", "10", "12", "20"], &["狩人の羽根帽子(装飾品)", "緊急召喚の宝珠(装飾品)", "毒の香水瓶(装飾品)", "女王の花冠(装飾品)", "魔将樹のドレス(鎧)"]])),
    (64, Row::new("霊樹の花にたまった水が神秘的な魔力を帯びている。[知力-6]判定に成功するとそれにポーションとしての効果があることがわかり、奇跡のポーション(消耗品)を入手する。【ポーションナレッジ】を習得したPCがいれば判定不要。有効:【賢人の知恵】", &[])),
    (65, Row::new("迷子のフラウを見つける。4ラウンド目が終了するまでに他のフラウに出会うイベントが発生した場合、偶然にもそのフラウと知り合いで、お礼に財宝ランク%sのアイテムを渡してくれる。フラウに出会えずシナリオをクリアした場合も、迷宮からひとまず出ることができたお礼に財宝ランク%sのアイテムを渡してくれる。", &[&["3", "5", "7", "9", "11"], &["1", "3", "5", "7", "9"]])),
    (66, Row::new("ランダムに選んだPC1人が、うっかり泉に武器を落としてしまう。すると泉から女神が出現して、落とした武器と同じ種類の超高級品の武器(魔将ドロップ品など)を落としたかどうかを聞いてくる。[意思%s]判定に成功すれば正直に答え、武器に%sGPまでの任意のアタッチメントを追加してくれる。失敗すると嘘をついて女神を怒らせてしまい、装備していた武器を失ってしまう。", &[&["", "-2", "-4", "-6", "-8"], &["5000", "10000", "30000", "50000", "100000"]])),
];

/// Ruby `EVENT_TABLES[5]`（墓場）の行。
static EVENT_ROWS_6: &[(i64, Row)] = &[
    (11, Row::new("回復の泉。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。", &[])),
    (12, Row::new("「彼らは生前の思いが強すぎ成仏できません。語りかけて鎮めてやるとよいでしょう…」アズマ風の墓石が佇んでいる。[知力-%s]判定か[意思-%s]判定に成功するとアズマ式お祈りや語りかけにより霊に感謝され、〔HP〕〔FP〕とFtが全回復する。失敗すると霊を怒らせて呪われ、全員のFtが1点減少する。有効:【賢人の知恵】(知力)【詩人の交渉術】(意思)", &[&["3", "6", "9", "12", "16"], &["3", "6", "9", "12", "16"]])),
    (13, Row::new("墓守の休憩所を見つける。心の落ち着くお線香の香りがする…。〔HP〕〔FP〕%s点とBSを回復する。更にシナリオ終了まで〔抵抗〕に+2のボーナス。", &[&["10", "15", "20", "25", "30"]])),
    (14, Row::new("「人間か。キミたちはここの亡者のようにならないようにね…」ヒガンバナの世話をするフラウと出会い、「ヒガンバナの毒」を武器に塗ってもらうことができる。物理攻撃の致傷力に+5のボーナス。", &[])),
    (15, Row::new("シルヴァテイルのネクロマンサー達が集まって、何やら怪しい儀式をしている。道中で「★死亡」した、もしくは保険を適用して強制送還されたキャラクターがいれば、〔HP〕〔FP〕とBSが全回復した状態で復活させてもらうことができる。それ以外の場合は不気味な儀式に不安を感じ、FPを1点失う。", &[])),
    (16, Row::new("回復の泉があるのだが…?PCのうち1人でも〔HP〕〔FP〕のいずれかが減少している場合このイベントは拒否できない。PC全員の〔HP〕〔FP〕が完全に回復する。「★死亡」以外のあらゆるBSも回復する。黄泉の国の水を飲んだことでシナリオ終了まで「分類:アンデッド」となり、【死者に鞭】【アマテラスの光】などの、一部の【特技】の効果を受ける。", &[])),
    (21, Row::new("財宝ランク%sの宝箱を発見する。カギやトラップはかかっていない。行動を消費することで開けることができる。", &[&["0", "2", "4", "6", "8"]])),
    (22, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのカギがかかっている。行動を消費して[感覚-%s]判定でカギを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (23, Row::new("財宝ランク%sの宝箱を発見する。難易度%sのトラップがかかっている。行動を消費して[感覚-%s]判定でトラップを解除して開けることができる。有効：【盗賊の心得】【ロックスミス】", &[&["1", "3", "5", "7", "9"], &["3", "6", "9", "12", "20"], &["3", "6", "9", "12", "20"]])),
    (24, Row::new("骸が何かを大事そうに握りしめている(抱きかかえている)。行動を消費して[体力-%s]判定に成功すれば、財宝ランク%sのアイテムを入手できるが、Ftが1点減少する。", &[&["2", "4", "6", "10", "15"], &["2", "4", "6", "8", "10"]])),
    (25, Row::new("ミミックの罠。ランダムなキャラクターに「3D+%s」の「斬撃」属性ダメージを与える。[知力-%s]判定に成功すればこの罠を見抜いて無効化でき、なおかつ%sGPを入手可能。有効：【盗賊の心得】【ロックスミス】", &[&["30", "50", "70", "100", "300"], &["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (26, Row::new("トレジャーイーターの罠。冒険中に入手したアイテム(消耗品・GP以外)を全て失う。[知力-%s]判定に成功すればこの罠を見抜いて無効化することができ、なおかつ%sGP入手可能。", &[&["3", "6", "9", "12", "20"], &["1000", "2000", "5000", "10000", "30000"]])),
    (31, Row::new("ちょっとなにか出そうな、柳の木でできたトンネルを発見する。PC全員が[意思]判定を行い、全員成功した場合はクリア時の財宝ランク+1。最も成功度の低かった(失敗度の大きかった)PCは揺れる柳を幽霊と思い込み、4ラウンド終了まであらゆる判定に-2のペナルティ。", &[])),
    (32, Row::new("石畳の道を歩いている途中で朽ちた祠を発見する。何かお供え物をしたほうが良いだろうか?アイテム欄の消耗品1つをお供えすると祠が動いて地下通路が出現する!この通路を通ることで近道できて、クリア時の財宝ランク+1。", &[])),
    (33, Row::new("道が途切れ、草が伸び放題の荒れ地に出る。方向もわからず、進むには骨が折れそうだ…[体力-%s]判定を行い、成功すればクリア時の財宝ランク+1。失敗するとクリア時の財宝ランク-1。", &[&["2", "4", "6", "8", "12"]])),
    (34, Row::new("どこまでも卒塔婆とススキしかない…同じ景色が続き、進んでいるか怪しくなる。1D6を振って1～3が出れば何もないが、4～6が出るとクリア時の財宝ランク-1。有利な特異点「天体時計」があればペナルティは受けない。", &[])),
    (35, Row::new("地面から突如骨の手が伸びて足を掴んでくる!PC全員が[移動%s]判定を行い、クリア時の財宝ランクが「失敗した人数」分減少する。【死者に鞭】を持つPCがいれば骨は威圧されて引っ込み、この効果は無視できる。", &[&["+4", "+2", "", "-2", "-5"]])),
    (36, Row::new("先へ進む道には纏わりつくような死者の呪いが満ちている。直進するとシナリオ終了まであらゆる判定に-2のペナルティ。遠回りをすると悪影響は受けないがクリア時の財宝ランク-2。【死者に鞭】か【鎮魂の盾】を持つPCがいれば呪いをはねつけ、直進してもペナルティを受けない。", &[])),
    (41, Row::new("墓場に浮かんでいた人魂が突如魔物になって襲いかかってきた!エネミーとエンカウント。エネミーが使用する【狐の送り火】のデータを「疲労：なし」に変更。", &[])),
    (42, Row::new("朽ちていた骨が突如起き上がり襲いかかってきた!エネミーとエンカウント。このエネミーは業物の武器を持っており、物理攻撃の致傷力に+%sのボーナス。", &[&["5", "10", "15", "20", "200"]])),
    (43, Row::new("不吉な歌を口ずさみ、ステップを踏みながらかわいらしい少女が近づいてくる。エネミーとエンカウント。エネミーの【華劇】はは最初から使用された状態になっており、【華劇】の効果に応じた強化がエネミー全員に適用される。", &[])),
    (44, Row::new("遠くから不吉な進軍の音を聞く。PC全員が[感覚-%s]判定を行い、誰か1人でも成功すれば、第二魔将グレヴディガーの軍勢が迫っているのを知る。全員判定に失敗してしまうとPCは第二魔将グレイヴディガーとその配下である無数の亡霊騎士団と相対することになる。%s有効:「狐の耳」「虫の知らせ」", &[&["2", "4", "6", "8", "10"], &["「保険」に入っていなければPCたちは死亡して、グレイヴディガーの配下にさせられてしまう。君たちの冒険はここで終了だ。", "「保険」に入っていなければPCたちは死亡して、グレイヴディガーの配下にさせられてしまう。君たちの冒険はここで終了だ。", "「保険」に入っていなければPCたちは死亡して、グレイヴディガーの配下にさせられてしまう。君たちの冒険はここで終了だ。", "エネミーとエンカウント。", "エネミーとエンカウント（魔将オリジン実装後）。オリジン未実装の場合はイベントの振り直しを行う。"]])),
    (45, Row::new("「我らの眠りを妨げるものは誰だ…!」貴族の棺とそれを守る人形だ。墓荒らしには容赦なく襲い掛かる!エネミーとエンカウント。棺の中には貴金属が入っており、一部エネミーの通常ドロップの金額が増加する。", &[])),
    (46, Row::new("壁に埋まった10mはあろうかという巨大な蛇の骨が動き出し襲い掛かってくる!エネミーとエンカウント。", &[])),
    (51, Row::new("PC全員に墓から突然幻影が襲いかかる。幻影なので実害はないのだが、本当の攻撃と思って「パリイ」か「シールド」を1回消費してしまう。どちらも行えない場合、腰を抜かして「☆転倒」を受ける。【トゥルービジョン】などがかかっているPCには無効。", &[])),
    (52, Row::new("「ひひっ、かかったなボケが!」理不尽にもギロチンが突如ランダムなPC1人に落ちてくる!「ドッジ-%s」判定を行い、失敗すると「%s」点の「斬撃」属性ダメージを受ける。", &[&["4", "6", "8", "10", "20"], &["3D6+40", "3D6+60", "3D6+80", "3D6+120", "400"]])),
    (53, Row::new("「すいっちおーん。私達の仲間にしてやるよ!」用途不明の建物に迷い込んでしまったが…なんてこった、ここは火葬場だ!早く脱出しないと灰になるまで焼かれてしまうぞ!PC全員が「%s」点の「火炎」属性ダメージを受ける。〔移動〕が%s以上のPCは何とか逃げ出しダメージを受けず、さらに自分より[体力]の低いキャラクター1人を連れて脱出可能。有効:【盗賊の心得】【ロックスミス】", &[&["3D6+30", "3D6+40", "3D6+50", "3D6+70", "300"], &["8", "9", "10", "12", "16"]])),
    (54, Row::new("墓石が動き出し中から何かが出てこようとしている![体力-%s]判定に成功すればなにも起こらないが、失敗すると探索中に倒されたエネミーの中で最もCLが高いエネミーが再度出現する。該当モンスターが存在しなかった場合は、昼寝をしていたフラウが出てくるだけ。", &[&["4", "6", "8", "10", "15"]])),
    (55, Row::new("金の亡者のゾンビが群がってくる。金を渡してやるまで満足しそうにない…%sGPを支払うと満足して去っていくが、支払いを拒否した場合(もしくはお金が足りなかった場合)はPC全員が「%s」の「打撃」属性ダメージを受ける。【死者に鞭】を持つPCがいればひと睨みでそそくさと去っていく。", &[&["1000", "2000", "4000", "8000", "20000"], &["3D6+10", "3D6+20", "3D6+30", "3D6+50", "200"]])),
    (56, Row::new("「おやおや。死者に気に入られましたか…困りましたねえホッホッ」ランダムなPC1人の武器が死者の怨念を吸い取り魔剣となる!魔剣は攻撃力に+5のボーナスを得るが、装備していると〔HP〕の最大値が10点減少する。魔剣となった武器は売却不可能で、不要な場合は捨てるしかない。すでに魔剣となっている武器と、【特技】によって入手した武器は魔剣にならない。", &[])),
    (61, Row::new("雷が突然鳴り響く。敵味方全員が3D6を振り、出目が一番高いキャラクターは「%s」点の「電撃」属性ダメージを受ける。", &[&["3D6+30", "3D6+50", "3D6+70", "3D6+100", "350"]])),
    (62, Row::new("「いいところに来たナ。どうだ、お前らも飲んでいくカ?」タルの上に腰かけ、減ることのない酒を口に運び続けるスケルトンがいる。このイベントが発生するとエネミーは消滅する。酒を飲む場合、行動を消費して[意思%s]判定を4回連続で行う。全て成功すると、「ついに酒が切れたカ、最後の晩酌に付き合ってもらって感謝するゼ」と感謝を述べて消滅し、あとには財宝ランク%sのアイテムが残る。途中で1度でも失敗した場合、酒に悪酔いしてシナリオ終了まであらゆる判定に-2のペナルティ。", &[&["", "-1", "-2", "-4", "-8"], &["3", "5", "7", "9", "11"]])),
    (63, Row::new("怪しい輝きを放つ宝石を発見する。手にとったPCは呪われてシナリオ終了まであらゆる判定に-2のペナルティ。手に取らない場合は全員に[意思%s]判定が必要で、不利な特異点「金の亡者」「好奇心は猫を殺す」があるとこの[意思]判定に-4のペナルティ。無事持ち帰った場合呪いは解け、%sGPで売却できる。", &[&["", "-1", "-2", "-3", "-6"], &["2000", "6000", "20000", "60000", "150000"]])),
    (64, Row::new("朽ちたギアの墓場だ。まだ使えるギアがあるかもしれない。【魔導工学のススメ】があれば、SLvに応じたアイテムが1つ入手可能。\n----------\nSLv1:カラクリバランサー(装飾品)\nSLv2:カラクリお茶キット(装飾品)\nSLv3:ゴーレムアーム(装飾品)\n%s\n%s\n%s\n%s", &[&["", "SLv4:ホイールブースター(装飾品)", "SLv4:ホイールブースター(装飾品)", "SLv4:ホイールブースター(装飾品)", "SLv4:ホイールブースター(装飾品)"], &["", "", "SLv5:ナグルファル装甲(装飾品)", "SLv5:ナグルファル装甲(装飾品)", "SLv5:ナグルファル装甲(装飾品)"], &["", "", "", "SLv6:死者の止まらない歯車(装飾品)", "SLv6:死者の止まらない歯車(装飾品)"], &["", "", "", "", "SLv7:特別アタッチメント券(大事なもの)"]])),
    (65, Row::new("「ほーらよっといでーよっといでー、地獄の沙汰も金次第…」うすら寒い気配の漂う川辺に出る。川辺には渡し守らしき、ローブをかぶった2人の少女がいる。1人あたり%sGPを支払えばクリア時の財宝ランク+1。支払いを拒否したかお金が足りない場合、エネミーとエンカウント。", &[&["200", "500", "1000", "2000", "5000"]])),
    (66, Row::new("『千ノ戦イデ千ノ勝チヲ知ル者、ココニ眠ル』と刻まれた棺を見つける。棺を開けた場合はエネミーとエンカウント。勝利すると%sを入手できる。不利な特異点「好奇心は猫を殺す」「脳みそ筋肉」を持つキャラクターは[意思]判定を行い、失敗すると棺を開けてしまう。", &[&["キメラパーツ(装飾品)", "キメラパーツ(装飾品)", "グレートキメラパーツ(装飾品)", "ウェポンマスターの証(装飾品)", "ウェポンマスターの証(装飾品)"]])),
];

/// Ruby `EVENT_TABLES`（地形1〜6の順）。
static EVENT_TABLES: [EventTable; 6] = [
    EventTable::new("洞窟", 1, EVENT_ROWS_1),
    EventTable::new("遺跡", 2, EVENT_ROWS_2),
    EventTable::new("山岳", 3, EVENT_ROWS_3),
    EventTable::new("水辺", 4, EVENT_ROWS_4),
    EventTable::new("森林", 5, EVENT_ROWS_5),
    EventTable::new("墓場", 6, EVENT_ROWS_6),
];

/// Ruby `AREA_LIST`。
static AREA_LIST: [&str; 6] = ["洞窟", "遺跡", "山岳", "水辺", "森林", "墓場"];

/// Ruby `ENEMY_DATA`（イベントダイス → 難易度順（E/N/H/L/X）のエネミー）。
static ENEMY_DATA: &[(&str, [&str; 5])] = &[
    ("141", [
        "キャンディークラウン(CL40)1匹",
        "キャンディクラウン(CL40)「1D-3」匹(最低1匹)",
        "キャンディークラウン「1D-2」匹(最低1匹)出目が6だった場合ゴールデンクラウン(CL177)1匹",
        "ゴールデンクラウン(CL177)とエンカウント",
        "キャンディークラウン(CL40/オリジン)1匹",
    ]),
    ("142", [
        "ベルセルク(CL7)3匹",
        "ベルセルク(CL7)3匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+20)",
        "ベルセルク(CL7)3匹(〔命中〕+8、〔HP〕+60、〔FP〕+20、あらゆる致傷力+40)",
        "ベルセルク(CL7)3匹(〔命中〕+16、〔HP〕+120、〔FP〕+40、あらゆる致傷力+80)",
        "ベルセルク(CL7/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("143", [
        "ガーゴイル(CL10)2匹",
        "ガーゴイル(CL10)2匹(〔命中〕+4、〔HP〕+20、〔FP〕+10、あらゆる致傷力と防護点+20)",
        "ガーゴイル(CL10)2匹(〔命中〕+8、〔HP〕+40、〔FP〕+20、あらゆる致傷力と防護点+40)",
        "ザッハーク(CL39)3匹",
        "ガーゴイル(CL10/オリジン)2匹(〔HP〕+500、あらゆる致傷力と防護点+100)",
    ]),
    ("144", [
        "デビルホイール(CL18)1匹",
        "デビルホイール(CL18)1匹、あらゆる致傷力+20",
        "デビルホイール(CL18)1匹、あらゆる致傷力+40",
        "ムスペルヘイムの獣(CL65)、あらゆる防護点+20",
        "デビルホイール(CL18/オリジン)1匹(〔HP〕+1000、あらゆる致傷力+100)",
    ]),
    ("145", [
        "ヘルハウンド(CL12)1匹、インプ(CL3)2匹",
        "フェニックス(CL25)2匹",
        "火龍ヘルブレイザー(CL50)1匹、ケルベロス(CL33)1匹",
        "煉獄龍バーガトリ(CL100)1匹",
        "ヘルハウンド(CL12/オリジン)1匹、インプ(CL3/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("146", [
        "アイスメイデン(CL10)1匹、アイスリザード(CL7)2匹",
        "クリスタルビースト(CL25)2匹",
        "氷河龍グレイシャル(CL65)1匹",
        "冥界龍コキュートス(CL110)1匹",
        "アイスメイデン(CL10/オリジン)1匹、アイスリザード(CL7/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("241", [
        "スケルトン(CL3)、アルテミス(CL8)、スターゲイザー(CL10)、ヤシャ(CL10)、ヘルハウンド(CL12)、ゴーレム(CL13)",
        "ナイトスケルトン(CL12)、カラクリフラウ弐式(CL16)、コスモロジスト(CL20)、クリスタルビースト(CL25)、ミスリルゴーレム(CL27)",
        "コガラシ(CL27)、アダマンゴーレム(CL45)、アシュラ(CL48)、火龍ヘルブレイザー(CL50)、氷龍グレイシャル(CL65)",
        "ロックダンサー(CL39)、デスサジタリー(CL45)、コンゴウ(CL60)、カラクリヒメショーグン(CL75)、フェンリル(CL85)、ゴールデンクラウン(CL177)\nゴールデンクラウンが最後に残り【逃走】した場合ドロップ品は入手できない",
        "スケルトン(CL3/オリジン)、アルテミス(CL8/オリジン)、スターゲイザー(CL10/オリジン)、ヤシャ(CL10/オリジン)、ヘルハウンド(CL12/オリジン)、ゴーレム(CL13/オリジン)(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("242", [
        "アルテミス(CL8)3匹",
        "アルテミス(CL8)3匹(〔命中〕+4、〔HP〕+30、〔FP〕+20、あらゆる致傷力+20)",
        "アルテミス(CL8)3匹(〔命中〕+8、〔HP〕+60、〔FP〕+30、あらゆる致傷力+40)",
        "アルテミス(CL8)3匹(〔命中〕+16、〔HP〕+120、〔FP〕+40、あらゆる致傷力+80)",
        "アルテミス(CL8/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("243", [
        "スターゲイザー(CL10)2匹",
        "コスモロジスト(CL20)2匹",
        "コスモロジスト(CL20)4匹",
        "デスサジタリー(CL45)2匹",
        "スターゲイザー(CL10/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("244", [
        "ヤシャ(CL10)1匹、スケルトン(CL3)3匹",
        "ヤシャ(CL10)3匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+15)",
        "アシュラ(CL48)2匹",
        "コンゴウ(CL60)2匹",
        "ヤシャ(CL10/オリジン)1匹、スケルトン(CL3/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("245", [
        "ヘルハウンド(CL12)2匹",
        "クリスタルビースト(CL25)2匹",
        "火龍ヘルブレイザー(CL50)1匹、氷河龍グレイシャル(CL65)",
        "『単眼の巨神』アイングロース(CL100/ネームド)",
        "ヘルハウンド(CL12/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("246", [
        "ゴーレム(CL13)1匹",
        "ミスリルゴーレム(CL27)1匹",
        "アダマンゴーレム(CL45)1匹",
        "カラクリヒメショーグン(CL75)1匹",
        "ゴーレム(CL13/オリジン)1匹(〔HP〕+1000、あらゆる致傷力+100)",
    ]),
    ("341", [
        "アイアンプリチン(CL5)5匹",
        "アイアンプリチン(CL5)5匹(〔命中〕+4、〔HP〕+30、〔FP〕+10)",
        "アイアンプリチン(CL5)5匹(〔命中〕+8、〔HP〕+60、〔FP〕+20)",
        "アイアンプリチン(CL5)5匹(〔命中〕+16、〔HP〕+120、〔FP〕+40)",
        "アイアンプリチン(CL5/オリジン)5匹(〔HP〕+300、あらゆる致傷力+100)",
    ]),
    ("342", [
        "ウイングスレイブ(CL4)5匹",
        "ウイングスレイブ(CL4)5匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+20)",
        "ガルーダ(CL40)2匹",
        "栄光の天使(CL60)2匹",
        "ウイングスレイブ(CL4/オリジン)5匹(〔HP〕+300、あらゆる致傷力+100)",
    ]),
    ("343", [
        "ブラックナイト(CL10)1匹、ソルジャー(CL4)3匹",
        "デュラハン(CL22)1匹、ブラックナイト(CL10)2匹",
        "サーバントマスター(CL35)1匹、デュラハン(CL22)2匹",
        "ジェネラル(CL55)1匹、サーバントマスター(CL35)2匹",
        "ブラックナイト(CL10/オリジン)1匹、ソルジャー(CL4/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("344", [
        "ソルジャーアント(CL8)2匹",
        "ソルジャーアント(CL8)2匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+20)",
        "インペリアルアント(CL30)2匹",
        "『女王キノコ』アカシックツリー(CL50/ネームド)1匹、インペリアルアント(CL30)2匹",
        "ソルジャーアント(CL8/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("345", [
        "カボチャの魔女(CL8)2匹",
        "カボチャの魔女(CL8)2匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+20)",
        "カボチャの大魔女(CL35)2匹",
        "堕落の黒天使(CL60)2匹",
        "カボチャの魔女(CL8/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("346", [
        "フェニックス(CL25)2匹",
        "フェニックス(CL25)2匹(〔命中〕+4、〔HP〕+40、〔FP〕+10、あらゆる致傷力+30)",
        "フェニックス(CL25)2匹(〔命中〕+8、〔HP〕+80、〔FP〕+20、あらゆる致傷力+60)",
        "煉獄フェニックス(CL85)2匹",
        "フェニックス(CL25/オリジン)2匹(〔HP〕+1000、あらゆる致傷力+200)",
    ]),
    ("441", [
        "フロストクラブ(CL6)2匹",
        "フロストクラブ(CL6)3匹",
        "『氷海の主』ギガントキャンサー(CL20/ネームド)",
        "『氷海の主』ギガントキャンサー(CL20/ネームド)、フロストクラブ(CL6)3匹",
        "フロストクラブ(CL6/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("442", [
        "アシガル(CL4)3匹",
        "ハタモト(CL10)3匹",
        "ヒトキリ(CL33)2匹",
        "スカーレス(CL38)3匹",
        "アシガル(CL4/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("443", [
        "アンブッシュマン(CL5)3匹",
        "アンブッシュマン(CL5)3匹(〔命中〕+4、〔HP〕+20、〔FP〕+10、あらゆる致傷力+10)",
        "シャドウストーカー(CL35)2匹",
        "シャドウストーカー(CL35)3匹",
        "アンブッシュマン(CL5/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("444", [
        "アップルドール(CL12)2匹",
        "アップルドール(CL12)2匹、ポイズンリリー(CL7)2匹",
        "テラードール(CL30)2匹",
        "テラードール(CL30)2匹、バリアリーフ(CL27)2匹",
        "アップルドール(CL12/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("445", [
        "ナイトライダー(CL15)1匹",
        "ナイトライダー(CL15)2匹",
        "ジェネラル(CL55)1匹",
        "ジェネラル(CL55)2匹",
        "ナイトライダー(CL15/オリジン)1匹(〔HP〕+800、あらゆる致傷力+100)",
    ]),
    ("446", [
        "ジェルスケルトン(CL10)1匹",
        "ジェルスケルトン(CL10)1匹(〔命中〕+4、〔FP〕+10、あらゆる致傷力+20)",
        "ジェルスケルトン(CL10)1匹(〔命中〕+8、〔FP〕+20、あらゆる致傷力+40)",
        "ジェルスケルトン(CL10)1匹(〔命中〕+12、〔FP〕+40、あらゆる致傷力+80)",
        "ジェルスケルトン(CL10/オリジン)1匹(〔HP〕+1500、あらゆる致傷力+200)",
    ]),
    ("541", [
        "スナッチャー(CL4)3匹",
        "ゴールデンビースト(CL10)3匹",
        "ヨウコ(CL30)2匹",
        "イビルオーメン(CL42)2匹",
        "スナッチャー(CL4/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("542", [
        "スターゲイザー(CL10)1匹、オーク(CL3)3匹",
        "スターゲイザー(CL10)1匹、ヘルハウンド(CL12)2匹",
        "コスモロジスト(CL20)1匹、ケルベロス(CL33)1匹",
        "サーバントマスター(CL35)1匹、ケルベロス(CL33)2匹",
        "スターゲイザー(CL10/オリジン)1匹、オーク(CL3/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("543", [
        "オウルセージ(CL5)1匹、インプ(CL3)3匹。指定エネミー:オウルセージ",
        "オウルセージ(CL5)3匹。指定エネミー:オウルセージ",
        "オウルコマンダー(CL18)1匹、キメラ(CL15)2匹。指定エネミー:オウルコマンダー",
        "オウルコマンダー(CL18)2匹、キメラグレート(CL60)1匹。指定エネミー:オウルコマンダー",
        "オウルセージ(CL/オリジン5)1匹、インプ(CL3/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)。指定エネミー:オウルセージ",
    ]),
    ("544", [
        "グリーンアクトレス(CL22)1匹",
        "魔将樹の騎士(CL40)1匹",
        "魔将樹の番人(CL80)1匹",
        "『魔将樹の剣姫』ロジエモール(CL120/ネームド)1匹",
        "グリーンアクトレス(CL22/オリジン)1匹(〔HP〕+200、あらゆる致傷力+200、分類「ネームド」と【始原の力】を追加)",
    ]),
    ("545", [
        "ポイズンリリー(CL7)1匹、リリーの尖兵(CL5)1匹",
        "ポイズンリリー(CL7)1匹、リリーの尖兵(CL5)1匹(〔命中〕+4、〔HP〕+30、〔FP〕+10、あらゆる致傷力+20)",
        "バリアリーフ(CL27)1匹、リリーの尖兵(CL5)1匹(〔命中〕+8、〔HP〕+60、〔FP〕+20、あらゆる致傷力+40)",
        "『密林の女王』クイーンリリー(CL70/ネームド)1匹、リリーの尖兵(CL5)1匹(〔命中〕+16、〔HP〕+120、〔FP〕+40、あらゆる致傷力+80)",
        "ポイズンリリー(CL7/オリジン)1匹、リリーの尖兵(CL5/オリジン)1匹(〔HP〕+800、あらゆる致傷力+100)",
    ]),
    ("546", [
        "ブラックナイト(CL10)1匹、ソルジャー(CL4)3匹",
        "ヴァルキリー(CL20)1匹、ブラックナイト(CL10)2匹",
        "フレイヤ(CL45)1匹、ヴァルキリー(CL20)2匹",
        "ジェネラル(CL55)1匹、フレイヤ(CL45)2匹",
        "ブラックナイト(CL10/オリジン)1匹、ソルジャー(CL4/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("641", [
        "ヤコ(CL6)3匹(【狐の送り火】のデータを「疲労:なし」に変更)",
        "ヨウコ(CL30)1匹(【狐の送り火】のデータを「疲労:なし」に変更)",
        "ヨウコ(CL30)3匹(【狐の送り火】のデータを「疲労:なし」に変更)",
        "タタリドラゴン(CL60/ネームド)1匹(【狐の送り火】のデータを「疲労:なし」に変更)",
        "ヤコ(CL6/オリジン)3匹((〔HP〕+500、あらゆる致傷力+100、【狐の送り火】のデータを「疲労:なし」に変更)",
    ]),
    ("642", [
        "スケルトン(CL3)4匹",
        "ナイトスケルトン(CL12)3匹",
        "ヒトキリ(CL33)2匹(「分類:アンデッド」を追加)",
        "堕ちた黒龍(CL48)2匹",
        "スケルトン(CL3/オリジン)4匹(〔HP〕+500、あらゆる致傷力+200)",
    ]),
    ("643", [
        "ポイズンリリー(CL7)1匹(【華劇:リリーオブザバレー】使用済み)、ファントム(CL5)2匹",
        "ポイズンリリー(CL7)1匹(【華劇:リリーオブザバレー】使用済み)、ゴースト(CL10)3匹",
        "グリーンアクトレス(CL22)1匹(【華劇:フォレストステップ】使用済み)、デュラハン(CL22)2匹",
        "グリーンアクトレス(CL22)1匹(【華劇:フォレストステップ】使用済み)、カラクリヒメショーグン(CL75)1匹",
        "ポイズンリリー(CL7/オリジン)1匹(【華劇:リリーオブザバレー】使用済み)、ファントム(CL5)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("644", [
        "君たちの冒険はここで終了だ。",
        "君たちの冒険はここで終了だ。",
        "君たちの冒険はここで終了だ。",
        "第二魔将『葬送者』グレイヴディガー(CL200/魔将)1匹、『ディガー様親衛隊長』フュネライユ(CL66/ネームド)1匹、『トラップマスター』アンテルモン(CL65/ネームド)1匹",
        "[現在エネミー未実装の為振り直ししてください]",
    ]),
    ("645", [
        "ゴースト(CL10)1匹(通常ドロップ「1000GP」)、ソルジャー(CL4)2匹",
        "ゴースト(CL10)1匹(通常ドロップ「2000GP」)、ハタモト(CL10)3匹",
        "リッチ(CL44)1匹(通常ドロップ「4000GP」)、ミスリルゴーレム(CL27)1匹",
        "エルダーリッチ(CL55)1匹(通常ドロップ「8000GP」)、アダマンゴーレム(CL45)1匹",
        "ゴースト(CL10/オリジン)1匹(通常ドロップ「1000GP」)、ソルジャー(CL4/オリジン)2匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("646", [
        "ボーンスネーク(CL20)1匹",
        "ボーンスネーク(CL20)2匹",
        "ラダマンティスの蛇(CL50)1匹",
        "ラダマンティスの蛇(CL50)1匹(ネームド仕様[HP4倍、【ネームドエネミー】習得])",
        "ボーンスネーク(CL20/オリジン)1匹(〔HP〕+1500、あらゆる致傷力+200)",
    ]),
    ("256", [
        "ボーンスネーク(CL50)1匹",
        "ラダマンティスの蛇(CL50)1匹",
        "銀河龍ラクテア/ドラゴン形態(CL90)1匹",
        "『三つ首の災厄』トライディザスター(CL100/ネームド)1匹",
        "ボーンスネーク(CL50/オリジン)1匹(〔HP〕+2000、あらゆる致傷力+200)",
    ]),
    ("265", [
        "キャンディークラウン(CL40)「1D6」匹",
        "キャンディークラウン(CL40)「1D6+1」匹",
        "キャンディークラウン(CL40)「1D6」匹、ゴールデンクラウン(CL177)1匹",
        "キャンディークラウン(CL40)「1D6」匹、ゴールデンクラウン(CL177)2匹",
        "キャンディークラウン(CL40/オリジン)「1D6」匹",
    ]),
    ("465", [
        "リビングデッド(CL5)3匹",
        "ゴースト(CL10)3匹",
        "デュラハン(CL22)3匹",
        "リッチ(CL44)3匹",
        "リビングデッド(CL5/オリジン)3匹(〔HP〕+500、あらゆる致傷力+100)",
    ]),
    ("665", [
        "カロン(CL12)2匹",
        "カロンキャプテン(CL25)2匹",
        "リッチ(CL44)2匹",
        "エルダーリッチ(CL55)2匹(〔HP〕+10)",
        "カロンキャプテン(CL25/オリジン)2匹",
    ]),
    ("666", [
        "『漆黒の戦鬼』ブラックナイト(CL10/ネームド)1匹",
        "『漆黒の戦鬼』ブラックナイト(CL10/ネームド)1匹(〔命中〕+4、あらゆる致傷力+20、〔HP〕+200)",
        "『白銀の猛将』ジェネラル(CL55/ネームド)1匹",
        "『白銀の猛将』ジェネラル(CL55/ネームド)1匹(〔命中〕+4、あらゆる致傷力+20、〔HP〕+500)",
        "[現在エネミー未実装の為振り直ししてください]",
    ]),
];

/// Ruby `TRESURE_TABLES`（財宝ランク 0〜12）。
static TRESURE_TABLES: [Table; 13] = [
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "HPポーション(消耗品)",
            "FPポーション(消耗品)",
            "マジックパウダー:火(消耗品)",
            "マジックパウダー:氷(消耗品)",
            "マジックパウダー:雷(消耗品)",
            "500GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "高級HPポーション(消耗品)",
            "高級FPポーション(消耗品)",
            "高級抵抗ポーション(消耗品)",
            "高級鉄壁ポーション(消耗品)",
            "マジックパウダー:火、氷、雷の3点セット(消耗品)",
            "1000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "鈴のお守り(装飾品)",
            "盗賊の小手(装飾品)",
            "狩人の羽帽子(装飾品)",
            "狙撃手の指貫(装飾品)",
            "「スタミナバンド」「健康お守り」「レザーマント」3点セット",
            "2000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "最高級HPポーション×2(消耗品)",
            "最高級FPポーション×2(消耗品)",
            "最高級抵抗ポーション×2(消耗品)",
            "任意の装飾品1つ(4000GPまでのもの)",
            "アタッチメント割引券(全員に1枚)",
            "3000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の「ミスリル」武器1つ",
            "ミスリルシールド(盾)",
            "ミスリルスケイル(鎧)",
            "任意の装飾品1つ(5000GPまでのもの)",
            "アタッチメント割引券(全員に2枚)",
            "5000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(10000GPまでのもの)",
            "任意の盾1つ(10000GPまでのもの)",
            "任意の鎧1つ(10000GPまでのもの)",
            "最高級HPポーション(人数分)",
            "任意の装飾品1つ(10000GPまでのもの)",
            "7500GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(15000GPまでのもの)",
            "任意の盾1つ(15000GPまでのもの)",
            "任意の鎧1つ(15000GPまでのもの)",
            "任意の装飾品1つ(15000GPまでのもの)",
            "最高級FPポーション(人数分)",
            "10000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(30000GPまでのもの)",
            "任意の盾1つ(30000GPまでのもの)",
            "任意の鎧1つ(30000GPまでのもの)",
            "任意の装飾品1つ(30000GPまでのもの)",
            "蘇生ポーション(消耗品)",
            "20000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(60000GPまでのもの)",
            "任意の盾1つ(60000GPまでのもの)",
            "任意の鎧1つ(60000GPまでのもの)",
            "任意の装飾品1つ(60000GPまでのもの)",
            "蘇生ポーション(装飾品)+アタッチメント割引券10枚(割引券は人数分)",
            "40000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(100000GPまでのもの)",
            "任意の盾1つ(100000GPまでのもの)",
            "任意の鎧1つ(100000GPまでのもの)",
            "任意の装飾品1つ(100000GPまでのもの)",
            "蘇生ポーション(装飾品)+アタッチメント割引券20枚(割引券は人数分)",
            "60000GP",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "任意の武器1つ(150000GPまでのもの)",
            "任意の盾1つ(150000GPまでのもの)",
            "任意の鎧1つ(150000GPまでのもの)",
            "任意の装飾品1つ(200000GPまでのもの)",
            "蘇生ポーション(装飾品)+アタッチメント割引券30枚(割引券は人数分)",
            "黄金の守護者の証(装飾品)(【ハッキング】があれば黄金の電子暗号キー(装飾品)も追加)",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "体力の欠片（大事なもの）(全員に10個)",
            "敏捷の欠片（大事なもの）(全員に10個)",
            "感覚の欠片（大事なもの）(全員に10個)",
            "知力の欠片（大事なもの）(全員に10個)",
            "意志の欠片（大事なもの）(全員に10個)",
            "お好きな副能力の欠片（大事なもの）(1人ずつ好きなものを選択して全員に50個)",
        ],
    ),
    Table::from_dice(
        "財宝表",
        1,
        6,
        &[
            "体力の欠片（大事なもの）(全員に20個)",
            "敏捷の欠片（大事なもの）(全員に20個)",
            "感覚の欠片（大事なもの）(全員に20個)",
            "知力の欠片（大事なもの）(全員に20個)",
            "意志の欠片（大事なもの）(全員に20個)",
            "お好きな副能力の欠片（大事なもの）(1人ずつ好きなものを選択して全員に100個)",
        ],
    ),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "FilledWith",
            "FilledWith.toml",
            47,
        );
    }
}
