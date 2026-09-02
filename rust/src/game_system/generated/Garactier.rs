//! P4で手書き移植した `lib/bcdice/game_system/Garactier.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Garactier#eval_game_system_specific_command`
//!   （`#cmd_gr` → `#roll_item` → `#roll_tables(BUI)` → `#roll_tables(SST)`）
//! - `#roll_search` / `#roll_target` / `#roll_gr` / `#roll_dice_with_modifier`
//! - `#determine_target_result` / `#determine_no_target_result`
//! - `ITEM_TABLES`（ITM2〜ITM6・D66表）と上級アイテム判定

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::Garactier`（ID: `Garactier`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Garactier;

impl GameSystem for Garactier {
    fn id(&self) -> &'static str {
        "Garactier"
    }

    fn name(&self) -> &'static str {
        "ガラクティア"
    }

    fn sort_key(&self) -> &'static str {
        "からくていあ"
    }

    fn help_message(&self) -> &'static str {
        r"ガラクティアVer1.04
x:基準値
y:目標値
x, yについては四則演算の入力が可能

■達成値の算出(GRx)
  クリティカル・ファンブルの判定、達成値の表示を行う。
■通常判定(GRx>=y)
  通常の判定を行う。
■命中判定(GRHx>=y)
  命中判定を行う。
■回避判定(GRDx>=y)
  回避判定を行う。
■抵抗判定(GRMx>=y)
  抵抗判定を行う。
■探索成功マスレベル算出(GRSx)
  探索・索敵判定時の最大成功マスレベル(ML)を算出する
■ 表
  アイテム決定表(ITMn)
    ランクnのアイテム表を振る。例)ITM2 ランク２アイテム表
    上級アイテムの判定も行います。
  命中部位決定表(BUI)
    命中時の部位を決定します。
  増強可能施設決定表(SST)
    報告フェイズの増強可能施設を決定します。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["^GR[HDMS]?", "ITM[2-6]", "BUI", "SST"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Garactier#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(r) = cmd_gr(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(r)));
        }
        if let Some(s) = roll_item(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(s)));
        }
        if let Some(s) = roll_named_table(command, rng, BUI_TABLES)? {
            return Ok(Some(SpecificCommandOutput::text(s)));
        }
        if let Some(s) = roll_named_table(command, rng, SISETSU_TABLES)? {
            return Ok(Some(SpecificCommandOutput::text(s)));
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `ITEM_TABLES`。
static ITEM_TABLES: &[(&str, D66Table)] = &[
    ("ITM2", ITM2),
    ("ITM3", ITM3),
    ("ITM4", ITM4),
    ("ITM5", ITM5),
    ("ITM6", ITM6),
];

static ITM2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("リペアスプレー")),
    (12, TableItem::Text("防壁シャボン")),
    (13, TableItem::Text("応援ボットちゃん")),
    (14, TableItem::Text("偵察ボットちゃん")),
    (15, TableItem::Text("回収ボットちゃん")),
    (16, TableItem::Text("ブラストチャージャー")),
    (21, TableItem::Text("仕掛け爆弾")),
    (22, TableItem::Text("おなじみドリル")),
    (23, TableItem::Text("清掃ボットちゃん")),
    (24, TableItem::Text("突撃ボットちゃん")),
    (25, TableItem::Text("修繕ボットちゃん")),
    (26, TableItem::Text("アンプリファイア")),
    (31, TableItem::Text("コバルトエール")),
    (32, TableItem::Text("カステラ")),
    (33, TableItem::Text("エアガン")),
    (34, TableItem::Text("安全靴")),
    (35, TableItem::Text("ヘルメット")),
    (36, TableItem::Text("フラッシュバルブ")),
    (41, TableItem::Text("クリアワックス")),
    (42, TableItem::Text("目薬")),
    (43, TableItem::Text("ラプチャーヒール")),
    (44, TableItem::Text("防弾盾")),
    (45, TableItem::Text("プレートアーマー")),
    (46, TableItem::Text("ホーリーチャーム")),
    (51, TableItem::Text("カーネルシガー")),
    (52, TableItem::Text("ニトロキャンディー")),
    (53, TableItem::Text("鉱樹の花飾り")),
    (54, TableItem::Text("アンプルシューター")),
    (55, TableItem::Text("メタル包帯")),
    (56, TableItem::Text("炸裂発煙筒")),
    (61, TableItem::Text("シグナルドラッグ")),
    (62, TableItem::Text("毒手")),
    (63, TableItem::Text("グルーガン")),
    (64, TableItem::Text("縫い針")),
    (65, TableItem::Text("パイロン")),
    (66, TableItem::Text("☆上級品☆")),
];
static ITM2: D66Table = D66Table::new("ランク２アイテム決定表", D66SortType::NoSort, ITM2_ITEMS);

static ITM3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バナナ")),
    (12, TableItem::Text("イージーバズーカ")),
    (13, TableItem::Text("マルチバーニア")),
    (14, TableItem::Text("赤い鉢巻き")),
    (15, TableItem::Text("カクテルポイズン")),
    (16, TableItem::Text("リペアジェル")),
    (21, TableItem::Text("金砕棒")),
    (22, TableItem::Text("オシャレステッキ")),
    (23, TableItem::Text("掃除機")),
    (24, TableItem::Text("ソーラーキャップ")),
    (25, TableItem::Text("タクティカルベスト")),
    (26, TableItem::Text("ホイッスル")),
    (31, TableItem::Text("パノラマバイザー")),
    (32, TableItem::Text("フリーズランチャー")),
    (33, TableItem::Text("オシャレスーツ")),
    (34, TableItem::Text("暗器")),
    (35, TableItem::Text("無限軌道")),
    (36, TableItem::Text("イルミネーション")),
    (41, TableItem::Text("光線銃")),
    (42, TableItem::Text("十手")),
    (43, TableItem::Text("銅鑼")),
    (44, TableItem::Text("オシャレハット")),
    (45, TableItem::Text("忍び足")),
    (46, TableItem::Text("釣り竿")),
    (51, TableItem::Text("ブラックパウダー")),
    (52, TableItem::Text("ダーティーマント")),
    (53, TableItem::Text("バッテリーケイン")),
    (54, TableItem::Text("バンデッドショルダー")),
    (55, TableItem::Text("オシャレシューズ")),
    (56, TableItem::Text("サテライト")),
    (61, TableItem::Text("キーパーゴーレム")),
    (62, TableItem::Text("混迷香")),
    (63, TableItem::Text("応援旗")),
    (64, TableItem::Text("黒子頭巾")),
    (65, TableItem::Text("バーナーランス")),
    (66, TableItem::Text("☆上級品☆")),
];
static ITM3: D66Table = D66Table::new("ランク３アイテム決定表", D66SortType::NoSort, ITM3_ITEMS);

static ITM4_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("金塊")),
    (12, TableItem::Text("パラボラアンテナ")),
    (13, TableItem::Text("くらましの敷布")),
    (14, TableItem::Text("無影灯")),
    (15, TableItem::Text("油圧ショベル")),
    (16, TableItem::Text("マシンテール")),
    (21, TableItem::Text("黒曜石の像")),
    (22, TableItem::Text("黄金のクローバー")),
    (23, TableItem::Text("朝霧の箒")),
    (24, TableItem::Text("ヘッドキャノン")),
    (25, TableItem::Text("レッグバルカン")),
    (26, TableItem::Text("ダイナモブロック")),
    (31, TableItem::Text("ジョウロ")),
    (32, TableItem::Text("スナイパーライフル")),
    (33, TableItem::Text("おてがるスコープ")),
    (34, TableItem::Text("ドーザーブレード")),
    (35, TableItem::Text("テツゲタ")),
    (36, TableItem::Text("ジェットスラスター")),
    (41, TableItem::Text("宝剣")),
    (42, TableItem::Text("指揮棒")),
    (43, TableItem::Text("大兜")),
    (44, TableItem::Text("妖精さん")),
    (45, TableItem::Text("ロングホーン")),
    (46, TableItem::Text("鎖がま")),
    (51, TableItem::Text("鳥籠")),
    (52, TableItem::Text("カタパルトアーム")),
    (53, TableItem::Text("スタンドマイク")),
    (54, TableItem::Text("臆病なカカシ")),
    (55, TableItem::Text("ローラーダッシュ")),
    (56, TableItem::Text("モミジ")),
    (61, TableItem::Text("マスターキー")),
    (62, TableItem::Text("隠れ蓑")),
    (63, TableItem::Text("番傘")),
    (64, TableItem::Text("駆動甲冑")),
    (65, TableItem::Text("波紋の杖")),
    (66, TableItem::Text("☆上級品☆")),
];
static ITM4: D66Table = D66Table::new("ランク４アイテム決定表", D66SortType::NoSort, ITM4_ITEMS);

static ITM5_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("因果の卵")),
    (12, TableItem::Text("ランプ")),
    (13, TableItem::Text("常盤の琥珀")),
    (14, TableItem::Text("新緑の冠")),
    (15, TableItem::Text("萌芽の靴")),
    (16, TableItem::Text("星の骸")),
    (21, TableItem::Text("夜の帳")),
    (22, TableItem::Text("スリップブローチ")),
    (23, TableItem::Text("拳法着")),
    (24, TableItem::Text("めがね")),
    (25, TableItem::Text("白旗")),
    (26, TableItem::Text("ディラックナイフ")),
    (31, TableItem::Text("エレキドレッサー")),
    (32, TableItem::Text("ネイルガン")),
    (33, TableItem::Text("木漏れ日のポプリ")),
    (34, TableItem::Text("ミスリルピッケル")),
    (35, TableItem::Text("デスマッチカフス")),
    (36, TableItem::Text("アダムスキースカート")),
    (41, TableItem::Text("主砲")),
    (42, TableItem::Text("マイクロポッド")),
    (43, TableItem::Text("樹皮の円盤")),
    (44, TableItem::Text("リンゴと蛇の紋章")),
    (45, TableItem::Text("セントリーガナー")),
    (46, TableItem::Text("化生の仮面")),
    (51, TableItem::Text("ガトリング")),
    (52, TableItem::Text("オカモチ")),
    (53, TableItem::Text("芭蕉扇")),
    (54, TableItem::Text("ハッピートリガー")),
    (55, TableItem::Text("蠢く湿布")),
    (56, TableItem::Text("メガホン")),
    (61, TableItem::Text("トランシーバー")),
    (62, TableItem::Text("好奇の鋲")),
    (63, TableItem::Text("スレッジハンマー")),
    (64, TableItem::Text("セントール")),
    (65, TableItem::Text("ケーブルナイト")),
    (66, TableItem::Text("☆上級品☆")),
];
static ITM5: D66Table = D66Table::new("ランク５アイテム決定表", D66SortType::NoSort, ITM5_ITEMS);

static ITM6_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("禍福の勾玉")),
    (12, TableItem::Text("祭壇")),
    (13, TableItem::Text("ネジまき心臓")),
    (14, TableItem::Text("まどろみの頭蓋")),
    (15, TableItem::Text("猛進拍車")),
    (16, TableItem::Text("炉心結晶")),
    (21, TableItem::Text("狂奔の鞭")),
    (22, TableItem::Text("暁のベル")),
    (23, TableItem::Text("ネコシッポ")),
    (24, TableItem::Text("鬼蜘蛛")),
    (25, TableItem::Text("戦上手の脚")),
    (26, TableItem::Text("薄絹の外套")),
    (31, TableItem::Text("ネコクロー")),
    (32, TableItem::Text("クライムチャンバー")),
    (33, TableItem::Text("古の灯火")),
    (34, TableItem::Text("かしこい触手")),
    (35, TableItem::Text("オペラグラス")),
    (36, TableItem::Text("大鉄拳")),
    (41, TableItem::Text("妖刀")),
    (42, TableItem::Text("ヘビーライター")),
    (43, TableItem::Text("緋緋色の針")),
    (44, TableItem::Text("バーサクシール")),
    (45, TableItem::Text("光芒のアンクレット")),
    (46, TableItem::Text("ネコブーツ")),
    (51, TableItem::Text("旅するコイン")),
    (52, TableItem::Text("光子鏡壁")),
    (53, TableItem::Text("フェザージャケット")),
    (54, TableItem::Text("レーザーミニオン")),
    (55, TableItem::Text("マニピュレーター")),
    (56, TableItem::Text("ランパートシールド")),
    (61, TableItem::Text("選定者の瞳")),
    (62, TableItem::Text("打ち上げ花火")),
    (63, TableItem::Text("魔笛")),
    (64, TableItem::Text("指輪")),
    (65, TableItem::Text("ネコミミ")),
    (66, TableItem::Text("☆上級品☆")),
];
static ITM6: D66Table = D66Table::new("ランク６アイテム決定表", D66SortType::NoSort, ITM6_ITEMS);

/// Ruby `SISETSU_TABLES`。
static SISETSU_TABLES: &[(&str, Table)] = &[(
    "SST",
    Table::from_dice("増強可能施設決定表", 1, 6, SISETSU_ITEMS),
)];

static SISETSU_ITEMS: &[&str] = &[
    "広場　マーケット　楽団",
    "広場　ガレージ　鉄工所",
    "広場　訓練場　農園　保健所",
    "広場　学舎　骨董屋",
    "広場　塗装工　菓子屋　貯蔵庫",
    "広場　診療所　礼拝堂",
];

/// Ruby `BUI_TABLES`。
static BUI_TABLES: &[(&str, Table)] =
    &[("BUI", Table::from_dice("命中部位決定表", 1, 6, BUI_ITEMS))];

static BUI_ITEMS: &[&str] = &["頭部", "胴体", "右腕", "左腕", "脚部", "任意部位"];

// ---------------------------------------------------------------------------
// コマンド本体
// ---------------------------------------------------------------------------

/// Ruby `Garactier#cmd_gr`（GR系コマンドの分割）。
fn cmd_gr(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    if command.starts_with("GRS") {
        roll_search(command, rng)
    } else if command.starts_with("GRH") || command.starts_with("GRD") || command.starts_with("GRM")
    {
        roll_target(command, rng)
    } else if command.starts_with("GR") {
        roll_gr(command, rng)
    } else {
        Ok(None)
    }
}

