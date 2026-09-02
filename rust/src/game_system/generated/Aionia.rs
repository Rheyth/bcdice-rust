//! P4で手書き移植した `lib/bcdice/game_system/Aionia.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `Aionia#roll_skills`（技能判定 `AB[T]?n+m>=d/m2`）
//! - `Aionia#roll_damage_check`（ダメージチェック `DMG>=dif`）

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Aionia`（ID: `Aionia`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aionia;

impl GameSystem for Aionia {
    fn id(&self) -> &'static str {
        "Aionia"
    }

    fn name(&self) -> &'static str {
        "慈悲なきアイオニア"
    }

    fn sort_key(&self) -> &'static str {
        "しひなきあいおにあ"
    }

    fn help_message(&self) -> &'static str {
        r"- 技能判定（クリティカル・ファンブルなし）
AB{n}>={dif} n=10面ダイスの数、dif=難易度
- 技能判定（クリティカル・ファンブルあり）
ABT{n}>={dif} n=10面ダイスの数、dif=難易度
- ダメージチェック
DMG>={dif} dif=ダメージ難易度

※ 技能判定、ダメージチェックともにダイス結果、難易度に対して四則演算（+ - * /）を用いた複数ボーナスを含めることが可能です。計算結果の小数は切り捨てられます。

例:AB2>=5          （一般技能を活用して難易度5の技能判定。 クリファンなし。）
例:ABT3>=15        （専門技能を活用して難易度15の技能判定。クリファンあり。）
例:AB1+1+2>=8      （一般技能を活用せず難易度8の技能判定。 ボーナスとして+1と+2点の補正あり。  クリファンなし。）
例:ABT3-3>=10+2    （専門技能を活用して難易度10+2の技能判定。ペナルティとして-3点の補正あり。クリファンあり。）
例:ABT2>=4/8/12    （一般技能を活用して難易度4/8/12の段階的な技能判定。クリファンあり。）
例:DMG>=50         （難易度50の判定。）
例:DMG>=20+50      （難易度20+50の判定。）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ABT?", "DMG"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Aionia#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        match roll_skills(command, rng)? {
            Some(result) => Ok(Some(SpecificCommandOutput::result(result))),
            None => match roll_damage_check(command, rng)? {
                Some(result) => Ok(Some(SpecificCommandOutput::result(result))),
                None => Ok(None),
            },
        }
    }
}

/// Ruby `Aionia#roll_skills`。
fn roll_skills(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: %r{^AB(T?)(\d+)((?:[-+]\d+)*)>=(\d+(?:/\d+)*)((?:[-+]\d+)*)$}
    let re = Regex::new(r"\AAB(T?)(\d+)((?:[-+]\d+)*)>=(\d+(?:/\d+)*)((?:[-+]\d+)*)\z")
        .expect("valid regex");
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // 値の取得
    let use_cf = !&m[1].is_empty();
    let times: i64 = m[2].parse().unwrap_or(i64::MAX);

    // 値の計算
    let bonus = if m[3].is_empty() {
        0
    } else {
        arithmetic::eval(&m[3], RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)
    };
    let base_targets: Vec<i64> = m[4]
        .split('/')
        .map(|t| t.parse().unwrap_or(i64::MAX))
        .collect();
    let target_bonus = if m[5].is_empty() {
        0
    } else {
        arithmetic::eval(&m[5], RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)
    };

    let targets: Vec<i64> = base_targets.iter().map(|t| t + target_bonus).collect();
    let target = targets[0];
    let min_target = *targets.iter().min().unwrap();
    let max_target = *targets.iter().max().unwrap();

    // ダイスロール
    let dice_list = rng.roll_barabara(times, 10)?;
    let dice_total: i64 = dice_list.iter().sum();
    let total = dice_total + bonus;

    // 結果判定
    let result;
    let mut is_success = false;
    let mut has_critical = false;
    let mut has_fumble = false;

    let fumble_hit = dice_list.iter().filter(|&&x| x == 1).count() == dice_list.len();

    if targets.len() == 1 {
        // 難易度が一つの場合
        if total >= target {
            is_success = true;
            if total >= target + 20 && use_cf {
                result = "クリティカル";
                has_critical = true;
            } else if target <= times {
                result = "自動成功";
            } else {
                result = "成功";
            }
        } else if fumble_hit && use_cf {
            result = "ファンブル";
            has_fumble = true;
        } else if target > 10 * times {
            result = "自動失敗";
        } else {
            result = "失敗";
        }
    } else {
        // 段階的な難易度判定の場合
        if total >= min_target {
            is_success = true;
            if total >= max_target + 20 && use_cf {
                result = "クリティカル";
                has_critical = true;
            } else if max_target <= times {
                result = "自動成功";
            } else if total >= max_target {
                result = "全成功";
            } else {
                let times_suc = targets.iter().filter(|&&x| x <= total).count();
                result = Box::leak(format!("{times_suc}段階成功").into_boxed_str());
            }
        } else if fumble_hit && use_cf {
            result = "ファンブル";
            has_fumble = true;
        } else if min_target > 10 * times {
            result = "自動失敗";
        } else {
            result = "失敗";
        }
    }

    // ボーナスがある場合の処理
    let bonus_text = &m[3];
    let bonus_result = if m[3].is_empty() {
        String::new()
    } else {
        format!("{total} ＞ ")
    };

    let text = format!(
        "({command}) ＞ {dice_total}[{}]{bonus_text} ＞ {bonus_result}{result}",
        dice_text::join_dice(&dice_list)
    );

    let mut r = EvalResult::with_text(text);
    r.critical = has_critical;
    r.fumble = has_fumble;
    r.success = is_success;
    r.failure = !is_success;
    Ok(Some(r))
}

