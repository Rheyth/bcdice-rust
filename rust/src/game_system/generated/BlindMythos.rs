//! `lib/bcdice/game_system/BlindMythos.rb` の移植。

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlindMythos;

impl GameSystem for BlindMythos {
    fn id(&self) -> &'static str {
        "BlindMythos"
    }
    fn name(&self) -> &'static str {
        "ブラインド・ミトスRPG"
    }
    fn sort_key(&self) -> &'static str {
        "ふらいんとみとすRPG"
    }
    fn help_message(&self) -> &'static str {
        r"・判定：BMx@y>=z、BMSx@y>=z
  　x:スキルレベル
　　y:目標難易度（省略可。デフォルト4）
　　z:必要成功度
　BMコマンドはダイスの振り足しを常に行い、
　BMSは振り足しを自動では行いません。
 例）BM>=1　BM@3>=1　BMS2>=1

・判定振り足し：ReRollx,x,x...@y>=z
  　x:振るダイスの個数
　　y:目標難易度（省略可。デフォルト4）
　　z:必要成功度
　振り足しを自動で行わない場合（BMSコマンド）に使用します。

・LE：失う感情表
・感情後遺症表 ESx
　ESH：喜、ESA：怒、ESS：哀、ESP：楽、ESL：愛、ESE：感
・DT：汚染チャート
・RPxyz：守護星表チェック
 xyz:守護星ナンバーを指定
 例）RP123　RP258
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "BM", "ReRoll", "RP", "DT", "LE", "ESH", "ESA", "ESS", "ESP", "ESL", "ESE",
        ]
    }
    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = judge_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = reroll(command, true, rng)? {
            return Ok(Some(SpecificCommandOutput::text(result.text)));
        }
        if let Some(result) = ruling_planet(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = dirty_table(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(roll_table(command, rng)?.map(SpecificCommandOutput::text))
    }
}

struct RollData {
    text: String,
    bits: Vec<i64>,
    successes: Vec<i64>,
    ones: Vec<i64>,
    can_reroll: bool,
}

fn judge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^BM(S)?(\d*)(@(\d+))?>=(\d+)$").unwrap())
}
fn reroll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^ReRoll([\d,]+)(@(\d+))?>=(\d+)$").unwrap())
}
fn rp_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^RP(\d+)$").unwrap())
}

fn judge_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = judge_pattern().captures(command) else {
        return Ok(None);
    };
    let stop = m.get(1).is_some();
    let skill = m[2].parse::<i64>().unwrap_or(0);
    let judge_text = m.get(3).map_or("", |v| v.as_str());
    let judge = m.get(4).map_or(4, |v| v.as_str().parse().unwrap_or(4));
    let target = m[5].parse::<i64>().unwrap_or(1);
    let data = roll_result(&[skill + 2], judge_text, judge, target, false, stop, rng)?;
    let mut result = total_result(
        &data.bits,
        &data.successes,
        &data.ones,
        target,
        stop,
        data.can_reroll,
    );
    result.text.insert_str(0, &data.text);
    Ok(Some(result))
}

struct RerollData {
    text: String,
    successes: Vec<i64>,
    ones: Vec<i64>,
}

