//! P4で手書き移植した `lib/bcdice/game_system/PersonaO.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `PersonaO#roll_attack`（基本判定 `PTx@y`）
//! - `PersonaO#roll_damage`（ダメージ計算 `nPD+x%y-z`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::PersonaO`（ID: `PersonaO`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersonaO;

impl GameSystem for PersonaO {
    fn id(&self) -> &'static str {
        "PersonaO"
    }

    fn name(&self) -> &'static str {
        "ペルソナTRPG-O"
    }

    fn sort_key(&self) -> &'static str {
        "へるそなTRPGO"
    }

    fn help_message(&self) -> &'static str {
        r"・基本判定
　PTx@y　x：目標値、y：クリティカル値（省略時は5）
　例）PT60　PT90@10

・ダメージ計算
　nPD+(x+y*2)%(z-a)-b　n：ダイス個数、x：スキル固定値、y：ボーナス、z：バフ倍率、a：耐性、b：敵防御力
　nPD+(x+y*2)までがスキルによる素のダメージ、zおよびaは計算式を入れてよい。
　
　例）ソニックパンチ、力B2点、
　　　タルカジャがかかっており、打撃耐性あり、
　　　目標の物理防御力は2点
　　　
　　　2PD+(20+2*2)%(100+50-50)-2
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["PT", r"\d+PD"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `PersonaO#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_attack(command) || roll_damage(command)
        if let Some(result) = roll_attack(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(roll_damage(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `/^PT(-?\d+)?(@(-?\d+))?$/i`。
fn attack_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^PT(-?\d+)?(@(-?\d+))?$").expect("valid regex"))
}

/// Ruby `/^(\d+)PD\+(-?\d+)%(-?\d+)-(\d+)$/i`。
fn damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)PD\+(-?\d+)%(-?\d+)-(\d+)$").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は符号側へ飽和させる。
///
/// 目標値・クリティカル値は1D100の出目との比較にしか使わないので、飽和させても分岐は変わらない。
/// ダメージ計算では飽和した値がそのまま出力に出るが、Ruby は多倍長のまま表示するので
/// 20桁を超える入力でのみ差が出る。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(if digits.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `PersonaO#roll_attack`（基本判定）。
fn roll_attack(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = attack_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1].to_i（nil.to_i == 0）
    let success_rate = captures.get(1).map_or(0, |m| to_i(m.as_str()));
    // Ruby: m[3]&.to_i || 5
    let critical_border = captures.get(3).map_or(5, |m| to_i(m.as_str()));

    let dice_value = rng.roll_once(100)?;
    let mut result = if dice_value <= critical_border {
        EvalResult::critical("クリティカル")
    } else if dice_value <= success_rate {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    };

    result.text = format!(
        "D100<={success_rate}@{critical_border} ＞ {dice_value} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `PersonaO#roll_damage`（ダメージ計算）。
fn roll_damage(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(captures) = damage_pattern().captures(command) else {
        return Ok(None);
    };

    let dice = to_i(&captures[1]);
    let kotei = to_i(&captures[2]);
    let hosei = to_i(&captures[3]);
    let bougyo = to_i(&captures[4]);

    let dice_list = rng.roll_barabara(dice, 10)?;
    let dice_sum: i64 = dice_list.iter().sum();

    // Ruby: (hosei * kotei / 100.0).to_i
    // 積は Ruby では多倍長なので i128 で正確に求め、Ruby の `Integer#to_f` と同じく
    // そこで初めて f64 に落とす。`Float#to_i` は0方向への切り捨て。
    let scaled = (i128::from(hosei) * i128::from(kotei)) as f64 / 100.0;
    let dmg = dice_sum
        .saturating_add(scaled.trunc() as i64)
        .saturating_sub(bougyo);

    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    Ok(Some(format!(
        "{dice}D10+{kotei}＊{hosei}%-{bougyo} ＞ [{dice_text}]+{kotei}＊{hosei}%-{bougyo} ＞ {dmg} ダメージ！"
    )))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("PersonaO", "PersonaO.toml", 13);
    }
}
