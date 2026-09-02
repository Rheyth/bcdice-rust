//! P4で手書き移植した `lib/bcdice/game_system/EtrianOdysseySRS.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! # 親クラス `SRS` の取り込み
//!
//! Ruby側の `EtrianOdysseySRS` は `SRS`（lib/bcdice/game_system/SRS.rb）を継承し、
//! 自身は ID・名前・接頭辞・エイリアス（`EO` / `SQ`）の設定しか持たない。
//! `SRS` はまだRustへ移植されていない（`generated/SRS.rs` はスタブのまま）ので、
//! `SRS` の2D6判定ロジック（`SRSRollNode` / `parse` / `parse_legacy` /
//! `execute_srs_roll` / `compare_result`）をこのファイルへ丸ごと取り込んである。
//! 後日 `SRS` 本体が移植されたら、そちらへ寄せて整理する前提の重複。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parsed, Parser};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int;

/// Ruby `SRS::DEFAULT_CRITICAL_VALUE`。
const DEFAULT_CRITICAL_VALUE: i64 = 12;
/// Ruby `SRS::DEFAULT_FUMBLE_VALUE`。
const DEFAULT_FUMBLE_VALUE: i64 = 2;

/// i18n `SRS/ja_jp.yml` の `SRS.auto_success`。
const AUTO_SUCCESS: &str = "自動成功";
/// i18n `SRS/ja_jp.yml` の `SRS.auto_failure`。
const AUTO_FAILURE: &str = "自動失敗";
/// i18n `ja_jp.yml` の `success`。
const SUCCESS: &str = "成功";
/// i18n `ja_jp.yml` の `failure`。
const FAILURE: &str = "失敗";

/// Ruby `SRS#parse` / `#parse_legacy` が使う
/// `Regexp.new(["2D6"].concat(aliases()).join('|'), Regexp::IGNORECASE)`。
///
/// `aliases` は `set_aliases_for_srs_roll('EO', 'SQ')` で設定された2件。
const PREFIX_RE: &str = r"(?i)2D6|EO|SQ";

/// Ruby `SRS#eval_game_system_specific_command` の `/(.+)\[(.*)\]\z/`。
fn legacy_c_f_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+)\[(.*)\]$").unwrap())
}

/// Ruby `SRS#parse_legacy` の `/^(-?\d+)?(?:,(-?\d+))?$/`。
fn c_f_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(-?\d+)?(?:,(-?\d+))?$").unwrap())
}

/// Ruby `SRS#parse` が使う共通パーサ（`@c` / `#f` 付き）。
fn srs_parser() -> Parser {
    Parser::new(&[PREFIX_RE], RoundType::Floor)
        .enable_critical()
        .enable_fumble()
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
}

/// Ruby `SRS#parse_legacy` が使うパーサ（`@c` / `#f` は無効）。
fn srs_legacy_parser() -> Parser {
    Parser::new(&[PREFIX_RE], RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
}

/// Ruby `SRS::SRSRollNode`。成功判定コマンドのノード。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SrsRollNode {
    modifier: crate::Int,
    critical_value: i64,
    fumble_value: i64,
    target_value: Option<crate::Int>,
}

impl std::fmt::Display for SrsRollNode {
    /// Ruby `SRSRollNode#to_s`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lhs = format!("2D6{}", modifier(&self.modifier));
        let expression = match &self.target_value {
            Some(t) => format!("{lhs}>={t}"),
            None => lhs,
        };
        write!(
            f,
            "{expression}[{},{}]",
            self.critical_value, self.fumble_value
        )
    }
}

