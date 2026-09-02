//! P4で手書き移植した `lib/bcdice/game_system/Postman.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#checkRoll`（判定 `[n]PO[+-a][> / >= / @X]`）
//! - `#get_weather_table`（天候チェック `WEA[n]`）
//! - `#get_free_situation_table`（自由行動シチュエーション表 `FRE`）
//!
//! 表データは `Postman.rb` にハードコードされているものを1文字も変えずに写した。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `when /(\d+)?PO(\d+)?(([+-]\d+)*)?((>|>=|@)(\d+)(([+-]\d+)*)?)?/i`。
///
/// Ruby側はアンカーなしの部分一致なので、`find`／`captures`（最左マッチ）で等価。
fn po_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(\d+)?PO(\d+)?(([+-]\d+)*)?((>|>=|@)(\d+)(([+-]\d+)*)?)?").unwrap()
    })
}

/// Ruby `when /WEA(\d+)?/i`。
fn wea_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)WEA(\d+)?").unwrap())
}

/// Ruby `String#scan(/[+-]\d+/)` の合計。
fn sum_modifiers(s: &str) -> i64 {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[+-]\d+").unwrap());
    re.find_iter(s)
        .filter_map(|m| m.as_str().parse::<i64>().ok())
        .sum()
}

/// Ruby `get_table_by_number(index, table)`。番号が `index` 以上の最初の項目を返す。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    for (number, text) in table {
        if *number >= index {
            return text;
        }
    }
    // Ruby: default = "1"
    "1"
}

/// Ruby `#checkRoll(diceCount, modify, type, target)`。
///
/// ダイスが2個未満しか振れなかった場合は `None`（＝コマンドとして成立しない）を返す。
/// `roll_barabara` は個数が `UPPER_LIMIT_DICE_TIMES`（200）を超えると空配列を返すので、
/// `201PO` のような入力でここに来る。Ruby は `diceArray[-2]` が `nil` になり
/// `nil + nil` で `NoMethodError` を起こす（`Postman` に rescue は無い）が、
/// 本移植は本家のクラッシュを再現しない方針（`command_parser.rs` / `dice_table/mod.rs`
/// と同じ扱い）なので、出力なしに畳む。
fn check_roll(
    dice_count: i64,
    modify: i64,
    r#type: &str,
    target: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let mut dice_array = rng.roll_barabara(dice_count, 6)?;
    dice_array.sort_unstable();
    let dice: i64 = dice_array.iter().sum();
    let dice_text = dice_array
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // Ruby: diceArray[-2] / diceArray[-1]
    if dice_array.len() < 2 {
        return Ok(None);
    }
    let last2 = dice_array[dice_array.len() - 2];
    let last1 = dice_array[dice_array.len() - 1];
    let dice2 = last2 + last1;
    let dice_text2 = format!("{last2},{last1}");
    let critical_count = dice_array.iter().filter(|d| **d == 6).count();

    // Ruby: modifyText は modify == 0 のとき未代入（nil → 補間で ""）
    let modify_text = if modify != 0 {
        let mut t = String::new();
        if modify > 0 {
            t.push('+');
        }
        t.push_str(&modify.to_string());
        t
    } else {
        String::new()
    };

    let result = dice2 + modify;

    // Ruby: resultText / operatorText も type == '' のときは nil
    let mut result_text = String::new();
    let mut operator_text = String::new();
    if !r#type.is_empty() {
        result_text = " 【失敗】".to_owned();
        operator_text = ">".to_owned();
        if r#type == ">" {
            if result > target {
                result_text = " 【成功】".to_owned();
            }
        } else {
            operator_text += "=";
            if result >= target {
                result_text = " 【成功】".to_owned();
            }
        }
    }

    if critical_count >= 2 {
        result_text = " 【成功】（クリティカル）".to_owned();
    } else if dice == dice_count {
        result_text = " 【失敗】（ファンブル）".to_owned();
    }

    let mut text = format!(
        "{dice_count}D6({dice_text}){modify_text} ＞ {dice2}({dice_text2}){modify_text} = 達成値：{result}"
    );
    if target > 0 {
        text += &format!("{operator_text}{target} ");
    }
    text += &result_text;

    Ok(Some(text))
}

/// Ruby `#get_weather_table(roc)`。
fn get_weather_table(roc: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let name = "天候チェック";

    let (dice, dice_text) = if roc == 0 {
        let dice_list = rng.roll_barabara(2, 6)?;
        let dice: i64 = dice_list.iter().sum();
        let text = dice_list
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        (dice, text)
    } else {
        let roc = roc.clamp(2, 12);
        (roc, format!("Choice:{roc}"))
    };

    let table_text = get_table_by_number(dice, WEATHER_TABLE);
    Ok(format!("{name}({dice_text}) ＞ {dice}：{table_text}"))
}

/// Ruby `#get_free_situation_table`。
fn get_free_situation_table(rng: &mut Randomizer) -> Result<String, EvalError> {
    let name = "自由行動シチュエーション表";
    let dice_list = rng.roll_barabara(2, 6)?;
    let dice: i64 = dice_list.iter().sum();
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let table_text = get_table_by_number(dice, FREE_SITUATION_TABLE);
    Ok(format!("{name}({dice_text}) ＞ {dice}：{table_text}"))
}

/// Ruby `get_weather_table` の `table`（2D6）。
static WEATHER_TABLE: &[(i64, &str)] = &[
    (2, "大雨と強風。探索判定の難易度に+4。"),
    (3, "風が強い1日になりそう。探索判定の難易度に+2。"),
    (4, "晴れ。特になし。"),
    (5, "夜の間の雨でぬかるむ。探索判定の難易度に+2。"),
    (6, "それなりの雨足。探索判定の難易度に+2。"),
    (7, "晴れ。特になし。"),
    (8, "天気は大荒れ。探索判定の難易度に+4。"),
    (9, "小雨が降る。探索判定の難易度に+1。"),
    (10, "それなりの雨足。探索判定の難易度に+2。"),
    (11, "晴れ。特になし。"),
    (12, "風が強い1日になりそう。探索判定の難易度に+2。"),
];

