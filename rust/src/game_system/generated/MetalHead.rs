//! P4で手書き移植した `lib/bcdice/game_system/MetalHead.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#change_text`（`AR` → `2D6`、`SR` → `1D100` の書き換え）
//! - `#eval_game_system_specific_command`（`roll_tables` → `CRCsn` → `HR<=`）
//! - `#result_2d6` / `#result_1d100`（絶対成功・絶対失敗）
//! - `#rollHit` / `#getHitResult`（命中判定ロール）
//! - `#mh_crc_table`（戦闘結果チャート）と `TABLES` 3種
//!
//! # `AR` / `SR` 接頭辞について
//!
//! `register_prefix` には `AR` と `SR` があるが、`change_text` が前処理で
//! `2D6` / `1D100` に書き換えてしまうため、この2つが `dice_command` 側で
//! 一致することはない（実際の判定は共通コマンドの加算ロールが行う）。
//! 原典どおりの構造を保つため接頭辞はそのまま残してある。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::range_table::{RangeRollResult, RangeTableItem};
use crate::dice_table::{RangeInc, RangeTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// 入力の書き換え（Ruby `#change_text`）
// ---------------------------------------------------------------------------

/// Ruby `/^(S)?AR/i`。
///
/// Ruby の `^` は **行頭** なので `(?m)` を付ける。`(?i)` は使わない
/// （`regex` クレートの `(?i)` はUnicodeケースフォールディングになり、
/// `K`(U+212A) 等まで拾ってしまう）ので大小を明示して書く。
fn ar_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^([Ss])?[Aa][Rr]").expect("valid regex"))
}

/// Ruby `/^(S)?SR/i`。
fn sr_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^([Ss])?[Ss][Rr]").expect("valid regex"))
}

/// Ruby `#change_text`。
///
/// Ruby のブロック `{ "#{Regexp.last_match(1)}2D6" }` は捕獲が `nil` なら空文字列を
/// 埋めるので、置換文字列は `${1}` を前置するだけでよい（`$1` と書くと `$12D6`
/// が12番目のグループ参照になってしまうため必ず `${1}` と書く）。
fn change_text_impl(text: &str) -> Cow<'_, str> {
    match ar_pattern().replace_all(text, "${1}2D6") {
        Cow::Borrowed(s) => sr_pattern().replace_all(s, "${1}1D100"),
        Cow::Owned(s) => Cow::Owned(sr_pattern().replace_all(&s, "${1}1D100").into_owned()),
    }
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `/\ACRC(\w)(\d+)\z/`。
///
/// Ruby の `\w` / `\d` はASCIIのみなので、明示クラスで書く。
fn crc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^CRC([0-9A-Za-z_])([0-9]+)$").expect("valid regex"))
}

/// Ruby `/\AHR<=(.+)/`。
fn hr_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^HR<=(.+)").expect("valid regex"))
}

/// Ruby `#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = roll_tables(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }

    if let Some(m) = crc_pattern().captures(command) {
        return Ok(Some(SpecificCommandOutput::text(mh_crc_table(
            &m[1], &m[2],
        ))));
    }

    if let Some(m) = hr_pattern().captures(command) {
        // Ruby: ArithmeticEvaluator.eval(..., round_type: @round_type)
        //       不正な式は 0 になる
        let target = arithmetic::eval(&m[1], RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0);
        return Ok(Some(SpecificCommandOutput::text(roll_hit(target, rng)?)));
    }

    Ok(None)
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

// ---------------------------------------------------------------------------
// 命中判定ロール
// ---------------------------------------------------------------------------

/// Ruby `#rollHit`。
fn roll_hit(target: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let total = rng.roll_once(100)?;
    let result_text = get_hit_result(total, target);
    Ok(format!("(1D100<={target}) ＞ {total}{result_text}"))
}

/// Ruby `#getHitResult`。1の位でクリティカル／アクシデントを見る。
fn get_hit_result(total_n: i64, diff: i64) -> &'static str {
    let dice_value = total_n % 100;
    let dice1 = dice_value % 10; // 1の位

    if total_n > diff {
        return " ＞ 失敗";
    }
    if dice1 == 1 {
        return " ＞ 成功（クリティカル）";
    }
    if dice1 == 0 {
        return " ＞ 失敗（アクシデント）";
    }
    " ＞ 成功"
}

