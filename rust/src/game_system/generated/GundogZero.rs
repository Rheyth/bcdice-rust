//! P4で手書き移植した `lib/bcdice/game_system/GundogZero.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 親クラス `Gundog` の `result_1d100`（1D100の成功度判定）と `@enabled_d9 = true`
//!   （`Gundog.rs` 側はスタブのままなので、親由来の機能はこのファイルに取り込んである）
//! - `GundogZero#eval_game_system_specific_command` → `roll_penalty_table`
//!   （ダメージペナルティー表 `xDPTn` / ファンブル表 `xFTn`）
//! - `getDamageTypeAndTable` / `getFumbleTypeAndTable` の表データ

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

/// Ruby `getDamageTypeAndTable("S")` が返す射撃ダメージペナルティー表。添字が出目（0〜18）。
static DAMAGE_TABLE_S: &[&str] = &[
    "対象は[死亡]",
    "[追加D]4D6/[出血]2D6/[重傷]-30％/[朦朧判定]15",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]14",
    "[追加D]3D6/[出血]2D6/[重傷]-20％/[朦朧判定]14",
    "[追加D]3D6/[出血]1D6/[重傷]-20％/[朦朧判定]12",
    "[追加D]2D6/[出血]1D6/[重傷]-10％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]10",
    "[追加D]2D6/[軽傷]-10％/[朦朧判定]10",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]8",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]6",
    "[追加D]2D6/[軽傷]-10％/[朦朧判定]4",
    "[追加D]1D6/[軽傷]-20％",
    "[追加D]1D6/[軽傷]-20％",
    "[追加D]1D6/[軽傷]-10％",
    "[軽傷]-20％",
    "[軽傷]-10％",
    "[軽傷]-10％",
    "手に持った武器を落とす",
    "ペナルティー無し",
];

/// Ruby `getDamageTypeAndTable("M")` が返す格闘ダメージペナルティー表。添字が出目（0〜18）。
static DAMAGE_TABLE_M: &[&str] = &[
    "対象は[死亡]",
    "[追加D]3D6/[出血]2D6/[重傷]-30％/[朦朧判定]15",
    "[追加D]2D6/[出血]2D6/[重傷]-30％/[朦朧判定]14",
    "[追加D]2D6/[出血]1D6/[重傷]-20％/[朦朧判定]14",
    "[追加D]3D6/[出血]1D6/[重傷]-10％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-10％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-10％/[朦朧判定]10",
    "[追加D]1D6/[軽傷]-20％/[朦朧判定]8",
    "[追加D]1D6/[軽傷]-10％/[朦朧判定]8",
    "[追加D]1D6/[軽傷]-10％/[朦朧判定]6",
    "[軽傷]-20％/[朦朧判定]6",
    "[軽傷]-10％/[朦朧判定]6",
    "[軽傷]-10％/[朦朧判定]4",
    "[軽傷]-20％",
    "[軽傷]-10％",
    "[軽傷]-10％",
    "手に持った武器を落とす",
    "ペナルティー無し",
];

/// Ruby `getDamageTypeAndTable("V")` が返す車両ダメージペナルティー表。添字が出目（0〜18）。
static DAMAGE_TABLE_V: &[&str] = &[
    "[クラッシュ]する。[チェイス]から除外",
    "[乗員D]3D6/[操縦性]-20％/[スピン判定]",
    "[乗員D]3D6/[操縦性]-20％/[スピン判定]",
    "[乗員D]2D6/[操縦性]-10％/[スピン判定]",
    "[乗員D]2D6/[操縦性]-10％/[スピン判定]",
    "[乗員D]3D6/[スピード]-2/[スピン判定]",
    "[乗員D]3D6/[スピード]-2/[スピン判定]",
    "[乗員D]2D6/[スピード]-1/[スピン判定]",
    "[乗員D]2D6/[スピード]-1/[スピン判定]",
    "[乗員D]2D6/[操縦判定]-20％",
    "[乗員D]2D6/[操縦判定]-20％",
    "[乗員D]1D6/[操縦判定]-10％",
    "[乗員D]1D6/[操縦判定]-10％",
    "[スピン判定]",
    "[スピン判定]",
    "乗員に[ショック]-20％",
    "乗員に[ショック]-10％",
    "乗員に[ショック]-10％",
    "ペナルティー無し",
];

