//! P4で手書き移植した `lib/bcdice/game_system/GoblinSlayer.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GoblinSlayer#eval_game_system_specific_command`
//!   → `getCheckResult`（`GS`）/ `murmurChantPrayInvoke`（`MCPI`）/ `damageBonus`（`DB`）
//! - `calc_threshold` / `resultStr`
//!
//! `MCPI` の接頭辞が `^MCPI.*$` なのは、因果点が共有リソースであるため
//! シークレットダイス（先頭 `S`）を無効化するため（原典コメントどおり）。

use std::sync::OnceLock;

use regex::Regex;

use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `GoblinSlayer::CRITICAL`。
const CRITICAL: i64 = 12;
/// Ruby `GoblinSlayer::FUMBLE`。
const FUMBLE: i64 = 2;

/// Ruby `BCDice::GameSystem::GoblinSlayer`（ID: `GoblinSlayer`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoblinSlayer;

impl GameSystem for GoblinSlayer {
    fn id(&self) -> &'static str {
        "GoblinSlayer"
    }

    fn name(&self) -> &'static str {
        "ゴブリンスレイヤーTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "こふりんすれいやあTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　GS(x)@c#f>=y
　2d6の判定を行い、達成値を出力します。
　xは基準値、yは目標値、cは大成功の下限、fは大失敗の上限です。いずれも省略可能です。
　cが未指定の場合には c=12 、fが未指定の場合には f=2 となります。
　yが設定されている場合、大成功/成功/失敗/大失敗を自動判定します。
　例）GS>=12　GS>10　GS(10)>14　GS+10>=15　GS10>=15　GS(10)　GS+10　GS10　GS
　　　GS@10　GS@10#3　GS#3@10

・祈念　MCPI(n)$m
　祈念を行います。
　nは【幸運】などによるボーナスです。この値は省略可能です。
　mは因果点の現在値です。
　因果点の現在値を使用して祈念を行い、成功/失敗を自動判定します。
　例）MCPI$3　MCPI(1)$4　MCPI+2$5　MCPI2$6

・命中判定の効力値によるボーナス　DB(n)
　ダメージ効力表による威力へのボーナスを自動で求めます。
　nは命中判定の効力値です。
　例）DB(15)　DB12

※上記コマンドの計算内で割り算を行った場合、小数点以下は切り上げされます。
　ただしダイス出目を割り算した場合、小数点以下は切り捨てされます。
　例）入力：GS(8+3/2)　実行結果：(GS10) ＞ 10 + 3[1,2] ＞ 13
　　　入力：2d6/2    　実行結果：(2D6/2) ＞ 3[1,2]/2 ＞ 1

※MCPIでは、シークレットダイスを使用できません。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["GS", "^MCPI.*$", "DB"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `GoblinSlayer#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command.len() >= 2 && command[..2].eq_ignore_ascii_case("GS") {
            return Ok(get_check_result(command, rng)?.map(SpecificCommandOutput::text));
        }
        if command.len() >= 4 && command[..4].eq_ignore_ascii_case("MCPI") {
            return Ok(murmur_chant_pray_invoke(command, rng)?.map(SpecificCommandOutput::text));
        }
        if command.len() >= 2 && command[..2].eq_ignore_ascii_case("DB") {
            return Ok(damage_bonus(command, rng)?.map(SpecificCommandOutput::text));
        }
        Ok(None)
    }
}

/// Ruby `String#to_i` 相当。桁あふれは符号方向へ飽和させる。
fn to_i(text: &str) -> i64 {
    text.parse().unwrap_or(if text.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `/^GS([-+]?\d+)?(?:(?:([@#])([-+]?\d+))(?:([@#])([-+]?\d+))?)?(?:(>=?)(\d+))?$/i`。
fn gs_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^GS([-+]?[0-9]+)?(?:(?:([@#])([-+]?[0-9]+))(?:([@#])([-+]?[0-9]+))?)?(?:(>=?)([0-9]+))?$",
        )
        .expect("valid regex")
    })
}

/// Ruby `/^MCPI(\+?\d+)?\$(\d+)$/i`。
fn mcpi_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^MCPI(\+?[0-9]+)?\$([0-9]+)$").expect("valid regex"))
}

/// Ruby `/^DB(\d+)$/i`。
fn db_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DB([0-9]+)$").expect("valid regex"))
}

/// Ruby `GoblinSlayer#getCheckResult`。
fn get_check_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = gs_pattern().captures(command) else {
        return Ok(None);
    };

    let basis = m.get(1).map(|g| to_i(g.as_str())).unwrap_or(0);
    let target = m.get(7).map(|g| to_i(g.as_str())).unwrap_or(0);
    let cmp_op = m.get(6).map(|g| g.as_str());
    let without_compare = cmp_op.is_none();

    let option1 = m.get(2).map(|g| g.as_str());
    let option1_value = m.get(3).map(|g| g.as_str());
    let option2 = m.get(4).map(|g| g.as_str());
    let option2_param = m.get(5).map(|g| g.as_str());

    if option1.is_some() && option1 == option2 {
        return Ok(None);
    }

    let (threshold_critical, threshold_fumble) =
        calc_threshold(option1, option1_value, option2, option2_param);

    let dice_list = rng.roll_barabara(2, 6)?;
    let total: i64 = dice_list.iter().sum();
    let dice_text = join_dice(&dice_list);
    let achievement = basis + total;

    let fumble = total <= threshold_fumble;
    let critical = total >= threshold_critical;

    let mut result = format!(
        " ＞ {}",
        result_str(achievement, target, cmp_op, fumble, critical)
    );
    if without_compare && !fumble && !critical {
        result.clear();
    }
    let basis_str = if basis == 0 {
        String::new()
    } else {
        format!("{basis} + ")
    };

    Ok(Some(format!(
        "({command}) ＞ {basis_str}{total}[{dice_text}] ＞ {achievement}{result}"
    )))
}

