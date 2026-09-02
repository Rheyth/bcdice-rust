//! P4で手書き移植した `lib/bcdice/game_system/Utakaze.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Utakaze#check_roll`（行為判定 `nUK[@c][>=t]`）
//! - `Utakaze#opposed_roll`（対抗判定 `nUR[@c]` / `nUO[@c]`）
//! - `#getSuccessInfo` / `#getDiceCountHash`（ゾロ目・龍のダイスの集計）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Utakaze`（ID: `Utakaze`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utakaze;

impl GameSystem for Utakaze {
    fn id(&self) -> &'static str {
        "Utakaze"
    }

    fn name(&self) -> &'static str {
        "ウタカゼ"
    }

    fn sort_key(&self) -> &'static str {
        "うたかせ"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定ロール（nUK）
  n個のサイコロで行為判定ロール。ゾロ目の最大個数を成功レベルとして表示。nを省略すると2UK扱い。
  例）3UK ：サイコロ3個で行為判定
  例）UK  ：サイコロ2個で行為判定
・難易度付き行為判定ロール（nUK>=t）
  tに難易度を指定した行為判定ロール。
  成功レベルと難易度tを比べて成否を判定します。
  例）6UK>=3 ：サイコロ6個で行為判定して、成功レベル3が出れば成功。
・クリティカルコール付き行為判定ロール（nUK@c or nUKc）
  cに「龍のダイス目」を指定した行為判定ロール。
  ゾロ目ではなく、cと同じ値の出目数x2が成功レベルとなります。難易度の指定も可能です。
  例）3UK@5 ：龍のダイス「月」でクリティカルコール宣言したサイコロ3個の行為判定
 ・対抗判定ロール(nUR[@c], nUO[@c]) n:ダイス数 c:クリティカルコール
 　行為判定ロールと同様にロールするが、最期に成功レベルとセット数から求めたマジックナンバーが表示される。
 　マジックナンバーの大きいものが成功、同値は引き分け。
 　ダイスは18個まで対応。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*U[KRO]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Utakaze#eval_game_system_specific_command`。
    ///
    /// `check_roll(command) || opposed_roll(command)`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = opposed_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `DRAGON_DICE_NAME[crit]`。範囲外は Ruby の `nil` と同じく空文字列。
fn dragon_dice_name(crit: i64) -> &'static str {
    match crit {
        1 => "風",
        2 => "雨",
        3 => "雲",
        4 => "影",
        5 => "月",
        6 => "歌",
        _ => "",
    }
}

/// Ruby `String#to_i`（数字列のみ）。桁あふれは `i64::MAX` に飽和させる。
///
/// Ruby は多倍長になるが、`getValue` で 100 を超える値は 0 に畳まれるので
/// 結果は変わらない。
/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `Utakaze#getValue`: 100 を超える値は 0 にする。
fn get_value(number: i64) -> i64 {
    if number > 100 {
        0
    } else {
        number
    }
}

/// Ruby `Utakaze#check_roll`（`nUK[@c][>=t]`）。
fn check_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?UK(@?(\d))?(>=(\d+))?$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    // Ruby: base = (m[1] || 2).to_i / crit = m[3].to_i / diff = m[5].to_i
    let base = m.get(1).map_or(2, |g| to_i(g.as_str()));
    let crit = m.get(3).map_or(0, |g| to_i(g.as_str()));
    let diff = m.get(5).map_or(0, |g| to_i(g.as_str()));

    let base = get_value(base);
    let crit = get_value(crit);

    if base < 1 {
        return Ok(None);
    }

    // Ruby: crit = 6 if crit > 6
    let crit = crit.min(6);

    let mut dice_list = rng.roll_barabara(base, 6)?;
    dice_list.sort_unstable();
    let mut result = get_roll_result(&dice_list, crit, diff);

    let sequence = [
        command.to_owned(),
        format!("({base}D6)"),
        format!("[{}]", dice_text::join_dice(&dice_list)),
        result.text.clone(),
    ];
    result.text = sequence.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `Utakaze#get_roll_result`。