fn reroll(
    command: &str,
    stop: bool,
    rng: &mut Randomizer,
) -> Result<Option<RerollData>, EvalError> {
    let Some(m) = reroll_pattern().captures(command) else {
        return Ok(None);
    };
    let counts = m[1]
        .split(',')
        .map(|x| x.parse::<i64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let judge_text = m.get(2).map_or("", |v| v.as_str());
    let judge = m.get(3).map_or(4, |v| v.as_str().parse().unwrap_or(4));
    let target = m[4].parse::<i64>().unwrap_or(0);
    let commands = counts
        .iter()
        .map(|count| format!("ReRoll{count}{judge_text}>={target}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut text = if counts.len() > 1 && stop {
        format!("({commands})")
    } else {
        String::new()
    };
    text.push('\n');
    let data = roll_result(&counts, judge_text, judge, target, true, stop, rng)?;
    text.push_str(&data.text);
    Ok(Some(RerollData {
        text,
        successes: data.successes,
        ones: data.ones,
    }))
}

fn roll_result(
    counts: &[i64],
    judge_text: &str,
    judge: i64,
    target: i64,
    is_reroll: bool,
    stop: bool,
    rng: &mut Randomizer,
) -> Result<RollData, EvalError> {
    let mut text = String::new();
    let mut bits = Vec::new();
    let mut successes = Vec::new();
    let mut ones = Vec::new();
    let mut reroll_targets = Vec::new();

    for (index, &count) in counts.iter().enumerate() {
        if index > 0 {
            text.push('\n')
        }
        let command_name = if is_reroll {
            format!("ReRoll{count}")
        } else if stop {
            format!("BMS{}", count - 2)
        } else {
            format!("BM{}", count - 2)
        };
        let command = format!("{command_name}{judge_text}>={target}");
        let mut dice = rng.roll_barabara(count, 6)?;
        dice.sort_unstable();
        if is_reroll {
            text.push_str(" ＞ ")
        }
        text.push_str(&format!("({command}) ＞ {count}D6[{}] ＞ ", join(&dice)));
        let success = dice.iter().filter(|&&die| die >= judge).count() as i64;
        let one = dice.iter().filter(|&&die| die == 1).count() as i64;
        if !is_reroll {
            bits.extend(dice.iter().copied().filter(|&die| die >= 4))
        }
        successes.push(success);
        ones.push(one);
        text.push_str(&format!("成功数:{success}"));

        let groups = same_dice(&dice);
        if !groups.is_empty() {
            text.push_str(&format!(
                "、リロール[{}]",
                groups
                    .iter()
                    .map(|g| g.iter().map(i64::to_string).collect::<String>())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            reroll_targets.push(
                groups
                    .iter()
                    .map(Vec::len)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
    }

    let reroll_command = if reroll_targets.is_empty() {
        String::new()
    } else {
        format!("ReRoll{}{judge_text}>={target}", reroll_targets.join(","))
    };
    if !reroll_command.is_empty() && stop {
        text.push_str(&format!("\n ＞ コマンド：{reroll_command}"))
    }
    let can_reroll = !reroll_command.is_empty();
    if can_reroll && !stop {
        if let Some(more) = reroll(&reroll_command, false, rng)? {
            text.push_str(&more.text);
            successes.extend(more.successes);
            ones.extend(more.ones);
        }
    }
    Ok(RollData {
        text,
        bits,
        successes,
        ones,
        can_reroll,
    })
}

fn same_dice(dice: &[i64]) -> Vec<Vec<i64>> {
    let mut groups = Vec::new();
    for value in 2..=6 {
        let group = dice
            .iter()
            .copied()
            .filter(|&die| die == value)
            .collect::<Vec<_>>();
        if group.len() > 1 {
            groups.push(group)
        }
    }
    groups
}

fn total_result(
    bits: &[i64],
    successes: &[i64],
    ones: &[i64],
    target: i64,
    stop: bool,
    can_reroll: bool,
) -> EvalResult {
    let success = successes.iter().sum::<i64>();
    let one = ones.iter().sum::<i64>();
    let mut text = if successes.len() > 1 {
        format!("\n ＞ 最終成功数:{success}")
    } else {
        String::new()
    };
    if can_reroll && stop {
        text.push('\n');
        if success >= target {
            text.push_str(" ＞ 現状で成功。コマンド実行で追加リロールも可能");
            return EvalResult::success(text);
        }
        text.push_str(" ＞ 現状のままでは失敗");
        if one > 0 {
            text.push_str(&format!("。汚染ポイント+{one}"));
            return EvalResult::fumble(text);
        }
        return EvalResult::failure(text);
    }
    if success >= target {
        text.push_str(" ＞ 成功");
        if !bits.is_empty() {
            text.push_str(&format!("、禁書ビット発生[{}]", join(bits)));
            EvalResult::critical(text)
        } else {
            EvalResult::success(text)
        }
    } else {
        text.push_str(" ＞ 失敗");
        if one > 0 {
            text.push_str(&format!("。汚染ポイント+{one}"));
            EvalResult::fumble(text)
        } else {
            EvalResult::failure(text)
        }
    }
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn ruling_planet(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = rp_pattern().captures(command) else {
        return Ok(None);
    };
    let targets = m[1]
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(i64::from)
        .collect::<Vec<_>>();
    let first = rng.roll_once(10)?;
    let mut second = rng.roll_once(10)?;
    while first == second {
        second = rng.roll_once(10)?
    }
    let dice = [
        if first == 10 { 0 } else { first },
        if second == 10 { 0 } else { second },
    ];
    let condition = dice.iter().any(|die| targets.contains(die));
    let mut result = EvalResult::with_text(format!(
        "守護星表チェック({}) ＞ 2D10[{}] ＞ {}",
        join(&targets),
        join(&dice),
        if condition { "発動" } else { "失敗" }
    ));
    result.set_condition(condition);
    Ok(Some(result))
}

static DIRTY: &[&str] = &[
    "汚染チャートを２回振り、その効果を適用する（1・2-2,5・6-12 なら振り直す）",
    "ＰＣ全員の「トラウマ」「喪失」すべてに２ダメージ",
    "ＰＣ全員の「喪失」２つに４ダメージ",
    "ＰＣ全員の「トラウマ」すべてに２ダメージ。その後さらに汚染が２増える",
    "ＰＣ全員、１つの【記憶】の両方の値が０になる。このときアクロバットダイス獲得不可",
    "ＰＣ全員の「喪失」１つに４ダメージ。このときアクロバットダイス獲得不可",
    "ＰＣ全員の「トラウマ」すべてに１ダメージ。その後さらに汚染が３増える",
    "ＰＣ全員の「トラウマ」すべてに１ダメージ。その後アクロバットダイスをＰＣ人数分失う",
    "ＰＣ全員の「喪失」すべてに２ダメージ。禁書ビットをすべて失う",
    "ＰＣ全員の「トラウマ」２つに３ダメージ。その後さらに汚染が１増える",
    "ＰＣ全員の「トラウマ」「喪失」すべてに１ダメージ",
    "ＰＣ全員の「喪失」１つに４ダメージ。禁書ビットをすべて失う",
    "ＰＣ全員の「トラウマ」すべてに２ダメージ",
    "ＰＣ全員の１つの【記憶】の「トラウマ」「喪失」それぞれに３ダメージ",
    "ＰＣ全員の「喪失」すべてに１ダメージ",
    "ＰＣ全員の「トラウマ」３つに２ダメージ",
    "ＰＣ全員の「トラウマ」と「喪失」それぞれ１つに３ダメージ",
    "ＰＣ全員の「喪失」３つに２ダメージ",
    "ＰＣ全員のすべての「トラウマ」に1 ダメージ",
    "ＰＣ全員のひとつの【記憶】の「トラウマ」「喪失」それぞれに３ダメージ",
    "ＰＣ全員の「喪失」すべてに２ダメージ",
    "ＰＣ全員の「トラウマ」ひとつに４ダメージ。禁書ビットをすべて失う",
    "ＰＣ全員の「トラウマ」「喪失」すべてに１ダメージ",
    "ＰＣ全員の「喪失」２つに３ダメージ。その後さらに汚染が１増える",
    "ＰＣ全員の「トラウマ」すべてに２ダメージ。禁書ビットをすべて失う",
    "ＰＣ全員の「喪失」すべてに１ダメージ。その後アクロバットダイスをＰＣ人数分失う",
    "ＰＣ全員の「喪失」すべてに１ダメージ。その後さらに汚染が３増える",
    "ＰＣ全員の「トラウマ」１つに４ダメージ。このときアクロバットダイス獲得不可",
    "ＰＣ全員、１つの【記憶】の両方の値が０になる。このときアクロバットダイス獲得不可",
    "ＰＣ全員の「喪失」すべてに２ダメージ。その後さらに汚染が２増える",
    "ＰＣ全員の「トラウマ」２つに４ダメージ",
    "ＰＣ全員の「トラウマ」「喪失」すべてに２ダメージ",
    "汚染チャートを２回振り、その効果を適用する（1・2-2,5・6-12 なら振り直す）",
];

fn dirty_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if !command.eq_ignore_ascii_case("DT") {
        return Ok(None);
    }
    let die1 = rng.roll_once(6)?;
    let die2 = rng.roll_once(6)? + rng.roll_once(6)?;
    let index = ((die2 - 2) * 3 + (die1 + 1) / 2 - 1) as usize;
    Ok(Some(format!(
        "汚染チャート({die1},{die2}) ＞ {}",
        DIRTY[index]
    )))
}

static LE: &[&str] = &[
    "喜：喜びは消えた。嬉しい気持ちとは、なんだっただろう。",
    "怒：激情は失われ、憎しみもどこかへと消える。",
    "哀：どんなに辛くても、悲しさを感じない。どうやら涙も涸れたらしい。",
    "楽：もはや楽しいことなどない。希望を抱くだけ無駄なのだ。",
    "愛：愛など幻想……無力で儚い、役に立たない世迷い言だ。",
    "感：なにを見ても、感動はない。心は凍てついている。",
];
static ESH: &[&str] = &[
    "日々喜びを求めてしまう。",
    "日々喜びを求めてしまう。",
    "嬉しい時間が長続きしない。",
    "素直に喜びを共有できないことがある。",
    "小さなことで大きく喜びを感じる。",
    "小さなことで大きく喜びを感じる。",
    "影響なし。",
    "影響なし。",
    "「喜」の後遺症をひとつ消してもよい。",
    "「喜」の後遺症をひとつ消してもよい。",
    "「喜」の後遺症をひとつ消してもよい。",
];
static ESA: &[&str] = &[
    "始終不機嫌になる。",
    "始終不機嫌になる。",
    "一度怒ると、なかなか収まらない。",
    "怒りっぽくなる",
    "怒りかたが激しくなる。",
    "怒りかたが激しくなる。",
    "影響なし。",
    "影響なし。",
    "「怒」の後遺症をひとつ消してもよい。",
    "「怒」の後遺症をひとつ消してもよい。",
    "「怒」の後遺症をひとつ消してもよい。",
];
static ESS: &[&str] = &[
    "一度涙が出るとなかなか止まらない。",
    "一度涙が出るとなかなか止まらない。",
    "夜、哀しいことを思い出して目が覚める。",
    "不意に哀しい気持ちになる。",
    "涙もろくなる。",
    "涙もろくなる。",
    "影響なし。",
    "影響なし。",
    "「哀」の後遺症をひとつ消してもよい。",
    "「哀」の後遺症をひとつ消してもよい。",
    "「哀」の後遺症をひとつ消してもよい。",
];
static ESP: &[&str] = &[
    "突然陽気になったり、不意に笑い出してしまう。",
    "突然陽気になったり、不意に笑い出してしまう。",
    "周りが楽しくなさそうだと不安になる。",
    "楽しいことがないと落ち着かない。",
    "些細なことでも笑ってしまう。",
    "些細なことでも笑ってしまう。",
    "影響なし。",
    "影響なし。",
    "「楽」の後遺症をひとつ消してもよい。",
    "「楽」の後遺症をひとつ消してもよい。",
    "「楽」の後遺症をひとつ消してもよい。",
];
static ESL: &[&str] = &[
    "少しでも気になる相手に愛を求めてしまう。",
    "少しでも気になる相手に愛を求めてしまう。",
    "愛する相手（恋人・家族・ペット・空想）から離れたくない。",
    "誰彼構わず優しくしてしまう。",
    "ひとりでいると不安を感じる。",
    "ひとりでいると不安を感じる。",
    "影響なし。",
    "影響なし。",
    "「愛」の後遺症をひとつ消してもよい。",
    "「愛」の後遺症をひとつ消してもよい。",
    "「愛」の後遺症をひとつ消してもよい。",
];
static ESE: &[&str] = &[
    "感動を共有できない相手を不信に思ってしまう。",
    "感動を共有できない相手を不信に思ってしまう。",
    "嬉しくても哀しくてもすぐに涙が出る。",
    "リアクションがオーバーになる。",
    "ちょっとしたことで感動する。",
    "ちょっとしたことで感動する。",
    "影響なし。",
    "影響なし。",
    "「感」の後遺症をひとつ消してもよい。",
    "「感」の後遺症をひとつ消してもよい。",
    "「感」の後遺症をひとつ消してもよい。",
];

fn roll_table(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let (name, dice, table) = match command.to_ascii_uppercase().as_str() {
        "LE" => ("失う感情表", 1, LE),
        "ESH" => ("「喜」の感情後遺症表", 2, ESH),
        "ESA" => ("「怒」の感情後遺症表", 2, ESA),
        "ESS" => ("「哀」の感情後遺症表", 2, ESS),
        "ESP" => ("「楽」の感情後遺症表", 2, ESP),
        "ESL" => ("「愛」の感情後遺症表", 2, ESL),
        "ESE" => ("「感」の感情後遺症表", 2, ESE),
        _ => return Ok(None),
    };
    let total = if dice == 1 {
        rng.roll_once(6)?
    } else {
        rng.roll_once(6)? + rng.roll_once(6)?
    };
    let index = if dice == 1 { total - 1 } else { total - 2 };
    Ok(Some(format!(
        "{name}({total}) ＞ {}",
        table[index as usize]
    )))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "BlindMythos",
            "BlindMythos.toml",
            39,
        );
    }
}
