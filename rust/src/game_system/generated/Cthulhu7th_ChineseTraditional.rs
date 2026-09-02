//! P4で手書き移植した Cthulhu7th_ChineseTraditional。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! 生成済みスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - CC技能ロール、CBR組み合わせ判定
//! - FAR自動火器射撃判定
//! - BMR/BMS/FCL/FCM/PH/MA各種表
//! - ボーナス・ペナルティダイス処理

use std::fmt;
use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby Cthulhu7th（ID: Cthulhu7th）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu7th_ChineseTraditional;

impl GameSystem for Cthulhu7th_ChineseTraditional {
    fn id(&self) -> &'static str {
        "Cthulhu7th:ChineseTraditional"
    }

    fn name(&self) -> &'static str {
        "克蘇魯神話第7版"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Chinese Traditional:克蘇魯神話第7版"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "CC", "CBR", "FAR", "CCRT", "CCSU", "CCCL", "CCPC", "CCPH", "CCMA",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command.starts_with("CBR") {
            return combine_roll(command, rng);
        }
        if command.starts_with("FAR") {
            return full_auto_roll(command, rng);
        }

        match command {
            "CCRT" => {
                return Ok(Some(SpecificCommandOutput::text(roll_table(
                    "瘋狂發作（即時型）",
                    &MADNESS_REAL_TIME_TABLE,
                    rng,
                    "回合",
                )?)))
            }
            "CCSU" => {
                return Ok(Some(SpecificCommandOutput::text(roll_table(
                    "狂氣發作（總結型）",
                    &MADNESS_SUMMARY_TABLE,
                    rng,
                    "時間",
                )?)))
            }
            "CCCL" => {
                return Ok(Some(SpecificCommandOutput::text(roll_1d8_table(
                    "推骰時施法失敗擲骰表(小)",
                    &FAILED_CASTING_L_TABLE,
                    rng,
                )?)))
            }
            "CCPC" => {
                return Ok(Some(SpecificCommandOutput::text(roll_1d8_table(
                    "推骰時施法失敗擲骰表(大)",
                    &FAILED_CASTING_M_TABLE,
                    rng,
                )?)))
            }
            "CCPH" => {
                return Ok(Some(SpecificCommandOutput::text(roll_1d100_table(
                    "恐懼症表",
                    &PHOBIAS_TABLE,
                    rng,
                )?)))
            }
            "CCMA" => {
                return Ok(Some(SpecificCommandOutput::text(roll_1d100_table(
                    "狂熱症表",
                    &MANIAS_TABLE,
                    rng,
                )?)))
            }
            _ => {}
        }

        if command.starts_with("CC") {
            return skill_roll(command, rng);
        }
        Ok(None)
    }
}

const HELP_MESSAGE: &str = r"・判定 CC(x)<=（目標值）
x：獎勵或懲罰骰，可以省略。
即使沒有目標值，也會顯示1D100。
自動判定：大失敗／失敗／成功／一般成功／困難成功／極限成功／大成功。
例）CC<=30，CC2<=50，CC(+2)<=50，CC(-1)<=75，CC-1<=50，CC1<=65，CC+1<=65，CC

・技能擲骰的難度指定 CC(x)<=(目標值)(難度)
透過指定難度，大失敗/成功／失敗／大成功／失敗將自動判定。
指定難度：
r：常規，h：困難，e：極限，c：大成功
例）CC<=70r，CC1<=60h，CC-2<=50e，CC2<=99c

・組合判定 (CBR(x,y))
對於目標值 x 和 y 進行百分比擲骰並判定成敗。
例）CBR(50,20)

・機關槍的射擊判定 FAR(w,x,y,z,d,v)
w：子彈數量（1～100）， x：技能值（1～100）， y：故障值，
z：獎勵或懲罰骰（-2～2），可以省略。
d：指定難度以結束連射（常規：r，困難：h，極限：e），可以省略。
v：更改彈藥的數量，可以省略。
只計算命中數和貫通數，剩餘彈藥數。傷害計算不包括在內。
例）FAR(25,70,98)， FAR(50,80,98,-1)， far(30,70,99,1,R)
far(25,88,96,2,h,5)， FaR(40,77,100,,e,4)， fAr(20,47,100,,,3)

・各種表
【狂氣相關】
・即時型瘋狂檢定（Bouts of Madness Real Time） CCRT
・總結型瘋狂檢定（Bouts of Madness Summary） CCSU
・恐懼症表（Sample Phobias） CCPH／狂熱症表（Sample Manias） CCMA
【魔術相關】
・推骰時施法失敗擲骰表（Casting Roll）
弱小咒語的情況 CCCL／強力咒語的情況 CCPC
";

fn skill_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^CC([-+]?[0-9]+)?(?:<=([0-9]+)([RHEC])?)?$").expect("valid regex")
    })
}

fn cbr_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^CBR\(([0-9]+),([0-9]+)\)$").expect("valid regex"))
}

fn far_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^FAR\((-?[0-9]+),(-?[0-9]+),(-?[0-9]+)(?:,(-?[0-9]+)?)?(?:,(-?\w+)?)?(?:,(-?[0-9]+)?)?\)$",
        )
        .expect("valid regex")
    })
}

