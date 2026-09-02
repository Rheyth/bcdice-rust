//! P4で手書き移植した `lib/bcdice/game_system/NightWizard.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NightWizard#eval_game_system_specific_command`（判定コマンド `aNW+b@x#y$z+c`）
//! - `#parse_nw` / `#parse_2r6`（`ParsedNW` / `Parsed2R6` の `to_s` を含む）
//! - `#roll_nw` / `#roll_once` / `#roll_once_first` / `#fumble_base_number`
//!
//! # サブクラスとの共有
//!
//! Ruby側の `SevenFortressMobius < NightWizard` は `initialize` で
//! `@nw_command` を `"SFM"` に変えるだけなので、その差分だけを [`SystemTables`] に束ね、
//! 判定本体はサブクラス側から使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::Int as I;

// ---------------------------------------------------------------------------
// システムごとの設定
// ---------------------------------------------------------------------------

/// 1システム分の設定。`NightWizard` と `SevenFortressMobius` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `@nw_command`（判定コマンドの語）
    pub(crate) nw_command: &'static str,
    /// `@nw_command` を埋め込んだ `parse_nw` の正規表現。
    ///
    /// キャッシュ用の `OnceLock` はシステムごとに1つ要るので、
    /// ここでは各モジュールが持つ `static` を返す関数ポインタだけを保持する。
    pub(crate) nw_pattern: fn() -> &'static Regex,
}

/// Ruby `parse_nw` の正規表現を `@nw_command` を埋めて組み立てる。
///
/// Ruby: `/^([-+]?\d+)?#{@nw_command}((?:[-+]\d+)+)?(?:@(\d+(?:,\d+)*))?(?:#(\d+(?:,\d+)*))?(?:\$(\d+))?((?:[-+]\d+)+)?(?:([>=]+)(\d+))?$/`
///
/// Rubyの `^`/`$` は行頭・行末だが、`Preprocessor` が最初の空白（改行を含む）より
/// 前しか残さないため、文字列全体のアンカーとして扱ってよい。
///
/// # Panics
///
/// `nw_command` が正規表現として解釈できない場合にパニックする。
/// 呼び出し側は原典どおり `"NW"` / `"SFM"` のような素の語だけを渡すこと。
pub(crate) fn build_nw_pattern(nw_command: &str) -> Regex {
    Regex::new(&format!(
        concat!(
            r"^([-+]?\d+)?{}((?:[-+]\d+)+)?(?:@(\d+(?:,\d+)*))?",
            r"(?:#(\d+(?:,\d+)*))?(?:\$(\d+))?((?:[-+]\d+)+)?(?:([>=]+)(\d+))?$"
        ),
        nw_command
    ))
    .expect("valid regex")
}

/// Ruby `/^2R6m\[...\]/i`。原典どおり末尾はアンカーしない。
fn r2r6_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)^2R6M\[([-+]?\d+(?:[-+]\d+)*)(?:,([-+]?\d+(?:[-+]\d+)*))?\]",
            r"(?:C\[(\d+(?:,\d+)*)\])?(?:F\[(\d+(?:,\d+)*)\])?(?:([>=]+)(\d+))?"
        ))
        .expect("valid regex")
    })
}

// ---------------------------------------------------------------------------
// パース結果
// ---------------------------------------------------------------------------

/// Ruby `NightWizard::Parsed`（`ParsedNW` / `Parsed2R6` の共通部分）。
struct Parsed {
    /// Ruby `critical_numbers`。クリティカルになる出目の一覧
    critical_numbers: Vec<i64>,
    /// Ruby `fumble_numbers`。ファンブルになる出目の一覧
    fumble_numbers: Vec<i64>,
    /// Ruby `prana`。プラーナによる補正のダイス個数
    prana: Option<i64>,
    /// Ruby `active_modify_number`。ファンブルでない時に適用される修正値
    active_modify_number: i64,
    /// Ruby `cmp_op`
    cmp_op: Option<CmpOp>,
    /// Ruby `target_number`
    target_number: Option<i64>,
    /// `ParsedNW` と `Parsed2R6` の差分
    kind: ParsedKind,
}