/// Ruby `Garactier#roll_search`（探索・索敵判定）。
fn roll_search(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: %r{^GRS([+-/*\d]+)?$}
    let re = regex(r"\AGRS([+\-/*\d]+)?\z");
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let modifier = arithmetic::eval(m.get(1).map(|x| x.as_str()).unwrap_or(""), RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let d = roll_dice_with_modifier(modifier, rng)?;
    let mut r = determine_no_target_result("S", d.total, d.critical, d.fumble)?;

    r.text = format!(
        "({}) ＞ {}[{}]{} ＞ {} ＞ {}",
        &m[0],
        d.dice_sum,
        dice_text::join_dice(&d.dice_list),
        m.get(1).map(|x| x.as_str()).unwrap_or(""),
        d.total,
        r.text
    );
    Ok(Some(r))
}

/// Ruby `Garactier#roll_target`（目標値を持つ判定ロール）。
fn roll_target(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: %r{^GR([HDM])([+-/*\d]+)?(?:>=?([+-/*\d]+)+)$}
    let re = regex(r"\AGR([HDM])([+\-/*\d]+)?(?:>=?([+\-/*\d]+)+)\z");
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let roll_type = &m[1];
    let modifier = arithmetic::eval(m.get(2).map(|x| x.as_str()).unwrap_or(""), RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let target = arithmetic::eval(m.get(3).map(|x| x.as_str()).unwrap_or(""), RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let d = roll_dice_with_modifier(modifier, rng)?;
    let mut r = determine_target_result(roll_type, d.total, target, d.critical, d.fumble);

    r.text = format!(
        "({}) ＞ {}[{}]{} ＞ {} ＞ {}",
        &m[0],
        d.dice_sum,
        dice_text::join_dice(&d.dice_list),
        m.get(2).map(|x| x.as_str()).unwrap_or(""),
        d.total,
        r.text
    );
    Ok(Some(r))
}

/// Ruby `Garactier#roll_gr`（GRのみの基本判定）。
fn roll_gr(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: %r{^GR([+-/*\d]+)?(>=)?\(?([+-/*\d]+)?\)?$}
    let re = regex(r"\AGR([+\-/*\d]+)?(>=)?\(?([+\-/*\d]+)?\)?\z");
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let modifier = arithmetic::eval(m.get(1).map(|x| x.as_str()).unwrap_or(""), RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let target_flag = m.get(2).is_some();
    let d = roll_dice_with_modifier(modifier, rng)?;

    let mut r = if target_flag {
        let target =
            arithmetic::eval(m.get(3).map(|x| x.as_str()).unwrap_or(""), RoundType::Floor)?
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0);
        determine_target_result("", d.total, target, d.critical, d.fumble)
    } else {
        determine_no_target_result("", d.total, d.critical, d.fumble)?
    };

    r.text = format!(
        "({}) ＞ {}[{}]{} ＞ {} ＞ {}",
        &m[0],
        d.dice_sum,
        dice_text::join_dice(&d.dice_list),
        m.get(1).map(|x| x.as_str()).unwrap_or(""),
        d.total,
        r.text
    );
    Ok(Some(r))
}

/// 判定結果の中間データ（Ruby の `roll_dice_with_modifier` 戻り値ハッシュ）。
struct DiceWithModifier {
    dice_list: Vec<i64>,
    dice_sum: i64,
    total: i64,
    critical: bool,
    fumble: bool,
}

/// Ruby `Garactier#roll_dice_with_modifier`（基準値をもとに2d6+基準値の判定）。
fn roll_dice_with_modifier(
    modifier: i64,
    rng: &mut Randomizer,
) -> Result<DiceWithModifier, EvalError> {
    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_sum: i64 = dice_list.iter().sum();
    let total = dice_sum + modifier;
    let critical = dice_list.iter().filter(|&&d| d == 6).count() == 2;
    let fumble = dice_list.iter().filter(|&&d| d == 1).count() == 2;
    Ok(DiceWithModifier {
        dice_list,
        dice_sum,
        total,
        critical,
        fumble,
    })
}

/// Ruby `Garactier#determine_target_result`（目標値あり）。
fn determine_target_result(
    roll_type: &str,
    total: i64,
    target: i64,
    critical: bool,
    fumble: bool,
) -> EvalResult {
    match roll_type {
        "H" => {
            if critical {
                EvalResult::critical("クリティカル命中")
            } else if fumble {
                EvalResult::fumble("ファンブル")
            } else if total >= target + 4 {
                EvalResult::success("急所命中")
            } else if total >= target {
                EvalResult::success("命中")
            } else {
                EvalResult::failure("失敗")
            }
        }
        "D" => {
            if critical {
                EvalResult::critical("クリティカル")
            } else if fumble {
                EvalResult::fumble("ファンブル")
            } else if total >= target {
                EvalResult::success("回避成功")
            } else if total >= target - 4 {
                EvalResult::failure("半減命中")
            } else {
                EvalResult::failure("失敗")
            }
        }
        // 抵抗判定は基準値以上(激情)のほうが悪い効果のことが多いためResultを反転
        // [6,6]の場合にファンブル
        "M" => {
            if critical {
                EvalResult::fumble("必ず激情")
            } else if fumble {
                EvalResult::critical("必ず平静")
            } else if total >= target {
                EvalResult::failure("激情")
            } else {
                EvalResult::success("平静")
            }
        }
        _ => {
            if critical {
                EvalResult::critical("クリティカル")
            } else if fumble {
                EvalResult::fumble("ファンブル")
            } else if total >= target {
                EvalResult::success("成功")
            } else {
                EvalResult::failure("失敗")
            }
        }
    }
}

/// Ruby `Garactier#determine_no_target_result`（目標値なし）。
fn determine_no_target_result(
    roll_type: &str,
    total: i64,
    critical: bool,
    fumble: bool,
) -> Result<EvalResult, EvalError> {
    match roll_type {
        "S" => {
            if critical {
                Ok(EvalResult::critical("クリティカル"))
            } else if fumble {
                Ok(EvalResult::fumble("ファンブル"))
            } else {
                // Ruby: success_level = (total - 4) / 2（Integer#/, 切り捨て。
                // divisor=2 固定のため ZeroDivision は到達しない）
                let mut success_level = crate::randomizer::sat_i64(&crate::arithmetic::ruby_div(
                    crate::Int::from(total - 4),
                    crate::Int::from(2),
                )?);
                if success_level >= 11 {
                    success_level = 11;
                } else if success_level <= 0 {
                    success_level = 1;
                }
                Ok(EvalResult::with_text(format!("成功ML {success_level}")))
            }
        }
        _ => {
            if critical {
                Ok(EvalResult::critical("クリティカル"))
            } else if fumble {
                Ok(EvalResult::fumble("ファンブル"))
            } else {
                Ok(EvalResult::with_text(format!("達成値 {total}")))
            }
        }
    }
}

/// Ruby `Garactier#roll_item`（アイテム表ロール ランク）。
fn roll_item(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if !command.contains("ITM") {
        return Ok(None);
    }

    // 1回目のアイテムロール
    let Some(mut result) = roll_item_tables(command, rng)? else {
        return Ok(None);
    };

    // 上級判定
    if result.contains("(66)") {
        let second =
            roll_item_tables(command, rng)?.expect("same command matches the same table again");
        result = format!("{second}*上級アイテム*");
    }
    // 選択判定
    if result.contains("(66)") {
        result = "上級アイテムを自由選択！！".to_string();
    }
    Ok(Some(result))
}

/// Ruby `Base#roll_tables(command, ITEM_TABLES)`。
fn roll_item_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = ITEM_TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `Base#roll_tables(command, tables)`（BUI / SST 用）。
fn roll_named_table(
    command: &str,
    rng: &mut Randomizer,
    tables: &'static [(&'static str, Table)],
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// 各所で使う正規表現のキャッシュ。
///
/// `pattern` はこのモジュール内の静的文字列のみ渡される。
fn regex(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<std::collections::HashMap<&'static str, Regex>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        [
            r"\AGRS([+\-/*\d]+)?\z",
            r"\AGR([HDM])([+\-/*\d]+)?(?:>=?([+\-/*\d]+)+)\z",
            r"\AGR([+\-/*\d]+)?(>=)?\(?([+\-/*\d]+)?\)?\z",
        ]
        .into_iter()
        .map(|p| (p, Regex::new(p).expect("valid regex")))
        .collect()
    });
    cache
        .get(pattern)
        .unwrap_or_else(|| panic!("unknown regex pattern: {pattern}"))
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
            .join("test/data/Garactier.toml");
        path.exists().then_some(path)
    }

    /// 余った注入乱数を許すケース（`(1始まりのケース番号, 残り個数)`）。
    ///
    /// Ruby本家の `RandomizerMock` は余りを検査しないので、TOMLには
    /// 「Ruby側もダイスを振る前に nil を返すコマンド」にもダイスが書かれている。
    /// ケース34 (`GRH>=`) は `roll_target` の正規表現が目標値部分を要求するため
    /// マッチせず nil。ケース35 (`GRR`) も `cmd_gr` のどこにもマッチしないため nil。
    /// いずれも Ruby も1個も振らない（rands unconsumed）。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[
        (34, 2), // GRH>= 命中判定、目標値なしコマンドエラー
        (35, 2), // GRR GRがあるがその後に不要な文字列が存在する
    ];

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Garactier.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Garactier.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Garactier.toml must parse");
        assert_eq!(
            data.tests.len(),
            62,
            "case count in test/data/Garactier.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Garactier",
                "unexpected game system in Garactier.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Garactier"), &tc.input, &mut src) {
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
                let allowed_surplus = SURPLUS_RANDS_ALLOWED
                    .iter()
                    .find(|(case, _)| *case == i + 1)
                    .map_or(0, |(_, remaining)| *remaining);
                if src.remaining() != allowed_surplus {
                    reasons.push(format!(
                        "unconsumed rands remain ({}, allowed {allowed_surplus})",
                        src.remaining()
                    ));
                }
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL Garactier:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Garactier cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
