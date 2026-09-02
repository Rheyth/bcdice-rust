//! P4で手書き移植した `lib/bcdice/game_system/BBN.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command` → `xBN±y>=z[c,f]` の判定
//! - `#critical_base` / `#critical_?` / `#fumble_?` / `#additional_roll`

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BBN#parse` の正規表現。
///
/// `/^(\d+)BN([+-]\d+)?(>=(([+-]?\d+)))?(\[([+-]?\d+)?(,([+-]?\d+))?\])?/`
fn command_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\d+)BN([+-]\d+)?(>=(([+-]?\d+)))?(\[([+-]?\d+)?(,([+-]?\d+))?\])?")
            .expect("valid regex")
    })
}

/// Ruby `BBN#parse` の結果。
struct Parsed {
    roll_times: i64,
    modify_str: String,
    modify: i64,
    difficulty: Option<i64>,
    critical: i64,
    fumble: i64,
}

/// Ruby `BBN#parse`。
fn parse(command: &str) -> Option<Parsed> {
    let m = command_re().captures(command)?;

    let roll_times: i64 = m.get(1)?.as_str().parse().ok()?;
    let modify_str = m.get(2).map(|c| c.as_str()).unwrap_or("");
    // Ruby: m[2].to_i（nil の場合は 0）
    let modify = to_i(m.get(2).map(|c| c.as_str()));
    let difficulty = m.get(4).and_then(|c| c.as_str().parse::<i64>().ok());

    let base = critical_base(roll_times);

    Some(Parsed {
        roll_times,
        modify_str: modify_str.to_string(),
        modify,
        difficulty,
        critical: base + to_i(m.get(7).map(|c| c.as_str())),
        fumble: base + to_i(m.get(9).map(|c| c.as_str())),
    })
}

/// Ruby `String#to_i` 相当（`nil` は 0）。正規表現が符号つき整数のみを捕捉するので
/// 前半一致の処理は不要。
fn to_i(s: Option<&str>) -> i64 {
    s.and_then(|s| s.parse::<i64>().ok()).unwrap_or(0)
}

/// Ruby `BBN#critical_base`。振るダイスの数からクリティカルとファンブルの基本値を算出する。
fn critical_base(roll_times: i64) -> i64 {
    match roll_times {
        1 | 2 => roll_times,
        // Ruby: (roll_times.to_f / 2).ceil
        n => n.div_euclid(2) + n.rem_euclid(2),
    }
}

/// Ruby `BBN#additional_roll`。クリティカルの追加ロールをする。
fn additional_roll(
    parsed: &Parsed,
    mut additional_dice: i64,
    mut total: i64,
    rng: &mut Randomizer,
) -> Result<Vec<String>, EvalError> {
    let mut sequence: Vec<String> = Vec::new();
    let mut reroll_count = 0;

    // 追加クリティカルは無限ループしうるので、10回に制限
    while additional_dice > 0 && reroll_count < 10 {
        reroll_count += 1;

        let dice_list = rng.roll_barabara(additional_dice, 6)?;
        let dice_total: i64 = dice_list.iter().sum();
        let dice_str = join(&dice_list);
        additional_dice = count(&dice_list, 6);

        sequence.push(format!("{total}+{dice_total}[{dice_str}]"));
        if additional_dice > 0 {
            sequence.push("追加クリティカル！".to_string());
        }

        total += dice_total;
    }

    if additional_dice > 0 {
        sequence.push("無限ループ防止のため中断".to_string());
    }

    sequence.push(total.to_string());
    if let Some(difficulty) = parsed.difficulty {
        sequence.push(
            if total >= difficulty {
                "成功"
            } else {
                "失敗"
            }
            .to_string(),
        );
    }

    Ok(sequence)
}

/// Ruby `Array#join(",")`。
fn join(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `Array#count(value)`。
fn count(dice_list: &[i64], value: i64) -> i64 {
    dice_list.iter().filter(|&&d| d == value).count() as i64
}

pub struct BBN;

impl GameSystem for BBN {
    fn id(&self) -> &'static str {
        "BBN"
    }

    fn name(&self) -> &'static str {
        "BBNTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ひいひいえぬTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定(xBN±y>=z[c,f])
　xD6の判定。クリティカル、ファンブルの自動判定を行います。
　1Dのクリティカル値とファンブル値は1。2Dのクリティカル値とファンブル値は2。
　nDのクリティカル値とファンブル値は n/2 の切り上げ。
　クリティカルとファンブルが同時に発生した場合、クリティカルを優先。
　x：xに振るダイス数を入力。
　y：yに修正値を入力。省略可能。
  z：zに目標値を入力。省略可能。
  c：クリティカルに必要なダイス目「6」の数の増減。省略可能。
  f：ファンブルに必要なダイス目「1」の数の増減。省略可能。
　例） 3BN+4　3BN>=8　3BN+1>=10[-1] 3BN+1>=10[,1] 3BN+1>=10[1,1]
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+BN"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let Some(parsed) = parse(command) else {
            return Ok(None);
        };

        // ダイスロール
        let dice_list = rng.roll_barabara(parsed.roll_times, 6)?;
        let dice: i64 = dice_list.iter().sum();
        let dice_str = join(&dice_list);

        let total = dice + parsed.modify;

        // 出力文の生成
        let mut sequence = vec![
            format!("({command})"),
            format!("{dice}[{dice_str}]{}", parsed.modify_str),
            total.to_string(),
        ];

        // クリティカルとファンブルが同時に発生した時にはクリティカルが優先
        if count(&dice_list, 6) >= parsed.critical {
            sequence.push("クリティカル！".to_string());
            sequence.extend(additional_roll(&parsed, count(&dice_list, 6), total, rng)?);
        } else if count(&dice_list, 1) >= parsed.fumble {
            sequence.push("ファンブル！".to_string());
        } else if let Some(difficulty) = parsed.difficulty {
            sequence.push(
                if total >= difficulty {
                    "成功"
                } else {
                    "失敗"
                }
                .to_string(),
            );
        }

        Ok(Some(SpecificCommandOutput::text(sequence.join(" ＞ "))))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("BBN", "BBN.toml", 29);
    }
}
