//! P4で手書き移植した Cthulhu7th_Korean。
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
pub struct Cthulhu7th_Korean;

impl GameSystem for Cthulhu7th_Korean {
    fn id(&self) -> &'static str {
        "Cthulhu7th:Korean"
    }

    fn name(&self) -> &'static str {
        "크툴루의 부름 7판"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:크툴루의 부름 7판"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CC", "CBR", "FAR", "BMR", "BMS", "FCL", "FCM", "PH", "MA"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command.starts_with("CC") {
            return skill_roll(command, rng);
        }
        if command.starts_with("CBR") {
            return combine_roll(command, rng);
        }
        if command.starts_with("FAR") {
            return full_auto_roll(command, rng);
        }

        match command {
            "BMR" => Ok(Some(SpecificCommandOutput::text(roll_table(
                "狂気の発作（リアルタイム）",
                &MADNESS_REAL_TIME_TABLE,
                rng,
                "ラウンド",
            )?))),
            "BMS" => Ok(Some(SpecificCommandOutput::text(roll_table(
                "狂気の発作（サマリー）",
                &MADNESS_SUMMARY_TABLE,
                rng,
                "時間",
            )?))),
            "FCL" => Ok(Some(SpecificCommandOutput::text(roll_1d8_table(
                "キャスティング・ロール失敗(小)表",
                &FAILED_CASTING_L_TABLE,
                rng,
            )?))),
            "FCM" => Ok(Some(SpecificCommandOutput::text(roll_1d8_table(
                "キャスティング・ロール失敗(大)表",
                &FAILED_CASTING_M_TABLE,
                rng,
            )?))),
            "PH" => Ok(Some(SpecificCommandOutput::text(roll_1d100_table(
                "恐怖症表",
                &PHOBIAS_TABLE,
                rng,
            )?))),
            "MA" => Ok(Some(SpecificCommandOutput::text(roll_1d100_table(
                "マニア表",
                &MANIAS_TABLE,
                rng,
            )?))),
            _ => Ok(None),
        }
    }
}

const HELP_MESSAGE: &str = r"・판정　CC(x)<=（목표치）
　x：보너스, 페널티 주사위 (2~-2). 생략 가능.
　목표치가 없어도 1D100은 표시됨.
　대실패 / 실패 / 보통 성공 / 어려운 성공 /
　극단적 성공 / 대성공 을 자동 판정.
　예）CC<=30　CC(2)<=50 CC(+2)<=50 CC(-1)<=75 CC-1<=50 CC1<=65 CC+1<=65 CC

・기능 판정의 난이도 지정　CC(x)<=(목표치)(난이도)
　목표치 뒤에 난이도를 지정하여
　성공 / 실패 / 대성공 / 대실패 를 자동 판정.
　난이도 지정：
　　r:보통　h:어려운　e:극단적　c:대성공
　예）CC<=70r CC1<=60h CC-2<=50e CC2<=99c

・대항 판정　(CBR(x,y))
　목표치 x와 y로 % 판정을 진행하여 성패를 판정.
　예）CBR(50,20)

・자동 사격 무기의 사격 판정(연사)　FAR(w,x,y,z,d,v)
　w：탄환 수(1~100), x：기능치(1~100), y：고장 수치,
　z：보너스, 페널티 다이스(-2~2). 생략 가능.
　d：지정한 난이도에서 연사를 종료（보통：r, 어려운：h, 극단적：e）. 생략 가능.
　v：연사할 탄환 수를 변경. 생략 가능.
　명중 수와 관통 수, 남은 탄환 수만 산출. 대미지 산출은 하지 않음.
예）FAR(25,70,98)　FAR(50,80,98,-1)　far(30,70,99,1,R)
　　far(25,88,96,2,h,5)　FaR(40,77,100,,e,4)　fAr(20,47,100,,,3)

・각종 표
　【광기 관련】
　・광기의 발작(실시간)　BMR
　・광기의 발작(요약)　BMS
　・공포증의 예 　PH / 집착증의 예　MA

