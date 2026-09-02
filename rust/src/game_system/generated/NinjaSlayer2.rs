//! P4で手書き移植した `lib/bcdice/game_system/NinjaSlayer2.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#proc_dice_2nd`（成功判定 `K{x}` / `E{x}` … のカンマ区切り複数ロール）と
//!   `#proc_appendix`（追加判定 `[>=y]` / `[Sy]` / `[Cy]` …）
//! - `#proc_satz_batz`（`SB`）/ `#proc_wasshoi`（`WS{x}`）/
//!   `#proc_wasshoi_entry`（`WSE`）/ `#proc_nrs`（`NRS`）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("NinjaSlayer2.…")` で `i18n/NinjaSlayer2/ja_jp.yml` から表を作る。
//! Rust側は同じ値を `static` として直接持つ（値は1文字も変えていない）。
//!
//! # バラバラロールの取り込み
//!
//! Ruby は `BCDice::CommonCommand::BarabaraDice.eval("{n}B6>={d}", …)` を呼んで
//! `last_dice_list` と `success_num` を取り出すが、Rust側の
//! [`crate::common_command::barabara_dice`] は整形済みテキストしか返さないので、
//! 同じ計算（`roll_barabara(n, 6)` と `>=` の成功数）をここに書いている。
//! `NinjaSlayer2` は `sort_barabara_dice` が既定（false）なので出目は振った順のまま。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// i18n `NinjaSlayer2.table.SATSUBATSU.items`（`i18n/NinjaSlayer2/ja_jp.yml`）。
static SATSUBATSU_ITEMS: &[&str] = &[
    "「イヤーッ！」腹部に強烈な一撃が命中！ 敵はくの字に折れ曲がり、ワイヤーアクションめいて吹っ飛んだ！ \n『痛打+1』。敵の【体力】を減らした場合、付属効果として『弾き飛ばし』を与える。",
    "「観念してハイクを詠め！」頭部への痛烈なカラテが命中！ 眼球破壊もしくは激しい脳震盪が敵を襲う！ \n『痛打+1』。敵の【体力】を減らした場合、付属効果として『ニューロンダメージ2』と『ワザマエダメージ1』と『●部位損傷：頭部』を与える。",
    "「苦しみ抜いて死ぬがいい！」急所や内臓を情け容赦なく破壊！ \n『痛打+1』。敵の【体力】を減らした場合、付属効果として『ニューロンダメージ1』と『精神力ダメージ2』と『●部位損傷：胴体』を与える。",
    "「どこへ逃げても無駄だ！」敵の脚を無慈悲に粉砕！ \n『痛打+1』。敵の【体力】を減らした場合、付属効果として『カラテダメージ1』と『脚力ダメージ2』と『●部位損傷：脚部』を与える。",
    "「これで手も足も出まい！」敵の両腕をダブルチョップ切断！ 傷口から鮮血がスプリンクラーめいて噴き出す！ \n『痛打+1』。敵の【体力】を減らした場合、付属効果として『カラテダメージ2』と『ワザマエダメージ2』と『●部位損傷：腕部』を与える。",
    "「さらばだ！ イイイヤアアアアーーーーッ！」ヤリめいたチョップが敵の胸を貫通！ さらに心臓を掴み取り、握りつぶした！ ゴウランガ！ \n『即死！』。敵が『即死耐性』を持つ場合、この効果は『痛打＋2D6』に置き換えられる。",
];
/// i18n `NinjaSlayer2.table.SATSUBATSU`（`DiceTable::Table.from_i18n`）。
static SATSUBATSU_TABLE: Table = Table::from_dice("サツバツ!!(D6) ＞ ", 1, 6, SATSUBATSU_ITEMS);

