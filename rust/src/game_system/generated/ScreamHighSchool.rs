//! P4で手書き移植した `lib/bcdice/game_system/ScreamHighSchool.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `GardenOrder` を継承しているので、判定の本体
//! （`#get_critical_border` / `#check_roll_repeat_attack` / `#get_check_result`）と
//! 負傷表 `DCxxy`（`#look_up_damage_chart`）は [`super::GardenOrder`] の実装を
//! `ja_jp` の表（[`JA_TABLES`]）で使い回す。
//!
//! 移植したもの:
//! - `ScreamHighSchool#eval_game_system_specific_command`
//!   （感情/性格傾向/恐怖判定 `EMx@z` / `TRx@z` / `FEx@z`、基本判定 `SHx/y@z`、負傷表 `DCxxy`）
//! - `#check_roll_sh` / `#get_supplementary`

use std::sync::OnceLock;

use regex::Regex;

use super::GardenOrder::{
    check_roll_repeat_attack, get_check_result, get_critical_border, look_up_damage_chart, to_i,
    JA_TABLES,
};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::ScreamHighSchool`（ID: `ScreamHighSchool`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreamHighSchool;

impl GameSystem for ScreamHighSchool {
    fn id(&self) -> &'static str {
        "ScreamHighSchool"
    }

    fn name(&self) -> &'static str {
        "スクリームハイスクール"
    }

    fn sort_key(&self) -> &'static str {
        "すくりいむはいすくうる"
    }

    fn help_message(&self) -> &'static str {
        r"・基本判定
　SHx/y@z　x：成功率、y：連続攻撃回数（省略可）、z：クリティカル値（省略可）
　（連続攻撃では1回の判定のみが実施されます）
　例）SH55　SH(40-20) SH100/2　SH70@10　SH155/3@44
・感情判定
　EMx@z　x：成功率、z：クリティカル値（省略可）
　例）EM50　EM50@15
・性格傾向判定
　TRx@z　x：成功率、z：クリティカル値（省略可）
　例）TR60　TR60@15
・恐怖判定
　FEx@z　x：成功率、z：クリティカル値（省略可）
　例）FE70　FE70@15
・負傷表
　DCxxy
　xx：属性（切断：SL，銃弾：BL，衝撃：IM，灼熱：BR，冷却：RF，電撃：EL）
　y：ダメージ
　例）DCSL7　DCEL22
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["(SH|SHS)", "(EM|TR|FE)", "DC(SL|BL|IM|BR|RF|EL).+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ScreamHighSchool#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `/(EM|TR|FE)(-?\d+)(@(\d+))?/i`（アンカー無し）。
fn special_check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(EM|TR|FE)(-?\d+)(@(\d+))?").expect("valid regex"))
}

/// Ruby `%r{(SH|SHS)(-?\d+)(/(\d+))?(@(\d+))?}i`（アンカー無し）。
///
/// `SHS100` は Ruby と同じく `SH` 側で `-?\d+` が失敗して `SHS` 側に倒れる
/// （`regex` クレートも leftmost-first のバックトラック意味論）。
fn sh_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(SH|SHS)(-?\d+)(/(\d+))?(@(\d+))?").expect("valid regex"))
}

/// Ruby `/^DC(SL|BL|IM|BR|RF|EL)(\d+)/i`。
fn damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC(SL|BL|IM|BR|RF|EL)(\d+)").expect("valid regex"))
}

/// Ruby `ScreamHighSchool#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(m) = special_check_pattern().captures(command) {
        // Ruby: Regexp.last_match(1).upcase
        let command_type = m[1].to_uppercase();
        let success_rate = to_i(&m[2]);
        let critical_border = get_critical_border(m.get(4).map(|v| v.as_str()), success_rate);

        return Ok(Some(SpecificCommandOutput::result(check_roll_sh(
            success_rate,
            critical_border,
            &command_type,
            rng,
        )?)));
    }

    if let Some(m) = sh_pattern().captures(command) {
        let success_rate = to_i(&m[2]);
        let repeat_count = m.get(4).map_or(1, |v| to_i(v.as_str()));
        let critical_border = get_critical_border(m.get(6).map(|v| v.as_str()), success_rate);

        return check_roll_repeat_attack(
            &JA_TABLES,
            success_rate,
            repeat_count,
            critical_border,
            rng,
        );
    }

    if let Some(m) = damage_pattern().captures(command) {
        let damage_value = to_i(&m[2]);
        return Ok(
            look_up_damage_chart(&JA_TABLES, &m[1], damage_value).map(SpecificCommandOutput::text)
        );
    }

    Ok(None)
}

/// Ruby `ScreamHighSchool#check_roll_sh`。
fn check_roll_sh(
    success_rate: i64,
    critical_border: i64,
    command_type: &str,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let success_rate = success_rate.max(0);
    let fumble_border = if success_rate < 100 { 96 } else { 99 };

    let dice_value = rng.roll_once(100)?;
    let mut result = get_check_result(
        &JA_TABLES,
        dice_value,
        success_rate,
        critical_border,
        fumble_border,
    );
    let (title, supplementary) = get_supplementary(command_type, &result.text);
    let supplementary = if supplementary.is_empty() {
        String::new()
    } else {
        format!("（{supplementary}）")
    };

    result.text = format!(
        "{title}判定 D100<={success_rate}@{critical_border} ＞ {dice_value} ＞ {}{supplementary}",
        result.text
    );
    Ok(result)
}

/// Ruby `ScreamHighSchool#get_supplementary`。戻り値は `[title, supplementary]`。
fn get_supplementary(command_type: &str, result: &str) -> (&'static str, &'static str) {
    match command_type {
        "EM" => (
            "感情",
            match result {
                "クリティカル" => "次に行う判定の成功率に+50%",
                "成功" => "次に行う判定の成功率に+30%",
                "失敗" => "次に行う判定の成功率に-20%、呪縛+1点",
                "ファンブル" => "次に行う判定の成功率に-50%、呪縛+1D5点",
                _ => "",
            },
        ),
        "TR" => (
            "性格傾向",
            match result {
                "失敗" => "反対側の性格傾向で再判定する。あるいは、もしこれがその再判定の結果であればプレイヤーが性格傾向を選択する",
                "ファンブル" => "反対側の性格傾向に従い、呪縛+1D5点する。あるいは、もしこれが失敗後の再判定の結果だった場合、PCは混乱し行動を放棄するか逃げ出す。呪縛+2点",
                _ => "判定した性格傾向に従う",
            },
        ),
        "FE" => (
            "恐怖",
            match result {
                "成功" => "ショックを受け流した。恐怖判定効果表の成功側の値分、呪縛が上昇する",
                "失敗" => "ショックを受けた。恐怖判定効果表の失敗側の値分、呪縛が上昇する",
                "ファンブル" => "深いショックを受けた。恐怖判定効果表の失敗側の値分に加え、さらに1D5点分、呪縛が上昇する",
                _ => "何もショックを受けなかった",
            },
        ),
        _ => ("", ""),
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
            .join("test/data/ScreamHighSchool.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ScreamHighSchool.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ScreamHighSchool.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ScreamHighSchool.toml must parse");
        assert_eq!(
            data.tests.len(),
            61,
            "case count in test/data/ScreamHighSchool.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ScreamHighSchool",
                "unexpected game system in ScreamHighSchool.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ScreamHighSchool"), &tc.input, &mut src) {
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
                    "FAIL ScreamHighSchool:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ScreamHighSchool cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
