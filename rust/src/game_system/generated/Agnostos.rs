//! P4で手書き移植した `lib/bcdice/game_system/Agnostos.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#condition_roll`（`CDx>=t`。行為判定）
//! - `#heart_condition_roll`（`HCDx`。心拍のコンディションチェック）
//! - `#special_roll`（`xSPy`。必殺技）
//!
//! # フラグの立ち方
//!
//! Ruby側は `Result.critical` / `Result.fumble` ではなく `Result.new.tap` の中で
//! `r.critical=` / `r.fumble=` を**直接**代入している。つまりクリティカル・ファンブルは
//! 成功・失敗と独立で、
//!
//! - `condition_roll` は `r.condition = value >= 目標値` で成功/失敗を決める
//!   （クリティカルだが失敗、ファンブルだが成功もありうる）
//! - `heart_condition_roll` は `condition` を**設定しない**ので、
//!   クリティカル/ファンブルだけが立ち成功も失敗も立たない
//!
//! となる。[`EvalResult::critical`] 等のコンストラクタは成功/失敗も立ててしまうので使わない。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `#eval_game_system_specific_command`。
///
/// Ruby: `condition_roll(command) || heart_condition_roll(command) || special_roll(command)`
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = condition_roll(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = heart_condition_roll(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(special_roll(command, rng)?.map(SpecificCommandOutput::text))
}

// ---------------------------------------------------------------------------
// 行為判定
// ---------------------------------------------------------------------------

/// Ruby `#condition_roll`。
fn condition_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: Command::Parser.new(/CD[A-E1-5]/, round_type: @round_type)
    //         .disable_modifier.restrict_cmp_op_to(:>=)
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["CD[A-E1-5]"], RoundType::Floor)
            .disable_modifier()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };
    // 比較演算子を `>=` に限定しているので目標値は必ず伴う（文法上 `CMP_OP add` のみ）。
    let Some(target_number) = parsed.target_number else {
        return Ok(None);
    };

    // Ruby: to_condition_level(parsed.command[-1])
    let last = parsed.command.chars().next_back().unwrap_or(' ');
    let condition_level = to_condition_level(last);
    let sides = to_sides(condition_level);
    let value = rng.roll_once(sides)?;

    let mut r = EvalResult::new();
    // Ruby は `r.critical=` / `r.fumble=` を直接代入する（成功/失敗とは独立）。
    r.critical = is_critical(sides, value);
    r.fumble = is_fumble(sides, value);
    r.set_condition(value >= crate::randomizer::sat_i64(&target_number));

    let outcome = if r.success { "成功" } else { "失敗" };
    r.text = [
        format!("(CD{condition_level}>={target_number})"),
        format!("(1D{sides}>={target_number})"),
        value.to_string(),
        outcome.to_string(),
        condition_change(sides, value).to_string(),
    ]
    .join(" ＞ ");

    Ok(Some(r))
}

/// Ruby `#to_condition_level`。数値表記をアルファベット表記に直す。
fn to_condition_level(c: char) -> char {
    match c {
        '5' => 'A',
        '4' => 'B',
        '3' => 'C',
        '2' => 'D',
        '1' => 'E',
        other => other,
    }
}

/// Ruby `#to_sides`。コンディションレベルに対応するダイスの面数。
fn to_sides(condition: char) -> i64 {
    match condition {
        'A' => 12,
        'B' => 10,
        'C' => 8,
        'D' => 6,
        // Ruby: else # "E"
        _ => 4,
    }
}

/// Ruby `#condition_change`。
fn condition_change(sides: i64, value: i64) -> &'static str {
    if is_critical(sides, value) {
        "コンディション：2段階上昇（クリティカル）"
    } else if is_fumble(sides, value) {
        "コンディション：2段階下降（ファンブル）"
    } else if sides != 12 && sides - value <= 1 {
        "コンディション：1段階上昇"
    // Ruby は面数ごとに `elsif` を3つ並べているが、条件は面数で排他なので
    // 同じ結果を返す3分岐を `||` でまとめた（評価順も短絡も原典と同じ）。
    } else if (sides == 12 && value <= 6)
        || (sides == 10 && value <= 3)
        || (sides == 8 && value <= 2)
    {
        "コンディション：1段階下降"
    } else {
        "コンディション：変動なし"
    }
}

/// Ruby `#critical?`。1D12にはクリティカルが無い。
fn is_critical(sides: i64, value: i64) -> bool {
    sides != 12 && sides == value
}

/// Ruby `#fumble?`。1D4にはファンブルが無い。
fn is_fumble(sides: i64, value: i64) -> bool {
    sides != 4 && value == 1
}

// ---------------------------------------------------------------------------
// 心拍のコンディションチェック
// ---------------------------------------------------------------------------

/// Ruby `/^HCD([A1-5]|[BC][+-])$/`。
fn hcd_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^HCD([A1-5]|[BC][+-])$").expect("valid regex"))
}

/// Ruby `#heart_condition_roll`。
///
/// `condition` を設定しないので、成功/失敗はどちらも立たない
/// （クリティカル/ファンブルだけが立つことがある）。
fn heart_condition_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = hcd_pattern().captures(command) else {
        return Ok(None);
    };

    let suffix = &m[1];
    let condition_level = to_heart_condition_level(suffix);
    let sides = to_heart_sides(condition_level);
    let value = rng.roll_once(sides)?;

    let mut r = EvalResult::new();
    r.critical = is_critical(sides, value);
    r.fumble = is_fumble(sides, value);
    r.text = [
        format!("(HCD{condition_level})"),
        format!("(1D{sides})"),
        value.to_string(),
        heart_condition_change(sides, value).to_string(),
    ]
    .join(" ＞ ");

    Ok(Some(r))
}