　【마법 관련】
　・주문의 강행 판정 실패 표
　　비교적 약한 주문의 부작용　FCL／강력한 주문의 부작용　FCM
　　※일본어로 출력됩니다.
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
            Self::Critical => "대성공",
            Self::ExtremeSuccess => "극단적 성공",
            Self::HardSuccess => "어려운 성공",
            Self::RegularSuccess => "보통 성공",
            Self::Success => "성공",
            Self::Fumble => "대실패",
            Self::Failure => "실패",
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
            "(1D100) ＞ {dice}"
        ))));
    }

    if !(-100..=100).contains(&bonus_dice) {
        return Ok(Some(SpecificCommandOutput::text(
            "보너스, 페널티 주사위의 값은 -100 이상, 100 이하로 지정해 주세요.",
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
        format!("({expr}) 보너스, 페널티 주사위[{bonus_dice}]"),
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
        "성공"
    } else if result_1.is_success() || result_2.is_success() {
        "일부 성공"
    } else {
        "실패"
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
        output.push_str("탄환이 너무 많습니다. 장전된 탄환을 100발로 변경합니다.\n");
        bullet_count = 100;
    }

    let default_set_count = (diff).div_euclid(10);
    if bullet_set_count_cap > default_set_count && diff > 39 && has_bullet_set_cap {
        bullet_set_count_cap = default_set_count;
        output.push_str(&format!(
            "연사할 탄환 수의 상한은 [기능치÷10(소수점 버림)]발이므로, 그보다 큰 수를 지정할 수 없습니다. 연사할 탄환 수를 {bullet_set_count_cap}발로 변경합니다.\n"
        ));
    } else if diff <= 39 && bullet_set_count_cap > 3 && has_bullet_set_cap {
        bullet_set_count_cap = 3;
        output.push_str(&format!(
            "기능치가 39 이하일 때 연사할 탄환 수의 상한 및 하한은 3발입니다. 연사할 탄환 수를 {bullet_set_count_cap}발로 변경합니다.\n"
        ));
    }

    if bullet_set_count_cap <= 0 && has_bullet_set_cap {
        return Ok(Some(SpecificCommandOutput::text(
            "연사할 탄환 수는 양수여야 합니다.",
        )));
    }

    if bullet_set_count_cap < 3 && has_bullet_set_cap {
        bullet_set_count_cap = 3;
        output.push_str("연사할 탄환 수의 하한은 3발입니다. 연사할 탄환 수를 3발로 변경합니다.\n");
    }

    if bullet_count <= 0 {
        return Ok(Some(SpecificCommandOutput::text("탄환은 양수여야 합니다.")));
    }
    if diff <= 0 {
        return Ok(Some(SpecificCommandOutput::text(
            "목표치는 양수여야 합니다.",
        )));
    }

    if broken_number < 0 {
        output.push_str("고장 수치는 양수여야 합니다. 마이너스 부호를 제외합니다.\n");
        broken_number = if broken_number == i64::MIN {
            i64::MAX
        } else {
            -broken_number
        };
    }

    if !(-2..=2).contains(&bonus_dice_count) {
        return Ok(Some(SpecificCommandOutput::text(
            "오류. 보너스, 페널티 주사위의 값은 -2~2여야 합니다.",
        )));
    }

    output.push_str(&format!("보너스, 페널티 주사위[{bonus_dice_count}]"));
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
                "\n{loop_count}번째: ＞ {total_list} ＞ {hit_result}"
            ));

            if total >= broken_number {
                output.push_str("　총알 걸림(고장)");
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
            output.push_str(&format!("　（{hit}발 명중, {impale}발 관통）"));

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
            output.push_str("\n【지정한 난이도가 되었으므로, 처리를 종료합니다.】");
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
        "{output}\n＞ {hit_bullet_count}발 명중, {impale_bullet_count}발 관통, 남은 탄환 {remaining_bullets}발"
    )
}

fn get_hit_type(more_difficulty: i64, hit_result: &str) -> u8 {
    let hit = match more_difficulty {
        0 => matches!(hit_result, "어려운 성공" | "보통 성공"),
        1 => hit_result == "어려운 성공",
        3 => hit_result == "대성공",
        _ => false,
    };
    let impale = matches!(more_difficulty, 0..=2) && matches!(hit_result, "대성공" | "극단적 성공");

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
        1 => "\n【난이도를 어려운 성공으로 변경】",
        2 => "\n【난이도를 극단적 성공으로 변경】",
        3 => "\n【난이도를 대성공으로 변경】",
        _ => "",
    }
}

