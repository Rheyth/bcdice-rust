//! P4で手書き移植した `lib/bcdice/game_system/SwordWorld.rb`。
//!
//! rating parser は `test/data/SwordWorld*.toml` に現れる v1 形式だけを扱う。

use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// `i18n/ja_jp.yml` と `i18n/SwordWorld/ja_jp.yml` の文言。
pub(crate) struct SystemText {
    pub success: &'static str,
    pub failure: &'static str,
    pub critical: &'static str,
    pub fumble: &'static str,
    pub keynumber_exceeds: &'static str,
    pub infinite_critical: &'static str,
    pub round_suffix: &'static str,
}

static JA_JP: SystemText = SystemText {
    success: "成功",
    failure: "失敗",
    critical: "自動的成功",
    fumble: "自動的失敗",
    keynumber_exceeds: "キーナンバーは100までです",
    infinite_critical: "C値を3以上にしてください",
    round_suffix: "回転",
};

#[derive(Debug, Clone, Copy, Default)]
enum FirstAdjust {
    #[default]
    None,
    To(i64),
    Modify(i64),
}

#[derive(Debug, Clone, Copy)]
struct RatingCommand {
    rate: i64,
    critical: i64,
    modifier: i64,
    first_adjust: FirstAdjust,
    half: bool,
    modifier_after_half: i64,
}

impl RatingCommand {
    fn label(self) -> String {
        let mut out = format!("KeyNo.{}", self.rate);
        if self.critical < 13 {
            out.push_str(&format!("c[{}]", self.critical));
        }
        match self.first_adjust {
            FirstAdjust::Modify(n) if n != 0 => {
                out.push_str(&format!("m[{}]", format::modifier(&n.into())));
            }
            FirstAdjust::To(n) if n != 0 => out.push_str(&format!("m[{n}]")),
            _ => {}
        }
        out.push_str(&format::modifier(&self.modifier.into()));
        out
    }
}

fn take_integer(bytes: &[u8], pos: &mut usize) -> Option<i64> {
    let start = *pos;
    while bytes.get(*pos).is_some_and(u8::is_ascii_digit) {
        *pos += 1;
    }
    (*pos > start).then(|| {
        bytes[start..*pos].iter().fold(0_i64, |n, digit| {
            n.saturating_mul(10).saturating_add(i64::from(digit - b'0'))
        })
    })
}

fn take_signed_integer(bytes: &[u8], pos: &mut usize) -> Option<i64> {
    let sign = match bytes.get(*pos) {
        Some(b'+') => {
            *pos += 1;
            1
        }
        Some(b'-') => {
            *pos += 1;
            -1
        }
        _ => 1,
    };
    take_integer(bytes, pos).map(|n| n.saturating_mul(sign))
}

/// `rating_parser.y` のうち TOML に現れる v1 コマンドだけを読む。
fn parse_rating(command: &str) -> Option<RatingCommand> {
    let bytes = command.as_bytes();
    let mut pos = 0;
    let prefix_half = bytes.get(pos) == Some(&b'H');
    pos += usize::from(prefix_half);
    if bytes.get(pos) != Some(&b'K') {
        return None;
    }
    pos += 1;

    let rate = take_integer(bytes, &mut pos)?;
    let mut critical = None;
    let mut modifier = 0_i64;
    let mut first_adjust = FirstAdjust::None;
    let mut suffix_half = false;
    let mut modifier_after_half = 0;

    while let Some(token) = bytes.get(pos).copied() {
        match token {
            b'[' => {
                if critical.is_some() {
                    return None;
                }
                pos += 1;
                critical = Some(take_signed_integer(bytes, &mut pos)?);
                if bytes.get(pos) != Some(&b']') {
                    return None;
                }
                pos += 1;
            }
            b'@' => {
                if critical.is_some() {
                    return None;
                }
                pos += 1;
                critical = Some(take_signed_integer(bytes, &mut pos)?);
            }
            b'$' => {
                if !matches!(first_adjust, FirstAdjust::None) {
                    return None;
                }
                pos += 1;
                let modify = matches!(bytes.get(pos), Some(b'+') | Some(b'-'));
                let value = take_signed_integer(bytes, &mut pos)?;
                first_adjust = if modify {
                    FirstAdjust::Modify(value)
                } else {
                    FirstAdjust::To(value)
                };
            }
            b'H' => {
                if suffix_half {
                    return None;
                }
                suffix_half = true;
                pos += 1;
                if matches!(bytes.get(pos), Some(b'+') | Some(b'-')) {
                    modifier_after_half = take_signed_integer(bytes, &mut pos)?;
                }
            }
            b'+' | b'-' => {
                modifier = modifier.saturating_add(take_signed_integer(bytes, &mut pos)?);
            }
            _ => return None,
        }
    }

    let half = prefix_half || suffix_half;
    Some(RatingCommand {
        rate,
        critical: critical.unwrap_or(if half { 13 } else { 10 }).clamp(0, 13),
        modifier,
        first_adjust,
        half,
        modifier_after_half,
    })
}

