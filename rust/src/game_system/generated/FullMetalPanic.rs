//! P4で手書き移植した `lib/bcdice/game_system/FullMetalPanic.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 親クラス `SRS`（`lib/bcdice/game_system/SRS.rb`）の成功判定一式
//!   （`#eval_game_system_specific_command` / `#parse` / `#parse_legacy` /
//!   `#execute_srs_roll` / `#compare_result` / `SRSRollNode#to_s`）
//! - `FullMetalPanic` 自身の `set_aliases_for_srs_roll('MG', 'FP')`（＝判定コマンドの
//!   表記に `2D6` の別名 `MG` / `FP` を足す設定）
//!
//! # SRS判定の置き場所
//!
//! Ruby側は `SRS` クラスに判定本体があり、`FullMetalPanic` は別名を足すだけ。
//! Rust側の `SRS.rs` はまだ生成スタブのままで別バッチの担当なので、ここでは
//! 判定本体をこのファイルに持つ（`TenkaRyouran.rs` / `MetallicGuardian.rs` も
//! 同じ理由で同じ実装を持つ）。`SRS` 本体が移植されたら、そちらへ寄せて重複を畳める。
//!
//! ロケール差のある文言は [`SystemTables`] に束ね、
//! `FullMetalPanic_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::Parser;
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ロケールごとの定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の設定と定型文。`FullMetalPanic` と `FullMetalPanic_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `["2D6"].concat(aliases())`（`set_aliases_for_srs_roll` の設定を含む）
    pub(crate) notations: &'static [&'static str],
    /// i18n `SRS.auto_success`
    pub(crate) auto_success: &'static str,
    /// i18n `SRS.auto_failure`
    pub(crate) auto_failure: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

/// Ruby `SRS::DEFAULT_CRITICAL_VALUE`。
const DEFAULT_CRITICAL_VALUE: i64 = 12;
/// Ruby `SRS::DEFAULT_FUMBLE_VALUE`。
const DEFAULT_FUMBLE_VALUE: i64 = 2;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `SRS::SRSRollNode`。
pub(crate) struct SrsRollNode {
    modifier: crate::Int,
    critical_value: i64,
    fumble_value: i64,
    target_value: Option<crate::Int>,
}

impl SrsRollNode {
    /// Ruby `SRSRollNode#to_s`。
    ///
    /// 別名（`MG` / `FP`）で入力しても表記は常に `2D6`。
    fn to_s(&self) -> String {
        let lhs = format!("2D6{}", modifier(&self.modifier));
        let expression = match &self.target_value {
            Some(target) => format!("{lhs}>={target}"),
            None => lhs,
        };
        format!(
            "{expression}[{},{}]",
            self.critical_value, self.fumble_value
        )
    }
}

/// Ruby `SRS#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: /(.+)\[(.*)\]\z/.match(command)
    let node = match legacy_c_f_pattern().captures(command) {
        Some(m) => parse_legacy(sys, &m[1], &m[2]),
        None => parse(sys, command),
    };

    match node {
        Some(node) => Ok(Some(SpecificCommandOutput::result(execute_srs_roll(
            sys, &node, rng,
        )?))),
        None => Ok(None),
    }
}

/// Ruby `/(.+)\[(.*)\]\z/`。
fn legacy_c_f_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(.+)\[(.*)\]\z").expect("valid regex"))
}

/// Ruby `/^(-?\d+)?(?:,(-?\d+))?$/`。
fn c_f_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(-?\d+)?(?:,(-?\d+))?\z").expect("valid regex"))
}

