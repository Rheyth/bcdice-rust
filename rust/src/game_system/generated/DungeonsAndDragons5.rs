//! P4で手書き移植した `lib/bcdice/game_system/DungeonsAndDragons5.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DungeonsAndDragons5#eval_game_system_specific_command`
//!   （`attack_roll` → `ability_roll` → `twohands_damage_roll` の順に試す）
//! - `DungeonsAndDragons5#exec_roll`（1D20の共通ロール。有利／不利・ボーナスダイス込み）
//!
//! `translate` する文言は [`Translations`] に切り出してあり、`ko_kr` ロケールの
//! [`super::DungeonsAndDragons5_Korean`] が同じ実装を別の文言で使う。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `translate(...)` が引くロケール別の文言。
pub(crate) struct Translations {
    /// i18n `critical`
    pub critical: &'static str,
    /// i18n `fumble`
    pub fumble: &'static str,
    /// i18n `success`
    pub success: &'static str,
    /// i18n `failure`
    pub failure: &'static str,
}

/// i18n `ja_jp`（`i18n/ja_jp.yml`）。
pub(crate) static JA_JP: Translations = Translations {
    critical: "クリティカル",
    fumble: "ファンブル",
    success: "成功",
    failure: "失敗",
};

/// Ruby `BCDice::GameSystem::DungeonsAndDragons5`（ID: `DungeonsAndDragons5`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonsAndDragons5;

impl GameSystem for DungeonsAndDragons5 {
    fn id(&self) -> &'static str {
        "DungeonsAndDragons5"
    }

    fn name(&self) -> &'static str {
        "ダンジョンズ＆ドラゴンズ第5版"
    }

    fn sort_key(&self) -> &'static str {
        "たんしよんすあんととらこんす5"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AT", "AR", "2H"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng, &JA_JP)
    }
}

/// Ruby `DungeonsAndDragons5#eval_game_system_specific_command`。
///
/// `attack_roll(command) || ability_roll(command) || twohands_damage_roll(command)`。
pub(crate) fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
    tr: &Translations,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = attack_roll(command, rng, tr)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = ability_roll(command, rng, tr)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = twohands_damage_roll(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(None)
}

/// Ruby `DungeonsAndDragons5#number_with_sign_from_int`。
fn number_with_sign_from_int(number: i64) -> String {
    if number == 0 {
        String::new()
    } else if number > 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `Regexp.last_match(n).to_i`。
///
/// Ruby は桁あふれしても Bignum になるが、i64 に収まらない入力は飽和させる
/// （振れるダイス数の上限や目標値の比較では、どちらでも結果が変わらない）。
fn to_i(text: &str) -> i64 {
    text.parse().unwrap_or(i64::MAX)
}

/// Ruby `dice.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `exec_roll` の正規表現。
fn exec_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^(AT|AR)([-+\d]+)?(@(\d+))?(>=(\d+))?([AD]?)6?(B(\[(\d+D\d+([-+]\d+D\d+)*)\])?)?",
        )
        .expect("valid regex")
    })
}

/// Ruby `attack_roll` の正規表現（`exec_roll` とグループ番号がずれる点に注意）。
fn attack_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^AT([-+\d]+)?(@(\d+))?(>=(\d+))?([AD]?)6?(B(\[(\d+D\d+([-+]\d+D\d+)*)\])?)?")
            .expect("valid regex")
    })
}

/// Ruby `ability_roll` の正規表現。
fn ability_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^AR([-+\d]+)?(>=(\d+))?([AD]?)6?(B(\[(\d+D\d+([-+]\d+D\d+)*)\])?)?")
            .expect("valid regex")
    })
}

/// Ruby `twohands_damage_roll` の正規表現。
fn twohands_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^2H(\d+)D(\d+)([-+\d]+)?").expect("valid regex"))
}

/// Ruby `exec_roll` の戻り値 `[usedie, dice_result, difficulty, output]`。
struct ExecRoll {
    /// 判定に使った1個の出目
    usedie: i64,
    /// 修正・ボーナス込みの合計
    dice_result: i64,
    /// 目標値（未指定なら0）
    difficulty: i64,
    /// 途中経過
    output: Vec<String>,
}

