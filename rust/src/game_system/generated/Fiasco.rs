//! `lib/bcdice/game_system/Fiasco.rb` の移植。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

pub(crate) struct SystemTables {
    pub(crate) white: &'static str,
    pub(crate) black: &'static str,
    pub(crate) count_suffix: &'static str,
    pub(crate) duplicate_white: &'static str,
    pub(crate) duplicate_black: &'static str,
}

pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = roll_fs(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_white_black(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(roll_white_black_single(tables, command, rng)?.map(SpecificCommandOutput::text))
}

fn roll_fs(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(count) = command.strip_prefix("FS").and_then(parse_digits) else {
        return Ok(None);
    };
    let dice = rng.roll_barabara(count, 6)?;
    let mut bucket = [0usize; 6];
    for value in dice {
        if let Some(slot) = usize::try_from(value - 1)
            .ok()
            .and_then(|index| bucket.get_mut(index))
        {
            *slot += 1;
        }
    }
    Ok(Some(
        bucket
            .iter()
            .enumerate()
            .map(|(index, count)| format!("{} => {}{}", index + 1, count, tables.count_suffix))
            .collect::<Vec<_>>()
            .join(", "),
    ))
}

fn roll_white_black_single(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([WB])(\d+)$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };
    let side = roll_side(tables, &captures[1], parse_i64(&captures[2]), rng)?;
    Ok(Some(format!(
        "{} ＞ {}{}",
        side.text, side.color, side.total
    )))
}

fn roll_white_black(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([WB])(\d+)([WB])(\d+)$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };
    if captures[1] == captures[3] {
        let message = if &captures[1] == "W" {
            tables.duplicate_white
        } else {
            tables.duplicate_black
        };
        return Ok(Some(format!("{command}：{message}")));
    }

    let a = roll_side(tables, &captures[1], parse_i64(&captures[2]), rng)?;
    let b = roll_side(tables, &captures[3], parse_i64(&captures[4]), rng)?;
    let diff = if a.total == b.total {
        "0".to_owned()
    } else if a.total > b.total {
        format!("{}{}", a.color, a.total - b.total)
    } else {
        format!("{}{}", b.color, b.total - a.total)
    };
    Ok(Some(format!("{} {} ＞ {diff}", a.text, b.text)))
}

struct Side {
    color: &'static str,
    total: i64,
    text: String,
}

fn roll_side(
    tables: &SystemTables,
    marker: &str,
    count: i64,
    rng: &mut Randomizer,
) -> Result<Side, EvalError> {
    let color = if marker == "W" {
        tables.white
    } else {
        tables.black
    };
    let dice = if count == 0 {
        vec![0]
    } else {
        rng.roll_barabara(count, 6)?
    };
    let total = dice.iter().sum::<i64>();
    let values = dice
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(Side {
        color,
        total,
        text: format!("{color}{total}[{values}]"),
    })
}

fn parse_digits(text: &str) -> Option<i64> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())).then(|| parse_i64(text))
}

fn parse_i64(text: &str) -> i64 {
    text.parse().unwrap_or(i64::MAX)
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    white: "白",
    black: "黒",
    count_suffix: "個",
    duplicate_white: "白指定(W)は重複できません。",
    duplicate_black: "黒指定(B)は重複できません。",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fiasco;

impl GameSystem for Fiasco {
    fn id(&self) -> &'static str {
        "Fiasco"
    }

    fn name(&self) -> &'static str {
        "フィアスコ"
    }

    fn sort_key(&self) -> &'static str {
        "ふいあすこ"
    }

    fn help_message(&self) -> &'static str {
        r"  ・判定コマンド(FSx, WxBx)
    相関図・転落要素用(FSx)：相関図や転落要素のためにx個ダイスを振り、出目ごとに分類する
    黒白差分判定用(WxBx)  ：転落、残響のために白ダイス(W指定)と黒ダイス(B指定)で差分を求める
      ※ WとBは片方指定(Bx, Wx)、入替指定(WxBx,BxWx)可能
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["FS", "W", "B"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "Fiasco",
            "Fiasco.toml",
            20,
            &[(9, 6), (10, 6), (19, 6), (20, 6)],
        );
    }
}
