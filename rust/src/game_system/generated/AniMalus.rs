//! P4で手書き移植した `lib/bcdice/game_system/AniMalus.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#resolute_action`（ステータス判定 `nAM<=t,x`）
//! - `#resolute_investigation`（探索技能判定 `AI<=t,x`）
//! - `#resolute_attacking`（攻撃判定 `AA<=t`）
//! - `#resolute_guarding`（防御判定 `AG=t`）
//! - `#resolute_dodging`（回避判定 `AD=t`）

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby側の正規表現はいずれも `/i` なし。入力は `dice_command` が大文字化済み。
macro_rules! re {
    ($name:ident, $pattern:literal) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).unwrap())
        }
    };
}

re!(action_re, r"^(\d+)([-+]\d+)?AM<=(\d+),(\d)$");
re!(investigation_re, r"^([-+]\d+)?AI<=(\d+),(\d)$");
re!(attacking_re, r"^([-+]\d+)?AA<=(\d+)$");
re!(guarding_re, r"^([-+]\d+)?AG=(\d+)$");
re!(dodging_re, r"^([-+]\d+)?AD=(\d+)$");

/// Ruby `Regexp.last_match(n).to_i`（`nil.to_i == 0`）。
fn group_i64(caps: &Captures, index: usize) -> i64 {
    caps.get(index)
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(0)
}