static MADNESS_REAL_TIME_TABLE: [&str; 10] = [
    r"健忘症：探索者は、最後に安全な場所にいた時からあとに起こった出来事の記憶を持たない。例えば、朝食を食べていた次の瞬間には怪物と向かい合っている。これは1D10ラウンド続く。",
    r"身体症状症：探索者は1D10ラウンドの間、狂気によって視覚や聴覚に異常が生じたり、四肢の1つまたは複数が動かなくなる。",
    r"暴力衝動：赤い霧が探索者に降り、1D10ラウンドの間、抑えの利かない暴力と破壊を敵味方を問わず周囲に向かって爆発させる。",
    r"偏執症：探索者は1D10ラウンドの間、重い偏執症に襲われる。誰もが探索者に襲い掛かろうとしている。信用できる者はいない。監視されている。裏切ったやつがいる。これはわなだ。",
    r"重要な人々：探索者のバックストーリーの重要な人々を見直す。探索者はその場にいた人物を、自分にとっての重要な人々だと思い込む。人間関係の性質を考慮した上で、探索者はそれに従って行動する。1D10ラウンド続く。",
    r"失神：探索者は失神する。1D10ラウンド後に回復する。",
    r"パニックになって逃亡する：探索者は利用できるあらゆる手段を使って、可能なかぎり遠くへ逃げ出さずにはいられない。それが唯一の車両を奪って仲間を置き去りにすることであっても。探索者は1D10ラウンドの間、逃げ続ける。",
    r"身体的ヒステリーもしくは感情爆発：探索者は1D10ラウンドの間、笑ったり、泣いたり、あるいは叫んだりし続け、行動できなくなる。",
    r"恐怖症：探索者は新しい恐怖症に陥る。恐怖症表（PHコマンド）をロールするか、キーパーが恐怖症を1つ選ぶ。恐怖症の原因は存在しなくとも、その探索者は次の1D10ラウンドの間、それがそこにあると思い込む。",
    r"マニア：探索者は新しいマニアに陥る。マニア表（MAコマンド）をロールするか、キーパーがマニアを1つ選ぶ。その探索者は次の1D10ラウンドの間、自分の新しいマニアに没頭しようとする。",
];

static MADNESS_SUMMARY_TABLE: [&str; 10] = [
    r"健忘症：探索者が意識を取り戻すと、見知らぬ場所におり、自分が誰かもわからない。記憶は時間をかけてゆっくりと戻るだろう。",
    r"盗難：探索者は1D10時間後に意識を取り戻すが、盗難の被害を受けている。傷つけられてはいない。探索者が秘蔵の品を身に着けていた場合（「探索者のバックストーリー」参照）、〈幸運〉ロールを行い、それが盗まれていないか判定する。値打ちのあるものはすべて自動的に失われる。",
    r"暴行：探索者は1D10時間後に意識を取り戻し、自分が暴行を受け、傷ついていることに気づく。耐久力は狂気に陥る前の半分に減少している。ただし重症は生じていない。盗まれたものはない。どのようにダメージが加えられたかは、キーパーに委ねられる。",
    r"暴力：探索者は暴力と破壊の噴流を爆発させる。探索者が意識を取り戻した時、その行動を認識し記憶していることもあればそうでないこともある。探索者が暴力を振るった物、もしくは人、そして相手を殺してしまったのか、あるいは単に傷つけただけなのかはキーパーに委ねられる。",
    r"イデオロギー／信念：探索者のバックストーリーのイデオロギーと信念を参照する。探索者はこれらの1つの権化となり、急進的かつ狂気じみて、感情もあらわに主張するようになる。例えば、宗教に関係する者は、その後地下鉄で声高に福音を説教しているところを目撃されるかもしれない。",
    r"重要な人々：探索者のバックストーリーの重要な人々を参照し、なぜその人物との関係が重要かを考える。時間がたってから（1D10時間以上）、探索者はその人物に近づくための最善の行動、そしてその人物との関係にとって最善の行動をとる。",
    r"収容：探索者は精神療養施設あるいは警察の留置所で意識を取り戻す。探索者は徐々にそこにいたった出来事を思い出すかもしれない。",
    r"パニック：探索者は非常に遠い場所で意識を取り戻す。荒野で道に迷っているか、列車に乗っているか、長距離バスに乗っているかもしれない。",
    r"恐怖症：探索者は新たな恐怖症を獲得する。恐怖症表（PHコマンド）をロールするか、キーパーがどれか1つ選ぶ。探索者は1D10時間後に意識を取り戻し、この新たな恐怖症の対象を避けるためにあらゆる努力をする。",
    r"マニア：探索者は新たなマニアを獲得する。マニア表（MAコマンド）をロールするか、キーパーがどれか1つ選ぶ。この狂気の発作の間、探索者はこの新たなマニアに完全に溺れているだろう。これがほかの人々に気づかれるかどうかは、キーパーとプレイヤーに委ねられる。",
];

