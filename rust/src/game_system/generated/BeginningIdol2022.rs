//! `lib/bcdice/game_system/BeginningIdol2022.rb` の移植。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginningIdol2022;

impl GameSystem for BeginningIdol2022 {
    fn id(&self) -> &'static str {
        "BeginningIdol2022"
    }

    fn name(&self) -> &'static str {
        "ビギニングアイドル（2022年改訂版）"
    }

    fn sort_key(&self) -> &'static str {
        "ひきにんくあいとる2022"
    }

    fn help_message(&self) -> &'static str {
        r"これは、2022年に大判サイズで発売された『駆け出しアイドルRPG ビギニングアイドル 基本ルールブック』に対応したコマンドです。

・行為判定　BIn@c#f+m>=t
　nD6をダイスロールし、行為判定に成功したかを出力します。スペシャルとファンブルの判定も行います。
　　n: ダイス数（省略時 2)
　　c: スペシャル値（省略時 12)
　　f: ファンブル値（省略時 2)
　　m: 修正値（省略可)
　　t: 目標値

・パフォーマンス判定　PDn+m
　nD6をダイスロールし、パフォーマンス値を出力します。パーフェクトミラクルとミラクルの判定も行います。
　　n: ダイス数
　　m: 修正値（省略可)

・シンフォニー　xxxPDn+m
　nD6をダイスロールし、場に残っているダイスを加味してパフォーマンス値を出力します。
　パーフェクトミラクルとミラクルシンクロの判定も行います。
　　xxx: 場に残っているダイスの出目を列挙したもの
　　n: ダイス数
　　m: 修正値（省略可)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["BI", "PD", "[1-6]+PD"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_skill_check(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = roll_performance_check(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(roll_symphony_check(command, rng)?.map(SpecificCommandOutput::result))
    }
}

fn roll_skill_check(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["BI"], RoundType::Floor)
        .enable_suffix_number()
        .enable_critical()
        .enable_fumble()
        .restrict_cmp_op_to(&[Some(CmpOp::Ge)]);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_times = parsed
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    let critical = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(12);
    let fumble = parsed
        .fumble
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    let mut dice = rng.roll_barabara(dice_times, 6)?;
    dice.sort_unstable();
    let dice_total = dice.iter().sum::<i64>();
    let total = dice_total + parsed.modify_number.clone();
    let mut result = if dice_total >= critical {
        EvalResult::critical("スペシャル(PCは【思い出】を1つ獲得する)")
    } else if dice_total <= fumble {
        EvalResult::fumble("ファンブル(【思い出】を1つ獲得し、ファンブル表を振る)")
    } else if total >= parsed.target_number.clone().unwrap_or(crate::Int::from(0)) {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };
    result.text = format!(
        "({}) ＞ {dice_total}[{}]{} ＞ {total} ＞ {}",
        parsed.to_s(SuffixPosition::AfterCommand),
        join(&dice),
        modifier(&parsed.modify_number),
        result.text
    );
    Ok(Some(result))
}

fn roll_performance_check(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^PD(\d+)([+-]\d+)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };
    let suffix = parse_i64(&captures[1]);
    let adjust = captures.get(2).map_or(0, |m| parse_i64(m.as_str()));
    let extension = suffix >= 7;
    let dice_times = if extension { 6 } else { suffix };
    if dice_times <= 0 {
        return Ok(None);
    }
    let extension_bonus = if extension { suffix - dice_times } else { 0 };
    let mut dice = rng.roll_barabara(dice_times, 6)?;
    dice.sort_unstable();
    let unique = select_uniqs(&dice);
    let perfect = unique == [1, 2, 3, 4, 5, 6];
    let miracle = unique.is_empty();
    let mut label = if perfect {
        format!("【パーフェクトミラクル】{}", 30 + extension_bonus + adjust)
    } else if miracle {
        format!("【ミラクル】{}", 10 + extension_bonus + adjust)
    } else {
        (unique.iter().sum::<i64>() + extension_bonus + adjust).to_string()
    };
    if extension {
        label.push_str(&format!(
            " (エクステンション: {extension_bonus}個まで振りなおし可能)"
        ));
    }

    let adjustments = format!(
        "{}{}",
        modifier(&crate::Int::from(extension_bonus)),
        modifier(&crate::Int::from(adjust))
    );
    let mut sequence = vec![
        format!("({command})"),
        "パフォーマンス判定".to_owned(),
        format!("[{}]{adjustments}", join(&dice)),
    ];
    if dice.len() != unique.len() {
        sequence.push(format!("[{}]{adjustments}", join(&unique)));
    }
    sequence.push(label);
    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.critical = perfect || miracle;
    Ok(Some(result))
}

fn roll_symphony_check(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([1-6]+)PD([1-6])([+-]\d+)?$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };
    let mut carries = captures[1]
        .bytes()
        .map(|byte| i64::from(byte - b'0'))
        .collect::<Vec<_>>();
    carries.sort_unstable();
    let dice_times = parse_i64(&captures[2]);
    let adjust = captures.get(3).map_or(0, |m| parse_i64(m.as_str()));
    let mut dice = rng.roll_barabara(dice_times, 6)?;
    dice.sort_unstable();
    let mut all = carries.clone();
    all.extend_from_slice(&dice);
    let unique = select_uniqs(&all);
    let perfect = unique == [1, 2, 3, 4, 5, 6];
    let synchro = unique.is_empty();
    let label = if perfect {
        format!("【パーフェクトミラクル】{}", 30 + adjust)
    } else if synchro {
        format!("【ミラクルシンクロ】{}", 20 + adjust)
    } else {
        (unique.iter().sum::<i64>() + adjust).to_string()
    };
    let mut result = EvalResult::with_text(format!(
        "({command}) ＞ シンフォニー ＞ [{}],[{}]{} ＞ [{}]{} ＞ {label}",
        join(&carries),
        join(&dice),
        modifier(&crate::Int::from(adjust)),
        join(&unique),
        modifier(&crate::Int::from(adjust))
    ));
    result.critical = perfect || synchro;
    Ok(Some(result))
}

fn select_uniqs(values: &[i64]) -> Vec<i64> {
    let mut counts = [0usize; 7];
    for value in values {
        if let Ok(index) = usize::try_from(*value) {
            if let Some(count) = counts.get_mut(index) {
                *count += 1;
            }
        }
    }
    (1..=6)
        .filter(|value| counts[*value] == 1)
        .map(|value| value as i64)
        .collect()
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_i64(text: &str) -> i64 {
    text.parse().unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "BeginningIdol2022",
            "BeginningIdol2022.toml",
            40,
        );
    }
}
