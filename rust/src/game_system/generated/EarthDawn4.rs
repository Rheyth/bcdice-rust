use std::sync::OnceLock;

use regex::Regex;

use super::EarthDawn3::{add_step, push_modify, roll_step};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarthDawn4;

impl GameSystem for EarthDawn4 {
    fn id(&self) -> &'static str {
        "EarthDawn4"
    }
    fn name(&self) -> &'static str {
        "アースドーン4版"
    }
    fn sort_key(&self) -> &'static str {
        "ああすとおん4"
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

static HELP_MESSAGE: &str = r"ステップダイス　(xEnK)
ステップx、目標値n(省略可能）でステップダイスをロール。
カルマダイス使用時は末尾にKを追加（省略可能）
例）ステップ10：10E
　　ステップ10、目標値8：10E8
　　ステップ10、目標値8、カルマダイス：10E8K
";

fn pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)E(\d+)?(K)?(\+\d+$)?(?:\+(.*))?").expect("valid regex")
    })
}

fn step_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let mut rest = command;
    let mut steps = Vec::new();
    let mut calcs = Vec::new();
    let mut totals = Vec::new();
    let mut target = 0;
    let mut last_failed = false;

    while !rest.is_empty() {
        let Some(m) = pattern().captures(rest) else {
            return Ok(None);
        };
        let step = m[1].parse().unwrap_or(0);
        let value = m.get(2).and_then(|v| v.as_str().parse().ok()).unwrap_or(0);
        let dice_modify = m.get(4).and_then(|v| v.as_str().parse().ok()).unwrap_or(0);
        let mut info = step_info(step);
        let modify = info[6] + dice_modify;
        let mut calc = String::new();
        let mut failed = true;
        let mut total = 0;

        for (sides, count) in [20, 12, 10, 8, 6, 4].into_iter().zip(info.drain(..6)) {
            total += roll_step(sides, count, &mut calc, &mut failed, rng)?;
        }
        if m.get(3).is_some() {
            total += roll_step(6, 1, &mut calc, &mut failed, rng)?;
        }
        last_failed = failed;
        push_modify(&mut calc, modify);
        total += modify;
        steps.push(format!("ステップ{step}"));
        calcs.push(calc);
        totals.push(total);
        if value != 0 {
            target = value;
        }

        let Some(next) = m.get(5) else { break };
        if next.as_str() == rest {
            break;
        }
        rest = next.as_str();
    }

    let step_text = steps.join("+");
    let mut calc_text = calcs.join(")+(");
    if calcs.len() > 1 {
        calc_text = format!(
            "({calc_text}) ＞ ({})",
            totals
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join("+")
        );
    }
    let total: i64 = totals.iter().sum();
    let output = if target == 0 {
        format!("{step_text} ＞ {calc_text} ＞ {total}")
    } else {
        let success = if total >= target {
            format!("成功 レベル：{}", (total - target) / 5 + 1)
        } else {
            "失敗".to_string()
        };
        let success = if last_failed {
            "自動失敗".to_string()
        } else {
            success
        };
        format!("{step_text}>={target} ＞ {calc_text} ＞ {total} ＞ {success}")
    };
    Ok(Some(output))
}

fn step_info(step: i64) -> Vec<i64> {
    const BASE: [[i64; 7]; 7] = [
        [0, 0, 0, 0, 0, 1, -2],
        [0, 0, 0, 0, 0, 1, -1],
        [0, 0, 0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0, 0],
    ];
    const RHYTHM: [[i64; 7]; 11] = [
        [0, 0, 0, 0, 2, 0, 0],
        [0, 0, 0, 1, 1, 0, 0],
        [0, 0, 0, 2, 0, 0, 0],
        [0, 0, 1, 1, 0, 0, 0],
        [0, 0, 2, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0],
        [0, 2, 0, 0, 0, 0, 0],
        [0, 1, 0, 0, 2, 0, 0],
        [0, 1, 0, 1, 1, 0, 0],
        [0, 1, 0, 2, 0, 0, 0],
        [0, 1, 1, 1, 0, 0, 0],
    ];
    if step <= 7 {
        return BASE[usize::try_from(step.saturating_sub(1)).unwrap_or(0)].to_vec();
    }
    let over = step - 8;
    let mut result = vec![0; 7];
    add_step(&mut result, &[1, 0, 0, 0, 0, 0, 0], over / 11);
    add_step(&mut result, &RHYTHM[(over % 11) as usize], 1);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EarthDawn4",
            "EarthDawn4.toml",
            35,
        );
    }
}