// ---------------------------------------------------------------------------
// 戦闘結果チャート
// ---------------------------------------------------------------------------

/// Ruby `table_point`。数値の1の位に対応する部位（0・1は `nil` ＝ 空文字列）。
static TABLE_POINT: &[&str] = &[
    "", // 0
    "", // 1
    "腕部", "腕部", "脚部", "脚部", "胴部", "胴部", "胴部", "頭部",
];

/// Ruby `table_damage` の1エントリ（`{損傷種別 => 下限値}`）。
type DamageStep = (&'static str, i64);

/// Ruby `table_damage[suv]`。耐久レベルごとの損傷種別と下限値（記載順）。
fn damage_table(suv: &str) -> Option<&'static [DamageStep]> {
    Some(match suv {
        "S" => &[
            ("N", 2),
            ("LW", 16),
            ("MD", 46),
            ("MW", 56),
            ("HD", 76),
            ("HW", 96),
            ("MO", 106),
            ("K", 116),
        ],
        "A" => &[("LW", 2), ("MW", 46), ("HW", 76), ("MO", 96), ("K", 116)],
        "B" => &[("LW", 2), ("MW", 36), ("HW", 66), ("MO", 96), ("K", 106)],
        "C" => &[("LW", 2), ("MW", 26), ("HW", 66), ("MO", 86), ("K", 106)],
        "D" => &[("LW", 2), ("MW", 26), ("HW", 46), ("MO", 76), ("K", 96)],
        "E" => &[("LW", 2), ("MW", 26), ("HW", 39), ("MO", 54), ("K", 76)],
        "F" => &[("LW", 2), ("MW", 16), ("HW", 39), ("MO", 54), ("K", 66)],
        "G" => &[("LW", 2), ("MW", 6), ("HW", 16), ("MO", 26), ("K", 39)],
        "M" => &[
            ("0", 2),
            ("1", 22),
            ("2", 42),
            ("3", 62),
            ("4", 82),
            ("5", 92),
            ("6", 102),
            ("8", 112),
        ],
        _ => return None,
    })
}