/// Ruby `Command::Parser.new(prefix_re, round_type:)` の共通部分。
///
/// `notations` は `["2D6", 別名...]`。Ruby側は `join('|')` した1本の正規表現だが、
/// パーサは先頭一致を順に試すので、配列で渡しても同じ語を受理する。
fn base_parser(sys: &SystemTables) -> Parser {
    Parser::new(sys.notations, RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
}

/// Ruby `SRS#parse`。
fn parse(sys: &SystemTables, command: &str) -> Option<SrsRollNode> {
    let parser = base_parser(sys).enable_critical().enable_fumble();
    let cmd = parser.parse(command)?;

    // Ruby: command.start_with?(/2D6/i) && 3つとも nil なら通常のダイスロールに任せる。
    // 入力は `dice_command` で大文字化済みなので、ここは単純な前方一致でよい。
    if command.starts_with("2D6")
        && cmd.critical.is_none()
        && cmd.fumble.is_none()
        && cmd.target_number.is_none()
    {
        return None;
    }

    Some(SrsRollNode {
        modifier: cmd.modify_number,
        // Ruby: cmd.critical ||= DEFAULT_CRITICAL_VALUE（0は真値なので上書きされない）
        critical_value: cmd
            .critical
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(DEFAULT_CRITICAL_VALUE),
        fumble_value: cmd
            .fumble
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(DEFAULT_FUMBLE_VALUE),
        target_value: cmd.target_number,
    })
}

/// Ruby `SRS#parse_legacy`。`2D6+m>=t[c,f]` の `[c,f]` 記法。
///
/// こちらのパーサは `@c` / `#f` を有効にしない（原典どおり）。
fn parse_legacy(sys: &SystemTables, command: &str, c_f: &str) -> Option<SrsRollNode> {
    let m = c_f_pattern().captures(c_f)?;

    // Ruby: m[1]&.to_i || DEFAULT_CRITICAL_VALUE
    let critical_value = m
        .get(1)
        .map_or(DEFAULT_CRITICAL_VALUE, |v| to_i(v.as_str()));
    let fumble_value = m.get(2).map_or(DEFAULT_FUMBLE_VALUE, |v| to_i(v.as_str()));

    let cmd = base_parser(sys).parse(command)?;

    Some(SrsRollNode {
        modifier: cmd.modify_number,
        critical_value,
        fumble_value,
        target_value: cmd.target_number,
    })
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない指定は飽和させる。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or_else(|_| {
        if digits.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `SRS#execute_srs_roll`。
fn execute_srs_roll(
    sys: &SystemTables,
    srs_roll: &SrsRollNode,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let mut dice_list = rng.roll_barabara(2, 6)?;
    // Ruby: dice_list.sort! if @sort_add_dice（SRS#initialize で true）
    dice_list.sort_unstable();

    let sum: i64 = dice_list.iter().sum();
    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let modified_sum: crate::Int = crate::Int::from(sum) + &srs_roll.modifier;

    let mut result = compare_result(
        sys,
        srs_roll,
        sum,
        crate::randomizer::sat_i64(&modified_sum),
    );

    let mut parts = vec![
        format!("({})", srs_roll.to_s()),
        format!("{sum}[{dice_str}]{}", modifier(&srs_roll.modifier)),
        modified_sum.to_string(),
    ];
    // Ruby: `Result.new` の text は nil なので `.compact` で落ちる
    if !result.text.is_empty() {
        parts.push(result.text.clone());
    }

    result.text = parts.join(" ＞ ");
    Ok(result)
}

/// Ruby `SRS#compare_result`。
///
/// クリティカル・ファンブルは修正前の出目の合計（`sum`）、
/// 目標値との比較は修正後（`modified_sum`）で行う。
fn compare_result(
    sys: &SystemTables,
    srs_roll: &SrsRollNode,
    sum: i64,
    modified_sum: i64,
) -> EvalResult {
    if sum >= srs_roll.critical_value {
        EvalResult::critical(sys.auto_success)
    } else if sum <= srs_roll.fumble_value {
        EvalResult::fumble(sys.auto_failure)
    } else if let Some(target_value) = &srs_roll.target_value {
        if modified_sum >= crate::randomizer::sat_i64(target_value) {
            EvalResult::success(sys.success)
        } else {
            EvalResult::failure(sys.failure)
        }
    } else {
        EvalResult::new()
    }
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの定型文
// ---------------------------------------------------------------------------

/// `ja_jp` ロケールの設定と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    notations: &["2D6", "MG", "FP"],
    auto_success: "自動成功",
    auto_failure: "自動失敗",
    success: "成功",
    failure: "失敗",
};

/// Ruby `BCDice::GameSystem::FullMetalPanic`（ID: `FullMetalPanic`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullMetalPanic;

impl GameSystem for FullMetalPanic {
    fn id(&self) -> &'static str {
        "FullMetalPanic"
    }

    fn name(&self) -> &'static str {
        "フルメタル・パニック！RPG"
    }

    fn sort_key(&self) -> &'static str {
        "ふるめたるはにつくRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　・通常判定：2D6+m@c#f>=t または 2D6+m>=t[c,f]
　　修正値m、目標値t、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]ごと省略可、@c・#fの指定は順不同）。
　　クリティカル値、ファンブル値の既定値は、それぞれ12、2です。
　　自動成功、自動失敗、成功、失敗を自動表示します。

　　例) 2d6>=10　　　　　修正値0、目標値10で判定
　　例) 2d6+2>=10　　　　修正値+2、目標値10で判定
　　例) 2d6+2>=10[11]　　↑をクリティカル値11で判定
　　例) 2d6+2@11>=10 　　↑をクリティカル値11で判定
　　例) 2d6+2>=10[12,4]　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4>=10 　↑をクリティカル値12、ファンブル値4で判定
　　例) 2d6+2>=10[,4]　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) 2d6+2#4>=10　　　↑をクリティカル値12、ファンブル値4で判定（クリティカル値の省略）
　　例) MG+2>=10　　　　 2d6+2>=10と同じ（MGが2D6のショートカットコマンド）
　　例) FP+2>=10　　　　 2d6+2>=10と同じ（FPが2D6のショートカットコマンド）

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) MG　　　　　 2d6[]と同じ（MGが2D6のショートカットコマンド）
　　例) MG+2@12#4　　2d6+2@12#4と同じ（MGが2D6のショートカットコマンド）
　　例) FP　　　　　 2d6[]と同じ（FPが2D6のショートカットコマンド）
　　例) FP+2@12#4　　2d6+2@12#4と同じ（FPが2D6のショートカットコマンド）

・D66ダイスあり（入れ替えなし)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6", "MG", "FP"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SRS#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `SRS#initialize` の `@d66_sort_type = D66SortType::NO_SORT`。
    ///
    /// `Base` の既定値と同じだが、原典が明示しているので合わせて書く。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    /// Ruby `SRS#eval_game_system_specific_command`。
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
            .join("test/data/FullMetalPanic.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/FullMetalPanic.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/FullMetalPanic.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("FullMetalPanic.toml must parse");
        assert_eq!(
            data.tests.len(),
            37,
            "case count in test/data/FullMetalPanic.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "FullMetalPanic",
                "unexpected game system in FullMetalPanic.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("FullMetalPanic"), &tc.input, &mut src) {
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
                    "FAIL FullMetalPanic:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} FullMetalPanic cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