/// Ruby `ParsedNW` / `Parsed2R6` の差分。
enum ParsedKind {
    /// Ruby `ParsedNW`
    Nw {
        /// Ruby `@command`（＝`@nw_command`）
        command: &'static str,
        /// Ruby `base`。判定の基礎値
        base: i64,
        /// Ruby `modify_number`。修正値
        modify_number: i64,
    },
    /// Ruby `Parsed2R6`
    R2r6 {
        /// Ruby `passive_modify_number`。常に適用される修正値
        passive_modify_number: i64,
    },
}

impl Parsed {
    /// Ruby `ParsedNW#passive_modify_number` / `Parsed2R6#passive_modify_number`。
    fn passive_modify_number(&self) -> i64 {
        match self.kind {
            ParsedKind::Nw {
                base,
                modify_number,
                ..
            } => base.saturating_add(modify_number),
            ParsedKind::R2r6 {
                passive_modify_number,
            } => passive_modify_number,
        }
    }

    /// Ruby `ParsedNW#to_s` / `Parsed2R6#to_s`。
    fn to_s(&self) -> String {
        let criticals = join_numbers(&self.critical_numbers);
        let fumbles = join_numbers(&self.fumble_numbers);
        // Ruby: `"#{@cmp_op}"` は Symbol#to_s（`:>=` → ">="、`:==` → "=="）
        let cmp_op = self.cmp_op.map(CmpOp::symbol_str).unwrap_or_default();
        let target_number = opt_str(self.target_number);

        match self.kind {
            ParsedKind::Nw {
                command,
                base,
                modify_number,
            } => {
                // Ruby: base = @base.zero? ? nil : @base
                let base = if base == 0 {
                    String::new()
                } else {
                    base.to_string()
                };
                // Ruby: dollar = @prana && "$#{@prana}"
                let dollar = self.prana.map(|p| format!("${p}")).unwrap_or_default();
                format!(
                    "{base}{command}{}@{criticals}#{fumbles}{dollar}{}{cmp_op}{target_number}",
                    modifier(&crate::Int::from(modify_number)),
                    modifier(&crate::Int::from(self.active_modify_number)),
                )
            }
            ParsedKind::R2r6 {
                passive_modify_number,
            } => format!(
                "2R6M[{passive_modify_number},{}]C[{criticals}]F[{fumbles}]{cmp_op}{target_number}",
                self.active_modify_number
            ),
        }
    }
}

/// Ruby `NightWizard#parse_nw`。
fn parse_nw(sys: &SystemTables, string: &str) -> Result<Option<Parsed>, EvalError> {
    let Some(m) = (sys.nw_pattern)().captures(string) else {
        return Ok(None);
    };

    Ok(Some(Parsed {
        critical_numbers: split_numbers(m.get(3).map(|v| v.as_str()), 10),
        fumble_numbers: split_numbers(m.get(4).map(|v| v.as_str()), 5),
        prana: m.get(5).map(|v| to_i(v.as_str())),
        active_modify_number: eval_modify_number(m.get(6).map(|v| v.as_str()))?,
        cmp_op: m
            .get(7)
            .and_then(|v| normalize::comparison_operator(v.as_str())),
        target_number: m.get(8).map(|v| to_i(v.as_str())),
        kind: ParsedKind::Nw {
            command: sys.nw_command,
            // Ruby: m[1].to_i（nilなら0）
            base: m.get(1).map_or(0, |v| to_i(v.as_str())),
            modify_number: eval_modify_number(m.get(2).map(|v| v.as_str()))?,
        },
    }))
}

