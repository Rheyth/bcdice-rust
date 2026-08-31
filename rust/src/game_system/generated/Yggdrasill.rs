//! P4で手書き移植した `lib/bcdice/game_system/Yggdrasill.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Yggdrasill#roll_cf`（行為判定 `CFx+nD6`）と `count_critical` / `count_fumble`
//! - `roll_ra` / `roll_treat` / `roll_down` / `roll_cond` / `roll_guki` /
//!   `roll_cont` / `roll_allr` / `roll_pafe`
//! - `Yggdrasill::TABLES` と `Base#roll_tables`（各種表）
//! - `Yggdrasill::YggTable` / `Yggdrasill::ChainTable`（このファイル内の
//!   [`YggTable`] / [`YggChainTable`]。Ruby側もゲームシステム固有の内部クラスで、
//!   `BCDice::DiceTable::ChainTable` とは別物）

use std::sync::OnceLock;

use regex::Regex;

use crate::common_command::add_dice::AddDiceRandomizer;
use crate::dice_table::{
    D66ParityTable, D66Table, RangeInc, RangeTable, RollableTable, Table, TableItem,
};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `TABLES` の各項目。
///
/// Ruby側は種類の違う表を1つの `Hash` に入れて `roll(randomizer).to_s` するだけなので、
/// 「振って文字列にする」操作だけをこのトレイトでまとめる。
trait YggRollableTable: Sync {
    /// Ruby `table.roll(@randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError>;
}

impl YggRollableTable for Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(RollableTable::roll(self, rng)?.to_string())
    }
}

impl YggRollableTable for D66Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(RollableTable::roll(self, rng)?.to_string())
    }
}

impl YggRollableTable for RangeTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng)?.to_string())
    }
}

/// Ruby `Yggdrasill::YggTable`（`DiceTable::Table` のサブクラス）。
///
/// 出目が `additonal_index` に含まれるときは追加のダイスを振って本文へ足し、
/// `out_of_control` と一致するときは代わりに `RA90`（暴走状態表）を引く。
struct YggTable {
    /// 親クラス `DiceTable::Table` の部分
    table: Table,
    /// Ruby `additonal_type` のダイス個数
    additional_times: i64,
    /// Ruby `additonal_type` のダイス面数
    additional_sides: i64,
    /// Ruby `additonal_format`（`%{total}` / `%{list}` を含む）
    format: &'static str,
    /// Ruby `additonal_index`
    index: &'static [i64],
    /// Ruby `out_of_control`
    out_of_control: Option<i64>,
}

impl YggRollableTable for YggTable {
    /// Ruby `YggTable#roll`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_sum(self.table.times(), self.table.sides())?;
        let chosen = self.table.choice(value);

        // Ruby: `return chosen unless @index.include?(value) || @out_of_control == value`
        if !self.index.contains(&value) && self.out_of_control != Some(value) {
            return Ok(chosen.to_string());
        }

        let body = if self.out_of_control == Some(value) {
            format!(
                "{} ： {}",
                chosen.last_body(),
                RollableTable::roll(&RA90, rng)?
            )
        } else {
            let list = rng.roll_barabara(self.additional_times, self.additional_sides)?;
            let total: i64 = list.iter().sum();
            format!(
                "{}{}",
                chosen.last_body(),
                format_total_list(self.format, total, &join_comma(&list))
            )
        };

        // Ruby: `DiceTable::RollResult.new(chosen.table_name, chosen.value, body)` を `to_s`
        Ok(format!(
            "{}({}) ＞ {body}",
            chosen.table_name(),
            chosen.value()
        ))
    }
}

/// Ruby `Yggdrasill::ChainTable`（`DiceTable::Table` のサブクラス）。
///
/// 出目が `additonal_index` に含まれるときだけ別の表を続けて引く。
struct YggChainTable {
    /// 親クラス `DiceTable::Table` の部分
    table: Table,
    /// Ruby `additonal_table`
    additional_table: &'static Table,
    /// Ruby `additonal_index`
    index: &'static [i64],
}

impl YggRollableTable for YggChainTable {
    /// Ruby `ChainTable#roll`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let value = rng.roll_sum(self.table.times(), self.table.sides())?;
        let chosen = self.table.choice(value);

        if !self.index.contains(&value) {
            return Ok(chosen.to_string());
        }

        let body = format!(
            "{} ＞ {}",
            chosen.last_body(),
            RollableTable::roll(self.additional_table, rng)?
        );
        Ok(format!(
            "{}({}) ＞ {body}",
            chosen.table_name(),
            chosen.value()
        ))
    }
}

/// Ruby `format(@format, total: ..., list: ...)`。
///
/// `additonal_format` に現れる名前付き参照は `%{total}` と `%{list}` の2つだけ。
fn format_total_list(template: &str, total: i64, list: &str) -> String {
    template
        .replace("%{total}", &total.to_string())
        .replace("%{list}", list)
}

