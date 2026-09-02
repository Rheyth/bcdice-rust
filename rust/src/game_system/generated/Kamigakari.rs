//! P4で手書き移植した `lib/bcdice/game_system/Kamigakari.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Kamigakari#eval_game_system_specific_command`（`MTx` 獲得素材チャート / 各種表）
//! - `getGetMaterialTableResult` / `getMaterialEffect` / `getMaterialEffectNomal`
//!   / `getMaterialEffectPower` / `getMaterialEffectRare` / `getAttribute` / `getPrice`
//!
//! 表データは `i18n/Kamigakari/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `Kamigakari_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static JA_RT_ITEMS: &[&str] = &[
    "邪神化：物理法則を超過しすぎた代償として、霊魂そのものが歪み、PCは即座にアラミタマへと変貌する。アラミタマ化したPCは、いずこかへと消え去る。",
    "存在消滅：アラミタマ化を最後の力で抑え込む。だがその結果、PCの霊魂は燃え尽きてしまい、この世界から消滅する。そのPCは[状態変化：死亡]となり死体も残らない。",
    "死亡：霊魂の歪みをかろうじて食い止めるが、霊魂が崩壊する。PCは[状態変化：死亡]となるが遺体は残る。",
    "霊魂半壊：霊魂の歪みを食い止めるものの、霊魂そのものに致命的な負傷を受け、全身に障害が残る。それに伴って霊紋も消滅し、一般人へと戻る。",
    "記憶消滅：奇跡的に霊魂の摩耗による身体的な悪影響を免れる。時間を置くことで霊紋も回復するが、精神的に影響を受け、すべての記憶を失ってしまう。",
    "影響なし：奇跡的に、霊魂の摩耗による悪影響を完全に退け、さらに霊紋の回復も早期を見込める。肉体や精神にも、特に影響はない。",
];
static JA_RT: Table = Table::from_dice("霊紋消費の代償表", 1, 6, JA_RT_ITEMS);

