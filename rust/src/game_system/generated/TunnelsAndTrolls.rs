//! P4で手書き移植した `lib/bcdice/game_system/TunnelsAndTrolls.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TunnelsAndTrolls#roll_action`（行為判定 `nD6+x>=nLV`。ゾロ目の振り足しと経験値計算）
//! - `TunnelsAndTrolls#replace_text` / `#eval_game_system_specific_command`
//!   （バーサーク `nBS+x` / ハイパーバーサーク `nHBS+x` → `nR6`）

use std::collections::VecDeque;
use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::arithmetic::{self};
use crate::command_parser::{Parsed, Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 行為判定（nD6+x>=nLV）
// ---------------------------------------------------------------------------

/// Ruby `/^\d+D\d+/i`。
fn add_dice_prefix_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\d+D\d+").expect("valid regex"))
}

/// Ruby `/\d+LV$/i`。SAVEの難易度「nLv」を数値へ読み替える。
fn level_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\d+LV$").expect("valid regex"))
}

/// Ruby `command.sub(/\d+LV$/i) { |level| level.to_i * 5 + 15 }`。
fn replace_level(command: &str) -> String {
    level_pattern()
        .replace(command, |caps: &Captures<'_>| {
            // `\d+LV` の先頭の数字列。Ruby `String#to_i` は末尾の "LV" を無視する
            let digits: String = caps[0].chars().take_while(|c| c.is_ascii_digit()).collect();
            let level: i64 = digits.parse().unwrap_or(0);
            (level * 5 + 15).to_string()
        })
        .into_owned()
}

/// Ruby `@dice_list` / `@dice_total` / `@count_6`（`roll_action_dice` が設定する状態）。
struct ActionDice {
    /// 振り足しごとのダイス目（ソート済み）
    dice_list: Vec<Vec<i64>>,
    /// 全ロールの合計
    dice_total: i64,
    /// 全ロール中の6の個数
    count_6: i64,
}

/// Ruby `same_all_dice?`。出目が全て同じか。
fn same_all_dice(dice_list: &[i64]) -> bool {
    dice_list.len() > 1 && dice_list.iter().all(|d| *d == dice_list[0])
}

/// Ruby `roll_action_dice`。ゾロ目なら同じ個数で振り足す。
fn roll_action_dice(times: i64, rng: &mut Randomizer) -> Result<ActionDice, EvalError> {
    let mut dice_list = rng.roll_barabara(times, 6)?;
    dice_list.sort_unstable();
    let mut rolls = vec![dice_list.clone()];

    while same_all_dice(&dice_list) {
        dice_list = rng.roll_barabara(times, 6)?;
        dice_list.sort_unstable();
        rolls.push(dice_list.clone());
    }

    let dice_total = rolls.iter().flatten().sum();
    let count_6 = rolls.iter().flatten().filter(|d| **d == 6).count() as i64;

    Ok(ActionDice {
        dice_list: rolls,
        dice_total,
        count_6,
    })
}

/// Ruby `interim_expr`。ダイス目と修正値の途中経過。
fn interim_expr(cmd: &Parsed, dice: &ActionDice) -> Option<String> {
    if dice.dice_list.iter().flatten().count() == 1 && cmd.modify_number == I::ZERO {
        return None;
    }

    let dice_list = dice
        .dice_list
        .iter()
        .map(|ds| {
            let body = ds
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        })
        .collect::<String>();

    Some(format!(
        "{}{dice_list}{}",
        dice.dice_total,
        format::modifier(&cmd.modify_number)
    ))
}

/// Ruby `int?(v)`（浮動小数点数が整数か）。
fn is_int(v: f64) -> bool {
    v == v.trunc()
}

/// Ruby `experience_point`。
///
/// Ruby は `1.0 * (target - 15) / 5 * dice_total` の浮動小数点計算なので、
/// ここでも f64 のまま計算する（`.5` 刻みの端数が出力に現れる）。
fn experience_point(target_number: i64, dice_total: i64) -> String {
    let ep = 1.0 * (target_number as f64 - 15.0) / 5.0 * (dice_total as f64);

    if ep <= 0.0 {
        "0".to_owned()
    } else if is_int(ep) {
        // Ruby `Float#to_i` は0方向への切り捨て。ep > 0 かつ整数なのでそのまま
        format!("{}", ep as i64)
    } else {
        format!("{ep:.1}")
    }
}

/// Ruby `success_level`。目標値が `?` のときの成功レベル。
fn success_level(total: i64, dice_total: i64) -> String {
    // Ruby `Integer#/` は床除算
    let level = (total - 15).div_euclid(5);
    if level <= 0 {
        format!("失敗 ＞ 経験値{dice_total}")
    } else {
        format!("{level}Lv成功 ＞ 経験値{dice_total}")
    }
}

