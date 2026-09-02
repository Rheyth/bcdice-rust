//! `lib/bcdice/game_system/NightmareHunterDeep.rb` の手書き移植。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NightmareHunterDeep;

impl GameSystem for NightmareHunterDeep {
    fn id(&self) -> &'static str {
        "NightmareHunterDeep"
    }
    fn name(&self) -> &'static str {
        "ナイトメアハンター=ディープ"
    }
    fn sort_key(&self) -> &'static str {
        "ないとめあはんたあていいふ"
    }
    fn help_message(&self) -> &'static str {
        r"判定（xD6+y>=a, xD6+y, xD6)
  出目6の個数をカウントして、その4倍を合計値に加算します。
  また、宿命を獲得したか表示します。

  Lv目標値 (xD6+y>=LVn, xD6+y>=NLn)
    レベルで目標値を指定することができます。
    LVn -> n*5+1, NLn -> n*5+5 に変換されます。
  目標値'?' (xD6+y>=?)
    目標値を '?' にすると何Lv成功か、何NL成功かを表示します。

※判定コマンドは xD6 から始まる必要があります。また xD6 が複数あると反応しません。
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+D6"]
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
        static LV: OnceLock<Regex> = OnceLock::new();
        static NL: OnceLock<Regex> = OnceLock::new();
        let command = LV
            .get_or_init(|| Regex::new(r"(?i)Lv(\d+)").unwrap())
            .replace(command, |c: &Captures| {
                (to_i(&c[1]).saturating_mul(5) - 1).to_string()
            });
        let command = NL
            .get_or_init(|| Regex::new(r"(?i)NL(\d+)").unwrap())
            .replace(&command, |c: &Captures| {
                (to_i(&c[1]).saturating_mul(5) + 5).to_string()
            });
        let Some(cmd) = Parser::new(&[r"\d+D6"], RoundType::Floor)
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
            .enable_question_target()
            .parse(&command)
        else {
            return Ok(None);
        };

        let times = to_i(cmd.command.split('D').next().unwrap_or("0"));
        let mut dice = rng.roll_barabara(times, 6)?;
        dice.sort_unstable();
        let dice_total: i64 = dice.iter().sum();
        let count6 = dice.iter().filter(|d| **d == 6).count() as i64;
        let base_total = dice_total + cmd.modify_number.clone();
        let total = base_total.clone() + count6 * 4;
        let mut sequence = vec![format!("({})", cmd.to_s(SuffixPosition::AfterCommand))];
        if dice.len() > 1 || cmd.modify_number != I::ZERO {
            sequence.push(format!(
                "{}[{}]{}",
                dice_total,
                dice.iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                modifier(&cmd.modify_number)
            ));
        }
        if count6 > 0 {
            sequence.push(format!("{base_total}+{count6}*4"));
        }
        sequence.push(total.to_string());
        if cmd.cmp_op == Some(CmpOp::Ge) {
            let result = if cmd.question_target {
                let success_lv =
                    crate::arithmetic::floor_div(total.clone() + I::from(1), I::from(5));
                let success_nl = crate::arithmetic::floor_div(total - I::from(5), I::from(5));
                if success_lv > I::ZERO {
                    format!("Lv{success_lv}/NL{success_nl}成功")
                } else {
                    "失敗".to_owned()
                }
            } else if total >= cmd.target_number.clone().unwrap_or(crate::Int::from(0)) {
                "成功".to_owned()
            } else {
                "失敗".to_owned()
            };
            sequence.push(result);
        }
        if dice.contains(&1) {
            sequence.push("宿命獲得".to_owned());
        }
        Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
    }
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(value: &str) -> i64 {
    str_helpers::to_i_max(value)
}

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::Path;
    #[test]
    fn all_toml_cases_pass() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test/data/NightmareHunterDeep.toml");
        if !path.exists() {
            return;
        }
        let data = TestDataFile::load(&path).unwrap();
        assert_eq!(
            data.tests.len(),
            51,
            "case count in test/data/NightmareHunterDeep.toml"
        );
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "NightmareHunterDeep");
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            let result = eval_command(
                &GameSystemId::new("NightmareHunterDeep"),
                &tc.input,
                &mut src,
            )
            .unwrap();
            if tc.expects_nil() {
                assert!(result.is_none());
            } else {
                let result = result.unwrap_or_else(|| panic!("case {} {}: nil", i + 1, tc.input));
                assert_eq!(result.text, tc.output, "case {} {} text", i + 1, tc.input);
                assert_eq!(
                    (
                        result.secret,
                        result.success,
                        result.failure,
                        result.critical,
                        result.fumble
                    ),
                    (tc.secret, tc.success, tc.failure, tc.critical, tc.fumble),
                    "case {} {} flags",
                    i + 1,
                    tc.input
                );
            }
            assert!(
                src.is_empty(),
                "case {} {}: {} rands remain",
                i + 1,
                tc.input,
                src.remaining()
            );
        }
    }
}