pub(crate) fn check_result_2d6(
    text: &SystemText,
    total: crate::Int,
    dice_total: crate::Int,
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    let result = if dice_total >= I::from(12) {
        EvalResult::critical(text.critical)
    } else if dice_total <= I::from(2) {
        EvalResult::fumble(text.fumble)
    } else if cmp_op != CmpOp::Ge || target == Target::Question {
        return None;
    } else if let Target::Number(target) = target {
        if total >= target {
            EvalResult::success(text.success)
        } else {
            EvalResult::failure(text.failure)
        }
    } else {
        return None;
    };
    Some(CheckOutcome::Result(Box::new(result)))
}

pub(crate) fn eval_specific_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(command) = parse_rating(command) else {
        return Ok(None);
    };
    if command.rate > 100 {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            text.keynumber_exceeds,
        ))));
    }
    if command.critical < 3 {
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            text.infinite_critical,
        ))));
    }

    let mut dice_texts = Vec::new();
    let mut dice_totals = Vec::new();
    let mut rate_results = Vec::new();
    let mut rating_total = 0_i64;
    let mut dice_only_total = 0_i64;
    let mut round = 0_i64;

    loop {
        let values = rng.roll_barabara(2, 6)?;
        let dice_text = values
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let raw = values.iter().sum::<i64>();
        let (raw, dice) = match (round, command.first_adjust) {
            (0, FirstAdjust::To(value)) if value != 0 => (value, value),
            (0, FirstAdjust::Modify(value)) if value != 0 => (raw, raw + value),
            _ => (raw, raw),
        };

        dice_texts.push(dice_text);
        if raw <= 2 {
            dice_totals.push(raw.to_string());
            rate_results.push("**".to_string());
            round += 1;
            break;
        }

        let dice = dice.clamp(2, 12);
        let rate = rating_value(command.rate, dice);
        rating_total = rating_total.saturating_add(rate);
        dice_only_total = dice_only_total.saturating_add(dice);
        dice_totals.push(dice.to_string());
        rate_results.push(rate.to_string());
        round += 1;

        if dice < command.critical {
            break;
        }
    }

    let mut sequence = vec![format!(
        "2D:[{}]={}",
        dice_texts.join(" "),
        dice_totals.join(",")
    )];
    let mut result = EvalResult::new();

    if dice_only_total <= 2 {
        sequence.push(rate_results.join(","));
        sequence.push(text.fumble.to_string());
        result.fumble = true;
    } else {
        if rate_results.len() > 1 || command.modifier != 0 {
            let mut calculation = format!(
                "{}{}",
                rate_results.join(","),
                format::modifier(&command.modifier.into())
            );
            if command.half {
                calculation = format!("({calculation})/2");
                calculation.push_str(&format::modifier(&command.modifier_after_half.into()));
            }
            sequence.push(calculation);
        } else if command.half {
            sequence.push(format!(
                "{}/2{}",
                rate_results[0],
                format::modifier(&command.modifier_after_half.into())
            ));
        }

        if round > 1 {
            sequence.push(format!("{}{}", round - 1, text.round_suffix));
        }

        let mut total = rating_total.saturating_add(command.modifier);
        if command.half {
            total = (total + 1)
                .div_euclid(2)
                .saturating_add(command.modifier_after_half);
        }
        sequence.push(total.to_string());
        result.critical = round > 1;
    }

    result.text = format!("{} ＞ {}", command.label(), sequence.join(" ＞ "));
    Ok(Some(SpecificCommandOutput::result(result)))
}

fn rating_value(key: i64, dice: i64) -> i64 {
    RATING_TABLE[key as usize]
        .split(',')
        .nth((dice - 2) as usize)
        .expect("complete SwordWorld rating row")
        .parse()
        .expect("numeric SwordWorld rating value")
}

