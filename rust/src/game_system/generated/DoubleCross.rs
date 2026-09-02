//! P4で手書き移植した `lib/bcdice/game_system/DoubleCross.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DoubleCross::DX`（成功判定ノード）と `DoubleCross::ValueGroup`（出目のグループ）
//! - `parse_dx` / `parse_dx_od`（OD Tool式 `xDX+y@c`）/ `parse_dx_shippu_doto`（疾風怒濤式 `xDXc+y`）
//! - `roll_emotion_table`（感情表 `ET`）と `TABLES`（`HC` / `PCP` / `PCN`）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("DoubleCross.…", locale:)` で `i18n/DoubleCross/ja_jp.yml` から表を作る。
//! Rust側は同じ値を `static` として直接持つ。データ部分（`JA_` 接頭辞の `static` 群）は
//! 同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`DoubleCross_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `DoubleCross_Korean < DoubleCross` なのに対応する）。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::common_command::reroll_dice::REROLL_LIMIT;
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// Ruby `TABLES` の値。`RangeTable`（`HC`）と `Table`（`PCP` / `PCN`）が混在する。
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

/// 1ロケール分の表と定型文。`DoubleCross` と `DoubleCross_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `POSITIVE_EMOTION_TABLE`
    pub(crate) positive_emotion_table: &'static RangeTable,
    /// Ruby `NEGATIVE_EMOTION_TABLE`
    pub(crate) negative_emotion_table: &'static RangeTable,
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, TableRef)],
    /// i18n `DoubleCross.ET.name`
    pub(crate) et_name: &'static str,
    /// i18n `DoubleCross.DX.invalid_critical`
    pub(crate) invalid_critical: &'static str,
    /// i18n `DoubleCross.DX.auto_failure`
    pub(crate) auto_failure: &'static str,
    /// i18n `fumble`
    pub(crate) fumble: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `DoubleCross#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(dx) = parse_dx(command) {
        return Ok(Some(SpecificCommandOutput::result(
            dx.execute(tables, rng)?,
        )));
    }

    if let Some(text) = roll_tables(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }

    if command == "ET" {
        return Ok(Some(SpecificCommandOutput::result(roll_emotion_table(
            tables, rng,
        )?)));
    }

    Ok(None)
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

/// Ruby `DoubleCross#roll_emotion_table`（感情表 `ET`）。
///
/// ポジティブとネガティブの両方を振り、`roll_once(2)` で○を付ける側を決める。
/// 消費する乱数は「1D100 → 1D100 → 1D2」の順。
fn roll_emotion_table(
    tables: &SystemTables,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let pos_result = tables.positive_emotion_table.roll(rng)?;
    let neg_result = tables.negative_emotion_table.roll(rng)?;

    // Ruby: positive = @randomizer.roll_once(2) == 1
    let positive = rng.roll_once(2)? == 1;
    let pos_neg_text = if positive {
        format!("○{} - {}", pos_result.content, neg_result.content)
    } else {
        format!("{} - ○{}", pos_result.content, neg_result.content)
    };

    // 表そのものの名前（`感情表（ポジティブ）`）ではなく `DoubleCross.ET.name` を使う。
    Ok(EvalResult::with_text(format!(
        "{}({}-{}) ＞ {}",
        tables.et_name, pos_result.sum, neg_result.sum, pos_neg_text
    )))
}

// ---------------------------------------------------------------------------
// 成功判定コマンド
// ---------------------------------------------------------------------------

/// Ruby `DoubleCross#parse_dx`。
fn parse_dx(command: &str) -> Option<Dx> {
    parse_dx_od(command).or_else(|| parse_dx_shippu_doto(command))
}

/// Ruby `DoubleCross#parse_dx_od`（OD Tool式 `xDX+y@c`）。
fn parse_dx_od(command: &str) -> Option<Dx> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new(/\d+DX/, round_type: round_type)
    //       `round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+DX"], RoundType::Floor)
            .enable_critical()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let parsed = parser.parse(command)?;

    // Ruby: parsed.command.to_i（"10DX" → 10）
    let num = ruby_to_i(&parsed.command);
    let critical_value = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(10);

    Some(Dx::new(
        num,
        critical_value,
        sat_i64(&parsed.modify_number),
        parsed
            .target_number
            .as_ref()
            .map(crate::randomizer::sat_i64),
    ))
}

