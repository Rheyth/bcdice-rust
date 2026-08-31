//! P4で手書き移植した `lib/bcdice/game_system/NegikureNegimaki.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NegikureNegimaki#eval_game_system_specific_command`
//!   （行為判定 `nNNx#y` / アタック判定 `nNAx#y` / ストライク判定 `nNS`）
//!
//! 定型文は Ruby が `translate("NegikureNegimaki.…")` で組み立てる。ここでは `ja_jp` の値を
//! `static` に持ち、`NegikureNegimaki_Korean` が `ko_kr` を差し替えられるよう
//! [`SystemTables`] に束ねた。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// 1ロケール分の定型文。`NegikureNegimaki` と `NegikureNegimaki_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `NegikureNegimaki.result_level`（`%{success_level}` / `%{required_level}`）
    pub(crate) result_level: &'static str,
    /// i18n `NegikureNegimaki.success_level`（`%{success_level}`）
    pub(crate) success_level: &'static str,
    /// i18n `NegikureNegimaki.damage`（`%{normal_damage}` / `%{direct_damage}`）
    pub(crate) damage: &'static str,
    /// i18n `NegikureNegimaki.guts_loss`（`%{guts_loss}`）
    pub(crate) guts_loss: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

/// i18n `ja_jp` の定型文。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    result_level: "成功レベル%{success_level}/要求%{required_level}",
    success_level: "成功レベル%{success_level}",
    damage: "通常ダメージ%{normal_damage}/直撃ダメージ%{direct_damage}",
    guts_loss: "ガッツ減少%{guts_loss}",
    success: "成功",
    failure: "失敗",
};

/// Ruby `I18n` の `%{name}` を数値に置換する。
fn interpolate(template: &str, values: &[(&str, i64)]) -> String {
    let mut text = template.to_owned();
    for (name, value) in values {
        text = text.replace(&format!("%{{{name}}}"), &value.to_string());
    }
    text
}

/// Ruby `/\A(\d+)?NN(\d+)?(?:#(\d+))?\z/i`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?NN(\d+)?(?:#(\d+))?$").expect("valid regex"))
}

/// Ruby `/\A(\d+)?NA(\d+)?(?:#(\d+))?\z/i`。
fn attack_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?NA(\d+)?(?:#(\d+))?$").expect("valid regex"))
}

/// Ruby `/\A(\d+)?NS\z/i`。
fn guts_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?NS$").expect("valid regex"))
}

/// Ruby `m[n]&.to_i`。
fn capture_int(caps: &Captures<'_>, index: usize) -> Option<i64> {
    caps.get(index)
        .map(|m| m.as_str().parse().unwrap_or(i64::MAX))
}

/// Ruby `"[#{dice_list.join(',')}]"`。
fn detail_text(dice_list: &[i64]) -> String {
    let joined = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{joined}]")
}

/// Ruby `NegikureNegimaki#eval_action_command`。
fn eval_action_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = capture_int(&caps, 1).unwrap_or(1);
    let difficulty = capture_int(&caps, 2).unwrap_or(4);
    // Ruby: [m[3]&.to_i || 1, 1].max
    let required_level = capture_int(&caps, 3).unwrap_or(1).max(1);

    if dice_count == 0 {
        return Ok(None);
    }

    let command_text = format!("({dice_count}NN{difficulty}#{required_level})");

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let success_level = dice_list.iter().filter(|&&v| v >= difficulty).count() as i64;
    let detail_text = detail_text(&dice_list);

    Ok(Some(build_result(
        sys,
        &command_text,
        &detail_text,
        success_level,
        required_level,
    )))
}

/// Ruby `NegikureNegimaki#eval_attack_command`。
fn eval_attack_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = attack_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = capture_int(&caps, 1).unwrap_or(1);
    let difficulty = capture_int(&caps, 2).unwrap_or(4);
    // Ruby: [m[3]&.to_i || 6, 1].max
    let critical_value = capture_int(&caps, 3).unwrap_or(6).max(1);

    if dice_count == 0 {
        return Ok(None);
    }

    let command_text = format!("({dice_count}NA{difficulty}#{critical_value})");

    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let success_level = dice_list.iter().filter(|&&v| v >= difficulty).count() as i64;
    let direct_damage = dice_list
        .iter()
        .filter(|&&v| v >= difficulty && v >= critical_value)
        .count() as i64;
    let guts_loss = dice_list.iter().filter(|&&v| v == 1).count() as i64;
    let detail_text = detail_text(&dice_list);

    Ok(Some(build_attack_result(
        sys,
        &command_text,
        &detail_text,
        success_level,
        direct_damage,
        guts_loss,
    )))
}