fn get_roll_result(dice_list: &[i64], crit: i64, diff: i64) -> EvalResult {
    let (success, maxnum, set_count) = get_success_info(dice_list, crit);

    let mut sequence: Vec<String> = Vec::new();

    if is_dragon_dice(crit) {
        sequence.push(format!(
            "龍のダイス「{}」({crit})を使用",
            dragon_dice_name(crit)
        ));
    }

    if success {
        sequence.push(format!("成功レベル:{maxnum} ({set_count}セット)"));
    } else {
        sequence.push("失敗".to_owned());
        return EvalResult::failure(sequence.join(" ＞ "));
    }

    if diff == 0 {
        // Ruby: 難易度なしでも成功として扱う
        EvalResult::success(sequence.join(" ＞ "))
    } else if maxnum >= diff {
        sequence.push("成功".to_owned());
        EvalResult::success(sequence.join(" ＞ "))
    } else {
        sequence.push("失敗".to_owned());
        EvalResult::failure(sequence.join(" ＞ "))
    }
}

/// Ruby `Utakaze#opposed_roll`（`nUR[@c]` / `nUO[@c]`）。
fn opposed_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Ruby: /^(\d+)?U[R|O](@?(\d))?$/i（文字クラス内の `|` も原典どおり残す）
    let re = RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?U[R|O](@?(\d))?$").expect("valid regex"));
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let base = m.get(1).map_or(2, |g| to_i(g.as_str()));
    let crit = m.get(3).map_or(0, |g| to_i(g.as_str()));

    let base = get_value(base);
    let crit = get_value(crit);

    // Ruby: return nil if base < 1 || base > 18
    if !(1..=18).contains(&base) {
        return Ok(None);
    }

    let crit = crit.min(6);

    let mut dice_list = rng.roll_barabara(base, 6)?;
    dice_list.sort_unstable();
    let mut result = get_opposed_roll_result(&dice_list, crit);

    let sequence = [
        command.to_owned(),
        format!("({base}D6)"),
        format!("[{}]", dice_text::join_dice(&dice_list)),
        result.text.clone(),
    ];
    result.text = sequence.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `Utakaze#get_opposed_roll_result`。
fn get_opposed_roll_result(dice_list: &[i64], crit: i64) -> EvalResult {
    let (success, maxnum, set_count) = get_success_info(dice_list, crit);

    let mut sequence: Vec<String> = Vec::new();

    if is_dragon_dice(crit) {
        sequence.push(format!(
            "龍のダイス「{}」({crit})を使用",
            dragon_dice_name(crit)
        ));
    }

    if success {
        sequence.push(format!("成功レベル:{maxnum} ({set_count}セット)"));
        // Ruby: "(" + format("%#02d%#1d", maxnum, setCount) + ")"
        // （`#` フラグは `d` 変換では効かないので `%02d%d` と同じ）
        sequence.push(format!("({maxnum:02}{set_count})"));
        // Ruby: 出力上は成功として扱う
        EvalResult::success(sequence.join(" ＞ "))
    } else {
        sequence.push("(000)".to_owned());
        EvalResult::failure(sequence.join(" ＞ "))
    }
}

/// Ruby `Utakaze#getSuccessInfo`。戻り値は `(成功したか, 成功レベル, セット数)`。
fn get_success_info(dice_list: &[i64], crit: i64) -> (bool, i64, usize) {
    let dice_count = get_dice_count(dice_list, crit);

    let mut maxnum = 0;
    let mut success_dice_count = 0usize;
    let count_threshold = if is_dragon_dice(crit) { 1 } else { 2 };

    for (_, count) in dice_count {
        if count > maxnum {
            maxnum = count;
        }
        if count >= count_threshold {
            success_dice_count += 1;
        }
    }

    if success_dice_count == 0 {
        // 失敗：ゾロ目無し(全部違う)
        return (false, 0, 0);
    }

    // 竜のダイスの場合
    if is_dragon_dice(crit) {
        maxnum *= 2;
    }

    // 成功：ゾロ目あり
    (true, maxnum, success_dice_count)
}

/// Ruby `Utakaze#getDiceCountHash`: 各ダイスの個数を数える（出現順）。
fn get_dice_count(dice_list: &[i64], critical: i64) -> Vec<(i64, i64)> {
    let mut counts: Vec<(i64, i64)> = Vec::new();
    for &dice in dice_list
        .iter()
        .filter(|&&dice| is_normal_dice(critical) || dice == critical)
    {
        match counts.iter_mut().find(|(d, _)| *d == dice) {
            Some((_, count)) => *count += 1,
            None => counts.push((dice, 1)),
        }
    }
    counts
}

/// Ruby `Utakaze#isNomalDice`。
fn is_normal_dice(crit: i64) -> bool {
    !is_dragon_dice(crit)
}

/// Ruby `Utakaze#isDragonDice`。
fn is_dragon_dice(crit: i64) -> bool {
    crit != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Utakaze", "Utakaze.toml", 49);
    }
}
