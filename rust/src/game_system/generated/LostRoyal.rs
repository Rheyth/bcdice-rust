//! P4で手書き移植した `lib/bcdice/game_system/LostRoyal.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `LostRoyal#check_lostroyal`（行為判定 `LR[x,x,x,x,x,x]`、連番チェイン判定）
//! - `#roll_fumble_chart`（`FC`）/ `#roll_wind_power_chart`（`WPC`）/
//!   `#roll_emotion_chart`（`EC`）/ `#roll_hope`（`HRx`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::LostRoyal`（ID: `LostRoyal`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostRoyal;

impl GameSystem for LostRoyal {
    fn id(&self) -> &'static str {
        "LostRoyal"
    }

    fn name(&self) -> &'static str {
        "ロストロイヤル"
    }

    fn sort_key(&self) -> &'static str {
        "ろすとろいやる"
    }

    fn help_message(&self) -> &'static str {
        r"・D66ダイスあり

行為判定
　LR[x,x,x,x,x,x]
　　x の並びには【判定表】の数値を順番に入力する。
　　（例： LR[1,3,0,1,2,3] ）

ファンブル表
　FC

風力決定表
　WPC

感情決定表
　EC

希望点の決定
　HRx
　　x にはダイスの数（ 1 - 2 ）を指定
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"LR\[[0-5],[0-5],[0-5],[0-5],[0-5],[0-5]\]",
            "FC",
            "WPC",
            "EC",
            "HR[1-2]",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `LostRoyal#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `LostRoyal#eval_game_system_specific_command` 本体。
///
/// Ruby の `case command` は各 `when` が**アンカーなし**の正規表現なので、
/// ここでも部分一致（`captures` / `contains`）で判定する。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static LR_RE: OnceLock<Regex> = OnceLock::new();
    static HR_RE: OnceLock<Regex> = OnceLock::new();
    let lr_re = LR_RE.get_or_init(|| {
        Regex::new(r"(?i)LR\[([0-5]),([0-5]),([0-5]),([0-5]),([0-5]),([0-5])\]")
            .expect("valid regex")
    });
    let hr_re = HR_RE.get_or_init(|| Regex::new(r"HR([1-2])").expect("valid regex"));

    if let Some(m) = lr_re.captures(command) {
        // 各キャプチャは1桁の数字なので必ずパースできる
        let checking_table: [i64; 6] = [1, 2, 3, 4, 5, 6].map(|i| m[i].parse::<i64>().unwrap_or(0));
        let text = check_lostroyal(&checking_table, rng)?;
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if command.contains("FC") {
        return Ok(Some(SpecificCommandOutput::text(roll_fumble_chart(rng)?)));
    }
    if command.contains("WPC") {
        return Ok(Some(SpecificCommandOutput::text(roll_wind_power_chart(
            rng,
        )?)));
    }
    if command.contains("EC") {
        return Ok(Some(SpecificCommandOutput::text(roll_emotion_chart(rng)?)));
    }
    if let Some(m) = hr_re.captures(command) {
        let number_of_dice = m[1].parse::<i64>().unwrap_or(0);
        return Ok(Some(SpecificCommandOutput::text(roll_hope(
            number_of_dice,
            rng,
        )?)));
    }

    Ok(None)
}

/// Ruby `LostRoyal#check_lostroyal`。
fn check_lostroyal(checking_table: &[i64; 6], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut keys: Vec<i64> = Vec::with_capacity(3);
    for _ in 0..3 {
        keys.push(rng.roll_once(6)?);
    }

    // Ruby: scores = keys.map { |k| checking_table[k - 1] }
    // （出目は 1〜6 なので範囲外にはならない）
    let scores: Vec<i64> = keys
        .iter()
        .map(|&k| {
            usize::try_from(k - 1)
                .ok()
                .and_then(|i| checking_table.get(i))
                .copied()
                .unwrap_or(0)
        })
        .collect();
    let total_score: i64 = scores.iter().sum();

    let chained_sequence = find_sequence(&keys);

    let mut text = format!(
        "3D6 => [{}] => ({}) => {total_score}",
        join(&keys, ","),
        join(&scores, "+")
    );

    if !chained_sequence.is_empty() {
        let fumble = is_fumble(&keys, &chained_sequence);
        let bonus = if fumble {
            3
        } else {
            chained_sequence.len() as i64
        };
        text.push_str(&format!(
            " | {} chain! ({}) => {}",
            chained_sequence.len(),
            join(&chained_sequence, ","),
            total_score + bonus
        ));

        if chained_sequence.len() >= 3 {
            text.push_str(" [スペシャル]");
        }

        if fumble {
            text.push_str(" [ファンブル]");
        }
    }

    Ok(text)
}

