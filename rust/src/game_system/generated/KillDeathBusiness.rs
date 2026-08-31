//! P4で手書き移植した `lib/bcdice/game_system/KillDeathBusiness.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KillDeathBusiness#result_2d6`（出目2ファンブル / 出目12スペシャル。それ以外は `nil`）
//! - `#eval_game_system_specific_command` → `judgeDice`（`JDx±y,z` / `JDx±y#z`）と
//!   `rollTableCommand`（各種表 / 指定特技表 / シーン・命名・サービス・罵倒・エキストラ /
//!   大喜利スペシャル）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("KillDeathBusiness.…", locale:)` で
//! `i18n/KillDeathBusiness/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として持つ。
//! `JA_` 接頭辞の `static` 群は同YAMLから機械抽出したもので、値は1文字も変えていない。
//!
//! # 継承
//!
//! Ruby `KillDeathBusiness_Korean < KillDeathBusiness` は `@locale = :ko_kr` にして
//! 表と訳文を組み直すだけ。Rustには継承がないので、差し替えうるデータは
//! [`SystemTables`] にまとめ、[`eval_specific_command`] / [`check_result_2d6`] へ渡す。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::sai_fic_skill_table::DEFAULT_RTTN_FORMAT;
use crate::dice_table::{D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::KillDeathBusiness`（ID: `KillDeathBusiness`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillDeathBusiness;

impl GameSystem for KillDeathBusiness {
    fn id(&self) -> &'static str {
        "KillDeathBusiness"
    }

    fn name(&self) -> &'static str {
        "キルデスビジネス"
    }

    fn sort_key(&self) -> &'static str {
        "きるてすひしねす"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&JA_SYSTEM, total, dice_total, cmp_op, target)
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

/// Ruby `HELP_MESSAGE`。
static HELP_MESSAGE: &str = r"・判定
　JDx or JDx±y or JDx,z JDx#z or JDx±y,z JDx±y#z
　（x＝難易度、y＝補正、z＝ファンブル率(リスク)）
・履歴表 (HST)
・願い事表 (-WT)
　死(DWT)、復讐(RWT)、勝利(VWT)、獲得(PWT)、支配(CWT)、繁栄(FWT)
　強化(IWT)、健康(HWT)、安全(SAWT)、長寿(LWT)、生(EWT)
・万能命名表 (NAME, NAMEx) xに数字(1,2,3)で表を個別ロール
・サブプロット表 (-SPT)
　オカルト(OSPT)、家族(FSPT)、恋愛(LOSPT)、正義(JSPT)、修行(TSPT)
　笑い(BSPT)、意地悪(MASPT)、恨み(UMSPT)、人気(POSPT)、仕切り(PASPT)
　金儲け(MOSPT)、対悪魔(ANSPT)
・シーン表 (ST)、サービスシーン表 (EST)
・CM表 (CMT)
・蘇生副作用表 (ERT)
・一週間表（WKT)
・ソウル放出表 (SOUL)
・汎用演出表 (STGT)
・ヘルスタイリスト罵倒表 (HSAT、HSATx) xに数字(1,2)で表を個別ロール
・指定特技ランダム決定表 (SKLT, RTTn nは分野番号)、指定特技分野ランダム決定表 (RCT, SKLJ)
・エキストラ表 (EXT、EXTx) xに数字(1,2,3,4)で表を個別ロール
・製作委員決定表　PCDT/実際どうだったのか表　OHT
・タスク表　ヘルライオン　PCT1/ヘルクロウ　PCT2/ヘルスネーク　PCT3/
　ヘルドラゴン　PCT4/ヘルフライ　PCT5/ヘルゴート　PCT6/ヘルベア　PCT7
・大喜利スペシャル表 (-OT)
　お題決定表(TOT)、〇〇を見て一言表(OOT)
　単語表(WOT, WOTx) xに英字(A,B,C)で単語表A(人物)(AOT)、単語表B(物)(BOT)、単語表C(場所)を個別ロール
　動詞表(VOT)、長め単語表(LOT)
　ヘル司会者 リアクション表(好印象ver)(POT)、ヘル司会者 リアクション表(不満ver)(NOT)
・D66ダイスあり
";

/// Ruby `register_prefix(...)`。スタブの配列をそのまま維持する。
static PREFIXES: &[&str] = &[
    "ST[1-2]?",
    "NAME[1-3]?",
    "EST",
    "sErviceST",
    "HSAT[1-2]?",
    "EXT[1-4]?",
    "JD",
    "TOT",
    "OOT",
    "WOT",
    "POT",
    "NOT",
    "DEATHWT",
    "REVENGEWT",
    "VICTORYWT",
    "POSSESIONWT",
    "CONTROLWT",
    "FLOURISHWT",
    "INTENSIFYWT",
    "HEALTHWT",
    "SAFETYWT",
    "LONGEVITYWT",
    "EXISTWT",
    "OCCULTSPT",
    "FAMILYSPT",
    "LOVESPT",
    "JUSTICESPT",
    "TRAININGSPT",
    "BEAMSPT",
    "HST",
    "DWT",
    "RWT",
    "VWT",
    "PWT",
    "CWT",
    "FWT",
    "IWT",
    "HWT",
    "SAWT",
    "LWT",
    "EWT",
    "OSPT",
    "FSPT",
    "LOSPT",
    "JSPT",
    "TSPT",
    "BSPT",
    "CMT",
    "ERT",
    "WKT",
    "SOUL",
    "STGT",
    "PCDT",
    "OHT",
    "PCT1",
    "PCT2",
    "PCT3",
    "PCT4",
    "PCT5",
    "PCT6",
    "PCT7",
    "ANSPT",
    "MASPT",
    "MOSPT",
    "PASPT",
    "POSPT",
    "UMSPT",
    "WOTA",
    "WOTB",
    "WOTC",
    "VOT",
    "LOT",
    "ST[1-2]?",
    "NAME[1-3]?",
    "EST",
    "sErviceST",
    "HSAT[1-2]?",
    "EXT[1-4]?",
    "JD",
    "TOT",
    "OOT",
    "WOT",
    "POT",
    "NOT",
    "DEATHWT",
    "REVENGEWT",
    "VICTORYWT",
    "POSSESIONWT",
    "CONTROLWT",
    "FLOURISHWT",
    "INTENSIFYWT",
    "HEALTHWT",
    "SAFETYWT",
    "LONGEVITYWT",
    "EXISTWT",
    "OCCULTSPT",
    "FAMILYSPT",
    "LOVESPT",
    "JUSTICESPT",
    "TRAININGSPT",
    "BEAMSPT",
    "RTT[1-6]?",
    "RCT",
    "SKLT",
    "SKLJ",
];

const NO_RTTN_ALIASES: &[&str] = &[];

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// サービスシーン表の副表。i18n `KillDeathBusiness.EST.tables.*`。
pub(crate) struct EstSubTable {
    /// 副表の名前。
    pub name: &'static str,
    /// 出目1〜6の項目。
    pub items: &'static [&'static str],
}

/// 判定コマンド `JD` の訳文。i18n `KillDeathBusiness.JD`。
pub(crate) struct JdTexts {
    pub name: &'static str,
    pub warn_over_target: &'static str,
    pub warn_min_target: &'static str,
    pub warn_over_fumble: &'static str,
    pub options: &'static str,
    pub dice_value: &'static str,
    pub fumble: &'static str,
    pub special: &'static str,
    pub less_than_fumble: &'static str,
    pub failure: &'static str,
    pub success: &'static str,
}

/// 1ロケール分の表と定型文。`KillDeathBusiness` と `KillDeathBusiness_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `KillDeathBusiness.fumble`
    pub fumble: &'static str,
    /// i18n `KillDeathBusiness.special`
    pub special: &'static str,
    /// i18n `KillDeathBusiness.JD`
    pub jd: JdTexts,
    /// i18n `KillDeathBusiness.ST.name`
    pub st_name: &'static str,
    /// i18n `KillDeathBusiness.ST.format`
    pub st_format: &'static str,
    pub st_table1: &'static [(i64, &'static str)],
    pub st_table2: &'static [(i64, &'static str)],
    /// i18n `KillDeathBusiness.NAME.name`
    pub name_name: &'static str,
    pub name_table1: &'static [(i64, &'static str)],
    pub name_table2: &'static [(i64, &'static str)],
    pub name_table3: &'static [(i64, &'static str)],
    /// i18n `KillDeathBusiness.EST.name`
    pub est_name: &'static str,
    /// i18n `KillDeathBusiness.EST.format`
    pub est_format: &'static str,
    pub est_tables: &'static [EstSubTable],
    /// i18n `KillDeathBusiness.HSAT.name`
    pub hsat_name: &'static str,
    pub hsat_abuse1: &'static [(i64, &'static str)],
    pub hsat_abuse2: &'static [(i64, &'static str)],
    pub hsat_prefix: &'static [&'static str],
    pub hsat_suffix: &'static [&'static str],
    /// i18n `KillDeathBusiness.EXT.name`
    pub ext_name: &'static str,
    pub ext_table1: &'static [(i64, &'static str)],
    pub ext_table2: &'static [(i64, &'static str)],
    pub ext_table3: &'static [(i64, &'static str)],
    pub ext_table4: &'static [(i64, &'static str)],
    /// Ruby `TABLES`
    pub tables: &'static [(&'static str, &'static dyn RollableTable)],
    /// Ruby `RTT`
    pub rtt: &'static SaiFicSkillTable,
    pub wota: &'static D66Table,
    pub wotb: &'static D66Table,
    pub wotc: &'static D66Table,
    pub vot: &'static D66Table,
    pub lot: &'static D66Table,
    /// i18n `KillDeathBusiness.table.TOT`
    pub tot_name: &'static str,
    pub tot_item1: &'static str,
    pub tot_item2: &'static str,
    pub tot_item3: &'static str,
    pub tot_item4: &'static str,
    pub tot_item5: &'static str,
    pub tot_item6: &'static str,
    /// i18n `KillDeathBusiness.table.OOT`
    pub oot_name: &'static str,
    pub oot_item1: &'static str,
    pub oot_item3: &'static str,
    pub oot_item5: &'static str,
    /// i18n `KillDeathBusiness.table.POT`
    pub pot_name: &'static str,
    pub pot_item1: &'static str,
    pub pot_item2: &'static str,
    pub pot_item3: &'static str,
    pub pot_item4: &'static str,
    pub pot_item5: &'static str,
    pub pot_item6: &'static str,
    /// i18n `KillDeathBusiness.table.NOT`
    pub not_name: &'static str,
    pub not_item1: &'static str,
    pub not_item2: &'static str,
    pub not_item3: &'static str,
    pub not_item4: &'static str,
    pub not_item5: &'static str,
    pub not_item6: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `KillDeathBusiness#result_2d6`。
