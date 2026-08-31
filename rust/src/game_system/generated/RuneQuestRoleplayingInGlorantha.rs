//! P4で手書き移植した `lib/bcdice/game_system/RuneQuestRoleplayingInGlorantha.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`RQG` / `RES` / `RSA` の振り分け）
//! - `#do_ability_roll`（技能判定 `RQG[<=]成功率`）
//! - `#do_resistance_roll`（抵抗判定 `RES能力差[M増強値]`）
//! - `#do_resistance_active_characteristic_roll`（能動側のみ `RSA能力値[M増強値]`）
//! - `#get_roll_result`（決定的成功 / 効果的成功 / 成功 / 失敗 / ファンブル）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::{self};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::RuneQuestRoleplayingInGlorantha`
/// （ID: `RuneQuestRoleplayingInGlorantha`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuneQuestRoleplayingInGlorantha;

impl GameSystem for RuneQuestRoleplayingInGlorantha {
    fn id(&self) -> &'static str {
        "RuneQuestRoleplayingInGlorantha"
    }

    fn name(&self) -> &'static str {
        "ルーンクエスト：ロールプレイング・イン・グローランサ"
    }

    fn sort_key(&self) -> &'static str {
        "るうんくえすと4"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド 決定的成功、効果的成功、ファンブルを含めた判定を行う。
RQG<=成功率      (基本書式)
RQG成功率        (省略記法)

例1：RQG<=80    （技能値80で判定）
例2：RQG<=80+20 （技能値100で判定）
例3：RQG80      （省略書式で技能値80の判定）
例4：RQG80+20   （省略書式で技能値100の判定）

・抵抗判定コマンド（能動-受動） 決定的成功、効果的成功、ファンブルを含めた判定を行う。
RES(能動能力-受動能力)m増強値
増強値は省略可能。

例1：RES(9-11)    (能動能力9 vs 受動能力11で判定)
例2：RES(9-11)m20 (能動能力9 vs 受動能力11、+20%の増強が能動側に入る判定)
例3：RES(9)m50    (能動能力と受動能力の差が9で、+50%の増強が能動側に入る判定)

・抵抗判定コマンド(能動側のみ) 決定的成功、効果的成功、ファンブルは含めず判定を行う。
RSA(能動能力)m増強値
増強値は省略可能。

例1：RSA(9)       (能動能力9で判定)
例2：RSA(9)m20    (能動能力9で判定、+20%の増強が能動側に入る判定)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["RQG", "RES", "RSA"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#eval_game_system_specific_command`。
    ///
    /// Ruby の `case ... when /RQG/i` はアンカーなしの照合で、いずれかに当たった時点で
    /// その `do_*` の戻り値（`nil` を含む）をそのまま返す。当たった枝が `nil` を返しても
    /// 次の枝は試さないので、ここでも同じ順序・同じ打ち切り方にする。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command.contains("RQG") {
            return do_ability_roll(command, rng);
        }
        if command.contains("RES") {
            return do_resistance_roll(command, rng);
        }
        if command.contains("RSA") {
            return do_resistance_active_characteristic_roll(command, rng);
        }
        Ok(None)
    }
}

