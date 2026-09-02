//! P4で手書き移植した `lib/bcdice/game_system/CastleInGray.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#roll_color`（色占い `BnWm`）と `#color_text`
//! - `#roll_mal`（悪意の渦による占い `MALn`）
//! - `TABLES`（感情表・暗示表(黒)・暗示表(白)）と `Base#roll_tables`
//!
//! 表データは `lib/bcdice/game_system/CastleInGray.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::CastleInGray`（ID: `CastleInGray`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleInGray;

impl GameSystem for CastleInGray {
    fn id(&self) -> &'static str {
        "CastleInGray"
    }

    fn name(&self) -> &'static str {
        "灰色城綺譚"
    }

    fn sort_key(&self) -> &'static str {
        "はいいろしようきたん"
    }

    fn help_message(&self) -> &'static str {
        r"■ 色占い (BnWm)
n: 黒
m: 白
n, m は1～12の異なる整数

例) B12W7
例) B5W12

■ 悪意の渦による占い (MALn)
n: 悪意の渦
n は1～12の整数

■ その他
・感情表 ET
・暗示表(黒) BIT
・暗示表(白) WIT
"
    }

    /// Ruby `register_prefix('B', 'MAL', TABLES.keys)`。
    fn prefixes(&self) -> &'static [&'static str] {
        &["B", "MAL", "ET", "BIT", "WIT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#eval_game_system_specific_command`:
    /// `roll_color(command) || roll_mal(command) || table_helpers::roll_table(command, TABLES, TABLES)`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = roll_color(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        if let Some(text) = roll_mal(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        if let Some(text) = table_helpers::roll_table(command, TABLES, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(None)
    }
}

/// Ruby `#roll_color`（色占い）。
fn roll_color(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^B(\d{1,2})W(\d{1,2})$").expect("valid regex"));

    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let black: i64 = m[1].parse().expect("1..2 digits");
    let white: i64 = m[2].parse().expect("1..2 digits");
    if !(1..=12).contains(&black) || !(1..=12).contains(&white) {
        return Ok(None);
    }

    let value = rng.roll_once(12)?;

    if black == white {
        return Ok(Some(color_text(
            black,
            white,
            value,
            "白と黒は重ねられません",
        )));
    }

    let result = if white > black {
        if black <= value && value < white {
            "黒"
        } else {
            "白"
        }
    } else if white <= value && value < black {
        "白"
    } else {
        "黒"
    };

    Ok(Some(color_text(black, white, value, result)))
}

/// Ruby `#color_text`。
fn color_text(black: i64, white: i64, value: i64, result: &str) -> String {
    format!("色占い(黒{black}白{white}) ＞ [{value}] ＞ {result}")
}

/// Ruby `#roll_mal`（悪意の渦による占い）。
fn roll_mal(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)^MAL(\d{1,2})$").expect("valid regex"));

    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let mal: i64 = m[1].parse().expect("1..2 digits");
    if !(1..=12).contains(&mal) {
        return Ok(None);
    }

    let value = rng.roll_once(12)?;
    let result = if value <= mal { "黒" } else { "白" };
    Ok(Some(format!("悪意の渦({mal}) ＞ [{value}] ＞ {result}")))
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/CastleInGray.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `TABLES["ET"]`（感情表）。
static ET_ITEMS: &[&str] = &[
    "友情(白)／敵視(黒)",
    "恋慕(白)／嫌悪(黒)",
    "信頼(白)／不信(黒)",
    "同情(白)／憐憫(黒)",
    "憧憬(白)／劣等感(黒)",
    "尊敬(白)／蔑視(黒)",
    "忠誠(白)／執着(黒)",
    "有用(白)／邪魔(黒)",
    "許容(白)／罪悪感(黒)",
    "羨望(白)／嫉妬(黒)",
    "共感(白)／拒絶(黒)",
    "愛情(白)／狂信(黒)",
];
static ET_TABLE: Table = Table::from_dice("感情表", 1, 12, ET_ITEMS);

/// Ruby `TABLES["BIT"]`（暗示表(黒)）。
static BIT_ITEMS: &[&str] = &[
    "終わりなき夜に生まれつく者もあり",
    "悪意もて真実を語らば",
    "笑えども笑みはなし",
    "影より抜け出ることあたわじ",
    "心の赴くままに手をとれ",
    "時ならぬ嵐の過ぎ去るを待つ",
    "赦されぬと知るがゆえに",
    "見張りは持ち場を離れる",
    "誰もが盲いたる彷徨い人なり",
    "落ちる日を眺めるがごとく",
    "冷たく雨ぞ降りしきる",
    "今日は笑む花も明日には枯れゆく",
];
static BIT_TABLE: Table = Table::from_dice("暗示表(黒)", 1, 12, BIT_ITEMS);

/// Ruby `TABLES["WIT"]`（暗示表(白)）。
static WIT_ITEMS: &[&str] = &[
    "無垢なる者のみが真実を得る",
    "げに慈悲深きは沈黙なり",
    "懐かしき日々は去りぬ",
    "束の間に光さす",
    "迷える者に手を差し伸べよ",
    "嵐の前には静けさがある",
    "どうか責めないで",
    "灯した明かりを絶やさぬように",
    "目を開けて見よ",
    "淑やかに訪れる",
    "今こそ泣け、さもなくば二度と泣くな",
    "時が許す間に薔薇を摘め",
];
static WIT_TABLE: Table = Table::from_dice("暗示表(白)", 1, 12, WIT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[("ET", &ET_TABLE), ("BIT", &BIT_TABLE), ("WIT", &WIT_TABLE)];

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
            .join("test/data/CastleInGray.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/CastleInGray.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/CastleInGray.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("CastleInGray.toml must parse");
        assert_eq!(
            data.tests.len(),
            30,
            "case count in test/data/CastleInGray.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "CastleInGray",
                "unexpected game system in CastleInGray.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("CastleInGray"), &tc.input, &mut src) {
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
                    "FAIL CastleInGray:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} CastleInGray cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