/// Ruby `DoubleCross#parse_dx_shippu_doto`（疾風怒濤式 `xDXc+y`）。
fn parse_dx_shippu_doto(command: &str) -> Option<Dx> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new(/\d+DX\d*/, round_type: round_type)（クリティカル値の `@` は無効）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+DX\d*"], RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let parsed = parser.parse(command)?;

    // Ruby: num, critical_value = parsed.command.split("DX", 2).map { |x| x&.to_i }
    //       Ruby の limit 付き split は末尾の空文字列を残すので、"10DX" は ["10", ""] ＝
    //       critical_value が 0 になる（`||= 10` は効かない）。Rust の splitn も同じ形。
    let mut parts = parsed.command.splitn(2, "DX");
    let num = parts.next().map_or(0, ruby_to_i);
    // `critical_value ||= 10`。notation が `\d+DX\d*` なので "DX" は必ず含まれ、
    // 2要素目が無い（＝nil）この枝には到達しない。原典どおり既定値だけ書いておく。
    let critical_value = parts.next().map_or(10, ruby_to_i);

    Some(Dx::new(
        num,
        critical_value,
        sat_i64(&parsed.modify_number),
        parsed
            .target_number
            .as_ref()
            .map(crate::randomizer::sat_i64),
    ))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn ruby_to_i(s: &str) -> i64 {
    str_helpers::leading_digits_to_i_max(s)
}

/// Ruby `DoubleCross::DX`（成功判定コマンドのノード）。
struct Dx {
    /// ダイス数
    num: i64,
    /// クリティカル値
    critical_value: i64,
    /// 修正値
    modifier: i64,
    /// 目標値
    target_value: Option<i64>,
    /// Ruby `@modifier_str`（`Format.modifier`）
    modifier_str: String,
    /// Ruby `@expression`（`#node_expression`）
    expression: String,
}

impl Dx {
    /// Ruby `DX#initialize`。
    fn new(num: i64, critical_value: i64, modifier_value: i64, target_value: Option<i64>) -> Self {
        let modifier_str = modifier(&crate::Int::from(modifier_value));
        // Ruby `#node_expression`
        let lhs = format!("{num}DX{critical_value}{modifier_str}");
        let expression = match target_value {
            Some(target) => format!("{lhs}>={target}"),
            None => lhs,
        };

        Self {
            num,
            critical_value,
            modifier: modifier_value,
            target_value,
            modifier_str,
            expression,
        }
    }

    /// Ruby `DX#execute`。
    fn execute(
        &self,
        tables: &SystemTables,
        rng: &mut Randomizer,
    ) -> Result<EvalResult, EvalError> {
        // Ruby: Result.new（フラグは一切立たない）
        if self.critical_value < 2 {
            return Ok(EvalResult::with_text(format!(
                "({}) ＞ {}",
                self.expression, tables.invalid_critical
            )));
        }

        // Ruby: Result.failure（failure だけが立つ）
        if self.num < 1 {
            return Ok(EvalResult::failure(format!(
                "({}) ＞ {}",
                self.expression, tables.auto_failure
            )));
        }

        // 出目のグループの配列
        let mut value_groups: Vec<ValueGroup> = Vec::new();
        // 次にダイスロールを行う際のダイス数
        let mut num_of_dice = self.num;
        // 回転数
        let mut loop_count = 0usize;

        while num_of_dice > 0 && loop_count < REROLL_LIMIT {
            let values = rng.roll_barabara(num_of_dice, 10)?;
            let value_group = ValueGroup::new(values, self.critical_value);

            // 次回はクリティカル発生数と等しい個数のダイスを振る
            // [3rd ルールブック1 p. 185]
            num_of_dice = value_group.num_of_critical_occurrences();
            value_groups.push(value_group);

            loop_count += 1;
        }

        Ok(self.result(tables, &value_groups))
    }