static JA_ET_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("運命/そのキャラクターに、運命的、あるいは宿命的なものを感じている。")),
    (12, TableItem::Text("運命/そのキャラクターに、運命的、あるいは宿命的なものを感じている。")),
    (13, TableItem::Text("家族/そのキャラクターに、家族のような親近感をいだいている。")),
    (14, TableItem::Text("家族/そのキャラクターに、家族のような親近感をいだいている。")),
    (15, TableItem::Text("腐れ縁/そのキャラクターに、腐れ縁を感じている。")),
    (16, TableItem::Text("腐れ縁/そのキャラクターに、腐れ縁を感じている。")),
    (21, TableItem::Text("師弟/そのキャラクターとは、まるで師弟のような関係だと感じている。どちらが弟子で、どちらが師匠かは相談して決定する。")),
    (22, TableItem::Text("師弟/そのキャラクターとは、まるで師弟のような関係だと感じている。どちらが弟子で、どちらが師匠かは相談して決定する。")),
    (23, TableItem::Text("好敵手/そのキャラクターを、好敵手だと感じている。")),
    (24, TableItem::Text("好敵手/そのキャラクターを、好敵手だと感じている。")),
    (25, TableItem::Text("親近感/そのキャラクターに、親近感をいだいている。")),
    (26, TableItem::Text("親近感/そのキャラクターに、親近感をいだいている。")),
    (31, TableItem::Text("誠意/そのキャラクターに、誠実さを感じている。")),
    (32, TableItem::Text("誠意/そのキャラクターに、誠実さを感じている。")),
    (33, TableItem::Text("友情/そのキャラクターに、友情をいだいている。")),
    (34, TableItem::Text("友情/そのキャラクターに、友情をいだいている。")),
    (35, TableItem::Text("尊敬/そのキャラクターに、尊敬をいだいている。")),
    (36, TableItem::Text("尊敬/そのキャラクターに、尊敬をいだいている。")),
    (41, TableItem::Text("庇護/そのキャラクターに、庇護の感情をいだいている。どちらが保護者で、どちらが被保護者かは相談して決定する。")),
    (42, TableItem::Text("庇護/そのキャラクターに、庇護の感情をいだいている。どちらが保護者で、どちらが被保護者かは相談して決定する。")),
    (43, TableItem::Text("好感/そのキャラクターに、好感をいだいている。")),
    (44, TableItem::Text("好感/そのキャラクターに、好感をいだいている。")),
    (45, TableItem::Text("興味/そのキャラクターに、興味をいだいている。")),
    (46, TableItem::Text("興味/そのキャラクターに、興味をいだいている。")),
    (51, TableItem::Text("感銘/そのキャラクターに、感銘をいだいている。")),
    (52, TableItem::Text("感銘/そのキャラクターに、感銘をいだいている。")),
    (53, TableItem::Text("畏怖/そのキャラクターに、畏怖をいだいている。")),
    (54, TableItem::Text("畏怖/そのキャラクターに、畏怖をいだいている。")),
    (55, TableItem::Text("お気に入り/そのキャラクターを、気に入っている。")),
    (56, TableItem::Text("お気に入り/そのキャラクターを、気に入っている。")),
    (61, TableItem::Text("愛情/そのキャラクターに愛情、またはそれに近い執着心をいだいている。")),
    (62, TableItem::Text("愛情/そのキャラクターに愛情、またはそれに近い執着心をいだいている。")),
    (63, TableItem::Text("信頼/そのキャラクターに、信頼を感じている。")),
    (64, TableItem::Text("信頼/そのキャラクターに、信頼を感じている。")),
    (65, TableItem::Text("＊PCの任意/プレイヤー、またはGMが設定した任意の感情をいだいている。")),
    (66, TableItem::Text("＊PCの任意/プレイヤー、またはGMが設定した任意の感情をいだいている。")),
];
static JA_ET: D66Table = D66Table::new("感情表", D66SortType::NoSort, JA_ET_ITEMS);