/// Ruby `getDamageTypeAndTable("G")` が返す汎用ダメージペナルティー表。添字が出目（0〜18）。
static DAMAGE_TABLE_G: &[&str] = &[
    "対象は[死亡]",
    "[追加D]4D6/[出血]2D6/[重傷]-30％/[朦朧判定]18",
    "[追加D]4D6/[出血]2D6/[重傷]-30％/[朦朧判定]16",
    "[追加D]3D6/[出血]2D6/[重傷]-20％/[朦朧判定]14",
    "[追加D]3D6/[出血]2D6/[重傷]-20％/[朦朧判定]14",
    "[追加D]3D6/[出血]1D6/[重傷]-10％/[朦朧判定]12",
    "[追加D]2D6/[出血]1D6/[重傷]-10％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-30％/[朦朧判定]12",
    "[追加D]2D6/[軽傷]-30％/[朦朧判定]10",
    "[追加D]2D6/[軽傷]-30％/[朦朧判定]8",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]8",
    "[追加D]2D6/[軽傷]-20％/[朦朧判定]6",
    "[追加D]2D6/[軽傷]-10％/[朦朧判定]6",
    "[追加D]1D6/[軽傷]-20％/[朦朧判定]4",
    "[追加D]1D6/[軽傷]-20％",
    "[追加D]1D6/[軽傷]-10％",
    "[軽傷]-20％",
    "[軽傷]-10％",
    "ペナルティー無し",
];

/// Ruby `getFumbleTypeAndTable("S")` が返す射撃ファンブル表。添字が出目（0〜18）。
static FUMBLE_TABLE_S: &[&str] = &[
    "銃器が暴発、自分に命中。[貫通D]",
    "銃器が暴発、自分に命中。[非貫通D]",
    "誤射。ランダムに味方に命中。[貫通D]",
    "誤射。ランダムに味方に命中。[非貫通D]",
    "銃器が完全に故障",
    "銃器が完全に故障",
    "故障。〈メカニック〉判定に成功するまで射撃不可",
    "故障。〈メカニック〉判定に成功するまで射撃不可",
    "作動不良。[アイテム使用]を2回行って修理するまで射撃不可",
    "作動不良。[アイテム使用]を2回行って修理するまで射撃不可",
    "作動不良。[アイテム使用]を行って修理するまで射撃不可",
    "作動不良。[アイテム使用]を行って修理するまで射撃不可",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-10％",
    "姿勢を崩す。[ショック]-10％",
    "ペナルティー無し",
];

/// Ruby `getFumbleTypeAndTable("M")` が返す格闘ファンブル表。添字が出目（0〜18）。
static FUMBLE_TABLE_M: &[&str] = &[
    "避けられて[転倒]、[朦朧]状態",
    "ランダムに[至近距離]の味方(居なければ自分)に命中。[貫通D]",
    "ランダムに[至近距離]の味方(居なければ自分)に命中。[貫通D]",
    "武器が完全に壊れる",
    "武器がガタつく。〈手先〉判定に成功するまで使用不可",
    "武器がガタつく。〈手先〉判定に成功するまで使用不可",
    "無理な姿勢で筋を伸ばす。[軽傷]-30％",
    "無理な姿勢で筋を伸ばす。[軽傷]-30％",
    "無理な姿勢で筋を伸ばす。[軽傷]-20％",
    "無理な姿勢で筋を伸ばす。[軽傷]-20％",
    "無理な姿勢で筋を伸ばす。[軽傷]-10％",
    "無理な姿勢で筋を伸ばす。[軽傷]-10％",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-10％",
    "姿勢を崩す。[ショック]-10％",
    "ペナルティー無し",
];

