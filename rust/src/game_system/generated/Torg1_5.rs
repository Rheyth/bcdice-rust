//! P4で手書き移植した `lib/bcdice/game_system/Torg1_5.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! # 親クラス（`Torg`）の扱い
//!
//! Ruby側は `Torg1_5 < Torg` で、`Torg` 側の
//! `replace_text` / `torg_check` / `torg_dice` / `eval_game_system_specific_command` /
//! `get_torg_table_result` / `get_torg_bonus` / `get_torg_bonus_text` を継承し、
//! 各結果表（一般結果・対人行為・ダメージチャート）と `get_torg_damage` だけを
//! 上書きしている。
//!
//! 親 `Torg` は Rust ではまだ未移植のスタブなので、`generated/Torg.rs` は触らず、
//! 必要な親の処理をこのファイルへ取り込んである（親が移植されたら整理する前提）。
//! **ここに置いた表の値は Torg1_5 側の上書き版**であり、`Torg` の同名メソッドの値とは
//! 別物なので流用しないこと。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::{self};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::Torg1_5`（ID: `Torg1.5`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Torg1_5;

impl GameSystem for Torg1_5 {
    fn id(&self) -> &'static str {
        "Torg1.5"
    }

    fn name(&self) -> &'static str {
        "トーグ1.5版"
    }

    fn sort_key(&self) -> &'static str {
        "とおく1.5"
    }

    fn help_message(&self) -> &'static str {
        r#"・判定　(TGm)
　TORG専用の判定コマンドです。
　"TG(技能基本値)"でロールします。Rコマンドに読替されます。
　振り足しを自動で行い、20の出目が出たときには技能無し値も並記します。
・各種表　"(表コマンド)(数値)"で振ります。
　・一般結果表 成功度出力「RTx or RESULTx」
　・威圧/威嚇 対人行為結果表「ITx or INTIMIDATEx or TESTx」
　・挑発/トリック 対人行為結果表「TTx or TAUNTx or TRICKx or CTx」
　・間合い 対人行為結果表「MTx or MANEUVERx」
　・オーズ（一般人）ダメージ　「ODTx or ORDSx or ODAMAGEx」
　・ポシビリティー能力者ダメージ「DTx or DAMAGEx」
　・ボーナス表「BTx+y or BONUSx+y or TOTALx+y」 xは数値, yは技能基本値
"#
    }

    /// Ruby `register_prefix(Torg.prefixes)`。
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "TG",
            "1R20",
            "RT",
            "Result",
            "IT",
            "Intimidate",
            "Test",
            "TT",
            "Taunt",
            "Trick",
            "CT",
            "MT",
            "Maneuver",
            "ODT",
            "ords",
            "odamage",
            "DT",
            "damage",
            "BT",
            "bonus",
            "total",
        ]
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

/// Ruby `Torg#eval_game_system_specific_command`（`Torg1_5` は上書きしない）。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: string = command.upcase（`dice_command` で大文字化済みだが原典どおり）
    let string = replace_text(&command.to_uppercase());

    if let Some(result) = torg_check(&string, rng)? {
        return Ok(Some(SpecificCommandOutput::text(result)));
    }

    let Some(m) = table_command_pattern().captures(&string) else {
        return Ok(None);
    };

    let table_type = m.get(1).expect("group 1 always participates").as_str();
    let num = m.get(2).expect("group 2 always participates").as_str();

    // Ruby: ttype は分岐で必ず埋まる（正規表現がこの6種にしかマッチしないため）。
    //       value は BT だけ文字列（"#{value}+#{mod}"）になりうるので String で持つ。
    let (output, value, ttype) = match table_type {
        "RT" => {
            let value = arithmetic_evaluator_eval(num)?;
            (
                get_torg_success_level(value).to_owned(),
                value.to_string(),
                "一般結果",
            )
        }
        "IT" => {
            let value = arithmetic_evaluator_eval(num)?;
            (
                get_torg_interaction_result_intimidate_test(value).to_owned(),
                value.to_string(),
                "威圧/威嚇",
            )
        }
        "TT" => {
            let value = arithmetic_evaluator_eval(num)?;
            (
                get_torg_interaction_result_taunt_trick(value).to_owned(),
                value.to_string(),
                "挑発/トリック",
            )
        }
        "MT" => {
            let value = arithmetic_evaluator_eval(num)?;
            (
                get_torg_interaction_result_maneuver(value).to_owned(),
                value.to_string(),
                "間合い",
            )
        }
        "DT" => {
            let value = arithmetic_evaluator_eval(num)?;
            // Ruby: string =~ /ODT/i
            if string.contains("ODT") {
                (
                    get_torg_damage_ords(value),
                    value.to_string(),
                    "オーズダメージ",
                )
            } else {
                (
                    get_torg_damage_posibility(value),
                    value.to_string(),
                    "ポシビリティ能力者ダメージ",
                )
            }
        }
        // 正規表現 `[RITMDB]T` の残りは BT のみ。
        _ => {
            let (output, value) = get_torg_bonus_text(num)?;
            (output, value, "ボーナス")
        }
    };

    // Ruby: if ttype != '' （分岐で必ず埋まるので常に真）
    Ok(Some(SpecificCommandOutput::text(format!(
        "{ttype}表[{value}] ＞ {output}"
    ))))
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（`Arithmetic.eval(expr, :floor) || 0`）。
fn arithmetic_evaluator_eval(expr: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `/([RITMDB]T)(\d+([+-]\d+)*)/i`。
fn table_command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)([RITMDB]T)(\d+([+-]\d+)*)").expect("valid regex"))
}

