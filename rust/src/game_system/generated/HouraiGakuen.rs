//! P4で手書き移植した `lib/bcdice/game_system/HouraiGakuen.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `HouraiGakuen#eval_game_system_specific_command`
//!   （`ROL` / `MED` / `RES` / `INY` / `HTK` / `GOG`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

const CRITICAL: &str = "大成功";
const SUCCESS: &str = "成功";
const FAILURE: &str = "失敗";
const FUMBLE: &str = "大失敗";

/// Ruby `BCDice::GameSystem::HouraiGakuen`（ID: `HouraiGakuen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HouraiGakuen;

impl GameSystem for HouraiGakuen {
    fn id(&self) -> &'static str {
        "HouraiGakuen"
    }

    fn name(&self) -> &'static str {
        "蓬莱学園の冒険!!"
    }

    fn sort_key(&self) -> &'static str {
        "ほうらいかくえんのほうけん"
    }

    fn help_message(&self) -> &'static str {
        r"・基本ロール：ROL(x+n)
  ROLL(自分の能力値 + 簡単値 + 応石 or 蓬莱パワー)と記述します。3D6をロールし、成功したかどうかを表示します。
  例）ROL(4+6)
・対人判定：MED(x,y)
  自分の能力値 x と 相手の能力値 y でロールを行い、成功したかどうかを表示します。
  例）MED(5,2)
・対抗判定：RES(x,y)
  自分の能力値 x と 相手の能力値 y で相互にロールし、どちらが成功したかを表示します。両者とも成功 or 失敗の場合は引き分けとなります。
  例）RES(6,4)
・陰陽コマンド INY
  例）Hourai : 陽（奇数の方が多い）
・五行コマンド：GOG
  例）Hourai : 五行表(3) → 五行【土】
・八徳コマンド：HTK
  例）Hourai : 仁義八徳は、【義】(奇数、奇数、偶数)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ROL", "MED", "RES", "INY", "HTK", "GOG"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `HouraiGakuen#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

fn rol_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)rol([-\d]+)").expect("valid regex"))
}

fn med_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)med\((\d+),(\d+)\)").expect("valid regex"))
}

fn res_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)res\((\d+),(\d+)\)").expect("valid regex"))
}

fn gog_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^GOG$").expect("valid regex"))
}

fn starts_ci(command: &str, prefix: &str) -> bool {
    command.len() >= prefix.len() && command[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Ruby `HouraiGakuen#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if starts_ci(command, "ROL") {
        return Ok(get_roll_result(command, rng)?.map(SpecificCommandOutput::text));
    }
    if starts_ci(command, "MED") {
        return Ok(get_med_result(command, rng)?.map(SpecificCommandOutput::text));
    }
    if starts_ci(command, "RES") {
        return Ok(get_res_result(command, rng)?.map(SpecificCommandOutput::text));
    }
    if starts_ci(command, "INY") {
        return Ok(Some(SpecificCommandOutput::text(get_innyou_result(rng)?)));
    }
    if starts_ci(command, "HTK") {
        return Ok(Some(SpecificCommandOutput::text(get_hattoku_result(rng)?)));
    }
    if gog_pattern().is_match(command) {
        return Ok(Some(SpecificCommandOutput::text(get_gogyou_result(rng)?)));
    }
    Ok(None)
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `HouraiGakuen#getRollResult`。
fn get_roll_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = rol_pattern().captures(command) else {
        return Ok(None);
    };
    let target = to_i(&m[1]);
    let dice_list = rng.roll_barabara(3, 6)?;
    let total: i64 = dice_list.iter().sum();
    let dice_text = dice_text::join_dice(&dice_list);
    let result = get_check_result(&dice_list, total, target);
    Ok(Some(format!(
        "(3d6<={target}) ＞ 出目{dice_text}＝合計{total} ＞ {result}"
    )))
}

/// Ruby `HouraiGakuen#getCheckResult`。
fn get_check_result(dice_list: &[i64], total: i64, target: i64) -> &'static str {
    let mut sorted = dice_list.to_vec();
    sorted.sort_unstable();
    if is_fumble(&sorted) {
        return FUMBLE;
    }
    if is_critical(&sorted) {
        return CRITICAL;
    }
    if total <= target {
        return SUCCESS;
    }
    FAILURE
}

fn is_fumble(sorted: &[i64]) -> bool {
    sorted == [6, 6, 6]
}

fn is_critical(sorted: &[i64]) -> bool {
    sorted == [1, 2, 3]
}