/// Ruby `Array#join(",")`。
fn join_comma(values: &[i64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Yggdrasill#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    Ok(eval_specific_text(command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `eval_game_system_specific_command` の本体（`||` で並ぶ各コマンド）。
///
/// `roll_cf` が `TREAT` を再帰的に呼ぶので、文字列を返す形で切り出してある。
fn eval_specific_text(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if let Some(text) = roll_tables(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_cf(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_ra(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_treat(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_down(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_cond(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_guki(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_cont(command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = roll_allr(command, rng)? {
        return Ok(Some(text));
    }
    roll_pafe(command, rng)
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll_text(rng)?))
}

// ----- 行為判定 (CF) -------------------------------------------------------

/// Ruby `roll_cf` の
/// `/^(H)?CF([LG])?(T)?((?:[+-]*\d+|\+?\d+D\d+)(?:[+-]+\d+|\++\d+D\d+)*)$/`。
///
/// マイナス補正にダイスロールを用いることはシステム上ありえないので、
/// 2項目以降のダイス項は `+` でしかつなげない。
fn cf_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(H)?CF([LG])?(T)?((?:[+-]*\d+|\+?\d+D\d+)(?:[+-]+\d+|\++\d+D\d+)*)$")
            .expect("valid regex")
    })
}

/// `roll_cf` の式を項に切るための正規表現（`符号の並び` + `定数` または `nDm`）。
fn cf_term_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([+-]*)(\d+)(?:D(\d+))?").expect("valid regex"))
}

/// `roll_cf` の式の項。
struct CfTerm<'a> {
    /// 項の前に書かれた符号の並び
    signs: &'a str,
    /// ダイス項なら `(個数, 面数)`、定数項なら `None`
    dice: Option<(i64, i64)>,
    /// 定数項の値（ダイス項では使わない）
    number: i64,
}

/// Ruby `CommonCommand::AddDice::Parser.parse(expr)` + `Node#eval` / `Node#output` の代替。
///
/// `AddDice` の構文木は `Node#eval` / `Node#output` がクレート内部に閉じていて
/// 外から呼べないため、`roll_cf` の正規表現が許す平坦な文法だけをここで解釈する。
/// 符号の扱いは `add_dice/parser.y` の `unary` 規則に合わせてある
/// （`PLUS unary` は符号を落とし、`MINUS unary` は `Negate` 同士を打ち消す）。
fn split_cf_terms(expr: &str) -> Option<Vec<CfTerm<'_>>> {
    let mut terms = Vec::new();
    let mut pos = 0;
    for caps in cf_term_pattern().captures_iter(expr) {
        let whole = caps.get(0)?;
        // 正規表現が許す形しか来ないが、取りこぼしがあれば式全体を諦める
        if whole.start() != pos {
            return None;
        }
        pos = whole.end();

        // 桁あふれは Ruby だと Bignum になる。飽和させてダイス個数の上限側へ倒す。
        let number: i64 = caps[2].parse().unwrap_or(i64::MAX);
        let dice = caps
            .get(3)
            .map(|s| (number, s.as_str().parse::<i64>().unwrap_or(i64::MAX)));

        terms.push(CfTerm {
            signs: caps.get(1)?.as_str(),
            dice,
            number,
        });
    }

    (pos == expr.len() && !terms.is_empty()).then_some(terms)
}

/// `roll_cf` の式を評価する。戻り値は `(達成値, Node#output 相当, 振った出目)`。
fn eval_cf_expr(
    terms: &[CfTerm<'_>],
    rng: &mut Randomizer,
) -> Result<(i64, String, Vec<i64>), EvalError> {
    // `AddDice::Randomizer` を通すことで D66・D9・`@sort_add_dice` の扱いを本家と揃える。
    let mut add_rng = AddDiceRandomizer::new(rng, &Yggdrasill);

    let mut total = 0i64;
    let mut output = String::new();
    let mut rand_values: Vec<i64> = Vec::new();

    for (i, term) in terms.iter().enumerate() {
        // 先頭以外は最初の符号が二項演算子、残りが単項の符号。
        let (binary_op, unary_signs) = if i == 0 {
            ("", term.signs)
        } else {
            term.signs.split_at(1)
        };
        // Ruby `unary: MINUS unary` は `Negate` の入れ子を潰すので、符号は偶奇だけが効く。
        let negate = unary_signs.matches('-').count() % 2 == 1;

        let (value, text) = match term.dice {
            Some((times, sides)) => {
                let dice_list = add_rng.roll(times.into(), sides.into())?;
                let sum = dice_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));
                rand_values.extend(dice_list.iter().copied());
                // Ruby `Node::DiceRoll#output` は `"#{total}[#{dice_list.join(',')}]"`
                (sum, format!("{sum}[{}]", join_comma(&dice_list)))
            }
            // Ruby `Node::Number#output` は `literal.to_s`
            None => (term.number, term.number.to_string()),
        };

        let signed = if negate { value.wrapping_neg() } else { value };
        total = if i == 0 {
            signed
        } else if binary_op == "+" {
            total.wrapping_add(signed)
        } else {
            total.wrapping_sub(signed)
        };

        output.push_str(binary_op);
        if negate {
            output.push('-');
        }
        output.push_str(&text);
    }

    Ok((total, output, rand_values))
}

/// Ruby `Yggdrasill#count_critical`。
fn count_critical(dice_list: &[i64], lucky_state: Option<&str>) -> i64 {
    let threshold = match lucky_state {
        Some("G") => 4,
        Some(_) => 5,
        None => 6,
    };
    dice_list.iter().filter(|x| **x >= threshold).count() as i64
}

/// Ruby `Yggdrasill#count_fumble`。
fn count_fumble(dice_list: &[i64], lucky_state: Option<&str>) -> i64 {
    let threshold = match lucky_state {
        Some("G") => 3,
        Some(_) => 2,
        None => 1,
    };
    dice_list.iter().filter(|x| **x <= threshold).count() as i64
}

/// Ruby `Yggdrasill#roll_cf`。
fn roll_cf(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = cf_pattern().captures(command) else {
        return Ok(None);
    };

    let half = m.get(1).is_some();
    let lucky_state = m.get(2).map(|s| s.as_str());
    let treat_flag = m.get(3).is_some();

    // Ruby: `return nil unless node`
    let Some(terms) = split_cf_terms(&m[4]) else {
        return Ok(None);
    };
    let (total, expr_output, rand_values) = eval_cf_expr(&terms, rng)?;

    let n1 = count_fumble(&rand_values, lucky_state);
    let n_max = count_critical(&rand_values, lucky_state);

    // ファンブルロール
    let fa_list = rng.roll_barabara(n1, 6)?;
    let fa1: i64 = fa_list.iter().sum();
    let fa2 = join_comma(&fa_list);

    // クリティカルの振り足し（6が出るかぎり続く）
    let mut critical_rerolls: Vec<Vec<i64>> = Vec::new();
    let mut rerolls = n_max;
    while rerolls > 0 {
        let list = rng.roll_barabara(rerolls, 6)?;
        rerolls = list.iter().filter(|x| **x == 6).count() as i64;
        critical_rerolls.push(list);
    }
    let cr1: i64 = critical_rerolls.iter().flatten().sum();
    let cr2: String = critical_rerolls
        .iter()
        .map(|x| format!("[{}]", join_comma(x)))
        .collect();
    let cr_count = critical_rerolls.iter().flatten().count();

    // 修正値&一投目出目 -ファンブル +クリティカル
    let mut total_n = total - fa1 + cr1;
    if half {
        // Ruby `Integer#/` は床除算
        total_n = (total_n).div_euclid(2);
    }

    let mut text = format!("({command}) ＞ 計【 {total_n} 】 ： {expr_output}");
    if n1 > 0 {
        text.push_str(&format!(" (fa:{n1})-{fa1}[{fa2}]"));
    }
    if cr1 > 0 {
        text.push_str(&format!(" (cr:{n_max})+{cr1}{cr2} (cr:計{cr_count}回)"));
    }

    if treat_flag {
        // Ruby: `heal = eval_game_system_specific_command("TREAT#{total_n}")`
        let heal = eval_specific_text(&format!("TREAT{total_n}"), rng)?.unwrap_or_default();
        text.push_str(&format!("\n ＞ {heal}"));
    }

    Ok(Some(text))
}

// ----- 暴走ロール (RA) -----------------------------------------------------

/// Ruby `roll_ra` の `/^RA(\d+)?$/`。
fn ra_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^RA(\d+)?$").expect("valid regex"))
}