/// Ruby `LostRoyal#find_sequence`。
///
/// 2個以上連なった列のうち最長のものを返す（無ければ空）。
/// Ruby の `max` は同じ長さなら先に見つかった方を残す。
fn find_sequence(keys: &[i64]) -> Vec<i64> {
    let mut keys = keys.to_vec();
    keys.sort_unstable();

    let mut longest: Vec<i64> = Vec::new();
    // Ruby: (1...6).map { |start_key| find_sequence_from_start_key(keys, start_key) }
    for start_key in 1..6 {
        let sequence = find_sequence_from_start_key(&keys, start_key);
        if sequence.len() > 1 && sequence.len() > longest.len() {
            longest = sequence;
        }
    }
    longest
}

/// Ruby `LostRoyal#find_sequence_from_start_key`。
///
/// `start_key` から昇順に連なる出目を集め、先頭が 1 なら 6 側からも遡って
/// 前に繋げる（6→1 のループ）。
fn find_sequence_from_start_key(keys: &[i64], start_key: i64) -> Vec<i64> {
    let mut chained_keys: Vec<i64> = Vec::new();

    let mut key = start_key;
    while keys.contains(&key) {
        chained_keys.push(key);
        key += 1;
    }

    if chained_keys.first() == Some(&1) {
        let mut key = 6;
        while keys.contains(&key) {
            chained_keys.insert(0, key);
            key -= 1;
        }
    }

    chained_keys
}

/// Ruby `LostRoyal#fumble_?`: 連なった出目のどれかが2個以上出ていればファンブル。
fn is_fumble(keys: &[i64], chained_sequence: &[i64]) -> bool {
    chained_sequence
        .iter()
        .any(|k| keys.iter().filter(|key| *key == k).count() >= 2)
}

/// Ruby `LostRoyal#roll_fumble_chart`（`FC`）。
fn roll_fumble_chart(rng: &mut Randomizer) -> Result<String, EvalError> {
    static TEXTS: [&str; 6] = [
        "何かの問題で言い争い、主君に無礼を働いてしまう。あなたは主君の名誉点を１点失うか、【時間】を１点消費して和解の話し合いを持つか選べる。",
        "見過ごせば人々を不幸にする危険に遭遇する。あなたは逃げ出して冒険の名誉点を１点失うか、これに立ち向かい【命数】を２点減らすかを選べる。",
        "あなたが惹かれたのは好意に付け込む人だった。あなたはその場を去って恋慕の名誉点を１点失うか【正義】を１点減らして礼を尽くすかを選べる。",
        "金銭的な問題で、生命と魂の苦しみを背負う人に出会う。あなたは庇護の名誉点を１点失うか出費を３点増やすかを選べる。",
        "襲撃を受ける。苦もなく叩き伏せると、卑屈な態度で命乞いをしてきた。容赦なく命を奪い寛容の名誉点を１点失うか、密告によって【血路】が１Ｄ６点増えるかを選ぶことができる。",
        "風聞により、友が悪に身を貶めたと知る。共に並んだ戦場が色褪せる想いだ。戦友の名誉点を１点減らすか、【酒と歌】すべてを失うかを選べる。",
    ];

    let key = rng.roll_once(6)?;
    let text = chart_text(&TEXTS, key);

    Ok(format!("1D6 => [{key}] {text}"))
}