/// Ruby `NightWizard#parse_2r6`。
fn parse_2r6(string: &str) -> Result<Option<Parsed>, EvalError> {
    let Some(m) = r2r6_pattern().captures(string) else {
        return Ok(None);
    };

    Ok(Some(Parsed {
        critical_numbers: split_numbers(m.get(3).map(|v| v.as_str()), 10),
        fumble_numbers: split_numbers(m.get(4).map(|v| v.as_str()), 5),
        prana: None,
        active_modify_number: eval_modify_number(m.get(2).map(|v| v.as_str()))?,
        cmp_op: m
            .get(5)
            .and_then(|v| normalize::comparison_operator(v.as_str())),
        target_number: m.get(6).map(|v| to_i(v.as_str())),
        kind: ParsedKind::R2r6 {
            passive_modify_number: eval_modify_number(m.get(1).map(|v| v.as_str()))?,
        },
    }))
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `NightWizard#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: cmd = parse_nw(string) || parse_2r6(string)
    let parsed = match parse_nw(sys, command)? {
        Some(cmd) => Some(cmd),
        None => parse_2r6(command)?,
    };
    let Some(cmd) = parsed else {
        // Ruby: return nil（接頭辞だけ一致した入力は共通コマンドへ落ちる）
        return Ok(None);
    };

    let (total, interim_expr, status) = roll_nw(&cmd, rng)?;

    // Ruby: total.send(cmd.cmp_op, cmd.target_number) ? "成功" : "失敗"
    // 比較演算子と目標値は正規表現上どちらも同時にしか現れない。
    let result = match (cmd.cmp_op, cmd.target_number) {
        (Some(cmp_op), Some(target_number)) => {
            Some(if cmp_op.apply(&I::from(total), &I::from(target_number)) {
                "成功"
            } else {
                "失敗"
            })
        }
        _ => None,
    };

    // Ruby: [..., status, total.to_s, result].compact.join(" ＞ ")
    let mut sequence = vec![format!("({})", cmd.to_s()), interim_expr];
    if let Some(status) = status {
        sequence.push(status.to_owned());
    }
    sequence.push(total.to_string());
    if let Some(result) = result {
        sequence.push(result.to_owned());
    }

    Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
}

/// Ruby `roll_once` の戻り値（`:critical` / `:fumble` / `nil`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollStatus {
    Critical,
    Fumble,
}

/// Ruby `roll_nw` が読み書きするインスタンス変数。
struct RollState<'a> {
    /// Ruby `@critical_numbers`
    critical_numbers: &'a [i64],
    /// Ruby `@fumble_numbers`
    fumble_numbers: &'a [i64],
    /// Ruby `@total`
    total: i64,
    /// Ruby `@interim_expr`
    interim_expr: String,
    /// Ruby `@status`（クリティカル・ファンブル以外のロールでは上書きされない）
    status: Option<&'static str>,
}

/// Ruby `NightWizard#roll_nw`。戻り値は `[@total, @interim_expr, @status]`。
fn roll_nw(
    parsed: &Parsed,
    rng: &mut Randomizer,
) -> Result<(i64, String, Option<&'static str>), EvalError> {
    let mut state = RollState {
        critical_numbers: &parsed.critical_numbers,
        fumble_numbers: &parsed.fumble_numbers,
        total: 0,
        interim_expr: String::new(),
        status: None,
    };

    // Ruby: status = roll_once_first() → roll_once(true)
    let mut status = roll_once(&mut state, true, rng)?;
    while status == Some(RollStatus::Critical) {
        status = roll_once(&mut state, false, rng)?;
    }

    // Ruby: if status != :fumble && parsed.prana
    // `$0` は Ruby では真値なので、0個のバラバラロール（＝"+0[]"）になる。
    let prana = if status == Some(RollStatus::Fumble) {
        None
    } else {
        parsed.prana
    };
    if let Some(prana) = prana {
        let dice_list = rng.roll_barabara(prana, 6)?;
        let prana_bonus: i64 = dice_list.iter().sum();
        let prana_list = join_numbers(&dice_list);

        state.total = state.total.saturating_add(prana_bonus);
        state
            .interim_expr
            .push_str(&format!("+{prana_bonus}[{prana_list}]"));
    }

    let base = if status == Some(RollStatus::Fumble) {
        fumble_base_number(parsed)
    } else {
        parsed
            .passive_modify_number()
            .saturating_add(parsed.active_modify_number)
    };

    state.total = state.total.saturating_add(base);
    // Ruby: @interim_expr = base.to_s + @interim_expr（基礎値は前置）
    state.interim_expr = format!("{base}{}", state.interim_expr);

    Ok((state.total, state.interim_expr, state.status))
}

/// Ruby `NightWizard#roll_once`。
///
/// ファンブル判定は第1ロール（`roll_once_first`）でしか行われない。
fn roll_once(
    state: &mut RollState<'_>,
    fumbleable: bool,
    rng: &mut Randomizer,
) -> Result<Option<RollStatus>, EvalError> {
    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_value: i64 = dice_list.iter().sum();
    let dice_str = join_numbers(&dice_list);

    if fumbleable && state.fumble_numbers.contains(&dice_value) {
        state.total -= 10;
        state.interim_expr.push_str(&format!("-10[{dice_str}]"));
        state.status = Some("ファンブル");
        Ok(Some(RollStatus::Fumble))
    } else if state.critical_numbers.contains(&dice_value) {
        state.total += 10;
        state.interim_expr.push_str(&format!("+10[{dice_str}]"));
        state.status = Some("クリティカル");
        Ok(Some(RollStatus::Critical))
    } else {
        state.total += dice_value;
        state
            .interim_expr
            .push_str(&format!("+{dice_value}[{dice_str}]"));
        Ok(None)
    }
}

/// Ruby `NightWizard#fumble_base_number`。
fn fumble_base_number(parsed: &Parsed) -> i64 {
    parsed.passive_modify_number()
}

// ---------------------------------------------------------------------------
// 小物
// ---------------------------------------------------------------------------

/// Ruby `ArithmeticEvaluator.eval(expr)`。`nil` も評価不能も 0。
fn eval_modify_number(expr: Option<&str>) -> Result<i64, EvalError> {
    match expr {
        None => Ok(0),
        Some(expr) => Ok(arithmetic::eval(expr, RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)),
    }
}

/// Ruby `m[n] ? m[n].split(',').map(&:to_i) : [default]`。
fn split_numbers(source: Option<&str>, default: i64) -> Vec<i64> {
    match source {
        Some(source) => source.split(',').map(to_i).collect(),
        None => vec![default],
    }
}

/// Ruby `Array#join(",")`。
fn join_numbers(numbers: &[i64]) -> String {
    numbers
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",")
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

/// Ruby `"#{nil}" == ""` / `"#{123}" == "123"`。
fn opt_str(value: Option<i64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// NightWizard 本体
// ---------------------------------------------------------------------------

/// Ruby `NightWizard#initialize` の `@nw_command = "NW"`。
fn nw_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| build_nw_pattern("NW"))
}