/// Ruby `Yggdrasill#roll_ra`。
fn roll_ra(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = ra_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: `m[1]&.to_i`（数値なしなら nil）
    let rate = m
        .get(1)
        .map(|s| s.as_str().parse::<i64>().unwrap_or(i64::MAX));

    let body = match rate {
        Some(50) => RA50.roll_text(rng)?,
        Some(70) => RA70.roll_text(rng)?,
        Some(90) => RollableTable::roll(&RA90, rng)?.to_string(),
        Some(110 | 120 | 130 | 140) => RA110.roll_text(rng)?,
        Some(150) => "＞ 因子崩壊【キャラロスト】".to_owned(),
        None => "＞ このコマンドは数値を付けてください".to_owned(),
        Some(_) => "＞ 指定の暴走率の暴走ロールはありません".to_owned(),
    };

    Ok(Some(format!("({command}) {body}")))
}

// ----- 【応急処置】 (TREAT) ------------------------------------------------

/// Ruby `roll_treat` の `/^TREAT(-?\d+)?$/`。
fn treat_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^TREAT(-?\d+)?$").expect("valid regex"))
}

/// Ruby `Yggdrasill#roll_treat`。
fn roll_treat(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = treat_pattern().captures(command) else {
        return Ok(None);
    };

    let Some(captured) = m.get(1) else {
        return Ok(Some(
            "ＡＥ【応急処置】 ＞ このコマンドは数値を付けてください".to_owned(),
        ));
    };

    // 桁あふれは Ruby だと Bignum になるので、符号に応じて端へ飽和させる。
    let raw = captured.as_str();
    let value: i64 = raw.parse().unwrap_or(if raw.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    });

    let recovery = if value <= 5 {
        "0".to_owned()
    } else if value <= 7 {
        "1".to_owned()
    } else if value <= 11 {
        let dice = rng.roll_once(6)?;
        // Ruby `Integer#/` は床除算
        let total = (dice).div_euclid(2);
        format!("{total}({dice}[{dice}]/2)")
    } else if value <= 14 {
        let dice = rng.roll_once(6)?;
        format!("{dice}({dice}[{dice}])")
    } else if value <= 17 {
        let dice = rng.roll_once(6)?;
        format!("{}({dice}[{dice}]+3)", dice + 3)
    } else {
        let list = rng.roll_barabara(2, 6)?;
        let dice: i64 = list.iter().sum();
        format!("{}({dice}[{}]+2)", dice + 2, join_comma(&list))
    };

    Ok(Some(format!("ＡＥ【応急処置】 ＞ HPが{recovery}回復")))
}

