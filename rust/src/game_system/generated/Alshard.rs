//! P4で手書き移植した `lib/bcdice/game_system/Alshard.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! # 親クラス `SRS` の扱い
//!
//! Ruby は `Alshard < SRS`（`lib/bcdice/game_system/SRS.rb`）で、Alshard 自身は
//! `set_aliases_for_srs_roll('AL')` でエイリアスを足すだけである。
//! `SRS` はまだ Rust へ移植されていない（`generated/SRS.rs` はスタブのまま）ので、
//! 判定に必要な `SRS` の実装（`parse` / `parse_legacy` / `execute_srs_roll` /
//! `compare_result` と `SRSRollNode`）はこのファイル内に取り込んである。
//! `SRS` が移植されたら、そちらへ寄せて整理する前提の重複である。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parsed, Parser};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `SRS::DEFAULT_CRITICAL_VALUE`（既定のクリティカル値）。
const DEFAULT_CRITICAL_VALUE: i64 = 12;
/// Ruby `SRS::DEFAULT_FUMBLE_VALUE`（既定のファンブル値）。
const DEFAULT_FUMBLE_VALUE: i64 = 2;

/// Ruby `Alshard.aliases`（`set_aliases_for_srs_roll('AL')`）を含めた
/// `Command::Parser` のコマンド表記。
///
/// Ruby: `Regexp.new(["2D6"].concat(aliases()).join('|'), Regexp::IGNORECASE)`
const NOTATION_PATTERN: &str = "(?i)2D6|AL";

/// i18n `ja_jp.SRS.auto_success`。
const AUTO_SUCCESS: &str = "自動成功";
/// i18n `ja_jp.SRS.auto_failure`。
const AUTO_FAILURE: &str = "自動失敗";
/// i18n `ja_jp.success`。
const SUCCESS: &str = "成功";
/// i18n `ja_jp.failure`。
const FAILURE: &str = "失敗";

/// Ruby `HELP_MESSAGE` 定数（`SRS::ClassMethods#concatenate_help_messages` が
/// エイリアス `AL` の説明を差し込んだもの。スタブ生成時の値をそのまま保つ）。
static HELP_MESSAGE: &str = r"・判定
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
　　例) AL+2>=10　　　　 2d6+2>=10と同じ（ALが2D6のショートカットコマンド）

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) AL　　　　　 2d6[]と同じ（ALが2D6のショートカットコマンド）
　　例) AL+2@12#4　　2d6+2@12#4と同じ（ALが2D6のショートカットコマンド）

・D66ダイスあり（入れ替えなし)
";

/// Ruby `BCDice::GameSystem::Alshard`（ID: `Alshard`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alshard;

impl GameSystem for Alshard {
    fn id(&self) -> &'static str {
        "Alshard"
    }

    fn name(&self) -> &'static str {
        "アルシャード"
    }

    fn sort_key(&self) -> &'static str {
        "あるしやあと"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6", "AL"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `SRS#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `SRS#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: /(.+)\[(.*)\]\z/.match(command)
        let node = match legacy_c_f_pattern().captures(command) {
            Some(m) => parse_legacy(self, &m[1], &m[2]),
            None => parse(self, command),
        };

        match node {
            Some(node) => Ok(Some(SpecificCommandOutput::result(execute_srs_roll(
                self, &node, rng,
            )?))),
            // Ruby: return nil
            None => Ok(None),
        }
    }
}

/// Ruby `SRS#eval_game_system_specific_command` の `/(.+)\[(.*)\]\z/`。
fn legacy_c_f_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(.+)\[(.*)\]$").expect("valid regex"))
}

/// Ruby `parse_legacy` の `/^(-?\d+)?(?:,(-?\d+))?$/`。
fn legacy_values_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(-?\d+)?(?:,(-?\d+))?$").expect("valid regex"))
}

/// Ruby `SRS` の成功判定コマンドのノード（`SRSRollNode`）。
struct SrsRollNode {
    /// 修正値
    modifier: i64,
    /// クリティカル値
    critical_value: i64,
    /// ファンブル値
    fumble_value: i64,
    /// 目標値
    target_value: Option<i64>,
}