/// Ruby `SRS#parse(command)`。
fn parse(command: &str) -> Option<SrsRollNode> {
    let cmd: Parsed = srs_parser().parse(command)?;

    // Ruby: command.start_with?(/2D6/i) && 全て未指定なら汎用2D6へフォールスルー
    if starts_with_2d6(command)
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

/// Ruby `command.start_with?(/2D6/i)`。
fn starts_with_2d6(command: &str) -> bool {
    // `get` はマルチバイト文字の途中で切れる範囲に None を返すので、
    // 先頭3バイトがASCIIでない入力でもパニックしない。
    command
        .get(..3)
        .is_some_and(|head| head.eq_ignore_ascii_case("2D6"))
}

/// Ruby `SRS#parse_legacy(command, c_f)`。
fn parse_legacy(command: &str, c_f: &str) -> Option<SrsRollNode> {
    let caps = c_f_re().captures(c_f)?;

    let critical = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(DEFAULT_CRITICAL_VALUE);
    let fumble = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(DEFAULT_FUMBLE_VALUE);

    let cmd: Parsed = srs_legacy_parser().parse(command)?;

    Some(SrsRollNode {
        modifier: cmd.modify_number,
        critical_value: critical,
        fumble_value: fumble,
        target_value: cmd.target_number,
    })
}

/// Ruby `SRS#compare_result`。
fn compare_result(node: &SrsRollNode, sum: i64, modified_sum: &crate::Int) -> EvalResult {
    if sum >= node.critical_value {
        EvalResult::critical(AUTO_SUCCESS)
    } else if sum <= node.fumble_value {
        EvalResult::fumble(AUTO_FAILURE)
    } else if let Some(target) = &node.target_value {
        if modified_sum >= target {
            EvalResult::success(SUCCESS)
        } else {
            EvalResult::failure(FAILURE)
        }
    } else {
        // Ruby: Result.new（text は nil）
        EvalResult::new()
    }
}

/// Ruby `SRS#execute_srs_roll`。
fn execute_srs_roll(node: &SrsRollNode, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let mut dice_list = rng.roll_barabara(2, 6)?;
    // Ruby: dice_list.sort! if @sort_add_dice（SRS は @sort_add_dice = true）
    dice_list.sort_unstable();

    let sum: i64 = dice_list.iter().sum();
    let dice_str = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let modified_sum = Int::from(sum) + &node.modifier;

    let mut result = compare_result(node, sum, &modified_sum);

    // Ruby: parts.compact.join(' ＞ ')。`Result.new` の text は nil なので除かれる。
    let mut parts = vec![
        format!("({node})"),
        format!("{sum}[{dice_str}]{}", modifier(&node.modifier)),
        modified_sum.to_string(),
    ];
    if !result.text.is_empty() {
        parts.push(std::mem::take(&mut result.text));
    }

    result.text = parts.join(" ＞ ");
    Ok(result)
}

/// Ruby `BCDice::GameSystem::EtrianOdysseySRS`（ID: `EtrianOdysseySRS`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtrianOdysseySRS;

impl GameSystem for EtrianOdysseySRS {
    fn id(&self) -> &'static str {
        "EtrianOdysseySRS"
    }

    fn name(&self) -> &'static str {
        "世界樹の迷宮SRS"
    }

    fn sort_key(&self) -> &'static str {
        "せかいしゆのめいきゆうSRS"
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
　　例) EO+2>=10　　　　 2d6+2>=10と同じ（EOが2D6のショートカットコマンド）
　　例) SQ+2>=10　　　　 2d6+2>=10と同じ（SQが2D6のショートカットコマンド）

　・クリティカルおよびファンブルのみの判定：2D6+m@c#f または 2D6+m[c,f]
　　目標値を指定せず、修正値m、クリティカル値c、ファンブル値fで判定ロールを行います。
　　修正値、クリティカル値、ファンブル値は省略可能です（[]は省略不可、@c・#fの指定は順不同）。
　　自動成功、自動失敗を自動表示します。

　　例) 2d6[]　　　　修正値0、クリティカル値12、ファンブル値2で判定
　　例) 2d6+2[11]　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2@11 　　修正値+2、クリティカル値11、ファンブル値2で判定
　　例) 2d6+2[12,4]　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) 2d6+2@12#4 　修正値+2、クリティカル値12、ファンブル値4で判定
　　例) EO　　　　　 2d6[]と同じ（EOが2D6のショートカットコマンド）
　　例) EO+2@12#4　　2d6+2@12#4と同じ（EOが2D6のショートカットコマンド）
　　例) SQ　　　　　 2d6[]と同じ（SQが2D6のショートカットコマンド）
　　例) SQ+2@12#4　　2d6+2@12#4と同じ（SQが2D6のショートカットコマンド）

・D66ダイスあり（入れ替えなし)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["2D6", "EO", "SQ"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `SRS#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let node = match legacy_c_f_re().captures(command) {
            Some(caps) => parse_legacy(&caps[1], &caps[2]),
            None => parse(command),
        };

        match node {
            Some(node) => Ok(Some(SpecificCommandOutput::result(execute_srs_roll(
                &node, rng,
            )?))),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EtrianOdysseySRS",
            "EtrianOdysseySRS.toml",
            33,
        );
    }
}