/// Ruby `getSW2_0_RatingTable`。先頭の `*` は出目2、以降は出目3〜12。
static RATING_TABLE: [&str; 101] = [
    "*,0,0,0,1,2,2,3,3,4,4",
    "*,0,0,0,1,2,3,3,3,4,4",
    "*,0,0,0,1,2,3,4,4,4,4",
    "*,0,0,1,1,2,3,4,4,4,5",
    "*,0,0,1,2,2,3,4,4,5,5",
    "*,0,1,1,2,2,3,4,5,5,5",
    "*,0,1,1,2,3,3,4,5,5,5",
    "*,0,1,1,2,3,4,4,5,5,6",
    "*,0,1,2,2,3,4,4,5,6,6",
    "*,0,1,2,3,3,4,4,5,6,7",
    "*,1,1,2,3,3,4,5,5,6,7",
    "*,1,2,2,3,3,4,5,6,6,7",
    "*,1,2,2,3,4,4,5,6,6,7",
    "*,1,2,3,3,4,4,5,6,7,7",
    "*,1,2,3,4,4,4,5,6,7,8",
    "*,1,2,3,4,4,5,5,6,7,8",
    "*,1,2,3,4,4,5,6,7,7,8",
    "*,1,2,3,4,5,5,6,7,7,8",
    "*,1,2,3,4,5,6,6,7,7,8",
    "*,1,2,3,4,5,6,7,7,8,9",
    "*,1,2,3,4,5,6,7,8,9,10",
    "*,1,2,3,4,6,6,7,8,9,10",
    "*,1,2,3,5,6,6,7,8,9,10",
    "*,2,2,3,5,6,7,7,8,9,10",
    "*,2,3,4,5,6,7,7,8,9,10",
    "*,2,3,4,5,6,7,8,8,9,10",
    "*,2,3,4,5,6,8,8,9,9,10",
    "*,2,3,4,6,6,8,8,9,9,10",
    "*,2,3,4,6,6,8,9,9,10,10",
    "*,2,3,4,6,7,8,9,9,10,10",
    "*,2,4,4,6,7,8,9,10,10,10",
    "*,2,4,5,6,7,8,9,10,10,11",
    "*,3,4,5,6,7,8,10,10,10,11",
    "*,3,4,5,6,8,8,10,10,10,11",
    "*,3,4,5,6,8,9,10,10,11,11",
    "*,3,4,5,7,8,9,10,10,11,12",
    "*,3,5,5,7,8,9,10,11,11,12",
    "*,3,5,6,7,8,9,10,11,12,12",
    "*,3,5,6,7,8,10,10,11,12,13",
    "*,4,5,6,7,8,10,11,11,12,13",
    "*,4,5,6,7,9,10,11,11,12,13",
    "*,4,6,6,7,9,10,11,12,12,13",
    "*,4,6,7,7,9,10,11,12,13,13",
    "*,4,6,7,8,9,10,11,12,13,14",
    "*,4,6,7,8,10,10,11,12,13,14",
    "*,4,6,7,9,10,10,11,12,13,14",
    "*,4,6,7,9,10,10,12,13,13,14",
    "*,4,6,7,9,10,11,12,13,13,15",
    "*,4,6,7,9,10,12,12,13,13,15",
    "*,4,6,7,10,10,12,12,13,14,15",
    "*,4,6,8,10,10,12,12,13,15,15",
    "*,5,7,8,10,10,12,12,13,15,15",
    "*,5,7,8,10,11,12,12,13,15,15",
    "*,5,7,9,10,11,12,12,14,15,15",
    "*,5,7,9,10,11,12,13,14,15,16",
    "*,5,7,10,10,11,12,13,14,16,16",
    "*,5,8,10,10,11,12,13,15,16,16",
    "*,5,8,10,11,11,12,13,15,16,17",
    "*,5,8,10,11,12,12,13,15,16,17",
    "*,5,9,10,11,12,12,14,15,16,17",
    "*,5,9,10,11,12,13,14,15,16,18",
    "*,5,9,10,11,12,13,14,16,17,18",
    "*,5,9,10,11,13,13,14,16,17,18",
    "*,5,9,10,11,13,13,15,17,17,18",
    "*,5,9,10,11,13,14,15,17,17,18",
    "*,5,9,10,12,13,14,15,17,18,18",
    "*,5,9,10,12,13,15,15,17,18,19",
    "*,5,9,10,12,13,15,16,17,19,19",
    "*,5,9,10,12,14,15,16,17,19,19",
    "*,5,9,10,12,14,16,16,17,19,19",
    "*,5,9,10,12,14,16,17,18,19,19",
    "*,5,9,10,13,14,16,17,18,19,20",
    "*,5,9,10,13,15,16,17,18,19,20",
    "*,5,9,10,13,15,16,17,19,20,21",
    "*,6,9,10,13,15,16,18,19,20,21",
    "*,6,9,10,13,16,16,18,19,20,21",
    "*,6,9,10,13,16,17,18,19,20,21",
    "*,6,9,10,13,16,17,18,20,21,22",
    "*,6,9,10,13,16,17,19,20,22,23",
    "*,6,9,10,13,16,18,19,20,22,23",
    "*,6,9,10,13,16,18,20,21,22,23",
    "*,6,9,10,13,17,18,20,21,22,23",
    "*,6,9,10,14,17,18,20,21,22,24",
    "*,6,9,11,14,17,18,20,21,23,24",
    "*,6,9,11,14,17,19,20,21,23,24",
    "*,6,9,11,14,17,19,21,22,23,24",
    "*,7,10,11,14,17,19,21,22,23,25",
    "*,7,10,12,14,17,19,21,22,24,25",
    "*,7,10,12,14,18,19,21,22,24,25",
    "*,7,10,12,15,18,19,21,22,24,26",
    "*,7,10,12,15,18,19,21,23,25,26",
    "*,7,11,13,15,18,19,21,23,25,26",
    "*,7,11,13,15,18,20,21,23,25,27",
    "*,8,11,13,15,18,20,22,23,25,27",
    "*,8,11,13,16,18,20,22,23,25,28",
    "*,8,11,14,16,18,20,22,23,26,28",
    "*,8,11,14,16,19,20,22,23,26,28",
    "*,8,12,14,16,19,20,22,24,26,28",
    "*,8,12,15,16,19,20,22,24,27,28",
    "*,8,12,15,17,19,20,22,24,27,29",
    "*,8,12,15,18,19,20,22,24,27,30",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwordWorld;

impl GameSystem for SwordWorld {
    fn id(&self) -> &'static str {
        "SwordWorld"
    }
    fn name(&self) -> &'static str {
        "ソード・ワールドRPG"
    }
    fn sort_key(&self) -> &'static str {
        "そおとわあると"
    }
    fn help_message(&self) -> &'static str {
        r"・SW　レーティング表　(Kx[c]+m$f) (x:キー, c:クリティカル値, m:ボーナス, f:出目修正)
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["H?K"]
    }
    crate::impl_prefixes_pattern!();

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&JA_JP, total, crate::Int::from(dice_total), cmp_op, target)
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_JP, command, rng)
    }
}