/// i18n `NinjaSlayer2.table.WASSHOI.items`（`i18n/NinjaSlayer2/ja_jp.yml`）。
static WASSHOI_ITEMS: &[&str] = &[
    "高所からの回転着地！ タタミ四枚の距離で睨み合った！ \n標的ニンジャから3または4マス離れた任意のマスに、【殺】コマを置くこと。",
    "ドアを蹴破って出現！ \n標的ニンジャがいる部屋の任意のドアの隣に【殺】コマを置くこと。 \nなお、ドアにあらかじめ鍵をかけるなどの行為は全て無駄である。",
    "KRAAAAASH！ 窓を突き破り出現！ \n標的ニンジャがいる部屋の任意の窓の隣に【殺】コマを置くこと。 \n【殺】コマが隣接している間、その窓は脱出用として使用できなくなる。",
    "天井破壊や床破砕、または垂直リフト射出により出現！ \n標的ニンジャから2マス離れた任意の場所に【殺】コマを置くこと。 \n激しい恐怖や動揺により、次のターンの終了時まで、その場にいる【DKK】1以上のニンジャ全員は『連続側転判定』の難易度が+1される。",
    "冷蔵庫や金庫から突如出現！ \nそのマップ上に存在するトレジャーボックス内に、ニンジャスレイヤーが潜んでいた。標的ニンジャから最も近いトレジャーボックス1個（もしくは適切な障害物や爆発物）の隣に【殺】コマを置くこと。 \n激しい恐怖や動揺により、次のターンの終了時まで、その場にいる【DKK】1以上のニンジャ全員は『連続側転判定』の難易度が+1される。",
    "「行き先はジゴクですよ」 \nマップ上にいるNPC1人（標的ニンジャから最も近くにいる者）が、実はニンジャスレイヤーの変装であった。そのNPCのコマを【殺】に変更せよ（本物のNPCがどこにいったのかはニンジャマスターが後で考える）。 \n激しい恐怖や動揺により、次のターンの終了時まで、その場にいる【DKK】1以上のニンジャ全員は『連続側転判定』の難易度が+2される。",
];
/// i18n `NinjaSlayer2.table.WASSHOI`（`DiceTable::Table.from_i18n`）。
static WASSHOI_TABLE: Table = Table::from_dice(
    "ニンジャスレイヤー=サンのエントリー!!(D6) ＞ ",
    1,
    6,
    WASSHOI_ITEMS,
);

/// i18n `NinjaSlayer2.table.NRS.items`（`i18n/NinjaSlayer2/ja_jp.yml`）。
static NRS_ITEMS: &[&str] = &[
    "絶叫 \n恐怖のあまり、その場で絶叫を上げる。 \n探索中の場合、近くの敵に存在を気づかれる可能性がある（シナリオ次第）。 \n戦闘中の場合、恐怖のあまり足がガクガクと震え、このターン終了まで【脚力】が0となり、『崩れ状態』とみなされる。",
    "失禁 \n恐怖とトラウマのあまりその場で立ちすくみ、失禁する。自尊心を失い【精神力】が-1される。 \n戦闘中の場合、足がすくんで手がブルブルと震え、このターン終了まであらゆる自発的行動の難易度が＋１され、【脚力】が１となり、『崩れ状態』とみなされる。",
    "パニック逃走や異常行動 \n恐怖のあまり絶叫し、仲間を見捨ててその場から全速力で逃げ出そうとしたり、銃を持っている場合は見えない敵に向かって乱射しようとする。 \nあるいは仲間のことをニンジャだと思い込んで攻撃を仕掛けたり、理解不能な異常行動を取ったりする。 \n探索シーケンスの場合、直ちに戦闘シーケンスが発生してこのPCの手番となり、1ターンだけマスターがこのPCを操作する。このPCは常軌を逸した身体能力を発揮し、【脚力】6として行動する。マスターはこのPCに自傷行為以外のどんな行動を取らせてもよい。このPCにとっては、あらゆるキャラが敵とみなされる。このターンの終了時に、このPCは正気に戻る。 \n戦闘中の場合も同様で、次の手番にこの状態となる。手番開始時に攻撃できる相手がニンジャしか見えていない場合、逃走を優先する。",
    "ドゲザ \n失禁し【精神力】に1ダメージ。突然ドゲザによる命乞いを行うため、このターン終了時まで『麻痺状態』とみなされる。",
    "激しい嘔吐や鼻血 \n【体力】と【精神力】にそれぞれ１ダメージを受ける。 \n戦闘中の場合、このターンのあらゆる判定難易度が+1される。 \nしかし目の前の脅威に立ち向かおうとする意志は砕けていない。",
    "気絶 \n失禁し【精神力】に1ダメージ。さらに打ち上げられたマグロめいてその場で倒れて口をパクパクとさせ、『気絶状態』となる。 \n戦闘が終了するか、一定時間が経過するか、誰かに蘇生してもらうまで、この状態は解除されない。",
    "心臓発作や狂死 \n急激なニンジャリアリティショックに耐えきれず、PCは心臓発作やニューロン損傷を起こして【体力】0となり、その場に倒れ『気絶状態』となる。 \nZBRアドレナリンでなければ蘇生できない。処置を受けずに時間が経過すると死亡する。",
];

