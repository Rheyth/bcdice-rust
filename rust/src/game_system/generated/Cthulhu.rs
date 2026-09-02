//! P4で手書き移植した `lib/bcdice/game_system/Cthulhu.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Cthulhu#eval_game_system_specific_command`（`case` による CCB/CC/RESB/CBRB/RES/CBR の振り分け）
//! - `#getCheckResult` / `#getRegistResult` / `#getCombineRoll`
//! - `#compare` と `Cthulhu::CompareResult`（`#text` / `#to_result`）
//!
//! # ロケール
//!
//! Ruby側は `translate("Cthulhu.critical")` などが `@locale` を見る。
//! `Cthulhu` 本体は `ja_jp`、`Cthulhu_English` など4つのバリアントは
//! それぞれ `en_us` / `ko_kr` / `zh_hans` / `zh_hant` を使うが、**違いは文言だけ**で
//! 判定ロジックは完全に共通なので、文言を [`Locale`] に束ねて
//! [`eval_specific_command`] を共有する（Ruby側で `Cthulhu_English < Cthulhu` などと
//! なっているのに対応する）。
//!
//! [`JA_JP`] の値は `i18n/Cthulhu/ja_jp.yml` と `i18n/ja_jp.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ロケールごとの文言
// ---------------------------------------------------------------------------

/// 1ロケール分の文言。`Cthulhu` と4つのバリアントはこれだけが違う。
pub(crate) struct Locale {
    /// i18n `success`（`i18n/<locale>.yml`）
    pub(crate) success: &'static str,
    /// i18n `failure`（`i18n/<locale>.yml`）
    pub(crate) failure: &'static str,
    /// i18n `Cthulhu.critical`
    pub(crate) critical: &'static str,
    /// i18n `Cthulhu.special`
    pub(crate) special: &'static str,
    /// i18n `Cthulhu.critical_special`
    pub(crate) critical_special: &'static str,
    /// i18n `Cthulhu.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `Cthulhu.partial_success`
    pub(crate) partial_success: &'static str,
    /// i18n `Cthulhu.automatic_success`
    pub(crate) automatic_success: &'static str,
    /// i18n `Cthulhu.automatic_failure`
    pub(crate) automatic_failure: &'static str,
    /// i18n `Cthulhu.broken`
    pub(crate) broken: &'static str,
    /// i18n `Cthulhu.broken_number`
    pub(crate) broken_number: &'static str,
}

/// `ja_jp` ロケールの文言一式（`Cthulhu` 本体が使う）。
pub(crate) static JA_JP: Locale = Locale {
    success: "成功",
    failure: "失敗",
    critical: "決定的成功",
    special: "スペシャル",
    critical_special: "決定的成功/スペシャル",
    fumble: "致命的失敗",
    partial_success: "部分的成功",
    automatic_success: "自動成功",
    automatic_failure: "自動失敗",
    broken: "故障",
    broken_number: "故障ナンバー",
};

// ---------------------------------------------------------------------------
// 判定
// ---------------------------------------------------------------------------

/// Ruby `@critical_percentage` / `@fumble_percentage`。
///
/// `Cthulhu#eval_game_system_specific_command` が枝ごとに代入し直す2値。
#[derive(Debug, Clone, Copy)]
struct Rates {
    critical: i64,
    fumble: i64,
}

/// `CC` / `RES` / `CBR`（1%ルール）。
const RATES_1: Rates = Rates {
    critical: 1,
    fumble: 1,
};

/// `CCB` / `RESB` / `CBRB`（5%ルール）。
const RATES_5: Rates = Rates {
    critical: 5,
    fumble: 5,
};

/// Ruby `@special_percentage`（`Cthulhu#initialize` が 20 に固定し、以後変えない）。
const SPECIAL_PERCENTAGE: i64 = 20;

/// Ruby `Cthulhu::CompareResult`。
///
/// Ruby側は `attr_accessor` を並べただけの入れ物で、`initialize` はすべて `false`
/// にする（`@broke = false` は `@broken` の書き間違いだが、既定値が同じなので影響しない）。
#[derive(Debug, Clone, Copy, Default)]
struct CompareResult {
    success: bool,
    failure: bool,
    critical: bool,
    fumble: bool,
    special: bool,
    broken: bool,
}

impl CompareResult {
    /// Ruby `CompareResult#text`。
    ///
    /// Ruby はどの条件にも当たらないと `nil` を返すが、[`compare`] が必ず
    /// `success` か `failure` のどちらかを立てるので到達しない。
    /// 万一到達しても `"#{nil}"` と同じになるよう空文字列を返す。
    fn text(self, loc: &Locale) -> String {
        if self.critical && self.special {
            loc.critical_special.to_owned()
        } else if self.critical {
            loc.critical.to_owned()
        } else if self.special {
            loc.special.to_owned()
        } else if self.success {
            loc.success.to_owned()
        } else if self.broken && self.fumble {
            format!("{}/{}", loc.fumble, loc.broken)
        } else if self.broken {
            loc.broken.to_owned()
        } else if self.fumble {
            loc.fumble.to_owned()
        } else if self.failure {
            loc.failure.to_owned()
        } else {
            String::new()
        }
    }