static JA_KT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("時空の捻じれ\n現在地の時空が捻じれ、PC全員は即時に[侵入エリア]へと戻る。")),
    (12, TableItem::Text("時空の捻じれ\n現在地の時空が捻じれ、PC全員は即時に[侵入エリア]へと戻る。")),
    (13, TableItem::Text("強敵登場\n突如、<崇り神>化した[モノノケ]が出撃する。GMは、PCの[世界干渉LV]の平均+3の[LV]を持つ任意の[モノノケ]を1体選び任意の[探索エリア]に配置。そこでは[迂回]不可で[戦闘]が発生する。")),
    (14, TableItem::Text("強敵登場\n突如、<崇り神>化した[モノノケ]が出撃する。GMは、PCの[世界干渉LV]の平均+3の[LV]を持つ任意の[モノノケ]を1体選び任意の[探索エリア]に配置。そこでは[迂回]不可で[戦闘]が発生する。")),
    (15, TableItem::Text("影の手\n瘴気で形成された無数の手がPC達を握りつぶそうとする。PC全員は[効果種別：魔法攻撃/距離：戦闘地帯/対象：戦闘地帯/達成値：20+PCの[世界干渉LV]の平均/魔法ダメージ：20×PCの[世界干渉LV]の平均/抵抗[半減]]を受ける。")),
    (16, TableItem::Text("影の手\n瘴気で形成された無数の手がPC達を握りつぶそうとする。PC全員は[効果種別：魔法攻撃/距離：戦闘地帯/対象：戦闘地帯/達成値：20+PCの[世界干渉LV]の平均/魔法ダメージ：20×PCの[世界干渉LV]の平均/抵抗[半減]]を受ける。")),
    (21, TableItem::Text("無数の邪眼\n空間全体に恐ろしい邪眼が出現する。PC全員は、[大休止]するまで[状態変化：暗闇・苦痛]となる。")),
    (22, TableItem::Text("無数の邪眼\n空間全体に恐ろしい邪眼が出現する。PC全員は、[大休止]するまで[状態変化：暗闇・苦痛]となる。")),
    (23, TableItem::Text("空間崩壊\n突如として、魔境の空間が崩壊する。PC全員は[効果種別：物理攻撃/距離：戦闘地帯/対象：戦闘地帯/達成値：30+PCの[世界干渉LV]の平均/物理ダメージ：30×PCの[世界干渉LV]の平均]]を受ける。")),
    (24, TableItem::Text("空間崩壊\n突如として、魔境の空間が崩壊する。PC全員は[効果種別：物理攻撃/距離：戦闘地帯/対象：戦闘地帯/達成値：30+PCの[世界干渉LV]の平均/物理ダメージ：30×PCの[世界干渉LV]の平均]]を受ける。")),
    (25, TableItem::Text("防具腐食\n周辺から異様な霧が立ち込め、防具を腐食する。PC全員は、[所持・装備]中の任意の[アイテム：防具]１つを失う。")),
    (26, TableItem::Text("防具腐食\n周辺から異様な霧が立ち込め、防具を腐食する。PC全員は、[所持・装備]中の任意の[アイテム：防具]１つを失う。")),
    (31, TableItem::Text("素材消失\n周囲から異様な光が零れ、所持中の[素材]を消失させる。PC全員が[所持]中の[素材]が、すべて消滅する。")),
    (32, TableItem::Text("素材消失\n周囲から異様な光が零れ、所持中の[素材]を消失させる。PC全員が[所持]中の[素材]が、すべて消滅する。")),
    (33, TableItem::Text("なし\n特に何も起こらない。")),
    (34, TableItem::Text("なし\n特に何も起こらない。")),
    (35, TableItem::Text("モノノケ強襲\n突如として<崇り神>化した[モノノケ]が出現し、PCたちに襲いかかる。GMはPCの[世界干渉LV]の平均+2の[LV]を持つ任意の[モノノケ]を2体選び、PC達の前に出現させ、即座に[戦闘]を開始する。")),
    (36, TableItem::Text("モノノケ強襲\n突如として<崇り神>化した[モノノケ]が出現し、PCたちに襲いかかる。GMはPCの[世界干渉LV]の平均+2の[LV]を持つ任意の[モノノケ]を2体選び、PC達の前に出現させ、即座に[戦闘]を開始する。")),
    (41, TableItem::Text("休息妨害\nPCが休息しようとするたびに、さまざまな空間から、触手や毒蠱などが出現して襲いかかってくる。PCたちは以降、[魔境討伐]が終了するまで[大休止]を行えない。")),
    (42, TableItem::Text("休息妨害\nPCが休息しようとするたびに、さまざまな空間から、触手や毒蠱などが出現して襲いかかってくる。PCたちは以降、[魔境討伐]が終了するまで[大休止]を行えない。")),
    (43, TableItem::Text("龍脈破壊\n霊力が暴走して空間が歪み、[霊力]が狂う。PC全員は即座に[霊力]をすべて振り直す。")),
    (44, TableItem::Text("龍脈破壊\n霊力が暴走して空間が歪み、[霊力]が狂う。PC全員は即座に[霊力]をすべて振り直す。")),
    (45, TableItem::Text("固有時間停止\nPCたちの肉体の一部が灰色と化し、動かなくなる。PC全員は[タイミング：準備・防御・特殊]から１つ選び、以後その[タイミング]を消費できなくなる。")),
    (46, TableItem::Text("固有時間停止\nPCたちの肉体の一部が灰色と化し、動かなくなる。PC全員は[タイミング：準備・防御・特殊]から１つ選び、以後その[タイミング]を消費できなくなる。")),
    (51, TableItem::Text("龍脈不順\n霊力が突如として混濁し、[霊力]の循環に悪影響が発生する。PC全員は以後、[魔境討伐]が終了するまで[霊力操作]が行えない。")),
    (52, TableItem::Text("龍脈不順\n霊力が突如として混濁し、[霊力]の循環に悪影響が発生する。PC全員は以後、[魔境討伐]が終了するまで[霊力操作]が行えない。")),
    (53, TableItem::Text("術技封印\n周囲の空気が変貌し、悪影響が起こる。PC全員は以後、修得済みの《タレント》中、使用する[コスト]が最も多いもの１つが[魔境討伐]終了まで使用不能となる。[コスト：なし]ばかりの場合、GMが任意で1つを決定する。")),
    (54, TableItem::Text("術技封印\n周囲の空気が変貌し、悪影響が起こる。PC全員は以後、修得済みの《タレント》中、使用する[コスト]が最も多いもの１つが[魔境討伐]終了まで使用不能となる。[コスト：なし]ばかりの場合、GMが任意で1つを決定する。")),
    (55, TableItem::Text("装飾品消滅\n周囲が青い光に包まれると、なぜかPCたちの装飾品が失われている。PC全員は[所持・装備中]の[アイテム・装飾]をすべて失う。")),
    (56, TableItem::Text("装飾品消滅\n周囲が青い光に包まれると、なぜかPCたちの装飾品が失われている。PC全員は[所持・装備中]の[アイテム・装飾]をすべて失う。")),
    (61, TableItem::Text("愚者の黄金消失\n周囲が赤い光に包まれると、なぜかPCたちの[G]が失われている。PC全員は、[所持金]が[半減]する。")),
    (62, TableItem::Text("愚者の黄金消失\n周囲が赤い光に包まれると、なぜかPCたちの[G]が失われている。PC全員は、[所持金]が[半減]する。")),
    (63, TableItem::Text("GMの任意\nこの表のなかから、GMが効果を1つ選んで発生させる。")),
    (64, TableItem::Text("GMの任意\nこの表のなかから、GMが効果を1つ選んで発生させる。")),
    (65, TableItem::Text("臨界重複\n[魔境臨界]が2回発生する。GMはこの表を2回振り、効果をそれぞれ適応できる。再び「臨界重複」が発生した場合、[GMの任意]１回として扱う。")),
    (66, TableItem::Text("臨界重複\n[魔境臨界]が2回発生する。GMはこの表を2回振り、効果をそれぞれ適応できる。再び「臨界重複」が発生した場合、[GMの任意]１回として扱う。")),
];
static JA_KT: D66Table = D66Table::new("魔境臨界表", D66SortType::NoSort, JA_KT_ITEMS);