// ---------------------------------------------------------------------------
// 正規表現（Ruby の定数と1対1）
// ---------------------------------------------------------------------------

macro_rules! re {
    ($name:ident, $pattern:literal) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).unwrap())
        }
    };
}

re!(re_count_satz_batz, r"(?i)S([1-6]|[1-6]+(?:/[1-6]+)?\+)?$");
re!(re_count_critical, r"(?i)C([1-6]|[1-6]+\+)?$");
re!(re_count_judge, r"(=|!=|>=|>|<=|<)([1-6]+\+|[1-6])");
re!(
    re_judge_diceroll,
    r"(?i)^((?:(?:UH|[KENHU])?\d+,?)+)(?:\[((?:(?:S([1-6]|[1-6]+(?:/[1-6]+)?\+)?|C([1-6]*\+?)?|(=|!=|>=|>|<=|<)([1-6]+\+?))(?:\]\[)?)+)\])?$"
);
re!(re_judge_satz_batz, r"(?i)^SB(?:@([1-6]))?$");
re!(re_judge_wasshoi, r"(?i)^WS([1-9]|10|11|12)$");
re!(re_judge_wasshoi_entry, r"(?i)^WSE(?:@([1-6]))?$");
re!(
    re_judge_nrs,
    r"(?i)^NRS(?:_(E|N|H|U|UH)(\d+))?(?:@([1-7]))?$"
);

// Ruby `proc_dice_2nd` 内の `/^(UH|[KENHU])?(\d+)$/i`。
re!(re_sub_command, r"(?i)^(UH|[KENHU])?(\d+)$");