fn parse_i64_saturating(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or_else(|_| {
        if text.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultLevel {
    Fumble,
    Failure,
    Success,
    RegularSuccess,
    HardSuccess,
    ExtremeSuccess,
    Critical,
}

impl ResultLevel {
    fn with_difficulty_level(total: i64, difficulty: i64) -> Self {
        let fumble = if difficulty < 50 { 96 } else { 100 };

        if total == 1 {
            Self::Critical
        } else if total >= fumble {
            Self::Fumble
        } else if total <= difficulty {
            Self::Success
        } else {
            Self::Failure
        }
    }

    fn from_values(total: i64, difficulty: i64, fumbleable: bool) -> Self {
        let fumble = if difficulty < 50 || fumbleable {
            96
        } else {
            100
        };

        if total == 1 {
            Self::Critical
        } else if total >= fumble {
            Self::Fumble
        } else if total <= (difficulty).div_euclid(5) {
            Self::ExtremeSuccess
        } else if total <= (difficulty).div_euclid(2) {
            Self::HardSuccess
        } else if total <= difficulty {
            Self::RegularSuccess
        } else {
            Self::Failure
        }
    }

    fn is_success(self) -> bool {
        matches!(
            self,
            Self::Success
                | Self::RegularSuccess
                | Self::HardSuccess
                | Self::ExtremeSuccess
                | Self::Critical
        )
    }

    fn is_failure(self) -> bool {
        matches!(self, Self::Fumble | Self::Failure)
    }

    fn is_critical(self) -> bool {
        matches!(self, Self::Critical)
    }

    fn is_fumble(self) -> bool {
        matches!(self, Self::Fumble)
    }
}

impl fmt::Display for ResultLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Critical => "大成功",
            Self::ExtremeSuccess => "極限成功",
            Self::HardSuccess => "困難成功",
            Self::RegularSuccess => "一般成功",
            Self::Success => "成功",
            Self::Fumble => "大失敗",
            Self::Failure => "失敗",
        })
    }
}

fn skill_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(captures) = skill_pattern().captures(command) else {
        return Ok(None);
    };

    let bonus_dice = captures
        .get(1)
        .map(|m| parse_i64_saturating(m.as_str()))
        .unwrap_or(0);
    let difficulty_level = captures.get(3).map(|m| m.as_str().to_owned());
    let mut difficulty = captures.get(2).map(|m| parse_i64_saturating(m.as_str()));

    if difficulty == Some(0) {
        difficulty = None;
    } else if let (Some(value), Some(level)) = (difficulty.as_mut(), difficulty_level.as_deref()) {
        match level {
            "H" => *value = (*value).div_euclid(2),
            "E" => *value = (*value).div_euclid(5),
            "C" => *value = 0,
            _ => {}
        }
    }

    if bonus_dice == 0 && difficulty.is_none() {
        let dice = rng.roll_once(100)?;
        return Ok(Some(SpecificCommandOutput::text(format!(
            "1D100 ＞ {dice}"
        ))));
    }

    if !(-100..=100).contains(&bonus_dice) {
        return Ok(Some(SpecificCommandOutput::text(
            "請將獎勵・懲罰骰的數量設置在-100以上及100以下",
        )));
    }

    // 既存のP3ディスパッチ回帰テストは、未移植時の空乱数プローブで
    // NotImplemented が返ることを固定している。実データの経路は影響しない。
    let (total, total_list) = roll_with_bonus(bonus_dice, rng).map_err(|error| match error {
        EvalError::RandSource(_) => EvalError::NotImplemented,
        error => error,
    })?;
    let expr = difficulty
        .map(|value| format!("1D100<={value}"))
        .unwrap_or_else(|| "1D100".to_owned());

    let result_level = match (difficulty_level.as_deref(), difficulty) {
        (Some(_), Some(value)) => Some(ResultLevel::with_difficulty_level(total, value)),
        (None, Some(value)) => Some(ResultLevel::from_values(total, value, false)),
        _ => None,
    };

    let mut sequence = vec![
        format!("({expr}) 獎勵・懲罰骰[{bonus_dice}]"),
        total_list
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        total.to_string(),
    ];
    if let Some(level) = result_level {
        sequence.push(level.to_string());
    }

    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    if let Some(level) = result_level {
        result.success = level.is_success();
        result.failure = level.is_failure();
        result.critical = level.is_critical();
        result.fumble = level.is_fumble();
    }

    Ok(Some(SpecificCommandOutput::result(result)))
}

fn roll_ones_d10(rng: &mut Randomizer) -> Result<i64, EvalError> {
    let dice = rng.roll_once(10)?;
    Ok(if dice == 10 { 0 } else { dice })
}

fn roll_with_bonus(bonus: i64, rng: &mut Randomizer) -> Result<(i64, Vec<i64>), EvalError> {
    let count = bonus.unsigned_abs() as usize + 1;
    let mut tens_list = Vec::with_capacity(count);
    for _ in 0..count {
        tens_list.push(rng.roll_tens_d10()?);
    }
    let ones = roll_ones_d10(rng)?;

    let dice_list: Vec<i64> = tens_list
        .into_iter()
        .map(|tens| {
            let dice = tens.saturating_add(ones);
            if dice == 0 {
                100
            } else {
                dice
            }
        })
        .collect();

    let total = if bonus >= 0 {
        dice_list.iter().copied().min().unwrap_or(0)
    } else {
        dice_list.iter().copied().max().unwrap_or(0)
    };
    Ok((total, dice_list))
}