static JA_NT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("御剣（みつるぎ）　陸/凛")),
    (12, TableItem::Text("獅子内（ししうち）　大和/楓")),
    (13, TableItem::Text("白銀（はくぎん）　隼人/桜")),
    (14, TableItem::Text("竹内（たけのうち）　真/遥")),
    (15, TableItem::Text("古太刀（こだち）　大地/美咲")),
    (16, TableItem::Text("空閑（くが）　俊/真央")),
    (21, TableItem::Text("鬼形（おにがた）　諒/舞")),
    (22, TableItem::Text("御巫（みかんなぎ）　匠/七海")),
    (23, TableItem::Text("護摩堂（ごまどう）　仁/千尋")),
    (24, TableItem::Text("龍円（りゅうえん）　拓真/茜")),
    (25, TableItem::Text("鏡部（かがみべ）　京/明日香")),
    (26, TableItem::Text("犬神（いぬがみ）　剛/栞")),
    (31, TableItem::Text("明月院（めいげついん）　葵/唯")),
    (32, TableItem::Text("百目鬼（どうめき）　蓮也/萌")),
    (33, TableItem::Text("恐神（おそがみ）　達也/綾香")),
    (34, TableItem::Text("蘭（あららぎ）　龍之介/梓")),
    (35, TableItem::Text("珠輝（たまき）　章/瞳")),
    (36, TableItem::Text("眼龍（がんりゅう）　圭/沙織")),
    (41, TableItem::Text("鉄砲塚（てっぽうづか）　雅人/沙良")),
    (42, TableItem::Text("檻神（おりがみ）　直哉/弥生")),
    (43, TableItem::Text("不死原（ふじわら）　純/千秋")),
    (44, TableItem::Text("九郎座（くろうざ）　武蔵/春菜")),
    (45, TableItem::Text("土御門（つちみかど）　亮介/翠")),
    (46, TableItem::Text("十六夜（いざよい）　啓二/双葉")),
    (51, TableItem::Text("転法輪（てんぽうりん）　英雄/麗菜")),
    (52, TableItem::Text("執行（しぎょう）　響/小百合")),
    (53, TableItem::Text("祝（ほうり）　良太郎/陽奈")),
    (54, TableItem::Text("神尊（こうそ）　智/紫苑")),
    (55, TableItem::Text("芦屋（あしや）　孝之/香澄")),
    (56, TableItem::Text("七社（ななしゃ）　克己/風香")),
    (61, TableItem::Text("騎馬（きば）　哲也/詩乃")),
    (62, TableItem::Text("当麻（とうま）　玄/沙耶")),
    (63, TableItem::Text("狐塚（きつねづか）　北斗/麻耶")),
    (64, TableItem::Text("天神林（てんじんばやし）　空/晶")),
    (65, TableItem::Text("明嵐（めあらし）　八雲/乙葉")),
    (66, TableItem::Text("草壁（くさかべ）　大悟/文")),
];
static JA_NT: D66Table = D66Table::new("伝奇名字・名前決定表", D66SortType::NoSort, JA_NT_ITEMS);