/// Ruby `Torg#replace_text`。表コマンドの別名を正規形へ畳む。
fn replace_text(string: &str) -> String {
    /// `(パターン, 置換文字列)` の並び。順序に意味がある（原典の `gsub` の順）。
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (r"(?i)Result", "RT"),
            (r"(?i)(Intimidate|Test)", "IT"),
            (r"(?i)(Taunt|Trick|CT)", "TT"),
            (r"(?i)Maneuver", "MT"),
            (r"(?i)(ords|odamage)", "ODT"),
            (r"(?i)damage", "DT"),
            (r"(?i)(bonus|total)", "BT"),
            (r"(?i)TG(\d+)", "1R20+${1}"),
            (r"(?i)TG", "1R20"),
        ]
        .into_iter()
        .map(|(pat, rep)| (Regex::new(pat).expect("valid regex"), rep))
        .collect()
    });

    let mut string = string.to_owned();
    for (re, rep) in rules {
        string = re.replace_all(&string, *rep).into_owned();
    }
    string
}

/// Ruby `/(^|\s)S?(1R20([+-]\d+)*)(\s|$)/i`。
///
/// `Preprocessor` が最初の空白より前しか残さないので `\s` 側の枝は実際には通らないが、
/// 原典どおりに残す。
fn torg_check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)S?(1R20([+-]\d+)*)(\s|$)").expect("valid regex"))
}

/// Ruby `Torg#torg_check`（`1R20` の振り足し判定）。
fn torg_check(string: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(md) = torg_check_pattern().captures(string) else {
        return Ok(None);
    };

    // Ruby: string = Regexp.last_match(2); mod = Regexp.last_match(3)
    // グループ3は `([+-]\d+)*` の**最後の1回**しか残らない（Rubyと同じ挙動）。
    let string = md.get(2).expect("group 2 always participates").as_str();
    // Ruby: mod = ArithmeticEvaluator.eval(mod) if mod; mod = mod.to_i（nil なら 0）
    let modifier = match md.get(3) {
        Some(mo) => arithmetic_evaluator_eval(mo.as_str())?,
        None => 0,
    };

    let (skilled, unskilled, dice_str) = torg_dice(rng)?;
    let sk_bonus =
        get_torg_bonus(skilled).ok_or(EvalError::Internal("Torg: 1D20 rolled below 1"))?;

    // Ruby は `if mod` で分岐するが、直前の `mod.to_i` で必ず Integer になるため
    // 「mod が nil」の枝（修正値なしの表記）には到達しない。0 のときは "…0" が付く。
    let mut output = if modifier > 0 {
        format!("{sk_bonus}[{dice_str}]+{modifier}")
    } else {
        format!("{sk_bonus}[{dice_str}]{modifier}")
    };

    output += " ＞ ";
    output += &(sk_bonus + modifier).to_string();

    if skilled != unskilled {
        let unsk_bonus =
            get_torg_bonus(unskilled).ok_or(EvalError::Internal("Torg: 1D20 rolled below 1"))?;
        output += "(技能無";
        output += &(unsk_bonus + modifier).to_string();
        output += ")";
    }

    Ok(Some(format!("({string}) ＞ {output}")))
}

/// Ruby `Torg#torg_dice`。20（技能あり側のみ振り足し）と10（両方振り足し）で振り足す。
///
/// 戻り値は `(skilled, unskilled, dice_str)`。
fn torg_dice(rng: &mut Randomizer) -> Result<(i64, i64, String), EvalError> {
    let mut is_skilled_critical = true;
    let mut is_critical = true;
    let mut skilled = 0;
    let mut unskilled = 0;
    let mut dice_str = String::new();

    while is_skilled_critical {
        let dice_n = rng.roll_once(20)?;
        skilled += dice_n;
        if is_critical {
            unskilled += dice_n;
        }

        if !dice_str.is_empty() {
            dice_str.push(',');
        }
        dice_str += &dice_n.to_string();

        if dice_n == 20 {
            is_critical = false;
        } else if dice_n != 10 {
            is_skilled_critical = false;
            is_critical = false;
        }
    }

    Ok((skilled, unskilled, dice_str))
}

