//! P4で手書き移植した `lib/bcdice/game_system/GhostLive.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GhostLive#eval_game_system_specific_command`（`ALIAS` 経由の表引き）
//! - `TABLES`（追加目標表・各霊障リスト／霊障効果リスト）

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `HELP_MESSAGE` 定数（スタブ生成時の値をそのまま保つ）。
static HELP_MESSAGE: &str = r"■追加目標表（p11）
ATT, AdditionalTargetTable

■種別：地縛霊（p26）
□Ａ.霊障リスト
JHA, JibakuHauntA
□Ｂ.霊障効果リスト
JHB, JibakuHauntB

■種別：シャイな幽霊（p27）
□Ａ.霊障リスト
SHA, ShyHauntA
□Ｂ.霊障効果リスト
SHB, ShyHauntB

■種別：ぐちゃぐちゃ（p28）
□Ａ.霊障リスト
GHA, GuchaHauntA
□Ｂ.霊障効果リスト
GHB, GuchaHauntB
";

/// Ruby `BCDice::GameSystem::GhostLive`（ID: `GhostLive`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhostLive;

impl GameSystem for GhostLive {
    fn id(&self) -> &'static str {
        "GhostLive"
    }

    fn name(&self) -> &'static str {
        "実況ゴーストライヴ"
    }

    fn sort_key(&self) -> &'static str {
        "しつきようこおすとらいふ"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "ADDITIONALTARGETTABLE",
            "JIBAKUHAUNTA",
            "JIBAKUHAUNTB",
            "SHYHAUNTA",
            "SHYHAUNTB",
            "GUCHAHAUNTA",
            "GUCHAHAUNTB",
            "ATT",
            "JHA",
            "JHB",
            "SHA",
            "SHB",
            "GHA",
            "GHB",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `GhostLive#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: command = ALIAS[command] || command
        let command = ALIAS
            .iter()
            .find(|(from, _)| *from == command)
            .map(|(_, to)| *to)
            .unwrap_or(command);

        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `ALIAS`（値は `transform_values(&:upcase)` 済み）。
static ALIAS: &[(&str, &str)] = &[
    ("ATT", "ADDITIONALTARGETTABLE"),
    ("JHA", "JIBAKUHAUNTA"),
    ("JHB", "JIBAKUHAUNTB"),
    ("SHA", "SHYHAUNTA"),
    ("SHB", "SHYHAUNTB"),
    ("GHA", "GUCHAHAUNTA"),
    ("GHB", "GUCHAHAUNTB"),
];

/// Ruby `TABLES["ADDITIONALTARGETTABLE"]`（追加目標表）の項目。
static ADDITIONAL_TARGET_TABLE_ITEMS: &[&str] = &[
    "オバケを撮影する。（依頼主：専門家／報酬：１Ｌ）",
    "誰かひとりが霊障を［サイクル数］回受ける。（依頼主：専門家／報酬：［サイクル数］Ｌ）",
    "誰かひとりが［精神力］を10以下の状態で帰る。（依頼主：専門家／報酬：３Ｌ）",
    "［精神力］の平均が20以下の状態で帰る。（依頼主：リスナー／報酬：［視聴回数］を10倍）",
    "全員がスマホ以外の［アイテム］を１個だけ持ち込んで生還する。（依頼主：リスナー／報酬：［視聴回数］を10倍）",
    "すべての［回収品］を集める。（依頼主：専門家／報酬：５Ｌ）",
];

/// Ruby `DiceTable::Table.new("追加目標表", "1D6", …)`。
static ADDITIONAL_TARGET_TABLE: Table =
    Table::from_dice("追加目標表", 1, 6, ADDITIONAL_TARGET_TABLE_ITEMS);

/// Ruby `TABLES["JIBAKUHAUNTA"]`（地縛霊：霊障リスト）の項目。
static JIBAKU_HAUNT_A_ITEMS: &[&str] = &[
    "隙間――家具の隙間、扉の隙間、そんな暗がりから視線を感じる。",
    "腐臭――吐き気を催すような、下水に似た臭いが漂ってくる。",
    "吐息――「ハァ……」耳元に、やけに湿った吐息が吹きかけられる。",
    "足音――立ち止まる度に、ひとつ多く足音が響く。誰か、いる……？",
    "背後――振り向いても、そこには誰もいない。それなのに、ずっと後ろに気配を感じる。",
    "鏡――鏡に背を向けた瞬間、あり得ない強さでそちらへ引き寄せられた。肩には手の形のアザができている。",
];

/// Ruby `DiceTable::Table.new("地縛霊：霊障リスト", "1D6", …)`。
static JIBAKU_HAUNT_A: Table = Table::from_dice("地縛霊：霊障リスト", 1, 6, JIBAKU_HAUNT_A_ITEMS);

/// Ruby `TABLES["JIBAKUHAUNTB"]`（地縛霊：霊障効果リスト）の項目。
static JIBAKU_HAUNT_B_ITEMS: &[&str] = &[
    "［精神力］減少：［１Ｄ２＋ＰＣ人数］点／［視聴回数］増加：とくになし／特殊効果：とくになし",
    "［精神力］減少：［１Ｄ４＋ＰＣ人数］点／［視聴回数］増加：とくになし／特殊効果：とくになし",
    "［精神力］減少：［１Ｄ６＋ＰＣ人数］点／［視聴回数］増加：２倍／特殊効果：とくになし",
    "［精神力］減少：［１Ｄ10＋ＰＣ人数］点／［視聴回数］増加：３倍／特殊効果：シーンに登場しているＰＣの［アイテム］を１つ破壊する。",
    "［精神力］減少：［１Ｄ20＋ＰＣ人数］点／［視聴回数］増加：５倍／特殊効果：シーンに登場しているＰＣのスマホを破壊する。",
    "［精神力］減少：［１Ｄ100＋ＰＣ人数］点／［視聴回数］増加：10倍／特殊効果：シーンに登場しているＰＣのスマホを破壊する。",
];

/// Ruby `DiceTable::Table.new("地縛霊：霊障効果リスト", "1D6", …)`。
static JIBAKU_HAUNT_B: Table =
    Table::from_dice("地縛霊：霊障効果リスト", 1, 6, JIBAKU_HAUNT_B_ITEMS);

/// Ruby `TABLES["SHYHAUNTA"]`（シャイな幽霊：霊障リスト）の項目。
static SHY_HAUNT_A_ITEMS: &[&str] = &[
    "倦怠感――歩くのも辛いくらいの倦怠感。生きているのも辛い。",
    "ラップ音――弾けるような、叩くような音が連続して聞こえる。",
    "空飛ぶ皿――棚に収まっていた食器が、不意に飛び出し、けたたましい音を立てて砕けていく。",
    "頭痛――頭が、割れそうに痛い。小さな物音ですら頭に響いてくる。",
    "点滅――灯りが明滅する。……あれ、ここ電気通ってたっけ？",
    "血文字――壁に、床に、赤⿊い液体が滲み出す。それは文字を形作った。「か　え　れ」",
];

/// Ruby `DiceTable::Table.new("シャイな幽霊：霊障リスト", "1D6", …)`。
static SHY_HAUNT_A: Table = Table::from_dice("シャイな幽霊：霊障リスト", 1, 6, SHY_HAUNT_A_ITEMS);

/// Ruby `TABLES["SHYHAUNTB"]`（シャイな幽霊：霊障効果リスト）の項目。
static SHY_HAUNT_B_ITEMS: &[&str] = &[
    "［精神力］減少：［２＋ＰＣ人数］点／［視聴回数］増加：とくになし／特殊効果：とくになし",
    "［精神力］減少：［４＋ＰＣ人数］点／［視聴回数］増加：２倍／特殊効果：シーンに登場しているＰＣがふたりの場合、追加で［精神力］を２減少させる。",
    "［精神力］減少：［６＋ＰＣ人数］点／［視聴回数］増加：３倍／特殊効果：シーンに登場しているＰＣがひとりの場合、追加で［精神力］を４減少させる。",
    "［精神力］減少：［10＋ＰＣ人数］点／［視聴回数］増加：５倍／特殊効果：シーンに登場しているＰＣがふたりの場合、追加で［精神力］を６減少させる。",
    "［精神力］減少：［20＋ＰＣ人数］点／［視聴回数］増加：10倍／特殊効果：シーンに登場しているＰＣがひとりの場合、追加で［精神力］を２減少させる。",
    "［精神力］減少：［40＋ＰＣ人数］点／［視聴回数］増加：20倍／特殊効果：シーンに登場しているＰＣのスマホを破壊する。",
];

/// Ruby `DiceTable::Table.new("シャイな幽霊：霊障効果リスト", "1D6", …)`。
static SHY_HAUNT_B: Table =
    Table::from_dice("シャイな幽霊：霊障効果リスト", 1, 6, SHY_HAUNT_B_ITEMS);

/// Ruby `TABLES["GUCHAHAUNTA"]`（ぐちゃぐちゃ：霊障リスト）の項目。
static GUCHA_HAUNT_A_ITEMS: &[&str] = &[
    "走る人形――ひび割れた人形が落ちている。一瞬視線をそらした瞬間、それはありえない動きで走り去っていった。",
    "血痕――天井から血が滴ってくる。その量は、おおよそ人一人分……いや、それ以上だ。",
    "着信――スマホの着信音が鳴る。こんな時に誰が――表示されていたのは、死んだはずの知り合いの名前だった。",
    "自分に似た他人――自分にそっくりな人が目の前に立っていた、気がする。",
    "衝撃――誰かに思いっきり押された気がしたのに誰もいない。",
    "記憶がない――数分間のことを何も覚えてない。コメント欄がリスナーの心配する声でいっぱいだ。いったい何が……？",
];

/// Ruby `DiceTable::Table.new("ぐちゃぐちゃ：霊障リスト", "1D6", …)`。
static GUCHA_HAUNT_A: Table =
    Table::from_dice("ぐちゃぐちゃ：霊障リスト", 1, 6, GUCHA_HAUNT_A_ITEMS);

/// Ruby `TABLES["GUCHAHAUNTB"]`（ぐちゃぐちゃ：霊障効果リスト）の項目。
static GUCHA_HAUNT_B_ITEMS: &[&str] = &[
    "［精神力］減少：［５＋ＰＣ人数］点／［視聴回数］増加：２倍／特殊効果：とくになし",
    "［精神力］減少：［10＋ＰＣ人数］点／［視聴回数］増加：３倍／特殊効果：とくになし",
    "［精神力］減少：［２Ｄ10＋ＰＣ人数］点／［視聴回数］増加：４倍／特殊効果：シーンに登場しているＰＣがふたりの場合、追加で［精神力］を５減少させる。",
    "［精神力］減少：［３Ｄ10＋ＰＣ人数］点／［視聴回数］増加：５倍／特殊効果：シーンに登場しているＰＣがひとりの場合、［アイテム］をランダムに１つ壊す。",
    "［精神力］減少：［１Ｄ100＋ＰＣ人数］点／［視聴回数］増加：10倍／特殊効果：シーンに登場しているＰＣのスマホを破壊する。",
    "［精神力］減少：［１Ｄ100＋10＋ＰＣ人数］点／［視聴回数］増加：20倍／特殊効果：すべてのＰＣのスマホを破壊する。",
];

/// Ruby `DiceTable::Table.new("ぐちゃぐちゃ：霊障効果リスト", "1D6", …)`。
static GUCHA_HAUNT_B: Table =
    Table::from_dice("ぐちゃぐちゃ：霊障効果リスト", 1, 6, GUCHA_HAUNT_B_ITEMS);

/// Ruby `TABLES`（キーは `transform_keys(&:upcase)` 済み）。
static TABLES: &[(&str, &Table)] = &[
    ("ADDITIONALTARGETTABLE", &ADDITIONAL_TARGET_TABLE),
    ("JIBAKUHAUNTA", &JIBAKU_HAUNT_A),
    ("JIBAKUHAUNTB", &JIBAKU_HAUNT_B),
    ("SHYHAUNTA", &SHY_HAUNT_A),
    ("SHYHAUNTB", &SHY_HAUNT_B),
    ("GUCHAHAUNTA", &GUCHA_HAUNT_A),
    ("GUCHAHAUNTB", &GUCHA_HAUNT_B),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GhostLive",
            "GhostLive.toml",
            15,
        );
    }
}
