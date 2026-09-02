//! P4で手書き移植した `lib/bcdice/game_system/Revulture.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Revulture#eval_game_system_specific_command` → `#roll_attack`（アタック判定 `xAT`）
//! - `#make_command_text` / `#calc_damage` / `#parse_additional_damage_rules` /
//!   `#make_additional_damage_condition`
//!
//! # 定型文
//!
//! Ruby側は `I18n.t("Revulture.…", locale:)` で `i18n/Revulture/ja_jp.yml` から引く。
//! Rust側は同じ値を `static` として直接持ち、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`Revulture_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `Revulture_Korean < Revulture` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::int_helpers::int_clamp;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// ロケールごとの定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の定型文。`Revulture` と `Revulture_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `Revulture.error.no_dice`
    pub(crate) no_dice: &'static str,
    /// i18n `Revulture.error.no_border`
    pub(crate) no_border: &'static str,
    /// i18n `Revulture.critical`（`%<count>d` を数で置換する）
    pub(crate) critical: &'static str,
    /// i18n `Revulture.hit_count`（同上）
    pub(crate) hit_count: &'static str,
    /// i18n `Revulture.damage`（同上）
    pub(crate) damage: &'static str,
}

/// i18n の `%<count>d` 置換。
fn interpolate_count(template: &str, count: i64) -> String {
    template.replace("%<count>d", &count.to_string())
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Revulture::ATTACK_ROLL_REG`。
///
/// `%r{^(\d+([+/]\d+)*)?AT(TACK|K)?(<=([1-6](\+\d)*))?((\[>?=\d+:\+\d+\])+)?}i`
/// 末尾はアンカーされていないので、原典どおり `\z` を付けない。
fn attack_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\A(\d+([+/]\d+)*)?AT(TACK|K)?(<=([1-6](\+\d)*))?((\[>?=\d+:\+\d+\])+)?")
            .expect("valid regex")
    })
}

/// Ruby `source.scan(/\[(>?=)(\d+):\+(\d+)\]/)`。
fn additional_damage_rule_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[(>?=)(\d+):\+(\d+)\]").expect("valid regex"))
}

/// Ruby `Revulture#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = attack_roll_pattern().captures(command) else {
        return Ok(None);
    };
    roll_attack(
        sys,
        m.get(1).map(|x| x.as_str()),
        m.get(5).map(|x| x.as_str()),
        m.get(7).map(|x| x.as_str()),
        rng,
    )
}

/// Ruby `Revulture#roll_attack`。
fn roll_attack(
    sys: &SystemTables,
    dice_count_expression: Option<&str>,
    border_expression: Option<&str>,
    additional_damage_rules: Option<&str>,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // 接頭辞 `\d+([+/]\d+)*AT` にマッチした入力しかここへ来ないので、
    // ダイス数の式は必ずある（Ruby も nil なら `Arithmetic.eval` でクラッシュする）。
    let Some(dice_count_expression) = dice_count_expression else {
        return Ok(None);
    };
    // Ruby は `Arithmetic.eval` が nil（ゼロ除算）を返すと `nil <= 0` でクラッシュする。
    // 本移植は他のコマンドと同じく「解釈できないコマンド＝nil」に畳む。
    let Some(dice_count) = arithmetic::eval(dice_count_expression, RoundType::Floor)? else {
        return Ok(None);
    };

    let border = match border_expression {
        // Ruby: Arithmetic.eval(border_expression, FLOOR).clamp(1, 6)
        Some(expr) => match arithmetic::eval(expr, RoundType::Floor)? {
            Some(value) => Some(int_clamp(&value, &I::ONE, &I::from(6))),
            None => return Ok(None),
        },
        None => None,
    };

    let command = make_command_text(
        sat_i64(&dice_count),
        border.as_ref().map(sat_i64),
        additional_damage_rules,
    );

    if dice_count <= I::ZERO {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "{command} ＞ {}",
            sys.no_dice
        ))));
    } else if border.is_none() && additional_damage_rules.is_some() {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "{command} ＞ {}",
            sys.no_border
        ))));
    }

    let mut dices = rng.roll_barabara(crate::randomizer::sat_i64(&dice_count), 6)?;
    dices.sort_unstable();

    let critical_hit_count = dices.iter().filter(|d| **d == 1).count() as i64;
    // Ruby: hit_count = dices.count { |dice| dice <= border } + critical_hit_count if border
    //       出目1は「目標値以下」とクリティカルの両方で数えられる（原典どおり）。
    let hit_count = border.map(|border| {
        dices
            .iter()
            .filter(|d| **d <= crate::randomizer::sat_i64(&border))
            .count() as i64
            + critical_hit_count
    });
    let damage = calc_damage(hit_count, additional_damage_rules);

    let mut message_elements = vec![
        command,
        dices
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(","),
    ];
    if critical_hit_count > 0 {
        message_elements.push(interpolate_count(sys.critical, critical_hit_count));
    }
    if let Some(hit_count) = hit_count {
        message_elements.push(interpolate_count(sys.hit_count, hit_count));
    }
    // Ruby: `if damage` は nil 判定なので、ダメージ0でも表示する
    if let Some(damage) = damage {
        message_elements.push(interpolate_count(sys.damage, damage));
    }

    // Ruby: Result.new(text).tap { r.condition = hit_count > 0 if hit_count; r.critical = ... }
    //       `critical=` は `Result.critical` と違って success を立てない。
    let mut result = EvalResult::with_text(message_elements.join(" ＞ "));
    if let Some(hit_count) = hit_count {
        result.set_condition(hit_count > 0);
    }
    result.critical = critical_hit_count > 0;

    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `Revulture#make_command_text`。表記は小文字の `attack`。
