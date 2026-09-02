//! P4で手書き移植した `lib/bcdice/game_system/Villaciel.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意
//! （`HANDWRITTEN_IDS` へ追加すると `mod.rs` とレジストリから落ちるため、
//! 手書き化したシステムの除外機構は上位の整理に委ねている）。
//!
//! 移植したもの:
//! - `Villaciel#eval_game_system_specific_command`
//!   → 判定 `nVBS` / フロンティア `nVF` / 採掘 `nVM` / 宝石加工 `nVG`
//!   → 前職表 `PJ` / ぷちクエスト `PQ` / アクシデント `AC`
//!   → もふもふ表 `MMx` / 釣り表 `Fx` / 不食植物 `IP` / 可食植物 `EP`
//!   → 変異植物 `MP` / 改良種 `IS`
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `D6`。
const D6: i64 = 6;
/// Ruby `LEAST_SUCCESS_ROLL`。
const LEAST_SUCCESS_ROLL: i64 = 4;
/// Ruby `SUCCESS_STR`。
const SUCCESS_STR: &str = " ＞ 成功";
/// Ruby `FAILURE_STR`。
const FAILURE_STR: &str = " ＞ 失敗";
/// Ruby `LEAST_MINING_SUCCESS_ROLL`。
const LEAST_MINING_SUCCESS_ROLL: i64 = 5;
/// Ruby `LEAST_GEM_SUCCESS_ROLL`。
const LEAST_GEM_SUCCESS_ROLL: i64 = 6;

/// Ruby `HELP_MESSAGE`。
const HELP_MESSAGE: &str = r"・判定　　　　　　　　nVBS[>=d]
　[]内省略時は達成数の計算のみ。トライアンフあり。
　n: ダイス数、d: 難易度
・フロンティア判定　　nVF
　n: ダイス数
　nVBSを行い、うでまえ表を参照した結果を表示します。
・採掘スキル判定　　　nVM
　n: ダイス数
　判定に成功した場合、自動的に獲得できるアイテム数も表示されます。
・宝石加工スキル判定　nVG
　n: ダイス数
・前職表　　　　　　　PJ[x]    x=V,A
　[]内は省略可能。
　PJ, PJV: 「蒼天のヴィラシエル」掲載の前職表　PJA: 「白雲のアルメサール」掲載の前職表
・ぷちクエスト表　　　PQ[x]    x=V,A
　[]内は省略可能。
　PQ, PQV: 「蒼天のヴィラシエル」掲載のぷちクエスト表　PQA: 「白雲のアルメサール」掲載のぷちクエスト表
・アクシデント表　　　AC
・もふもふ表　　　　　MMx      x=I,A,V,VV,VA,D
  MMI: 昆虫　MMA: 動物　MMV, MMVV: ヴィラシエル種（「蒼天のヴィラシエル」掲載）　MMVA: ヴィラシエル種（「白雲のアルメサール」掲載）　MMD: 鋼龍種
・釣り表　　　　　　　Fx       x=L,R,W,G,B,C,S
　FL: 湖　FR: 河　FW: 白雲　FG: 灰雲　FB: 黒雲　FC: 共通　FS: 塩湖
・不食植物表　　　　　IP[x]    x=V,A
　IP, IPV: 「蒼天のヴィラシエル」掲載の不食植物表　IPA: 「白雲のアルメサール」掲載の不食植物表
・可食植物表　　　　　EP[x][n] x=V,A
　[]内は省略可能。
　n: 可食植物表番号
　EP[n], EPV[n]: 「蒼天のヴィラシエル」掲載の可食植物表。[]内省略時はnを1D6で決定し、EPVnを実行。ただし、1D6の出目が6ならば、「好きな表を選んでおっけー！」と表示。
　EPA[n]: 「白雲のアルメサール」掲載の可食植物表。[]内省略時は1D6を振り、出目が偶数ならばEPA1、奇数ならばEPA2を実行。
・変異植物表　　　　　MP
・改良種表　　　　　　IS
";

/// `eval_game_system_specific_command` の `case` が使う正規表現。
/// 原典どおりアンカー無し（部分一致）。コマンドは upcase 済み。
struct Patterns {
    vbs: Regex,
    vf: Regex,
    vm: Regex,
    vg: Regex,
    pj: Regex,
    pq: Regex,
    mm: Regex,
    mmv: Regex,
    ip: Regex,
    ep: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        vbs: Regex::new(r"(\d+)VBS(>=(\d+))?").expect("valid regex"),
        vf: Regex::new(r"(\d+)VF").expect("valid regex"),
        vm: Regex::new(r"(\d+)VM").expect("valid regex"),
        vg: Regex::new(r"(\d+)VG").expect("valid regex"),
        pj: Regex::new(r"PJ([VA]?)").expect("valid regex"),
        pq: Regex::new(r"PQ([VA]?)").expect("valid regex"),
        mm: Regex::new(r"MM([IAD]|V[VA]?)").expect("valid regex"),
        mmv: Regex::new(r"MMV([VA]?)").expect("valid regex"),
        ip: Regex::new(r"IP([VA]?)").expect("valid regex"),
        ep: Regex::new(r"EP([VA]?)(\d?)").expect("valid regex"),
    })
}

/// Ruby の `String#to_i`。`i64` に収まらない入力は飽和させる。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// 2次元表の行。範囲外は空行（`get_table_by_1d6` が `["1", 0]` を返す）。
fn table_row(table: &[&'static [&'static str]], index: i64) -> &'static [&'static str] {
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i).copied())
        .unwrap_or(&[])
}

