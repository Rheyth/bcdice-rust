//! P4で手書き移植した `lib/bcdice/game_system/Strave.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Strave#checkRoll`（モラトリアムフェイズ用判定 `MPm` / 命中判定 `nSTm*p`）
//! - 所属表 `AFF`/`AFV` とアイデンティティ表 `IDT`/`IDV`（1D100 → 20項目へ丸める表）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `get_affiliation_table` の表（`所属表：基本`）。
static AFFILIATION_TABLE: &[(i64, &str)] = &[
    (
        1,
        "アリウス管理委員会：あなたはアリウス管理委員会に所属している。",
    ),
    (
        2,
        "オーヴァーブルー：あなたはオーヴァーブルーに所属している。",
    ),
    (3, "ウォルゲイト：あなたはウォルゲイトに所属している。"),
    (
        4,
        "暁部隊：あなたはかつて、反逆者・暁弥琴と同じ部隊に所属していた。",
    ),
    (5, "天文部：あなたは天文部に所属している。"),
    (6, "吹奏楽部：あなたは吹奏楽部に所属している。"),
    (7, "剣道部：あなたは剣道部に所属している。"),
    (8, "ボクシング部：あなたはボクシング部に所属している。"),
    (9, "陸上部：あなたは陸上部に所属している。"),
    (10, "茶道部：あなたは茶道部に所属している。"),
    (11, "パソコン部：あなたはパソコン部に所属している。"),
    (12, "新聞部：あなたは新聞部に所属している。"),
    (13, "弓道部：あなたは弓道部に所属している。"),
    (14, "美術部：あなたは美術部に所属している。"),
    (
        15,
        "ミリタリー研究会：あなたはミリタリー研究会に所属している。",
    ),
    (16, "歴史研究会：あなたは歴史研究会に所属している。"),
    (17, "ロボット研究会：あなたはロボット研究会に所属している。"),
    (18, "図書委員会：あなたは図書委員会に所属している。"),
    (19, "任意：あなたの任意の所属を設定せよ。"),
    (20, "任意：あなたの任意の所属を設定せよ。"),
];

/// Ruby `get_identity_table` の表（`アイデンティティ表：基本`）。
static IDENTITY_TABLE: &[(i64, &str)] = &[
    (1, "戦い：戦いこそが、あなたをあなたたらしめている。"),
    (2, "守護：あなたには守るべきものがある。"),
    (
        3,
        "復讐：あなたは復讐を誓っている。何かに、あるいは誰かに。",
    ),
    (4, "名声：その身に浴びる脚光を、何よりも誉としている。"),
    (5, "恋愛：あなたはその身を焦がす恋に生きている。"),
    (6, "家族：あなたにとって、家族はかけがえの無いものだ。"),
    (7, "友人：あなたは友のために戦っている。"),
    (8, "部隊：共に戦う部隊の仲間が、あなたに力をくれる。"),
    (
        9,
        "ストレイヴ：あなたは自身のストレイヴを誇りに思っている。",
    ),
    (
        10,
        "スフィアブレイク：あなたはスフィアブレイクを熱烈に目指している。",
    ),
    (
        11,
        "お金：あなたはお金を求めている。報酬こそが自分の価値だ。",
    ),
    (12, "夢：あなたには夢がある。自分を突き動かす夢が。"),
    (
        13,
        "忠誠：あなたは忠誠を誓っている。何かに、あるいは誰かに。",
    ),
    (14, "共生：あなたは、ヴァイエルと人類との共生を望んでいる。"),
    (15, "居場所：自身の居場所こそが、あなたに力をくれる。"),
    (16, "強制：あなたは不本意ながら今の立場にいる。"),
    (17, "碧空：見上げた青空が、あなたを変えた。"),
    (18, "任意：あなたの任意のアイデンティティを設定せよ。"),
    (19, "任意：あなたの任意のアイデンティティを設定せよ。"),
    (20, "任意：あなたの任意のアイデンティティを設定せよ。"),
];