/// Ruby `Aionia#roll_damage_check`。
fn roll_damage_check(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: Command::Parser.new("DMG", round_type: FLOOR).restrict_cmp_op_to(:>=)
    // 原典の Command::Parser は式も受け付けるが、TOML のケースは `DMG>=<式>` のみ。
    // 単純な数式も解釈できるよう target 部分を Arithmetic.eval に通す。
    let re = Regex::new(r"\ADMG(>=)(.+)\z").expect("valid regex");
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // 値の計算
    // Ruby: dif = parsed.target_number（式も Arithmetic.eval で解決される）
    let dif = match arithmetic::eval(&m[2], RoundType::Floor)? {
        Some(v) => v,
        // Ruby: return nil unless dif（式が空のとき）
        None => return Ok(None),
    };

    // ダイスロール
    let dice_result = rng.roll_once(100)?;

    // 結果判定
    let result_str;
    let is_success;
    if dice_result < sat_i64(&(dif.clone() / I::from(5))) {
        let second_result = rng.roll_once(100)?;
        if second_result >= crate::randomizer::sat_i64(&dif) {
            result_str = format!("失敗 > 弱点追加 ＞ {second_result} ＞ 戦闘不能状態");
        } else {
            result_str = format!("失敗 > 弱点追加 ＞ {second_result} ＞ 死亡状態");
        }
        is_success = false;
    } else if dice_result < sat_i64(&(dif.clone() / I::from(2))) {
        result_str = "失敗 > 弱点追加 > 戦闘不能状態".to_owned();
        is_success = false;
    } else if dice_result < crate::randomizer::sat_i64(&dif) {
        result_str = "失敗 > 戦闘不能状態".to_owned();
        is_success = false;
    } else {
        result_str = "成功".to_owned();
        is_success = true;
    }

    let text = format!("({command}) ＞ {dice_result} ＞ {result_str}");
    let mut r = EvalResult::with_text(text);
    r.success = is_success;
    r.failure = !is_success;
    Ok(Some(r))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "Aionia",
            "Aionia.toml",
            89,
            &[(89, 1)],
        );
    }
}