impl std::fmt::Display for SrsRollNode {
    /// Ruby `SRSRollNode#to_s`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lhs = format!("2D6{}", modifier(&crate::Int::from(self.modifier)));
        let expression = match self.target_value {
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
fn parse(system: &Alshard, command: &str) -> Option<SrsRollNode> {
    let parser = srs_parser(system)
        .enable_critical()
        .enable_fumble()
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)]);
    let cmd: Parsed = parser.parse(command)?;

    // Ruby: command.start_with?(/2D6/i) なら既定のダイスコマンドへ委ねる
    if starts_with_2d6(command)
        && cmd.critical.is_none()
        && cmd.fumble.is_none()
        && cmd.target_number.is_none()
    {
        return None;
    }

    Some(SrsRollNode {
        modifier: crate::randomizer::sat_i64(&cmd.modify_number),
        // Ruby: cmd.critical ||= DEFAULT_CRITICAL_VALUE
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
        target_value: cmd.target_number.as_ref().map(crate::randomizer::sat_i64),
    })
}

/// Ruby `SRS#parse_legacy`。
fn parse_legacy(system: &Alshard, command: &str, c_f: &str) -> Option<SrsRollNode> {
    let m = legacy_values_pattern().captures(c_f)?;

    // Ruby: m[1]&.to_i || DEFAULT_CRITICAL_VALUE
    let critical = m
        .get(1)
        .map_or(DEFAULT_CRITICAL_VALUE, |x| to_i(x.as_str()));
    let fumble = m.get(2).map_or(DEFAULT_FUMBLE_VALUE, |x| to_i(x.as_str()));

    let parser = srs_parser(system).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)]);
    let cmd: Parsed = parser.parse(command)?;

    Some(SrsRollNode {
        modifier: crate::randomizer::sat_i64(&cmd.modify_number),
        critical_value: critical,
        fumble_value: fumble,
        target_value: cmd.target_number.as_ref().map(crate::randomizer::sat_i64),
    })
}

/// Ruby `Command::Parser.new(prefix_re, round_type: @round_type)`。
fn srs_parser(system: &Alshard) -> Parser {
    Parser::new(&[NOTATION_PATTERN], system.round_type())
}

/// Ruby `String#start_with?(/2D6/i)`。
fn starts_with_2d6(command: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^2D6").expect("valid regex"))
        .is_match(command)
}

/// Ruby `String#to_i`（`-?\d+` にマッチした部分文字列を整数にする）。
///
/// 桁あふれする入力は Ruby だと Bignum のまま比較に使われる。i64 に収まらない
/// 場合は同じ向きに飽和させる。
/// Ruby `String#to_i`。`i64` 範囲外は符号方向に飽和。
fn to_i(text: &str) -> i64 {
    str_helpers::to_i_signed_saturating(text)
}

/// Ruby `SRS#execute_srs_roll`。
fn execute_srs_roll(
    system: &Alshard,
    srs_roll: &SrsRollNode,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let mut dice_list = rng.roll_barabara(2, 6)?;
    if system.sort_add_dice() {
        dice_list.sort_unstable();
    }

    let sum: i64 = dice_list.iter().sum();
    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let modified_sum: crate::Int = crate::Int::from(sum) + srs_roll.modifier;

    let mut result = compare_result(srs_roll, sum, crate::randomizer::sat_i64(&modified_sum));

    // Ruby: parts.compact.join(' ＞ ')
    // 目標値なしで自動成功も自動失敗もしなかった場合、`compare_result` は
    // `Result.new`（text が nil）を返すので compact で落ちる。
    let mut parts = vec![
        format!("({srs_roll})"),
        format!(
            "{sum}[{dice_str}]{}",
            modifier(&crate::Int::from(srs_roll.modifier))
        ),
        modified_sum.to_string(),
    ];
    if !result.text.is_empty() {
        parts.push(result.text.clone());
    }

    result.text = parts.join(" ＞ ");
    Ok(result)
}

/// Ruby `SRS#compare_result`。
fn compare_result(srs_roll: &SrsRollNode, sum: i64, modified_sum: i64) -> EvalResult {
    if sum >= srs_roll.critical_value {
        EvalResult::critical(AUTO_SUCCESS)
    } else if sum <= srs_roll.fumble_value {
        EvalResult::fumble(AUTO_FAILURE)
    } else {
        match srs_roll.target_value {
            // Ruby: Result.new（text は nil）
            None => EvalResult::new(),
            Some(target) if modified_sum >= target => EvalResult::success(SUCCESS),
            Some(_) => EvalResult::failure(FAILURE),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Alshard", "Alshard.toml", 19);
    }
}