pub(crate) fn check_result_2d6(
    sys: &SystemTables,
    _total: crate::Int,
    dice_total: i64,
    cmp_op: CmpOp,
    _target: Target,
) -> Option<CheckOutcome> {
    if cmp_op != CmpOp::Ge {
        return None;
    }
    if dice_total <= 2 {
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            sys.fumble,
        ))))
    } else if dice_total >= 12 {
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            sys.special,
        ))))
    } else {
        None
    }
}

/// Ruby `KillDeathBusiness#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if command.starts_with("JD") {
        return Ok(judge_dice(sys, command, rng)?.map(SpecificCommandOutput::text));
    }
    Ok(roll_table_command(sys, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `KillDeathBusiness#judgeDice`。
fn judge_dice(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"JD\d+"], RoundType::Floor)
            .enable_critical()
            .enable_fumble()
            .restrict_cmp_op_to(&[None])
    });

    let (source, comma_fumble) = split_comma_fumble(command);
    let Some(cmd) = parser.parse(source) else {
        return Ok(None);
    };

    let mut target = cmd
        .command
        .strip_prefix("JD")
        .and_then(ruby_to_i)
        .unwrap_or(0);
    let modify = cmd.modify_number;
    let mut fumble = comma_fumble.unwrap_or(
        cmd.fumble
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
    );
    let expr = judge_expr(target, crate::randomizer::sat_i64(&modify), fumble);

    let mut result = String::new();
    if target > 12 {
        push_jd_warning(&mut result, &expr, sys.jd.warn_over_target);
        target = 12;
    }
    if target < 5 {
        push_jd_warning(&mut result, &expr, sys.jd.warn_min_target);
        target = 5;
    }
    if fumble < 2 {
        fumble = 2;
    } else if fumble > 11 {
        push_jd_warning(&mut result, &expr, sys.jd.warn_over_fumble);
        fumble = 11;
    }

    let dice_list = rng.roll_barabara(2, 6)?;
    let number: i64 = dice_list.iter().sum();
    let dice_text = join_dice(&dice_list);

    result.push_str(&interpolate(
        sys.jd.options,
        &[
            ("target", &target.to_string()),
            ("modifier", &modify.to_string()),
            ("fumble", &fumble.to_string()),
        ],
    ));
    result.push_str(" ＞ ");
    result.push_str(&interpolate(
        sys.jd.dice_value,
        &[("dice_value", &dice_text)],
    ));
    result.push_str(" ＞ ");

    if number == 2 {
        result.push_str(sys.jd.fumble);
    } else if number == 12 {
        result.push_str(sys.jd.special);
    } else if number <= fumble {
        result.push_str(sys.jd.less_than_fumble);
    } else {
        let value = number + modify;
        if value < crate::Int::from(target) {
            result.push_str(&interpolate(
                sys.jd.failure,
                &[("value", &value.to_string())],
            ));
        } else {
            result.push_str(&interpolate(
                sys.jd.success,
                &[("value", &value.to_string())],
            ));
        }
    }

    Ok(Some(format!("{}{result}", sys.jd.name)))
}