static JA_MT_ITEMS: &[&str] = &[
    "真紅の断片",
    "ざらつく断片",
    "紺碧の断片",
    "鋭い断片",
    "黄金の断片",
    "柔らかな断片",
    "銀色の断片",
    "尖った断片",
    "純白の断片",
    "硬い断片",
    "漆黒の断片",
    "輝く断片",
    "なめらかな断片",
    "濁った断片",
    "ふさふさの断片",
    "邪悪な断片",
    "粘つく断片",
    "聖なる断片",
    "灼熱の断片",
    "炎の断片",
    "氷結の断片",
    "氷の断片",
    "熱い断片",
    "風の断片",
    "冷たい断片",
    "雷の断片",
    "土の断片",
    "幻の断片",
    "骨状の断片",
    "刻印の断片",
    "牙状の断片",
    "鱗状の断片",
    "石状の断片",
    "宝石状の断片",
    "毛皮状の断片",
    "羽根状の断片",
];

static JA_COMMON_ITEMS: &[(i64, &str)] = &[
    (13, "体力+n"),
    (16, "敏捷+n"),
    (23, "知性+n"),
    (26, "精神+n"),
    (33, "幸運+n"),
    (35, "物D+n"),
    (41, "魔D+n"),
    (43, "行動+n"),
    (46, "生命+n×3"),
    (53, "装甲+n"),
    (56, "結界+n"),
    (63, "移動+nマス"),
    (66, "※PCの任意"),
];

static JA_ATTRIBUTE: &[(i64, &str)] = &[
    (21, "［火炎］"),
    (33, "［冷気］"),
    (43, "［電撃］"),
    (53, "［風圧］"),
    (56, "［幻覚］"),
    (62, "［魔毒］"),
    (64, "［磁力］"),
    (66, "［閃光］"),
];

/// Ruby `TABLES`（`translate_tables(:ja_jp)`）。
pub(crate) static JA_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("RT", &JA_RT),
    ("ET", &JA_ET),
    ("KT", &JA_KT),
    ("NT", &JA_NT),
];