/// Ruby `GoblinSlayer#calc_threshold`。
fn calc_threshold(
    option1: Option<&str>,
    option1_value: Option<&str>,
    _option2: Option<&str>,
    option2_value: Option<&str>,
) -> (i64, i64) {
    let (critical, fumble) = if option1 == Some("@") {
        (option1_value, option2_value)
    } else {
        (option2_value, option1_value)
    };
    (
        critical.map(to_i).unwrap_or(CRITICAL),
        fumble.map(to_i).unwrap_or(FUMBLE),
    )
}

/// Ruby `GoblinSlayer#murmurChantPrayInvoke`。
fn murmur_chant_pray_invoke(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(m) = mcpi_pattern().captures(command) else {
        return Ok(None);
    };

    let luck = m.get(1).map(|g| to_i(g.as_str())).unwrap_or(0);
    let volition = to_i(&m[2]);
    if volition >= 12 {
        return Ok(Some(
            "因果点が12点以上の場合、因果点は使用できません。".to_owned(),
        ));
    }

    let dice_list = rng.roll_barabara(2, 6)?;
    let total: i64 = dice_list.iter().sum();
    let dice_text = join_dice(&dice_list);
    let achievement = total + luck;
    let result = format!(
        " ＞ {}",
        result_str(achievement, volition, Some(">="), false, false)
    );
    let luck_str = if luck == 0 {
        String::new()
    } else {
        format!("+{luck}")
    };

    Ok(Some(format!(
        "祈念(2d6{luck_str}) ＞ {total}[{dice_text}]{luck_str} ＞ {achievement}{result}, 因果点：{volition}点 → {}点",
        volition + 1
    )))
}

/// Ruby `GoblinSlayer#damageBonus`。
fn damage_bonus(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = db_pattern().captures(command) else {
        return Ok(None);
    };

    let num = to_i(&m[1]);
    let fmt = "命中判定の効力値によるボーナス ＞ ";
    if num < 15 {
        return Ok(Some(format!("{fmt}なし")));
    }

    let times = if num >= 40 {
        5
    } else if num >= 30 {
        4
    } else if num >= 25 {
        3
    } else if num >= 20 {
        2
    } else {
        1
    };

    let dice_list = rng.roll_barabara(times, 6)?;
    let total: i64 = dice_list.iter().sum();
    let dice_text = join_dice(&dice_list);
    Ok(Some(format!("{fmt}{total}[{dice_text}] ＞ {total}")))
}

/// Ruby `GoblinSlayer#resultStr`。
fn result_str(
    achievement: i64,
    target: i64,
    cmp_op: Option<&str>,
    fumble: bool,
    critical: bool,
) -> &'static str {
    if fumble {
        return "大失敗";
    }
    if critical {
        return "大成功";
    }
    if cmp_op == Some(">=") {
        if achievement >= target {
            "成功"
        } else {
            "失敗"
        }
    } else if achievement > target {
        "成功"
    } else {
        "失敗"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GoblinSlayer",
            "GoblinSlayer.toml",
            60,
        );
    }
}