// ----- その他の判定 --------------------------------------------------------

/// Ruby `Yggdrasill#roll_down`（気絶判定）。
fn roll_down(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "DOWN" {
        return Ok(None);
    }

    let dice = rng.roll_once(6)?;
    let result = if dice % 2 == 0 {
        "回避".to_owned()
    } else {
        let fell = rng.roll_once(6)?;
        format!("気絶【{fell}R行動不能】")
    };

    Ok(Some(format!("気絶判定 ＞ {dice} ＞ {result}")))
}

/// Ruby `Yggdrasill#roll_cond`（コンディションロール）。
fn roll_cond(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "COND" {
        return Ok(None);
    }

    let hp_list = rng.roll_barabara(2, 6)?;
    let hp_total: i64 = hp_list.iter().sum();

    let pp_list = rng.roll_barabara(2, 6)?;
    let pp_total: i64 = pp_list.iter().sum();

    Ok(Some(format!(
        "({command}) ＞ HP{hp_total}[{}] 、 PP{pp_total}[{}]",
        join_comma(&hp_list),
        join_comma(&pp_list)
    )))
}

/// Ruby `Yggdrasill#roll_guki`（偶奇判定）。
fn roll_guki(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "GUKI" {
        return Ok(None);
    }

    let dice = rng.roll_once(6)?;
    let result = if dice % 2 == 0 { "成功" } else { "失敗" };

    Ok(Some(format!("(GUKI) ＞ {dice} ＞ {result}")))
}

/// Ruby `roll_cont` の `/CO(NT)?/`。Ruby側は行頭・行末を固定していない部分一致。
fn cont_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"CO(NT)?").expect("valid regex"))
}

/// Ruby `Yggdrasill#roll_cont`（コンティニュー判定）。
fn roll_cont(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if !cont_pattern().is_match(command) {
        return Ok(None);
    }

    let dice = rng.roll_once(6)?;
    let text = if dice <= 4 { "1R追加" } else { "2R追加" };

    Ok(Some(format!("コンティニュー判定 ＞ {dice} ＞ {text}")))
}

/// Ruby `Yggdrasill#roll_allr`（オールレンジ発動ロール）。
fn roll_allr(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "ALLR" {
        return Ok(None);
    }

    let dice = rng.roll_once(6)?;
    let text = if dice == 1 {
        "発動失敗【技対象が敵味方含めた全員となる】"
    } else {
        "発動成功"
    };

    Ok(Some(format!("オールレンジ判定 ＞ {dice} ＞ {text}")))
}

/// Ruby `Yggdrasill#roll_pafe`（パーフェクト発動ロール）。
fn roll_pafe(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "PAFE" {
        return Ok(None);
    }

    let dice = rng.roll_once(6)?;
    let text = if dice == 1 {
        "発動失敗【通常命中・回避判定となり、発動時のアクション内の命中力＆回避力が半減する】"
    } else {
        "発動成功"
    };

    Ok(Some(format!("発動ロール ＞ {dice} ＞ {text}")))
}

/// Ruby `PSY_TABLE`。
static PSY_TABLE: Table = Table::from_dice(
    "能力タイプ",
    1,
    6,
    &[
        "サイキッカー",
        "エスパー",
        "トランサー",
        "クリエイター",
        "アンノウン",
        "好きな能力タイプを選択。ノーマル選択でも可",
    ],
);

/// Ruby `RA50`。
static RA50: YggTable = YggTable {
    table: Table::from_dice(
        "暴走Lv.1",
        1,
        6,
        &[
            "発作【自爆÷2ダメージ。（自身に能力攻撃ロールダメージ÷2）。防御無視】",
            "高揚【1D6暴走率上昇】",
            "高揚【1D6暴走率上昇】",
            "自制【暴走なし】",
            "自制【暴走なし】",
            "自制【暴走なし】",
        ],
    ),
    additional_times: 1,
    additional_sides: 6,
    format: " ： %{total}[%{list}] ％",
    index: &[2, 3],
    out_of_control: None,
};

/// Ruby `RA70`。
static RA70: YggTable = YggTable {
    table: Table::from_dice(
        "暴走Lv.3",
        1,
        6,
        &[
            "自爆【自爆ダメージ。自身に能力攻撃ロールダメージ。防御無視】",
            "自爆【自爆ダメージ。自身に能力攻撃ロールダメージ。防御無視】",
            "暴発【ランダム攻撃。基本的に能力攻撃。対象は自分、キャラ、オブジェクトの三種類】",
            "連鎖【2D6暴走率上昇】",
            "発症",
            "自制【暴走無し】",
        ],
    ),
    additional_times: 2,
    additional_sides: 6,
    format: " ： %{total}[%{list}] ％",
    index: &[4],
    out_of_control: Some(5),
};