/// 6x6 表の束から 1 枚。`chart_id` は 1 始まり。
fn chart_by_id(
    charts: &[&'static [&'static [&'static str]]],
    chart_id: i64,
) -> &'static [&'static [&'static str]] {
    usize::try_from(chart_id - 1)
        .ok()
        .and_then(|i| charts.get(i).copied())
        .unwrap_or(&[])
}

/// Ruby `BCDice::GameSystem::Villaciel`（ID: `Villaciel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Villaciel;

impl GameSystem for Villaciel {
    fn id(&self) -> &'static str {
        "Villaciel"
    }

    fn name(&self) -> &'static str {
        "蒼天のヴィラシエル"
    }

    fn sort_key(&self) -> &'static str {
        "そうてんのういらしえる"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+VBS(>=\d+)?",
            r"\d+VF",
            r"\d+VM",
            r"\d+VG",
            "PJ[VA]?",
            "PQ[VA]?",
            "AC",
            "MM([IAD]|V[VA]?)",
            "F[LRWGBCS]",
            "IP[VA]?",
            "EP[VA]?",
            "MP",
            "IS",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `Villaciel#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let p = patterns();
    if p.vbs.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(resolute_action(
            command, rng,
        )?)));
    }
    if p.vf.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(resolute_frontier_action(
            command, rng,
        )?)));
    }
    if p.vm.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(resolute_mining_action(
            command, rng,
        )?)));
    }
    if p.vg.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(
            resolute_cutting_gem_action(command, rng)?,
        )));
    }
    if p.pj.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(use_previous_job_chart(
            command, rng,
        )?)));
    }
    if p.pq.is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(use_petit_quest_chart(
            command, rng,
        )?)));
    }
    if command == "AC" {
        return Ok(Some(SpecificCommandOutput::text(use_accident_chart(rng)?)));
    }
    if p.mm.is_match(command) {
        return Ok(use_mohumohu_chart(command, rng)?.map(SpecificCommandOutput::text));
    }
    // Ruby `when /F[LRWGBCS]/` のあと exact 照合。不一致は nil。
    if let Some(text) = use_fishing_chart(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if p.ip.is_match(command) {
        return Ok(use_inedible_plant_chart(command, rng)?.map(SpecificCommandOutput::text));
    }
    if p.ep.is_match(command) {
        // 不正な表番号は空文字。`dice_command` が nil に畳む。
        return Ok(Some(SpecificCommandOutput::text(use_edible_plant_chart(
            command, rng,
        )?)));
    }
    if command == "MP" {
        return Ok(Some(SpecificCommandOutput::text(use_6x6_chart(
            MUTANT_PLANT_CHART,
            "変異植物表",
            rng,
        )?)));
    }
    if command == "IS" {
        return Ok(Some(SpecificCommandOutput::text(use_6x6_chart(
            IMPROVED_SPECIES_CHART,
            "改良種表",
            rng,
        )?)));
    }
    Ok(None)
}

/// Ruby `Villaciel#derive_achievement`。
fn derive_achievement(
    num_dices: i64,
    command: &str,
    rng: &mut Randomizer,
) -> Result<(i64, String), EvalError> {
    let dice_list = rng.roll_barabara(num_dices, D6)?;
    let dice_str = join_dice(&dice_list);
    let num_triumph_dices = dice_list.iter().filter(|&&dice| dice == 6).count() as i64;
    let num_successes = dice_list
        .iter()
        .filter(|&&dice| dice >= LEAST_SUCCESS_ROLL)
        .count() as i64;
    let achievement = num_successes + num_triumph_dices;
    let output = format!("({command}) ＞ [{dice_str}] ＞ 達成数: {achievement}");
    Ok((achievement, output))
}

/// Ruby `Villaciel#resolute_action`。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().vbs.captures(command).expect("matched");
    let num_dices = to_i(&caps[1]);
    let (achievement, output) = derive_achievement(num_dices, command, rng)?;
    if caps.get(2).is_none() {
        return Ok(output);
    }
    let difficulty = to_i(&caps[3]);
    if achievement >= difficulty {
        Ok(output + SUCCESS_STR)
    } else {
        Ok(output + FAILURE_STR)
    }
}

/// Ruby `Villaciel#resolute_frontier_action`。
fn resolute_frontier_action(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().vf.captures(command).expect("matched");
    let num_dices = to_i(&caps[1]);
    let (achievement, output) = derive_achievement(num_dices, command, rng)?;
    let chart_index = match achievement {
        0..=2 => achievement,
        3 | 4 => 3,
        5..=8 => 4,
        _ => 5,
    };
    let skill = usize::try_from(chart_index)
        .ok()
        .and_then(|i| SKILL_CHART.get(i).copied())
        .unwrap_or("");
    Ok(format!("{output} ＞ {skill}"))
}

/// Ruby `Villaciel#resolute_difficult_action`。
fn resolute_difficult_action(
    num_dices: i64,
    least_success_roll: i64,
    command: &str,
    rng: &mut Randomizer,
) -> Result<(String, bool), EvalError> {
    let dice_list = rng.roll_barabara(num_dices, D6)?;
    let dice_str = join_dice(&dice_list);
    // Ruby `dice_list.max()`。0個のときは TypeError だが、ここは失敗扱いにする。
    let largest_roll = dice_list.iter().copied().max().unwrap_or(0);
    let is_successful = largest_roll >= least_success_roll;
    let mut output = format!("({command}) ＞ [{dice_str}]");
    output.push_str(if is_successful {
        SUCCESS_STR
    } else {
        FAILURE_STR
    });
    Ok((output, is_successful))
}

/// Ruby `Villaciel#resolute_mining_action`。
fn resolute_mining_action(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().vm.captures(command).expect("matched");
    let num_dices = to_i(&caps[1]);
    let (output, is_successful) =
        resolute_difficult_action(num_dices, LEAST_MINING_SUCCESS_ROLL, command, rng)?;
    if !is_successful {
        return Ok(output);
    }
    let roll_result = rng.roll_once(D6)?;
    Ok(format!(
        "{output} ＞ (1D6) ＞ [{roll_result}] ＞ アイテムを{roll_result}個獲得"
    ))
}

/// Ruby `Villaciel#resolute_cutting_gem_action`。
fn resolute_cutting_gem_action(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().vg.captures(command).expect("matched");
    let num_dices = to_i(&caps[1]);
    let (output, _) = resolute_difficult_action(num_dices, LEAST_GEM_SUCCESS_ROLL, command, rng)?;
    Ok(output)
}

/// Ruby `Villaciel#use_previous_job_chart`。
fn use_previous_job_chart(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().pj.captures(command).expect("matched");
    let symbol = match caps.get(1).map(|m| m.as_str()).unwrap_or("") {
        "" => "V",
        s => s,
    };
    let roll_result1 = rng.roll_once(D6)?;
    let (chart_text, roll_result2, chart_title) = match symbol {
        "A" => {
            let row = table_row(ARMESEAR_PREVIOUS_JOB_CHART, (roll_result1 - 1) / 2);
            let (text, roll2) = get_table_by_1d6(row, rng)?;
            (text, roll2, "前職表（アルメサール）")
        }
        _ => {
            let row = table_row(VILLACIEL_PREVIOUS_JOB_CHART, (roll_result1 - 1) / 3);
            let (text, roll2) = get_table_by_1d6(row, rng)?;
            (text, roll2, "前職表（ヴィラシエル）")
        }
    };
    Ok(format!(
        "{chart_title} ＞ [{roll_result1},{roll_result2}] ＞ {chart_text}"
    ))
}

/// Ruby `Villaciel#use_petit_quest_chart`。
fn use_petit_quest_chart(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().pq.captures(command).expect("matched");
    let symbol = match caps.get(1).map(|m| m.as_str()).unwrap_or("") {
        "" => "V",
        s => s,
    };
    let roll_result1 = rng.roll_once(D6)?;
    let (chart_text, roll_result2, chart_title) = match symbol {
        "A" => {
            let row = table_row(ARMESEAR_PETIT_QUEST_CHART, (roll_result1 - 1) / 2);
            let (text, roll2) = get_table_by_1d6(row, rng)?;
            (text, roll2, "ぷちクエスト表（アルメサール）")
        }
        _ => {
            let chart_index = match roll_result1 {
                1 | 2 => 0,
                3 | 4 => 1,
                5 => 2,
                6 => 3,
                _ => 0,
            };
            let row = table_row(VILLACIEL_PETIT_QUEST_CHART, chart_index);
            let (text, roll2) = get_table_by_1d6(row, rng)?;
            (text, roll2, "ぷちクエスト表（ヴィラシエル）")
        }
    };
    Ok(format!(
        "{chart_title} ＞ [{roll_result1},{roll_result2}] ＞ {chart_text}"
    ))
}

