//! P4で手書き移植した `lib/bcdice/game_system/TorgEternity.rb`。
//!
//! メタデータは生成スタブの値をそのまま保ち、固有ロールと結果表を移植している。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::{self};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TorgEternity;

impl GameSystem for TorgEternity {
    fn id(&self) -> &'static str {
        "TorgEternity"
    }
    fn name(&self) -> &'static str {
        "トーグ エタニティ"
    }
    fn sort_key(&self) -> &'static str {
        "とおくえたにてい"
    }

    fn help_message(&self) -> &'static str {
        r#"・判定
　・TG
　　"TG[m]"で1d20をロールします。[]内は省略可能。
　　mは技能基本値を入れて下さい。Rコマンドに読替されます。
　　振り足しを自動で行い、20の出目が出たときには技能無し値も並記します。
　　(TORGダイスボットと同じ挙動をするコマンドです。ロールボーナスの読み替えのみ、Eternity版となります)
　・TE
　　"TE"で1d20をロールします。
　　振り足しを自動で行い、20の出目が出たときには技能無し値も並記します。
　　出目1の時には「Mishap!　自動失敗！」と出力されます。
　・UP
　　"UP[m]"で高揚状態のロール(通常の1d20に加え、1d20を追加で振り足し)を行います。
　　[]内は省略可能。mは技能基本値を入れて下さい。
　　各ロールでの振り足しを自動で行い、20の出目が出たときには技能無し値も並記します。
　　一投目で出目1の時には「Mishap!　自動失敗！」と出力され、二投目は行われません。
　・POS
　　"POSm"で、ポシビリティ使用による1d20のロールを行います。
　　mはポシビリティを使用する前のロール結果を入れて下さい。
　　出目が10未満の場合は、10への読み替えが行われます。
　　また、振り足しを自動で行い、20の出目が出たときには技能無し値も並記します。
　・CPOS
　　"CPOSm"で、コズムポシビリティ使用による1d20のロールを行います。
　　mはポシビリティを使用する前のロール結果を入れて下さい。
　　出目が10未満の場合でも、10への読み替えが行われません。
　　振り足しは自動で行い、20の出目が出たときには技能無し値も並記します。
・ボーナスダメージロール
　"xBD[+y]"でロールします。[]内は省略可能。
　xはダメージダイス数。yはダメージ基本値 or 式を入れて下さい。
　xは1以上が必要です。0だとエラーが出力されます。マイナス値はコマンドとして認識されません。
　振り足し処理は自動で行われます。(振り足し発生時の目は、「5∞」と出力されます)
・各種表
　"(表コマンド)(数値)"で振ります。
　・成功レベル表「RTx or RESULTx」
　・ダメージ結果表「DTx or DAMAGEx」
　・ロールボーナス表「BTx+y or BONUSx+y or TOTALx+y」 xは数値, yは技能基本値
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "TE", "UP", "POS", "CPOS", r"\d+BD", "TG", "RT", "Result", "DT", "damage", "BT",
            "bonus", "total", "1R20",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(output) = torg_check(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_d20(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_up(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_possibility(command, false, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_possibility(command, true, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_bonus_damage(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = success_level(command)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = damage_result(command)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    if let Some(output) = roll_bonus(command)? {
        return Ok(Some(SpecificCommandOutput::text(output)));
    }
    Ok(None)
}

fn eval_number(expr: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

fn regex(slot: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    slot.get_or_init(|| Regex::new(pattern).expect("valid regex"))
}

fn torg_check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^1R20(([+-]\d+)*)$")
}

fn tg_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)TG(\d+)")
}

fn replace_text(command: &str) -> String {
    static TG: OnceLock<Regex> = OnceLock::new();
    let command = tg_pattern().replace_all(command, "1R20+${1}");
    regex(&TG, r"(?i)TG")
        .replace_all(&command, "1R20")
        .into_owned()
}

fn torg_check(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let command = replace_text(command);
    let Some(captures) = torg_check_pattern().captures(&command) else {
        return Ok(None);
    };
    let modifier = captures
        .get(1)
        .filter(|m| !m.as_str().is_empty())
        .map(|m| eval_number(m.as_str()))
        .transpose()?
        .unwrap_or(0);
    let roll = torg_eternity_dice(false, false, rng)?;
    let skilled_bonus = get_torg_eternity_bonus(roll.skilled).unwrap_or(0);
    let mut output = if modifier > 0 {
        format!("{skilled_bonus}[{}]+{modifier}", roll.dice_str)
    } else {
        format!("{skilled_bonus}[{}]{modifier}", roll.dice_str)
    };
    output.push_str(&format!(" ＞ {}", skilled_bonus + modifier));
    if roll.skilled != roll.unskilled {
        output.push_str(&format!(
            "(技能無{})",
            get_torg_eternity_bonus(roll.unskilled).unwrap_or(0) + modifier
        ));
    }
    Ok(Some(format!("({command}) ＞ {output}")))
}

fn roll_d20(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "TE" {
        return Ok(None);
    }
    let roll = torg_eternity_dice(false, true, rng)?;
    if roll.mishap {
        return Ok(Some(format!(
            "d20ロール（通常） ＞ 1d20[{}] ＞ Mishap!　絶対失敗！",
            roll.dice_str
        )));
    }
    let skilled_bonus = get_torg_eternity_bonus(roll.skilled).unwrap_or(0);
    let result = if roll.skilled != roll.unskilled {
        let unskilled_bonus = get_torg_eternity_bonus(roll.unskilled).unwrap_or(0);
        format!("d20ロール（通常） ＞ 1d20[{}] ＞ {skilled_bonus:+}[{}]（技能有） / {unskilled_bonus:+}[{}]（技能無）", roll.dice_str, roll.skilled, roll.unskilled)
    } else {
        format!(
            "d20ロール（通常） ＞ 1d20[{}] ＞ {skilled_bonus:+}[{}]",
            roll.dice_str, roll.skilled
        )
    };
    Ok(Some(result))
}

fn up_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^UP(\d*)$")
}

fn roll_up(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(captures) = up_pattern().captures(command) else {
        return Ok(None);
    };
    let modifier = captures[1].parse::<i64>().unwrap_or(0);
    let first = torg_eternity_dice(false, true, rng)?;
    if first.mishap {
        return Ok(Some(format!(
            "d20ロール（高揚） ＞ 1d20[{}] ＞ Mishap!　絶対失敗！",
            first.dice_str
        )));
    }
    let second = torg_eternity_dice(false, false, rng)?;
    let skilled = first.skilled + second.skilled;
    let unskilled = first.unskilled + second.unskilled;
    let skilled_bonus = get_torg_eternity_bonus(skilled).unwrap_or(0);
    let unskilled_bonus = get_torg_eternity_bonus(unskilled).unwrap_or(0);
    let dice = format!("1d20[{}] + 1d20[{}]", first.dice_str, second.dice_str);
    let result = if modifier <= 0 {
        if skilled != unskilled {
            format!("d20ロール（高揚） ＞ {dice} ＞ {skilled_bonus:+}[{skilled}]（技能有） / {unskilled_bonus:+}[{unskilled}]（技能無）")
        } else {
            format!("d20ロール（高揚） ＞ {dice} ＞ {skilled_bonus:+}[{skilled}]")
        }
    } else if skilled != unskilled {
        format!("d20ロール（高揚） ＞ {dice} + {modifier} ＞ {skilled_bonus:+}[{skilled}]+{modifier}（技能有） / {unskilled_bonus:+}[{unskilled}]+{modifier}（技能無） ＞ {:+}（技能有） / {:+}（技能無）", skilled_bonus + modifier, unskilled_bonus + modifier)
    } else {
        format!("d20ロール（高揚） ＞ {dice} + {modifier} ＞ {skilled_bonus:+}[{skilled}]+{modifier} ＞ {:+}", skilled_bonus + modifier)
    };
    Ok(Some(result))
}

fn possibility_pattern(cosm: bool) -> &'static Regex {
    static POS: OnceLock<Regex> = OnceLock::new();
    static CPOS: OnceLock<Regex> = OnceLock::new();
    if cosm {
        regex(&CPOS, r"(?i)^CPOS((\d+)(\+\d+)?)$")
    } else {
        regex(&POS, r"(?i)^POS((\d+)(\+\d+)?)$")
    }
}

fn roll_possibility(
    command: &str,
    cosm: bool,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(captures) = possibility_pattern(cosm).captures(command) else {
        return Ok(None);
    };
    let modifier = eval_number(&captures[1])?;
    let roll = torg_eternity_dice(!cosm, false, rng)?;
    let skilled = roll.skilled + modifier;
    let unskilled = roll.unskilled + modifier;
    let skilled_bonus = get_torg_eternity_bonus(skilled).unwrap_or(0);
    let result = if skilled != unskilled {
        let unskilled_bonus = get_torg_eternity_bonus(unskilled).unwrap_or(0);
        format!("d20ロール（ポシビリティ） ＞ {modifier}+1d20[{}] ＞ {skilled_bonus:+}[{skilled}]（技能有） / {unskilled_bonus:+}[{unskilled}]（技能無）", roll.dice_str)
    } else {
        format!(
            "d20ロール（ポシビリティ） ＞ {modifier}+1d20[{}] ＞ {skilled_bonus:+}[{skilled}]",
            roll.dice_str
        )
    };
    Ok(Some(result))
}

fn bonus_damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^(\d+)BD(([+-]\d+)*)$")
}

