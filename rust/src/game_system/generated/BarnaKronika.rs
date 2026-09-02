//! P4で手書き移植した `lib/bcdice/game_system/BarnaKronika.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `BarnaKronika#replace_text`（`nBKC` / `nBAC` / `nBK` / `nBA` → `nR6[mode,call]`）
//! - `BarnaKronika#eval_game_system_specific_command` → `roll_barna_kronika`
//!   （出目の最大重複＝成功数。戦闘モードは命中部位表）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::BarnaKronika`（ID: `BarnaKronika`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarnaKronika;

impl GameSystem for BarnaKronika {
    fn id(&self) -> &'static str {
        "BarnaKronika"
    }

    fn name(&self) -> &'static str {
        "バルナ・クロニカ"
    }

    fn sort_key(&self) -> &'static str {
        "はるなくろにか"
    }

    fn help_message(&self) -> &'static str {
        r"・通常判定　nBK
　ダイス数nで判定ロールを行います。
　セット数が1以上の時はセット数も表示します。
・攻撃判定　nBA
　ダイス数nで判定ロールを行い、攻撃値と命中部位も表示します。
・クリティカルコール　nBKCt　nBACt
　判定コマンドの後ろに「Ct」を付けるとクリティカルコールです。
　ダイス数n,コール数tで判定ロールを行います。
　ダイス数nで判定ロールを行います。
　セット数が1以上の時はセット数も表示し、攻撃判定の場合は命中部位も表示します。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+BK", r"\d+BA", r"\d+BKC", r"\d+BAC", r"\d+R6"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `/(\d+)BKC(\d)/`。
fn bkc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)BKC(\d)").expect("valid regex"))
}

/// Ruby `/(\d+)BAC(\d)/`。
fn bac_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)BAC(\d)").expect("valid regex"))
}

/// Ruby `/(\d+)BK/`。
fn bk_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)BK").expect("valid regex"))
}

/// Ruby `/(\d+)BA/`。
fn ba_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)BA").expect("valid regex"))
}

/// Ruby `/(^|\s)S?((\d+)[rR]6(\[([,\d]+)\])?)(\s|$)/i`。
fn r6_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)S?((\d+)R6(\[([,\d]+)\])?)(\s|$)").expect("valid regex")
    })
}

/// Ruby `BarnaKronika#replace_text`。
///
/// `BKC` / `BAC` を先に置換しないと `BK` / `BA` が食い込む。
fn replace_text(string: &str) -> String {
    let s = bkc_pattern().replace_all(string, "${1}R6[0,${2}]");
    let s = bac_pattern().replace_all(&s, "${1}R6[1,${2}]");
    let s = bk_pattern().replace_all(&s, "${1}R6[0,0]");
    ba_pattern().replace_all(&s, "${1}R6[1,0]").into_owned()
}

/// Ruby `BarnaKronika#eval_game_system_specific_command`。
fn eval_specific_command(
    string: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let string = replace_text(string);
    let Some(caps) = r6_pattern().captures(&string) else {
        return Ok(None);
    };

    let matched = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_owned();
    let option = caps.get(5).map(|m| m.as_str());
    let dice_n: i64 = caps
        .get(3)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(1);

    let mut is_battle_mode = false;
    let mut critical_call_dice: i64 = 0;
    if let Some(option) = option {
        let parts: Vec<i64> = option.split(',').map(|p| p.parse().unwrap_or(0)).collect();
        is_battle_mode = parts.first().copied().unwrap_or(0) == 1;
        critical_call_dice = parts.get(1).copied().unwrap_or(0);
    }

    let (dice_str, suc, set, at_str) =
        roll_barna_kronika(dice_n, critical_call_dice, is_battle_mode, rng)?;

    let mut output = format!("({matched}) ＞ [{dice_str}] ＞ ");
    if is_battle_mode {
        output.push_str(&at_str);
    } else {
        if suc > 1 {
            output.push_str(&format!("成功数{suc}"));
        } else {
            output.push_str("失敗");
        }
        if set > 0 {
            output.push_str(&format!(",セット{set}"));
        }
    }

    Ok(Some(SpecificCommandOutput::text(output)))
}

/// 命中部位表。Ruby `getAttackHitLocation`。
fn attack_hit_location(num: i64) -> &'static str {
    match num {
        1 => "頭部",
        2 => "右腕",
        3 => "左腕",
        4 => "右脚",
        5 => "左脚",
        6 => "胴体",
        _ => "1",
    }
}

/// Ruby `BarnaKronika#roll_barna_kronika`。
fn roll_barna_kronika(
    dice_n: i64,
    critical_call_dice: i64,
    is_battle_mode: bool,
    rng: &mut Randomizer,
) -> Result<(String, i64, i64, String), EvalError> {
    let mut suc: i64 = 0;
    let mut set: i64 = 0;
    let mut dice_count_list = [0i64; 6];

    for _ in 0..dice_n {
        let index = rng.roll_index(6)?;
        if let Ok(i) = usize::try_from(index) {
            if let Some(slot) = dice_count_list.get_mut(i) {
                *slot += 1;
                if *slot > suc {
                    suc = *slot;
                }
            }
        }
    }

    let mut output = String::new();
    let mut at_str = String::new();

    for (i, &dice_count) in dice_count_list.iter().enumerate() {
        if dice_count == 0 {
            continue;
        }
        for _ in 0..dice_count {
            output.push_str(&(i + 1).to_string());
            output.push(',');
        }

        let is_critical_call =
            is_battle_mode && critical_call_dice != 0 && critical_call_dice == (i as i64 + 1);
        let is_normal_attack = is_battle_mode && critical_call_dice == 0 && dice_count > 1;

        if is_critical_call {
            let hit = attack_hit_location(i as i64 + 1);
            at_str.push_str(&format!("{hit}:攻撃値{},", dice_count * 2));
        } else if is_normal_attack {
            let hit = attack_hit_location(i as i64 + 1);
            at_str.push_str(&format!("{hit}:攻撃値{dice_count},"));
        }

        if dice_count > 1 {
            set += 1;
        }
    }

    if critical_call_dice != 0 {
        let idx = critical_call_dice - 1;
        let c_cnt = usize::try_from(idx)
            .ok()
            .and_then(|i| dice_count_list.get(i).copied())
            .unwrap_or(0);
        suc = c_cnt * 2;
        set = if c_cnt != 0 { 1 } else { 0 };
    }

    if is_battle_mode && suc < 2 {
        at_str = "失敗".to_owned();
    }

    if output.ends_with(',') {
        output.pop();
    }
    if at_str.ends_with(',') {
        at_str.pop();
    }

    Ok((output, suc, set, at_str))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "BarnaKronika",
            "BarnaKronika.toml",
            120,
        );
    }
}