/// 1ロケール分の表と定型文。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`
    pub(crate) tables: &'static [(&'static str, &'static dyn RollableTable)],
    /// i18n `Kamigakari.MT.name`
    pub(crate) mt_name: &'static str,
    /// i18n `Kamigakari.MT.items`（D66 の 36 項目）
    pub(crate) mt_items: &'static [&'static str],
    /// i18n `Kamigakari.MT.result_format`（`%{material}` / `%{effect}`）
    pub(crate) result_format: &'static str,
    /// i18n `Kamigakari.MT.common_material.name`
    pub(crate) common_name: &'static str,
    /// i18n `Kamigakari.MT.common_material.items`（`get_table_by_number` 用）
    pub(crate) common_items: &'static [(i64, &'static str)],
    /// i18n `Kamigakari.MT.rare_material.name`
    pub(crate) rare_name: &'static str,
    /// i18n `Kamigakari.MT.rare_material.give_attribute`
    pub(crate) give_attribute: &'static str,
    /// i18n `Kamigakari.MT.rare_material.halve_damage`
    pub(crate) halve_damage: &'static str,
    /// i18n `Kamigakari.MT.rare_material.optional_by_GM`
    pub(crate) optional_by_gm: &'static str,
    /// i18n `Kamigakari.MT.attribute`（`get_table_by_number` 用）
    pub(crate) attribute: &'static [(i64, &'static str)],
    /// i18n `Kamigakari.MT.effect_power`
    pub(crate) effect_power: &'static str,
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    mt_name: "獲得素材チャート",
    mt_items: JA_MT_ITEMS,
    result_format: "%{material}。%{effect}",
    common_name: "よく見つかる素材",
    common_items: JA_COMMON_ITEMS,
    rare_name: "珍しい素材",
    give_attribute: "付与",
    halve_damage: "半減",
    optional_by_gm: "※GMの任意",
    attribute: JA_ATTRIBUTE,
    effect_power: "効果値",
};

/// Ruby `getMaterialEffectPower` の表。`(強度の上限, 1D6 → 効果値)`。
static POWER_TABLE: &[(i64, &[&str])] = &[
    (4, &["1", "1", "1", "2", "2", "3"]),
    (8, &["1", "1", "2", "2", "3", "3"]),
    (9, &["1", "2", "3", "3", "4", "5"]),
];

/// Ruby `/^MT(\d*)$/`（`command.upcase` に対して適用する）。
fn mt_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^MT(\d*)$").expect("valid regex"))
}

/// Ruby `getPrice` の `/\+(\d+)/`。
fn plus_number_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\+(\d+)").expect("valid regex"))
}

/// Ruby `Base#get_table_by_number`。最初に `number >= index` となる項目（無ければ既定の `"1"`）。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    table
        .iter()
        .find(|(number, _)| *number >= index)
        .map_or("1", |(_, value)| *value)
}

