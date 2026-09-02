//! P4で手書き移植した `lib/bcdice/game_system/MeikyuKingdomBasic.rb` と
//! `lib/bcdice/game_system/meikyu_kingdom_basic/*.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MeikyuKingdomBasic#kiryoku_result`（`MeikyuKingdom` の上書き）
//! - `MeikyuKingdomBasic#eval_game_system_specific_command`
//!   （`TABLES` / `A2Z_TABLES` / `RR236_TABLES` / `TK_TABLES` の `roll_tables` →
//!   固有の `case` → どれにも当たらなければ `super`）
//! - `meikyu_kingdom_basic/*.rb` の表データとロジック
//!
//! # 継承
//!
//! Ruby `class MeikyuKingdomBasic < MeikyuKingdom` の `super(command)` は
//! [`MeikyuKingdom::eval_specific`](super::MeikyuKingdom::eval_specific) を
//! [`MKB_HOOKS`] 付きで呼ぶことで表す。`kiryoku_result` や
//! `mk_name_ex_table` / `mk_word_*_table` / `mk_kingdom_name_*_table` は
//! Ruby側で上書きされており、親クラスのコードから動的ディスパッチで
//! こちらの実装が呼ばれるため、[`MkHooks`] 経由で差し替える。
//!
//! # 表データ
//!
//! `MKB_TABLE_*` は Ruby の `TABLES` などが持つ `DiceTable::*` オブジェクトを、
//! `MKB_*_TABLE` はメソッド内のリテラル配列を、原典rbから機械抽出したもの。
//! 値は1文字も変えていない。
//!
//! # 原典の到達不能アーム
//!
//! Ruby の `case` には `NRWT` / `NRUT` / `ARWT` / `ARUT` / `CIR` / `RWIR` / `RUIR` /
//! `RMS` / `ABUS`〜`ATOS` のアームがあるが、呼び出し先のメソッド
//! （`mk_normal_rare_weapon_item_table` など）は定義されていない。
//! これらのコマンドはちょうど `TABLES` のキーでもあるため、完全一致する入力は
//! 手前の `roll_tables` で処理され、アームには届かない。届くのは
//! `NRWTX` のような接尾辞つきの入力だけで、Ruby はそこで `NoMethodError` を投げる。
//! 本移植は例外の代わりに「アームに当たらなかった」ものとして `super` へ落とす
//! （このリポジトリの他の移植と同じく、原典が落ちる経路はクラッシュさせない方針）。

use std::sync::OnceLock;

use regex::Regex;

use super::MeikyuKingdom::{eval_specific, get_count, head, n_number, result_nd6_impl, MkHooks};
use crate::arithmetic;
use crate::dice_table::{ChainTable, D66GridTable, D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::CheckOutcome;

/// Ruby `BCDice::GameSystem::MeikyuKingdomBasic`（ID: `MeikyuKingdomBasic`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeikyuKingdomBasic;

impl GameSystem for MeikyuKingdomBasic {
    fn id(&self) -> &'static str {
        "MeikyuKingdomBasic"
    }

    fn name(&self) -> &'static str {
        "迷宮キングダム 基本ルールブック"
    }

    fn sort_key(&self) -> &'static str {
        "めいきゆうきんくたむきほんるうるふつく"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `MeikyuKingdom#result_nd6`（`kiryoku_result` だけが上書きされる）。
    fn result_nd6(
        &self,
        total: crate::Int,
        dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd6_impl(
            crate::randomizer::sat_i64(&total),
            dice_total,
            value_list,
            cmp_op,
            target,
            &MKB_HOOKS,
        )
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific_basic(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `HELP_MESSAGE`。
static HELP_MESSAGE: &str = r"  ・判定　(nMK+m)
  　n個のD6を振って大きい物二つだけみて達成値を算出します。修正mも可能です。
  　絶対成功と絶対失敗も自動判定します。
  ・各種表
  　・休憩表：才覚 TBT／魅力 CBT／探索 SBT／武勇 VBT
             お祭り FBT／空振り EBT／全体 WBT／カップル LBT
             お食事 FDBT
  　・ハプニング表：才覚 THT／魅力 CHT／探索 SHT／武勇 VHT
  　・視察表 RT／情報収集表 IG／ランダムマップ選択表 RMS
  　・痛打表 CAT／致命傷表 FWT／戦闘ファンブル表 CFT
  　・道中表 TT／交渉表 NT／相場表 MPT／王国災厄表 KDT／王国変動表 KCT
  　・感情表 ET／好意表 FET／敵意表 HET
  　・お宝表１／２／３／４／５ T1T／T2T／T3T／T4T／T5T
  　・特殊遭遇表 SE
  　　　上級：人工 ARN／水域 WEN／自然 NEN／洞窟 CEN／天空 SEN／異界 OEN
  　・他国との関係表 ORT
  ・潜在能力：スキル決定表 SDT
  　　基本：肉弾 BUS／射撃 SHS／星術 ASS／召喚 SUS／科学 SCS
  　　　　　迷宮 LAS／交渉 NES／便利 COS／芸能 ENS／道具 TOS
  　　　　　一般 GES
  　　上級：肉弾 ABUS／射撃 ASHS／星術 AASS／召喚 ASUS／科学 ASCS
  　　　　　迷宮 ALAS／交渉 ANES／便利 ACOS／芸能 AENS／道具 ATOS
  ・アイテム関連（上級不使用の場合、カッコ書きのものを使用して下さい）
  　・コモンアイテムランダム決定表　CIR
  　　　コモンアイテム表：武具 WIT／生活 LIT／回復 RIT／探索 SIT
  　・レア武具アイテムランダム決定表　RWIR／レア一般アイテムランダム決定表　RUIR（上級込）
  　　　レアアイテム表：基本武具 NRWT／基本一般 NRUT／上級武具 ARWT／上級一般 ARUT
  　・デヴァイスファクトリー DFTx (xは特性の個数)
  ・王国人物作成関連
  　・王国名決定表１／２／３ KNT1／KNT2／KNT3
  　・王国環境表 KET：技術 TET／国風 NST／資源 RET／人材 HRT／施設 FAT／血族 BLT
  　・名前表 NAMEx (xは個数)
  　　　名前A NAMEA／名前B NAMEB／エキゾチック NAMEEX／ファンタジック NAMEFA
  　・新名前表 NNAMEx (xは個数)
  　　　芸術 NMAR／食べ物 NMFO／日用品 NMDN／地名 NMPL／機械 NMMA／神様 NMGO
  　・単語表１／２／３／４　WORD1／WORD2／WORD3／WORD4
  　・生まれ決定表 BDT／生まれ表：才覚 TBO／魅力 CBO／探索 SBO／武勇 VBO
  　・初期装備表 IEQ
　・地名決定表　PNTx (xは個数)／迷宮風景表 MLTx (xは個数)
  ・百万世界AtoZ関連
  　・ご祝儀表 GIFT
  　・ニュース表 NWST
  　・王国ハプニング表 KDHT
  　・王国試練表 KDTT
  　・騎士道表 CHVT
  　・儀式表 RITT
  　・限定表 LIMT
  　・固有の文化表 UNCT
  　・後世の評価表 POET
  　・高レベル特殊遭遇表 KSET
  　・死霊の日々表 DODT
  　・事件名表1／2 INT1／INT2
  　・守護星座表 GUCT
  　・需要表 DEMTx (x:名物レベル)
  　・小鬼と一緒表 WLDT
  　・侵略ハプニング表 INHT
  　・新・情報収集表 NIGT
  　・人種特徴表／人種名文字表 RACT／RNCTx (x:文字数)
  　・迷宮化現象表／人体迷宮化表 HBLT／LAPT
  　・勢力表 POWT
  　・中立特殊遭遇表 NSET
  　・超協調行動表 ECBT
  　・通貨外見表／通貨材質表／通貨名称表 CUAT／CUMT／CUNT
  　・天候表 WEAT
  　・毒の追加効果表 PAET
  　・媒体表 MEDT
  　・反応表／民の反応表 REAT／PRET
  　・遍歴表 HIST
  　・由来表 ORIT
  　・誘致表 ATRT
  　・妖精の悪戯表 FAMT
  ・R&R236関連
  　・四字熟語系二つ名表／代入命名表／組み合わせ系二つ名表／名文句系二つ名表 FCNT／ASNT／CMNT／FPNT
  　・形容表／用語表／行動表／呼び名表／称号表 DEST／TRMT／BEHT／NAMT／TTLT
  ・旅する王国と無名階域関連
  　・サブキャラクター反応表 CSRT
  　・スキルグループ表 KSGT
  　・ダイナマイト帝国:休憩表／特殊遭遇表／二つ名表／名前表 DRET／DSET／DNNT／DNAT
  　・ハグルマ資本主義神聖共和国:休憩表／特殊遭遇表／二つ名表／名前表 HRET／HSET／HNNT／HNAT
  　・メトロ汗国:休憩表／特殊遭遇表／二つ名表／名前表 MRET／MSET／MNNT／MNAT
  　・千年王朝:休憩表／特殊遭遇表／二つ名表／名前表 TRET／TSET／TNNT／TNAT
  　・遠征王国変動表／遠征道中表 EKCT／EXITx (x:修正)
  　・候補生背景表／職長背景表 CABT／FOBT
  　・上級ジョブ表・甲／乙 AJTA／AJTB
  　・深階休憩表／特殊遭遇表 KART／KAET
  　・天階休憩表／特殊遭遇表 TERT／TEET
  　・星の階休憩表 HKRT
  　・装備決定表 EQDT
  　・大冒険時代表 GART
  　・燃料表 FUET
  　・反乱表 RBLT
  　・旅する王国環境表／王国名決定表1／王国名決定表2／王国名決定表3 TKET／TKNT1／TKNT2／TKNT3
  　・列強経歴表 GPHT
 ・D66ダイスあり
";

/// Ruby `register_prefix(...)`。
static PREFIXES: &[&str] = &[
    r"\d+MK", r"\d+R6", "IG", "TT", "NT", "RMS", "CFT", "FWT", "CAT", "KDT", "KCT", "TBT", "CBT",
    "SBT", "VBT", "FBT", "EBT", "WBT", "LBT", "THT", "CHT", "SHT", "VHT", "BDT", "TBO", "CBO",
    "SBO", "VBO", "ET", "FET", "HET", "SDT", "IEQ", "FRT", "T1T", "T2T", "T3T", "T4T", "T5T",
    "MPT", "KNT", "WORD", "NAME", "NNAME", "NM", "RT", "CIR", "RUIR", "RWIR", "WIT", "LIT", "RIT",
    "SIT", "NRWT", "NRUT", "ARWT", "ARUT", "KET", "TET", "NST", "RET", "FAT", "HRT", "BLT", "BUS",
    "SHS", "ASS", "SUS", "SCS", "LAS", "NES", "COS", "ENS", "TOS", "ABUS", "ASHS", "AASS", "ASUS",
    "ASCS", "ALAS", "ANES", "ACOS", "AENS", "ATOS", "SE", "ARN", "WEN", "NEN", "CEN", "SEN", "OEN",
    "DFT", "PNT", "MLT", "FDBT", "GIFT", "NWST", "EKCT", "KDHT", "KDTT", "CHVT", "RITT", "LIMT",
    "UNCT", "POET", "KSET", "DODT", "INT1", "INT2", "GUCT", "DEMT", "WLDT", "INHT", "NIGT", "RACT",
    "RNCT", "HBLT", "POWT", "NSET", "ECBT", "CUAT", "CUMT", "CUNT", "WEAT", "PAET", "MEDT", "REAT",
    "HIST", "PRET", "LAPT", "ORIT", "ATRT", "FAMT", "FCNT", "ASNT", "TRMT", "BEHT", "DEST", "NAMT",
    "TTLT", "CMNT", "FPNT", "CSRT", "KSGT", "DRET", "DSET", "DNNT", "DNAT", "HRET", "HSET", "HNNT",
    "HNAT", "MRET", "MSET", "MNNT", "MNAT", "EXIT", "TRET", "TSET", "TNNT", "TNAT", "CABT", "AJTA",
    "AJTB", "FOBT", "KAET", "KART", "TEET", "TERT", "HKRT", "EQDT", "GART", "FUET", "RBLT", "TKET",
    "GPHT", "TKNT1", "TKNT2", "TKNT3", "GES", "ORT",
];

/// Ruby `MeikyuKingdomBasic#kiryoku_result`。
///
/// 親クラスと違い「もしくは」の枝がなく、6の目の数だけを見る。
fn kiryoku_result(_total_n: i64, dice_list: &[i64], _diff: i64) -> String {
    let num_6 = dice_list.iter().filter(|d| **d == 6).count();
    if num_6 == 0 {
        String::new()
    } else {
        format!(" ＆ 《気力》{num_6}点獲得")
    }
}

/// Ruby `MeikyuKingdomBasic` が上書きするメソッド群。
pub(crate) static MKB_HOOKS: MkHooks = MkHooks {
    kiryoku_result,
    name_ex_table: mkb_name_ex_table,
    word_tables: [
        mkb_word_1_table,
        mkb_word_2_table,
        mkb_word_3_table,
        mkb_word_4_table,
    ],
    kingdom_name_tables: [
        mkb_kingdom_name_1_table,
        mkb_kingdom_name_2_table,
        mkb_kingdom_name_3_table,
    ],
};

/// Ruby `MeikyuKingdomBasic#mk_name_ex_table(num)`。
fn mkb_name_ex_table(num: i64) -> String {
    n_number(num, MKB_NAME_EX_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_word_1_table(num)`。
fn mkb_word_1_table(num: i64) -> String {
    n_number(num, MKB_WORD_1_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_word_2_table(num)`。
fn mkb_word_2_table(num: i64) -> String {
    n_number(num, MKB_WORD_2_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_word_3_table(num)`。
fn mkb_word_3_table(num: i64) -> String {
    n_number(num, MKB_WORD_3_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_word_4_table(num)`。
fn mkb_word_4_table(num: i64) -> String {
    n_number(num, MKB_WORD_4_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_1_table(num)`。
fn mkb_kingdom_name_1_table(num: i64) -> String {
    n_number(num, MKB_KINGDOM_NAME_1_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_2_table(num)`。
fn mkb_kingdom_name_2_table(num: i64) -> String {
    n_number(num, MKB_KINGDOM_NAME_2_TABLE).to_string()
}

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_3_table(num)`。
fn mkb_kingdom_name_3_table(num: i64) -> String {
    n_number(num, MKB_KINGDOM_NAME_3_TABLE).to_string()
}

// ---------------------------------------------------------------------------
// アイテム特性（Ruby `ItemFeature` / `ItemFeaturesTable`）
// ---------------------------------------------------------------------------

/// Ruby `ItemFeature#@items` の要素。
enum FeatureItem {
    /// 文字列の項目。
    Text(&'static str),
    /// 別の `ItemFeature`（`choice` で連鎖する）。
    Feature(&'static ItemFeature),
}

/// Ruby `MeikyuKingdomBasic::ItemFeature`。
///
/// Ruby の `@name` は `choice` で使われないため持っていない。
struct ItemFeature {
    times: i64,
    sides: i64,
    items: &'static [FeatureItem],
}

impl ItemFeature {
    const fn new(times: i64, sides: i64, items: &'static [FeatureItem]) -> Self {
        Self {
            times,
            sides,
            items,
        }
    }

    /// Ruby `ItemFeature#choice(randomizer)`。
    ///
    /// 項目が範囲外なら Ruby は `nil` を埋め込んで `"[#{dice}]"` になる。
    fn choice(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        let dice = rng.roll_sum(self.times, self.sides)?;
        let index = dice - self.times;
        let chosen = match usize::try_from(index).ok().and_then(|i| self.items.get(i)) {
            None => String::new(),
            Some(FeatureItem::Text(t)) => (*t).to_string(),
            Some(FeatureItem::Feature(f)) => f.choice(rng)?,
        };
        Ok(format!("[{dice}]{chosen}"))
    }
}

/// Ruby `ItemFeaturesTable#@items` の各行の要素。
enum FeatureRowItem {
    /// 文字列。
    Text(&'static str),
    /// `ItemFeature`（`choice` を呼ぶ）。
    Feature(&'static ItemFeature),
    /// `DiceTable` の表（`roll` して `to_s` する）。
    Table(&'static ChainTable),
    /// 自分自身（`ItemFeaturesTable` を再帰的に引く）。
    SelfRef,
}

/// Ruby `ItemFeaturesTable#roll(randomizer)`。
fn item_features_roll(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice = rng.roll_sum(2, 6)?;
    let index = dice - 2;
    // Ruby: @items[index] は 2..12 の範囲で必ず取れる（範囲外なら nil.map で例外）。
    let row: &[FeatureRowItem] = usize::try_from(index)
        .ok()
        .and_then(|i| MKB_ITEM_FEATURES_ROWS.get(i))
        .copied()
        .unwrap_or(&[]);

    let mut body = String::new();
    for item in row {
        match item {
            FeatureRowItem::Text(t) => body += t,
            FeatureRowItem::Feature(f) => body += &f.choice(rng)?,
            FeatureRowItem::Table(t) => body += &t.roll(rng)?.to_string(),
            FeatureRowItem::SelfRef => body += &item_features_roll(rng)?,
        }
    }

    Ok(format!("特性[{dice}]：{body}"))
}

/// Ruby `MeikyuKingdomBasic#roll_device_factory_table(num)`。
fn roll_device_factory_table(num: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let item = MKB_ITEM_RANDOM_TABLE.roll(rng)?.last_body().to_string();
    let intro =
        format!("デヴァイス・ファクトリー表 (特性{num}個) ＞ ベースアイテム：{item}（もしくは任意のアイテム）");

    // Ruby: num = [0, num].max
    let num = num.max(0);
    let mut lines = vec![intro];
    for _ in 0..num {
        lines.push(item_features_roll(rng)?);
    }
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// ロジックを持つ表
// ---------------------------------------------------------------------------

/// Ruby `MeikyuKingdomBasic#mk_new_name_table`。戻り値は `[output, dice]`。
fn mk_new_name_table(rng: &mut Randomizer) -> Result<(String, String), EvalError> {
    let nick_n = rng.roll_once(6)?;
    let name_n = rng.roll_once(6)?;
    let d1 = rng.roll_d66(D66SortType::Asc)?;
    let d2 = rng.roll_d66(D66SortType::Asc)?;

    // Ruby: 二つ名の分岐は最初だけ nick_n、以降は name_n を見る（原典どおり）。
    let nick_table = if nick_n <= 1 {
        n_number(d1, MKB_NICK_PR_TABLE)
    } else if name_n <= 2 {
        n_number(d1, MKB_NICK_FO_TABLE)
    } else if name_n <= 3 {
        n_number(d1, MKB_NICK_OU_TABLE)
    } else if name_n <= 4 {
        n_number(d1, MKB_NICK_TI_TABLE)
    } else if name_n <= 5 {
        n_number(d1, MKB_NICK_PH_TABLE)
    } else {
        n_number(d1, MKB_NICK_CO_TABLE)
    };

    let name_table = if name_n <= 1 {
        n_number(d2, MKB_NAME_AR_TABLE)
    } else if name_n <= 2 {
        n_number(d2, MKB_NAME_FO_TABLE)
    } else if name_n <= 3 {
        n_number(d2, MKB_NAME_DN_TABLE)
    } else if name_n <= 4 {
        n_number(d2, MKB_NAME_PL_TABLE)
    } else if name_n <= 5 {
        n_number(d2, MKB_NAME_MA_TABLE)
    } else {
        n_number(d2, MKB_NAME_GO_TABLE)
    };

    Ok((
        format!("{nick_table}{name_table}"),
        format!("{nick_n},{name_n},{d1},{d2}"),
    ))
}

/// Ruby `MeikyuKingdomBasic#mk_combination_nickname_table`。戻り値は `[output, total]`。
fn mk_combination_nickname_table(rng: &mut Randomizer) -> Result<(String, String), EvalError> {
    let num1 = rng.roll_once(6)?;
    let num2 = rng.roll_d66(D66SortType::Asc)?;
    let num3 = rng.roll_d66(D66SortType::Asc)?;

    let output = match num1 {
        1 => format!(
            "{}、{}",
            n_number(num2, MKB_TERMINOLOGY_TABLE),
            n_number(num3, MKB_RR236_NAME_TABLE)
        ),
        2 => format!(
            "{}、{}",
            n_number(num2, MKB_TERMINOLOGY_TABLE),
            n_number(num3, MKB_TITLE_TABLE)
        ),
        3 => format!(
            "{}、{}",
            n_number(num2, MKB_BEHAVIOR_TABLE),
            n_number(num3, MKB_RR236_NAME_TABLE)
        ),
        4 => format!(
            "{}、{}",
            n_number(num2, MKB_BEHAVIOR_TABLE),
            n_number(num3, MKB_TITLE_TABLE)
        ),
        5 => format!(
            "{}、{}",
            n_number(num2, MKB_DESCRIPTIVE_TABLE),
            n_number(num3, MKB_RR236_NAME_TABLE)
        ),
        6 => format!(
            "{}、{}",
            n_number(num2, MKB_DESCRIPTIVE_TABLE),
            n_number(num3, MKB_TITLE_TABLE)
        ),
        // Ruby: どの when にも当たらなければ output は "" のまま
        _ => String::new(),
    };

    Ok((output, format!("{num1},{num2},{num3}")))
}

/// Ruby `MeikyuKingdomBasic#get_demand_table(level)`。戻り値は `[output, dice]`。
fn get_demand_table(level: i64, rng: &mut Randomizer) -> Result<(String, String), EvalError> {
    let mut dice: Vec<i64> = Vec::new();
    let mut count = [0i64; 6];
    for _ in 1..=level {
        let num = rng.roll_once(6)?;
        dice.push(num);
        if let Some(slot) = usize::try_from(num - 1).ok().and_then(|i| count.get_mut(i)) {
            *slot += 1;
        }
    }

    let mut output = String::new();
    for idx in 1..=6i64 {
        let c = count[(idx - 1) as usize];
        if c > 0 {
            if !output.is_empty() {
                output += "\n";
            }
            output += n_number(idx, MKB_DEMAND_TABLE);
        }

        if level >= 3 && c > 1 {
            if idx == 1 {
                output += &format!(" ＞ 維持費{level}MG上昇。");
            } else {
                output += &format!(" ＞ {level}MG獲得。");
            }
        }
    }

    dice.sort_unstable();
    let dice_str: Vec<String> = dice.iter().map(|d| d.to_string()).collect();
    Ok((output, dice_str.join(",")))
}

/// Ruby `MeikyuKingdomBasic#get_race_name_character_table(count)`。戻り値は `[output, total_n]`。
fn get_race_name_character_table(
    count: i64,
    rng: &mut Randomizer,
) -> Result<(String, String), EvalError> {
    let mut output = String::new();
    let mut total_n = String::new();
    for _ in 1..=count {
        let a: i64 = rng.roll_barabara(2, 6)?.iter().sum();
        let b: i64 = rng.roll_barabara(2, 6)?.iter().sum();
        if !total_n.is_empty() {
            total_n += ",";
        }
        total_n += &format!("{a},{b}");
        output += n_number(a * 100 + b, MKB_RACE_NAME_CHARACTER_TABLE);
    }
    Ok((output, total_n))
}

/// Ruby `MeikyuKingdomBasic#get_currency_material_table(num1, num2)`。
fn get_currency_material_table(num1: i64, num2: i64) -> String {
    let sub = match num1 {
        2 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE2),
        3 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE3),
        4 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE4),
        5 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE5),
        6 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE6),
        7 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE7),
        8 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE8),
        9 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE9),
        10 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE10),
        11 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE11),
        12 => n_number(num2, MKB_CURRENCY_MATERIAL_TABLE12),
        _ => "",
    };
    format!("{}:{sub}", n_number(num1, MKB_CURRENCY_MATERIAL_TABLE))
}

/// Ruby `MeikyuKingdomBasic#get_expedition_itinerary_table(modify)`。戻り値は `[output, num]`。
fn get_expedition_itinerary_table(
    modify: i64,
    rng: &mut Randomizer,
) -> Result<(String, i64), EvalError> {
    let sum: i64 = rng.roll_barabara(2, 6)?.iter().sum();
    let num = sum.saturating_add(modify).clamp(0, 14);
    Ok((
        n_number(num, MKB_EXPEDITION_ITINERARY_TABLE).to_string(),
        num,
    ))
}

/// Ruby `MeikyuKingdomBasic#get_modify(modify)`（`Arithmetic.eval(modify, @round_type).to_i`）。
///
/// `@round_type` は `RoundType::CEIL`。式として解釈できなければ Ruby の `nil.to_i` で0。
fn get_modify(modify: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(modify, RoundType::Ceil)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `MeikyuKingdomBasic#mk_kingdom_environment_table(num)`（項目が `lambda` の表）。
///
/// Ruby は表を作る前に1D6を振るので、`num` が範囲外でもダイスは消費される。
fn mk_kingdom_environment_table(num: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let d1 = rng.roll_once(6)?;
    let table: [&[(i64, &'static str)]; 6] = [
        MKB_TECHNIC_DECIDE_TABLE,
        MKB_NATIONAL_STYLE_DECIDE_TABLE,
        MKB_RESOURCE_DECIDE_TABLE,
        MKB_FACILITY_DECIDE_TABLE,
        MKB_HUMAN_RESOURCES_DECIDE_TABLE,
        MKB_BLOOD_DECIDE_TABLE,
    ];
    match table
        .iter()
        .enumerate()
        .find(|(i, _)| (*i as i64) + 1 >= num)
    {
        None => Ok("1".to_string()),
        Some((_, t)) => Ok(n_number(d1, t).to_string()),
    }
}

// ---------------------------------------------------------------------------
// ディスパッチャ
// ---------------------------------------------------------------------------

/// Ruby `Base#roll_tables(command, tables)`。キーの完全一致で引く。
/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    command: &str,
    tables: &'static [(&'static str, &'static dyn RollableTable)],
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    table_helpers::roll_table(command, tables, rng)
}

/// Ruby `/^DFT(\d*)$/i`。
fn dft_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DFT(\d*)$").expect("valid regex"))
}

/// Ruby `/^NNAME(\d*)/i` などの「接頭辞＋個数」パターン。
fn count_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(?:NNAME|KNT|DEMT|RNCT)(\d*)").expect("valid regex"))
}

/// `command` の接頭辞に続く数字を取り出す。
fn captured_count(command: &str) -> &str {
    count_pattern()
        .captures(command)
        .and_then(|c| c.get(1))
        .map_or("", |m| m.as_str())
}

/// Ruby `/^EXIT([-+]?\d*)/i`。
fn exit_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^EXIT([-+]?\d*)").expect("valid regex"))
}

/// Ruby `MeikyuKingdomBasic#eval_game_system_specific_command`。
fn eval_specific_basic(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    for tables in [MKB_TABLES, MKB_A2Z_TABLES, MKB_RR236_TABLES, MKB_TK_TABLES] {
        if let Some(output) = roll_tables(command, tables, rng)? {
            return Ok(Some(output));
        }
    }

    if let Some(m) = dft_pattern().captures(command) {
        // Ruby: return roll_device_factory_table(feature_count)（"#{type}表(...)" で包まない）
        // `Regexp.last_match(1).to_i` 相当。空文字は0。i64 に収まらない桁数は0扱いにする
        // （Rubyは巨大な回数だけループしてしまう経路）。
        let feature_count: i64 = m[1].parse().unwrap_or(0);
        return Ok(Some(roll_device_factory_table(feature_count, rng)?));
    }

    let (type_name, output, total_n): (&str, String, String) = if head(command, "NMAR") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "芸術系名前",
            n_number(n, MKB_NAME_AR_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NMFO") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "食べ物系名前",
            n_number(n, MKB_NAME_FO_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NMDN") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "日用品系名前",
            n_number(n, MKB_NAME_DN_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NMPL") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "地名系名前",
            n_number(n, MKB_NAME_PL_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NMMA") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "機械系名前",
            n_number(n, MKB_NAME_MA_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NMGO") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "神様系名前",
            n_number(n, MKB_NAME_GO_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NNAME") {
        let count = get_count(captured_count(command));
        let mut names = String::new();
        let mut total = String::new();
        for _ in 0..count.max(0) {
            let (name, dice) = mk_new_name_table(rng)?;
            names += &format!("[{dice}]{name} ");
            // Ruby も `total_n = count` をループ内で行う（0回なら "" のまま）
            total = count.to_string();
        }
        // Ruby: count が0だと output が nil のままで `nil.strip` の NoMethodError。
        // ここでは空文字列として扱う（total_n も "" のまま）。
        let stripped = names
            .trim_matches(|c: char| c.is_ascii_whitespace())
            .to_string();
        ("新名前", stripped, total)
    } else if head(command, "KNT") && !captured_count(command).is_empty() {
        let count = get_count(captured_count(command));
        let n = rng.roll_d66(D66SortType::Asc)?;
        let out = match count {
            1..=3 => (MKB_HOOKS.kingdom_name_tables[(count - 1) as usize])(n),
            // Ruby: case count に一致する when がなく output は nil のまま → super
            _ => return eval_specific(command, rng, &MKB_HOOKS),
        };
        ("王国名決定", out, n.to_string())
    } else if head(command, "KET") {
        let n = rng.roll_once(6)?;
        let out = mk_kingdom_environment_table(n, rng)?;
        ("王国環境", out, n.to_string())
    } else if head(command, "TET") {
        let n = rng.roll_once(6)?;
        (
            "技術決定",
            n_number(n, MKB_TECHNIC_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NST") {
        let n = rng.roll_once(6)?;
        (
            "国風決定",
            n_number(n, MKB_NATIONAL_STYLE_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "RET") {
        let n = rng.roll_once(6)?;
        (
            "資源決定",
            n_number(n, MKB_RESOURCE_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "FAT") {
        let n = rng.roll_once(6)?;
        (
            "施設決定",
            n_number(n, MKB_FACILITY_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "HRT") {
        let n = rng.roll_once(6)?;
        (
            "人材決定",
            n_number(n, MKB_HUMAN_RESOURCES_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "BLT") {
        let n = rng.roll_once(6)?;
        (
            "血族決定",
            n_number(n, MKB_BLOOD_DECIDE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "GUCT") {
        let n = rng.roll_once(6)? * 100 + rng.roll_once(6)? * 10 + rng.roll_once(6)?;
        (
            "守護星座",
            n_number(n, MKB_GUARDIAN_CONSTELLATION_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "DEMT") && !captured_count(command).is_empty() {
        let count = get_count(captured_count(command));
        let (out, total) = get_demand_table(count, rng)?;
        ("需要", out, total)
    } else if head(command, "RNCT") {
        let count = get_count(captured_count(command));
        let (out, total) = get_race_name_character_table(count, rng)?;
        ("人種名文字", out, total)
    } else if head(command, "CUMT") {
        let sum: i64 = rng.roll_barabara(2, 6)?.iter().sum();
        let d = rng.roll_once(6)?;
        (
            "通貨材質",
            get_currency_material_table(sum, d),
            format!("{sum},{d}"),
        )
    } else if head(command, "CMNT") {
        let (out, total) = mk_combination_nickname_table(rng)?;
        ("組み合わせ系二つ名", out, total)
    } else if head(command, "DEST") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "形容",
            n_number(n, MKB_DESCRIPTIVE_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "TRMT") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "用語",
            n_number(n, MKB_TERMINOLOGY_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "BEHT") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "行動",
            n_number(n, MKB_BEHAVIOR_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "NAMT") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "呼び名",
            n_number(n, MKB_RR236_NAME_TABLE).to_string(),
            n.to_string(),
        )
    } else if head(command, "TTLT") {
        let n = rng.roll_d66(D66SortType::Asc)?;
        (
            "称号",
            n_number(n, MKB_TITLE_TABLE).to_string(),
            n.to_string(),
        )
    } else if let Some(m) = exit_pattern().captures(command) {
        let modify = get_modify(&m[1])?;
        let (out, num) = get_expedition_itinerary_table(modify, rng)?;
        ("遠征道中", out, num.to_string())
    } else {
        // Ruby: どの when にも当たらず output は nil のまま → super(command)
        return eval_specific(command, rng, &MKB_HOOKS);
    };

    Ok(Some(format!("{type_name}表({total_n}) ＞ {output}")))
}

// ---------------------------------------------------------------------------
// 表データ（原典rbから機械抽出。値は1文字も変えていない）
// ---------------------------------------------------------------------------

/// Ruby `才覚休憩表`の項目。
static MKB_TABLE_TBT_ITEMS: &[&str] = &[
    "寝付けないので、民と噂話に花を咲かせる。すると、経費削減のアイデアが……。[才覚/9]の判定を行う。成功すると、このセッションの《維持費》を（1D6）MG減少できる。",
    "自分の嫌いなものに追い回される夢を見る。心寂しくなったところに、仲間が様子を見に来てくれた。宮廷の中からキャラクター1人を選ぶ。そのキャラクターへの《好意》+1。",
    "好きなものの夢を見る。鳴呼、もっと……もっと……。好きなもの1つを選ぶ。その好きなものに関する幸せそうなシチュエーションを考え、他のプレイヤーやGMに伝える。その夢が幸せそうだと感じる者がいたら、《気力》+2。",
    "さて一眠りするか……というときに、1人の民が青い顔をして震えている。どうやら、 国に残した家族のことが心配なようだ。[才覚/11]の判定を行う。成功すると、《民の声》+2。",
    "「もう少しだ。頑張ろう」あなたは、あらん限りの力をこめて、仲間に呼びかけた。[才覚/9]の判定を行う。成功すると、宮廷のキャラクターは《気力》を1点ずつ消費できる。消費した《気力》と合計値だけ《民の声》が回復する。",
    "配下や仲間たちに指示を出し、休憩中も休む暇なく働く。くたくたになって、あくびをすると配下がお茶を差し入れてくれた。《民の声》+1。",
    "地図を前にして、今後の冒険について口角泡を飛ばす。意見の対立はあったが、あなたの意見が通った。我々に必要なのは英雄的死亡ではなく、卑劣な生存なのだ。 宮廷の好きなキャラクター1体を選ぶ。そのキャラクターの自分に対する《敵意》を好きなだけ上昇させ、上昇した値だけ《民の声》を回復する。",
    "たまには、わたしが料理してみるか……。【お弁当】か【フルコース】の効果を使用して、食事をとることができる。食事をしたら、（1D6）を振る。奇数だったら思いのほか美味しい出来映え。《民の声》+1。偶数だったら腹にはたまるが二度とごめんという出来映え。宮廷全員のあなたに対する《敵意》+1。",
    "配下の中でも年若い者たちがあなたの周りに群がり、冒険の話を聞かせてくれとせがむ。[才覚/現在の《民の声》の値+3] の判定を行う。成功すれば、《民の声》+（1D6）。失敗すると、次の1クォーターは行動ができない。",
    "迷宮に囚われた哀れな人々を見つける。助けたいのはやまやまだが、食料がやや心配だ。[才覚/9]の判定を行う。成功すると、自分の《配下》+（1D6）人。",
    "「やはりな……」迷宮は予想通り、一筋縄ではいかないようだ。こんなときこそ、準備しておいたアレが役に立つ。自分の修得しているスキル1種を選ぶ。そのスキルを喪失して、そのスキルと同じスキルグループのスキル1種を修得してもよい。この効果は永続する。",
];

/// Ruby `才覚休憩表`（`2D6`）。
static MKB_TABLE_TBT: Table = Table::from_dice("才覚休憩表", 2, 6, MKB_TABLE_TBT_ITEMS);

/// Ruby `魅力休憩表`の項目。
static MKB_TABLE_CBT_ITEMS: &[&str] = &[
    "妖精のワイン蔵を発見、酒盛りが始まる。宮廷全員の《気力》+1。[魅カ/9]の判定に失敗すると、酔っ払ったあなたは服を脱ぎはじめる。（1D6）を振る。自分を除く宮廷全員のあなたに対する《感情値》+1、奇数ならその属性が《好意》、偶数なら《敵意》になる。",
    "「実はわたし……むにゃむにゃむにゃ」休憩中、意外な寝言を言ってしまう。自分を除く宮廷全員は、自分に対する《好意》と《敵意》を反転させることができる。",
    "休憩中、冷たい床があなたの体温を奪っていく。あなたは、無意識のうちにぬくもりを求め、体を寄せ合う。あなたに《好意》を持っているキャラクターの数だけ、《気カ》と《HP》が回復する。",
    "こっそり2人で抜け出していい雰囲気に。その部屋の中に、自分と好きなものが同じキャラクターがいれば、そのキャラクター1体を選び、互いに対する《好意》+1。",
    "星の灯りがあなたの顔をロマンチックに照らし出す。その部屋にいる人物の中から好きなキャラクター1人を選び、[魅力/9+そのキャラクターのあなたに対する《好意》]の判定を行う。成功すると、そのキャラクターのあなたに対する《好意》+1。",
    "あいつと目が合う。[魅力/9]の判定を行う。成功したら、自分以外の宮廷の中から、ランダムにキャラクター1体を選ぶ。そのキャラクターから自分に対する《好意》か、自分からそのキャラクターに対する《好意》かのいずれかが1点上昇する。",
    "見張りの途中にうたた寝。目を覚ますと、誰かが毛布をかけてくれていた。ランダムにキャラクターを選ぶ。自分のそのキャラクターに対する《好意》+1。",
    "野営に最適な場所を見つける。たき火を囲みながら、思い思い会話を楽しむ。GMの左隣にいるプレイヤーから順番に、自分のPCが《好意》を持っているキャラクター1体を選ぶ。選ばれたキャラクターは、《気力》+1。誰からも選ばれなかったキャラクターは《気力》-1、宮廷の中からランダムにキャラクター1体を選ぶ。そのキャラクターに対する《敵意》+1。",
    "疲れた体を癒やすため、テントの中で楽な衣装に着替えよう。するとそこに侵入者が……。宮廷からランダムにキャラクターを1人選び（1D6）を振る。奇数ならあなたは大声を出し、宮廷全員のそのキャラクターに対する《敵意》+1。偶数ならそのキャラクターとあなたの互いに対する《好意》+1。",
    "部屋のすみに隠れていた怪物が現れた！ すぐには襲いかかってこないようだが……。[魅力/10]の判定を行う。成功すれば怪物と友好関係を結ぶことができる。自分のレベル以下のモンスター1体を選び、そのモンスターが自分の《配下》になる。失敗すると、モンスターに襲われる。宮廷全員の《HP》が（1D6）点減少する。",
    "ふとした拍子に唇が触れあう★ 好きなキャラクター1体を選ぶ。そのキャラクターの自分以外に対する《好意》を合計し、その値を自分に対する《好意》に加える。その後、そのキャラクターの自分以外に対する《好意》をすべて0にする。",
];

/// Ruby `魅力休憩表`（`2D6`）。
static MKB_TABLE_CBT: Table = Table::from_dice("魅力休憩表", 2, 6, MKB_TABLE_CBT_ITEMS);

/// Ruby `探索休憩表`の項目。
static MKB_TABLE_SBT_ITEMS: &[&str] = &[
    "一休みする前に道具の手入れ。使い慣れた道具ほど手になじむ。ランダムに自分の装備しているアイテム1つを選ぶ。そのアイテムのレベルが1上昇する。",
    "寝床を探していたら、アルコーブがあり、その奥に宝箱をみつける。[探索/9]の判定を行う。成功すると、好きな素材1種類を選び、それを(1D6)個獲得する。",
    "民が寝静まったあと、あなたも一眠り。するとその夢の中で……。[探索/11]の判定を行う。成功したら、好きな部屋を指定する。その部屋の脅威情報を、GMから教えてもらうことができる。 ",
    "配下が眠りにつき、部屋が静寂に包まれると、隣の部屋から妙な音が聞こえる。この部屋に隣接する好きな部屋1つを選ぶ。[探索/9]の判定に成功すると、その部屋のモンスターの種類と数が分かる。",
    "一休みしようと思ったら、モンスターの墓場を発見！ みんなで捜索だ。好きな素材を1種類選ぶ。宮廷全員の中で、あなたに対する《好意》の合計値だけ、その素材が手に入る。",
    "この部屋はなぜか落ち着く。もしも、その部屋の中にあなたの好きなものがあれば、《気力》を（1D6）点回復することができる。あなたはGMにその部屋に自分の好きなものがないか質問してもよい。",
    "壁に描かれた奇妙な壁画が、あなたを見つめているような気がする……。[探索/9]の判定を行う。成功すると、【エレベータ】を発見する。",
    "白骨化した先客の死体が見つかる。使えそうな装備は、ありがたく頂戴しておこう。[探索/10]の判定を行う。成功したら、コモンアイテムのカテゴリの中から好きなもの1つを選び、その中からランダムに決めたアイテム1個を手に入れる。",
    "星の灯りで地図を眺める。この部屋の構造からすると、この辺りに何かあるはずなんだが……？ [探索/10]の判定に成功すると、この部屋に仕掛けられたイベント型のトラップをすべて発見する。",
    "自然の呼び声。休んでいる間にトイレにいきたくなった……。[探索/10]の判定を行う。成功すると、その部屋に迷宮のほころびを見つける。このセッションの間、この部屋から迷宮の外に帰還することができる。",
    "こ、これは秘密の扉！？ [探索/11]の判定を行う。成功すると、この部屋に隣接する好きな部屋に通路を伸ばすことができる。",
];

/// Ruby `探索休憩表`（`2D6`）。
static MKB_TABLE_SBT: Table = Table::from_dice("探索休憩表", 2, 6, MKB_TABLE_SBT_ITEMS);

/// Ruby `武勇休憩表`の項目。
static MKB_TABLE_VBT_ITEMS: &[&str] = &[
    "時が満ちるにつれ、闘志が高まる。現在の経過ターン数と等しい値だけ、《気力》が回復する。",
    "もっと……もっと敵と戦いたい。血に飢えた自分を発見する。[武勇/9]の判定を行う。成功すると、《気力》+1、《HP》が（1D6）点回復する。",
    "部屋の片隅にうち捨てられたむごたらしい亡骸を発見する。このマップの支配者の名前が分かっていれば、宮廷全員、このマップの支配者への《敵意》+1できる。",
    "部屋のすみに隠れていた怪物が、休憩中の民に襲いかかる！ あなたは、咄嗟に武器を手にし、怪物たちに躍りかかった！ [武勇/9]の判定を行う。成功すれば怪物を追い払い、《民の声》+1。失敗すると、自分の《配下》-（1D6）人、《民の声》-1。",
    "危ない！ 短剣があなたの横をかすめる。すると、そこにはあなたに躍りかかろうとしていた毒蛇が。もしかして、アイツのことを誤解していたかも……。自分が《敵意》を持っているキャラクター1体を選び、そのキャラクターに対する《好意》+2。",
    "少し見ないうちに、恐るべき実力を身につけている。今のうちに潰しておくか……。あなたの中にドス黒い気持ちがわき上がる。名前を知っているキャラクター1体を選び、そのキャラクターへの《敵意》+1。",
    "ちょっとした行き違いから、軽い口論になってしまう。宮廷の中からランダムにキャラクターを1体選ぶ。そのキャラクターとあなたの互いに対する《敵意》+1。",
    "ライバルの活躍が気になる。宮廷全員の中で、あなたに対する最も高い《敵意》の値と同じだけ《気力》を獲得する。",
    "休むときに休まなければ、いざというときに戦えない。他の仲間にまかせて、しっかりと体を休めることにする。《HP》を（2D6）点回復することができる。",
    "この足跡は……もしや？ 怪物のいた痕跡を発見する。[武勇/10]の判定を行う。成功すると、このゲームで遭遇する予定のまだ種類の分かっていないモンスターを1種類、GMから教えてもらうことができる。",
    "……殺気！ あなたは、毛布をはねのけ、戦闘態勢を整えるよう指示した。「特殊遭遇表」を1回使用し、その後、好きな素材を（1D6）個獲得する。さらに、ランダムにレアアイテム1種を選び、それを手に入れる。",
];

/// Ruby `武勇休憩表`（`2D6`）。
static MKB_TABLE_VBT: Table = Table::from_dice("武勇休憩表", 2, 6, MKB_TABLE_VBT_ITEMS);

/// Ruby `才覚ハプニング表`の項目。
static MKB_TABLE_THT_ITEMS: &[&str] = &[
    "自分に王国を導くことなど可能なのだろうか……。【お酒】を1個消費することができなければ、このセッションの間、[才覚]-1。",
    "国王の威信が問われる。（2D6）を振り、その値が[《民の声》+宮廷全員の国王に対する《好意》の合計]以上だった場合、《民の声》-（1D6）、さらにもう1度（2D6）を振って、才覚ハプニング表の効果を適用する。",
    "思考に霧の帳が降りる。「散漫2」の変調を受ける。",
    "重大な裏切りを犯してしまう！ あなたに対する《好意》が最も高いキャラクターを1人選ぶ。そのキャラクターのあなたに対する《感情値》を《敵意》に反転させる。",
    "この人についていっていいのだろうか……？ 宮廷全員のあなたに対する《好意》-1(0未満にはならない)。その結果、誰かの《好意》が0になると《民の声》-1。",
    "宮廷のスキャンダルが暴露される！ 宮廷全員のあなたに対する《敵意》の中で、最も高い値と同じだけ《民の声》が減少する。",
    "あなたの失策が近隣で噂になる。近隣の国からランダムに国を1つ選ぶ。その国との関係が1段階悪化する。",
    "王国の経済に破綻の危険が発見される。[生活レベル/9+現在の経過ターン数]の判定を行う。失敗すると、維持費が（1D6）MG上昇する。",
    "この区画一帯の疲労が一層激しくなる。1クォーターが経過する。",
    "逸材の賃上げ要求が始まる。終了フェイズの予算会議のとき、[今回使用した逸材の数×1]MGだけ維持費が上昇する。",
    "今の自分に自信が持てなくなる。生まれ表からランダムにジョブを1つ選び、現在のジョブをそのジョブに変更する。",
];

/// Ruby `才覚ハプニング表`（`2D6`）。
static MKB_TABLE_THT: Table = Table::from_dice("才覚ハプニング表", 2, 6, MKB_TABLE_THT_ITEMS);

/// Ruby `魅力ハプニング表`の項目。
static MKB_TABLE_CHT_ITEMS: &[&str] = &[
    "民同士のいさかいに心を痛め、頭髪にダメージが！ 【お酒】を1個消費することができなければ、このセッションの間、[魅力]-1。",
    "あなたの何気ない一言が不和の種に……。好きなキャラクター1人選ぶ。そのキャラクターに対する宮廷全員の《敵意》+1。",
    "あなたの美しさに嫉妬した迷宮が、あなたの姿を変える。「呪い3」の変調を受ける。",
    "可愛さあまって憎さ百倍。あなたに対する《好意》が最も高いキャラクターを1人選ぶ。そのキャラクターのあなたに対する《感情値》を《敵意》に反転する。",
    "あなたをめぐって不穏な空気……。宮廷全員のあなたに対する愛情の《好意》を比べ、上から2人を選ぶ。その2人の互いに対する《敵意》+1。",
    "いがみ合う宮廷の面々を見て、民の士気が減少する。宮廷全員のあなたに対する《敵意》の中で、最も高い値と同じだけ、自分の《配下》が減少する。",
    "宮廷に嫉妬の嵐が巻き起こる。宮廷の中で、あなたに対して《好意》を持つキャラクターの数を数える。このセッションの間、行為判定を行うとき、サイコロの目の合計がこの数以下だった場合、絶対失敗となる(2未満にはならない)。",
    "愛想をつかされる。宮廷全員のあなたに対する《好意》-1(0未満にはならない)。",
    "あなたの指揮に疑問を訴える者が……。[魅力/自分の《配下》の値×1]の判定を行う。失敗した場合、[難易度-達成値]人の《配下》が減少する。",
    "あなたの恋人だという異性が現れる！ 宮廷全員のあなたに対する《好意》を比べ、最も高いキャラクターを1人選ぶ。そのキャラクターの[武勇]の値と同じだけ《HP》を減少する。",
    "他人が信用できなくなる。このセッションの間、協調行動を行えなくなる。",
];

/// Ruby `魅力ハプニング表`（`2D6`）。
static MKB_TABLE_CHT: Table = Table::from_dice("魅力ハプニング表", 2, 6, MKB_TABLE_CHT_ITEMS);

/// Ruby `探索ハプニング表`の項目。
static MKB_TABLE_SHT_ITEMS: &[&str] = &[
    "指の震えが止まらない……。【お酒】を1個消費することができなければ、このセッション中、[探索]-1。",
    "流れ星に直撃。《HP》-（1D6）。",
    "敵の過去を知り、相手に同情してしまう。あなたは、このマップの支配者に対する《好意》+1。このセッションの間、《好意》を持ったキャラクターに対して攻撃を行い、絶対失敗した場合、その《好意》の値だけ《気力》が減少する。",
    "昨日の友は今日の敵。あなたに対する《好意》が最も高いキャラクターを1人選ぶ。そのキャラクターのあなたに対する《感情値》を《敵意》に反転する。",
    "うっかりアイテムを落として壊してしまう。ランダムにアイテムスロットを1つ選ぶ。そのスロットにアイテムが入っていれば、そのアイテムをすべて破壊する。",
    "カーネルが活性化し、トラップが強化される。このセッションの間、トラップを解除するための難易度+1。",
    "友情にヒビが！ 宮廷全員のあなたに対する《敵意》+1。",
    "敵の疲労攻撃！ 宮廷全員は[探索/11]の判定を行う。失敗したキャラクターは（2D6）点のダメージを受ける。",
    "つい出来心から、国費に手を出してしまう。GMは好きなコモンアイテム1つを選ぶ。そのキャラクターはそのアイテムを入手するが、維持費+（1D6）、《民の声》-1。同じ部屋に別のPCがいれば、《希望》1点消費し、[探索/9]の判定に成功すればそれを止めることができる。",
    "封印されていたトラップを作動させてしまう。ランダムに災害系トラップの中から1つ選ぶ。そのトラップが発動する。",
    "あなたを憎む迷宮支配者が、あなたの首に賞金をかけた。このセッションの間、モンスターの攻撃やトラップの目標をランダムに決める場合、その目標は必ずあなたになる(この効果を2人以上が受けた場合、この効果を受けた者の中でランダムに決定する)。",
];

/// Ruby `探索ハプニング表`（`2D6`）。
static MKB_TABLE_SHT: Table = Table::from_dice("探索ハプニング表", 2, 6, MKB_TABLE_SHT_ITEMS);

/// Ruby `武勇ハプニング表`の項目。
static MKB_TABLE_VHT_ITEMS: &[&str] = &[
    "つい幼児退行を起こしそうになる。【お酒】を1個消費することができなければ、このセッション中、[武勇]-1。",
    "バカな！ 不意打ちか！？ 次に行う戦闘は奇襲扱いとなる。",
    "配下の期待が、あなたの重荷となる。[現在の《民の声》-1D6]点だけ《気力》が減少する。",
    "「あ、危ないッ！」配下があなたをかばう！ 自分の《配下》-（1D6）。",
    "ムカついたので思わず殴る。自分の《敵意》の中で、最も高いキャラクターをランダムに1人選ぶ。そのキャラクターの《HP》が、自分の[武勇]と等しい値だけ減少する。",
    "決闘だッ！ 宮廷全員のあなたに対する《敵意》の中で、最も高い値を選ぶ。その値の分だけ、あなたの《HP》が減少し、《気力》+2。",
    "豚どもめ……。宮廷全員に対する《敵意》+1。",
    "古傷が痛み出す。このセッションの間、戦闘であなたに対する敵の攻撃が成功すると、常に1点余分にダメージを受ける。",
    "不意に絶望と虚無感が襲い、あなたたちの心が折れる。宮廷全員の《気力》-1。",
    "あなたの親の仇を名乗るものたちが現れた。ランダムにセッション中に倒したモンスターの中から1種類を選ぶ。そのモンスター（1D6）体と戦闘を行うこと。",
    "自分の失敗が許せない。このセッションの間、《器》が1点減少したものとして扱う。",
];

/// Ruby `武勇ハプニング表`（`2D6`）。
static MKB_TABLE_VHT: Table = Table::from_dice("武勇ハプニング表", 2, 6, MKB_TABLE_VHT_ITEMS);

/// Ruby `王国災厄表`の項目。
static MKB_TABLE_KDT_ITEMS: &[&str] = &[
    "王国の悪い噂が蔓延する。既知の土地にある他国との関係が、すべて1段階悪化する。",
    "自国のモンスターが凶暴化する！ 自国の《モンスターの民》の中からランダムに1種類のモンスターを選ぶ。自国の《民》を[そのモンスターのレベル]人減少する。また、そのモンスターと同じ種類の《モンスターの民》は、すべて王国からいなくなる。",
    "王国に疫病が大流行……。自国に残した《民》を[自国に残した《民》の数×1/10]人減少する。",
    "自国の疲労が進行する。自国の領土のマップ数と等しい値のMGだけ維持費が上昇する。",
    "敵国のテロリズムが横行！ [治安レベル/9]の判定を行う。失敗すると、ランダムに選んだ施設1件が破壊される。",
    "敵国の襲来！ あなたがたの留守を狙って、敵国が同盟を結んで奇襲を行う。[軍事レベル/9]の判定を行う。失敗すると、ランダムに選んだ自国の領土1つを失う。",
    "敵国が陰謀を仕掛けてくる。[文化レベル/9]の判定を行う。失敗すると、ランダムに選んだ逸材1人を失う。",
    "食糧危機が発生！ [生活レベル/9]の判定を行う。失敗すると、自国に残した《民》を[自国に残した《民》×1/5]人減少する。王国にある「肉」の素材1個を消費するたびに、《民》の減少を5人軽減することができる。",
    "王国が何者かに呪われる。このセッションの間、国力を使った行為判定で選んだ(2D6)の目が3以下だと、絶対失敗になる。",
    "極地的な迷宮津波が発生。ランダムに自国の領土のマップ1つを選ぶ。その後、既知の土地の中からランダムに土地1つを選ぶ。その2つの場所を入れ替える。",
    "敵国の勢力が強大化する。GMは、関係が敵対の国すべてについて、その国の領土に接する好きな土地1つを選ぶ。その土地をその国の領土にする。",
];

/// Ruby `王国災厄表`（`2D6`）。
static MKB_TABLE_KDT: Table = Table::from_dice("王国災厄表", 2, 6, MKB_TABLE_KDT_ITEMS);

/// Ruby `王国変動表`の項目。
static MKB_TABLE_KCT_ITEMS: &[&str] = &[
    "列強のプロパガンダが現れる。（1D6）を振り、その目が現在の《民の声》以下で、現在列強の属国になっていたら属国から抜けることができる。上回っていたら、ランダムに列強を1つ選びその属国になる。",
    "冒険の成功を祝う民たちが出迎えてくれる。《民の声》+2。この結果を出したプレイヤー以外の全員は、今回の冒険を振り返り当プレイヤーのPCが《好意》を得るとしたら誰が一番ふさわしいかを協議する。決定したキャラへのPCの《好意》+1",
    "唐突な奇襲。周辺階域の中からランダムに自国の領土を選び[軍事レベル/9]の判定を行う。成功すれば（1D6）MG獲得。失敗すると選ばれた領土の入口から順番に通路を辿り失われる部屋を（[王国レベル+1]D6）個選ぶ。（同じ部屋は2度選べない）。失われた部屋の施設と部屋につながる道が全て破壊される。その部屋からすべての部屋がなくなり、終了フェイズで入口が1個もなければ自国の領土でなくなる。",
    "民の労働の結果が明らかに。[生活レベル/9]の判定に成功すると《予算》が自国の領土のマップ数と同じだけ増える。失敗したら《予算》が同じだけ減る。",
    "あなたの活躍を耳にした者たちがやってくる。シナリオの目的を満たしている場合、関係が良好・同盟の国の数だけ（1D6）を振り、[合計値+治安レベル]人だけ《民》が増える。",
    "王国の子どもたちがあなた方を見て成長する。』《民》が([王国に残した《民》の数÷10＋治安レベル]D6)人増える。",
    "民は領土を渇望していた。5MGを支払えば、隣接する未知の土地1つを領土にできる。（1D6）を振り、その数だけ通路を引くことができる。通路でつながっていない部屋は自国の領土として扱わない。",
    "街の機能に異変が！？ [治安レベル/9]の判定に成功すると、自国の好きな施設1軒を選び、その施設のレベルを1点上昇する。失敗したら、自国のタイプ：部屋の施設をランダムに1軒選び、破壊する。",
    "王国同士の交流が行われた。[文化レベル/9]の判定に成功すると、生まれ表でランダムにジョブを決めた逸材が1人増え、好きな国1つとの関係を1段階良好にする。失敗すると、自国の逸材1人を選んで失い、ランダムに決めた国1つとの関係が1段階悪化する。",
    "ただ無為に時が過ぎていたわけではない。冒険フェイズで過ごした1ターンにつき予算が1MG増える。",
    "民の意識が大きく揺れる。（1D6）を振り、その目が現在の《民の声》以下だったら、好きな国力を選び基本値が1点上昇する（基本値を3点以上にはできない）。出目が上回っていたら、好きな国力が1点減少する。",
];

/// Ruby `王国変動表`（`2D6`）。
static MKB_TABLE_KCT: Table = Table::from_dice("王国変動表", 2, 6, MKB_TABLE_KCT_ITEMS);

/// Ruby `痛打表`の項目。
static MKB_TABLE_CAT_ITEMS: &[&str] = &[
    "あなたの攻撃の手応えが、武器に刻まれる。その攻撃に使用した武具アイテムのレベルが1点上昇する。",
    "電光石火の一撃。攻撃の処理が終了した後、もう一度、行動を行うことができる。",
    "凄まじい一撃は、相手の姿形を変えるほどだ。攻撃目標に「呪い4」の変調を与える。",
    "乾坤一擲！ その攻撃のダメージを算出したあと、それをさらに2倍にすることができる。",
    "凄まじい威力で相手を吹き飛ばす。攻撃目標を好きなエリアに移動させる。",
    "会心の一撃！！ ダメージが（1D6）点上昇する。",
    "敵の勢いを利用し、大ダメージ！ ダメージが攻撃目標のレベルと同じ値だけ上昇する。",
    "あと1歩まで追い詰める。ダメージを与える代わりに、攻撃目標の残り《HP》を（1D6）点にすることができる。",
    "狙いが的中！ 敵の技を封じる！ 攻撃目標のスキル1種を選ぶ。その戦闘の間、そのスキルを喪失させる。",
    "怒りの一撃！ ダメージが（2D6）点上昇する。",
    "敵の急所をとらえ、一撃のもとに斬り伏せる。攻撃目標の《HP》を0点にする。",
];

/// Ruby `痛打表`（`2D6`）。
static MKB_TABLE_CAT: Table = Table::from_dice("痛打表", 2, 6, MKB_TABLE_CAT_ITEMS);

/// Ruby `致命傷表`の項目。
static MKB_TABLE_FWT_ITEMS: &[&str] = &[
    "圧倒的な攻撃が、急所を貫く。死亡する。",
    "致命的な一撃が、頭をかすめる。[探索/5+受けたダメージ]の判定に成功すると、行動不能になる。判定に失敗すると、死亡する。",
    "昏睡し、体中から血と生命の息吹が失われつつある。行動不能になる。この戦闘が終了するまでに《HP》を1点以上にしないと、そのキャラクターは死亡する。",
    "頭を強くうちつけ、昏睡している。行動不能になる。このクォーターが終了するまでに《HP》を1点以上にしないと、そのキャラクターは死亡する。",
    "重傷を負い、意識を失う。行動不能になる。（1D6）クォーターが経過するまでに《HP》を1点以上にしないと、そのキャラクターは死亡する。",
    "すさまじい一撃に意識を失う。行動不能になる。",
    "偶然、アイテムが衝撃からキミを護る。装備しているアイテムから、ランダムに1つを選ぶ。そのアイテムを破壊し、ダメージを無効にする。もし、破壊できるアイテムを1つも装備していないと行動不能になる。",
    "《民》たちが、その身を犠牲にしてキミを護る。自分の《配下》を（2D6）人減少し、ダメージを無効にする。もし、《配下》が1人もいなければ、行動不能になる。",
    "根性で攻撃を跳ね返す！ [探索/5+受けたダメージ]の判定を行う。成功すると、《HP》が1点になる。失敗すると、行動不能になる。",
    "精神力だけで耐え忍ぶ。[武勇/5+受けたダメージ]の判定を行う。成功すると、《HP》が1点になる。失敗すると、行動不能になる。",
    "幸運なことに、ダメージは避けられる。しかし、ランダムに変調1つを選び、それを受ける。数値がある場合、3になる。",
];

/// Ruby `致命傷表`（`2D6`）。
static MKB_TABLE_FWT: Table = Table::from_dice("致命傷表", 2, 6, MKB_TABLE_FWT_ITEMS);

/// Ruby `戦闘ファンブル表`の項目。
static MKB_TABLE_CFT_ITEMS: &[&str] = &[
    "敵に援軍が現れる！ 敵軍の中でもっともレベルの低いモンスターが（1D6）体増える。モンスターがこの結果になった場合、好きなPCの《配下》が（1D6）体上昇する。",
    "敵の士気がおおいに揺らぐ。自軍のキャラクター全員は1マス後退する。",
    "勢いあまって仲間を攻撃！ 自分のいるエリアの中から、ランダムに自軍キャラクター1人を選ぶ。そのキャラクターに使用している武器と同じ威力のダメージを与える。",
    "つい仲間と口論に。自軍の未行動のキャラクターの中からランダムに1人選ぶ。そのキャラクターが行動済みになる。",
    "馬鹿な！ 魔法の効果が！ 自軍のキャラクターが使用したスキルやアイテムの効果で、その戦闘の間持続するものが、全て無効になる。",
    "いてててて。自分を傷つけてしまう。自分に（1D6）点ダメージ。",
    "自分の攻撃の勢いを利用され、相手の反撃を受ける。自分の《HP》を現在の値の半分にする。",
    "おおっと、アイテムを落っことした。自分が装備しているアイテムからランダムに1個を選ぶ。そのアイテムが破壊される。モンスターの場合、自分に（1D6）ダメージ。",
    "激しい戦いに、カーネルが活性化。戦闘系トラップからランダムに1種類を選ぶ。その場に、トラップが配置される。",
    "あなたの攻撃は空をきり、絶望に囚われる。自分と、自分に対して1点以上《好意》を持ったキャラクター全員の《気力》-1 。モンスター側の場合、自分に（1D6）点ダメージ。",
    "あっ！ 武器がすっぽぬけた。攻撃に使用していたアイテムが破壊される。モンスターの場合、自分に（1D6）点ダメージ。さらに、戦場シートにいるキャラクターの中からランダムにキャラクター1体を選ぶ。そのキャラクターの《HP》が1点になる。",
];

/// Ruby `戦闘ファンブル表`（`2D6`）。
static MKB_TABLE_CFT: Table = Table::from_dice("戦闘ファンブル表", 2, 6, MKB_TABLE_CFT_ITEMS);

/// Ruby `道中表`の項目。
static MKB_TABLE_TT_ITEMS: &[&str] = &[
    "道中の時間が、人間関係に変化をもたらす。全員、好きなキャラクター1人を選ぶ。そのキャラクターに対する《感情値》が1点上昇する。",
    "ん？ 何かの死体が転がっている。好きな素材1種類を選ぶ。宮廷のPC1人は、その素材を（1D6）個手に入れる。",
    "カーネルの異常が発生し、あたりが闇に包まれる。宮廷の中から、ランダムにPC1人を選ぶ。そのPCが【星の欠片】を持っていたら、それが1個破壊される。",
    "迷宮災厄のせいか、道に迷いそうになる。全員、[才覚/9]の判定を行う。[（1D6）-成功したPCの数]クォーターの時間が経過する(0クォーター未満にはならない)。",
    "陰湿なトラップにひっかかる。全員、[探索/9]の判定を行う。失敗したPCは、《HP》を（1D6）点減少する。",
    "迷宮は不気味に静まり返っている……。特に何も起こらなかった。",
    "モンスターの襲撃を受ける。全員、[武勇/9]の判定を行う。失敗したPCは、《HP》を（1D6）点減少する。",
    "恐ろしげな咆哮があたりに響き、すぐに静まり返る。全員、[魅力/9]の判定を行う。失敗したPCは、《配下》が（1D6）人自国に逃走する。",
    "迷宮災厄発生！ 気がつくと自分たちの王国に戻っていた。",
    "を？ 何かが落ちてるぞ。ランダムにコモンアイテム1個を選ぶ。そのアイテムを手に入れる。",
    "ラッキー♪ 1MGを拾った。",
];

/// Ruby `道中表`（`2D6`）。
static MKB_TABLE_TT: Table = Table::from_dice("道中表", 2, 6, MKB_TABLE_TT_ITEMS);

/// Ruby `交渉表`の項目。
static MKB_TABLE_NT_ITEMS: &[&str] = &[
    "中立的な態度は偽装だった。彼らは油断をついて不意打ちを行う。奇襲扱いで戦闘を行うこと。",
    "交渉は決裂！ 戦闘を行うこと。",
    "交渉は決裂！ 戦闘を行うこと。",
    "「贄をささげれば話を聞こう」モンスターの中でもっともレベルが高いもののレベルと等しい数だけ《配下》を消費すれば、モンスターたちは友好的になる。ただし《民の声》を（1D6）点減少する。《配下》を消費しない場合、戦闘を行うこと。",
    "「……お前の趣味、なに？」好きな単語表1個を選び、（D66）を振る。宮廷の中に、その項目を好きなものにしているPCがいれば、モンスターたちは友好的になる。そうでなければ、戦闘を行うこと。",
    "怪物たちは、物欲しそうにこちらを見ている。「肉」の素材をモンスターの数だけ消費するか、【お弁当】、【フルコース】1個を消費すれば、モンスターたちは友好的になる。消費しなければ、戦闘を行うこと。",
    "怪物たちは、値踏みするようにこちらを見ている。維持費を（1D6）MG上昇させれば、モンスターたちは友好的になる。上昇させなければ、戦闘を行うこと。",
    "「何かいいもんよこせ」モンスターの中でもっともレベルが高いもののレベル以上の価格のアイテムを消費すれば、モンスターたちは友好的になる。レアアイテムは、()内の数字に10を足したものとして考える。それを渡せなければ、戦闘を行うこと。",
    "「面白い話を聞かせろよ」怪物たちは、面白い話を要求してきた。プレイヤーたちは、モンスターたちが興味のありそうな話を聞かせること。GMはその話を聞いて面白いと思えば、宮廷の代表に[魅力/9]の判定を行わせること。成功した場合、モンスターたちは友好的になる。失敗した場合、戦闘を行うこと。",
    "「俺に勝てたら話を聞いてやろう」怪物が一騎打ちを申し込んできた。宮廷の代表は[武勇/モンスターの中で最も高い[武勇]+7]の判定を行う。判定に成功すると、モンスターたちは友好的になる。失敗すると、判定を行った者が《HP》を（1D6）点減少した後、全員で戦闘を行うこと。",
    "運命の出会い。一目見た瞬間、うち解け合った。モンスターたちの宮廷の代表に対する《好意》+1、さらにモンスターたちは友好的になる。",
];

/// Ruby `交渉表`（`2D6`）。
static MKB_TABLE_NT: Table = Table::from_dice("交渉表", 2, 6, MKB_TABLE_NT_ITEMS);

/// Ruby `感情表`の項目。
static MKB_TABLE_ET_ITEMS: &[&str] = &["忠誠", "友情", "愛情", "怒り", "不信", "侮蔑"];

/// Ruby `感情表`（`1D6`）。
static MKB_TABLE_ET: Table = Table::from_dice("感情表", 1, 6, MKB_TABLE_ET_ITEMS);

/// Ruby `お祭り表`の項目。
static MKB_TABLE_FRT_ITEMS: &[&str] = &[
    "祈願祭。国や重要人物の無病息災を祈ったり、戦いの勝利などを祈る祭り。災害や飢饉、流行り病が起こった付近で行われる。シナリオの目的をクリアしていれば、《民》+（1D6）。",
    "血祭り。戦いに向け、士気を向上させる祭り。戦争だけでなく、迷宮探索に向けて行われることも多い。生贄の血を軍神に捧げたりする。このゲームの間、戦闘に勝利すると《民の声》+1、逃走すると《民の声》-1。",
    "記念日。建国記念日や領土獲得などの記念日のお祝い。簡単につくることができるが、気がつくと記念日だらけで、何の記念だったかを忘れてしまう。ほどほどに。このセッションの間、行為判定の目で3でも絶対失敗、11でも絶対成功になる（「呪い」の変調を受けているものは、行為判定のサイコロの目が[呪いの数値+1]以下で絶対失敗が発生する。）。",
    "星祭。季節のお祭り。冬至や夏至などの祭りや、七夕、お花見、雪祭りなどが含まれる。季節感の少ない迷宮では、殊更にその風情を楽しもうとやたら盛り上がる。宮廷全員、好きなキャラクター1人を選び、そのキャラクターに対する《好意》+1。",
    "民衆の宴。民が自発的に開くお祭り、イベント。アキハバラ電気祭りに餃子祭り、コミックマーケットなど、文化や地域の活性化と結びつくものが多い。このセッションの間、好きな施設1つを選んで、その施設の施設レベル+1。",
    "誕生日。ランドメイカーや逸材、国の重要人物の誕生日。聖誕祭や花祭りなど、国教の聖人などを祝う国も多い。現王の誕生日を「父の日」、后の誕生日を「母の日」とする国も多い。そのゲームの間、ケーキやおにぎり、缶ジュースなど、1人分が明確な食べ物を食べきったとき、自分のPCが《気力》1点を獲得する。",
    "冠婚葬祭。国の重要人物の元服（成人）、婚礼、葬儀、祖先の慰霊などの儀式。格式の高い王国では、もっとも重要な祭礼である。このセッションの間、国力を使った判定の達成値+1。",
    "感謝祭。豊漁や豊作などがあったときに自然（迷宮）や精霊、信仰対象など、偉大なるものへの感謝を捧げるお祭り。獲物の毛の一部を切りとって迷宮に感謝する毛祭りや瀬祭り、豊饒を祝う新嘗祭などがある。王国変動表を使用したとき、1回だけ「木」や「革」、「肉」のいずれかを1つ消費すると、その結果を±1の範囲でずらすことができる。",
    "鬼祭り。お正月に旧年の悪を正す修正会、豆をまいて福を呼び込む追儺の儀式、怪物に仮装した子供たちが夜の王国をねり歩くハロウィーンなど、悪魔や悪霊を払うお祭り。モンスター除けに行われる。このセッションの間2回だけ、戦闘後に使用する「お宝表」を1段階高いランクのものを使用する。",
    "舞踏会。最高の音楽と芸術的な食事、そしてとびきりの衣装で臨む社交界の華。身分や素性を隠してパートナーを探す仮面舞踏会も人気は高い。ちなみに仮面舞踏会では、女性の側から男性をダンスに誘うのが礼儀だぞ。宮廷全員、ランダムにキャラクター1人を選び、そのキャラクターに対する《好意》+1。",
    "競技会。国をあげて、スポーツや芸術、ゲームなど、さまざまなジャンルの一番を決めるお祭り、大会。オリンピックや料理勝負、歌合戦などがある。ランダムに能力値1つを選び、宮廷全員は【その能力値/15】の判定を行う。このとき成功した中で、もっとも達成値が高かったキャラクターは、シナリオ終了後、終了フェイズの探索会議で決定されるキャラクターとは別に、勲章を得る。",
];

/// Ruby `お祭り表`（`2D6`）。
static MKB_TABLE_FRT: Table = Table::from_dice("お祭り表", 2, 6, MKB_TABLE_FRT_ITEMS);

/// Ruby `お祭り休憩表`の項目。
static MKB_TABLE_FBT_ITEMS: &[&str] = &[
    "お祭りに向かう旅人たちとすれ違う。《予算》を3MG獲得する。自国に【宿屋】か【夜店】があればさらに（1D6）MG獲得する。",
    "なんでこんなときに、ダンジョンに行かなきゃいけないんだ！ 「あ、電報でーす」。このマップの支配者から、お祭りによせて祝辞の電報がやってくる。そうか、オマエのせいかッ！！ マップの支配者の名前が分かり、そのキャラクターへの《敵意》+（1D6）。",
    "「そういえば、国のみんなが何か言ってたなぁ……」回想シーン。「視察表」を1回使用する。",
    "あー、早く帰って、お祭りを楽しみたーい！ この時点でキャンプを終了し、すぐに次の部屋に移動すれば、このクォーターは時間の経過が発生しない。",
    "どこからか美味しそうな匂いが漂ってくる。「あ、うまそう」死んだふりをしていた民が起き上がる。《配下》を（1D6）人回復する。",
    "雰囲気がいつもと違うせいかな。なんかあの人がステキに見える。好きなキャラクターを1人選ぶ。そのキャラクターへの《好意》+1。",
    "あ、こんなところにまで屋台が！　あてくじ屋さんだ。1MG減らして、好きなアイテムカテゴリを選び、さらにそのカテゴリの中からランダムにアイテム1種を選ぶ。そのアイテムを1個獲得する（レアアイテムは飾ってあるが、絶対当たらない）。",
    "お祭りを目指す交易商人と出会う。「あ、王様。これから王国行くんすよ」宮廷の持つ好きな素材を何個でも、同じ数の別の好きな素材と交換してくれる。",
    "せっかくお祭りなんだし、肩肘はってないでノリノリでGO！！　このゲーム中は食事をするたびに、《民の声》+1。この効果は累積しない。",
    "「あ、この歌は……」祭囃子がキミの封印されていたモンスターにまつわる過去の記憶を呼び戻す。好きなモンスター1種類選ぶ。そのモンスターへの《敵意》+1。この感情値は、そのモンスター全般へのものになる。",
    "みんなのわくわくがアイテムに乗り移った？　ランダムに自分のアイテムスロット1つを選ぶ。そのアイテムのレベルを1点上昇する。",
];

/// Ruby `お祭り休憩表`（`2D6`）。
static MKB_TABLE_FBT: Table = Table::from_dice("お祭り休憩表", 2, 6, MKB_TABLE_FBT_ITEMS);

/// Ruby `全体休憩表`の項目。
static MKB_TABLE_WBT_ITEMS: &[&str] = &[
    "部屋の中から使えそうな装備をみつくろう。宮廷全員は、それぞれ好きなコモンアイテムのカテゴリを選び、ランダムにコモンアイテムを1個獲得する。そのアイテムにレベルがあれば、それは1レベルのものとなる。",
    "みんなで今後の作戦を練る。宮廷全員は、そのターンの間、あらゆる判定の達成値+1。この効果は累積しない。",
    "手分けして物資の調達を行う。各キャラクターは、好きな素材を（1D6）個獲得できる。このとき、各キャラクターはアイテム作成を1回行うことができる。",
    "体を休めながら他愛もない世間話に花が咲く。宮廷全員は、それぞれ宮廷の中から好きなキャラクター1人を選び、そのキャラクターに対する《好意》+1。",
    "宮廷メンバーで交代で見張りを行い、疲労した配下たちを休ませる。《民の声》を[宮廷の人数]点回復する。",
    "一行はしっかりと休息を取り、鋭気を養う。宮廷全員の《気力》+2。",
    "配下たちと一緒に体を休める。《民の声》+（1D6）。",
    "配下たちに見張りを任せ、体を休める。宮廷全員の《HP》を最大値まで回復する。",
    "緊急ミーティング！　国家運営に関してみんなで知恵を出し合う。《予算》を[宮廷の人数]MG獲得する。",
    "負傷した配下たちの治療を行う。宮廷全員の《配下》が（1D6）人回復する。",
    "宮廷の前に光り輝くアイテムが降臨する。レア武具アイテムかレア一般アイテムのどちらかを選ぶ。ランダムにそのアイテムを1種類選び、それを1個獲得する。",
];

/// Ruby `全体休憩表`（`2D6`）。
static MKB_TABLE_WBT: Table = Table::from_dice("全体休憩表", 2, 6, MKB_TABLE_WBT_ITEMS);

/// Ruby `カップル休憩表`の項目。
static MKB_TABLE_LBT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("「あーもう、最悪！」仲良く休憩するつもりが、ひどい喧嘩になってしまう。「カップル休憩表」使用者のお互いに対する《敵意》+2。")),
    (12, TableItem::Text("「何もかもがお前が悪かったのかッ！！」大きな誤解が生まれる。受け身キャラの攻め気キャラ以外に対する《感情値》がすべて0になり、その値の分だけ攻め気キャラに対する《敵意》が上昇する。")),
    (13, TableItem::Text("「でさぁ、あの人のことなんだけど……」せっかく2人きりなのに、他人の話で盛り上がる。「カップル休憩表」使用者は、宮廷の中から自分たち以外のキャラクター1人を選び、そのキャラクターに対する《好意》+1。")),
    (14, TableItem::Text("「へぇ、そんなのあるんだ」互いの好きなものについて語り合う。受け身キャラは、攻め気キャラの「好きなもの」1つを選ぶ。受け身キャラは、自分の「好きなもの」1つをそれに変更し、攻め気キャラへの《好意》+1。")),
    (15, TableItem::Text("「なぁ、オレのことどう思う？」思い切った質問！　受け身キャラは、攻め気キャラに対する《好意》か《敵意》を1点上昇させ、その属性を好きなものに変更できる。")),
    (16, TableItem::Text("「だいじょうぶ？　無茶するんだから」少し前の失敗について色々と言われてしまう。ありがたいんだけど、少しムカつく。攻め気キャラは受け身キャラに対する《好意》+1、受け身キャラは攻め気キャラに対する《敵意》+1。")),
    (22, TableItem::Text("「え、もうこんな時間！？」一休みするつもりが、気がつくとかなり時間がたっている。キャンプが終了すると、通常の時間の経過に加え、さらに1クォーターが経過する。「カップル休憩表」使用者のお互いに対する《好意》+1。また、「カップル休憩表」使用者以外のキャラクターは、使用者2人に対する《敵意》+1。")),
    (23, TableItem::Text("「ねぇねぇ、これわかる？」何気ない質問だが、これは難しい。変な答えはできないぞ。攻め気キャラは[才覚/9]の判定を行う。成功すると、「カップル休憩表」使用者のお互いに対する《好意》+1。失敗すると、何とか危機を切り抜けることができるが、受け身キャラの攻め気キャラに対する《敵意》+1。")),
    (24, TableItem::Text("「おいおい、まずは落ち着け！」配下同士が喧嘩を始めた。うまく仲裁しないと……。攻め気キャラは、[魅力/9]の判定を行う。成功すると、「カップル休憩表」使用者のお互いに対する《好意》+1。失敗すると、何とか危機を切り抜けることができるが、受け身キャラの攻め気キャラに帯する《敵意》+1。")),
    (25, TableItem::Text("「オレが解除するからちょっと待ってろ」2人で休憩するつもりが、一緒にトラップにひっかかってしまった。互いの体が密着してしまう。攻め気キャラは、[探索/9]の判定を行う。成功すると、「カップル休憩表」使用者のお互いに対する《好意》+1。失敗すると、何とか危機を切り抜けることができるが、受け身キャラの攻め気キャラに対する《敵意》+1。")),
    (26, TableItem::Text("「お前は後ろに下がってろ！」休憩中に怪物に襲われる。攻め気キャラは、[武勇/9]の判定を行う。成功すると、「カップル休憩表」使用者のお互いに対する《好意》+1。失敗すると、何とか危機を切り抜けることができるが、受け身キャラの攻め気キャラに対する《敵意》+1。")),
    (33, TableItem::Text("「なぁ、さっきは悪かったな」誤解が解ける。「カップル休憩表」使用者のお互いに対する《好意》+1。")),
    (34, TableItem::Text("「これを言ったのはあなただけです。誰にも言わないでくださいね」受け身キャラが隠している夢や秘密を攻め気キャラが知ってしまう。受け身キャラの攻め気キャラに対する《好意》+1。攻め気キャラの受け身キャラに対する《感情値》が《好意》になり、その属性を「忠誠」にする。")),
    (35, TableItem::Text("「これからも、よろしく頼むぜ。相棒」攻め気キャラが快活に微笑む。受け身キャラの攻め気キャラに対する《好意》+1。攻め気キャラの受け身キャラに対する《感情値》が《好意》になり、その属性を「友情」にする。")),
    (36, TableItem::Text("「わ、わたしは、あなたのことが……」受け身キャラの思わぬ告白！　受け身キャラの攻め気キャラに対する《好意》+1。攻め気キャラの受け身キャラに対する《感情値》が《好意》になり、その属性を「愛情」にする。")),
    (44, TableItem::Text("「大丈夫？　痛くないか？」互いに傷を治療しあう。「カップル休憩表」使用者は、お互いの自分に対する《好意》の分だけ、自分の《HP》を回復することができる。どちらかが《HP》を1点以上回復したら、この表の使用者のお互いに対する《好意》+1。")),
    (45, TableItem::Text("「この冒険が終わったら、伝えたいことが……あるんだ」攻め気キャラの真剣な言葉。え、それって……？　受け身キャラの攻め気キャラに対する《好意》+2。終了フェイズのエピローグ時に攻め気キャラが生きていれば、受け身キャラになにかを伝える。受け身キャラは、それを聞いて《好意》を最大2点まで上昇できる。")),
    (46, TableItem::Text("「蝕ッ！？　……って、どこ触ってるんですかッ！？」あたりが不意に暗くなり、思わず変なところを触ってしまう。攻め気キャラの受け身キャラに対する《好意》+2、受け身キャラの攻め気キャラに対する《敵意》+2。「カップル休憩表」使用者のどちらか装備・収納している【星の欠片】1個を消費すれば、このイベントを無効化できる。")),
    (55, TableItem::Text("「これ、はんぶんこしない？」2人仲良く、アイテムを分け合う。「カップル休憩表」使用者が消費アイテムを持っていれば、それを1個使用できる。ただし、その効果の目標は、「カップル休憩表」使用者の2人に変更される。「カップル休憩表」使用者のお互いに対する《好意》+1。")),
    (56, TableItem::Text("「え？　え？　えぇぇぇぇッ！？」ふとした拍子に唇がふれあう。受け身キャラの攻め気キャラ以外に対する《好意》がすべて0点になり、その値の分だけ攻め気キャラに対する《好意》を上昇する。")),
    (66, TableItem::Text("「…………」気がつくとお互い、目をそらせなくなってしまう。そのまま顔を寄せ合い……。「カップル休憩表」使用者のお互いに対する《好意》+2、その属性を「愛情」にする。")),
];

/// Ruby `カップル休憩表`（D66）。
static MKB_TABLE_LBT: D66Table =
    D66Table::new("カップル休憩表", D66SortType::Asc, MKB_TABLE_LBT_ITEMS);

/// Ruby `お宝１表`の項目。
static MKB_TABLE_T1T_ITEMS: &[&str] = &[
    "何もなし",
    "何もなし",
    "そのモンスターの素材欄の中から、好きな素材１個",
    "そのモンスターの素材欄の中から、好きな素材２個",
    "そのモンスターの素材欄の中から、好きな素材３個",
    "【お弁当】１つを手に入れる",
];

/// Ruby `お宝１表`（`1D6`）。
static MKB_TABLE_T1T: Table = Table::from_dice("お宝１表", 1, 6, MKB_TABLE_T1T_ITEMS);

/// Ruby `お宝２表`の項目。
static MKB_TABLE_T2T_ITEMS: &[&str] = &[
    "そのモンスターの素材欄の中から、好きな素材３個",
    "そのモンスターの素材欄の中から、好きな素材４個",
    "そのモンスターの素材欄の中から、好きな素材５個",
    "ランダムに回復アイテム１個を選ぶ",
    "ランダムに武具アイテム１個を選ぶ。そのアイテムにレベルがあれば、１レベルのものが手に入る",
    "ランダムにレア一般アイテム１個を選び、それを手に入れる",
];

/// Ruby `お宝２表`（`1D6`）。
static MKB_TABLE_T2T: Table = Table::from_dice("お宝２表", 1, 6, MKB_TABLE_T2T_ITEMS);

/// Ruby `お宝３表`の項目。
static MKB_TABLE_T3T_ITEMS: &[&str] = &[
    "そのモンスターの素材欄の中から、好きな素材５個",
    "そのモンスターの素材欄の中から、好きな素材７個",
    "そのモンスターの素材欄の中から、好きな素材１０個",
    "好きなコモンアイテムのカテゴリ１種を選び、そのカテゴリからランダムにアイテム１個を選ぶ。そのアイテムにレベルがあれば、アイテムなら１レベルのものが手に入る",
    "ランダムにレア一般アイテム１個を選ぶ。そのアイテムにレベルがあれば、１レベルのものが手に入る",
    "ランダムにレア武具アイテム１個を選び、それを手に入れる",
];

/// Ruby `お宝３表`（`1D6`）。
static MKB_TABLE_T3T: Table = Table::from_dice("お宝３表", 1, 6, MKB_TABLE_T3T_ITEMS);

/// Ruby `お宝４表`の項目。
static MKB_TABLE_T4T_ITEMS: &[&str] = &[
    "そのモンスターの素材欄の中から、好きな素材５個",
    "そのモンスターの素材欄の中から、好きな素材１０個",
    "好きなコモンアイテムのカテゴリ１種を選び、そのカテゴリからランダムにアイテム１個を選ぶ。そのアイテムにレベルがあれば、２レベルのものが手に入る",
    "好きなコモンアイテムのカテゴリ１種を選び、そのカテゴリからランダムにアイテム１個を選ぶ。そのアイテムにレベルがあれば、３レベルのものが手に入る",
    "ランダムにレア一般アイテム１個を選ぶ。そのアイテムにレベルがあれば、２レベルのものが手に入る",
    "ランダムにレア武具アイテム１個を選ぶ。そのアイテムにレベルがあれば、１レベルのものが手に入る",
];

/// Ruby `お宝４表`（`1D6`）。
static MKB_TABLE_T4T: Table = Table::from_dice("お宝４表", 1, 6, MKB_TABLE_T4T_ITEMS);

/// Ruby `お宝５表`の項目。
static MKB_TABLE_T5T_ITEMS: &[&str] = &[
    "そのモンスターの素材欄の中から、好きな素材１０個",
    "そのモンスターの素材欄の中から、好きな素材１５個",
    "好きなコモンアイテムのカテゴリ１種を選び、そのカテゴリからランダムにアイテム１個を選ぶ。そのアイテムにレベルがあれば、４レベルのものが手に入る",
    "ランダムにレア一般アイテム１個を選ぶ。そのアイテムにレベルがあれば、３レベルのものが手に入る",
    "ランダムにレア武具アイテム１個を選ぶ。そのアイテムにレベルがあれば、２レベルのものが手に入る",
    "好きなレアアイテム１個を選び、それを入手する",
];

/// Ruby `お宝５表`（`1D6`）。
static MKB_TABLE_T5T: Table = Table::from_dice("お宝５表", 1, 6, MKB_TABLE_T5T_ITEMS);

/// Ruby `上級肉弾スキル表`の項目。
static MKB_TABLE_ABUS_ITEMS: &[&str] = &[
    "屈強",
    "屈強",
    "追い討ち",
    "追い討ち",
    "即席武器",
    "即席武器",
];

/// Ruby `上級肉弾スキル表`（`1D6`）。
static MKB_TABLE_ABUS: Table = Table::from_dice("上級肉弾スキル表", 1, 6, MKB_TABLE_ABUS_ITEMS);

/// Ruby `上級射撃スキル表`の項目。
static MKB_TABLE_ASHS_ITEMS: &[&str] = &[
    "先制射撃",
    "先制射撃",
    "鷹の目",
    "鷹の目",
    "ブルズアイ",
    "ブルズアイ",
];

/// Ruby `上級射撃スキル表`（`1D6`）。
static MKB_TABLE_ASHS: Table = Table::from_dice("上級射撃スキル表", 1, 6, MKB_TABLE_ASHS_ITEMS);

/// Ruby `上級星術スキル表`の項目。
static MKB_TABLE_AASS_ITEMS: &[&str] = &[
    "星に願いを",
    "星に願いを",
    "星のこえ",
    "星のこえ",
    "破裂星",
    "破裂星",
];

/// Ruby `上級星術スキル表`（`1D6`）。
static MKB_TABLE_AASS: Table = Table::from_dice("上級星術スキル表", 1, 6, MKB_TABLE_AASS_ITEMS);

/// Ruby `上級召喚スキル表`の項目。
static MKB_TABLE_ASUS_ITEMS: &[&str] = &[
    "式神",
    "式神",
    "お引っ越し",
    "お引っ越し",
    "戦闘召喚",
    "戦闘召喚",
];

/// Ruby `上級召喚スキル表`（`1D6`）。
static MKB_TABLE_ASUS: Table = Table::from_dice("上級召喚スキル表", 1, 6, MKB_TABLE_ASUS_ITEMS);

/// Ruby `上級科学スキル表`の項目。
static MKB_TABLE_ASCS_ITEMS: &[&str] = &[
    "蘇生",
    "蘇生",
    "強化術式",
    "強化術式",
    "心霊研究",
    "心霊研究",
];

/// Ruby `上級科学スキル表`（`1D6`）。
static MKB_TABLE_ASCS: Table = Table::from_dice("上級科学スキル表", 1, 6, MKB_TABLE_ASCS_ITEMS);

/// Ruby `上級迷宮スキル表`の項目。
static MKB_TABLE_ALAS_ITEMS: &[&str] = &[
    "迷宮工事",
    "迷宮工事",
    "迷核解析",
    "迷核解析",
    "轟宮",
    "轟宮",
];

/// Ruby `上級迷宮スキル表`（`1D6`）。
static MKB_TABLE_ALAS: Table = Table::from_dice("上級迷宮スキル表", 1, 6, MKB_TABLE_ALAS_ITEMS);

/// Ruby `上級交渉スキル表`の項目。
static MKB_TABLE_ANES_ITEMS: &[&str] = &["色気", "色気", "威光", "威光", "挑発", "挑発"];

/// Ruby `上級交渉スキル表`（`1D6`）。
static MKB_TABLE_ANES: Table = Table::from_dice("上級交渉スキル表", 1, 6, MKB_TABLE_ANES_ITEMS);

/// Ruby `上級便利スキル表`の項目。
static MKB_TABLE_ACOS_ITEMS: &[&str] = &["心眼", "心眼", "隠し味", "隠し味", "ながら", "ながら"];

/// Ruby `上級便利スキル表`（`1D6`）。
static MKB_TABLE_ACOS: Table = Table::from_dice("上級便利スキル表", 1, 6, MKB_TABLE_ACOS_ITEMS);

/// Ruby `上級芸能スキル表`の項目。
static MKB_TABLE_AENS_ITEMS: &[&str] = &["即興詩", "即興詩", "国歌", "国歌", "隠し芸", "隠し芸"];

/// Ruby `上級芸能スキル表`（`1D6`）。
static MKB_TABLE_AENS: Table = Table::from_dice("上級芸能スキル表", 1, 6, MKB_TABLE_AENS_ITEMS);

/// Ruby `上級道具スキル表`の項目。
static MKB_TABLE_ATOS_ITEMS: &[&str] = &["中かばん", "中かばん", "節約", "節約", "相棒", "相棒"];

/// Ruby `上級道具スキル表`（`1D6`）。
static MKB_TABLE_ATOS: Table = Table::from_dice("上級道具スキル表", 1, 6, MKB_TABLE_ATOS_ITEMS);

/// Ruby `ランダムマップ選択表`の項目（6行×6列）。
static MKB_TABLE_RMS_ITEMS: &[&[&str]] = &[
    &["A-1", "A-1", "A-2", "A-2", "A-3", "A-3"],
    &["A-1", "A-1", "A-2", "A-2", "A-3", "A-3"],
    &["B-1", "B-1", "B-2", "B-2", "B-3", "B-3"],
    &["B-1", "B-1", "B-2", "B-2", "B-3", "B-3"],
    &["C-1", "C-1", "C-2", "C-2", "C-3", "C-3"],
    &["C-1", "C-1", "C-2", "C-2", "C-3", "C-3"],
];

/// Ruby `ランダムマップ選択表`（D66の6×6表）。
static MKB_TABLE_RMS: D66GridTable = D66GridTable::new("ランダムマップ選択表", MKB_TABLE_RMS_ITEMS);

/// Ruby `視察表`の項目。
static MKB_TABLE_RT_ITEMS: &[&str] = &[
    "神託が下る。苦難がPCを襲うが、それは救いのための試練である。このセッションの間、PCが10点以上のダメージをモンスターから受けるたび《民の声》+1。",
    "長老が迷宮の昔話をしてくれた。この表を使用したPCが判定で失敗したとき、その判定のサイコロを振り直すことができる。この効果は、このセッションの間に1回だけ使用できる。",
    "民は怪物の脅威に怯えている。この表を使用したPCがモンスターの《HP》を0点にすると、《民の声》+2。この効果は、このセッションの間に1回だけ使用できる。",
    "日用品が不足しているという不満を持つ民がいるようだ。このセッションの間、自国に「革」を5個輸送するたび《民の声》+1。",
    "民たちは王国の守りが薄いのではという不安を抱えていた。このセッションの間、自国に「鉄」を5個輸送するたび《民の声》+1。",
    "主婦たちが食糧不足に対する不安を訴えてきた。このセッションの間、自国に「肉」を5個輸送するたび《民の声》+1。",
    "民たちは新しい施設の建設を望んでいる。このセッションの間、自国に「木」を5個輸送するたび《民の声》+1。",
    "武器の備えが乏しいのではないかという不安があるようだ。このセッションの間、自国に「牙」を5個輸送するたび《民の声》+1。",
    "配下にした若者が熱心に未来を語る。この表を使用したPCは《配下》を1人消費して、《特殊配下》を1人増やす。その《特殊配下》に名前をつけ、「生まれ表」でなりたいジョブを決定すること。なりたいジョブに対応した能力値(その《特殊配下》がなりたいジョブの能力値ボーナス欄に書いてある能力値)を使った判定で、このセッションの間に自分が絶対成功すると、その《特殊配下》は、そのジョブの逸材になる。",
    "王国は活気に満ちている。この表を使用したPCは《気力》+1、もう一度王国フェイズに行動することができる。",
    "民たちはワクワクするような冒険譚を求めている！ このセッションのシナリオの目的を達成していたら、終了フェイズの円卓会議の開始時に、（1D6）MGが手に入る。",
];

/// Ruby `視察表`（`2D6`）。
static MKB_TABLE_RT: Table = Table::from_dice("視察表", 2, 6, MKB_TABLE_RT_ITEMS);

/// Ruby `特殊遭遇表`の項目。
static MKB_TABLE_SE_ITEMS: &[&str] = &[
    "宙を舞う【グレムリン】が、宮廷の方を物欲しそうに眺めている。宮廷の中で、素材欄に「機械」が含まれているアイテムを持っているPC全員は、[才覚/7+装備している素材欄に「機械」が含まれるアイテムの数]の判定を行う。失敗したPCは、そのアイテムをすべて破壊し、[装備している素材欄に「機械」が含まれるアイテムの数]D6点のダメージを受ける。",
    "迷宮の壁や床の中に隠れた【群狼】が、キミたちを待ち伏せていた！ 【狼牙】にさらされた宮廷全員は、[探索/5+宮廷の人数]の判定を行う。失敗したPCは、自分の《HP》が（1D6）点になる。",
    "部屋を埋め尽くすほど大勢の【小鬼】の群れに遭遇する。【小鬼】たちは瞳を赤くし、我を忘れて襲いかかってくる。宮廷全員は[武勇/5+宮廷の人数]の判定を行う。成功したキャラクターは、「牙」の素材を（1D6）個獲得する。失敗したキャラクターは、[（1D6）+宮廷の平均レベル]点のダメージを受ける。",
    "【鬼婆】の奴隷商人に出会う。鎖につながれた無数の奴隷が、恨めしそうにこちらを見ている。宮廷の代表は、[魅力/7+宮廷の人数]の判定を行う。成功すれば、【鬼婆】から奴隷を購入することができる。《予算》を1MG消費するたびに、（1D6）人の《民》を獲得できる。その場で自由に宮廷の《配下》として編成すること。判定に失敗すると、【鬼婆】は奴隷を差し向け、襲いかかってくる。宮廷全員は[武勇/9]の判定を行う。失敗したPCは[（1D6）+宮廷の平均レベル]点のダメージを受けた上、《配下》-（1D6）。",
    "年若い娘が1人倒れている。宮廷の中で誰か彼女を助ける者がいるなら、（1D6）を振ること。その目が奇数なら、彼女は有能な逸材だった。彼女はお礼を言い、王国に仕えさせてくれという。「生まれ表」でランダムに選んだジョブの逸材になる。偶数なら、彼女は【メデューサ】だった。【石化の視線】が襲いかかる。彼女を助けようとした者は[才覚/7+宮廷の人数]、残りのPCは[才覚/5+宮廷の人数]の判定を行う。失敗した者は、（1D6）点のダメージを受け、「呪い3」の変調を受ける。この判定に宮廷全員が失敗すると宮廷は全滅する。",
    "災厄教の巡礼者の一団に出会う。彼らは、迷宮災厄こそおごり高ぶった人類への罰であり、悔い改めよとその教えを説いた。《配下》を1人以上連れているキャラクターは、[魅力/自分の《配下》の数+5]の判定を行う。失敗したPC1人につき、《民の声》-1。",
];

/// Ruby `特殊遭遇表`（`1D6`）。
static MKB_TABLE_SE: Table = Table::from_dice("特殊遭遇表", 1, 6, MKB_TABLE_SE_ITEMS);

/// Ruby `情報収集表`の項目。
static MKB_TABLE_IG_ITEMS: &[&str] = &[
    "調査隊は、伝説の財宝の噂を聞きつける。《配下》を（1D6）人消費すると、迷宮マップの中からランダムに部屋を1つ目標に選ぶことができる。冒険フェイズに目標の捜索に成功すると、ランダムに選んだレアアイテム1個を獲得する。",
    "素材のある部屋を見つける。迷宮マップの中からランダムに部屋を1つ目標に選び、好きな素材を1種類選ぶ。冒険フェイズに目標の捜索に成功すると、その素材を[（1D6）+宮廷の平均レベル]個獲得する。",
    "噂に聞いたことのある怪物を発見する。迷宮マップの中からランダムに部屋を1つ目標に選ぶ。その部屋に、レベルが[PCの平均レベル+5]以下の好きなモンスターを1体、中立的なモンスターとして配置することができる。",
    "調査隊は、怪物にまつわる情報を入手した！ 迷宮マップの中から好きな部屋を2つ目標に選ぶ。目標の脅威情報をGMに教えてもらう。",
    "危険な迷宮を調査隊は進む。《配下》を1人消費すると、迷宮マップの中から好きな部屋を1つ目標に選ぶことができる。目標の脅威情報と通路情報をGMに教えてもらう。目標から他の部屋に通路がつながっていない場合、PCは行動済みにならず、もう一度、指揮判定を行うことができる。",
    "入り口にたどりつく。迷宮マップの中から【入り口】のある部屋1つをGMに教えてもらい、その部屋を目標に選ぶ。目標の脅威情報をGMに教えてもらう。その後、《配下》を消費することができる。《配下》を（1D6）人消費すると、PCは行動済みにならず、もう一度、指揮判定を行うことができる。",
    "調査隊は不慮の事故に巻き込まれる。《配下》を1人消費すると、迷宮マップの中から好きな部屋を1つ目標に選ぶことができる。目標の脅威情報と通路情報をGMに教えてもらう。",
    "調査隊は無事、迷宮にたどりつく。迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の脅威情報と通路情報をGMに教えてもらう。",
    "難民のいる部屋を発見する。迷宮マップの中からランダムに部屋を1つ目標に選ぶ。冒険フェイズに目標の捜索に成功すると、宮廷の1人は《配下》を（1D6）人獲得する。",
    "調査隊は隠し財産がある部屋に接近した！ 迷宮マップの中からランダムに部屋を1つ目標に選ぶ。冒険フェイズに目標の捜索に成功すると（1D6）MGを獲得する。",
    "調査隊の素晴らしい活躍！ 迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の脅威情報と通路情報をGMに教えてもらう。さらに、「情報収集表」をもう1回使用できる。",
];

/// Ruby `情報収集表`（`2D6`）。
static MKB_TABLE_IG: Table = Table::from_dice("情報収集表", 2, 6, MKB_TABLE_IG_ITEMS);

/// Ruby `生まれ決定表`の項目。
static MKB_TABLE_BDT_ITEMS: &[&str] = &[
    "才覚系生まれ表で決定",
    "魅力系生まれ表で決定",
    "探索系生まれ表で決定",
    "武勇系生まれ表で決定",
    "好きな生まれ表で決定",
    "好きな生まれ表で決定",
];

/// Ruby `生まれ決定表`（`1D6`）。
static MKB_TABLE_BDT: Table = Table::from_dice("生まれ決定表", 1, 6, MKB_TABLE_BDT_ITEMS);

/// Ruby `才覚系生まれ表`の項目。
static MKB_TABLE_TBO_ITEMS: &[&str] = &["魔導師", "博士", "医者", "宦官", "商人", "地図師"];

/// Ruby `才覚系生まれ表`（`1D6`）。
static MKB_TABLE_TBO: Table = Table::from_dice("才覚系生まれ表", 1, 6, MKB_TABLE_TBO_ITEMS);

/// Ruby `魅力系生まれ表`の項目。
static MKB_TABLE_CBO_ITEMS: &[&str] = &["星術師", "召喚師", "貴族", "亭主", "寿ぎ屋", "語り部"];

/// Ruby `魅力系生まれ表`（`1D6`）。
static MKB_TABLE_CBO: Table = Table::from_dice("魅力系生まれ表", 1, 6, MKB_TABLE_CBO_ITEMS);

/// Ruby `探索系生まれ表`の項目。
static MKB_TABLE_SBO_ITEMS: &[&str] = &["迷宮職人", "料理人", "働き者", "狩人", "盗賊", "鉱工"];

/// Ruby `探索系生まれ表`（`1D6`）。
static MKB_TABLE_SBO: Table = Table::from_dice("探索系生まれ表", 1, 6, MKB_TABLE_SBO_ITEMS);

/// Ruby `武勇系生まれ表`の項目。
static MKB_TABLE_VBO_ITEMS: &[&str] = &["武人", "処刑人", "衛視", "冒険者", "怠け者", "番人"];

/// Ruby `武勇系生まれ表`（`1D6`）。
static MKB_TABLE_VBO: Table = Table::from_dice("武勇系生まれ表", 1, 6, MKB_TABLE_VBO_ITEMS);

/// Ruby `好意表`の項目。
static MKB_TABLE_FET_ITEMS: &[&str] = &["忠誠", "忠誠", "友情", "友情", "愛情", "愛情"];

/// Ruby `好意表`（`1D6`）。
static MKB_TABLE_FET: Table = Table::from_dice("好意表", 1, 6, MKB_TABLE_FET_ITEMS);

/// Ruby `敵意表`の項目。
static MKB_TABLE_HET_ITEMS: &[&str] = &["怒り", "怒り", "不信", "不信", "侮蔑", "侮蔑"];

/// Ruby `敵意表`（`1D6`）。
static MKB_TABLE_HET: Table = Table::from_dice("敵意表", 1, 6, MKB_TABLE_HET_ITEMS);

/// Ruby `初期装備表`の項目。
static MKB_TABLE_IEQ_ITEMS: &[&str] = &[
    "鉄砲",
    "爆弾",
    "お守り",
    "フルコース",
    "星の欠片",
    "お弁当",
    "ポーション",
    "お酒",
    "乗騎",
    "衣装",
    "魔導書",
];

/// Ruby `初期装備表`（`2D6`）。
static MKB_TABLE_IEQ: Table = Table::from_dice("初期装備表", 2, 6, MKB_TABLE_IEQ_ITEMS);

/// Ruby `スキル決定表`の項目。
static MKB_TABLE_SDT_ITEMS: &[&str] = &[
    "基本スキル表で決定",
    "基本スキル表で決定",
    "基本スキル表で決定",
    "上級スキル表で決定",
    "上級スキル表で決定",
    "上級スキル表で決定",
];

/// Ruby `スキル決定表`（`1D6`）。
static MKB_TABLE_SDT: Table = Table::from_dice("スキル決定表", 1, 6, MKB_TABLE_SDT_ITEMS);

/// Ruby `基本肉弾スキル表`の項目。
static MKB_TABLE_BUS_ITEMS: &[&str] = &["投げる", "鉄腕", "かばう", "突撃", "乱舞", "二刀流"];

/// Ruby `基本肉弾スキル表`（`1D6`）。
static MKB_TABLE_BUS: Table = Table::from_dice("基本肉弾スキル表", 1, 6, MKB_TABLE_BUS_ITEMS);

/// Ruby `基本射撃スキル表`の項目。
static MKB_TABLE_SHS_ITEMS: &[&str] = &["狙う", "連射", "魔弾", "援護射撃", "必殺", "零距離射撃"];

/// Ruby `基本射撃スキル表`（`1D6`）。
static MKB_TABLE_SHS: Table = Table::from_dice("基本射撃スキル表", 1, 6, MKB_TABLE_SHS_ITEMS);

/// Ruby `基本星術スキル表`の項目。
static MKB_TABLE_ASS_ITEMS: &[&str] = &["刻騙し", "流れ星", "星占い", "星剣", "星界", "星戦"];

/// Ruby `基本星術スキル表`（`1D6`）。
static MKB_TABLE_ASS: Table = Table::from_dice("基本星術スキル表", 1, 6, MKB_TABLE_ASS_ITEMS);

/// Ruby `基本召喚スキル表`の項目。
static MKB_TABLE_SUS_ITEMS: &[&str] = &["宅配便", "大転移", "送還", "転送", "魔物使い", "憑依"];

/// Ruby `基本召喚スキル表`（`1D6`）。
static MKB_TABLE_SUS: Table = Table::from_dice("基本召喚スキル表", 1, 6, MKB_TABLE_SUS_ITEMS);

/// Ruby `基本科学スキル表`の項目。
static MKB_TABLE_SCS_ITEMS: &[&str] = &[
    "設計",
    "分析",
    "マルチタスク",
    "錬成",
    "抗魔式",
    "理力の一撃",
];

/// Ruby `基本科学スキル表`（`1D6`）。
static MKB_TABLE_SCS: Table = Table::from_dice("基本科学スキル表", 1, 6, MKB_TABLE_SCS_ITEMS);

/// Ruby `基本迷宮スキル表`の項目。
static MKB_TABLE_LAS_ITEMS: &[&str] = &["罠師", "すりぬけ", "足止め", "軽業", "地裂", "隠形"];

/// Ruby `基本迷宮スキル表`（`1D6`）。
static MKB_TABLE_LAS: Table = Table::from_dice("基本迷宮スキル表", 1, 6, MKB_TABLE_LAS_ITEMS);

/// Ruby `基本交渉スキル表`の項目。
static MKB_TABLE_NES_ITEMS: &[&str] =
    &["スカウト", "人脈", "時間稼ぎ", "命乞い", "右腕", "仲間割れ"];

/// Ruby `基本交渉スキル表`（`1D6`）。
static MKB_TABLE_NES: Table = Table::from_dice("基本交渉スキル表", 1, 6, MKB_TABLE_NES_ITEMS);

/// Ruby `基本便利スキル表`の項目。
static MKB_TABLE_COS_ITEMS: &[&str] = &[
    "合体攻撃",
    "目覚めのキス",
    "不屈",
    "電撃作戦",
    "デート",
    "連携攻撃",
];

/// Ruby `基本便利スキル表`（`1D6`）。
static MKB_TABLE_COS: Table = Table::from_dice("基本便利スキル表", 1, 6, MKB_TABLE_COS_ITEMS);

/// Ruby `基本芸能スキル表`の項目。
static MKB_TABLE_ENS_ITEMS: &[&str] = &["宴", "軍楽", "武楽", "呪歌", "音霊", "ナルシスト"];

/// Ruby `基本芸能スキル表`（`1D6`）。
static MKB_TABLE_ENS: Table = Table::from_dice("基本芸能スキル表", 1, 6, MKB_TABLE_ENS_ITEMS);

/// Ruby `基本道具スキル表`の項目。
static MKB_TABLE_TOS_ITEMS: &[&str] = &[
    "大かばん",
    "お買い物",
    "修理",
    "プレゼント",
    "武器習熟",
    "渾身の力",
];

/// Ruby `基本道具スキル表`（`1D6`）。
static MKB_TABLE_TOS: Table = Table::from_dice("基本道具スキル表", 1, 6, MKB_TABLE_TOS_ITEMS);

/// Ruby `基本一般表`の項目。
static MKB_TABLE_GES_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("名乗る")),
    (12, TableItem::Text("大食漢")),
    (13, TableItem::Text("全軍突撃!!")),
    (14, TableItem::Text("美形")),
    (15, TableItem::Text("熱血")),
    (16, TableItem::Text("特訓")),
    (21, TableItem::Text("名乗る")),
    (22, TableItem::Text("大食漢")),
    (23, TableItem::Text("全軍突撃!!")),
    (24, TableItem::Text("美形")),
    (25, TableItem::Text("熱血")),
    (26, TableItem::Text("特訓")),
    (31, TableItem::Text("名乗る")),
    (32, TableItem::Text("大食漢")),
    (33, TableItem::Text("全軍突撃!!")),
    (34, TableItem::Text("美形")),
    (35, TableItem::Text("熱血")),
    (36, TableItem::Text("特訓")),
    (41, TableItem::Text("お宝")),
    (42, TableItem::Text("受け流し")),
    (43, TableItem::Text("跳ぶ")),
    (44, TableItem::Text("殺意")),
    (45, TableItem::Text("真の想い")),
    (46, TableItem::Text("好きこそ上手")),
    (51, TableItem::Text("お宝")),
    (52, TableItem::Text("受け流し")),
    (53, TableItem::Text("跳ぶ")),
    (54, TableItem::Text("殺意")),
    (55, TableItem::Text("真の想い")),
    (56, TableItem::Text("好きこそ上手")),
    (61, TableItem::Text("お宝")),
    (62, TableItem::Text("受け流し")),
    (63, TableItem::Text("跳ぶ")),
    (64, TableItem::Text("殺意")),
    (65, TableItem::Text("真の想い")),
    (66, TableItem::Text("好きこそ上手")),
];

/// Ruby `基本一般表`（D66）。
static MKB_TABLE_GES: D66Table =
    D66Table::new("基本一般表", D66SortType::NoSort, MKB_TABLE_GES_ITEMS);

/// Ruby `空振り休憩表`の項目。
static MKB_TABLE_EBT_ITEMS: &[&str] = &[
    "「おつとめ、ご苦労様です」同行する民たちが感謝の言葉をかける。《民の声》+1。",
    "「おい、サボるな」と仲間から怒られた。いやいや、こっちは今までマジメにやってましたよ。宮廷の中から好きなキャラクター1人を選ぶ。自分のそのキャラクターに対する《敵意》+1。",
    "大量大量！　色々な素材が見つかる。肉、牙、鉄、魔素、機械の素材（キャラクターシートの上の段の素材）を1個ずつ獲得する。",
    "そこはもう、使い魔が探索してくれていたようだ。サンキュー相棒！　この捜索の判定に【使い魔】を利用していれば、行動済みにならず、さらにもう1回行動を行うことができる。",
    "危険なトラップを見つけたが、何とか無力化できた。任務完了。自分の《気力》+1。",
    "何も見つからなかったか、と思っていたら「いつも、ありがとう」と宮廷の仲間から声をかけられた。まぁ、何もない方がいいか。宮廷の中から好きなキャラクター1人を選ぶ。自分のそのキャラクターに対する《好意》+1",
    "「さすが！　素晴らしいお手並みだ」鮮やかな捜索に、仲間の見る目が変わる。宮廷の中から好きなキャラクター1人を選ぶ。そのキャラクターの自分に対する《好意》+1。",
    "よしよし、これはいいものが見つかった。好きな1種類の素材を（1D6）個獲得する。この捜索の判定に【使い魔】を使用していれば、獲得数が（1D6）個上昇する。",
    "大量大量！　色々な素材が見つかる。衣料、木、火薬、情報、革の素材（キャラクターシートの上の段の素材）を1個ずつ獲得する。",
    "うわ！　罠だ。余計なものまで見つけてしまった。宮廷全員は（1D6）点のダメージを受ける。",
    "「へぇ、こんなヤツだったのか」仲間の意外な一面を見つける。宮廷の中から好きなキャラクター1人を選ぶ。自分のそのキャラクターに対する《感情値》を反転させ、属性を好きなものに変更できる。",
];

/// Ruby `空振り休憩表`（`2D6`）。
static MKB_TABLE_EBT: Table = Table::from_dice("空振り休憩表", 2, 6, MKB_TABLE_EBT_ITEMS);

/// Ruby `人工部屋特殊遭遇表`の項目。
static MKB_TABLE_ARN_ITEMS: &[&str] = &[
    "他の王国のランドメイカーらしき一行が現れる。彼らは食事が尽きているらしく、アイテムの交換を持ちかけてきた。話を聞くなら、1クォーターが経過し、食事アイテム1個と交換で【ポーション】か【特効薬】1個を獲得できる。話をきかないなら、彼らは食事を無理矢理奪おうとしてくる。宮廷の代表は[魅力/宮廷の人数+5]の判定を行う。失敗すると、食事アイテムを持っているPCは（1D6）点のダメージを受け、持っている食事アイテムをすべて消費する。",
    "数人の【人間の屑】が物欲しそうな顔つきでこちらを見ながら、ひそひそと話しあっている。宮廷が、価格が3以上のコモンアイテムを[宮廷の人数の半分]個のアイテムを消費すると、【人間の屑】たちは卑屈な笑みを浮かべながら、この部屋を去っていく。消費しないなら、宮廷全員は[探索/宮廷の人数+5]の判定を行う。失敗した者は、ランダムにアイテムスロット1つを選び、そのスロットに装備・収納されているアイテムをすべて破壊する。",
    "ラストエグザイルという修行の旅をしている【ラストサムライ】の一団に出会う。PC1人が素材欄に「鉄」を含むアイテム1個を消費すると、彼らは喜んで旅の噂話を教えてくれる。1クォーターが経過し、宮廷の代表は「情報」の素材を（1D6）個獲得する。各PCは、望むなら食事アイテムを1個ずつ使用できる。アイテムを消費しない場合、彼らは襲いかかってくる。宮廷全員は[武勇/宮廷の人数+5]の判定を行う。失敗した者は、ランダムにアイテムスロット1つを選び、そのスロットに装備・収納されているアイテムをすべて破壊し、（1D6）+1点のダメージを受ける。",
    "部屋の片隅に宝箱を見つける。宝箱を開けてみるなら（1D6）を振ること。1なら【宝石】1個を獲得する。2ならランダムに選んだ1レベルのコモンアイテム1個を獲得する。3ならランダムに選んだレア一般アイテムを1個獲得する。4なら【箱入り娘】に魅了されて、ランダムに選んだ自分以外のPC1体に（2D6）点のダメージを与える。5なら【匣男】に抱きつかれ、そのターンの間「散漫1」の変調を受け、《HP》の最大値-3。6なら【生き金貨】がブレスを吐いてきて、宮廷全員は4点のダメージを受ける。",
    "宮廷たちの背後から、迷宮の壁に描かれた絵がゆっくりと襲いかかってくる。【逆壁】だ！ 宮廷の代表は[才覚/宮廷の人数+7]の判定を行う。成功したら、宮廷は【逆壁】の不意打ちに気づいて、返り討ちにする。失敗したら、宮廷全員は（1D6）点のダメージを受ける。",
    "【ウマトカゲ】に乗ったメトロ汗国の斥候たちに出会う。彼らは奴隷を集めに来たようだ。宮廷全員は[武勇/宮廷の人数+5]の判定を行う。失敗した者は《配下》-（1D6）。",
];

/// Ruby `人工部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_ARN: Table = Table::from_dice("人工部屋特殊遭遇表", 1, 6, MKB_TABLE_ARN_ITEMS);

/// Ruby `水域部屋特殊遭遇表`の項目。
static MKB_TABLE_WEN_ITEMS: &[&str] = &[
    "【エルフ】の集団が現れた。【エルフ】たちは、抜け目なく宮廷の様子をうかがっている。宮廷の代表は、[才覚/13]の判定を行う(「言語」の選択ルールを適用して、深人語を修得していたら自動的に成功する)。成功すると、彼らがPCたちの王国を襲撃しようとしていることが分かる。宮廷全員は[武勇/13-宮廷の人数]の判定を行う。成否にかかわらず、【エルフ】たちの企みは止めることができるが、失敗した者は（1D6）+1点のダメージを受ける。[才覚]の判定に失敗すると彼らの狙いに気づくことができない。終了フェイズの「王国変動」のタイミングで、追加で1回「王国変動表」の4番の効果が発生する。",
    "突如現れた【マッハペンギン】に向かって、水中から【鉄砲魚】が砲撃を行う。このままでは、天使と深人の争いに巻き込まれてしまいそうだ。どちらかの加勢をするなら、宮廷全員は[好きな能力値/宮廷の人数+7]の判定を行う。成功したPCが宮廷の人数の半分以上いると、加勢した側が勝利する。天使側に加勢したならPC全員はランダムに回復アイテムを1個ずつ、深人側に加勢したならPC全員はランダムに武具アイテムを1個ずつ獲得する。成功したPCが宮廷の人数の半分未満だと、PC全員は（2D6）点のダメージを受ける。",
    "水域の近くから「モウレン、ヤッサ、イナガ貸セエ」という声が近づいてくる。【丹幽霊】だ。宮廷の誰かが【鍋】を1個消費すると、不思議そうな顔をしてそれを持っていき、彼らは水域の向こう側へと消えていく。そうでなければ、宮廷が持っている乗物アイテムがすべて消費される。",
    "【河ドワーフ】が水路を伸ばす工事を行っている。このままだと、この部屋は完全に水没してしまうかもしれない。止めたほうがいいのだろうか？ 止めるなら、宮廷の代表は[魅力/宮廷の人数+7]の判定を行う。成功すると、快く【河ドワーフ】たちは水路を伸ばす方向を変えてくれる。失敗すると、【河ドワーフ】たちに愉快な罵倒を浴びせられ、宮廷全員の《気力》-1、《民の声》-1。止められないなら、その部屋に【水槽】のトラップが配置される。",
    "「ヨーホー！ 金目のものをよこしやがれ！」【階賊】の集団に襲われる！ 宮廷全員は[武勇/13-宮廷の人数]の判定を行う。失敗した者は、ランダムに自分のアイテムスロット1つを選び、そのスロットに装備・収納されたものをすべて消費し、（1D6）点のダメージを受ける。",
    "水の中から突如触手が現れた！ 宮廷の1人にからみつくと、水の中に引きずり込んでしまう。宮廷の中からランダムに1人を選ぶ。選ばれたPCは[探索/9+装備・収納している、素材欄に「鉄」が含まれるアイテムの数]の判定を行う。失敗すると、《HP》を（[判定の難易度-判定の達成値]D6）点減少する。また、そのPCが装備・収納している、素材欄に「火薬」が含まれるアイテムを破壊する。",
];

/// Ruby `水域部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_WEN: Table = Table::from_dice("水域部屋特殊遭遇表", 1, 6, MKB_TABLE_WEN_ITEMS);

/// Ruby `自然部屋特殊遭遇表`の項目。
static MKB_TABLE_NEN_ITEMS: &[&str] = &[
    "大きな地響きが聞こえる。この森を構成している大勢の【トレント】たちが別の部屋へと移動しているようだ。ほかの生き物たちも、木々の行進に続いている。森の大移動だ。宮廷の代表は[探索/宮廷の人数+7]の判定を行う。失敗すると、宮廷は【トレント】たちの大行進に出くわしてしまう。宮廷全員は《HP》の現在値を（1D6）点にして、《配下》-（1D6）。",
    "天井近くに【アラクネ】の巣を見つける。近くに【蜘蛛の王】の領域があるのかもしれない。駆除しておくべきか……。駆除に挑戦するなら、1クォーターが経過し、PC全員は[武勇/13-宮廷の人数]の判定を行う。判定の成否に関わらず巣を除去することができるが、失敗した者は、アラクネの反撃を受け、（2D6）-2点のダメージを受ける。放っておく場合、終了フェイズの王国変動のタイミングで（1D6）を振る。その出目が、[「周辺階域」欄のそのマップがある土地から自国がある土地までのマス数]以下だった場合、【蜘蛛の王】の襲撃により、自国に残っていた《民》が（5D6）人減少する。",
    "やぶの中から突如現れた巨大な怪物を目撃する。【睨み蜥蜴】だ！ PC全員は[探索/9]の判定を行う。失敗した者は《HP》を1点にする。",
    "【緑の親指】が森の木々を手入れしている。自分が管理する森にやってきたPCたちを警戒しているようだ。宮廷の代表は[才覚/宮廷の人数+7]の判定を行う。成功すると、日常アイテム1個と交換で「木」の素材を（1D6）個獲得できる。失敗した者は（1D6）+6点のダメージを受ける。",
    "森の奥から何かを叩くポコポコという音が響いてくる。のぞいてみると、【豆狸】たちが、腹鼓を叩きながら、楽しげに唄っている。PC全員は[魅力/宮廷の人数+5] の判定を行う。成功したPCが宮廷の人数の半分以上いると、楽しい時間を過ごす。各PCは《気力》+1、望むなら食事のアイテムを1個ずつ使用できる。成功した PCが宮廷の人数の半分未満だと、気がつくと辺りには誰もいなくなっている。2クォーターが経過し、各PCは、ランダムにアイテムスロット1つを選び、そのスロットに装備・収納されたものをすべて消費する。",
    "その部屋の奥には、茸の森が広がっていた。その中心にたたずむ巨大な【オバケ茸】を【茸人】たちが囲んで、何か祈りを捧げている。……ここなら、もしかすると【百年茸】があるかも。【百年茸】を探すなら、宮廷の中から望む者は[探索/9+この判定に挑戦した回数(初回は1回と数える)]の判定を行う。成功した者は、レア一般アイテムの 【百年茸】を1個獲得する。誰か1人でも失敗すると、【茸人】に見つかり、PC全員は「毒2」の変調を受ける。【百年茸】を探さないなら、安全にその場を離れ、何も起こらない。",
];

/// Ruby `自然部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_NEN: Table = Table::from_dice("自然部屋特殊遭遇表", 1, 6, MKB_TABLE_NEN_ITEMS);

/// Ruby `洞窟部屋特殊遭遇表`の項目。
static MKB_TABLE_CEN_ITEMS: &[&str] = &[
    "突如、天井から魔法の掘削機械が飛び出してくる。【ドワーフ】の直線主義者の一団だ。このままだと押しつぶされてしまう！ PC全員は[探索/宮廷の人数+5]の判定を行う。失敗した者は（1D6）点のダメージを受け、《配下》-（1D6）。その後GMは、その部屋に隣接するシナリオ上、遭遇が設定されていない部屋があれば、そこに向けて通路1本を引く。",
    "眠っている【洞窟熊】を見つける。攻撃するか？ それとも音を立てないようにやり過ごすか？ 攻撃するなら、PC全員は[武勇/7]の判定を行う。判定の成否にかかわらず【洞窟熊】を倒すことはできるが、失敗した者は《HP》を1点にする。やり過ごすなら、PC全員は[探索/宮廷の人数+5]の判定を行う。失敗した者が宮廷の人数の半分以上いると逃げ切れず、PC全員は（1D6）点のダメージを受ける。",
    "【まじない師】に率いられた【穴人】に取り囲まれる。【まじない師】は、謎かけをしてくる。宮廷の代表は[才覚/12]の判定を行う。成功すると、彼らはこの部屋を立ち去る。失敗すると【穴人】に襲いかかられ、PC全員は（2D6）点のダメージを受ける。",
    "洞窟の奥から【大蝙蝠】の群れが飛んでくる。PC全員は[探索/宮廷の人数+7]の判定を行う。失敗した者は「毒3」の変調を受ける。",
    "【ドラゴン】が現れた！ 流暢な「ひとつの言葉」を使って、その巨大な生き物は「うるさくて眠れない」と苦情を言ってきた。宮廷の代表は[魅力/宮廷の人数+7]の判定を行う。成功すると丁重にお帰りいただくことができる。失敗すると、宮廷全員は15点のダメージを受ける。",
    "空気がじめじめとしてくる。【黴姫】の領域が近いようだ。下手をすると食事を駄目にしてしまうかもしれない。PC全員は[才覚/宮廷の人数+5]の判定を行う。失敗した者は自分の装備・収納している食事アイテムをすべて破壊する。",
];

/// Ruby `洞窟部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_CEN: Table = Table::from_dice("洞窟部屋特殊遭遇表", 1, 6, MKB_TABLE_CEN_ITEMS);

/// Ruby `天空部屋特殊遭遇表`の項目。
static MKB_TABLE_SEN_ITEMS: &[&str] = &[
    "【取立人】が現れ、慇懃に挨拶すると、PCたちの栄光を褒め称える。そして、その栄光は天使の導きによるものだから、と対価を要求してくる。対価を支払うなら《予算》を[PCたちの平均レベル]MG消費するか、王国に残った《民》を[PCたちの平均レベル]人消費する。いずれかを消費すると【取立人】は満足そうにうなずき、未来に起こる出来事をこっそり耳打ちする。宮廷は、そのセッション中、振ったサイコロを1度だけ振り直すことができるようになる。対価をはねのけると、PC全員は「呪い3」の変調を受ける。",
    "【羽根兜の乙女】が立ち塞がり、「勇者よ！ きさまの魂をもらい受ける！」と決闘を挑んでくる。決闘を受けるなら、宮廷の代表は[武勇/14]の判定を行う。成功すると、【羽根兜の乙女】は、「次は絶対勝つ！」と、泣きながら逃げていく。判定に成功したPCが装備可能なら【愛】を1個獲得する。失敗した者は（1D6）+8点のダメージを受け、【羽根兜の乙女】から「腰抜けが。とんだ見込み違いだ」と罵倒される。決闘を拒否するなら、【羽根兜の乙女】の怒りを買い、PC全員は（2D6）点のダメージを受ける。",
    "一天にわかにかき曇る。【雲神】だ！ （1D6）を振る。1なら雨が振ってきて、PC全員は素材欄に「火薬」が含まれるアイテムを破壊する。2なら雷が落ちてきて、素材欄に「鉄」が含まれるアイテムを装備・収納しているPCは（3D6）点のダメージを受ける。3なら霧がたちこめ、PC全員は、その部屋で行う判定の達成値が2点減少する。4なら突風が吹き、PC全員は《配下》を[（1D6）×1/2]人減少する。5か6なら心地良い風が吹き、PC全員は《気力》+2。",
    "腹を空かせた【鷲獅子】が、空中から襲いかかる！ PC全員は[武勇/宮廷の人数+7]の判定を行う。【乗騎】を装備・収納しているPCは難易度が2点上昇する。判定に失敗した者は（2D6）点のダメージを受ける。【乗騎】を装備・収納しているPCが判定に失敗した場合、その【乗騎】がすべて消費される。",
    "空に巨大な星が輝く。その星が不気味に笑った気がする。あれは【星首】だ。宮廷の中から、ランダムに2人のPCを選ぶ。そのPCが装備・収納している【星の欠片】をすべて破壊する。",
    "何か雪のようなものが降ってきたと思ったら、気分が悪くなってきた。上空を見あげると、【蝶の王】が羽ばたいている。狂気の鱗粉だ！ PC全員は[魅力/9]の判定を行う。失敗した者は、「毒6」と「散漫1」と「憤怒」の変調を受ける。",
];

/// Ruby `天空部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_SEN: Table = Table::from_dice("天空部屋特殊遭遇表", 1, 6, MKB_TABLE_SEN_ITEMS);

/// Ruby `異界部屋特殊遭遇表`の項目。
static MKB_TABLE_OEN_ITEMS: &[&str] = &[
    "扉をあけて、ハグルマ風の衣装を着た人物が現れる。何やら話が通じない。もしかすると噂に名高い【稀人】というやつか？ 《配下》たちが何かを期待しているのを感じる。宮廷の代表は[才覚/宮廷の人数+7]の判定を行う。成功すると、意思の疎通に成功する。【稀人】1体を《特殊配下》にできる。王国につれて帰ることができると《モンスターの民》になる。失敗すると、【稀人】は話が通じず途方にくれ、扉の向こうに帰っていく。《民の声》-1。",
    "【ケチャップリンス】と【メイクイーン】が激論を交わしている。どうやら、どちらが【マヨネーズキング】にふさわしいかについて語り合っているようだ。仲裁するなら、宮廷の代表は[才覚/宮廷の人数+7の判定を行う。成功すると美味しい食べ物を御馳走してくれる。PC全員は《HP》を（1D6）点回復し、《気力》+1。失敗するとPC全員は「肥満2」の変調を受ける。スルーするなら、PC全員は[探索/7]の判定を行う。誰か1人でも失敗するとPC全員は「肥満2」の変調を受ける。",
    "突然、その部屋が闇に包まれ、重力がなくなる。扉が開く音がして、そこから強い光がさしこんできた。【灰色の宇宙人】だ。【乗騎】、【使い魔】、【家畜】のいずれかを装備・収納しているPCは[魅力/宮廷の人数+7]の判定を行う。成功した者は【乗騎】、【使い魔】、【家畜】のうちいずれか1個と交換で【星の欠片】か【携帯電話】1個を獲得できる。判定に失敗した者は【乗騎】、【使い魔】、【家畜】のうちいずれか1個を消費する。",
    "扉を破って、無数の「死にぞこないの群れが現れた。ゾンビラッシュ！ PC全員は[武勇/宮廷の人数+7]の判定を行う。失敗した者は、（1D6）点のダメージと「毒3」の変調を受ける。",
    "遠くの方から何かが転がってくる。ゴロゴロと音が大きくなり、気がつくと【悪意の骰子】が眼前に迫っていた！ 宮廷全員は[探索/9]の判定を行う。失敗した者は「呪い4」の変調を受け、奇妙な姿に変えられる。",
    "暗闇の中に幾つかの星が輝く。あれは【星座獣】だ！ PC全員は[魅力/9]の判定を行う。失敗した者は[そのPCが装備・収納している【星の欠片】の合計数+1]D6点のダメージを受ける。",
];

/// Ruby `異界部屋特殊遭遇表`（`1D6`）。
static MKB_TABLE_OEN: Table = Table::from_dice("異界部屋特殊遭遇表", 1, 6, MKB_TABLE_OEN_ITEMS);

/// Ruby `他国との関係表`の項目。
static MKB_TABLE_ORT_ITEMS: &[&str] = &["同盟", "友好", "中立", "中立", "険悪", "敵対"];

/// Ruby `他国との関係表`（`1D6`）。
static MKB_TABLE_ORT: Table = Table::from_dice("他国との関係表", 1, 6, MKB_TABLE_ORT_ITEMS);

/// Ruby `武具アイテム表`（Ruby定数 `WEAPON_ITEM_TABLE`）の項目。
static MKB_TABLE_WIT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("だんびら")),
    (12, TableItem::Text("網（だんびら）")),
    (13, TableItem::Text("短剣")),
    (14, TableItem::Text("戦斧")),
    (15, TableItem::Text("盾")),
    (16, TableItem::Text("ホウキ")),
    (22, TableItem::Text("棘（だんびら）")),
    (23, TableItem::Text("鑓")),
    (24, TableItem::Text("石弓")),
    (25, TableItem::Text("甲冑")),
    (26, TableItem::Text("戦鎚")),
    (33, TableItem::Text("鎖（だんびら）")),
    (34, TableItem::Text("爆弾")),
    (35, TableItem::Text("鉄砲")),
    (36, TableItem::Text("大剣")),
    (44, TableItem::Text("大弓（だんびら）")),
    (45, TableItem::Text("徹甲弾")),
    (46, TableItem::Text("拳銃")),
    (55, TableItem::Text("手裏剣（だんびら）")),
    (56, TableItem::Text("大砲")),
    (66, TableItem::Text("籠手（だんびら）")),
];

/// Ruby `武具アイテム表`（D66）。
static MKB_TABLE_WIT: D66Table =
    D66Table::new("武具アイテム表", D66SortType::Asc, MKB_TABLE_WIT_ITEMS);

/// Ruby `生活アイテム表`（Ruby定数 `LIFE_ITEM_TABLE`）の項目。
static MKB_TABLE_LIT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("鍋")),
    (12, TableItem::Text("家畜（鍋）")),
    (13, TableItem::Text("バックパック")),
    (14, TableItem::Text("クラッカー")),
    (15, TableItem::Text("がまぐち")),
    (16, TableItem::Text("マント")),
    (22, TableItem::Text("裁縫道具（鍋）")),
    (23, TableItem::Text("カード")),
    (24, TableItem::Text("エプロン")),
    (25, TableItem::Text("住民台帳")),
    (26, TableItem::Text("携帯電話")),
    (33, TableItem::Text("外套（鍋）")),
    (34, TableItem::Text("肖像画")),
    (35, TableItem::Text("衣装")),
    (36, TableItem::Text("山吹色のお菓子")),
    (44, TableItem::Text("鏡（鍋）")),
    (45, TableItem::Text("眼鏡")),
    (46, TableItem::Text("クレジットカード")),
    (55, TableItem::Text("召喚鍵（鍋）")),
    (56, TableItem::Text("魔道書")),
    (66, TableItem::Text("宝石（鍋）")),
];

/// Ruby `生活アイテム表`（D66）。
static MKB_TABLE_LIT: D66Table =
    D66Table::new("生活アイテム表", D66SortType::Asc, MKB_TABLE_LIT_ITEMS);

/// Ruby `回復アイテム表`（Ruby定数 `REST_ITEM_TABLE`）の項目。
static MKB_TABLE_RIT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("チョコレート（お弁当）")),
    (12, TableItem::Text("砂糖菓子（お弁当）")),
    (13, TableItem::Text("特効薬")),
    (14, TableItem::Text("保存食")),
    (15, TableItem::Text("担架")),
    (16, TableItem::Text("お弁当（チョコレート）")),
    (22, TableItem::Text("バナナ（お弁当）")),
    (23, TableItem::Text("お酒")),
    (24, TableItem::Text("珈琲")),
    (25, TableItem::Text("フルコース")),
    (26, TableItem::Text("ポーション")),
    (33, TableItem::Text("魔素水（お弁当）")),
    (34, TableItem::Text("救急箱")),
    (35, TableItem::Text("強壮剤")),
    (36, TableItem::Text("迷宮保険")),
    (44, TableItem::Text("魔薬（お弁当）")),
    (45, TableItem::Text("科学調味料")),
    (46, TableItem::Text("惚れ薬")),
    (55, TableItem::Text("軟膏（お弁当）")),
    (56, TableItem::Text("復活薬")),
    (66, TableItem::Text("万能薬（お弁当）")),
];

/// Ruby `回復アイテム表`（D66）。
static MKB_TABLE_RIT: D66Table =
    D66Table::new("回復アイテム表", D66SortType::Asc, MKB_TABLE_RIT_ITEMS);

/// Ruby `探索アイテム表`（Ruby定数 `SEARCH_ITEM_TABLE`）の項目。
static MKB_TABLE_SIT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("拷問具（星の欠片）")),
    (12, TableItem::Text("ロープ（星の欠片）")),
    (13, TableItem::Text("旗")),
    (14, TableItem::Text("お守り")),
    (15, TableItem::Text("星の欠片（拷問具）")),
    (16, TableItem::Text("パワーリスト")),
    (22, TableItem::Text("松明（星の欠片）")),
    (23, TableItem::Text("テント")),
    (24, TableItem::Text("楽器")),
    (25, TableItem::Text("工具")),
    (26, TableItem::Text("使い魔")),
    (33, TableItem::Text("迷宮迷彩（星の欠片）")),
    (34, TableItem::Text("乗騎")),
    (35, TableItem::Text("罠百科")),
    (36, TableItem::Text("迷宮防護服")),
    (44, TableItem::Text("聴診器（星の欠片）")),
    (45, TableItem::Text("地図")),
    (46, TableItem::Text("時計")),
    (55, TableItem::Text("飛行騎（星の欠片）")),
    (56, TableItem::Text("カボチャの馬車")),
    (66, TableItem::Text("もぐら棒（星の欠片）")),
];

/// Ruby `探索アイテム表`（D66）。
static MKB_TABLE_SIT: D66Table =
    D66Table::new("探索アイテム表", D66SortType::Asc, MKB_TABLE_SIT_ITEMS);

/// Ruby `コモンアイテムランダム決定表`（Ruby定数 `COMMON_ITEM_RANDOM_TABLE`）の項目。
static MKB_TABLE_CIR_ITEMS: &[TableItem] = &[
    TableItem::Table(&MKB_TABLE_WIT),
    TableItem::Table(&MKB_TABLE_LIT),
    TableItem::Table(&MKB_TABLE_RIT),
    TableItem::Table(&MKB_TABLE_SIT),
];

/// Ruby `コモンアイテムランダム決定表`（`1D4` の連鎖表）。
static MKB_TABLE_CIR: ChainTable =
    ChainTable::from_dice("コモンアイテムランダム決定表", 1, 4, MKB_TABLE_CIR_ITEMS);

/// Ruby `基本レア一般アイテム表`（Ruby定数 `NORMAL_RARE_ITEM_TABLE`）の項目（6行×6列）。
static MKB_TABLE_NRUT_ITEMS: &[&[&str]] = &[
    &["愚者の冠", "香水", "煙玉", "悪名", "藁人形", "王妃の鏡"],
    &[
        "星籠",
        "転ばぬ先の杖",
        "悟りの書",
        "鉛の兵隊",
        "黄金の林檎",
        "百年茸",
    ],
    &["愚者の冠", "香水", "煙玉", "悪名", "藁人形", "王妃の鏡"],
    &[
        "星籠",
        "転ばぬ先の杖",
        "悟りの書",
        "鉛の兵隊",
        "黄金の林檎",
        "百年茸",
    ],
    &[
        "操りロープ",
        "盗賊の七つ道具",
        "露眼鏡",
        "災厄王の遺物",
        "魔法の鞍",
        "琵琶",
    ],
    &["兎の足", "視肉", "衛星帯", "魔法の絨毯", "軍配", "聖杯"],
];

/// Ruby `基本レア一般アイテム表`（D66の6×6表）。
static MKB_TABLE_NRUT: D66GridTable =
    D66GridTable::new("基本レア一般アイテム表", MKB_TABLE_NRUT_ITEMS);

/// Ruby `上級レア一般アイテム表`（Ruby定数 `ADVANCED_RARE_ITEM_TABLE`）の項目（6行×6列）。
static MKB_TABLE_ARUT_ITEMS: &[&[&str]] = &[
    &[
        "砂時計週報",
        "兵糧丸",
        "遊星葉書",
        "百科辞典",
        "夢枕",
        "蓄音機",
    ],
    &[
        "砂時計週報",
        "兵糧丸",
        "遊星葉書",
        "百科辞典",
        "夢枕",
        "蓄音機",
    ],
    &[
        "水晶球",
        "狭間の棺桶",
        "不思議なたまご",
        "魔法瓶",
        "不死鳥の羽飾り",
        "紅葫蘆",
    ],
    &[
        "水晶球",
        "狭間の棺桶",
        "不思議なたまご",
        "魔法瓶",
        "不死鳥の羽飾り",
        "紅葫蘆",
    ],
    &[
        "打ち出の小槌",
        "消火器",
        "滅びの予言書",
        "召魔鏡",
        "鉄仮面",
        "愛",
    ],
    &[
        "打ち出の小槌",
        "消火器",
        "滅びの予言書",
        "召魔鏡",
        "鉄仮面",
        "愛",
    ],
];

/// Ruby `上級レア一般アイテム表`（D66の6×6表）。
static MKB_TABLE_ARUT: D66GridTable =
    D66GridTable::new("上級レア一般アイテム表", MKB_TABLE_ARUT_ITEMS);

/// Ruby `レア一般アイテムランダム決定表`（Ruby定数 `RARE_ITEM_RANDOM_TABLE`）の項目。
static MKB_TABLE_RUIR_ITEMS: &[TableItem] = &[
    TableItem::Table(&MKB_TABLE_NRUT),
    TableItem::Table(&MKB_TABLE_NRUT),
    TableItem::Table(&MKB_TABLE_NRUT),
    TableItem::Table(&MKB_TABLE_ARUT),
    TableItem::Table(&MKB_TABLE_ARUT),
    TableItem::Table(&MKB_TABLE_ARUT),
];

/// Ruby `レア一般アイテムランダム決定表`（`1D6` の連鎖表）。
static MKB_TABLE_RUIR: ChainTable =
    ChainTable::from_dice("レア一般アイテムランダム決定表", 1, 6, MKB_TABLE_RUIR_ITEMS);

/// Ruby `基本レア武具アイテム表`（Ruby定数 `NORMAL_RARE_WEAPON_ITEM_TABLE`）の項目（6行×6列）。
static MKB_TABLE_NRWT_ITEMS: &[&[&str]] = &[
    &["蛍矢", "小麦粉", "喇叭銃", "まわし", "しゃべる剣", "大盾"],
    &[
        "王笏",
        "ぬいぐるみ",
        "魔杖",
        "獣の毛皮",
        "バカには見えない鎧",
        "ビキニアーマー",
    ],
    &["蛍矢", "小麦粉", "喇叭銃", "まわし", "しゃべる剣", "大盾"],
    &[
        "王笏",
        "ぬいぐるみ",
        "魔杖",
        "獣の毛皮",
        "バカには見えない鎧",
        "ビキニアーマー",
    ],
    &[
        "チェインソード",
        "輝く者",
        "貪る者",
        "滅ぼす者",
        "機械の体",
        "刈り取る者",
    ],
    &[
        "断ち切る者",
        "竜の鱗鎧",
        "射貫く者",
        "貫く者",
        "剥ぎ取る者",
        "王剣",
    ],
];

/// Ruby `基本レア武具アイテム表`（D66の6×6表）。
static MKB_TABLE_NRWT: D66GridTable =
    D66GridTable::new("基本レア武具アイテム表", MKB_TABLE_NRWT_ITEMS);

/// Ruby `上級レア武具アイテム表`（Ruby定数 `ADVANCED_RARE_WEAPON_ITEM_TABLE`）の項目（6行×6列）。
static MKB_TABLE_ARWT_ITEMS: &[&[&str]] = &[
    &["虚弾", "小鬼の襟巻", "眼弾", "釣竿", "虹柱", "服従の鞭"],
    &["虚弾", "小鬼の襟巻", "眼弾", "釣竿", "虹柱", "服従の鞭"],
    &["星の杖", "聖印", "迷い傘", "邪眼", "徒手空拳", "隠れ兜"],
    &["星の杖", "聖印", "迷い傘", "邪眼", "徒手空拳", "隠れ兜"],
    &[
        "太刀鋏",
        "破城槌",
        "黄金の鶴嘴",
        "ムラサマ",
        "君主の衣",
        "蒸気甲冑",
    ],
    &[
        "太刀鋏",
        "破城槌",
        "黄金の鶴嘴",
        "ムラサマ",
        "君主の衣",
        "蒸気甲冑",
    ],
];

/// Ruby `上級レア武具アイテム表`（D66の6×6表）。
static MKB_TABLE_ARWT: D66GridTable =
    D66GridTable::new("上級レア武具アイテム表", MKB_TABLE_ARWT_ITEMS);

/// Ruby `レア武具アイテムランダム決定表`（Ruby定数 `RARE_WEAPON_ITEM_RANDOM_TABLE`）の項目。
static MKB_TABLE_RWIR_ITEMS: &[TableItem] = &[
    TableItem::Table(&MKB_TABLE_NRWT),
    TableItem::Table(&MKB_TABLE_NRWT),
    TableItem::Table(&MKB_TABLE_NRWT),
    TableItem::Table(&MKB_TABLE_ARWT),
    TableItem::Table(&MKB_TABLE_ARWT),
    TableItem::Table(&MKB_TABLE_ARWT),
];

/// Ruby `レア武具アイテムランダム決定表`（`1D6` の連鎖表）。
static MKB_TABLE_RWIR: ChainTable =
    ChainTable::from_dice("レア武具アイテムランダム決定表", 1, 6, MKB_TABLE_RWIR_ITEMS);

/// Ruby `お食事休憩表`の項目。
static MKB_TABLE_FDBT_ITEMS: &[&str] = &[
    "あなたの、主食の上に調味料で「LOVE」とメッセージが書かれていた。あたりを見回すと、顔を赤らめている人物が……。宮廷の中にあなたに対して「愛情」の〈好意）を持つ者がいれば、それはその人物の仕業だ。あなたの、そのキャラクターに対する〈好意〉を1点上昇する。もし、あなたに対して「愛情」の《好意》を持つ者が1人もいなければ、そのメッセージは天階（大宇宙）からのものだということに気づく。世界から祝福されていることに気づいたあなたは、《気力》を3点獲得し、「憤怒」の変調を受ける。",
    "美味しい香りに誘われて怪物たちがやってきた。怪物たちは食畢を分けて欲しそうにしているが……。「特殊遭遇表」（基本p67）を1回使用すること。宮廷の誰かが食事アイテムを1個消費することにした場合、その遺遇に書かれたモンスターが1体ずつ《モンスターの民》になる。消費しない場合、その遭遇の処理を行うこと。",
    "「はい。あー一ーーん」調子に乗って、好きな仲間にごはんを食べさせてあげる。自分の想い人の中から、《好意》が最も高い者1人を選ぶ。そのキャラクターのあなたに対する《好意》を1点上昇する。",
    "ちょっとタンパク質が足りないなぁ。そう言って、あなたは周囲に食べられそうな獲物を探しに出かけた。〔武勇〕で難易度［宮廷の人数+7］の判定を行う。成功すると、美味そうな獲物を狩ることに成功。メインディッシュが追加され、食事をしたキャラクター全員の《気力》を2点回復する。失敗すると、あなたは［1D6+宮廷の平均レベル］点のダメージを受ける。",
    "やったー！今日のメニューはあなたの大好物だ。うれしさのあまり、ついつい食べ過ぎてしまった。《気力》を1点獲得し、〔才覚〕で難易度9の判定を行う。失敗すると「肥満1」の変調を受ける。",
    "を！食後のお茶に茶柱がたっていた。これはラッキーな気がする。《気力》を1点回復する。",
    "軽く物足りない。アルコールがあるといいんだけど……ちょっとした宴会が始まりそうな予感。【お酒】を1個以上消費することができれば、1個消費するたびに《民の声》を1点回復する。",
    "配下にも食事を配る。彼ら彼女らは、慣れない迷宮でほうっと一息つきながら、この冒険の調子を聞いてきた。〔オ覚〕で難易度［現在の《民の声》+3］の判定を行う。成功すると、《民の声》を3点回復する。",
    "ちょっと趣向を変えて、ひとつの鍋をみんなで囲む。キャラクター全員は、〔魅力〕で難易度9の判定を行う。成功すると、妙に話題が盛り上がる。好きなキャラクター一人を選ぶ。そのキャラクターに対する《好意》を1点上昇する。失敗したキャラクターは、自分の食べたい具材を誰かにとられる。宮廷の中からランダムに選んだキャラクター1人に対する《敵意》を1点上昇する。",
    "むむむ。食事の中に苦手なものが……。〔探索〕で難易度9の判定を行う。成功すると、無事食ぺることができる。失敗すると、あなただけは、今回の食事の効果を受けず、食事を食べていない扱いとなる。",
    "むむむむ。ちょっと食べ物が迷宮化してたかも……（汗）。〔探索〕で難易度11の判定を行う。成功したら、迷宮の力を取りこみ《HP》を最大値まで回復する。失敗すると、よくない方向に迷宮化が……。ランダムに変調1つを選び、それを受ける（数値は3として扱う）。GMが認めた場合、「人体迷宮化表」(p49）を1回使用して変異を1つ獲得する。",
];

/// Ruby `お食事休憩表`（`2D6`）。
static MKB_TABLE_FDBT: Table = Table::from_dice("お食事休憩表", 2, 6, MKB_TABLE_FDBT_ITEMS);

/// Ruby `ご祝儀表`の項目。
static MKB_TABLE_GIFT_ITEMS: &[&str] = &[
    "大国からの御祝い。王国に隣接する未知の土地1つを領土してもらうことができる。1D6を振り、その数だけ通路をひき、【入り口】を1つ配置することができる。通路でつながっていない部屋は、自国の領土として扱わない。",
    "ご祝儀として怪物が贈られる。1D6を振る。1なら鬼族、2なら妖精、3なら魔獣、4なら異形、5なら死霊、6なら呪物カテゴリのモンスターの中から、レベルが［結婚する2人の〔魅力〕の合計値］以下の好きなモンスター1体を選ぶ。そのモンスター1体が《モンスターの民》となる。",
    "新郎新婦の幸せそうな姿に、思わず影響を受ける。結婚したPC以外のキャラクター全員は、好きなキャラクターに対して《好意》を2点上昇させる。",
    "2人の仲むつまじさに、思わず御祝いが集まる。自国の《予算》を［2人の互いに対する《好意》の合計値］MG上昇する。",
    "とっておきのアイテムが贈られる。宮廷の代表は「レア一般アイテム表」（基本p106か上級p66）か、「レア武具アイテム表」（基本p109か上級p66）のいずれかを選ぶ。その表を使用してランダムにレアアイテム1個を選び、それを獲得する。",
    "結婚式の参列者が精一杯のご祝儀をくれる。自国の〈予算〉を1D6MG上昇する。",
    "結婚を記念して新しい施設が建てられる。ランダムに施設のカテゴリを1つ選ぶ。そのカテゴリの施設（特殊施設も含む）の中から、価格が［結婚する2人の〔魅力〕の合計値+1D6］MGまでの施設を1つ選ぶ。その施設を建設する。",
    "たくさんの素材をいただく。［結婚する2人の〔魅力〕の合計値×3］個まで、好きな素材を好きな組み合わせで獲得する。",
    "神々からの贈り物が届く。結婚したPCは、自分の好きなスキル1種を喪失し、レベルが［自分のレベル］以下の天使カテゴリ、または深人カテゴリのモンスターが修得している好きなスキル1種を選びそれを修得することができる。また、結婚したPCは、好きな「背景表」を1回使用し、自分の背景と使命を変更することができる。",
    "新郎新婦の見事な美しさに歓声が上がる。予算が、［2人の〔魅力〕の合計値］MG上昇する。",
    "不思議な〈魔女〉がやってきて、王国に祝福を与える。王国の《民》が、［2人の〔魅力〕の合計値］D6人だけ増える。",
];

/// Ruby `ご祝儀表`（`2D6`）。
static MKB_TABLE_GIFT: Table = Table::from_dice("ご祝儀表", 2, 6, MKB_TABLE_GIFT_ITEMS);

/// Ruby `ニュース表`の項目。
static MKB_TABLE_NWST_ITEMS: &[&str] = &[
    "陰謀の予感!?このマップの支配者は、ある勢力に雇われ、PCたちを破滅させようとしていた。「情報」はその証拠だったのだ。GMは「勢力表」の中から、好きなもの1つを選ぶ。この支配者の裏には、そうした勢力がいることが分かった。このマップの支配者との戦闘の問、宮廷は作戦判定の達成値が1上昇する。",
    "迷宮災厄注意報！静寂が漂い、怪物たちの気配が消えた。急激な迷宮化現象の兆候を見て取ることができる。そのセッションの間、宮廷は捜索判定や解除判定の達成値が1上昇し、特殊週遇が発生すると「特殊遭遇表」を使用する代わりに、1クォーターが経過するようになる。",
    "真の黒幕！このマップの支配者は、操られているだけで真の黒幕が別にいることが分かる。このマップの支配者を「手加減」することで行動不能にすることができれば、【人類の敵】でない限り、その支配者は正気を取り戻し、倒したものの《配下》となる。GMは「勢力表」の中から、黒幕の正体を自由に決定してよい。",
    "虐待の爪痕！この迷宮には、迷宮支配者の奴隷となって働かされている人々がいることが判明する。このマップの支配者の《HP》を0点にしたPCは、《配下》を2D6人獲得することができる。",
    "哀しいトラウマ。この迷宮の迷宮支配者は、過去に大切な人を迷宮災厄のせいで失ったらしい。それが原因で迷宮病におかされ、カーネル化してしまったようだ。ランダムにクラス1つを選ぶ。迷宮支配者が失った大切な人は、そのクラスだったようだ。このマップの支配者は、そのクラスのキャラクターに対して「不可蝕」の弱点（基本p169）を修得する。また、望むならPCは、その支配者に対する《好意》を1点上昇することができる。",
    "賞金首！このマップの支配者は、色々な悪事を働くお尋ね者で、賞金首だったことが判明する。このマップの支配者を倒すと、PCたちの王国は終了フェイズの「収支報告」のタイミングで1D6MGを獲得することができる。",
    "お宝情報！この迷宮には、素晴らしいお宝が隠されているらしい。「武具アイテム表」（基本p98、もしくは上級p62）を使用してランダムに武具アイテム1種を選ぶ。もし、そのセッション中に、宮廷が選んだアイテムと同じものを獲得した場合、そのアイテムのレベルは3になる（獲得したアイテムのレベルが4以上だった場合、この効果は発生しない） 。",
    "酷聞！このマップの迷宮化の背景には、某国のスキャンダルが関係していたことが分かる。プレイヤーは列強、もしくは周辺階域の中からNPKを1つ選ぶ。このマップの支配者の誕生には、その国の秘すべき汚点が隠されていた！この支配者を倒すと、宮廷は選んだ国との関係を一段階良好にするか、関係を一段階悪化させて終了フェイズの「収支報告」のタイミングで1D6MGを獲得するかを選ぶことができる。",
    "驚愕の事実！このマップの支配者は、宮廷の誰かに恋心を抱いていたらしい。このマップの支配者は、この表を使用したキャラクターに対する《好意》を4点上昇し、その属性を「愛情」にする。",
    "出生の謎!?このマップの支配者は、宮廷の誰かと血を分けた兄弟姉妹だということが分かる。このマップの支配者は、この表を使用したキャラクターに対する《敵意》を4点上昇する。また、この表を使用したキャラクターは、このマップの支配者に対する《好意》、もしくは《敵意》を4点上昇し、その属性を好きなものにする。",
    "え！あなた、そうだったの!?意外な人物の、自分に対する秘めた想いを知ってしまう。この表を使用したキャラクターは、好きなキャラクター1人を選ぶ。そのキャラクターのあなたに対する《感情値》を4点上昇し、その属性を好きなものにする。",
];

/// Ruby `ニュース表`（`2D6`）。
static MKB_TABLE_NWST: Table = Table::from_dice("ニュース表", 2, 6, MKB_TABLE_NWST_ITEMS);

/// Ruby `遠征王国変動表`の項目。
static MKB_TABLE_EKCT_ITEMS: &[&str] = &[
    "旅する王国は燃料切れを起こしてしまったようだ。以降、国力判定を行うとき、燃料を2個消費する必要がある。消費できなかった場合、その国力判定の達成値を1点減少する。この効果が発生するたび、減少する達成値が累積する（消費する燃料の数は累積しない）。",
    "遠征中に現地の勢力から接触を受ける。〔文化レベル〕で難易度9の判定を行う。成功すると、そのセッションに登場したモンスターの中からレベルが［王国レベル］以下のモンスター1種を選ぶ。そのモンスター1D6体が《モンスターの民》になる。失敗すると、周辺から［1D6］部屋を選ぶ。その部屋が部屋ダメージを受ける。部屋ダメージの処理は、遠征イベントと同様に処理する。",
    "何者かによる唐突な奇襲攻繋。〔軍事レベル〕で難易度9の判定を行う。成功すると、返り討ちにして《予算》を1D6MG獲得する。失敗すると、自国からランダムに3部屋を選ぶ。その部屋が部屋ダメージを受ける。部屋ダメージの処理は、遠征イベントと同様に処理する。",
    "民の労働の結果が明らかに……。〔生活レベル〕で難易度9の判定を行う。成功すると《予算》を［自国の生産施設の数］MG獲得する。失敗すると、《予算》を［王国レベル］MG減少する。",
    "あなたの活躍を耳にした者たちがやってくる。シナリオの目的を満たしていた場合、関係が良好・同盟の国が1つあるたびに1D6を振る。［その合計値+〔治安レベル〕人、《民》が増える。",
    "王国の子供たちがあなたがたを見て成長する。〔治安レベル〕で難易度9の判定を行う。成功すると《民》が［王国に残した《民》の数×1/10］D6人が増える。ただし、1回のセッションでこの効果によって増える《民》の上限は、［自国の居住施設の数×20+50］人までである。",
    "王国に残っている民たちが、旅する王国を改造していた。自国マップに通路を1D6本追加することができる。",
    "街の機能に異変がッ!?〔治安レベル〕で難易度9の判定を行う。成功すると、自国の好きな施設1軒を選び、その施設のレベルを1点上昇する。失敗すると、自国の部屋タイプの施設1軒をランダムに選び、その施設を破壊する。",
    "地元勢力との交流が行われた。〔文化レベル〕で難易度9の判定を行う。成功すると、「生まれ決定表」（基本p.24）でランダムにジョブを決めた逸材が1人増える。失敗すると、自国の好きな逸材1人を失う。",
    "ただ、無為に時が過ぎていたわけではない。冒険フェイズで過ごした1ターンにつき、《予算》を1MG上昇する。",
    "民の意識が大きく揺れる。1D6を振り、その出目が現在の《民の声》以下だったら、好きな国力を1つ選び、その基本値を1点上昇する（この効果で国力の基本値を3点以上にすることはできない）。その出目が現在の《民の声》を上回っていたら、好きな国力1つの基本値を1点減少する。",
];

/// Ruby `遠征王国変動表`（`2D6`）。
static MKB_TABLE_EKCT: Table = Table::from_dice("遠征王国変動表", 2, 6, MKB_TABLE_EKCT_ITEMS);

/// Ruby `王国ハプニング表`の項目。
static MKB_TABLE_KDHT_ITEMS: &[&str] = &[
    "不況に突入し、国内の景気が一気に悪化する。維持費を［1D6×1/2］MG上昇する。もし「国難」（大冒険p12）の選択ルールを使用している場合、維持費を上昇する代わりに、そのセッションの間、自国は「不況1」の国難を受ける。",
    "宮廷の暗殺計画の噂が流れる。そのセッションの間、PCが絶対失敗したとき、そのPCの《HP》を2D6点減少する。もし「国難」（大冒険p12）の選択ルールを使用している場合、《HP》を減少する代わりに、そのセッションの間、自国は「失望1」の国難を受ける。",
    "衛生状態が悪化し､疫病が発生する。〔生活レベル〕で難易度9の判定を行う。失敗すると《民》を2D6人減少する。もし「国難」（大冒険p12）の選択ルールを使用している場合、《民》を減少する代わりに、そのセッションの間、自国は「疫病1」の国難を受ける。",
    "火事だ！ 自国の領土の中からランダムに1つを選び、そのマップの中からランダムに施設1つを選ぶ。その施設に火災が発生する。〔治安レベル〕で難易度9の判定を行う。失敗すると、その施設は破壊される。そして、その部屋に隣接する部屋の施設の中からランダムに1つを選んで、再び〔治安レベル〕の判定を行う。このとき難易度は前回より1減少する。この処理を〔治安レベル〕の判定に成功するか、隣接する部屋の施設がなくなるまで繰り返す。もし「国難」（大冒険p12）の選択ルールを使用している場合、施設を破壊する代わりに、そのセッションの間、自国は「混迷1」の国難を受ける。",
    "怪しげな宗教が蔓延する｡〔文化レベル〕で難易度9の判定を行う。失敗すると《民の声》を1D6点減少する。もし｢国難」（大冒険p12）の選択ルールを使用している場合､《民の声》を減少する代わりに､そのセッションの間、自国は「飢餓1」の国難を受ける。",
    "近隣の王国による襲撃。関係が険悪か敵対になっている国があれば、その中からランダムに1国を選ぶ。〔軍事レベル〕で難易度9の判定を行う。失敗すると自国の領土の中からランダムに1つを選ぶ。そのマップの中から1D6個の部屋を失う。もし「国難」（大冒険p12）の選択ルールを使用している場合、領土を失う代わりに、そのセッションの間、自国は「戦乱1」の国難を受ける。",
];

/// Ruby `王国ハプニング表`（`1D6`）。
static MKB_TABLE_KDHT: Table = Table::from_dice("王国ハプニング表", 1, 6, MKB_TABLE_KDHT_ITEMS);

/// Ruby `王国試練表`の項目。
static MKB_TABLE_KDTT_ITEMS: &[&str] = &[
    "旧王国との交流がある。1D6を振る。奇数なら、救援物資がやってくる。「レア一般アイテム表」（基本p106､上級p66）の中からランダムに決めた1個を獲得する。偶数なら、《民》の一部がホームシックにかかり、新王国の《民》を2D6人減少し、その値だけ旧王国の《民》を増やす。",
    "民たちが､王国を発展させている｡1D6を振る。その目が奇数なら、「居住施設表」（基本p129）の中からランダムに決めた1軒を好きな部屋に建設する。その目が偶数なら、《予算》を1MG獲得する。",
    "他国の大使がやってくる。〔文化レベル〕か〔軍事レベル〕で難易度9の判定を行う。成功すると、周辺階域にある好きな国1つとの関係を「友好」にする。失敗すると、《民》を1D6人減少し、維持費が1MG上昇する。",
    "怪物の襲撃があった。〔軍事レベル〕か〔治安レベル〕で難易度9の判定を行う｡成功すると、怪物を撃退し、好きな素材を2D6個獲得できる。失敗すると、《民》を1D6人減少し、好きな部屋1つを失う（その部屋につながる通路もすべて失う）。",
    "新たな王国の噂を聞きつけ、周囲のならず者がやってくる。［王国の総人口×1/10］人、【人間の屑】（基本p195）が《モンスターの民》となる。この効果が発生したとき、【人間の屑】の数が［自国の王国レベル×20］人を超えていると、自国の施設の中からランダムに1軒を選び、それを破壊する。",
    "王国の子供たちが、あなたを見て成長する。《民》が［王国に残した《民》の数×1/10+〔治安レベル〕］D6人増える。",
    "新たに平和な国が生まれたという噂を聞きつけ、移民たちがやってくる。移民を受け入れると、維持費を2MG上昇し、《民》を［〔治安レベル〕×2］D6人増やす。",
    "王国の中で民同士の諍いが起きた。〔生活レベル〕か〔文化レベル〕で難易度9の判定を行う。成功すると、雨降って地固まり、《民の声》を1D6点上昇する。失敗すると、《民》を1D6人減少し、維持費を1MG上昇する。",
    "迷宮化現象が発生した。〔生活レベル〕か〔治安レベル〕で難易度9の判定を行う。成功すると、この土地に隠された資産を発見し、1D6MGを獲得する。失敗すると、《民》を1D6人減少し、領土の中から好きな部屋1つを失う（そこにつながる通路もすべて失う）。",
    "ランドメイカーが国を離れている間に、民たちの意識が変わる。人間カテゴリに《モンスターの民》1体選び、1D6を振る。その目が奇数なら、そのモンスターは、その目と等しいレベルの人間カテゴリの【人類の敵】を修得していない好きなモンスターに変化する。偶数なら、そのモンス夕一は逸材に変化する。「生まれ決定表」（基本p24）を使ってランダムにジョブを決定すること。その《モンスターの民》は、そのジョブの逸材に変化する。",
    "新王国は活気に溢れ、新しい施設が生まれる。1D6を振る。1なら【農地】（基本p130）、2なら【学校】（基本p131）、3なら【刑務所】（基本p131）、4なら【砦】（基本p131）、5なら【病院】（基本p131）、6なら【鍜冶屋】（基本p130）を1軒建設する。",
];

/// Ruby `王国試練表`（`2D6`）。
static MKB_TABLE_KDTT: Table = Table::from_dice("王国試練表", 2, 6, MKB_TABLE_KDTT_ITEMS);

/// Ruby `騎士道表`の項目。
static MKB_TABLE_CHVT_ITEMS: &[&str] = &[
    "勇猛。どんな敵に対しても決して退かない。戦闘中、自軍後方に1回移動するたび、この誓いは破られる。ただし、敵軍キャラクターの何らかの効果による強制的な移動は、これに含まない。特典：攻撃を受けたとき、ダメージを1点軽減できる（1未満にはならない） 。",
    "勇気。常に先陣をきらなければいけない。戦闘中、自軍の行動の順番になったとき、宮廷の中で一番最初に移動と行動の処理を行わなければならない。宮廷の他のキャラクターの後に移動と行動の処理を行うたび、この誓いは破られる。特典：自分の攻撃で威力を決定するとき、サイコロを振った湯合、その中から奇数の出目1つを選び、そのサイコロを裏返すことができる（1は6に、3は5に、5は2になる）。",
    "高潔。嘘をついてはいけない。意図的な嘘をつくたび、この誓いは破られる。特典：行為判定で絶対成功すると、自分が受けている変調をすべて回復する。",
    "忠誠。二君には仕えない。自分が「忠誠」の《好意》を持つ想い人を1人以下にする。誰かに「忠誠」の《好意》を1点以上持っている状態で、ほかの誰かに「忠誠」誠」の（好意〉が上昇するたび、この誓いは破られる。特典：〈HP〉の最大値と現在値が5点上昇する",
    "寛容。自分に刃向かうものであれ、命乞いするものを殺してはならない。命乞いしたキャラクターを死亡させるたび、この誓いは破られる。特典：自分を目標にして、タイプが「支援」のスキルが使用された場合、〔武勇〕で難易度［そのスキルの使用者の〔魅力〕+7］の判定を行うことができる。成功すると、その効果を無効化できる。",
    "信念。自分の「好きなもの」を踏みにじった者がいたとき、そのキャラクターに対し、戦いを挑まなければならない。それを見過ごすたび、この誓いは破られる。特典：自分が自分の「好きなもの」に関連する行為判定を行うとき、達成値が1点上昇する。",
    "礼節。初めて出会った友好的な相手に対して、必ず自分の頁実の名前を名乗り、自己紹介しなければならない。自己紹介をしないたび、この誓いは破られる。特典：そのセッションの間、血統スキル（上級p51）として【名乗る】（基本p114）を修得する。",
    "謙譲。高価な品を譲らなければならない。宮廷のほかのメンバーが持っていないレベルが1以上、もしくはレアアイテムを装備するたび、この誓いは破られる。特典：《HP》を回復する効果の対象になったとき、その効果が2点上昇する。",
    "奉仕。進んで宮廷に奉仕しなければいけない。あるクォーターで、自分以外の宮廷のキャラクター全員が、休憩を行っているときしか、休憩を行うことはできない。それ以外の状態で、休憩を行うたび、この誓いは破られる。特典：そのセッションの間《器》を1点上昇する。",
    "献身。自分の民を守らなければならない。自分の《配下》を減少するたび、この誓いは破られる。特典：このキャラクターが行為判定で絶対成功すると、《民の声》を1点回復する。",
    "士道。この誓いは「騎士の十戒」ではなく、フジヤマ系国家に伝わる特殊な騎士道である。異国の言葉を話してはならない。この誓いを修得したPCを操作するプレイヤーが外来語を話すたび、この誓いは破られる。特典：【アメリカ小鬼】以外を目標にして攻撃を行うとき、その威力を2点上昇する。",
];

/// Ruby `騎士道表`（`2D6`）。
static MKB_TABLE_CHVT: Table = Table::from_dice("騎士道表", 2, 6, MKB_TABLE_CHVT_ITEMS);

/// Ruby `儀式表`の項目。
static MKB_TABLE_RITT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("罠にかかる")),
    (12, TableItem::Text("互いに紐で体の一部を結ぶ")),
    (13, TableItem::Text("アイテムの交換")),
    (14, TableItem::Text("食べ物を投げつける")),
    (15, TableItem::Text("かくれんぼ")),
    (16, TableItem::Text("料理")),
    (22, TableItem::Text("歌を歌う")),
    (23, TableItem::Text("民の前で誓う")),
    (24, TableItem::Text("神官の前で誓う")),
    (25, TableItem::Text("星の前で誓う")),
    (26, TableItem::Text("怪物の前で誓う")),
    (33, TableItem::Text("名前を変える")),
    (34, TableItem::Text("ブーケやガーターを投げる")),
    (35, TableItem::Text("奇抜な衣装を着る")),
    (36, TableItem::Text("刺青や染髪、化粧をする")),
    (44, TableItem::Text("踊る")),
    (45, TableItem::Text("冒険")),
    (46, TableItem::Text("誓いのキス")),
    (55, TableItem::Text("断食")),
    (56, TableItem::Text("決闘")),
    (66, TableItem::Text("裸になる")),
];

/// Ruby `儀式表`（D66）。
static MKB_TABLE_RITT: D66Table = D66Table::new("儀式表", D66SortType::Asc, MKB_TABLE_RITT_ITEMS);

/// Ruby `限定表`の項目。
static MKB_TABLE_LIMT_ITEMS: &[&str] = &[
    "年齢。子どもや年寄りなど、特定の年齢の者のみ行う。",
    "性別。男性や女性など、特定の性別の者のみ行う。",
    "職能。クラスやジョブなど、特定の職能を持つ者のみ行う。",
    "地位。民や逸材、ランドメイカーなど、特定の社会的地位の者のみ行う。",
    "期間。特定の日や季節、死ぬ間際、誰かが生まれたとき、夏星や冬星の目覚めや眠り、狩りや収穫の時期など、特定の時期・期間にだけ行われる。",
    "場所。宮廷や農地のような、王国内やその近くにある特定の場所・施設で行われる。",
];

/// Ruby `限定表`（`1D6`）。
static MKB_TABLE_LIMT: Table = Table::from_dice("限定表", 1, 6, MKB_TABLE_LIMT_ITEMS);

/// Ruby `固有の文化表`の項目。
static MKB_TABLE_UNCT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("婚姻。結婚にまつわる異質な儀式や習慣がある。"),
    ),
    (
        12,
        TableItem::Text("言語。ユニークな放言や特殊語尾を交えて会話する。"),
    ),
    (
        13,
        TableItem::Text("育成。珍しい動物や植物を育成している。"),
    ),
    (14, TableItem::Text("旅行。特定の場所を旅する習慣がある。")),
    (
        15,
        TableItem::Text("衣装。独特な民族衣装や髪型、刺青がある。"),
    ),
    (
        16,
        TableItem::Text("舞楽。伝統的な歌や踊り、民族楽器がある。"),
    ),
    (
        22,
        TableItem::Text("風呂。特色のある風呂や入浴方法がある。"),
    ),
    (23, TableItem::Text("料理。味わい深い民族料理がある。")),
    (
        24,
        TableItem::Text("飾り物。へんてこなものを戸口や家の中に飾る。"),
    ),
    (25, TableItem::Text("詩句。変わったリズムの詩を読む。")),
    (
        26,
        TableItem::Text("不敬。特定のポーズや習慣が侮辱的な意味を持つ。"),
    ),
    (33, TableItem::Text("挨拶。クセがすごい挨拶の習慣がある。")),
    (
        34,
        TableItem::Text("断食。食べたり、飲んだりしてはいけないものがある。"),
    ),
    (
        35,
        TableItem::Text("労働。ほかと異なる時間や形式で働いている。"),
    ),
    (36, TableItem::Text("贈り物。特定のものを贈り合う。")),
    (
        44,
        TableItem::Text("住居。天井や水の中など、あまりない場所に住んでいる。"),
    ),
    (45, TableItem::Text("成人。不思議な成人の儀式がある。")),
    (
        46,
        TableItem::Text("供養。葬送や物の廃棄に関する神秘的な儀式や習慣がある。"),
    ),
    (
        55,
        TableItem::Text("遊戯。風変わりな遊びやゲームが伝わっている。"),
    ),
    (56, TableItem::Text("決闘。奇妙な手法で戦いを行う。")),
    (66, TableItem::Text("教育。不可解な教育法がある。")),
];

/// Ruby `固有の文化表`（D66）。
static MKB_TABLE_UNCT: D66Table =
    D66Table::new("固有の文化表", D66SortType::Asc, MKB_TABLE_UNCT_ITEMS);

/// Ruby `後世の評価表`の項目。
static MKB_TABLE_POET_ITEMS: &[&str] = &[
    "曰く【事件名】は、王国に何の影響ももたらさなかった。しかし、なぜかその噂は様々なおひれをともなって百万迷宮中に流布し、【事件名表1の結果】を見物するために、大勢の観光客が訪れることになったという。",
    "曰く【事件名】は、後に起こる王国崩壊の前触れであった。この事件をきっかけとして、【自国名】は、【王国名表1の部分だけランダムに変更した自国名】と【王国名表1の部分だけランダムに変更した自国名】の二つに分裂したという。",
    "曰く【事件名】は、【自国名】最大の恥部とされ、王国史から抹殺された。その秘密は、代々の国王のみに継承され、王国でその名前を口にすることは禁忌となった。いつか【事件名表1で決めた結果】のことを知る者は、国王を除きすべていなくなったという。",
    "曰く【事件名】は、百万迷宮中を揺るがす大事件の幕開けに過ぎなかった。この後、【自国名】は百万迷宮全体を揺るがす巨大な陰謀の一部に呑み込まれていったという。",
    "曰く【事件名表1で決めた結果】は、この事件をきっかけとして近隣の小王国から列強にまで、その名を轟かせた。【事件名表1で決めた結果】こそ【自国名】を象徴する何かであり、その名は代々、王国の名と共に語り継がれたという。",
    "曰く【事件名】は、王国の記念すべき一日となった。その日は、王国の休日となり、毎年、その日になるとある者は父祖の偉業を讃え、ある者は【事件名】の礎となった者たちに想いをはせたという。",
    "曰く【事件名表1で決めた結果】の功績により、王国の危機は救われた。【自国名表1で決めた結果】は民の間でも長く語り継がれ、後に【自国名】に伝わる国教の聖人（聖地、聖獣、聖遺物など）として、広く信仰されることになったという。",
    "曰く【事件名表1で決めた結果】は、近隣の迷宮に棲む怪物たちに大きな影響を与えた。その結果、ある怪物は【事件名表1で決めた結果】を恐れてその活動を縮小し、ある怪物はこの地を去った。そうした出来事が重なっていくうちに、いつの間にか近隣から【その冒険に登場した怪物】が絶滅したという。",
    "曰く【事件名】は、列強や周辺の小王国の間で大変不名誉な出来事として語り継がれることになった。今でも幾つかの国の人々は、あなたの国名を聞くと、「あなたがあの【事件名表1で決めた結果】で有名な」と罵倒されるという。",
    "曰く【事件名】は、【ランダムに決めたランドメイカー】が運命の人に、初めて巡り会った瞬間だった。この事件をきっかけとして、二人は狂おしく引かれあい、そして後に憎み、呪いあった。その激しい様は、王国中を騒がせたという。",
    "曰く【事件名】は、各国の取材を受け、大層楽しまれた。この事件は、多くの戯作者の心をトリコにし、絵巻物や演劇、プレイレポートやリプレイ、実況動画などの幅広い分野で人口に膾炙したという。",
];

/// Ruby `後世の評価表`（`2D6`）。
static MKB_TABLE_POET: Table = Table::from_dice("後世の評価表", 2, 6, MKB_TABLE_POET_ITEMS);

/// Ruby `高レベル特殊遭遇表`の項目。
static MKB_TABLE_KSET_ITEMS: &[&str] = &[
    "侵攻途中の巨鬼領の軍に遭遇する。【巨鬼英雄】（上級p95）は雄叫びをあげ、【小鬼司令部】（基本p172）は野蛮な戦歌を奏でた。侵攻の前哨戦とばかりに鬼たちは、猛然と宮廷へと襲いかかってきた。宮廷全員はこの戦いの嵐に立ち向かうか、逃げるかを選択する。立ち向かう場合、宮廷全員は〔武勇〕難易度13の判定を行う。失敗した者は［2D6+18－判定の達成値］点のダメージを受ける。逃げることを選択した場合、宮廷の代表は〔探索〕で難易度10の判定を行う。成功すると、宮廷全員はその部屋から通路でつながっている探索済みの部屋へ移動する。失敗した場合、宮廷全員は［2D6+3］点のダメージを受ける。もし「国難」（大冒険p12）の選択ルールを使用している場合、そのセッションの間、自国は「戦乱2」の国難を受ける。",
    "いつの間にか【スフィンクス】（基本p181）が、そこにいた。賢しげな獣は有無を言わせぬ調子で宮廷に問いかけてきた。宮廷の代表は〔才覚〕で難易度14の判定を行う。成功すると、【スフィンクス】は自分の不明を恥じて部屋を去る。失敗した者は《気力》をすべて失い、「散漫1」と「呪い4」の変調を受ける。もし「国難」（大冒険p12）の選択ルールを使用している場合、そのセッションの間、自国は「失望2」の国難を受ける。",
    "無数の飛蝗の群れに遭遇する。【飛蝗の王】（基本p186）だ！宮廷全員は〔武勇〕で難易度10の判定を行う。自国の施設の中から［1D6－判定に成功したPCの数］軒を選び、それを破壊する。もし自国に【農地】があれば、それを優先して破壊すること。もし「国難」（大冒険p12）の選択ルールを使用しているときに、施設が1軒以上破壊された場合、そのセッションの間、自国は「飢餓2」の国難を受ける。",
    "周囲が気温が下がり、【死神】（基本p190）が現れる。【死神】はその鎌を宮廷に向けた。宮廷全員は〔武勇〕で難易農14で判定を行う。一度でも死亡したことがあるPCは、この難易度が2上昇する。失敗した者は、2D6+4点のダメージを受け、《配下》を1D6人減少する。もし「国難」（大冒険p12）の選択ルールを使用しているときに、誰か1人でも判定に失敗すると、そのセッションの間、自国は「疫病2」の国難を受ける。",
    "通路から大量の水が流れ込んでくる。そしてその水の中から【魔蟹】（基本p206）が浮上した。その部屋に【水槽】（基本p153）のトラップを1つ配置し、1クォーターが経過する。宮廷全員は〔探索〕で難易度14の判定を行う。失敗した者は、「迷宮化減少表」（p48）を1回使用する。もし「国難」（大冒険p12）の選択ルールを使用している場合、誰か1人でも判定に失敗すると、そのセッションの間、自国は「混迷2」の国難を受ける。",
    "「汝らは悪しき未来に向かっている」【処罰天使】（大冒険p114）が降臨し、PCたちの国がいかに堕落し、退廃しているかを説く。そして自分たちの罪を認め、贖罪を行うようにうながす。宮廷の代表は〔魅力〕で難易度14の判定を行う。成功すると、【処罰天使】は自分の非を認め、天階へと戻っていく。失敗すると､《民の声》を1D6点減少し､判定を行ったPCは3D6+1点のダメージを受け、《配下》を1D6人減少する。残りの宮廷全員は1D6点のダメージを受ける。",
];

/// Ruby `高レベル特殊遭遇表`（`1D6`）。
static MKB_TABLE_KSET: Table = Table::from_dice("高レベル特殊遭遇表", 1, 6, MKB_TABLE_KSET_ITEMS);

/// Ruby `死霊の日々表`の項目。
static MKB_TABLE_DODT_ITEMS: &[&str] = &[
    "死霊たちは死霊仲間たちを呼び出す。《特殊配下》が1D6人増える。死霊カテゴリの3レベル以下のモンスターの中から1種類を選ぶ。この《特殊配下》は、その種類の《モンスターの民》として扱う。終了フェイズになると、この効果によって増えた《特殊配下》は失われる。",
    "死霊たちは【死者との会話】のまじないをかける。そのセッション中に死亡したキャラクター1体を選ぶ。そのキャラクターの知っている情報（GMが判断する）を何か1つ入手できる。",
    "死霊たちは見えないなにかと会話をしている。〔魅力〕で難易度9の判定を行う。成功すると、シナリオの舞台となっているマップの中から好きな部屋1つを選ぶ。その部屋の脅威情報を獲得する。",
    "死霊たちの祝福を受ける｡〔魅力〕で難易度9の判定を行う｡成功すると､そのキャラクターは「呪い3」の変調を受ける。この「呪い3」の効果を受けている間、そのキャラクターは、攻撃目標の【外皮】、【霊体】、【ヨロイ】（基本p165）を無効化してダメージを与えることができるようになる。失敗すると、《HP》を1D6点減少する。",
    "死霊たちは心が渇いたと言い出す。死霊カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合､《配下》にいる《モンスターの民》から選ぶ）。1D6を振る。その目が奇数なら、そのモンスターはあなたに対する好きな《感情値》を1点上昇する。偶数なら、そのモンスターはあなたに対する好きな《感情値》を1点上昇する。もし「契約者」（上級p42）の選択ルールを使用している場合、その死霊は、「死霊の日々表」を使用したキャラクターに契約を申し出る。",
    "死霊たちは不思議な踊りを踊った。宮廷の中からランダムにキャラクターを1人選ぶ。そのセッションの間、そのキャラクターはダメージを1点軽減できるようになり、スキルやアイテムによる《HP》の回復量が1点減少する（共に1未満にはならない）。この効果は最大3点まで累積する。",
    "死霊たちはお腹が空いたと言い出す。宮廷の中から好きなキャラクター1人を選ぶ。死霊に生命力を分け与えるなら、そのキャラクターの《HP》を［1D6+そのモンスターのレベル］点減少する。そして、そのセッションの間、【死者の眠り】（基本p188）を修得（このモンスタースキルの使用に《希望》消費は必要ない）し、この効果によって減少した《HP》の半分の値だけ、そのキャラクターの《HP》の最大値が減少する。自分の生命力を分け与えないなら、死霊カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合､《配下》にいる《モンスターの民》から選ぶ）。そのモンスターを失う。",
    "死霊たちは王国の祖霊たちに働きかける。〔才覚〕で難易度9の判定を行う。成功すると、好きな国力を1つ選ぶ｡そのセッションの間、その国力の判定の達成値を1上昇する。この効果によって、同じ国力の判定の達成値を3以上、上昇させることはできない。",
    "死霊たちは逆さ言葉で不吉な詩を読む。宮廷の中からランダムにキャラクターを1人選ぶ。次に戦闘が起こった場合、最初のラウンドの敵軍キャラクター全員は、攻撃を行うとき、その性格にかかわらず、そのキャラクターを目標に選ぼうとする。敵軍キャラクターは、そのキャラクターを目標に選べない場合、最初のラウンドは攻撃を行わない。この効果が複数回発生し、選ばれたキャラクターが2人以上いる場合、敵軍キャラクターは、その中から好きな者を攻撃の目標に選ぶことができる。",
    "死霊たちはつい国民を食べようとする。〔武勇〕で難易度9の判定を行う。成功すると、死霊は民を食べるのを諦める。《民の声》を1点回復する。失敗すると、《民》を1D6人減少し（冒険フェイズの場合、この表を使用したPCの《配下》を1D6人減少）、《民の声》を2点減少する。",
    "「こんなところに昇降機が」死霊は天階に昇天、もしくは深階に沈没する。その部屋に【エレベータ】（基本p152）を発見する（王国フェイズの場合、自国のマップの中から好きな部屋1つに【神殿】（基本p128）を1軒建設できる）。死霊カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ）。その《モンスターの民》を失う。昇降機の中から、好きな素材を［失ったモンスターのレベル×3］個獲得する。",
];

/// Ruby `死霊の日々表`（`2D6`）。
static MKB_TABLE_DODT: Table = Table::from_dice("死霊の日々表", 2, 6, MKB_TABLE_DODT_ITEMS);

/// Ruby `事件名表1`の項目。
static MKB_TABLE_INT1_ITEMS: &[&str] = &[
    "王国にある施設の名前",
    "自国の名前、またはその一部",
    "冒険に登場したトラップ名",
    "ランダムに選んだランドメイカー1名のクラスかジョブ",
    "冒険に登場したモンスター名かNPC名",
    "ランダムに選んだランドメイカー1名の名前",
    "冒険を行ったマップ名、もしくは部屋の名前",
    "冒険に登場したアイテム名",
    "ランダムに選んだランドメイカーの好きなものか嫌いなもの",
    "ランダムに選んだランドメイカー1名の修得しているスキル",
    "シナリオの名前、またはその一部",
];

/// Ruby `事件名表1`（`2D6`）。
static MKB_TABLE_INT1: Table = Table::from_dice("事件名表1", 2, 6, MKB_TABLE_INT1_ITEMS);

/// Ruby `事件名表2`の項目。
static MKB_TABLE_INT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("の変（事変）")),
    (12, TableItem::Text("の屈辱")),
    (13, TableItem::Text("の乱（反乱）")),
    (14, TableItem::Text("の役（戦役）")),
    (15, TableItem::Text("の勅令")),
    (16, TableItem::Text("の戦い")),
    (22, TableItem::Text("の憂鬱")),
    (23, TableItem::Text("の発明")),
    (24, TableItem::Text("遠征")),
    (25, TableItem::Text("攻略")),
    (26, TableItem::Text("統一")),
    (33, TableItem::Text("計画")),
    (34, TableItem::Text("暗殺")),
    (35, TableItem::Text("会議")),
    (36, TableItem::Text("裁判")),
    (44, TableItem::Text("宣言")),
    (45, TableItem::Text("征服")),
    (46, TableItem::Text("騒動")),
    (55, TableItem::Text("事件")),
    (56, TableItem::Text("大戦")),
    (66, TableItem::Text("革命")),
];

/// Ruby `事件名表2`（D66）。
static MKB_TABLE_INT2: D66Table =
    D66Table::new("事件名表2", D66SortType::Asc, MKB_TABLE_INT2_ITEMS);

/// Ruby `小鬼と一緒表`の項目。
static MKB_TABLE_WLDT_ITEMS: &[&str] = &[
    "小鬼たちは、いつの間にか増えていた。ただし、何か失敗したあとが……。「探索ハプニング表」を1回使用する。その後、名前に「小鬼」を含む《モンスターの民》の中からランダムに1種を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ） 。その《モンスターの民》を1D6体増やす。",
    "小鬼は一所懸命に働いている。好きな素材1個を獲得し、《民の声》を1点回復する。",
    "小鬼は、がんばって宮廷の装備を磨いている。一通り旅の道具を整備しおわった小鬼は、他にも整備するものがないかをきいてくる。あなたは、小鬼にアイテムを渡すことができる。アイテムを渡すことにした場合、サイコロを振ること。奇数なら、1D6クォーターの間、そのアイテムのレベルを1点上昇する。偶数なら、小鬼は、そのアイテムを破壊する。",
    "巨鬼の傭兵団が現れる。巨鬼は、配下の中に小鬼がいるのを見つけると、その小鬼を引き渡すように告げた。あなたたちは、小鬼を引き渡すこともできるし、巨鬼と戦うこともできる。小鬼を1体以上引き渡すことにした場合、引き渡した名前に「小鬼」を含む《モンスターの民》1体につき、「牙」か「革」の素材を［そのモンスターのレベル］個獲得する。巨鬼たちと戦うことにした場合、宮廷の代表は〔武勇〕で難易度［7+宮廷の平均レベル］の判定を行う。失敗すると、名前に「小鬼」を含む《モンスターの民》をすべて失い、宮廷全員は《HP》を1D6点減少する。",
    "小鬼は、小さな鍋で小鬼汁をつくっている。微妙なニオイが漂っているが……宮廷全員は、小鬼汁を食べることができる。食べることにしたキャラクターは〔探索〕で難易度9の判定を行う。成功すると、《HP》を1D6点回復し、1ターンの間、与えるダメージを1点上昇する。失敗すると、「毒2」の変調を受け、《気力》を1点減少する。判定の成功・失敗にかかわらず、小鬼汁を食べたキャラクターは「食事」をしたことになる。",
    "小鬼たちは、いそいそと冒険や休憩の準備をしている。その様子は愛らしく、思わず心が温かくなる。宮廷全員は〔魅力〕で難易度9の判定を行う。成功した者は《気力》を1点回復する。",
    "小鬼はうっかり転んでしまう。名前に「小鬼」を含む《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ）。あなたは、〔武勇〕で難易度9の判定を行うことができる。成功すると、あなたは小鬼をかばうことに成功し、その小鬼は、あなたに対し《好意》を1D6点上昇する。もし「契約者」（上級p42）の選択ルールを使用している場合、その小鬼は、あなたに契約を申し出る。失敗するとそのケガがもとでその小鬼は死んでしまう。その《モンスターの民》を失う。",
    "小鬼があなたと仲間の間をとりもってくれる。宮廷の中から好きなキャラクター1人を選ぶ。そのキャラクターのあなたに対する《好意》を1点上昇する。",
    "あなたは小鬼と一緒に焚き火にあたりながら昔話をした。小鬼は、だまってその話を聞いている。あなたは〔魅力〕で難易度9の判定を行う。成功すると、そのセッションの間あなたは名前に「小鬼」を含む《特殊配下》1体を、《希望》1点として使用することができる。",
    "異形の群れが影から現れる。不思議な妖術によって、あなたはさらわれてしまう。小鬼は、主人をさらわれた怒りに燃えている。1D6クォーターの間、あなたは自分のPCの代わりに、《モンスターの民》の中から名前に「小鬼」を含むモンスター1体を選び、そのデータを使用してゲームに参加すること。1D6クォーター後、あなたは再び自分のPCを使用できるようになる。",
    "「今までにホントにお世話になりました」小鬼は、贈り物を置いて、あなたがたの王国を去る。名前に「小鬼」を含む《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスタ－の民》から選ぶ）。そのモンスターを失う。そして、価格の（　）内の数字が、そのモンスターのレベル未満のレアアイテムを1個獲得する。",
];

/// Ruby `小鬼と一緒表`（`2D6`）。
static MKB_TABLE_WLDT: Table = Table::from_dice("小鬼と一緒表", 2, 6, MKB_TABLE_WLDT_ITEMS);

/// Ruby `侵略ハプニング表`の項目。
static MKB_TABLE_INHT_ITEMS: &[&str] = &[
    "敵軍の奇襲！宮廷全員は〔武勇〕で難易度9の判定を行う。失敗したキャラクターは《HP》を1D6点減少し、修得しているスキルの中からランダムに1つを選んで、1ターンの間喪失する。もし、その部屋を制圧していた場合、その部屋に配置した《配下》を［1D6－この〔武勇〕の判定に成功したPCの数］人減少する。",
    "敵軍の妨害工作により異変が！〔軍事レベル〕で難易度9の判定を行う。失敗すると､GMは、制圧した部屋の中からランダムに1つを選ぶ。その部屋に配置した《配下》を2D6人減少する。",
    "敵軍があなたたちの食料に毒を混ぜた。〔治安レベル〕で難易度9の判定を行う。失敗すると、配置したすべての部屋の《配下》を1D6人減少し、宮廷全員の《HP》を1D6点減少する。また宮廷全員は「毒2」の変調を受ける。",
    "あなたたちの留守を狙って敵国が襲来！〔軍事レベル〕で難易度7の判定を行う。失敗すると、ランダムに選んだ自国の領土1つを失う。もしも今戦っている国以外に、関係が険悪・敵対の国が周辺階域にいれば、この判定は自動的に失敗する。もしも、そのターン以内に、宮廷が自国に帰還することができれば、この判定が失敗していても領土を失わずに済む。",
    "列強の調停が入った。そのターンの間しか侵略を行えなくなる。そのターンが終了すると、強制的に冒険フエイズは終了する。",
    "兵站の消耗が激しい。宮廷の代表は【お弁当】か【フルコース】1個を消費する。消費できなければ、配置したすべての部屋の《配下》を1D6人減少する。",
];

/// Ruby `侵略ハプニング表`（`1D6`）。
static MKB_TABLE_INHT: Table = Table::from_dice("侵略ハプニング表", 1, 6, MKB_TABLE_INHT_ITEMS);

/// Ruby `新・情報収集表`の項目。
static MKB_TABLE_NIGT_ITEMS: &[&str] = &[
    "調査隊に的確な指示を与えることができた。迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の通路情報をGMに教えてもらう。【携帯電話】を装備していたら、「新・情報収集表」をもう1回使用できる。",
    "調査隊は罠にかかるが､なんとか情報を持ち帰る。《配下》を1人消費すると、迷宮マップの中から好きな部屋を1つ目標に選ぶことができる。目標の脅威情報の中から卜ラップの数と通路情報をGMに教えてもらう。目標から他の部屋に通路がつながっている場合、さらに｢新・情報収集表」をもう1回使用できる。",
    "ある部屋にいる怪物の噂を聞く。迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の脅威情報の中からモンスターの数と、その部屋にいるモンスターの種類を教えてもらう。",
    "調査隊は､怪物にまつわる情報を入手した！迷宮マップの中から好きな部屋を2つ目標に選ぶ。目標の脅威情報をGMに教えてもらう。",
    "危険な迷宮を調査隊は進む。《配下》を1人消費すると、迷宮マップの中から好きな部屋を1つ目標に選ぶことができる。目標の脅威情報と通路情報をGMに教えてもらう。目標から他の部屋に通路がつながっていない場合、PCは行動済みにならず、もう一度、指揮判定を行うことができる。",
    "入り口にたどりつく。迷宮マップの中から【入り口】のある部屋1つをGMに教えてもらい、その部屋を目標に選ぶ。目標の脅威情報をGMに教えてもらう。その後、《配下》を1D6人消費することができる｡《配下》を1D6人消費すると、PCは行動済みにならず、もう一度、指揮判定を行うことができる。",
    "調査隊は不慮の事故に巻き込まれる。《配下》を1人消費すると、迷宮マップの中から好きな部屋を1つ目標に選ぶことができる。目標の脅威情報と通路情報をGMに教えてもらう。",
    "調査隊は無事、迷宮にたどりつく。迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の脅威情報と通路情報をGMに教えてもらう。",
    "調査隊は的確に危険な部屋を見つける。《配下》を1人消費すると、GMは、迷宮マップの中からPCがまだ脅威情報を入手しておらず、かつモンスターの数が1体以上いる部屋を1つ目標に選ぶ。目標の脅威情報をGMに教えてもらう。",
    "調査隊は無理をしつつも、その情報を入手した。《配下》を3人消費すると、迷宮マップの中から好きな部屋を3つ目標に選ぶ。目標の脅威情報と通路情報をGMに教えてもらう。",
    "調査隊の素晴らしい活躍！迷宮マップの中から好きな部屋を1つ目標に選ぶ。目標の脅威情報と通路情報をGMに教えてもらう。「新・情報収集表」をもう1回使用できる。",
];

/// Ruby `新・情報収集表`（`2D6`）。
static MKB_TABLE_NIGT: Table = Table::from_dice("新・情報収集表", 2, 6, MKB_TABLE_NIGT_ITEMS);

/// Ruby `人種特徴表`の項目。
static MKB_TABLE_RACT_ITEMS: &[&str] = &[
    "その人種は、通常の人間は食べない不思議なものから栄養を摂取できる。人種名から想像される食性を設定すること。",
    "その人種は、優れた感覚器官を持っている。人種名から想像される特徴的な知覚能力を設定すること。",
    "その人種は、何かをつくることに優れている。人種名から想像される得意な制作技術を設定すること。",
    "その人種は、運動能力に優れている。人種名から想像される、得意な運動技術を設定すること。",
    "その人種は、変わった精神構造をしている。人種名から想像される特徴的な性格や嗜好を設定すること。",
    "その人種は、変わった外見的特徴を持っている。人種名から想像される外見的特徴を設定すること。",
];

/// Ruby `人種特徴表`（`1D6`）。
static MKB_TABLE_RACT: Table = Table::from_dice("人種特徴表", 1, 6, MKB_TABLE_RACT_ITEMS);

/// Ruby `人体迷宮化表`の項目。
static MKB_TABLE_HBLT_ITEMS: &[&str] = &[
    "目神の祝福（めがみのしゅくふく）。身体の一部に光彩が迷宮化した異形の目が生まれる。射程1までの対象には、【星の欠片】がなくても射撃戦用武器を使って攻撃を試みることができる。名前に「視線」を含むスキルの目標になったとき、その判定の難易度が2上昇する。",
    "右手症。右手が怪物の如き異形の形に変わる。攻撃時、【右手】（射程：0、威力：3の白兵戦用武器）を武器として選ぶことができる。【右手】はアイテムを目標とした効果の影響を受けない。また、【徒手空拳】（上級p70）の威力を3点上昇する。《配下》の最大値が2減少する。",
    "胞。皮膚の上に幾つかの胞状器官が生まれる。別名、脅威の小部屋、偽乳、〇〇袋などとも呼ばれる。セッション中、このキャラクターが絶対失敗すると1D6を振る。奇数なら胞状器官の中から、古典的トラップの中からランダムに選んだものが1つ生まれる（配置する場所は、GMが決定する）。偶数なら、コモンアイテムの中からランダムに選んだもの1個を獲得する。",
    "壁化症。身体が硬化し、石壁のようになる。別名、鍾乳骨、蝸牛殻などとも呼ばれる。この変異を煩っているキャラクターは、そのゲーム中に起きた絶対失敗の回数を記録する。自分がダメージを受けたとき、その回数と同じ値だけダメージを軽減できる（1未満にはならない）。ただし、その回数が6以上になると、迷宮の壁や床、柱に取り込まれ、死亡する。",
    "捻れ葉翅（ねじればね）。背中から、植物の葉のような迷宮化した羽根状の器官が生える。この器官は、鉱物をエネルギーに変換する鉱合成を行える。鉱合成は支援行動として扱う。鉱合成を行う場合、「鉄」と「火薬」の素材を1個ずつ消費して1D6を振る。1なら【お弁当】、2なら【特効薬】、3なら【お酒】、4なら【フルコース】を1個獲得し、5か6なら「肥満3」の変調を受ける。",
    "心性迷宮化症候群。精神の迷宮化現象。様々な症例があるが、いずれも強い攻撃性を誘発する。サイクルの終了時に、このキャラクターが《敵意》を持っている相手に対し、その《敵意》の値と同じだけダメージを与える。ダメージを与える相手は、戦闘中なら同じエリアにいるキャラクター、それ以外なら同じ部屋にいるキャラクターに限定される。",
];

/// Ruby `人体迷宮化表`（`1D6`）。
static MKB_TABLE_HBLT: Table = Table::from_dice("人体迷宮化表", 1, 6, MKB_TABLE_HBLT_ITEMS);

/// Ruby `勢力表`の項目。
static MKB_TABLE_POWT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("災厄書宮（災厄王の痕跡を消して回る秘密結社）"),
    ),
    (
        12,
        TableItem::Text("災厄教（徳と法を重んじる禁欲主義者たち。基本p222、上級p20）"),
    ),
    (
        13,
        TableItem::Text("勝利教（ひたすらに戦争や闘争を求める戦闘主義者たち。基本p222、上級p20）"),
    ),
    (
        14,
        TableItem::Text("狂宴教（本能や欲望に忠実に生きる享楽主義者たち。基本p222、上級p20）"),
    ),
    (
        15,
        TableItem::Text("博覧教（知識と魔術の蒐集者たち。基本p222、上級p20）"),
    ),
    (
        16,
        TableItem::Text("巨鬼領（獲物と戦場を求めて征服戦争を続ける巨鬼たち。大冒険p82）"),
    ),
    (
        22,
        TableItem::Text(
            "タルタル盗賊団（メトロ汗国の先祖「竜の民」の血を引く大盗賊団。大冒険p39）",
        ),
    ),
    (
        23,
        TableItem::Text("ダイナマイト帝国（列強。基本p230、大冒険p30）"),
    ),
    (24, TableItem::Text("千年王朝（列強。基本p232、大冒険p34）")),
    (
        25,
        TableItem::Text("メトロ汗国（列強。基本p234、大冒険p38）"),
    ),
    (
        26,
        TableItem::Text("ハグルマ資本主義神聖共和国（列強。基本p236、大冒険p42）"),
    ),
    (
        33,
        TableItem::Text("緋色の研究（魔道師たちの暗殺結社。怪物の合成を得意とする。上級p117）"),
    ),
    (
        34,
        TableItem::Text(
            "ルドスの末裔（迷宮災厄以前にいた迷路職人の末裔。狂的な迷宮主義者。上級p104）",
        ),
    ),
    (
        35,
        TableItem::Text("黒襟巻（神話時代より続く小鬼の誘拐団。上級p91）"),
    ),
    (
        36,
        TableItem::Text("腐敗の跫音（蝿の王を信奉する邪教集団。大冒険p95）"),
    ),
    (
        44,
        TableItem::Text(
            "月喰らい楽団（魔法実験の犠牲者たちによる魔道師への復讐結社。上級p112、大冒険p37）",
        ),
    ),
    (
        45,
        TableItem::Text("善なる鉄槌（天使の加護のもと独善的な自誓行為を続ける騎士団。上級p120）"),
    ),
    (
        46,
        TableItem::Text(
            "ロストムンド（ビクトリー帝国のあとにできた巨大な死霊帝国。基本p187、上級p108）",
        ),
    ),
    (
        55,
        TableItem::Text(
            "黒真珠砦（グランドゼロ侵攻のためにつくられた深人たちの前線基地。上級p124）",
        ),
    ),
    (
        56,
        TableItem::Text(
            "ホラアナ城（百万迷宮の中心にある迷宮職人の街。巨大昇降機がシンボル。大冒険p106）",
        ),
    ),
    (
        66,
        TableItem::Text("アスベスト（反ダイナマイト帝国の武装組織。構成員に死霊が多い。上級p108）"),
    ),
];

/// Ruby `勢力表`（D66）。
static MKB_TABLE_POWT: D66Table = D66Table::new("勢力表", D66SortType::Asc, MKB_TABLE_POWT_ITEMS);

/// Ruby `中立特殊遭遇表`の項目。
static MKB_TABLE_NSET_ITEMS: &[&str] = &[
    "【牛頭】（基本p172）の傭兵2体が現れた。要求：2MG〔武勇〕4",
    "手負いの【小鬼英雄】（基本p171）1体が現れた。要求：タイプが補助か割り込みの《HP》回復効果を持つスキルかアイテムの使用〔武勇〕3",
    "壁の穴を開けて【黒ドワーワ】（基本p174）の鉱夫が2体現れた。要求：「鉄」か「魔素」か「牙」の素材を好きな組み合わせで5個〔武勇〕4",
    "野良の【ウマトカゲ】（基本p178）が1頭現れた。要求：「肉」か「衣料」の素材を好きな組み合わせで5個〔武勇〕4",
    "床に伸びた影の中から【影人】（基本p189）が1体浮かび上がる。要求：誰かの《気力》を2点〔武勇〕5",
    "血に飢えた【魔剣】（基本p193）1本が契約を迫る。要求：PC1人の《HP》3D6点か《配下》1人〔武勇〕7",
];

/// Ruby `中立特殊遭遇表`（`1D6`）。
static MKB_TABLE_NSET: Table = Table::from_dice("中立特殊遭遇表", 1, 6, MKB_TABLE_NSET_ITEMS);

/// Ruby `超協調行動表`の項目。
static MKB_TABLE_ECBT_ITEMS: &[&str] = &[
    "あなたの言葉に、人だけでなく装備も応えてくれる。協調行動を受けたキャラクターは、自分が持っているアイテムの中からランダムに1個を選ぶ。そのレベルを1点上昇する。",
    "あなたの声援は、相手の心に染み渡り、その肉体を癒す。相手のことを心配するロールプレイを行うと、協調行動を受けたキャラクターは、《HP》を［その《好意》の値+1］D6点回復する。",
    "相手の成功を心から信じる。協調行動を受けたキャラクターが、その判定に成功すると、自分と協調行動を受けたキャラクターは、《気力》を2点回復する。",
    "あなたは思いがけず言った自分の言葉で、自分の気持ちに気づいてしまう。協調行動を行った相手への《好意》を1点上昇させる。",
    "熱い友情が炸裂する！協調行動を行った相手への《好意》が「友情」の場合、協調行動による達成値の上昇が、《好意》の2倍になる。もし、その《好意》が「友情」以外なら、そのキャラクターとの友情を確認するロールプレイを行うと､その属性を｢友情」に変更し､協調行動による達成値の上昇を《好意》の2倍にできる。",
    "その人への想いがほとばしる！協調行動を行った相手への《好意》が「愛情」の場合、協調行動による達成値の上昇が、《好意》の2倍になる。もし、その《好意》が「愛情」以外なら、愛の告白をするロールプレイを行うと、その属性を｢愛情」に変更し、協調行動による達成値の上昇を《好意》の2倍にできる。",
    "その人を褒め称える！協調行動を行った相手への《好意》が「忠誠」の場合、協調行動による達成値の上昇が、《好意》の2倍になる。もし、その《好意》が「忠誠」以外なら、相手の美点を讃えるロールプレイを行うと、その属性を「忠誠」に変更し、協調行動による達成値の上昇を《好意》の2倍にできる。",
    "あなたは人を応援するだけでなく、ついつい自分の自慢話をしてしまう。相手の声援にかこつけて自慢をすると、次の自分が行う行為判定の達成値を1点上昇する。",
    "大切な約束を交わす。協調行動を行った相手と大切な約束を交わすと、自分と強調行動を受けたキャラクターは、《HP》を2D6点回復するか、好きな変調1つを回復できる。ただし、約束を破ると、その2人は「散漫1」の変調を受ける。",
    "あなたの声援は、部屋中にこだまし、相手を奮い立たせた。相手のやる気が出そうなロールプレイを行うと、協調行動を受けたキャラクターは《気力》を［あなたのそのキャラクターに対する《好意》］点回復する。",
    "あなたは、民を鼓舞するような言葉をかける。相手と相手の《配下》も勇気づけることができそうなロールプレイを行うと、《民の声》を［協調行動を受けたキャラクターの《配下》×1/5］点回復する。",
];

/// Ruby `超協調行動表`（`2D6`）。
static MKB_TABLE_ECBT: Table = Table::from_dice("超協調行動表", 2, 6, MKB_TABLE_ECBT_ITEMS);

/// Ruby `通貨外見表`の項目。
static MKB_TABLE_CUAT_ITEMS: &[&str] = &[
    "歴代ランドメイカーの肖像が描かれた",
    "中央に穴のあいた",
    "魔法的呪紋の刻まれた",
    "ピカピカに光っている",
    "王国名のすかしが入っている",
    "その国のランドマークが描かれた",
];

/// Ruby `通貨外見表`（`1D6`）。
static MKB_TABLE_CUAT: Table = Table::from_dice("通貨外見表", 1, 6, MKB_TABLE_CUAT_ITEMS);

/// Ruby `通貨名称表`の項目。
static MKB_TABLE_CUNT_ITEMS: &[&str] = &[
    "国王の名前の－部をもじった名前の",
    "王国の名前の一部をもじった名前の",
    "材質の名前の一部をもじった名前の",
    "王国の名前に実在の通貨の名称を足した名前の",
    "王国名のイニシャルをアルファベットにした名前の",
    "GMの名前の一部をもじった名前の",
];

/// Ruby `通貨名称表`（`1D6`）。
static MKB_TABLE_CUNT: Table = Table::from_dice("通貨名称表", 1, 6, MKB_TABLE_CUNT_ITEMS);

/// Ruby `天候表`の項目。
static MKB_TABLE_WEAT_ITEMS: &[&str] = &[
    "ひでり。星の強い輝きがじりじりと照りつける。その部屋で1サイクルを過ごすたび、【甲冑】を装備しているキャラクターは《HP》を1点減少する。",
    "大雨。この部屋で1サイクルを過ごすたび、濡れていない【衣装】を持っているキャラクターは〔探索〕で難易度9の判定を行う。失敗すると、そのセッションの間、その【衣装】は濡れて効果が失われる。",
    "小雨。天井からぴちょんぴちょんと雨だれが。その部屋で戦闘になった場合、GMはランダムに選んだエリア1つに【水たまり】（基本p161）の戦闘トラップを配置する。",
    "そよ風。心地よい風が吹く。宮廷全員は《気力》1点を獲得する。",
    "快晴。その部屋では、【星の欠片】の光量が1上昇する。",
    "よい天気。特に何もなし",
    "快晴。その部屋では、【星の欠片】の光量が1上昇する。",
    "そよ風。心地よい風が吹く。宮廷全員は《気力》1点を獲得する。",
    "霧が深い。その部屋では、すべてのキャラクターは、攻撃を行うとき、射程が1低いものとして扱う（0未満にはならない）。",
    "雪。雪がしんしんと降り積もっている。この部屋では捜索の難易度が1点上昇する。",
    "雷。天使の怒りか？時々天井から雷撃がッ！その部屋で1サイクルを過ごすたび、ランダムにキャラクター1体を選ぶ。そのキャラクターの《HP》を1D6点減少する。",
];

/// Ruby `天候表`（`2D6`）。
static MKB_TABLE_WEAT: Table = Table::from_dice("天候表", 2, 6, MKB_TABLE_WEAT_ITEMS);

/// Ruby `毒の追加効果表`の項目。
static MKB_TABLE_PAET_ITEMS: &[&str] = &[
    "老化毒。この「毒」の効果を受けたキャラクターは、この「毒」の効果で《HP》を減少するたび、年齢が1歳増加していく。この効果によって年齢が100歳以上になったキャラクターは死亡する。",
    "変心毒。この「毒」の効果を受けたキャラクターは、この「毒」の効果で《HP》が減少するたび、その場にいるキャラクターの中からランダムに1人を選び、その人物に対するランダムに選んだ《感情値》を1点上昇する。",
    "劇毒。この「毒」の効果を受けたキャラクターは、「毒」を受けたときに〔探索）で難易度［9+そのモンスターのレベル×1/5］の判定を行う。失敗すると《HP》を1にする。",
    "過剰免疫反応誘発毒。最初にこの「毒」を受けても特に追加効果はない。しかし、同じクォーター中に、もう一度過剰免疫反応誘発毒の効果を受けると、《HP》を0点にして「毒」の変調を回復する。",
    "病毒。この「毒」の効果で《HP》が減少するとき、《HP》の最大値も一緒に減少する。《HP》の最大値の減少は、その「毒」の変調が回復すると無効化される。",
    "溶血毒。この「毒」の効果で《HP》を減少するとき、追加で1D6点減少する。",
    "遅効性毒。この「毒」の効果を受けたキャラクターは、1D6サイクルが経過した後、まだ「毒」を受けていたら、《HP》を0点にして「毒」の変調を回復する。",
    "壊死毒。この「毒」は休憩を行っても回復しない。",
    "屍毒。この「毒」の効果を受けたキャラクターは、「毒」を受けたときに〔探索〕で難易度［9+そのモンスターのレベル×1/5］の判定を行う。失敗すると、ランダムに能力値1つを選び、その「毒」を回復するまでの間、その能力値を1点減少する。この効果によって能力値がマイナスの値をとったキャラクターは、死亡し、死霊のNPCとなる（ 【復活薬】や【聖杯】で死亡を無効化できない）。",
    "抗魔毒。この「毒」の効果を受けたキャラクターは、その「毒」が回復するまでの間、あらゆる神官のスキルと星術・召喚スキルの目標にならない。",
    "異形毒。この効果を受けたキャラクターは、「毒」を受けたときに〔探索〕で難易度［9+そのモンスターのレベル×1/5］の判定を行う。失敗すると、「人体迷宮化表」(p49）でランダムに変異を1つ選んで、それを修得する。",
];

/// Ruby `毒の追加効果表`（`2D6`）。
static MKB_TABLE_PAET: Table = Table::from_dice("毒の追加効果表", 2, 6, MKB_TABLE_PAET_ITEMS);

/// Ruby `媒体表`の項目。
static MKB_TABLE_MEDT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("導画")),
    (12, TableItem::Text("手紙")),
    (13, TableItem::Text("日記")),
    (14, TableItem::Text("陳情書")),
    (15, TableItem::Text("命令書")),
    (16, TableItem::Text("密書")),
    (22, TableItem::Text("帳簿")),
    (23, TableItem::Text("医療記録")),
    (24, TableItem::Text("手配書")),
    (25, TableItem::Text("新聞")),
    (26, TableItem::Text("風刺画")),
    (33, TableItem::Text("音盤")),
    (34, TableItem::Text("吟遊詩人の歌")),
    (35, TableItem::Text("旅人の噂")),
    (36, TableItem::Text("寿ぎ屋の冗談")),
    (44, TableItem::Text("記録星")),
    (45, TableItem::Text("語り部の物語")),
    (46, TableItem::Text("経典")),
    (55, TableItem::Text("水晶球")),
    (56, TableItem::Text("使い魔の伝令")),
    (66, TableItem::Text("写真")),
];

/// Ruby `媒体表`（D66）。
static MKB_TABLE_MEDT: D66Table = D66Table::new("媒体表", D66SortType::Asc, MKB_TABLE_MEDT_ITEMS);

/// Ruby `反応表`の項目。
static MKB_TABLE_REAT_ITEMS: &[&str] = &[
    "その国宝を一目見た者は、あまりの秀麗さに、思わず顔を赤らめ、恍惚とし、ほうとため息をつくという。",
    "その国宝を一目見た者は、あまりの勇壮さに、思わず立ち上がり、驚異の眼差しを向け、万雷の拍手でほめそやすという。",
    "その国宝を一目見た者は、あまりの神聖さに、思わずひれ伏し、手を合わせ、神への祈りを捧げるという。",
    "その国宝を一目見た者は、あまりの絢燭さに、思わず恥じ入り、爪をかみ、持ち主をうらやむという。",
    "その国宝を一目見た者は、あまりの怪異さに、思わず目を背け、がたがたと震え、涙を流すという。",
    "その国宝を一目見た者は、あまりの滑稽さに、思わず腹をよじり、息も絶え絶えに笑い続け、のたうち回るという。",
];

/// Ruby `反応表`（`1D6`）。
static MKB_TABLE_REAT: Table = Table::from_dice("反応表", 1, 6, MKB_TABLE_REAT_ITEMS);

/// Ruby `遍歴表`の項目。
static MKB_TABLE_HIST_ITEMS: &[&str] = &[
    "旅の途中、配下との絆が深まる。《配下》がいれば、好きな能力値で難易度9の判定を行うことができる。成功すると、自国に無事帰還したとき《配下》の中の1人が逸材となる。《配下》は、「生まれ決定表」（基本p24）でランダムにジョブを選び、そのジョブの逸材になる。失敗すると、《配下》に襄切られ、そのキャラクターは死亡する。",
    "世をすねた迷宮支配者の庵を訪ねる。〔魅力〕で難易度11の判定を行う。成功すると、魔道の秘奥を授かり、そのキャラクターのレベルを1上昇する。失敗すると流れ星に撃たれ、《HP》を1D6点減少する。",
    "戦乱に巻き込まれ、助太刀を頼まれる。〔才覚〕で難易度10の判定を行う。成功すると、救いを求めてきた者たちを勝利に導くことができ、そのキャラクターのレベルを1上昇する。望むなら、無事帰還できる。失敗すると戦渦に巻き込まれ、《HP》を1D6点減少する。",
    "ひょんなことから怪物たちと共に暮らすことになる。〔オ覚〕で難易度9の判定を行う。成功すると、平和な暮らしを行うことができ、《HP》をすべて回復することができる。望むなら、無事帰還できる。失敗すると、不興を買って呪いをかけられ、孤独な探索の間、すべての能力値を1点低いものと扱うことになる。",
    "小鬼の群れに襲われる。〔武勇〕で難易度9の判定を行う。成功すると、コモンアイテムのカテゴリの中から好きなものを1種選ぶ。そのカテゴリの中からランダムにアイテム1種を選ぶ。2レベルのそのアイテム1個を獲得する。失敗すると《HP》を1D6点減少する。",
    "とりたてて何もなく一年がたった。年齢を1歳上昇すること。望むなら、旅に飽きて無事帰還できる。",
    "伝説のお宝が眠る大迷宮に挑戦する。周辺階域の中からランダムに未知の土地1つを選び、そこに「地名決定表」（基本p144）でランダムに決めた地名を記入する。〔探索〕で難易度9の判定を行う。成功すると、迷宮に眠るお宝を手に入れ、レア一般アイテムかレア武具アイテムの中から、ランダムに1つを選び、それを1個獲得できる。失敗すると、トラップに巻き込まれ、《HP》と《配下》を1D6点減少する。",
    "別の王国を訪れる。周辺階域の中からランダムに未知の土地1つを選び、そこに「王国名決定表」（基本p14）でランダムに決めた国名の王国を記入する。自国との関係は中立となる。〔魅力〕で難易度9の判定を行う。成功すると、《配下》を1D6人回復する。望むなら自分の国まで送ってもらえ、無事帰還できる。失敗すると、その国のランドメイカーと諍いを起こし、その国との関係が一段階悪化する。",
    "列強の英雄と出会い、共に冒険を続ける。〔探索〕で難易度10の判定を行う。成功すると、共に列強にまつわる巨大な陰謀を解き明かし、そのキャラクターのレベルを1上昇する。望むなら、無事帰還できる。失敗すると魔の手にかかり、《HP》を1D6点減少する。",
    "怪物と戦いにあけくれる。〔武勇〕で難易度11の判定を行う。成功すると、何かに開眼し、そのキャラクターのレベルを1上昇する。失敗すると《HP》を1D6点減少する。",
    "探索の旅を振り返り、その結果に満足する。無事帰還できる。",
];

/// Ruby `遍歴表`（`2D6`）。
static MKB_TABLE_HIST: Table = Table::from_dice("遍歴表", 2, 6, MKB_TABLE_HIST_ITEMS);

/// Ruby `民の反応表`の項目。
static MKB_TABLE_PRET_ITEMS: &[&str] = &[
    "配下たちは、家族のことを思い出す。そして、部隊全体がにわかに活気づいた。",
    "配下たちは軽口を叩きながら、不敵な笑みを浮かべてランドメイカーの不安を吹き飛ばす。",
    "配下たちは跪き、ランドメイカーの命令を一言一句聞きのがさないようにしている。",
    "配下たちは、ランドメイカーを讃える歌を歌い出す。迷宮に荘厳な歌が響き渡る。",
    "配下は歓声をあげ、手に持ったなにかを思い思いに投げつける。辺り一面に服や武器が飛び交う。",
    "「あ、あの技は伝説のッ!?」「知っているのか!?」配下たちがランドメイカーの行った行動を、それはご大層な奥義であると解説し出す。",
    "ありったけの太鼓や金物が打ち鳴らされ、鼓膜をつんざく。配下たちは拳をつきあげ、ランドメイカーの名前を連呼する。",
    "配下たちは、ランドメイカーの背中に熱い視線を注ぐ。その命令をいまかいまかと待ちわび、一心に待機する。",
    "配下たちは一心不乱に突撃する。怒号が渦を巻く。まるで、迷宮の床は沸騰したかのようだった。",
    "「出過ぎたことを申しました。お許しください」配下の一人が、良案を奏上した。",
    "配下たちは、不安で心が張り裂けそうになり、部隊に混乱が生じる。その混乱が、一瞬の隙をつくった！",
];

/// Ruby `民の反応表`（`2D6`）。
static MKB_TABLE_PRET: Table = Table::from_dice("民の反応表", 2, 6, MKB_TABLE_PRET_ITEMS);

/// Ruby `迷宮化現象表`の項目。
static MKB_TABLE_LAPT_ITEMS: &[&str] = &[
    "あなたの心が、迷宮化を始める。その場にいる、あなたに《敵意》を持っているキャラクター全員は「人体迷宮化表」を1回使用し、その変異を獲得する。",
    "カチリという音がする。どこかで【リセットスイッチ】（基本p157）が発動し、そのセッション中に倒したモンスターや解除したトラップが全て復活する。",
    "時の流れが加速する。1D6クォーターが経過する。",
    "迷宮嵐が起こる！GMは、プレイヤーに見えないようにサイコロを振り、そのマップの中にある移動可能な部屋の中からランダムに部屋を一つ選ぶ。その部屋と現在宮廷のいる部屋の位置を入れ替える（通路は移動しない）。",
    "迷宮の意志があなたたちに注意を向ける。その部屋に【毒の霧】（基本p156）と【禍粉】（基本p157）のトラップが配置される。",
    "迷宮化により、壁や天井が崩れ、周囲の風景が一変する。「迷宮風景表」（基本p145）で、その風景を決定する。そして、宮廷全員は《HP》を1D6点減少する。",
    "残留思念が活性化する。その部屋に【大混雑】（基本p155）のトラップが配置される。",
    "突如部屋が暗くなる。その部屋にレベル3の【隠蔽工作】（基本p152）のトラップが配置される。",
    "あなたの装備しているアイテムが、迷宮化を始める。自分が装備しているアイテムの中からランダムに1種類を選ぶ。宮廷内にあるそのアイテムが全て破壊される。",
    "異階へのトビラが開く。宮廷の平均レベルが1レベル高いものとして「特殊遭遇表」（基本p67）を使用する。戦闘中の場合、その遭遇に書かれたモンスター1体が、敵軍に参加する。",
    "自分の身体が、迷宮化を始める。「人体迷宮化表」を1回使用し、その変異を獲得する。",
];

/// Ruby `迷宮化現象表`（`2D6`）。
static MKB_TABLE_LAPT: Table = Table::from_dice("迷宮化現象表", 2, 6, MKB_TABLE_LAPT_ITEMS);

/// Ruby `由来表`の項目。
static MKB_TABLE_ORIT_ITEMS: &[&str] = &[
    "伝統的。その国宝は、百万迷宮開闢以前より伝わる古き遺物（もしくはその技術の伝承者）である。",
    "神話的。その国宝は、神々より授かった神器（もしくは神に認められた者）である。",
    "建国的。その国宝は、王国創生の時から伝わる由緒正しい逸品（もしくは王国の伝統技術の伝承者）である。",
    "秘宝的。その国宝は、王国の中でもその存在を秘され、名前のみ伝わる秘宝（もしくは人材）である。",
    "最新鋭。その国宝は、王国の最新技術によって造られた至宝（もしくはその技術者）である。",
    "勝利の証｡その国宝は、ある国に勝利し、入手した何か（もしくはその人物の子孫）である。",
];

/// Ruby `由来表`（`1D6`）。
static MKB_TABLE_ORIT: Table = Table::from_dice("由来表", 1, 6, MKB_TABLE_ORIT_ITEMS);

/// Ruby `誘致表`の項目。
static MKB_TABLE_ATRT_ITEMS: &[&str] = &[
    "珍しいお客がやってきた。1D6を振る。1なら【ドワーフ】と【黒ドワーフ】（基本p174）の直線主義者、2なら周遊する【星霊】（基本p175）、3なら異世界からやってきた【稀人】（基本p198）、4ならPCたちの先祖の【首無し騎士】（基本p189）、5なら天階からやってきた【取立人】（基本p201）、6なら深階からやってきた【兵隊エルフ】と【闇エルフ】（基本p205）。〔文化レベル〕で難易度9の判定に成功すると、やってきたモンスターすべてが1体ずつモンスターの《民》になる。失敗すると、そのモンスターのレベルの合計値分だけ、王国に残した《民》を減少する。",
    "旅芸人の一座がやってくる。一座を受け入れる場合、宮廷全員は《気力》を1点獲得し、《民の声》を1点回復し、王国に残した《民》を1人減少する。一座を受け入れない場合、《民の声》を1点減少する。",
    "国境周辺に難民のキャンプができた。難民を受け入れる場合、《民の声》を1D6点減少し、《民》を3D6人増える。受け入れない場合、そのセッションの間、〔冶安レベル〕を2点減少する（1未満にはならない）。",
    "旅の商人がやってきた。何やら売りたいものがあるようだが……。〔生活レベル〕で難易度9の判定を行う。成功すると、掘り出しものを売ってくれる。「レア一般アイテム表」（基本p106か上級p66）か、「レア武具アイテム表」（基本p109か上級p66）のいずれかを使用してランダムにレアアイテム1個を選ぶ。そのアイテムを［（　）内の価格］MGで購入できる。また、ランダムにつくった特性の数が1個のデヴァイス（上級p38）を1MGで購入できる。",
    "旅の芸術家がやってきた。新作のモチーフを探しているようだが……。〔文化レベル〕で難易度9の判定を行う。成功すると、芸術家はこの国のよいところを褒め称え、民たちはそれを誇りに思う。《民の声》を1D6点上昇する。",
    "どこかの国の使節がやってくる。周辺階域の中から、ランダムに未知の土地1つを選ぶ。使節は、その土地からやってきた。王国名をランダムに決定する。そして、〔文化レベル〕で難易度9の判定に成功すれば、その国との関係を友好にする。失敗すると、その国との関係を険悪にする。",
    "多くの旅人があなたの国を訪れ、そして去っていく。〔治安レベル〕で難易度9の判定を行う。成功すると、そのうちの何人かが、王国に居着く。〔冶安レベル〕と王国にある【宿屋】（基本p129）と娯楽施設のレベルを合計する。その合計値と等しい数だけ《民》を上昇する。",
    "他国の軍隊がやってきた。〔軍事レベル〕で難易度9の判定を行う。成功すると、金を払うので休ませてほしいと頼んでくる。1D6MGを獲得する。失敗すると、彼らは略奪を始める。《民》を2D6人減少し、《民の声》を1D6点減少する。",
    "諸国を放浪する冒険者がやってきた。〔治安レベル〕で難易度9の判定を行う。成功すると、伝説のお宝の噂話を教えてくれる。「レア一般アイテム表」（基本p106か上級p66）か、「レア武具アイテム表」（基本p109か上級p66）のいずれかを使用してランダムにレアアイテム1個を選ぶ。そのセッションで支配者を倒すと、そのレアアイテム1個を獲得できる。判定に失敗すると、彼らは盗賊と化す。お客さんを呼んだキャラクターは、自分が装備しているアイテムの中からランダムに1個を選び、それを失う。",
    "宮廷の身内を名乗る人物が現れた。〔生活レベル〕で難易度9の判定を行う。成功すると「生まれ決定表」（基本p24）を使って、ランダムにジョブを1つ決める。そのジョブの逸材になる。失敗すると、そのセッションの維持費を2MG上昇する。",
    "みすぼらしい老人がやってきた。彼に優しく接する場合1D6を振る。その正体は、1なら【貧乏神】（基本p175）で、そのセッションの維持費を1D6MG上昇する。2なら余所の国の王様で、周辺階域の中に王国1つをつくり、その国との関係を同盟にする。3ならただのおじいさんで、《民》を1人上昇する。4なら迷宮病患者で、宮廷全員はランダムに選んだ変調1つを受ける。5なら英雄で、上級ジョブの中からランダムに1つを選び、その上級ジョブの逸材になる。6なら災厄王で、自国の施設の中からランダムに選んだ1軒を破壊し、【災厄王の遺物】（基本p108）を1個獲得する。",
];

/// Ruby `誘致表`（`2D6`）。
static MKB_TABLE_ATRT: Table = Table::from_dice("誘致表", 2, 6, MKB_TABLE_ATRT_ITEMS);

/// Ruby `妖精の悪戯表`の項目。
static MKB_TABLE_FAMT_ITEMS: &[&str] = &[
    "妖精は、宮廷の面々が眠っている間に手仕事を開始する。宮廷の中からランダムにキャラクター一人を選び、そのキャラクターの装備している好きなアイテムを一つ選ぶ。そのアイテムのレベルを1点上昇する。",
    "妖精はアベコベこっこをしたいと言い出す。「今度はあなたが家来になる番だ！」妖精カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ）。そして、宮廷の中からランダムに1人を選ぶ。その2人の身体が入れ替わる。そのキャラクターは、1D6クォーターの間、選ばれた妖精のデータを使ってセッションに参加すること。",
    "妖精は、あなたの秘めた想いを、その想い人に告げてしまう。「妖精の悪戯表」を使用したPCが持っている「愛情」の《好意》の中で、最も高い値を持っているキャラクターの中から1人を選ぶ。そのキャラクターは、「妖精の悪戯表」を使ったPCに対する《好意》を好きな値に設定し、好きな属性に変化させることができる。もし、「妖精の悪戯表」を使用したPCが、誰に対しても「愛情」の《好意》を持っていなければ、そのPCは妖精に「愛する者を持たないなんて、つまんないヤツ」と罵倒され、その妖精に対する《敵意》を1点上昇する。",
    "妖精は、そろそろ新しい恋人が欲しいと騒ぎ出す。妖精カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ）。宮廷の中から、誰か1人がその妖精の恋人に立候補することができる。立候補したキャラクターは〔魅力〕で難易度［5+その妖精の〔魔力〕］の判定を行う。成功すれば、妖精は満足し、お互い相手に対する《好意》を4点にする。もし「契約者」（上級p42）の選択ルールを使用している場合、その妖精は、立候補したキャラクターに契約を申し出る。誰も立候補しなかったり、〔魅力〕の判定に失敗した場合、妖精は怒って、宮廷の中からランダムに1人を選んで「呪い4」の変調を与える。",
    "妖精が【羽妖精】、【ドワーフ】、【小人さん】（基本p174）など、友達の妖精を大勢連れてきてどんちゃん騒ぎ！この騒ぎに宮廷一行も巻き込まれてしまう。宮廷全員は《気力》を1D6点回復する。そして、宮廷全員は〔武勇〕で難易度9の判定を行う。失敗したキャラクターは宿酔いに苦しみ、「毒1」の変調を受ける。",
    "バグパイプの素敵な演奏が始まり、妖精たちは曲に合わせて飛んだり、はね回ったりしだす。その陽気な旋律に誘われて、怪物たちが集まってきた！宮廷全員は〔魅力〕で難易度［7+宮廷の平均レベル］の判定を行う。成功したキャラクターは、怪物と素敵なダンスを踊ることができる。贈り物として回復アイテムの中からランダムに選んだ1つを獲得する。失敗したキャラクターは、怪物の怒りを買い、2D6点のダメージを受ける。",
    "妖精はお腹が空いたと騒ぎ出す。【お弁当】や【フルコース】を使用することができる。使用した場合、妖精たちは分け前にあずかり機嫌をよくする。使用したPCは、そのセッションの間、「妖精の悪戯」表を使用したとき、その2D6を1回まで振り直すことができる。もし「まじない」（上級p21）の選択ルールを使用している場合、使用したPCは「遺失魔法まじない」の中からランダムに1つを修得できる。【お弁当】や【フルコース】を使用しなければ、妖精は怒り出す。宮廷の中からランダムにキャラクター1人を選んだ後、そのキャラクターが装備しているアイテムの中からランダムに1つを選び、そのアイテムは無くなる。",
    "妖精が【性別逆転】のまじないをかける！宮廷の中からランダムにキャラクター1人を選ぶ。1D6クォーターの間、そのキャラクターの性別が逆転する。宮廷のほかのキャラクターは1D6を振り、奇数なら性別が逆転したキャラクターに対する《好意》を1点、偶数ならそのキャラクターに対する《敵意》を1点上昇する。",
    "取り替え子。王国の《民》がいつのまにか、妖精の子どもと入れ替わる。1D6を振る。1か2なら【羽妖精】（基本p174）、3か4なら【グレムリン】（基本p174）、5か6なら【 取り替え子】（基本p174）の《モンスターの民》が1人増える。その後、王国の《民》を1人減少する。",
    "「こっち、こっち。こっちだよー」妖精は、宮廷一行を不思議な場所に案内する。そこは、妖精の王国だった。【妖精騎士】や【運命の三女神】（基本p175）の歓待を受け、美しく充実した時間を過ごす。宮廷全員は、好きなキャラクターに対する《好意》を1点獲得し、《HP》と《気力》を最大値まで回復する。そして、1D6クォーターの時間が経過する。",
    "「今までにホントにお世話になりました」妖精は、贈り物を置いて、あなたがたの王国を去る。妖精カテゴリの《モンスターの民》の中からランダムに1体を選ぶ（冒険フェイズの場合、《配下》にいる《モンスターの民》から選ぶ）。そのモンスターを失う。そして、価格の（　）内の数字が、そのモンスターのレベル未満のレアアイテムを1個獲得する。",
];

/// Ruby `妖精の悪戯表`（`2D6`）。
static MKB_TABLE_FAMT: Table = Table::from_dice("妖精の悪戯表", 2, 6, MKB_TABLE_FAMT_ITEMS);

/// Ruby `四字熟語系二つ名表`の項目。
static MKB_TABLE_FCNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("人類最強（の）")),
    (12, TableItem::Text("天下無双（の）")),
    (13, TableItem::Text("電光石火（の）")),
    (14, TableItem::Text("冷静沈着（の）")),
    (15, TableItem::Text("星河一天（の）")),
    (16, TableItem::Text("幽玄美麗（の）")),
    (22, TableItem::Text("義理人情（の）")),
    (23, TableItem::Text("堅忍不抜（の）")),
    (24, TableItem::Text("忠勇義烈（の）")),
    (25, TableItem::Text("風林火山（の）")),
    (26, TableItem::Text("破顔一笑（の）")),
    (33, TableItem::Text("破顔一笑（の）")),
    (34, TableItem::Text("一騎当千（の）")),
    (35, TableItem::Text("阿鼻叫喚（の）")),
    (36, TableItem::Text("一蓮托生（の）")),
    (44, TableItem::Text("一期一会（の）")),
    (45, TableItem::Text("天涯孤独（の）")),
    (46, TableItem::Text("鬼哭啾々（の）")),
    (55, TableItem::Text("一宿一飯（の）")),
    (56, TableItem::Text("軽挙妄動（の）")),
    (66, TableItem::Text("贅沢三昧（の）")),
];

/// Ruby `四字熟語系二つ名表`（D66）。
static MKB_TABLE_FCNT: D66Table =
    D66Table::new("四字熟語系二つ名表", D66SortType::Asc, MKB_TABLE_FCNT_ITEMS);

/// Ruby `代入命名表`の項目。
static MKB_TABLE_ASNT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("［好きなもの／嫌いなもの］［呼び名表］"),
    ),
    (12, TableItem::Text("［行動表］が取り柄の")),
    (13, TableItem::Text("宮廷に［行動表］をうながす")),
    (14, TableItem::Text("二度と［行動表］しない")),
    (15, TableItem::Text("ベテラン［行動表］請負人")),
    (16, TableItem::Text("絶対［行動表］マン／ウーマン")),
    (22, TableItem::Text("［好きなもの／嫌いなもの］警察")),
    (23, TableItem::Text("［好きなもの／嫌いなもの］と生きる")),
    (
        24,
        TableItem::Text("［呼び名表］の中の［同じ単語の繰り返し］"),
    ),
    (25, TableItem::Text("［用語表］を呼ぶ［呼び名表／称号表］")),
    (26, TableItem::Text("王国［行動表］部部長")),
    (33, TableItem::Text("［用語表／行動表］省幹部")),
    (34, TableItem::Text("迷宮三大［呼び名表］の一人")),
    (35, TableItem::Text("猛き［用語表］の勇者")),
    (36, TableItem::Text("一級［行動表］資格所持者")),
    (44, TableItem::Text("［形容表］［行動表］の使い手")),
    (45, TableItem::Text("過激な［用語表／行動表］主義者")),
    (
        46,
        TableItem::Text("［行動表／好きなもの／嫌いなもの］界の大物"),
    ),
    (55, TableItem::Text("［形容表］［PCの名前］構文")),
    (56, TableItem::Text("特殊［行動表］技能士")),
    (66, TableItem::Text("［用語表］を［行動表］する影")),
];

/// Ruby `代入命名表`（D66）。
static MKB_TABLE_ASNT: D66Table =
    D66Table::new("代入命名表", D66SortType::Asc, MKB_TABLE_ASNT_ITEMS);

/// Ruby `名文句系二つ名表`の項目。
static MKB_TABLE_FPNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("またお前か（の）")),
    (12, TableItem::Text("もう死んでいる")),
    (13, TableItem::Text("訴訟も辞さない")),
    (14, TableItem::Text("ですよねー（の）")),
    (15, TableItem::Text("そんなに悪くなかった")),
    (16, TableItem::Text("お値段以上（の）")),
    (22, TableItem::Text("ファイト一発（の）")),
    (23, TableItem::Text("それでもやってない")),
    (24, TableItem::Text("テストに出る")),
    (25, TableItem::Text("まるで成長していない")),
    (26, TableItem::Text("幸せならOKの")),
    (33, TableItem::Text("希望を乗せて走る")),
    (34, TableItem::Text("誰もお前を愛さない")),
    (35, TableItem::Text("すぐおいしい")),
    (36, TableItem::Text("毎日が新しい")),
    (44, TableItem::Text("本気になった")),
    (45, TableItem::Text("あなたとコンビに")),
    (46, TableItem::Text("50MG欲しい")),
    (55, TableItem::Text("またしても何も知らない")),
    (56, TableItem::Text("マジでこれの")),
    (66, TableItem::Text("めっちゃ厳つい")),
];

/// Ruby `名文句系二つ名表`（D66）。
static MKB_TABLE_FPNT: D66Table =
    D66Table::new("名文句系二つ名表", D66SortType::Asc, MKB_TABLE_FPNT_ITEMS);

/// Ruby `サブキャラクター反応表`の項目。
static MKB_TABLE_CSRT_ITEMS: &[&str] = &[
    "忠誠っぼい反応。サブキャラクターは、メインキャラクターの言動に対して疑問を抱くことが少なく、無批判にその指示に従ってしまうことが多い。",
    "友情っぽい反応。サブキャラクターは、メインキャラクターの言動に対して時には真剣に、時にはふざけて反応する。",
    "愛情っぽい反応。サブキャラクターは、メインキャラクターの言動に対して理解を示し、優しく、フォローするように行動することが多い。",
    "怒りっぽい反応。サブキャラクターは、メインキャラクターの言動に対して、反発したり怒りを露わにすることが多い。ただし、内心では、憎からず想っているようである。",
    "不信っぽい反応。サブキャラクターは、メインキャラクターの言動に対してほとんど口を開かず、その視線も冷ややかである。その内心は謎に包まれている。",
    "侮蔑っぽい反応。サブキャラクターは、メインキャラクターの言動に対して、異議や疑問を唱えることが多い。ただし、内心では、その実力を評価している。",
];

/// Ruby `サブキャラクター反応表`（`1D6`）。
static MKB_TABLE_CSRT: Table =
    Table::from_dice("サブキャラクター反応表", 1, 6, MKB_TABLE_CSRT_ITEMS);

/// Ruby `スキルグループ表`の項目。
static MKB_TABLE_KSGT_ITEMS: &[&str] = &[
    "迷宮",
    "召喚",
    "星術",
    "科学",
    "道具",
    "便利",
    "芸能",
    "交渉",
    "射撃",
    "肉弾",
    "自由に1種を選ぶ",
];

/// Ruby `スキルグループ表`（`2D6`）。
static MKB_TABLE_KSGT: Table = Table::from_dice("スキルグループ表", 2, 6, MKB_TABLE_KSGT_ITEMS);

/// Ruby `ダイナマイト帝国休憩表`の項目。
static MKB_TABLE_DRET_ITEMS: &[&str] = &[
    "念入りに装備の手入れをする。持っている中から武具アイテムを1つ選び、そのアイテムのレベルを1上げる。",
    "なんとなく座ってみた箱の中から水音が。開けてみると軍隊用の酒樽だ。宮廷全員の《気力》を1点回復する。",
    "遠くから響いてくる爆発音を聞いていると血が騒ぐ。何かを壊したくてたまらない。《気力》を1点回復する。",
    "帝国に滅ぼされた王家の残党と遺遇する。彼らを民として受け入れるのであれば、宮廷は《配下》に1D6人割り振ることができる。",
    "あいつには護身術が必要だろうから教えておかないと。〔武勇〕で難易度9の判定を行う。成功すると、宮廷内の好きなキャラクター1体を選び、自分に対する《好意》を2点上昇する。",
    "上から人が降ってきて、目の前で床に激突して死ぬ。死体からランダムに選んだ武具アイテムか探索アイテムを1個獲得できる。",
    "何気なく扉を開けた部屋が弾薬庫だった。あわてて火の気を払ってから、【爆弾】もしくは「火薬」の素材のいずれかを1D6個手に入れる。",
    "好戦的な階賊の一群と遭遇する。宮廷全員は〔武勇〕で難易度9の判定を行う。1人判定に成功するたびに、《民の声》を1点回復する。判定に失敗したキャラクターは《HP》を2D6点減少する。",
    "大帝の不興をかって追放されたと称する人物が1人現れる。「生まれ決定表」を使ってジョブをランダムに決定すること。逸材として国に迎え入れることも可能だが、帝国のスパイということも考えられる。「豪傑」のルールを使っている場合、1D6を振って奇数なら、その人物は豪傑になる。",
    "迷宮解体工事を進めている職人の一団が通りかかる。報酬次第で、迷宮工事を行ってくれそうだ。名前に「ドワーフ」を含む《特殊配下》を連れているか、《予算》を1MG消費すると、隣接する好きな部屋に通路を1本つなげる。",
    "なにげない雑談から喧嘩になり、やがて武器を抜いた大立ち回りに発展する。奇跡的に怪我はなかったが……。宮廷全員の宮廷全員に対する《敵意》を1点上昇する。",
];

/// Ruby `ダイナマイト帝国休憩表`（`2D6`）。
static MKB_TABLE_DRET: Table =
    Table::from_dice("ダイナマイト帝国休憩表", 2, 6, MKB_TABLE_DRET_ITEMS);

/// Ruby `ダイナマイト帝国特殊遭遇表`の項目。
static MKB_TABLE_DSET_ITEMS: &[&str] = &[
    "あたりに満ちる暴力の気配に身がすくんでしまう。各PCは【お酒】を1個使用できる。使用しなかったPCはそのセッションの間、《HP》最大値を5点減少する。",
    "大帝の行おうとしている工事の規模と、帝国の民が共有している夢の大きさに圧倒される。宮廷全員は「散漫1」の変調を受ける。",
    "ふと空しさを覚える。愛や憎しみにどれだけの価値があるというのか。迷宮を破壊し、モンスターを殺すだけの機械になってしまいたい。宮廷全員は、《感情値》を合計2点減少する。",
    "C-4独立混成兵団による援軍！GMはモンスターがいる好きな部屋を1つ選ぶ。以降GMは、その部屋で戦闘が発生したとき【牛頭】を1D6体追加で配置することができる。",
    "無様な失敗に失望した配下が新天地を目指して出発してしまう。引き止めることはできない。宮廷全員は《配下》を1D6人減少する。",
    "おびえた配下たちが追加のわけまえを要求する。《予算》を1MG消費する。",
    "どこか遠くで戦っているダイナマイト大帝の剣圧が迷宮を揺るがす。落石だ！宮廷全員は1D6点のダメージを受ける。",
    "足元を掘りぬいて【土竜人】の一団が現れると、周囲を気にする様子もなく弁当を食べ始める。あまりに美味しそうなので相伴したら太った。宮廷全員は「肥満2」の変調を受ける。",
    "「たしかに迷宮なんてなくなったほうがいいかもしれないな」と呟いてしまったのを配下に聞きつけられてしまう。《民の声》を2点減少する。",
    "帝国の使者が現れる。帝国への貢物として、《予算》を2MG消費するか、価格の（　）内の数値が2以上のレアアイテム1個を消費することができる。この要求に応じればダイナマイト帝国との関係が1段階良好になる。断るなら、宮廷の代表は〔武勇〕で難易度11の判定を行う。失敗すると、宮廷全員は2D6点のダメージを受ける。",
    "食べ物の匂いを嗅ぎつけて、無数の餓鬼が集まってくる。宮廷全口が全ての「肉」の素材を差し出す。もしくは宮廷全員は《HP》を2D6点減少する。",
];

/// Ruby `ダイナマイト帝国特殊遭遇表`（`2D6`）。
static MKB_TABLE_DSET: Table =
    Table::from_dice("ダイナマイト帝国特殊遭遇表", 2, 6, MKB_TABLE_DSET_ITEMS);

/// Ruby `ダイナマイト帝国二つ名表`の項目。
static MKB_TABLE_DNNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("猪突猛進（の）")),
    (12, TableItem::Text("（そのPCの好きなもの）は爆発だ")),
    (13, TableItem::Text("武士に二言はない")),
    (14, TableItem::Text("堪忍袋の緒が切れる")),
    (15, TableItem::Text("腹が減っては戦ができぬ")),
    (16, TableItem::Text("ーか八かの")),
    (22, TableItem::Text("一触即発（の）")),
    (23, TableItem::Text("天下分け目の")),
    (24, TableItem::Text("逃げるが勝ちの")),
    (25, TableItem::Text("売り言葉に買い言葉の")),
    (26, TableItem::Text("肉を切らせて骨を断つ")),
    (33, TableItem::Text("豪放磊落（の）")),
    (34, TableItem::Text("鎬を削る")),
    (35, TableItem::Text("小の虫を殺して大の虫を助ける")),
    (36, TableItem::Text("勝てば官軍の")),
    (44, TableItem::Text("一刀両断（の）")),
    (45, TableItem::Text("先鞭をつける")),
    (46, TableItem::Text("英雄色を好む")),
    (55, TableItem::Text("弱肉強食（の）")),
    (56, TableItem::Text("英雄は英雄を知る")),
    (66, TableItem::Text("国士無双（の）")),
];

/// Ruby `ダイナマイト帝国二つ名表`（D66）。
static MKB_TABLE_DNNT: D66Table = D66Table::new(
    "ダイナマイト帝国二つ名表",
    D66SortType::Asc,
    MKB_TABLE_DNNT_ITEMS,
);

/// Ruby `ダイナマイト帝国名前表`の項目。
static MKB_TABLE_DNAT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バルディッシュ／カタナ")),
    (12, TableItem::Text("ブロードソード／ダガー")),
    (13, TableItem::Text("カトラス／シャムシール")),
    (14, TableItem::Text("ファルシオン／ククリ")),
    (15, TableItem::Text("グラディウス／レイピア")),
    (16, TableItem::Text("クレイモア／メイス")),
    (22, TableItem::Text("マインゴーシュ／パタ")),
    (23, TableItem::Text("フレイル／モーニングスター")),
    (24, TableItem::Text("パイク／サリッサ")),
    (25, TableItem::Text("ランス／フランベルジュ")),
    (26, TableItem::Text("ハルバード／コルセスカ")),
    (33, TableItem::Text("バリスタ／ファラリカ")),
    (34, TableItem::Text("ボウ／マカナ")),
    (35, TableItem::Text("ドラグノフ／ベレッタ")),
    (36, TableItem::Text("ナパーム／デイジーカッター")),
    (44, TableItem::Text("ブリガンダイン／キュイラス")),
    (45, TableItem::Text("ホーバーク／ハルニッシュ")),
    (46, TableItem::Text("バシネット／アーメット")),
    (55, TableItem::Text("バレルヘルム／ガントレット")),
    (56, TableItem::Text("ゴルゲット／チシャク")),
    (66, TableItem::Text("チェインメイル／ラメラーアーマー")),
];

/// Ruby `ダイナマイト帝国名前表`（D66）。
static MKB_TABLE_DNAT: D66Table = D66Table::new(
    "ダイナマイト帝国名前表",
    D66SortType::Asc,
    MKB_TABLE_DNAT_ITEMS,
);

/// Ruby `ハグルマ資本主義神聖共和国休憩表`の項目。
static MKB_TABLE_HRET_ITEMS: &[&str] = &[
    "フリーランサーの一団と遭遇する。《予算》を1D6MG消費するたび、彼らを1人雇うことができる。彼ら1人雇うたび、そのセッションの間、好きなキャラクター1人の能力値1つか国力1つを1点上昇する。ただし、複数人雇っても、同じキャラクターの能力面を2点以上伸ばすことはできない。",
    "トイレに立ったつもりが道に迷い、気がつけば暗い部屋で深人に囲まれている。そして気を失い、目を開けたときにはすでに深人はおらず、いつの間にか目と目の間隔が広い人たちに付き従われていた。なぜか怒りがこみあげてくる。「憤怒」の変調を受け、《配下》を2D6人増やす。",
    "ランドメイカーを対象にした、王国運宮についての街頭アンケートに答えて粗品を貰う。【カード】を1個獲得する。",
    "深階に続くエレベーターシャフトヘの横穴を発見する。流れ落ちていく供物を掠め取ることができるかもしれない。〔才覚〕で難易度9の判定を行う。成功すると《予算》を1MG獲得する。失敗すると「毒3」の変調を受ける。",
    "いたるところにある飲み物の自動販売機で買った小さな瓶の栄養剤を飲んでみる。効いているような効いていないような。好きな変調1つを回復し、《気力》を1点回復する。",
    "はやくあいつに追いつかなければ……焦りが生まれる。自分を除く宮廷で最も勲章が多いキャラクターに対する《敵意》を1点上昇する（同値の人物がいる場合、そのキャラクター全員に対する《敵意》を1点上昇する）。",
    "新しいポスターのモデルを探しているハグルマ広報部のスカウトマンと遺遇する。〔魅力〕で難易度9の判定を行う。成功すると採用されて《民の声》を1点回復する。",
    "放棄された武器の量産工場を発見する。武具アイテムの中からランダムに好きなアイテム1つを選ぶ。宮廷全員は、好きなだけその武具アイテムを手に入れることができる。",
    "古くなって壊れたポーション自動販売機の墓場を発見する。好きなだけ【ポーション】を獲得する。ただし、この効果によって獲得した【ポーション】は、使用するときに1D6を振る。奇数が出たら「毒3」の変調を受ける（《HP》は回復しない）。",
    "「ひと味違う大人のお料理はいかがですか？今ならもれなくオマケがプレゼントされますよ」フルコース販促キャンペーン小隊と遭遇する。《予算》を1MG消費すると、【フルコース】1個を獲得し、宮廷全員は【ポーション】、【チョコレート】、【珈琲】のいずれか1個を獲得できる。",
    "著作権代理人が走りこんできて、あなたの書いた冒険記が百万迷宮全域で大ヒットしたという奇跡的なニュースを伝えてくれる。《予算》を1D6MG獲得する。",
];

/// Ruby `ハグルマ資本主義神聖共和国休憩表`（`2D6`）。
static MKB_TABLE_HRET: Table = Table::from_dice(
    "ハグルマ資本主義神聖共和国休憩表",
    2,
    6,
    MKB_TABLE_HRET_ITEMS,
);

/// Ruby `ハグルマ資本主義神聖共和国特殊遭遇表`の項目。
static MKB_TABLE_HSET_ITEMS: &[&str] = &[
    "こんな国じゃ酒でも飲まないとやっていけない。各PCは【お酒】を1個使用できる。使用しなかったPCは、そのセッションの間、好きなスキル1種類を選び、それを喪失する。",
    "「ハグルマでは目端を利かせることだね。儲け話を見逃すんじゃないよ」母親の言葉が脳裏をよぎる。宮廷の代表は「散漫2」の変調を受ける。",
    "装備していたアイテムが突如呪物として目覚め、「俺はゴミだ！」と叫ぶと、【廃奇物】となるべく迷宮の闇へと消えていった。宮廷の代表は、自分の装備スロットをランダムに1つ選び、そのスロットに装備・収納されたものをすべて消費する。",
    "通勤ラッシュだ！突如スーツを着た群衆が現れて、宮廷全員を押し込む。この部屋からのびる通路をランダムに1つ選ぶ。宮廷は、その通路を通って強制的に、隣の部屋へ移動する。",
    "もっと金を稼がなければ国は大きくならない。もっと稼がなければ人は強くならない。ハグルマに感化された自分の中で大切なものが萎縮していく。宮廷全員は最も高い《感情値》を1点減少する。",
    "ハグルマの会計士が王国の台所事情をすっぱぬいて国民に知らせてしまう。《民の声》を1点減少する。",
    "敷石の隙間から青白い触手が足首に巻きつく。宮廷全員は1D6点のダメージを受ける。",
    "ハグルマの情報工作。情報収集によって判明した脅威情報が分からなくなる。GMは、迷宮マップに書かれている脅威情報をすべて消す。新たに情報収集しない限り、その部屋のモンスターの正確な数は判明していないものとする。",
    "資徒の群れが現れ、始祖ハグルマヘの供物として、レアアイテムを1つ要求する。これを断ればハグルマとの関係が1段階悪化する。",
    "【舎利蟹司令】の執務室に迷い込んでしまう。彼は紳士的でお茶などを出して歓迎してくれたが、そのお茶には眠り薬が入っていた。宮廷の代表は〔才覚〕で難易度11の判定を行う。判定に失敗すると、以降GMは、1度だけ好きなタイミングでそのPCに「眠り6」の変調を与えることができる。",
    "突然壁が崩れ、明らかに理性を失っている【人工天使】が現れると、止める間もなくあなたに襲いかかってくる。そして【人工天使】は天に向けて鬨の声をあげると、腐って崩壊する。宮廷の代表は《配下》を2D6人減少する。",
];

/// Ruby `ハグルマ資本主義神聖共和国特殊遭遇表`（`2D6`）。
static MKB_TABLE_HSET: Table = Table::from_dice(
    "ハグルマ資本主義神聖共和国特殊遭遇表",
    2,
    6,
    MKB_TABLE_HSET_ITEMS,
);

/// Ruby `ハグルマ資本主義神聖共和国二つ名表`の項目。
static MKB_TABLE_HNNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("立身出世（の）")),
    (12, TableItem::Text("宵越しの金を持たない")),
    (13, TableItem::Text("ー旗揚げる")),
    (14, TableItem::Text("失敗は成功のもとの")),
    (15, TableItem::Text("立っている者は親でも使う")),
    (16, TableItem::Text("損して得とる")),
    (22, TableItem::Text("千客万来（の）")),
    (23, TableItem::Text("腐っても鯛の")),
    (24, TableItem::Text("嘘も方便の")),
    (25, TableItem::Text("氏より育ちの")),
    (26, TableItem::Text("出血大サービスの")),
    (33, TableItem::Text("薄利多売（の）")),
    (34, TableItem::Text("坊主丸儲けの")),
    (35, TableItem::Text("金持ち喧嘩しない")),
    (36, TableItem::Text("海老で鯛を釣る")),
    (44, TableItem::Text("呉越同舟（の）")),
    (45, TableItem::Text("郷に入っては郷に従う")),
    (46, TableItem::Text("売り切れごめんの")),
    (55, TableItem::Text("社交辞令（の）")),
    (56, TableItem::Text("商人に系図なしの")),
    (66, TableItem::Text("適材適所（の）")),
];

/// Ruby `ハグルマ資本主義神聖共和国二つ名表`（D66）。
static MKB_TABLE_HNNT: D66Table = D66Table::new(
    "ハグルマ資本主義神聖共和国二つ名表",
    D66SortType::Asc,
    MKB_TABLE_HNNT_ITEMS,
);

/// Ruby `ハグルマ資本主義神聖共和国名前表`の項目。
static MKB_TABLE_HNAT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("開拓／教義部")),
    (12, TableItem::Text("威力／人事部")),
    (13, TableItem::Text("信仰／総務部")),
    (14, TableItem::Text("特殊／経理部")),
    (15, TableItem::Text("迷宮／営業部")),
    (16, TableItem::Text("戦略／開発部")),
    (22, TableItem::Text("外交／布教本部")),
    (23, TableItem::Text("食品／製造部")),
    (24, TableItem::Text("コモンアイテム／業務部")),
    (25, TableItem::Text("深階／企画部")),
    (26, TableItem::Text("対天階／システム部")),
    (33, TableItem::Text("侵略／マーケティング部")),
    (34, TableItem::Text("武装／広報部")),
    (35, TableItem::Text("巡回／設計部")),
    (36, TableItem::Text("最強／法務部")),
    (44, TableItem::Text("妄想／調達部")),
    (45, TableItem::Text("生活／通信部")),
    (46, TableItem::Text("国民／物流部")),
    (55, TableItem::Text("文化／情報部")),
    (56, TableItem::Text("魔法／金融部")),
    (66, TableItem::Text("怪物／監査室")),
];

/// Ruby `ハグルマ資本主義神聖共和国名前表`（D66）。
static MKB_TABLE_HNAT: D66Table = D66Table::new(
    "ハグルマ資本主義神聖共和国名前表",
    D66SortType::Asc,
    MKB_TABLE_HNAT_ITEMS,
);

/// Ruby `メトロ汗国休憩表`の項目。
static MKB_TABLE_MRET_ITEMS: &[&str] = &[
    "焚き火を囲んだ食事に、いつの間にか名のある公主が紛れ込んでいた。彼女は料理を気に入ったようだ。ランダムにレア一般アイテムを1個選ぶ。望むなら、そのレア一般アイテムを、宮廷が持っている好きなレアアイテム1個と交換してくれる。",
    "すぐ先のT字路を天使の群れが横切る。荘厳な光景に心が洗われ、《気力》を2点回復し、《HP》を最大値まで回復する。",
    "引き込み線に入ったまま朽ち果てている車輌を発見する。昔は誰かがここで暮らしていたようだが、今はもう動かないただの廃車だ。望むなら家捜しができる。ランダムに日常アイテムを1個選ぶ。そのアイテムを獲得できる。",
    "この部屋にはウマトカゲやヒツジの皮がたくさんぶら下がっている。列車の座席などを加工する皮革製品工場の跡だろう。〔探索〕で難易度9の判定を行う。成功すれば「革」の素材を1D6個獲得する。",
    "休憩しているすぐ横を列車が轟音を立てながら通り過ぎ、驚いて誰かにぶつかり、そのまま押し倒してしまう。その部屋にいるキャラクターの中からランダムに1人を選ぶ。そのキャラクターと自分の互いに対する《好意》を1点上昇する。",
    "路道の巡礼と遷遇する。巡礼に【お弁当】をあげると巡礼は宮廷を祝福してくれる。宮廷は食事アイテムを好きな数だけ消費できる。食事アイテムを1個消費するたびに、好きなキャラクター1人の《気カ》を1点回復する。",
    "不規則に線路を駆け抜ける列車のタイミングを測ることになる。〔探索〕で難易度11の判定を行う。成功すれば比較的安全なルートを発見して、《民の声》を1点回復する。",
    "乱暴な主人のもとから逃げ出してきた奴隷と遭遇する。彼らを民として受け入れるのであれば、宮廷は《配下》に1D6人割り振ることができるが、元の主人に会うことがあればトラブルになるだろう。",
    "暗い迷宮の中を【夜羽】の群れが飛び交う。この辺りで大勢の人間が亡くなられたのだろう。死者の魂は次に生まれ変わる場所を探しているのかもしれない。宮廷全員は〔魅力〕で難易度9の判定を行う。1人成功するたびに、自国の《民》を1D6人増やす。判定に失敗したキャラクターは《HP》を1D6点減少する。",
    "気の良さそうな線路敷設部隊のトロッコが通りかかる。そのセッションの間、宮廷は、この部屋で素材を1個消費すると、自国に帰還できる。",
    "ぼんやりしていたら列車に轢かれてしまう。〔探索〕で難易度11の判定を行う。成功すると、奇跡的に助かったあなたは神に祝福された人間として民の崇敬を集め、《民の声》を4点回復する。失敗すると《HP》を1点にする。",
];

/// Ruby `メトロ汗国休憩表`（`2D6`）。
static MKB_TABLE_MRET: Table = Table::from_dice("メトロ汗国休憩表", 2, 6, MKB_TABLE_MRET_ITEMS);

/// Ruby `メトロ汗国特殊遭遇表`の項目。
static MKB_TABLE_MSET_ITEMS: &[&str] = &[
    "すごい速さで迷宮を駆け抜けるメトロの民に圧倒され、自分の鈍重さが嫌になる。各PCは【お酒】を1個使用できる。使用しなかったPCは、そのセッションの間、《回避値》が2点減少する。",
    "石だと思って腰を下ろしたものは路線整備用の小型車輌だった。車は勢い良く転がり、粗忽者を振り落とす。宮廷全員は［経過ターン+1］D6点のダメージを受ける。",
    "線路敷設部隊が突然現れて、この先に線路を引くための人柱として配下を1D6人よこせと要求してくる。断ればメトロ汗国との関係が1段階悪化する。",
    "路道の博愛の教えに従い、敵の事情を想像してみてしまう。そのセッションの間、反応が友好・中立の怪物と戦闘を行うと《民の声》を1D6点減少する。",
    "道に迷った小さな女の子を拾った……と思ったら、【二口女】だった。宮廷全員は食事アイテムをすべて消費する。",
    "無数の花束が供えられた急カーブを発見してしまう。花束の上には何か半透明のものがふわふわと浮かんでいる。背筋が凍り付く。宮廷全員は《気力》を1点減少する。",
    "たった今、重大なことに気がついた。この部屋は恐ろしくゆっくり動いていて、やっと移動をやめたところだ。GMは、この部屋をマップの好きな場所に移動させ、シナリオ上設定されている部屋に通路を1本ひく。",
    "勝手に閉まるドアに駆け込んで《配下》を置き去りにしてしまう。幸いあとで合流できたが……。《民の声》を1点減少する。",
    "【麒麟】が現れ、あなたに小さな星の欠片を渡すと去っていく。【麒麟】が見えなくなった瞬間に星の欠片が爆発した。宮廷全員は2D6点のダメージを受ける。",
    "上に座ったり無理に通ろうとして踏み切りを破損してしまう。損害賠償を請求された。《予算》を4MG消費する。",
    "この先は、どうやら列車でしか移動できなさそうだが、今日の終電が行ってしまったらしい。こうなってしまっては朝を待つしかない。次のターンの開始までクォーターが経過する。",
];

/// Ruby `メトロ汗国特殊遭遇表`（`2D6`）。
static MKB_TABLE_MSET: Table = Table::from_dice("メトロ汗国特殊遭遇表", 2, 6, MKB_TABLE_MSET_ITEMS);

/// Ruby `メトロ汗国二つ名表`の項目。
static MKB_TABLE_MNNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("海千山千（の）")),
    (12, TableItem::Text("先んずれば人を制す")),
    (13, TableItem::Text("賽は投げられた")),
    (14, TableItem::Text("白線の内側にいる")),
    (15, TableItem::Text("各駅停車の")),
    (16, TableItem::Text("明日は明日の風が吹く")),
    (22, TableItem::Text("一石二鳥（の）")),
    (23, TableItem::Text("正鵠を射る")),
    (24, TableItem::Text("縁の下の力持ちの")),
    (25, TableItem::Text("仕上げが肝心の")),
    (26, TableItem::Text("当たって砕ける")),
    (33, TableItem::Text("荒唐無稽（の）")),
    (34, TableItem::Text("命あっての物種の")),
    (35, TableItem::Text("尻馬に乗る")),
    (36, TableItem::Text("二の足を踏む")),
    (44, TableItem::Text("神出鬼没（の）")),
    (45, TableItem::Text("なせばなる")),
    (46, TableItem::Text("急がば回る")),
    (55, TableItem::Text("前人未踏（の）")),
    (56, TableItem::Text("寝る子は育つ")),
    (66, TableItem::Text("電光石火（の）")),
];

/// Ruby `メトロ汗国二つ名表`（D66）。
static MKB_TABLE_MNNT: D66Table =
    D66Table::new("メトロ汗国二つ名表", D66SortType::Asc, MKB_TABLE_MNNT_ITEMS);

/// Ruby `メトロ汗国名前表`の項目。
static MKB_TABLE_MNAT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("ヤマノテドラコン／アクアマリン（藍玉）"),
    ),
    (12, TableItem::Text("レッドドラゴン／アンバー（琥珀）")),
    (
        13,
        TableItem::Text("セントラルドラゴン／エメラルド（翠玉）"),
    ),
    (14, TableItem::Text("サクラドラゴン／サファイア（鋼玉）")),
    (15, TableItem::Text("ブックドラゴン／シナバー（辰砂）")),
    (
        16,
        TableItem::Text("スノードラゴン／タイガーアイ（虎目石）"),
    ),
    (22, TableItem::Text("ツインドラゴン／トパーズ（黄玉）")),
    (
        23,
        TableItem::Text("ピンクドラゴン／ブラッドストーン（血石）"),
    ),
    (
        24,
        TableItem::Text("ウォータードラゴン／フローライト（蛍石）"),
    ),
    (
        25,
        TableItem::Text("スタードラゴン／ムーンストーン（月長石）"),
    ),
    (26, TableItem::Text("ロストドラゴン／マラカイト（孔雀石）")),
    (33, TableItem::Text("ユーロドラゴン／ラピスラズリ（瑠璃）")),
    (
        34,
        TableItem::Text("ドランクドラゴン／アマゾナイト（天河石）"),
    ),
    (
        35,
        TableItem::Text("ロウゴクドラゴン／アクシナイト（斧石）"),
    ),
    (
        36,
        TableItem::Text("オリエントドラゴン／アラバスター（石膏）"),
    ),
    (
        44,
        TableItem::Text("ロマンスドラゴン／ガーネット（柘榴石）"),
    ),
    (
        45,
        TableItem::Text("ソウルドラゴン／ダイアモンド（金剛石）"),
    ),
    (
        46,
        TableItem::Text("ストームドラゴン／クリソベリル（金緑石）"),
    ),
    (55, TableItem::Text("アトミックドラゴン／アゲ一卜（瑪瑙）")),
    (56, TableItem::Text("モテモテドラゴン／スピネル（尖晶石）")),
    (66, TableItem::Text("エンシェントドラゴン／ルビー（紅玉）")),
];

/// Ruby `メトロ汗国名前表`（D66）。
static MKB_TABLE_MNAT: D66Table =
    D66Table::new("メトロ汗国名前表", D66SortType::Asc, MKB_TABLE_MNAT_ITEMS);

/// Ruby `千年王朝休憩表`の項目。
static MKB_TABLE_TRET_ITEMS: &[&str] = &[
    "床の模様につまずいて、悪魔を閉じ込めていた太古の封印を破ってしまう。悪魔は配下を捧げれば、力を与えると言ってくる。《配下》を2D6人消費すると、ランダムに選んだレア武具アイテム1個を獲得する。ただし、レア武具アイテムを獲得すると、《民の声》を1点減少する。",
    "王国に留学している学生の一団と出会う。宮廷の持っている素材を10個まで好きな素材と交換してもらうことができる。",
    "恐ろしい怪物と戦っている自分を描いた数百年前の壁画を見つけてしまう。〔魅力〕で難易度9の判定を行う。成功すればその雄々しい姿に、宮廷全員の自分に対する《好意》を1点上昇する。",
    "呪文の書かれた巻物を発見する。「まじない」のルールを使用していれば、そのセッションの間、好きなカテゴリのまじないをランダムに1種修得する。そうでなければ《気力》2点を獲得する。",
    "古い本にまぎれて新しいメモが床に落ちているのを見つける。誰が落としたものだろう？〔才覚〕で難易度9の判定を行う。成功すれば「情報」の素材を1D6個獲得する。",
    "忘れられていた書庫を発見する。埃まみれの本棚から出てきた古書には、あなたの先祖に関する様々な伝説が記されていた。《民の声》が1点回復する。",
    "のんびり寝ていると霊夢を見る。死んだ親族や顔も知らないはずの先祖が語りかけてくる。起きたときには全く内容を覚えていないが、気分だけはとてもすがすがしい。《気力》を1点回復する。",
    "柔らかな優しい星の光が、宮廷一同を照らし出す。宮廷全員は、受けている変調をすべて回復する。",
    "この辺りは、迷宮化の影響がひどい。しかし、この地形、もしかするとうまく利用できるかもしれない……。セッション中1度だけ、戦闘開始時、好きなエリアに好きな戦闘トラッブ1個を配置することができる。",
    "封印が施された扉の前で配下たちが騒いでいる。どうやら、合い言葉が必要なようだ。〔才覚〕で難易度11の判定を行う。成功すれば扉が開き、隣接する好きな部屋に通路を1本つなげる。",
    "この場所は昔行われた魔法実験で汚染されていた！体の一部が、GMの選んだモンスターの体のようになる。そのセッションの間、星術、召喚、科学のスキルを使用するための判定でサイコロを1個余分に振ることができるようになる。",
];

/// Ruby `千年王朝休憩表`（`2D6`）。
static MKB_TABLE_TRET: Table = Table::from_dice("千年王朝休憩表", 2, 6, MKB_TABLE_TRET_ITEMS);

/// Ruby `千年王朝特殊遭遇表`の項目。
static MKB_TABLE_TSET_ITEMS: &[&str] = &[
    "これだけの人が災厄王の帰還を二千年も待ち望んでいる。それと自分を比べてブルーになってしまう。各PCは【お酒】を1個使用できる。使用しなかったPCは、そのセッションの間、《配下》の最大値を2点減少する。",
    "ふと見やった扉の隙間から目がこちらをにらむ。脚から力が抜け、宮廷全員は《HP》を1D6点減少し、宮廷の代表は【星の欠片】を1つ消費する。このままでは睨み殺される、と思った瞬間に目は消えた。あれが【星首】だったのだろうか？",
    "壁画の人物の腕がにゅっと伸び、あなたのアイテムを失敬しようとする。宮廷全員は、装備しているアイテムの中から好きなもの1つを消費する。",
    "博物管理士と名乗る人物が現れ、宮廷は王朝の文化遺産を破損していると主張し、代価として領土1部屋を要求する。領土を渡さなければ、千年王朝との関係が1段階悪化する。",
    "何故か周囲の迷宮化の進行が早い。何か変わったものでもいるのだろうか？GMは迷宮マップの任意の通路に【難所】トラップ1個を配置することができる。",
    "魔物の露店で、前から欲しかったレアだが何の役にも立たない小物を見つけてしまい、欲望に負けて国費を使い込む。《予算》を1MG消費する。",
    "ふとしたはずみで【木乃伊】の頭を踏み潰してしまい、恐ろしい呪いがふりかかる。宮廷全員は、「呪い3」の変調を受ける。",
    "国王と同じ家名を名乗る物乞いが現れたうえ、物乞いの家のほうが本家だと主張し始める。宮廷の国王に対する《敵意》の合計値だけ《民の声》を減少する。",
    "マジックミサイルの罠を踏んでしまう。避けたつもりが当たるまで追いかけてこられる。宮廷の代表は［自分のレベル×1/2］D6点のダメージを受ける。",
    "悪臭雲の罠を踏んでしまう。その部屋に【毒の霧】のトラップを1個配置する。",
    "部屋の片隅を破壊しながら【星首】がかすめてゆく。幸い襲い掛かってくることはなかったが…… 。宮廷が持つ【星の欠片】を1D6個消費する。",
];

/// Ruby `千年王朝特殊遭遇表`（`2D6`）。
static MKB_TABLE_TSET: Table = Table::from_dice("千年王朝特殊遭遇表", 2, 6, MKB_TABLE_TSET_ITEMS);

/// Ruby `千年王朝二つ名表`の項目。
static MKB_TABLE_TNNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("清廉潔白（の）")),
    (12, TableItem::Text("終わりよければすべてよい")),
    (13, TableItem::Text("亀の甲より年の功の")),
    (14, TableItem::Text("迷える子羊の")),
    (15, TableItem::Text("故きを温め新しきを知る")),
    (16, TableItem::Text("芸は身を助ける")),
    (22, TableItem::Text("聖人君子（の）")),
    (23, TableItem::Text("歴史に学ぶ")),
    (24, TableItem::Text("言いたいことを明日言う")),
    (25, TableItem::Text("汝の敵を愛する")),
    (26, TableItem::Text("死中に活を求める")),
    (33, TableItem::Text("知者楽水（の）")),
    (34, TableItem::Text("最後に勝つ")),
    (35, TableItem::Text("災厄転じて福となす")),
    (36, TableItem::Text("情けは人のためならぬ")),
    (44, TableItem::Text("七歩之才（の）")),
    (45, TableItem::Text("ーを聞いて＋を知る")),
    (46, TableItem::Text("能ある鷹は爪をかくす")),
    (55, TableItem::Text("臨機応変（の）")),
    (56, TableItem::Text("言わぬが花の")),
    (66, TableItem::Text("乾坤一擲（の）")),
];

/// Ruby `千年王朝二つ名表`（D66）。
static MKB_TABLE_TNNT: D66Table =
    D66Table::new("千年王朝二つ名表", D66SortType::Asc, MKB_TABLE_TNNT_ITEMS);

/// Ruby `千年王朝名前表`の項目。
static MKB_TABLE_TNAT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヴェガ／カモシカ")),
    (12, TableItem::Text("デネブ／ピグレット")),
    (13, TableItem::Text("アルビレオ／アント")),
    (14, TableItem::Text("スワロキン／コニー")),
    (15, TableItem::Text("ポラリス／ボヴァン")),
    (16, TableItem::Text("リゲル／カリブー")),
    (22, TableItem::Text("アルデバラン／ルー")),
    (23, TableItem::Text("スピカ／ティグレ")),
    (24, TableItem::Text("シャウラ／ベラトリックス")),
    (25, TableItem::Text("サルガス／マシラ")),
    (26, TableItem::Text("アンタレス／フント")),
    (33, TableItem::Text("ベラトリックス／ヒッポ")),
    (34, TableItem::Text("レグルス／メーヴェ")),
    (35, TableItem::Text("ミラ／バロット")),
    (36, TableItem::Text("アルゴル／カイユ")),
    (44, TableItem::Text("サダルバリ／スクァーロ")),
    (45, TableItem::Text("ブロブス／マッカレル")),
    (46, TableItem::Text("ヘカー／ジュゴン")),
    (55, TableItem::Text("タビト／ボビー")),
    (56, TableItem::Text("メイサ／アサガオ")),
    (66, TableItem::Text("ワズン／メイプル")),
];

/// Ruby `千年王朝名前表`（D66）。
static MKB_TABLE_TNAT: D66Table =
    D66Table::new("千年王朝名前表", D66SortType::Asc, MKB_TABLE_TNAT_ITEMS);

/// Ruby `候補生背景表`の項目。
static MKB_TABLE_CABT_ITEMS: &[&str] = &[
    "憧れの人。あなたは自国のランドメイカーに憧れ、その人のようになりたくて候補生を目指した。宮廷の候補生以外のキャラクターの中から、憧れの人を1人選ぶ。あなたの【使命】は、その人と互いに対する《好意》を4点以上にすることである。【使命】を達成するまで憧れの人から自分に対する《好意》が1点上昇するたび．自分の《気力》を1点回復する。",
    "親の仇。あなたの親は多くの国民から慕われているランドメイカーだった。しかし、ある日、冒険の途中で命を落とした。あなたは、親の仇をとるために剣をとった。GMと相談して、親と親を倒した相手の設定を決めること。あなたの【使命】は、親の仇をとることである。仇をとれたかどうかは、GMが判断すること。【使命】を達成するまで、一般スキル【真の想い】を修得する。このスキルの目標は「親の仇」となり、初期の属性は《敵意》のいずれかから選ぶ。また、セッション開始時に「親の仇」に対する属性が《好意》になっていた場合、それを《敵意》に変更できる。",
    "英雄になりたい！あなたは、物語の中の英雄に憧れている。物語のような冒険をして、物語のような活躍がしたくて、候補生を目指した。あなたの【使命】は、勲章を3つ以上獲得することである。【使命】を達成するまで、自分が判定に絶対成功したとき、追加で《気力》を1点獲得することができる。",
    "あの日の誓い。あなたは幼い頃、怪物に襲われ、命の危機に瀕したことがある。そのとき、他国のランドメイカーが通りすがり、助けてもらった。以来、自分を助けてくれた恩人ランドメイカーのようになりたくて候補生を目指した。助けてくれた恩人の設定（特にクラスとジョブ）を決めること。あなたの【使命】は、その恩人の苦境や困難を救うことである。救うことができたかどうかは、GMが判断すること。【使命】を達成するまで、自分が【見習い】のスキルの目標を選ぶとき、通常の効果で選んだ目標1体に加え、恩人1人を目標に選ぶことができる。",
    "大家族。あなたの家族はとても多く、生活苦におちいった結果、10MGの借金を負っている。この借金を返すため、あなたは候補生になった。あなたの【使命】は、借金を完済することである。補助行動として《予算》を消費し、借金を返済できる。返済した金額をメモすること。借金を1MGも返済しなかったセッションが終了するたび、利子として借金は1MGずつ上昇していく。この【使命】を持つ者が1人いるたび、自国の《民》を10人増やすこと。",
    "劣等感。あなたは、とある王国のランドメイカーの子として生まれた。あなたの家族は、みな優秀でランドメイカーや逸材として王国に頁献していたが、あなたは力を発揮できる場所がなく、家族に対する劣等感を持ったままその国を去った。その後、あなたはこの国にたどりつき、力足りないながらも剣をとった。GMと相談して、故郷と家族の設定を決めること。あなたの【使命】は家族に対する劣等感を払拭することである。払拭できたかどうかはGMが判断すること。【使命】を運成するまで自分が絶対失敗しても《民の声》は減少しない。",
];

/// Ruby `候補生背景表`（`1D6`）。
static MKB_TABLE_CABT: Table = Table::from_dice("候補生背景表", 1, 6, MKB_TABLE_CABT_ITEMS);

/// Ruby `上級ジョブ表・甲`の項目。
static MKB_TABLE_AJTA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("勇者（基本p.239）")),
    (12, TableItem::Text("アイドル（基本p.239）")),
    (13, TableItem::Text("魔女（基本p.239）")),
    (14, TableItem::Text("剣闘士（基本p.240）")),
    (15, TableItem::Text("錬金術師（基本p.240）")),
    (16, TableItem::Text("乗り手（基本p.240）")),
    (21, TableItem::Text("夢売り（上級p.56）")),
    (22, TableItem::Text("凶戦士（上級p.56）")),
    (23, TableItem::Text("法律家（上級p.56）")),
    (24, TableItem::Text("暗殺者（上級p.57）")),
    (25, TableItem::Text("売国奴（上級p.57）")),
    (26, TableItem::Text("花嫁／花婿（上級p.57）")),
    (31, TableItem::Text("異端審問官（上級p.58）")),
    (32, TableItem::Text("禍々師（上級p.58）")),
    (33, TableItem::Text("軍師（上級p.58）")),
    (34, TableItem::Text("魔物使い（上級p.59）")),
    (35, TableItem::Text("委員長（上級p.59）")),
    (36, TableItem::Text("罪人（上級p.59）")),
    (41, TableItem::Text("刀匠（上級p.60）")),
    (42, TableItem::Text("仕掛け人（上級p.60）")),
    (43, TableItem::Text("祓魔師（上級p.60）")),
    (44, TableItem::Text("極道（大冒険p.67）")),
    (45, TableItem::Text("迷宮探偵（大冒険p.67）")),
    (46, TableItem::Text("大使（大冒険p.67）")),
    (51, TableItem::Text("征服者（大冒険p.51）")),
    (52, TableItem::Text("魔砲使い（大冒険p.51）")),
    (53, TableItem::Text("戦闘絋兵（大冒険p.51）")),
    (54, TableItem::Text("氷点師（大冒険p.53）")),
    (55, TableItem::Text("火葬官（大冒険p.53）")),
    (56, TableItem::Text("司書（大冒険p.53）")),
    (61, TableItem::Text("竜拳士（大冒険p.55）")),
    (62, TableItem::Text("人足頭（大冒険p.55）")),
    (63, TableItem::Text("即身仏（大冒険p.55）")),
    (64, TableItem::Text("闘資家（大冒険p.57）")),
    (65, TableItem::Text("絡繰使い（大冒険p.57）")),
    (66, TableItem::Text("工作員（大冒険p.57）")),
];

/// Ruby `上級ジョブ表・甲`（D66）。
static MKB_TABLE_AJTA: D66Table = D66Table::new(
    "上級ジョブ表・甲",
    D66SortType::NoSort,
    MKB_TABLE_AJTA_ITEMS,
);

/// Ruby `上級ジョブ表・乙`の項目。
static MKB_TABLE_AJTB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("勇者（基本p.239）")),
    (12, TableItem::Text("アイドル（基本p.239）")),
    (13, TableItem::Text("魔女（基本p.239）")),
    (14, TableItem::Text("剣闘土（基本p.240）")),
    (15, TableItem::Text("錬金術師（基本p.240）")),
    (16, TableItem::Text("乗り手（基本p.240）")),
    (22, TableItem::Text("迷宮探偵（大冒険p.67）")),
    (23, TableItem::Text("征服者（大冒険p.51）")),
    (24, TableItem::Text("魔砲使い（大冒険p.51）")),
    (25, TableItem::Text("戦闘鉱兵（大冒険p.51）")),
    (26, TableItem::Text("氷点師（大冒険p.53）")),
    (33, TableItem::Text("火葬宮（大冒険p.53）")),
    (34, TableItem::Text("司書（大冒険p.53）")),
    (35, TableItem::Text("竜拳士（大冒険p.55）")),
    (36, TableItem::Text("人足頭（大冒険p.55）")),
    (44, TableItem::Text("即身仏（大冒険p.55）")),
    (45, TableItem::Text("闘資家（大冒険p.57）")),
    (46, TableItem::Text("絡繰使い（大冒険p.57）")),
    (55, TableItem::Text("工作員（大冒険p.57）")),
    (56, TableItem::Text("極道（大冒険p,67）")),
    (66, TableItem::Text("大使（大冒険p.67）")),
];

/// Ruby `上級ジョブ表・乙`（D66）。
static MKB_TABLE_AJTB: D66Table =
    D66Table::new("上級ジョブ表・乙", D66SortType::Asc, MKB_TABLE_AJTB_ITEMS);

/// Ruby `職長背景表`の項目。
static MKB_TABLE_FOBT_ITEMS: &[&str] = &[
    "教え子。あなたには、あなたが見込んだ弟子が1人いる。あなたは、この弟子を育てて一人前にしたいと思っている。自国の《民》の中から弟子を1人選び、設定を決めること。弟子は、あなたが冒険におもむくときは必ずあなたの《特殊配下》として同行する。あなたの【使命】は、弟子を目分と同じジョブの逸材にすることである。【使命】を達成するまで、《特殊配下》に弟子がいる場合、1セッションに1度、キャンプ中に判定を必要としない支援行動を追加で1回行うことができる。",
    "ライバル。あなたには同じ道を歩むライバルがいる。あなたはそのライバルと正々堂々と戦い、勝利したいと考えている。ライバルの設定をGMと相談して決めること。あなたの【使命】は、ライバルと自分のジョブの技術にまつわる競技や試合を行い、それに勝利することである。【使命】を達成するまで、自分が「致命傷表」を使用することになったとき、そのサイコロを1度だけ振りなおすことができる。この効果は1セッションに1度まで使用できる。",
    "師匠の教え。あなたは、自分のジョブの技術に関する奥義を師匠から授けられている。しかし、あなたは師匠の教えを完全には解き明かしていない。あなたの【使命】は、師匠の教えを理解することである。教えを理解したかどうかは、GMが判断すること。【使命】を達成するまで、王国フェイズの「行動の処理」のタイミングで、計画的行動として《予算》を1MG獲得することができる。",
    "道に迷う。あなたには自分のジョブに関する素晴らしい才能があるが、その技術に対する情熱を失っている。この技術では本当に大切なことを成し遂げられないのではないかと思う出来事があったのだ。あなたの【使命】は、自分の技術に対する情熱を取り戻すことである。情熱を取り戻せたかどうかはGMが判断すること。【使命】を達成するまで、1セッションに1度、好きなタイミングで自分のジョブスキル1つを喪失し、【潜在能力1】を修得することができる。この潜在能力は、通常のルールに従って開花することができる。",
    "失われた技術。あなたは代々、そのジョブにまつわる技術体系を伝えてきた名門流派の一員である。あなたの流派には、過去に失われた技術に関する秘伝が存在している。秘伝に関する設定をGMと相談して決定すること。あなたの【使命】は、その秘伝を見つけ、失われた技術を復活させることである。秘伝を獲得する方法はGMが設定すること。【使命】を達成するまで1セッションに1度、自分と同じジョブのジョブスキルを修得しているキャラクターや自分のジョブに設定されているスキルグループのアドバンスドスキルを修得しているキャラクターと出会ったとき、そのキャラクターに対する《感情値》を1点上昇し、その属性を好きなものに設定できる。",
    "ブランド。あなたは自分の技術にまつわるブランドを持っている。その名声を喧伝したいと思っている。名物の選択ルールを使って、あなたのブランドを名物として設定すること。名物の名前やカテゴリは、あなたのジョブやその技術にちなんだものにする。あなたの【使命】は、その名物のレベルを4以上にすることである。【使命】を達成するまで、セッション開始時に《民の声》を1点回復することができる。",
];

/// Ruby `職長背景表`（`1D6`）。
static MKB_TABLE_FOBT: Table = Table::from_dice("職長背景表", 1, 6, MKB_TABLE_FOBT_ITEMS);

/// Ruby `深階特殊遭遇表`の項目。
static MKB_TABLE_KAET_ITEMS: &[&str] = &[
    "深階の暗闇の中をぼんやりとした光が漂っている。【夜這い海星】だ。配下たちは虚ろな目になり、その光を目指して夢遊病のように歩き出す。PC全員は〔才覚〕で難易度9の判定を行う。失敗したPCは、《配下》を1D6人減少する。",
    "どこかから美しい歌声が聞こえてくる。まずい。あれは【禍歌女】の歌だ。PC全員は〔探索〕で難易度9の判定を行う。失敗した者は、「呪い3」の変調を受け、自分に発生していた「そのクォーターの間」、「そのターンの間」、「その戦闘の間」、「そのセッションの間」持続するスキルの効果をすべて無効化する。",
    "床や壁の様子がおかしい。いつの間にか【クラーケン】の体内に入ってしまっていたようだ。PCたちは【クラーケン】の体内から脱出しないと、この部屋から移動することができなくなる。以降、脱出を試みるPCは、支援行動として好きな能力値で難易度12の判定を行うことができるようになる。宮廷で手分けして〔才覚〕、〔魅力〕、〔武勇〕、〔探索〕の能力値すべての判定に成功すると【クラーケン】の中から脱出できる。判定に失敗したPCは《気力》1点を減少し、1D6点のダメージを受ける。また、【クラーケン】の体内にいる間、各クォーターの経過時に、PC全貝は《HP》を1D6点減少する。このとき、6の目が出た者は、所持しているアイテム1個を消費する。",
    "ぬらぬらとした鱗を持つ【魚人】の群れに遭遇する。その目は、何を考えているのか分からない。PCの代表は〔才覚〕で難易度11の判定を行う（「言語」の選択ルールを使用して、深人語を修得していたら自動的に成功する）。成功すると、彼らがPCたちの王国を襲撃しようとしていることが分かる。PC全員は〔武勇〕で難易度［15－宮廷の人数］の判定を行う。成否にかかわらず、【魚人】たちの企みは止めることができるが、失敗した者は6点のダメージと「毒2」の変調を受ける。〔才覚〕の判定に失敗すると彼らの狙いに気づくことができない。終了フェイズの「王国変動」のタイミングで、追加で1回「王国変動表（遠征王国変動表）」の4番の効果が発生する。",
    "地面に突き刺さっている【鑓魚】を見つける。「俺の使い手にふさわしいかどうか、試してみるか？」と問われる。各PCは、誰かが成功するまで、各1回ずつ試すことができる。試してみるPCは、〔武勇〕で難易度15の判定を行う。成功したPCは【鑓魚】を抜き、その【鑓魚】1体を《特殊配下》に加えることができる。この【鑓魚】を《特殊配下》にしている間、そのPCは、2レベルの【鑓】を装備しているものとして扱う。失敗したPCは、《HP》を現在値の半分の値にし、「呪い3」の変調を受ける。",
    "濡れた床に足をとられ、思わず手にしていた武器を泉に落としてしまう。PCの中からランダムに1人を選ぶ。そのPCは、自分の装備している武具アイテムの中からランダムに1個を選び、それを消費する。その後、泉の中から【ニーニアン】が現れ、「あなたの落としたのはこの【貪る者】でしょうか？ それとも【滅ぼす者】でしょうか？」と訊ねてくる。正直に答えると【ニーニアン】は泉に消える。いずれかのレア武具アイテムだと答えると、武器を落としたPCは〔魅力〕で難易度16の判定を行う。成功すると、そのレア武具アイテム1個を獲得する。失敗すると［2D6+5］点のダメージと「呪い4」の変調を受ける。",
];

/// Ruby `深階特殊遭遇表`（`1D6`）。
static MKB_TABLE_KAET: Table = Table::from_dice("深階特殊遭遇表", 1, 6, MKB_TABLE_KAET_ITEMS);

/// Ruby `深階休憩表`の項目。
static MKB_TABLE_KART_ITEMS: &[&str] = &[
    "深階の闇の中で真実に気づき始める。ここで生き残るために必要なのは理性ではない。狂気だ。宮廷の中で《気力》が0点のキャラクターは《気力》を1D6点回復する。",
    "見覚えのある人物に出会う。しかし、あなたは以前に死んだはずッ!?その人物は、死後、この迷宮の支配者に囚われているらしい。支配者を倒してほしいと依頼してくる。GMは「苦手な物品表」（基本p.169）を1回使用して、その支配者にとっての「毒」の弱点を1つ設定する。この効果は3回まで累積する。",
    "闇の中で感覚が研ぎ澄まされてくる。そのセッションの間、《希望》1点を消費すると、そのラウンドの間、射撃戦用武器を使って攻撃を行うとき、光源の範囲にいないキャラクターを目標に選ぶことができるようになる。",
    "濡れた髪や衣装が肌にはりつく。〔魅力〕で難易度9の判定に成功すると、宮廷の中からランダムにキャラクター1人を選ぶ。そのキャラクターから自分に対する《好意》を1点上昇する。",
    "漂流物が堆積している。「素材表」を1回使用する。2が出なければ、その素材を［1D6+3］個獲得する。",
    "深階にふさわしくない甘い香りのする泉を発見する。【特効薬】か【ポーション】を1D6個獲得する。この効果によって獲得した【ポーション】や【特効薬】を使用するたび、そのセッションの間、《HP》の最大値を1点上昇する。このアイテムを使用したキャラクターは、そのセッションの終了フェイズの解散のタイミングで2D6を振る。その出目が［この効果によって獲得したアイテムによって上昇した《HP》の最大値］以下になった場合、深人に変貌し、そのキャラクターはNPCになる。",
    "周囲を回遊している渡り魚の群れを見つける。〔探索〕か〔武勇〕で難易度5の判定を行う。成功すると、［判定の達成値－判定の難易度］個の「肉」を獲得する。望むならその「肉」を使用して、【お弁当】か【フルコース】1個を作成することができる。",
    "この辺りは周囲に比べて温かい水がある。温泉のようだ！宮廷は温泉に入ることができる。温泉に入ることにしたキャラクターは《気力》を2点回復する。",
    "水辺に浮いているガラス瓶を見つける。中に何か書類が入っている気がする……。〔才覚〕で難易度11の判定に成功すると、好きな部屋1を選ぶ。その部屋の通路情報と脅威情報をGMから教えてもらう。",
    "水の中を移動するのにも慣れてきた。【水槽】（基本p.153）、【底なし沼】（基本p.161）、【水たまり】（基本p.161）、【冷や水】（上級p.88）の中からトラップを2つ選ぶ。そのセッションの間、自分はそのトラップの目標にならない。",
    "目の前の扉が突然開くと、そこには宝箱があった。値打ち物を見つけることができるかもしれない！【エレベータ】を発見する。そして、〔探索〕で難易度9の判定を行う。成功すると、ランダムに武具アイテム1種を選ぶ。2レベルのそのアイテム1個を獲得する。失敗すると、【大爆発】（基本p.151）のトラップの効果が発生する。",
];

/// Ruby `深階休憩表`（`2D6`）。
static MKB_TABLE_KART: Table = Table::from_dice("深階休憩表", 2, 6, MKB_TABLE_KART_ITEMS);

/// Ruby `星の階休憩表`の項目。
static MKB_TABLE_HKRT_ITEMS: &[&str] = &[
    "星の階からあなたに、「予備動力を使い、攻撃のためのエネルギーを1回分充填しました」という通信が入る。宮廷の代表は、砲撃と同じ処理を1度行うことができる（施設は必要ない）。",
    "窓の外を眺めていたら、敵の増援が飛来するのを発見した。GMに、次のラウンドの増援の情報を教えてもらうことができる。増援のモンスター名と数、どの部屋に来るかが分かる。",
    "天使の攻撃によって船は激しく揺れ、子供たちが泣き叫んでいる。子供のために歌を歌っていた女性が、あなたを見て微笑む。何か気の利いたことを言って励ましてやれないだろうか。〔才覚〕で難易度7の判定を行う。成功すると《民の声》を2点回復する。",
    "突然、床が抜けて落下していった。あなたは華麗に身をかわそうとする。〔探索〕で難易度7の判定を行う。成功すれば隠し通路を発見し、好きな部屋に移動することができる。失敗すると、自分の《配下》を1人減少する。",
    "敵の攻撃で柵が壊れて家畜が逃げ出し、農夫がそれを追いかけている。あなたは家畜を捕まえようとする。〔魅力〕で難易度7の判定を行う。成功すると家畜を捕まえることに成功し、感謝した農夫はあなたに【乗騎】のアイテムをくれる。",
    "勇敢に戦うあなたたちを間近で見て、民が感激している。《民の声》を1点回復する。",
    "衛生兵が重そうな荷物を背負って移動しているのに出くわす。あちこちに怪我人がおり、治療が間に合わないそうだ。あなたは荷物を運ぶのを手伝い、かわりに元気が出るスープを飲ませてもらった。《気力》を1点回復し、《HP》を2点回復する。",
    "突入に失敗したマッハペンギンの死体が外壁に突き刺さっていた。「肉」と「鉄」の素材を1個ずつ獲得する。",
    "敵の攻撃が近くの窓に着弾し、砲弾が飛び込んできた。あなたは体を張って砲弾を処理しようとする。〔武勇〕で難易度7の判定を行う。成功したら、同じ部屋にいる自軍のキャラクター1体を選ぶ。そのキャラクターの自分に対する《好意》を1点上昇する。もし同じ部屋に選べる自軍のキャラクターがいなかった場合は、《民の声》を1点回復する。失敗したら、1D6点のダメージを受ける。",
    "伝令が走っているのに遭遇する。敵の通信の解読に成功したようだ。それによって、敵の指揮官が判明した。GMに、敵の指揮官のモンスター名と場所を教えてもらうことができる。",
    "船内に密航者がいるのを発見した。こんな時だ、罰を与えるかわりに働いてもらおう。「生まれ決定表」（基本p.24）でランダムにジョブを決めた逸材が1人増える。",
];

/// Ruby `星の階休憩表`（`2D6`）。
static MKB_TABLE_HKRT: Table = Table::from_dice("星の階休憩表", 2, 6, MKB_TABLE_HKRT_ITEMS);

/// Ruby `装備決定表`の項目。
static MKB_TABLE_EQDT_ITEMS: &[&str] = &[
    "あなたの国は防御対策を重視している。自国に【バリケード】を1軒建設する。",
    "あなたの国は敵軍の攻撃に備えている。自国に【砲台】を1軒建設する。",
    "あなたの国は迫り来る敵の迎撃に力を入れている。自国に【秘密兵器】を1軒建設する。",
    "あなたの国は戦況の把握を行うための作戦室を持っている。自国に【司令室】を1軒建設する。",
    "あなたの国は多様な資源を動力に変換できる。自国に【動力室】を1軒建設する。",
    "あなたの国は色々な場所を旅できるよう設計されている。自国に【機動装置】を1軒建設する。",
];

/// Ruby `装備決定表`（`1D6`）。
static MKB_TABLE_EQDT: Table = Table::from_dice("装備決定表", 1, 6, MKB_TABLE_EQDT_ITEMS);

/// Ruby `大冒険時代表`の項目。
static MKB_TABLE_GART_ITEMS: &[&str] = &[
    "美しい天使と出会い、宮廷のほかのメンバーを巻き込んだ大恋愛に発展する。その後、悲劇的な別れを経験し、人間的な成長を遂げた。",
    "愛する者が死に、その人物をよみがえらせるために聖杯の探索を行う。天階を揺るがす大冒険のすえ、聖杯を入手し、愛する者をよみがえらせる。",
    "自分のために命を落とした者と天国で再会する。「私の命を犠牲にした価値があったのでしょうか？」と問われ、その問いに応えるため、ひたすら修行に打ち込む。",
    "時空階に転落し、様々な時代を旅する。巨鬼戦役、ビクトリー帝国の没落、ハグルマの誕生、迷宮大戦……過去の様々な大事件に立ち会い、見聞を広める。",
    "記憶を失い、宮廷と別れる。しばし、天使と共に天階を放浪し、異階の技術を身につける。あるとき唐突に記憶が戻り、宮廷のもとに戻って来た。",
    "天階の戦争に巻き込まれる。羽根兜の乙女に見込まれ、護国天使らと共に深人と戦う。生きるか死ぬかの日々のなかで、戦闘技術が研ぎ澄まされていく。",
];

/// Ruby `大冒険時代表`（`1D6`）。
static MKB_TABLE_GART: Table = Table::from_dice("大冒険時代表", 1, 6, MKB_TABLE_GART_ITEMS);

/// Ruby `天階特殊遭遇表`の項目。
static MKB_TABLE_TEET_ITEMS: &[&str] = &[
    "冬星が輝きだし、周囲の気温が下がり始める。【冬将軍】が現れたようだ。彼はPCたちの行いが正しいかどうかを判断しに来たようだ。そのセッション中に、PCが正当な理由なく弱者を踏みにじったり、搾取したりしていた場合、宮廷全員の《HP》を現在値の半分の値にする。PCたちが、そのセッション中に、そうした振る舞いをしていなかった場合、贈り物を1つくれる。ランダムにレア一般アイテム1種を選ぶ。宮廷の代表は、1レベルのそのアイテム1個を獲得する。",
    "極彩色の烏が一羽急降下して、鼻の穴からすっと入ってくる。脳内の言語を混乱させるという【バベル鳥】だ！PCの中からランダムに目標を1人選ぶ。目標は〔探索〕で難易度13の判定を行う。失敗すると、そのターンの間、目標は何らかのスキルを使用するとき、《希望》1点を消費しないと、自分以外のキャラクターをそのスキルの目標に選ぶことができない。",
    "【白衣の天使】の一団に遺遇する。PCは〔魅力〕で難易度9の判定を行うことができる。成功した者は、《HP》を2D6点回復するか、好きな変調1つを回復する。失敗した者は《気力》1D6点を減少する。もし、宮廷の中に行動不能のキャラクターがいれば、判定の達成値は2点上昇する。もし、PCの中に深階を信仰しているキャラクターがいれば、判定の達成値は2点減少する。",
    "奇妙な形をした無機天使が編隊を形成して飛んでいる。【十機翼】だ。PC全員は〔探索〕で難易度9の判定を行う。誰か1人でも失敗すると、PC全員は［1D6+5］点のダメージを受ける。そのセッション中に、PCの誰かが天使カテゴリのモンスターを1体以上死亡させていた場合、この判定は自動的に失敗する。",
    "上空から【龍殺天使】が襲いかかってくる。【飛行】を修得していないPCは〔武勇〕で難易度13の判定を行う。失敗した者は［3D6+2］点のダメージを受ける。",
    "遥か遠くに【浮遊宮殿】が見える。距離は離れているはずなのに、恐ろしく巨大だ。その異質な様相に恐怖し、体がすくんでくる。PC全員は〔武勇〕で難易度15の判定を行う。失敗したキャラクターは《配下》が［判定の難易度－判定の達成値］人王国に帰還する。",
];

/// Ruby `天階特殊遭遇表`（`1D6`）。
static MKB_TABLE_TEET: Table = Table::from_dice("天階特殊遭遇表", 1, 6, MKB_TABLE_TEET_ITEMS);

/// Ruby `天階休憩表`の項目。
static MKB_TABLE_TERT_ITEMS: &[&str] = &[
    "高音が響き渡る。いつの間にか仲間の姿は消え、誰もいない真っ青な空間に1人浮かんでいる。過去や未来の幻があなたの周囲を通り抜けていく。「天階休憩表」をあと2回使用する。",
    "遠くに見える空星が、強い光を放つ。自分の所持している【星の欠片】の中からランダムに1個を選ぶ。その【星の欠片】のレベルを1個上昇する。",
    "翼を持つ天使たちが頭上を飛んでいく。羽根が1枚落ちてきた。【星の欠片】か【お守り】を1個獲得する。",
    "強い風が吹き、一行の服や髪がたなびく。〔魅力〕で難易度9の判定に成功すると、宮廷の中からランダムにキャラクター1人を選ぶ。そのキャラクターから自分に対する《好意》を1点上昇する。",
    "無限に広がる空を見あげる。恐怖と同時に、清々しい感情がわき上がってくる。自分が受けている変調1つを回復する。",
    "思考が澄み渡り、意識が高くなってきているのを感じる。そのセッション中、この効果を受けると1度目は《気力》を1点回復する。2度目は、そのセッションの間《器》を1点上昇する。3度目は、そのセッションの間、【飛行】を修得する。4度目以降は、そのセッションの終了フェイズの解散のタイミングで、天使に変貌し、そのキャラクターはNPCになる。",
    "壁や天井に視界が遮られることがない空間。遠くが見える。隣接する部屋1つを目標に選ぶ（通路でつながっていなくてもかまわない）。「情報収集表」を1回使用する。「情報収集表」で入手できる情報は、自動的に目標に関するものに変更される。",
    "一瞬、自分達が冒険している姿が脳裏に浮かび上がる。もしかして、これは未来の光景……？〔才覚〕か〔探索〕で難易度9の判定に成功すると、そのセッションの間、宮廷の誰かが行った行為判定のサイコロを1回だけ振り直すことができる。",
    "何もない青い空間を見あげているうちに、将来の夢を語りたくなってきた。〔才覚〕か〔魅力〕で難易度9の判定に成功すると、宮廷全員の《気力》1点を回復し、《民の声》1点を回復する。",
    "いつの間にかうたた寝していたのだろうか。見たことのない技を振るう自分の姿を夢で見た。あの技はもしや……。自分のスキルグループにあるカテゴリのアドバンスドスキルの中から、好きなスキル1つを選ぶ。そのセッションの間、潜在能力に開花したとき、ランダムではなく、そのスキルを修得することができる。",
    "扉がぽつんと立っているのを見つける。それが開き、不思議そうな表情をした人々が現れた。彼らは、口々に自分は死んだはずだと言っている。もしかすると天階側の昇降機なんだろうか？【エレベータ】を発見する。そして、《配下》を1D6人獲得し、宮廷に自由に割り振ることができる。",
];

/// Ruby `天階休憩表`（`2D6`）。
static MKB_TABLE_TERT: Table = Table::from_dice("天階休憩表", 2, 6, MKB_TABLE_TERT_ITEMS);

/// Ruby `燃料表`の項目。
static MKB_TABLE_FUET_ITEMS: &[&str] = &["火薬", "魔素", "肉", "機械", "鉄", "木"];

/// Ruby `燃料表`（`1D6`）。
static MKB_TABLE_FUET: Table = Table::from_dice("燃料表", 1, 6, MKB_TABLE_FUET_ITEMS);

/// Ruby `反乱表`の項目。
static MKB_TABLE_RBLT_ITEMS: &[&str] = &[
    "退廃。豪傑は、冗談めかしてほかの逸材にランドメイカーの不満を話す。自国の逸材の中からランダムに選んだ1人の《野望》が［1D6×1/2］点上昇する。",
    "扇動。豪傑は、秘かに王国内の不満を煽る。《民の声》を1D6点減少する。",
    "分断。豪傑は、王国の民同士の対立を煽る。宮廷全員は、自分の《配下》を［《配下》の現在値×1/2］人、王国に帰還させる。",
    "テロ。豪傑は陰から不満分子を操り、王国にテロを仕掛ける。ランダムに選んだ国難を受ける。その数値は1となる。この効果によって、すでに受けている国難を受けた場合、その国難の数値が1点上昇する（最大3まで）。",
    "独立。豪傑は、幾人かの仲間とともに独立を宣言する。GMは、自国の領土の中からランダムに部屋を1つ選び、それを豪傑の領土とする。自国から、その部屋につながる通路をすべて無くすこと。その結果、その部屋以外にも、自国とまったく通路でつながらなくなってしまう領士が発生した場合、そうした部屋も豪傑の領土となる。",
    "翻意。時はまだ熟していない。豪傑は、そう判断して反乱を延期する。その豪傑の《野望》を1点減少する。",
];

/// Ruby `反乱表`（`1D6`）。
static MKB_TABLE_RBLT: Table = Table::from_dice("反乱表", 1, 6, MKB_TABLE_RBLT_ITEMS);

/// Ruby `旅する王国環境表`の項目。
static MKB_TABLE_TKET_ITEMS: &[&str] = &[
    "あなたの国は独自の技術が発展している。「技術決定表」（基本p.15）を使用する。",
    "あなたの国は独特の気風がある。「国風決定表」（基本p.15）を使用する。",
    "あなたの国は特殊な装備を搭載している。「装備決定表」を使用する。",
    "あなたの国は幾つかの施設がある。「施設決定表」（基本p.16）を使用する。",
    "あなたの国は有能な人材がいる。「人材決定表」（基本p.16）を使用する。",
    "あなたの国は特殊な血筋の末裔である。「血族決定表」（基本p.16）を使用する。",
];

/// Ruby `旅する王国環境表`（`1D6`）。
static MKB_TABLE_TKET: Table = Table::from_dice("旅する王国環境表", 1, 6, MKB_TABLE_TKET_ITEMS);

/// Ruby `旅する王国名決定表1`の項目。
static MKB_TABLE_TKNT1_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("宇宙/秘境/最終/空飛ぶ")),
    (12, TableItem::Text("銀河/移動/秘密/夢の")),
    (13, TableItem::Text("飛空/浮遊/魔進/のんびり")),
    (14, TableItem::Text("天階/航空/原始/戦え")),
    (15, TableItem::Text("深階/虚空/暗黒/世界の")),
    (16, TableItem::Text("異階/魔階/新/泳ぐ")),
    (22, TableItem::Text("因果/忘却/時空/旅する")),
    (23, TableItem::Text("機動/変位/歴史/夜明けの")),
    (24, TableItem::Text("疾風/巡礼/創世/真夜中の")),
    (25, TableItem::Text("海底/下降/装甲/戦う")),
    (26, TableItem::Text("爆発/戦闘/決戦/はたらく")),
    (33, TableItem::Text("閃光/漂流/無限/生きている")),
    (34, TableItem::Text("発進/彷徨/勇者/怒れる")),
    (35, TableItem::Text("空中/巡航/階賊/楽しい")),
    (36, TableItem::Text("特急/高速/空賊/転がる")),
    (44, TableItem::Text("弾丸/上昇/機界/輝く")),
    (45, TableItem::Text("急行/光臨/神仙/よろめく")),
    (46, TableItem::Text("回転/回遊/軍事/さまよう")),
    (55, TableItem::Text("潜水/螺旋/自動/うごめく")),
    (56, TableItem::Text("神秘/潜行/携帯/歩く")),
    (66, TableItem::Text("大/未確認/コンビニ/たゆたう")),
];

/// Ruby `旅する王国名決定表1`（D66）。
static MKB_TABLE_TKNT1: D66Table = D66Table::new(
    "旅する王国名決定表1",
    D66SortType::Asc,
    MKB_TABLE_TKNT1_ITEMS,
);

/// Ruby `旅する王国名決定表2`の項目。
static MKB_TABLE_TKNT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("軍艦/大陸/廃園/鬼神")),
    (12, TableItem::Text("戦艦/海溝/楽園/怪鳥")),
    (13, TableItem::Text("空母/海峡/監獄/鉄人")),
    (14, TableItem::Text("艦隊/暗礁/迷宮/死霊")),
    (15, TableItem::Text("船団/群島/王国/神亀")),
    (16, TableItem::Text("艇/島/帝国/恐竜")),
    (22, TableItem::Text("船/列島/要塞/巻貝")),
    (23, TableItem::Text("戦機/現象/城塞/飛天")),
    (24, TableItem::Text("天体/（の、する）星/衛星/妖魔")),
    (25, TableItem::Text("空機/彗星/基地/天使")),
    (26, TableItem::Text("飛行機/星雲/鉄橋/飛龍")),
    (33, TableItem::Text("飛行体/天虹/墓場/魔獣")),
    (34, TableItem::Text("（の、する）翼/惑星/企業/妖精")),
    (35, TableItem::Text("車両/荒野/神殿/幻獣")),
    (36, TableItem::Text("戦車/氷原/宮殿/大蛇")),
    (44, TableItem::Text("急行/密林/回廊/巨獣")),
    (45, TableItem::Text("列車/砂漠/劇場/魔竜")),
    (46, TableItem::Text("器官車/山脈/酒場/海獣")),
    (55, TableItem::Text("円盤/衛星/教室/魔神")),
    (56, TableItem::Text("鉄道/洞窟/楼閣/地霊")),
    (66, TableItem::Text("（の、する）足/温泉/旅館/精霊")),
];

/// Ruby `旅する王国名決定表2`（D66）。
static MKB_TABLE_TKNT2: D66Table = D66Table::new(
    "旅する王国名決定表2",
    D66SortType::Asc,
    MKB_TABLE_TKNT2_ITEMS,
);

/// Ruby `旅する王国名決定表3`の項目。
static MKB_TABLE_TKNT3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("フジヤマ/イカルガ/ラプラス/ヴァイス")),
    (12, TableItem::Text("カナタ/ミライ/アルゴー/ヴェルヌ")),
    (13, TableItem::Text("ナユタ/キザハシ/アルカディア/バロウズ")),
    (14, TableItem::Text("バンリ/キボウ/インビンシブル/ウェルズ")),
    (
        15,
        TableItem::Text("アスカ/ハバタキ/エンタープライズ/ゼラズニィ"),
    ),
    (16, TableItem::Text("バサン/ホウオウ/マクスウェル/イーガン")),
    (
        22,
        TableItem::Text("ハイタカ/ワタリガラス/ファルコン/タナカ"),
    ),
    (23, TableItem::Text("ハヤブサ/ウミネコ/オリンピア/スミス")),
    (24, TableItem::Text("ウミグモ/シャチホコ/タートル/ギブスン")),
    (25, TableItem::Text("ウツボ/アンコウ/バハムート/ムアコック")),
    (
        26,
        TableItem::Text("カゲワニ/ワダツミ/ノーチラス/オーウェル"),
    ),
    (33, TableItem::Text("ゴウテン/ムザン/トラベラー/マサムネ")),
    (
        34,
        TableItem::Text("タカラ/オモカゲ/クレギオン/ガーンズバック"),
    ),
    (35, TableItem::Text("フガク/キラボシ/アーク/ティプトリ")),
    (36, TableItem::Text("アラシ/タソガレ/ニルヴァーナ/ディック")),
    (
        44,
        TableItem::Text("モグラ/シルベ/インフェルノ/ハインライン"),
    ),
    (45, TableItem::Text("オボロ/セツナ/パイオニア/アシモフ")),
    (46, TableItem::Text("ビャクヤ/ゲッコウ/エスケープ/クラーク")),
    (
        55,
        TableItem::Text("シラベ/シジマ/ギャラクティカ/ブラッドベリ"),
    ),
    (56, TableItem::Text("シラヌイ/ライテイ/ディスカバリー/レム")),
    (66, TableItem::Text("フブキ/ヒサメ/エンデバー/ラブクラフト")),
];

/// Ruby `旅する王国名決定表3`（D66）。
static MKB_TABLE_TKNT3: D66Table = D66Table::new(
    "旅する王国名決定表3",
    D66SortType::Asc,
    MKB_TABLE_TKNT3_ITEMS,
);

/// Ruby `列強経歴表`の項目。
static MKB_TABLE_GPHT_ITEMS: &[&str] = &[
    "あなたは元々列強で生まれた。生家で問題を起こして、国を飛び出した。",
    "あなたは元々はPCの国にいた。家族や仕事の関係で、列強にしばらく滞在していた。",
    "あなたは今でも列強に忠誠を誓っている。しかし、そのことを隠してPCたちの国に移住してきた。これは、ある秘密任務の一環である。",
    "あなたは元々列強で生まれた。その後、世界を見るべく迷宮を旅している途中、PCの国の民に大きな恩を受けた。この恩を返すまで、この国の食客となっている。",
    "あなたは列強の属国の一つにいた。その属国が滅ぼされ、放浪の後にPCの王国にたどりついた。",
    "あなたは実は列強の有力者の子息である。行儀見習いとして、PCたちの王国に滞在していた。",
];

/// Ruby `列強経歴表`（`1D6`）。
static MKB_TABLE_GPHT: Table = Table::from_dice("列強経歴表", 1, 6, MKB_TABLE_GPHT_ITEMS);

/// Ruby `アイテムカテゴリ決定表`（Ruby定数 `ITEM_RANDOM_TABLE`）の項目。
static MKB_ITEM_RANDOM_TABLE_ITEMS: &[TableItem] = &[
    TableItem::Table(&MKB_TABLE_WIT),
    TableItem::Table(&MKB_TABLE_LIT),
    TableItem::Table(&MKB_TABLE_RIT),
    TableItem::Table(&MKB_TABLE_SIT),
    TableItem::Table(&MKB_TABLE_RUIR),
    TableItem::Table(&MKB_TABLE_RWIR),
];

/// Ruby `アイテムカテゴリ決定表`（`1D6` の連鎖表）。
static MKB_ITEM_RANDOM_TABLE: ChainTable =
    ChainTable::from_dice("アイテムカテゴリ決定表", 1, 6, MKB_ITEM_RANDOM_TABLE_ITEMS);

/// Ruby `MeikyuKingdomBasic::TABLES`。
static MKB_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("TBT", &MKB_TABLE_TBT),
    ("CBT", &MKB_TABLE_CBT),
    ("SBT", &MKB_TABLE_SBT),
    ("VBT", &MKB_TABLE_VBT),
    ("THT", &MKB_TABLE_THT),
    ("CHT", &MKB_TABLE_CHT),
    ("SHT", &MKB_TABLE_SHT),
    ("VHT", &MKB_TABLE_VHT),
    ("KDT", &MKB_TABLE_KDT),
    ("KCT", &MKB_TABLE_KCT),
    ("CAT", &MKB_TABLE_CAT),
    ("FWT", &MKB_TABLE_FWT),
    ("CFT", &MKB_TABLE_CFT),
    ("TT", &MKB_TABLE_TT),
    ("NT", &MKB_TABLE_NT),
    ("ET", &MKB_TABLE_ET),
    ("FRT", &MKB_TABLE_FRT),
    ("FBT", &MKB_TABLE_FBT),
    ("WBT", &MKB_TABLE_WBT),
    ("LBT", &MKB_TABLE_LBT),
    ("T1T", &MKB_TABLE_T1T),
    ("T2T", &MKB_TABLE_T2T),
    ("T3T", &MKB_TABLE_T3T),
    ("T4T", &MKB_TABLE_T4T),
    ("T5T", &MKB_TABLE_T5T),
    ("ABUS", &MKB_TABLE_ABUS),
    ("ASHS", &MKB_TABLE_ASHS),
    ("AASS", &MKB_TABLE_AASS),
    ("ASUS", &MKB_TABLE_ASUS),
    ("ASCS", &MKB_TABLE_ASCS),
    ("ALAS", &MKB_TABLE_ALAS),
    ("ANES", &MKB_TABLE_ANES),
    ("ACOS", &MKB_TABLE_ACOS),
    ("AENS", &MKB_TABLE_AENS),
    ("ATOS", &MKB_TABLE_ATOS),
    ("RMS", &MKB_TABLE_RMS),
    ("RT", &MKB_TABLE_RT),
    ("SE", &MKB_TABLE_SE),
    ("IG", &MKB_TABLE_IG),
    ("BDT", &MKB_TABLE_BDT),
    ("TBO", &MKB_TABLE_TBO),
    ("CBO", &MKB_TABLE_CBO),
    ("SBO", &MKB_TABLE_SBO),
    ("VBO", &MKB_TABLE_VBO),
    ("FET", &MKB_TABLE_FET),
    ("HET", &MKB_TABLE_HET),
    ("IEQ", &MKB_TABLE_IEQ),
    ("SDT", &MKB_TABLE_SDT),
    ("BUS", &MKB_TABLE_BUS),
    ("SHS", &MKB_TABLE_SHS),
    ("ASS", &MKB_TABLE_ASS),
    ("SUS", &MKB_TABLE_SUS),
    ("SCS", &MKB_TABLE_SCS),
    ("LAS", &MKB_TABLE_LAS),
    ("NES", &MKB_TABLE_NES),
    ("COS", &MKB_TABLE_COS),
    ("ENS", &MKB_TABLE_ENS),
    ("TOS", &MKB_TABLE_TOS),
    ("GES", &MKB_TABLE_GES),
    ("EBT", &MKB_TABLE_EBT),
    ("ARN", &MKB_TABLE_ARN),
    ("WEN", &MKB_TABLE_WEN),
    ("NEN", &MKB_TABLE_NEN),
    ("CEN", &MKB_TABLE_CEN),
    ("SEN", &MKB_TABLE_SEN),
    ("OEN", &MKB_TABLE_OEN),
    ("ORT", &MKB_TABLE_ORT),
    ("WIT", &MKB_TABLE_WIT),
    ("LIT", &MKB_TABLE_LIT),
    ("RIT", &MKB_TABLE_RIT),
    ("SIT", &MKB_TABLE_SIT),
    ("CIR", &MKB_TABLE_CIR),
    ("NRUT", &MKB_TABLE_NRUT),
    ("ARUT", &MKB_TABLE_ARUT),
    ("RUIR", &MKB_TABLE_RUIR),
    ("NRWT", &MKB_TABLE_NRWT),
    ("ARWT", &MKB_TABLE_ARWT),
    ("RWIR", &MKB_TABLE_RWIR),
];

/// Ruby `MeikyuKingdomBasic::A2Z_TABLES`。
static MKB_A2Z_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("FDBT", &MKB_TABLE_FDBT),
    ("GIFT", &MKB_TABLE_GIFT),
    ("NWST", &MKB_TABLE_NWST),
    ("EKCT", &MKB_TABLE_EKCT),
    ("KDHT", &MKB_TABLE_KDHT),
    ("KDTT", &MKB_TABLE_KDTT),
    ("CHVT", &MKB_TABLE_CHVT),
    ("RITT", &MKB_TABLE_RITT),
    ("LIMT", &MKB_TABLE_LIMT),
    ("UNCT", &MKB_TABLE_UNCT),
    ("POET", &MKB_TABLE_POET),
    ("KSET", &MKB_TABLE_KSET),
    ("DODT", &MKB_TABLE_DODT),
    ("INT1", &MKB_TABLE_INT1),
    ("INT2", &MKB_TABLE_INT2),
    ("WLDT", &MKB_TABLE_WLDT),
    ("INHT", &MKB_TABLE_INHT),
    ("NIGT", &MKB_TABLE_NIGT),
    ("RACT", &MKB_TABLE_RACT),
    ("HBLT", &MKB_TABLE_HBLT),
    ("POWT", &MKB_TABLE_POWT),
    ("NSET", &MKB_TABLE_NSET),
    ("ECBT", &MKB_TABLE_ECBT),
    ("CUAT", &MKB_TABLE_CUAT),
    ("CUNT", &MKB_TABLE_CUNT),
    ("WEAT", &MKB_TABLE_WEAT),
    ("PAET", &MKB_TABLE_PAET),
    ("MEDT", &MKB_TABLE_MEDT),
    ("REAT", &MKB_TABLE_REAT),
    ("HIST", &MKB_TABLE_HIST),
    ("PRET", &MKB_TABLE_PRET),
    ("LAPT", &MKB_TABLE_LAPT),
    ("ORIT", &MKB_TABLE_ORIT),
    ("ATRT", &MKB_TABLE_ATRT),
    ("FAMT", &MKB_TABLE_FAMT),
];

/// Ruby `MeikyuKingdomBasic::RR236_TABLES`。
static MKB_RR236_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("FCNT", &MKB_TABLE_FCNT),
    ("ASNT", &MKB_TABLE_ASNT),
    ("FPNT", &MKB_TABLE_FPNT),
];

/// Ruby `MeikyuKingdomBasic::TK_TABLES`。
static MKB_TK_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("CSRT", &MKB_TABLE_CSRT),
    ("KSGT", &MKB_TABLE_KSGT),
    ("DRET", &MKB_TABLE_DRET),
    ("DSET", &MKB_TABLE_DSET),
    ("DNNT", &MKB_TABLE_DNNT),
    ("DNAT", &MKB_TABLE_DNAT),
    ("HRET", &MKB_TABLE_HRET),
    ("HSET", &MKB_TABLE_HSET),
    ("HNNT", &MKB_TABLE_HNNT),
    ("HNAT", &MKB_TABLE_HNAT),
    ("MRET", &MKB_TABLE_MRET),
    ("MSET", &MKB_TABLE_MSET),
    ("MNNT", &MKB_TABLE_MNNT),
    ("MNAT", &MKB_TABLE_MNAT),
    ("TRET", &MKB_TABLE_TRET),
    ("TSET", &MKB_TABLE_TSET),
    ("TNNT", &MKB_TABLE_TNNT),
    ("TNAT", &MKB_TABLE_TNAT),
    ("CABT", &MKB_TABLE_CABT),
    ("AJTA", &MKB_TABLE_AJTA),
    ("AJTB", &MKB_TABLE_AJTB),
    ("FOBT", &MKB_TABLE_FOBT),
    ("KAET", &MKB_TABLE_KAET),
    ("KART", &MKB_TABLE_KART),
    ("HKRT", &MKB_TABLE_HKRT),
    ("EQDT", &MKB_TABLE_EQDT),
    ("GART", &MKB_TABLE_GART),
    ("TEET", &MKB_TABLE_TEET),
    ("TERT", &MKB_TABLE_TERT),
    ("FUET", &MKB_TABLE_FUET),
    ("RBLT", &MKB_TABLE_RBLT),
    ("TKET", &MKB_TABLE_TKET),
    ("TKNT1", &MKB_TABLE_TKNT1),
    ("TKNT2", &MKB_TABLE_TKNT2),
    ("TKNT3", &MKB_TABLE_TKNT3),
    ("GPHT", &MKB_TABLE_GPHT),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table` の表。
static MKB_CURRENCY_MATERIAL_TABLE: &[(i64, &str)] = &[
    (2, "そのほかの材質"),
    (3, "人貨"),
    (4, "天貨"),
    (5, "獣貨"),
    (6, "紙幣"),
    (7, "金貨幣"),
    (8, "石貨"),
    (9, "食貨"),
    (10, "深貨"),
    (11, "魔貨"),
    (12, "そのほかの材質"),
];

/// Ruby `MeikyuKingdomBasic#mk_demand_table` の表。
static MKB_DEMAND_TABLE: &[(i64, &str)] = &[
    (1, "疫病の流行。「医薬品」か「著作物」なら〈予算〉を2MG獲得。「嗜好品」か「道具」なら維持費を1MG上昇。「資源」なら維持費を2MG上昇。"),
    (2, "近くで新しい王国が興る！「道具」か「資源」なら〈予算〉を1MG獲得。「嗜好品」か「著作物」なら維持費を1MG上昇。"),
    (3, "各国で供給過多。ものが溢れている！「嗜好品」なら〈予算〉を1MG獲得。「食品」なら維持費を1MG上昇。"),
    (4, "周囲の産業が順調。「食品」か「著作物」なら〈予算〉を2MG獲得。「嗜好品」か「道具」なら〈予算〉を1MG獲得。"),
    (5, "どこかで戦争の予感？「資源」か「医薬品」なら〈予算〉を2MG獲得。「食品」か「道具」か「資源」なら〈予算〉を1MG獲得。「著作物」なら維持費を2MG上昇。"),
    (6, "平穏な日々が続く。「嗜好品」なら〈予算〉を3MG獲得。「著作物」なら〈予算〉を2MG獲得。「食品」か「道具」か「資源」なら〈予算〉を1MG獲得。「医薬品」なら維持を1MG上昇。"),
];

/// Ruby `MeikyuKingdomBasic#mk_expedition_itinerary_table` の表。
static MKB_EXPEDITION_ITINERARY_TABLE: &[(i64, &str)] = &[
    (0, "侵略。あなたたちの王国を狙う勢力が、その不利な状況をついて襲撃してくる。周辺から発生箇所を3部屋、自国の【王宮】のある部屋の中から発生箇所を1部屋選ぶ。〔軍事レベル〕で難易度9の判定を行う。失敗すると、発生箇所が部屋ダメージを受ける。"),
    (1, "群れ。怪物の群れに遭遇する。恐ろしい数だ！周辺から発生箇所を4部屋選ぶ。〔軍事レベル〕で難易度［7+遠征道中表の使用回数］の判定を行う。失敗すると、発生箇所が部屋ダメージを受ける。"),
    (2, "暴動。遠征中、民の不満が爆発し、暴動が発生してしまう。自国の中から発生箇所をランダムに1部屋選ぶ。〔治安レベル〕で難易度［7+遠征道中表の使用回数］の判定を行う。失敗すると、発生箇所が部屋ダメージを受け、自国の《民》を［王国レベル］D6人減少する。"),
    (3, "遭難。奇妙な階域に迷い込んでしまう。元のルートに戻るためには準備が必要そうだ。自国の宮廷施設のある部屋の中から発生箇所を1部屋選ぶ。〔治安レベル〕で難易度9の判定を行う。失敗すると、目的地までの遠征距離を1D6点上昇する。"),
    (4, "疲労。長い遠征期間に栄養状態も悪くなってきている。自国の生産施設のある部屋の中から発生箇所を1部屋選ぶ。〔生活レベル〕で難易度9の判定を行う。失敗すると、各PCはランダムに変調1つを選ぶ。冒険フェイズが始まると、各PCは、その変調を受ける（「怒り」以外は、数値は3になる）。"),
    (5, "不和。遠征中、民たちの間で不満や怒りが爆発しそうになる。何か気晴らしが必要だろう。自国の娯楽施設のある部屋の中から発生箇所を1部屋選ぶ。〔文化レベル〕で難易度9の判定を行う。失敗すると、《民の声》を2点減少する。"),
    (6, "障害。進路上に幾つかの障害物を発見する。対処する必要があるだろう。周辺から発生箇所を2部屋選ぶ。〔治安レベル〕で難易度9の判定を行う。失敗すると、発生箇所が部屋ダメージを受ける。"),
    (7, "敵襲。階賊や敵国が襲撃してくる。応戦せよ！自国の中から発生箇所をランダムに1部屋選ぶ。〔軍事レベル〕で難易度9の判定を行う。成功すると、もう1度追加で「遠征道中表」を1回使用する（修正や持ち場は変更しない）。失敗すると、発生箇所が部屋ダメージを受ける。"),
    (8, "偵察。民から近道らしきルートを発見したという報告が起きる。自国の公共施設のある部屋の中から発生箇所を1部屋選ぶ。〔軍事レベル〕で難易度9の判定を行う。成功すると、PCの代表1人が「行動の処理」を1回行うか、遠征距離を1点減少する。"),
    (9, "事故。燃料に設定された素材を1D6個消費する。必要な数だけ消費できなかった場合、自国の中から発生箇所をランダムに1部屋選ぶ。発生箇所が部屋ダメージを受ける。"),
    (10, "交流。旅の空気のせいか。普段は言えないような思い切って踏み込んだ話をしてみる。自国のPCが2人以上いる部屋の中から発生箇所1部屋を選ぶ。発生箇所にいる各PCは、その部屋にいる自分以外のPC1人に対する《感情値》を1点上昇する。"),
    (11, "小冒険！ちょっとした挑戦！冒険の舞台に対応する好きな「特殊遭遇表」を1回使用する。その後、自国の燃料に指定されている素材1種を1D6個を獲得する。"),
    (12, "絶景。なんて景色だ！こんな場所は見たことがない！民たちは見たことのない光景に興奮している。《民の声》を1点回復し、PC全員は《気力》1点を回復する。"),
    (13, "出会い。遠征の途中、ひょんなことから1人の旅人を乗せることになる。自国の居住施設のある部屋の中から発生箇所を1部屋選ぶ。〔文化レベル〕で難易度9の判定を行う。成功すると、逸材を1人獲得する。「生まれ決定表」（基本p.24）を使って、ランダムにその逸材のジョブを決定すること。"),
    (14, "平穏な旅。何事もなく遠征が一区切りつき、民たちにも安堵の色がうかがえる。《民の声》を1点回復し、PC各自は「行動の処理」を1回行う。"),
];

/// Ruby `MeikyuKingdomBasic#mk_race_name_character_table` の表。
static MKB_RACE_NAME_CHARACTER_TABLE: &[(i64, &str)] = &[
    (202, "男"),
    (203, "木"),
    (204, "晶"),
    (205, "氷"),
    (206, "指"),
    (207, "銀"),
    (208, "舌"),
    (209, "鼠"),
    (210, "迷"),
    (211, "溶"),
    (212, "新"),
    (302, "音"),
    (303, "葉"),
    (304, "壁"),
    (305, "土"),
    (306, "首"),
    (307, "桃"),
    (308, "汁"),
    (309, "牛"),
    (310, "怒"),
    (311, "楽"),
    (312, "夕"),
    (402, "都"),
    (403, "花"),
    (404, "砂"),
    (405, "風"),
    (406, "鼻"),
    (407, "緑"),
    (408, "骨"),
    (409, "虎"),
    (410, "柔"),
    (411, "高"),
    (412, "春"),
    (502, "旅"),
    (503, "蔦"),
    (504, "門"),
    (505, "泉"),
    (506, "口"),
    (507, "黒"),
    (508, "毛"),
    (509, "兎"),
    (510, "寒"),
    (511, "太"),
    (512, "夏"),
    (602, "詩"),
    (603, "微"),
    (604, "高"),
    (605, "闇"),
    (606, "足"),
    (607, "黄"),
    (608, "牙"),
    (609, "猫"),
    (610, "早"),
    (611, "小"),
    (612, "秋"),
    (702, "食"),
    (703, "茸"),
    (704, "道"),
    (705, "星"),
    (706, "目"),
    (707, "白"),
    (708, "羽"),
    (709, "蛇"),
    (710, "嬉"),
    (711, "大"),
    (712, "冬"),
    (802, "剣"),
    (803, "苔"),
    (804, "壁"),
    (805, "天"),
    (806, "手"),
    (807, "青"),
    (808, "鱗"),
    (809, "羊"),
    (810, "長"),
    (811, "円"),
    (812, "朝"),
    (902, "鎧"),
    (903, "枝"),
    (904, "穴"),
    (905, "森"),
    (906, "耳"),
    (907, "茶"),
    (908, "尾"),
    (909, "猿"),
    (910, "熱"),
    (911, "細"),
    (912, "夜"),
    (1002, "幻"),
    (1003, "香"),
    (1004, "影"),
    (1005, "火"),
    (1006, "髪"),
    (1007, "赤"),
    (1008, "爪"),
    (1009, "鳥"),
    (1010, "硬"),
    (1011, "深"),
    (1012, "河"),
    (1102, "髭"),
    (1103, "草"),
    (1104, "柱"),
    (1105, "水"),
    (1106, "胸"),
    (1107, "紫"),
    (1108, "角"),
    (1109, "犬"),
    (1110, "笑"),
    (1111, "真"),
    (1112, "湖"),
    (1202, "女"),
    (1203, "蜜"),
    (1204, "丘"),
    (1205, "雪"),
    (1206, "尻"),
    (1207, "金"),
    (1208, "嘴"),
    (1209, "猪"),
    (1210, "埋"),
    (1211, "呪"),
    (1212, "古"),
];

/// Ruby `MeikyuKingdomBasic#mk_behavior_table` の表。
static MKB_BEHAVIOR_TABLE: &[(i64, &str)] = &[
    (11, "建国（の）"),
    (12, "休憩（の）"),
    (13, "捜索（の）"),
    (14, "攻撃（の）"),
    (15, "号令（の）"),
    (16, "集中（の）"),
    (22, "支援（の）"),
    (23, "移動（の）"),
    (24, "取引（の）"),
    (25, "交渉（の）"),
    (26, "視察（の）"),
    (33, "計画（の）"),
    (34, "建設（の）"),
    (35, "遭遇（の）"),
    (36, "協調行動（の）"),
    (44, "補助（の）"),
    (45, "情報収集（の）"),
    (46, "逃走（の）"),
    (55, "割り込み（の）"),
    (56, "貿易（の）"),
    (66, "滅亡（の）"),
];

/// Ruby `MeikyuKingdomBasic#mk_blood_decide_table` の表。
static MKB_BLOOD_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国は、鬼族の蹂躙を受けた歴史を持ち、混血が進んでいる。その国のキャラクターは新たにスキルを修得するとき、鬼族カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
    (2, "あなたの国は、古代に迷宮から姿を消した妖精女王の末裔といわれている。その国のキャラクターは新たにスキルを修得するとき、妖精カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
    (3, "あなたの国は、偉大なる古龍が迷宮と化した場所であり、その尊い血を引いているといわれる。その国のキャラクターは新たにスキルを修得するとき、魔獣カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
    (4, "あなたの国は、魔階からやってきた魔王の子供たちといわれている。その国のキャラクターは新たにスキルを修得するとき、異形カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
    (5, "あなたの国は、死霊術師によって死者の王国に変えられた悲劇的な過去を持つ。その国のキャラクターは新たにスキルを修得するとき、死霊カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
    (6, "あなたの国は、古代の錬金術師たちによって造られた人造生命が多数使役されている。その国のキャラクターは新たにスキルを修得するとき、呪物カテゴリの自分のレベル以下のモンスターが修得しているモンスタースキルの中から、修得するスキルを選ぶことができるようになる。"),
];

/// Ruby `MeikyuKingdomBasic#mk_terminology_table` の表。
static MKB_TERMINOLOGY_TABLE: &[(i64, &str)] = &[
    (11, "災厄／不幸（の）"),
    (12, "爆笑／哀切／憤怒（の）"),
    (13, "流星／疾風／光速（の）"),
    (14, "狂乱／暴虐／殺戮（の）"),
    (15, "稲妻／熱血（の）"),
    (16, "純白／灰色／漆黒（の）"),
    (22, "絶対／究極／至高（の）"),
    (23, "迷宮／世界／鉄（の）"),
    (24, "天階／深階（の）"),
    (25, "斬撃／打撃／衝撃（の）"),
    (26, "体制／革新（の）"),
    (33, "平和／神聖／自己防衛（の）"),
    (34, "男装／女装（の）"),
    (35, "マヨネーズ／じゃがいも（の）"),
    (36, "増税／権益（の）"),
    (44, "星術／召喚／科学（の）"),
    (45, "百獣／死神／異形（の）"),
    (46, "最強／万能（の）"),
    (55, "お祭り／平民／仮面（の）"),
    (56, "神秘／幻（の）"),
    (66, "未来／希望（の）"),
];

/// Ruby `MeikyuKingdomBasic#mk_rr236_name_table` の表。
static MKB_RR236_NAME_TABLE: &[(i64, &str)] = &[
    (11, "男／女"),
    (12, "詩人／俳人"),
    (13, "王／王者／女王"),
    (14, "閣下／殿下"),
    (15, "将軍／宰相"),
    (16, "画伯／絵師"),
    (22, "おじさん／おばさん"),
    (23, "暴れん坊／豪傑"),
    (24, "美丈夫／麗人"),
    (25, "哲人／賢者"),
    (26, "眼鏡"),
    (33, "イケメン／美女"),
    (34, "鉄人／巨人"),
    (35, "英雄／魔王"),
    (36, "支配者／主"),
    (44, "ヤンキー／ギャル"),
    (45, "頭脳／脳細胞"),
    (46, "戦士／ファイター"),
    (55, "父／母"),
    (56, "太夫／役者"),
    (66, "貴公子／令嬢"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table10` の表。
static MKB_CURRENCY_MATERIAL_TABLE10: &[(i64, &str)] = &[
    (1, "珊瑚"),
    (2, "貝殻"),
    (3, "鱗"),
    (4, "深人の骨"),
    (5, "甲羅"),
    (6, "塩"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table11` の表。
static MKB_CURRENCY_MATERIAL_TABLE11: &[(i64, &str)] = &[
    (1, "宗教的シンボル"),
    (2, "サイコロ"),
    (3, "人形"),
    (4, "生き血"),
    (5, "思い出の品"),
    (6, "迷核だったものの欠片"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table12` の表。
static MKB_CURRENCY_MATERIAL_TABLE12: &[(i64, &str)] = &[
    (1, "スプーンやフォーク"),
    (2, "お皿"),
    (3, "鍵"),
    (4, "布"),
    (5, "糸"),
    (6, "針"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table2` の表。
static MKB_CURRENCY_MATERIAL_TABLE2: &[(i64, &str)] = &[
    (1, "葉っぱ"),
    (2, "人の感情を結晶化したもの"),
    (3, "矢じり"),
    (4, "剣"),
    (5, "楽器"),
    (6, "眼鏡"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table3` の表。
static MKB_CURRENCY_MATERIAL_TABLE3: &[(i64, &str)] = &[
    (1, "美人"),
    (2, "農民"),
    (3, "職人"),
    (4, "役人"),
    (5, "戦士"),
    (6, "小鬼"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table4` の表。
static MKB_CURRENCY_MATERIAL_TABLE4: &[(i64, &str)] = &[
    (1, "羽毛"),
    (2, "皮膜"),
    (3, "くちばし"),
    (4, "天使の骨"),
    (5, "時計"),
    (6, "雪や氷"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table5` の表。
static MKB_CURRENCY_MATERIAL_TABLE5: &[(i64, &str)] = &[
    (1, "猫"),
    (2, "犬"),
    (3, "羊"),
    (4, "牛"),
    (5, "キンギョ"),
    (6, "ウマトカゲ"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table6` の表。
static MKB_CURRENCY_MATERIAL_TABLE6: &[(i64, &str)] = &[
    (1, "いわゆるお礼"),
    (2, "切手"),
    (3, "証文や契約書"),
    (4, "手紙"),
    (5, "宝くじ"),
    (6, "物語の断片"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table7` の表。
static MKB_CURRENCY_MATERIAL_TABLE7: &[(i64, &str)] = &[
    (1, "金貨"),
    (2, "銀貨"),
    (3, "銅貨"),
    (4, "鉄貨"),
    (5, "白金貨"),
    (6, "プルトニウム貨"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table8` の表。
static MKB_CURRENCY_MATERIAL_TABLE8: &[(i64, &str)] = &[
    (1, "巨大な石片"),
    (2, "陶貨"),
    (3, "輝石"),
    (4, "つるつるに磨いた小石"),
    (5, "変わった色の砂"),
    (6, "星の粒"),
];

/// Ruby `MeikyuKingdomBasic#mk_currency_material_table9` の表。
static MKB_CURRENCY_MATERIAL_TABLE9: &[(i64, &str)] = &[
    (1, "お米"),
    (2, "パン"),
    (3, "卵"),
    (4, "お酒"),
    (5, "バナナ"),
    (6, "マヨネーズ"),
];

/// Ruby `MeikyuKingdomBasic#mk_descriptive_table` の表。
static MKB_DESCRIPTIVE_TABLE: &[(i64, &str)] = &[
    (11, "闇に潜む"),
    (12, "遥かなる"),
    (13, "百年に一度の"),
    (14, "笑わない"),
    (15, "燃える"),
    (16, "彷徨う"),
    (22, "歌う"),
    (23, "破滅を呼ぶ"),
    (24, "人を喰った"),
    (25, "ついてない"),
    (26, "国民的"),
    (33, "美しき"),
    (34, "七つの顔を持つ"),
    (35, "眠れる"),
    (36, "優しい"),
    (44, "もっとも神に近い"),
    (45, "踊る"),
    (46, "虫愛ずる"),
    (55, "死せる"),
    (56, "早すぎる"),
    (66, "親愛なる"),
];

/// Ruby `MeikyuKingdomBasic#mk_facility_decide_table` の表。
static MKB_FACILITY_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国は、その地方を代々統治する伝統ある王国だ。宮廷系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
    (2, "あなたの国は、交易路の周囲にあり、多くの人々が流入する。居住系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
    (3, "あなたの国は、職人気質のものが多く、物作りがさかんだ。生産系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
    (4, "あなたの国は民を第一に考え、福祉に力を入れている。公共系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
    (5, "あなたの国は、歓楽国家として知られ、他国からの客もよく出入りしている。娯楽系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
    (6, "あなたの国は、辺境に位置する王国だ。周辺には怪物も少ない。保管系施設の中からランダムに1種を選ぶ。自国にその施設を1件建設する。"),
];

/// Ruby `MeikyuKingdomBasic#mk_guardian_constellation_table` の表。
static MKB_GUARDIAN_CONSTELLATION_TABLE: &[(i64, &str)] = &[
    (111, "小鬼（基本p171）"),
    (112, "小鬼（基本p171）"),
    (113, "毒小鬼（上級P92）"),
    (114, "毒小鬼（上級P92）"),
    (115, "小鬼呪術師（基本p171）"),
    (116, "小鬼呪術師（基本p171）"),
    (121, "犬頭（上級p92）"),
    (122, "犬頭（上級p92）"),
    (123, "鹿頭（基本p171）"),
    (124, "鹿頭（基本p171）"),
    (125, "牛頭（基本p172）"),
    (126, "牛頭（基本p172）"),
    (131, "巨鬼（基本P172）"),
    (132, "巨鬼（基本P172）"),
    (133, "雪男（上級p94）"),
    (134, "雪男（上級p94）"),
    (135, "崖鬼（上級p94）"),
    (136, "崖鬼（上級p94）"),
    (141, "赤頭巾（基本p171）"),
    (142, "赤頭巾（基本p171）"),
    (143, "天邪鬼（上級p94）"),
    (144, "天邪鬼（上級p94）"),
    (145, "鬼婆（基本p172）"),
    (146, "鬼婆（基本p172）"),
    (151, "羽妖精（基本p174）"),
    (152, "羽妖精（基本p174）"),
    (153, "シルキー（上級p98）"),
    (154, "シルキー（上級p98）"),
    (155, "妖精の愛人（上級p99）"),
    (156, "妖精の愛人（上級p99）"),
    (161, "猫の手（大冒険p89）"),
    (162, "猫の手（大冒険p89）"),
    (163, "鬼火（基本p174）"),
    (164, "鬼火（基本p174）"),
    (165, "ドワーフ（基本p174）"),
    (166, "ドワーフ（基本p174）"),
    (211, "花精（p141）"),
    (212, "花精（p141）"),
    (213, "眠りの精（基本p175）"),
    (214, "眠りの精（基本p175）"),
    (215, "紅目（大旨険p89）"),
    (216, "紅目（大旨険p89）"),
    (221, "バーンニク（上級p98）"),
    (222, "バーンニク（上級p98）"),
    (223, "ドモヴォーイ（基本p174）"),
    (224, "ドモヴォーイ（基本p174）"),
    (225, "レーシイ（基本p175）"),
    (226, "レーシイ（基本p175）"),
    (231, "大蝙蝠（基本p177）"),
    (232, "大蝙蝠（基本p177）"),
    (233, "鷲獅子（上級p102）"),
    (234, "鷲獅子（上級p102）"),
    (235, "ドラゴン（基本p181）"),
    (236, "ドラゴン（基本p181）"),
    (241, "豆狸（上級p101）"),
    (242, "豆狸（上級p101）"),
    (243, "ごんぎつね（基本p178）"),
    (244, "ごんぎつね（基本p178）"),
    (245, "大喰らい（基本p179）"),
    (248, "大喰らい（基本p179）"),
    (251, "ウマトカゲ（基本p178）"),
    (252, "ウマトカゲ（基本p178）"),
    (253, "群狼（基本p179）"),
    (254, "群狼（基本p179）"),
    (255, "一角獣（基本p180）"),
    (256, "一角獣（基本p180）"),
    (261, "血吸い蔓（基本p177）"),
    (262, "血吸い蔓（基本p177）"),
    (263, "どろぼう猫（基本p178）"),
    (264, "どろぼう猫（基本p178）"),
    (265, "樹木子（大冒険p93）"),
    (266, "樹木子（大冒険p93）"),
    (311, "二面人（基本p183）"),
    (312, "二面人（基本p183）"),
    (313, "鋏男（上級p105）"),
    (314, "鋏男（上級p105）"),
    (315, "吹雪魔神（大冒険p97）"),
    (316, "吹雪魔神（大冒険p97）"),
    (321, "箱入り娘（基本p183）"),
    (322, "箱入り娘（基本p183）"),
    (323, "二口女（基本p184）"),
    (324, "二口女（基本p184）"),
    (325, "ブレンギン（上級p106）"),
    (326, "ブレンギン（上級p106）"),
    (331, "腐ったたまご（基本p1B3）"),
    (332, "腐ったたまご（基本p1B3）"),
    (333, "マヨネーズキング（基本p1B4）"),
    (334, "マヨネーズキング（基本p1B4）"),
    (335, "メイクイーン（基本P1B4）"),
    (336, "メイクイーン（基本P1B4）"),
    (341, "大目玉（基本P184）"),
    (342, "大目玉（基本P184）"),
    (343, "人形師（基本p184）"),
    (344, "人形師（基本p184）"),
    (345, "蠍人（上級p107）"),
    (346, "蠍人（上級p107）"),
    (351, "お化けシーツ（基本p188）"),
    (352, "お化けシーツ（基本p188）"),
    (353, "影人（基本p189）"),
    (354, "影人（基本p189）"),
    (355, "人魂（上級p109）"),
    (356, "人魂（上級p109）"),
    (361, "歩き髑髏（基本p188）"),
    (362, "歩き髑髏（基本p188）"),
    (363, "墓暴き（基本p189）"),
    (364, "墓暴き（基本p189）"),
    (365, "塚人（基本p190）"),
    (366, "塚人（基本p190）"),
    (411, "僵尸（基本p189）"),
    (412, "僵尸（基本p189）"),
    (413, "首無し騎士（基本p189）"),
    (414, "首無し騎士（基本p189）"),
    (415, "木乃伊（蟇本p189）"),
    (416, "木乃伊（蟇本p189）"),
    (421, "歌い骨（基本p188）"),
    (422, "歌い骨（基本p188）"),
    (423, "わたわた（上級p109）"),
    (424, "わたわた（上級p109）"),
    (425, "死神（基本p190）"),
    (426, "死神（基本p190）"),
    (431, "カップメン（基本p192）"),
    (432, "カップメン（基本p192）"),
    (433, "バナナ暴徒（上級p113）"),
    (434, "バナナ暴徒（上級p113）"),
    (435, "パンマン（上級p115）"),
    (436, "パンマン（上級p115）"),
    (441, "吃驚絨毯（上級p113）"),
    (442, "吃驚絨毯（上級p113）"),
    (443, "生き金貨（基本p182）"),
    (444, "生き金貨（基本p182）"),
    (445, "魔剣（基本p193）"),
    (446, "魔剣（基本p193）"),
    (451, "土偶（上級p114）"),
    (452, "土偶（上級p114）"),
    (453, "ガーゴイル（基本p192）"),
    (454, "ガーゴイル（基本p192）"),
    (455, "ゴーレム（基本p193）"),
    (456, "ゴーレム（基本p193）"),
    (461, "禍福の骰子（上級p114）"),
    (462, "禍福の骰子（上級p114）"),
    (463, "迷い筆（上級p114）"),
    (464, "迷い筆（上級p114）"),
    (465, "巡り案山子（上級p114）"),
    (466, "巡り案山子（上級p114）"),
    (511, "人間の屑（基本P195）"),
    (512, "人間の屑（基本P195）"),
    (513, "毒婆（上級P118）"),
    (514, "毒婆（上級P118）"),
    (515, "蛮人（大冒険p109）"),
    (516, "蛮人（大冒険p109）"),
    (521, "傭兵（基本p195）"),
    (522, "傭兵（基本p195）"),
    (523, "極悪中隊（基本p196）"),
    (524, "極悪中隊（基本p196）"),
    (525, "抜け忍（基本P197）"),
    (526, "抜け忍（基本P197）"),
    (531, "ラストサムライ（基本P196）"),
    (532, "ラストサムライ（基本P196）"),
    (533, "穴人（基本P196）"),
    (534, "穴人（基本P196）"),
    (535, "無頭人（上級p118）"),
    (536, "無頭人（上級p118）"),
    (541, "漂流民（上級P118）"),
    (542, "漂流民（上級P118）"),
    (543, "稀人（基本p198）"),
    (544, "稀人（基本p198）"),
    (545, "天井人（上級p118）"),
    (546, "天井人（上級p118）"),
    (551, "マッハペンギン（基本p200）"),
    (552, "マッハペンギン（基本p200）"),
    (553, "天馬（上級p121）"),
    (554, "天馬（上級p121）"),
    (555, "バベル鳥（上級p122）"),
    (556, "バベル鳥（上級p122）"),
    (561, "白衣の天使（基本p201）"),
    (562, "白衣の天使（基本p201）"),
    (563, "妙音鳥（上級P122）"),
    (564, "妙音鳥（上級P122）"),
    (565, "羽根兜の乙女（基本p201）"),
    (566, "羽根兜の乙女（基本p201）"),
    (611, "クピド（上級p121）"),
    (612, "クピド（上級p121）"),
    (613, "季士（大冒険p113）"),
    (614, "季士（大冒険p113）"),
    (615, "三羽ガラス（基本p201）"),
    (616, "三羽ガラス（基本p201）"),
    (621, "エンゼルヘアー（上級p121）"),
    (622, "エンゼルヘアー（上級p121）"),
    (623, "紙鳶（大冒険p113）"),
    (624, "紙鳶（大冒険p113）"),
    (625, "雲神（基本p201）"),
    (626, "雲神（基本p201）"),
    (631, "鉄砲魚（基本p204）"),
    (632, "鉄砲魚（基本p204）"),
    (633, "植木魚（大冒険p118）"),
    (634, "植木魚（大冒険p118）"),
    (635, "河豚提灯（p141）"),
    (636, "河豚提灯（p141）"),
    (641, "エルフ（基本p205）"),
    (642, "エルフ（基本p205）"),
    (643, "人魚（上級p125）"),
    (644, "人魚（上級p125）"),
    (645, "イカロス（基本p205）"),
    (646, "イカロス（基本p205）"),
    (651, "河童（蟇本p205）"),
    (652, "河童（蟇本p205）"),
    (653, "魚人（基本p205）"),
    (654, "魚人（基本p205）"),
    (655, "鰐人（大冒険p119）"),
    (656, "鰐人（大冒険p119）"),
    (661, "水霊（上級p125）"),
    (662, "水霊（上級p125）"),
    (663, "海馬（上級p125）"),
    (664, "海馬（上級p125）"),
    (665, "まぼろし蛤（上級p125）"),
    (666, "まぼろし蛤（上級p125）"),
];

/// Ruby `MeikyuKingdomBasic#mk_human_resources_decide_table` の表。
static MKB_HUMAN_RESOURCES_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国には、高い見識を持つ知識人がいる。「才覚系生まれ表」でランダムにジョブ1種を選ぶ。そのジョブの逸材を1人獲得する。逸材の名前を決定すること。"),
    (2, "あなたの国には、皆を魅了する好人物がいる。「魅了系生まれ表」でランダムにジョブ1種を選ぶ。そのジョブの逸材を1人獲得する。逸材の名前を決定すること。"),
    (3, "あなたの国には、巧みな技術を持つ専門家がいる。「探索系生まれ表」でランダムにジョブ1種を選ぶ。そのジョブの逸材を1人獲得する。逸材の名前を決定すること。"),
    (4, "あなたの国には、見事な腕前の戦士がいる。「武勇系生まれ表」でランダムにジョブ1種を選ぶ。そのジョブの逸材を1人獲得する。逸材の名前を決定すること。"),
    (5, "あなたの国は、怪物と共存している？ （1D6）を振ること。1なら【小鬼】、2なら【ウマトカゲ】、3なら【ドワーフ】、4なら【エルフ】、5なら【キンギョ】、6なら【ごんぎつね】の《モンスターの民》を（1D6）人獲得する。"),
    (6, "あなたの国は、ここしばらく怪物や敵国の襲撃もなく、平和な日々が続いていた。《民》が（2D6）人増加する。"),
];

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_1_table` の表。
static MKB_KINGDOM_NAME_1_TABLE: &[(i64, &str)] = &[
    (
        11,
        "暗黒、クラヤミ、星灯り、カガヤキ、シャイニング、黄昏、暁",
    ),
    (12, "王政、帝政、爆発、回転"),
    (13, "超、スーパー、秘密主義、高等、ハイ、どん底"),
    (14, "共和制、立憲、公立、私立"),
    (15, "古代、新、ネオ、笑う、歌う"),
    (16, "共産、社会主義、自由、自由主義、ぶらり、ここは"),
    (22, "専制、民主主義、踊る、眠れる"),
    (23, "第三、最終、特別、標準"),
    (24, "神聖、聖、セント、絶対主義、現代、未来"),
    (25, "正統、本格、裏、偽、リバース、怪奇、幻想"),
    (26, "本家、元祖、荒ぶる、分かる"),
    (33, "大、グレート、小、スモール、必殺、淡麗"),
    (34, "天階、上、上方、深階、下、下方、異世界、現代"),
    (35, "東、東方、西、西方、かわいい、世界の"),
    (36, "北、北方、南、南方、赤い、緑の"),
    (44, "中央、辺境、飛び出せ、戦え"),
    (45, "独立、統一、ちはやぶる、八雲立つ"),
    (46, "永世、臨時、ザ、ラ"),
    (55, "さよなら、おはよう、太平、汎"),
    (56, "好戦的、平和的、素晴らしき、衝撃の"),
    (66, "優しい、怖い、ぼくらの、みんなの"),
];

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_2_table` の表。
static MKB_KINGDOM_NAME_2_TABLE: &[(i64, &str)] = &[
    (11, "凸凹、仄仄、子ども、大人"),
    (12, "迷宮、ダンジョン、監獄、封印、墓場"),
    (13, "グランドゼロ、エレベータ、コンパス、行き止まり"),
    (14, "サイコロ、ダイス、ランダム、ファンブル、ピンゾロ、シャッフル"),
    (15, "災厄、征服、無敵、野蛮"),
    (16, "デーモン、魔神、エンジェル、天使、超人、哲人"),
    (22, "野球、サッカー、クリケット、バドミントン"),
    (23, "商、武、科学、クラフト"),
    (24, "ドラゴン、龍、ヴァンパイア、吸血鬼、ヒューマン、人間、フェアリー、妖精"),
    (25, "猫、狼、キリン、キンギョ"),
    (26, "バナナ、ボルシチ、スシ、チーズ"),
    (33, "ファンタジー、魔法、マジカル、冒険、英雄"),
    (34, "大砲、刀剣、装甲、鉄拳"),
    (35, "ひきこもり、乙女、3D、転生"),
    (36, "崖っぷち、片隅、路地裏、炎上"),
    (44, "電脳、浪漫、蒸気、退廃"),
    (45, "コンプライアンス、ダイバーシティ、アグリー、ウィンウィン"),
    (46, "ローマ、中華、エジプト、アステカ"),
    (55, "(単語表1で決定)、(単語表2で決定)、(単語表3で決定)、(単語表4で決定)"),
    (56, "(プレイ会場の地名)、(GMの出身地)、(この表を使用した者の住所)、(GMの苗字)"),
    (66, "(国王の名前。後で決定)、(国王のジョブ名。後で決定)、(ランダムな武具アイテム名)、(ランダムな日常アイテム名)"),
];

/// Ruby `MeikyuKingdomBasic#mk_kingdom_name_3_table` の表。
static MKB_KINGDOM_NAME_3_TABLE: &[(i64, &str)] = &[
    (11, "王国、キングダム、王朝"),
    (12, "会社、公社、本舗"),
    (13, "学園、学校、食堂"),
    (14, "汗国、国"),
    (15, "合衆国、政府"),
    (16, "共同体、共和国"),
    (22, "司教国、教皇領"),
    (23, "星、伯国"),
    (24, "公国、大公国"),
    (25, "市、シティ、ポリス、都、のみやこ"),
    (26, "自治国、騎士団領"),
    (33, "植民地、統一機構"),
    (34, "帝国、皇国"),
    (35, "同盟、機関"),
    (36, "首長国、土侯国"),
    (44, "幕府、藩王国"),
    (45, "領、クラブ"),
    (46, "村、町、街"),
    (55, "横丁、亭、社中"),
    (56, "ランド、戦線"),
    (66, "連邦、連合"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_ar_table` の表。
static MKB_NAME_AR_TABLE: &[(i64, &str)] = &[
    (11, "コーラス／メロディ"),
    (12, "シタール／コト"),
    (13, "トロンボーン／ティンパニ"),
    (14, "マーチ／セレナーデ"),
    (15, "ソロ／オーケストラ"),
    (16, "パッソ／プリマ"),
    (22, "モノローグ／アポローズ"),
    (23, "スクリプト／カメリーノ"),
    (24, "アール／エピカ"),
    (25, "ラインズ／ムジカ"),
    (26, "トルバドール／リリカ"),
    (33, "ノベル／ラマーン"),
    (34, "クリーミ／ストーリア"),
    (35, "エッセイ／メモワール"),
    (36, "ジャケット／コロフォン"),
    (44, "デビュー／セーヌ"),
    (45, "タンゴ／バル"),
    (46, "イーゼル／パレット"),
    (55, "カンバス／タトゥー"),
    (56, "ウッドカット／キラーミカ"),
    (66, "ポートレイト／パノラマ"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_dn_table` の表。
static MKB_NAME_DN_TABLE: &[(i64, &str)] = &[
    (11, "ファイバー／シルク"),
    (12, "ジーンズ／キュロット"),
    (13, "ガーター／ソックス"),
    (14, "クラヴァッテ／スカーフ"),
    (15, "サンダル／ハイヒール"),
    (16, "リング／ブローチ"),
    (22, "ボタン／リカーモ"),
    (23, "シュピーゲル／ルージュ"),
    (24, "オーデコロン／マニキュア"),
    (25, "シルクハット／サリー"),
    (26, "ソープ／コーム"),
    (33, "スツール／ソファー"),
    (34, "ブランケット／マクラ"),
    (35, "ケトル／ポット"),
    (36, "ゲイト／ポーチ"),
    (44, "ギムレット／レンチ"),
    (45, "シェイヴァー／シャンプー"),
    (46, "タオル／マスカラ"),
    (55, "クローゼット／クッション"),
    (56, "カウチ／クリップ"),
    (66, "スタンプ／カレンダー"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_ex_table` の表。
static MKB_NAME_EX_TABLE: &[(i64, &str)] = &[
    (11, "モアイ／スイショウドクロ"),
    (12, "チュパカブラ／ムベンベ"),
    (13, "カンフー／インヤン"),
    (14, "ブシドー／ミヤコ"),
    (15, "チャンピオン／バービー"),
    (16, "ウパニシャッド／ゾルゲ"),
    (22, "デスマーチ／インテル"),
    (23, "ゴッホ／ヴィクトリア"),
    (24, "ゾンビ／オニャンコポン"),
    (25, "ケロッパ／カルメン"),
    (26, "オーバーキル／サシミ"),
    (33, "ブッチャー／デヴィ"),
    (34, "ブロンソン／マドンナ"),
    (35, "ガイギャックス／エロイカ"),
    (36, "好きな星の名前(スピカ,オリオン)"),
    (44, "好きな武器の名前(エペ,フランベルジュ)"),
    (45, "好きな動物の名前(イタチ,パグ)"),
    (46, "好きな鉱物の名前(ルビィ,ヒスイ)"),
    (55, "好きな言葉＋ドラゴン"),
    (56, "好きな単語表で決定する"),
    (66, "プレイヤーと同じ名前"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_fo_table` の表。
static MKB_NAME_FO_TABLE: &[(i64, &str)] = &[
    (11, "ダージリン／マンデリン"),
    (12, "コニャック／ピーノ"),
    (13, "グラス／テキーラ"),
    (14, "ハングオーバー／リキュール"),
    (15, "ブレッド／プレッツェル"),
    (16, "バケット／コロネ"),
    (22, "クロワッサン／ヤムチャ"),
    (23, "ヤキソバ／パッタイ"),
    (24, "ニョッキ／ペンネ"),
    (25, "ハニー／メイプル"),
    (26, "ガトー／フラン"),
    (33, "ジュレ／ソルベ"),
    (34, "リゾット／チマキ"),
    (35, "フリット／テンプラ"),
    (36, "カルビ／ハラミ"),
    (44, "ポージョ／ユーリンチー"),
    (45, "アイスバイン／イベリコ"),
    (46, "ブルスト／キシュカ"),
    (55, "ドリアン／キウィ"),
    (56, "ココ／プラム"),
    (66, "ガリガリ／ポテチ"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_go_table` の表。
static MKB_NAME_GO_TABLE: &[(i64, &str)] = &[
    (11, "ケルヌンノス／アリアンロッド"),
    (12, "ジーザス／マリア"),
    (13, "ブッダ／スジャータ"),
    (14, "ゼウス／ヘラ"),
    (15, "シヴァ／パールヴァティ"),
    (16, "マルス／ミネルヴァ"),
    (22, "スサノオ／ウズメ"),
    (23, "バンコ／ジョカ"),
    (24, "インティ／パチャママ"),
    (25, "ダグザ／モリガン"),
    (26, "バロン／ランダ"),
    (33, "アヌビス／バステト"),
    (34, "ジャンゴ／アナンシ"),
    (35, "トラロック／コアトリクエ"),
    (36, "バアル／アシュタルテ"),
    (44, "アフラマズダ／アムルタート"),
    (45, "ベロボーグ／モコシ"),
    (46, "エンキ／イナンナ"),
    (55, "オーディン／フレイヤ"),
    (56, "ココペリ／ココペルマナ"),
    (66, "クトゥルフ／ハイドラ"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_ma_table` の表。
static MKB_NAME_MA_TABLE: &[(i64, &str)] = &[
    (11, "ウォッチ／シーナ"),
    (12, "アンテナ／テレ"),
    (13, "グリル／バティドーラ"),
    (14, "ステレオ／カリヨン"),
    (15, "マキナ／アルマ"),
    (16, "ロケット／ヴィルタリオート"),
    (22, "ルー／フラン"),
    (23, "モーター／モトーレ"),
    (24, "ドライラート／コーチェ"),
    (25, "クロック／セニャーレ"),
    (26, "ポンプ／アントリア"),
    (33, "スケイルズ／プランチャ"),
    (34, "ランプ／シャンデリア"),
    (35, "ガジエラ／カノン"),
    (36, "リフト／エクレール"),
    (44, "ナルキ／プランタ"),
    (45, "サカプンタス／アーラ"),
    (46, "シュレッダー／ナウス"),
    (55, "ファブリーク／ユジーヌ"),
    (56, "ボイラー／カルダイヤ"),
    (66, "エンジン／トリシクル"),
];

/// Ruby `MeikyuKingdomBasic#mk_name_pl_table` の表。
static MKB_NAME_PL_TABLE: &[(i64, &str)] = &[
    (11, "シアトル／ヴァージニア"),
    (12, "デーン／ヴァーサ"),
    (13, "タイガ／ユルガ"),
    (14, "クルスク／トゥール"),
    (15, "アラド／モルダヴィア"),
    (16, "キエフ／ユークレイン"),
    (22, "ウガンダ／ガーナ"),
    (23, "ギザ／アレクサンドリア"),
    (24, "キリマンジャロ／ソマリ"),
    (25, "ガイアナ／リオ"),
    (26, "イグアス／アマゾン"),
    (33, "サンティアゴ／ナスカ"),
    (34, "クーロン／シャンハイ"),
    (35, "ベナレス／デリー"),
    (36, "バリ／セイロン"),
    (44, "ティモール／スマトラ"),
    (45, "トリノ／シチリア"),
    (46, "バスク／グラナダ"),
    (55, "キプロス／クレタ"),
    (56, "ザクセン／ケルン"),
    (66, "リヨン／ニース"),
];

/// Ruby `MeikyuKingdomBasic#mk_national_style_decide_table` の表。
static MKB_NATIONAL_STYLE_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国は、古くからあり、伝統を重んじる気風を持つ。宮廷系施設を建設・発展するための価格が1MG軽減される（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
    (2, "あなたの国は、広い国土と高い天井に恵まれている。居住系施設を建設するための価格が1MG軽減される（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
    (3, "あなたの国は、夏星が豊富で、作物がたくさん収穫できる。生産系施設を建設・発展するための価格が1MG軽減される。（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
    (4, "あなたの国は、しっかりとした規律と礼節があり、それを守る風潮がある。公共系施設を建設・発展するための価格が1MG軽減される（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
    (5, "あなたの国は、芸術を奨励し、文化的な国民性を誇る。娯楽系施設を建設・発展するための価格が1MG軽減される（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
    (6, "あなたの国は、物を大切にし、質素な生活を心がける気風を持つ。保管系施設を建設・発展するための価格が1MG軽減される（最大2MGまで軽減される。3回目以降は振り直すこと）。"),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_pr_table` の表。
static MKB_NICK_PR_TABLE: &[(i64, &str)] = &[
    (11, "“九死に一生を得る”"),
    (12, "“風前の灯火の”"),
    (13, "“類は友を呼ぶ”"),
    (14, "“性格がいい方の”"),
    (15, "“三階に家なき”"),
    (16, "“五分の理はある”"),
    (22, "“危ない橋を渡る”"),
    (23, "“バカって言った方がバカの”"),
    (24, "“長いものに巻かれる”"),
    (25, "“火の無いところの”"),
    (26, "“あばたもえくぼの”"),
    (33, "“将を射んとせばまず”"),
    (34, "“氷山の一角の”"),
    (35, "“木乃伊取りが木乃伊になる”"),
    (36, "“一見の価値ありの”"),
    (44, "“一日の長ある”"),
    (45, "“遠くの親類より近くの”"),
    (46, "“笑う門には福来る”"),
    (55, "“花は桜木、人は”"),
    (56, "“猫に小判の”"),
    (
        66,
        "“（クラス名／ジョブ名）による（クラス名／ジョブ名）のための”",
    ),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_co_table` の表。
static MKB_NICK_CO_TABLE: &[(i64, &str)] = &[
    (11, "“（王国名／氷）の牙”"),
    (12, "“（王国名／不可視）の猟犬”"),
    (13, "“（王国名／暴虐）の女神”"),
    (14, "“（王国名／無限）の境界”"),
    (15, "“（王国名／偽り）の救世主”"),
    (16, "“（王国名／闇）の扉”"),
    (22, "“（王国名／暁）の凶星”"),
    (23, "“（王国名／災禍）の中心”"),
    (24, "“（王国名／始まり）の記憶”"),
    (25, "“（王国名／絶対）の歌声”"),
    (26, "“（王国名／星霜）の死神”"),
    (33, "“（王国名／不確定）の隠者”"),
    (34, "“（王国名／冥府）の番人”"),
    (35, "“（王国名／深淵）の使途”"),
    (36, "“（王国名／罪）の華”"),
    (44, "“（王国名／終末）の翼”"),
    (45, "“（王国名／絶望）の匠”"),
    (46, "“（王国名／鮮血）の芸術家”"),
    (55, "“（王国名／流星）の魔剣”"),
    (56, "“（王国名／漆黒）の堕天使”"),
    (66, "“（王国名／無貌）の悪夢”"),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_fo_table` の表。
static MKB_NICK_FO_TABLE: &[(i64, &str)] = &[
    (11, "“自画自賛（の）”"),
    (12, "“人畜無害（の）”"),
    (13, "“不言実行（の）”"),
    (14, "“痛快無比（の）”"),
    (15, "“外柔内剛（の）”"),
    (16, "“百戦錬磨（の）”"),
    (22, "“前代未聞（の）”"),
    (23, "“粉骨砕身（の）”"),
    (24, "“天真爛漫（の）”"),
    (25, "“暴飲暴食（の）”"),
    (26, "“意志薄弱（の）”"),
    (33, "“慇懃無礼（の）”"),
    (34, "“沈魚落雁（の）”"),
    (35, "“波乱万丈（の）”"),
    (36, "“二束三文（の）”"),
    (44, "“行雲流水（の）”"),
    (45, "“驚天動地（の）”"),
    (46, "“破邪顕正（の）”"),
    (55, "“以心伝心（の）”"),
    (56, "“博覧強記（の）”"),
    (66, "“殺人事件（の）”"),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_ou_table` の表。
static MKB_NICK_OU_TABLE: &[(i64, &str)] = &[
    (11, "“もふもふの”"),
    (12, "“裸の”"),
    (13, "“猫耳の”"),
    (14, "“歩くと音がする”"),
    (15, "“緑髪の”"),
    (16, "“黄金（の）”"),
    (22, "“羽根つき（の）”"),
    (23, "“小さな”"),
    (24, "“蛇手の”"),
    (25, "“鉤シッポの”"),
    (26, "“ぎざぎざの”"),
    (33, "“輝ける”"),
    (34, "“角持ち（の）”"),
    (35, "“とんがり帽子の”"),
    (36, "“青ざめた”"),
    (44, "“赤目の”"),
    (45, "“黒衣の”"),
    (46, "“ねじれ声の”"),
    (55, "“銀の腕”"),
    (56, "“長靴下の”"),
    (66, "“ぬるぬるの”"),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_ph_table` の表。
static MKB_NICK_PH_TABLE: &[(i64, &str)] = &[
    (11, "“世界が嫉妬する”"),
    (12, "“うまい、うますぎる”"),
    (13, "“２４時間戦える”"),
    (14, "“脱いでもすごい”"),
    (15, "“ピカピカの１年生”"),
    (16, "“どうあがいても絶望の”"),
    (22, "“ダメ絶対の”"),
    (23, "“すべての王国を過去にする”"),
    (24, "“１００人乗っても大丈夫な”"),
    (25, "“綺麗なおねえさんが好きな”"),
    (26, "“食う寝る遊ぶの”"),
    (33, "“かわいいは正義の”"),
    (34, "“それにつけても”"),
    (35, "“お口の恋人”"),
    (36, "“やめられない止まらない”"),
    (44, "“半分はやさしさの”"),
    (45, "“国民的美少女”"),
    (46, "“プライスレスの”"),
    (55, "“驚きの白さの”"),
    (56, "“楽器のマークの”"),
    (66, "“パンツじゃないから恥ずかしくない”"),
];

/// Ruby `MeikyuKingdomBasic#mk_nick_ti_table` の表。
static MKB_NICK_TI_TABLE: &[(i64, &str)] = &[
    (11, "“（王国名）の星”"),
    (12, "“（王国名）の独眼竜”"),
    (13, "“（王国名）の麒麟児”"),
    (14, "“（王国名）の虎”"),
    (15, "“（王国名）のマムシ”"),
    (16, "“（王国名）１Ｄ６天王”"),
    (22, "“（王国名）１Ｄ６傑”"),
    (23, "“（王国名）１Ｄ６銃士”"),
    (24, "“（王国名）１０＋１Ｄ６神将”"),
    (25, "“（王国名）２Ｄ６（兄弟／姉妹）”"),
    (26, "“（王国名）２Ｄ６賢人”"),
    (33, "“あの（クラス名／ジョブ名）”"),
    (34, "“最後の（クラス名／ジョブ名）”"),
    (35, "“メカ（クラス名／ジョブ名）”"),
    (36, "“殺人（クラス名／ジョブ名）”"),
    (44, "“カリスマ（クラス名／ジョブ名）”"),
    (45, "“超級（クラス名／ジョブ名）”"),
    (46, "“攻め（クラス名／ジョブ名）”"),
    (55, "“スタイリッシュ（クラス名／ジョブ名）”"),
    (56, "“大（クラス名／ジョブ名）”"),
    (66, "“鬼（クラス名／ジョブ名）”"),
];

/// Ruby `MeikyuKingdomBasic#mk_resource_decide_table` の表。
static MKB_RESOURCE_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国は、過去に善政がしかれ、非常に安定している。セッション開始時の《民の声》の値が1点上昇する（最大3点まで上昇する。4回目以降は振り直すこと）。"),
    (2, "あなたの国は、天然の要害に囲まれており、外敵に襲われにくい。《民》が（2D6）人増加する。"),
    (3, "あなたの国には、名工がつくった武器がある。ランダムに選んだ武具アイテム1個を獲得する。その武具アイテムはレベル1として扱う。"),
    (4, "あなたの国には、先頃友誼を誓い合った同盟国がある。王国シートの周辺階域から、ランダムに未知の土地1つを選ぶ。その土地に、王国を1つ設定すること。この国は【特産物】を持つ。「相場表」を使って、【特産物】の素材をランダムに決定すること。この国との関係は「同盟」となる。"),
    (5, "あなたの国で先頃、前王の隠し財産が発見された。《予算》を（1D6） MG獲得する。"),
    (6, "あなたの国には、隠し扉があった。「自国の地理」を決定したあと、追加で通路を2本引くことができる。通路でつながっている部屋は領土として扱う。"),
];

/// Ruby `MeikyuKingdomBasic#mk_technic_decide_table` の表。
static MKB_TECHNIC_DECIDE_TABLE: &[(i64, &str)] = &[
    (1, "あなたの国は、魔法の研究、開発に力をそそぐ魔道国家である。その国のキャラクターは、星術、召喚、科学スキルの判定を行うとき、その達成値が1点上昇する（最大3点まで上昇する。4回目以降は振り直すこと）。"),
    (2, "あなたの国は、神話的遺物の逸話が残っている。レア一般アイテムの中からランダムに1種を選ぶ。そのレアアイテムのレシピを持っている。【王宮】のある部屋に、そのレア一般アイテムの名前を記入すること。"),
    (3, "あなたの国は、英雄が用いた武具の伝説が残っている。レア武具アイテムの中からランダムに1種を選ぶ。そのレアアイテムのレシピを持っている。【王宮】のある部屋に、そのレア武具アイテムの名前を記入すること。"),
    (4, "あなたの国は、有名な職人たちが揃う工業国家である。コモンアイテムを作成するとき、それらのアイテムを作成するための必要国力が1点高いものとして扱う。"),
    (5, "あなたの国は、質実剛健な兵士たちが揃っている。その国のキャラクターは、《配下》最大値が1人上昇する（最大2人まで上昇する。3回目以降は振り直すこと）。"),
    (6, "あなたの国は、過去に列強に臣従し、いまでも友好的な関係を築いている。（1D6）を振ること。1ならダイナマイト帝国、2なら千年王朝、3ならメトロ汗国、4ならハグルマ資本主義神聖共和国との関係が「友好」になる。5や6なら振り直すこと。また、その列強の列強系施設1軒を獲得する。"),
];

/// Ruby `MeikyuKingdomBasic#mk_title_table` の表。
static MKB_TITLE_TABLE: &[(i64, &str)] = &[
    (11, "お坊ちゃま／お嬢様"),
    (12, "申し子／落とし子"),
    (13, "神／神様"),
    (14, "天才／異端児"),
    (15, "爆弾／爆発"),
    (16, "超人"),
    (22, "紳士／乙女"),
    (23, "恐怖／悪夢"),
    (24, "天使／悪魔"),
    (25, "魂／権化"),
    (26, "神童／新星"),
    (33, "王子／王女"),
    (34, "劇場／舞台"),
    (35, "伝説／道"),
    (36, "鬼／怪物"),
    (44, "君／姫"),
    (45, "帝／皇帝／女皇帝"),
    (46, "名人／達人"),
    (55, "信者／教徒"),
    (56, "業者／界隈"),
    (66, "斬り／殺し"),
];

/// Ruby `MeikyuKingdomBasic#mk_word_1_table` の表。
static MKB_WORD_1_TABLE: &[(i64, &str)] = &[
    (11, "魔法"),
    (12, "おめかし"),
    (13, "狭いところ"),
    (14, "夜更かし"),
    (15, "節約"),
    (16, "会議"),
    (22, "ヒゲ"),
    (23, "孤独"),
    (24, "説教"),
    (25, "自分探し"),
    (26, "異性"),
    (33, "ヒラヒラした服"),
    (34, "平穏な生活"),
    (35, "自分語り"),
    (36, "風呂"),
    (44, "古いもの"),
    (45, "頭が悪い人"),
    (46, "暗闇"),
    (55, "許嫁"),
    (56, "民"),
    (66, "バカ"),
];

/// Ruby `MeikyuKingdomBasic#mk_word_2_table` の表。
static MKB_WORD_2_TABLE: &[(i64, &str)] = &[
    (11, "科学"),
    (12, "読書"),
    (13, "広いところ"),
    (14, "早起き"),
    (15, "ムダ"),
    (16, "仕事"),
    (22, "陰謀"),
    (23, "みんなで集まること"),
    (24, "ナンパ"),
    (25, "昔話"),
    (26, "同性"),
    (33, "武器の手入れ"),
    (34, "戦争"),
    (35, "人の噂"),
    (36, "散髪"),
    (44, "新しいもの"),
    (45, "頭がよい人"),
    (46, "光"),
    (55, "親"),
    (56, "外国人"),
    (66, "ホラ話"),
];

/// Ruby `MeikyuKingdomBasic#mk_word_3_table` の表。
static MKB_WORD_3_TABLE: &[(i64, &str)] = &[
    (11, "子供"),
    (12, "弱い人"),
    (13, "処刑"),
    (14, "叙事詩"),
    (15, "煙草"),
    (16, "病院"),
    (22, "演説"),
    (23, "酒盛り"),
    (24, "料理"),
    (25, "武芸"),
    (26, "田舎"),
    (33, "自分の国"),
    (34, "伝統"),
    (35, "お祭り"),
    (36, "告げ口"),
    (44, "自分の声"),
    (45, "マヨネーズ"),
    (46, "おせっかい"),
    (55, "猫"),
    (56, "混沌"),
    (66, "占い"),
];

/// Ruby `MeikyuKingdomBasic#mk_word_4_table` の表。
static MKB_WORD_4_TABLE: &[(i64, &str)] = &[
    (11, "年寄り"),
    (12, "強い人"),
    (13, "空想"),
    (14, "冗談"),
    (15, "クスリ"),
    (16, "怪物"),
    (22, "一騎打ち"),
    (23, "賭け事"),
    (24, "歌"),
    (25, "勉強"),
    (26, "都会"),
    (33, "冒険"),
    (34, "ダイナマイト大帝"),
    (35, "盗み"),
    (36, "言い訳"),
    (44, "隣のキャラのジョブ"),
    (45, "小鬼"),
    (46, "謝ること"),
    (55, "隣のキャラのクラス"),
    (56, "星"),
    (66, "肉"),
];
/// Ruby `神力決定表`（`ITEM_POWER_TABLE`）の項目。
static MKB_ITEM_POWER_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("才覚"),
    FeatureItem::Text("魅力"),
    FeatureItem::Text("探索"),
    FeatureItem::Text("武勇"),
    FeatureItem::Text("《器》"),
    FeatureItem::Text("《回避値》"),
];

/// Ruby `ITEM_POWER_TABLE`（`1D6`）。
static MKB_ITEM_POWER_TABLE: ItemFeature = ItemFeature::new(1, 6, MKB_ITEM_POWER_TABLE_ITEMS);

/// Ruby `呪紋決定表`（`ITEM_JYUMON_TABLE`）の項目。
static MKB_ITEM_JYUMON_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("ランダムに選んだモンスターカテゴリ1種のうち、【人類の敵】未習得かつ（2D6）レベル以下のモンスタースキル"),
    FeatureItem::Text("便利スキルから好きなスキル1種"),
    FeatureItem::Text("芸能スキルから好きなスキル1種"),
    FeatureItem::Text("迷宮スキルから好きなスキル1種"),
    FeatureItem::Text("星術スキルから好きなスキル1種"),
    FeatureItem::Text("一般スキルから好きなスキル1種"),
    FeatureItem::Text("召喚スキルから好きなスキル1種"),
    FeatureItem::Text("科学スキルから好きなスキル1種"),
    FeatureItem::Text("交渉スキルから好きなスキル1種"),
    FeatureItem::Text("神官のクラススキルから好きなスキル1種"),
    FeatureItem::Text("生まれ表を使用してランダムに決めたジョブのジョブスキル"),
];

/// Ruby `ITEM_JYUMON_TABLE`（`2D6`）。
static MKB_ITEM_JYUMON_TABLE: ItemFeature = ItemFeature::new(2, 6, MKB_ITEM_JYUMON_TABLE_ITEMS);

/// Ruby `呪禍表`（`ITEM_JYUKA_TABLE`）の項目。
static MKB_ITEM_JYUKA_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("このアイテムを装備している限り「呪い3」の変調を受ける"),
    FeatureItem::Text("このアイテムを装備している限り「肥満3」の変調を受ける"),
    FeatureItem::Text("このアイテムを装備している限り「憤怒」の変調を受ける"),
    FeatureItem::Text("このアイテムを装備している限り「毒2」の変調を受ける"),
    FeatureItem::Text("このアイテムを装備している限り、条件を満たしても誰とも人間関係を結べない"),
    FeatureItem::Text("このアイテムを装備している限り「散漫1」の変調を受ける"),
];

/// Ruby `ITEM_JYUKA_TABLE`（`1D6`）。
static MKB_ITEM_JYUKA_TABLE: ItemFeature = ItemFeature::new(1, 6, MKB_ITEM_JYUKA_TABLE_ITEMS);

/// Ruby `性別決定表`（`GENDER_TABLE`）の項目。
static MKB_GENDER_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("性別が男であること"),
    FeatureItem::Text("性別が女であること"),
    FeatureItem::Text("性別が男であること"),
    FeatureItem::Text("性別が女であること"),
    FeatureItem::Text("性別が男であること"),
    FeatureItem::Text("性別が女であること"),
];

/// Ruby `GENDER_TABLE`（`1D6`）。
static MKB_GENDER_TABLE: ItemFeature = ItemFeature::new(1, 6, MKB_GENDER_TABLE_ITEMS);

/// Ruby `適正表`（`ITEM_APTITUDE_TABLE`）の項目。
static MKB_ITEM_APTITUDE_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("ランダムなクラス1種であること"),
    FeatureItem::Text("生まれ表でランダムに選んだジョブであること"),
    FeatureItem::Feature(&MKB_GENDER_TABLE),
    FeatureItem::Text("上級ジョブであること"),
    FeatureItem::Text("モンスタースキルを修得していること"),
    FeatureItem::Text("童貞、もしくは処女であること"),
];

/// Ruby `ITEM_APTITUDE_TABLE`（`1D6`）。
static MKB_ITEM_APTITUDE_TABLE: ItemFeature = ItemFeature::new(1, 6, MKB_ITEM_APTITUDE_TABLE_ITEMS);

/// Ruby `属性表`（`ITEM_ATTRIBUTE_TABLE`）の項目。
static MKB_ITEM_ATTRIBUTE_TABLE_ITEMS: &[FeatureItem] = &[
    FeatureItem::Text("自然の力。10レベル以下の人間カテゴリのモンスターと重要NPC"),
    FeatureItem::Text("幻夢の力。10レベル以下の異形と呪物カテゴリのモンスター"),
    FeatureItem::Text("星炎の力。10レベル以下の魔獣と鬼族カテゴリのモンスター"),
    FeatureItem::Text("暗黒の力。10レベル以下の妖精と天使カテゴリのモンスター"),
    FeatureItem::Text("聖なる力。10レベル以下の死霊と深人カテゴリのモンスター"),
    FeatureItem::Text("災厄の力。支配者として設定されているキャラクター"),
];

/// Ruby `ITEM_ATTRIBUTE_TABLE`（`1D6`）。
static MKB_ITEM_ATTRIBUTE_TABLE: ItemFeature =
    ItemFeature::new(1, 6, MKB_ITEM_ATTRIBUTE_TABLE_ITEMS);

/// Ruby `ItemFeaturesTable#@items`（`2D6` の各行）。
static MKB_ITEM_FEATURES_ROWS: &[&[FeatureRowItem]] = &[
    &[
        FeatureRowItem::Text("そのアイテムは「"),
        FeatureRowItem::Feature(&MKB_ITEM_POWER_TABLE),
        FeatureRowItem::Text("」の神力を宿す。"),
    ],
    &[
        FeatureRowItem::Text("そのアイテムは寿命を持つ。寿命の値を決定する。\nさらに、"),
        FeatureRowItem::SelfRef,
    ],
    &[FeatureRowItem::Text(
        "そのアイテムは境界障壁を持つ。《HP》の値を決定する。",
    )],
    &[FeatureRowItem::Text(
        "そのアイテムは銘を持つ。銘を決定する。",
    )],
    &[
        FeatureRowItem::Text("そのアイテムは合成具である。もう1つの機能は「"),
        FeatureRowItem::Table(&MKB_ITEM_RANDOM_TABLE),
        FeatureRowItem::Text("」である。"),
    ],
    &[
        FeatureRowItem::Text(
            "そのアイテムにレベルがあれば、レベルを1点上昇する。\nレベルが設定されていなければ、",
        ),
        FeatureRowItem::SelfRef,
    ],
    &[
        FeatureRowItem::Text("そのアイテムは「"),
        FeatureRowItem::Feature(&MKB_ITEM_JYUMON_TABLE),
        FeatureRowItem::Text("」の呪紋を持つ。"),
    ],
    &[
        FeatureRowItem::Text("そのアイテムは「"),
        FeatureRowItem::Feature(&MKB_ITEM_JYUKA_TABLE),
        FeatureRowItem::Text("」の呪禍を持つ。\nさらに、"),
        FeatureRowItem::SelfRef,
    ],
    &[FeatureRowItem::Text(
        "そのアイテムは高価だ。価格を設定する。",
    )],
    &[
        FeatureRowItem::Text("そのアイテムは「条件："),
        FeatureRowItem::Feature(&MKB_ITEM_APTITUDE_TABLE),
        FeatureRowItem::Text("」の適正を持つ。\nさらに、"),
        FeatureRowItem::SelfRef,
    ],
    &[
        FeatureRowItem::Text("そのアイテムは「"),
        FeatureRowItem::Feature(&MKB_ITEM_ATTRIBUTE_TABLE),
        FeatureRowItem::Text("」の属性を持つ。"),
    ],
];

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
            .join("test/data/MeikyuKingdomBasic.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MeikyuKingdomBasic.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MeikyuKingdomBasic.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MeikyuKingdomBasic.toml must parse");
        assert_eq!(
            data.tests.len(),
            373,
            "case count in test/data/MeikyuKingdomBasic.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MeikyuKingdomBasic",
                "unexpected game system in MeikyuKingdomBasic.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("MeikyuKingdomBasic"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL MeikyuKingdomBasic:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MeikyuKingdomBasic cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