    /// Ruby `DX#result`。
    ///
    /// `@num >= 1` が保証されているので `value_groups` は必ず1件以上ある。
    fn result(&self, tables: &SystemTables, value_groups: &[ValueGroup]) -> EvalResult {
        let mut r = EvalResult::new();

        // Ruby: r.fumble = value_groups[0].values.all?(1)
        // `attr_writer` なので failure は立たない（`Result.fumble` とは違う）。
        r.fumble = value_groups[0].values.iter().all(|value| *value == 1);

        let sum = value_groups
            .iter()
            .fold(0i64, |acc, group| acc.wrapping_add(group.max()));
        let achieved_value = if r.fumble {
            0
        } else {
            sum.wrapping_add(self.modifier)
        };

        // ファンブルかどうかを含む達成値の表記
        let achieved_value_with_if_fumble = if r.fumble {
            format!("{achieved_value} ({})", tables.fumble)
        } else {
            achieved_value.to_string()
        };

        // Ruby: r.critical = value_groups.length > 1（success は立たない）
        r.critical = value_groups.len() > 1;

        let groups_str: Vec<String> = value_groups.iter().map(ValueGroup::to_string).collect();
        let mut parts = vec![
            format!("({})", self.expression),
            format!("{}{}", groups_str.join("+"), self.modifier_str),
            achieved_value_with_if_fumble,
        ];

        if let Some(target_value) = self.target_value {
            // 行為判定成功か？
            //
            // ファンブル時は自動失敗、達成値が目標値以上ならば行為判定成功
            // [3rd ルールブック1 pp. 186-187]
            let success = !r.fumble && achieved_value >= target_value;

            if success {
                r.success = true;
            } else {
                r.failure = true;
            }

            parts.push(
                if success {
                    tables.success
                } else {
                    tables.failure
                }
                .to_owned(),
            );
        }

        r.text = parts.join(" ＞ ");

        r
    }
}

/// Ruby `DoubleCross::ValueGroup`（出目のグループ）。
struct ValueGroup {
    /// 出目の配列（昇順）
    values: Vec<i64>,
    /// クリティカル値
    critical_value: i64,
}

impl ValueGroup {
    /// Ruby `ValueGroup#initialize`（`values.sort`）。
    fn new(mut values: Vec<i64>, critical_value: i64) -> Self {
        values.sort_unstable();
        Self {
            values,
            critical_value,
        }
    }

    /// Ruby `ValueGroup#max`。
    ///
    /// クリティカル値以上の出目が含まれていた場合は10を返す。
    /// [3rd ルールブック1 pp. 185-186]
    ///
    /// 空のグループ（`roll_barabara` の上限200個を超えたダイス数）では Ruby は `nil` を返し、
    /// 呼び出し元の `sum` が `TypeError` でクラッシュする。ここでは 0 として扱う。
    fn max(&self) -> i64 {
        if self.values.iter().any(|value| self.is_critical(*value)) {
            10
        } else {
            self.values.iter().copied().max().unwrap_or(0)
        }
    }

    /// Ruby `ValueGroup#num_of_critical_occurrences`。
    fn num_of_critical_occurrences(&self) -> i64 {
        self.values
            .iter()
            .filter(|value| self.is_critical(**value))
            .count() as i64
    }

    /// Ruby `ValueGroup#critical?`。
    ///
    /// クリティカル値以上の値が出た場合、クリティカルとする。
    /// [3rd ルールブック1 pp. 185-186]
    fn is_critical(&self, value: i64) -> bool {
        value >= self.critical_value
    }
}

