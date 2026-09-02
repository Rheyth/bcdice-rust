//! P4で手書き移植した `lib/bcdice/game_system/Chill.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意
//! （`HANDWRITTEN_IDS` へ追加すると `mod.rs` とレジストリから落ちるため、
//! 手書き化したシステムの除外機構は上位の整理に委ねている）。
//!
//! 移植したもの:
//! - `Chill#result_1d100`（1D100の成功度判定）
//! - `Chill#eval_game_system_specific_command` → `roll_strike_rank_result` / `check_strike_rank`
//!   （ストライク・ランク `SRx`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::floor_div;
use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Chill`（ID: `Chill`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chill;

impl GameSystem for Chill {
    fn id(&self) -> &'static str {
        "Chill"
    }

    fn name(&self) -> &'static str {
        "Chill"
    }

    fn sort_key(&self) -> &'static str {
        "ちる"
    }

    fn help_message(&self) -> &'static str {
        r#"・ストライク・ランク　(SRx)
　"SRストライク・ランク"の形で記入します。
　ストライク・ランク・チャートに従って自動でダイスロールを行い、
　負傷とスタミナロスを計算します。
　ダイスロールと同様に、他のプレイヤーに隠れてロールすることも可能です。
　例）SR7　　　sr13　　　SR(7+4)　　　Ssr10
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["SR"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Chill#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil if target == '?'
        let Target::Number(target) = target else {
            return None;
        };
        // Ruby: return nil if cmp_op != :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        let result = if total >= I::from(100) {
            EvalResult::fumble("ファンブル")
        } else if total > target {
            EvalResult::failure("失敗")
        } else if is_ge_ninety_percent(sat_i64(&total), sat_i64(&target)) {
            EvalResult::success("Ｌ成功")
        } else if total >= floor_div(target.clone(), I::from(2)) {
            // Ruby `Integer#/` は床除算
            EvalResult::success("Ｍ成功")
        } else if total >= floor_div(target, I::from(10)) {
            EvalResult::success("Ｈ成功")
        } else {
            EvalResult::critical("Ｃ成功")
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_strike_rank_result(command, rng)
    }
}

/// Ruby `total >= (target * 0.9)`。
///
/// Ruby側は `Integer * Float` なので浮動小数点演算になり、整数演算に置き換えると
/// 結果が変わる（target=11 なら 9.9 で total=9 は届かないが、`target * 9 / 10` だと
/// 9 になって届いてしまう）。Ruby と同じく f64 で掛けて f64 のまま比較する。
fn is_ge_ninety_percent(total: i64, target: i64) -> bool {
    (total as f64) >= (target as f64) * 0.9
}

/// Ruby `Chill#roll_strike_rank_result` のストライク・ランク抽出。
///
/// Ruby: `/(^|\s)[sS]?(SR|sr)(\d+)($|\s)/`
/// `Preprocessor` が最初の空白より前しか残さないので `\s` 側の枝は実際には通らないが、
/// 原典どおりに残す。
fn strike_rank_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(^|\s)[sS]?(SR|sr)(\d+)($|\s)").expect("valid regex"))
}

/// Ruby `Chill#check_strike_rank` の戻り値 `[damage, dice, dice_add, dice_str]`。
struct StrikeRankRoll {
    /// ダメージ（スタミナ損失または負傷）
    damage: i64,
    /// 振ったダイスの式
    dice: String,
    /// 計算過程
    dice_add: String,
    /// 出目
    dice_str: String,
}