/// Ruby `action_result`。
fn action_result(total: i64, dice_total: i64, cmd: &Parsed) -> Option<String> {
    if dice_total == 3 {
        return Some("自動失敗".to_owned());
    }
    if cmd.question_target {
        // Ruby: target_number == "?"
        return Some(success_level(total, dice_total));
    }
    let target_number = cmd.target_number.clone()?;

    if total >= crate::randomizer::sat_i64(&target_number) {
        Some(format!(
            "成功 ＞ 経験値{}",
            experience_point(crate::randomizer::sat_i64(&target_number), dice_total)
        ))
    } else {
        Some("失敗".to_owned())
    }
}

/// Ruby `additional_result`。
fn additional_result(count_6: i64) -> Option<String> {
    (count_6 > 0).then(|| format!("悪意{count_6}"))
}

/// Ruby `TunnelsAndTrolls#roll_action`。
fn roll_action(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let command = replace_level(command);

    let parser = Parser::new(&[r"\d+D6"], RoundType::Floor)
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
        .enable_question_target();
    let Some(cmd) = parser.parse(&command) else {
        return Ok(None);
    };

    // Ruby: cmd.command.to_i（"2D6" → 2）
    let times: i64 = cmd
        .command
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0);

    let dice = roll_action_dice(times, rng)?;
    let total = dice.dice_total + cmd.modify_number.clone();

    let sequence: Vec<String> = [
        Some(format!("({})", cmd.to_s(SuffixPosition::AfterCommand))),
        interim_expr(&cmd, &dice),
        Some(total.to_string()),
        action_result(crate::randomizer::sat_i64(&total), dice.dice_total, &cmd),
        additional_result(dice.count_6),
    ]
    .into_iter()
    .flatten()
    .collect();

    Ok(Some(sequence.join(" ＞ ")))
}

// ---------------------------------------------------------------------------
// バーサーク（nBS / nHBS → nR6）
// ---------------------------------------------------------------------------

/// Ruby `replace_text` の4本の `gsub`。
fn berserk_patterns() -> &'static [(Regex, &'static str); 4] {
    static RE: OnceLock<[(Regex, &'static str); 4]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            (
                Regex::new(r"(?i)(\d+)HBS([^\d\s][+\-\d]+)").expect("valid regex"),
                "${1}R6${2}[H]",
            ),
            (
                Regex::new(r"(?i)(\d+)HBS").expect("valid regex"),
                "${1}R6[H]",
            ),
            (
                Regex::new(r"(?i)(\d+)BS([^\d\s][+\-\d]+)").expect("valid regex"),
                "${1}R6${2}",
            ),
            (Regex::new(r"(?i)(\d+)BS").expect("valid regex"), "${1}R6"),
        ]
    })
}

/// Ruby `TunnelsAndTrolls#replace_text`。
fn replace_text(string: &str) -> String {
    // Ruby: `if /BS/i =~ string`
    if !string.to_uppercase().contains("BS") {
        return string.to_owned();
    }

    let mut current = string.to_owned();
    for (re, rep) in berserk_patterns() {
        current = re.replace_all(&current, *rep).into_owned();
    }
    current
}

/// Ruby `eval_game_system_specific_command` のバーサーク書式。
///
/// Ruby: `/(^|\s)S?((\d+)[rR]6([+\-\d]*)(\[(\w+)\])?)(\s|$)/i`
/// Ruby の `\w` はASCIIのみなので明示クラスに置き換えた。
fn berserk_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)S?((\d+)R6([+\-\d]*)(\[([0-9A-Za-z_]+)\])?)(\s|$)")
            .expect("valid regex")
    })
}

