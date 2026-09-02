//! P4で手書き移植した `lib/bcdice/game_system/OracleEngine.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `OracleEngine#eval_game_system_specific_command` の振り分け
//! - `#clutch_roll`（クラッチロール `xCL+y>=z` / `xCL7+y>=z`）
//! - `#r_roll`（判定 `xR6+y@c#f$b>=z`）
//! - `#damage_roll`（ダメージロールのダイスブレイク `xD6+y$b`）
//!
//! Ruby側は `@cmd` / `@times` / `@max_shift` / `@critical` / `@fumble` / `@break` を
//! インスタンス変数で持ち回るが、ここでは各ロールの文脈構造体
//! （[`ClutchRoll`] / [`RRoll`] / [`DamageRoll`]）に束ねて関数へ渡す。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parsed, Parser};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::OracleEngine`（ID: `OracleEngine`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleEngine;

impl GameSystem for OracleEngine {
    fn id(&self) -> &'static str {
        "OracleEngine"
    }

    fn name(&self) -> &'static str {
        "オラクルエンジン"
    }

    fn sort_key(&self) -> &'static str {
        "おらくるえんしん"
    }

    fn help_message(&self) -> &'static str {
        r"  ・クラッチロール （xCL+y>=z)
  ダイスをx個振り、1個以上目標シフトzに到達したか判定します。修正yは全てのダイスにかかります。
  成功した時は目標シフトを、失敗した時はダイスの最大値-1シフトを返します
  zが指定されないときは、ダイスをx個を振り、それに修正yしたものを返します。
  通常、最低シフトは1、最大シフトは6です。目標シフトもそろえられます。
  また、CLの後に7を入れ、(xCL7+y>=z)と入力すると最大シフトが7になります。
 ・判定 (xR6+y@c#f$b>=z)
  ダイスをx個振り、大きいもの2つだけを見て達成値を算出し、成否を判定します。修正yは達成値にかかります。
  ダイスブレイクとしてbを、クリティカル値としてcを、ファンブル値としてfを指定できます。
  それぞれ指定されない時、0,12,2になります。
  クリティカル値の上限はなし、下限は2。ファンブル値の上限は12、下限は0。
  zが指定されないとき、達成値の算出のみ行います。
 ・ダメージロールのダイスブレイク (xD6+y$b)
  ダイスをx個振り、合計値を出します。修正yは合計値にかかります。
  ダイスブレイクとしてbを指定します。合計値は0未満になりません。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+CL", r"\d+R6", r"\d+D6.*\$[\+\-]?\d+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `OracleEngine#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `OracleEngine#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `OracleEngine#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let text = if clutch_dispatch_pattern().is_match(command) {
            clutch_roll(command, rng)?
        } else if damage_dispatch_pattern().is_match(command) {
            damage_roll(command, rng)?
        } else if r_dispatch_pattern().is_match(command) {
            r_roll(command, rng)?
        } else {
            None
        };
        Ok(text.map(SpecificCommandOutput::text))
    }
}

// ---------------------------------------------------------------------------
// 振り分け
// ---------------------------------------------------------------------------

/// Ruby `when /\d+CL.*/i`。
fn clutch_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\d+CL.*").expect("valid regex"))
}

/// Ruby `when /\d+D6.*\$[+-]?\d.*/`。
fn damage_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+D6.*\$[+-]?\d.*").expect("valid regex"))
}

/// Ruby `when /\d+R6/`。
fn r_dispatch_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+R6").expect("valid regex"))
}

// ---------------------------------------------------------------------------
// 共通の部品
// ---------------------------------------------------------------------------

/// Ruby `String#to_i`（先頭の数字列。空なら 0）。`i64` 範囲外は `i64::MAX` に飽和。
fn to_i(s: &str) -> i64 {
    str_helpers::leading_digits_to_i_max(s)
}

/// Ruby `OracleEngine#clamp`。
fn clamp(i: i64, min: i64, max: i64) -> i64 {
    if i < min {
        min
    } else if i > max {
        max
    } else {
        i
    }
}

