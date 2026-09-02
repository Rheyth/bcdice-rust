use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarthDawn3;

impl GameSystem for EarthDawn3 {
    fn id(&self) -> &'static str {
        "EarthDawn3"
    }
    fn name(&self) -> &'static str {
        "アースドーン3版"
    }
    fn sort_key(&self) -> &'static str {
        "ああすとおん3"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+e"]
    }
    crate::impl_prefixes_pattern!();
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(step_result(command, rng)?.map(SpecificCommandOutput::text))
    }
}

static HELP_MESSAGE: &str = r"ステップダイス　(xEn+k)
ステップx、目標値n(省略可能）、カルマダイスk(D2～D20)でステップダイスをロールします。
振り足しも自動。
例）ステップ10：10E
　　ステップ10、目標値8：10E8
　　ステップ12、目標値8、カルマダイスD12：10E8+1D6
";

fn pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)E(\d+)?(?:\+(\d*)D(\d+))?(\+\d)?").expect("valid regex")
    })
}

fn to_i(value: Option<regex::Match<'_>>) -> i64 {
    value.and_then(|m| m.as_str().parse().ok()).unwrap_or(0)
}

fn step_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = pattern().captures(command) else {
        return Ok(None);
    };
    let step = to_i(m.get(1));
    let target = to_i(m.get(2)).min(20);
    let karma = m
        .get(3)
        .map(|v| v.as_str().parse().unwrap_or(0))
        .unwrap_or(0)
        .max(1);
    let karma_type = to_i(m.get(4));
    let dice_modify = to_i(m.get(5));
    let mut info = step_info_3(step);
    let modify = info[6] + dice_modify;
    let mut calc = String::new();
    let mut failed = true;
    let mut total = 0;

    for (sides, count) in [20, 12, 10, 8, 6, 4].into_iter().zip(info.drain(..6)) {
        total += roll_step(sides, count, &mut calc, &mut failed, rng)?;
    }
    if m.get(4).is_some() {
        total += roll_step(karma_type, karma, &mut calc, &mut failed, rng)?;
    }
    push_modify(&mut calc, modify);
    total += modify;

    let mut output = if target == 0 {
        format!("ステップ{step} ＞ {calc} ＞ {total}")
    } else {
        format!("ステップ{step}>={target} ＞ {calc} ＞ {total}")
    };
    if target != 0 {
        output.push_str(" ＞ ");
        output.push_str(success(target, total, failed));
    }
    Ok(Some(output))
}

fn step_info_3(step: i64) -> Vec<i64> {
    const BASE: [[i64; 7]; 7] = [
        [0, 0, 0, 0, 1, 0, -3],
        [0, 0, 0, 0, 1, 0, -2],
        [0, 0, 0, 0, 1, 0, -1],
        [0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0, 0],
    ];
    const RHYTHM: [[i64; 7]; 7] = [
        [0, 0, 0, 0, 2, 0, 0],
        [0, 0, 0, 1, 1, 0, 0],
        [0, 0, 0, 2, 0, 0, 0],
        [0, 0, 1, 1, 0, 0, 0],
        [0, 0, 2, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0],
        [0, 2, 0, 0, 0, 0, 0],
    ];
    if step <= 7 {
        return BASE[usize::try_from(step.saturating_sub(1)).unwrap_or(0)].to_vec();
    }
    let over = step - 8;
    let mut result = vec![0; 7];
    add_step(&mut result, &[0, 1, 0, 0, 0, 0, 0], over / 7);
    add_step(&mut result, &RHYTHM[(over % 7) as usize], 1);
    result
}

pub(crate) fn add_step(result: &mut [i64], step: &[i64], times: i64) {
    for (value, add) in result.iter_mut().zip(step) {
        *value += add * times;
    }
}