/// Ruby `"【#{command}】 ＞ #{warning}\n"`。
fn push_jd_warning(result: &mut String, expr: &str, warning: &str) {
    result.push('【');
    result.push_str(expr);
    result.push_str("】 ＞ ");
    result.push_str(warning);
    result.push('\n');
}

/// Ruby `KillDeathBusiness#judge_expr`。
fn judge_expr(target: i64, modify: i64, fumble: i64) -> String {
    let fumble = if fumble > 0 {
        format!(",{fumble}")
    } else {
        String::new()
    };
    format!("JD{target}{}{fumble}", modifier(&crate::Int::from(modify)))
}

/// Ruby `/,(\d+)$/.match(command)` と `pre_match`。
fn split_comma_fumble(command: &str) -> (&str, Option<i64>) {
    let Some((head, tail)) = command.rsplit_once(',') else {
        return (command, None);
    };
    if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
        return (command, None);
    }
    (head, tail.parse().ok())
}

/// Ruby `KillDeathBusiness#rollTableCommand`。
fn roll_table_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let command = alias(command);

    if let Some(text) = roll_tables(sys, command, rng)? {
        return Ok(Some(text));
    }
    if let Some(text) = sys.rtt.roll_command(rng, command)? {
        return Ok(Some(text));
    }

    let rolled = if let Some(type_) = parse_typed(command, "ST", 2) {
        Some(get_scene_table_result(sys, type_, rng)?)
    } else if let Some(type_) = parse_typed(command, "NAME", 3) {
        Some(get_name_table_result(sys, type_, rng)?)
    } else if command == "EST" || command == "SERVICEST" {
        Some(get_service_scene_table_result(sys, rng)?)
    } else if let Some(type_) = parse_typed(command, "HSAT", 2) {
        Some(get_hair_stylist_abuse_table_result(sys, type_, rng)?)
    } else if let Some(type_) = parse_typed(command, "EXT", 4) {
        Some(get_extra_table_result(sys, type_, rng)?)
    } else if matches!(command, "TO" | "TOT") {
        Some(get_theme_table_result(sys, rng)?)
    } else if matches!(command, "OO" | "OOT") {
        let (name, result, number, _) = get_one_word_table_result(sys, rng)?;
        Some((name, result, number.to_string()))
    } else if matches!(command, "WO" | "WOT") {
        let (name, result, number, _) = get_word_table_result(sys, rng)?;
        Some((name, result, number.to_string()))
    } else if matches!(command, "PO" | "POT") {
        Some(get_positive_table_result(sys, rng)?)
    } else if matches!(command, "NO" | "NOT") {
        Some(get_negative_table_result(sys, rng)?)
    } else {
        None
    };

    let Some((table_name, result, number)) = rolled else {
        return Ok(None);
    };
    if result.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("{table_name}({number}) ＞ {result}")))
}