/// Ruby `DIFFICULTY_SYMBOL_TO_INTEGER`。難易度の文字表現から整数値への対応。
fn difficulty_symbol_to_integer(symbol: &str) -> Option<i64> {
    match symbol {
        "K" => Some(2),
        "E" => Some(3),
        "N" => Some(4),
        "H" => Some(5),
        "U" => Some(6),
        "UH" => Some(6),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 補助
// ---------------------------------------------------------------------------

/// Ruby `#s_to_i(string, default)`。
fn s_to_i(string: Option<&str>, default: i64) -> i64 {
    match string {
        None => default,
        Some("") => default,
        // Ruby `String#to_i` は先頭の数字だけを読む。ここに来る文字列は
        // `[1-6]` 由来なので単純にパースできる。
        Some(s) => s.parse::<i64>().unwrap_or(0),
    }
}

/// Ruby `Array#[]` の負index（末尾から数える）を再現する。範囲外は空文字列。
fn ruby_index(items: &[&'static str], index: i64) -> &'static str {
    let len = items.len() as i64;
    let i = if index < 0 { index + len } else { index };
    if i < 0 || i >= len {
        ""
    } else {
        items[i as usize]
    }
}

/// Ruby `String#split(sep)`（末尾の空要素を落とす）。
fn ruby_split<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Ruby `#check_difficulty`。条件を満たした出目の配列を返す。
fn check_difficulty(dice: &[i64], difficulty: i64, cmp_op: CmpOp) -> Vec<i64> {
    dice.iter()
        .copied()
        .filter(|d| cmp_op.apply(&I::from(*d), &I::from(difficulty)))
        .collect()
}

/// Ruby `#check_difficulty_new`。難易度文字列の全条件を突破したか。
fn check_difficulty_new(dice: &[i64], difficulty: Option<&str>, cmp_op: CmpOp) -> bool {
    let Some(difficulty) = difficulty else {
        return false;
    };
    if dice.is_empty() {
        return false;
    }

    let mut sorted_dice = dice.to_vec();
    sorted_dice.sort_unstable();
    let mut sorted_difficulty: Vec<char> = difficulty.chars().collect();
    sorted_difficulty.sort_unstable();

    let mut check_index = 0usize;
    for dice_value in sorted_dice {
        // Ruby: sorted_difficulty[check_index].to_i（範囲外は nil.to_i == 0）
        let target = sorted_difficulty
            .get(check_index)
            .and_then(|c| c.to_digit(10))
            .map_or(0, i64::from);
        if cmp_op.apply(&I::from(dice_value), &I::from(target)) {
            check_index += 1;
        }
    }

    check_index >= sorted_difficulty.len()
}

/// `diff_result_array.sort.reverse.join(',')` を `[...]` で包んだもの。空なら空文字列。
fn descending_bracket(dice: &[i64]) -> String {
    if dice.is_empty() {
        return String::new();
    }
    let mut sorted = dice.to_vec();
    sorted.sort_unstable();
    sorted.reverse();
    format!("[{}]", dice_text::join_dice(&sorted))
}

/// Ruby `#proc_appendix(roll_result, ap_command)`。
fn proc_appendix(dice: &[i64], ap_command: &str) -> String {
    let mut output_text = String::new();

    if let Some(caps) = re_count_satz_batz().captures(ap_command) {
        let diff_condition = caps.get(1).map(|m| m.as_str());
        if ap_command.ends_with('+') {
            // サツバツ発生チェック
            let stripped = diff_condition.unwrap_or("").replacen('+', "", 1);
            let parts = ruby_split(&stripped, "/");
            let sb_condition = parts.first().copied();
            let nm_condition = parts.get(1).copied();

            if check_difficulty_new(dice, nm_condition, CmpOp::Ge) {
                output_text += &format!(", ナムアミダブツ！[{ap_command}]");
            } else if check_difficulty_new(dice, sb_condition, CmpOp::Ge) {
                output_text += &format!(", サツバツ！[{ap_command}]");
            }
        } else {
            // サツバツ数カウント
            let diff_result = check_difficulty(dice, s_to_i(diff_condition, 6), CmpOp::Ge);
            output_text += &format!(
                ", サツバツ判定[{ap_command}]:{}{}",
                diff_result.len(),
                descending_bracket(&diff_result)
            );
        }
    } else if let Some(caps) = re_count_critical().captures(ap_command) {
        let diff_condition = caps.get(1).map(|m| m.as_str());
        if ap_command.ends_with('+') {
            // クリティカル発生チェック
            let stripped = diff_condition.unwrap_or("").replacen('+', "", 1);
            if check_difficulty_new(dice, Some(stripped.as_str()), CmpOp::Ge) {
                output_text += &format!(", クリティカル！[{ap_command}]");
            }
        } else {
            // クリティカル数カウント
            let diff_result = check_difficulty(dice, s_to_i(diff_condition, 6), CmpOp::Ge);
            output_text += &format!(
                ", クリティカル判定[{ap_command}]:{}{}",
                diff_result.len(),
                descending_bracket(&diff_result)
            );
        }
    } else if let Some(caps) = re_count_judge().captures(ap_command) {
        let diff_type = &caps[1];
        let diff_condition = caps.get(2).map(|m| m.as_str());
        // Ruby: Normalize.comparison_operator は既知の演算子しか来ないので必ず Some
        let Some(cmp_op) = normalize::comparison_operator(diff_type) else {
            return output_text;
        };

        if ap_command.ends_with('+') {
            // 追加判定チェック
            let stripped = diff_condition.unwrap_or("").replacen('+', "", 1);
            if check_difficulty_new(dice, Some(stripped.as_str()), cmp_op) {
                output_text += &format!(", 追加判定成功！[{ap_command}]");
            }
        } else {
            // 追加判定カウント
            let diff_result = check_difficulty(dice, s_to_i(diff_condition, 6), cmp_op);
            output_text += &format!(
                ", 追加判定[{ap_command}]:{}{}",
                diff_result.len(),
                descending_bracket(&diff_result)
            );
        }
    }

    output_text
}

/// Ruby `#proc_dice_2nd(match)`。判定結果テキストと達成値の合計を返す。
///
/// Ruby側は書式に合わない部分コマンドで `NoMethodError` を起こし、呼び出し元の
/// `rescue StandardError` が `nil` を返す（例: `1K2`）。ここでは `Ok(None)` で表す。
fn proc_dice_2nd(
    caps: &Captures,
    rng: &mut Randomizer,
) -> Result<Option<(String, i64)>, EvalError> {
    let mut output_text = String::new();
    let mut total_success_num = 0i64;

    let command = &caps[1];
    let appendix = caps.get(2).map(|m| m.as_str());
    let mut difficulty = 0i64;

    for sub_command in ruby_split(command, ",") {
        let Some(sub) = re_sub_command().captures(sub_command) else {
            return Ok(None);
        };
        if let Some(symbol) = sub.get(1) {
            let Some(value) = difficulty_symbol_to_integer(symbol.as_str()) else {
                return Ok(None);
            };
            difficulty = value;
        }
        let dice_num: i64 = sub[2].parse().unwrap_or(0);

        // Ruby: BCDice::CommonCommand::BarabaraDice.eval("#{dice_num}B6>=#{difficulty}", …)
        let roll_command = format!("{dice_num}B6>={difficulty}");
        let dice_list = rng.roll_barabara(dice_num, 6)?;
        let success_num = dice_list.iter().filter(|d| **d >= difficulty).count() as i64;

        output_text += &format!(
            "({roll_command}) ＞ {} ＞ 成功数:{success_num}",
            dice_text::join_dice(&dice_list)
        );

        if let Some(appendix) = appendix {
            for ap_command in ruby_split(appendix, "][") {
                output_text += &proc_appendix(&dice_list, ap_command);
            }
        }
        output_text += " \n";

        total_success_num += success_num;
    }

    Ok(Some((output_text, total_success_num)))
}

/// Ruby `#proc_satz_batz(type)`。
fn proc_satz_batz(r#type: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    if r#type > 0 {
        Ok(format!(
            "サツバツ!!({type}) ＞ {}",
            ruby_index(SATSUBATSU_ITEMS, r#type - 1),
            type = r#type
        ))
    } else {
        Ok(SATSUBATSU_TABLE.roll(rng)?.to_string())
    }
}

/// Ruby `#proc_wasshoi(dkk)`。
///
/// 成否のフラグが表示テキストと逆（`sum > dkk` で `Result.success` かつ「判定失敗」）
/// なのは Ruby 原典どおり。
fn proc_wasshoi(dkk: i64, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let dice_array = rng.roll_barabara(2, 6)?;
    let sum: i64 = dice_array.iter().sum();
    let mut output_text = format!(
        "Wasshoi!判定(2D6) ＞ ({}) ＞ {sum}",
        dice_array
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("+")
    );

    if sum > dkk {
        output_text += &format!("(>{dkk}) 判定失敗");
        Ok(EvalResult::success(output_text))
    } else {
        output_text += &format!("(<={dkk}) 判定成功!! \nニンジャスレイヤー=サンのエントリーだ!!");
        Ok(EvalResult::failure(output_text))
    }
}

/// Ruby `#proc_wasshoi_entry(type)`。
fn proc_wasshoi_entry(r#type: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    if r#type > 0 {
        Ok(format!(
            "ニンジャスレイヤー=サンのエントリー!!({type}) ＞ {}",
            ruby_index(WASSHOI_ITEMS, r#type - 1),
            type = r#type
        ))
    } else {
        Ok(WASSHOI_TABLE.roll(rng)?.to_string())
    }
}

/// Ruby `#proc_nrs(dice_num, dificulty_s, type)`。
fn proc_nrs(
    dice_num: i64,
    dificulty_s: Option<&str>,
    r#type: i64,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    // 難易度も乱数表の番号も指定が無ければコマンドミス
    let dificulty_i = match dificulty_s {
        Some(s) => difficulty_symbol_to_integer(s).unwrap_or(0),
        None => 0,
    };
    if dificulty_i == 0 && r#type == 0 {
        return Ok(None);
    }

    let mut output_text = String::new();
    if dificulty_i > 0 {
        let roll_command = format!("{dice_num}B6>={dificulty_i}");
        let dice_list = rng.roll_barabara(dice_num, 6)?;
        let success_num = dice_list.iter().filter(|d| **d >= dificulty_i).count() as i64;
        output_text += &format!(
            "NRS判定({roll_command}) ＞ {} ＞ 成功数:{success_num}",
            dice_text::join_dice(&dice_list)
        );
        if success_num > 0 {
            output_text += " NRS克服!!";
            return Ok(Some(EvalResult::success(output_text)));
        }
        output_text += " NRS発症!! \n";
    }

    // NRS発狂表の決定
    let mut dice_face = 0i64;
    let mut additional = 0i64;
    let mut r#type = r#type;
    if r#type == 0 {
        match dificulty_s {
            Some("E") => dice_face = 3,
            Some("N") => dice_face = 6,
            Some("H") | Some("U") => {
                dice_face = 6;
                additional = 1;
            }
            _ => {}
        }
        r#type = rng.roll_once(dice_face)? + additional;
    }
    let roll_command = format!(
        "1D{dice_face}{}",
        if additional > 0 {
            format!("+{additional}")
        } else {
            String::new()
        }
    );
    output_text += &format!(
        "NRS発狂{}({type}) ＞ {}",
        if dice_face > 0 {
            format!("({roll_command}) ＞ ")
        } else {
            String::new()
        },
        ruby_index(NRS_ITEMS, r#type - 1),
        type = r#type
    );

    Ok(Some(EvalResult::failure(output_text)))
}

/// Ruby `#proc_text(command)`。テキスト系処理。
fn proc_text(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(caps) = re_judge_satz_batz().captures(command) {
        let r#type = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        return Ok(Some(SpecificCommandOutput::text(proc_satz_batz(
            r#type, rng,
        )?)));
    }
    if let Some(caps) = re_judge_wasshoi().captures(command) {
        let dkk = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        return Ok(Some(SpecificCommandOutput::result(proc_wasshoi(dkk, rng)?)));
    }
    if let Some(caps) = re_judge_wasshoi_entry().captures(command) {
        let r#type = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        return Ok(Some(SpecificCommandOutput::text(proc_wasshoi_entry(
            r#type, rng,
        )?)));
    }
    if let Some(caps) = re_judge_nrs().captures(command) {
        let dice_num = caps
            .get(2)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let dificulty_s = caps.get(1).map(|m| m.as_str());
        let r#type = caps
            .get(3)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        return Ok(proc_nrs(dice_num, dificulty_s, r#type, rng)?.map(SpecificCommandOutput::result));
    }

    Ok(None)
}

/// Ruby `BCDice::GameSystem::NinjaSlayer2`（ID: `NinjaSlayer2`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NinjaSlayer2;

impl GameSystem for NinjaSlayer2 {
    fn id(&self) -> &'static str {
        "NinjaSlayer2"
    }

    fn name(&self) -> &'static str {
        "ニンジャスレイヤーTRPG 2版"
    }

    fn sort_key(&self) -> &'static str {
        "にんしやすれいやあTRPG2"
    }

    fn help_message(&self) -> &'static str {
        r"--- 成功判定コマンド ---