fn make_command_text(
    dice_count: i64,
    border: Option<i64>,
    additional_damage_rules: Option<&str>,
) -> String {
    let mut command = format!("{dice_count}attack");
    if let Some(border) = border {
        command.push_str(&format!("<={border}"));
    }
    if let Some(rules) = additional_damage_rules {
        command.push_str(rules);
    }
    format!("({command})")
}

/// Ruby `Revulture#calc_damage`。
fn calc_damage(hit_count: Option<i64>, additional_damage_rules: Option<&str>) -> Option<i64> {
    // Ruby: return nil unless additional_damage_rules
    let rules = additional_damage_rules?;
    // 追加ダメージ規則があるなら目標値も必ずある（無い場合は上で早期リターンしている）。
    let hit_count = hit_count?;

    let mut damage = hit_count;
    for (condition, additional_damage) in parse_additional_damage_rules(rules) {
        if condition.matches(hit_count) {
            damage = damage.saturating_add(additional_damage);
        }
    }
    Some(damage)
}

/// Ruby `Revulture#make_additional_damage_condition` が返すラムダ。
enum DamageCondition {
    /// `=a`（ヒット数が a に等しい）
    Equal(i64),
    /// `>=a`（ヒット数が a 以上）
    GreaterEqual(i64),
}

impl DamageCondition {
    fn matches(&self, hit_count: i64) -> bool {
        match *self {
            DamageCondition::Equal(target) => hit_count == target,
            DamageCondition::GreaterEqual(target) => hit_count >= target,
        }
    }
}

/// Ruby `Revulture#parse_additional_damage_rules`。
fn parse_additional_damage_rules(source: &str) -> Vec<(DamageCondition, i64)> {
    additional_damage_rule_pattern()
        .captures_iter(source)
        .filter_map(|m| {
            let target = to_i(&m[2]);
            let condition = match &m[1] {
                "=" => DamageCondition::Equal(target),
                ">=" => DamageCondition::GreaterEqual(target),
                // Ruby: case が漏れると nil（＝ラムダなし）になる。正規表現上は起きない。
                _ => return None,
            };
            Some((condition, to_i(&m[3])))
        })
        .collect()
}

/// Ruby の `String#to_i`（ここに来るのは `\d+` にマッチした文字列だけ）。
///
/// 桁あふれは Ruby だと Bignum になるので、`i64` に収まらない場合は飽和させる。
/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの定型文
// ---------------------------------------------------------------------------

/// `ja_jp` ロケールの定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    no_dice: "ダイス数が 0 です",
    no_border: "目標値が指定されていないため、追加ダメージを算出できません",
    critical: "クリティカル %<count>d",
    hit_count: "ヒット数 %<count>d",
    damage: "ダメージ %<count>d",
};

/// Ruby `BCDice::GameSystem::Revulture`（ID: `Revulture`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revulture;

impl GameSystem for Revulture {
    fn id(&self) -> &'static str {
        "Revulture"
    }

    fn name(&self) -> &'static str {
        "光砕のリヴァルチャー"
    }

    fn sort_key(&self) -> &'static str {
        "こうさいのりうあるちやあ"
    }

    fn help_message(&self) -> &'static str {
        r"■アタック判定（ xAT, xATK, xATTACK ）
x: ダイス数（加算 + と除算 / を使用可能）
例） 3AT, 4ATK, 5+6ATTACK, 15/2AT

□アタック判定　目標値つき（ xAT<=y, xATK<=y, xATTACK<=y ）
x: ダイス数（加算 + と除算 / を使用可能）
y: 目標値（ 1 以上 6 以下。加算 + を使用可能）
例） 3AT<=4, 3AT<=2+1

□アタック判定　目標値＆追加ダメージつき（ xAT<=y[>=a:+b], xATK<=y[>=a:+b], xATTACK<=y[z] ）
x: ダイス数（加算 + と除算 / を使用可能）
y: 目標値（ 1 以上 6 以下。加算 + を使用可能）
z: 追加ダメージの規則（詳細は後述）（※複数同時に指定可能）

▽追加ダメージの規則 [a:+b]
a: ヒット数が a なら
　=a　（ヒット数が a に等しい）
　>=a　（ヒット数が a 以上）
b: ダメージを b 点追加

例） 3AT<=4[>=2:+3] #ルールブックp056「グレングラントAR」
例） 2AT<=4[=1:+5][>=2:+8] #ルールブックp067「ファーボル・ドラゴンブレス」
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+([+\/]\d+)*AT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Revulture#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Revulture",
            "Revulture.toml",
            38,
        );
    }
}