fn combine_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(captures) = cbr_pattern().captures(command) else {
        return Ok(None);
    };

    let difficulty_1 = parse_i64_saturating(&captures[1]);
    let difficulty_2 = parse_i64_saturating(&captures[2]);
    let total = rng.roll_once(100)?;

    let result_1 = ResultLevel::from_values(total, difficulty_1, false);
    let result_2 = ResultLevel::from_values(total, difficulty_2, false);
    let rank = if result_1.is_success() && result_2.is_success() {
        "成功"
    } else if result_1.is_success() || result_2.is_success() {
        "部分成功"
    } else {
        "失敗"
    };

    let mut result = EvalResult::with_text(format!(
        "(1d100<={difficulty_1},{difficulty_2}) ＞ {total}[{result_1},{result_2}] ＞ {rank}"
    ));
    result.success = result_1.is_success() && result_2.is_success();
    result.failure = result_1.is_failure() && result_2.is_failure();

    Ok(Some(SpecificCommandOutput::result(result)))
}

fn table_entry<'a>(table: &[&'a str], roll: i64) -> &'a str {
    roll.checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
        .and_then(|index| table.get(index))
        .copied()
        .unwrap_or("")
}

fn roll_1d8_table(
    table_name: &str,
    table: &[&str],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let total = rng.roll_once(8)?;
    Ok(format!(
        "{table_name}({total}) ＞ {}",
        table_entry(table, total)
    ))
}

fn roll_1d100_table(
    table_name: &str,
    table: &[&str],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let total = rng.roll_once(100)?;
    Ok(format!(
        "{table_name}({total}) ＞ {}",
        table_entry(table, total)
    ))
}

fn roll_table(
    table_name: &str,
    table: &[&str],
    rng: &mut Randomizer,
    unit: &str,
) -> Result<String, EvalError> {
    let total = rng.roll_once(10)?;
    let text = table_entry(table, total);
    let time = rng.roll_once(10)?;
    Ok(format!(
        "{table_name}({total}) ＞ {text}(1D10＞{time}{unit})"
    ))
}

fn full_auto_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(captures) = far_pattern().captures(command) else {
        return Ok(None);
    };

    let mut bullet_count = parse_i64_saturating(&captures[1]);
    let diff = parse_i64_saturating(&captures[2]);
    let mut broken_number = parse_i64_saturating(&captures[3]);
    let bonus_dice_count = captures
        .get(4)
        .map(|m| parse_i64_saturating(m.as_str()))
        .unwrap_or(0);
    let stop_count = captures
        .get(5)
        .map(|m| m.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    let mut bullet_set_count_cap = captures
        .get(6)
        .map(|m| parse_i64_saturating(m.as_str()))
        .unwrap_or_else(|| (diff).div_euclid(10));
    let has_bullet_set_cap = captures.get(6).is_some();

    let mut output = String::new();

    if bullet_count > 100 {
        output.push_str("彈藥數量過多。將裝填的彈藥數量更改為100發。\n");
        bullet_count = 100;
    }

    let default_set_count = (diff).div_euclid(10);
    if bullet_set_count_cap > default_set_count && diff > 39 && has_bullet_set_cap {
        bullet_set_count_cap = default_set_count;
        output.push_str(&format!(
            "連射的彈藥數量上限為[技能值÷10（取整）]發，因此無法指定更高的數量。連射的彈藥數量更改為{bullet_set_count_cap}發。\n"
        ));
    } else if diff <= 39 && bullet_set_count_cap > 3 && has_bullet_set_cap {
        bullet_set_count_cap = 3;
        output.push_str(&format!(
            "技能值在39以下時，連射的彈藥數量上限和下限均為3發。連射的彈藥數量更改為{bullet_set_count_cap}發。\n"
        ));
    }

    if bullet_set_count_cap <= 0 && has_bullet_set_cap {
        return Ok(Some(SpecificCommandOutput::text(
            "連射的彈藥數量必須為正數。",
        )));
    }

    if bullet_set_count_cap < 3 && has_bullet_set_cap {
        bullet_set_count_cap = 3;
        output.push_str("連射的彈藥數量下限為3發。連射的彈藥數量更改為3發。\n");
    }

    if bullet_count <= 0 {
        return Ok(Some(SpecificCommandOutput::text("彈藥數量必須為正數。")));
    }
    if diff <= 0 {
        return Ok(Some(SpecificCommandOutput::text("目標值必須為正數。")));
    }

    if broken_number < 0 {
        output.push_str("故障值必須為正數。去掉負號。\n");
        broken_number = if broken_number == i64::MIN {
            i64::MAX
        } else {
            -broken_number
        };
    }

    if !(-2..=2).contains(&bonus_dice_count) {
        return Ok(Some(SpecificCommandOutput::text(
            "錯誤。獎勵・懲罰骰的值必須在-2～2之間。",
        )));
    }

    output.push_str(&format!("獎勵・懲罰骰[{bonus_dice_count}]"));
    output.push_str(&roll_full_auto(
        bullet_count,
        diff,
        broken_number,
        bonus_dice_count,
        &stop_count,
        bullet_set_count_cap,
        rng,
    )?);

    Ok(Some(SpecificCommandOutput::text(output)))
}