fn roll_bonus_damage(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(captures) = bonus_damage_pattern().captures(command) else {
        return Ok(None);
    };
    let number = captures[1].parse::<i64>().unwrap_or(i64::MAX);
    let (modifier, modifier_text) = modifier(&captures[2])?;
    if number <= 0 {
        return Ok(Some("エラーです。xBD (x≧1) として下さい".to_owned()));
    }
    let (value, dice) = damage_bonus_dice(number, rng)?;
    Ok(Some(format!("ボーナスダメージロール({number}BD{modifier_text}) ＞ {value}[{dice}]{modifier_text} ＞ {}ダメージ", value + modifier)))
}

fn success_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^(RT|Result)(-*\d+([+-]\d+)*)$")
}

fn success_level(command: &str) -> Result<Option<String>, EvalError> {
    let Some(captures) = success_pattern().captures(command) else {
        return Ok(None);
    };
    let value = eval_number(&captures[2])?;
    let result = if value < 0 {
        "Failure."
    } else {
        table_result(value, SUCCESS_TABLE).unwrap_or("")
    };
    Ok(Some(format!("成功レベル表[{value}] ＞ {result}")))
}

fn damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^(DT|Damage)(-*\d+([+-]\d+)*)$")
}

fn damage_result(command: &str) -> Result<Option<String>, EvalError> {
    let Some(captures) = damage_pattern().captures(command) else {
        return Ok(None);
    };
    let value = eval_number(&captures[2])?;
    Ok(Some(format!(
        "ダメージ結果表[{value}] ＞ {}",
        table_result(value, DAMAGE_TABLE).unwrap_or("")
    )))
}