/// Ruby `DungeonsAndDragons5#exec_roll`。
fn exec_roll(command: &str, rng: &mut Randomizer) -> Result<Option<ExecRoll>, EvalError> {
    let Some(m) = exec_roll_pattern().captures(command) else {
        return Ok(None);
    };

    let mut modify = I::ZERO;
    let mut mod_str = String::new();
    if let Some(modifier) = m.get(2) {
        // Ruby: Arithmetic.eval が nil を返したら（"+" だけなど）コマンド不成立
        let Some(value) = arithmetic::eval(modifier.as_str(), RoundType::Floor)? else {
            return Ok(None);
        };
        modify = value.clone();
        mod_str = number_with_sign_from_int(crate::randomizer::sat_i64(&value));
    }
    let critical_no = m.get(4).map_or(0, |x| to_i(x.as_str()));
    let difficulty = m.get(6).map_or(0, |x| to_i(x.as_str()));
    let advantage = m.get(7).map_or("", |x| x.as_str());
    let bonus = m.get(8).map_or("", |x| x.as_str());
    let bonus_dice = m.get(10).map_or("", |x| x.as_str());

    let mut dice_command = format!(
        "{}{}",
        &m[1],
        number_with_sign_from_int(crate::randomizer::sat_i64(&modify))
    );
    if critical_no >= 1 {
        dice_command += &format!("@{critical_no}");
    }
    if difficulty > 0 {
        dice_command += &format!(">={difficulty}");
    }
    dice_command += advantage;
    dice_command += bonus;

    let mut output = vec![format!("({dice_command})")];

    let (usedie, roll_die) = if advantage.is_empty() {
        let usedie = rng.roll_once(20)?;
        (usedie, usedie.to_string())
    } else {
        let dice = rng.roll_barabara(2, 20)?;
        // Ruby: advantage == "A" なら最大、それ以外（"D"）なら最小
        let usedie = if advantage == "A" {
            dice.iter().copied().max().expect("2 dice")
        } else {
            dice.iter().copied().min().expect("2 dice")
        };
        (usedie, format!("{usedie}[{}]", join_dice(&dice)))
    };

    let mut bonus_mod = 0;
    let mut bonus_str = String::new();
    if !bonus.is_empty() {
        if bonus == "B" {
            bonus_mod = rng.roll_once(4)?;
            bonus_str = number_with_sign_from_int(bonus_mod);
        } else if !bonus_dice.is_empty() {
            // Ruby: bonus_dice.gsub(/([+-])/, ",\\1").split(',')
            let mut bonus_mod_arr: Vec<i64> = Vec::new();
            for part in split_bonus_dice(bonus_dice) {
                // Ruby: i.split("D") → [個数, 面数]
                let mut fields = part.splitn(2, 'D');
                let now_dice_count = fields.next().map_or(0, ruby_to_i);
                let now_dice_number = fields.next().map_or(0, ruby_to_i);
                let dice = rng.roll_barabara(now_dice_count.abs(), now_dice_number)?;
                let sum: i64 = dice.iter().sum();
                bonus_mod_arr.push(if now_dice_count > 0 { sum } else { -sum });
            }
            bonus_mod = bonus_mod_arr.iter().sum();
            bonus_str = format!(
                "{}[{}]",
                number_with_sign_from_int(bonus_mod),
                join_dice(&bonus_mod_arr)
            );
        }
    }

    output.push(format!("{roll_die}{mod_str}{bonus_str}"));
    if !(mod_str.is_empty() && advantage.is_empty() && bonus.is_empty()) {
        output.push((usedie + crate::randomizer::sat_i64(&modify) + bonus_mod).to_string());
    }

    Ok(Some(ExecRoll {
        usedie,
        dice_result: usedie + crate::randomizer::sat_i64(&modify) + bonus_mod,
        difficulty,
        output,
    }))
}

/// Ruby `bonus_dice.gsub(/([+-])/, ",\\1").split(',')`。
///
/// 符号の前にカンマを挿入してから分割するので、`1D4+1D8` は `["1D4", "+1D8"]` になる。
fn split_bonus_dice(bonus_dice: &str) -> Vec<String> {
    let mut replaced = String::new();
    for c in bonus_dice.chars() {
        if c == '+' || c == '-' {
            replaced.push(',');
        }
        replaced.push(c);
    }
    // Ruby String#split(',') は末尾の空要素を落とすが、ここでは正規表現の形から
    // 空要素は現れない。
    replaced
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect()
}

/// Ruby `String#to_i`（先頭の符号つき数字だけを読み、読めなければ0）。
fn ruby_to_i(text: &str) -> i64 {
    let bytes = text.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return 0;
    }
    text[..end].parse().unwrap_or(i64::MAX)
}

/// Ruby `DungeonsAndDragons5#attack_roll`（攻撃ロール）。
fn attack_roll(
    command: &str,
    rng: &mut Randomizer,
    tr: &Translations,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = attack_roll_pattern().captures(command) else {
        return Ok(None);
    };
    // Ruby: critical_no = 20 unless m[3]
    let critical_no = m.get(3).map_or(20, |x| to_i(x.as_str()));

    let Some(mut roll) = exec_roll(command, rng)? else {
        // Ruby: usedie が nil（exec_roll が nil を返した）なら nil
        return Ok(None);
    };

    let mut critical = false;
    let mut fumble = false;
    let mut success = false;
    if roll.usedie >= critical_no {
        critical = true;
        success = true;
        roll.output.push(tr.critical.to_owned());
    } else if roll.usedie == 1 {
        fumble = true;
        roll.output.push(tr.fumble.to_owned());
    } else if roll.difficulty > 0 {
        if roll.dice_result >= roll.difficulty {
            success = true;
            roll.output.push(tr.success.to_owned());
        } else {
            roll.output.push(tr.failure.to_owned());
        }
    }

    let mut result = EvalResult::with_text(roll.output.join(" ＞ "));
    if roll.difficulty > 0 || critical || fumble {
        result.set_condition(success);
    }
    result.critical = critical;
    result.fumble = fumble;

    Ok(Some(result))
}