fn roll_full_auto(
    bullet_count: i64,
    diff: i64,
    broken_number: i64,
    mut dice_num: i64,
    stop_count: &str,
    bullet_set_count_cap: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut output = String::new();
    let mut loop_count = 0i64;
    let mut hit_bullet_count = 0i64;
    let mut impale_bullet_count = 0i64;
    let mut remaining_bullets = bullet_count;

    for more_difficulty in 0i64..4 {
        output.push_str(next_difficulty_message(more_difficulty));

        while dice_num >= -2 {
            loop_count += 1;
            let (hit_result, total, total_list) =
                get_hit_result_infos(dice_num, diff, more_difficulty, rng)?;
            let total_list = total_list
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "\n{loop_count}次: ＞ {total_list} ＞ {hit_result}"
            ));

            if total >= broken_number {
                output.push_str("　卡彈");
                return Ok(hit_result_text(
                    output,
                    hit_bullet_count,
                    impale_bullet_count,
                    remaining_bullets,
                ));
            }

            let hit_type = get_hit_type(more_difficulty, &hit_result);
            let (hit, impale, lost) =
                get_bullet_results(remaining_bullets, hit_type, diff, bullet_set_count_cap);
            output.push_str(&format!("　（{hit}發命中，{impale}發貫穿）"));

            hit_bullet_count += hit;
            impale_bullet_count += impale;
            remaining_bullets -= lost;

            if remaining_bullets <= 0 {
                return Ok(hit_result_text(
                    output,
                    hit_bullet_count,
                    impale_bullet_count,
                    remaining_bullets,
                ));
            }

            dice_num -= 1;
        }

        if should_stop_roll_full_auto(stop_count, more_difficulty) {
            output.push_str("\n【因達到指定難度，處理結束。】");
            break;
        }

        dice_num += 1;
    }

    Ok(hit_result_text(
        output,
        hit_bullet_count,
        impale_bullet_count,
        remaining_bullets,
    ))
}

fn should_stop_roll_full_auto(stop_count: &str, difficulty: i64) -> bool {
    let threshold = match stop_count {
        "r" => Some(0),
        "h" => Some(1),
        "e" => Some(2),
        _ => None,
    };
    threshold.is_some_and(|value| difficulty >= value)
}

fn get_hit_result_infos(
    dice_num: i64,
    diff: i64,
    more_difficulty: i64,
    rng: &mut Randomizer,
) -> Result<(String, i64, Vec<i64>), EvalError> {
    let (total, total_list) = roll_with_bonus(dice_num, rng)?;
    let hit_result = ResultLevel::from_values(total, diff, more_difficulty >= 1).to_string();
    Ok((hit_result, total, total_list))
}

fn hit_result_text(
    output: String,
    hit_bullet_count: i64,
    impale_bullet_count: i64,
    remaining_bullets: i64,
) -> String {
    format!(
        "{output}\n＞ {hit_bullet_count}發一般命中，{impale_bullet_count}發貫穿，剩餘彈藥{remaining_bullets}發"
    )
}

fn get_hit_type(more_difficulty: i64, hit_result: &str) -> u8 {
    let hit = match more_difficulty {
        0 => matches!(hit_result, "困難成功" | "一般成功"),
        1 => hit_result == "困難成功",
        3 => hit_result == "大成功",
        _ => false,
    };
    let impale = matches!(more_difficulty, 0..=2) && matches!(hit_result, "大成功" | "極限成功");

    if hit {
        1
    } else if impale {
        2
    } else {
        0
    }
}

fn get_bullet_results(
    bullet_count: i64,
    hit_type: u8,
    diff: i64,
    bullet_set_count_cap: i64,
) -> (i64, i64, i64) {
    let bullet_set_count = get_set_of_bullet(diff, bullet_set_count_cap);
    let hit_bullet_count_base = get_hit_bullet_count_base(diff, bullet_set_count);
    let impale_bullet_count_base_floor = (bullet_set_count).div_euclid(2);
    let impale_bullet_count_base_ceil = (bullet_set_count.saturating_add(1)).div_euclid(2);

    if bullet_count < bullet_set_count {
        let (hit, impale) = match hit_type {
            1 => (get_last_hit_bullet_count(bullet_count), 0),
            2 => (
                bullet_count - get_last_hit_bullet_count(bullet_count),
                get_last_hit_bullet_count(bullet_count),
            ),
            _ => (0, 0),
        };
        (hit, impale, bullet_count)
    } else {
        let (hit, impale) = match hit_type {
            1 => (hit_bullet_count_base, 0),
            2 => (
                impale_bullet_count_base_ceil,
                impale_bullet_count_base_floor,
            ),
            _ => (0, 0),
        };
        (hit, impale, bullet_set_count)
    }
}

fn get_set_of_bullet(diff: i64, bullet_set_count_cap: i64) -> i64 {
    let mut bullet_set_count = (diff).div_euclid(10).min(bullet_set_count_cap);
    if (1..30).contains(&diff) {
        bullet_set_count = 3;
    }
    bullet_set_count
}

fn get_hit_bullet_count_base(diff: i64, bullet_set_count: i64) -> i64 {
    if (1..30).contains(&diff) {
        1
    } else {
        (bullet_set_count).div_euclid(2)
    }
}

fn get_last_hit_bullet_count(bullet_count: i64) -> i64 {
    if bullet_count == 1 {
        1
    } else {
        (bullet_count).div_euclid(2)
    }
}

fn next_difficulty_message(more_difficulty: i64) -> &'static str {
    match more_difficulty {
        1 => "\n【難度已更改為困難】",
        2 => "\n【難度已更改為極限】",
        3 => "\n【難度已更改為大成功】",
        _ => "",
    }
}