/// Ruby `LostRoyal#roll_emotion_chart`（`EC`）。
fn roll_emotion_chart(rng: &mut Randomizer) -> Result<String, EvalError> {
    static TEXTS: [&str; 6] = [
        "愛情／殺意",
        "友情／負目",
        "崇拝／嫌悪",
        "興味／侮蔑",
        "信頼／嫉妬",
        "守護／欲情",
    ];

    let key = rng.roll_once(6)?;
    let text = chart_text(&TEXTS, key);

    Ok(format!("1D6 => [{key}] {text}"))
}

/// Ruby `[...][key - 1]`。範囲外は Ruby の `nil` と同じく空文字列。
fn chart_text(texts: &[&'static str], key: i64) -> &'static str {
    usize::try_from(key - 1)
        .ok()
        .and_then(|i| texts.get(i))
        .copied()
        .unwrap_or("")
}

/// Ruby `LostRoyal#roll_wind_power_chart`（`WPC`）。
///
/// 出目の累計が 1〜2 の間は振り足す。
fn roll_wind_power_chart(rng: &mut Randomizer) -> Result<String, EvalError> {
    /// Ruby の `[add, bonus, current_text]` の表。
    static CHART: [(bool, i64, &str); 7] = [
        (true, 0, "ほぼ凪（振り足し）"),
        (true, 0, "弱い風（振り足し）"),
        (false, 0, "ゆるやかな風"),
        (false, 0, "ゆるやかな風"),
        (false, 1, "やや強い風（儀式点プラス１）"),
        (false, 2, "強い風（龍を幻視、儀式点プラス２）"),
        (false, 3, "体が揺らぐほどの風（龍を幻視、儀式点プラス３）"),
    ];

    let mut key = 0;
    let mut total_bonus = 0;
    let mut text = String::new();

    loop {
        let dice = rng.roll_once(6)?;
        key += dice;

        // Ruby: [...][[key, 7].min - 1]（出目は 1 以上なので添字は 0〜6 に収まる）
        let index = usize::try_from(key.min(7) - 1).unwrap_or(0);
        let (add, bonus, current_text) = CHART[index];

        total_bonus += bonus;

        let current_text = if key != dice {
            format!("1D6[{dice}]+{} {current_text}", key - dice)
        } else {
            format!("1D6[{dice}] {current_text}")
        };

        if text.is_empty() {
            text = current_text;
        } else {
            text = format!("{text} => {current_text}");
        }

        if !add {
            text.push_str(&format!(" [合計：儀式点 +{total_bonus} ]"));
            return Ok(text);
        }
    }
}

/// Ruby `LostRoyal#roll_hope`（`HRx`）。
///
/// 出目に 1 か 2 があれば振り足す。
fn roll_hope(number_of_dice: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut total = 0;
    let mut text = String::new();

    loop {
        let d1 = rng.roll_once(6)?;
        let mut d2 = 0;

        if number_of_dice >= 2 {
            d2 = rng.roll_once(6)?;
        }

        total += d1 + d2;

        if number_of_dice == 2 {
            text.push_str(&format!("2D6[{d1},{d2}]"));
        } else {
            text.push_str(&format!("1D6[{d1}]"));
        }

        if is_1or2(d1) || is_1or2(d2) {
            text.push_str(" （振り足し） => ");
        } else {
            text.push_str(&format!(" => 合計 {total}"));
            return Ok(text);
        }
    }
}

/// Ruby `LostRoyal#is_1or2`。
fn is_1or2(n: i64) -> bool {
    n == 1 || n == 2
}

/// Ruby `Array#join(sep)`。
fn join(values: &[i64], sep: &str) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(sep)
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
            .join("test/data/LostRoyal.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/LostRoyal.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/LostRoyal.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("LostRoyal.toml must parse");
        assert_eq!(
            data.tests.len(),
            51,
            "case count in test/data/LostRoyal.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "LostRoyal",
                "unexpected game system in LostRoyal.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("LostRoyal"), &tc.input, &mut src) {
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
                    "FAIL LostRoyal:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} LostRoyal cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