fn roll_bonus_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    regex(&RE, r"(?i)^(BT|Bonus|Total)(\d+)(([+-]\d+)*)$")
}

fn roll_bonus(command: &str) -> Result<Option<String>, EvalError> {
    let Some(captures) = roll_bonus_pattern().captures(command) else {
        return Ok(None);
    };
    let value = captures[2].parse::<i64>().unwrap_or(i64::MAX);
    let bonus = get_torg_eternity_bonus(value).unwrap_or(0);
    let (modifier, modifier_text) = modifier(&captures[3])?;
    let result = if value <= 1 {
        format!("ロールボーナス表[{value}] ＞ Mishap!!")
    } else if modifier_text.is_empty() {
        format!("ロールボーナス表[{value}] ＞ {bonus}")
    } else {
        format!(
            "ロールボーナス表[{value}]{modifier_text} ＞ {bonus}[{value}]{modifier_text} ＞ {}",
            bonus + modifier
        )
    };
    Ok(Some(result))
}

fn modifier(source: &str) -> Result<(i64, String), EvalError> {
    if source.is_empty() {
        Ok((0, String::new()))
    } else {
        let value = eval_number(source)?;
        Ok((value, format!("{value:+}")))
    }
}

struct TorgRoll {
    skilled: i64,
    unskilled: i64,
    dice_str: String,
    mishap: bool,
}