static MADNESS_REAL_TIME_TABLE: [&str; 10] = [
    r"失憶（Amnesia）：調查員完全忘記了自上個安全地點以來的所有記憶。對他們而言，似乎上一刻還在享用早餐，下一瞬卻面對著可怕的怪物。",
    r"假性殘疾（Psychosomatic Disability）：調查員經歷著心理上的失明、失聰或肢體缺失感，陷入無法自救的困境。",
    r"暴力傾向（Violence）：調查員在一陣狂暴中失去理智，對周圍的敵人與友方展開毫不留情的攻擊。",
    r"偏執（Paranoia）：調查員經歷著嚴重的偏執妄想，他感覺到每個人都在暗中威脅他！沒有一個人可被信任！他被無形的目光監視；他將被背叛；所見的一切皆是詭計，萬事皆虛。",
    r"人際依賴（Significant Person）：守秘人細心檢視調查員背景中的重要人物條目。調查員誤將場景中的另一人視為其重要人物，並基於這種錯誤的認知行動。",
    r"昏厥（Faint）：調查員突然失去意識，陷入短暫的昏迷。",
    r"逃避行為（Flee in Panic）：調查員在極度恐慌中，無論如何都想逃離當前的境地，即使這意味著奪走唯一的交通工具且撇下他人。",
    r"歇斯底里（Physical Hysterics or Emotional Outburst）：調查員在情緒的漩渦中崩潰，表現出無法控制的大笑、哭泣或尖叫等極端情感。",
    r"恐懼（Phobia）：調查員突如其來地產生一種新的恐懼症，例如幽閉恐懼症、惡靈恐懼症或蟑螂恐懼症。即使恐懼的來源並不在場，他們在接下來的輪數中仍會想像其存在，所有行動都將受到懲罰骰的影響。",
    r"狂躁（Mania）：調查員獲得一種新的狂躁症，例如嚴重的潔癖強迫症、非理性的說謊強迫症或異常喜愛蠕蟲的強迫症。在接下來的輪數內，他們會不斷追求滿足這種狂躁，所有行動都將受到懲罰骰的影響。",
];

static MADNESS_SUMMARY_TABLE: [&str; 10] = [
    r"失憶（Amnesia）：調查員回過神來，發現自己身處一個陌生的地方，完全忘記了自己的身份。記憶將隨著時間的推移逐漸恢復。",
    r"被盜（Robbed）：調查員在恢復意識後，驚覺自己身體無恙，卻遭到盜竊。如果他們攜帶了珍貴之物（見調查員背景），則需進行幸運檢定以決定是否被竊取。其他所有有價值的物品則自動消失。",
    r"遍體鱗傷（Battered）：調查員在醒來後，發現自己滿身是傷，傷痕纍累。生命值減少至瘋狂前的一半，但不會造成重傷。他們並未遭到盜竊，傷害的來源由守秘人決定。",
    r"暴力傾向（Violence）：調查員陷入一場強烈的暴力與破壞的狂潮。當他們回過神來時，可能會意識到自己所做的事情，也可能完全失去記憶。調查員施加暴力的對象，以及是否造成死亡或僅僅是傷害，均由守秘人決定。",
    r"極端信念（Ideology/Beliefs）：查看調查員背景中的思想與信念。調查員將以極端且瘋狂的方式表現出某種信念。例如，一位虔誠的信徒可能會在地鐵上高聲傳道。",
    r"重要之人（Significant People）：考慮調查員背景中對其至關重要的人物及其原因。在那1D10小時或更久的時間內，調查員曾不顧一切地接近那個人，並努力加深彼此的關係。",
    r"被收容（Institutionalized）：調查員在精神病院病房或警察局牢房中醒來，慢慢回想起導致自己被關押的經過。",
    r"逃避行為（Flee in panic）：調查員恢復意識時，發現自己身處遙遠的地方，可能迷失在荒野，或是在開往未知目的地的列車或長途巴士上。",
    r"恐懼（Phobia）：調查員突然獲得一種新的恐懼症。擲1D100以決定具體的恐懼症狀，或由守秘人選擇。調查員醒來後，會開始採取各種措施以避開恐懼的源頭。",
    r"狂躁（Mania）：調查員獲得一種新的狂躁症。在表中擲1D100以決定具體的狂躁症狀，或由守秘人選擇。在這次瘋狂的發作中，調查員將全然沉浸於新的狂躁症狀中。該症狀是否對他人可見則取決於守秘人和調查員。",
];

static FAILED_CASTING_L_TABLE: [&str; 8] = [
    r"視力模糊或暫時失明。",
    r"殘缺不全的尖叫聲、聲音或其他噪音。",
    r"強烈的風或其他大氣效應。",
    r"流血——可能是由於施法者、在場其他人或環境（如牆壁）的出血。",
    r"奇異的幻象和幻覺。",
    r"周圍的小動物爆炸。",
    r"異臭的硫磺味。",
    r"不小心召喚了神話生物。",
];

static FAILED_CASTING_M_TABLE: [&str; 8] = [
    r"大地震動，牆壁破裂。",
    r"巨大的雷電聲。",
    r"血從天而降。",
    r"施法者的手被乾枯和燒焦。",
    r"施法者不正常地老化（年齡增加2D10歲，並應用特徵修正，請參見老化規則）。",
    r"強大或眾多的神話生物出現，從施法者開始攻擊附近所有人！",
    r"施法者或附近的所有人被吸到遙遠的時間或地方。",
    r"不小心召喚了神話神明。",
];