/// Ruby `RA110`。
static RA110: YggTable = YggTable {
    table: Table::from_dice(
        "臨界ロール",
        1,
        6,
        &[
            "自壊【自爆ダメージ。自身の最も高い攻撃ロールダメージ。防御無視】",
            "超活性【HP・PPを2D6回復】",
            "自壊【自爆ダメージ。自身の最も高い攻撃ロールダメージ。防御無視】",
            "超活性【HP・PPを2D6回復】",
            "自壊【自爆ダメージ。自身の最も高い攻撃ロールダメージ。防御無視】",
            "超活性【HP・PPを2D6回復】",
        ],
    ),
    additional_times: 2,
    additional_sides: 6,
    format: " ： %{total}[%{list}] 回復",
    index: &[2, 4, 6],
    out_of_control: None,
};

/// Ruby `RA90` の左ダイス奇数側。
static RA90_ODD: &[&str] = &[
    "能力異常【能力使用時に偶奇判定。奇数の場合は消費だけ行い能力発動失敗。暴走チェックごとに+2％される（発症時も発生）。能力精度の判定結果が半減】",
    "言語異常【AE使用時に偶奇判定。奇数の場合は消費だけ行いAE発動失敗。話術の判定結果が半減】",
    "記憶異常【命中判定結果が半減する。知識の判定結果が半減】",
    "精神異常【自分のリアクション（回避判定など）で偶奇判定。奇数の場合は行動自動失敗。隠密、読心の判定結果が半減】",
    "忘我【自プリアクション時に偶奇判定。奇数の場合は宣言せずにターン終了。あらゆる技能判定結果が半減】",
    "自制【暴走無し】",
];
/// Ruby `RA90` の左ダイス偶数側。
static RA90_EVEN: &[&str] = &[
    "制御異常【自プリアクション毎（行動決定前）に偶奇判定。奇数の場合は暴発によるランダム攻撃。（発症時も発生）。技術、幸運の判定結果が半減】",
    "過負荷【ワンアクション毎に能力精度÷3の防御無視ダメージ（発症時も発生）。閃きの判定結果が半減】",
    "聴覚異常【回避判定結果が半減する。察知の半減結果が半減】",
    "視覚異常【SS＆命中力＆回避力が半減する※判定結果は半減しない。観察眼の判定結果が半減】",
    "身体異常【防御を差し引く前のダメージロールが半減する。力技、俊敏の判定結果が半減】",
    "自制【暴走なし】",
];
/// Ruby `RA90`。
static RA90: D66ParityTable = D66ParityTable::new("暴走状態表", RA90_ODD, RA90_EVEN);

/// Ruby `TABLES["MIKUZI"]`。
static TABLE_MIKUZI: RangeTable = RangeTable::from_dice(
    "おみくじ",
    1,
    100,
    &[
        (RangeInc::new(1, 17), "大吉"),
        (RangeInc::new(18, 52), "吉"),
        (RangeInc::new(53, 57), "半吉"),
        (RangeInc::new(58, 61), "小吉"),
        (RangeInc::new(62, 64), "末小吉"),
        (RangeInc::new(65, 70), "末吉"),
        (RangeInc::new(71, 100), "凶"),
    ],
);

/// Ruby `TABLES["SO1"]`。
static TABLE_SO1: YggTable = YggTable {
    table: Table::from_dice(
        "SOペナルティ表 1オーバー",
        1,
        6,
        &[
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "反動",
            "反動",
            "制御成功【発動成功　ペナルティ無し】",
        ],
    ),
    additional_times: 1,
    additional_sides: 6,
    format: "【命中＆回避－１Ｄ６（%{total}[%{list}]）　１ラウンド継続】",
    index: &[4, 5],
    out_of_control: None,
};

/// Ruby `TABLES["SO2"]`。
static TABLE_SO2: YggTable = YggTable {
    table: Table::from_dice(
        "SOペナルティ表 2オーバー",
        1,
        6,
        &[
            "自爆【自分へ能力攻撃ダメージ　※防御無視】",
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "反動",
            "反動",
            "制御成功【発動成功　ペナルティ無し】",
        ],
    ),
    additional_times: 1,
    additional_sides: 6,
    format: "【命中＆回避－１Ｄ６（%{total}[%{list}]）　１ラウンド継続】",
    index: &[4, 5],
    out_of_control: None,
};

/// Ruby `TABLES["SO3"]`。
static TABLE_SO3: YggTable = YggTable {
    table: Table::from_dice(
        "SOペナルティ表 3オーバー",
        1,
        6,
        &[
            "自爆【自分へ能力攻撃ダメージ　※防御無視】",
            "自爆【自分へ能力攻撃ダメージ　※防御無視】",
            "消費負荷【ＰＰ２倍消費　※ＡＥ消費は含まない】",
            "過反動",
            "過反動",
            "制御成功【発動成功　ペナルティ無し】",
        ],
    ),
    additional_times: 2,
    additional_sides: 6,
    format: "【命中＆回避－２Ｄ６（%{total}[%{list}]）　１ラウンド継続】",
    index: &[4, 5],
    out_of_control: None,
};

