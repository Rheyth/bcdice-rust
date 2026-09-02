//! P4で手書き移植した `lib/bcdice/game_system/NightWizard3rd.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - 親クラス `NightWizard`（`lib/bcdice/game_system/NightWizard.rb`）の判定一式
//!   （`#eval_game_system_specific_command` / `#parse_nw` / `#parse_2r6` / `#roll_nw` /
//!   `#roll_once` / `ParsedNW#to_s` / `Parsed2R6#to_s`）
//! - `NightWizard3rd` 自身の `#fumble_base_number`（ファンブル時も
//!   常時以外の補正を足す点だけが 2nd Edition と違う）
//!
//! # 判定本体の置き場所
//!
//! Ruby側は `NightWizard` クラスに判定本体があり、`NightWizard3rd` は
//! `fumble_base_number` を上書きするだけ。Rust側の [`super::NightWizard`] も移植済みだが、
//! そちらの `SystemTables` が持つ差分は `@nw_command` だけで `fumble_base_number` を
//! 差し替える口が無いため、ここでは判定本体をこのファイルに持つ
//! （`SRS` 系の `TenkaRyouran.rs` / `MetallicGuardian.rs` と同じ扱い）。
//! `NightWizard` 側に `fumble_base_number` の差分が入ったら、そちらへ寄せて重複を畳める。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::Int as I;

/// Ruby `NightWizard#initialize` の `@nw_command`。
const NW_COMMAND: &str = "NW";

// ---------------------------------------------------------------------------
// コマンドのパース結果
// ---------------------------------------------------------------------------

/// Ruby `ParsedNW` / `Parsed2R6` で表記と `passive_modify_number` が違う部分。
enum ParsedKind {
    /// Ruby `ParsedNW`
    Nw {
        /// 判定の基礎値
        base: i64,
        /// 修正値
        modify_number: i64,
    },
    /// Ruby `Parsed2R6`
    R2r6 {
        /// 常に適用される修正値
        passive_modify_number: i64,
    },
}

/// Ruby `NightWizard::Parsed` とその2つのサブクラス。
struct Parsed {
    kind: ParsedKind,
    /// クリティカルになる出目の一覧
    critical_numbers: Vec<i64>,
    /// ファンブルになる出目の一覧
    fumble_numbers: Vec<i64>,
    /// プラーナによる補正
    prana: Option<i64>,
    /// ファンブルでない時に適用される修正値
    active_modify_number: i64,
    /// 比較演算子
    cmp_op: Option<CmpOp>,
    /// 目標値
    target_number: Option<i64>,
}

impl Parsed {
    /// Ruby `ParsedNW#passive_modify_number` / `Parsed2R6#passive_modify_number`。
    fn passive_modify_number(&self) -> i64 {
        match self.kind {
            ParsedKind::Nw {
                base,
                modify_number,
            } => base.wrapping_add(modify_number),
            ParsedKind::R2r6 {
                passive_modify_number,
            } => passive_modify_number,
        }
    }

    /// Ruby `ParsedNW#to_s` / `Parsed2R6#to_s`。
    ///
    /// 比較演算子は `Format.comparison_operator` ではなく **Symbol#to_s** が
    /// そのまま連結される（`:==` は `"=="`）。
    fn to_s(&self) -> String {
        let cmp = self
            .cmp_op
            .map(|op| op.symbol_str().to_owned())
            .unwrap_or_default();
        let target = self
            .target_number
            .map(|t| t.to_string())
            .unwrap_or_default();

        match self.kind {
            ParsedKind::Nw {
                base,
                modify_number,
            } => {
                // Ruby: base = @base.zero? ? nil : @base（0のときは表示しない）
                let base = if base == 0 {
                    String::new()
                } else {
                    base.to_string()
                };
                let dollar = self.prana.map(|p| format!("${p}")).unwrap_or_default();
                format!(
                    "{base}{NW_COMMAND}{}@{}#{}{dollar}{}{cmp}{target}",
                    modifier(&crate::Int::from(modify_number)),
                    join_numbers(&self.critical_numbers),
                    join_numbers(&self.fumble_numbers),
                    modifier(&crate::Int::from(self.active_modify_number)),
                )
            }
            ParsedKind::R2r6 {
                passive_modify_number,
            } => format!(
                "2R6M[{passive_modify_number},{}]C[{}]F[{}]{cmp}{target}",
                self.active_modify_number,
                join_numbers(&self.critical_numbers),
                join_numbers(&self.fumble_numbers),
            ),
        }
    }
}