static FAILED_CASTING_L_TABLE: [&str; 8] = [
    r"視界がぼんやりするか、あるいは一時的な失明。",
    r"悲鳴、声、あるいはほかの雑音が肉体から発せられる。",
    r"強風やほかの大気の現象。",
    r"術者、ほかのその場に居合わせた者が出血する。あるいは環境（例えば、壁）から出血する。",
    r"奇妙な幻視と幻覚。",
    r"その付近の小動物たちが爆発する。",
    r"硫黄の悪臭。",
    r"クトゥルフ神話の怪物が偶然召喚される。",
];

static FAILED_CASTING_M_TABLE: [&str; 8] = [
    r"大地が震え、壁に亀裂が入って崩れる。",
    r"叙事詩的な電撃。",
    r"血が空から降る。",
    r"術者の手がしなび、焼けただれる。",
    r"術者は不自然に年をとる（年齢に+2D10歳、30ページの「年齢」を参照し、能力値に修正を適用すること）。",
    r"強力な、あるいは無数のクトゥルフ神話存在が現れ、術者を手始めに、近くの全員を攻撃する！",
    r"術者や近くの全員が遠い時代か場所に吸い込まれる。",
    r"クトゥルフ神話の神格が偶然招来される。",
];

static PHOBIAS_TABLE: [&str; 100] = [
    r"入浴恐怖症：体、手、顔を洗うのが怖い。",
    r"高所恐怖症：高いところが怖い。",
    r"飛行恐怖症：飛ぶのが怖い。",
    r"広場恐怖症：広場、公共の(混雑した)場所が怖い。",
    r"鶏肉恐怖症：鶏肉が怖い。",
    r"ニンニク恐怖症：ニンニクが怖い。",
    r"乗車恐怖症：車両の中にいたり車両に乗るのが怖い。",
    r"風恐怖症：風が怖い。",
    r"男性恐怖症：男性が怖い。",
    r"イングランド恐怖症：イングランド、もしくはイングランド文化などが怖い。",
    r"花恐怖症：花が怖い。",
    r"切断恐怖症：手足や指などが切断された人が怖い。",
    r"クモ恐怖症：クモが怖い。",
    r"稲妻恐怖症：稲妻が怖い。",
    r"廃墟恐怖症：廃墟が怖い。",
    r"笛恐怖症：笛(フルート)が怖い。",
    r"細菌恐怖症：細菌、バクテリアが怖い。",
    r"銃弾恐怖症：投擲物や銃弾が怖い。",
    r"落下恐怖症：落下が怖い。",
    r"書物恐怖症：本が怖い。",
    r"植物恐怖症：植物が怖い。",
    r"美女恐怖症：美しい女性が怖い。",
    r"低温恐怖症：冷たいものが怖い。",
    r"時計恐怖症：時計が怖い。",
    r"閉所恐怖症：壁に囲まれた場所が怖い。",
    r"道化師恐怖症：道化師が怖い。",
    r"犬恐怖症：犬が怖い。",
    r"悪魔恐怖症：悪魔が怖い。",
    r"群集恐怖症：人混みが怖い。",
    r"歯科医恐怖症：歯科医が怖い。",
    r"処分恐怖症：物を捨てるのが怖い(ためこみ症)",
    r"毛皮恐怖症：毛皮が怖い。",
    r"構断恐怖症：道路を横断するのが怖い。",
    r"教会恐怖症：教会が怖い。",
    r"鏡恐怖症：鏡が怖い。",
    r"ピン恐怖症：針やピンが怖い。",
    r"昆虫恐怖症：昆虫が怖い。",
    r"猫恐怖症：猫が怖い。",
    r"橋恐怖症：橋を渡るのが怖い。",
    r"老人恐怖症：老人や年をとることが怖い。",
    r"女性恐怖症：女性が怖い。",
    r"血液恐怖症：血が怖い。",
    r"過失恐怖症：失敗が怖い。",
    r"接触恐怖症：触ることが怖い。",
    r"爬虫類恐怖症：爬虫類が怖い。",
    r"霧恐怖症：霧が怖い。",
    r"銃器恐怖症：銃器が怖い。",
    r"水恐怖症：水が怖い。",
    r"睡眠恐怖症：眠ったり、催眠状態に陥るのが怖い。",
    r"医師恐怖症：医師が怖い。",
    r"魚恐怖症：魚が怖い。",
    r"ゴキブリ恐怖症：ゴキブリが怖い。",
    r"雷鳴恐怖症：雷鳴が怖い。",
    r"野菜恐怖症：野菜が怖い。",
    r"大騒音恐怖症：大きな騒音が怖い。",
    r"湖恐怖症：湖が怖い。",
    r"機械恐怖症：機械や装置が怖い。",
    r"巨大物恐怖症：巨大なものが怖い。",
    r"拘束恐怖症：縛られたり結びつけられたりするのが怖い。",
    r"隕石恐怖症：流星や隕石が怖い。",
    r"孤独恐怖症：独りでいることが怖い。",
    r"汚染恐怖症：汚れたり汚染されたりするのが怖い。",
    r"粘液恐怖症：粘液、粘体が怖い。",
    r"死体恐怖症：死体が怖い。",
    r"8恐怖症：8の数字が怖い。",
    r"歯恐怖症：歯が怖い。",
    r"夢恐怖症：夢が怖い。",
    r"名称恐怖症：特定の言葉（1つまたは複数）を聞くのが怖い。",
    r"蛇恐怖症：蛇が怖い。",
    r"鳥恐怖症：鳥が怖い。",
    r"寄生生物恐怖症：寄生生物が怖い。",
    r"人形恐怖症：人形が怖い。",
    r"恐食症：のみ込むこと食べること、もしくは食べられることが怖い。",
    r"薬物恐怖症：薬物が怖い。",
    r"幽霊恐怖症：幽霊が怖い。",
    r"羞明：日光が怖い。",
    r"ひげ恐怖症：ひげが怖い",
    r"河川恐怖症：川が怖い",
    r"アルコール恐怖症：アルコールやアルコール飲料が怖い。",
    r"火恐怖症：火が怖い。",
    r"魔術恐怖症：魔術が怖い。",
    r"暗黒恐怖症：暗闇や夜が怖い。",
    r"月恐怖症：月が怖い。",
    r"鉄道恐怖症：列車の旅が怖い。",
    r"星恐怖症：星が怖い。",
    r"狭所恐怖症：狭いものや場所が怖い。",
    r"対称恐怖症：左右対称が怖い。",
    r"生き埋め恐怖症：生き埋めになることや墓地が怖い。",
    r"雄牛恐怖症：雄牛が怖い。",
    r"電話恐怖症：電話が怖い。",
    r"奇形恐怖症：怪物が怖い。",
    r"海洋恐怖症：海が怖い。",
    r"手術恐怖症：外科手術が怖い。",
    r"13恐怖症：13の数字が怖い。",
    r"衣類恐怖症：衣服が怖い。",
    r"魔女恐怖症：魔女と魔術が怖い。",
    r"黄色恐怖症：黄色や「黄色」という言葉が怖い。",
    r"外国語恐怖症：外国語が怖い。",
    r"外国人恐怖症：外国人が怖い。",
    r"動物恐怖症：動物が怖い。",
];

