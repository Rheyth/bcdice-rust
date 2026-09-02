//! P4で手書き移植した `lib/bcdice/game_system/AFF2e.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `AFF2e#eval_game_system_specific_command`（`FF`＝対抗なしロール / `FR`＝対抗ロール /
//!   `FD`＝武器防具ロール）
//! - 補助メソッド `explicit_sign` / `eval_term` / `parentheses` / `successful_or_failed` /
//!   `critical` / `clamp`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::AFF2e`（ID: `AFF2e`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AFF2e;

impl GameSystem for AFF2e {
    fn id(&self) -> &'static str {
        "AFF2e"
    }

    fn name(&self) -> &'static str {
        "ADVANCED FIGHTING FANTASY 2nd Edition"
    }

    fn sort_key(&self) -> &'static str {
        "あとはんすとふあいていんくふあんたしい2"
    }

    fn help_message(&self) -> &'static str {
        r"対抗なしロール	FF{目標値}+{補正}
対抗ロール	FR{能力値}+{補正}
武器ロール	FD[2,3,3,3,3,3,4]+{補正}
防具ロール	FD[0,0,0,0,1+1,1+1,2+2]+{補正}
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["FF.+", "FR.+", "FD.+"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `AFF2e#eval_game_system_specific_command`。
///
/// Ruby側は `case command` のどの枝にも入らないと `sequence` が `nil` のまま
/// `nil.join` で落ちるが、接頭辞 `FF.+` / `FR.+` / `FD.+` にマッチした入力しか
/// ここへ来ないため、実際には必ずいずれかの枝に入る。
/// 到達しない枝は `Ok(None)`（＝共通コマンドへのフォールスルー）にしてある。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: md = Regexp.last_match; term = md.post_match
    let sequence: Vec<String> = if let Some(term) = command.strip_prefix("FF") {
        // 対抗なしロール。'成功' or '失敗' を出力する

        // 目標値
        let diff = eval_term(term);

        let dice_command = format!("2D6<={diff}");
        let dice_list = rng.roll_barabara(2, 6)?;
        let total: i64 = dice_list.iter().sum();
        let dice_str = dice_text::join_dice(&dice_list);
        let expr = format!("{total}[{dice_str}]");
        let succ = successful_or_failed(total, diff);
        vec![parentheses(&dice_command), expr, succ.to_owned()]
    } else if let Some(term) = command.strip_prefix("FR") {
        // 対抗ロール。値を出力する

        // 補正値
        let corr = eval_term(term);

        let dice_command = format!("2D6{}", explicit_sign(corr));
        let dice_list = rng.roll_barabara(2, 6)?;
        let total: i64 = dice_list.iter().sum();
        let dice_str = dice_text::join_dice(&dice_list);
        let expr = format!("{total}[{dice_str}]{}", explicit_sign(corr));
        let crit = critical(total);
        // Ruby: [..., crit, total + corr].compact （crit が nil のとき落ちる）
        let mut sequence = vec![parentheses(&dice_command), expr];
        if let Some(crit) = crit {
            sequence.push(crit.to_owned());
        }
        sequence.push((total + corr).to_string());
        sequence
    } else if let Some(term) = command.strip_prefix("FD") {
        // 武器防具ロール。ダメージを出力する
        let Some(md) = damage_slots_pattern().captures(term) else {
            return Ok(Some(SpecificCommandOutput::text(
                "ダメージスロットは必須です。",
            )));
        };

        let slots_src = md.get(1).expect("group 1 always participates").as_str();
        // Ruby: term = md.post_match
        let term = &term[md.get(0).expect("group 0 always exists").end()..];
        let damage_slots: Vec<i64> = ruby_split(slots_src, ',')
            .iter()
            .map(|t| eval_term(t))
            .collect();
        if damage_slots.len() != 7 {
            return Ok(Some(SpecificCommandOutput::text(
                "ダメージスロットの長さに誤りがあります。",
            )));
        }

        // 補正値
        let corr = eval_term(term);

        let dice_command = format!("1D6{}", explicit_sign(corr));
        let total = rng.roll_once(6)?;
        let expr = format!("{total}{}", explicit_sign(corr));
        let slot_number = clamp(total + corr, 1, 7);
        let damage = damage_slots[(slot_number - 1) as usize];
        vec![
            parentheses(&dice_command),
            expr,
            (total + corr).to_string(),
            format!("{damage}ダメージ"),
        ]
    } else {
        return Ok(None);
    };

    Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
}

/// Ruby `/\A\[(.+)\]/`（ダメージスロットの並び）。
fn damage_slots_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\[(.+)\]").expect("valid regex"))
}

/// Ruby `/[+-]?\d+/`（`eval_term` が拾う項）。
fn term_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[+-]?\d+").expect("valid regex"))
}

/// Ruby `AFF2e#explicit_sign`（`format('%+d', i)`）。
fn explicit_sign(i: i64) -> String {
    format!("{i:+}")
}

/// Ruby `AFF2e#eval_term`。`[+-]?\d+` を拾って足し合わせる（他の文字は無視される）。
fn eval_term(term: &str) -> i64 {
    term_pattern()
        .find_iter(term)
        // Ruby の `to_i` は多倍長。i64 に収まらない入力は飽和させる
        // （どちらにせよ表示用の数値でしかない）。
        .map(|m| m.as_str().parse::<i64>().unwrap_or(i64::MAX))
        .fold(0i64, |a, b| a.wrapping_add(b))
}

/// Ruby `AFF2e#parentheses`。
fn parentheses(str: &str) -> String {
    format!("({str})")
}

/// Ruby `AFF2e#successful_or_failed`。
fn successful_or_failed(total: i64, diff: i64) -> &'static str {
    match total {
        2 => {
            if diff <= 1 {
                "成功（大成功ではない）"
            } else {
                "大成功！"
            }
        }
        12 => {
            if diff >= 12 {
                "失敗（大失敗ではない）"
            } else {
                "大失敗！"
            }
        }
        _ => {
            if total <= diff {
                "成功"
            } else {
                "失敗"
            }
        }
    }
}

/// Ruby `AFF2e#critical`。`2` と `12` 以外は `nil`。
fn critical(total: i64) -> Option<&'static str> {
    match total {
        2 => Some("ファンブル！"),
        12 => Some("強打！"),
        _ => None,
    }
}

/// Ruby `AFF2e#clamp`。
fn clamp(i: i64, min: i64, max: i64) -> i64 {
    if i < min {
        min
    } else if i > max {
        max
    } else {
        i
    }
}

/// Ruby `String#split(sep)`（limit 省略）。**末尾の空文字列を落とす**のが Rust との差。
///
/// `"0,0,"` は Ruby だと `["0", "0"]`（2要素）になるので、
/// ダメージスロットの個数検査（`size != 7`）の結果が Rust の `split` とは変わる。
fn ruby_split(s: &str, sep: char) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("AFF2e", "AFF2e.toml", 19);
    }
}
