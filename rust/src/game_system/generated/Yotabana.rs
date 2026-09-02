//! P4で手書き移植した `lib/bcdice/game_system/Yotabana.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Yotabana#eval_game_system_specific_command` → `table_helpers::roll_table(command, TABLES, TABLES)`
//! - `TABLES`（収束表 `COT` / イベント表 `EVT`）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない。

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
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
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Yotabana", "Yotabana.toml", 2);
    }
}