通常のダイスの「{ダイス個数}B6>={難易度ごとの目標出目}」を実行するための簡易入力コマンドです。

- K{x}
難易度K([K]ids/目標値=2)の成功判定をダイス{x}個で実行します。
先頭の文字を変えることで、難易度E([E]asy/目標値=3),N([N]ormal/目標値=4),H([H]ard/目標値=5),U([U]ltra-hard/目標値=6)もしくはUH([U]ltra-[H]ard/目標値=6)でも実行可能です。

- K{x1},N{x2},...,U{xn}
K{x}の複数ロール版。
カンマ(,)で区切って複数回入力すると、区切られたセットごとに成功判定を行います。
2回目以降は難易度指定を省略可能で、省略した場合はひとつ前の難易度を引き継いで判定を行います。
以下のコマンドについても同様の書式で複数ロールしての同時判定が可能です。

--- 追加判定コマンド ---
以下のコマンド群は、成功判定コマンドの後ろに付けて実行してください。

- [>={y}]
- [>{y}]
- [<={y}]
- [<{y}]
- [={y}]
- [!={y}]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[]内で指定された条件で追加判定を行います。
それぞれ、{y}以上(>=)、{y}より大きい(>)、{y}以下(<=)、{y}未満(<)、{y}のみ(=)、{y}以外(!=)を判定し、
ロール結果の中で条件を満たしたダイスの個数を「追加判定」というテキストと共に出力します。
[=5][=6]のように複数記述することで、ひとつのロールに対して複数パターンでの追加判定が可能です。
※ 条件は一括でしか指定できないため、ロールごとに異なる条件を指定したい場合はコマンドを分けてください。以下も同様です。。

