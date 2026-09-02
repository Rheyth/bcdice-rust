//! P4で手書き移植した `lib/bcdice/game_system/Alsetto.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Alsetto#eval_game_system_specific_command` → `check_roll`（`nAL[C|G][m][*p]`）
//!
//! 定型文は Ruby が `translate("Alsetto.…")` で組み立てる。ここでは `ja_jp` の値を
//! `static` に持ち、`Alsetto_Korean` が `ko_kr` を差し替えられるよう
//! [`SystemTables`] に束ねた。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// 1ロケール分の定型文。`Alsetto` と `Alsetto_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `Alsetto.damage`（`%{total_damage}` を数値に置換する）
    pub(crate) damage: &'static str,
    /// i18n `Alsetto.success_count`（`%{success_count}` を置換する）
    pub(crate) success_count: &'static str,
    /// i18n `Alsetto.triumph`（`%{critical_count}` を置換する。先頭の区切り ` / ` を含む）
    pub(crate) triumph: &'static str,
}

/// i18n `ja_jp` の定型文。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    damage: "%{total_damage}ダメージ",
    success_count: "成功数：%{success_count}",
    triumph: " / %{critical_count}トライアンフ",
};

/// Ruby `I18n` の `%{name}` を数値に置換する。
fn interpolate(template: &str, name: &str, value: i64) -> String {
    template.replace(&format!("%{{{name}}}"), &value.to_string())
}

/// Ruby `Alsetto#parse_check_roll` の正規表現。
///
/// Ruby: `/(\d+)AL(C|G)?(\d+)?((x|\*)(\d+))?$/i`
fn check_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)AL(C|G)?(\d+)?((x|\*)(\d+))?$").expect("valid regex"))
}

struct ParsedRoll {
    rapid: i64,
    enable_critical: bool,
    critical_number: i64,
    target: i64,
    damage: i64,
}

/// Ruby `Alsetto#parse_check_roll`。
fn parse_check_roll(command: &str) -> Option<ParsedRoll> {
    let captures = check_roll_pattern().captures(command)?;

    let rapid: i64 = captures
        .get(1)
        .map_or(0, |m| m.as_str().parse().unwrap_or(0));
    // Ruby: m[2] は upcase 済みの入力から取るので "C" / "G" / nil のいずれか
    let kind = captures.get(2).map(|m| m.as_str());
    let enable_critical = kind.is_none() || kind == Some("G");
    let critical_number = match kind {
        Some("G") => 2,
        Some("C") => 0,
        _ => 1,
    };
    let target: i64 = captures
        .get(3)
        .map_or(3, |m| m.as_str().parse().unwrap_or(0));
    // Ruby: m[6].to_i（nil.to_i == 0）
    let damage: i64 = captures
        .get(6)
        .map_or(0, |m| m.as_str().parse().unwrap_or(0));

    Some(ParsedRoll {
        rapid,
        enable_critical,
        critical_number,
        target,
        damage,
    })
}

/// Ruby `Alsetto#eval_game_system_specific_command` / `#check_roll`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(parsed) = parse_check_roll(command) else {
        return Ok(None);
    };

    let mut total_success_count = 0i64;
    let mut total_critical_count = 0i64;
    let mut text = String::new();
    let mut roll_count = parsed.rapid;

    while roll_count > 0 {
        let mut dice_array = rng.roll_barabara(roll_count, 6)?;
        dice_array.sort_unstable();
        let dice_text = join_dice(&dice_array);

        let success_count = dice_array.iter().filter(|&&v| v <= parsed.target).count() as i64;
        let critical_count = dice_array
            .iter()
            .filter(|&&v| v <= parsed.critical_number)
            .count() as i64;
        total_success_count += success_count;
        // Ruby: total_critical_count += 1 if critical_count > 0（振り足しが起きた回数を数える）
        if critical_count > 0 {
            total_critical_count += 1;
        }

        if !text.is_empty() {
            text.push('+');
        }
        text.push_str(&format!("{success_count}[{dice_text}]"));

        if !parsed.enable_critical {
            break;
        }
        roll_count = critical_count;
    }

    let is_damage = parsed.damage != 0;
    let mut result = if is_damage {
        let total_damage = total_success_count * parsed.damage;
        let damage_text = interpolate(sys.damage, "total_damage", total_damage);
        format!(
            "({}D6<={}) ＞ {text} ＞ Hits：{}*{} ＞ {damage_text}",
            parsed.rapid, parsed.target, total_success_count, parsed.damage
        )
    } else {
        let success_text = interpolate(sys.success_count, "success_count", total_success_count);
        format!(
            "({}D6<={}) ＞ {text} ＞ {success_text}",
            parsed.rapid, parsed.target
        )
    };

    if parsed.enable_critical {
        result.push_str(&interpolate(
            sys.triumph,
            "critical_count",
            total_critical_count,
        ));
    }

    Ok(Some(SpecificCommandOutput::text(result)))
}

/// Ruby `dice_array.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::Alsetto`（ID: `Alsetto`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alsetto;

impl GameSystem for Alsetto {
    fn id(&self) -> &'static str {
        "Alsetto"
    }

    fn name(&self) -> &'static str {
        "詩片のアルセット"
    }

    fn sort_key(&self) -> &'static str {
        "うたかたのあるせつと"
    }

    fn help_message(&self) -> &'static str {
        r"・成功判定：nAL[m]　　　　・トライアンフ無し：nALC[m]
・命中判定：nAL[m]*p　　　・トライアンフ無し：nALC[m]*p
・命中判定（ガンスリンガーの根源詩）：nALG[m]*p
[]内は省略可能。

ALコマンドはトライアンフの分だけ、自動で振り足し処理を行います。
「n」でダイス数を指定。
「m」で目標値を指定。省略時は、デフォルトの「3」が使用されます。
「p」で攻撃力を指定。「*」は「x」でも可。
攻撃力指定で命中判定となり、成功数ではなく、ダメージを結果表示します。

ALCコマンドはトライアンフ無しで、成功数、ダメージを結果表示します。
ALGコマンドは「2以下」でトライアンフ処理を行います。

【書式例】
・5AL → 5d6で目標値3。
・5ALC → 5d6で目標値3。トライアンフ無し。
・6AL2 → 6d6で目標値2。
・4AL*5 → 4d6で目標値3、攻撃力5の命中判定。
・7AL2x10 → 7d6で目標値2、攻撃力10の命中判定。
・8ALC4x5 → 8d6で目標値4、攻撃力5、トライアンフ無しの命中判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+AL[CG]?"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Alsetto#eval_game_system_specific_command`。
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
        crate::game_system::test_support::assert_toml_cases_strict("Alsetto", "Alsetto.toml", 25);
    }
}