/// Ruby `%r{\A(RQG)((<=)?([+-/*\d]+))?$}`。
///
/// Ruby の文字クラス `[+-/*\d]` は `+`〜`/`（`+ , - . /`）の範囲＋`*`＋数字。
/// 丸括弧は含まれない（`RES(9-11)` のような入力は `Preprocessor` が先に畳む）。
fn ability_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(RQG)((<=)?([+\-./*,0-9]+))?$").expect("valid regex"))
}

/// Ruby `%r{\A(RES)([+-/*\d]+)(M([+-/*\d]+))?$}`。
fn resistance_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(RES)([+\-./*,0-9]+)(M([+\-./*,0-9]+))?$").expect("valid regex")
    })
}

/// Ruby `%r{\A(RSA)(\d+)(M([+-/*\d]+))?$}`。
fn resistance_active_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(RSA)([0-9]+)(M([+\-./*,0-9]+))?$").expect("valid regex"))
}

/// Ruby `Arithmetic.eval(source, RoundType::ROUND)`。
///
/// Ruby側は式が壊れていると `nil` を返し、呼び出し元は `nil` のまま計算へ進んで
/// `NoMethodError` / `TypeError` になる（例: `RES+`）。ここではクラッシュを再現せず
/// 「出力なし」に畳む。TOMLの全ケースはこの経路を通らない。
fn arith(source: &str) -> Result<Option<I>, EvalError> {
    arithmetic::eval(source, RoundType::Round)
}

/// Ruby `#do_ability_roll`。技能などの一般判定。
fn do_ability_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = ability_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: 目標値の有無に関わらず先に1D100を振る
    let roll_value = rng.roll_once(100)?;

    let Some(expr) = m.get(4) else {
        // RQGのみ指定された場合は1d100を振ったのと同じ挙動
        return Ok(Some(SpecificCommandOutput::text(format!(
            "(1D100) ＞ {roll_value}"
        ))));
    };

    let Some(ability_value) = arith(expr.as_str())? else {
        return Ok(None);
    };
    let result_prefix_str = format!("(1D100<={ability_value}) ＞");

    if ability_value == I::ZERO {
        // 0%は判定なしで失敗
        return Ok(Some(SpecificCommandOutput::result(EvalResult::failure(
            format!("{result_prefix_str} 失敗"),
        ))));
    }

    let result_str = format!("{result_prefix_str} {roll_value} ＞");
    Ok(Some(SpecificCommandOutput::result(get_roll_result(
        &result_str,
        crate::randomizer::sat_i64(&ability_value),
        roll_value,
    ))))
}

/// Ruby `#do_resistance_roll`。抵抗判定。
fn do_resistance_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = resistance_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby の `unless m[2]` はグループが必須なので常に偽（到達しない）。
    let Some(mut difference_value) = arith(&m[2])? else {
        return Ok(None);
    };
    if difference_value < I::from(-10) {
        difference_value = I::from(-10);
    }

    let mut resistance_value = I::from(50) + (difference_value * 5);
    if let Some(modifier) = m.get(4) {
        let Some(v) = arith(modifier.as_str())? else {
            return Ok(None);
        };
        resistance_value += v;
    }

    let roll_value = rng.roll_once(100)?;
    let result_str = format!("(1D100<={resistance_value}) ＞ {roll_value} ＞");

    Ok(Some(SpecificCommandOutput::result(get_roll_result(
        &result_str,
        crate::randomizer::sat_i64(&resistance_value),
        roll_value,
    ))))
}

/// Ruby `#do_resistance_active_characteristic_roll`。能動側のみの対抗判定。
fn do_resistance_active_characteristic_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = resistance_active_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: `m[2].to_i`。桁あふれは Bignum になるが、i64 に収まらない入力は
    // どのみち能力値としては意味を持たないので飽和させる。
    let active_ability_value: i64 = m[2].parse().unwrap_or(i64::MAX);
    if active_ability_value == 0 {
        // Ruby はここでダイスを振らずに戻る
        return Ok(Some(SpecificCommandOutput::text("0は指定できません。")));
    }

    let modify_value = match m.get(4) {
        Some(expr) => match arith(expr.as_str())? {
            Some(v) => v,
            None => return Ok(None),
        },
        None => I::ZERO,
    };

    let roll_value = rng.roll_once(100)?;
    let active_value = I::from(active_ability_value * 5) + &modify_value;
    let result_prefix_str = format!("(1D100<={active_value}) ＞ {roll_value} ＞");
    let note_str = "決定的成功、効果的成功、ファンブルは未処理。必要なら確認すること。";

    let out = if roll_value >= 96 {
        // 96以上は無条件で失敗
        SpecificCommandOutput::result(EvalResult::failure(format!(
            "{result_prefix_str} 失敗\n{note_str}"
        )))
    } else if roll_value <= 5 || I::from(roll_value) <= modify_value {
        // 05以下あるいは修正値以下は無条件で成功
        SpecificCommandOutput::result(EvalResult::success(format!(
            "{result_prefix_str} 成功\n{note_str}"
        )))
    } else {
        // 上記全てが当てはまらない時に突破可能な能力値を算出
        // Ruby の `Integer#/` は床除算。分子は負にもなる。
        let reachable = active_ability_value
            + crate::randomizer::sat_i64(&crate::arithmetic::floor_div(
                I::from(50) + modify_value - roll_value,
                I::from(5),
            ));
        SpecificCommandOutput::text(format!(
            "{result_prefix_str} 相手側能力値{reachable}まで成功\n{note_str}"
        ))
    };

    Ok(Some(out))
}

/// Ruby `#get_roll_result`。判定結果の取得。
///
/// `(x.to_f / 20).round` は Ruby の `Float#round`（絶対値が大きい側への四捨五入）で、
/// Rust の [`f64::round`] と同じ丸め方。
fn get_roll_result(result_str: &str, success_value: i64, roll_value: i64) -> EvalResult {
    let critical_value = (success_value as f64 / 20.0).round() as i64;
    let special_value = (success_value as f64 / 5.0).round() as i64;
    let fumble_value = ((100.0 - success_value as f64) / 20.0).round() as i64;

    if roll_value == 1 || roll_value <= critical_value {
        // 決定的成功(01は必ず決定的成功)
        EvalResult::critical(format!("{result_str} 決定的成功"))
    } else if roll_value == 100 || roll_value >= (100 - fumble_value + 1) {
        // ファンブル(00は必ずファンブル)
        EvalResult::fumble(format!("{result_str} ファンブル"))
    } else if roll_value >= 96 || (roll_value > success_value && roll_value > 5) {
        // 失敗(96以上は必ず失敗、出目が01-05ではなく技能値より上なら失敗)
        EvalResult::failure(format!("{result_str} 失敗"))
    } else if roll_value <= special_value {
        // 効果的成功
        EvalResult::success(format!("{result_str} 効果的成功"))
    } else if roll_value <= 5 || roll_value <= success_value {
        // 成功(05以下は必ず成功)
        EvalResult::success(format!("{result_str} 成功"))
    } else {
        // ここには到達しないはずだが、念のため捕捉
        EvalResult::failure(format!("{result_str} エラー"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    /// 余った注入乱数を許すケース（`(1始まりのケース番号, 残り個数)`）。
    ///
    /// Ruby本家の `RandomizerMock`（test/randomizer_mock.rb）は余りを検査しないので、
    /// TOMLには「Rubyもダイスを振る前に戻るコマンド」にもダイスが書かれている。
    /// TOMLは期待値の正本なので書き換えず、ここで明示的に許可する。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[
        (115, 1), // RSA0 （Ruby も0判定で即 return し、1D100を振らない）
    ];

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/RuneQuestRoleplayingInGlorantha.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/RuneQuestRoleplayingInGlorantha.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/RuneQuestRoleplayingInGlorantha.toml not found");
            return;
        };

        let data =
            TestDataFile::load(&path).expect("RuneQuestRoleplayingInGlorantha.toml must parse");
        assert_eq!(
            data.tests.len(),
            159,
            "case count in test/data/RuneQuestRoleplayingInGlorantha.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "RuneQuestRoleplayingInGlorantha",
                "unexpected game system in RuneQuestRoleplayingInGlorantha.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("RuneQuestRoleplayingInGlorantha"),
                &tc.input,
                &mut src,
            ) {
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

            let allowed_surplus = SURPLUS_RANDS_ALLOWED
                .iter()
                .find(|(case, _)| *case == i + 1)
                .map_or(0, |(_, remaining)| *remaining);
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL RuneQuestRoleplayingInGlorantha:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} RuneQuestRoleplayingInGlorantha cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
