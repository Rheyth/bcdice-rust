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
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/PersonaO.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/PersonaO.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/PersonaO.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("PersonaO.toml must parse");
        assert_eq!(
            data.tests.len(),
            13,
            "case count in test/data/PersonaO.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "PersonaO",
                "unexpected game system in PersonaO.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("PersonaO"), &tc.input, &mut src) {
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
                    "FAIL PersonaO:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} PersonaO cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