/// Ruby `#mh_crc_table(suv, num)`。戦闘結果チャートを振る（ダイスは振らない）。
///
/// `num` は正規表現 `(\d+)` にマッチした文字列。Ruby の `String#to_i` は多倍長だが、
/// ここでは飽和させる（実用上到達しない経路）。
fn mh_crc_table(suv: &str, num: &str) -> String {
    const SEPARATOR: &str = " ＞ ";
    let header = format!("戦闘結果チャート{SEPARATOR}{num}");

    let suv = suv.to_uppercase();
    let num_value = num.parse::<i64>().unwrap_or(i64::MAX);
    let mut numbuf = num_value;
    if numbuf < 1 {
        return format!("{header}{SEPARATOR}数値が不正です");
    }

    let num_d1 = numbuf % 10;
    if num_d1 == 1 {
        numbuf = numbuf.saturating_add(1);
    }
    if num_d1 == 0 {
        numbuf = numbuf.saturating_sub(1);
    }
    let num_d1 = numbuf % 10;

    let Some(steps) = damage_table(&suv) else {
        return format!(
            "{header}{SEPARATOR}耐久レベル(SUV)[{suv}]{SEPARATOR}耐久レベル(SUV)の値が不正です"
        );
    };

    // Ruby: 記載順に走査し、下限値以下なら上書きするので最後に一致したものが残る。
    let mut damage_level = "";
    for (level, lower) in steps {
        if *lower <= numbuf {
            damage_level = level;
        }
    }

    let mut result_parts: Vec<String> = Vec::new();
    if numbuf != num_value {
        result_parts.push(numbuf.to_string());
    }

    if suv == "M" {
        result_parts.push("耐物".to_string());
        result_parts.push(format!("HP[{damage_level}]"));
    } else {
        let point = usize::try_from(num_d1)
            .ok()
            .and_then(|i| TABLE_POINT.get(i))
            .copied()
            .unwrap_or("");
        result_parts.push(format!("耐久レベル(SUV)[{suv}]"));
        result_parts.push(format!("部位[{point}] ： 損傷種別[{damage_level}]"));
    }

    format!("{header}{SEPARATOR}{}", result_parts.join(SEPARATOR))
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `TABLE_ROLL_RESULT_FORMATTER`。
///
/// Ruby: `[table.name, result.sum, result.content].join(' ＞ ')`
fn table_roll_result_formatter(table: &RangeTable, result: &RangeRollResult) -> String {
    format!("{} ＞ {} ＞ {}", table.name(), result.sum, result.content)
}

/// Ruby `TABLES['CC']`（クリティカルチャート / 1D10）。
static TABLE_CC_ITEMS: &[RangeTableItem] = &[
    (RangeInc::single(1), "相手は知覚系に多大なダメージを受けた。PERを1にして頭部にHWのダメージ、および心理チェック。"),
    (RangeInc::single(2), "相手の運動神経を断ち切った。DEXを1にして腕部にHWのダメージ、および心理チェック。さらに腕に持っていた武器などは落としてしまう。"),
    (RangeInc::single(3), "相手の移動手段は完全に奪われた。REFを1にして脚部にHWダメージと心理チェック。また、次回からのこちらの攻撃は必ず命中する。"),
    (RangeInc::new(4, 5), "相手の急所に命中。激痛のため気絶した上、胴にHWダメージ。"),
    (RangeInc::single(6), "効果的な一撃。胴にHWダメージ。心理チェック。"),
    (RangeInc::single(7), "効果的な一撃。胴にMOダメージ。心理チェック。"),
    (RangeInc::new(8, 10), "君の一撃は相手の中枢を完全に破壊した。即死である。"),
];
static TABLE_CC: RangeTable = RangeTable::from_dice("クリティカルチャート", 1, 10, TABLE_CC_ITEMS)
    .with_formatter(table_roll_result_formatter);

/// Ruby `TABLES['ACL']`（アクシデントチャート（射撃・投擲） / 1D10）。
static TABLE_ACL_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 7), "ささいなミス。特にペナルティーはない。"),
    (RangeInc::single(8), "不発、またはジャム。弾を取り出さねばならない物は次のターンは射撃できない。"),
    (RangeInc::single(9), "ささいな故障。可能なら次のターンから個別武器のスキルロールで修理を行える。"),
    (RangeInc::single(10), "武器の暴発、または爆発。頭部HWの心理効果ロール。さらに、その武器は破壊されPERとDEXのどちらか、または両方に計2ポイントのマイナスを与える。（遠隔操作の場合、射手への被害は無し）"),
];
static TABLE_ACL: RangeTable =
    RangeTable::from_dice("アクシデントチャート（射撃・投擲）", 1, 10, TABLE_ACL_ITEMS)
        .with_formatter(table_roll_result_formatter);

/// Ruby `TABLES['ACS']`（アクシデントチャート（格闘） / 1D10）。
static TABLE_ACS_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 3), "足を滑らせて転倒し、起き上がるまで相手に+20の命中修正を与える。"),
    (RangeInc::new(4, 6), "手を滑らせて、武器を落とす。素手の時は関係ない。"),
    (RangeInc::new(7, 9), "使用武器の破壊。素手戦闘のときはMWのダメージを受ける。"),
    (RangeInc::single(10), "手を滑らせ、不幸にも武器は飛んでいき、5m以内に人がいれば誰かに刺さるか、または打撃を与えるかもしれない。ランダムに決定し、普通どおり判定を続ける。素手のときは関係ない。"),
];
static TABLE_ACS: RangeTable =
    RangeTable::from_dice("アクシデントチャート（格闘）", 1, 10, TABLE_ACS_ITEMS)
        .with_formatter(table_roll_result_formatter);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &RangeTable)] =
    &[("CC", &TABLE_CC), ("ACL", &TABLE_ACL), ("ACS", &TABLE_ACS)];

// ---------------------------------------------------------------------------
// ゲームシステム
// ---------------------------------------------------------------------------

/// Ruby `BCDice::GameSystem::MetalHead`（ID: `MetalHead`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalHead;

impl GameSystem for MetalHead {
    fn id(&self) -> &'static str {
        "MetalHead"
    }

    fn name(&self) -> &'static str {
        "メタルヘッド"
    }

    fn sort_key(&self) -> &'static str {
        "めたるへつと"
    }

    fn help_message(&self) -> &'static str {
        r"・アビリティロール  AR>=目標値
・スキルロール      SR<=目標値(%)
・命中判定ロール    HR<=目標値(%)

  例）AR>=5
  例）SR<=(40+25)
  例）HR<=(50-10)

  これらのロールで成否、絶対成功/絶対失敗、クリティカル/アクシデントを自動判定します。

