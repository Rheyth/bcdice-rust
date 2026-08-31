//! P4で手書き移植した `lib/bcdice/game_system/EndBreaker.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `EndBreaker#checkRoll`（判定 `nEB`。1の出目ごとにダブルトリガーで2個振り足し）
//! - `EndBreaker#getLifeAndDeathUnknownResult`（生死不明表 `LDUT`）
//!
//! 表データは原典rbの配列をそのまま写したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `/(\d+)EB/i`。アンカーが無いので部分一致でよい。
fn check_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)EB").expect("valid regex"))
}

/// Ruby `EndBreaker#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(m) = check_roll_pattern().captures(command) {
        // Ruby: Regexp.last_match(1).to_i。i64に収まらない個数は飽和させ、
        // `roll_barabara` の上限超過（TooManyRandsError）へ落とす。
        let dice_count: i64 = m[1].parse().unwrap_or(i64::MAX);
        return Ok(Some(SpecificCommandOutput::text(check_roll(
            dice_count, rng,
        )?)));
    }

    // Ruby: case command when "LDUT" ... else return nil
    if command != "LDUT" {
        return Ok(None);
    }

    let table_name = "生死不明表";
    let (text, number) = life_and_death_unknown_result(rng)?;

    Ok(Some(SpecificCommandOutput::text(format!(
        "{table_name}({number}):{text}"
    ))))
}

/// Ruby `EndBreaker#checkRoll`。
fn check_roll(dice_count: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    // Ruby: rollCount = diceCount # ダブルトリガー
    let mut roll_count = dice_count;

    let mut result = String::new();
    let mut dice_full_list: Vec<i64> = Vec::new();

    while roll_count != 0 {
        let mut dice_list = rng.roll_barabara(roll_count, 6)?;
        dice_list.sort_unstable();
        dice_full_list.extend_from_slice(&dice_list);

        // 1の出目ごとにダブルトリガーで2個ダイス追加
        roll_count = dice_list.iter().filter(|&&i| i == 1).count() as i64 * 2;

        // Ruby: diceList.join（区切り文字なし）
        let joined: String = dice_list.iter().map(|d| d.to_string()).collect();
        result.push_str(&format!("[{joined}]"));
        if roll_count > 0 {
            result.push_str(" ダブルトリガー! ");
        }
    }

    // ダイスの出目の個数を集計
    result.push_str(" ＞");
    for num in 2..=6 {
        let count = dice_full_list.iter().filter(|&&i| i == num).count();
        if count != 0 {
            result.push_str(&format!(" [{num}:{count}個]"));
        }
    }

    Ok(result)
}

/// Ruby `EndBreaker#getLifeAndDeathUnknownResult`。
fn life_and_death_unknown_result(
    rng: &mut Randomizer,
) -> Result<(&'static str, String), EvalError> {
    get_table_by_d66(&LIFE_AND_DEATH_UNKNOWN_TABLE, rng)
}

/// Ruby `Base#get_table_by_d66(table)`。戻り値は `[text, indexText]`。
///
/// `indexText` は `"#{dice1}#{dice2}"` の**文字列**（D66の値ではない）。
fn get_table_by_d66(
    table: &[&'static str],
    rng: &mut Randomizer,
) -> Result<(&'static str, String), EvalError> {
    let dice1 = rng.roll_once(6)?;
    let dice2 = rng.roll_once(6)?;

    let num = (dice1 - 1) * 6 + (dice2 - 1);
    let index_text = format!("{dice1}{dice2}");

    // Ruby: return "1", indexText if text.nil?
    let text = usize::try_from(num)
        .ok()
        .and_then(|i| table.get(i).copied())
        .unwrap_or("1");

    Ok((text, index_text))
}

/// Ruby `EndBreaker#getLifeAndDeathUnknownResult` の `table`。
static LIFE_AND_DEATH_UNKNOWN_TABLE: [&str; 36] = [
    // D66 11〜16
    " 1日：生還！",
    " 1日：生還！",
    " 1日：生還！",
    " 1日：生還！",
    " 1日：生還！",
    " 1日：生還！",
    // D66 21〜26
    " 1日：生還！",
    " 5日：敵に捕らわれ、ひどい暴行と拷問を受けた。",
    " 2日：謎の人物に命を救われた。",
    "10日：奴隷として売り飛ばされた。",
    " 8日：おぞましい儀式の生贄として連れ去られた。",
    " 9日：幽閉・投獄された。",
    // D66 31〜36
    " 1日：生還！",
    " 7日：モンスター蠢く地下迷宮に滑落した。",
    "12日強力なマスカレイドにとらわれ、実験台にされた。",
    " 8日：放浪中に遭遇した事件を、颯爽と解決していた。",
    " 5日：飢餓状態に追い込まれた。",
    "15日：記憶を失い放浪した。",
    // D66 41〜46
    " 1日：生還！",
    "10日：異性に命を救われて、手厚い看病を受けた。",
    " 3日：負傷からくる熱病で、生死の境を彷徨った。",
    "11日：闘奴にされたが、戦いと友情の末に自由を獲得した。",
    " 6日：負傷したまま川に落ち、遥か下流まで流された。",
    " 9日：敵に連れ去られ、執拗な拷問を受け続けた。",
    // D66 51〜56
    " 1日：生還！",
    " 4日：繰り返す「死の悪夢」に苛まれた。",
    " 3日：巨獣の巣に連れ去られた。",
    "10日：謎の集団に救われて、手厚い看病を受けた。",
    " 3日：チッタニアンの集落に迷い込み、もてなしを受けた。",
    " 7日：ピュアリィの群れにとらわれ、弄ばれた。",
    // D66 61〜66
    " 1日：生還！",
    " 6日：楽園のような場所を発見し、しばらく逗留した。",
    " 9日：盗賊団に救われ、恩返しとして少し用心棒をした。",
    "10日：熱病の見せる官能的な幻影にとらわれ、彷徨った。",
    " 5日：謎の賞金首に狙われ、傷めつけられていた。",
    " - ：「五分五分」の一般判定。失敗すると死亡。",
];

/// Ruby `BCDice::GameSystem::EndBreaker`（ID: `EndBreaker`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndBreaker;

impl GameSystem for EndBreaker {
    fn id(&self) -> &'static str {
        "EndBreaker"
    }

    fn name(&self) -> &'static str {
        "エンドブレイカー！"
    }

    fn sort_key(&self) -> &'static str {
        "えんとふれいかあ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 (nEB)
  n個のD6を振る判定。ダブルトリガー発動で自動振り足し。
・各種表
  ・生死不明表 (LDUT)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+EB", "LDUT"]
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
            .join("test/data/EndBreaker.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/EndBreaker.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/EndBreaker.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("EndBreaker.toml must parse");
        assert_eq!(
            data.tests.len(),
            8,
            "case count in test/data/EndBreaker.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "EndBreaker",
                "unexpected game system in EndBreaker.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("EndBreaker"), &tc.input, &mut src) {
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
                    "FAIL EndBreaker:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} EndBreaker cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