fn torg_eternity_dice(
    mut check_pos: bool,
    mut check_mishap: bool,
    rng: &mut Randomizer,
) -> Result<TorgRoll, EvalError> {
    let mut skilled_critical = true;
    let mut critical = true;
    let mut skilled = 0;
    let mut unskilled = 0;
    let mut mishap = false;
    let mut dice = Vec::new();
    while skilled_critical {
        let rolled = rng.roll_once(20)?;
        let mut value = rolled;
        if check_pos && rolled < 10 {
            dice.push(format!("{rolled}→10"));
            value = 10;
            skilled_critical = false;
        } else {
            dice.push(rolled.to_string());
        }
        skilled += value;
        if critical {
            unskilled += value;
        }
        if value == 20 {
            critical = false;
        } else if value != 10 {
            skilled_critical = false;
            critical = false;
            if check_mishap && value == 1 {
                mishap = true;
            }
        }
        check_pos = false;
        check_mishap = false;
    }
    Ok(TorgRoll {
        skilled,
        unskilled,
        dice_str: dice.join(","),
        mishap,
    })
}

fn damage_bonus_dice(mut number: i64, rng: &mut Randomizer) -> Result<(i64, String), EvalError> {
    let mut value = 0;
    let mut dice = Vec::new();
    while number > 0 {
        let rolled = rng.roll_once(6)?;
        if rolled == 6 {
            value += 5;
            dice.push("5∞".to_owned());
            number += 1;
        } else {
            value += rolled;
            dice.push(rolled.to_string());
        }
        number -= 1;
    }
    Ok((value, dice.join(",")))
}

fn table_result(value: i64, table: &[(i64, &'static str)]) -> Option<&'static str> {
    table
        .iter()
        .rev()
        .find(|(threshold, _)| *threshold <= value)
        .map(|(_, result)| *result)
}

fn get_torg_eternity_bonus(value: i64) -> Option<i64> {
    let mut bonus = BONUS_TABLE
        .iter()
        .rev()
        .find(|(threshold, _)| *threshold <= value)
        .map(|(_, result)| *result)?;
    if value > 20 {
        bonus += (value - 21).div_euclid(5) + 1;
    }
    Some(bonus)
}

static SUCCESS_TABLE: &[(i64, &str)] = &[
    (0, "Success - Standard."),
    (5, "Success - Good!"),
    (10, "Success - Outstanding!!"),
];
static DAMAGE_TABLE: &[(i64, &str)] = &[
    (-50, "ノーダメージ"),
    (-5, "1ショック"),
    (0, "2ショック"),
    (5, "1レベル負傷 + 2ショック"),
    (10, "2レベル負傷 + 4ショック"),
    (15, "3レベル負傷 + 6ショック"),
    (20, "4レベル負傷 + 8ショック"),
    (25, "5レベル負傷 + 10ショック"),
    (30, "6レベル負傷 + 12ショック"),
    (35, "7レベル負傷 + 14ショック"),
    (40, "8レベル負傷 + 16ショック"),
    (45, "9レベル負傷 + 18ショック"),
    (50, "10レベル負傷 + 20ショック"),
];
static BONUS_TABLE: &[(i64, i64)] = &[
    (1, -10),
    (2, -8),
    (3, -6),
    (5, -4),
    (7, -2),
    (9, -1),
    (11, 0),
    (13, 1),
    (15, 2),
    (16, 3),
    (17, 4),
    (18, 5),
    (19, 6),
    (20, 7),
];

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::{Path, PathBuf};

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/TorgEternity.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/TorgEternity.toml not found");
            return;
        };
        let data = TestDataFile::load(&path).expect("TorgEternity.toml must parse");
        assert_eq!(
            data.tests.len(),
            105,
            "case count in test/data/TorgEternity.toml"
        );
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "TorgEternity");
            let mut reasons = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);
            match eval_command(&GameSystemId::new("TorgEternity"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!("eval returned nil, expected {:?}", tc.output));
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
                    "FAIL TorgEternity:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{}/{} TorgEternity cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
