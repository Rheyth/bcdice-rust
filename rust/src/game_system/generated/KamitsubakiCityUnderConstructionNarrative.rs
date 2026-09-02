//! P4で手書き移植した `lib/bcdice/game_system/KamitsubakiCityUnderConstructionNarrative.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KamitsubakiCityUnderConstructionNarrative#eval_game_system_specific_command`
//!   → `roll_kumi`（組ダイス `KA*` / `RI*` / `HA*` / `SE*` / `CO*` / `GM*` / `Q12`）
//!   と `roll_existence`（存在証明 `EXI<=x`）
//! - 内部クラス `KumiDice` / `KumiD6` / `QDice`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `KumiDice::CRITICAL` / `QDice::CRITICAL`。
const CRITICAL: &str = "M";
/// Ruby `KumiDice::FUMBLE`。
const FUMBLE: &str = "Q";

/// Ruby `KumiD6::TABLE`。
const KUMI_D6: &[&str] = &["裏", "羽", "星", "狐", "可", "Q"];

const KA8: &[&str] = &["Q", "", "", "", "可", "可", "可", "M"];
const KA10: &[&str] = &["Q", "", "", "可", "可", "可", "可", "可", "M", "M"];
const KA12: &[&str] = &[
    "Q", "", "", "可", "可", "可", "可", "可", "可", "可", "M", "M",
];

const RI8: &[&str] = &["Q", "", "", "", "裏", "裏", "裏", "M"];
const RI10: &[&str] = &["Q", "", "", "裏", "裏", "裏", "裏", "裏", "M", "M"];
const RI12: &[&str] = &[
    "M", "M", "裏", "裏", "裏", "裏", "裏", "裏", "裏", "", "", "Q",
];

const HA8: &[&str] = &["Q", "", "", "", "羽", "羽", "羽", "M"];
const HA10: &[&str] = &["Q", "", "", "羽", "羽", "羽", "羽", "羽", "M", "M"];
const HA12: &[&str] = &["Q", "Q", "羽", "羽", "羽", "", "", "", "M", "M", "M", "M"];

const SE8: &[&str] = &["Q", "", "", "", "星", "星", "星", "M"];
const SE10: &[&str] = &["Q", "", "", "星", "星", "星", "星", "星", "M", "M"];
const SE12: &[&str] = &[
    "星", "", "星", "星", "M", "Q", "M", "星", "星", "星", "", "星",
];

const CO8: &[&str] = &["Q", "", "", "", "狐", "狐", "狐", "M"];
const CO10: &[&str] = &["Q", "", "", "狐", "狐", "狐", "狐", "狐", "M", "M"];
const CO12: &[&str] = &[
    "Q",
    "",
    "",
    "狐狐狐",
    "狐狐",
    "狐",
    "狐狐狐",
    "狐狐",
    "狐",
    "狐",
    "M",
    "M",
];

const GM8: &[&str] = &["Q", "", "", "", "W", "W", "W", "M"];
const GM10: &[&str] = &["Q", "", "", "W", "W", "W", "W", "W", "M", "M"];
const Q12: &[&str] = &["", "", "", "Q", "Q", "Q", "Q", "Q", "Q", "Q", "M", "M"];

/// Ruby `BCDice::GameSystem::KamitsubakiCityUnderConstructionNarrative`
/// （ID: `KamitsubakiCityUnderConstructionNarrative`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KamitsubakiCityUnderConstructionNarrative;

impl GameSystem for KamitsubakiCityUnderConstructionNarrative {
    fn id(&self) -> &'static str {
        "KamitsubakiCityUnderConstructionNarrative"
    }

    fn name(&self) -> &'static str {
        "神椿市建設中。NARRATIVE"
    }

    fn sort_key(&self) -> &'static str {
        "かみつはきしけんせつちゆうならていふ"
    }

    fn help_message(&self) -> &'static str {
        r"・可組（KA）
　KA6 行動判定
　KA8 技能ロール
　KA10 特技ロール
　KA12 Aロール

・裏組（RI）
　RI6 行動判定
　RI8 技能ロール
　RI10 特技ロール
　RI12 Aロール

・羽組（HA）
　HA6 行動判定
　HA8 技能ロール
　HA10 特技ロール
　HA12 Aロール

・星組（SE）
　SE6 行動判定
　SE8 技能ロール
　SE10 特技ロール
　SE12 Aロール

・狐組（CO）
　CO6 行動判定
　CO8 技能ロール
　CO10 特技ロール
　CO12 Aロール

・GM用
　GM6 （成否判定なし）
　GM8 技能ロール
　GM10 特技ロール
　Q12 Qロール

・存在証明 EXI<=x
　存在証明の判定を行う
　x: 存在値
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "EXI", "KA6", "KA8", "KA10", "KA12", "RI6", "RI8", "RI10", "RI12", "HA6", "HA8",
            "HA10", "HA12", "SE6", "SE8", "SE10", "SE12", "CO6", "CO8", "CO10", "CO12", "GM6",
            "GM8", "GM10", "Q12",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `KamitsubakiCityUnderConstructionNarrative#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_kumi(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = roll_existence(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `KamitsubakiCityUnderConstructionNarrative#roll_kumi`。
