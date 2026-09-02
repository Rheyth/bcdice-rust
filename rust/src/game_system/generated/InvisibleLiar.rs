//! P4で手書き移植した `lib/bcdice/game_system/InvisibleLiar.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `InvisibleLiar#eval_game_system_specific_command` → `roll_tables(command, TABLES)`
//! - `TABLES`（採取表 8地形 × 採取時間 1〜5 の40表。すべて `1D6`）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない。

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// 採取表を1つ定義する（`Table.new(name, "1D6", items)` 相当）。
macro_rules! gather_table {
    ($ident:ident, $name:literal, [$($item:literal),+ $(,)?]) => {
        static $ident: Table = Table::from_dice($name, 1, 6, &[$($item),+]);
    };
}

gather_table!(
    WOODS1,
    "森 1時間",
    [
        "麗し草",
        "麗し草",
        "麗し草",
        "神秘のキノコ",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    WOODS2,
    "森 2時間",
    [
        "麗し草",
        "麗し草",
        "神秘のキノコ",
        "神秘のキノコ",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    WOODS3,
    "森 3時間",
    [
        "麗し草",
        "神秘のキノコ",
        "神秘のキノコ",
        "神秘のキノコ",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    WOODS4,
    "森 4時間",
    [
        "麗し草",
        "神秘のキノコ",
        "神秘のキノコ",
        "妖精の羽",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    WOODS5,
    "森 5時間",
    [
        "麗し草",
        "神秘のキノコ",
        "妖精の羽",
        "妖精の羽",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    PRAIRIE1,
    "草原 1時間",
    [
        "太陽の花",
        "太陽の花",
        "太陽の花",
        "生命の虫",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    PRAIRIE2,
    "草原 2時間",
    [
        "太陽の花",
        "太陽の花",
        "生命の虫",
        "生命の虫",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    PRAIRIE3,
    "草原 3時間",
    [
        "太陽の花",
        "生命の虫",
        "生命の虫",
        "生命の虫",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    PRAIRIE4,
    "草原 4時間",
    [
        "太陽の花",
        "生命の虫",
        "生命の虫",
        "妖精の羽",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    PRAIRIE5,
    "草原 5時間",
    [
        "太陽の花",
        "生命の虫",
        "妖精の羽",
        "妖精の羽",
        "妖精の羽",
        "成果なし"
    ]
);
gather_table!(
    LAKE1,
    "湖 1時間",
    [
        "竜の鱗",
        "竜の鱗",
        "竜の鱗",
        "マンドレイク",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    LAKE2,
    "湖 2時間",
    [
        "竜の鱗",
        "竜の鱗",
        "マンドレイク",
        "マンドレイク",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    LAKE3,
    "湖 3時間",
    [
        "竜の鱗",
        "マンドレイク",
        "マンドレイク",
        "マンドレイク",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    LAKE4,
    "湖 4時間",
    [
        "竜の鱗",
        "マンドレイク",
        "マンドレイク",
        "幻獣の涙",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    LAKE5,
    "湖 5時間",
    [
        "竜の鱗",
        "マンドレイク",
        "幻獣の涙",
        "幻獣の涙",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    RIVER1,
    "川辺 1時間",
    ["魔魚", "魔魚", "魔魚", "麗し草", "幻獣の涙", "成果なし"]
);
gather_table!(
    RIVER2,
    "川辺 2時間",
    ["魔魚", "魔魚", "麗し草", "麗し草", "幻獣の涙", "成果なし"]
);
gather_table!(
    RIVER3,
    "川辺 3時間",
    ["魔魚", "麗し草", "麗し草", "麗し草", "幻獣の涙", "成果なし"]
);
gather_table!(
    RIVER4,
    "川辺 4時間",
    [
        "魔魚",
        "麗し草",
        "麗し草",
        "幻獣の涙",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    RIVER5,
    "川辺 5時間",
    [
        "魔魚",
        "麗し草",
        "幻獣の涙",
        "幻獣の涙",
        "幻獣の涙",
        "成果なし"
    ]
);
gather_table!(
    SWAMP1,
    "沼地 1時間",
    [
        "マンドレイク",
        "マンドレイク",
        "マンドレイク",
        "魔魚",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    SWAMP2,
    "沼地 2時間",
    [
        "マンドレイク",
        "マンドレイク",
        "魔魚",
        "魔魚",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    SWAMP3,
    "沼地 3時間",
    [
        "マンドレイク",
        "魔魚",
        "魔魚",
        "魔魚",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    SWAMP4,
    "沼地 4時間",
    [
        "マンドレイク",
        "魔魚",
        "魔魚",
        "畏怖の化石",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    SWAMP5,
    "沼地 5時間",
    [
        "マンドレイク",
        "魔魚",
        "畏怖の化石",
        "畏怖の化石",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    CAVE1,
    "洞窟 1時間",
    [
        "神秘のキノコ",
        "神秘のキノコ",
        "神秘のキノコ",
        "魔素の結晶",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    CAVE2,
    "洞窟 2時間",
    [
        "神秘のキノコ",
        "神秘のキノコ",
        "魔素の結晶",
        "魔素の結晶",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    CAVE3,
    "洞窟 3時間",
    [
        "神秘のキノコ",
        "魔素の結晶",
        "魔素の結晶",
        "魔素の結晶",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    CAVE4,
    "洞窟 4時間",
    [
        "神秘のキノコ",
        "魔素の結晶",
        "魔素の結晶",
        "畏怖の化石",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    CAVE5,
    "洞窟 5時間",
    [
        "神秘のキノコ",
        "魔素の結晶",
        "畏怖の化石",
        "畏怖の化石",
        "畏怖の化石",
        "成果なし"
    ]
);
gather_table!(
    ROCKY1,
    "岩場 1時間",
    [
        "生命の虫",
        "生命の虫",
        "生命の虫",
        "竜の鱗",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    ROCKY2,
    "岩場 2時間",
    [
        "生命の虫",
        "生命の虫",
        "竜の鱗",
        "竜の鱗",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    ROCKY3,
    "岩場 3時間",
    [
        "生命の虫",
        "竜の鱗",
        "竜の鱗",
        "竜の鱗",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    ROCKY4,
    "岩場 4時間",
    [
        "生命の虫",
        "竜の鱗",
        "竜の鱗",
        "星の欠片",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    ROCKY5,
    "岩場 5時間",
    [
        "生命の虫",
        "竜の鱗",
        "星の欠片",
        "星の欠片",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    MOUNTAIN1,
    "山岳 1時間",
    [
        "魔素の結晶",
        "魔素の結晶",
        "魔素の結晶",
        "太陽の花",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    MOUNTAIN2,
    "山岳 2時間",
    [
        "魔素の結晶",
        "魔素の結晶",
        "太陽の花",
        "太陽の花",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    MOUNTAIN3,
    "山岳 3時間",
    [
        "魔素の結晶",
        "太陽の花",
        "太陽の花",
        "太陽の花",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    MOUNTAIN4,
    "山岳 4時間",
    [
        "魔素の結晶",
        "太陽の花",
        "太陽の花",
        "星の欠片",
        "星の欠片",
        "成果なし"
    ]
);
gather_table!(
    MOUNTAIN5,
    "山岳 5時間",
    [
        "魔素の結晶",
        "太陽の花",
        "星の欠片",
        "星の欠片",
        "星の欠片",
        "成果なし"
    ]
);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[
    ("WOODS1", &WOODS1),
    ("WOODS2", &WOODS2),
    ("WOODS3", &WOODS3),
    ("WOODS4", &WOODS4),
    ("WOODS5", &WOODS5),
    ("PRAIRIE1", &PRAIRIE1),
    ("PRAIRIE2", &PRAIRIE2),
    ("PRAIRIE3", &PRAIRIE3),
    ("PRAIRIE4", &PRAIRIE4),
    ("PRAIRIE5", &PRAIRIE5),
    ("LAKE1", &LAKE1),
    ("LAKE2", &LAKE2),
    ("LAKE3", &LAKE3),
    ("LAKE4", &LAKE4),
    ("LAKE5", &LAKE5),
    ("RIVER1", &RIVER1),
    ("RIVER2", &RIVER2),
    ("RIVER3", &RIVER3),
    ("RIVER4", &RIVER4),
    ("RIVER5", &RIVER5),
    ("SWAMP1", &SWAMP1),
    ("SWAMP2", &SWAMP2),
    ("SWAMP3", &SWAMP3),
    ("SWAMP4", &SWAMP4),
    ("SWAMP5", &SWAMP5),
    ("CAVE1", &CAVE1),
    ("CAVE2", &CAVE2),
    ("CAVE3", &CAVE3),
    ("CAVE4", &CAVE4),
    ("CAVE5", &CAVE5),
    ("ROCKY1", &ROCKY1),
    ("ROCKY2", &ROCKY2),
    ("ROCKY3", &ROCKY3),
    ("ROCKY4", &ROCKY4),
    ("ROCKY5", &ROCKY5),
    ("MOUNTAIN1", &MOUNTAIN1),
    ("MOUNTAIN2", &MOUNTAIN2),
    ("MOUNTAIN3", &MOUNTAIN3),
    ("MOUNTAIN4", &MOUNTAIN4),
    ("MOUNTAIN5", &MOUNTAIN5),
];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `BCDice::GameSystem::InvisibleLiar`（ID: `InvisibleLiar`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvisibleLiar;

impl GameSystem for InvisibleLiar {
    fn id(&self) -> &'static str {
        "InvisibleLiar"
    }

    fn name(&self) -> &'static str {
        "インビジブルライアー"
    }

    fn sort_key(&self) -> &'static str {
        "いんひしふるらいああ"
    }

    fn help_message(&self) -> &'static str {
        r"■ 採取表
WOODSn 森
PRAIRIEn 草原
LAKEn 湖
RIVERn 川辺
SWAMPn 沼地
CAVEn 洞窟
ROCKYn 岩場
MOUNTAINn 山岳
  n: 採取時間（1〜5）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "WOODS1",
            "WOODS2",
            "WOODS3",
            "WOODS4",
            "WOODS5",
            "PRAIRIE1",
            "PRAIRIE2",
            "PRAIRIE3",
            "PRAIRIE4",
            "PRAIRIE5",
            "LAKE1",
            "LAKE2",
            "LAKE3",
            "LAKE4",
            "LAKE5",
            "RIVER1",
            "RIVER2",
            "RIVER3",
            "RIVER4",
            "RIVER5",
            "SWAMP1",
            "SWAMP2",
            "SWAMP3",
            "SWAMP4",
            "SWAMP5",
            "CAVE1",
            "CAVE2",
            "CAVE3",
            "CAVE4",
            "CAVE5",
            "ROCKY1",
            "ROCKY2",
            "ROCKY3",
            "ROCKY4",
            "ROCKY5",
            "MOUNTAIN1",
            "MOUNTAIN2",
            "MOUNTAIN3",
            "MOUNTAIN4",
            "MOUNTAIN5",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `InvisibleLiar#eval_game_system_specific_command`。
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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "InvisibleLiar",
            "InvisibleLiar.toml",
            41,
        );
    }
}
