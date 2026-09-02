//! P4で手書き移植した `lib/bcdice/game_system/Avandner.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（AVコマンドのパース）
//! - `#checkRoll`（クリティカル分の自動振り足しループと結果整形）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::Avandner`（ID: `Avandner`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Avandner;

impl GameSystem for Avandner {
    fn id(&self) -> &'static str {
        "Avandner"
    }

    fn name(&self) -> &'static str {
        "黒絢のアヴァンドナー"
    }

    fn sort_key(&self) -> &'static str {
        "こつけんのあうあんとなあ"
    }

    fn help_message(&self) -> &'static str {
        r"・調査判定：nAVm[Cx]
・命中判定：nAVm*p[+t][Cx]
[]内は省略可能。

クリティカルヒットの分だけ、自動で振り足し処理を行います。0
「n」でダイス数を指定。
「m」で目標値を指定。省略は出来ません。
「Cx」でクリティカル値を指定。省略時は「1」、最大値は「2」、「0」でクリティカル無し。
「p」で攻撃力を指定。「*」は「x」でも可。
「+t」でクリティカルトリガーを指定。省略可能です。
攻撃力指定で命中判定となり、成功数ではなく、ダメージを結果表示します。

【書式例】
・5AV3 → 5d10で目標値3。
・6AV2C0 → 6d10で目標値2。クリティカル無し。
・4AV3*5 → 4d10で目標値3、攻撃力5の命中判定。
・7AV2x10 → 7d10で目標値2、攻撃力10の命中判定。
・8av4*7+10 → 8d10で目標値4、攻撃力7、クリティカルトリガー10の命中判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AV"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize`: `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)(\d+)AV(\d+)((x|\*)(\d+))?(\+(\d+))?(C(\d+))?$").expect("valid regex")
        });

        let Some(m) = re.captures(command) else {
            return Ok(None);
        };

        let dice_count = to_i(m.get(1));
        let target = to_i(m.get(2));
        // Ruby: `(Regexp.last_match(5) || 0).to_i` — 未捕獲は 0。
        let damage = to_i(m.get(5));
        let critical_trigger = to_i(m.get(7));
        // Ruby: `(Regexp.last_match(9) || 1).to_i.clamp(0, 2)` — 未捕獲は 1。
        let critical_number = m
            .get(9)
            .map_or(1, |c| c.as_str().parse::<i64>().unwrap_or(i64::MAX))
            .clamp(0, 2);

        let text = check_roll(
            dice_count,
            target,
            damage,
            critical_trigger,
            critical_number,
            rng,
        )?;
        Ok(Some(SpecificCommandOutput::text(text)))
    }
}

/// Ruby `String#to_i`（未捕獲は `0`）。
///
/// 捕獲する形は `\d+` だけなので、桁あふれ（Ruby だと Bignum）は飽和させる。
fn to_i(c: Option<regex::Match<'_>>) -> i64 {
    c.map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(i64::MAX))
}

/// Ruby `#checkRoll`。
fn check_roll(
    dice_count: i64,
    target: i64,
    damage: i64,
    critical_trigger: i64,
    critical_number: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut total_success_count: i64 = 0;
    let mut total_critical_count: i64 = 0;
    let mut text = String::new();

    let mut roll_count = dice_count;

    // クリティカル数だけ振り足す。振り足し分に出たクリティカルでさらに振り足す。
    while roll_count > 0 {
        let mut dice_array = rng.roll_barabara(roll_count, 10)?;
        dice_array.sort_unstable();
        let dice_text = dice_array
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let success_count = dice_array.iter().filter(|&&i| i <= target).count() as i64;
        let critical_count = dice_array.iter().filter(|&&i| i <= critical_number).count() as i64;

        total_success_count += success_count;
        total_critical_count += critical_count;

        if !text.is_empty() {
            text.push('+');
        }
        text += &format!("{success_count}[{dice_text}]");

        roll_count = critical_count;
    }

    let mut result = String::new();
    let is_damage = damage != 0;

    if is_damage {
        let total_damage = total_success_count * damage + total_critical_count * critical_trigger;

        result += &format!(
            "({dice_count}D10<={target}) ＞ {text} ＞ Hits：{total_success_count}*{damage}"
        );
        if critical_trigger > 0 {
            result += &format!(" + Trigger：{total_critical_count}*{critical_trigger}");
        }
        result += &format!(" ＞ {total_damage}ダメージ");
    } else {
        result +=
            &format!("({dice_count}D10<={target}) ＞ {text} ＞ 成功数：{total_success_count}");
    }

    if total_critical_count > 0 {
        result += &format!(" / {total_critical_count}クリティカル");
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Avandner", "Avandner.toml", 21);
    }
}