    /// Ruby `CompareResult#to_result`。
    ///
    /// Ruby は `Result.new` に4つのフラグを**素で代入**する。
    /// `EvalResult::critical()` などのコンストラクタは `Result.critical(text)` 相当で
    /// `success` も同時に立ててしまうので、ここでは使えない
    /// （`CBRB(98,50)` が `success` なし + `fumble` あり の組み合わせを要求する）。
    fn to_result(self) -> EvalResult {
        EvalResult {
            success: self.success,
            failure: self.failure,
            critical: self.critical,
            fumble: self.fumble,
            ..EvalResult::default()
        }
    }
}

/// Ruby `Cthulhu#compare`。
fn compare(rates: Rates, total: i64, target: i64, broken_number: i64) -> CompareResult {
    let mut result = CompareResult::default();

    // Ruby: (target * @special_percentage / 100).clamp(1, 100)
    // Ruby の Integer#/ は床除算。`target` はこの関数に入る時点で必ず 0 以上
    // （CC は `diff > 0`、RES は `5..=95`、CBR は `(\d+)`）なので符号差は出ない。
    let target_special = target
        .saturating_mul(SPECIAL_PERCENTAGE)
        .div_euclid(100)
        .clamp(1, 100);

    if total <= target && total < 100 {
        result.success = true;
        result.special = total <= target_special;
        result.critical = total <= rates.critical;
    } else {
        result.failure = true;
        result.fumble = total >= (101 - rates.fumble);
    }

    if broken_number > 0 && total >= broken_number {
        result.broken = true;
        result.failure = true;
        result.success = false;
        result.special = false;
        result.critical = false;
    }

    result
}