/// Ruby `get_affiliation_table2` の表（`所属表：ヴァリアンスネイヴァー`）。
static AFFILIATION_TABLE2: &[(i64, &str)] = &[
    (
        1,
        "シュヴァレ・トワール：あなたはシュヴァレ・トワールに所属している。",
    ),
    (
        2,
        "ディープシンカー：あなたはディープシンカーに所属している。",
    ),
    (
        3,
        "ヴェルクシュタット：あなたはヴェルクシュタットに所属している。",
    ),
    (4, "アウスヴァル：あなたはアウスヴァルに所属している。"),
    (5, "美術科：あなたは美術科に所属している。"),
    (6, "哲学科：あなたは哲学科に所属している。"),
    (7, "数学科：あなたは数学科に所属している。"),
    (8, "地理学科：あなたは地理学科に所属している。"),
    (9, "工学科：あなたは工学科に所属している。"),
    (10, "体育学科：あなたは体育学科に所属している。"),
    (11, "農学科：あなたは農学科に所属している。"),
    (12, "歴史学科：あなたは歴史学科に所属している。"),
    (13, "医学科：あなたは医学科に所属している。"),
    (14, "情報学科：あなたは情報学科に所属している。"),
    (15, "音楽科：あなたは音楽科に所属している。"),
    (16, "心理学科：あなたは心理学科に所属している。"),
    (17, "文学科：あなたは文学科に所属している。"),
    (18, "任意：あなたの任意の所属を設定すること。"),
    (19, "任意：あなたの任意の所属を設定すること。"),
    (20, "任意：あなたの任意の所属を設定すること。"),
];

/// Ruby `get_identity_table2` の表（`アイデンティティ表：ヴァリアンスネイヴァー`）。
static IDENTITY_TABLE2: &[(i64, &str)] = &[
    (1, "戦い：戦いへの衝動が、あなたをあなたたらしめている。"),
    (2, "守護：守るべきものの存在が、あなたをあなたたらしめている。"),
    (3, "復讐：復讐の誓いこそが、あなたをあなたたらしめている。"),
    (4, "名声：与えられた名誉こそが、あなたをあなたたらしめている。"),
    (5, "恋愛：愛する者への想いが、あなたをあなたたらしめている。"),
    (6, "家族：かけがえのない家族が、あなたをあなたたらしめている。"),
    (7, "友人：友の存在が、あなたをあなたたらしめている。"),
    (8, "部隊：部隊の戦友こそが、あなたをあなたたらしめている。"),
    (9, "ストレイヴ：ストレイヴの存在が、あなたの心を保っている。"),
    (10, "宇宙：やがて来る旅立ちの日まで、あなたはあなたであろうとしている。"),
    (11, "お金：与えられる報酬のため、あなたはあなたであろうとしている。"),
    (12, "夢：あなたには、己の心に誓った夢がある。"),
    (13, "忠誠：その心でもって、誓った忠義がある。"),
    (14, "共生：あなたは、ヴァイエルと人類との共生を望んでいる。"),
    (15, "居場所：自身の居場所への思いが、あなたをあなたたらしめている。"),
    (16, "ヴァイエル：あなたと同じでありながら、あなたと異なる存在。彼らへの思いが、あなたをあなたたらしめている。"),
    (17, "エコール：自身の生きる場所への思いが、あなたをあなたたらしめている。"),
    (18, "任意：あなたの任意のアイデンティティを設定せよ。"),
    (19, "任意：あなたの任意のアイデンティティを設定せよ。"),
    (20, "任意：あなたの任意のアイデンティティを設定せよ。"),
];

/// Ruby `BCDice::GameSystem::Strave`（ID: `Strave`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strave;

impl GameSystem for Strave {
    fn id(&self) -> &'static str {
        "Strave"
    }

    fn name(&self) -> &'static str {
        "碧空のストレイヴ"
    }

    fn sort_key(&self) -> &'static str {
        "へきくうのすとれいふ"
    }

    fn help_message(&self) -> &'static str {
        r"・モラトリアムフェイズ用判定：MPm
・命中判定：nSTm*p

「n」でダイス数を指定。
「m」で目標値を指定。省略は出来ません。
「p」で攻撃力を指定。「*」は「x」でも可。

【書式例】
・MP6 → 目標値6のモラトリアムフェイズ用判定。
・5ST6*10 → 5d10で目標値6、攻撃力10の命中判定。

【各種表】
・所属表：AFF　　VN版：AFV
・アイデンティティ表：IDT　　VN版：IDV

