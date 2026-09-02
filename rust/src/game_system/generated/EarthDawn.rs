//! P4で手書き移植した `lib/bcdice/game_system/EarthDawn.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `EarthDawn#eval_game_system_specific_command` → `#ed_step` → `#getStepResult`
//! - `#getStepTable`（ステップ表）/ `#rollStep`（振り足しつきのダイスロール）
//!
//! ステップ表のデータは `lib/bcdice/game_system/EarthDawn.rb` から機械的に
//! 書き出したもので、値は1つも変えていない。
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// ステップ表（Ruby `EarthDawn#getStepTable`）
// ---------------------------------------------------------------------------

/// ステップごとの修正値（Ruby `mod`）。
static MOD: &[i64] = &[
    -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// ステップごとの20面ダイスの個数（Ruby `d20`）。
static D20: &[i64] = &[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 2, 2, 2, 2, 2, 2, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// ステップごとの12面ダイスの個数（Ruby `d12`）。
static D12: &[i64] = &[
    0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0,
    0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1,
];
/// ステップごとの10面ダイスの個数（Ruby `d10`）。
static D10: &[i64] = &[
    0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 1, 1, 1, 1, 2, 1, 1, 1, 2,
    3, 2, 1, 1, 1, 2, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 1,
];
/// ステップごとの8面ダイスの個数（Ruby `d8`）。
static D8: &[i64] = &[
    0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 2, 1, 1, 1, 2, 2,
    1, 1, 1, 1, 2, 1, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0,
];
/// ステップごとの6面ダイスの個数（Ruby `d6`）。
static D6: &[i64] = &[
    0, 0, 0, 1, 0, 0, 0, 2, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 2, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 2, 1, 0,
    0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0, 2, 1, 1, 0, 0, 0,
];
/// ステップごとの4面ダイスの個数（Ruby `d4`）。
static D4: &[i64] = &[
    1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// 目標値ごとの最良成功のしきい値（Ruby `exsuc`）。
static EXSUC: &[i64] = &[
    6, 8, 10, 12, 14, 17, 19, 20, 22, 24, 25, 27, 29, 32, 33, 35, 37, 38, 39, 41, 42, 44, 45, 47,
    48, 49, 51, 52, 54, 55, 56, 58, 59, 60, 62, 64, 65, 67, 68, 70, 71, 72,
];
/// 目標値ごとの優成功のしきい値（Ruby `ssuc`）。
static SSUC: &[i64] = &[
    4, 6, 8, 10, 11, 13, 15, 16, 18, 19, 21, 22, 24, 26, 27, 29, 30, 32, 33, 34, 35, 37, 38, 40,
    41, 42, 43, 45, 46, 47, 48, 49, 51, 52, 53, 55, 56, 58, 59, 60, 61, 62,
];
/// 目標値ごとの良成功のしきい値（Ruby `gsuc`）。
static GSUC: &[i64] = &[
    2, 4, 6, 7, 9, 10, 12, 13, 14, 15, 17, 18, 20, 21, 22, 24, 25, 26, 27, 28, 29, 31, 32, 33, 34,
    35, 36, 38, 39, 40, 41, 42, 43, 45, 46, 47, 48, 50, 51, 52, 53, 54,
];
/// 目標値ごとの大失敗のしきい値（Ruby `fsuc`）。
///
/// Ruby `stable[10]`（`nsuc`）は表に載っているだけで参照されない（成功判定は
/// `stepTotal >= targetNumber` を直接使う）ので、こちらには持たない。
static FSUC: &[i64] = &[
    0, 1, 1, 1, 1, 2, 2, 3, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 13, 14, 15, 16, 17, 18, 18,
    18, 20, 21, 22, 23, 23, 24, 25, 26, 26, 27, 28, 29, 30,
];

/// Ruby `Array#[]` の添字参照。負の添字は末尾から数える。
///
/// `step` は 0 も取りうる（接頭辞が `\d+e` なので `0E` が通る）。
/// Ruby は `stable[0][-1]` を末尾の要素として読むので、その挙動をそのまま再現する。
fn ruby_at(values: &[i64], index: i64) -> Option<i64> {
    let index = if index < 0 {
        index.checked_add(values.len() as i64)?
    } else {
        index
    };
    usize::try_from(index)
        .ok()
        .and_then(|i| values.get(i))
        .copied()
}

/// Ruby `EarthDawn#getStepResult` の `/(\d+)E(\d+)?(\+)?(\d+)?(d\d+)?/i`。
///
/// 先頭が固定されていない（`^` が無い）ので、原典どおり途中一致も許す。
fn step_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)E(\d+)?(\+)?(\d+)?(d\d+)?").expect("valid regex"))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `EarthDawn#rollStep`。
///
/// 面数と同じ出目が出た間は振り足す。1つでも「最初の出目が1でない」ダイスがあれば
/// `@isFailed` を下ろす（振り足した分は見ない）。
fn roll_step(
    dice_type: i64,
    dice_count: i64,
    string: &mut String,
    is_failed: &mut bool,
    rng: &mut Randomizer,
) -> Result<i64, EvalError> {
    let mut step_total = 0;
    if dice_count <= 0 {
        return Ok(step_total);
    }

    if !string.is_empty() {
        string.push('+');
    }
    string.push_str(&format!("{dice_count}d{dice_type}["));

    for i in 0..dice_count {
        let mut dice_now = rng.roll_once(dice_type)?;

        if dice_now != 1 {
            *is_failed = false;
        }

        let mut dice_in = dice_now;

        while dice_now == dice_type {
            dice_now = rng.roll_once(dice_type)?;
            dice_in += dice_now;
        }

        step_total += dice_in;

        if i != 0 {
            string.push(',');
        }
        string.push_str(&dice_in.to_string());
    }

    string.push(']');

    Ok(step_total)
}

/// Ruby `EarthDawn#getStepResult`。
fn get_step_result(str_: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = step_pattern().captures(str_) else {
        return Ok(None);
    };

    let mut step_total = 0;
    let mut is_failed = true;

    // ステップ
    let mut step = to_i(&m[1]);
    // 目標値
    let mut target_number = 0;

    // 空値があった時の為のばんぺいくんRX
    if step > 40 {
        step = 40;
    }

    if let Some(c) = m.get(2) {
        target_number = to_i(c.as_str());
        // Ruby: targetNumber = 42 if targetNumber > 43
        //（`43` はクランプされないまま下の表参照へ進む。原典どおりにしておく）
        if target_number > 43 {
            target_number = 42;
        }
    }

    // Ruby: hasKarmaDice = Regexp.last_match(3).to_i if Regexp.last_match(3)
    //       `"+".to_i` は 0 だが Ruby では 0 も真なので「`+` があったか」と同義。
    let has_karma_dice = m.get(3).is_some();
    // カルマダイスの個数又は修正
    let karma_dice_count = m.get(4).map_or(0, |c| to_i(c.as_str()));
    // カルマダイスの種類
    let karma_dice_type = m.get(5).map(|c| c.as_str().to_ascii_lowercase());

    // Ruby: return nil if targetNumber < 0（`(\d+)` なので負にはならない）

    // Ruby: stable[n][step - 1]
    // 添字が表の外に出るのは Ruby でも `nil` になり、直後の演算でクラッシュする。
    // 本移植は他のコマンドと同じく「解釈できないコマンド＝nil」に畳む。
    let index = step - 1;
    let (
        Some(mut nmod),
        Some(mut d20step),
        Some(mut d12step),
        Some(mut d10step),
        Some(mut d8step),
        Some(mut d6step),
        Some(mut d4step),
    ) = (
        ruby_at(MOD, index),
        ruby_at(D20, index),
        ruby_at(D12, index),
        ruby_at(D10, index),
        ruby_at(D8, index),
        ruby_at(D6, index),
        ruby_at(D4, index),
    )
    else {
        return Ok(None);
    };

    if has_karma_dice {
        // Ruby: case karmaDiceType when /d20/i ...（部分一致・大文字小文字を無視）
        // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる
        // （飽和した個数は `roll_barabara` が上限超過で弾く）。
        match karma_dice_type.as_deref() {
            Some(t) if t.contains("d20") => d20step = d20step.saturating_add(karma_dice_count),
            Some(t) if t.contains("d12") => d12step = d12step.saturating_add(karma_dice_count),
            Some(t) if t.contains("d10") => d10step = d10step.saturating_add(karma_dice_count),
            Some(t) if t.contains("d8") => d8step = d8step.saturating_add(karma_dice_count),
            Some(t) if t.contains("d6") => d6step = d6step.saturating_add(karma_dice_count),
            Some(t) if t.contains("d4") => d4step = d4step.saturating_add(karma_dice_count),
            _ => nmod = nmod.saturating_add(karma_dice_count),
        }
    }

    let mut string = String::new();

    step_total += roll_step(20, d20step, &mut string, &mut is_failed, rng)?;
    step_total += roll_step(12, d12step, &mut string, &mut is_failed, rng)?;
    step_total += roll_step(10, d10step, &mut string, &mut is_failed, rng)?;
    step_total += roll_step(8, d8step, &mut string, &mut is_failed, rng)?;
    step_total += roll_step(6, d6step, &mut string, &mut is_failed, rng)?;
    step_total += roll_step(4, d4step, &mut string, &mut is_failed, rng)?;

    // 修正分の適用
    if nmod > 0 {
        string.push('+');
    }
    if nmod != 0 {
        string.push_str(&nmod.to_string());
        // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる。
        step_total = step_total.saturating_add(nmod);
    }

    // ステップ判定終了
    string.push_str(&format!(" ＞ {step_total}"));

    if target_number == 0 {
        return Ok(Some(format!("ステップ{step} ＞ {string}")));
    }

    // 結果判定
    string.push_str(" ＞ ");

    // Ruby: stable[7][targetNumber - 1] など。`targetNumber == 43` だと表の外に出て
    // Ruby は `stepTotal >= nil` でクラッシュする。ここでは nil に畳む。
    let index = target_number - 1;
    let (
        Some(excelent_success_number),
        Some(super_success_number),
        Some(good_success_number),
        Some(failed_number),
    ) = (
        ruby_at(EXSUC, index),
        ruby_at(SSUC, index),
        ruby_at(GSUC, index),
        ruby_at(FSUC, index),
    )
    else {
        return Ok(None);
    };

    if is_failed {
        string.push_str("自動失敗");
    } else if step_total >= excelent_success_number {
        string.push_str("最良成功");
    } else if step_total >= super_success_number {
        string.push_str("優成功");
    } else if step_total >= good_success_number {
        string.push_str("良成功");
    } else if step_total >= target_number {
        string.push_str("成功");
    } else if step_total < failed_number {
        string.push_str("大失敗");
    } else {
        string.push_str("失敗");
    }

    Ok(Some(format!("ステップ{step}>={target_number} ＞ {string}")))
}

/// Ruby `BCDice::GameSystem::EarthDawn`（ID: `EarthDawn`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarthDawn;

impl GameSystem for EarthDawn {
    fn id(&self) -> &'static str {
        "EarthDawn"
    }

    fn name(&self) -> &'static str {
        "アースドーン"
    }

    fn sort_key(&self) -> &'static str {
        "ああすとおん"
    }

    fn help_message(&self) -> &'static str {
        r"ステップダイス　(xEn+k)