fn roll_kumi(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    match command {
        "KA6" => roll_kumi_d6(Some("可"), rng).map(Some),
        "KA8" => roll_kumi_dice(KA8, rng).map(Some),
        "KA10" => roll_kumi_dice(KA10, rng).map(Some),
        "KA12" => roll_kumi_dice(KA12, rng).map(Some),
        "RI6" => roll_kumi_d6(Some("裏"), rng).map(Some),
        "RI8" => roll_kumi_dice(RI8, rng).map(Some),
        "RI10" => roll_kumi_dice(RI10, rng).map(Some),
        "RI12" => roll_kumi_dice(RI12, rng).map(Some),
        "HA6" => roll_kumi_d6(Some("羽"), rng).map(Some),
        "HA8" => roll_kumi_dice(HA8, rng).map(Some),
        "HA10" => roll_kumi_dice(HA10, rng).map(Some),
        "HA12" => roll_kumi_dice(HA12, rng).map(Some),
        "SE6" => roll_kumi_d6(Some("星"), rng).map(Some),
        "SE8" => roll_kumi_dice(SE8, rng).map(Some),
        "SE10" => roll_kumi_dice(SE10, rng).map(Some),
        "SE12" => roll_kumi_dice(SE12, rng).map(Some),
        "CO6" => roll_kumi_d6(Some("狐"), rng).map(Some),
        "CO8" => roll_kumi_dice(CO8, rng).map(Some),
        "CO10" => roll_kumi_dice(CO10, rng).map(Some),
        "CO12" => roll_kumi_dice(CO12, rng).map(Some),
        "GM6" => roll_kumi_d6(None, rng).map(Some),
        "GM8" => roll_kumi_dice(GM8, rng).map(Some),
        "GM10" => roll_kumi_dice(GM10, rng).map(Some),
        "Q12" => roll_q_dice(Q12, rng).map(Some),
        _ => Ok(None),
    }
}

/// Ruby `KumiDice#roll`。
fn roll_kumi_dice(items: &[&str], rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let sides = items.len() as i64;
    let dice = rng.roll_once(sides)?;
    let chosen = usize::try_from(dice - 1)
        .ok()
        .and_then(|i| items.get(i).copied())
        .unwrap_or("");

    let fumble = chosen == FUMBLE;
    let critical = chosen == CRITICAL;
    let result_tail = if fumble {
        "ファンブル"
    } else if critical {
        "マジック"
    } else if !chosen.is_empty() {
        "成功"
    } else {
        "失敗"
    };

    let mut r = EvalResult::with_text(join_kumi(sides, dice, chosen, Some(result_tail)));
    r.critical = critical;
    r.fumble = fumble;
    r.set_condition(!chosen.is_empty() && !r.fumble);
    Ok(r)
}

/// Ruby `KumiD6#roll`。
fn roll_kumi_d6(
    success_symbol: Option<&str>,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let dice = rng.roll_once(6)?;
    let chosen = usize::try_from(dice - 1)
        .ok()
        .and_then(|i| KUMI_D6.get(i).copied())
        .unwrap_or("");

    let mut r = EvalResult::new();
    if let Some(symbol) = success_symbol {
        r.fumble = chosen == "Q";
        r.set_condition(chosen == symbol);
    }

    let result_tail = if r.fumble {
        Some("ファンブル")
    } else if r.success {
        Some("成功")
    } else if r.failure {
        Some("失敗")
    } else {
        None
    };

    r.text = join_kumi(6, dice, chosen, result_tail);
    Ok(r)
}

/// Ruby `QDice#roll`。
fn roll_q_dice(items: &[&str], rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let sides = items.len() as i64;
    let dice = rng.roll_once(sides)?;
    let chosen = usize::try_from(dice - 1)
        .ok()
        .and_then(|i| items.get(i).copied())
        .unwrap_or("");

    let critical = chosen == CRITICAL;
    let result_tail = if critical {
        "マジック"
    } else if !chosen.is_empty() {
        "成功"
    } else {
        "失敗"
    };

    let mut r = EvalResult::with_text(join_kumi(sides, dice, chosen, Some(result_tail)));
    r.critical = critical;
    r.set_condition(!chosen.is_empty());
    Ok(r)
}

/// Ruby `[(D#{n}), dice, chosen or nil, result_tail].compact.join(" ＞ ")`。
fn join_kumi(sides: i64, dice: i64, chosen: &str, result_tail: Option<&str>) -> String {
    let mut parts = vec![format!("(D{sides})"), dice.to_string()];
    if !chosen.is_empty() {
        parts.push(chosen.to_string());
    }
    if let Some(tail) = result_tail {
        parts.push(tail.to_string());
    }
    parts.join(" ＞ ")
}

/// Ruby `KamitsubakiCityUnderConstructionNarrative#roll_existence`。
fn existence_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^EXI<=(\d+)$").expect("valid regex"))
}

fn roll_existence(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(captures) = existence_pattern().captures(command) else {
        return Ok(None);
    };
    let target: i64 = captures[1].parse().unwrap_or(i64::MAX);
    let dice = rng.roll_once(20)?;

    let mut r = EvalResult::new();
    r.critical = dice == 1;
    r.fumble = dice == 20;
    r.set_condition((dice <= target && !r.fumble) || r.critical);

    let result_tail = if r.critical {
        "M ＞ マジック"
    } else if r.fumble {
        "Q ＞ ファンブル"
    } else if r.success {
        "成功"
    } else {
        "失敗"
    };

    r.text = format!("(D20<={target}) ＞ {dice} ＞ {result_tail}");
    Ok(Some(r))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "KamitsubakiCityUnderConstructionNarrative",
            "KamitsubakiCityUnderConstructionNarrative.toml",
            113,
        );
    }
}