/// Ruby `%r{^CCB?(\d+)?(?:<=([+-/*\d]+))?$}i`。
///
/// 文字クラスの `+-/` は 0x2B–0x2F の**範囲**（`+` `,` `-` `.` `/`）なので、
/// `,` と `.` も通る。Ruby の表記を1文字も変えずに写している。
fn check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\ACCB?(\d+)?(?:<=([+-/*\d]+))?\z").expect("valid regex"))
}

/// Ruby `/^RESB?(-?\d+)$/i`。
fn regist_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\ARESB?(-?\d+)\z").expect("valid regex"))
}

/// Ruby `/^CBR(B)?\((\d+),(\d+)\)$/i`。
fn combine_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\ACBR(B)?\((\d+),(\d+)\)\z").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `Cthulhu#getCheckResult`。
fn get_check_result(
    loc: &Locale,
    rates: Rates,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = check_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1].to_i（`(\d+)?` が不一致なら `nil.to_i` で 0）
    let broken_num = m.get(1).map_or(0, |x| to_i(x.as_str()));
    // Ruby: ArithmeticEvaluator.eval(m[2])
    //   = 引数が nil なら 0、`Arithmetic.eval(expr, FLOOR)` が nil でも 0。
    //   端数処理はゲームシステムの round_type ではなくキーワード既定の FLOOR。
    let diff = match m.get(2) {
        Some(x) => arithmetic::eval(x.as_str(), RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };

    if diff <= 0 {
        let total = rng.roll_once(100)?;
        return Ok(Some(EvalResult::with_text(format!("(1D100) ＞ {total}"))));
    }

    let mut expr = format!("(1D100<={diff})");
    if broken_num > 0 {
        expr.push_str(&format!(" {}[{broken_num}]", loc.broken_number));
    }

    let total = rng.roll_once(100)?;
    let compare_result = compare(rates, total, diff, broken_num);

    let mut result = compare_result.to_result();
    result.text = format!("{expr} ＞ {total} ＞ {}", compare_result.text(loc));
    Ok(Some(result))
}

/// Ruby `Cthulhu#getRegistResult`。
fn get_regist_result(
    loc: &Locale,
    rates: Rates,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = regist_pattern().captures(command) else {
        return Ok(None);
    };

    let value = to_i(&m[1]);
    // Ruby は多倍長なので桁あふれしないが、Rustでは i64 に飽和させる。
    // 飽和する入力では自動成功／自動失敗の出力に埋まる目標値だけが Ruby と食い違う
    // （どちらへ振れても `< 5` / `> 95` の判定結果は変わらない）。
    let target = value.saturating_mul(5).saturating_add(50);

    if target < 5 {
        return Ok(Some(EvalResult::failure(format!(
            "(1d100<={target}) ＞ {}",
            loc.automatic_failure
        ))));
    }

    if target > 95 {
        return Ok(Some(EvalResult::success(format!(
            "(1d100<={target}) ＞ {}",
            loc.automatic_success
        ))));
    }

    // 通常判定
    let total_n = rng.roll_once(100)?;
    let compare_result = compare(rates, total_n, target, 0);

    let mut result = compare_result.to_result();
    result.text = format!(
        "(1d100<={target}) ＞ {total_n} ＞ {}",
        compare_result.text(loc)
    );
    Ok(Some(result))
}

/// Ruby `Cthulhu#getCombineRoll`。
fn get_combine_roll(
    loc: &Locale,
    rates: Rates,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = combine_pattern().captures(command) else {
        return Ok(None);
    };

    let diff_1 = to_i(&m[2]);
    let diff_2 = to_i(&m[3]);

    let total = rng.roll_once(100)?;

    let result_1 = compare(rates, total, diff_1, 0);
    let result_2 = compare(rates, total, diff_2, 0);

    let rank = if result_1.success && result_2.success {
        loc.success
    } else if result_1.success || result_2.success {
        loc.partial_success
    } else {
        loc.failure
    };

    let mut result = EvalResult::with_text(format!(
        "(1d100<={diff_1},{diff_2}) ＞ {total}[{},{}] ＞ {rank}",
        result_1.text(loc),
        result_2.text(loc)
    ));
    result.critical = result_1.critical || result_2.critical;
    result.fumble = result_1.fumble || result_2.fumble;
    // Ruby: r.condition = result_1.success || result_2.success
    result.set_condition(result_1.success || result_2.success);
    Ok(Some(result))
}

/// Ruby `Cthulhu#eval_game_system_specific_command`。
///
/// Ruby の `case command` は**アンカー無し**の `/CCB/i` などで、
/// 最初に一致した `when` の枝だけを実行して `return` する。
/// 枝が `nil` を返したらそれがそのまま結果になり、次の枝は試さない
/// （例: `RESB18-11` は RESB の枝で `nil` 確定であって CBR を試さない）。
pub(crate) fn eval_specific_command(
    loc: &Locale,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // `Base#dice_command` が `@enabled_upcase_input` で大文字化済みだが、
    // Ruby の `/…/i` と同じになるよう ASCII 大文字化してから探す。
    let upper = command.to_ascii_uppercase();

    let result = if upper.contains("CCB") {
        // 5%
        get_check_result(loc, RATES_5, command, rng)?
    } else if upper.contains("CC") {
        // 1%
        get_check_result(loc, RATES_1, command, rng)?
    } else if upper.contains("RESB") {
        // 5%
        get_regist_result(loc, RATES_5, command, rng)?
    } else if upper.contains("CBRB") {
        // 5%
        get_combine_roll(loc, RATES_5, command, rng)?
    } else if upper.contains("RES") {
        // 1%
        get_regist_result(loc, RATES_1, command, rng)?
    } else if upper.contains("CBR") {
        // 1%
        get_combine_roll(loc, RATES_1, command, rng)?
    } else {
        // Ruby: どの when にも当たらなければ nil
        None
    };

    Ok(result.map(SpecificCommandOutput::result))
}

/// Ruby `Base#result_ndx` を任意のロケールで行う。
///
/// トレイトの既定実装は `ja_jp` 固定なので `Cthulhu` 本体はそれで足りる。
/// 4つのロケールバリアントだけがこれを経由して上書きする
/// （接頭辞に一致しない `1D100<=70` などがこの経路を通る）。
pub(crate) fn result_ndx_localized(
    loc: &Locale,
    total: crate::Int,
    cmp_op: CmpOp,
    target: Target,
) -> Option<EvalResult> {
    // Ruby: return nil if target.is_a?(String)（目標値 "?"）
    let Target::Number(target) = target else {
        return None;
    };
    if cmp_op.apply(&total, &target) {
        Some(EvalResult::success(loc.success))
    } else {
        Some(EvalResult::failure(loc.failure))
    }
}

/// Ruby `BCDice::GameSystem::Cthulhu`（ID: `Cthulhu`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cthulhu;

impl GameSystem for Cthulhu {
    fn id(&self) -> &'static str {
        "Cthulhu"
    }

    fn name(&self) -> &'static str {
        "クトゥルフ神話TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "くとうるふしんわTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"c=クリティカル値 ／ f=ファンブル値 ／ s=スペシャル

1d100<=n    c・f・sすべてオフ（単純な数値比較判定のみ行います）

・cfs判定付き判定コマンド

CC	 1d100ロールを行う c=1、f=100
CCB  同上、c=5、f=96

例：CC<=80  （技能値80で行為判定。1%ルールでcf適用）
例：CCB<=55 （技能値55で行為判定。5%ルールでcf適用）

・組み合わせロールについて

CBR(x,y)	c=1、f=100
CBRB(x,y)	c=5、f=96

・抵抗表ロールについて
RES(x-y)	c=1、f=100
RESB(x-y)	c=5、f=96

※故障ナンバー判定

・CC(x) c=1、f=100
x=故障ナンバー。出目x以上が出た上で、ファンブルが同時に発生した場合、共に出力する（テキスト「ファンブル＆故障」）
ファンブルでない場合、成功・失敗に関わらずテキスト「故障」のみを出力する（成功・失敗を出力せず、上書きしたものを出力する形）

・CCB(x) c=5、f=96
同上

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["CCB?", "RESB?", "CBRB?"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_JP, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Cthulhu", "Cthulhu.toml", 105);
    }
}