ステップx、目標値n(省略可能）、カルマダイスk(D2-D20)でステップダイスをロールします。
振り足しも自動。
例）9E　10E8　10E+D12
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+e"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `EarthDawn#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `EarthDawn#eval_game_system_specific_command` → `#ed_step`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(get_step_result(command, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    /// `test/data/EarthDawn.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EarthDawn",
            "EarthDawn.toml",
            72,
        );
    }

    /// 桁あふれするカルマダイス修正でも panic しないこと。
    ///
    /// Ruby の Integer は多倍長なのでそのまま計算されるが、Rustでは `to_i` が
    /// `i64::MAX` に飽和し、以降の加算も飽和演算になる。TOMLにこの経路のケースが
    /// 無いのでここで固定する（デバッグビルドはオーバーフローで panic するため）。
    #[test]
    fn huge_karma_modifier_saturates_instead_of_panicking() {
        let mut src = SeededRandomizer::new(vec![(3, 8), (4, 6)]);
        let result = eval_command(
            &GameSystemId::new("EarthDawn"),
            "9E+99999999999999999999",
            &mut src,
        )
        .expect("eval")
        .expect("result");
        assert_eq!(
            result.text,
            "ステップ9 ＞ 1d8[3]+1d6[4]+9223372036854775807 ＞ 9223372036854775807"
        );
        assert!(src.is_empty(), "unconsumed rands");
    }

    /// ステップ0は Ruby の負の添字（末尾から数える）で 1d12+1d10 になること。
    ///
    /// 接頭辞 `\d+e` は `0E` を通すので実際に到達する。TOMLに無いのでここで固定する。
    #[test]
    fn step_zero_wraps_around_like_ruby() {
        let mut src = SeededRandomizer::new(vec![(3, 12), (4, 10)]);
        let result = eval_command(&GameSystemId::new("EarthDawn"), "0E", &mut src)
            .expect("eval")
            .expect("result");
        assert_eq!(result.text, "ステップ0 ＞ 1d12[3]+1d10[4] ＞ 7");
        assert!(src.is_empty(), "unconsumed rands");
    }
}
