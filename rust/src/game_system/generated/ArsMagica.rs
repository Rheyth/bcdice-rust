//! `lib/bcdice/game_system/ArsMagica.rb` の移植。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArsMagica;

impl GameSystem for ArsMagica {
    fn id(&self) -> &'static str {
        "ArsMagica"
    }
    fn name(&self) -> &'static str {
        "アルスマギカ"
    }
    fn sort_key(&self) -> &'static str {
        "あるすまきか"
    }
    fn help_message(&self) -> &'static str {
        r#"・ストレスダイス　(ArSx+y)
　"ArS(ボッチダイス)+(修正)"です。判定にも使えます。Rコマンド(1R10+y[m])に読替をします。
　ボッチダイスと修正は省略可能です。(ボッチダイスを省略すると1として扱います)
　botchダイスの0の数が2以上の時は、数えて表示します。
　（注意！） botchの判断が発生したときには、そのダイスを含めてロールした全てのダイスを[]の中に並べて表示します。
　例) (1R10[5]) ＞ 0[0,1,8,0,8,1] ＞ Botch!
　　最初の0が判断基準で、その右側5つがボッチダイスです。1*2,8*2,0*1なので1botchという訳です。
"#
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["ArS", "1R10"]
    }
    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_ars_magica(command, rng)?.map(SpecificCommandOutput::text))
    }
}

struct Parsed {
    botch: i64,
    bonus: I,
    cmp_op: Option<CmpOp>,
    target: Option<i64>,
}

fn parse(command: &str) -> Result<Option<Parsed>, EvalError> {
    static ARS: OnceLock<Regex> = OnceLock::new();
    static R10: OnceLock<Regex> = OnceLock::new();
    let ars = ARS
        .get_or_init(|| Regex::new(r"(?i)^ArS(\d+)?((?:[+-]-?\d+)+)?(?:([>=]+)(\d+))?$").unwrap());
    let r10 = R10.get_or_init(|| {
        Regex::new(r"(?i)^1R10((?:[+-]-?\d+)+)?(?:\[(\d+)\])?(?:([>=]+)(\d+))?$").unwrap()
    });

    let Some((botch, modifier, op, target)) = ars
        .captures(command)
        .map(|m| (m.get(1), m.get(2), m.get(3), m.get(4)))
        .or_else(|| {
            r10.captures(command)
                .map(|m| (m.get(2), m.get(1), m.get(3), m.get(4)))
        })
    else {
        return Ok(None);
    };

    Ok(Some(Parsed {
        botch: botch.map_or(1, |m| m.as_str().parse().unwrap_or(1)),
        bonus: arithmetic::eval(modifier.map_or("", |m| m.as_str()), RoundType::Floor)?
            .unwrap_or(I::ZERO),
        cmp_op: op.and_then(|m| normalize::comparison_operator(m.as_str())),
        target: target.and_then(|m| m.as_str().parse().ok()),
    }))
}

fn eval_ars_magica(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(parsed) = parse(command)? else {
        return Ok(None);
    };
    let mut cmp_op = parsed.cmp_op;
    let mut total = I::ZERO;
    let mut die = rng.roll_once(10)? - 1;
    let mut output = format!(
        "(1R10{}[{}]{}{}) ＞ ",
        format::modifier(&parsed.bonus),
        parsed.botch,
        format::comparison_operator(cmp_op),
        parsed.target.map_or_else(String::new, |n| n.to_string())
    );

    if die == 0 {
        let dice = (0..parsed.botch)
            .map(|_| rng.roll_once(10).map(|d| d - 1))
            .collect::<Result<Vec<_>, _>>()?;
        let count0 = dice.iter().filter(|&&d| d == 0).count();
        output.push_str(&format!(
            "0[0,{}]",
            dice.iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
        if count0 > 0 {
            if count0 > 1 {
                output.push_str(&format!(" ＞ {count0}Botch!"));
            } else {
                output.push_str(" ＞ Botch!");
            }
            cmp_op = None;
        } else {
            total = parsed.bonus.clone();
            if parsed.bonus > I::ZERO {
                output.push_str(&format!("+{} ＞ {total}", parsed.bonus));
            } else if parsed.bonus < I::ZERO {
                output.push_str(&format!("{} ＞ {total}", parsed.bonus));
            } else {
                output.push_str(" ＞ 0");
            }
        }
    } else if die == 1 {
        let mut multiplier = 1;
        let mut critical_dice = Vec::new();
        while die == 1 {
            multiplier *= 2;
            die = rng.roll_once(10)?;
            critical_dice.push(die);
        }
        total = crate::Int::from(die * multiplier);
        output.push_str(&format!(
            "{total}[1,{}]",
            critical_dice
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
        total += parsed.bonus.clone();
        if parsed.bonus != I::ZERO {
            output.push_str(&format!("{} ＞ {total}", format::modifier(&parsed.bonus)));
        }
    } else {
        total = crate::Int::from(die) + parsed.bonus.clone();
        if parsed.bonus == I::ZERO {
            output.push_str(&total.to_string());
        } else {
            output.push_str(&format!(
                "{die}{} ＞ {total}",
                format::modifier(&parsed.bonus)
            ));
        }
    }

    if cmp_op == Some(CmpOp::Ge) {
        output.push_str(
            if total >= parsed.target.map_or(I::ZERO, crate::Int::from) {
                " ＞ 成功"
            } else {
                " ＞ 失敗"
            },
        );
    }
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ArsMagica",
            "ArsMagica.toml",
            38,
        );
    }
}