/// Ruby `TABLES["SO4"]`。
static TABLE_SO4: YggTable = YggTable {
    table: Table::from_dice(
        "SOペナルティ表 4オーバー",
        1,
        6,
        &[
            "崩壊【自爆ダメージ×２　※防御無視】",
            "崩壊【自爆ダメージ×２　※防御無視】",
            "超負荷【ＰＰ３倍消費　※ＡＥ消費は含まない】",
            "過反動",
            "過反動",
            "制御成功【発動成功　ペナルティ無し】",
        ],
    ),
    additional_times: 2,
    additional_sides: 6,
    format: "【命中＆回避－２Ｄ６（%{total}[%{list}]）　１ラウンド継続】",
    index: &[4, 5],
    out_of_control: None,
};

/// Ruby `TABLES["SO5"]`。
static TABLE_SO5: Table = Table::from_dice(
    "SOペナルティ表 5オーバー",
    1,
    6,
    &[
        "崩壊【自爆ダメージ×２　※防御無視】",
        "崩壊【自爆ダメージ×２　※防御無視】",
        "崩壊【自爆ダメージ×２　※防御無視】",
        "超負荷【ＰＰ３倍消費　※ＡＥ消費は含まない】",
        "超負荷【ＰＰ３倍消費　※ＡＥ消費は含まない】",
        "制御成功【発動成功　ペナルティ無し】",
    ],
);

/// Ruby `TABLES["RISK"]`。
static TABLE_RISK: Table = Table::from_dice(
    "リスク判定",
    1,
    6,
    &[
        "能力自爆【能力は発動せず、ＰＰを２倍消費する。併用ＡＥのＰＰは含まない。それに加え【自爆】する。能力攻撃力分を自身へ防御無視ダメージ】",
        "能力不発【能力は発動せず、ＰＰを２倍消費する。併用ＡＥのＰＰは含まない】",
        "効果不発【リスクの効果はゼロで能力発動】",
        "通常発動【（能力精度÷３）＋１Ｄ６を加える】",
        "活性発動【（能力精度÷３）＋２Ｄ６を加える】",
        "覚醒発動【（能力精度÷３）＋３Ｄ６を加える】",
    ],
);

/// Ruby `TABLES["FATAL1"]`。
static TABLE_FATAL1: Table = Table::from_dice(
    "後遺症判定",
    1,
    6,
    &[
        "聴覚崩壊【聴覚に異常が起きる。幻聴、難聴、失聴、など】",
        "視覚崩壊【視覚に異常が起こる。幻覚、色盲、失明、など】",
        "言語崩壊【言語の認識に異常が起きる。しゃべる事に支障をきたす。吃音、失語症、失読症、など】",
        "身体崩壊【身体に異常が起こる。欠損、異形化、麻痺、など】",
        "精神崩壊【精神に異常が起こる。人格破綻、性格変化、妄想・幻覚による異常行動、など】",
        "記憶崩壊【記憶に異常が起こる。記憶障害、記憶喪失、など】",
    ],
);

/// Ruby `TABLES["FATAL2"]`。
static TABLE_FATAL2: YggChainTable = YggChainTable {
    table: Table::from_dice(
        "因子変化判定",
        1,
        6,
        &[
            "能力変化【能力がまったく別ものに変化する】",
            "能力変化【能力がまったく別ものに変化する】",
            "因子抑制【能力変化は起こらない】",
            "因子抑制【能力変化は起こらない】",
            "能力喪失・能力覚醒【能力を持つものは失い、ノーマルは能力に覚醒する。喪失者はノーマルのキャラ特性ポイントを1p獲得する。覚醒者はノーマルのキャラ特性ポイントを1p失い、キャラ特性を6つ取得していた場合は1つ喪失する】",
            "能力喪失・能力覚醒【能力を持つものは失い、ノーマルは能力に覚醒する。喪失者はノーマルのキャラ特性ポイントを1p獲得する。覚醒者はノーマルのキャラ特性ポイントを1p失い、キャラ特性を6つ取得していた場合は1つ喪失する】",
        ],
    ),
    additional_table: &PSY_TABLE,
    index: &[1, 2, 5, 6],
};