/// Ruby `Array#join(',')`。
fn join_numbers(numbers: &[i64]) -> String {
    numbers
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `/^([-+]?\d+)?NW((?:[-+]\d+)+)?(?:@(\d+(?:,\d+)*))?(?:#(\d+(?:,\d+)*))?(?:\$(\d+))?((?:[-+]\d+)+)?(?:([>=]+)(\d+))?$/`。
fn nw_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"\A([-+]?\d+)?NW((?:[-+]\d+)+)?(?:@(\d+(?:,\d+)*))?",
            r"(?:#(\d+(?:,\d+)*))?(?:\$(\d+))?((?:[-+]\d+)+)?(?:([>=]+)(\d+))?\z"
        ))
        .expect("valid regex")
    })
}

/// Ruby `/^2R6m\[([-+]?\d+(?:[-+]\d+)*)(?:,([-+]?\d+(?:[-+]\d+)*))?\](?:c\[(\d+(?:,\d+)*)\])?(?:f\[(\d+(?:,\d+)*)\])?(?:([>=]+)(\d+))?/i`。
///
/// 原典は末尾を固定していない（`$` が無い）ので、こちらも `\z` を付けない。
fn r2r6_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)\A2R6m\[([-+]?\d+(?:[-+]\d+)*)(?:,([-+]?\d+(?:[-+]\d+)*))?\]",
            r"(?:c\[(\d+(?:,\d+)*)\])?(?:f\[(\d+(?:,\d+)*)\])?(?:([>=]+)(\d+))?"
        ))
        .expect("valid regex")
    })
}