/// Ruby `Base#get_table_by_d66`。1D6 を2回振り `(dice1 - 1) * 6 + (dice2 - 1)` 番目を引く。
///
/// 返り値は `(項目, 出目の文字列 "dice1dice2")`。項目が無ければ `"1"`。
fn get_table_by_d66(
    table: &[&'static str],
    rng: &mut Randomizer,
) -> Result<(&'static str, String), EvalError> {
    let dice1 = rng.roll_once(6)?;
    let dice2 = rng.roll_once(6)?;
    let num = (dice1 - 1) * 6 + (dice2 - 1);
    let index_text = format!("{dice1}{dice2}");
    let text = usize::try_from(num)
        .ok()
        .and_then(|i| table.get(i).copied())
        .unwrap_or("1");
    Ok((text, index_text))
}

/// Ruby `Base#get_table_by_1d6`。返り値は `(項目, 出目)`。項目が無ければ `("1", 0)`。
fn get_table_by_1d6(
    table: &[&'static str],
    rng: &mut Randomizer,
) -> Result<(&'static str, i64), EvalError> {
    let num = rng.roll_sum(1, 6)?;
    match usize::try_from(num - 1)
        .ok()
        .and_then(|i| table.get(i).copied())
    {
        Some(text) => Ok((text, num)),
        None => Ok(("1", 0)),
    }
}

/// Ruby `Kamigakari#getGetMaterialTableResult`。返り値は `(結果, 出目の列)`。
fn get_material_table_result(
    sys: &SystemTables,
    rank: i64,
    rng: &mut Randomizer,
) -> Result<(String, String), EvalError> {
    let (material, number) = get_table_by_d66(sys.mt_items, rng)?;
    let (effect, number2) = get_material_effect(sys, rank, rng)?;
    let number = format!("{number},{number2}");

    let price = get_price(sys, &effect);

    let mut result = sys
        .result_format
        .replace("%{material}", material)
        .replace("%{effect}", &effect);
    if let Some(price) = price {
        result.push('：');
        result.push_str(&price);
    }

    Ok((result, number))
}

/// Ruby `Kamigakari#getMaterialEffect`。返り値は `(種別：効果, 出目の列)`。
fn get_material_effect(
    sys: &SystemTables,
    rank: i64,
    rng: &mut Randomizer,
) -> Result<(String, String), EvalError> {
    let number = rng.roll_once(6)?;

    let (result, number2, kind) = if number < 6 {
        let (result, number2) = get_material_effect_normal(sys, rank, rng)?;
        (result, number2, sys.common_name)
    } else {
        let (result, number2) = get_material_effect_rare(sys, rng)?;
        (result, number2, sys.rare_name)
    };

    Ok((format!("{kind}：{result}"), format!("{number},{number2}")))
}

/// Ruby `Kamigakari#getMaterialEffectNomal`。
fn get_material_effect_normal(
    sys: &SystemTables,
    rank: i64,
    rng: &mut Randomizer,
) -> Result<(String, String), EvalError> {
    let number = rng.roll_d66(D66SortType::NoSort)?;
    let result = get_table_by_number(number, sys.common_items);

    // Ruby: if result =~ /\+n/ → result.sub(/\+n/, "+#{power}")
    if result.contains("+n") {
        let (power, number2) = get_material_effect_power(rank, rng)?;
        Ok((
            result.replacen("+n", &format!("+{power}"), 1),
            format!("{number},{number2}"),
        ))
    } else {
        Ok((result.to_owned(), number.to_string()))
    }
}

/// Ruby `Kamigakari#getMaterialEffectPower`。返り値は `(効果値, 1D6 の出目)`。
fn get_material_effect_power(
    rank: i64,
    rng: &mut Randomizer,
) -> Result<(&'static str, i64), EvalError> {
    // Ruby: rank = 9 if rank > 9
    let rank = rank.min(9);
    // Ruby: get_table_by_number(rank, table)（強度 0〜9 は必ずいずれかの行に当たる）
    let rank_table = match POWER_TABLE.iter().find(|(limit, _)| *limit >= rank) {
        Some((_, table)) => *table,
        None => &[],
    };
    get_table_by_1d6(rank_table, rng)
}

/// Ruby `Kamigakari#getMaterialEffectRare`。
fn get_material_effect_rare(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(String, String), EvalError> {
    // Ruby: [[3, "**付与"], [5, "**半減"], [6, "※GMの任意"]]
    let table = [
        (3, format!("**{}", sys.give_attribute)),
        (5, format!("**{}", sys.halve_damage)),
        (6, sys.optional_by_gm.to_owned()),
    ];

    let number = rng.roll_once(6)?;
    let result = table
        .iter()
        .find(|(limit, _)| *limit >= number)
        .map_or("1", |(_, value)| value.as_str());

    // Ruby: if result.include?("**") → result.sub("**", attribute.to_s)
    if result.contains("**") {
        let (attribute, number2) = get_attribute(sys, rng)?;
        Ok((
            result.replacen("**", attribute, 1),
            format!("{number},{number2}"),
        ))
    } else {
        Ok((result.to_owned(), number.to_string()))
    }
}

/// Ruby `Kamigakari#getAttribute`。返り値は `(属性, D66 の出目)`。
fn get_attribute(
    sys: &SystemTables,
    rng: &mut Randomizer,
) -> Result<(&'static str, i64), EvalError> {
    let number = rng.roll_d66(D66SortType::NoSort)?;
    Ok((get_table_by_number(number, sys.attribute), number))
}

/// Ruby `Kamigakari#getPrice`。効果値に応じた価格。効果値 0 や表の範囲外は `nil`。
fn get_price(sys: &SystemTables, effect: &str) -> Option<String> {
    let power: usize = if let Some(caps) = plus_number_pattern().captures(effect) {
        // Ruby: m[1].to_i（表の範囲外は table[power] が nil になる）
        caps[1].parse().ok()?
    } else if effect.contains(sys.give_attribute) {
        3
    } else if effect.contains(sys.halve_damage) {
        4
    } else {
        0
    };

    let gold = match power {
        1 => "500G",
        2 => "1000G",
        3 => "1500G",
        4 => "2000G",
        5 => "3000G",
        _ => return None,
    };
    Some(format!("{gold}({}:{power})", sys.effect_power))
}

/// Ruby `Kamigakari#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(caps) = mt_pattern().captures(command) else {
        return Ok(
            table_helpers::roll_table(command, sys.tables, rng)?.map(SpecificCommandOutput::text)
        );
    };

    // Ruby: rank = Regexp.last_match(1); rank ||= 1; rank = rank.to_i
    // `(\d*)` は空文字列にも一致するので nil にはならず、`MT` 単独は `"".to_i == 0`
    // （強度1と同じ行に落ちる）。9 を超える値は getMaterialEffectPower 側で 9 に丸める。
    let rank_text = &caps[1];
    let rank = if rank_text.is_empty() {
        0
    } else {
        rank_text.parse::<i64>().unwrap_or(i64::MAX)
    };

    let (result, number) = get_material_table_result(sys, rank, rng)?;
    // Ruby: return "" if result.empty?（`dice_command` が nil に畳む）
    if result.is_empty() {
        return Ok(Some(SpecificCommandOutput::text("")));
    }

    Ok(Some(SpecificCommandOutput::text(format!(
        "{}({number}) ＞ {result}",
        sys.mt_name
    ))))
}

/// Ruby `BCDice::GameSystem::Kamigakari`（ID: `Kamigakari`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kamigakari;

impl GameSystem for Kamigakari {
    fn id(&self) -> &'static str {
        "Kamigakari"
    }

    fn name(&self) -> &'static str {
        "神我狩"
    }

    fn sort_key(&self) -> &'static str {
        "かみかかり"
    }

    fn help_message(&self) -> &'static str {
        r"・各種表
 ・感情表(ET)
 ・霊紋消費の代償表(RT)
 ・伝奇名字・名前決定表(NT)
 ・魔境臨界表(KT)
 ・獲得素材チャート(MTx xは［法則障害］の［強度］。省略時は１)
　　例） MT　MT3　MT9
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["MT", "RT", "ET", "KT", "NT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Kamigakari#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
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
            .join("test/data/Kamigakari.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Kamigakari.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/Kamigakari.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Kamigakari.toml must parse");
        assert_eq!(
            data.tests.len(),
            23,
            "case count in test/data/Kamigakari.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Kamigakari",
                "unexpected game system in Kamigakari.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Kamigakari"), &tc.input, &mut src) {
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
                    "FAIL Kamigakari:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Kamigakari cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