/// Ruby `getFumbleTypeAndTable("T")` が返す投擲ファンブル表。添字が出目（0〜18）。
static FUMBLE_TABLE_T: &[&str] = &[
    "[転倒]、[朦朧]状態",
    "自分に命中。[貫通D]",
    "自分に命中。[非貫通D]",
    "ランダムに味方(居なければ自分)に命中。[非貫通D]",
    "ランダムに味方(居なければ自分)に命中。[非貫通D]",
    "武器が完全に壊れる",
    "武器が完全に壊れる",
    "腰を痛める。[軽傷]-30％",
    "肩を痛める。[軽傷]-20％",
    "肩を痛める。[軽傷]-20％",
    "肘に違和感。[軽傷]-10％",
    "肘に違和感。[軽傷]-10％",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[不安定]",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-20％",
    "姿勢を崩す。[ショック]-10％",
    "姿勢を崩す。[ショック]-10％",
    "ペナルティー無し",
];

/// Ruby `BCDice::GameSystem::GundogZero`（ID: `GundogZero`）。
///
/// Ruby側は `class GundogZero < Gundog`。継承していた `result_1d100` と
/// `@enabled_d9 = true` はここで直接実装する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GundogZero;

impl GameSystem for GundogZero {
    fn id(&self) -> &'static str {
        "GundogZero"
    }

    fn name(&self) -> &'static str {
        "ガンドッグゼロ"
    }

    fn sort_key(&self) -> &'static str {
        "かんとつくせろ"
    }

    fn help_message(&self) -> &'static str {
        r"失敗、成功、クリティカル、ファンブルとロールの達成値の自動判定を行います。
nD9ロールも対応。
・ダメージペナルティ表　　(〜DPTx) (x:修正)
　射撃(SDPT)、格闘(MDPT)、車両(VDPT)、汎用(GDPT)の各表を引くことが出来ます。
　修正を後ろに書くことも出来ます。
・ファンブル表　　　　　　(〜FTx)  (x:修正)
　射撃(SFT)、格闘(MFT)、投擲(TFT)の各表を引くことが出来ます。
　修正を後ろに書くことも出来ます。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[".DPT", ".FT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Gundog#initialize` の `@enabled_d9 = true`。
    fn enabled_d9(&self) -> bool {
        true
    }

    /// Ruby `Gundog#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        // 目標値 `?` の判定は `total >= 100` と `total <= 1` の**後**に来る。
        // 先頭に出すと `1D100<=?` のファンブル／絶対成功が拾えなくなる。
        if total >= I::from(100) {
            return Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "ファンブル",
            ))));
        }
        if total <= I::ONE {
            return Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "絶対成功(達成値1+SL)",
            ))));
        }

        // Ruby: elsif target == "?" -> Result.nothing
        // `nil`（＝次のフックへ進む）ではなく `:nothing`（＝以降を打ち切って nil）。
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };

        if total > target {
            return Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗"))));
        }

        // ここに来る total は 2..=99 なので、Ruby側の
        // `dig10 = 0 if dig10 >= 10` / `dig1 = 0 if dig1 >= 10` は到達しない。
        let dig10 = &total / 10;
        let dig1 = &total - &dig10 * 10;

        let result = if dig1 <= I::ZERO {
            EvalResult::critical("クリティカル(達成値20+SL)")
        } else {
            EvalResult::success(format!("成功(達成値{}+SL)", dig10 + dig1))
        };
        Some(CheckOutcome::Result(Box::new(result)))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        roll_penalty_table(command, rng)
    }
}

/// Ruby `/(\w)DPT([+\-\d]*)/i`。
///
/// Rubyの `\w` / `\d` はASCII限定なので `[0-9A-Za-z_]` / `[0-9]` に置き換える
/// （Rustの `regex` は既定でUnicode）。
fn damage_table_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9A-Za-z_])DPT([+\-0-9]*)").expect("valid regex"))
}

/// Ruby `/(\w)FT([+\-\d]*)/i`。ASCII限定にする理由は [`damage_table_pattern`] と同じ。
fn fumble_table_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([0-9A-Za-z_])FT([+\-0-9]*)").expect("valid regex"))
}