/// Ruby `dice_list.pop(n)`：末尾（ソート済みなら大きい方）から `n` 個を取り出す。
///
/// `n` が要素数を超えるときは全部取り出す（Ruby `Array#pop(n)` と同じ）。
fn pop_n(list: &mut Vec<i64>, n: u64) -> Vec<i64> {
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    let keep = list.len().saturating_sub(n);
    list.split_off(keep)
}

/// Ruby `#{@cmd.target_number}`（`nil` なら空文字列）。
fn target_text(cmd: &Parsed) -> String {
    cmd.target_number
        .as_ref()
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Ruby `#dice_result_r` / `#result_damage` 共通の出目表示。
///
/// `dice_total[出目]×[ブレイクした出目]修正値`。ブレイクが無ければ `×[…]` は付かない。
fn dice_result_text(
    dice_total: i64,
    dice_list: &[i64],
    break_list: &[i64],
    modify_number: &crate::Int,
) -> String {
    let modify_number_text = format::modifier(modify_number);

    if break_list.is_empty() {
        format!(
            "{dice_total}[{}]{modify_number_text}",
            dice_text::join_dice_with_comma_space(dice_list)
        )
    } else {
        format!(
            "{dice_total}[{}]×[{}]{modify_number_text}",
            dice_text::join_dice_with_comma_space(dice_list),
            dice_text::join_dice_with_comma_space(break_list)
        )
    }
}

// ---------------------------------------------------------------------------
// クラッチロール
// ---------------------------------------------------------------------------

/// Ruby `#clutch_roll` の文脈（`@cmd` / `@times` / `@max_shift`）。
struct ClutchRoll {
    cmd: Parsed,
    times: i64,
    max_shift: i64,
}

/// Ruby `OracleEngine#clutch_roll`。
fn clutch_roll(string: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: round_type は Base 既定の :floor
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+CL[67]?"], RoundType::Floor).restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });

    let Some(mut cmd) = parser.parse(string) else {
        return Ok(None);
    };

    // Ruby: @times, @max_shift = @cmd.command.split("CL").map(&:to_i)
    //       @max_shift ||= 6
    // Ruby の String#split は末尾の空要素を落とすので、"2CL" は ["2"] になる
    let mut parts = cmd.command.split("CL");
    let times = parts.next().map_or(0, to_i);
    let max_shift = parts.next().filter(|s| !s.is_empty()).map_or(6, to_i);

    // Ruby: @cmd.target_number = clamp(@cmd.target_number, 1, @max_shift) if @cmd.cmp_op
    if cmd.cmp_op.is_some() {
        cmd.target_number = cmd
            .target_number
            .map(|t| clamp(crate::randomizer::sat_i64(&t), 1, max_shift).into());
    }

    if times == 0 {
        return Ok(None);
    }

    let ctx = ClutchRoll {
        cmd,
        times,
        max_shift,
    };

    let mut dice_list: Vec<i64> = rng
        .roll_barabara(ctx.times, 6)?
        .into_iter()
        .map(|x| {
            clamp(
                x.saturating_add(crate::randomizer::sat_i64(&ctx.cmd.modify_number)),
                1,
                ctx.max_shift,
            )
        })
        .collect();
    dice_list.sort_unstable();

    // Ruby: result_clutch(dice_list.last)（ダイスが振られなかったときは nil）
    let Some(result) = result_clutch(&ctx, dice_list.last().copied()) else {
        return Ok(None);
    };

    let sequence = [
        expr_clutch(&ctx),
        format!("[{}]", dice_text::join_dice_with_comma_space(&dice_list)),
        result,
    ];

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `OracleEngine#expr_clutch`。
fn expr_clutch(ctx: &ClutchRoll) -> String {
    let max_shift = if ctx.max_shift == 7 { "7" } else { "" };
    let cmp_op = format::comparison_operator(ctx.cmd.cmp_op);
    let modify_number = format::modifier(&ctx.cmd.modify_number);

    format!(
        "({}CL{max_shift}{modify_number}{cmp_op}{})",
        ctx.times,
        target_text(&ctx.cmd)
    )
}

/// Ruby `OracleEngine#result_clutch`。
///
/// `after_shift` が `None`（ダイスが1個も振られなかった）のときは、Ruby だと
/// 目標値付きなら `nil >= target` で `NoMethodError` になる（→ `None`）。
/// 目標値なしなら `"シフト#{nil}"` ＝ `"シフト"` が返る。
fn result_clutch(ctx: &ClutchRoll, after_shift: Option<i64>) -> Option<String> {
    match (ctx.cmd.cmp_op, ctx.cmd.target_number.clone()) {
        (Some(CmpOp::Ge), Some(target_number)) => {
            let after_shift = after_shift?;
            if crate::Int::from(after_shift) >= target_number {
                Some(format!("成功 シフト{target_number}"))
            } else {
                let after_shift = (after_shift - 1).max(1);
                Some(format!("失敗 シフト{after_shift}"))
            }
        }
        _ => Some(format!(
            "シフト{}",
            after_shift.map(|v| v.to_string()).unwrap_or_default()
        )),
    }
}

// ---------------------------------------------------------------------------
// 判定
// ---------------------------------------------------------------------------

/// Ruby `#r_roll` の文脈（`@cmd` / `@times` / `@critical` / `@fumble` / `@break`）。
struct RRoll {
    cmd: Parsed,
    times: i64,
    critical: i64,
    fumble: i64,
    /// Ruby `(@cmd.dollar || 0).abs`
    dice_break: u64,
}

/// Ruby `OracleEngine#r_roll`。
fn r_roll(string: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+R6"], RoundType::Floor)
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
            .enable_critical()
            .enable_fumble()
            .enable_dollar()
    });

    let Some(cmd) = parser.parse(string) else {
        return Ok(None);
    };

    // Ruby: @times = @cmd.command.to_i
    let times = to_i(&cmd.command);
    if times == 0 {
        return Ok(None);
    }

    let critical = normalize_critical(
        cmd.critical
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(12),
        string,
    );
    let fumble = normalize_fumble(
        cmd.fumble
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(2),
        string,
    );
    let dice_break =
        crate::randomizer::sat_i64(&cmd.dollar.clone().unwrap_or(0.into())).unsigned_abs();

    let ctx = RRoll {
        cmd,
        times,
        critical,
        fumble,
        dice_break,
    };

    let mut dice_list = rng.roll_barabara(ctx.times, 6)?;
    dice_list.sort_unstable();
    let dice_broken = pop_n(&mut dice_list, ctx.dice_break);

    // ブレイク後のダイスから最大値２つの合計がダイスの値
    let dice_total: i64 = dice_list.iter().rev().take(2).sum();
    let total = dice_total + ctx.cmd.modify_number.clone();

    let sequence = [
        expr_r(&ctx),
        dice_result_text(dice_total, &dice_list, &dice_broken, &ctx.cmd.modify_number),
        result_r(&ctx, dice_total, crate::randomizer::sat_i64(&total)),
    ];

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `OracleEngine#expr_r`。
fn expr_r(ctx: &RRoll) -> String {
    let modify_number = format::modifier(&ctx.cmd.modify_number);
    let critical = if ctx.critical == 12 {
        String::new()
    } else {
        format!("c[{}]", ctx.critical)
    };
    let fumble = if ctx.fumble == 2 {
        String::new()
    } else {
        format!("f[{}]", ctx.fumble)
    };
    let brak = if ctx.dice_break == 0 {
        String::new()
    } else {
        format!("b[{}]", ctx.dice_break)
    };
    let cmp_op = format::comparison_operator(ctx.cmd.cmp_op);

    format!(
        "({}R6{modify_number}{critical}{fumble}{brak}{cmp_op}{})",
        ctx.times,
        target_text(&ctx.cmd)
    )
}

/// Ruby `OracleEngine#result_r`。
fn result_r(ctx: &RRoll, dice_total: i64, total: i64) -> String {
    if dice_total <= ctx.fumble {
        "ファンブル!".to_owned()
    } else if dice_total >= ctx.critical {
        "クリティカル!".to_owned()
    } else {
        match (ctx.cmd.cmp_op, ctx.cmd.target_number.clone()) {
            (Some(CmpOp::Ge), Some(target_number)) => {
                if total >= crate::randomizer::sat_i64(&target_number) {
                    format!("{total} 成功")
                } else {
                    format!("{total} 失敗")
                }
            }
            _ => total.to_string(),
        }
    }
}

/// Ruby `/@[+-]/`。
fn critical_offset_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"@[+-]").expect("valid regex"))
}