/// Ruby `get_free_situation_table` の `table`（2D6）。
static FREE_SITUATION_TABLE: &[(i64, &str)] = &[
    (2, "何をするでもなく、霞がかったような夜空を見上げる。ふと隣に目を向ければ、彼/彼女が居た。彼/彼女は、こうなる前の夜空を知っているのだろうか。"),
    (3, "夢を見た。大戦の最中、街が、人が、世界が焼けていく悪夢を。追い立てられるようにして目を覚ますと、彼/彼女が君を見ていた。　……もしかして、自分はよほどうなされていたのだろうか。"),
    (4, "周囲で見つけたガラクタを使って、ちょっとしたビックリ玩具を作ってみた。「彼/ 彼女」にコイツをけしかけたら、どんな反応をするだろうか？"),
    (5, "使えそうなものがないか探していると、カタンと物音がして何かが落ちた。拾い上げてみたそれは、かつてここで生活していた誰かの名残（写真、家具、玩具等）だった。"),
    (6, "テントの中で夜を過ごしていると、ふと彼/彼女と話したくてたまらない気持ちになった。言ってしまえば、夜の静けさに寂しさを覚えてしまったのだ。"),
    (7, "ここまでの配達の記録をつけていたら、背後から視線を感じる……！　もしや、彼/彼女に覗かれている……！？"),
    (8, "周囲を探索していると、君一人では手の届かないところに金属製の箱か何かがあることに気づいた。彼/彼女に手伝ってもらえば、取れるだろうか……？"),
    (9, "朝まではまだしばらくあるというのに、目が覚めてしまった。二度寝しようにも寝付けずに居ると、隣でもぞもぞと動く気配がする。彼/彼女も、どうやら同じらしい。"),
    (10, "他愛のない話をするうちに、君は彼/彼女に問いかけていた。「何故、ポストマンになろうと思ったのか」　……そういえば、君自身はどうだったろうか。"),
    (11, "保存食にありつこうとしたその時、君は気づいた。一匹のネズミが、彼/彼女の荷物の中に潜り込もうとしている。彼/彼女は気づいていないが、このままでは食料が危ない！"),
    (12, "テントを設営し、落ち着いた頃にふと気づく。　……身体が熱い。少し、だるさもあるような気もする。大したことはないと思うが、彼/彼女に相談しておいた方がいいだろうか。"),
];

/// Ruby `BCDice::GameSystem::Postman`（ID: `Postman`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Postman;

impl GameSystem for Postman {
    fn id(&self) -> &'static str {
        "Postman"
    }

    fn name(&self) -> &'static str {
        "壊れた世界のポストマン"
    }

    fn sort_key(&self) -> &'static str {
        "こわれたせかいのほすとまん"
    }

    fn help_message(&self) -> &'static str {
        r"◆判定：[n]PO[+-a][> or >= or @X]　　[]内省略可。

達成値と判定の成否、クリティカル、ファンブルを結果表示します。
「n」でダイス数を指定。省略時は2D。
「+-a」で達成値への修正を指定。「+2+1-4」のような複数回指定可。
「>X」「>=X」「@X」で難易度を指定可。
「>X」は達成値>難易度、「>=X」「@X」は達成値>=難易度で判定します。

【書式例】
3PO+2-1 → 3Dで達成値修正+1の判定。達成値のみ表示。
PO@5+2 → 2Dで目標値7の判定。判定の成否と達成値を表示。
4PO-2+1>7+2 → 4Dで達成値修正-1、目標値9（同値は失敗）の判定。


◆天候チェック：WEA[n]　　[]内省略可。

天候チェック表を参照します。
「n」を指定すると、指定した結果を表示します。（【幸運点】使用時用）


◆自由行動シチュエーション表：FRE
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["WEA", r"(\d+)?PO", "FRE"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command.upcase
        let command = command.to_uppercase();

        if let Some(caps) = po_re().captures(&command) {
            let mut dice_count = caps
                .get(1)
                .and_then(|m| m.as_str().parse::<i64>().ok())
                .unwrap_or(2);
            if dice_count < 2 {
                dice_count = 2;
            }

            let mut modify = caps
                .get(2)
                .and_then(|m| m.as_str().parse::<i64>().ok())
                .unwrap_or(0);
            let modify_add_string = caps.get(3).map_or("", |m| m.as_str());

            let r#type = caps.get(6).map_or("", |m| m.as_str());
            let mut target = caps
                .get(7)
                .and_then(|m| m.as_str().parse::<i64>().ok())
                .unwrap_or(0);
            let target_add_string = caps.get(8).map_or("", |m| m.as_str());

            modify += sum_modifiers(modify_add_string);

            if target != 0 {
                target += sum_modifiers(target_add_string);
            }

            return Ok(check_roll(dice_count, modify, r#type, target, rng)?
                .map(SpecificCommandOutput::text));
        }

        if let Some(caps) = wea_re().captures(&command) {
            let roc = caps
                .get(1)
                .and_then(|m| m.as_str().parse::<i64>().ok())
                .unwrap_or(0);
            return Ok(Some(SpecificCommandOutput::text(get_weather_table(
                roc, rng,
            )?)));
        }

        if command == "FRE" {
            return Ok(Some(SpecificCommandOutput::text(get_free_situation_table(
                rng,
            )?)));
        }

        // Ruby: text が nil のまま返る
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Postman", "Postman.toml", 31);
    }
}