/// Ruby `GundogZero#eval_game_system_specific_command`。
fn roll_penalty_table(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: string = command.upcase
    let string = command.to_uppercase();

    let mut ttype = "";
    let mut type_and_table: Option<(&'static str, &'static [&str])> = None;
    let mut modifier = 0;

    // ダメージペナルティー表
    if let Some(captures) = damage_table_pattern().captures(&string) {
        ttype = "ダメージペナルティー";
        // Ruby: `([+\-\d]*)` は空マッチでも nil にならないので、常に評価される。
        modifier = arithmetic_evaluator_eval(&captures[2])?;
        type_and_table = Some(damage_type_and_table(&captures[1]));
    }

    // ファンブル表
    // Ruby側は `elsif` ではなく独立した `if` で、両方に当たる文字列（`SDPTMFT` など）では
    // ファンブル表側が上書きする。
    if let Some(captures) = fumble_table_pattern().captures(&string) {
        ttype = "ファンブル";
        modifier = arithmetic_evaluator_eval(&captures[2])?;
        type_and_table = Some(fumble_type_and_table(&captures[1]));
    }

    // Ruby: return '1' if type.empty?
    // 接頭辞 `.DPT` / `.FT` の `.` は任意の1文字なので、`-DPT` のように
    // `\w` に当たらない文字が先頭のときここへ来る（呼び出し元が nil に畳む）。
    let Some((type_name, table)) = type_and_table else {
        return Ok(Some(SpecificCommandOutput::text("1")));
    };

    // 左から順に振る（Ruby の `roll_index(10) + roll_index(10)`）。
    let first = rng.roll_index(10)?;
    let second = rng.roll_index(10)?;
    let dice = first + second + modifier;

    // Ruby: diceOriginalText は丸める前の値
    let dice_original_text = dice;
    let dice = dice.clamp(0, 18) as usize;

    Ok(Some(SpecificCommandOutput::text(format!(
        "{type_name}{ttype}表[{dice_original_text}] ＞ {}",
        table[dice]
    ))))
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（`Arithmetic.eval(expr, :floor) || 0`）。
fn arithmetic_evaluator_eval(expr: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `GundogZero#getDamageTypeAndTable`。
///
/// Ruby側は未知の頭文字を `head = "S"` として自分自身を呼び直す（射撃扱い）。
fn damage_type_and_table(head: &str) -> (&'static str, &'static [&'static str]) {
    match head {
        "M" => ("格闘", DAMAGE_TABLE_M),
        "V" => ("車両", DAMAGE_TABLE_V),
        "G" => ("汎用", DAMAGE_TABLE_G),
        // "S" と、間違った頭文字（射撃扱い）
        _ => ("射撃", DAMAGE_TABLE_S),
    }
}

/// Ruby `GundogZero#getFumbleTypeAndTable`。未知の頭文字が射撃扱いなのも同じ。
fn fumble_type_and_table(head: &str) -> (&'static str, &'static [&'static str]) {
    match head {
        "M" => ("格闘", FUMBLE_TABLE_M),
        "T" => ("投擲", FUMBLE_TABLE_T),
        // "S" と、間違った頭文字（射撃扱い）
        _ => ("射撃", FUMBLE_TABLE_S),
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
            .join("test/data/GundogZero.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// 表は Ruby と同じく出目0〜18の19項目。
    #[test]
    fn tables_have_nineteen_entries() {
        for (name, table) in [
            ("DAMAGE_TABLE_S", super::DAMAGE_TABLE_S),
            ("DAMAGE_TABLE_M", super::DAMAGE_TABLE_M),
            ("DAMAGE_TABLE_V", super::DAMAGE_TABLE_V),
            ("DAMAGE_TABLE_G", super::DAMAGE_TABLE_G),
            ("FUMBLE_TABLE_S", super::FUMBLE_TABLE_S),
            ("FUMBLE_TABLE_M", super::FUMBLE_TABLE_M),
            ("FUMBLE_TABLE_T", super::FUMBLE_TABLE_T),
        ] {
            assert_eq!(table.len(), 19, "{name}");
        }
    }

    /// `test/data/GundogZero.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/GundogZero.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("GundogZero.toml must parse");
        assert_eq!(
            data.tests.len(),
            259,
            "case count in test/data/GundogZero.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "GundogZero",
                "unexpected game system in GundogZero.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("GundogZero"), &tc.input, &mut src) {
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
                    "FAIL GundogZero:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} GundogZero cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