・クリティカルチャート  CC
・アクシデントチャート  射撃・投擲用:ACL  格闘用:ACS
・戦闘結果チャート      CRCsn   s=耐久レベル(SUV) n=数値

  例）CRCA61 SUV=Aを対象とした数値61(62に変換される)の戦闘結果を参照する。
  例）CRCM98 対物で数値98の戦闘結果を参照する。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AR", "SR", "HR<=", "CC", "ACT", "ACL", "ACS", "CRC[A-Z]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `MetalHead#change_text`。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        change_text_impl(text)
    }

    /// Ruby `MetalHead#result_2d6`。アビリティロール（`AR`）用。
    fn result_2d6(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil if cmp_op != :>=
        if cmp_op != CmpOp::Ge {
            return None;
        }

        if dice_total >= 12 {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "絶対成功",
            ))))
        } else if dice_total <= 2 {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "絶対失敗",
            ))))
        } else {
            // Ruby: if/elsif に else が無いので nil（＝ `result_ndx` へ）
            None
        }
    }

    /// Ruby `MetalHead#result_1d100`。スキルロール（`SR`）用。
    fn result_1d100(
        &self,
        _total: crate::Int,
        dice_total: i64,
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :<=
        if cmp_op != CmpOp::Le {
            return None;
        }

        if dice_total <= 5 {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "絶対成功",
            ))))
        } else if dice_total >= 96 {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "絶対失敗",
            ))))
        } else {
            None
        }
    }

    /// Ruby `MetalHead#eval_game_system_specific_command`。
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
    use std::path::{Path, PathBuf};

    use super::{TABLE_ACL, TABLE_ACS, TABLE_CC};
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/MetalHead.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MetalHead.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MetalHead.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MetalHead.toml must parse");
        assert_eq!(
            data.tests.len(),
            42,
            "case count in test/data/MetalHead.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MetalHead",
                "unexpected game system in MetalHead.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("MetalHead"), &tc.input, &mut src) {
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
                    "FAIL MetalHead:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MetalHead cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 表が Ruby の `RangeTable` と同じ健全性（隙間・重なり無し）を満たすこと。
    #[test]
    fn tables_are_valid() {
        for table in [&TABLE_CC, &TABLE_ACL, &TABLE_ACS] {
            table
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", table.name()));
        }
    }

    /// TOMLに無い経路の固定。
    ///
    /// - `CRC` の数値が2桁以上／1の位の繰り上げ・繰り下げ
    /// - `HR<=` の式が不正なら目標値0（Ruby `ArithmeticEvaluator.eval` は0を返す）
    /// - `ACT` は接頭辞にはあるが表が無いので `nil`
    #[test]
    fn crc_rounding_and_invalid_hr_target() {
        // 1の位が1なら+1、0なら-1。CRCA61 はヘルプの例（62に変換され、部位は腕部）。
        let mut src = SeededRandomizer::new(vec![]);
        let result = eval_command(&GameSystemId::new("MetalHead"), "CRCA61", &mut src)
            .expect("CRCA61 must not error")
            .expect("CRCA61 must produce output");
        assert_eq!(
            result.text,
            "戦闘結果チャート ＞ 61 ＞ 62 ＞ 耐久レベル(SUV)[A] ＞ 部位[腕部] ： 損傷種別[MW]"
        );

        let mut src = SeededRandomizer::new(vec![]);
        let result = eval_command(&GameSystemId::new("MetalHead"), "CRCM98", &mut src)
            .expect("CRCM98 must not error")
            .expect("CRCM98 must produce output");
        assert_eq!(result.text, "戦闘結果チャート ＞ 98 ＞ 耐物 ＞ HP[5]");

        // 不正な式は 0 になるので、どの出目でも「失敗」。
        let mut src = SeededRandomizer::new(vec![(1, 100)]);
        let result = eval_command(&GameSystemId::new("MetalHead"), "HR<=X", &mut src)
            .expect("HR<=X must not error")
            .expect("HR<=X must produce output");
        assert_eq!(result.text, "(1D100<=0) ＞ 1 ＞ 失敗");

        let mut src = SeededRandomizer::new(vec![]);
        assert!(
            eval_command(&GameSystemId::new("MetalHead"), "ACT", &mut src)
                .expect("must not error")
                .is_none(),
            "ACT must be nil"
        );
    }
}