/// Ruby `ArithmeticEvaluator.eval(expr)`。`nil` も不正な式も0になる。
fn arithmetic_evaluator_eval(expr: Option<&str>) -> Result<i64, EvalError> {
    let Some(expr) = expr else {
        return Ok(0);
    };
    Ok(arithmetic::eval(expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `m[n].split(',').map(&:to_i)`。
fn split_numbers(source: &str) -> Vec<i64> {
    source.split(',').map(to_i).collect()
}

/// Ruby `NightWizard#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let cmd = match parse_nw(command)? {
        Some(cmd) => cmd,
        None => match parse_2r6(command)? {
            Some(cmd) => cmd,
            None => return Ok(None),
        },
    };

    let roll = roll_nw(&cmd, rng)?;

    // Ruby: total.send(cmp_op, cmd.target_number) ? "成功" : "失敗"
    // 比較演算子と目標値は正規表現で同じ任意グループに入っているので、
    // 片方だけが立つことはない（Ruby側も nil 相手の比較は例外になる）。
    let result = match (cmd.cmp_op, cmd.target_number) {
        (Some(cmp_op), Some(target_number)) => Some(
            if cmp_op.apply(&I::from(roll.total), &I::from(target_number)) {
                "成功"
            } else {
                "失敗"
            },
        ),
        _ => None,
    };

    // Ruby: [..., status, total.to_s, result].compact.join(" ＞ ")
    let mut sequence = vec![format!("({})", cmd.to_s()), roll.interim_expr];
    if let Some(status) = roll.status {
        sequence.push(status.to_owned());
    }
    sequence.push(roll.total.to_string());
    if let Some(result) = result {
        sequence.push(result.to_owned());
    }

    Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
}

/// Ruby `NightWizard#parse_nw`。
fn parse_nw(string: &str) -> Result<Option<Parsed>, EvalError> {
    let Some(m) = nw_pattern().captures(string) else {
        return Ok(None);
    };

    Ok(Some(Parsed {
        kind: ParsedKind::Nw {
            // Ruby: m[1].to_i（nil なら 0）
            base: m.get(1).map_or(0, |x| to_i(x.as_str())),
            modify_number: arithmetic_evaluator_eval(m.get(2).map(|x| x.as_str()))?,
        },
        critical_numbers: m
            .get(3)
            .map_or_else(|| vec![10], |x| split_numbers(x.as_str())),
        fumble_numbers: m
            .get(4)
            .map_or_else(|| vec![5], |x| split_numbers(x.as_str())),
        prana: m.get(5).map(|x| to_i(x.as_str())),
        active_modify_number: arithmetic_evaluator_eval(m.get(6).map(|x| x.as_str()))?,
        cmp_op: m
            .get(7)
            .and_then(|x| normalize::comparison_operator(x.as_str())),
        target_number: m.get(8).map(|x| to_i(x.as_str())),
    }))
}

/// Ruby `NightWizard#parse_2r6`。
fn parse_2r6(string: &str) -> Result<Option<Parsed>, EvalError> {
    let Some(m) = r2r6_pattern().captures(string) else {
        return Ok(None);
    };

    Ok(Some(Parsed {
        kind: ParsedKind::R2r6 {
            passive_modify_number: arithmetic_evaluator_eval(m.get(1).map(|x| x.as_str()))?,
        },
        critical_numbers: m
            .get(3)
            .map_or_else(|| vec![10], |x| split_numbers(x.as_str())),
        fumble_numbers: m
            .get(4)
            .map_or_else(|| vec![5], |x| split_numbers(x.as_str())),
        // Ruby: Parsed2R6 は prana を代入しないので常に nil
        prana: None,
        active_modify_number: arithmetic_evaluator_eval(m.get(2).map(|x| x.as_str()))?,
        cmp_op: m
            .get(5)
            .and_then(|x| normalize::comparison_operator(x.as_str())),
        target_number: m.get(6).map(|x| to_i(x.as_str())),
    }))
}

/// Ruby `NightWizard#roll_nw` の戻り値 `[total, interim_expr, status]`。
struct NwRoll {
    total: i64,
    interim_expr: String,
    /// Ruby `@status`。クリティカル・ファンブルの枝でしか代入されないので、
    /// 「クリティカル→通常」の順で振ると `"クリティカル"` が残る（原典どおり）。
    status: Option<&'static str>,
}

/// `roll_once` の戻り値。Ruby のシンボル（`:critical` / `:fumble` / `nil`）。
#[derive(PartialEq, Eq)]
enum RollStatus {
    Critical,
    Fumble,
    Normal,
}

/// Ruby `NightWizard#roll_nw`。
fn roll_nw(parsed: &Parsed, rng: &mut Randomizer) -> Result<NwRoll, EvalError> {
    let mut roll = NwRoll {
        total: 0,
        interim_expr: String::new(),
        status: None,
    };

    // Ruby: status = roll_once_first(); while status == :critical; status = roll_once(); end
    let mut status = roll_once(parsed, &mut roll, true, rng)?;
    while status == RollStatus::Critical {
        status = roll_once(parsed, &mut roll, false, rng)?;
    }

    if status != RollStatus::Fumble {
        if let Some(prana) = parsed.prana {
            let dice_list = rng.roll_barabara(prana, 6)?;
            let prana_bonus: i64 = dice_list.iter().sum();
            let prana_list = join_numbers(&dice_list);

            roll.total += prana_bonus;
            roll.interim_expr
                .push_str(&format!("+{prana_bonus}[{prana_list}]"));
        }
    }

    let base = if status == RollStatus::Fumble {
        fumble_base_number(parsed)
    } else {
        parsed
            .passive_modify_number()
            .wrapping_add(parsed.active_modify_number)
    };

    roll.total += base;
    roll.interim_expr = format!("{base}{}", roll.interim_expr);

    Ok(roll)
}

/// Ruby `NightWizard#roll_once` / `#roll_once_first`。
fn roll_once(
    parsed: &Parsed,
    roll: &mut NwRoll,
    fumbleable: bool,
    rng: &mut Randomizer,
) -> Result<RollStatus, EvalError> {
    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_value: i64 = dice_list.iter().sum();
    let dice_str = join_numbers(&dice_list);

    if fumbleable && parsed.fumble_numbers.contains(&dice_value) {
        roll.total -= 10;
        roll.interim_expr.push_str(&format!("-10[{dice_str}]"));
        roll.status = Some("ファンブル");
        Ok(RollStatus::Fumble)
    } else if parsed.critical_numbers.contains(&dice_value) {
        roll.total += 10;
        roll.interim_expr.push_str(&format!("+10[{dice_str}]"));
        roll.status = Some("クリティカル");
        Ok(RollStatus::Critical)
    } else {
        roll.total += dice_value;
        roll.interim_expr
            .push_str(&format!("+{dice_value}[{dice_str}]"));
        Ok(RollStatus::Normal)
    }
}

/// Ruby `NightWizard3rd#fumble_base_number`。
///
/// 2nd Edition（`NightWizard#fumble_base_number`）は `passive_modify_number` だけだが、
/// 3rd Edition はファンブル時も「常時以外の特技・支援効果による補正」を足す。
fn fumble_base_number(parsed: &Parsed) -> i64 {
    parsed
        .passive_modify_number()
        .wrapping_add(parsed.active_modify_number)
}

/// Ruby `BCDice::GameSystem::NightWizard3rd`（ID: `NightWizard3rd`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NightWizard3rd;

impl GameSystem for NightWizard3rd {
    fn id(&self) -> &'static str {
        "NightWizard3rd"
    }

    fn name(&self) -> &'static str {
        "ナイトウィザード The 3rd Edition"
    }

    fn sort_key(&self) -> &'static str {
        "ないとういさあと3"
    }

    fn help_message(&self) -> &'static str {
        r"・判定用コマンド　(aNW+b@x#y$z+c)
