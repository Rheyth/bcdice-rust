//! P4で手書き移植した `lib/bcdice/game_system/SRS.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `SRS#eval_game_system_specific_command`（`[c,f]` 記法と `@c#f` 記法の振り分け）
//! - `SRS#parse` / `SRS#parse_legacy` / `SRS#execute_srs_roll` / `SRS#compare_result`
//!
//! `SRS.set_aliases_for_srs_roll` によるエイリアス（`2D6` のショートカット）は
//! `SRS` / `SRS_Korean` 自身では未設定（`aliases` は空）なので、判定に使う接頭辞は
//! `2D6` だけになる。エイリアスを設定する派生システム（`Arianrhod` など）は
//! 別バッチの担当。
//!
//! `translate` する文言は [`Translations`] に切り出してあり、`ko_kr` ロケールの
//! [`super::SRS_Korean`] が同じ実装を別の文言で使う。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::Parser;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `translate(...)` が引くロケール別の文言。
pub(crate) struct Translations {
    /// i18n `SRS.auto_success`
    pub auto_success: &'static str,
    /// i18n `SRS.auto_failure`
    pub auto_failure: &'static str,
    /// i18n `success`
    pub success: &'static str,
    /// i18n `failure`
    pub failure: &'static str,
}

/// i18n `ja_jp`（`i18n/SRS/ja_jp.yml` と `i18n/ja_jp.yml`）。
pub(crate) static JA_JP: Translations = Translations {
    auto_success: "自動成功",
    auto_failure: "自動失敗",
    success: "成功",
    failure: "失敗",
};

/// Ruby `SRS::DEFAULT_CRITICAL_VALUE`。
const DEFAULT_CRITICAL_VALUE: i64 = 12;
/// Ruby `SRS::DEFAULT_FUMBLE_VALUE`。
const DEFAULT_FUMBLE_VALUE: i64 = 2;

/// Ruby `BCDice::GameSystem::SRS`（ID: `SRS`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SRS;

impl GameSystem for SRS {
    fn id(&self) -> &'static str {
        "SRS"
    }

    fn name(&self) -> &'static str {
        "スタンダードRPGシステム"
    }

    fn sort_key(&self) -> &'static str {
        "すたんたあとRPGしすてむ"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng, &JA_JP)
    }
}

/// Ruby `SRS#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
    tr: &Translations,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: legacy_c_f_match = /(.+)\[(.*)\]\z/.match(command)
    let node = match legacy_c_f_pattern().captures(command) {
        Some(m) => parse_legacy(&m[1], &m[2]),
        None => parse(command),
    };

    let Some(node) = node else {
        return Ok(None);
    };

    Ok(Some(SpecificCommandOutput::result(execute_srs_roll(
        &node, rng, tr,
    )?)))
}

/// Ruby `/(.+)\[(.*)\]\z/`（旧記法 `2D6+m>=t[c,f]` の切り出し）。
fn legacy_c_f_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(.+)\[(.*)\]\z").expect("valid regex"))
}

/// Ruby `/^(-?\d+)?(?:,(-?\d+))?$/`（`[c,f]` の中身）。
fn c_f_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(-?\d+)?(?:,(-?\d+))?$").expect("valid regex"))
}

/// Ruby `SRS::SRSRollNode`（成功判定コマンドのノード）。
struct SrsRollNode {
    modifier: crate::Int,
    critical_value: i64,
    fumble_value: i64,
    target_value: Option<crate::Int>,
}

impl std::fmt::Display for SrsRollNode {
    /// Ruby `SRSRollNode#to_s`。目標値の有無にかかわらず `[c,f]` を必ず出す。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lhs = format!("2D6{}", format::modifier(&self.modifier));
        let expression = match &self.target_value {
            Some(target) => format!("{lhs}>={target}"),
            None => lhs,
        };
        write!(
            f,
            "{expression}[{},{}]",
            self.critical_value, self.fumble_value
        )
    }
}