pub(crate) fn roll_step(
    sides: i64,
    count: i64,
    calc: &mut String,
    failed: &mut bool,
    rng: &mut Randomizer,
) -> Result<i64, EvalError> {
    if count <= 0 {
        return Ok(0);
    }
    if !calc.is_empty() {
        calc.push('+');
    }
    calc.push_str(&format!("{count}d{sides}["));
    let mut total = 0;
    for i in 0..count {
        let mut die = rng.roll_once(sides)?;
        if die != 1 {
            *failed = false;
        }
        let mut subtotal = die;
        while die == sides {
            die = rng.roll_once(sides)?;
            subtotal += die;
        }
        if i != 0 {
            calc.push(',');
        }
        calc.push_str(&subtotal.to_string());
        total += subtotal;
    }
    calc.push(']');
    Ok(total)
}

pub(crate) fn push_modify(calc: &mut String, modify: i64) {
    if modify > 0 {
        calc.push('+');
    }
    if modify != 0 {
        calc.push_str(&modify.to_string());
    }
}

fn success(target: i64, total: i64, failed: bool) -> &'static str {
    if failed {
        return "自動失敗";
    }
    let row = SUCCESS
        .iter()
        .find(|row| row[0] >= target)
        .unwrap_or(&SUCCESS[SUCCESS.len() - 1]);
    if total >= row[6] {
        "Extraordinary(極上)"
    } else if total >= row[5] {
        "Excelent(最高)"
    } else if total >= row[4] {
        "Good(上出来)"
    } else if total >= row[3] {
        "Average(そこそこ)"
    } else if total >= row[2] {
        "Poor(お粗末)"
    } else {
        "Pathetic(惨め)"
    }
}

static SUCCESS: [[i64; 7]; 39] = [
    [2, 0, 1, 2, 5, 7, 9],
    [3, 0, 1, 3, 6, 8, 10],
    [4, 0, 1, 4, 7, 10, 12],
    [5, 1, 2, 5, 8, 11, 14],
    [6, 1, 2, 6, 9, 13, 17],
    [7, 1, 3, 7, 11, 15, 19],
    [8, 1, 4, 8, 13, 16, 20],
    [9, 1, 5, 9, 15, 18, 22],
    [10, 1, 6, 10, 16, 20, 24],
    [11, 1, 6, 11, 17, 21, 25],
    [12, 1, 7, 12, 18, 23, 27],
    [13, 1, 7, 13, 20, 25, 29],
    [14, 1, 8, 14, 21, 26, 31],
    [15, 1, 9, 15, 23, 27, 31],
    [16, 1, 10, 16, 24, 28, 33],
    [17, 1, 11, 17, 25, 30, 34],
    [18, 1, 12, 18, 26, 31, 36],
    [19, 1, 12, 19, 28, 33, 37],
    [20, 1, 13, 20, 29, 34, 39],
    [21, 1, 14, 21, 30, 36, 41],
    [22, 1, 15, 22, 31, 37, 42],
    [23, 1, 16, 23, 33, 38, 43],
    [24, 1, 16, 24, 34, 39, 44],
    [25, 1, 17, 25, 35, 41, 46],
    [26, 1, 18, 26, 36, 42, 47],
    [27, 1, 19, 27, 37, 43, 49],
    [28, 1, 19, 28, 39, 45, 50],
    [29, 1, 21, 29, 40, 46, 51],
    [30, 1, 21, 30, 41, 47, 53],
    [31, 1, 22, 31, 42, 48, 54],
    [32, 1, 23, 32, 43, 49, 55],
    [33, 1, 24, 33, 45, 51, 57],
    [34, 1, 24, 34, 46, 52, 58],
    [35, 1, 25, 35, 47, 53, 60],
    [36, 1, 26, 36, 48, 54, 60],
    [37, 1, 27, 37, 49, 56, 62],
    [38, 1, 28, 38, 51, 57, 63],
    [39, 1, 29, 39, 52, 58, 64],
    [40, 1, 30, 40, 53, 59, 66],
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EarthDawn3",
            "EarthDawn3.toml",
            35,
        );
    }
}