/// `NightWizard` の設定一式。
static NW_SYSTEM: SystemTables = SystemTables {
    nw_command: "NW",
    nw_pattern,
};

/// Ruby `BCDice::GameSystem::NightWizard`（ID: `NightWizard`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NightWizard;

impl GameSystem for NightWizard {
    fn id(&self) -> &'static str {
        "NightWizard"
    }

    fn name(&self) -> &'static str {
        "ナイトウィザード The 2nd Edition"
    }

    fn sort_key(&self) -> &'static str {
        "ないとういさあと2"
    }

    fn help_message(&self) -> &'static str {
        r"・判定用コマンド　(aNW+b@x#y$z+c)
　　a : 基本値
　　b : 常時に準じる特技による補正
　　c : 常時以外の特技、および支援効果による補正（ファンブル時には適用されない）
　　x : クリティカル値のカンマ区切り（省略時 10）
　　y : ファンブル値のカンマ区切り（省略時 5）
　　z : プラーナによる達成値補正のプラーナ消費数（ファンブル時には適用されない）
　クリティカル値、ファンブル値が無い場合は1や13などのあり得ない数値を入れてください。
　例）12NW-5@7#2$3 1NW 50nw+5@7,10#2,5 50nw-5+10@7,10#2,5+15+25
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"([-+]?\d+)?NW", "2R6"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&NW_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NightWizard",
            "NightWizard.toml",
            102,
        );
    }
}