/// Ruby `HouraiGakuen#getMedResult`。
fn get_med_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = med_pattern().captures(command) else {
        return Ok(None);
    };
    let your_value = to_i(&m[1]);
    let enemy_value = to_i(&m[2]);
    let target = get_target_from_value(your_value, enemy_value);
    let dice_list = rng.roll_barabara(3, 6)?;
    let total: i64 = dice_list.iter().sum();
    let dice_text = dice_text::join_dice(&dice_list);
    let result = get_check_result(&dice_list, total, target);
    Ok(Some(format!(
        "(あなたの値{your_value}、相手の値{enemy_value}、3d6<={target}) ＞ 出目{dice_text}＝合計{total} ＞ {result}"
    )))
}

fn get_target_from_value(your_value: i64, enemy_value: i64) -> i64 {
    your_value + (10 - enemy_value)
}

/// Ruby `HouraiGakuen#getResResult`。
fn get_res_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = res_pattern().captures(command) else {
        return Ok(None);
    };
    let your_value = to_i(&m[1]);
    let enemy_value = to_i(&m[2]);
    let your_target = get_target_from_value(your_value, enemy_value);
    let enemy_target = get_target_from_value(enemy_value, your_value);

    let your_dice = rng.roll_barabara(3, 6)?;
    let your_total: i64 = your_dice.iter().sum();
    let your_dice_text = dice_text::join_dice(&your_dice);
    let enemy_dice = rng.roll_barabara(3, 6)?;
    let enemy_total: i64 = enemy_dice.iter().sum();
    let enemy_dice_text = dice_text::join_dice(&enemy_dice);

    let your_result = get_check_result(&your_dice, your_total, your_target);
    let enemy_result = get_check_result(&enemy_dice, enemy_total, enemy_target);
    let result = get_resist_check_result(your_result, enemy_result);

    Ok(Some(format!(
        "あなたの値{your_value}、相手の値{enemy_value}\n(あなたのロール 3d6<={your_target}) ＞ {your_dice_text}={your_total} ＞ {your_result}\n(相手のロール 3d6<={enemy_target}) ＞ {enemy_dice_text}={enemy_total} ＞ {enemy_result}\n＞{result}"
    )))
}

fn get_resist_check_result(your_result: &str, enemy_result: &str) -> &'static str {
    let your_rank = result_rank(your_result);
    let enemy_rank = result_rank(enemy_result);
    if your_rank > enemy_rank {
        "あなたが勝利"
    } else if your_rank < enemy_rank {
        "相手が勝利"
    } else {
        "引き分け"
    }
}

fn result_rank(result: &str) -> i32 {
    match result {
        x if x == FUMBLE => 0,
        x if x == FAILURE => 1,
        x if x == SUCCESS => 2,
        x if x == CRITICAL => 3,
        _ => -1,
    }
}

/// Ruby `HouraiGakuen#getInnyouResult`。
fn get_innyou_result(rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut odd_count = 0i64;
    let mut even_count = 0i64;
    for _ in 0..3 {
        let dice = rng.roll_once(6)?;
        if dice % 2 == 0 {
            even_count += 1;
        } else {
            odd_count += 1;
        }
    }
    if even_count < odd_count {
        Ok("陽（奇数の方が多い）".to_owned())
    } else {
        Ok("陰（偶数の方が多い）".to_owned())
    }
}

/// Ruby `HouraiGakuen#getHattokuResult`。
fn get_hattoku_result(rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut odd_even = Vec::new();
    for _ in 0..3 {
        odd_even.push(get_odd_even(rng)?);
    }
    let odd_even_text = odd_even.join("、");
    let name = match odd_even_text.as_str() {
        "奇数、奇数、奇数" => "仁",
        "奇数、奇数、偶数" => "義",
        "奇数、偶数、奇数" => "礼",
        "奇数、偶数、偶数" => "智",
        "偶数、奇数、奇数" => "忠",
        "偶数、奇数、偶数" => "信",
        "偶数、偶数、奇数" => "孝",
        "偶数、偶数、偶数" => "悌",
        _ => {
            return Ok("異常終了".to_owned());
        }
    };
    Ok(format!("仁義八徳は、【{name}】({odd_even_text})"))
}

fn get_odd_even(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    let dice = rng.roll_once(6)?;
    if dice % 2 == 0 {
        Ok("偶数")
    } else {
        Ok("奇数")
    }
}

/// Ruby `HouraiGakuen#getGogyouResult`。
fn get_gogyou_result(rng: &mut Randomizer) -> Result<String, EvalError> {
    let table = [
        "五行【木】",
        "五行【火】",
        "五行【土】",
        "五行【金】",
        "五行【水】",
        "五行は【任意選択】",
    ];
    let number = rng.roll_sum(1, 6)?;
    let index = number - 1;
    let text = usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i).copied())
        .unwrap_or("1");
    Ok(format!("五行表({number}) ＞ {text}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "HouraiGakuen",
            "HouraiGakuen.toml",
            47,
        );
    }
}
