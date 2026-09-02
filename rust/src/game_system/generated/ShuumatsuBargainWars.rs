//! P4で手書き移植した `lib/bcdice/game_system/ShuumatsuBargainWars.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ShuumatsuBargainWars#roll_bg`（行為判定 `nBGk+y>=t`）
//! - `TABLES`（イベント表 `ET` / トラブル表 `TT` / アイテム表3種 `RT`・`CT`・`WT` /
//!   それらを連鎖で引くワゴン `WG`）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{ChainTable, D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `RecoveryItemTable`（回復系アイテム表）の項目。
static RECOVERY_ITEM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("飴玉")),
    (12, TableItem::Text("エナジードリンク")),
    (13, TableItem::Text("せんべい")),
    (14, TableItem::Text("餅")),
    (15, TableItem::Text("ロウソク")),
    (16, TableItem::Text("酒")),
    (22, TableItem::Text("寿司")),
    (23, TableItem::Text("ばんそうこう")),
    (24, TableItem::Text("お布団")),
    (25, TableItem::Text("カレー")),
    (26, TableItem::Text("消毒液")),
    (33, TableItem::Text("缶詰")),
    (34, TableItem::Text("みたらし団子")),
    (35, TableItem::Text("骨付き肉")),
    (36, TableItem::Text("ステーキ")),
    (44, TableItem::Text("うちわ")),
    (45, TableItem::Text("ぬいぐるみ")),
    (46, TableItem::Text("のり")),
    (55, TableItem::Text("美容液")),
    (56, TableItem::Text("黄色いハンカチ")),
    (66, TableItem::Text("洗剤")),
];

/// Ruby `ConvenienceItemTable`（便利系アイテム表）の項目。
static CONVENIENCE_ITEM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ちくわ")),
    (12, TableItem::Text("焼き芋")),
    (13, TableItem::Text("トイレットペーパー")),
    (14, TableItem::Text("熊手")),
    (15, TableItem::Text("胡椒")),
    (16, TableItem::Text("鏡")),
    (22, TableItem::Text("割りばし")),
    (23, TableItem::Text("輪ゴム")),
    (24, TableItem::Text("塩の結晶")),
    (25, TableItem::Text("プチプチマット")),
    (26, TableItem::Text("長靴")),
    (33, TableItem::Text("バケツ")),
    (34, TableItem::Text("アルミホイル")),
    (35, TableItem::Text("下敷き")),
    (36, TableItem::Text("長芋")),
    (44, TableItem::Text("鉛筆")),
    (45, TableItem::Text("まな板")),
    (46, TableItem::Text("フライパン")),
    (55, TableItem::Text("ほうき")),
    (56, TableItem::Text("クラッカー")),
    (66, TableItem::Text("消臭スプレー")),
];

/// Ruby `WeaponItemTable`（武器系アイテム表）の項目。
static WEAPON_ITEM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アズキアイス")),
    (12, TableItem::Text("スプーン")),
    (13, TableItem::Text("フォーク")),
    (14, TableItem::Text("カミソリ")),
    (15, TableItem::Text("電池")),
    (16, TableItem::Text("デッキブラシ")),
    (22, TableItem::Text("傘")),
    (23, TableItem::Text("物干し竿")),
    (24, TableItem::Text("鉄パイプ")),
    (25, TableItem::Text("くぎ打ち機")),
    (26, TableItem::Text("モンキーレンチ")),
    (33, TableItem::Text("ハエタタキ")),
    (34, TableItem::Text("鎌")),
    (35, TableItem::Text("蛍光灯")),
    (36, TableItem::Text("包丁")),
    (44, TableItem::Text("ハサミ")),
    (45, TableItem::Text("ショベル")),
    (46, TableItem::Text("釣り竿")),
    (55, TableItem::Text("芝刈り機")),
    (56, TableItem::Text("ステッキ")),
    (66, TableItem::Text("小麦粉")),
];