/// Ruby `Villaciel#use_accident_chart`。
fn use_accident_chart(rng: &mut Randomizer) -> Result<String, EvalError> {
    let (chart_text, roll_result) = get_table_by_1d6(ACCIDENT_CHART, rng)?;
    Ok(format!("アクシデント表 ＞ [{roll_result}] ＞ {chart_text}"))
}

/// Ruby `Villaciel#use_6x6_chart`。
fn use_6x6_chart(
    chart: &'static [&'static [&'static str]],
    chart_name: &str,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let y_roll = rng.roll_once(D6)?;
    let row = table_row(chart, y_roll - 1);
    let (cell_text, x_roll) = get_table_by_1d6(row, rng)?;
    Ok(format!(
        "{chart_name} ＞ [{y_roll},{x_roll}] ＞ 下{y_roll}マス、右{x_roll}マス ＞ {cell_text}"
    ))
}

/// Ruby `Villaciel#use_mohumohu_chart`。
fn use_mohumohu_chart(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match command {
        "MMI" => Ok(Some(use_6x6_chart(
            MOHUMOHU_INSECT_CHART,
            "もふもふ表・昆虫",
            rng,
        )?)),
        "MMA" => Ok(Some(use_6x6_chart(
            MOHUMOHU_ANIMAL_CHART,
            "もふもふ表・動物",
            rng,
        )?)),
        "MMD" => Ok(Some(use_6x6_chart(
            MOHUMOHU_DRAGON_CHART,
            "もふもふ表・鋼龍種",
            rng,
        )?)),
        _ => {
            let Some(caps) = patterns().mmv.captures(command) else {
                return Ok(None);
            };
            let symbol = match caps.get(1).map(|m| m.as_str()).unwrap_or("") {
                "" => "V",
                s => s,
            };
            match symbol {
                "A" => Ok(Some(use_6x6_chart(
                    MOHUMOHU_VILLACIEL2_CHART,
                    "もふもふ表・ヴィラシエル種（アルメサール）",
                    rng,
                )?)),
                _ => {
                    let y_roll = rng.roll_once(D6)?;
                    let row = table_row(MOHUMOHU_VILLACIEL_CHART, 1 - y_roll % 2);
                    let (cell_text, x_roll) = get_table_by_1d6(row, rng)?;
                    let parity = if y_roll % 2 == 0 { "偶数" } else { "奇数" };
                    Ok(Some(format!(
                        "もふもふ表・ヴィラシエル種（ヴィラシエル） ＞ [{y_roll},{x_roll}] ＞ 下{parity}、右{x_roll}マス ＞ {cell_text}"
                    )))
                }
            }
        }
    }
}

/// Ruby `Villaciel#use_fishing_chart`。
fn use_fishing_chart(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let (chart, name) = match command {
        "FL" => (FISHING_LAKE_CHART, "釣り・湖表"),
        "FR" => (FISHING_RIVER_CHART, "釣り・河表"),
        "FW" => (FISHING_WHITE_CHART, "釣り・白雲表"),
        "FG" => (FISHING_GRAY_CHART, "釣り・灰雲表"),
        "FB" => (FISHING_BLACK_CHART, "釣り・黒雲表"),
        "FC" => (FISHING_COMMON_CHART, "釣り・共通表"),
        "FS" => (FISHING_SALT_LAKE_CHART, "釣り・塩湖表"),
        _ => return Ok(None),
    };
    Ok(Some(use_6x6_chart(chart, name, rng)?))
}

/// Ruby `Villaciel#use_inedible_plant_chart`。
fn use_inedible_plant_chart(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let caps = patterns().ip.captures(command).expect("matched");
    let symbol = match caps.get(1).map(|m| m.as_str()).unwrap_or("") {
        "" => "V",
        s => s,
    };
    match symbol {
        "A" => Ok(Some(use_6x6_chart(
            INEDIBLE_PLANT2_CHART,
            "不食植物表（アルメサール）",
            rng,
        )?)),
        "V" => Ok(Some(use_6x6_chart(
            INEDIBLE_PLANT_CHART,
            "不食植物表（ヴィラシエル）",
            rng,
        )?)),
        _ => Ok(None),
    }
}

/// Ruby `Villaciel#use_villaciel_edible_plant_chart`。
fn use_villaciel_edible_plant_chart(
    chart_id: i64,
    prefix: &str,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let chart = chart_by_id(EDIBLE_PLANT_CHARTS, chart_id);
    let body = use_6x6_chart(chart, &format!("可食植物表{chart_id}（ヴィラシエル）"), rng)?;
    Ok(format!("{prefix}{body}"))
}

/// Ruby `Villaciel#use_armesear_edible_plant_chart`。
fn use_armesear_edible_plant_chart(
    chart_id: i64,
    prefix: &str,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let chart = chart_by_id(EDIBLE_PLANT2_CHARTS, chart_id);
    let body = use_6x6_chart(chart, &format!("可食植物表{chart_id}（アルメサール）"), rng)?;
    Ok(format!("{prefix}{body}"))
}

/// Ruby `Villaciel#use_edible_plant_chart`。
fn use_edible_plant_chart(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let caps = patterns().ep.captures(command).expect("matched");
    let letter = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let symbol = if letter.is_empty() { "V" } else { letter };
    let n = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    match symbol {
        "V" => {
            if n.is_empty() {
                let roll_result = rng.roll_once(D6)?;
                if roll_result == D6 {
                    return Ok("(1D6) ＞ [6] ＞ 好きな表を選んでおっけー！".to_owned());
                }
                use_villaciel_edible_plant_chart(
                    roll_result,
                    &format!("(1D6) ＞ [{roll_result}] ＞ "),
                    rng,
                )
            } else {
                let chart_id = to_i(n);
                if (1..=5).contains(&chart_id) {
                    use_villaciel_edible_plant_chart(chart_id, "", rng)
                } else {
                    Ok(String::new())
                }
            }
        }
        "A" => {
            if n.is_empty() {
                let roll_result = rng.roll_once(D6)?;
                let chart_id = if roll_result % 2 == 0 { 1 } else { 2 };
                use_armesear_edible_plant_chart(
                    chart_id,
                    &format!("(1D6) ＞ [{roll_result}] ＞ "),
                    rng,
                )
            } else {
                let chart_id = to_i(n);
                if chart_id == 1 || chart_id == 2 {
                    use_armesear_edible_plant_chart(chart_id, "", rng)
                } else {
                    Ok(String::new())
                }
            }
        }
        _ => Ok(String::new()),
    }
}

/// Ruby `Base#get_table_by_1d6`（= `get_table_by_nDx(table, 1, 6)`）。
///
/// 項目が無ければ Ruby と同じく `["1", 0]` を返す。
fn get_table_by_1d6(
    table: &[&'static str],
    rng: &mut Randomizer,
) -> Result<(&'static str, i64), EvalError> {
    let num = rng.roll_sum(1, 6)?;
    let text = usize::try_from(num - 1)
        .ok()
        .and_then(|i| table.get(i))
        .copied();
    match text {
        Some(text) => Ok((text, num)),
        None => Ok(("1", 0)),
    }
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/Villaciel.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `SKILL_CHART`。
static SKILL_CHART: &[&str] = &[
    "左に3マス、上に3マス動かす",
    "左に2マス、上に2マス動かす",
    "右か下に1マス動かしてもよい",
    "右に1マス、下に1マス動かす",
    "好きな方向に最大で3マス動かしてもよい（1マスでも良い）",
    "好きな方向に最大で5マス動かしてもよい（1〜3マスでもよい）",
];

/// Ruby `VILLACIEL_PREVIOUS_JOB_CHART`。
static VILLACIEL_PREVIOUS_JOB_CHART: &[&[&str]] = &[
    &[
        "農家: 知力+1 器用さ+1 開拓／1Lv",
        "漁師: 知力+1 ひらめき+1 釣り／1Lv",
        "狩人: 武力+1 ひらめき+1 穴掘り／1Lv",
        "鍛冶職人: 武力+1 器用さ+1 採掘／1Lv",
        "牧場主: 仲良し+2 開拓／1Lv",
        "採掘師: 器用さ+1 ひらめき+1 採掘／1Lv",
    ],
    &[
        "家事手伝い: 器用さ+1 仲良し+1 調理／1Lv",
        "調理師: 知力+1 ひらめき+1 調理／1Lv",
        "細工師: 器用さ+2 採掘／1Lv",
        "大工: 武力+1 器用さ+1 木こり／1Lv",
        "荒くれ者: 武力+2 穴掘り／1Lv",
        "王国騎士: 武力+1 知力+1 木こり／1Lv",
    ],
];

/// Ruby `ARMESEAR_PREVIOUS_JOB_CHART`。
static ARMESEAR_PREVIOUS_JOB_CHART: &[&[&str]] = &[
    &[
        "農家: 知力+1 器用さ+1 開拓／1Lv",
        "漁師: 知力+1 ひらめき+1 釣り／1Lv",
        "狩人: 武力+1 ひらめき+1 穴掘り／1Lv",
        "鍛冶職人: 武力+1 器用さ+1 採掘／1Lv",
        "牧場主: 仲良し+2 開拓／1Lv",
        "採掘師: 器用さ+1 ひらめき+1 採掘／1Lv",
    ],
    &[
        "羊飼い: 仲良し+2 もふもふ／1Lv",
        "芽拾い: 知力+1 武力+1 採集／1Lv",
        "服屋見習い: 器用さ+2 裁縫／1Lv",
        "革細工見習い: 知力+2 裁縫／1Lv",
        "商人: 知力+1 仲良し+1 基礎になるスキル／1Lv",
        "旅人: 武力+1 知力+1 基礎になるスキル／1Lv",
    ],
    &[
        "家事手伝い: 器用さ+1 仲良し+1 調理／1Lv",
        "調理師: 知力+1 ひらめき+1 調理／1Lv",
        "細工師: 器用さ+2 採掘／1Lv or 調合・細工／1Lv",
        "大工: 武力+1 器用さ+1 木こり／1Lv",
        "荒くれ者: 武力+2 穴掘り／1Lv",
        "王国騎士: 武力+1 知力+1 木こり／1Lv",
    ],
];

/// Ruby `VILLACIEL_PETIT_QUEST_CHART`。
static VILLACIEL_PETIT_QUEST_CHART: &[&[&str]] = &[
    &[
        "家の補強のために: 【目的：木を1個納品】【報酬：各自2プサイ】見えを張っていい木材で家を作ったら木材が枯渇しちまった。頼む、原木を分けてくれないか？",
        "孫のために: 【目的：花を1個納品】【報酬：各自2プサイ】綺麗な花があればいい色に染められるだろうと思うてな。孫のために必要なの。",
        "人間界の草: 【目的：草を2個納品】【報酬：各自3プサイ】魔界にはない草が生えていると噂で聞いたことがある。その草がほしい。",
        "種の生存のために: 【目的：可食植物（改良種を除く）を1個納品】【報酬：各自1プサイ】育ちが悪い同種の植物と掛け合わせてみたいのでサンプルがほしい。",
        "にんげんのたべもの！: 【目的：可食植物（改良種を除く）を1個納品】【報酬：各自2プサイ】ひゅーいあはなにをたべるの！ たべたい！",
        "まかいのたべものって？: 【目的：可食植物の改良種を2個納品】【報酬：各自3プサイ】まぞくさんはなにたべるですか！ おしえてください。",
    ],
    &[
        "おうちなおしたいの！: 【目的：石材を1個納品】【報酬：各自1プサイ】おうちがぼろぼろだから、ママのかわりになおしたいの。",
        "娘の結婚式に必要なんだ！: 【目的：宝石を2個納品】【報酬：各自3プサイ】ちょっとさきなんですが、娘が結婚するので結婚式用の宝石を集めています。",
        "金属がたりない！: 【目的：金属を1個納品】【報酬：各自2プサイ】いい武器にはいい金属を。今回必要なのは……。",
        "村の聖堂を直したいんだ！: 【目的：石材を1個納品】【報酬：各自2プサイ】聖堂を直していたが石材がたりない！",
        "弟の甲冑に使うんだ！: 【目的：金属を2個納品】【報酬：各自3プサイ】最近、近くの鉱山から「ある金属」が姿を消した。",
        "おねえちゃんのたんじょうびに: 【目的：宝石を1個納品】【報酬：各自2プサイ】たんじょうびぷれぜんとにほうせきあげたらおねえちゃんよろこぶかな？",
    ],
    &[
        "パパのために: 【目的：木材の家具を1個納品】【報酬：各自2プサイ】はたらいてばっかりのパパにプレゼントしたいの。おねがいします！",
        "癒やされたい……: 【目的：石材の家具を1個納品】【報酬：各自2プサイ】仕事時間は短いとはいえ、激務。めちゃつらい。癒しになる家具がほしい。",
        "いい家具に囲まれてみたい: 【目的：金属の家具を1個納品】【報酬：各自2プサイ】開拓も最高だけど、他の島の人とも交流したい。人を呼べるような家を作るためには最高の家具が必要！",
        "家具の在庫不足: 【目的：木材の装飾品を1個納品】【報酬：各自3プサイ】困ったことに職人に逃げられた！ このままじゃ、お店開けない！！",
        "と、ともだちにあげるの！: 【目的：石材の装飾品を1個納品】【報酬：各自3プサイ】えっと、お、おきにいりのともだちがいるんだ。そ、そのこのたんじょうびだから、プレゼントしたくって。",
        "親の木に飾りを: 【目的：金属の装飾品を1個納品】【報酬：各自3プサイ】元気のない親の木を心配してペッコ達が大騒ぎしているんだ。君はいつまでも美しいよと伝えたくてね。一つ助力をお願いするよ。",
    ],
    &[
        "そちらの河魚を食してみたい: 【目的：河魚を2個納品】【報酬：各自3プサイ】おいしい河魚がいるときいたことがあるのです。さぁ、はやく、釣ってきてくださいまし。",
        "研究に使用したい: 【目的：湖魚を1個納品】【報酬：各自1プサイ】そちらの世界にある同名の魚が本当にこちらの世界にいるものと一緒か確かめたいのです。",
        "しろいくもにすむおさかながみたい！: 【目的：白雲の雲魚を1個納品】【報酬：各自2プサイ】こっちにはしろいくもってなかなかないの！ しろいくものおさかな、たべてみたいな。",
        "釣り師がいないのでお魚がほしい: 【目的：灰雲の雲魚を2個納品】【報酬：各自3プサイ】野菜や肉もいいが魚も食べたい……。頼む、魚を釣ってきてくれないか？",
        "まっくろなくもにすむおさかな！: 【目的：黒雲の雲魚を1個納品】【報酬：各自2プサイ】まっくろなくもにはどんなさかながすんでるの？ みせて、みせて！",
        "人間界では見られない魚が見たい！: 【目的：共通の雲魚を1個納品】【報酬：各自2プサイ】他の魚の雲を利用して泳ぎ回る魚がいると聞いたよ。ぜひ見せてほしいな。",
    ],
];

/// Ruby `ARMESEAR_PETIT_QUEST_CHART`。
static ARMESEAR_PETIT_QUEST_CHART: &[&[&str]] = &[
    &[
        "お祭り用の布が足りないの！: 【目的：布を2個納品】【報酬：各自4プサイ】お祭り前なのに、布職人が腰を痛めちゃったの！",
        "お洋服がぼろぼろになっちゃったの: 【目的：布を1個納品】【報酬：各自2プサイ】おばあちゃんに作ってもらった服がボロボロになっちゃったから、なおしたいの。",
        "ぎっくり腰からのヘルプ: 【目的：薪を3個納品】【報酬：各自3プサイ】仕事してたらぎっくり腰になっちゃったのだ。頼むのだ。",
        "不調には栄養たっぷりのミルクを: 【目的：ミルクを1個納品】【報酬：各自3プサイ】体調を崩しちゃったの。栄養満点のミルクを頂戴。",
        "材料がたりない！: 【目的：？？？の粗皮を1個納品】【報酬：各自3プサイ】革細工師を目指してるんだけど、皮が足りないんだ。種類は問わないから、早めに頼むよ。",
        "愛しのガードナーのために: 【目的：？？？の肉を1個納品】【報酬：各自3プサイ】ガードナーの調子が悪いから、栄養をつけさせたいんだ。肉はなんだっていい、とびっきりのを頼むよ。",
    ],
    &[
        "灯火をひとつ: 【目的：キャンドルを1個納品】【報酬：各自3プサイ】家の裏に知らない建物があるんだ。まっくらだから明かりが必要で……。",
        "布の色を頂戴: 【目的：染料を1個納品】【報酬：各自2プサイ】んー、コンテストのために布を織ったのだけど、色が決められないんだ。お願いするよ。",
        "きれいなのお花を: 【目的：花を1個納品】【報酬：各自2プサイ】パパの誕生日プレゼントを妹と作りたいんだ。お願いできる？",
        "旅立ちのために: 【目的：衣類を1個納品】【報酬：各自15プサイ】旅立つ弟に服をプレゼントしたいんだ。",
        "納品物が足りない！: 【目的：革を1個納品】【報酬：各自4プサイ】どうしても納品する皮がたりない……頼む、なんとか用意できないか？",
        "求）照明: 【目的：照明を1個納品】【報酬：各自10プサイ】引っ越しする最中に照明を壊してしまった！ 明日から明かりがないのはつらい……。作ってくれないか？",
    ],
    &[
        "装備の修復のため: 【目的：革を2個納品】【報酬：各自5プサイ】大事な装備が壊れちゃったんだ！ 直すのに必要なんだけど、革を持っているかい？",
        "主に祝いの品を: 【目的：敷物を1個納品】【報酬：各自15プサイ】誕生日を迎える主にささやかなながらわたしからも祝いの品を送りたいのです。",
        "手料理を求めて: 【目的：出来栄え5の料理を1個納品】【報酬：各自5プサイ】たまには誰かの料理が食べたいんだ。",
        "釣り竿が折れちゃって……: 【目的：塩魚を2個納品】【報酬：各自3プサイ】釣り竿が折れちゃったから釣りができないんだ。一匹頼める？",
        "蝋がほしいの: 【目的：蝋を1個納品】【報酬：各自2プサイ】お兄ちゃんとパパの誕生日プレゼントを作るの。見つからないからお願いできる？",
        "美しさを求めて: 【目的：アルメサール産の花を1個納品】【報酬：各自3プサイ】美しいお花を摘んで来てくださらない？ 美のために必要でしてよ。",
    ],
];

/// Ruby `ACCIDENT_CHART`。
static ACCIDENT_CHART: &[&str] = &[
    "飛び猪襲来！: 空飛ぶ猪が浮遊島めがけて突撃してきた！ 建物が粉砕される前に迎撃だ！（「蒼天のヴィラシエル」P.46）",
    "嵐がくるぞ！: 嵐が来るらしいぞ！ どれだけ対策できるかが鍵だ！（「蒼天のヴィラシエル」P.47）",
    "雨が降らないぞ！: おかしいなぁ、雨が降らないぞぉ……？ こうなったら雨乞いの踊りだ！（「蒼天のヴィラシエル」P.48）",
    "トビウオ流星群: きらきら光る流れ星……いや待て！ あれはトビウオの群れだー！？（「蒼天のヴィラシエル」P.49）",
    "すごい雷雨: すごい。ごろごろばりばり聞こえてくる。これは早々に対策しないと直撃するぞ！（「蒼天のヴィラシエル」P.50）",
    "野菜泥棒出現！: 畑の野菜が盗まれているぞ……？ これは犯人を捕まえないと！（「蒼天のヴィラシエル」P.51）",
];

/// Ruby `MOHUMOHU_INSECT_CHART`。
static MOHUMOHU_INSECT_CHART: &[&[&str]] = &[
    &[
        "小さな虫",
        "小さな虫",
        "カマキリ",
        "カマキリ",
        "バッタ",
        "クワガタ",
    ],
    &[
        "小さな虫",
        "カラスアゲハ",
        "カマキリ",
        "バッタ",
        "オオスカシバ",
        "カイコ",
    ],
    &[
        "ハンミョウ",
        "カラスアゲハ",
        "カマキリ",
        "バッタ",
        "カイコ",
        "トンボ",
    ],
    &[
        "ハンミョウ",
        "カラスアゲハ",
        "カラスアゲハ",
        "チッチハチ",
        "トンボ",
        "トンボ",
    ],
    &[
        "クワガタ",
        "カラスアゲハ",
        "チッチハチ",
        "チッチハチ",
        "アリ",
        "アリ",
    ],
    &[
        "クワガタ",
        "チッチハチ",
        "チッチハチ",
        "チッチハチ",
        "アリ",
        "アリ",
    ],
];

/// Ruby `MOHUMOHU_ANIMAL_CHART`。
static MOHUMOHU_ANIMAL_CHART: &[&[&str]] = &[
    &["トリサン", "トリサン", "ブタ", "ヒツジ", "タヌキ", "タヌキ"],
    &["トリサン", "ブタ", "ヒツジ", "ウッシ", "キツネ", "タヌキ"],
    &["ブタ", "オグマ", "ヒツジ", "キツネ", "キツネ", "アタウサギ"],
    &[
        "ブタ",
        "ヒツジ",
        "ヒツジ",
        "リス",
        "シシ",
        "ヴィラシエル種(MMV)",
    ],
    &[
        "ウッシ",
        "ウサギ",
        "ウサギ",
        "シシ",
        "アタウサギ",
        "オオカミ",
    ],
    &[
        "ウッシ",
        "オグマ",
        "クーマ",
        "シシ",
        "オオカミ",
        "ヴィラシエル種(MMV)",
    ],
];

/// Ruby `MOHUMOHU_VILLACIEL_CHART`。
/// 2行（奇数/偶数）。
static MOHUMOHU_VILLACIEL_CHART: &[&[&str]] = &[
    &["ウドン", "ウドン", "オボン", "オボン", "オボン", "オワン"],
    &["ウドン", "ウドン", "オボン", "オワン", "オワン", "オワン"],
];

/// Ruby `MOHUMOHU_VILLACIEL2_CHART`。
static MOHUMOHU_VILLACIEL2_CHART: &[&[&str]] = &[
    &[
        "すねーくあし",
        "すねーくあし",
        "すねーくあし",
        "ウタヒ",
        "オオトリサン",
        "オオトリサン",
    ],
    &[
        "すねーくあし",
        "すねーくあし",
        "ホネホネ",
        "オオトリサン",
        "アマアマガニ",
        "ホワホワ",
    ],
    &[
        "すねーくあし",
        "ホネホネ",
        "オオトリサン",
        "ウタヒ",
        "アマアマガニ",
        "ペロリ",
    ],
    &[
        "オオトリサン",
        "オオトリサン",
        "ホネホネ",
        "ホネホネ",
        "ホワホワ",
        "アマアマガニ",
    ],
    &[
        "ホネホネ",
        "ウタヒ",
        "アマアマガニ",
        "ペロリ",
        "ペロリ",
        "ペロリ",
    ],
    &[
        "オオトリサン",
        "ホワホワ",
        "ホワホワ",
        "アマアマガニ",
        "ペロリ",
        "ペロリ",
    ],
];

/// Ruby `MOHUMOHU_DRAGON_CHART`。
static MOHUMOHU_DRAGON_CHART: &[&[&str]] = &[
    &[
        "モドモドリス",
        "テロメ",
        "モドモドリス",
        "オジサン",
        "オジサン",
        "グロッチ",
    ],
    &[
        "テロメ",
        "モドモドリス",
        "オジサン",
        "テロメ",
        "ニホンツノ",
        "グロッチ",
    ],
    &[
        "テロメ",
        "グロッチ",
        "グロッチ",
        "グロッチ",
        "オジサン",
        "コディ",
    ],
    &[
        "モドモドリス",
        "グロッチ",
        "ニホンツノ",
        "テロメ",
        "テーリー",
        "ケラプス",
    ],
    &[
        "オジサン",
        "テロメ",
        "テロメ",
        "コディ",
        "コディ",
        "ケラプス",
    ],
    &[
        "コディ",
        "テーリー",
        "テーリー",
        "コディ",
        "ケラプス",
        "アサール・ゴッツ",
    ],
];

/// Ruby `FISHING_LAKE_CHART`。
static FISHING_LAKE_CHART: &[&[&str]] = &[
    &[
        "ヤマアイズリ",
        "ヤマアイズリ",
        "ヤマアイズリ",
        "シコウチャ",
        "シコウチャ",
        "ハナロクショウ",
    ],
    &[
        "ヤマアイズリ",
        "ヤマアイズリ",
        "ヤマアイズリ",
        "シコウチャ",
        "ハナロクショウ",
        "ハナロクショウ",
    ],
    &[
        "ヤマアイズリ",
        "ヤマアイズリ",
        "シコウチャ",
        "シコウチャ",
        "ハナモエギ",
        "トノチャ",
    ],
    &[
        "ヤマアイズリ",
        "カラスアゲハ",
        "シコウチャ",
        "ハナロクショウ",
        "トノチャ",
        "ハナモエギ",
    ],
    &[
        "シコウチャ",
        "シコウチャ",
        "ハナロクショウ",
        "ハナロクショウ",
        "トノチャ",
        "ハナモエギ",
    ],
    &[
        "シコウチャ",
        "ハナロクショウ",
        "トノチャ",
        "トノチャ",
        "ハナモエギ",
        "シンペキ",
    ],
];

/// Ruby `FISHING_RIVER_CHART`。
static FISHING_RIVER_CHART: &[&[&str]] = &[
    &[
        "ケイカンセキ",
        "ケイカンセキ",
        "ケイカンセキ",
        "ケイカンセキ",
        "カナリア",
        "イワヌ",
    ],
    &[
        "ケイカンセキ",
        "ケイカンセキ",
        "カナリア",
        "カナリア",
        "カナリア",
        "イワヌ",
    ],
    &[
        "ケイカンセキ",
        "ケイカンセキ",
        "カナリア",
        "イワヌ",
        "イワヌ",
        "ヤマブキ",
    ],
    &[
        "ケイカンセキ",
        "カナリア",
        "イワヌ",
        "アメイロ",
        "アメイロ",
        "ヤマブキ",
    ],
    &[
        "カナリア",
        "カナリア",
        "イワヌ",
        "アメイロ",
        "ヤマブキ",
        "ヤマブキ",
    ],
    &[
        "カナリア",
        "イワヌ",
        "アメイロ",
        "アメイロ",
        "ヤマブキ",
        "コハク",
    ],
];

/// Ruby `FISHING_WHITE_CHART`。
static FISHING_WHITE_CHART: &[&[&str]] = &[
    &[
        "ウメガサネ",
        "ウメガサネ",
        "ウメガサネ",
        "ウメガサネ",
        "ハネズ",
        "ユルシ",
    ],
    &[
        "ウメガサネ",
        "ウメガサネ",
        "ウメガサネ",
        "ハネズ",
        "ソホ",
        "シンク",
    ],
    &[
        "ウメガサネ",
        "ウメガサネ",
        "ハネズ",
        "ソホ",
        "ユルシ",
        "ユルシ",
    ],
    &["ウメガサネ", "ハネズ", "ソホ", "ユルシ", "シンク", "シンク"],
    &["ハネズ", "ソホ", "ソホ", "ユルシ", "シンク", "共通(FC)"],
    &["ハネズ", "ソホ", "ユルシ", "シンク", "共通(FC)", "シュアン"],
];

/// Ruby `FISHING_GRAY_CHART`。
static FISHING_GRAY_CHART: &[&[&str]] = &[
    &[
        "ウメガサネ",
        "ウメガサネ",
        "セイラン",
        "セイラン",
        "ミハナダ",
        "ミハナダ",
    ],
    &[
        "ウメガサネ",
        "セイラン",
        "セイラン",
        "ミハナダ",
        "ミハナダ",
        "ミハナダ",
    ],
    &[
        "ウメガサネ",
        "ユルシ",
        "ミハナダ",
        "ミハナダ",
        "ミハナダ",
        "リンドウ",
    ],
    &[
        "ユルシ",
        "ユルシ",
        "セイラン",
        "リンドウ",
        "リンドウ",
        "スミレ",
    ],
    &[
        "ユルシ",
        "ユルシ",
        "リンドウ",
        "スミレ",
        "スミレ",
        "共通(FC)",
    ],
    &[
        "ユルシ",
        "リンドウ",
        "スミレ",
        "スミレ",
        "共通(FC)",
        "シゴク",
    ],
];

/// Ruby `FISHING_BLACK_CHART`。
static FISHING_BLACK_CHART: &[&[&str]] = &[
    &[
        "セイラン",
        "セイラン",
        "テツコン",
        "テツコン",
        "ウスハナ",
        "ウスハナ",
    ],
    &[
        "セイラン",
        "セイラン",
        "テツコン",
        "ウスハナ",
        "ウスハナ",
        "フカガワネズミ",
    ],
    &[
        "セイラン",
        "テツコン",
        "ウスハナ",
        "ウスハナ",
        "ミハナダ",
        "フカガワネズミ",
    ],
    &[
        "セイラン",
        "テツコン",
        "ミハナダ",
        "ウスハナ",
        "フカガワネズミ",
        "フカガワネズミ",
    ],
    &[
        "セイラン",
        "ウスハナ",
        "ミハナダ",
        "ミハナダ",
        "ミハナダ",
        "共通(FC)",
    ],
    &[
        "テツコン",
        "ウスハナ",
        "ミハナダ",
        "フカガワネズミ",
        "共通(FC)",
        "ルリ",
    ],
];

/// Ruby `FISHING_COMMON_CHART`。
static FISHING_COMMON_CHART: &[&[&str]] = &[
    &[
        "トビウオ",
        "トビウオ",
        "トビウオ",
        "オオガメ",
        "ロブスター",
        "オオサンショウウオ",
    ],
    &[
        "トビウオ",
        "トビウオ",
        "エイ",
        "オオガメ",
        "クジラ",
        "ロブスター",
    ],
    &[
        "トビウオ",
        "エイ",
        "マグロ",
        "マグロ",
        "カジキ",
        "イタチザメ",
    ],
    &[
        "トビウオ",
        "ミズダコ",
        "クラゲ",
        "マグロ",
        "オオクラゲ",
        "ハンマーヘッド・シャーク",
    ],
    &[
        "トビウオ",
        "エイ",
        "オオガメ",
        "オオガメ",
        "イタチザメ",
        "ミズダコ",
    ],
    &[
        "トビウオ",
        "クラゲ",
        "ロブスター",
        "ハンマーヘッド・シャーク",
        "ミズダコ",
        "ダイオウイカ",
    ],
];

/// Ruby `FISHING_SALT_LAKE_CHART`。
static FISHING_SALT_LAKE_CHART: &[&[&str]] = &[
    &[
        "シラユリ",
        "シラユリ",
        "シラユリ",
        "ゲッパク",
        "ゲッパク",
        "ゲッパク",
    ],
    &[
        "シラユリ",
        "シラユリ",
        "シラユリ",
        "ゲッパク",
        "スズ",
        "ナマリ",
    ],
    &[
        "シラユリ",
        "ゲッパク",
        "ゲッパク",
        "スズ",
        "ナマリ",
        "ナマリ",
    ],
    &[
        "シラユリ",
        "シラユリ",
        "ナマリ",
        "ナマリ",
        "ナマリ",
        "ナマリ",
    ],
    &["ゲッパク", "ゲッパク", "スズ", "スズ", "ロイロ", "ロイロ"],
    &["ナマリ", "スズ", "スズ", "スズ", "ロイロ", "クロツルバミ"],
];

/// Ruby `INEDIBLE_PLANT_CHART`。
static INEDIBLE_PLANT_CHART: &[&[&str]] = &[
    &[
        "シュイの花",
        "ダデオの花",
        "ロキの花",
        "シェラの花",
        "トトイト",
        "ポロネイマ",
    ],
    &[
        "シュイの花",
        "ロキの花",
        "アウディの花",
        "イディウの花",
        "トトイト",
        "ポロネイマ",
    ],
    &[
        "ダデオの花",
        "アウディの花",
        "イディウの花",
        "マトイト",
        "ポポトマ",
        "ルタタ",
    ],
    &[
        "シュイの花",
        "ミカギの花",
        "ロトイト",
        "ロトイト",
        "ツルイド",
        "ルタタ",
    ],
    &[
        "ミカギの花",
        "ロトイト",
        "ロトイト",
        "ツルイド",
        "ルタタ",
        "変異植物(MP)",
    ],
    &[
        "トトイト",
        "マトイト",
        "ポポトマ",
        "ツルイド",
        "変異植物(MP)",
        "サボサボ",
    ],
];

/// Ruby `INEDIBLE_PLANT2_CHART`。
static INEDIBLE_PLANT2_CHART: &[&[&str]] = &[
    &[
        "マトラの花",
        "マトラの花",
        "蜜蝋",
        "ポルラの花",
        "ウェスドの花",
        "ポルラの花",
    ],
    &[
        "マトラの花",
        "ホイの花",
        "マトラの花",
        "ウェスドの花",
        "蜜蝋",
        "ロロの花",
    ],
    &[
        "ホイの花",
        "ポルラの花",
        "ウェスドの花",
        "ホイの花",
        "ポルラの花",
        "ポルラの花",
    ],
    &[
        "ポルラの花",
        "ホイの花",
        "ロロの花",
        "ウェスドの花",
        "ポルラの花",
        "ドダの実",
    ],
    &[
        "ポルラの花",
        "ウェスドの花",
        "ロロの花",
        "ロロの花",
        "ロロの花",
        "ロロの花",
    ],
    &[
        "ウェスドの花",
        "ロロの花",
        "ポルラの花",
        "ロロの花",
        "ドダの実",
        "ロロの花",
    ],
];

/// Ruby `EDIBLE_PLANT_CHARTS`。
/// ヴィラシエル可食植物表 1〜5。
static EDIBLE_PLANT_CHARTS: &[&[&[&str]]] = &[
    &[
        &["小麦", "小麦", "さつまいも", "ねぎ", "白菜", "きゅうり"],
        &[
            "小麦",
            "さつまいも",
            "さといも",
            "白菜",
            "白菜",
            "とうもろこし",
        ],
        &[
            "さといも",
            "さといも",
            "ねぎ",
            "白菜",
            "とうもろこし",
            "枝豆",
        ],
        &["シソ", "ひらたけ", "エリンギ", "枝豆", "枝豆", "ラズベリー"],
        &[
            "シソ",
            "ひらたけ",
            "ひらたけ",
            "エリンギ",
            "ラズベリー",
            "さといも",
        ],
        &[
            "ナシ",
            "ナシ",
            "ナシ",
            "ラズベリー",
            "ラズベリー",
            "さといも",
        ],
    ],
    &[
        &["米", "米", "にんじん", "じゃがいも", "ふき", "まいたけ"],
        &["米", "じゃがいも", "じゃがいも", "にら", "ふき", "きくらげ"],
        &["冬瓜", "しょうが", "冬瓜", "ふき", "ふき", "きくらげ"],
        &["しょうが", "冬瓜", "ビワ", "にら", "まいたけ", "まいたけ"],
        &[
            "ビワ",
            "ビワ",
            "もも",
            "かぼちゃ",
            "グリーンピース",
            "まいたけ",
        ],
        &["ビワ", "もも", "もも", "かぼちゃ", "かぼちゃ", "かぼちゃ"],
    ],
    &[
        &["もち米", "トマト", "オクラ", "とうがらし", "大根", "グミ"],
        &["もち米", "オクラ", "オクラ", "大根", "大根", "とうがらし"],
        &[
            "しいたけ",
            "マッシュルーム",
            "オクラ",
            "グミ",
            "玉ねぎ",
            "小松菜",
        ],
        &[
            "ブロッコリー",
            "しいたけ",
            "トマト",
            "玉ねぎ",
            "さやえんどう",
            "玉ねぎ",
        ],
        &[
            "しいたけ",
            "マッシュルーム",
            "ブロッコリー",
            "小松菜",
            "さやえんどう",
            "改良種(IS)",
        ],
        &[
            "マッシュルーム",
            "ブロッコリー",
            "マッシュルーム",
            "小松菜",
            "改良種(IS)",
            "グミ",
        ],
    ],
    &[
        &["大豆", "大豆", "にんにく", "そらまめ", "しめじ", "みかん"],
        &["かぶ", "大豆", "かぶ", "キャベツ", "そらまめ", "みかん"],
        &[
            "にんにく",
            "かぶ",
            "にんにく",
            "しめじ",
            "クランベリー",
            "ピーマン",
        ],
        &[
            "キャベツ",
            "キャベツ",
            "ほうれん草",
            "しめじ",
            "レタス",
            "ピーマン",
        ],
        &[
            "ほうれん草",
            "ほうれん草",
            "クランベリー",
            "レタス",
            "ピーマン",
            "改良種(IS)",
        ],
        &[
            "松茸",
            "ほうれん草",
            "松茸",
            "レタス",
            "クランベリー",
            "改良種(IS)",
        ],
    ],
    &[
        &[
            "小豆",
            "れんこん",
            "みつば",
            "やまのいも",
            "デコポン",
            "イチゴ",
        ],
        &[
            "れんこん",
            "れんこん",
            "小豆",
            "なめこ",
            "かいわれ大根",
            "なめこ",
        ],
        &[
            "やまのいも",
            "アスパラガス",
            "なす",
            "なめこ",
            "やまのいも",
            "デコポン",
        ],
        &[
            "なす",
            "やまのいも",
            "みつば",
            "えのきたけ",
            "かいわれ大根",
            "デコポン",
        ],
        &[
            "アスパラガス",
            "アスパラガス",
            "やまのいも",
            "みつば",
            "なめこ",
            "改良種(IS)",
        ],
        &[
            "なす",
            "もやし",
            "えのきたけ",
            "えのきたけ",
            "改良種(IS)",
            "イチゴ",
        ],
    ],
];

/// Ruby `EDIBLE_PLANT2_CHARTS`。
/// アルメサール可食植物表 1〜2。
static EDIBLE_PLANT2_CHARTS: &[&[&[&str]]] = &[
    &[
        &[
            "テンサイ",
            "バノ",
            "テンサイ",
            "サトウモロ",
            "サトウモロ",
            "パンノミ",
        ],
        &[
            "テンサイ",
            "バノ",
            "サトウモロ",
            "バノ",
            "ミソレグア",
            "パンノミ",
        ],
        &[
            "テンサイ",
            "サトウモロ",
            "バノ",
            "ニクニク",
            "パンノミ",
            "メーズム",
        ],
        &["バノ", "バノ", "バノ", "パンノミ", "ミソレグア", "メーズム"],
        &[
            "テンサイ",
            "パンノミ",
            "ニクニク",
            "ニクニク",
            "メーズム",
            "ミソレグア",
        ],
        &[
            "サトウモロ",
            "ニクニク",
            "メーズム",
            "ミソレグア",
            "メーズム",
            "メーズム",
        ],
    ],
    &[
        &[
            "アロアベリー",
            "パンノミ",
            "ミソレグア",
            "サイングア",
            "パンノミ",
            "アロアベリー",
        ],
        &[
            "パンノミ",
            "サイングア",
            "パンノミ",
            "ミソレグア",
            "アロアベリー",
            "ミソレグア",
        ],
        &[
            "パンノミ",
            "アロアベリー",
            "サイングア",
            "パンノミ",
            "パンノミ",
            "トロアベリア",
        ],
        &[
            "パンノミ",
            "アロアベリー",
            "パンノミ",
            "ミソレグア",
            "ミソレグア",
            "トロアベリア",
        ],
        &[
            "サイングア",
            "パンノミ",
            "トロアベリア",
            "ミソレグア",
            "アロアベリー",
            "サイングア",
        ],
        &[
            "ミソレグア",
            "トロアベリア",
            "サイングア",
            "アロアベリー",
            "トロアベリア",
            "トロアベリア",
        ],
    ],
];

/// Ruby `MUTANT_PLANT_CHART`。
static MUTANT_PLANT_CHART: &[&[&str]] = &[
    &[
        "ガドゴン",
        "ガドゴン",
        "レディダン",
        "ボディア",
        "ブタマル",
        "ブタマル",
    ],
    &[
        "レディダン",
        "レディダン",
        "ボディア",
        "トロコッコ",
        "ブタマル",
        "ツァイド",
    ],
    &[
        "ボディア",
        "ボディア",
        "マメノキ",
        "ナッキュ",
        "ツァイド",
        "ボディア",
    ],
    &[
        "ナッキュ",
        "マメノキ",
        "ナッキュ",
        "ガドゴン",
        "レディダン",
        "レディダン",
    ],
    &[
        "ポメラマ",
        "ポメラマ",
        "ナッキュ",
        "ツァイド",
        "ガドゴン",
        "ボディア",
    ],
    &[
        "ナッキュ",
        "ツァイド",
        "ツァイド",
        "ツァイド",
        "ボディア",
        "グラディエゴ",
    ],
];

/// Ruby `IMPROVED_SPECIES_CHART`。
static IMPROVED_SPECIES_CHART: &[&[&str]] = &[
    &[
        "ワワ",
        "ワワ",
        "ブラックカロット",
        "ビーズ",
        "レモン",
        "ブラッドオレンジ",
    ],
    &[
        "ポポ",
        "ポポ",
        "グランツェ",
        "オオカサゲ",
        "ブラッドオレンジ",
        "レモン",
    ],
    &[
        "ヒットト",
        "グランツェ",
        "ブラックベリー",
        "ピマット",
        "ブラッドオレンジ",
        "レモン",
    ],
    &[
        "ブルーベリー",
        "ヒットト",
        "グランツェ",
        "ブラッドオレンジ",
        "ユズ",
        "ブラックベリー",
    ],
    &[
        "ビーズ",
        "ピマット",
        "オオカサゲ",
        "ライム",
        "ブルーベリー",
        "ユズ",
    ],
    &[
        "ビーズ",
        "レッドキャベツ",
        "ライム",
        "オオカサゲ",
        "ライム",
        "リンゴ",
    ],
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Villaciel",
            "Villaciel.toml",
            51,
        );
    }
}