/// Ruby `ALIAS[command] || command`。
fn alias(command: &str) -> &str {
    match command {
        "DEATHWT" => "DWT",
        "REVENGEWT" => "RWT",
        "VICTORYWT" => "VWT",
        "POSSESIONWT" => "PWT",
        "CONTROLWT" => "CWT",
        "FLOURISHWT" => "FWT",
        "INTENSIFYWT" => "IWT",
        "HEALTHWT" => "HWT",
        "SAFETYWT" => "SAWT",
        "LONGEVITYWT" => "LWT",
        "EXISTWT" => "EWT",
        "OCCULTSPT" => "OSPT",
        "FAMILYSPT" => "FSPT",
        "LOVESPT" => "LOSPT",
        "JUSTICESPT" => "JSPT",
        "TRAININGSPT" => "TSPT",
        "BEAMSPT" => "BSPT",
        other => other,
    }
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match sys.tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// `PREFIX(\d)?` の末尾数字。無しは 0（Ruby `Regexp.last_match(1).to_i`）。
fn parse_typed(command: &str, prefix: &str, max: u32) -> Option<u32> {
    let rest = command.strip_prefix(prefix)?;
    if rest.is_empty() {
        return Some(0);
    }
    if rest.len() == 1 {
        if let Some(digit) = rest.as_bytes().first().copied().filter(u8::is_ascii_digit) {
            let n = u32::from(digit - b'0');
            if (1..=max).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

/// Ruby `KillDeathBusiness#getSceneTableResult`。
fn get_scene_table_result(
    sys: &SystemTables,
    type_: u32,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    match type_ {
        1 => {
            let (result, number) = d66_swap(sys.st_table1, rng)?;
            Ok((
                sys.st_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        2 => {
            let (result, number) = d66_swap(sys.st_table2, rng)?;
            Ok((
                sys.st_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        _ => {
            let (result1, num1) = d66_swap(sys.st_table1, rng)?;
            let (result2, num2) = d66_swap(sys.st_table2, rng)?;
            let result = interpolate(sys.st_format, &[("result1", result1), ("result2", result2)]);
            Ok((sys.st_name.to_string(), result, format!("{num1},{num2}")))
        }
    }
}

/// Ruby `KillDeathBusiness#getNameTableResult`。
fn get_name_table_result(
    sys: &SystemTables,
    type_: u32,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    match type_ {
        1 => {
            let (result, number) = d66_swap(sys.name_table1, rng)?;
            Ok((
                sys.name_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        2 => {
            let (result, number) = d66_swap(sys.name_table2, rng)?;
            Ok((
                sys.name_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        3 => {
            let (result, number) = d66_swap(sys.name_table3, rng)?;
            Ok((
                sys.name_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        _ => {
            let (result1, num1) = d66_swap(sys.name_table1, rng)?;
            let (result2, num2) = d66_swap(sys.name_table2, rng)?;
            let (result3, num3) = d66_swap(sys.name_table3, rng)?;
            Ok((
                sys.name_name.to_string(),
                format!("{result1}{result2}{result3}"),
                format!("{num1},{num2},{num3}"),
            ))
        }
    }
}

/// Ruby `KillDeathBusiness#getServiceSceneTableResult`。
fn get_service_scene_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    let number1 = rng.roll_once(6)?;
    let scene_table = sys
        .est_tables
        .get(usize::try_from(number1 - 1).unwrap_or(usize::MAX))
        .ok_or(EvalError::Internal("EST table index out of range"))?;
    let number2 = rng.roll_once(6)?;
    let chosen = scene_table
        .items
        .get(usize::try_from(number2 - 1).unwrap_or(usize::MAX))
        .copied()
        .unwrap_or("");
    let result = interpolate(
        sys.est_format,
        &[("scene", scene_table.name), ("chosen", chosen)],
    );
    Ok((
        sys.est_name.to_string(),
        result,
        format!("{number1}{number2}"),
    ))
}

/// Ruby `KillDeathBusiness#getHairStylistAbuseTableResult`。
fn get_hair_stylist_abuse_table_result(
    sys: &SystemTables,
    type_: u32,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    match type_ {
        1 => {
            let (result, number) = d66_swap(sys.hsat_abuse1, rng)?;
            Ok((
                sys.hsat_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        2 => {
            let (result, number) = d66_swap(sys.hsat_abuse2, rng)?;
            Ok((
                sys.hsat_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        _ => {
            let (result1, num1) = d66_swap(sys.hsat_abuse1, rng)?;
            let (result2, num2) = d66_swap(sys.hsat_abuse2, rng)?;
            let (before, _) = table_1d6(sys.hsat_prefix, rng)?;
            let (after, _) = table_1d6(sys.hsat_suffix, rng)?;
            Ok((
                sys.hsat_name.to_string(),
                format!("{before}{result1} {result2}{after}"),
                format!("{num1},{num2}"),
            ))
        }
    }
}

/// Ruby `KillDeathBusiness#getExtraTableResult`。
fn get_extra_table_result(
    sys: &SystemTables,
    type_: u32,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    match type_ {
        1 => {
            let (result, number) = extra_table1(sys, rng)?;
            Ok((sys.ext_name.to_string(), result, number.to_string()))
        }
        2 => {
            let (result, number) = d66_swap(sys.ext_table2, rng)?;
            Ok((
                sys.ext_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        3 => {
            let (result, number) = d66_swap(sys.ext_table3, rng)?;
            Ok((
                sys.ext_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        4 => {
            let (result, number) = d66_swap(sys.ext_table4, rng)?;
            Ok((
                sys.ext_name.to_string(),
                result.to_string(),
                number.to_string(),
            ))
        }
        _ => {
            let (result1, num1) = extra_table1(sys, rng)?;
            let (result2, num2) = d66_swap(sys.ext_table2, rng)?;
            let (result3, num3) = d66_swap(sys.ext_table3, rng)?;
            let (result4, num4) = d66_swap(sys.ext_table4, rng)?;
            Ok((
                sys.ext_name.to_string(),
                format!("{result1}{result2}が{result3}{result4}"),
                format!("{num1},{num2},{num3},{num4}"),
            ))
        }
    }
}

/// Ruby `extraTable1`（56 は万能命名表を差し込む lambda）。
fn extra_table1(sys: &SystemTables, rng: &mut Randomizer) -> Result<(String, i64), EvalError> {
    let number = rng.roll_d66(D66SortType::Asc)?;
    let template = lookup_by_number(sys.ext_table1, number);
    if template.contains("%{name}") {
        let (_, name, _) = get_name_table_result(sys, 0, rng)?;
        Ok((interpolate(template, &[("name", &name)]), number))
    } else {
        Ok((template.to_string(), number))
    }
}

/// Ruby `KillDeathBusiness#getThemeTableResult`。
fn get_theme_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    let d6 = rng.roll_once(6)?;
    let mut result = String::new();
    match d6 {
        1 => {
            let (one_table_name, one_result, one_d6, one) = get_one_word_table_result(sys, rng)?;
            result.push_str(&format!(
                "[{one_table_name}]を見て一言。\n{one_table_name}({one_d6}) ＞ {one_result}\n＞ "
            ));
            result.push_str(&interpolate(sys.tot_item1, &[("one", &one)]));
        }
        2 => {
            let (word1_table_name, word1_result, word1_d6, word1) =
                get_word_table_result(sys, rng)?;
            let (word2_table_name, word2_result, word2_d6, word2) =
                get_word_table_result(sys, rng)?;
            // Ruby は導入文の両方で word1TableName を使う（word2 ではない）。
            result.push_str(&format!(
                "この[{word1_table_name}]、ひょっとして[{word1_table_name}]かも、どうしてそう思った？\n{word1_table_name}({word1_d6}) ＞ {word1_result}\n{word2_table_name}({word2_d6}) ＞ {word2_result}\n＞ "
            ));
            result.push_str(&interpolate(
                sys.tot_item2,
                &[("word1", &word1), ("word2", &word2)],
            ));
        }
        3 => {
            let vot = sys.vot.roll(rng)?;
            let (word_table_name, word_result, word_d6, word) = get_word_table_result(sys, rng)?;
            let verb_table_name = vot.table_name();
            let verb = vot.last_body();
            let number = vot.value();
            result.push_str(&format!(
                "[{verb_table_name}]した[{word_table_name}]が言いそうなこと。\n{verb_table_name}({number}) ＞ {verb}\n{word_table_name}({word_d6}) ＞ {word_result}\n＞ "
            ));
            result.push_str(&interpolate(
                sys.tot_item3,
                &[("verb", verb), ("word", &word)],
            ));
        }
        4 => {
            let (word1_table_name, word1_result, word1_d6, word1) =
                get_word_table_result(sys, rng)?;
            let (word2_table_name, word2_result, word2_d6, word2) =
                get_word_table_result(sys, rng)?;
            result.push_str(&format!(
                "[{word1_table_name}]が[{word1_table_name}]になった世界ではどんなことが起こる？\n{word1_table_name}({word1_d6}) ＞ {word1_result}\n{word2_table_name}({word2_d6}) ＞ {word2_result}\n＞ "
            ));
            result.push_str(&interpolate(
                sys.tot_item4,
                &[("word1", &word1), ("word2", &word2)],
            ));
        }
        5 => {
            let (word_table_name, word_result, word_d6, word) = get_word_table_result(sys, rng)?;
            result.push_str(&format!(
                "こんな[{word_table_name}]は嫌だ。どんなの？\n{word_table_name}({word_d6}) ＞ {word_result}\n＞ "
            ));
            result.push_str(&interpolate(sys.tot_item5, &[("word", &word)]));
        }
        6 => {
            let lot = sys.lot.roll(rng)?;
            let long_table_name = lot.table_name();
            let long = lot.last_body();
            let number = lot.value();
            result.push_str(&format!(
                "[{long_table_name}]みたいなことを言って下さい。\n{long_table_name}({number}) ＞ {long}\n＞ "
            ));
            result.push_str(&interpolate(sys.tot_item6, &[("long", long)]));
        }
        _ => {}
    }
    Ok((sys.tot_name.to_string(), result, d6.to_string()))
}

/// Ruby `KillDeathBusiness#getOneWordTableResult`。
fn get_one_word_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, i64, String), EvalError> {
    let d6 = rng.roll_once(6)?;
    let (result, one_word) = match d6 {
        1 | 2 => (sys.oot_item1.to_string(), sys.oot_item1.to_string()),
        3 | 4 => (sys.oot_item3.to_string(), sys.oot_item3.to_string()),
        5 | 6 => {
            let (word_table_name, word_result, word_d6, word) = get_word_table_result(sys, rng)?;
            let one_word = interpolate(sys.oot_item5, &[("word", &word)]);
            let result = format!(
                "[{word_table_name}]で検索して出てくる６番目の画像\n{word_table_name}({word_d6}) ＞ {word_result}\n＞ {one_word}"
            );
            (result, one_word)
        }
        _ => (String::new(), String::new()),
    };
    Ok((sys.oot_name.to_string(), result, d6, one_word))
}

/// Ruby `KillDeathBusiness#getWordTableResult`。
///
/// 表名 `"単語表"` は Ruby でもハードコード（韓国語ロケールでも日本語のまま）。
fn get_word_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, i64, String), EvalError> {
    let d6 = rng.roll_once(6)?;
    let table = match d6 {
        1 | 2 => sys.wota,
        3 | 4 => sys.wotb,
        5 | 6 => sys.wotc,
        _ => return Ok(("単語表".to_string(), String::new(), d6, String::new())),
    };
    let rolled = table.roll(rng)?;
    let body = rolled.last_body().to_string();
    Ok(("単語表".to_string(), rolled.to_string(), d6, body))
}

/// Ruby `KillDeathBusiness#getPositiveTableResult`。
fn get_positive_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    let number = rng.roll_sum(1, 6)?;
    let result = match number {
        1 => {
            let size = rng.roll_sum(1, 6)?.to_string();
            interpolate(sys.pot_item1, &[("size", &size)])
        }
        2 => {
            let size = rng.roll_sum(1, 6)?.to_string();
            interpolate(sys.pot_item2, &[("size", &size)])
        }
        3 => {
            let size = rng.roll_sum(2, 6)?.to_string();
            interpolate(sys.pot_item3, &[("size", &size)])
        }
        4 => {
            let size = rng.roll_sum(2, 6)?.to_string();
            interpolate(sys.pot_item4, &[("size", &size)])
        }
        5 => sys.pot_item5.to_string(),
        6 => {
            let size = (rng.roll_sum(1, 6)? - 3).to_string();
            interpolate(sys.pot_item6, &[("size", &size)])
        }
        _ => "1".to_string(),
    };
    Ok((sys.pot_name.to_string(), result, number.to_string()))
}

/// Ruby `KillDeathBusiness#getNegativeTableResult`。
fn get_negative_table_result(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String, String), EvalError> {
    let number = rng.roll_sum(1, 6)?;
    let result = match number {
        1 => sys.not_item1.to_string(),
        2 => sys.not_item2.to_string(),
        3 => {
            let size = rng.roll_sum(1, 6)?.to_string();
            interpolate(sys.not_item3, &[("size", &size)])
        }
        4 => {
            let size = rng.roll_sum(1, 6)?.to_string();
            interpolate(sys.not_item4, &[("size", &size)])
        }
        5 => {
            let size = rng.roll_sum(1, 6)?.to_string();
            interpolate(sys.not_item5, &[("size", &size)])
        }
        6 => sys.not_item6.to_string(),
        _ => "1".to_string(),
    };
    Ok((sys.not_name.to_string(), result, number.to_string()))
}

// ---------------------------------------------------------------------------
// 補助
// ---------------------------------------------------------------------------

/// Ruby `get_table_by_d66_swap`。
fn d66_swap<'a>(
    table: &'a [(i64, &'a str)],
    rng: &mut Randomizer,
) -> Result<(&'a str, i64), EvalError> {
    let number = rng.roll_d66(D66SortType::Asc)?;
    Ok((lookup_by_number(table, number), number))
}

/// Ruby `get_table_by_number`。最初に `number >= index` な項目を返す。
fn lookup_by_number<'a>(table: &'a [(i64, &'a str)], index: i64) -> &'a str {
    for (number, text) in table {
        if *number >= index {
            return text;
        }
    }
    "1"
}

/// Ruby `get_table_by_1d6`。
fn table_1d6<'a>(items: &'a [&'a str], rng: &mut Randomizer) -> Result<(&'a str, i64), EvalError> {
    let num = rng.roll_sum(1, 6)?;
    let text = usize::try_from(num - 1)
        .ok()
        .and_then(|i| items.get(i))
        .copied()
        .unwrap_or("1");
    Ok((text, num))
}

/// i18n `%{name}` 補間。
fn interpolate(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("%{{{key}}}"), value);
    }
    out
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `String#to_i`（先頭の十進数。無ければ 0）。
fn ruby_to_i(s: &str) -> Option<i64> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        Some(0)
    } else {
        digits.parse().ok().or(Some(i64::MAX))
    }
}

include!("KillDeathBusiness_ja_data.rs");

static JA_EST_TABLES: &[EstSubTable] = &[
    EstSubTable {
        name: JA_EST_UNDRESSING_NAME,
        items: JA_EST_UNDRESSING_ITEMS,
    },
    EstSubTable {
        name: JA_EST_VIOLENCE_NAME,
        items: JA_EST_VIOLENCE_ITEMS,
    },
    EstSubTable {
        name: JA_EST_TRAVEL_NAME,
        items: JA_EST_TRAVEL_ITEMS,
    },
    EstSubTable {
        name: JA_EST_LOVE_NAME,
        items: JA_EST_LOVE_ITEMS,
    },
    EstSubTable {
        name: JA_EST_EMOTION_NAME,
        items: JA_EST_EMOTION_ITEMS,
    },
    EstSubTable {
        name: JA_EST_OTHER_GENRE_NAME,
        items: JA_EST_OTHER_GENRE_ITEMS,
    },
];

static JA_RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new(JA_RTT_CAT1_NAME, JA_RTT_CAT1_SKILLS),
    SaiFicCategory::new(JA_RTT_CAT2_NAME, JA_RTT_CAT2_SKILLS),
    SaiFicCategory::new(JA_RTT_CAT3_NAME, JA_RTT_CAT3_SKILLS),
    SaiFicCategory::new(JA_RTT_CAT4_NAME, JA_RTT_CAT4_SKILLS),
    SaiFicCategory::new(JA_RTT_CAT5_NAME, JA_RTT_CAT5_SKILLS),
    SaiFicCategory::new(JA_RTT_CAT6_NAME, JA_RTT_CAT6_SKILLS),
];

static JA_RTT: SaiFicSkillTable = SaiFicSkillTable::new(JA_RTT_CATEGORIES)
    .with_commands(Some("SKLT"), Some("SKLJ"), NO_RTTN_ALIASES)
    .with_formats(SaiFicFormats {
        rtt: JA_RTT_RTT_FORMAT,
        rct: JA_RTT_RCT_FORMAT,
        rttn: DEFAULT_RTTN_FORMAT,
        skill: JA_RTT_S_FORMAT,
    });

static JA_ROLL_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("HST", &JA_HST),
    ("DWT", &JA_DWT),
    ("RWT", &JA_RWT),
    ("VWT", &JA_VWT),
    ("PWT", &JA_PWT),
    ("CWT", &JA_CWT),
    ("FWT", &JA_FWT),
    ("IWT", &JA_IWT),
    ("HWT", &JA_HWT),
    ("SAWT", &JA_SAWT),
    ("LWT", &JA_LWT),
    ("EWT", &JA_EWT),
    ("OSPT", &JA_OSPT),
    ("FSPT", &JA_FSPT),
    ("LOSPT", &JA_LOSPT),
    ("JSPT", &JA_JSPT),
    ("TSPT", &JA_TSPT),
    ("BSPT", &JA_BSPT),
    ("CMT", &JA_CMT),
    ("ERT", &JA_ERT),
    ("WKT", &JA_WKT),
    ("SOUL", &JA_SOUL),
    ("STGT", &JA_STGT),
    ("PCDT", &JA_PCDT),
    ("OHT", &JA_OHT),
    ("PCT1", &JA_PCT1),
    ("PCT2", &JA_PCT2),
    ("PCT3", &JA_PCT3),
    ("PCT4", &JA_PCT4),
    ("PCT5", &JA_PCT5),
    ("PCT6", &JA_PCT6),
    ("PCT7", &JA_PCT7),
    ("ANSPT", &JA_ANSPT),
    ("MASPT", &JA_MASPT),
    ("MOSPT", &JA_MOSPT),
    ("PASPT", &JA_PASPT),
    ("POSPT", &JA_POSPT),
    ("UMSPT", &JA_UMSPT),
    ("WOTA", &JA_WOTA),
    ("WOTB", &JA_WOTB),
    ("WOTC", &JA_WOTC),
    ("VOT", &JA_VOT),
    ("LOT", &JA_LOT),
];

/// ja_jp ロケールの表一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    fumble: JA_FUMBLE,
    special: JA_SPECIAL,
    jd: JdTexts {
        name: JA_JD_NAME,
        warn_over_target: JA_JD_WARN_OVER_TARGET,
        warn_min_target: JA_JD_WARN_MIN_TARGET,
        warn_over_fumble: JA_JD_WARN_OVER_FUMBLE,
        options: JA_JD_OPTIONS,
        dice_value: JA_JD_DICE_VALUE,
        fumble: JA_JD_FUMBLE,
        special: JA_JD_SPECIAL,
        less_than_fumble: JA_JD_LESS_THAN_FUMBLE,
        failure: JA_JD_FAILURE,
        success: JA_JD_SUCCESS,
    },
    st_name: JA_ST_NAME,
    st_format: JA_ST_FORMAT,
    st_table1: JA_ST_TABLE1,
    st_table2: JA_ST_TABLE2,
    name_name: JA_NAME_NAME,
    name_table1: JA_NAME_TABLE1,
    name_table2: JA_NAME_TABLE2,
    name_table3: JA_NAME_TABLE3,
    est_name: JA_EST_NAME,
    est_format: JA_EST_FORMAT,
    est_tables: JA_EST_TABLES,
    hsat_name: JA_HSAT_NAME,
    hsat_abuse1: JA_HSAT_ABUSE1,
    hsat_abuse2: JA_HSAT_ABUSE2,
    hsat_prefix: JA_HSAT_PREFIX,
    hsat_suffix: JA_HSAT_SUFFIX,
    ext_name: JA_EXT_NAME,
    ext_table1: JA_EXT_TABLE1,
    ext_table2: JA_EXT_TABLE2,
    ext_table3: JA_EXT_TABLE3,
    ext_table4: JA_EXT_TABLE4,
    tables: JA_ROLL_TABLES,
    rtt: &JA_RTT,
    wota: &JA_WOTA,
    wotb: &JA_WOTB,
    wotc: &JA_WOTC,
    vot: &JA_VOT,
    lot: &JA_LOT,
    tot_name: JA_TOT_NAME,
    tot_item1: JA_TOT_ITEM1,
    tot_item2: JA_TOT_ITEM2,
    tot_item3: JA_TOT_ITEM3,
    tot_item4: JA_TOT_ITEM4,
    tot_item5: JA_TOT_ITEM5,
    tot_item6: JA_TOT_ITEM6,
    oot_name: JA_OOT_NAME,
    oot_item1: JA_OOT_ITEM1,
    oot_item3: JA_OOT_ITEM3,
    oot_item5: JA_OOT_ITEM5,
    pot_name: JA_POT_NAME,
    pot_item1: JA_POT_ITEM1,
    pot_item2: JA_POT_ITEM2,
    pot_item3: JA_POT_ITEM3,
    pot_item4: JA_POT_ITEM4,
    pot_item5: JA_POT_ITEM5,
    pot_item6: JA_POT_ITEM6,
    not_name: JA_NOT_NAME,
    not_item1: JA_NOT_ITEM1,
    not_item2: JA_NOT_ITEM2,
    not_item3: JA_NOT_ITEM3,
    not_item4: JA_NOT_ITEM4,
    not_item5: JA_NOT_ITEM5,
    not_item6: JA_NOT_ITEM6,
};

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
            .join("test/data/KillDeathBusiness.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/KillDeathBusiness.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/KillDeathBusiness.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KillDeathBusiness.toml must parse");
        assert_eq!(
            data.tests.len(),
            169,
            "case count in test/data/KillDeathBusiness.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KillDeathBusiness",
                "unexpected game system in KillDeathBusiness.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KillDeathBusiness"), &tc.input, &mut src) {
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
                    "FAIL KillDeathBusiness:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KillDeathBusiness cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