/// Ruby `TABLES["ET"]`（イベント表）の項目。
static EVENT_ITEMS: &[&str] = &[
    "ドッキン！一目惚れ！好きなキャラクターを1人選ぶ。このセッション中その相手との関係の深度を互いに3以上にすることができた場合、シナリオの結末に関係なく貴方は完全無欠のハッピーエンドを迎え経験点を100点得る。達成できなかった場合、エンディングフェイズで目が覚める。",
    "おや？こんな所にアイテムが転がっている。ランダムに選んだアイテムを獲得する。そのアイテムの種別が支援・計画ならば[技術]/5の判定に成功すれば手番を消費せずそのアイテムを使用しても良い。",
    "チームメンバーと二人っきりになる。ちょっといい雰囲気かも。好きなキャラクターを目標に選び、『関係』のチェックを外す事ができる。",
    "あぶな～い！チームメンバーに危機が襲い掛かる。PCの中からランダムに1人を選び[武力]/5の判定を行う。成功すると互いに『関係』を結ぶことができる。失敗すると2人とも体力に1d6点のダメージ。",
    "ちょっとお食事でも如何？自身の体力3点と活力1点を回復させる。",
    "穏やかな時が流れる。このメンバーならこれからも上手くやっていけそうだ。ランダムにPCを選び『関係』を獲得する。",
    "チームメンバーの意外な一面を覗く、まさかアイツあんな趣味があったなんて！PCの中からランダムで1人を選び[精神]/6で判定を行う。成功すると互いに『関係』を獲得する。失敗すると互いに活力が1点減少する。",
    "仲間と意見が対立する。アイツにだけは負けられない！関係を持つPCの中からランダムで1人を選び、対象との関係の深度を1下げてもよい（0未満にならない）。下げた場合、以降のセッション中任意の能力値が1上昇したものとして判定を行う事ができる。この効果で実際に能力値は上がらない。",
    "何かお手伝いをしよう。好きなキャラクターを1人選ぶ。この休憩中次に相手が判定を行う場合、その判定に修正値+1を加える。その後、目標は自分に対し『関係』を獲得する。",
    "酒を発見、宴だぁああ！！！PCは全員回復アイテムの「酒」の効果を使用できる。その後、自分の持つ全ての『関係』をランダムな相手に同じ《深度》で取り直す。",
    "不味い！敵襲だ！バナナワニにキリミウオが戦闘を仕掛けに来る。戦闘に勝利した場合、好きなアイテムを1つ得る。この処理が面倒ならば戦闘を行う代わりにPC達全員の体力の値を半分にし戦闘に勝利したものとして扱っても良い。",
];

/// Ruby `TABLES["TT"]`（トラブル表）の項目。
static TROUBLE_ITEMS: &[&str] = &[
    "緊張感からか焦りが生じる。以降スポットフェイズに行くまでの間あらゆる判定の成功度が1減少する。",
    "カートの操作が効かなくなった！このラウンドは操作表を全員ダイスを振りランダムで決定する事。",
    "派手な振動が起き頭をぶつける。全員1d6点のダメージを受ける。",
    "集中力が切れて来た……全員の活力を1 点減少させる",
    "激しく揺さぶられ荷物が落下する。カート内にあるアイテムを1 つ選ぶ。そのアイテムを失う。",
    "不気味な超市場の雰囲気がパッセンジャー達の不安を煽る。特に何も起こらない。",
];

/// Ruby `RecoveryItemTable`。
static RECOVERY_ITEM_TABLE: D66Table =
    D66Table::new("回復系アイテム表", D66SortType::Asc, RECOVERY_ITEM_ITEMS);

/// Ruby `ConvenienceItemTable`。
static CONVENIENCE_ITEM_TABLE: D66Table =
    D66Table::new("便利系アイテム表", D66SortType::Asc, CONVENIENCE_ITEM_ITEMS);

/// Ruby `WeaponItemTable`。
static WEAPON_ITEM_TABLE: D66Table =
    D66Table::new("武器系アイテム表", D66SortType::Asc, WEAPON_ITEM_ITEMS);

/// Ruby `TABLES["ET"]`。
static EVENT_TABLE: Table = Table::from_dice("イベント表", 2, 6, EVENT_ITEMS);

/// Ruby `TABLES["TT"]`。
static TROUBLE_TABLE: Table = Table::from_dice("トラブル表", 1, 6, TROUBLE_ITEMS);

