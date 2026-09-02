//! P4で手書き移植した `lib/bcdice/game_system/WorldOfDarkness.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `WorldOfDarkness#eval_game_system_specific_command`（`xST[SAB]?n+y` 判定）
//! - `#roll_wod`（出目10と1、難易度以上の成功をカウント）

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::WorldOfDarkness`（ID: `WorldOfDarkness`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldOfDarkness;

impl GameSystem for WorldOfDarkness {
    fn id(&self) -> &'static str {
        "WorldOfDarkness"
    }

    fn name(&self) -> &'static str {
        "ワールド・オブ・ダークネス"
    }

    fn sort_key(&self) -> &'static str {
        "わあるとおふたあくねす"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(xSTn+y or xSTSn+y or xSTAn+y)
　(ダイス個数)ST(難易度)+(自動成功)
　(ダイス個数)STS(難易度)+(自動成功) ※出目10で振り足し、振り足し分の出目1で打ち消されない
　(ダイス個数)STB(難易度)+(自動成功) ※出目10で振り足し、振り足し分の出目1で打ち消される
　(ダイス個数)STA(難易度)+(自動成功) ※出目10は2成功 [20thルール]

　難易度=省略時6
　自動成功=省略時0、出目1で打ち消されない自動成功を指定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+ST"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `WorldOfDarkness#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: md = command.match(/\A(\d+)(ST[SAB]?)(\d+)?([+-]\d+)?/)
        // （レジスタプレフィックス `\d+ST` を通過済みなのでマッチしないケースはない）
        let re = Regex::new(r"\A(\d+)(ST[SAB]?)(\d+)?([+-]\d+)?").expect("valid regex");
        let md = re.captures(command).expect("prefixes guarantee match");

        let dice_pool: i64 = md[1].parse().unwrap_or(0);
        let enabled_reroll = &md[2] == "STS";
        let enabled_20th = &md[2] == "STA";
        let enabled_reroll_with_botch = &md[2] == "STB";

        // Ruby: difficulty = md[3].to_i if md[3]（nil なら既定値 6 のまま）
        let mut difficulty: i64 = md
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(6);
        // Ruby: auto_success = md[4].to_i if md[4]
        // 桁あふれは Ruby だと Bignum になるので i64 に飽和させる
        let auto_success: i64 = md.get(4).map(|m| to_i(m.as_str())).unwrap_or(0);

        if difficulty < 2 {
            difficulty = 6;
        }

        let mut sequence = vec![format!(
            "DicePool={dice_pool}, Difficulty={difficulty}, AutomaticSuccess={auto_success}"
        )];

        // 出力では Difficulty=11..12 もあり得る
        if difficulty > 10 {
            difficulty = 10;
        }

        let mut total_success: i64 = 0;
        let mut total_botch: i64 = 0;
        let mut once_success = false;

        let (dice, ten_success, success, botch) = roll_wod(dice_pool, difficulty, rng)?;
        sequence.push(dice_text::join_dice(&dice));
        total_success += success;
        total_botch += botch;

        // 成功がひとつでもあったか覚えておく
        if success > 0 || ten_success > 0 {
            once_success = true;
        }

        if enabled_20th {
            // 20周年記念版なら10の目は2成功扱い
            total_success += ten_success * 2;
        } else {
            // Revised Editionでは10は1成功と数える
            total_success += ten_success;

            // 振り足し判定ありなら10が出ただけ振り足しを行う
            if enabled_reroll || enabled_reroll_with_botch {
                let mut ten_success = ten_success;
                while ten_success > 0 {
                    let (dice, next_ten, success, botch) = roll_wod(ten_success, difficulty, rng)?;
                    sequence.push(dice_text::join_dice(&dice));
                    total_success += success + next_ten;
                    ten_success = next_ten;

                    if enabled_reroll_with_botch {
                        // 振り足しでのボッチありなら出目1をカウントする
                        total_botch += botch;
                    }
                }
            }
        }

        total_success -= total_success.min(total_botch);

        total_success += auto_success; // 意志力による自動成功は打ち消されない

        let text = sequence.join(" ＞ ");
        let result = if total_success > 0 {
            sequence.push(format!("成功数{total_success}"));
            EvalResult::success(sequence.join(" ＞ "))
        } else if total_botch > 0 && !once_success {
            // ボッチが存在し、かつ成功がひとつもない場合のみ大失敗
            sequence.push("大失敗".to_owned());
            EvalResult::fumble(sequence.join(" ＞ "))
        } else {
            sequence.push("失敗".to_owned());
            EvalResult::failure(sequence.join(" ＞ "))
        };
        let _ = text;

        Ok(Some(SpecificCommandOutput::result(result)))
    }
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `WorldOfDarkness#roll_wod`。
///
/// 出目10と1、難易度以上が出た成功の目をカウントする。
/// それぞれの解釈はバージョンによって異なるため、呼び出し元で行う。
fn roll_wod(
    dice_pool: i64,
    difficulty: i64,
    rng: &mut Randomizer,
) -> Result<(Vec<i64>, i64, i64, i64), EvalError> {
    let mut dice = vec![0; dice_pool.max(0) as usize];
    for d in dice.iter_mut() {
        *d = rng.roll_once(10)?;
    }

    dice.sort_unstable();

    let mut success = 0;
    let mut botch = 0;
    let mut ten_success = 0;

    for &d in &dice {
        match d {
            10 => ten_success += 1,
            // Ruby: when difficulty...10（difficulty <= d < 10）
            // difficulty は 2..10 に補正済みなので d == 1 と重複しない
            d if (difficulty..10).contains(&d) => success += 1,
            1 => botch += 1,
            _ => {}
        }
    }

    Ok((dice, ten_success, success, botch))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WorldOfDarkness",
            "WorldOfDarkness.toml",
            74,
        );
    }
}