/// Ruby `TABLES["STAG"]`。
static TABLE_STAG: D66Table = D66Table::new(
    "ステージ決定",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("ロシアンルーレット【幸運にて判定。参加者は銃をこめかみにあて、１発の銃弾をひかないように祈る。 敗者は３Ｄ６ダメージ】")),
        (12, TableItem::Text("チキンレース【察知にて判定。に向ってバイクでダッシュだ。敗者は２Ｄ６ダメージ。落ちても大丈夫です、電脳だから】")),
        (13, TableItem::Text("取り立て【力技or威圧にて判定。あのモヒカン借金払わないんですよ。よろしくお願いしますね。電脳を通しての実際の取り立てらしい】")),
        (14, TableItem::Text("舌戦【威圧or話術にて判定。参加者同士で舌戦で勝者を決めろ！敗者は心に２Ｄ６ダメージ】")),
        (15, TableItem::Text("ギャンブル【読心or幸運にて判定。ポーカー、ルーレット、麻雀、好きなものを選べ。勝利の鍵は運か、それとも人の心か】")),
        (16, TableItem::Text("トラップ【ＳＳにて判定。君達の目の前に広がるのはそう、地雷原だ。敗者は３Ｄ６ダメージ】")),
        (21, TableItem::Text("サバゲー【隠密or俊敏にて判定。軍人となって、相手を屠れ！敗者は死ぬ。敗者は２Ｄ６ダメージ】")),
        (22, TableItem::Text("追跡【察知or隠密にて判定。ニンジャの姿となって下手人を追え！コアな人気を誇るステージ。ニンジャ人気すごい】")),
        (23, TableItem::Text("推理【閃きにて判定。あなたたちは探偵となり、事件を解決に導く。犯人は、お前だ！２時間放送になるのが玉に瑕】")),
        (24, TableItem::Text("潜入【隠密にて判定。スパイとなり、機密情報を盗め！あれ、これ実際の企業の機密情報じゃ・・・？】")),
        (25, TableItem::Text("かくれんぼ【隠密or読心にて判定。あなたを追うのはホラーな化け物・・・。スリリングなかくれんぼをどうぞ堪能下さい】")),
        (26, TableItem::Text("絶対絶命！【回避力にて判定。君達はマフィアにおびき出されたのだ。大勢の銃が君を狙う。敗者は３Ｄ６ダメージ】")),
        (31, TableItem::Text("クイズ【知識にて判定。己の知識を存分に披露しろ！負けたら奈落に落されます。敗者は１Ｄ６ダメージ】")),
        (32, TableItem::Text("迷路【察知or幸運にて判定。巨大迷路をクリアしろ！あれ、なんでこんなところに骸骨が・・・】")),
        (33, TableItem::Text("パズル【知識or閃きにて判定。３Ｄの難解パズルを解き明かせ！！時折金庫破りのパスワードがターゲットになってたり】")),
        (34, TableItem::Text("間違い探し【観察眼or閃きにて判定。大量の鍵から正しい鍵を。美女の中からオカマを。そんな間違いを見つけるのだ！】")),
        (35, TableItem::Text("目利き【観察眼or知識にて判定。あなたの鑑定で値段を当てろ！はずれたらかっこ悪いです】")),
        (36, TableItem::Text("スナイパー【命中力にて判定。一撃必殺でターゲットを仕留めろ！なお、ターゲットはお互いだ。敗者は２Ｄ６ダメージ】")),
        (41, TableItem::Text("腕相撲【力技にて判定。必要なのは、力のみ！！敗者は２Ｄ６ダメージ】")),
        (42, TableItem::Text("インディジョーンズ【俊敏にて判定。なぜか大岩が後ろから！逃げろー！敗者は３Ｄ６ダメージ】")),
        (43, TableItem::Text("ＰＫ【力技or察知にて判定。見極め、ゴールしろ！パワーで破ってもいい】")),
        (44, TableItem::Text("ダンス【技術or俊敏にて判定。己の舞を魅せろ！ジャンル問わず】")),
        (45, TableItem::Text("ボディコンテスト【威圧にて判定。魅せるのはマッスルか、それとも美しい肢体か！容姿ボーナスはつきません】")),
        (46, TableItem::Text("突破しろ！【ダメージ量にて判定。立ちはだかる扉をぶち破れ！扉は防御１０】")),
        (51, TableItem::Text("早食い【力技or俊敏にて判定。くって！くって！！くいまくれ！！敗者は胃に２Ｄ６ダメージ】")),
        (52, TableItem::Text("ナンパ天国【話術or読心にて判定。電脳世界で老若男女を口説き落せ！相手はプログラムだったり電脳に入っているアバターだったり】")),
        (53, TableItem::Text("スリーサイズ【観察眼にて判定。魅惑のボディをなめまわせ！勝利者はある意味で尊敬され、ある意味で嫌われる】")),
        (54, TableItem::Text("ワサビ寿司【観察眼or幸運にて判定。高級寿司の中に、死ぬほどの刺激が・・・！敗者は２Ｄ６ダメージ】")),
        (55, TableItem::Text("じゃんけん【読心にて判定。じゃんけんとは運ではない、読み合いなのだ！】")),
        (56, TableItem::Text("瓦割り【ダメージ量にて判定。どんな方法でもいい。とにかく枚数を割れ！！！ダメージ量の２倍くらいが割った枚数】")),
        (61, TableItem::Text("料理対決【知識or技術にて判定。胃袋をつかめ！絶品料理対決！料理によってはＲ１８Ｇ指定になる場合がある】")),
        (62, TableItem::Text("歌合戦【威圧or技術にて判定。その歌唱力で心をつかめ！アイドルデビューも夢じゃない！電脳なのでお好きな衣装でどうぞ】")),
        (63, TableItem::Text("漫才【話術or閃きにて判定。即興漫才で画面の向こうを爆笑の渦へ！相方が必要な方は漫才プログラムアバターをレンタル。有料】")),
        (64, TableItem::Text("画伯【技術にて判定。テーマをもとに、あなたの画力を見せつけろ！時々下手うまな人が勝つことも】")),
        (65, TableItem::Text("プレゼンテーション【話術にて判定。本日の商品は、こちら！！実際に販売します。してもらいます】")),
        (66, TableItem::Text("無双撃破！【ダメージ量にて判定。た、大量のモヒカンだぁ～！ダメージ量の２倍くらいが倒した数。敗者は２Ｄ６ダメージ。ＳＥ【オールレンジ】技は成功で判定＋１０】")),
    ],
);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &dyn YggRollableTable)] = &[
    ("MIKUZI", &TABLE_MIKUZI),
    ("SO1", &TABLE_SO1),
    ("SO2", &TABLE_SO2),
    ("SO3", &TABLE_SO3),
    ("SO4", &TABLE_SO4),
    ("SO5", &TABLE_SO5),
    ("RISK", &TABLE_RISK),
    ("FATAL1", &TABLE_FATAL1),
    ("FATAL2", &TABLE_FATAL2),
    ("STAG", &TABLE_STAG),
];