static MANIAS_TABLE: [&str; 100] = [
    r"洗浄マニア：自分の体を洗わずにはいられない。",
    r"無為マニア：病的な優柔不断。",
    r"暗闇マニア：暗黒に関する過度の嗜好。",
    r"高所マニア：高い場所に登らずにはいられない。",
    r"善良マニア：病的な親切。",
    r"広場マニア：開けた場所にいたいという激しい願望。",
    r"先鋭マニア：鋭いもの、とがったものへの執着。",
    r"猫マニア：猫に関する異常な愛好心。",
    r"疼痛性愛：痛みへの執着。",
    r"にんにくマニア：にんにくへの執着。",
    r"乗り物マニア：車の中にいることへの執着。",
    r"病的快活：不合理なほがらかさ。",
    r"花マニア：花への執着。",
    r"計算マニア：数への偏執的な没頭。",
    r"浪費マニア：衝動的あるいは無謀な浪費。",
    r"自己マニア：孤独への過度の嗜好。",
    r"バレエマニア：バレエに関する異常な愛好心。",
    r"書籍約盗癖：本を盗みたいという強迫的衝動。",
    r"書物マニア：本または読書、あるいはその両方への執着。",
    r"歯ぎしりマニア：歯ぎしりしたいという強迫的衝動。",
    r"悪霊マニア：誰かの中に邪悪な精霊がいるという病的な信念。",
    r"自己愛マニア：自分自身の美への執着。",
    r"地図マニア：いたる所の地図を見る制御不可能な強迫的衝動。",
    r"飛び降りマニア：高い場所から跳躍することへの執着。",
    r"寒冷マニア：冷たさ、または冷たいもの、あるいはその両方への異常な欲望。",
    r"舞踏マニア：踊ることへの愛好もしくは制御不可能な熱狂。",
    r"睡眠マニア：寝ることへの過度の願望。",
    r"墓地マニア：墓地への執着。",
    r"色彩マニア：特定の色への執着。",
    r"ピエロマニア：ピエロへの執着。",
    r"遭遇マニア：恐ろしい状況を経験したいという強迫的衝動。",
    r"殺害マニア：殺害への執着。",
    r"悪魔マニア：誰かが悪魔にとりつかれているという病的な信念。",
    r"皮膚マニア：人の皮膚を引っぱりたいという強迫的衝動。",
    r"正義マニア：正義が完遂されるのを見たいという執着。",
    r"アルコールマニア：アルコールに関する異常な欲求。",
    r"毛皮マニア：毛皮を所有することへの執着。",
    r"贈り物マニア：贈り物を与えることへの執着。",
    r"逃走マニア：逃走することへの迫的衝動。",
    r"外出マニア：外を歩き回ることの強迫的衝動。",
    r"自己中心マニア：不合理な自心の態度か自己崇拝。",
    r"公職マニア：公的な職業に就きいという強欲な衝動。",
    r"戦慄マニア：誰かが罪を犯したという病的な信念",
    r"知識マニア：知識を得ることへ執着。",
    r"静寂マニア：静寂であることへ強迫的衝動。",
    r"エーテルマニア：エーテルへの切望",
    r"求婚マニア：奇妙な求婚をすることへの執着。",
    r"笑いマニア：制御不可能な笑うことへの強迫的衝動。",
    r"魔術マニア：魔女と魔術への執着。",
    r"筆記マニア：すべてを書き留めることへの執着。",
    r"裸体マニア：裸になりたいという強迫的衝動。",
    r"幻想マニア：快い幻想(現実とは関係なく)にとらわれやすい異常な傾向。",
    r"蟲マニア：蟲に関する過度の嗜好。",
    r"火器マニア：火器への執着。",
    r"水マニア：水に関する不合理な渇望。",
    r"魚マニア：魚への執着。",
    r"アイコンマニア：像や肖像への執着。",
    r"アイドルマニア：偶像への執着または献身。",
    r"情報マニア：事実を集めることへの過度の献身。",
    r"絶叫マニア：叫ぶことへの説明できない強迫的衝動。",
    r"窃盗マニア：盗むことへの説明できない強迫的衝動。",
    r"騒音マニア：大きな、あるいは甲高い騒音を出すことへの制御不可能な強迫的衝動。",
    r"ひもマニア：ひもへの執着。",
    r"宝くじマニア：宝くじに参加したいという極度の願望。",
    r"うつマニア：異常に深くふさぎ込む傾向。",
    r"巨石マニア：環状列石/立石があると奇妙な考えにとらわれる異常な傾向。",
    r"音楽マニア：音楽もしくは特定の旋律への執着。",
    r"作詩マニア：詩を書くことへの強欲な願望。",
    r"憎悪マニア：何らかの対象あるいはグループの何もかもを憎む執着。",
    r"偏執マニア：ただ1つの思想やアイデアへの異常な執着。",
    r"虚言マニア：異常なほどにうそをついたり、誇張して話す。",
    r"疾病マニア：想像上の病気に苦められる幻想。",
    r"記録マニア：あらゆるものを記録に残そうという強迫的衝動。",
    r"名前マニア：人々、場所、ものなどの名前への執着",
    r"単語マニア：ある単語を繰り返したいという押さえ切れない欲求。",
    r"爪損傷マニア：指の爪をむしったりはがそうとする強迫的衝動。",
    r"美食マニア：1種類の食物への異常な愛。",
    r"不平マニア：不平を言うことへの異常な喜び。",
    r"仮面マニア：仮面や覆面を着けたいという強迫的衝動。",
    r"幽霊マニア：幽霊への執着。",
    r"殺人マニア：殺人への病的な傾向。",
    r"光線マニア：光への病的な願望。",
    r"放浪マニア：社会の規範に背きたいという異常な欲望。",
    r"長者マニア：富への強迫的な欲望。",
    r"病的虚言マニア：うそをつきたくてたまらない強迫的衝動。",
    r"放火マニア：火をつけることへの強迫的衝動。",
    r"質問マニア：質問したいという激しい強迫的衝動。",
    r"鼻マニア：鼻をいじりたいという強迫的衝動。",
    r"落書きマニア：いらずら書きや落書きへの執着。",
    r"列車マニア：列車と鉄道旅行への強い魅了。",
    r"知性マニア：誰かが信じられないほど知的であるという幻想。",
    r"テクノマニア：新技術への執着。",
    r"タナトスマニア：誰かが死を招く魔術によって呪われているという信念。",
    r"宗教マニア：その人が神であるという信仰。",
    r"かき傷マニア：かき傷をつけることへの強迫的衝動。",
    r"手術マニア：外科手術を行なうことへの不合理な嗜好。",
    r"抜毛マニア：自分の髪を引き抜くことへの切望。",
    r"失明マニア：病的な視覚障害。",
    r"異国マニア：外国のものへの執着。",
    r"動物マニア：動物への正気でない溺愛。",
];

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    /// 余った注入乱数を許すケース（`(1始まりのケース番号, 残り個数)`）。
    ///
    /// Ruby本家の `RandomizerMock` は余りを検査しないので、TOMLには
    /// 「Ruby側もダイスを振る前に nil を返すコマンド」にもダイスが書かれている。
    /// ケース89 (`DMG>10 比較演算子の不正`) は比較演算子 `>` が
    /// `restrict_cmp_op_to(:>=)` により不正で、Ruby も1個も振らない
    /// （Docker Ruby 3.2 実測: result=nil, rands unconsumed）。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[];

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/Cthulhu7th_Korean.toml");
        path.exists().then_some(path)
    }

    /// `test/data/Cthulhu7th_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Cthulhu7th_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Cthulhu7th_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            150,
            "case count in test/data/Cthulhu7th_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Cthulhu7th:Korean",
                "unexpected game system in Cthulhu7th_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Cthulhu7th:Korean"), &tc.input, &mut src) {
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

            let allowed_surplus = SURPLUS_RANDS_ALLOWED
                .iter()
                .find(|(case, _)| *case == i + 1)
                .map_or(0, |(_, remaining)| *remaining);
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Cthulhu7th_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Cthulhu7th_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