/// Ruby `Chill#roll_strike_rank_result`。
fn roll_strike_rank_result(
    string: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(captures) = strike_rank_pattern().captures(string) else {
        // Ruby: 接頭辞にはマッチしたが SRx ではなかった場合の番兵
        return Ok(Some(SpecificCommandOutput::text("1")));
    };

    // Ruby: Regexp.last_match(3).to_i
    // 桁あふれする入力（SR99999999999999999999999）は Ruby だと Bignum になり
    // `roll_barabara` が TooManyRandsError を上げる。i64 に収まらない場合も
    // 同じ経路（振れる数の上限超過）へ落ちるように飽和させる。
    let strike_rank: i64 = captures[3].parse().unwrap_or(i64::MAX);

    let sta_loss;
    let wounds;
    let dice;
    let dice_add;
    let dice_str;

    if strike_rank < 14 {
        let sta = check_strike_rank(strike_rank, rng)?;
        let wound = check_strike_rank(strike_rank - 3, rng)?;

        sta_loss = sta.damage;
        wounds = wound.damage;
        dice = format!("{}, {}", sta.dice, wound.dice);
        dice_add = format!("{}, {}", sta.dice_add, wound.dice_add);
        dice_str = format!("{}, {}", sta.dice_str, wound.dice_str);
    } else {
        // Ruby: sta_loss, _dice, dice_add, dice_str = check_strike_rank(13)
        //       （dice は捨てて、下で 5d10*3 込みの式を組み立て直す）
        let sta = check_strike_rank(13, rng)?;
        sta_loss = sta.damage;

        let extra_times = strike_rank.saturating_sub(13).saturating_mul(2);

        let dice_list = rng.roll_barabara(4, 10)?;
        let wounds_base: i64 = dice_list.iter().sum();
        dice = format!("5d10*3, 4d10+{extra_times}d10");
        let mut dice_add_buf = format!("{}, {wounds_base}", sta.dice_add);
        let mut dice_str_buf = format!("{}, {}", sta.dice_str, dice_text::join_dice(&dice_list));

        let dice_list = rng.roll_barabara(extra_times, 10)?;
        let wounds_wk: i64 = dice_list.iter().sum();
        dice_str_buf.push_str(&format!("+{}", dice_text::join_dice(&dice_list)));
        dice_add_buf.push_str(&format!("+{wounds_wk}"));

        wounds = wounds_base + wounds_wk;
        dice_add = dice_add_buf;
        dice_str = dice_str_buf;
    }

    // Ruby は `output.empty?` を見て "1" を返すが、固定文言を含むので空にならない。
    let output = format!("{dice_str} ＞ {dice_add} ＞ スタミナ損失{sta_loss}, 負傷{wounds}");
    // Ruby: string += ':' + dice
    Ok(Some(SpecificCommandOutput::text(format!(
        "({string}:{dice}) ＞ {output}"
    ))))
}

/// Ruby `Chill#check_strike_rank`。
fn check_strike_rank(strike_rank: i64, rng: &mut Randomizer) -> Result<StrikeRankRoll, EvalError> {
    if strike_rank < 1 {
        return Ok(StrikeRankRoll {
            damage: 0,
            dice: "-".to_owned(),
            dice_add: "-".to_owned(),
            dice_str: "-".to_owned(),
        });
    }

    if strike_rank < 2 {
        let rolled = rng.roll_once(2)?;
        // Ruby: dice_str は -1 する前、dice_add は -1 した後の値
        let damage = rolled - 1;
        return Ok(StrikeRankRoll {
            damage,
            dice: "0or1".to_owned(),
            dice_add: damage.to_string(),
            dice_str: rolled.to_string(),
        });
    }

    if strike_rank < 3 {
        let damage = rng.roll_once(2)?;
        return Ok(StrikeRankRoll {
            damage,
            dice: "1or2".to_owned(),
            dice_add: damage.to_string(),
            dice_str: damage.to_string(),
        });
    }

    if strike_rank < 4 {
        let damage = rng.roll_once(5)?;
        return Ok(StrikeRankRoll {
            damage,
            dice: "1d5".to_owned(),
            dice_add: damage.to_string(),
            dice_str: damage.to_string(),
        });
    }

    if strike_rank < 10 {
        let times = strike_rank - 3;
        let dice_list = rng.roll_barabara(times, 10)?;
        let damage: i64 = dice_list.iter().sum();
        return Ok(StrikeRankRoll {
            damage,
            dice: format!("{times}d10"),
            dice_add: damage.to_string(),
            dice_str: dice_text::join_dice(&dice_list),
        });
    }

    if strike_rank < 13 {
        let times = strike_rank - 6;
        let dice_list = rng.roll_barabara(times, 10)?;
        let total: i64 = dice_list.iter().sum();
        return Ok(StrikeRankRoll {
            damage: total * 2,
            dice: format!("{times}d10*2"),
            dice_add: format!("{total}*2"),
            dice_str: format!("({})*2", dice_text::join_dice(&dice_list)),
        });
    }

    let dice_list = rng.roll_barabara(5, 10)?;
    let total: i64 = dice_list.iter().sum();
    Ok(StrikeRankRoll {
        damage: total * 3,
        dice: "5d10*3".to_owned(),
        dice_add: format!("{total}*3"),
        dice_str: format!("({})*3", dice_text::join_dice(&dice_list)),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Chill", "Chill.toml", 203);
    }
}