/// Ruby `NegikureNegimaki#eval_guts_command`。
fn eval_guts_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = guts_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = capture_int(&caps, 1).unwrap_or(1);
    if dice_count == 0 {
        return Ok(None);
    }

    let command_text = format!("({dice_count}NS)");
    let dice_list = rng.roll_barabara(dice_count, 6)?;
    let guts_loss = dice_list.iter().filter(|&&v| v == 1).count() as i64;
    let detail_text = detail_text(&dice_list);

    Ok(Some(build_guts_result(
        sys,
        &command_text,
        &detail_text,
        guts_loss,
    )))
}

/// Ruby `NegikureNegimaki#build_result`。
fn build_result(
    sys: &SystemTables,
    command_text: &str,
    detail_text: &str,
    success_level: i64,
    required_level: i64,
) -> EvalResult {
    let success = success_level >= required_level;
    let judge_text = if success { sys.success } else { sys.failure };
    let result_level_text = interpolate(
        sys.result_level,
        &[
            ("success_level", success_level),
            ("required_level", required_level),
        ],
    );
    let text = format!("{command_text} ＞ {detail_text} ＞ {result_level_text} ＞ {judge_text}");

    if success {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }
}

/// Ruby `NegikureNegimaki#build_attack_result`。
fn build_attack_result(
    sys: &SystemTables,
    command_text: &str,
    detail_text: &str,
    success_level: i64,
    direct_damage: i64,
    guts_loss: i64,
) -> EvalResult {
    // Ruby: [success_level - direct_damage, 0].max
    let normal_damage = (success_level - direct_damage).max(0);
    let success = success_level > 0;
    let mut damage_text = interpolate(
        sys.damage,
        &[
            ("normal_damage", normal_damage),
            ("direct_damage", direct_damage),
        ],
    );
    if guts_loss > 0 {
        damage_text.push('/');
        damage_text.push_str(&interpolate(sys.guts_loss, &[("guts_loss", guts_loss)]));
    }
    let success_level_text = interpolate(sys.success_level, &[("success_level", success_level)]);
    let text = format!("{command_text} ＞ {detail_text} ＞ {success_level_text} ＞ {damage_text}");

    if success {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }
}

/// Ruby `NegikureNegimaki#build_guts_result`。
fn build_guts_result(
    sys: &SystemTables,
    command_text: &str,
    detail_text: &str,
    guts_loss: i64,
) -> EvalResult {
    let success = guts_loss == 0;
    let guts_loss_text = interpolate(sys.guts_loss, &[("guts_loss", guts_loss)]);
    let text = format!("{command_text} ＞ {detail_text} ＞ {guts_loss_text}");

    if success {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }
}

/// Ruby `NegikureNegimaki#eval_game_system_specific_command`。
///
/// Ruby: `eval_action_command(command) || eval_attack_command(command) || eval_guts_command(command)`
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = eval_action_command(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = eval_attack_command(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(eval_guts_command(sys, command, rng)?.map(SpecificCommandOutput::result))
}

/// Ruby `BCDice::GameSystem::NegikureNegimaki`（ID: `NegikureNegimaki`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegikureNegimaki;

impl GameSystem for NegikureNegimaki {
    fn id(&self) -> &'static str {
        "NegikureNegimaki"
    }

    fn name(&self) -> &'static str {
        "ネジクレネジマキ"
    }

    fn sort_key(&self) -> &'static str {
        "ねしくれねしまき"
    }

    fn help_message(&self) -> &'static str {
        r"■ 行為判定
nNNx#y: n個のD6を振り、x以上の出目の個数を成功レベルとして判定する
n: ダイス数（省略時1）
x: 難易度（省略時4）
y: 要求成功レベル（省略時1、0は1として扱う）

■ 戦闘判定（アタック判定）
nNAx#y: n個のD6を振り、x以上を成功とする。y以上の成功は直撃ダメージになる
n: ダイス数（省略時1）
x: 難易度（省略時4）
y: クリティカル値（省略時6、0は1として扱う）
通常ダメージ = 成功レベル - 直撃ダメージ
直撃ダメージ = 成功した出目のうち y 以上の個数
ガッツ減少 = 出目 1 の個数

■ ストライクの判定
nNS: n個のD6を振り、出目 1 の個数だけガッツ減少を算出する
n: ダイス数（省略時1）
ガッツ減少が 0 なら成功、1 以上なら失敗
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*NN\d*(#\d+)?", r"\d*NA\d*(#\d+)?", r"\d*NS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `NegikureNegimaki#eval_game_system_specific_command`。
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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/NegikureNegimaki.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NegikureNegimaki.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/NegikureNegimaki.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NegikureNegimaki.toml must parse");
        assert_eq!(
            data.tests.len(),
            26,
            "case count in test/data/NegikureNegimaki.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NegikureNegimaki",
                "unexpected game system in NegikureNegimaki.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("NegikureNegimaki"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL NegikureNegimaki:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NegikureNegimaki cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
