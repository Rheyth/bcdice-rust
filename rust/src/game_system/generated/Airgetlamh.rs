//! P4で手書き移植した `lib/bcdice/game_system/Airgetlamh.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Airgetlamh#eval_game_system_specific_command` → `check_roll`（`[n]AA[m]*p[+t][Cx]` / `AL`）
//!
//! 定型文は Ruby が `I18n.t("Airgetlamh.…")` で組み立てる。ここでは `ja_jp` の値を
//! `static` に持ち、`Airgetlamh_Korean` が `ko_kr` を差し替えられるよう
//! [`SystemTables`] に束ねた。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// 1ロケール分の定型文。`Airgetlamh` と `Airgetlamh_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `Airgetlamh.damage`（`%<count>d` を数値に置換する）
    pub(crate) damage: &'static str,
    /// i18n `Airgetlamh.success_count`
    pub(crate) success_count: &'static str,
    /// i18n `Airgetlamh.critical`
    pub(crate) critical: &'static str,
}

/// i18n `ja_jp` の定型文。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    damage: "%<count>dダメージ",
    success_count: "成功数：%<count>d",
    critical: "%<count>dクリティカル",
};

/// Ruby `I18n` の `%<count>d` を置換する。
fn format_count(template: &str, count: i64) -> String {
    template.replace("%<count>d", &count.to_string())
}

/// Ruby `Airgetlamh#parse_check_roll` の正規表現。
///
/// Ruby: `/(\d+)?A(A|L)(\d+)?(?:[X*](\d+)(?:\+(\d+))?)?(?:C(\d+))?$/i`
fn check_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(\d+)?A(A|L)(\d+)?(?:[X*](\d+)(?:\+(\d+))?)?(?:C(\d+))?$")
            .expect("valid regex")
    })
}

struct ParsedRoll {
    dice_count: i64,
    target: i64,
    damage: i64,
    critical_trigger: i64,
    critical_number: i64,
}

/// Ruby `Airgetlamh#parse_check_roll`。
fn parse_check_roll(command: &str) -> Option<ParsedRoll> {
    let captures = check_roll_pattern().captures(command)?;

    let dice_count = captures
        .get(1)
        .map(|m| m.as_str().parse().unwrap_or(0))
        .unwrap_or(2);
    let target = captures
        .get(3)
        .map(|m| m.as_str().parse().unwrap_or(0))
        .unwrap_or(6);
    let damage = captures
        .get(4)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let mut critical_trigger = captures
        .get(5)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let mut critical_number = captures
        .get(6)
        .map(|m| m.as_str().parse().unwrap_or(0))
        .unwrap_or(1);

    // Ruby: m[2] == "L"（upcased 後なので "L" 固定）
    if captures.get(2).map(|m| m.as_str()) == Some("L") {
        critical_trigger = 0;
        critical_number = 0;
    } else if critical_number > 4 {
        critical_number = 3;
    }

    Some(ParsedRoll {
        dice_count,
        target,
        damage,
        critical_trigger,
        critical_number,
    })
}

/// Ruby `Airgetlamh#eval_game_system_specific_command` / `#check_roll`。
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
    let mut roll_count = parsed.dice_count;

    while roll_count > 0 {
        let mut dice_array = rng.roll_barabara(roll_count, 10)?;
        dice_array.sort_unstable();
        let dice_text = dice_text::join_dice(&dice_array);

        let success_count = dice_array.iter().filter(|&&i| i <= parsed.target).count() as i64;
        let critical_count = dice_array
            .iter()
            .filter(|&&i| i <= parsed.critical_number)
            .count() as i64;

        total_success_count += success_count;
        total_critical_count += critical_count;

        if !text.is_empty() {
            text.push('+');
        }
        text.push_str(&format!("{success_count}[{dice_text}]"));

        roll_count = critical_count;
    }

    let is_damage = parsed.damage != 0;
    let mut result = if is_damage {
        let total_damage =
            total_success_count * parsed.damage + total_critical_count * parsed.critical_trigger;
        let mut s = format!(
            "({}D10<={}) ＞ {text} ＞ Hits：{}*{}",
            parsed.dice_count, parsed.target, total_success_count, parsed.damage
        );
        if parsed.critical_trigger > 0 {
            s.push_str(&format!(
                " + Trigger：{}*{}",
                total_critical_count, parsed.critical_trigger
            ));
        }
        s.push_str(" ＞ ");
        s.push_str(&format_count(sys.damage, total_damage));
        s
    } else {
        format!(
            "({}D10<={}) ＞ {text} ＞ {}",
            parsed.dice_count,
            parsed.target,
            format_count(sys.success_count, total_success_count)
        )
    };

    if total_critical_count > 0 {
        result.push_str(" / ");
        result.push_str(&format_count(sys.critical, total_critical_count));
    }

    Ok(Some(SpecificCommandOutput::text(result)))
}

/// Ruby `BCDice::GameSystem::Airgetlamh`（ID: `Airgetlamh`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Airgetlamh;

impl GameSystem for Airgetlamh {
    fn id(&self) -> &'static str {
        "Airgetlamh"
    }

    fn name(&self) -> &'static str {
        "朱の孤塔のエアゲトラム"
    }

    fn sort_key(&self) -> &'static str {
        "あけのことうのえあけとらむ"
    }

    fn help_message(&self) -> &'static str {
        r"【Reg2.0『THE ANSWERER』～】
・調査判定（成功数を表示）：[n]AA[m]
・命中判定（ダメージ表示）：[n]AA[m]*p[+t][Cx]
【～Reg1.1『昇華』】
・調査判定（成功数を表示）：[n]AL[m]
・命中判定（ダメージ表示）：[n]AL[m]*p
----------------------------------------
[]内のコマンドは省略可能。

「n」でダイス数（攻撃回数）を指定。省略時は「2」。
「m」で目標値を指定。省略時は「6」。
「p」で威力を指定。「*」は「x」で代用可。
「+t」でクリティカルトリガーを指定。省略可。
「Cx」でクリティカル値を指定。省略時は「1」、最大値は「3」、「0」でクリティカル無し。

攻撃力指定で命中判定となり、成功数ではなく、ダメージを結果表示します。
クリティカルヒットの分だけ、自動で振り足し処理を行います。
（ALコマンドではクリティカル処理を行いません）

【書式例】
・AL → 2d10で目標値6の調査判定。
・5AA7*12 → 5d10で目標値7、威力12の命中判定。
・AA7x28+5 → 2d10で目標値7、威力28、クリティカルトリガー5の命中判定。
・9aa5*10C2 → 9d10で目標値5、威力10、クリティカル値2の命中判定。
・15AAx4c0 → 15d10で目標値6、威力4、クリティカル無しの命中判定。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*A[AL]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Airgetlamh#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Airgetlamh#eval_game_system_specific_command`。
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
            "Airgetlamh",
            "Airgetlamh.toml",
            29,
        );
    }
}