/// Ruby `with_symbol(number)`。
fn with_symbol(number: i64) -> String {
    if number == 0 {
        String::new()
    } else if number > 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `dice.include?(1) && dice.include?(2) && dice.include?(3)`。
fn has_123(dice: &[i64]) -> bool {
    dice.contains(&1) && dice.contains(&2) && dice.contains(&3)
}

/// Ruby `dice.include?(4) && dice.include?(5) && dice.include?(6)`。
fn has_456(dice: &[i64]) -> bool {
    dice.contains(&4) && dice.contains(&5) && dice.contains(&6)
}

/// Ruby `Result.new.tap { ... }` の共通部分。
///
/// `result.critical` / `result.fumble` は `condition=`（success/failure）とは
/// 独立に立つので、`Result.critical` / `Result.fumble` 生成子は使えない。
fn build_result(critical: bool, fumble: bool, condition: bool) -> EvalResult {
    let mut result = EvalResult::new();
    result.critical = critical;
    result.fumble = fumble;
    result.set_condition(condition);
    result
}

/// Ruby `#resolute_action`。ステータスの判定。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = action_re().captures(command) else {
        return Ok(None);
    };

    let num_dice = group_i64(&caps, 1);
    let num_bonus = group_i64(&caps, 2);
    let num_target = group_i64(&caps, 3);
    let num_success = group_i64(&caps, 4);

    let mut dice = rng.roll_barabara(num_dice + num_bonus, 6)?;
    dice.sort_unstable();
    let success_num = dice.iter().filter(|v| **v <= num_target).count() as i64;

    let mut result = build_result(has_123(&dice), has_456(&dice), success_num >= num_success);

    let mut sequence = vec![
        format!(
            "({num_dice}{}AM<={num_target},{num_success})",
            with_symbol(num_bonus)
        ),
        dice_text::join_dice(&dice),
        format!("成功数{success_num}"),
        if result.success { "成功" } else { "失敗" }.to_owned(),
    ];
    if result.critical {
        sequence.push("クリティカル".to_owned());
    }
    if result.fumble {
        sequence.push("ファンブル".to_owned());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `#resolute_investigation`。探索技能の判定。
fn resolute_investigation(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = investigation_re().captures(command) else {
        return Ok(None);
    };

    let num_bonus = group_i64(&caps, 1);
    let num_target = group_i64(&caps, 2);
    let num_success = group_i64(&caps, 3);

    let mut dice = rng.roll_barabara(3 + num_bonus, 6)?;
    dice.sort_unstable();
    let success_num = dice.iter().filter(|v| **v <= num_target).count() as i64;

    let mut result = build_result(has_123(&dice), has_456(&dice), success_num >= num_success);

    let mut sequence = vec![
        format!("({}AI<={num_target},{num_success})", with_symbol(num_bonus)),
        dice_text::join_dice(&dice),
        format!("成功数{success_num}"),
        if result.success { "成功" } else { "失敗" }.to_owned(),
    ];
    if result.critical {
        sequence.push("クリティカル".to_owned());
    }
    if result.fumble {
        sequence.push("ファンブル".to_owned());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `#resolute_attacking`。攻撃技能の判定。
fn resolute_attacking(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = attacking_re().captures(command) else {
        return Ok(None);
    };

    let num_bonus = group_i64(&caps, 1);
    let num_target = group_i64(&caps, 2);

    let mut dice = rng.roll_barabara(3 + num_bonus, 6)?;
    dice.sort_unstable();
    let success_num = dice.iter().filter(|v| **v <= num_target).count() as i64;

    // Ruby: damage1 = damage2 = dice.max（空配列なら nil だが、その場合は失敗して未使用）
    let damage1 = dice.iter().copied().max().unwrap_or(0);
    let mut damage2 = damage1;
    // Ruby: (1..num_target).each。出目は1〜6なので7以上の idx は必ず count 0 になり
    // damage2 に影響しない。`AA<=9223372036854775807` のような入力で実質ハングするのを
    // 避けるため6で打ち切る（結果はRubyと同一）。
    for idx in 1..=num_target.min(6) {
        let count = dice.iter().filter(|v| **v == idx).count() as i64;
        if count > 1 {
            let now_damage = damage1 + 3 * (count - 1);
            if damage2 < now_damage {
                damage2 = now_damage;
            }
        }
    }

    let mut result = build_result(has_123(&dice), has_456(&dice), success_num > 0);

    let mut sequence = vec![
        format!("({}AA<={num_target})", with_symbol(num_bonus)),
        dice_text::join_dice(&dice),
        format!("成功数{success_num}"),
        if result.success { "成功" } else { "失敗" }.to_owned(),
    ];
    if result.success {
        sequence.push(format!("最大ダメージ({damage2})"));
    }
    if result.critical {
        sequence.push("クリティカル".to_owned());
    }
    if result.fumble {
        sequence.push("ファンブル".to_owned());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `#resolute_guarding`。防御技能の判定。
fn resolute_guarding(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = guarding_re().captures(command) else {
        return Ok(None);
    };

    let num_bonus = group_i64(&caps, 1);
    let num_target = group_i64(&caps, 2);

    let mut dice = rng.roll_barabara(3 + num_bonus, 6)?;
    dice.sort_unstable();
    let success_num = dice.iter().filter(|v| **v == num_target).count() as i64;

    let mut result = build_result(has_123(&dice), has_456(&dice), success_num > 0);

    let mut sequence = vec![
        format!("({}AG={num_target})", with_symbol(num_bonus)),
        dice_text::join_dice(&dice),
        format!("成功数{success_num}"),
        if result.success {
            format!("成功 ＞ ダメージ軽減({})", success_num * 2)
        } else {
            "失敗".to_owned()
        },
    ];
    if result.critical {
        sequence.push("クリティカル".to_owned());
    }
    if result.fumble {
        sequence.push("ファンブル".to_owned());
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `#resolute_dodging`。回避技能の判定。
///
/// 他の判定と違い出目をソートしない（Ruby側に `.sort` が無い）。
fn resolute_dodging(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = dodging_re().captures(command) else {
        return Ok(None);
    };

    let num_bonus = group_i64(&caps, 1);
    let num_target = group_i64(&caps, 2);

    let dice = rng.roll_barabara(1 + num_bonus, 6)?;
    let success_num = dice.iter().filter(|v| **v == num_target).count() as i64;

    let mut result = EvalResult::new();
    result.set_condition(success_num > 0);

    let sequence = [
        format!("({}AD={num_target})", with_symbol(num_bonus)),
        dice_text::join_dice(&dice),
        format!("成功数{success_num}"),
        if result.success {
            "成功(ダメージ無効)".to_owned()
        } else {
            "失敗".to_owned()
        },
    ];

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::AniMalus`（ID: `AniMalus`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AniMalus;

impl GameSystem for AniMalus {
    fn id(&self) -> &'static str {
        "AniMalus"
    }

    fn name(&self) -> &'static str {
        "アニマラス"
    }

    fn sort_key(&self) -> &'static str {
        "あにまらす"
    }

    fn help_message(&self) -> &'static str {
        r"■ステータスのダイス判定　n[+-b]AM<=t,x        n:能力値 b:修正値(省略可能) t:成功値 x:必要成功数
例)3AM<=2,1: ダイスを3個振って、成功値2,必要成功数1で判定。その結果(成功数,成功・失敗,クリティカル,ファンブル)を表示

■探索技能のダイス判定　[+-b]AI<=t,x        t:探索技能レベル b:修正値(省略可能) x:必要成功数
例)AI<=3,1: ダイスを3個振って、探索技能レベル3,必要成功数1で判定。その結果(成功数,成功・失敗,クリティカル,ファンブル)を表示

■攻撃判定　[+-b]AA<=t       t:戦闘技能レベル b:修正値(省略可能)
例)AA<=3: ダイスを3個振って、戦闘技能レベル3で判定。その結果(成功・失敗,ダメージ,クリティカル,ファンブル)を表示

■防御判定　[+-b]AG=t        t:攻撃技能レベル b:修正値(省略可能)
例)AG=2: ダイスを3個振って、攻撃技能レベル2で判定。その結果(成功・失敗,ダメージ軽減,クリティカル,ファンブル)を表示

■回避判定　[+-b]AD=t        t:攻撃技能レベル b:修正値(省略可能)
例)AD=3: ダイスを1個振って、攻撃技能レベル3で判定。その結果(成功・失敗)を表示
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[-+\d]*A[MIAGD]"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `#eval_game_system_specific_command`。
    ///
    /// Ruby: `resolute_action(command) || resolute_investigation(command) || ...`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let result = match resolute_action(command, rng)? {
            Some(r) => Some(r),
            None => match resolute_investigation(command, rng)? {
                Some(r) => Some(r),
                None => match resolute_attacking(command, rng)? {
                    Some(r) => Some(r),
                    None => match resolute_guarding(command, rng)? {
                        Some(r) => Some(r),
                        None => resolute_dodging(command, rng)?,
                    },
                },
            },
        };

        Ok(result.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("AniMalus", "AniMalus.toml", 31);
    }
}