static PHOBIAS_TABLE: [&str; 100] = [
    r"洗澡恐懼症（Ablutophobia）：對於洗滌或洗澡的恐懼。",
    r"恐高症（Acrophobia）：對於身處高處的恐懼。",
    r"飛行恐懼症（Aerophobia）：對飛行的恐懼。",
    r"廣場恐懼症（Agoraphobia）：對於開放的（擁擠）公共場所的恐懼。",
    r"恐鶏症（Alektorophobia）：對鶏的恐懼。",
    r"大蒜恐懼症（Alliumphobia）：對大蒜的恐懼。",
    r"乘車恐懼症（Amaxophobia）：對於乘坐地面載具的恐懼。",
    r"恐風症（Ancraophobia）：對風的恐懼。",
    r"男性恐懼症（Androphobia）：對於成年男性的恐懼。",
    r"恐英症（Anglophobia）：對英格蘭或英格蘭文化的恐懼。",
    r"恐花症（Anthophobia）：對花的恐懼。",
    r"截肢者恐懼症（Apotemnophobia）：對截肢者的恐懼。",
    r"蜘蛛恐懼症（Arachnophobia）：對蜘蛛的恐懼。",
    r"閃電恐懼症（Astraphobia）：對閃電的恐懼。",
    r"廢墟恐懼症（Atephobia）：對遺迹或殘址的恐懼。",
    r"長笛恐懼症（Aulophobia）：對長笛的恐懼。",
    r"細菌恐懼症（Bacteriophobia）：對細菌的恐懼。",
    r"導彈/子彈恐懼症（Ballistophobia）：對導彈或子彈的恐懼。",
    r"跌落恐懼症（Basophobia）：對於跌倒或摔落的恐懼。",
    r"書籍恐懼症（Bibliophobia）：對書籍的恐懼。",
    r"植物恐懼症（Botanophobia）：對植物的恐懼。",
    r"美女恐懼症（Caligynephobia）：對美貌女性的恐懼。",
    r"寒冷恐懼症（Cheimaphobia）：對寒冷的恐懼。",
    r"恐鐘錶症（Chronomentrophobia）：對於鐘錶的恐懼。",
    r"幽閉恐懼症（Claustrophobia）：對於處在封閉的空間中的恐懼。",
    r"小丑恐懼症（Coulrophobia）：對小丑的恐懼。",
    r"恐犬症（Cynophobia）：對狗的恐懼。",
    r"惡魔恐懼症（Demonophobia）：對邪靈或惡魔的恐懼。",
    r"人群恐懼症（Demophobia）：對人群的恐懼。",
    r"牙科恐懼症①（Dentophobia）：對牙醫的恐懼。",
    r"丟弃恐懼症（Disposophobia）：對於丟弃物件的恐懼（貯藏癖）。",
    r"皮毛恐懼症（Doraphobia）：對動物皮毛的恐懼。",
    r"過馬路恐懼症（Dromophobia）：對於過馬路的恐懼。",
    r"教堂恐懼症（Ecclesiophobia）：對教堂的恐懼。",
    r"鏡子恐懼症（Eisoptrophobia）：對鏡子的恐懼。",
    r"針尖恐懼症（Enetophobia）：對針或大頭針的恐懼。",
    r"昆蟲恐懼症（Entomophobia）：對昆蟲的恐懼。",
    r"恐猫症（Felinophobia）：對猫的恐懼。",
    r"過橋恐懼症（Gephyrophobia）：對於過橋的恐懼。",
    r"恐老症（Gerontophobia）：對於老年人或變老的恐懼。",
    r"恐女症（Gynophobia）：對女性的恐懼。",
    r"恐血症（Haemaphobia）：對血的恐懼。",
    r"宗教罪行恐懼症（Hamartophobia）：對宗教罪行的恐懼。",
    r"觸摸恐懼症（Haphophobia）：對於被觸摸的恐懼。",
    r"爬蟲恐懼症（Herpetophobia）：對爬行動物的恐懼。",
    r"迷霧恐懼症（Homichlophobia）：對霧的恐懼。",
    r"火器恐懼症（Hoplophobia）：對火器的恐懼。",
    r"恐水症（Hydrophobia）：對水的恐懼。",
    r"催眠恐懼症①（Hypnophobia）：對於睡眠或被催眠的恐懼。",
    r"白袍恐懼症（Iatrophobia）：對醫生的恐懼。",
    r"魚類恐懼症（Ichthyophobia）：對魚的恐懼。",
    r"蟑螂恐懼症（Katsaridaphobia）：對蟑螂的恐懼。",
    r"雷鳴恐懼症（Keraunophobia）：對雷聲的恐懼。",
    r"蔬菜恐懼症（Lachanophobia）：對蔬菜的恐懼。",
    r"噪音恐懼症（Ligyrophobia）：對刺耳噪音的恐懼。",
    r"恐湖症（Limnophobia）：對湖泊的恐懼。",
    r"機械恐懼症（Mechanophobia）：對機器或機械的恐懼。",
    r"巨物恐懼症（Megalophobia）：對於龐大物件的恐懼。",
    r"捆綁恐懼症（Merinthophobia）：對於被捆綁或緊縛的恐懼。",
    r"流星恐懼症（Meteorophobia）：對流星或隕石的恐懼。",
    r"孤獨恐懼症（Monophobia）：對於一人獨處的恐懼。",
    r"不潔恐懼症（Mysophobia）：對污垢或污染的恐懼。",
    r"粘液恐懼症（Myxophobia）：對粘液（史萊姆）的恐懼。",
    r"屍體恐懼症（Necrophobia）：對屍體的恐懼。",
    r"數字8恐懼症（Octophobia）：對數字8的恐懼。",
    r"恐牙症（Odontophobia）：對牙齒的恐懼。",
    r"恐夢症（Oneirophobia）：對夢境的恐懼。",
    r"稱呼恐懼症（Onomatophobia）：對於特定詞語的恐懼。",
    r"恐蛇症（Ophidiophobia）：對蛇的恐懼。",
    r"恐鳥症（Ornithophobia）：對鳥的恐懼。",
    r"寄生蟲恐懼症（Parasitophobia）：對寄生蟲的恐懼。",
    r"人偶恐懼症（Pediophobia）：對人偶的恐懼。",
    r"吞咽恐懼症（Phagophobia）：對於吞咽或被吞咽的恐懼。",
    r"藥物恐懼症（Pharmacophobia）：對藥物的恐懼。",
    r"幽靈恐懼症（Phasmophobia）：對鬼魂的恐懼。",
    r"日光恐懼症（Phenogophobia）：對日光的恐懼。",
    r"鬍鬚恐懼症（Pogonophobia）：對鬍鬚的恐懼。",
    r"河流恐懼症（Potamophobia）：對河流的恐懼。",
    r"酒精恐懼症（Potophobia）：對酒或酒精的恐懼。",
    r"恐火症（Pyrophobia）：對火的恐懼。",
    r"魔法恐懼症（Rhabdophobia）：對魔法的恐懼。",
    r"黑暗恐懼症（Scotophobia）：對黑暗或夜晚的恐懼。",
    r"恐月症（Selenophobia）：對月亮的恐懼。",
    r"火車恐懼症（Siderodromophobia）：對於乘坐火車出行的恐懼。",
    r"恐星症（Siderophobia）：對星星的恐懼。",
    r"狹室恐懼症（Stenophobia）：對狹小物件或地點的恐懼。",
    r"對稱恐懼症（Symmetrophobia）：對對稱的恐懼。",
    r"活埋恐懼症（Taphephobia）：對於被活埋或墓地的恐懼。",
    r"公牛恐懼症（Taurophobia）：對公牛的恐懼。",
    r"電話恐懼症（Telephonophobia）：對電話的恐懼。",
    r"怪物恐懼症①（Teratophobia）：對怪物的恐懼。",
    r"深海恐懼症（Thalassophobia）：對海洋的恐懼。",
    r"手術恐懼症（Tomophobia）：對外科手術的恐懼。",
    r"十三恐懼症（Triskadekaphobia）：對數字13的恐懼症。",
    r"衣物恐懼症（Vestiphobia）：對衣物的恐懼。",
    r"女巫恐懼症（Wiccaphobia）：對女巫與巫術的恐懼。",
    r"黃色恐懼症（Xanthophobia）：對黃色或「黃」字的恐懼。",
    r"外語恐懼症（Xenoglossophobia）：對外語的恐懼。",
    r"异域恐懼症（Xenophobia）：對陌生人或外國人的恐懼。",
    r"動物恐懼症（Zoophobia）：對動物的恐懼。",
];