- [>={y1}{y2}...{yn}+]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[>={y1}]～[>={yn}]の各条件で追加判定を行います。
出目の中に条件を満たしたダイスが**全て**含まれていた場合、「追加判定成功！」というテキストを出力します。
例えば[>=665+]とした場合、出目の中に6以上のダイスが2つと5以上のダイスが1つ含まれていれば成功扱いになります。

- [S{y}]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[>={y}]と同等の追加判定を行います。
条件を満たしたダイスの個数を「サツバツ判定」というテキストと共に出力します。
※ {y}は省略可能、省略した場合は固定値6で処理します。

- [S{y1}{y2}...{yn}+]
- [S{y1}{y2}...{yn}/{z1}{z2}...{zn}+]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[>={y1}{y2}...{yn}+]と[>={z1}{z2}...{zn}+]と同等の追加判定を行います。
{z1}～{zn}で条件を満たした場合、「ナムアミダブツ！」というテキストを出力します。
{z1}～{zn}の条件を満たせず、{y1}～{yn}で条件を満たした場合、「サツバツ！」というテキストを出力します。
※ {z1}～{zn}を省略した場合は、「サツバツ！」の判定のみを行ないます。

- [C{y}]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[>={y}]と同等の追加判定を行います。
条件を満たしたダイスの個数を「クリティカル判定」というテキストと共に出力します。
※ {y}は省略可能、省略した場合は固定値6で処理します。