/// Ruby `Torg#get_torg_table_result`。
///
/// 添字が最初の項目より小さいときは Ruby の初期値である文字列 `"1"` が返る。
fn get_torg_table_result(value: i64, table: &[(i64, &'static str)]) -> &'static str {
    let mut output = "1";
    for (item_index, item_body) in table {
        if *item_index > value {
            break;
        }
        output = item_body;
    }
    output
}

/// Ruby `Torg#get_torg_bonus`。
///
/// Ruby は `value < 1` のとき `get_torg_table_result` の初期値である**文字列** `"1"` を
/// 返すので、整数として使うと `String + Integer` で TypeError になる。
/// Rust では `None` で区別し、呼び出し側でその枝を明示する。
fn get_torg_bonus(value: i64) -> Option<i64> {
    /// Ruby `bonus_table`。
    static BONUS_TABLE: &[(i64, i64)] = &[
        (1, -12),
        (2, -10),
        (3, -8),
        (5, -5),
        (7, -2),
        (9, -1),
        (11, 0),
        (13, 1),
        (15, 2),
        (16, 3),
        (17, 4),
        (18, 5),
        (19, 6),
        (20, 7),
    ];

    let mut bonus: Option<i64> = None;
    for (item_index, item_body) in BONUS_TABLE {
        if *item_index > value {
            break;
        }
        bonus = Some(*item_body);
    }

    let mut bonus = bonus?;
    if value > 20 {
        // Ruby `Integer#/` は床除算
        let over_value_bonus = (value - 21).div_euclid(5) + 1;
        bonus += over_value_bonus;
    }

    Some(bonus)
}

/// Ruby `Torg#get_torg_bonus_text`。戻り値は `(output, value)`。
fn get_torg_bonus_text(num: &str) -> Result<(String, String), EvalError> {
    // Ruby: val_arr = num.split(/\+/); value = val_arr.shift.to_i
    let mut val_arr: Vec<&str> = num.split('+').collect();
    // Ruby の `split` は末尾の空文字列を落とす。
    while val_arr.last() == Some(&"") {
        val_arr.pop();
    }
    let first = if val_arr.is_empty() {
        // Ruby: [].shift は nil。`nil.to_i` は 0。
        ""
    } else {
        val_arr.remove(0)
    };
    let value = ruby_to_i(first);

    let modifier = arithmetic_evaluator_eval(&val_arr.join("+"))?;
    let result_value = get_torg_bonus(value);

    if modifier == 0 {
        // Ruby: output = resultValue.to_s（`get_torg_bonus` が文字列 "1" を返す枝も含む）
        let output = result_value.map_or_else(|| "1".to_owned(), |v| v.to_string());
        return Ok((output, value.to_string()));
    }

    // Ruby `getTorgBonusOutputTextWhenModDefined` は `resultValue + mod` を計算するので、
    // `get_torg_bonus` が文字列 "1" を返す（value < 1）と TypeError で落ちる。
    let result_value =
        result_value.ok_or(EvalError::Internal("Torg: BT with a bonus value below 1"))?;
    let output = if modifier > 0 {
        format!(
            "{result_value}[{value}]+{modifier} ＞ {}",
            result_value + modifier
        )
    } else {
        format!(
            "{result_value}[{value}]{modifier} ＞ {}",
            result_value + modifier
        )
    };

    Ok((output, format!("{value}+{modifier}")))
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn ruby_to_i(s: &str) -> i64 {
    str_helpers::ruby_to_i(s)
}

// ---------------------------------------------------------------------------
// Torg1_5 が上書きしている各種結果表
// ---------------------------------------------------------------------------

/// Ruby `Torg1_5#get_torg_success_level`（一般結果表 成功度）。
fn get_torg_success_level(value: i64) -> &'static str {
    static SUCCESS_TABLE: &[(i64, &str)] = &[
        (0, "ぎりぎり"),
        (1, "ふつう"),
        (3, "まあよい"),
        (7, "かなりよい"),
        (12, "すごい"),
    ];

    get_torg_table_result(value, SUCCESS_TABLE)
}

/// Ruby `Torg1_5#get_torg_interaction_result_intimidate_test`（威圧／威嚇）。
fn get_torg_interaction_result_intimidate_test(value: i64) -> &'static str {
    static INTERACTION_RESULTS_TABLE: &[(i64, &str)] = &[
        (0, "萎縮"),
        (5, "技能なし"),
        (10, "逆転負け"),
        (15, "モラル崩壊"),
        (17, "プレイヤーズコール"),
    ];

    get_torg_table_result(value, INTERACTION_RESULTS_TABLE)
}

/// Ruby `Torg1_5#get_torg_interaction_result_taunt_trick`（挑発／トリック）。
fn get_torg_interaction_result_taunt_trick(value: i64) -> &'static str {
    static INTERACTION_RESULTS_TABLE: &[(i64, &str)] = &[
        (0, "萎縮"),
        (5, "技能なし"),
        (10, "逆転負け"),
        (15, "高揚／逆転負け"),
        (17, "プレイヤーズコール"),
    ];

    get_torg_table_result(value, INTERACTION_RESULTS_TABLE)
}

/// Ruby `Torg1_5#get_torg_interaction_result_maneuver`（間合い）。
fn get_torg_interaction_result_maneuver(value: i64) -> &'static str {
    static INTERACTION_RESULTS_TABLE: &[(i64, &str)] = &[
        (0, "疲労"),
        (5, "萎縮"),
        (10, "技能なし"),
        (15, "逆転負け／疲労"),
        (17, "プレイヤーズコール"),
    ];

    get_torg_table_result(value, INTERACTION_RESULTS_TABLE)
}