/// Ruby `SRS#parse`。
///
/// Ruby側の `prefix_re` は `["2D6"].concat(aliases()).join('|')` だが、
/// `SRS` / `SRS_Korean` の `aliases` は空なので `2D6` だけになる。
fn parse(command: &str) -> Option<SrsRollNode> {
    let parser = Parser::new(&["2D6"], RoundType::Floor)
        .enable_critical()
        .enable_fumble()
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)]);
    let cmd = parser.parse(command)?;

    // Ruby: 素の 2D6 でクリティカル値・ファンブル値・目標値のどれも無ければ
    // 通常の加算ダイスへフォールバックさせる（入力は upcase 済み）
    if command.starts_with("2D6")
        && cmd.critical.is_none()
        && cmd.fumble.is_none()
        && cmd.target_number.is_none()
    {
        return None;
    }

    Some(SrsRollNode {
        modifier: cmd.modify_number,
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

/// Ruby `SRS#parse_legacy`。
fn parse_legacy(command: &str, c_f: &str) -> Option<SrsRollNode> {
    let m = c_f_pattern().captures(c_f)?;
    let critical = m
        .get(1)
        .map_or(DEFAULT_CRITICAL_VALUE, |x| to_i(x.as_str()));
    let fumble = m.get(2).map_or(DEFAULT_FUMBLE_VALUE, |x| to_i(x.as_str()));

    let parser =
        Parser::new(&["2D6"], RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)]);
    let cmd = parser.parse(command)?;

    Some(SrsRollNode {
        modifier: cmd.modify_number,
        critical_value: critical,
        fumble_value: fumble,
        target_value: cmd.target_number,
    })
}

/// Ruby `String#to_i`。桁あふれは飽和させる（Ruby は Bignum になる）。
fn to_i(text: &str) -> i64 {
    text.parse().unwrap_or(if text.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `SRS#execute_srs_roll`。
fn execute_srs_roll(
    srs_roll: &SrsRollNode,
    rng: &mut Randomizer,
    tr: &Translations,
) -> Result<EvalResult, EvalError> {
    let mut dice_list = rng.roll_barabara(2, 6)?;
    // Ruby: dice_list.sort! if @sort_add_dice（SRS は常に true）
    dice_list.sort_unstable();

    let sum: i64 = dice_list.iter().sum();
    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let modified_sum: crate::Int = crate::Int::from(sum) + &srs_roll.modifier;

    let mut result = compare_result(srs_roll, sum, modified_sum.clone(), tr);

    // Ruby: parts.compact.join(' ＞ ')。Result.new の text は nil なので落ちる。
    let mut parts = vec![
        format!("({srs_roll})"),
        format!("{sum}[{dice_str}]{}", format::modifier(&srs_roll.modifier)),
        modified_sum.to_string(),
    ];
    if !result.text.is_empty() {
        parts.push(result.text.clone());
    }

    result.text = parts.join(" ＞ ");
    Ok(result)
}

/// Ruby `SRS#compare_result`。
fn compare_result(
    srs_roll: &SrsRollNode,
    sum: i64,
    modified_sum: crate::Int,
    tr: &Translations,
) -> EvalResult {
    if sum >= srs_roll.critical_value {
        EvalResult::critical(tr.auto_success)
    } else if sum <= srs_roll.fumble_value {
        EvalResult::fumble(tr.auto_failure)
    } else {
        match &srs_roll.target_value {
            None => EvalResult::new(),
            Some(target) if modified_sum >= *target => EvalResult::success(tr.success),
            Some(_) => EvalResult::failure(tr.failure),
        }
    }
}

/// Ruby `HELP_MESSAGE` 定数（`DEFAULT_HELP_MESSAGE`）。
const HELP_MESSAGE: &str = r"・判定
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

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定

・D66ダイスあり（入れ替えなし)
";

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
            .join("test/data/SRS.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/SRS.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/SRS.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("SRS.toml must parse");
        assert_eq!(data.tests.len(), 74, "case count in test/data/SRS.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "SRS", "unexpected game system in SRS.toml");

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("SRS"), &tc.input, &mut src) {
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
                    "FAIL SRS:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} SRS cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