- [C{y1}{y2}...{yx}+]
成功判定コマンドでカンマ区切りで指定した各ロール結果に対して、[>={y1}{y2}...{yn}+]と同等の追加判定を行います。
条件を条件を満たした場合、「クリティカル！」というテキストを出力します。

--- 定型文コマンド ---
以下のコマンド群はそれぞれ単体で使用してください。

- SB or SB@{x}
{x}(1-6/省略時はd6)に対応したサツバツ([S]atz-[B]atz)・クリティカル表の内容を返します。

- WS{x}
{x}(1-12/省略不可)に対応する[W]as[s]hoi!判定(2d6<={x})を行います

- WSE or WSE@{x}
{x}(1-6/省略時はd6)に対応する死神のエントリー決定表([W]as[s]hoi! [E]ntry)の内容を返します。

- NRS_E{x} or NRS_E{x}@{y} or NRS@{y}
ダイス{x}個で難易度[E]asy(>=3)のNRS判定({x}省略時はスキップ)を行い、失敗した場合は{y}(1～7/省略時は難易度に応じたダイス目)に対応するNRS発狂表を返します。
「_E」部分を変更することで、難易度N,H,U,UHでも利用可能です。(Kはありません)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["UH", "[KENHU]", "SB", "WS", "WSE", "NRS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(caps) = re_judge_diceroll().captures(command) {
            // 2版用のダイス判定
            let Some((text, total_success_num)) = proc_dice_2nd(&caps, rng)? else {
                return Ok(None);
            };

            let result = if total_success_num > 0 {
                EvalResult::success(text)
            } else {
                EvalResult::failure(text)
            };
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        // ダイスでなければ定型文処理
        proc_text(command, rng)
    }
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
            .join("test/data/NinjaSlayer2.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NinjaSlayer2.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/NinjaSlayer2.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NinjaSlayer2.toml must parse");
        assert_eq!(
            data.tests.len(),
            33,
            "case count in test/data/NinjaSlayer2.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NinjaSlayer2",
                "unexpected game system in NinjaSlayer2.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("NinjaSlayer2"), &tc.input, &mut src) {
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
                    "FAIL NinjaSlayer2:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NinjaSlayer2 cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