/// Ruby `TABLES["WG"]` の項目。3つのアイテム表を2つずつ並べたもの。
static WAGON_ITEMS: &[TableItem] = &[
    TableItem::Table(&RECOVERY_ITEM_TABLE),
    TableItem::Table(&RECOVERY_ITEM_TABLE),
    TableItem::Table(&CONVENIENCE_ITEM_TABLE),
    TableItem::Table(&CONVENIENCE_ITEM_TABLE),
    TableItem::Table(&WEAPON_ITEM_TABLE),
    TableItem::Table(&WEAPON_ITEM_TABLE),
];

/// Ruby `TABLES["WG"]`（`DiceTable::ChainTable`）。
static WAGON_TABLE: ChainTable = ChainTable::from_dice("ワゴン", 1, 6, WAGON_ITEMS);

/// Ruby `TABLES` の値。`Table` / `D66Table` / `ChainTable` が混在する。
///
/// いずれも `to_s` は `"表名(値) ＞ 内容"` なので、`roll_tables` からは文字列で揃えて扱う。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("ET", &EVENT_TABLE),
    ("TT", &TROUBLE_TABLE),
    ("RT", &RECOVERY_ITEM_TABLE),
    ("CT", &CONVENIENCE_ITEM_TABLE),
    ("WT", &WEAPON_ITEM_TABLE),
    ("WG", &WAGON_TABLE),
];

/// Ruby `BCDice::GameSystem::ShuumatsuBargainWars`（ID: `ShuumatsuBargainWars`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShuumatsuBargainWars;

impl GameSystem for ShuumatsuBargainWars {
    fn id(&self) -> &'static str {
        "ShuumatsuBargainWars"
    }

    fn name(&self) -> &'static str {
        "終末買い物戦争"
    }

    fn sort_key(&self) -> &'static str {
        "しゆうまつはあけんうおおす"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定 （nBGk+y>=t）n:ダイス数、k:心根、y:修正値（省略可)、t:目標値
  例）3BG1>=3 2BG3+1>=4 4BG5-1>=3
・アイテム表
  ・RT 回復系アイテム表
  ・CT 便利系アイテム表
  ・WT 武器系アイテム表
  ・WG ワゴン(全アイテムランダム)
・ET イベント表
・TT トラブル表
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+BG", "ET", "TT", "RT", "CT", "WT", "WG"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `ShuumatsuBargainWars#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: roll_bg(command) || roll_tables(command, TABLES)
        if let Some(result) = roll_bg(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        Ok(None)
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `ShuumatsuBargainWars#roll_bg`。
fn roll_bg(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("BG", round_type: @round_type)（Base の既定 :floor）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["BG"], RoundType::Floor)
            .has_prefix_number()
            .has_suffix_number()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `has_prefix_number` / `has_suffix_number` / `restrict_cmp_op_to(:>=)` により
    // パースに成功した時点でダイス数・心根・目標値は必ずある。
    let times = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let kokorone = parsed
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let correction = parsed.modify_number.clone();
    let target = parsed.target_number.clone().unwrap_or(crate::Int::from(0));

    let mut dice_list = rng.roll_barabara(times, 6)?;
    dice_list.sort_unstable();

    let success = dice_list
        .iter()
        .filter(|&&number| number >= crate::randomizer::sat_i64(&(&target - correction.clone())))
        .count();
    // 活力の獲得数は修正値を受けない（原典どおり）
    let get_vitality = dice_list
        .iter()
        .filter(|&&number| number == kokorone)
        .count();

    let mut result = if dice_list.iter().filter(|&&number| number == 6).count() >= 2 {
        EvalResult::critical(format!(
            "スペシャル！ 成功度{}、活力{get_vitality}獲得",
            success + 1
        ))
    } else if dice_list.iter().all(|&number| number == 1) {
        // Ruby `[].all?(1)` は true なので、ダイス0個（`0BG…`）もここに落ちる
        EvalResult::fumble("ファンブル 活力をすべて失う")
    } else {
        // Ruby: Result.new("...")（フラグは一切立たない）
        EvalResult::with_text(format!("成功度{success}、活力{get_vitality}獲得"))
    };

    result.text = format!(
        "({}) ＞ [{}] ＞ {}",
        parsed.to_s(SuffixPosition::AfterCommand),
        join_dice(&dice_list),
        result.text
    );

    Ok(Some(result))
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ShuumatsuBargainWars",
            "ShuumatsuBargainWars.toml",
            11,
        );
    }
}