　　a : 基本値
　　b : 常時に準じる特技による補正
　　c : 常時以外の特技、および支援効果による補正
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

    /// Ruby `NightWizard#eval_game_system_specific_command`。
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

    /// `test/data/NightWizard3rd.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "NightWizard3rd",
            "NightWizard3rd.toml",
            3,
        );
    }

    /// TOMLが覆っていない経路（`2R6` 記法・プラーナ `$z`・目標値つき）の固定。
    ///
    /// 期待値は `test/data/NightWizard.toml`（2nd Edition）の同じ入力・同じ乱数の
    /// ケースから採った。`NightWizard3rd` の差分は `fumble_base_number` だけなので、
    /// 「ファンブルしない」ケースと「常時以外の補正が0のケース」は2nd Editionと
    /// 出力が一致する。唯一違うのが最初のケース（ファンブル＋補正+3）で、
    /// 2nd Editionは基礎値5・合計-5、3rd Editionは基礎値8・合計-2になる
    /// （同じ判定を `NW` 記法で書いた `0nw+5@8#6+3` が
    /// `test/data/NightWizard3rd.toml` の1件目と一致する）。
    #[test]
    fn covers_paths_missing_from_toml() {
        let cases = [
            // 2R6記法・ファンブル（3rd Edition ではファンブル時も +3 が乗る）
            (
                "2R6M[0+5,+3]C[8]F[6]",
                vec![(5, 6), (1, 6)],
                "(2R6M[5,3]C[8]F[6]) ＞ 8-10[5,1] ＞ ファンブル ＞ -2",
            ),
            // 2R6記法・通常（2nd Edition と同じ）
            (
                "2R6M[0+5,+3]C[8]F[6]",
                vec![(5, 6), (2, 6)],
                "(2R6M[5,3]C[8]F[6]) ＞ 8+7[5,2] ＞ 15",
            ),
            // 2R6記法・クリティカル（2nd Edition と同じ）
            (
                "2R6M[0+5,+3]C[8]F[6]",
                vec![(5, 6), (3, 6), (5, 6), (2, 6)],
                "(2R6M[5,3]C[8]F[6]) ＞ 8+10[5,3]+7[5,2] ＞ クリティカル ＞ 25",
            ),
            // 2R6記法・目標値つき
            (
                "2R6M[50+5]C[7,10]F[2,5]>=66",
                vec![(5, 6), (6, 6)],
                "(2R6M[55,0]C[7,10]F[2,5]>=66) ＞ 55+11[5,6] ＞ 66 ＞ 成功",
            ),
            (
                "2R6M[50+5]C[7,10]F[2,5]>=66",
                vec![(3, 6), (2, 6)],
                "(2R6M[55,0]C[7,10]F[2,5]>=66) ＞ 55-10[3,2] ＞ ファンブル ＞ 45 ＞ 失敗",
            ),
            // プラーナ（通常時は加算される）
            (
                "12NW-5@7#2$3",
                vec![(5, 6), (1, 6), (1, 6), (2, 6), (3, 6)],
                "(12NW-5@7#2$3) ＞ 7+6[5,1]+6[1,2,3] ＞ 19",
            ),
            // プラーナ（ファンブル時は加算されない＝3個振らない）
            (
                "12NW-5@7#2$3",
                vec![(1, 6), (1, 6)],
                "(12NW-5@7#2$3) ＞ 7-10[1,1] ＞ ファンブル ＞ -3",
            ),
            // プラーナ（クリティカル継続後に加算される）
            (
                "12NW-5@7#2$3",
                vec![(5, 6), (2, 6), (5, 6), (5, 6), (1, 6), (2, 6), (3, 6)],
                "(12NW-5@7#2$3) ＞ 7+10[5,2]+10[5,5]+6[1,2,3] ＞ クリティカル ＞ 33",
            ),
            // 既定のクリティカル値10・ファンブル値5、基礎値0は表記から落ちる
            ("NW", vec![(6, 6), (5, 6)], "(NW@10#5) ＞ 0+11[6,5] ＞ 11"),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("NightWizard3rd"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