#[cfg(test)]
pub(crate) fn assert_toml_cases(system: &str, file: &str) {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::Path;

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test/data")
        .join(file);
    if !path.exists() {
        eprintln!("skip: {} not found", path.display());
        return;
    }
    let data = TestDataFile::load(&path).unwrap_or_else(|e| panic!("{file} must parse: {e}"));
    assert_eq!(data.tests.len(), 230, "case count in {file}");
    let mut failures = Vec::new();
    for (index, tc) in data.tests.iter().enumerate() {
        assert_eq!(tc.game_system, system, "unexpected game system in {file}");
        let mut reasons = Vec::new();
        let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
        match eval_command(&GameSystemId::new(system), &tc.input, &mut src) {
            Err(error) => reasons.push(format!("eval error: {error}")),
            Ok(None) if !tc.expects_nil() => reasons.push("eval returned nil".to_string()),
            Ok(None) => {}
            Ok(Some(result)) => {
                if tc.expects_nil() {
                    reasons.push(format!("expected nil, got {:?}", result.text));
                } else if result.text != tc.output {
                    reasons.push(format!(
                        "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                        tc.output, result.text
                    ));
                }
                for (name, expected, actual) in [
                    ("secret", tc.secret, result.secret),
                    ("success", tc.success, result.success),
                    ("failure", tc.failure, result.failure),
                    ("critical", tc.critical, result.critical),
                    ("fumble", tc.fumble, result.fumble),
                ] {
                    if expected != actual {
                        reasons.push(format!(
                            "{name} flag mismatch: expected {expected}, actual {actual}"
                        ));
                    }
                }
            }
        }
        if !src.is_empty() {
            reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
        }
        if !reasons.is_empty() {
            failures.push(format!(
                "FAIL {system}:{}:{}\n  - {}",
                index + 1,
                tc.input,
                reasons.join("\n  - ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{} {system} cases failed:\n{}",
        failures.len(),
        data.tests.len(),
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        super::assert_toml_cases("SwordWorld", "SwordWorld.toml");
    }
}