impl std::fmt::Display for ValueGroup {
    /// Ruby `ValueGroup#to_s`: `"#{max}[#{@values.join(',')}]"`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values: Vec<String> = self.values.iter().map(|v| v.to_string()).collect();
        write!(f, "{}[{}]", self.max(), values.join(","))
    }
}

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::DoubleCross`（ID: `DoubleCross`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoubleCross;

impl GameSystem for DoubleCross {
    fn id(&self) -> &'static str {
        "DoubleCross"
    }

    fn name(&self) -> &'static str {
        "ダブルクロス2nd,3rd"
    }

    fn sort_key(&self) -> &'static str {
        "たふるくろす2"
    }

    fn help_message(&self) -> &'static str {
        r#"・判定コマンド（xDX+y@c or xDXc+y）
　"(個数)DX(修正)@(クリティカル値)" もしくは "(個数)DX(クリティカル値)(修正)" で指定します。
　修正値も付けられます。
　例）10dx　　10dx+5@8（OD tool式)　　5DX7+7-3（疾風怒濤式）

・各種表
　・感情表（ET）
　　ポジティブとネガティブの両方を振って、表になっている側に○を付けて表示します。
　　もちろん任意で選ぶ部分は変更して構いません。

・ハプニングチャート（HC）
・RWプロローグチャート ポジティブ (PCP)
・RWプロローグチャート ネガティブ (PCN)
・D66ダイスあり
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DX", "ET", r"\d+DX", "HC", "PCP", "PCN"]
    }

    crate::impl_prefixes_pattern!();

    fn sides_implicit_d(&self) -> i64 {
        10
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

// ---------------------------------------------------------------------------
// 表データ（i18n/DoubleCross/ja_jp.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `DoubleCross.ET.positive.items`。
static JA_POSITIVE_EMOTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "好奇心(こうきしん)"),
    (RangeInc::new(6, 10), "憧憬(どうけい)"),
    (RangeInc::new(11, 15), "尊敬(そんけい)"),
    (RangeInc::new(16, 20), "連帯感(れんたいかん)"),
    (RangeInc::new(21, 25), "慈愛(じあい)"),
    (RangeInc::new(26, 30), "感服(かんぷく)"),
    (RangeInc::new(31, 35), "純愛(じゅんあい)"),
    (RangeInc::new(36, 40), "友情(ゆうじょう)"),
    (RangeInc::new(41, 45), "慕情(ぼじょう)"),
    (RangeInc::new(46, 50), "同情(どうじょう)"),
    (RangeInc::new(51, 55), "遺志(いし)"),
    (RangeInc::new(56, 60), "庇護(ひご)"),
    (RangeInc::new(61, 65), "幸福感(こうふくかん)"),
    (RangeInc::new(66, 70), "信頼(しんらい)"),
    (RangeInc::new(71, 75), "執着(しゅうちゃく)"),
    (RangeInc::new(76, 80), "親近感(しんきんかん)"),
    (RangeInc::new(81, 85), "誠意(せいい)"),
    (RangeInc::new(86, 90), "好意(こうい)"),
    (RangeInc::new(91, 95), "有為(ゆうい)"),
    (RangeInc::new(96, 100), "尽力(じんりょく)"),
];

/// Ruby `POSITIVE_EMOTION_TABLE`（`1D100`）。
static JA_POSITIVE_EMOTION_TABLE: RangeTable =
    RangeTable::from_dice("感情表（ポジティブ）", 1, 100, JA_POSITIVE_EMOTION_ITEMS);

/// i18n `DoubleCross.ET.negative.items`。
static JA_NEGATIVE_EMOTION_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "食傷(しょくしょう)"),
    (RangeInc::new(6, 10), "脅威(きょうい)"),
    (RangeInc::new(11, 15), "嫉妬(しっと)"),
    (RangeInc::new(16, 20), "悔悟(かいご)"),
    (RangeInc::new(21, 25), "恐怖(きょうふ)"),
    (RangeInc::new(26, 30), "不安(ふあん)"),
    (RangeInc::new(31, 35), "劣等感(れっとうかん)"),
    (RangeInc::new(36, 40), "疎外感(そがいかん)"),
    (RangeInc::new(41, 45), "恥辱(ちじょく)"),
    (RangeInc::new(46, 50), "憐憫(れんびん)"),
    (RangeInc::new(51, 55), "偏愛(へんあい)"),
    (RangeInc::new(56, 60), "憎悪(ぞうお)"),
    (RangeInc::new(61, 65), "隔意(かくい)"),
    (RangeInc::new(66, 70), "嫌悪(けんお)"),
    (RangeInc::new(71, 75), "猜疑心(さいぎしん)"),
    (RangeInc::new(76, 80), "厭気(いやけ)"),
    (RangeInc::new(81, 85), "不信感(ふしんかん)"),
    (RangeInc::new(86, 90), "不快感(ふかいかん)"),
    (RangeInc::new(91, 95), "憤懣(ふんまん)"),
    (RangeInc::new(96, 100), "敵愾心(てきがいしん)"),
];

/// Ruby `NEGATIVE_EMOTION_TABLE`（`1D100`）。
static JA_NEGATIVE_EMOTION_TABLE: RangeTable =
    RangeTable::from_dice("感情表（ネガティブ）", 1, 100, JA_NEGATIVE_EMOTION_ITEMS);

/// i18n `DoubleCross.HC.items`。
static JA_HC_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 5), "こともなし。修正は特にない。"),
    (RangeInc::new(6, 10), "専門的知識が必要。そのラウンドの間、指定された技能が4レベル以下のキャラクターが獲得する進行値は-1となる(最低0)"),
    (RangeInc::new(11, 15), "焦り。そのラウンド中、難易度+1D10。"),
    (RangeInc::new(16, 20), "一歩間違えば致命的な状況。次の進行判定に失敗した場合、今まで獲得した進行値が0になる。"),
    (RangeInc::new(21, 25), "異常な興奮。そのラウンド中、進行判定に失敗したキャラクターは暴走を受ける。"),
    (RangeInc::new(26, 30), "プレッシャー。そのラウンド中に進行判定を行なったキャラクターは、判定の直後に重圧を受ける。"),
    (RangeInc::new(31, 35), "幸運がほほえむ。このラウンド中に行なう進行判定はすべてクリティカル値-1される。"),
    (RangeInc::new(36, 40), "破滅的不運。このラウンド中に行なう進行判定はすべてクリティカル値+1される。"),
    (RangeInc::new(41, 45), "一か八かのチャンス。このラウンド中、最大達成値と難易度に+10。"),
    (RangeInc::new(46, 55), "膠着した進行。修正は特にない。"),
    (RangeInc::new(56, 60), "綱渡りのような状況。このラウンド中、難易度+1D10。"),
    (RangeInc::new(61, 65), "あるかなきかのチャンス。このラウンド中、最大達成値+10。"),
    (RangeInc::new(66, 70), "消耗を伴う作業。このラウンド中に進行判定を行なったキャラクターは、判定の直後に1D10点のHPダメージを受ける。"),
    (RangeInc::new(71, 75), "チャンス到来。このラウンド中に行なう進行判定は、ダイスが+5個される。"),
    (RangeInc::new(76, 80), "予想外のピンチ。このラウンド中に行なう進行判定は、ダイスが-5個される。"),
    (RangeInc::new(81, 85), "緊張がレネゲイドを活性化。そのラウンド中に進行判定を行なったキャラクターは、判定の直後に1D10点侵蝕率が増加。"),
    (RangeInc::new(86, 90), "突破口の発見。このシーン中の最大達成値+10。この効果は重複しない。"),
    (RangeInc::new(91, 95), "事態の断続的な悪化。このシーン中の難易度+1D10。この効果は重複する。"),
    (RangeInc::new(96, 100), "順当な進行。このラウンド中に進行判定に成功したキャラクターは、進行値を+1得る。"),
];

/// Ruby `TABLES["HC"]`（`1D100`）。
static JA_HC: RangeTable = RangeTable::from_dice("ハプニングチャート", 1, 100, JA_HC_ITEMS);

/// i18n `DoubleCross.PCP.items`。
static JA_PCP_ITEMS: &[&str] = &[
    "【ヴィクトリー】 ヴィランの集団と戦い、勝利する。報道陣や観衆がその勝利を称える。",
    "【ハプニング】 銀行や商店などにいる際、突発的な犯罪に巻き込まれ、それを解決する。",
    "【レスキュー】 火事や爆発事故、倒壊などの災害現場で、市民を救出する。",
    "【ヴァーサス】 ライバルである強力なヴィランと対決している。決着はつかず、ヴィランは逃亡する。",
    "【ヒーローインタビュー】 メディアから取材を受ける。事件の解決や首長からの表彰によって、あるいは注目のヒーローとしてなど。",
    "【トレーニング】 ヒーローとしてトレーニングを行なっている。身体能力やエフェクトの訓練、知識の補強など。",
    "【オリジン】 自分がヒーローとなったきっかけ、発端の場面を回想する。初めてオーヴァードに覚醒した場面や、初めて他人を救った時、かつての憧れのヒーローについてなど。",
    "【エブリデイ・ライフ】 日常生活を送っている。久々の休暇か、ヒーロー以外の生活か。ロイスの対象と会話するのもよいだろう。",
    "【ニューパワー】 新しいエフェクトや装備を身につける、受け取る。これで新たな力を手に入れたことになる。",
    "【サクセス】 なにかに大成功した場面だ。仕事でもよいし、休暇中のゲームやスポーツかもしれない。",
];

/// Ruby `TABLES["PCP"]`（`1D10`）。
static JA_PCP: Table = Table::from_dice("プロローグチャート(ポジティブ)", 1, 10, JA_PCP_ITEMS);

/// i18n `DoubleCross.PCN.items`。
static JA_PCN_ITEMS: &[&str] = &[
    "【ディフィート】 ヴィランと戦い、敗北した場面を回想する。その時の負傷は既に回復しているが、誇りはまだ回復していない。",
    "【アクシデント】 不運に見舞われる。事故に巻き込まれる、たまたまヴィランの攻撃を受けるなど。その不幸で誰かが助かるかもしれない。",
    "【ディザスター】 事故や災害に巻き込まれる、あるいは過去に巻き込まれた場面の回想。自分はオーヴァードの能力で生き残るが、他は⋯⋯。",
    "【ウィークポイント】 ライバルである強力なヴィランと対決している。ヴィランはあなたの弱点や致命的欠陥、不吉な未来を告げて去っていく。",
    "【バッシング】 メディアや市民から批判を受ける。過去の失敗や、今の乱暴な解決法など。",
    "【リカバリー】 治療を受けている。最近怪我をした、過去の古傷が残っている、もう肉体が限界だ⋯⋯など。",
    "【トラウマ】 過去の不幸、悲劇、失敗などを回想している。それがヒーローになったきっかけかもしれない。",
    "【アキューズ】 あなたを責める会話。ロイスの対象、被害者の肉親など。それでもヒーローを続けるしかない⋯⋯。",
    "【タイムリミット】 診療を受け、限界が近いことを告げられる。オーヴァードの能力に肉体が耐えられない、能力が衰えているなど。",
    "【プリーズ】 ロイスの対象などから、ヒーローを引退するようにお願いされる。あなたが心配だ、危険すぎる、など。聞き入れるわけにはいかないが⋯⋯。",
];

/// Ruby `TABLES["PCN"]`（`1D10`）。
static JA_PCN: Table = Table::from_dice("プロローグチャート(ネガティブ)", 1, 10, JA_PCN_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static JA_TABLES: &[(&str, TableRef)] = &[
    ("HC", TableRef::Range(&JA_HC)),
    ("PCP", TableRef::Plain(&JA_PCP)),
    ("PCN", TableRef::Plain(&JA_PCN)),
];

/// `ja_jp` ロケールの表と定型文。
static JA_SYSTEM: SystemTables = SystemTables {
    positive_emotion_table: &JA_POSITIVE_EMOTION_TABLE,
    negative_emotion_table: &JA_NEGATIVE_EMOTION_TABLE,
    tables: JA_TABLES,
    et_name: "感情表",
    invalid_critical: "クリティカル値が低すぎます。2以上を指定してください。",
    auto_failure: "自動失敗",
    fumble: "ファンブル",
    success: "成功",
    failure: "失敗",
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
            .join("test/data/DoubleCross.toml");
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
        for (name, table) in [
            ("positive_emotion_table", JA_POSITIVE_EMOTION_TABLE),
            ("negative_emotion_table", JA_NEGATIVE_EMOTION_TABLE),
            ("HC", JA_HC),
        ] {
            assert_eq!(table.validate(), Ok(()), "{name}");
        }
    }

    /// `test/data/DoubleCross.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/DoubleCross.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DoubleCross.toml must parse");
        assert_eq!(
            data.tests.len(),
            101,
            "case count in test/data/DoubleCross.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DoubleCross",
                "unexpected game system in DoubleCross.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("DoubleCross"), &tc.input, &mut src) {
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
                    "FAIL DoubleCross:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DoubleCross cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