/// Ruby `DungeonsAndDragons5#ability_roll`（能力値ロール）。
fn ability_roll(
    command: &str,
    rng: &mut Randomizer,
    tr: &Translations,
) -> Result<Option<EvalResult>, EvalError> {
    if !ability_roll_pattern().is_match(command) {
        return Ok(None);
    }

    let Some(mut roll) = exec_roll(command, rng)? else {
        return Ok(None);
    };

    let mut success = false;
    if roll.difficulty > 0 {
        if roll.dice_result >= roll.difficulty {
            success = true;
            roll.output.push(tr.success.to_owned());
        } else {
            roll.output.push(tr.failure.to_owned());
        }
    }

    let mut result = EvalResult::with_text(roll.output.join(" ＞ "));
    if roll.difficulty > 0 {
        result.set_condition(success);
    }

    Ok(Some(result))
}

/// Ruby `DungeonsAndDragons5#twohands_damage_roll`（武器の両手持ちダメージ）。
fn twohands_damage_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = twohands_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = to_i(&m[1]);
    let dice_number = to_i(&m[2]);

    let mut modify = I::ZERO;
    let mut mod_str = String::new();
    if let Some(modifier) = m.get(3) {
        let Some(value) = arithmetic::eval(modifier.as_str(), RoundType::Floor)? else {
            return Ok(None);
        };
        modify = value.clone();
        mod_str = number_with_sign_from_int(crate::randomizer::sat_i64(&value));
    }

    let mut output = vec![format!("(2H{dice_count}D{dice_number}{mod_str})")];

    let dice = rng.roll_barabara(dice_count, dice_number)?;
    output.push(format!("[{}]{mod_str}", join_dice(&dice)));

    // 出目1,2を振り直す（パラディン／ファイターの両手持ち）
    let mut ex_dice: Vec<i64> = Vec::new();
    let mut new_dice: Vec<i64> = Vec::new();
    let mut sum_dice = 0;
    for num in &dice {
        if *num > 2 {
            sum_dice += num;
            ex_dice.push(*num);
        } else {
            let one_die = rng.roll_once(dice_number)?;
            sum_dice += one_die;
            new_dice.push(one_die);
        }
    }
    if !new_dice.is_empty() {
        output.push(format!(
            "[{}][{}]{mod_str}",
            join_dice(&ex_dice),
            join_dice(&new_dice)
        ));
    }
    output.push((sum_dice + crate::randomizer::sat_i64(&modify)).to_string());

    Ok(Some(EvalResult::with_text(output.join(" ＞ "))))
}

/// Ruby `HELP_MESSAGE` 定数。
const HELP_MESSAGE: &str = r"・攻撃ロール　AT[x][@c][>=t][y][B]
　x：+-修正。省略可。
　c:クリティカル値。省略可。
　t:敵のアーマークラス。>=を含めて省略可。
　y:有利(A), 不利(D)。省略可。
　B:ブレスやガイダンス等によるボーナス。省略可。
　　Bだけを書くと+1d4、B[1D4+1D8]などと書くと[]内のロールをボーナスとしてロールします。
　ファンブル／失敗／成功／クリティカル を自動判定。
　例）AT AT>=10 AT+5>=18 AT-3>=16 ATA AT>=10A AT+3>=18A AT-3>=16 ATD AT>=10D AT+5>=18D AT-5>=16D
　    AT@19 AT+5@18 AT-2@19>=15 AT+3>=18AB AT+3>=18DB[1D6]
・能力値判定　AR[x][>=t][y][b]
　攻撃ロールと同様。失敗／成功を自動判定。
　例）AR AR>=10 AR+5>=18 AR-3>=16 ARA AR>=10A AR+3>=18A AR-3>=16 ARD AR>=10D AR+5>=18D AR-5>=16D
　     AR+3>=18AB AR+3>=18DB[1D6]
・両手持ちのダメージ　2HnDx[m]
　n:ダイスの個数
　x:ダイスの面数
　m:+-修正。省略可。
　パラディンとファイターの武器の両手持ちによるダメージダイスの1,2の出目の振り直しを行います。
　例)2H3D6 2H1D10+3 2H2D8-1
";

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DungeonsAndDragons5",
            "DungeonsAndDragons5.toml",
            74,
        );
    }
}