/// Ruby `eval_game_system_specific_command` のバーサーク処理。
fn roll_berserk(string: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let string = replace_text(string);

    let Some(m) = berserk_roll_pattern().captures(&string) else {
        // Ruby: output = "1"（`dice_command` が nil に畳む）
        return Ok("1".to_owned());
    };

    let notation = m[2].to_owned();
    let dice_c: i64 = m[3].parse().unwrap_or(0);
    // Ruby: `ArithmeticEvaluator.eval` は nil・不正な式に 0 を返す
    let bonus = arithmetic::eval(&m[4], RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    // Ruby: `m[5] && (m[6] =~ /[Hh]/)`
    let is_hyper_berserk = m.get(6).is_some_and(|g| g.as_str().contains(['H', 'h']));

    let mut dice_arr: VecDeque<i64> = VecDeque::new();
    let mut dice_now = 0;
    let mut dice_str = String::new();
    let mut is_first_loop = true;
    let mut n_max = 0;
    let mut bonus2 = 0;

    // ２回目以降
    dice_arr.push_back(dice_c);

    loop {
        let dice_wk = dice_arr.pop_front().unwrap_or(0);

        let mut dice_list = rng.roll_barabara(dice_wk, 6)?;
        dice_list.sort_unstable();
        let roll_total: i64 = dice_list.iter().sum();
        let mut roll_dice_max_count = dice_list.iter().filter(|d| **d == 6).count() as i64;

        if dice_wk >= 2 {
            // ダイスが二個以上
            const DICE_TYPE: usize = 6;
            let mut dice_face = [0i64; DICE_TYPE];
            for dice_o in &dice_list {
                dice_face[(*dice_o - 1) as usize] += 1;
            }

            for count in dice_face {
                if count >= 2 {
                    // Ruby はブロック変数への代入なので、push されるのは +1 した値
                    dice_arr.push_back(if is_hyper_berserk { count + 1 } else { count });
                }
            }

            if is_first_loop && dice_arr.is_empty() {
                // 出目が全て異なる場合、下から２番目の出目を最小の出目までずらす
                let mut min1 = 0i64;
                let mut min2 = 0i64;
                for i in 0..DICE_TYPE {
                    let index = DICE_TYPE - i - 1;
                    if dice_face[index] > 0 {
                        min2 = min1;
                        min1 = index as i64;
                    }
                }

                bonus2 = -(min2 - min1);
                if min2 == 5 {
                    roll_dice_max_count -= 1;
                }

                dice_arr.push_back(if is_hyper_berserk { 3 } else { 2 });
            }
        }

        dice_now += roll_total;
        if !dice_str.is_empty() {
            dice_str.push_str("][");
        }
        dice_str.push_str(
            &dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        n_max += roll_dice_max_count;
        is_first_loop = false;

        if dice_arr.is_empty() {
            break;
        }
    }

    let total_n = dice_now + bonus + bonus2;

    let mut output = format!("{dice_now}[{dice_str}]");

    if bonus2 < 0 {
        output.push_str(&bonus2.to_string());
    }

    if bonus > 0 {
        output.push_str(&format!("+{bonus}"));
    } else if bonus < 0 {
        output.push_str(&bonus.to_string());
    }

    // Ruby: `output =~ /[^\d\[\]]+/`（数字と角括弧以外を含むか）
    let mut output = if output
        .chars()
        .any(|c| !c.is_ascii_digit() && c != '[' && c != ']')
    {
        format!("({notation}) ＞ {output} ＞ {total_n}")
    } else {
        format!("({notation}) ＞ {total_n}")
    };

    if n_max > 0 {
        output.push_str(&format!(" ＞ 悪意{n_max}"));
    }

    Ok(output)
}

/// Ruby `BCDice::GameSystem::TunnelsAndTrolls`（ID: `TunnelsAndTrolls`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunnelsAndTrolls;

impl GameSystem for TunnelsAndTrolls {
    fn id(&self) -> &'static str {
        "TunnelsAndTrolls"
    }

    fn name(&self) -> &'static str {
        "トンネルズ＆トロールズ"
    }

    fn sort_key(&self) -> &'static str {
        "とんねるすあんととろおるす"
    }

    fn help_message(&self) -> &'static str {
        r#"・行為判定　(nD6+x>=nLV)
失敗、成功、自動失敗の自動判定とゾロ目の振り足し経験値の自動計算を行います。
SAVEの難易度を「レベル」で表記することが出来ます。
例えば「2Lv」と書くと「25」に置換されます。
判定時以外は悪意ダメージを表示します。
バーサークとハイパーバーサーク用に専用コマンドが使えます。
例）2D6+1>=1Lv
　 (2D6+1>=20) ＞ 7[2,5]+1 ＞ 8 ＞ 失敗
　判定時にはゾロ目を自動で振り足します。

・バーサークとハイパーバーサーク　(nBS+x or nHBS+x)
　"(ダイス数)BS(修正値)"でバーサーク、"(ダイス数)HBS(修正値)"でハイパーバーサークでロールできます。
　最初のダイスの読替は、個別の出目はそのままで表示。
　下から２番目の出目をずらした分だけ合計にマイナス修正を追加して表示します。
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+H?BS", r"\d+R6", r"\d+D\d+"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `TunnelsAndTrolls#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if add_dice_prefix_pattern().is_match(command) {
            // Ruby: roll_action は nil を返しうる（`\d+D6` 以外の加算ロール）
            return Ok(roll_action(command, rng)?.map(SpecificCommandOutput::text));
        }

        Ok(Some(SpecificCommandOutput::text(roll_berserk(
            command, rng,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TunnelsAndTrolls",
            "TunnelsAndTrolls.toml",
            141,
        );
    }
}