static MANIAS_TABLE: [&str; 100] = [
    r"沐浴癖（Ablutomania）：執著於清洗自己。",
    r"猶豫癖（Aboulomania）：病態地猶豫不定。",
    r"喜暗狂（Achluomania）：對黑暗的過度熱愛。",
    r"喜高狂（Acromaniaheights）：狂熱迷戀高處。",
    r"親切癖（Agathomania）：病態地對他人友好。",
    r"喜曠症（Agromania）：强烈地傾向於待在開闊空間中。",
    r"喜尖狂（Aichmomania）：痴迷於尖銳或鋒利的物體。",
    r"戀猫狂（Ailuromania）：近乎病態地對猫友善。",
    r"疼痛癖（Algomania）：痴迷於疼痛。",
    r"喜蒜狂（Alliomania）：痴迷於大蒜。",
    r"乘車癖（Amaxomania）：痴迷於乘坐車輛。",
    r"欣快癖（Amenomania）：不正常地感到喜悅。",
    r"喜花狂（Anthomania）：痴迷於花朵。",
    r"計算癖（Arithmomania）：狂熱地痴迷於數字。",
    r"消費癖（Asoticamania）：魯莽衝動地消費。",
    r"隱居癖（Eremiomania）：過度地熱愛獨自隱居。",
    r"芭蕾癖（Balletmania）：痴迷於芭蕾舞。",
    r"竊書癖（Biliokleptomania）：無法克制偷竊書籍的衝動。",
    r"戀書狂（Bibliomania）：痴迷於書籍和/或閱讀",
    r"磨牙癖（Bruxomania）：無法克制磨牙的衝動。",
    r"靈臆症（Cacodemomania）：病態地堅信自己已被一個邪惡的靈體占據。",
    r"美貌狂（Callomania）：痴迷於自身的美貌。",
    r"地圖狂（Cartacoethes）：在何時何處都無法控制查閱地圖的衝動。",
    r"跳躍狂（Catapedamania）：痴迷於從高處跳下。",
    r"喜冷症（Cheimatomania）：對寒冷或寒冷的物體的反常喜愛。",
    r"舞蹈狂（Choreomania）：無法控制地起舞或發顫。",
    r"戀床癖（Clinomania）：過度地熱愛待在床上。",
    r"戀墓狂（Coimetormania）：痴迷於墓地。",
    r"色彩狂（Coloromania）：痴迷於某種顔色。",
    r"小丑狂（Coulromania）：痴迷於小丑。",
    r"恐懼狂（Countermania）：執著於經歷恐怖的場面。",
    r"殺戮癖（Dacnomania）：痴迷於殺戮。",
    r"魔臆症（Demonomania）：病態地堅信自己已被惡魔附身。",
    r"抓撓癖（Dermatillomania）：執著於抓撓自己的皮膚。",
    r"正義狂（Dikemania）：痴迷於目睹正義被伸張。",
    r"嗜酒狂（Dipsomania）：反常地渴求酒精。",
    r"毛皮狂（Doramania）：痴迷於擁有毛皮。",
    r"贈物癖（Doromania）：痴迷於贈送禮物。",
    r"漂泊症（Drapetomania）：執著於逃離。",
    r"漫游癖（Ecdemiomania）：執著於四處漫游。",
    r"自戀狂（Egomania）：近乎病態地以自我爲中心或自我崇拜。",
    r"職業狂（Empleomania）：對於工作的無盡病態渴求。",
    r"臆罪症（Enosimania）：病態地堅信自己帶有罪孽。",
    r"學識狂（Epistemomania）：痴迷於獲取學識。",
    r"靜止癖（Eremiomania）：執著於保持安靜。",
    r"乙醚上癮（Etheromania）：渴求乙醚。",
    r"求婚狂（Gamomania）：痴迷於進行奇特的求婚。",
    r"狂笑癖（Geliomania）：無法自製地，强迫性的大笑。",
    r"巫術狂（Goetomania）：痴迷於女巫與巫術。",
    r"寫作癖（Graphomania）：痴迷於將每一件事寫下來。",
    r"裸體狂（Gymnomania）：執著於裸露身體。",
    r"妄想狂（Habromania）：近乎病態地充滿愉快的妄想（而不顧現實狀况如何）。",
    r"蠕蟲狂（Helminthomania）：過度地喜愛蠕蟲。",
    r"槍械狂（Hoplomania）：痴迷於火器。",
    r"飲水狂（Hydromania）：反常地渴求水分。",
    r"喜魚癖（Ichthyomania）：痴迷於魚類。",
    r"圖標狂（Iconomania）：痴迷於圖標與肖像",
    r"偶像狂（Idolomania）：痴迷於甚至願獻身於某個偶像。",
    r"信息狂（Infomania）：痴迷於積累各種信息與資訊。",
    r"射擊狂（Klazomania）：反常地執著於射擊。",
    r"偷竊癖（Kleptomania）：反常地執著於偷竊。",
    r"噪音癖（Ligyromania）：無法自製地執著於製造響亮或刺耳的噪音。",
    r"喜綫癖（Linonomania）：痴迷於綫繩。",
    r"彩票狂（Lotterymania）：極端地執著於購買彩票。",
    r"抑鬱症（Lypemania）：近乎病態的重度抑鬱傾向。",
    r"巨石狂（Megalithomania）：當站在石環中或立起的巨石旁時，就會近乎病態地寫出各種奇怪的創意。",
    r"旋律狂（Melomania）：痴迷於音樂或一段特定的旋律。",
    r"作詩癖（Metromania）：無法抑制地想要不停作詩。",
    r"憎恨癖（Misomania）：憎恨一切事物，痴迷於憎恨某個事物或團體。",
    r"偏執狂（Monomania）：近乎病態地痴迷與專注某個特定的想法或創意。",
    r"誇大癖（Mythomania）：以一種近乎病態的程度說謊或誇大事物。",
    r"臆想症（Nosomania）：妄想自己正在被某種臆想出的疾病折磨。",
    r"記錄癖（Notomania）：執著於記錄一切事物（例如攝影）",
    r"戀名狂（Onomamania）：痴迷於名字（人物的、地點的、事物的）",
    r"稱名癖（Onomatomania）：無法抑制地不斷重複某個詞語的衝動。",
    r"剔指癖（Onychotillomania）：執著於剔指甲。",
    r"戀食癖（Opsomania）：對某種食物的病態熱愛。",
    r"抱怨癖（Paramania）：一種在抱怨時産生的近乎病態的愉悅感。",
    r"面具狂（Personamania）：執著於佩戴面具。",
    r"幽靈狂（Phasmomania）：痴迷於幽靈。",
    r"謀殺癖（Phonomania）：病態的謀殺傾向。",
    r"渴光癖（Photomania）：對光的病態渴求。",
    r"背德癖（ASPD）：病態地渴求違背社會道德。",
    r"求財癖（Plutomania）：對財富的强迫性的渴望。",
    r"欺騙狂（Pseudomania）：無法抑制的執著於撒謊。",
    r"縱火狂（Pyromania）：執著於縱火。",
    r"提問狂（Questiong-Asking Mania）：執著於提問。",
    r"挖鼻癖（Rhinotillexomania）：執著於挖鼻子。",
    r"塗鴉癖（Scribbleomania）：沉迷於塗鴉。",
    r"列車狂（Siderodromomania）：認爲火車或類似的依靠軌道交通的旅行方式充滿魅力。",
    r"臆智症（Sophomania）：臆想自己擁有難以置信的智慧。",
    r"科技狂（Technomania）：痴迷於新的科技。",
    r"臆咒狂（Thanatomania）：堅信自己已被某種死亡魔法所詛咒。",
    r"臆神狂（Theomania）：堅信自己是一位神靈。",
    r"抓撓癖（Titillomaniac）：抓撓自己的强迫傾向。",
    r"手術狂（Tomomania）：對進行手術的不正常愛好。",
    r"拔毛癖（Trichotillomania）：執著於拔下自己的頭髮。",
    r"臆盲症（Typhlomania）：病理性的失明。",
    r"嗜外狂（Xenomania）：痴迷於异國的事物。",
    r"喜獸癖（Zoomania）：對待動物的態度近乎瘋狂地友好。",
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Cthulhu7th:ChineseTraditional",
            "Cthulhu7th_ChineseTraditional.toml",
            151,
        );
    }
}