/// Ruby `Torg1_5#get_torg_damage_ords`（オーズダメージチャート）。
fn get_torg_damage_ords(value: i64) -> String {
    static DAMAGE_TABLE_ORDS: &[(i64, &str)] = &[
        (0, "1"),
        (1, "O1"),
        (2, "K1"),
        (3, "O2"),
        (4, "K2"),
        (5, "転倒 O3"),
        (6, "転倒 K3"),
        (7, "転倒 K／O4"),
        (8, "1レベル負傷  KO4"),
        (9, "1レベル負傷  K／O5"),
        (10, "1レベル負傷  KO5"),
        (11, "2レベル負傷  K／O6"),
        (12, "2レベル負傷  KO6"),
        (13, "3レベル負傷  K／O7"),
        (14, "3レベル負傷  KO7"),
        (15, "4レベル負傷  KO8"),
    ];

    get_torg_damage(value, 4, 8, DAMAGE_TABLE_ORDS)
}

/// Ruby `Torg1_5#get_torg_damage_posibility`（ポシビリティー能力者ダメージチャート）。
fn get_torg_damage_posibility(value: i64) -> String {
    static DAMAGE_TABLE_POSIBILITY: &[(i64, &str)] = &[
        (0, "1"),
        (1, "1"),
        (2, "O1"),
        (3, "K1"),
        (4, "2"),
        (5, "O2"),
        (6, "転倒 K2"),
        (7, "転倒 O3"),
        (8, "転倒 K3"),
        (9, "転倒 K／O3"),
        (10, "1レベル負傷  K／O4"),
        (11, "1レベル負傷  K／O4"),
        (12, "1レベル負傷  KO4"),
        (13, "2レベル負傷  K／O5"),
        (14, "2レベル負傷  KO5"),
        (15, "3レベル負傷  KO5"),
    ];

    get_torg_damage(value, 3, 5, DAMAGE_TABLE_POSIBILITY)
}

/// Ruby `Torg1_5#get_torg_damage`（表を超えた分をオーバーキルとして換算する）。
///
/// 親 `Torg` の同名メソッドとは引数（`maxDamageString`）も式も違う。
fn get_torg_damage(
    value: i64,
    max_damage: i64,
    max_shock: i64,
    damage_table: &[(i64, &'static str)],
) -> String {
    if value < 0 {
        return "1".to_owned();
    }

    let table_max_value = damage_table.len() as i64 - 1;

    if value <= table_max_value {
        return get_torg_table_result(value, damage_table).to_owned();
    }

    // Ruby `Integer#/` は床除算
    let over_kill_value = (value - table_max_value).div_euclid(2);
    let over_kill_damage = max_damage + over_kill_value;
    let over_kill_shock = max_shock + over_kill_value;

    format!("{over_kill_damage}レベル負傷  KO{over_kill_shock}")
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
            .join("test/data/Torg1_5.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Torg1_5.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Torg1_5.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Torg1_5.toml must parse");
        assert_eq!(data.tests.len(), 16, "case count in test/data/Torg1_5.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Torg1.5",
                "unexpected game system in Torg1_5.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Torg1.5"), &tc.input, &mut src) {
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
                    "FAIL Torg1.5:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Torg1.5 cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