/// Ruby `#to_heart_condition_level`。数値表記を `C+` などの表記に直す。
fn to_heart_condition_level(s: &str) -> &str {
    match s {
        "5" => "C+",
        "4" => "B+",
        "3" => "A",
        "2" => "B-",
        "1" => "C-",
        other => other,
    }
}

/// Ruby `#to_heart_sides`。
fn to_heart_sides(condition: &str) -> i64 {
    match condition {
        "C+" => 12,
        "B+" => 10,
        "A" => 8,
        "B-" => 6,
        // Ruby: else # "C-"
        _ => 4,
    }
}

/// Ruby `#fainted?`。
fn is_fainted(sides: i64, value: i64) -> bool {
    match sides {
        12 => value >= 7,
        10 => value >= 10,
        8 => false,
        6 => value <= 1,
        // Ruby: else # 4
        _ => value <= 2,
    }
}

/// Ruby `#heart_condition_change`。
fn heart_condition_change(sides: i64, value: i64) -> &'static str {
    if is_fainted(sides, value) {
        "気絶"
    } else {
        condition_change(sides, value)
    }
}

// ---------------------------------------------------------------------------
// 必殺技
// ---------------------------------------------------------------------------

/// Ruby `/^([A-E1-5])SP([A-E1-5])$/`。
fn sp_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-E1-5])SP([A-E1-5])$").expect("valid regex"))
}

/// Ruby `#special_roll`。
fn special_roll(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = sp_pattern().captures(command) else {
        return Ok(None);
    };

    // `[A-E1-5]` にマッチした1文字なので `chars().next()` は必ず `Some`。
    let times_condition_level = to_condition_level(m[1].chars().next().unwrap_or(' '));
    let sides_condition_level = to_condition_level(m[2].chars().next().unwrap_or(' '));

    let times = to_times(times_condition_level);
    let sides = to_sides(sides_condition_level);

    let dice_list = rng.roll_barabara(times, sides)?;
    let value: i64 = dice_list.iter().sum();

    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    Ok(Some(
        [
            format!("({times_condition_level}SP{sides_condition_level})"),
            format!("({times}D{sides})"),
            format!("{value}[{dice_text}]"),
            value.to_string(),
        ]
        .join(" ＞ "),
    ))
}

/// Ruby `#to_times`。メインコンディションに対応するダイスの個数。
fn to_times(condition: char) -> i64 {
    match condition {
        'A' => 5,
        'B' => 4,
        'C' => 3,
        'D' => 2,
        // Ruby: else # "E"
        _ => 1,
    }
}

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::Agnostos`（ID: `Agnostos`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Agnostos;

impl GameSystem for Agnostos {
    fn id(&self) -> &'static str {
        "Agnostos"
    }

    fn name(&self) -> &'static str {
        "アグノストス"
    }

    fn sort_key(&self) -> &'static str {
        "あくのすとす"
    }

    fn help_message(&self) -> &'static str {
        r"■ 行為判定
  CDx>=t
  x: コンディションレベル（A~E もしくは 5~1)
  t: 目標値
  の成否とコンディションの変動量を判定します。

■ 心拍のコンディションチェック
  HCDx
  x: コンディションレベル（C+, B+, A, B-, C-, もしくは 5~1）
  酸素の消費量、コンディションの変動量、気絶したかを判定します。

■ 必殺技
  xSPy
  x: メインコンディション（A~E もしくは 5~1)
  y: サブコンディション（A~E もしくは 5~1)
  必殺技のダメージ量を判定します。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CD", "HCD", "[A-E1-5]SP[A-E1-5]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Agnostos#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/Agnostos.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Agnostos", "Agnostos.toml", 41);
    }

    /// TOMLに無い経路の固定。
    ///
    /// - `CD` に `>=` 以外の比較演算子・修正値は使えない
    /// - `HCD` の未知の接尾辞、`SP` の未知のコンディションは `nil`
    /// - クリティカルと失敗が同時に立つ（`Result.critical` を使っていないことの確認）
    #[test]
    fn restricted_and_independent_flag_paths() {
        for command in ["CDB<=6", "CDB+1>=6", "CDF>=6", "HCDD", "HCDB", "FSPA"] {
            let mut src = SeededRandomizer::new(vec![]);
            assert!(
                eval_command(&GameSystemId::new("Agnostos"), command, &mut src)
                    .expect("must not error")
                    .is_none(),
                "{command} must be nil"
            );
        }

        // 1D10で10（クリティカル）だが目標値には届かない＝クリティカルかつ失敗。
        let mut src = SeededRandomizer::new(vec![(10, 10)]);
        let result = eval_command(&GameSystemId::new("Agnostos"), "CDB>=11", &mut src)
            .expect("CDB>=11 must not error")
            .expect("CDB>=11 must produce output");
        assert_eq!(
            result.text,
            "(CDB>=11) ＞ (1D10>=11) ＞ 10 ＞ 失敗 ＞ コンディション：2段階上昇（クリティカル）"
        );
        assert!(result.critical, "critical must be set");
        assert!(result.failure, "failure must be set");
        assert!(!result.success, "success must not be set");
    }
}