/// Ruby `/#[+-]/`。
fn fumble_offset_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"#[+-]").expect("valid regex"))
}

/// Ruby `OracleEngine#normalize_critical`。
///
/// `@+n` / `@-n` は 12 からの相対指定。下限は 2、上限なし。
fn normalize_critical(critical: i64, string: &str) -> i64 {
    let critical = if critical_offset_pattern().is_match(string) {
        critical.saturating_add(12)
    } else {
        critical
    };

    critical.max(2)
}

/// Ruby `OracleEngine#normalize_fumble`。
///
/// `#+n` / `#-n` は 2 からの相対指定。`0..=12` に丸める。
fn normalize_fumble(fumble: i64, string: &str) -> i64 {
    let fumble = if fumble_offset_pattern().is_match(string) {
        fumble.saturating_add(2)
    } else {
        fumble
    };

    clamp(fumble, 0, 12)
}

// ---------------------------------------------------------------------------
// ダメージロール
// ---------------------------------------------------------------------------

/// Ruby `#damage_roll` の文脈（`@cmd` / `@times` / `@break`）。
struct DamageRoll {
    cmd: Parsed,
    times: i64,
    /// Ruby `(@cmd.dollar || 0).abs`
    dice_break: u64,
}

/// Ruby `OracleEngine#damage_roll`。
fn damage_roll(string: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&[r"\d+D6"], RoundType::Floor)
            .restrict_cmp_op_to(&[None])
            .enable_dollar()
    });

    let Some(cmd) = parser.parse(string) else {
        return Ok(None);
    };

    let times = to_i(&cmd.command);
    let dice_break =
        crate::randomizer::sat_i64(&cmd.dollar.clone().unwrap_or(0.into())).unsigned_abs();

    if times == 0 {
        return Ok(None);
    }

    let ctx = DamageRoll {
        cmd,
        times,
        dice_break,
    };

    let mut dice_list = rng.roll_barabara(ctx.times, 6)?;
    dice_list.sort_unstable();
    let dice_broken = pop_n(&mut dice_list, ctx.dice_break);

    let dice_total: i64 = dice_list.iter().sum();
    // Ruby: total_n = 0 if total_n < 0
    let total_n = (crate::Int::from(dice_total) + &ctx.cmd.modify_number).max(0.into());

    let sequence = [
        expr_damage(&ctx),
        dice_result_text(dice_total, &dice_list, &dice_broken, &ctx.cmd.modify_number),
        total_n.to_string(),
    ];

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `OracleEngine#expr_damage`。
fn expr_damage(ctx: &DamageRoll) -> String {
    let modify_number = format::modifier(&ctx.cmd.modify_number);
    let brak = if ctx.dice_break == 0 {
        String::new()
    } else {
        format!("b[{}]", ctx.dice_break)
    };

    format!("({}D6{modify_number}{brak})", ctx.times)
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
            .join("test/data/OracleEngine.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/OracleEngine.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/OracleEngine.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("OracleEngine.toml must parse");
        assert_eq!(
            data.tests.len(),
            116,
            "case count in test/data/OracleEngine.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "OracleEngine",
                "unexpected game system in OracleEngine.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("OracleEngine"), &tc.input, &mut src) {
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
                    "FAIL OracleEngine:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} OracleEngine cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
