//! P4で手書き移植した `lib/bcdice/game_system/GehennaAn.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GehennaAn#replace_text`（`nGAt+m` / `nGt+m` → `nR6+m>=t[mode]` への書き換え）
//! - `GehennaAn#eval_game_system_specific_command`（幸運の助けを含む成功数判定）
//! - `getAnastasisBonusText` / `getTougiBonus`（連撃増加値・闘技チット）

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::arithmetic::{self};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::GehennaAn`（ID: `GehennaAn`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GehennaAn;

impl GameSystem for GehennaAn {
    fn id(&self) -> &'static str {
        "GehennaAn"
    }

    fn name(&self) -> &'static str {
        "ゲヘナ・アナスタシス"
    }

    fn sort_key(&self) -> &'static str {
        "けへなあなすたしす"
    }

    fn help_message(&self) -> &'static str {
        r"戦闘判定と通常判定に対応。幸運の助け、連撃増加値(戦闘判定)、闘技チット(戦闘判定)を自動表示します。
・戦闘判定　(nGAt+m)
　ダイス数n、目標値t、修正値mで戦闘判定を行います。
　幸運の助け、連撃増加値、闘技チットを自動処理します。
・通常判定　(nGt+m)
　ダイス数n、目標値t、修正値mで通常判定を行います。
　幸運の助けを自動処理します。(連撃増加値、闘技チットを表示抑制します)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+G\d+", r"\d+GA\d+", r"\d+R6"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `GehennaAn#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let string = replace_text(command);

        // Ruby: unless /…/i =~ string -> return nil
        let Some(m) = roll_pattern().captures(&string) else {
            return Ok(None);
        };

        let string = m[2].to_owned();
        let dice_count = to_i(&m[3]);
        let mod_text = m.get(4).map(|x| x.as_str());
        let diff = to_i(&m[6]);
        let mode = to_i(&m[8]);

        // Ruby: ArithmeticEvaluator.eval(modText)（nil も不正な式も 0）
        let mod_value = match mod_text {
            Some(expr) => arithmetic::eval(expr, RoundType::Floor)?
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0),
            None => 0,
        };

        let mut dice_array = rng.roll_barabara(dice_count, 6)?;
        dice_array.sort_unstable();
        // Ruby は `diceValue = diceArray.sum()` を一度代入するが、
        // 幸運の助けチェックの直前に 0 で上書きするので合計は使われない。
        let dice_text = dice_array
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");

        // 幸運の助けチェック
        let mut dice_1st: Option<i64> = None;
        let mut is_luck = true;
        let mut dice_value: i64 = 0;

        for i in &dice_array {
            match dice_1st {
                Some(first) => {
                    if (first != *i) || (*i < diff) {
                        is_luck = false;
                    }
                }
                None => dice_1st = Some(*i),
            }

            if *i >= diff {
                dice_value += 1;
            }
        }

        if is_luck && dice_count > 1 {
            dice_value *= 2;
        }

        let mut output = format!("{dice_value}[{dice_text}]");
        let mut success = dice_value + mod_value;
        if success < 0 {
            success = 0;
        }

        let mut failed = dice_count - dice_value;
        if failed < 0 {
            failed = 0;
        }

        if mod_value > 0 {
            output += &format!("+{mod_value}");
        } else if mod_value < 0 {
            output += &mod_value.to_string();
        }

        // Ruby: if /[^\d\[\]]+/ =~ output
        if output
            .chars()
            .any(|c| !c.is_ascii_digit() && c != '[' && c != ']')
        {
            output = format!("({string}) ＞ {output} ＞ 成功{success}、失敗{failed}");
        } else {
            output = format!("({string}) ＞ {output}");
        }

        // 連撃増加値と闘技チット
        output += &anastasis_bonus_text(mode, success);

        Ok(Some(SpecificCommandOutput::text(output)))
    }
}

/// Ruby `String#to_i` 相当（`\d+` にマッチした部分文字列を整数にする）。
///
/// 桁あふれする入力は Ruby だと Bignum になり、`roll_barabara` の個数上限で
/// TooManyRandsError になる。i64 に収まらない場合も同じ経路へ落とす。
/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX` に飽和。
fn to_i(text: &str) -> i64 {
    str_helpers::to_i_max(text)
}

/// Ruby `GehennaAn#replace_text`。4つの `gsub` を宣言順に適用する。
fn replace_text(string: &str) -> String {
    static RE_GA_MOD: OnceLock<Regex> = OnceLock::new();
    static RE_GA: OnceLock<Regex> = OnceLock::new();
    static RE_G_MOD: OnceLock<Regex> = OnceLock::new();
    static RE_G: OnceLock<Regex> = OnceLock::new();

    let re_ga_mod =
        RE_GA_MOD.get_or_init(|| Regex::new(r"(\d+)GA(\d+)([+-][+\-\d]+)").expect("valid regex"));
    let re_ga = RE_GA.get_or_init(|| Regex::new(r"(\d+)GA(\d+)").expect("valid regex"));
    let re_g_mod =
        RE_G_MOD.get_or_init(|| Regex::new(r"(\d+)G(\d+)([+-][+\-\d]+)").expect("valid regex"));
    let re_g = RE_G.get_or_init(|| Regex::new(r"(\d+)G(\d+)").expect("valid regex"));

    let s = re_ga_mod.replace_all(string, |c: &Captures| {
        format!("{}R6{}>={}[1]", &c[1], &c[3], &c[2])
    });
    let s = re_ga.replace_all(&s, |c: &Captures| format!("{}R6>={}[1]", &c[1], &c[2]));
    let s = re_g_mod.replace_all(&s, |c: &Captures| {
        format!("{}R6{}>={}[0]", &c[1], &c[3], &c[2])
    });
    let s = re_g.replace_all(&s, |c: &Captures| format!("{}R6>={}[0]", &c[1], &c[2]));
    s.into_owned()
}

/// Ruby の判定コマンド抽出正規表現。
///
/// Ruby: `/(^|\s)S?((\d+)[rR]6([+\-\d]+)?([>=]+(\d+))(\[(\d)\]))(\s|$)/i`
fn roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)S?((\d+)[rR]6([+\-\d]+)?([>=]+(\d+))(\[(\d)\]))(\s|$)")
            .expect("valid regex")
    })
}

/// Ruby `GehennaAn#getAnastasisBonusText`。
fn anastasis_bonus_text(mode: i64, success: i64) -> String {
    if mode == 0 {
        return String::new();
    }

    // Ruby `Integer#/` は床除算
    let mut ma_bonus = (success - 1).div_euclid(2);
    if ma_bonus > 7 {
        ma_bonus = 7;
    }

    let mut bonus_str = String::new();
    if ma_bonus > 0 {
        bonus_str += &format!("連撃[+{ma_bonus}]/");
    }
    bonus_str += &format!("闘技[{}]", tougi_bonus(success));
    format!(" ＞ {bonus_str}")
}

/// Ruby `GehennaAn#getTougiBonus`（`Base#get_table_by_number` の既定値は `"1"`）。
fn tougi_bonus(success: i64) -> &'static str {
    static TABLE: &[(i64, &str)] = &[(6, "1"), (13, "2"), (18, "3"), (22, "4"), (99, "5")];

    for (number, value) in TABLE {
        if *number >= success {
            return value;
        }
    }
    "1"
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GehennaAn",
            "GehennaAn.toml",
            19,
        );
    }
}