※アイデンティティ表はエラッタ適用済です。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["MP", r"\d+ST", "AFF", "IDT", "AFV", "IDV"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Strave#eval_game_system_specific_command`。
    ///
    /// Ruby は `command.upcase` してから `case` に掛けるが、`Base#dice_command` が
    /// すでに大文字化しているので追加の変換は不要（`@enabled_upcase_input` は既定の true）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: when /MP(\d+)$/i（モラトリアムフェイズ用判定）
        if let Some(m) = moratorium_pattern().captures(command) {
            let target = ruby_to_i(&m[1]);
            return Ok(Some(SpecificCommandOutput::text(check_roll(
                2, target, None, rng,
            )?)));
        }

        // Ruby: when /(\d+)ST(\d+)(x|\*)(\d+)$/i（命中判定）
        if let Some(m) = hit_pattern().captures(command) {
            let dice_count = ruby_to_i(&m[1]);
            let target = ruby_to_i(&m[2]);
            // Ruby: (Regexp.last_match(4) || 0).to_i。`(\d+)` は必須なので必ずマッチする。
            let damage = ruby_to_i(&m[4]);
            return Ok(Some(SpecificCommandOutput::text(check_roll(
                dice_count,
                target,
                Some(damage),
                rng,
            )?)));
        }

        let table = match command {
            "AFF" => Some(("所属表：基本", AFFILIATION_TABLE)),
            "IDT" => Some(("アイデンティティ表：基本", IDENTITY_TABLE)),
            "AFV" => Some(("所属表：ヴァリアンスネイヴァー", AFFILIATION_TABLE2)),
            "IDV" => Some((
                "アイデンティティ表：ヴァリアンスネイヴァー",
                IDENTITY_TABLE2,
            )),
            _ => None,
        };
        let Some((name, table)) = table else {
            return Ok(None);
        };

        Ok(Some(SpecificCommandOutput::text(roll_1d100_table(
            name, table, rng,
        )?)))
    }
}

/// Ruby `/MP(\d+)$/i`。
fn moratorium_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)MP(\d+)$").expect("valid regex"))
}

/// Ruby `/(\d+)ST(\d+)(x|\*)(\d+)$/i`。
fn hit_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d+)ST(\d+)(x|\*)(\d+)$").expect("valid regex"))
}

/// Ruby `Strave#checkRoll`。
///
/// `damage` が `None` ならモラトリアムフェイズ用判定（成功/失敗の表示）、
/// `Some` なら命中判定（ダメージの表示）。
fn check_roll(
    dice_count: i64,
    target: i64,
    damage: Option<i64>,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let target = target.clamp(1, 10);

    let mut dice_array = rng.roll_barabara(dice_count, 10)?;
    dice_array.sort_unstable();
    let dice_text = join_dice(&dice_array);

    let success_count = dice_array.iter().filter(|&&i| i <= target).count() as i64;

    let head = format!("({dice_count}D10<={target}) ＞ {dice_text}");
    match damage {
        Some(damage) => {
            let total_damage = success_count * damage;
            Ok(format!(
                "{head} ＞ Hits：{success_count}*{damage} ＞ {total_damage}ダメージ"
            ))
        }
        None if success_count > 0 => Ok(format!("{head} ＞ 【成功】")),
        None => Ok(format!("{head} ＞ 【失敗】")),
    }
}

/// Ruby `Strave#get_strave_1d100_table_result`。
///
/// 1D100の出目を5刻みで1..20へ丸め、`get_table_by_number` で項目を引く。
fn roll_1d100_table(
    name: &str,
    table: &[(i64, &'static str)],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice = rng.roll_once(100)?;
    // Ruby: ((dice.to_i - 1) / 5).floor + 1（Integer#/ は床除算）
    let index = (dice - 1).div_euclid(5) + 1;
    let result = get_table_by_number(index, table);
    // Ruby `get_strave_table_result`
    Ok(format!("{name}({dice}) ＞ {result}"))
}

/// Ruby `Base#get_table_by_number(index, table)`（既定値は `"1"`）。
///
/// 「最初に `item[0] >= index` となった項目」を返す。完全一致ではない。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    table
        .iter()
        .find(|(number, _)| *number >= index)
        .map_or("1", |(_, text)| *text)
}

/// Ruby `String#to_i`。ここに来るのは `\d+` なので符号や空白は現れない。
fn ruby_to_i(s: &str) -> i64 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        // Ruby: "".to_i == 0
        return 0;
    }
    // 桁あふれは Ruby だと Bignum になる。i64 に収まらない場合は飽和させ、
    // ダイス個数なら `roll_barabara` の上限（TooManyRandsError）へ落ちるようにする。
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `diceArray.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
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
            .join("test/data/Strave.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Strave.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Strave.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Strave.toml must parse");
        assert_eq!(data.tests.len(), 12, "case count in test/data/Strave.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Strave",
                "unexpected game system in Strave.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Strave"), &tc.input, &mut src) {
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

            let allowed_surplus = 0;
            if src.remaining() != allowed_surplus {
                reasons.push(format!(
                    "unconsumed rands remain ({}, allowed {allowed_surplus})",
                    src.remaining()
                ));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Strave:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Strave cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
