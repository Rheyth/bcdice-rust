//! P4で手書き移植した `lib/bcdice/game_system/Yotabana.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Yotabana#eval_game_system_specific_command` → `roll_tables(command, TABLES)`
//! - `TABLES`（収束表 `COT` / イベント表 `EVT`）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない。

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static COT_ITEMS: &[&str] = &[
    "サプライズ忍者／唐突に忍者が乱入し、場面にいるキャラクターを倒して去っていく",
    "仙人／唐突に仙人が乱入し、不思議な力で事態を収束させて帰っていく",
    "洗脳薬／不思議な薬が散布され、キャラクターを洗脳し、事態を収束させる",
    "作者の手／キャラクターたちの言動が唐突に変わり、事態が収束する。作者の大いなる手だ……",
    "神の奇跡／神が奇跡を起こし事態を収束させる。または神の信徒になり、信仰の前に争いは無意味であると悟る",
    "和解／話し合えば分かり合えた。この世は対話で通じ合える",
];
static COT: Table = Table::from_dice("収束表", 1, 6, COT_ITEMS);

static EVT_ITEMS: &[&str] = &[
    "道端に刺さっていた聖剣を拾う",
    "ゾンビの群れと遭遇する",
    "落ちていたコインを拾う。ちょっとラッキーな気分になる",
    "あらゆるところで爆発が！？",
    "唐突に冬が訪れ、猛吹雪が襲う",
    "無人のトラックが突っ込んでくる",
    "ネコちゃんに懐かれる",
    "料金滞納で水道を止められる",
    "ゴキゲンな音楽が鳴り響く",
    "水着になる",
    "オークションにかけられる",
    "殺人アンドロイドが襲いかかってくる",
];
static EVT: Table = Table::from_dice("イベント表", 1, 12, EVT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[("COT", &COT), ("EVT", &EVT)];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `BCDice::GameSystem::Yotabana`（ID: `Yotabana`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Yotabana;

impl GameSystem for Yotabana {
    fn id(&self) -> &'static str {
        "Yotabana"
    }

    fn name(&self) -> &'static str {
        "ヨタバナ"
    }

    fn sort_key(&self) -> &'static str {
        "よたはな"
    }

    fn help_message(&self) -> &'static str {
        r"▪️ 各種表
  COT 収束表
  EVT イベント表
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["COT", "EVT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Yotabana#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/Yotabana.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Yotabana.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Yotabana.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Yotabana.toml must parse");
        assert_eq!(data.tests.len(), 2, "case count in test/data/Yotabana.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Yotabana",
                "unexpected game system in Yotabana.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Yotabana"), &tc.input, &mut src) {
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
                    "FAIL Yotabana:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Yotabana cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