/// Ruby `BCDice::GameSystem::Yggdrasill`（ID: `Yggdrasill`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Yggdrasill;

impl GameSystem for Yggdrasill {
    fn id(&self) -> &'static str {
        "Yggdrasill"
    }

    fn name(&self) -> &'static str {
        "鋼鉄のユグドラシル"
    }

    fn sort_key(&self) -> &'static str {
        "こうてつのゆくとらしる"
    }

    fn help_message(&self) -> &'static str {
        r"■ 行為判定 (CFx+nD6)
  クリティカルとファンブルによるダイス追加を行う
  先頭のcfを変更することで、動作が変更される
  hcf: 達成値が半減
  cfl: 付加効果【幸運】を付与
  cfg: 付加効果【ギャンブル】を付与
  cft: 【応急処置】判定 (tは末尾に記入してください)
  例）
    CF10+1D6, HCFL6+2D6, CFG11+1D6-2, cfgt10+1D6

■ 暴走ロール (RAx)
  暴走率xの暴走ロールおよび臨界ロールを行う
  例）
    RA50, RA110, RA150

■ SOペナルティ表 (SOx)
  スペック数がxオーバーした際のペナルティロールを行う
  例）
    SO1, SO5

■ 【応急処置】 (TREATx)
  達成値xの【応急処置】による回復量を決定する
  例）
    TREAT1, TREAT18

■ その他の判定および表
  down：気絶判定
  cont：コンティニュー判定
  risk：リスク判定
  guki：偶奇判定
  cond：コンディションロール
  allr：オールレンジ発動ロール
  pafe：パーフェクト発動ロール
  stag：ステージ決定（電脳ロワイヤル用）
  fatal1：後遺症
  fatal2：因子変化ロール
  mikuzi：浅草寺みくじ。1d100であなたの運勢を占います
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "H?CF", "RA", "SO", "DOWN", "CO(NT)?", "RISK", "GUKI", "COND", "TREAT", "ALLR", "PAFE",
            "FATAL", "STAG", "MIKUZI",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Yggdrasill#eval_game_system_specific_command`。
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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/Yggdrasill.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Yggdrasill.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Yggdrasill.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Yggdrasill.toml must parse");
        assert_eq!(
            data.tests.len(),
            173,
            "case count in test/data/Yggdrasill.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Yggdrasill",
                "unexpected game system in Yggdrasill.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Yggdrasill"), &tc.input, &mut src) {
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
                    "FAIL Yggdrasill:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Yggdrasill cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// `test/data/Yggdrasill.toml` が通らない分岐を押さえる。
    ///
    /// - `HCF`（達成値の半減）は Ruby の `Integer#/` が床除算なので、
    ///   達成値が負のときに Rust の `/`（0方向切り捨て）と結果が分かれる
    /// - `RA` の数値なし・未定義の暴走率
    /// - 負の達成値から呼ばれる `TREAT`（`roll_cf` の `T` 付きで実際に起きる）
    #[test]
    fn branches_not_covered_by_toml() {
        fn eval(input: &str, rands: Vec<(i64, i64)>) -> Option<String> {
            let mut src = SeededRandomizer::new(rands);
            let out = eval_command(&GameSystemId::new("Yggdrasill"), input, &mut src)
                .expect("must not error");
            assert!(src.is_empty(), "unconsumed rands for {input}");
            out.map(|r| r.text)
        }

        // 達成値 1 - ファンブル 6 = -5。Ruby の -5 / 2 は -3（床除算）。
        assert_eq!(
            eval("hcf1d6", vec![(1, 6), (6, 6)]).as_deref(),
            Some("(HCF1D6) ＞ 計【 -3 】 ： 1[1] (fa:1)-6[6]")
        );
        assert_eq!(
            eval("ra", vec![]).as_deref(),
            Some("(RA) ＞ このコマンドは数値を付けてください")
        );
        assert_eq!(
            eval("ra60", vec![]).as_deref(),
            Some("(RA60) ＞ 指定の暴走率の暴走ロールはありません")
        );
        // 負の達成値でも TREAT の正規表現（`-?\d+`）に一致する
        assert_eq!(
            eval("treat-3", vec![]).as_deref(),
            Some("ＡＥ【応急処置】 ＞ HPが0回復")
        );
    }
}
