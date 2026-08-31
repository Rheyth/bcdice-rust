//! `lib/bcdice/game_system/FinalFantasyXIV.rb` の移植。

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

pub(crate) struct SystemTables {
    pub(crate) critical: &'static str,
    pub(crate) direct_hit: &'static str,
    pub(crate) normal_hit: &'static str,
    pub(crate) success: &'static str,
    pub(crate) failure: &'static str,
}

pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = check_roll(tables, command, "AB", true, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(check_roll(tables, command, "DC", false, rng)?.map(SpecificCommandOutput::result))
}

fn check_roll(
    tables: &SystemTables,
    command: &str,
    notation: &str,
    ability: bool,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&[notation], RoundType::Floor)
        .enable_prefix_number()
        .restrict_cmp_op_to(&[Some(CmpOp::Ge), None]);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };
    let times = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(1);
    let mut all_dice = rng.roll_barabara(times, 20)?;
    all_dice.sort_unstable();
    let Some(die) = all_dice.last().copied() else {
        return Ok(None);
    };
    let total = die + parsed.modify_number.clone();

    let mut result = if ability && die == 20 {
        EvalResult::critical(tables.critical)
    } else if parsed.cmp_op.is_none() {
        EvalResult::new()
    } else if total >= parsed.target_number.clone().unwrap_or(crate::Int::from(0)) {
        EvalResult::success(if ability {
            tables.direct_hit
        } else {
            tables.success
        })
    } else {
        EvalResult::failure(if ability {
            tables.normal_hit
        } else {
            tables.failure
        })
    };

    let modify_str = modifier(&parsed.modify_number);
    let mut sequence = vec![format!(
        "({})",
        parsed.to_s(SuffixPosition::AfterModifyNumber)
    )];
    if times > 1 {
        sequence.push(format!("[{}]", join(&all_dice)));
    }
    sequence.push(format!("{die}[{die}]{modify_str}"));
    sequence.push(total.to_string());
    if !result.text.is_empty() {
        sequence.push(result.text.clone());
    }
    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    critical: "クリティカル",
    direct_hit: "ダイレクトヒット",
    normal_hit: "基本効果のみ",
    success: "成功",
    failure: "失敗",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalFantasyXIV;

impl GameSystem for FinalFantasyXIV {
    fn id(&self) -> &'static str {
        "FinalFantasyXIV"
    }

    fn name(&self) -> &'static str {
        "FINAL FANTSY XIV TTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ふあいなるふあんたしい14TTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・アビリティ判定 nAB+m>=x
  d20のアビリティ判定を行う。ダイス数が指定された場合、大きい出目1個を採用する。
  n: ダイス数（省略時 1）
  m: 修正値（省略可）
  x: 目標値（省略可）
  基本効果のみ、ダイレクトヒット、クリティカルを自動判定。
  例）AB, AB+5, AB+5>=14, 2AB+5>=14
・行為判定 nDC+m>=x
  アビリティ判定と同様。
  失敗、成功を自動判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*AB", r"\d*DC"]
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
    use std::path::Path;

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    #[test]
    fn all_toml_cases_pass() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data/FinalFantasyXIV.toml");
        if !path.exists() {
            return;
        }
        let data = TestDataFile::load(&path).expect("FinalFantasyXIV.toml must parse");
        assert_eq!(data.tests.len(), 20);
        for (index, case) in data.tests.iter().enumerate() {
            let mut rng =
                SeededRandomizer::new(case.rands.iter().map(|rand| (rand.value, rand.sides)));
            let result = eval_command(&GameSystemId::new("FinalFantasyXIV"), &case.input, &mut rng)
                .unwrap_or_else(|error| panic!("case {}: {error}", index + 1))
                .unwrap_or_else(|| panic!("case {} returned nil", index + 1));
            assert_eq!(result.text, case.output, "case {}", index + 1);
            assert_eq!(
                (
                    result.secret,
                    result.success,
                    result.failure,
                    result.critical,
                    result.fumble
                ),
                (
                    case.secret,
                    case.success,
                    case.failure,
                    case.critical,
                    case.fumble
                ),
                "case {} flags",
                index + 1
            );
            assert!(rng.is_empty(), "case {} left random values", index + 1);
        }
    }
}
