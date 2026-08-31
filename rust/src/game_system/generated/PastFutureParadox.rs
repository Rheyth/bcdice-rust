//! P4で手書き移植した `lib/bcdice/game_system/PastFutureParadox.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`
//!   （`action_roll` → `RTT.roll_command` → `roll_tables` → `roll_table_command` の順）
//! - `#action_roll`（行為判定 `PP@s#f[+m/-m]>=x`）
//! - `#roll_table_command` / `#get_table_result` / `#get_table_index`
//!   / `#get_table_minus_index`（出目指定 `=n` と修正値 `+n` / `-n` に対応する表）
//! - `RTT`（ランダム指定特技表 / `DiceTable::SaiFicSkillTable`）
//! - `TABLES`（D66表）と `TABLES_MOD_2D` / `TABLES_MOD_1D` / `TABLES_MOD_MINUS`

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::sai_fic_skill_table::{DEFAULT_RCT_FORMAT, DEFAULT_RTTN_FORMAT};
use crate::dice_table::{
    D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table, TableItem,
};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::PastFutureParadox`（ID: `PastFutureParadox`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PastFutureParadox;

/// Ruby `register_prefix(RTT.prefixes, TABLES.keys, ...)` が登録した接頭辞。
static PREFIXES: &[&str] = &[
    "PP",
    "RTT[1-6]?",
    "RCT",
    "CT1",
    "CT2",
    "CT3",
    "CT4",
    "CT5",
    "CT6",
    "CTD",
    "CTG",
    "CTFR",
    "CTFM",
    "CTY",
    "CTM",
    "CTJ",
    "CTF",
    "CTR",
    "CTAL",
    "CTAM",
    "CTAD",
    "NMTD",
    "NFTD",
    "NMTGG",
    "NFTGG",
    "NMTGJ",
    "NFTGJ",
    "NMTGN",
    "NFTGN",
    "NMTGE",
    "NFTGE",
    "NMT2C",
    "NMTFR",
    "NFTFR",
    "NLTFR",
    "NMTFM",
    "NFTFM",
    "NMTY",
    "NFTY",
    "NMTM",
    "NFTM",
    "NLTM",
    "NMT4J",
    "NFT4J",
    "NLT4J",
    "NMT4F",
    "NFT4F",
    "NLT4F",
    "NPTR",
    "NMTR",
    "NNTR",
    "NMTAL",
    "NFTAL",
    "NMTAM",
    "NFTAM",
    "NMTAD",
    "NFTAD",
    "NMT1",
    "NFT1",
    "NMT2",
    "NFT2",
    "NLT2",
    "NMT3",
    "NFT3",
    "NLT3",
    "NMT3W",
    "NFT3W",
    "NLT3W",
    "NMT4",
    "NFT4",
    "NLT4",
    "NMT4W",
    "NFT4W",
    "NLT4W",
    "NMT5",
    "NFT5",
    "NLT5",
    "NMT6",
    "NFT6",
    "NLT6",
    "CCT",
    "ACT",
    "ST1A",
    "ST1B",
    "ST1C",
    "ST2A",
    "ST2B",
    "ST2C",
    "ST3A",
    "ST3B",
    "ST3C",
    "ST4",
    "ST4A",
    "ST4B",
    "ST4C",
    "ST5A",
    "ST5B",
    "ST5C",
    "ST6A",
    "ST6B",
    "ST6C",
    "ST6S",
    "ST0",
    "ST7A",
    "ST7B",
    "TT",
    "RT",
    "CPT",
    "CNT",
    "IT",
    "AGT",
    "MCT",
    "SCT",
    "BCT",
    "AMCT",
    "ASCT1",
    "ASCT2",
    "CTT0",
    "CTT1",
    "CTT2",
    "CTT3",
    "CTT4",
    "CTT5",
    "CTT6",
    "CTT7",
    "CTTD",
    "CTT4M",
    "NTT0",
    "NTT1",
    "NTT2",
    "NTT3",
    "NTT4",
    "NTT5",
    "NTT6",
    "NTT7",
    "NTTG",
    "NTTD",
    "NMT2J",
    "NFT2J",
    "NMTGM",
    "NFTGM",
    "NMTGI",
    "NFTGI",
    "SBET",
    "MBET",
    "TBET",
];

impl GameSystem for PastFutureParadox {
    fn id(&self) -> &'static str {
        "PastFutureParadox"
    }

    fn name(&self) -> &'static str {
        "パストフューチャーパラドックス"
    }

    fn sort_key(&self) -> &'static str {
        "はすとふゆうちやあはらとつくす"
    }

    fn help_message(&self) -> &'static str {
        r"◇行為判定 PP@s#f[+m/-m]>=x 　2D6の行為判定を行う。
　　s: スペシャル値 (省略時 12)、 f: ファンブル値 (省略時 2)
　　[+m/-m]: 修正値（省略可）、 x: 目標値 (省略可)
　　　例）PP, PP-1, PP@11, PP@11+2, PP@11#3, PP@11#3-1,
　　　　　PP#3>=7, PP#3+2>=7, PP>=7, PP-1>=7
◇D66ダイスあり
◇各種表
※1D6および2D6を振る表は、末尾に=nと付けることで出目nの内容を指定可能。
　また、末尾に-n／+nと付けることで、出目に修正を付けることが可能。
　　例：SBET=2　MBET-1　TBET+2
・特技表
　　ランダム分野表 RCT
　　ランダム特技表 RTTn（n：分野番号、省略時は全分野からランダム）
　　　科学　（RTT1）、知識（RTT2）、身体（RTT3）、
　　　センス（RTT4）、知恵（RTT5）、迷信（RTT6）
・因縁関連表
　　因縁種別表 CCT
　　ポジティブ因縁内容表 CPT
　　ネガティブ因縁内容表 CNT
・バタフライエフェクト表
　　※バタフライエフェクト表は-5～12までの結果を算出可能
　　重度バタフライエフェクト表 SBET
　　軽度バタフライエフェクト表 MBET
　　タイムトラベラー重度バタフライエフェクト表 TBET
・セッション進行用
　　アクシデント表 ACT
　　タイムトラベル演出表 TT
　　帰還演出表 RT
　　アイテム決定表 IT
　　時代決定表 AGT
・シーン表
　　原始時代（リアル原始）シーン表 ST1A
　　原始時代（恐竜と人類）シーン表 ST1B
　　原始時代（恐竜人文明）シーン表 ST1C
　　古代（リアル古代）シーン表 ST2A
　　古代（都市伝説文明）シーン表 ST2B
　　古代（天上の神々）シーン表 ST2C
　　中世時代（リアル中世）シーン表 ST3A
　　中世時代（妖と陰陽道）シーン表 ST3B
　　中世時代（剣と魔法）シーン表 ST3C
　　現代シーン表 ST4
　　現代シーン表2 ST4A
　　現代（近代日本）シーン表 ST4B
　　現代（西部開拓時代）シーン表 ST4C
　　超情報化時代（ユートピア）シーン表 ST5A
　　超情報化時代（ディストピア）シーン表 ST5B
　　超情報化時代（サイバーパンク）シーン表 ST5C
　　宇宙時代（地球人類銀河帝国）シーン表 ST6A
　　宇宙時代（異形の隣人たち）シーン表 ST6B
　　宇宙時代（遺棄された地球）シーン表 ST6C
　　宇宙時代（宇宙船船内）シーン表 ST6D
　　開闢時代シーン表 ST0
　　終局時代シーン表 ST7A
　　終局時代（無限図書館）シーン表 ST7B
・クラス決定表
　　メインクラス決定表 MCT
　　サブクラス決定表 SCT
　　基本クラス表 BCT
　　追加クラス（メインクラス専用）表 AMCT
　　追加クラス（サブクラス専用）表1 ASCT1
　　追加クラス（サブクラス専用）表2 ASCT2
・経歴表決定表
　　開闢時代経歴表決定表 CTT0
　　原始時代経歴表決定表 CTT1
　　古代経歴表決定表 CTT2
　　中世時代経歴表決定表 CTT3
　　現代経歴表決定表 CTT4
　　超情報化時代経歴表決定表 CTT5
　　宇宙時代経歴表決定表 CTT6
　　終局経歴表決定表 CTT7
　　亜人経歴表決定表 CTTD
　　近代人経歴表決定表 CTT4M
・経歴表
　　原始時代経歴表 CT1
　　古代経歴表 CT2
　　中世時代経歴表 CT3
　　現代経歴表 CT4
　　超情報化時代経歴表 CT5
　　宇宙時代経歴表 CT6
　　恐竜人経歴表 CTD
　　天界人経歴表 CTG
　　亜人（ハイファンタジー種族）経歴表 CTFR
　　亜人（ハイファンタジー魔物）経歴表 CTFM
　　亜人（妖怪）経歴表 CTY
　　亜人（ミュータント）経歴表 CTM
　　近代人（明治・大正・昭和）経歴表 CTJ
　　近代人（西部開拓時代）経歴表 CTF
　　機械人経歴表 CTR
　　異星人経歴表 CTAL
　　軟体人経歴表 CTAM
　　高次元人経歴表 CTAD
・名前表決定表
　　開闢時代名前表決定表 NTT0
　　原始時代名前表決定表 NTT1
　　古代名前表決定表 NTT2
　　中世時代名前表決定表 NTT3
　　現代名前表決定表 NTT4
　　超情報化時代名前表決定表 NTT5
　　宇宙時代名前表決定表 NTT6
　　終局時代名前表決定表 NTT7
　　天界人名前表決定表 NTTG
　　亜人名前表決定表 NTTD
・名前表
　　原始時代名前表／男性名 NMT1
　　原始時代名前表／女性名 NFT1
　　古代名前表／男性名 NMT2
　　古代名前表／女性名 NFT2
　　古代名前表／姓 NLT2
　　中世時代（日本）名前表／男性名 NMT3
　　中世時代（日本）名前表／女性名 NFT3
　　中世時代（日本）名前表／姓 NLT3
　　中世時代（西洋）名前表／男性名 NMT3W
　　中世時代（西洋）名前表／女性名 NFT3W
　　中世時代（西洋）名前表／姓 NLT3W
　　現代（日本）名前表／男性名 NMT4
　　現代（日本）名前表／女性名 NFT4
　　現代（日本）名前表／姓 NLT4
　　現代（西洋）名前表／男性名 NMT4W
　　現代（西洋）名前表／女性名 NFT4W
　　現代（西洋）名前表／姓 NLT4W
　　超情報化時代名前表／男性名 NMT5
　　超情報化時代名前表／女性名 NFT5
　　超情報化時代名前表／姓 NLT5
　　宇宙時代名前表／男性名 NMT6
　　宇宙時代名前表／女性名 NFT6
　　宇宙時代名前表／姓 NLT6
　　恐竜人名前表／男性名 NMTD
　　恐竜人名前表／女性名 NFTD
　　天界人（ギリシャ神話）名前表／男性名 NMTGG
　　天界人（ギリシャ神話）名前表／女性名 NFTGG
　　天界人（日本神話）名前表／男性名 NMTGJ
　　天界人（日本神話）名前表／女性名 NFTGJ
　　天界人（北欧神話）名前表／男性名 NMTGN
　　天界人（北欧神話）名前表／女性名 NFTGN
　　天界人（エジプト神話）名前表／男性名 NMTGE
　　天界人（エジプト神話）名前表／女性名 NFTGE
　　天界人（メソポタミア神話）名前表／男性名 NMTGM
　　天界人（メソポタミア神話）名前表／女性名 NFTGM
　　天界人（インド神話）名前表／男性名 NMTGI
　　天界人（インド神話）名前表／女性名 NFTGI
　　古代（日本）名前表／男性名 NMT2J
　　古代（日本）名前表／女性名 NFT2J
　　古代（中国）名前表 NMT2C　（※この表を2回か3回振った結果を繋げる）
　　亜人（ハイファンタジー種族）名前表／男性名 NMTFR
　　亜人（ハイファンタジー種族）名前表／女性名 NFTFR
　　亜人（ハイファンタジー種族）名前表／姓  NLTFR
　　亜人（ハイファンタジー魔物）名前表／男性名 NMTFM
　　亜人（ハイファンタジー魔物）名前表／女性名 NFTFM
　　亜人（妖怪）名前表／男性名 NMTY
　　亜人（妖怪）名前表／女性名 NFTY
　　亜人（ミュータント）名前表／男性名 NMTM
　　亜人（ミュータント）名前表／女性名 NFTM
　　亜人（ミュータント）名前表／姓 NLTM
　　近代人（明治・大正・昭和）名前表／男性名 NMT4J
　　近代人（明治・大正・昭和）名前表／女性名 NFT4J
　　近代人（明治・大正・昭和）名前表／姓 NLT4J
　　近代人（西部開拓時代）名前表／男性名 NMT4F
　　近代人（西部開拓時代）名前表／女性名 NFT4F
　　近代人（西部開拓時代）名前表／姓 NLT4F
　　機械人名前表／プレフィックス NPTR
　　機械人名前表／型番 NMTR
　　機械人名前表／愛称 NNTR
　　異星人名前表／男性名 NMTAL
　　異星人名前表／女性名 NFTAL
　　軟体人名前表／男性名 NMTAM
　　軟体人名前表／女性名 NFTAM
　　高次元人名前表／男性名 NMTAD
　　高次元人名前表／女性名 NFTAD
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `#initialize` の `@d66_sort_type = D66SortType::ASC`。
    ///
    /// 同じく設定される `@sort_add_dice = false` / `@sort_barabara_dice = false` は
    /// トレイトの既定値と同じなので上書きしない。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `#eval_game_system_specific_command`。
    ///
    /// Ruby: `action_roll(command) || RTT.roll_command(@randomizer, command)
    ///        || roll_tables(command, TABLES) || roll_table_command(command)`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(out) = action_roll(command, rng)? {
            return Ok(Some(out));
        }
        if let Some(text) = RTT.roll_command(rng, command)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        // Ruby の `roll_table_command` は該当表がなければ空文字列を返し、
        // `Base#dice_command` がそれを nil に畳む（＝出力なし・ダイスも振らない）。
        Ok(Some(SpecificCommandOutput::text(roll_table_command(
            command, rng,
        )?)))
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(table) = TABLES.iter().find(|(k, _)| *k == command).map(|(_, t)| *t) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `#action_roll`。2D6の行為判定。
fn action_roll(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Command::Parser.new("PP", round_type: round_type)
    //         .restrict_cmp_op_to(:>=, nil).enable_critical.enable_fumble
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["PP"], RoundType::Floor)
            .restrict_cmp_op_to(&[Some(CmpOp::Ge), None])
            .enable_critical()
            .enable_fumble()
    });

    let Some(mut cmd) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: cmd.critical ||= 12 / cmd.fumble ||= 2
    let critical = cmd
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(12);
    let fumble = cmd
        .fumble
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(2);
    cmd.critical = Some(crate::Int::from(critical));
    cmd.fumble = Some(crate::Int::from(fumble));

    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_total: i64 = dice_list.iter().sum();
    let total = dice_total + cmd.modify_number.clone();

    let mut result = if dice_total <= fumble {
        EvalResult::fumble("ファンブル(判定失敗。改変度を1D6点増加してバタフライエフェクト発生)")
    } else if dice_total >= critical {
        EvalResult::critical("スペシャル(判定成功。疲労度を1D6点減少してバタフライエフェクト発生)")
    } else if cmd.cmp_op.is_none() {
        // Ruby `Result.new`（text は nil）
        EvalResult::new()
    } else {
        // cmp_op があれば Parser が必ず目標値も埋める
        let target = cmd.target_number.clone().unwrap_or(crate::Int::from(0));
        if total >= target {
            EvalResult::success("成功")
        } else {
            EvalResult::failure("失敗")
        }
    };

    let mut sequence = vec![
        format!("({})", cmd.to_s(SuffixPosition::AfterModifyNumber)),
        format!(
            "{dice_total}[{}]{}",
            join_dice(&dice_list),
            modifier(&cmd.modify_number)
        ),
        total.to_string(),
    ];
    // Ruby: `[...].compact` は `Result.new` の nil な text だけを落とす
    if !result.text.is_empty() {
        sequence.push(std::mem::take(&mut result.text));
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `#roll_table_command` の `/([A-Za-z0-9]+)(([+]|-|=)((-\d+)|\d+))?/`。
///
/// アンカーが無いので `CT6a` は `CT6a` 全体が、`ST4=-10` は `ST4` と `=` と `-10` が取れる。
fn table_command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([A-Za-z0-9]+)(([+]|-|=)((-\d+)|\d+))?").expect("valid regex"))
}

/// Ruby `#roll_table_command`。
///
/// Ruby は該当表がないと空配列を `join("\n")` して空文字列を返す。
fn roll_table_command(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let Some(m) = table_command_pattern().captures(command) else {
        return Ok(String::new());
    };

    let name = m.get(1).map_or("", |x| x.as_str());
    let operator = m.get(3).map(|x| x.as_str());
    // Ruby: `m[4].to_i`（nil なら 0）。i64 に収まらない桁数は飽和させる
    // （どのみち直後の `clamp` で表の範囲へ丸められる）。
    let value = m.get(4).map_or(0, |x| parse_saturating(x.as_str()));

    Ok(get_table_result(name, operator, value, rng)?.unwrap_or_default())
}

/// Ruby `String#to_i` の代用。i64 の範囲外は飽和させる。
fn parse_saturating(s: &str) -> i64 {
    s.parse().unwrap_or(if s.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `#get_table_result`。3つの表ハッシュを順に引く。
fn get_table_result(
    command: &str,
    operator: Option<&str>,
    value: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    if let Some(table) = find_table(TABLES_MOD_2D, command) {
        return Ok(Some(get_table_index(table, operator, value, 2, 6, rng)?));
    }
    if let Some(table) = find_table(TABLES_MOD_1D, command) {
        return Ok(Some(get_table_index(table, operator, value, 1, 6, rng)?));
    }
    if let Some(table) = find_table(TABLES_MOD_MINUS, command) {
        return Ok(Some(get_table_minus_index(table, operator, value, rng)?));
    }
    Ok(None)
}

/// Ruby の `Hash#[]` 相当。
fn find_table(tables: &'static [(&str, &'static Table)], command: &str) -> Option<&'static Table> {
    tables.iter().find(|(k, _)| *k == command).map(|(_, t)| *t)
}

/// Ruby `#get_table_index`。出目の合計をそのまま添字に使う表。
fn get_table_index(
    table: &Table,
    operator: Option<&str>,
    value: i64,
    dice_count: i64,
    dice_type: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    // Ruby: index.clamp(dice_count * 1, dice_count * dice_type)
    let min = dice_count;
    let max = dice_count * dice_type;

    if operator == Some("=") {
        let info = table.choice(value.clamp(min, max));
        return Ok(format!(
            "{}:{} ＞ {}:{}",
            info.table_name(),
            info.value(),
            info.value(),
            info.body()
        ));
    }

    let modify = modify_of(operator, value);
    let dice_list = rng.roll_barabara(dice_count, dice_type)?;
    let sum: i64 = dice_list.iter().sum();
    let info = table.choice(sum.saturating_add(modify).clamp(min, max));

    Ok(format_with_dice(
        &info,
        info.value(),
        sum,
        &dice_list,
        operator,
        modify,
    ))
}

/// Ruby `#get_table_minus_index`。出目 -5〜12 を返すバタフライエフェクト表。
///
/// 添字は `7 + 2D6 + 修正値`（2〜19にクランプ）で、表示する値は添字から7を引いたもの。
fn get_table_minus_index(
    table: &Table,
    operator: Option<&str>,
    value: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    if operator == Some("=") {
        let info = table.choice(value.saturating_add(7).clamp(2, 19));
        let shown = info.value() - 7;
        return Ok(format!(
            "{}:{} ＞ {}:{}",
            info.table_name(),
            shown,
            shown,
            info.body()
        ));
    }

    let modify = modify_of(operator, value);
    let dice_list = rng.roll_barabara(2, 6)?;
    let sum: i64 = dice_list.iter().sum();
    let index = 7i64.saturating_add(sum).saturating_add(modify).clamp(2, 19);
    let info = table.choice(index);

    Ok(format_with_dice(
        &info,
        info.value() - 7,
        sum,
        &dice_list,
        operator,
        modify,
    ))
}

/// Ruby の `case operator when "+" then value when "-" then value * -1 else 0`。
fn modify_of(operator: Option<&str>, value: i64) -> i64 {
    match operator {
        Some("+") => value,
        Some("-") => value.saturating_neg(),
        _ => 0,
    }
}

/// Ruby `get_table_index` / `get_table_minus_index` のダイスを振った側の出力。
fn format_with_dice(
    info: &crate::dice_table::RollResult,
    shown: i64,
    sum: i64,
    dice_list: &[i64],
    operator: Option<&str>,
    modify: i64,
) -> String {
    let dice = join_dice(dice_list);
    if modify != 0 {
        format!(
            "{}:{sum}[{dice}]{}{} ＞ {shown}:{}",
            info.table_name(),
            operator.unwrap_or(""),
            modify.unsigned_abs(),
            info.body()
        )
    } else {
        format!(
            "{}:{sum}[{dice}] ＞ {shown}:{}",
            info.table_name(),
            info.body()
        )
    }
}

/// Ruby `dice_list.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// 表データ（以下の `static` 群）は `lib/bcdice/game_system/PastFutureParadox.rb` から
// 機械的に書き出したもので、値は1文字も変えていない。

// ---------------------------------------------------------------------------
// Ruby `RTT`（ランダム指定特技表 / SaiFicSkillTable）
// ---------------------------------------------------------------------------

/// Ruby `RTT` の分野「科学」の特技（2D6）。
static RTT_SKILLS1: &[&str] = &[
    "人工知能",
    "計算機",
    "電子工学",
    "機械工学",
    "物理学",
    "数学",
    "天文学",
    "地学",
    "化学",
    "医療",
    "バイオ技術",
];
/// Ruby `RTT` の分野「知識」の特技（2D6）。
static RTT_SKILLS2: &[&str] = &[
    "帝王学",
    "経済",
    "政治",
    "社会",
    "法律",
    "情報",
    "労働",
    "教育",
    "歴史",
    "語学",
    "文学",
];
/// Ruby `RTT` の分野「身体」の特技（2D6）。
static RTT_SKILLS3: &[&str] = &[
    "狙う",
    "斬る",
    "殴る",
    "跳ぶ",
    "走る",
    "避ける",
    "柔軟",
    "持ち上げる",
    "食べる",
    "飲む",
    "叫ぶ",
];
/// Ruby `RTT` の分野「センス」の特技（2D6）。
static RTT_SKILLS4: &[&str] = &[
    "魔法",
    "超能力",
    "第六感",
    "宗教",
    "倫理",
    "観察",
    "我慢",
    "操縦",
    "哲学",
    "心理",
    "芸術",
];
/// Ruby `RTT` の分野「知恵」の特技（2D6）。
static RTT_SKILLS5: &[&str] = &[
    "戦略",
    "方便",
    "機転",
    "洞察力",
    "記憶力",
    "段取り",
    "応急処置",
    "漢方",
    "胆力",
    "勘",
    "人徳",
];
/// Ruby `RTT` の分野「迷信」の特技（2D6）。
static RTT_SKILLS6: &[&str] = &[
    "思い込み",
    "インチキ",
    "未確認物体",
    "雨乞い",
    "風水",
    "占い",
    "縁起",
    "魔除け",
    "心霊",
    "運命",
    "民間伝承",
];

/// Ruby `RTT` の分野リスト（1D6の出目順）。
static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("科学", RTT_SKILLS1),
    SaiFicCategory::new("知識", RTT_SKILLS2),
    SaiFicCategory::new("身体", RTT_SKILLS3),
    SaiFicCategory::new("センス", RTT_SKILLS4),
    SaiFicCategory::new("知恵", RTT_SKILLS5),
    SaiFicCategory::new("迷信", RTT_SKILLS6),
];

/// Ruby `RTT = DiceTable::SaiFicSkillTable.new(..., s_format:, rtt_format:)`。
static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES).with_formats(SaiFicFormats {
    rtt: "ランダム指定特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
    rct: DEFAULT_RCT_FORMAT,
    rttn: DEFAULT_RTTN_FORMAT,
    skill: "分野「%<category_name>s」《%<skill_name>s》",
});

// ---------------------------------------------------------------------------
// Ruby `TABLES`（D66表 / `roll_tables` が引く）
// ---------------------------------------------------------------------------

/// Ruby `TABLES["CT1"]`（原始時代経歴表）の項目。
static CT1_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("狩猟者")),
    (12, TableItem::Text("石器職人")),
    (13, TableItem::Text("毛皮職人")),
    (14, TableItem::Text("骨加工職人")),
    (15, TableItem::Text("木工職人")),
    (16, TableItem::Text("木こり")),
    (22, TableItem::Text("採集者")),
    (23, TableItem::Text("法螺貝吹き")),
    (24, TableItem::Text("集落の守り手")),
    (25, TableItem::Text("食糧管理者")),
    (26, TableItem::Text("石板職人")),
    (33, TableItem::Text("部族戦士")),
    (34, TableItem::Text("恐竜騎手")),
    (35, TableItem::Text("恐竜学者")),
    (36, TableItem::Text("恐竜飼育員")),
    (44, TableItem::Text("まじない師")),
    (45, TableItem::Text("花摘み")),
    (46, TableItem::Text("乱暴者")),
    (55, TableItem::Text("族長")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("長老")),
];
static CT1: D66Table = D66Table::new("原始時代経歴表", D66SortType::Asc, CT1_ITEMS);
/// Ruby `TABLES["CT2"]`（古代経歴表）の項目。
static CT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("祭司")),
    (12, TableItem::Text("陶工")),
    (13, TableItem::Text("金細工職人")),
    (14, TableItem::Text("貿易商")),
    (15, TableItem::Text("墓守")),
    (16, TableItem::Text("壁画画家")),
    (22, TableItem::Text("哲学者")),
    (23, TableItem::Text("彫刻家")),
    (24, TableItem::Text("刻印師")),
    (25, TableItem::Text("ガラス職人")),
    (26, TableItem::Text("粘土職人")),
    (33, TableItem::Text("魔法研究者")),
    (34, TableItem::Text("神官戦士")),
    (35, TableItem::Text("木工職人")),
    (36, TableItem::Text("踊り子")),
    (44, TableItem::Text("予言者")),
    (45, TableItem::Text("墓荒らし")),
    (46, TableItem::Text("カルト教祖")),
    (55, TableItem::Text("超科学者")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("薬師")),
];
static CT2: D66Table = D66Table::new("古代経歴表", D66SortType::Asc, CT2_ITEMS);
/// Ruby `TABLES["CT3"]`（中世時代経歴表）の項目。
static CT3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("農夫")),
    (12, TableItem::Text("鍛冶屋")),
    (13, TableItem::Text("商人")),
    (14, TableItem::Text("宿屋の主人")),
    (15, TableItem::Text("盗賊")),
    (16, TableItem::Text("御者")),
    (22, TableItem::Text("騎士・武士")),
    (23, TableItem::Text("兵士・衛士")),
    (24, TableItem::Text("錬金術師")),
    (25, TableItem::Text("羊飼い")),
    (26, TableItem::Text("音楽家")),
    (33, TableItem::Text("貴族")),
    (34, TableItem::Text("画家")),
    (35, TableItem::Text("道化師")),
    (36, TableItem::Text("町医者")),
    (44, TableItem::Text("神職")),
    (45, TableItem::Text("王族")),
    (46, TableItem::Text("占い師")),
    (55, TableItem::Text("魔術師")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("勇者")),
];
static CT3: D66Table = D66Table::new("中世時代経歴表", D66SortType::Asc, CT3_ITEMS);
/// Ruby `TABLES["CT4"]`（現代経歴表）の項目。
static CT4_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("医師")),
    (12, TableItem::Text("弁護士")),
    (13, TableItem::Text("教師")),
    (14, TableItem::Text("エンジニア")),
    (15, TableItem::Text("ミュージシャン")),
    (16, TableItem::Text("イラストレーター")),
    (22, TableItem::Text("経営者")),
    (23, TableItem::Text("秘書")),
    (24, TableItem::Text("パイロット")),
    (25, TableItem::Text("銀行員")),
    (26, TableItem::Text("テレビマン")),
    (33, TableItem::Text("営業")),
    (34, TableItem::Text("作家")),
    (35, TableItem::Text("ジャーナリスト")),
    (36, TableItem::Text("俳優")),
    (44, TableItem::Text("警察官")),
    (45, TableItem::Text("消防士")),
    (46, TableItem::Text("ギャンブラー")),
    (55, TableItem::Text("ギャング")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("学生")),
];
static CT4: D66Table = D66Table::new("現代経歴表", D66SortType::Asc, CT4_ITEMS);
/// Ruby `TABLES["CT5"]`（超情報化時代経歴表）の項目。
static CT5_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ブレードランナー")),
    (12, TableItem::Text("ドローンパイロット")),
    (13, TableItem::Text("ロボット工学者")),
    (14, TableItem::Text("データサイエンティスト")),
    (15, TableItem::Text("VRゲームデザイナー")),
    (16, TableItem::Text("原子力工学技術者")),
    (22, TableItem::Text("AIエンジニア")),
    (23, TableItem::Text("脳科学者")),
    (24, TableItem::Text("環境工学エンジニア")),
    (25, TableItem::Text("セキュリティエンジニア")),
    (26, TableItem::Text("オペレーター")),
    (33, TableItem::Text("SNSインフルエンサー")),
    (34, TableItem::Text("プログラマ")),
    (35, TableItem::Text("ハードウェアエンジニア")),
    (36, TableItem::Text("ネットワークエンジニア")),
    (44, TableItem::Text("サイバネエンジニア")),
    (45, TableItem::Text("メガコーポ役員")),
    (46, TableItem::Text("個人発信メディア運営")),
    (55, TableItem::Text("サイバーウェアドクター")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("ハッカー")),
];
static CT5: D66Table = D66Table::new("超情報化時代経歴表", D66SortType::Asc, CT5_ITEMS);
/// Ruby `TABLES["CT6"]`（宇宙時代経歴表）の項目。
static CT6_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("宇宙飛行士")),
    (12, TableItem::Text("テラフォーミング技術者")),
    (13, TableItem::Text("軌道エレベーターガール")),
    (14, TableItem::Text("宇宙船メカニック")),
    (15, TableItem::Text("宇宙物理学者")),
    (16, TableItem::Text("超光速通信技術者")),
    (22, TableItem::Text("恒星間密輸業者")),
    (23, TableItem::Text("ダークマター技術者")),
    (24, TableItem::Text("宇宙警察")),
    (25, TableItem::Text("宇宙ニンジャ")),
    (26, TableItem::Text("銀河レーサー")),
    (33, TableItem::Text("銀河の騎士")),
    (34, TableItem::Text("銀河スパイ")),
    (35, TableItem::Text("銀河行商人")),
    (36, TableItem::Text("銀河放浪者")),
    (44, TableItem::Text("元老院議員")),
    (45, TableItem::Text("銀河帝国機動歩兵")),
    (46, TableItem::Text("銀河皇帝")),
    (55, TableItem::Text("賞金稼ぎ")),
    (56, TableItem::Text("その日暮らし")),
    (66, TableItem::Text("宇宙海賊")),
];
static CT6: D66Table = D66Table::new("宇宙時代経歴表", D66SortType::Asc, CT6_ITEMS);
/// Ruby `TABLES["CTD"]`（恐竜人経歴表）の項目。
static CTD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("恐竜議会議員")),
    (12, TableItem::Text("火山活動監視官")),
    (13, TableItem::Text("気候調整官")),
    (14, TableItem::Text("変温管理者")),
    (15, TableItem::Text("卵管理者")),
    (16, TableItem::Text("化石研究者")),
    (22, TableItem::Text("恐竜社会学者")),
    (23, TableItem::Text("羽毛装飾者")),
    (24, TableItem::Text("肉食草食間コーディネーター")),
    (25, TableItem::Text("恐竜語学者")),
    (26, TableItem::Text("かぎ爪ネイリスト")),
    (33, TableItem::Text("恐竜戦士")),
    (34, TableItem::Text("恐竜シスター")),
    (35, TableItem::Text("恐竜神官")),
    (36, TableItem::Text("恐竜拳法家")),
    (44, TableItem::Text("恐竜調教師")),
    (45, TableItem::Text("恐竜ライダー")),
    (46, TableItem::Text("恐竜魔術師")),
    (55, TableItem::Text("恐竜軍軍師")),
    (56, TableItem::Text("隕石研究者")),
    (66, TableItem::Text("恐竜国国王")),
];
static CTD: D66Table = D66Table::new("恐竜人経歴表", D66SortType::Asc, CTD_ITEMS);
/// Ruby `TABLES["CTG"]`（天界人経歴表）の項目。
static CTG_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("天空の神")),
    (12, TableItem::Text("大地の神")),
    (13, TableItem::Text("豊穣の神")),
    (14, TableItem::Text("戦争の神")),
    (15, TableItem::Text("勝利の神")),
    (16, TableItem::Text("復讐の神")),
    (22, TableItem::Text("海の神")),
    (23, TableItem::Text("山の神")),
    (24, TableItem::Text("川の神")),
    (25, TableItem::Text("火の神")),
    (26, TableItem::Text("獣の神")),
    (33, TableItem::Text("冥府の神")),
    (34, TableItem::Text("愛の神")),
    (35, TableItem::Text("美の神")),
    (36, TableItem::Text("知恵の神")),
    (44, TableItem::Text("太陽の神")),
    (45, TableItem::Text("月の神")),
    (46, TableItem::Text("生命の神")),
    (55, TableItem::Text("創造の神")),
    (56, TableItem::Text("死の神")),
    (66, TableItem::Text("時間の神")),
];
static CTG: D66Table = D66Table::new("天界人経歴表", D66SortType::Asc, CTG_ITEMS);
/// Ruby `TABLES["CTFR"]`（亜人（ハイファンタジー種族）経歴表）の項目。
static CTFR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("エルフ／弓使い")),
    (12, TableItem::Text("エルフ／魔術師")),
    (13, TableItem::Text("エルフ／錬金術師")),
    (14, TableItem::Text("エルフ／森の守り手")),
    (15, TableItem::Text("エルフ／司祭")),
    (16, TableItem::Text("エルフ／剣士")),
    (22, TableItem::Text("ドワーフ／戦士")),
    (23, TableItem::Text("ドワーフ／鍛冶職人")),
    (24, TableItem::Text("ドワーフ／傭兵")),
    (25, TableItem::Text("ドワーフ／炭鉱夫")),
    (26, TableItem::Text("ドワーフ／酒場の主人")),
    (33, TableItem::Text("ホビット／農夫")),
    (34, TableItem::Text("ホビット／盗賊")),
    (35, TableItem::Text("ホビット／宿屋の主人")),
    (36, TableItem::Text("ホビット／銀細工師")),
    (44, TableItem::Text("ダークエルフ／妖術士")),
    (45, TableItem::Text("ダークエルフ／暗殺者")),
    (46, TableItem::Text("ピクシー／魔法使い")),
    (55, TableItem::Text("ピクシー／踊り子")),
    (56, TableItem::Text("レプラコーン／靴職人")),
    (66, TableItem::Text("レプラコーン／商人")),
];
static CTFR: D66Table = D66Table::new(
    "亜人（ハイファンタジー種族）経歴表",
    D66SortType::Asc,
    CTFR_ITEMS,
);
/// Ruby `TABLES["CTFM"]`（亜人（ハイファンタジー魔物）経歴表）の項目。
static CTFM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ミノタウロス")),
    (12, TableItem::Text("ケルベロス")),
    (13, TableItem::Text("メデューサ")),
    (14, TableItem::Text("ハーピー")),
    (15, TableItem::Text("インキュバス／サキュバス")),
    (16, TableItem::Text("ラミア")),
    (22, TableItem::Text("ウェアウルフ")),
    (23, TableItem::Text("ゴブリン")),
    (24, TableItem::Text("グール")),
    (25, TableItem::Text("オーク")),
    (26, TableItem::Text("トロール")),
    (33, TableItem::Text("アウルベア")),
    (34, TableItem::Text("インプ")),
    (35, TableItem::Text("スプライト")),
    (36, TableItem::Text("ナーガ")),
    (44, TableItem::Text("リザードマン")),
    (45, TableItem::Text("ヴァンパイア")),
    (46, TableItem::Text("マーマン／マーメイド")),
    (55, TableItem::Text("デーモン")),
    (56, TableItem::Text("スケルトン")),
    (66, TableItem::Text("ドラゴニュート")),
];
static CTFM: D66Table = D66Table::new(
    "亜人（ハイファンタジー魔物）経歴表",
    D66SortType::Asc,
    CTFM_ITEMS,
);
/// Ruby `TABLES["CTY"]`（亜人（妖怪）経歴表）の項目。
static CTY_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("鬼")),
    (12, TableItem::Text("一反木綿")),
    (13, TableItem::Text("子啼き爺")),
    (14, TableItem::Text("砂かけ婆")),
    (15, TableItem::Text("座敷童子")),
    (16, TableItem::Text("かまいたち")),
    (22, TableItem::Text("妖狐")),
    (23, TableItem::Text("のっぺらぼう")),
    (24, TableItem::Text("奪衣婆")),
    (25, TableItem::Text("輪入道")),
    (26, TableItem::Text("泥田坊")),
    (33, TableItem::Text("天狗")),
    (34, TableItem::Text("さとり")),
    (35, TableItem::Text("塗壁")),
    (36, TableItem::Text("猫又")),
    (44, TableItem::Text("河童")),
    (45, TableItem::Text("二口女")),
    (46, TableItem::Text("枕返し")),
    (55, TableItem::Text("天邪鬼")),
    (56, TableItem::Text("雪女")),
    (66, TableItem::Text("ぬらりひょん")),
];
static CTY: D66Table = D66Table::new("亜人（妖怪）経歴表", D66SortType::Asc, CTY_ITEMS);
/// Ruby `TABLES["CTM"]`（亜人（ミュータント）経歴表）の項目。
static CTM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("治癒因子能力（ヒーリングファクタ）")),
    (12, TableItem::Text("精神感応能力（テレパス）")),
    (13, TableItem::Text("念動能力（テレキネシス）")),
    (
        14,
        TableItem::Text("目から出る破壊光線能力（オプティック・ブラスト）"),
    ),
    (15, TableItem::Text("天候操作能力")),
    (16, TableItem::Text("カード爆弾能力")),
    (22, TableItem::Text("磁力操作能力")),
    (23, TableItem::Text("変身能力")),
    (24, TableItem::Text("氷結能力")),
    (25, TableItem::Text("火炎操作能力")),
    (26, TableItem::Text("怪力能力")),
    (33, TableItem::Text("高速移動能力")),
    (34, TableItem::Text("幸運能力")),
    (35, TableItem::Text("自身の肉体を金属化する能力")),
    (36, TableItem::Text("亀人間")),
    (44, TableItem::Text("鼠人間")),
    (45, TableItem::Text("猪人間")),
    (46, TableItem::Text("サイ人間")),
    (55, TableItem::Text("蜘蛛人間")),
    (56, TableItem::Text("蜥蜴人間")),
    (66, TableItem::Text("蝿人間")),
];
static CTM: D66Table = D66Table::new("亜人（ミュータント）経歴表", D66SortType::Asc, CTM_ITEMS);
/// Ruby `TABLES["CTJ"]`（近代人（明治・大正・昭和）経歴表）の項目。
static CTJ_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("華族")),
    (12, TableItem::Text("探偵")),
    (13, TableItem::Text("士族")),
    (14, TableItem::Text("文筆家")),
    (15, TableItem::Text("新聞記者")),
    (16, TableItem::Text("執事／メイド")),
    (22, TableItem::Text("書生／女学生")),
    (23, TableItem::Text("戦闘機パイロット")),
    (24, TableItem::Text("通信兵")),
    (25, TableItem::Text("砲兵")),
    (26, TableItem::Text("工兵")),
    (33, TableItem::Text("歩兵")),
    (34, TableItem::Text("補給部隊")),
    (35, TableItem::Text("軍艦艦長")),
    (36, TableItem::Text("将校")),
    (44, TableItem::Text("軍需工場労働者")),
    (45, TableItem::Text("戦場慰問団員")),
    (46, TableItem::Text("軍医")),
    (55, TableItem::Text("憲兵隊")),
    (56, TableItem::Text("愛国者／婦人会会員")),
    (66, TableItem::Text("天皇")),
];
static CTJ: D66Table = D66Table::new(
    "近代人（明治・大正・昭和）経歴表",
    D66SortType::Asc,
    CTJ_ITEMS,
);
/// Ruby `TABLES["CTF"]`（近代人（西部開拓時代）経歴表）の項目。
static CTF_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ガンマン")),
    (12, TableItem::Text("カウボーイ")),
    (13, TableItem::Text("牧場経営者")),
    (14, TableItem::Text("鍛冶屋")),
    (15, TableItem::Text("馬具職人")),
    (16, TableItem::Text("郵便配達員")),
    (22, TableItem::Text("賞金稼ぎ")),
    (23, TableItem::Text("インディアンの酋長")),
    (24, TableItem::Text("インディアンの戦士")),
    (25, TableItem::Text("インディアンのシャーマン")),
    (26, TableItem::Text("黒人奴隷")),
    (33, TableItem::Text("用心棒")),
    (34, TableItem::Text("蒸気機関車車掌")),
    (35, TableItem::Text("金鉱夫")),
    (36, TableItem::Text("音楽教師")),
    (44, TableItem::Text("保安官")),
    (45, TableItem::Text("仕立屋")),
    (46, TableItem::Text("町医者")),
    (55, TableItem::Text("酒場の主人")),
    (56, TableItem::Text("政治家")),
    (66, TableItem::Text("荒くれ者")),
];
static CTF: D66Table = D66Table::new("近代人（西部開拓時代）経歴表", D66SortType::Asc, CTF_ITEMS);
/// Ruby `TABLES["CTR"]`（機械人経歴表）の項目。
static CTR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("医療ロボット")),
    (12, TableItem::Text("配送ドローン")),
    (13, TableItem::Text("清掃ロボット")),
    (14, TableItem::Text("秘書ロボット")),
    (15, TableItem::Text("警備ロボット")),
    (16, TableItem::Text("エンタメ・ロボット")),
    (22, TableItem::Text("宇宙探査ロボット")),
    (23, TableItem::Text("農業ロボット")),
    (24, TableItem::Text("観光ガイドロボット")),
    (25, TableItem::Text("軍人ロボット")),
    (26, TableItem::Text("メイドロボット")),
    (33, TableItem::Text("犯罪者ロボット")),
    (34, TableItem::Text("刑事ロボット")),
    (35, TableItem::Text("作業用ロボット")),
    (36, TableItem::Text("子守りロボット")),
    (44, TableItem::Text("ヒューマノイドパソコン")),
    (45, TableItem::Text("配偶者用アンドロイド")),
    (46, TableItem::Text("プロトコル・ドロイド")),
    (55, TableItem::Text("アストロ・ドロイド")),
    (56, TableItem::Text("バトル・ドロイド")),
    (66, TableItem::Text("手作りロボット")),
];
static CTR: D66Table = D66Table::new("機械人経歴表", D66SortType::Asc, CTR_ITEMS);
/// Ruby `TABLES["CTAL"]`（異星人経歴表）の項目。
static CTAL_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("重力波解析官")),
    (12, TableItem::Text("量子エンジニア")),
    (13, TableItem::Text("恒星核融合制御エンジニア")),
    (14, TableItem::Text("恒星コロナ制御エンジニア")),
    (15, TableItem::Text("ブラックホールエンジニア")),
    (16, TableItem::Text("エネルギー場エンジニア")),
    (22, TableItem::Text("通商連合商人")),
    (23, TableItem::Text("プラズマエンジニア")),
    (24, TableItem::Text("惑星形成エンジニア")),
    (25, TableItem::Text("生体波動治療師")),
    (26, TableItem::Text("惑星大気分析官")),
    (33, TableItem::Text("恒星居住者")),
    (34, TableItem::Text("ガス巨星居住者")),
    (35, TableItem::Text("小惑星居住者")),
    (36, TableItem::Text("ブラックホール居住者")),
    (44, TableItem::Text("宇宙犯罪者捜査官")),
    (45, TableItem::Text("真空空間居住者")),
    (46, TableItem::Text("彗星居住者")),
    (55, TableItem::Text("異星の支配者")),
    (56, TableItem::Text("宇宙怪獣駆除業者")),
    (66, TableItem::Text("銀河外交官")),
];
static CTAL: D66Table = D66Table::new("異星人経歴表", D66SortType::Asc, CTAL_ITEMS);
/// Ruby `TABLES["CTAM"]`（軟体人経歴表）の項目。
static CTAM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("分裂技術者")),
    (12, TableItem::Text("摂取栄養管理者")),
    (13, TableItem::Text("環境調整官")),
    (14, TableItem::Text("粘液製造者")),
    (15, TableItem::Text("汚染管理者")),
    (16, TableItem::Text("繁殖調整官")),
    (22, TableItem::Text("捕食者")),
    (23, TableItem::Text("細胞修復士")),
    (24, TableItem::Text("温度調整者")),
    (25, TableItem::Text("バイオーム設計者")),
    (26, TableItem::Text("ミクロ環境デザイナー")),
    (33, TableItem::Text("水分管理者")),
    (34, TableItem::Text("エネルギー転換者")),
    (35, TableItem::Text("シグナル発信者")),
    (36, TableItem::Text("捕獲者")),
    (44, TableItem::Text("エコシステム監視官")),
    (45, TableItem::Text("繁殖戦略家")),
    (46, TableItem::Text("拡張研究者")),
    (55, TableItem::Text("進化戦略家")),
    (56, TableItem::Text("エネルギー供給者")),
    (66, TableItem::Text("リーダー")),
];
static CTAM: D66Table = D66Table::new("軟体人経歴表", D66SortType::Asc, CTAM_ITEMS);
/// Ruby `TABLES["CTAD"]`（高次元人経歴表）の項目。
static CTAD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("量子ネットワーク管理者")),
    (12, TableItem::Text("情報融合コーディネーター")),
    (13, TableItem::Text("情報システム設計士")),
    (14, TableItem::Text("バーチャル環境創造者")),
    (15, TableItem::Text("仮想知覚エンジニア")),
    (16, TableItem::Text("メタデータ管理者")),
    (22, TableItem::Text("高次元シミュレーション技術者")),
    (23, TableItem::Text("意識連携アーキテクト")),
    (24, TableItem::Text("異空間整備士")),
    (25, TableItem::Text("デジタルアーカイブ担当官")),
    (26, TableItem::Text("エネルギー構造エンジニア")),
    (33, TableItem::Text("時間操作技術者")),
    (34, TableItem::Text("情報流動分析官")),
    (35, TableItem::Text("意識変換技術者")),
    (36, TableItem::Text("仮想存在保護官")),
    (44, TableItem::Text("高次元相互作用デザイナー")),
    (45, TableItem::Text("情報遺伝子編集者")),
    (46, TableItem::Text("仮想社会調整官")),
    (55, TableItem::Text("高次元空間配置エンジニア")),
    (56, TableItem::Text("仮想意識調査官")),
    (66, TableItem::Text("高次元倫理監査官")),
];
static CTAD: D66Table = D66Table::new("高次元人経歴表", D66SortType::Asc, CTAD_ITEMS);
/// Ruby `TABLES["NMTD"]`（恐竜人名前表／男性名）の項目。
static NMTD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ティラン")),
    (12, TableItem::Text("ジュラディス")),
    (13, TableItem::Text("タラコン")),
    (14, TableItem::Text("テラニクス")),
    (15, TableItem::Text("パキディアン")),
    (16, TableItem::Text("ラプリサス")),
    (22, TableItem::Text("トリケル")),
    (23, TableItem::Text("イグナス")),
    (24, TableItem::Text("ドラカウロス")),
    (25, TableItem::Text("メガロネス")),
    (26, TableItem::Text("クサゴリウス")),
    (33, TableItem::Text("ステゴ")),
    (34, TableItem::Text("カズランティス")),
    (35, TableItem::Text("サルバンティス")),
    (36, TableItem::Text("アルカノス")),
    (44, TableItem::Text("プテラン")),
    (45, TableItem::Text("ラガノス")),
    (46, TableItem::Text("バロー")),
    (55, TableItem::Text("アンキラ")),
    (56, TableItem::Text("タイロス")),
    (66, TableItem::Text("スピノ")),
];
static NMTD: D66Table = D66Table::new("恐竜人名前表／男性名", D66SortType::Asc, NMTD_ITEMS);
/// Ruby `TABLES["NFTD"]`（恐竜人名前表／女性名）の項目。
static NFTD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ウタ")),
    (12, TableItem::Text("マイア")),
    (13, TableItem::Text("ベロクサ")),
    (14, TableItem::Text("ディナヴァス")),
    (15, TableItem::Text("バリスティア")),
    (16, TableItem::Text("マグトリクス")),
    (22, TableItem::Text("ダコタ")),
    (23, TableItem::Text("クラウディノス")),
    (24, TableItem::Text("ドロメニア")),
    (25, TableItem::Text("フィトリス")),
    (26, TableItem::Text("ディノヴィア")),
    (33, TableItem::Text("オヴィ")),
    (34, TableItem::Text("プリメトラ")),
    (35, TableItem::Text("ヴィサロス")),
    (36, TableItem::Text("バロキシス")),
    (44, TableItem::Text("メガ")),
    (45, TableItem::Text("フュリオサノス")),
    (46, TableItem::Text("ブラキオニア")),
    (55, TableItem::Text("ニャサ")),
    (56, TableItem::Text("プレシオニア")),
    (66, TableItem::Text("ユーバ")),
];
static NFTD: D66Table = D66Table::new("恐竜人名前表／女性名", D66SortType::Asc, NFTD_ITEMS);
/// Ruby `TABLES["NMTGG"]`（天界人（ギリシャ神話）名前表／男性名）の項目。
static NMTGG_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ゼウス")),
    (12, TableItem::Text("アレス")),
    (13, TableItem::Text("ヘパイストス")),
    (14, TableItem::Text("プロメテウス")),
    (15, TableItem::Text("エピメテウス")),
    (16, TableItem::Text("アトラス")),
    (22, TableItem::Text("ポセイドン")),
    (23, TableItem::Text("ヘルメス")),
    (24, TableItem::Text("エロス")),
    (25, TableItem::Text("オケアノス")),
    (26, TableItem::Text("ディオニュソス")),
    (33, TableItem::Text("ハデス")),
    (34, TableItem::Text("ヘラクレス")),
    (35, TableItem::Text("ペルセウス")),
    (36, TableItem::Text("アキレウス")),
    (44, TableItem::Text("アポロン")),
    (45, TableItem::Text("エレボス")),
    (46, TableItem::Text("カオス")),
    (55, TableItem::Text("ウラノス")),
    (56, TableItem::Text("タナトス")),
    (66, TableItem::Text("クロノス")),
];
static NMTGG: D66Table = D66Table::new(
    "天界人（ギリシャ神話）名前表／男性名",
    D66SortType::Asc,
    NMTGG_ITEMS,
);
/// Ruby `TABLES["NFTGG"]`（天界人（ギリシャ神話）名前表／女性名）の項目。
static NFTGG_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヘラ")),
    (12, TableItem::Text("イリス")),
    (13, TableItem::Text("アテナ")),
    (14, TableItem::Text("ヘスティア")),
    (15, TableItem::Text("デメテル")),
    (16, TableItem::Text("マイア")),
    (22, TableItem::Text("アムピトリテ")),
    (23, TableItem::Text("アパイア")),
    (24, TableItem::Text("アプロディテ")),
    (25, TableItem::Text("テミス")),
    (26, TableItem::Text("セレネ")),
    (33, TableItem::Text("ペルセポネ")),
    (34, TableItem::Text("アルクメネ")),
    (35, TableItem::Text("ダナエ")),
    (36, TableItem::Text("ペレウス")),
    (44, TableItem::Text("アルテミス")),
    (45, TableItem::Text("テティス")),
    (46, TableItem::Text("エオス")),
    (55, TableItem::Text("ガイア")),
    (56, TableItem::Text("ケール")),
    (66, TableItem::Text("レア")),
];
static NFTGG: D66Table = D66Table::new(
    "天界人（ギリシャ神話）名前表／女性名",
    D66SortType::Asc,
    NFTGG_ITEMS,
);
/// Ruby `TABLES["NMTGJ"]`（天界人（日本神話）名前表／男性名）の項目。
static NMTGJ_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("天御中主神（あめのみなかぬしのかみ）")),
    (12, TableItem::Text("国常立神（くにとこたちのかみ）")),
    (13, TableItem::Text("一言主神（ひとことぬしのかみ）")),
    (14, TableItem::Text("火雷神（ほのいかづちのかみ）")),
    (15, TableItem::Text("経津主神（ふつぬしのかみ）")),
    (16, TableItem::Text("武御雷神（たけみかづちのかみ）")),
    (22, TableItem::Text("伊邪那岐神（いざなぎのかみ）")),
    (23, TableItem::Text("速秋津彦神（はやあきつひこのかみ）")),
    (24, TableItem::Text("久久能智神（くくのちのかみ）")),
    (25, TableItem::Text("大山祇神（おおやまつみのかみ）")),
    (26, TableItem::Text("蛭子神（ひるこのかみ）")),
    (33, TableItem::Text("邇邇芸神（ににぎのかみ）")),
    (34, TableItem::Text("迦具土神（かぐつちのかみ）")),
    (35, TableItem::Text("級長津彦神（しなつひこのかみ）")),
    (36, TableItem::Text("金山彦神（かなやまひこのかみ）")),
    (44, TableItem::Text("素戔嗚尊（すさのおのみこと）")),
    (45, TableItem::Text("埴安彦神（はにやすひこのかみ）")),
    (46, TableItem::Text("思兼神（おもいかねのかみ）")),
    (55, TableItem::Text("猿田彦神（さるだひこのかみ）")),
    (56, TableItem::Text("日本武尊（やまとたけるのみこと）")),
    (66, TableItem::Text("大国主神（おおくにぬしのかみ）")),
];
static NMTGJ: D66Table = D66Table::new(
    "天界人（日本神話）名前表／男性名",
    D66SortType::Asc,
    NMTGJ_ITEMS,
);
/// Ruby `TABLES["NFTGJ"]`（天界人（日本神話）名前表／女性名）の項目。
static NFTGJ_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("天照大神（あまてらすおおみかみ）")),
    (12, TableItem::Text("水波女神（みずはのめのかみ）")),
    (13, TableItem::Text("玉依姫神（たまよりひめのかみ）")),
    (14, TableItem::Text("鹿屋野姫神（かやのひめのかみ）")),
    (15, TableItem::Text("豊玉姫神（とよたまひめのかみ）")),
    (16, TableItem::Text("下照姫神（したてるひめのかみ）")),
    (22, TableItem::Text("伊邪那美神（いざなみのかみ）")),
    (23, TableItem::Text("速秋津姫神（はやあきつひめのかみ）")),
    (24, TableItem::Text("菊理媛神（くくりひめのかみ）")),
    (
        25,
        TableItem::Text("木花咲耶姫神（このはやさくやひめのかみ）"),
    ),
    (26, TableItem::Text("石長姫神（いわながひめのかみ）")),
    (33, TableItem::Text("月読神（つくよみのかみ）")),
    (34, TableItem::Text("保食神（うけもちのかみ）")),
    (35, TableItem::Text("級長津姫神（しなつひめのかみ）")),
    (36, TableItem::Text("金山姫神（かなやまひめのかみ）")),
    (44, TableItem::Text("稲田姫神（いなだひめのかみ）")),
    (45, TableItem::Text("埴安姫神（はにやすひめのかみ）")),
    (46, TableItem::Text("泣沢女神（なきさわめのかみ）")),
    (55, TableItem::Text("天鈿女神（あめのうずめのかみ）")),
    (56, TableItem::Text("豊受大神（とようけのおおかみ）")),
    (66, TableItem::Text("大綿津見神（おおわたつみのかみ）")),
];
static NFTGJ: D66Table = D66Table::new(
    "天界人（日本神話）名前表／女性名",
    D66SortType::Asc,
    NFTGJ_ITEMS,
);
/// Ruby `TABLES["NMTGN"]`（天界人（北欧神話）名前表／男性名）の項目。
static NMTGN_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("トール")),
    (12, TableItem::Text("ヘイムダル")),
    (13, TableItem::Text("テュール")),
    (14, TableItem::Text("フェンリル")),
    (15, TableItem::Text("バルドル")),
    (16, TableItem::Text("ヘズ")),
    (22, TableItem::Text("オーディン")),
    (23, TableItem::Text("ヴァーリ")),
    (24, TableItem::Text("ナリ")),
    (25, TableItem::Text("ヘーニル")),
    (26, TableItem::Text("ミーミル")),
    (33, TableItem::Text("フレイ")),
    (34, TableItem::Text("スキールニル")),
    (35, TableItem::Text("オーズ")),
    (36, TableItem::Text("オッタル")),
    (44, TableItem::Text("ロキ")),
    (45, TableItem::Text("ブラギ")),
    (46, TableItem::Text("ベルセルク")),
    (55, TableItem::Text("ユミル")),
    (56, TableItem::Text("スルト")),
    (66, TableItem::Text("ヨルムンガルド")),
];
static NMTGN: D66Table = D66Table::new(
    "天界人（北欧神話）名前表／男性名",
    D66SortType::Asc,
    NMTGN_ITEMS,
);
/// Ruby `TABLES["NFTGN"]`（天界人（北欧神話）名前表／女性名）の項目。
static NFTGN_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("シヴ")),
    (12, TableItem::Text("ヨルズ")),
    (13, TableItem::Text("イズン")),
    (14, TableItem::Text("ラーン")),
    (15, TableItem::Text("ナンナ")),
    (16, TableItem::Text("モッドグート")),
    (22, TableItem::Text("フリッグ")),
    (23, TableItem::Text("ノルン")),
    (24, TableItem::Text("ウルズ")),
    (25, TableItem::Text("ヴェルザンディ")),
    (26, TableItem::Text("スクルド")),
    (33, TableItem::Text("フレイヤ")),
    (34, TableItem::Text("グルヴェイグ")),
    (35, TableItem::Text("ワルキューレ")),
    (36, TableItem::Text("スカジ")),
    (44, TableItem::Text("シギュン")),
    (45, TableItem::Text("ブリュンヒルド")),
    (46, TableItem::Text("アングルボザ")),
    (55, TableItem::Text("ヘル")),
    (56, TableItem::Text("ゲルズ")),
    (66, TableItem::Text("イアルンヴィジュル")),
];
static NFTGN: D66Table = D66Table::new(
    "天界人（北欧神話）名前表／女性名",
    D66SortType::Asc,
    NFTGN_ITEMS,
);
/// Ruby `TABLES["NMTGE"]`（天界人（エジプト神話）名前表／男性名）の項目。
static NMTGE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ラー")),
    (12, TableItem::Text("オシリス")),
    (13, TableItem::Text("コンス")),
    (14, TableItem::Text("プタハ")),
    (15, TableItem::Text("モントゥ")),
    (16, TableItem::Text("ワジェト")),
    (22, TableItem::Text("アトゥム")),
    (23, TableItem::Text("トト")),
    (24, TableItem::Text("セベク")),
    (25, TableItem::Text("アテン")),
    (26, TableItem::Text("ヌン")),
    (33, TableItem::Text("アメン")),
    (34, TableItem::Text("シュウ")),
    (35, TableItem::Text("ゲブ")),
    (36, TableItem::Text("セト")),
    (44, TableItem::Text("ネフェルテム")),
    (45, TableItem::Text("オシリス")),
    (46, TableItem::Text("クヌム")),
    (55, TableItem::Text("アヌビス")),
    (56, TableItem::Text("ミン")),
    (66, TableItem::Text("ホルス")),
];
static NMTGE: D66Table = D66Table::new(
    "天界人（エジプト神話）名前表／男性名",
    D66SortType::Asc,
    NMTGE_ITEMS,
);
/// Ruby `TABLES["NFTGE"]`（天界人（エジプト神話）名前表／女性名）の項目。
static NFTGE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ムート")),
    (12, TableItem::Text("イシス")),
    (13, TableItem::Text("ネクベト")),
    (14, TableItem::Text("マアト")),
    (15, TableItem::Text("ネイト")),
    (16, TableItem::Text("ネフティス")),
    (22, TableItem::Text("セクメト")),
    (23, TableItem::Text("ヘケト")),
    (24, TableItem::Text("サテト")),
    (25, TableItem::Text("アナト")),
    (26, TableItem::Text("アスタルテ")),
    (33, TableItem::Text("バスト")),
    (34, TableItem::Text("テフヌト")),
    (35, TableItem::Text("セルケト")),
    (36, TableItem::Text("ヌト")),
    (44, TableItem::Text("ハトホル")),
    (45, TableItem::Text("ケデシェト")),
    (46, TableItem::Text("ケベフト")),
    (55, TableItem::Text("バステト")),
    (56, TableItem::Text("ハピ")),
    (66, TableItem::Text("タウエレト")),
];
static NFTGE: D66Table = D66Table::new(
    "天界人（エジプト神話）名前表／女性名",
    D66SortType::Asc,
    NFTGE_ITEMS,
);
/// Ruby `TABLES["NMT2C"]`（古代（中国）名前表）の項目。
static NMT2C_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("劉")),
    (12, TableItem::Text("張")),
    (13, TableItem::Text("関")),
    (14, TableItem::Text("董")),
    (15, TableItem::Text("呂")),
    (16, TableItem::Text("袁")),
    (22, TableItem::Text("備")),
    (23, TableItem::Text("飛")),
    (24, TableItem::Text("羽")),
    (25, TableItem::Text("卓")),
    (26, TableItem::Text("布")),
    (33, TableItem::Text("曹")),
    (34, TableItem::Text("紹")),
    (35, TableItem::Text("馬")),
    (36, TableItem::Text("諸")),
    (44, TableItem::Text("操")),
    (45, TableItem::Text("超")),
    (46, TableItem::Text("葛")),
    (55, TableItem::Text("孫")),
    (56, TableItem::Text("亮")),
    (66, TableItem::Text("権")),
];
static NMT2C: D66Table = D66Table::new("古代（中国）名前表", D66SortType::Asc, NMT2C_ITEMS);
/// Ruby `TABLES["NMTFR"]`（亜人（ハイファンタジー種族）名前表／男性名）の項目。
static NMTFR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("レゴラス")),
    (12, TableItem::Text("エルロンド")),
    (13, TableItem::Text("ルゼーブ")),
    (14, TableItem::Text("エルセリオン")),
    (15, TableItem::Text("ファルミラス")),
    (16, TableItem::Text("アルセンドール")),
    (22, TableItem::Text("オルフェン")),
    (23, TableItem::Text("ギムリ")),
    (24, TableItem::Text("トーリン")),
    (25, TableItem::Text("ブリン")),
    (26, TableItem::Text("グリーバス")),
    (33, TableItem::Text("ブロック")),
    (34, TableItem::Text("ヘイトリ")),
    (35, TableItem::Text("エインヘリアル")),
    (36, TableItem::Text("フロド")),
    (44, TableItem::Text("ビルボ")),
    (45, TableItem::Text("サムワイズ")),
    (46, TableItem::Text("ハルベン")),
    (55, TableItem::Text("ジール")),
    (56, TableItem::Text("ピック")),
    (66, TableItem::Text("ルービン")),
];
static NMTFR: D66Table = D66Table::new(
    "亜人（ハイファンタジー種族）名前表／男性名",
    D66SortType::Asc,
    NMTFR_ITEMS,
);
/// Ruby `TABLES["NFTFR"]`（亜人（ハイファンタジー種族）名前表／女性名）の項目。
static NFTFR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ガラドリエル")),
    (12, TableItem::Text("アルウェン")),
    (13, TableItem::Text("ディードリット")),
    (14, TableItem::Text("イリシア")),
    (15, TableItem::Text("アルヴィナ")),
    (16, TableItem::Text("メリアンド")),
    (22, TableItem::Text("リセリナ")),
    (23, TableItem::Text("トーラ")),
    (24, TableItem::Text("グロリンダ")),
    (25, TableItem::Text("モーヴ")),
    (26, TableItem::Text("ダリナ")),
    (33, TableItem::Text("カールディ")),
    (34, TableItem::Text("ドリーナ")),
    (35, TableItem::Text("ユグドラシル")),
    (36, TableItem::Text("メリアドク")),
    (44, TableItem::Text("プリムラ")),
    (45, TableItem::Text("ミラベラ")),
    (46, TableItem::Text("フィドラ")),
    (55, TableItem::Text("エリス")),
    (56, TableItem::Text("ティンク")),
    (66, TableItem::Text("ローリ")),
];
static NFTFR: D66Table = D66Table::new(
    "亜人（ハイファンタジー種族）名前表／女性名",
    D66SortType::Asc,
    NFTFR_ITEMS,
);
/// Ruby `TABLES["NLTFR"]`（亜人（ハイファンタジー種族）名前表／姓）の項目。
static NLTFR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("グリーンリーフ")),
    (12, TableItem::Text("ウンドーミエル")),
    (13, TableItem::Text("ブルーウッド")),
    (14, TableItem::Text("スレイヴェル")),
    (15, TableItem::Text("ラクシリオン")),
    (16, TableItem::Text("アールヴヘイム")),
    (22, TableItem::Text("ヴァルトアールヴヘイム")),
    (23, TableItem::Text("グロイン")),
    (24, TableItem::Text("オーケンシールド")),
    (25, TableItem::Text("ブレイジングスミス")),
    (26, TableItem::Text("ストームブレイカー")),
    (33, TableItem::Text("ストーンハンマー")),
    (34, TableItem::Text("マグマブラッド")),
    (35, TableItem::Text("ニダヴェリール")),
    (36, TableItem::Text("バギンズ")),
    (44, TableItem::Text("ブランディバック")),
    (45, TableItem::Text("ギャムジー")),
    (46, TableItem::Text("ダルシン")),
    (55, TableItem::Text("ドゥアーデン")),
    (56, TableItem::Text("ライトウィング")),
    (66, TableItem::Text("ゴールド")),
];
static NLTFR: D66Table = D66Table::new(
    "亜人（ハイファンタジー種族）名前表／姓",
    D66SortType::Asc,
    NLTFR_ITEMS,
);
/// Ruby `TABLES["NMTFM"]`（亜人（ハイファンタジー魔物）名前表／男性名）の項目。
static NMTFM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("スメアゴル")),
    (12, TableItem::Text("デアゴル")),
    (13, TableItem::Text("ウグルク")),
    (14, TableItem::Text("ラーツ")),
    (15, TableItem::Text("グリシュナッハ")),
    (16, TableItem::Text("ゴルバグ")),
    (22, TableItem::Text("サウロン")),
    (23, TableItem::Text("ワーウィック")),
    (24, TableItem::Text("ドラグル")),
    (25, TableItem::Text("バーグラ")),
    (26, TableItem::Text("ザロス")),
    (33, TableItem::Text("グラヴィル")),
    (34, TableItem::Text("カーゴス")),
    (35, TableItem::Text("サヴレック")),
    (36, TableItem::Text("モルグリス")),
    (44, TableItem::Text("ムーミン")),
    (45, TableItem::Text("ラゴス")),
    (46, TableItem::Text("ヴォルカン")),
    (55, TableItem::Text("ギルガメシュ")),
    (56, TableItem::Text("ニーズヘッグ")),
    (66, TableItem::Text("エンキドゥ")),
];
static NMTFM: D66Table = D66Table::new(
    "亜人（ハイファンタジー魔物）名前表／男性名",
    D66SortType::Asc,
    NMTFM_ITEMS,
);
/// Ruby `TABLES["NFTFM"]`（亜人（ハイファンタジー魔物）名前表／女性名）の項目。
static NFTFM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("デアゴル")),
    (12, TableItem::Text("シェルロブ")),
    (13, TableItem::Text("マウフル")),
    (14, TableItem::Text("スナガ")),
    (15, TableItem::Text("シャルク")),
    (16, TableItem::Text("ゴスモグ")),
    (22, TableItem::Text("アルタノ")),
    (23, TableItem::Text("マートル")),
    (24, TableItem::Text("ヴァルシャ")),
    (25, TableItem::Text("ザリスカ")),
    (26, TableItem::Text("メリサス")),
    (33, TableItem::Text("サグラナ")),
    (34, TableItem::Text("ヴィルドラ")),
    (35, TableItem::Text("グロッサ")),
    (36, TableItem::Text("バルテリナ")),
    (44, TableItem::Text("フローレン")),
    (45, TableItem::Text("ガレッサ")),
    (46, TableItem::Text("ドリアラ")),
    (55, TableItem::Text("エキドナ")),
    (56, TableItem::Text("リリス")),
    (66, TableItem::Text("モルガナ")),
];
static NFTFM: D66Table = D66Table::new(
    "亜人（ハイファンタジー魔物）名前表／女性名",
    D66SortType::Asc,
    NFTFM_ITEMS,
);
/// Ruby `TABLES["NMTY"]`（亜人（妖怪）名前表／男性名）の項目。
static NMTY_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("酒呑（しゅてん）")),
    (12, TableItem::Text("茨木（いばらき）")),
    (13, TableItem::Text("蒼（あお）")),
    (14, TableItem::Text("丹波（たんば）")),
    (15, TableItem::Text("黒（くろ）")),
    (16, TableItem::Text("九尾（きゅうび）")),
    (22, TableItem::Text("御嶽（おんたけ）")),
    (23, TableItem::Text("高野（こうや）")),
    (24, TableItem::Text("比叡（ひえい）")),
    (25, TableItem::Text("鞍馬（くらま）")),
    (26, TableItem::Text("男体（なんたい）")),
    (33, TableItem::Text("金剛（こんごう）")),
    (34, TableItem::Text("蔵王（ざおう）")),
    (35, TableItem::Text("英彦（ひこ）")),
    (36, TableItem::Text("熊野（くまの）")),
    (44, TableItem::Text("戸隠（とがくし）")),
    (45, TableItem::Text("琵琶（びわ）")),
    (46, TableItem::Text("鳴門（なると）")),
    (55, TableItem::Text("浄土（じょうど）")),
    (56, TableItem::Text("大叫喚（だいきょうかん）")),
    (66, TableItem::Text("焦熱（しょうねつ）")),
];
static NMTY: D66Table = D66Table::new("亜人（妖怪）名前表／男性名", D66SortType::Asc, NMTY_ITEMS);
/// Ruby `TABLES["NFTY"]`（亜人（妖怪）名前表／女性名）の項目。
static NFTY_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("金霊（かねたま）")),
    (12, TableItem::Text("三輪（みわ）")),
    (13, TableItem::Text("赤（あか）")),
    (14, TableItem::Text("紅葉（もみじ）")),
    (15, TableItem::Text("白（しろ）")),
    (16, TableItem::Text("大蛇（おろち）")),
    (22, TableItem::Text("富士（ふじ）")),
    (23, TableItem::Text("白（はく）")),
    (24, TableItem::Text("薬師（やくし）")),
    (25, TableItem::Text("宝満（ほうまん）")),
    (26, TableItem::Text("女体（にょたい）")),
    (33, TableItem::Text("霧島（きりしま）")),
    (34, TableItem::Text("八幡（はちまん）")),
    (35, TableItem::Text("恐（おそれ）")),
    (36, TableItem::Text("伊勢（いせ）")),
    (44, TableItem::Text("出雲（いずも）")),
    (45, TableItem::Text("高千穂（たかちほ）")),
    (46, TableItem::Text("諏訪（すわ）")),
    (55, TableItem::Text("瑠璃（るり）")),
    (56, TableItem::Text("弥勒（みろく）")),
    (66, TableItem::Text("黒縄（こくじょう）")),
];
static NFTY: D66Table = D66Table::new("亜人（妖怪）名前表／女性名", D66SortType::Asc, NFTY_ITEMS);
/// Ruby `TABLES["NMTM"]`（亜人（ミュータント）名前表／男性名）の項目。
static NMTM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ローガン")),
    (12, TableItem::Text("スコット")),
    (13, TableItem::Text("レミー")),
    (14, TableItem::Text("ロバート")),
    (15, TableItem::Text("セントジョン")),
    (16, TableItem::Text("ピョートル")),
    (22, TableItem::Text("チャールズ")),
    (23, TableItem::Text("ピエトロ")),
    (24, TableItem::Text("ハンク")),
    (25, TableItem::Text("ケイン")),
    (26, TableItem::Text("ウォーレン")),
    (33, TableItem::Text("エリック")),
    (34, TableItem::Text("ピーター")),
    (35, TableItem::Text("マイルズ")),
    (36, TableItem::Text("カーティス")),
    (44, TableItem::Text("レオナルド")),
    (45, TableItem::Text("ラファエロ")),
    (46, TableItem::Text("ミケランジェロ")),
    (55, TableItem::Text("ドナテロ")),
    (56, TableItem::Text("スプリンター")),
    (66, TableItem::Text("セス")),
];
static NMTM: D66Table = D66Table::new(
    "亜人（ミュータント）名前表／男性名",
    D66SortType::Asc,
    NMTM_ITEMS,
);
/// Ruby `TABLES["NFTM"]`（亜人（ミュータント）名前表／女性名）の項目。
static NFTM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ローラ")),
    (12, TableItem::Text("ジーン")),
    (13, TableItem::Text("キティ")),
    (14, TableItem::Text("オロロ")),
    (15, TableItem::Text("ジュビリー")),
    (16, TableItem::Text("エマ")),
    (22, TableItem::Text("カサンドラ")),
    (23, TableItem::Text("ワンダ")),
    (24, TableItem::Text("ニーナ")),
    (25, TableItem::Text("マリー")),
    (26, TableItem::Text("エリザベス")),
    (33, TableItem::Text("レイヴン")),
    (34, TableItem::Text("メリー")),
    (35, TableItem::Text("ジェーン")),
    (36, TableItem::Text("グウェン")),
    (44, TableItem::Text("カライ")),
    (45, TableItem::Text("ヴィーナス")),
    (46, TableItem::Text("ミツ")),
    (55, TableItem::Text("シンシア")),
    (56, TableItem::Text("エイプリル")),
    (66, TableItem::Text("ヴェロニカ")),
];
static NFTM: D66Table = D66Table::new(
    "亜人（ミュータント）名前表／女性名",
    D66SortType::Asc,
    NFTM_ITEMS,
);
/// Ruby `TABLES["NLTM"]`（亜人（ミュータント）名前表／姓）の項目。
static NLTM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ハウレット")),
    (12, TableItem::Text("サマーズ")),
    (13, TableItem::Text("グレイ")),
    (14, TableItem::Text("モンロー")),
    (15, TableItem::Text("アラーダイス")),
    (16, TableItem::Text("フロスト")),
    (22, TableItem::Text("エグゼビア")),
    (23, TableItem::Text("マキシモフ")),
    (24, TableItem::Text("マッコイ")),
    (25, TableItem::Text("マルコ")),
    (26, TableItem::Text("ワージントン")),
    (33, TableItem::Text("レーンシャー")),
    (34, TableItem::Text("パーカー")),
    (35, TableItem::Text("モラレス")),
    (36, TableItem::Text("コナーズ")),
    (44, TableItem::Text("なし")),
    (45, TableItem::Text("なし")),
    (46, TableItem::Text("なし")),
    (55, TableItem::Text("なし")),
    (56, TableItem::Text("オニール")),
    (66, TableItem::Text("ブランドル")),
];
static NLTM: D66Table = D66Table::new(
    "亜人（ミュータント）名前表／姓",
    D66SortType::Asc,
    NLTM_ITEMS,
);
/// Ruby `TABLES["NMT4J"]`（近代人（明治・大正・昭和）名前表／男性名）の項目。
static NMT4J_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("小五郎")),
    (12, TableItem::Text("貞次郎")),
    (13, TableItem::Text("栄作")),
    (14, TableItem::Text("久蔵")),
    (15, TableItem::Text("源太郎")),
    (16, TableItem::Text("庄三郎")),
    (22, TableItem::Text("歳三")),
    (23, TableItem::Text("作次郎")),
    (24, TableItem::Text("重吉")),
    (25, TableItem::Text("忠蔵")),
    (26, TableItem::Text("彦三郎")),
    (33, TableItem::Text("由紀夫")),
    (34, TableItem::Text("源三")),
    (35, TableItem::Text("良吉")),
    (36, TableItem::Text("重蔵")),
    (44, TableItem::Text("直弼")),
    (45, TableItem::Text("寛之助")),
    (46, TableItem::Text("清作")),
    (55, TableItem::Text("平八郎")),
    (56, TableItem::Text("喜三郎")),
    (66, TableItem::Text("竜馬")),
];
static NMT4J: D66Table = D66Table::new(
    "近代人（明治・大正・昭和）名前表／男性名",
    D66SortType::Asc,
    NMT4J_ITEMS,
);
/// Ruby `TABLES["NFT4J"]`（近代人（明治・大正・昭和）名前表／女性名）の項目。
static NFT4J_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("小春")),
    (12, TableItem::Text("ふさえ")),
    (13, TableItem::Text("ひろこ")),
    (14, TableItem::Text("つね子")),
    (15, TableItem::Text("いく子")),
    (16, TableItem::Text("すみ子")),
    (22, TableItem::Text("すえ")),
    (23, TableItem::Text("よしこ")),
    (24, TableItem::Text("たまえ")),
    (25, TableItem::Text("せつ子")),
    (26, TableItem::Text("しげ子")),
    (33, TableItem::Text("ようこ")),
    (34, TableItem::Text("たけ子")),
    (35, TableItem::Text("さわ子")),
    (36, TableItem::Text("たみえ")),
    (44, TableItem::Text("まさこ")),
    (45, TableItem::Text("きよ子")),
    (46, TableItem::Text("まつ子")),
    (55, TableItem::Text("テツ子")),
    (56, TableItem::Text("はるえ")),
    (66, TableItem::Text("りょう")),
];
static NFT4J: D66Table = D66Table::new(
    "近代人（明治・大正・昭和）名前表／女性名",
    D66SortType::Asc,
    NFT4J_ITEMS,
);
/// Ruby `TABLES["NLT4J"]`（近代人（明治・大正・昭和）名前表／姓）の項目。
static NLT4J_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("明智")),
    (12, TableItem::Text("高橋")),
    (13, TableItem::Text("田中")),
    (14, TableItem::Text("小林")),
    (15, TableItem::Text("井上")),
    (16, TableItem::Text("山田")),
    (22, TableItem::Text("土方")),
    (23, TableItem::Text("木村")),
    (24, TableItem::Text("斎藤")),
    (25, TableItem::Text("森本")),
    (26, TableItem::Text("渡辺")),
    (33, TableItem::Text("三島")),
    (34, TableItem::Text("藤井")),
    (35, TableItem::Text("黒田")),
    (36, TableItem::Text("村上")),
    (44, TableItem::Text("井伊")),
    (45, TableItem::Text("野村")),
    (46, TableItem::Text("原田")),
    (55, TableItem::Text("東郷")),
    (56, TableItem::Text("西川")),
    (66, TableItem::Text("坂本")),
];
static NLT4J: D66Table = D66Table::new(
    "近代人（明治・大正・昭和）名前表／姓",
    D66SortType::Asc,
    NLT4J_ITEMS,
);
/// Ruby `TABLES["NMT4F"]`（近代人（西部開拓時代）名前表／男性名）の項目。
static NMT4F_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("クライド")),
    (12, TableItem::Text("バック")),
    (13, TableItem::Text("ワイアット")),
    (14, TableItem::Text("クレイトン")),
    (15, TableItem::Text("ハリソン")),
    (16, TableItem::Text("サミュエル")),
    (22, TableItem::Text("クリス")),
    (23, TableItem::Text("チコ")),
    (24, TableItem::Text("カルヴェラ")),
    (25, TableItem::Text("ジェシー")),
    (26, TableItem::Text("イライアス")),
    (33, TableItem::Text("ジョー")),
    (34, TableItem::Text("ラモン")),
    (35, TableItem::Text("ザッカリー")),
    (36, TableItem::Text("キャシディ")),
    (44, TableItem::Text("ダン")),
    (45, TableItem::Text("ベン")),
    (46, TableItem::Text("チャーリー")),
    (55, TableItem::Text("ビリー")),
    (56, TableItem::Text("ダスク")),
    (66, TableItem::Text("サンダンス")),
];
static NMT4F: D66Table = D66Table::new(
    "近代人（西部開拓時代）名前表／男性名",
    D66SortType::Asc,
    NMT4F_ITEMS,
);
/// Ruby `TABLES["NFT4F"]`（近代人（西部開拓時代）名前表／女性名）の項目。
static NFT4F_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ボニー")),
    (12, TableItem::Text("ブランチ")),
    (13, TableItem::Text("クララ")),
    (14, TableItem::Text("スザンナ")),
    (15, TableItem::Text("アニー")),
    (16, TableItem::Text("ハンナ")),
    (22, TableItem::Text("ヒルダ")),
    (23, TableItem::Text("ペトラ")),
    (24, TableItem::Text("カリノ")),
    (25, TableItem::Text("フィオナ")),
    (26, TableItem::Text("マチルダ")),
    (33, TableItem::Text("コンスエロ")),
    (34, TableItem::Text("マリソル")),
    (35, TableItem::Text("ナンシー")),
    (36, TableItem::Text("パール")),
    (44, TableItem::Text("アリス")),
    (45, TableItem::Text("エマ")),
    (46, TableItem::Text("ホリスター")),
    (55, TableItem::Text("サリー")),
    (56, TableItem::Text("ナイト")),
    (66, TableItem::Text("エッタ")),
];
static NFT4F: D66Table = D66Table::new(
    "近代人（西部開拓時代）名前表／女性名",
    D66SortType::Asc,
    NFT4F_ITEMS,
);
/// Ruby `TABLES["NLT4F"]`（近代人（西部開拓時代）名前表／姓）の項目。
static NLT4F_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("バロウ")),
    (12, TableItem::Text("パーカー")),
    (13, TableItem::Text("マグロー")),
    (14, TableItem::Text("バーンズ")),
    (15, TableItem::Text("ブラックウェル")),
    (16, TableItem::Text("ダルトン")),
    (22, TableItem::Text("アダムス")),
    (23, TableItem::Text("タナー")),
    (24, TableItem::Text("ダルトン")),
    (25, TableItem::Text("ラングフォート")),
    (26, TableItem::Text("フェアバンクス")),
    (33, TableItem::Text("バクスター")),
    (34, TableItem::Text("ロホ")),
    (35, TableItem::Text("ソーントン")),
    (36, TableItem::Text("タナー")),
    (44, TableItem::Text("エヴァンス")),
    (45, TableItem::Text("ウェイド")),
    (46, TableItem::Text("プリンス")),
    (55, TableItem::Text("（ザ・）キッド")),
    (56, TableItem::Text("（ザ・）キッド")),
    (66, TableItem::Text("（ザ・）キッド")),
];
static NLT4F: D66Table = D66Table::new(
    "近代人（西部開拓時代）名前表／姓",
    D66SortType::Asc,
    NLT4F_ITEMS,
);
/// Ruby `TABLES["NPTR"]`（機械人名前表／プレフィックス）の項目。
static NPTR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("C-")),
    (12, TableItem::Text("RX-")),
    (13, TableItem::Text("BX-")),
    (14, TableItem::Text("ZQ-")),
    (15, TableItem::Text("TK-")),
    (16, TableItem::Text("2B-")),
    (22, TableItem::Text("R2-")),
    (23, TableItem::Text("A1-")),
    (24, TableItem::Text("D2-")),
    (25, TableItem::Text("SV-")),
    (26, TableItem::Text("7X-")),
    (33, TableItem::Text("T-")),
    (34, TableItem::Text("MK-")),
    (35, TableItem::Text("3P-")),
    (36, TableItem::Text("NDR-")),
    (44, TableItem::Text("R-")),
    (45, TableItem::Text("T-")),
    (46, TableItem::Text("O-")),
    (55, TableItem::Text("WALL-")),
    (56, TableItem::Text("なし")),
    (66, TableItem::Text("R-")),
];
static NPTR: D66Table = D66Table::new("機械人名前表／プレフィックス", D66SortType::Asc, NPTR_ITEMS);
/// Ruby `TABLES["NMTR"]`（機械人名前表／型番）の項目。
static NMTR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("3PO")),
    (12, TableItem::Text("78G")),
    (13, TableItem::Text("450")),
    (14, TableItem::Text("91A")),
    (15, TableItem::Text("33X")),
    (16, TableItem::Text("KV7")),
    (22, TableItem::Text("D2")),
    (23, TableItem::Text("X2")),
    (24, TableItem::Text("Q8")),
    (25, TableItem::Text("12X")),
    (26, TableItem::Text("ALP")),
    (33, TableItem::Text("800")),
    (34, TableItem::Text("17B")),
    (35, TableItem::Text("LX1")),
    (36, TableItem::Text("114")),
    (44, TableItem::Text("66Y")),
    (45, TableItem::Text("260G")),
    (46, TableItem::Text("RX4")),
    (55, TableItem::Text("E")),
    (56, TableItem::Text("EVE")),
    (66, TableItem::Text("DANEEL")),
];
static NMTR: D66Table = D66Table::new("機械人名前表／型番", D66SortType::Asc, NMTR_ITEMS);
/// Ruby `TABLES["NNTR"]`（機械人名前表／愛称）の項目。
static NNTR_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("なし")),
    (12, TableItem::Text("なし")),
    (13, TableItem::Text("なし")),
    (14, TableItem::Text("なし")),
    (15, TableItem::Text("なし")),
    (16, TableItem::Text("なし")),
    (22, TableItem::Text("なし")),
    (23, TableItem::Text("なし")),
    (24, TableItem::Text("なし")),
    (25, TableItem::Text("シルバ")),
    (26, TableItem::Text("アルプス")),
    (33, TableItem::Text("ターミネイト")),
    (34, TableItem::Text("マーク")),
    (35, TableItem::Text("ルクス")),
    (36, TableItem::Text("アンドリュー")),
    (44, TableItem::Text("プロメテス")),
    (45, TableItem::Text("レオナルド")),
    (46, TableItem::Text("オメガ")),
    (55, TableItem::Text("ウォーリー")),
    (56, TableItem::Text("イヴ")),
    (66, TableItem::Text("ダニール")),
];
static NNTR: D66Table = D66Table::new("機械人名前表／愛称", D66SortType::Asc, NNTR_ITEMS);
/// Ruby `TABLES["NMTAL"]`（異星人名前表／男性名）の項目。
static NMTAL_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヨーダ")),
    (12, TableItem::Text("チューイ")),
    (13, TableItem::Text("ジャー・ジャー")),
    (14, TableItem::Text("プロ")),
    (15, TableItem::Text("コールマン")),
    (16, TableItem::Text("キット")),
    (22, TableItem::Text("ジャバ")),
    (23, TableItem::Text("アクバー")),
    (24, TableItem::Text("ワトー")),
    (25, TableItem::Text("グリード")),
    (26, TableItem::Text("セブルバ")),
    (33, TableItem::Text("サノス")),
    (34, TableItem::Text("ドラックス")),
    (35, TableItem::Text("ノバ")),
    (36, TableItem::Text("ベノム")),
    (44, TableItem::Text("コーヴァス")),
    (45, TableItem::Text("ロナン")),
    (46, TableItem::Text("エゴ")),
    (55, TableItem::Text("ジェイク")),
    (56, TableItem::Text("エイティカン")),
    (66, TableItem::Text("ゼノモーフ")),
];
static NMTAL: D66Table = D66Table::new("異星人名前表／男性名", D66SortType::Asc, NMTAL_ITEMS);
/// Ruby `TABLES["NFTAL"]`（異星人名前表／女性名）の項目。
static NFTAL_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アソーカ")),
    (12, TableItem::Text("アイラ")),
    (13, TableItem::Text("ヤドル")),
    (14, TableItem::Text("アディガリア")),
    (15, TableItem::Text("デパビラバ")),
    (16, TableItem::Text("シャアク")),
    (22, TableItem::Text("オール")),
    (23, TableItem::Text("マズ")),
    (24, TableItem::Text("ルミナーラ")),
    (25, TableItem::Text("マラジェイド")),
    (26, TableItem::Text("サルラック")),
    (33, TableItem::Text("ガモーラ")),
    (34, TableItem::Text("マンティス")),
    (35, TableItem::Text("アイーシャ")),
    (36, TableItem::Text("グース")),
    (44, TableItem::Text("プロキシマ")),
    (45, TableItem::Text("ネビュラ")),
    (46, TableItem::Text("ラブ")),
    (55, TableItem::Text("ネイティリ")),
    (56, TableItem::Text("モアト")),
    (66, TableItem::Text("クイーン")),
];
static NFTAL: D66Table = D66Table::new("異星人名前表／女性名", D66SortType::Asc, NFTAL_ITEMS);
/// Ruby `TABLES["NMTAM"]`（軟体人名前表／男性名）の項目。
static NMTAM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ディスコイド")),
    (12, TableItem::Text("プロテウス")),
    (13, TableItem::Text("アカンタメーバ")),
    (14, TableItem::Text("エンセファ")),
    (15, TableItem::Text("デスモデ")),
    (16, TableItem::Text("ファス")),
    (22, TableItem::Text("リボ")),
    (23, TableItem::Text("トリポソーマ")),
    (24, TableItem::Text("エキノコクス")),
    (25, TableItem::Text("ココッカス")),
    (26, TableItem::Text("メギマティウム")),
    (33, TableItem::Text("リマックス")),
    (34, TableItem::Text("ヴァルクス")),
    (35, TableItem::Text("マキシマス")),
    (36, TableItem::Text("カリス")),
    (44, TableItem::Text("エウハドラ")),
    (45, TableItem::Text("アチャティナ")),
    (46, TableItem::Text("ブラディバエナ")),
    (55, TableItem::Text("エウハドラ")),
    (56, TableItem::Text("ヘリックス")),
    (66, TableItem::Text("スネイル")),
];
static NMTAM: D66Table = D66Table::new("軟体人名前表／男性名", D66SortType::Asc, NMTAM_ITEMS);
/// Ruby `TABLES["NFTAM"]`（軟体人名前表／女性名）の項目。
static NFTAM_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ネグレリア")),
    (12, TableItem::Text("アミーバ")),
    (13, TableItem::Text("メノスポラ")),
    (14, TableItem::Text("リトゾーン")),
    (15, TableItem::Text("スモシス")),
    (16, TableItem::Text("シオラ")),
    (22, TableItem::Text("エピクリア")),
    (23, TableItem::Text("アモビニウム")),
    (24, TableItem::Text("アストロマエバ")),
    (25, TableItem::Text("ジスティ")),
    (26, TableItem::Text("ビリネアタム")),
    (33, TableItem::Text("シネレオニガー")),
    (34, TableItem::Text("レーマニア")),
    (35, TableItem::Text("ヴァレンティナ")),
    (36, TableItem::Text("フラブス")),
    (44, TableItem::Text("クエシタ")),
    (45, TableItem::Text("フリカ")),
    (46, TableItem::Text("シミラリス")),
    (55, TableItem::Text("ヘルクロツィ")),
    (56, TableItem::Text("ポマティア")),
    (66, TableItem::Text("マイマイ")),
];
static NFTAM: D66Table = D66Table::new("軟体人名前表／女性名", D66SortType::Asc, NFTAM_ITEMS);
/// Ruby `TABLES["NMTAD"]`（高次元人名前表／男性名）の項目。
static NMTAD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ツイッタ")),
    (12, TableItem::Text("リンクト")),
    (13, TableItem::Text("ユー")),
    (14, TableItem::Text("ワッツ")),
    (15, TableItem::Text("ピンタ")),
    (16, TableItem::Text("タン")),
    (22, TableItem::Text("フェイス")),
    (23, TableItem::Text("ウィー")),
    (24, TableItem::Text("ライン")),
    (25, TableItem::Text("スカ")),
    (26, TableItem::Text("ディス")),
    (33, TableItem::Text("ミク")),
    (34, TableItem::Text("テレグ")),
    (35, TableItem::Text("スラック")),
    (36, TableItem::Text("ギット")),
    (44, TableItem::Text("ニチャン")),
    (45, TableItem::Text("クラブ")),
    (46, TableItem::Text("マスト")),
    (55, TableItem::Text("ティック")),
    (56, TableItem::Text("ブルー")),
    (66, TableItem::Text("インスタ")),
];
static NMTAD: D66Table = D66Table::new("高次元人名前表／男性名", D66SortType::Asc, NMTAD_ITEMS);
/// Ruby `TABLES["NFTAD"]`（高次元人名前表／女性名）の項目。
static NFTAD_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("エックス")),
    (12, TableItem::Text("イン")),
    (13, TableItem::Text("チューブ")),
    (14, TableItem::Text("アップ")),
    (15, TableItem::Text("レスト")),
    (16, TableItem::Text("ブラー")),
    (22, TableItem::Text("ブック")),
    (23, TableItem::Text("チャット")),
    (24, TableItem::Text("ワークス")),
    (25, TableItem::Text("イプ")),
    (26, TableItem::Text("コード")),
    (33, TableItem::Text("シィ")),
    (34, TableItem::Text("ラム")),
    (35, TableItem::Text("チームス")),
    (36, TableItem::Text("ハブ")),
    (44, TableItem::Text("ネル")),
    (45, TableItem::Text("ハウス")),
    (46, TableItem::Text("ドン")),
    (55, TableItem::Text("トック")),
    (56, TableItem::Text("スカイ")),
    (66, TableItem::Text("グラム")),
];
static NFTAD: D66Table = D66Table::new("高次元人名前表／女性名", D66SortType::Asc, NFTAD_ITEMS);
/// Ruby `TABLES["NMT1"]`（原始時代名前表／男性名）の項目。
static NMT1_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ドン")),
    (12, TableItem::Text("ヒュー")),
    (13, TableItem::Text("ゴロゴロ")),
    (14, TableItem::Text("ギュウ")),
    (15, TableItem::Text("バキ")),
    (16, TableItem::Text("カサカサ")),
    (22, TableItem::Text("ガッシャン")),
    (23, TableItem::Text("ピカ")),
    (24, TableItem::Text("モグモグ")),
    (25, TableItem::Text("ボロ")),
    (26, TableItem::Text("ドサ")),
    (33, TableItem::Text("ギラ")),
    (34, TableItem::Text("モクモク")),
    (35, TableItem::Text("ガチャ")),
    (36, TableItem::Text("ジュワ")),
    (44, TableItem::Text("ジャギン")),
    (45, TableItem::Text("ゴツン")),
    (46, TableItem::Text("ブル")),
    (55, TableItem::Text("ギュイン")),
    (56, TableItem::Text("ムチャ")),
    (66, TableItem::Text("ドデカーン")),
];
static NMT1: D66Table = D66Table::new("原始時代名前表／男性名", D66SortType::Asc, NMT1_ITEMS);
/// Ruby `TABLES["NFT1"]`（原始時代名前表／女性名）の項目。
static NFT1_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("パチン")),
    (12, TableItem::Text("プシュ")),
    (13, TableItem::Text("チラチラ")),
    (14, TableItem::Text("キャア")),
    (15, TableItem::Text("ペラ")),
    (16, TableItem::Text("サラサラ")),
    (22, TableItem::Text("クルクル")),
    (23, TableItem::Text("ハハ")),
    (24, TableItem::Text("パタパタ")),
    (25, TableItem::Text("ムニャ")),
    (26, TableItem::Text("クスクス")),
    (33, TableItem::Text("シーン")),
    (34, TableItem::Text("ピシャ")),
    (35, TableItem::Text("チク")),
    (36, TableItem::Text("サク")),
    (44, TableItem::Text("メラ")),
    (45, TableItem::Text("ジュルリ")),
    (46, TableItem::Text("ポチャ")),
    (55, TableItem::Text("ピリリ")),
    (56, TableItem::Text("ビビビ")),
    (66, TableItem::Text("モフ")),
];
static NFT1: D66Table = D66Table::new("原始時代名前表／女性名", D66SortType::Asc, NFT1_ITEMS);
/// Ruby `TABLES["NMT2"]`（古代名前表／男性名）の項目。
static NMT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("クフ")),
    (12, TableItem::Text("アメンホテプ")),
    (13, TableItem::Text("セティ")),
    (14, TableItem::Text("ホレムヘブ")),
    (15, TableItem::Text("ネフェルカレ")),
    (16, TableItem::Text("サルゴン")),
    (22, TableItem::Text("ハンムラビ")),
    (23, TableItem::Text("ナボニドゥス")),
    (24, TableItem::Text("アシュール・ナシル")),
    (25, TableItem::Text("ネルガル")),
    (26, TableItem::Text("キニチ・ジャナーブ")),
    (33, TableItem::Text("ジャサウ・チャン")),
    (34, TableItem::Text("バラム")),
    (35, TableItem::Text("ワクサクラフーン")),
    (36, TableItem::Text("アトラノス")),
    (44, TableItem::Text("ポセイドニス")),
    (45, TableItem::Text("アクアリオン")),
    (46, TableItem::Text("アトランテオス")),
    (55, TableItem::Text("ゼファイロス")),
    (56, TableItem::Text("オーシャナス")),
    (66, TableItem::Text("サラッソス")),
];
static NMT2: D66Table = D66Table::new("古代名前表／男性名", D66SortType::Asc, NMT2_ITEMS);
/// Ruby `TABLES["NFT2"]`（古代名前表／女性名）の項目。
static NFT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("イシス")),
    (12, TableItem::Text("ティエ")),
    (13, TableItem::Text("メルネイト")),
    (14, TableItem::Text("サティア")),
    (15, TableItem::Text("ネフェルティティ")),
    (16, TableItem::Text("アタリヤ")),
    (22, TableItem::Text("ナキア")),
    (23, TableItem::Text("シブトゥ")),
    (24, TableItem::Text("プアビ")),
    (25, TableItem::Text("クババ")),
    (26, TableItem::Text("ワク")),
    (33, TableItem::Text("ヨク")),
    (34, TableItem::Text("タキエン")),
    (35, TableItem::Text("イシュチャック")),
    (36, TableItem::Text("サラッサ")),
    (44, TableItem::Text("トリトニア")),
    (45, TableItem::Text("ラディアンティア")),
    (46, TableItem::Text("アトランティア")),
    (55, TableItem::Text("ムアナ")),
    (56, TableItem::Text("オリカルシア")),
    (66, TableItem::Text("サフィラ")),
];
static NFT2: D66Table = D66Table::new("古代名前表／女性名", D66SortType::Asc, NFT2_ITEMS);
/// Ruby `TABLES["NLT2"]`（古代名前表／姓）の項目。
static NLT2_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("なし")),
    (12, TableItem::Text("なし")),
    (13, TableItem::Text("なし")),
    (14, TableItem::Text("なし")),
    (15, TableItem::Text("なし")),
    (16, TableItem::Text("なし")),
    (22, TableItem::Text("なし")),
    (23, TableItem::Text("なし")),
    (24, TableItem::Text("パル")),
    (25, TableItem::Text("ウセジブ")),
    (26, TableItem::Text("パカル")),
    (33, TableItem::Text("カウィル")),
    (34, TableItem::Text("アジャウ")),
    (35, TableItem::Text("チャパット")),
    (36, TableItem::Text("なし")),
    (44, TableItem::Text("なし")),
    (45, TableItem::Text("なし")),
    (46, TableItem::Text("なし")),
    (55, TableItem::Text("なし")),
    (56, TableItem::Text("なし")),
    (66, TableItem::Text("なし")),
];
static NLT2: D66Table = D66Table::new("古代名前表／姓", D66SortType::Asc, NLT2_ITEMS);
/// Ruby `TABLES["NMT3"]`（中世時代（日本）名前表／男性名）の項目。
static NMT3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("九郎右衛門")),
    (12, TableItem::Text("弥太郎")),
    (13, TableItem::Text("光太夫（こうだゆう）")),
    (14, TableItem::Text("十兵衛")),
    (15, TableItem::Text("五十六")),
    (16, TableItem::Text("雪道")),
    (22, TableItem::Text("義元")),
    (23, TableItem::Text("長政")),
    (24, TableItem::Text("清正")),
    (25, TableItem::Text("政秀")),
    (26, TableItem::Text("隆盛")),
    (33, TableItem::Text("田三郎")),
    (34, TableItem::Text("晋作")),
    (35, TableItem::Text("鉄舟（てっしゅう）")),
    (36, TableItem::Text("昌幸")),
    (44, TableItem::Text("武蔵")),
    (45, TableItem::Text("親蓮（しんれん）")),
    (46, TableItem::Text("澄海（せいかい）")),
    (55, TableItem::Text("法鸞（ほうらん）")),
    (56, TableItem::Text("少林（しょうりん）")),
    (66, TableItem::Text("鑑禅（かんぜん）")),
];
static NMT3: D66Table = D66Table::new(
    "中世時代（日本）名前表／男性名",
    D66SortType::Asc,
    NMT3_ITEMS,
);
/// Ruby `TABLES["NFT3"]`（中世時代（日本）名前表／女性名）の項目。
static NFT3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("静（しず）")),
    (12, TableItem::Text("市（いち）")),
    (13, TableItem::Text("初（はつ）")),
    (14, TableItem::Text("春（はる）")),
    (15, TableItem::Text("鶴（つる）")),
    (16, TableItem::Text("とき")),
    (22, TableItem::Text("しの")),
    (23, TableItem::Text("光（みつ）")),
    (24, TableItem::Text("ゆき")),
    (25, TableItem::Text("京（きょう）")),
    (26, TableItem::Text("すず")),
    (33, TableItem::Text("さよ")),
    (34, TableItem::Text("とせ")),
    (35, TableItem::Text("とよ")),
    (36, TableItem::Text("幸（さち）")),
    (44, TableItem::Text("琴（こと）")),
    (45, TableItem::Text("美濃（みの）")),
    (46, TableItem::Text("千（せん）")),
    (55, TableItem::Text("松（まつ）")),
    (56, TableItem::Text("淀（よど）")),
    (66, TableItem::Text("月（つき）")),
];
static NFT3: D66Table = D66Table::new(
    "中世時代（日本）名前表／女性名",
    D66SortType::Asc,
    NFT3_ITEMS,
);
/// Ruby `TABLES["NLT3"]`（中世時代（日本）名前表／姓）の項目。
static NLT3_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("石田")),
    (12, TableItem::Text("今川")),
    (13, TableItem::Text("上杉")),
    (14, TableItem::Text("武田")),
    (15, TableItem::Text("大友")),
    (16, TableItem::Text("真田")),
    (22, TableItem::Text("北条")),
    (23, TableItem::Text("宮本")),
    (24, TableItem::Text("山岡")),
    (25, TableItem::Text("井伊")),
    (26, TableItem::Text("柳生")),
    (33, TableItem::Text("樋口")),
    (34, TableItem::Text("本間")),
    (35, TableItem::Text("尾高")),
    (36, TableItem::Text("大黒屋")),
    (44, TableItem::Text("森")),
    (45, TableItem::Text("なし")),
    (46, TableItem::Text("なし")),
    (55, TableItem::Text("なし")),
    (56, TableItem::Text("なし")),
    (66, TableItem::Text("なし")),
];
static NLT3: D66Table = D66Table::new("中世時代（日本）名前表／姓", D66SortType::Asc, NLT3_ITEMS);
/// Ruby `TABLES["NMT3W"]`（中世時代（西洋）名前表／男性名）の項目。
static NMT3W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ソルガー")),
    (12, TableItem::Text("アルデン")),
    (13, TableItem::Text("エイモン")),
    (14, TableItem::Text("ヴァリス")),
    (15, TableItem::Text("ロリック")),
    (16, TableItem::Text("セイン")),
    (22, TableItem::Text("エルリック")),
    (23, TableItem::Text("オズリック")),
    (24, TableItem::Text("ジャレス")),
    (25, TableItem::Text("エレンディル")),
    (26, TableItem::Text("エルドレッド")),
    (33, TableItem::Text("エリック")),
    (34, TableItem::Text("ケルドーン")),
    (35, TableItem::Text("ダーリン")),
    (36, TableItem::Text("ラダガスト")),
    (44, TableItem::Text("ダゴン")),
    (45, TableItem::Text("エッダード")),
    (46, TableItem::Text("アラン")),
    (55, TableItem::Text("ジハード")),
    (56, TableItem::Text("ライアン")),
    (66, TableItem::Text("バルバトス")),
];
static NMT3W: D66Table = D66Table::new(
    "中世時代（西洋）名前表／男性名",
    D66SortType::Asc,
    NMT3W_ITEMS,
);
/// Ruby `TABLES["NFT3W"]`（中世時代（西洋）名前表／女性名）の項目。
static NFT3W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("セラフィン")),
    (12, TableItem::Text("エリシア")),
    (13, TableItem::Text("エララ")),
    (14, TableItem::Text("ミリ")),
    (15, TableItem::Text("アルテア")),
    (16, TableItem::Text("サリエル")),
    (22, TableItem::Text("アストリッド")),
    (23, TableItem::Text("エルウィン")),
    (24, TableItem::Text("モルガナ")),
    (25, TableItem::Text("シルヴァ")),
    (26, TableItem::Text("タリア")),
    (33, TableItem::Text("オデッサ")),
    (34, TableItem::Text("カサンドラ")),
    (35, TableItem::Text("アイリス")),
    (36, TableItem::Text("ガラドリエル")),
    (44, TableItem::Text("ダフネ")),
    (45, TableItem::Text("ネリダ")),
    (46, TableItem::Text("ルーシェン")),
    (55, TableItem::Text("フィオラ")),
    (56, TableItem::Text("キーラ")),
    (66, TableItem::Text("エリノラ")),
];
static NFT3W: D66Table = D66Table::new(
    "中世時代（西洋）名前表／女性名",
    D66SortType::Asc,
    NFT3W_ITEMS,
);
/// Ruby `TABLES["NLT3W"]`（中世時代（西洋）名前表／姓）の項目。
static NLT3W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アイアンハンド")),
    (12, TableItem::Text("ストームライダー")),
    (13, TableItem::Text("ウィンドウォーカー")),
    (14, TableItem::Text("シャドウクローク")),
    (15, TableItem::Text("ドラゴンベイン")),
    (16, TableItem::Text("アースシェイカー")),
    (22, TableItem::Text("ウルフハート")),
    (23, TableItem::Text("ハンマーフィスト")),
    (24, TableItem::Text("ベアクロー")),
    (25, TableItem::Text("フロストビアード")),
    (26, TableItem::Text("ストームフォージ")),
    (33, TableItem::Text("ムーンシャドウ")),
    (34, TableItem::Text("スターウィスパー")),
    (35, TableItem::Text("ストームダンサー")),
    (36, TableItem::Text("ライトフット")),
    (44, TableItem::Text("スターフォール")),
    (45, TableItem::Text("ムーンビーム")),
    (46, TableItem::Text("ホワイトソーン")),
    (55, TableItem::Text("なし")),
    (56, TableItem::Text("なし")),
    (66, TableItem::Text("なし")),
];
static NLT3W: D66Table = D66Table::new("中世時代（西洋）名前表／姓", D66SortType::Asc, NLT3W_ITEMS);
/// Ruby `TABLES["NMT4"]`（現代（日本）名前表／男性名）の項目。
static NMT4_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("明")),
    (12, TableItem::Text("隆")),
    (13, TableItem::Text("優")),
    (14, TableItem::Text("謙")),
    (15, TableItem::Text("直樹")),
    (16, TableItem::Text("真一")),
    (22, TableItem::Text("健一")),
    (23, TableItem::Text("純一")),
    (24, TableItem::Text("良太")),
    (25, TableItem::Text("達也")),
    (26, TableItem::Text("悠")),
    (33, TableItem::Text("龍一")),
    (34, TableItem::Text("一真")),
    (35, TableItem::Text("虎之介")),
    (36, TableItem::Text("流星")),
    (44, TableItem::Text("慎之介")),
    (45, TableItem::Text("大和")),
    (46, TableItem::Text("大輝")),
    (55, TableItem::Text("瑛人")),
    (56, TableItem::Text("義人")),
    (66, TableItem::Text("海斗")),
];
static NMT4: D66Table = D66Table::new("現代（日本）名前表／男性名", D66SortType::Asc, NMT4_ITEMS);
/// Ruby `TABLES["NFT4"]`（現代（日本）名前表／女性名）の項目。
static NFT4_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("麻美")),
    (12, TableItem::Text("美紀")),
    (13, TableItem::Text("真理子")),
    (14, TableItem::Text("香織")),
    (15, TableItem::Text("美穂")),
    (16, TableItem::Text("恵美")),
    (22, TableItem::Text("佳子")),
    (23, TableItem::Text("雅子")),
    (24, TableItem::Text("真由美")),
    (25, TableItem::Text("彩")),
    (26, TableItem::Text("千沙")),
    (33, TableItem::Text("菜々子")),
    (34, TableItem::Text("真理")),
    (35, TableItem::Text("桜")),
    (36, TableItem::Text("美月")),
    (44, TableItem::Text("美優")),
    (45, TableItem::Text("麗華")),
    (46, TableItem::Text("真緒")),
    (55, TableItem::Text("陽菜")),
    (56, TableItem::Text("聖華")),
    (66, TableItem::Text("愛梨")),
];
static NFT4: D66Table = D66Table::new("現代（日本）名前表／女性名", D66SortType::Asc, NFT4_ITEMS);
/// Ruby `TABLES["NLT4"]`（現代（日本）名前表／姓）の項目。
static NLT4_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("佐藤")),
    (12, TableItem::Text("鈴木")),
    (13, TableItem::Text("高橋")),
    (14, TableItem::Text("田中")),
    (15, TableItem::Text("渡辺")),
    (16, TableItem::Text("伊藤")),
    (22, TableItem::Text("山本")),
    (23, TableItem::Text("中村")),
    (24, TableItem::Text("小林")),
    (25, TableItem::Text("吉田")),
    (26, TableItem::Text("山田")),
    (33, TableItem::Text("佐々木")),
    (34, TableItem::Text("斎藤")),
    (35, TableItem::Text("木村")),
    (36, TableItem::Text("長谷部")),
    (44, TableItem::Text("井上")),
    (45, TableItem::Text("山口")),
    (46, TableItem::Text("藤井")),
    (55, TableItem::Text("櫻井")),
    (56, TableItem::Text("百瀬")),
    (66, TableItem::Text("十文字")),
];
static NLT4: D66Table = D66Table::new("現代（日本）名前表／姓", D66SortType::Asc, NLT4_ITEMS);
/// Ruby `TABLES["NMT4W"]`（現代（西洋）名前表／男性名）の項目。
static NMT4W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ジェームズ")),
    (12, TableItem::Text("ジョン")),
    (13, TableItem::Text("ロバート")),
    (14, TableItem::Text("マイケル")),
    (15, TableItem::Text("ウィリアム")),
    (16, TableItem::Text("デイビッド")),
    (22, TableItem::Text("リチャード")),
    (23, TableItem::Text("ジョセフ")),
    (24, TableItem::Text("トーマス")),
    (25, TableItem::Text("チャールズ")),
    (26, TableItem::Text("クリストファー")),
    (33, TableItem::Text("ダニエル")),
    (34, TableItem::Text("マシュー")),
    (35, TableItem::Text("マーク")),
    (36, TableItem::Text("ポール")),
    (44, TableItem::Text("スティーブン")),
    (45, TableItem::Text("アンドリュー")),
    (46, TableItem::Text("ジョシュア")),
    (55, TableItem::Text("レオナルド")),
    (56, TableItem::Text("マクシマス")),
    (66, TableItem::Text("マーティ")),
];
static NMT4W: D66Table = D66Table::new("現代（西洋）名前表／男性名", D66SortType::Asc, NMT4W_ITEMS);
/// Ruby `TABLES["NFT4W"]`（現代（西洋）名前表／女性名）の項目。
static NFT4W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("メアリー")),
    (12, TableItem::Text("ジェニファー")),
    (13, TableItem::Text("リンダ")),
    (14, TableItem::Text("パトリシア")),
    (15, TableItem::Text("エリザベス")),
    (16, TableItem::Text("スーザン")),
    (22, TableItem::Text("ジェシカ")),
    (23, TableItem::Text("サラ")),
    (24, TableItem::Text("カレン")),
    (25, TableItem::Text("ナンシー")),
    (26, TableItem::Text("リサ")),
    (33, TableItem::Text("マーガレット")),
    (34, TableItem::Text("ベティ")),
    (35, TableItem::Text("サンドラ")),
    (36, TableItem::Text("アシュリー")),
    (44, TableItem::Text("ドロシー")),
    (45, TableItem::Text("キム")),
    (46, TableItem::Text("エミリー")),
    (55, TableItem::Text("ドナ")),
    (56, TableItem::Text("ミシェル")),
    (66, TableItem::Text("ジェニファー")),
];
static NFT4W: D66Table = D66Table::new("現代（西洋）名前表／女性名", D66SortType::Asc, NFT4W_ITEMS);
/// Ruby `TABLES["NLT4W"]`（現代（西洋）名前表／姓）の項目。
static NLT4W_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("スミス")),
    (12, TableItem::Text("ジョンソン")),
    (13, TableItem::Text("ウィリアムズ")),
    (14, TableItem::Text("ブラウン")),
    (15, TableItem::Text("ジョーンズ")),
    (16, TableItem::Text("ミラー")),
    (22, TableItem::Text("デイビス")),
    (23, TableItem::Text("マルティネス")),
    (24, TableItem::Text("ロドリゲス")),
    (25, TableItem::Text("ウィルソン")),
    (26, TableItem::Text("リーブ")),
    (33, TableItem::Text("ムーア")),
    (34, TableItem::Text("ジャクソン")),
    (35, TableItem::Text("トンプソン")),
    (36, TableItem::Text("アンダーソン")),
    (44, TableItem::Text("テイラー")),
    (45, TableItem::Text("マーチン")),
    (46, TableItem::Text("ホワイト")),
    (55, TableItem::Text("ハリス")),
    (56, TableItem::Text("ジョイ")),
    (66, TableItem::Text("マクフライ")),
];
static NLT4W: D66Table = D66Table::new("現代（西洋）名前表／姓", D66SortType::Asc, NLT4W_ITEMS);
/// Ruby `TABLES["NMT5"]`（超情報化時代名前表／男性名）の項目。
static NMT5_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ゼファー")),
    (12, TableItem::Text("オリオン")),
    (13, TableItem::Text("カシウス")),
    (14, TableItem::Text("アトラス")),
    (15, TableItem::Text("シリウス")),
    (16, TableItem::Text("ソル")),
    (22, TableItem::Text("ドラコ")),
    (23, TableItem::Text("ヴェガ")),
    (24, TableItem::Text("マーキュリー")),
    (25, TableItem::Text("ハレー")),
    (26, TableItem::Text("リゲル")),
    (33, TableItem::Text("アスター")),
    (34, TableItem::Text("クェーサー")),
    (35, TableItem::Text("ノヴァ")),
    (36, TableItem::Text("ヘリオス")),
    (44, TableItem::Text("ゲイレン")),
    (45, TableItem::Text("ネビュラ")),
    (46, TableItem::Text("パルサー")),
    (55, TableItem::Text("トーラス")),
    (56, TableItem::Text("ファン")),
    (66, TableItem::Text("ヨリノブ")),
];
static NMT5: D66Table = D66Table::new("超情報化時代名前表／男性名", D66SortType::Asc, NMT5_ITEMS);
/// Ruby `TABLES["NFT5"]`（超情報化時代名前表／女性名）の項目。
static NFT5_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("リラ")),
    (12, TableItem::Text("ヴィーナス")),
    (13, TableItem::Text("アウロラ")),
    (14, TableItem::Text("ステラ")),
    (15, TableItem::Text("セレネ")),
    (16, TableItem::Text("ヴェラ")),
    (22, TableItem::Text("ジルナ")),
    (23, TableItem::Text("ヴェガ")),
    (24, TableItem::Text("セレスト")),
    (25, TableItem::Text("クリサンセマ")),
    (26, TableItem::Text("フクシア")),
    (33, TableItem::Text("サファイア")),
    (34, TableItem::Text("アザレア")),
    (35, TableItem::Text("ノヴァ")),
    (36, TableItem::Text("ベゴニア")),
    (44, TableItem::Text("カメリア")),
    (45, TableItem::Text("マグノリア")),
    (46, TableItem::Text("ハイビス")),
    (55, TableItem::Text("フリージア")),
    (56, TableItem::Text("マリーゴールド")),
    (66, TableItem::Text("ハナコ")),
];
static NFT5: D66Table = D66Table::new("超情報化時代名前表／女性名", D66SortType::Asc, NFT5_ITEMS);
/// Ruby `TABLES["NLT5"]`（超情報化時代名前表／姓）の項目。
static NLT5_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ヤマモト")),
    (12, TableItem::Text("ンコシ")),
    (13, TableItem::Text("パテル")),
    (14, TableItem::Text("ジェン")),
    (15, TableItem::Text("クリシュナン")),
    (16, TableItem::Text("シン")),
    (22, TableItem::Text("タボ")),
    (23, TableItem::Text("ワン")),
    (24, TableItem::Text("クォン")),
    (25, TableItem::Text("ウー")),
    (26, TableItem::Text("ウォン")),
    (33, TableItem::Text("ミヤザキ")),
    (34, TableItem::Text("ファン")),
    (35, TableItem::Text("セロン")),
    (36, TableItem::Text("バティア")),
    (44, TableItem::Text("スズキ")),
    (45, TableItem::Text("ホー")),
    (46, TableItem::Text("ウィズ")),
    (55, TableItem::Text("カウル")),
    (56, TableItem::Text("グプタ")),
    (66, TableItem::Text("アラサカ")),
];
static NLT5: D66Table = D66Table::new("超情報化時代名前表／姓", D66SortType::Asc, NLT5_ITEMS);
/// Ruby `TABLES["NMT6"]`（宇宙時代名前表／男性名）の項目。
static NMT6_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("アルタイア")),
    (12, TableItem::Text("コメット")),
    (13, TableItem::Text("フェニックス")),
    (14, TableItem::Text("アストラ")),
    (15, TableItem::Text("ソラス")),
    (16, TableItem::Text("スタリオン")),
    (22, TableItem::Text("スカイゲイザー")),
    (23, TableItem::Text("ハイペリオン")),
    (24, TableItem::Text("ソラリウス")),
    (25, TableItem::Text("プラズマ")),
    (26, TableItem::Text("スペクトル")),
    (33, TableItem::Text("クラスター")),
    (34, TableItem::Text("ヴォイド")),
    (35, TableItem::Text("アストロノーム")),
    (36, TableItem::Text("インフィニティ")),
    (44, TableItem::Text("スパイラル")),
    (45, TableItem::Text("センチュリオン")),
    (46, TableItem::Text("パラダイム")),
    (55, TableItem::Text("スターフィールド")),
    (56, TableItem::Text("ニュートロン")),
    (66, TableItem::Text("メテオライト")),
];
static NMT6: D66Table = D66Table::new("宇宙時代名前表／男性名", D66SortType::Asc, NMT6_ITEMS);
/// Ruby `TABLES["NFT6"]`（宇宙時代名前表／女性名）の項目。
static NFT6_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("カシオペア")),
    (12, TableItem::Text("ルナ")),
    (13, TableItem::Text("ソラナ")),
    (14, TableItem::Text("アンドラ")),
    (15, TableItem::Text("ギャレナ")),
    (16, TableItem::Text("セレスティア")),
    (22, TableItem::Text("スターライト")),
    (23, TableItem::Text("ガラクシア")),
    (24, TableItem::Text("メテオラ")),
    (25, TableItem::Text("エクリプス")),
    (26, TableItem::Text("ルナリス")),
    (33, TableItem::Text("サテライト")),
    (34, TableItem::Text("プレアデス")),
    (35, TableItem::Text("サンフラワー")),
    (36, TableItem::Text("ホライゾン")),
    (44, TableItem::Text("フェノメナ")),
    (45, TableItem::Text("アストリッド")),
    (46, TableItem::Text("ミレニアム")),
    (55, TableItem::Text("ヴォルテックス")),
    (56, TableItem::Text("フュージョン")),
    (66, TableItem::Text("マトリクス")),
];
static NFT6: D66Table = D66Table::new("宇宙時代名前表／女性名", D66SortType::Asc, NFT6_ITEMS);
/// Ruby `TABLES["NLT6"]`（宇宙時代名前表／姓）の項目。
static NLT6_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("スターボーン")),
    (12, TableItem::Text("ネブラ")),
    (13, TableItem::Text("ギャラクソス")),
    (14, TableItem::Text("オリオニス")),
    (15, TableItem::Text("スターウィーバー")),
    (16, TableItem::Text("ライトイヤー")),
    (22, TableItem::Text("スターダスト")),
    (23, TableItem::Text("スカイドリーム")),
    (24, TableItem::Text("ギャラクシオン")),
    (25, TableItem::Text("スターファイア")),
    (26, TableItem::Text("ラプソディ")),
    (33, TableItem::Text("アポロ")),
    (34, TableItem::Text("ガリレオ")),
    (35, TableItem::Text("オービット")),
    (36, TableItem::Text("ステラート")),
    (44, TableItem::Text("アストラル")),
    (45, TableItem::Text("シグマ")),
    (46, TableItem::Text("アーク")),
    (55, TableItem::Text("オメガ")),
    (56, TableItem::Text("エンデバー")),
    (66, TableItem::Text("リープ")),
];
static NLT6: D66Table = D66Table::new("宇宙時代名前表／姓", D66SortType::Asc, NLT6_ITEMS);
/// Ruby `TABLES["CCT"]`（因縁種別表（因縁種別／因縁強度））の項目。
static CCT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("実の父・母／6")),
    (12, TableItem::Text("養父・養母／4")),
    (13, TableItem::Text("養子／4")),
    (14, TableItem::Text("ペット／4")),
    (15, TableItem::Text("親友／3")),
    (16, TableItem::Text("友人／2")),
    (22, TableItem::Text("実の兄弟・姉妹／6")),
    (23, TableItem::Text("義理の兄弟・姉妹／4")),
    (24, TableItem::Text("同僚／3")),
    (25, TableItem::Text("隣人／1")),
    (26, TableItem::Text("お客さんor君のファン／1")),
    (33, TableItem::Text("実の祖父母／5")),
    (34, TableItem::Text("クラスメイト／2")),
    (35, TableItem::Text("先輩／2")),
    (36, TableItem::Text("後輩／2")),
    (44, TableItem::Text("実の子／6")),
    (45, TableItem::Text("上司or師／3")),
    (46, TableItem::Text("部下or生徒／3")),
    (55, TableItem::Text("異性の配偶者／5")),
    (56, TableItem::Text("同性の配偶者／4")),
    (66, TableItem::Text("恋人／3")),
];
static CCT: D66Table = D66Table::new(
    "因縁種別表（因縁種別／因縁強度）",
    D66SortType::Asc,
    CCT_ITEMS,
);

/// Ruby `TABLES`。コマンド名 → D66表。
static TABLES: &[(&str, &D66Table)] = &[
    ("CT1", &CT1),
    ("CT2", &CT2),
    ("CT3", &CT3),
    ("CT4", &CT4),
    ("CT5", &CT5),
    ("CT6", &CT6),
    ("CTD", &CTD),
    ("CTG", &CTG),
    ("CTFR", &CTFR),
    ("CTFM", &CTFM),
    ("CTY", &CTY),
    ("CTM", &CTM),
    ("CTJ", &CTJ),
    ("CTF", &CTF),
    ("CTR", &CTR),
    ("CTAL", &CTAL),
    ("CTAM", &CTAM),
    ("CTAD", &CTAD),
    ("NMTD", &NMTD),
    ("NFTD", &NFTD),
    ("NMTGG", &NMTGG),
    ("NFTGG", &NFTGG),
    ("NMTGJ", &NMTGJ),
    ("NFTGJ", &NFTGJ),
    ("NMTGN", &NMTGN),
    ("NFTGN", &NFTGN),
    ("NMTGE", &NMTGE),
    ("NFTGE", &NFTGE),
    ("NMT2C", &NMT2C),
    ("NMTFR", &NMTFR),
    ("NFTFR", &NFTFR),
    ("NLTFR", &NLTFR),
    ("NMTFM", &NMTFM),
    ("NFTFM", &NFTFM),
    ("NMTY", &NMTY),
    ("NFTY", &NFTY),
    ("NMTM", &NMTM),
    ("NFTM", &NFTM),
    ("NLTM", &NLTM),
    ("NMT4J", &NMT4J),
    ("NFT4J", &NFT4J),
    ("NLT4J", &NLT4J),
    ("NMT4F", &NMT4F),
    ("NFT4F", &NFT4F),
    ("NLT4F", &NLT4F),
    ("NPTR", &NPTR),
    ("NMTR", &NMTR),
    ("NNTR", &NNTR),
    ("NMTAL", &NMTAL),
    ("NFTAL", &NFTAL),
    ("NMTAM", &NMTAM),
    ("NFTAM", &NFTAM),
    ("NMTAD", &NMTAD),
    ("NFTAD", &NFTAD),
    ("NMT1", &NMT1),
    ("NFT1", &NFT1),
    ("NMT2", &NMT2),
    ("NFT2", &NFT2),
    ("NLT2", &NLT2),
    ("NMT3", &NMT3),
    ("NFT3", &NFT3),
    ("NLT3", &NLT3),
    ("NMT3W", &NMT3W),
    ("NFT3W", &NFT3W),
    ("NLT3W", &NLT3W),
    ("NMT4", &NMT4),
    ("NFT4", &NFT4),
    ("NLT4", &NLT4),
    ("NMT4W", &NMT4W),
    ("NFT4W", &NFT4W),
    ("NLT4W", &NLT4W),
    ("NMT5", &NMT5),
    ("NFT5", &NFT5),
    ("NLT5", &NLT5),
    ("NMT6", &NMT6),
    ("NFT6", &NFT6),
    ("NLT6", &NLT6),
    ("CCT", &CCT),
];

// ---------------------------------------------------------------------------
// Ruby `TABLES_MOD_2D`（2D6の表（出目指定・修正値つき））
// ---------------------------------------------------------------------------

/// Ruby `TABLES_MOD_2D["ACT"]`（アクシデント表 / 2D6）の項目。
static ACT_ITEMS: &[&str] = &[
    "一匹の蝶の羽ばたきが、地球の裏側では竜巻を巻き起こす。あなたは目の前にある他愛のない何かを拾って、置き直した。GMはランダムに指定特技を決定する。シーンに登場しているPCは全員その指定特技で判定する。判定に成功すれば何も起こらない。判定に失敗すると自分を対象としてバタフライエフェクトが発生する。その際の対象の因縁強度は6として処理する。",
    "目の前で大事故が発生する。事故に巻き込まれた人を助けなければ。シーンプレイヤーのPCは《医療》《労働》《持ち上げる》《倫理》《応急処置》《魔除け》のいずれかを指定特技として判定を行う。判定に成功すると、目の前の人を助けることができる。PCの［疲労度］が1D6点減少する。だがその人物は本来はここで死ぬべき運命だったようだ。自分の【因縁】からランダムに選んだキャラクターを対象としてバタフライエフェクトが発生する。判定に失敗するとPCの［疲労度］が1D6点増加する。",
    "あなたの出身時代では知りえない情報を知ってしまう。その情報をあなたが覚えている限り歴史に重大な影響をもたらすだろう。シーンプレイヤーのPCは《記憶力》の判定を行う。判定に失敗すると自分の【因縁】のうちの好きなキャラクターを対象としてバタフライエフェクトが発生する。",
    "誰かと出会い頭に衝突しそうになる。シーンプレイヤーは舞台となっている時代の名前表と経歴表を使用し、年齢は8D6を振って、相手の設定概要を決める。TAはその相手の設定に従って、最適な指定特技を決定する。シーンプレイヤーのPCはその指定特技で判定を行う。判定に失敗すると、相手とぶつかってしまい、シーンプレイヤーのPCの［改変度］が1D6増加する。",
    "困っている人がいる。シーンプレイヤーは舞台となっている時代の名前表と経歴表を使用し、年齢は8D6を振って、相手の設定概要を決める。TAはその相手の設定に従って、最適な指定特技を決定する。シーンプレイヤーのPCはその指定特技で判定を行う。判定に成功すれば、［改変度］が1点増加しつつも困りごとを解決してあげることができる。シーンプレイヤーのPCはお礼として好きな【アイテム】1つを獲得する。",
    "シーンプレイヤーは好きなPCを登場させることができる。そしてシーンプレイヤーはシーンに登場しているキャラクター一人を選び、二人で何かしらひと時を過ごす。シーンプレイヤーのPCは好きな指定特技で判定を行う。判定に成功すれば、選んだキャラクターを新たな【因縁】として獲得できる。相手のPCもシーンプレイヤーのPCを新たな【因縁】として獲得できる。因縁種別は「タイムトラベル仲間／因縁強度6」となる。因縁内容は状況にふさわしいものをポジティブ因縁内容表もしくはネガティブ因縁内容表から選択する。",
    "運命的な出会い。シーンプレイヤーのPCは好きな指定特技で判定を行う。判定に成功すれば、シーンプレイヤーはセッションの舞台となっている時代/ELの新たな【因縁】を獲得する。因縁内容表を用いて【因縁】を作成する。1D6を振って奇数が出ればポジティブ因縁内容表、偶数が出ればネガティブ因縁内容表を使用する。ただし因縁種別は「親友／因縁強度3」もしくは「恋人／因縁強度4」のいずれかから選択する。細かい設定は名前表や経歴表を振って決めること。",
    "遠く離れて、思い起こされる人物。シーンプレイヤーのPCは好きな指定特技で判定を行う。判定に成功すれば、自分の出身時代/ELの新たな【因縁】を獲得する。初期作成時と同じく、因縁種別表と因縁内容表を用いて【因縁】を作成する。1D6を振って奇数が出ればポジティブ因縁内容表、偶数が出ればネガティブ因縁内容表を使用する。細かい設定は名前表や経歴表を振って決めること。",
    "シーンプレイヤーは好きなPCを登場させることができる。宇宙開闢の女神があなたに微笑みかける。これを成功させれば歴史改変を修正できる、という理路が導き出される。シーンプレイヤーのPCはランダムに決定した指定特技の判定を行う。判定に成功すれば、登場しているPCのうち好きな一人の［改変度］が1D6点減少する。",
    "シーンプレイヤーは好きなPCを登場させることができる。この時代にも心を落ち着けることができる場所を見つけることができた。何をして疲れを癒そう。シーンプレイヤーのPCはランダムに決定した指定特技で判定を行う。判定に成功すれば、シーンに登場しているPC全員の［疲労度］が1D6点減少する。",
    "シーンプレイヤーは好きなPCを登場させることができる。宇宙開闢の女神があなたたちを抱擁する。これを成功させれば大幅に歴史改変を修正できる、という理路が導き出される。シーンプレイヤーのPCはランダムに決定した指定特技の判定を行う。判定に成功すれば、シーンに登場しているPC全員の［改変度］が1D6点減少する。",
];
static ACT: Table = Table::from_dice("アクシデント表", 2, 6, ACT_ITEMS);
/// Ruby `TABLES_MOD_2D["ST1A"]`（原始時代（リアル原始）シーン表 / 2D6）の項目。
static ST1A_ITEMS: &[&str] = &[
    "突然の雷雨。太古の大地に止まない雨が降り注ぐ。",
    "水辺の湿地帯。原始人がカエルや小魚を捕まえ、食べ物にする。",
    "川沿いの森。原始人たちが木の実を集め、焚き火の周りに集まる。",
    "乾いた平野。槍を手にした原始人たちが獲物を追い、仲間と共に狩りをする。",
    "洞窟の入り口。焚き火が静かに燃え、壁には狩猟の絵が描かれている。",
    "木陰の広場。子供たちが遊び、大人は動物の皮を加工している。",
    "草原の丘。原始人が見張りに立ち、遠くの群れを観察している。",
    "川辺の浅瀬。原始人の女性たちが水を汲み、魚を捕まえている。",
    "荒涼とした砂地。原始人が乾燥した植物を集め、食料として保存している。",
    "山間の道。石を投げる原始人が、群れから離れた獲物を狙う。",
    "溶岩流れる火山地帯。地響きが鳴り続け、火口からは黒煙があがっている。",
];
static ST1A: Table = Table::from_dice("原始時代（リアル原始）シーン表", 2, 6, ST1A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST1B"]`（原始時代（恐竜と人類）シーン表 / 2D6）の項目。
static ST1B_ITEMS: &[&str] = &[
    "突然の雷雨。酸性の雨が降り注ぐ。",
    "滝の裏側。地響きが聞こえる。外ではティラノサウルスがうろついているようだ。",
    "森林。下等な哺乳類が木の実をとってのんびり過ごしている。",
    "小高い丘。眼下には小型の肉食恐竜ヴェロキラプトルの群れが狩りのために走り回っている。",
    "薄暗い洞窟。異名は剣竜、草食恐竜ステゴザウルスが卵を抱いてすやすやと眠っている。",
    "肥沃な大地。首の長い草食恐竜ブラキオサウルスが背の高い木の葉っぱを食んでいる。",
    "草原。角を持った草食恐竜トリケラトプスの親子が水場を求めて横断している。",
    "海辺。沖では海の覇者モササウルスが巨大な尾を海面にたたきつけしぶきをあげている。",
    "荒れ果てた土地。パキケファロサウルスたちが自慢の石頭をぶつけ合い縄張り争いをしている。",
    "山のふもと。鬱蒼とした森の上空では翼竜プテラノドンの群れが旋回している。",
    "溶岩流れる火山地帯。地響きと大型恐竜のいななきが聞こえる。",
];
static ST1B: Table = Table::from_dice("原始時代（恐竜と人類）シーン表", 2, 6, ST1B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST1C"]`（原始時代（恐竜人文明）シーン表 / 2D6）の項目。
static ST1C_ITEMS: &[&str] = &[
    "突然のにわか雨。ラプトル型恐竜人の子供たちが屋根を求めて我先にと走ってゆく。",
    "密林の奥。藤が絡まる塔が天へと伸び、恐竜人間の姿を模した巨大な石像がそびえたつ。",
    "山間の谷。巨大な石像が並ぶ街道を恐竜人が行き交う。",
    "広大な平野。恐竜人の兵隊が地を揺らしながら進軍してゆく。",
    "高原の神殿。強い風が吹きすさぶなか、恐竜人の賢者が集まり議論を交わす。",
    "石畳の広場。様々な恐竜人たちが往来する恐竜人都市の中心地だ。",
    "商店街。光を反射する水晶の壁が輝くアーケードで恐竜人商人たちが商売に精を出している。",
    "岩山の城。恐竜人の王が玉座に座り、全てを見渡す。",
    "巨大な湖のほとり。恐竜人の漁師が静かに網を引き上げている。",
    "沼地の村。水に映る太陽の光が、木造の家々を照らす。",
    "都市の地下に広がる鍾乳洞の洞窟。ひんやりとした空気が静寂を包み込む。",
];
static ST1C: Table = Table::from_dice("原始時代（恐竜人文明）シーン表", 2, 6, ST1C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST2A"]`（古代（リアル古代）シーン表 / 2D6）の項目。
static ST2A_ITEMS: &[&str] = &[
    "突然の大雨。この雨がもたらすは豊穣の実りか、河川の氾濫か。",
    "空中庭園。宮殿の屋上に植えられた植物が垂れ下がり、緑豊かな光景が広がっている。",
    "工場。青銅器の製造工房では、職人たちが火を使って武器や道具を作り続けている。",
    "王の墓。巨大な石造りの構造物が空にそびえ、労働者たちが石を積み上げている。",
    "市場。商人たちが絹や香料を売り買いし、川のほとりで交易が盛んに行われている。",
    "街角。青空の下で市民が行き交い、パン屋や染物屋が日常の生活を営んでいる。",
    "大河の氾濫原。毎年の増水で肥沃な土が広がり、農民たちは作物を育てるために忙しく働いている。",
    "神殿。壮麗な柱廊が続き、太陽の光が神殿の石壁に反射し、神聖な雰囲気を醸し出している。",
    "闘技場。群衆が歓声を上げ、剣闘士たちが命を懸けて戦いを繰り広げている。",
    "公衆浴場。住民たちが大きな水槽で体を清め、共同生活の重要な一部となっている。",
    "活火山の麓。硫黄の臭いが立ち込めている。",
];
static ST2A: Table = Table::from_dice("古代（リアル古代）シーン表", 2, 6, ST2A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST2B"]`（古代（都市伝説文明）シーン表 / 2D6）の項目。
static ST2B_ITEMS: &[&str] = &[
    "雨。神殿の石像に雨が滴り落ち、湿った大地が深い緑に覆われ、霧が立ち込める。",
    "巨大図書館。全ての知識が収められた館の中で、未来の技術を解読する者たちが集まっている。",
    "賢人会議棟。未来を見通す知識人が集まり、世界の終焉を議論している。",
    "天文台。祭司たちが精巧な天体観測を行い、次の時代の到来を予言している。",
    "予言の石碑。刻まれた神聖文字が未来を暗示し、祭司たちが星の動きを読み解いている。",
    "神殿。巨大な石像が立ち並び、天空の星々と共鳴する神秘的な儀式が行われている。",
    "水晶都市。透き通るような塔が海中に立ち、エネルギーが四方に放射されている。",
    "港湾。光り輝く船が神秘的な力で浮かび、太古の貿易が超次元的な方法で行われている。",
    "ピラミッドの頂上。祭司が神に供物を捧げ、天体の周期が終わりを告げる予兆を読み取っている。",
    "空飛ぶ船。古代の技術で空中を浮遊する船が、地上を支配する力を誇示している。",
    "庭園。透き通る風が吹き抜け、透明な泉の周りで賢者たちが平和なひとときを過ごしている。",
];
static ST2B: Table = Table::from_dice("古代（都市伝説文明）シーン表", 2, 6, ST2B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST2C"]`（古代（天上の神々）シーン表 / 2D6）の項目。
static ST2C_ITEMS: &[&str] = &[
    "暴風雨。天空の神が荒れ狂う空を支配している。",
    "虹の橋。空にかかる七色の橋が、天界と地上を結び、神々がそこを行き来している。",
    "神殿。慈愛の女神が神殿で祈りを捧げる信者たちを見守り、癒しの力を広げている。",
    "神山の頂上。雲間から閃く雷が主神の玉座を照らす。神々が集まり議論を交わしている。",
    "森。狩猟の女神が月光に照らされた森を駆け巡り、野生の生き物たちが彼女の後を追っている。",
    "空中都市。富の神が空飛ぶ都市で、宝物と富を分配し、世界の豊穣を守っている。",
    "大広間。主神が玉座に座り、戦士たちの運命を決めるために集まっている。",
    "聖なる川。知恵と水の神が川のほとりで思索にふけり、生命の源を世界に与えている。",
    "酒宴。野外で神々が葡萄酒を楽しみ、豊穣の喜びに満ちた歌と踊りが絶え間なく続いている。",
    "深淵。黒い霧が漂い、冥界の神の怒りを避けるため、罪深き魂が恐怖に震えている。",
    "庭園。美しい花々が咲き誇り、愛と美の女神が池のほとりで静かに佇んでいる。",
];
static ST2C: Table = Table::from_dice("古代（天上の神々）シーン表", 2, 6, ST2C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST3A"]`（中世時代（リアル中世）シーン表 / 2D6）の項目。
static ST3A_ITEMS: &[&str] = &[
    "雨。苔むした石壁に雨水が流れ落ちている。",
    "宴。広間に豪勢な食事が並び、有力者たちが酒を酌み交わしている。",
    "屋敷。庭を望む広間に武具が掛けられ、戦士たちが静かに茶を飲みながら戦略を語り合っている。",
    "農村。広がる畑で農民が穀物を刈り取り、麦わら／稲の束が整然と積み上げられている。",
    "城下町の通り。職人たちが店先で道具を磨き、通行人が行き交う賑やかな市場が広がっている。",
    "城門前。民衆が領主に進物を捧げ、厳格な視線のもとで感謝の言葉を述べている。",
    "市場の広場。商人たちが色とりどりの品々を並べ、活気に満ちた声が交錯している。",
    "港町の桟橋。帆船が係留され、商人たちが積荷を運び出し、海の潮風が町中に漂っている。",
    "教会／寺院。瞑想する聖職者たちの周りには、厳粛な静寂が満ちている。",
    "砦。厚い石壁がそびえ、矢狭間から弓兵が外を見張り、緊張が走る防衛線が張られている。",
    "剣術の試合。城内の庭で見物人が息を呑んで果たし合いを見守る。",
];
static ST3A: Table = Table::from_dice("中世時代（リアル中世）シーン表", 2, 6, ST3A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST3B"]`（中世時代（妖と陰陽道）シーン表 / 2D6）の項目。
static ST3B_ITEMS: &[&str] = &[
    "雨。静寂の中で雨が木々を伝い滴り落ちる。",
    "荒れ果てた寺院。苔むした仏像の背後に潜む妖怪の影。",
    "屋敷の庭。庭石の間から薄暗い気配が漂い、周囲に不気味な冷気が立ち込める。",
    "茶屋の裏。薄暗い裏路地に、おぞましい声がこだまし、不気味な気配が漂う。",
    "京の大路。霧の中から現れる淡い人影が、鈴の音とともに闇へと消えていく。",
    "古木が立ち並ぶ街道。木々の影が不気味に揺れ動き、霧の中から視線を感じる。",
    "裏路地。夕闇に包まれる中、かげろうのような異形の影が街灯の下を横切り、静かに姿を消す。",
    "漁村の浜辺。海辺の静寂を破るように、ぼんやりとした人影が波打ち際に佇んでいる。",
    "神社の参道。淡い灯火の下、影が滑るように移動し、霧が周囲を包み込む。",
    "山奥の隠れ里。里の中央では火が焚かれ、不気味な影が火を囲むように揺れ動く。",
    "寺院の庭。柔らかな陽光が苔むした石庭を照らし、枝垂れ桜の花びらが風にそっと舞い落ちる。",
];
static ST3B: Table = Table::from_dice("中世時代（妖と陰陽道）シーン表", 2, 6, ST3B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST3C"]`（中世時代（剣と魔法）シーン表 / 2D6）の項目。
static ST3C_ITEMS: &[&str] = &[
    "雷雨。雷雲の間に魔物の群れが見え隠れする。戦いがすぐそこまで迫っている。",
    "港。大海原を望む船着場。冒険者はここから新たな地へ旅立つ。",
    "教会。神を祀る神聖な施設。ここを訪れれば、冒険者に降りかかってしまった邪気は払われるだろう。",
    "荒れ果てた大地。獰猛な猛禽類が冒険者の死骸をついばむ。毒の沼には危険な生物が巣くっている。",
    "森。豊かな緑は冒険者に時に恵を与え、時に試練を与えるだろう。エルフや精霊が顔を出すこともあるかもしれない。",
    "見果てぬ平原。穏やかな風が通り抜け、冒険者を旅へと誘う。草葉の陰に隠れた魔物に注意しろ。",
    "民家。地域の風習が色濃く反映された室内。冒険者を迎え入れ休息の時間を与えてくれることもあるだろう。",
    "城。城壁に囲まれた風格漂う時の君主の居城。冒険者にはそこに住むものからの依頼が待っているかもしれない。",
    "洞窟。暗い洞窟の奥には多くの魔物と金銀財宝が潜んでいるかもしれない。ドワーフが出迎えてくれることもあるだろう。",
    "雪深い山。極寒の山道だけでも危険が伴うのに、野蛮な山賊集団が冒険者を襲うこともしばしばだ。",
    "魔物の軍団。間一髪鉢合わせすることは避けられた。剣と魔法の世界の冒険にはこんな危険がつきものだ。",
];
static ST3C: Table = Table::from_dice("中世時代（剣と魔法）シーン表", 2, 6, ST3C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST4"]`（現代シーン表 / 2D6）の項目。
static ST4_ITEMS: &[&str] = &[
    "鳴り響く雷鳴と土砂降りの雨。嵐の路地でなすべきことをしろ。",
    "その地域の有名観光地。旅行者たちが自撮りにいそしんでいる。気楽なものだ。",
    "ブロックバスター映画のポスターが飾られた映画館。この時代の文化を知るには役に立つだろうか。",
    "ノートPCを開く人々でひしめき合う、有名コーヒーチェーン店。ひとまず落ち着こう。焦ってもいいことはない。",
    "古びた公園。撤去された遊具の支柱だけがさびたままに放置されており、少し寂しげだ。",
    "駅前のショッピングビル。この建物だけで欲しいものが全て手に入る。なんて便利な時代なんだ。",
    "鮮やかなネオンが照らす夜の繁華街。若者たちが新型の携帯通信端末を振り回しながら往来している。この時代が平和な証拠だろう。",
    "様々なテナントが軒を連ねる大型ショッピングモール。物質にあふれたこの時代に心の安らぎはあるのか。",
    "ヴィーガンメニューが並ぶスタイリッシュなレストラン。野菜だけを食べる習慣。この時代に流行りの多様性ってやつか。",
    "旅行者で賑わう空港のラウンジ。離着陸する飛行機は流体力学を利用した単純な作りだが、安全性は高いらしい。",
    "おだやかな風が通り抜ける。自然あふれるこんな場所も、この時代にあったのだな。",
];
static ST4: Table = Table::from_dice("現代シーン表", 2, 6, ST4_ITEMS);
/// Ruby `TABLES_MOD_2D["ST4A"]`（現代シーン表2 / 2D6）の項目。
static ST4A_ITEMS: &[&str] = &[
    "雨。傘を差した人々が行き交い、路面にできた水たまりが歩行者の足元で波紋を描く。",
    "駅。人々が整列し、ホームに並ぶ電車が次々と滑り込み、せわしない足音が絶えない。",
    "高速道路。車のライトが列を作り、赤や白の光が揺れながら遠くのビル群に向かって進む。",
    "公園。子供たちが遊具で遊ぶ声が響き、芝生にシートを広げてピクニックを楽しむ家族がいる。",
    "ビル街。窓に映る青空が、ガラス張りの高層ビル群を青く染め、車の列がせわしなく交差点を行き交う。",
    "住宅街の路地。低い塀に囲まれた家々が並び、洗濯物が風に揺れ、遠くから食事の香りが漂ってくる。",
    "繁華街。ネオンがカラフルに点滅し、店の呼び込みの声が響き、足早に歩く人々の影が路地に延びる。",
    "駅前のバスターミナル。発車を待つバスが並び、行き交う人々が目的地に向かって歩みを急ぐ。",
    "図書館。本棚が整然と並び、窓から差し込む光の中で人々が黙々と本を読みふける。",
    "展望台。無数の光が遠くまで広がり、カップルや観光客が窓の外の景色に見入っている。",
    "公園の芝生広場。のんびりとシートを広げて読書する人々の周りで、子供たちが駆け回る。",
];
static ST4A: Table = Table::from_dice("現代シーン表2", 2, 6, ST4A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST4B"]`（現代（近代日本）シーン表 / 2D6）の項目。
static ST4B_ITEMS: &[&str] = &[
    "雨。木製の雨樋から水が流れ落ち、傘を差した人々が濡れないように集まる。",
    "銭湯。脱衣所には桶やタオルが並び、湯煙が立ちこめる中で人々が疲れを癒している。",
    "料亭街。提灯が揺れる通りに和服姿の人々が行き交い、しっとりとした雰囲気が漂う。",
    "郵便局。窓口には行列ができ、制服を着た職員が手際よく封書や小包を扱う。",
    "路地裏の商店。小さな店の軒先に行灯が灯り、夕暮れの中、買い物帰りの人々がゆっくりと足を運ぶ。",
    "煉瓦造りの大通り。温かい陽が差し込み、洋装の若者たちが談笑する。",
    "田園。草の揺れる音と虫の声が静けさを彩り、農作業に励む人の姿が見える。",
    "港。古びた船が岸に繋がれ、働き手たちが忙しく荷物を運ぶ音が聞こえる。",
    "町工場。油の匂いが漂い、機械の音が絶え間なく響く中、職人たちが黙々と作業している。",
    "神社の境内。参道の砂利が足音を響かせ、古びた鳥居の向こうに木々が生い茂る。",
    "田舎道。木々が並ぶ静かな道を歩くと、遠くで列車の汽笛が響く。",
];
static ST4B: Table = Table::from_dice("現代（近代日本）シーン表", 2, 6, ST4B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST4C"]`（現代（西部開拓時代）シーン表 / 2D6）の項目。
static ST4C_ITEMS: &[&str] = &[
    "突然の雷雨。空が暗くなり、雷鳴が轟く。雨が大地に叩きつけられ、草がしなびて揺れる。",
    "納屋。農具や飼料が置かれ、開け放たれた扉から吹き込む風が積もった干し草を微かに揺らしている。",
    "小さな駅。貨物列車が到着し、業者たちが荷物を下ろし、商売の準備を進めている。",
    "小道。馬車がホコリを巻き上げ、行商人が商品を積んだ荷物を引きずりながら元気よく声を張り上げる。",
    "野営地跡。燃え残った焚き火の灰が広がり、打ち捨てられた空き瓶や壊れた椅子が散乱している。",
    "町の広場。砂埃舞う路地を行き交う人々の横を、タンブルウィードが転がってっゆく。",
    "酒場。陽気な音楽が流れ、客たちが踊り明かしながら、酒を楽しんでいる。",
    "木造の商店。壁に掲げられた看板には「肉屋」の文字が描かれ、香ばしい肉の匂いが漂っている。",
    "鉱山。労働者たちが鉄鋼のツルハシを持ち、掘削した鉱石を積み込む光景が広がっている。",
    "河川の渡し場。木製の渡し船が行き来し、旅人や業者が川を渡るために列を作って待つ。",
    "牧場。金色に輝く草原の中で、牛たちがのんびりと食事をしている。",
];
static ST4C: Table = Table::from_dice("現代（西部開拓時代）シーン表", 2, 6, ST4C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST5A"]`（超情報化時代（ユートピア）シーン表 / 2D6）の項目。
static ST5A_ITEMS: &[&str] = &[
    "雨。透明なドームがはじく、穏やかな水音が響き、虹色の光が水滴に反射し、幻想的な光景が広がる。",
    "海岸のビーチリゾート。ロボットがサンベッドのセッティングをし、訪れる人々が穏やかな海を楽しむ。",
    "空中農園。ロボットが自動で収穫した野菜がその場で調理され、フレッシュな料理が提供される。",
    "空中の交差点。スムーズに飛び交う空飛ぶ車が、信号に従って、流れるように通過していく。",
    "住宅街の街路。ロボットが庭の手入れをし、住民たちが微笑みを交わしながら生活する。",
    "都市の広場。ロボットたちが行き交う人々に道案内をし、さまざまな言語で話しかける。",
    "公園。ロボットが来園者に飲み物を提供し、子供たちがその周りで遊びながら楽しそうに笑い声を上げる。",
    "ショッピングモール。ロボットが商品を並べ、来店者に特売品を案内する。",
    "オフィスビル。ロボットが書類を整理し、社員たちの手助けをしながら、快適な仕事環境を整えている。",
    "高層ビルのテラス。人々が街を一望できる景色を楽しんでいる。",
    "花屋。ロボットが花束を作りながら、通りすがりの人々に笑顔で接客している姿が印象的だ。",
];
static ST5A: Table = Table::from_dice("超情報化時代（ユートピア）シーン表", 2, 6, ST5A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST5B"]`（超情報化時代（ディストピア）シーン表 / 2D6）の項目。
static ST5B_ITEMS: &[&str] = &[
    "雨。灰色の空から滴り落ちる雨水は人工のもの、かもしれない。",
    "暗い地下鉄の駅。無表情の乗客たちが自動改札を通り抜け、まるで機械のように移動している。",
    "医療施設。人々はAIによる診断と治療を受け、感情のないロボットが淡々と処置を進める。",
    "食堂。栄養管理された配給食に選択肢はなく、行列に並んだ人々が無表情でそれ受け取っている。",
    "オフィスビル。AIが効率を追求し、笑顔のない労働者たちに厳しい指示を出している。",
    "街角。人々は無表情で歩き、ロボットがリアルタイムで行動を監視している。全ての動きがデータとして記録され、自由な発言は封じられている。",
    "道路。自動運転車の運点はAIによって制御されており、人々に自由な移動は許されていない。",
    "街路。氾濫する広告が個人の嗜好に合わせた広告を延々と流し続け、無意識に消費を促される。",
    "一律のデザインの住居。住民たちは個性や夢を奪われ、皆同じような生活を送っている。",
    "データセンター。無機質な空間で、AIが全ての情報を処理し、個々の市民の生活を支配している。",
    "都市の郊外。脱走者に対応するため監視ロボットが巡回している。",
];
static ST5B: Table = Table::from_dice("超情報化時代（ディストピア）シーン表", 2, 6, ST5B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST5C"]`（超情報化時代（サイバーパンク）シーン表 / 2D6）の項目。
static ST5C_ITEMS: &[&str] = &[
    "雨。工場排煙まじりの雨粒がエアカーのフロントガラスに黒い筋を残す。",
    "高層ビル屋上。都市の全景が見渡せる。密売人と違法チップを取り引きするには丁度いい場所だ。",
    "油で汚れた整備工場。改造車が何台も並び、整備士が工具を手に違法改造に手を染めている。",
    "人工川沿いのスラム街。水面にはゴミが浮き、薄暗い小屋の中では密造酒を作る者の影が揺れている。",
    "ゴミで溢れる裏通り。人工皮膚が剥がれかけたロボットがゴミ捨て場を漁り、その隣でホームレスが震えている。",
    "ネオンで彩られた歓楽街。中華風の提灯が並ぶ通りの裏で、怪しい情報屋たちが情報交換に勤しむ。",
    "クラブ。脳内音楽装置を装着したトランス状態の客が踊り狂い、新作の薬物を売りさばく密売人がうろつく。",
    "ガラス張りの高層ビル。都市の中心を支配するようにそびえ立ち、無数のホログラム広告が壁面を飾る。",
    "サイボーグ修理屋の工房。雑然とした部品の山のなか、怪しい光に照らされた手術台が次の患者を待つ。",
    "廃れた教会の礼拝堂。犯罪者たちがここを隠れ家にしているのか、壁には反乱の計画図が貼られている。",
    "地下カジノ。違法改造されたサイバーギャンブルマシンに接続した客たちが巨額のデータを賭けている。",
];
static ST5C: Table = Table::from_dice("超情報化時代（サイバーパンク）シーン表", 2, 6, ST5C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST6A"]`（宇宙時代（地球人類銀河帝国）シーン表 / 2D6）の項目。
static ST6A_ITEMS: &[&str] = &[
    "宇宙軍基地。広大な格納庫には最新鋭の戦闘艦が整列し、兵士たちが訓練に汗を流している。",
    "宇宙貿易センター。光り輝く宇宙船が連なり、各惑星の商人が市場で独自の特産品を売り込んでいる。",
    "図書館。整然と並ぶデータ端末の中で、研究者たちが古代の記録を静かに調べている。",
    "帝国宮殿の大広間。天井に映る星図が静かに回転し、宮廷官僚たちが低い声で議論を交わしている。",
    "学術センター。精密なホログラム装置が古代の科学理論を再現し、学生たちがその光景を真剣に見つめる。",
    "中央広場。ホログラム広告が空中に浮かび、歩行者が往来する中、露店が帝国の最新技術を売り込む。",
    "居住区。密集した住居の間をつなぐ細い路地を清掃ドローンが整備している。",
    "宇宙港。巨大な貨物船や民間の探査船、個人所有のプライベート機など様々な宇宙船が離着陸する。",
    "広大な荒野。風が吹き抜ける草原には星間交易所の廃墟。遠くには赤い惑星が沈んでいく。",
    "巨大農場。地平線まで続く作物を収穫ドローンが滑らかに行き来し、遠くの倉庫への積み込みを続ける。",
    "赤い砂漠。銀色に輝く鉱石が地表を覆う。炭鉱夫たちが鉱山用スーツに身を包み、採掘に精を出す。",
];
static ST6A: Table = Table::from_dice("宇宙時代（地球人類銀河帝国）シーン表", 2, 6, ST6A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST6B"]`（宇宙時代（異形の隣人たち）シーン表 / 2D6）の項目。
static ST6B_ITEMS: &[&str] = &[
    "漂流市場。無重力空間に浮かぶ船団が商業区を形成し、異形の商人たちが商売に精を出す。",
    "虹の渓谷。多彩な色彩を放つ岩石が広がり、色を吸収して輝く独特な皮膚の種族が暮らしている。",
    "水晶洞窟。壁一面に輝く巨大な水晶が広がり、発光するエイリアンが静かに洞窟を照らしている。",
    "反転都市。重力が逆転した建物が逆さに建設された街だ。とある異星人の文化圏で最近流行のものだ。",
    "星間駅。様々な種族が集う巨大な宇宙港で、甲殻を持つクルーが船を修理し、喧騒が絶えない。",
    "丘陵集落。緩やかな丘が連なる地形の住民たちは石畳の道を歩きながら小競り合いを繰り返している。",
    "エイリアンの巣。巨大な蟻塚のような構造が広がり、種族全体が協調して作業を行っている。",
    "バイオ都市。生物そのものが建築物を形成し、住民たちは共生しながら建物を「成長」させている。",
    "闘技場。多脚のエイリアンたちが観客席を埋め尽くし、中央では獣に似た戦士たちが戦いを繰り広げている。",
    "空中庭園。巨大な浮遊植物が街を覆い、羽根の生えた種族が花粉を運びながら優雅に飛び回っている。",
    "海底都市。透明なドームに囲まれた街で、魚のような体を持つ住民が自由に泳ぎながら生活している。",
];
static ST6B: Table = Table::from_dice("宇宙時代（異形の隣人たち）シーン表", 2, 6, ST6B_ITEMS);
/// Ruby `TABLES_MOD_2D["ST6C"]`（宇宙時代（遺棄された地球）シーン表 / 2D6）の項目。
static ST6C_ITEMS: &[&str] = &[
    "豪雨。何もかもを洗い流す滝のような雨が降り注ぎ、地上に残された廃棄物の砂埃をすすぐ。",
    "故障した軌道エレベーター。レールは割れたガラスで覆われ、誰も足を踏み入れることはない。",
    "廃れたオペラハウス。割れたガラス窓から草が伸び、かつての繁栄を偲ばせる形骸だけが静かに残る。",
    "廃墟となった電車車両基地。錆びついた列車が放置され、ホームには無数の雑草が生い茂る。",
    "崩れた高層ビル街。植物が建物を侵食し、住む者は少なく、瓦礫の間で古びたドラム缶の火が揺れる。",
    "ゴーストタウン。ひび割れたアスファルトの街路に、朽ち果てた高級車が何台も折り重なっている。",
    "繁華街だった区画。看板が倒れ、荒れた路地にはゴミが吹き溜まり、かつての喧騒が嘘のようだ。",
    "汚染された河川。濁った水面に倒壊した橋の影が映る。",
    "枯れた海岸線。干からびた海底が露出し、壊れたボートが砂の中に沈んでいる。",
    "廃工場地帯。黒ずんだ煙突がそびえ立ち、工場跡地を荒らすスカベンジャーたちの姿も見える。",
    "浸食された発電施設。砂に埋もれたソーラーパネルが住民が風よけとして使うだけのものになっている。",
];
static ST6C: Table = Table::from_dice("宇宙時代（遺棄された地球）シーン表", 2, 6, ST6C_ITEMS);
/// Ruby `TABLES_MOD_2D["ST6S"]`（宇宙時代（宇宙船船内）シーン表 / 2D6）の項目。
static ST6S_ITEMS: &[&str] = &[
    "避難ポッドエリア。非常時に備えて整然と配置されたポッドが、クルーに安全を保証している。",
    "武装デッキ。船体に並ぶ無数のタレットが、敵対船を迎え撃つ準備を整えながら静止している。",
    "貨物デッキ。宇宙資源が詰め込まれたコンテナが整然と並び、作業用ドローンが効率的に動き回る。",
    "医療ラボ。銀色の壁に囲まれ、自動診断装置が乗員の健康状態をスキャンし、治療を施している。",
    "観測デッキ。透明なドーム越しに星々が映し出され、恒星間航行に必要な情報を集めている。",
    "居住区画。清潔な廊下が続き、各部屋では乗員たちがリラックスしながら次のミッションに備える。",
    "食堂ホール。人工重力の中で乗員たちが窓の外に広がる星雲を背に軽食を楽しみ、ミーティングを行なう。",
    "艦橋－司令ブリッジ。広大なスクリーンに映し出される銀河系の地図を前に艦長席が鎮座する。",
    "中枢制御ルーム。AIが運営する無数のスクリーンが並び、クルーは次元航行の計算結果の確認が可能だ。",
    "冷凍睡眠ルーム。乗員は長距離航行のために永い眠りにつく。薄い霧が漂う静寂の空間。",
    "エンジンルーム。無数の光を放つコアが脈動し、船全体にエネルギーを供給しながら船の推進力を生成する。",
];
static ST6S: Table = Table::from_dice("宇宙時代（宇宙船船内）シーン表", 2, 6, ST6S_ITEMS);
/// Ruby `TABLES_MOD_2D["ST0"]`（開闢時代シーン表 / 2D6）の項目。
static ST0_ITEMS: &[&str] = &[
    "酸の雨。鉛色の空一面に広がる厚い雲から強酸性の雨が降り注ぎ、大地を浸食する。",
    "硫黄の霧。視界が遮られるほど濃密な霧が地表を包み込み、生命の気配はない。",
    "沸騰する湖。酸性の湖が泡立ち、黄色い硫黄が湖面に漂う。",
    "灼熱の砂漠。高温で焼けつく砂が広がり、どこまでも乾いた熱風が吹き荒れる。",
    "絶えず噴き上がる火山。灰色の煙と赤い火柱が空を裂き、終わりのない轟音が大地を揺らす。",
    "赤黒く染まった大地。灼熱のマグマが地表を覆い、激しい硫黄の臭いが立ち込める。",
    "焼け焦げた岩の裂け目。マグマが裂け目から吹き出し、煙と蒸気が絶えず空を覆う。",
    "激しく流れる溶岩の川。赤く光る溶岩が地形を飲み込み、ゆっくりと広がっていく。",
    "灰が降り積もる大地。空から灰が舞い降り、地表が一面灰色の層に覆われている。",
    "崩れ落ちる岩山。熱と酸が山肌を溶かし、大量の岩石が地響きを立てながら崩れていく。",
    "荒れ狂う酸の海。緑色に輝く海が泡立ち、鋭い蒸気が空気中に溶け込む。",
];
static ST0: Table = Table::from_dice("開闢時代シーン表", 2, 6, ST0_ITEMS);
/// Ruby `TABLES_MOD_2D["ST7A"]`（終局時代シーン表 / 2D6）の項目。
static ST7A_ITEMS: &[&str] = &[
    "情報の雨。0と1で表された無数の雨粒が落ちるたび、解釈の海へ波紋を広げていく。",
    "無限に広がる光の平原。人々が光の波を利用して思考を共有し、会話を交わしている。",
    "果てしないデータの川。人々が情報の流れを泳ぎながら、必要な記憶やスキルを直接ダウンロードしている。",
    "次元を超える門。複数の仮想世界を繋ぎ、人々が自由に異なる現実を行き来している。",
    "星屑の海に浮かぶ都市。高次元に投影された建物が、感情や思考に応じた空間に自動変換される。",
    "浮遊する立体迷宮。人々が迷宮の構造そのものとなり、複雑なパズルとして他者と意思疎通を行っている。",
    "迷宮図書館。動的に変化する無限の階層が、知識の探求者を試すかのように新たな道を生み出す。",
    "永遠に続く階段の街。上昇するほど複雑な思考が展開され、人々が階層ごとに異なる哲学を学んでいる。",
    "無重力の感情の池。人々が感情の波紋を送り合い、それが形を持つ生命体のように進化していく。",
    "虹色の空間の渦巻き。各色が異なる感覚を司り、人々がそれぞれの色を渡り歩いて体験を共有している。",
    "三次元宇宙。エントロピーが最大となり冷え切った宇宙。現実界。ここでは何も起こらない。",
];
static ST7A: Table = Table::from_dice("終局時代シーン表", 2, 6, ST7A_ITEMS);
/// Ruby `TABLES_MOD_2D["ST7B"]`（終局時代（無限図書館）シーン表 / 2D6）の項目。
static ST7B_ITEMS: &[&str] = &[
    "思考の海。意識そのものが溶け合い、無限の知識が波紋のように広がっていく空間。",
    "記憶を渡す橋。互いの情報を触れることで交換し、橋の中央で一体化する儀式的空間。",
    "無音知識の間。音も光もない中、情報体同士の波動だけで静かに情報が交換されていく。",
    "記録結晶の林。情報が結晶化した無数の構造体がそびえ立ち、触れるだけで思念が流れ込む。",
    "光速思念の走路。情報体たちが光の粒となり、知の奔流として空間中を駆け抜ける。",
    "概念の庭園。花や木に似た形の情報体が咲き、触れた者に抽象的な真理を伝える。",
    "意識共鳴の円形広場。互いの存在が近づくたびに波紋のような知識の振動が広がっていく。",
    "存在記録の碑石。訪れたすべての情報体の思念の痕跡が刻まれ、時を超えて残される記憶の石板。",
    "情報圧縮の洞窟。膨大な知識が極限まで圧縮されて保存され、意識が接続して解凍を試みる。",
    "多層時間の閲覧室。同時に複数の時空に存在できる情報体たちが並行して過去未来を読んでいる。",
    "全存在が一瞬で繋がる界面。一つの接触が、全図書空間と即座に繋がるポータルとして機能する。",
];
static ST7B: Table = Table::from_dice("終局時代（無限図書館）シーン表", 2, 6, ST7B_ITEMS);
/// Ruby `TABLES_MOD_2D["TT"]`（タイムトラベル演出表 / 2D6）の項目。
static TT_ITEMS: &[&str] = &[
    "科学者の実験で発生した七色の光を浴びてしまい、あなたの身体は分解され、違う時代で再構成された。",
    "科学者が作った物の大きさを自在に変化させる量子物質に触れてしまい、あなたの体は微小レベルまで縮んでしまった。量子世界をしばらくさまよったあと、運よく元の大きさに戻れたが別の時代へと移動してしまっていた。",
    "あなたは偶然手に入れた腕時計を気まぐれで腕に装着した。時刻を合わせようとをリューズを回すと、時空にゆがみが生じ、タイムトラベルしてしまった。腕時計はその力を使い果たし粉々にくだけた。",
    "あなたの乗った乗物が突如超高速で走り始めた。それは光速に近づきつつあった。いや、すでに光速を超えているかもしれない。光速に限りなく近い速度なら未来に、光速を超えたスピードなら過去に移動してしまうだろう。",
    "科学者が試乗をしてくれというので、乗り込んだ車が怪しげなメカが搭載されたタイムマシンだった。帰りの分の燃料は当然ない。",
    "雷に打たれ、気を失った。そして目を覚ますと、あなたがもといた時代とは、違う時代へ来てしまっていた。",
    "あなたの目の前に偶然ワームホールが出現する。ワームホールから放たれる引力には抗えず、あなたは時空を超えてしまう。",
    "緑色に光る怪しげな石を見つけたあなたは、その美しさに魅入られてしまう。どれくらい時がたっただろうか。気づけはあなたは違う時代にいた。",
    "偶然手に入れた謎の書物に書かれた呪文を読み上げた瞬間、あなたの身体は光に包まれ、違う時代へと転送されてしまった。",
    "あなたは黒ずくめの服を着た謎の組織に拘束され、怪しげなクスリを飲まされてしまった。気づけばそこは違う時代だった。",
    "自宅の机の引き出しを開けると、そこは混沌の空間が広がっていた。青い腕の先についた指のない白い手があなたを掴み（指がないにも関わらず、だ）、怪しげな板にあなたを乗せた。正体不明の青いずんぐりとしたフォルムの存在はその板に設置された操縦桿を操り、混沌の中を進んだ。しばらくしてあなたはもといた時代とは全く別の場所へ放り出された。",
];
static TT: Table = Table::from_dice("タイムトラベル演出表", 2, 6, TT_ITEMS);

/// Ruby `TABLES_MOD_2D`。コマンド名 → 表。
static TABLES_MOD_2D: &[(&str, &Table)] = &[
    ("ACT", &ACT),
    ("ST1A", &ST1A),
    ("ST1B", &ST1B),
    ("ST1C", &ST1C),
    ("ST2A", &ST2A),
    ("ST2B", &ST2B),
    ("ST2C", &ST2C),
    ("ST3A", &ST3A),
    ("ST3B", &ST3B),
    ("ST3C", &ST3C),
    ("ST4", &ST4),
    ("ST4A", &ST4A),
    ("ST4B", &ST4B),
    ("ST4C", &ST4C),
    ("ST5A", &ST5A),
    ("ST5B", &ST5B),
    ("ST5C", &ST5C),
    ("ST6A", &ST6A),
    ("ST6B", &ST6B),
    ("ST6C", &ST6C),
    ("ST6S", &ST6S),
    ("ST0", &ST0),
    ("ST7A", &ST7A),
    ("ST7B", &ST7B),
    ("TT", &TT),
];

// ---------------------------------------------------------------------------
// Ruby `TABLES_MOD_1D`（1D6の表（出目指定・修正値つき））
// ---------------------------------------------------------------------------

/// Ruby `TABLES_MOD_1D["RT"]`（帰還演出表 / 1D6）の項目。
static RT_ITEMS: &[&str] = &[
    "この時代に来た方法と同じ演出で帰還できる。",
    "この時代に来た方法と同じ演出で帰還できる。",
    "この時代に来た方法と同じ演出で帰還できる。",
    "少し目を閉じて、故郷へ想いを馳せる。眼を開けると懐かしい景色が広がっている。元の時代へ戻って来たのだ。",
    "目の前の空間に別時代へのポータルが開く。それをくぐればあなたの住んでいた元の時代だ。",
    "天から神々しい光が降りそそぐ。宇宙開闢の女神が微笑みかけると、あなたは強い光に包まれる。その光はあなたを元の時代へと導く。",
];
static RT: Table = Table::from_dice("帰還演出表", 1, 6, RT_ITEMS);
/// Ruby `TABLES_MOD_1D["CPT"]`（ポジティブ因縁内容表 / 1D6）の項目。
static CPT_ITEMS: &[&str] = &[
    "共存。一緒にいて自然な関係だ。",
    "互助。つらい時にはいつでもそばにいた。",
    "同志。共に道を歩むかけがえのない仲間だ。",
    "片愛。あなたは、相手のことが大好きだ。",
    "相愛。お互いのことが大好きだ。",
    "理解。何も言わなくても相手のことならなんでもわかる。",
];
static CPT: Table = Table::from_dice("ポジティブ因縁内容表", 1, 6, CPT_ITEMS);
/// Ruby `TABLES_MOD_1D["CNT"]`（ネガティブ因縁内容表 / 1D6）の項目。
static CNT_ITEMS: &[&str] = &[
    "邪魔。なぜかいつも視界の端にいる。",
    "不快。一緒にいるとちょっとイラつく。",
    "厄介。関わりたくもないのに、いつもちょっかいを出してくる。",
    "嫌悪。やることなすことすべてが気に食わない。",
    "憎悪。過去の恨みか、激しい感情を持っている。",
    "天敵。不倶戴天の敵、いつでも対立して喧嘩ばかりしている。",
];
static CNT: Table = Table::from_dice("ネガティブ因縁内容表", 1, 6, CNT_ITEMS);
/// Ruby `TABLES_MOD_1D["IT"]`（アイテム決定表 / 1D6）の項目。
static IT_ITEMS: &[&str] = &[
    "癒しの品。いつでも使用可能。好きなキャラクター（自身含む）の［疲労度］を1D6点減少させることができる。使用すると失われる。",
    "癒しの品。いつでも使用可能。好きなキャラクター（自身含む）の［疲労度］を1D6点減少させることができる。使用すると失われる。",
    "幸運の品。誰か（自身含む）の判定のサイコロを振った直後に使用可能。自身の［改変度］を1点増加すれば、その判定にプラス1の修正をつけることができる。使用すると失われる。",
    "幸運の品。誰か（自身含む）の判定のサイコロを振った直後に使用可能。自身の［改変度］を1点増加すれば、その判定にプラス1の修正をつけることができる。使用すると失われる。",
    "運命の品。誰か（自身含む）がシステムおよびシナリオで用意された表を使用してダイスを振った直後に使用可能。ダイスの結果を±1ずらすことができる。ただしその表に設定されていない値にずらすことはできない。使用すると失われる。",
    "運命の品。誰か（自身含む）がシステムおよびシナリオで用意された表を使用してダイスを振った直後に使用可能。ダイスの結果を±1ずらすことができる。ただしその表に設定されていない値にずらすことはできない。使用すると失われる。",
];
static IT: Table = Table::from_dice("アイテム決定表", 1, 6, IT_ITEMS);
/// Ruby `TABLES_MOD_1D["AGT"]`（時代決定表 / 1D6）の項目。
static AGT_ITEMS: &[&str] = &[
    "原始時代／EL1",
    "古代／EL2",
    "中世時代／EL3",
    "現代／EL4",
    "超情報化時代／EL5",
    "宇宙時代／EL6",
];
static AGT: Table = Table::from_dice("時代決定表", 1, 6, AGT_ITEMS);
/// Ruby `TABLES_MOD_1D["MCT"]`（メインクラス決定表 / 1D6）の項目。
static MCT_ITEMS: &[&str] = &[
    "基本クラス表 BCT を使用する",
    "基本クラス表 BCT を使用する",
    "基本クラス表 BCT を使用する",
    "基本クラス表 BCT を使用する",
    "追加クラス（メインクラス専用）表 AMCT を使用する",
    "追加クラス（メインクラス専用）表 AMCT を使用する",
];
static MCT: Table = Table::from_dice("メインクラス決定表", 1, 6, MCT_ITEMS);
/// Ruby `TABLES_MOD_1D["SCT"]`（サブクラス決定表 / 1D6）の項目。
static SCT_ITEMS: &[&str] = &[
    "基本クラス表 BCT を使用する",
    "基本クラス表 BCT を使用する",
    "追加クラス（サブクラス専用）表１ ASCT1 を使用する",
    "追加クラス（サブクラス専用）表１ ASCT1 を使用する",
    "追加クラス（サブクラス専用）表２ ASCT2 を使用する",
    "追加クラス（サブクラス専用）表２ ASCT2 を使用する",
];
static SCT: Table = Table::from_dice("サブクラス決定表", 1, 6, SCT_ITEMS);
/// Ruby `TABLES_MOD_1D["BCT"]`（基本クラス表 / 1D6）の項目。
static BCT_ITEMS: &[&str] = &[
    "原始人",
    "古代人",
    "中世期人",
    "現代人",
    "近未来人",
    "宇宙人",
];
static BCT: Table = Table::from_dice("基本クラス表", 1, 6, BCT_ITEMS);
/// Ruby `TABLES_MOD_1D["AMCT"]`（追加クラス（メインクラス専用）表 / 1D6）の項目。
static AMCT_ITEMS: &[&str] = &[
    "開闢期人",
    "開闢期人",
    "終末期人",
    "終末期人",
    "メインクラス決定表 MCT から振りなおす",
    "メインクラス決定表 MCT から振りなおす",
];
static AMCT: Table = Table::from_dice("追加クラス（メインクラス専用）表", 1, 6, AMCT_ITEMS);
/// Ruby `TABLES_MOD_1D["ASCT1"]`（追加クラス（サブクラス専用）表1 / 1D6）の項目。
static ASCT1_ITEMS: &[&str] = &["恐竜人", "天界人", "亜人", "近代人", "機械人", "異星人"];
static ASCT1: Table = Table::from_dice("追加クラス（サブクラス専用）表1", 1, 6, ASCT1_ITEMS);
/// Ruby `TABLES_MOD_1D["ASCT2"]`（追加クラス（サブクラス専用）表2 / 1D6）の項目。
static ASCT2_ITEMS: &[&str] = &[
    "軟体人",
    "高次元人",
    "サブクラス決定表 SCT から振りなおす",
    "サブクラス決定表 SCT から振りなおす",
    "サブクラス決定表 SCT から振りなおす",
    "サブクラス決定表 SCT から振りなおす",
];
static ASCT2: Table = Table::from_dice("追加クラス（サブクラス専用）表2", 1, 6, ASCT2_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT0"]`（開闢時代経歴表決定表 / 1D6）の項目。
static CTT0_ITEMS: &[&str] = &[
    "軟体人経歴表 CTAM",
    "軟体人経歴表 CTAM",
    "軟体人経歴表 CTAM",
    "軟体人経歴表 CTAM",
    "軟体人経歴表 CTAM",
    "軟体人経歴表 CTAM",
];
static CTT0: Table = Table::from_dice("開闢時代経歴表決定表", 1, 6, CTT0_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT1"]`（原始時代経歴表決定表 / 1D6）の項目。
static CTT1_ITEMS: &[&str] = &[
    "原始時代経歴表 CT1",
    "原始時代経歴表 CT1",
    "恐竜人経歴表 CTD",
    "恐竜人経歴表 CTD",
    "天界人経歴表 CTG",
    "天界人経歴表 CTG",
];
static CTT1: Table = Table::from_dice("原始時代経歴表決定表", 1, 6, CTT1_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT2"]`（古代経歴表決定表 / 1D6）の項目。
static CTT2_ITEMS: &[&str] = &[
    "古代経歴表 CT2",
    "古代経歴表 CT2",
    "天界人経歴表 CTG",
    "天界人経歴表 CTG",
    "異星人経歴表 CTAL",
    "異星人経歴表 CTAL",
];
static CTT2: Table = Table::from_dice("古代経歴表決定表", 1, 6, CTT2_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT3"]`（中世時代経歴表決定表 / 1D6）の項目。
static CTT3_ITEMS: &[&str] = &[
    "中世時代経歴表 CT3",
    "亜人（ハイファンタジー種族）経歴表 CTFR",
    "亜人（ハイファンタジー魔物）経歴表 CTFM",
    "亜人（妖怪）経歴表 CTY",
    "天界人経歴表 CTG",
    "この表を振り直す CTT3",
];
static CTT3: Table = Table::from_dice("中世時代経歴表決定表", 1, 6, CTT3_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT4"]`（現代経歴表決定表 / 1D6）の項目。
static CTT4_ITEMS: &[&str] = &[
    "現代経歴表 CT4",
    "現代経歴表 CT4",
    "近代人（明治・大正・昭和）経歴表 CTJ",
    "近代人（明治・大正・昭和）経歴表 CTJ",
    "近代人（西部開拓時代）経歴表 CTF",
    "近代人（西部開拓時代）経歴表 CTF",
];
static CTT4: Table = Table::from_dice("現代経歴表決定表", 1, 6, CTT4_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT5"]`（超情報化時代経歴表決定表 / 1D6）の項目。
static CTT5_ITEMS: &[&str] = &[
    "超情報化時代経歴表 CT5",
    "亜人（ミュータント）経歴表 CTM",
    "機械人経歴表 CTR",
    "恐竜人経歴表 CTD",
    "この表を振り直す CTT5",
    "この表を振り直す CTT5",
];
static CTT5: Table = Table::from_dice("超情報化時代経歴表決定表", 1, 6, CTT5_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT6"]`（宇宙時代経歴表決定表 / 1D6）の項目。
static CTT6_ITEMS: &[&str] = &[
    "宇宙時代経歴表 CT6",
    "宇宙時代経歴表 CT6",
    "異星人経歴表 CTAL",
    "異星人経歴表 CTAL",
    "機械人経歴表 CTR",
    "機械人経歴表 CTR",
];
static CTT6: Table = Table::from_dice("宇宙時代経歴表決定表", 1, 6, CTT6_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT7"]`（終局経歴表決定表 / 1D6）の項目。
static CTT7_ITEMS: &[&str] = &[
    "高次元人経歴表 CTAD",
    "高次元人経歴表 CTAD",
    "高次元人経歴表 CTAD",
    "高次元人経歴表 CTAD",
    "高次元人経歴表 CTAD",
    "高次元人経歴表 CTAD",
];
static CTT7: Table = Table::from_dice("終局経歴表決定表", 1, 6, CTT7_ITEMS);
/// Ruby `TABLES_MOD_1D["CTTD"]`（亜人経歴表決定表 / 1D6）の項目。
static CTTD_ITEMS: &[&str] = &[
    "亜人（ハイファンタジー種族）経歴表 CTFR",
    "亜人（ハイファンタジー魔物）経歴表 CTFM",
    "亜人（妖怪）経歴表 CTY",
    "亜人（ミュータント）経歴表 CTM",
    "この表を振り直す CTTD",
    "この表を振り直す CTTD",
];
static CTTD: Table = Table::from_dice("亜人経歴表決定表", 1, 6, CTTD_ITEMS);
/// Ruby `TABLES_MOD_1D["CTT4M"]`（近代人経歴表決定表 / 1D6）の項目。
static CTT4M_ITEMS: &[&str] = &[
    "近代人（明治・大正・昭和）経歴表 CTJ",
    "近代人（明治・大正・昭和）経歴表 CTJ",
    "近代人（明治・大正・昭和）経歴表 CTJ",
    "近代人（西部開拓時代）経歴表 CTF",
    "近代人（西部開拓時代）経歴表 CTF",
    "近代人（西部開拓時代）経歴表 CTF",
];
static CTT4M: Table = Table::from_dice("近代人経歴表決定表", 1, 6, CTT4M_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT0"]`（開闢時代名前表決定表 / 1D6）の項目。
static NTT0_ITEMS: &[&str] = &[
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
    "軟体人名前表／男性名 NMTAM ／女性名 NFTAM",
];
static NTT0: Table = Table::from_dice("開闢時代名前表決定表", 1, 6, NTT0_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT1"]`（原始時代名前表決定表 / 1D6）の項目。
static NTT1_ITEMS: &[&str] = &[
    "原始時代名前表／男性名 NMT1 ／女性名 NFT1",
    "原始時代名前表／男性名 NMT1 ／女性名 NFT1",
    "恐竜人名前表／男性名 NMTD ／女性名 MFTD",
    "恐竜人名前表／男性名 NMTD ／女性名 MFTD",
    "天界人名前表決定表 NTTG",
    "天界人名前表決定表 NTTG",
];
static NTT1: Table = Table::from_dice("原始時代名前表決定表", 1, 6, NTT1_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT2"]`（古代名前表決定表 / 1D6）の項目。
static NTT2_ITEMS: &[&str] = &[
    "古代名前表／男性名 NMT2 ／女性名 NFT2",
    "古代（日本）名前表／男性名 NMT2J ／女性名 NFT2J",
    "古代（中国）名前表 NMT2C を2回か3回振った結果を繋げる",
    "天界人名前表決定表 NTTG",
    "異星人名前表／男性名 NMTAL ／女性名 NFTAL",
    "この表を振り直す NTT2",
];
static NTT2: Table = Table::from_dice("古代名前表決定表", 1, 6, NTT2_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT3"]`（中世時代名前表決定表 / 1D6）の項目。
static NTT3_ITEMS: &[&str] = &[
    "中世時代（西洋）名前表／男性名 NMT3W ／女性名 NFT3W ／姓 NLT3W",
    "中世時代（日本）名前表／男性名 NMT3 ／女性名 NFT3 ／姓 NLT3",
    "亜人（ハイファンタジー種族）名前表／男性名 NMTFR ／女性名 NFTFR ／姓  NLTFR",
    "亜人（ハイファンタジー魔物）名前表／男性名 NMTFM ／女性名 NFTFM",
    "亜人（妖怪）名前表／男性名 NMTY ／女性名 NFTY",
    "天界人名前表決定表 NTTG",
];
static NTT3: Table = Table::from_dice("中世時代名前表決定表", 1, 6, NTT3_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT4"]`（現代名前表決定表 / 1D6）の項目。
static NTT4_ITEMS: &[&str] = &[
    "現代（西洋）名前表／男性名 NMT4W ／女性名 NFT4W ／姓 NLT4W",
    "現代（日本）名前表／男性名 NMT4 ／女性名 NFT4 ／姓 NLT4",
    "近代人（明治・大正・昭和）名前表／男性名 NMT4J ／女性名 NFT4J ／姓 NLT4J",
    "近代人（西部開拓時代）名前表／男性名 NMT4F ／女性名 NFT4F ／姓 NLT4F",
    "この表を振り直す NTT4",
    "この表を振り直す NTT4",
];
static NTT4: Table = Table::from_dice("現代名前表決定表", 1, 6, NTT4_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT5"]`（超情報化時代名前表決定表 / 1D6）の項目。
static NTT5_ITEMS: &[&str] = &[
    "超情報化時代名前表／男性名 NMT5 ／女性名 NFT5 ／姓 NLT5",
    "亜人（ミュータント）名前表／男性名 NMTM ／女性名 NFTM ／姓 NLTM",
    "機械人名前表／プレフィックス NPTR ／型番 NMTR ／愛称 NNTR",
    "恐竜人名前表／男性名 NMTD ／女性名 MFTD",
    "この表を振り直す NTT5",
    "この表を振り直す NTT5",
];
static NTT5: Table = Table::from_dice("超情報化時代名前表決定表", 1, 6, NTT5_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT6"]`（宇宙時代名前表決定表 / 1D6）の項目。
static NTT6_ITEMS: &[&str] = &[
    "宇宙時代名前表／男性名 NMT6 ／女性名 NFT6 ／姓 NLT6",
    "宇宙時代名前表／男性名 NMT6 ／女性名 NFT6 ／姓 NLT6",
    "異星人名前表／男性名 NMTAL ／女性名 NFTAL",
    "異星人名前表／男性名 NMTAL ／女性名 NFTAL",
    "機械人名前表／プレフィックス NPTR ／型番 NMTR ／愛称 NNTR",
    "機械人名前表／プレフィックス NPTR ／型番 NMTR ／愛称 NNTR",
];
static NTT6: Table = Table::from_dice("宇宙時代名前表決定表", 1, 6, NTT6_ITEMS);
/// Ruby `TABLES_MOD_1D["NTT7"]`（終局時代名前表決定表 / 1D6）の項目。
static NTT7_ITEMS: &[&str] = &[
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
    "高次元人名前表／男性名 NMTAD ／女性名 NFTAD",
];
static NTT7: Table = Table::from_dice("終局時代名前表決定表", 1, 6, NTT7_ITEMS);
/// Ruby `TABLES_MOD_1D["NTTG"]`（天界人名前表決定表 / 1D6）の項目。
static NTTG_ITEMS: &[&str] = &[
    "天界人（ギリシャ神話）名前表／男性名 NMTGG ／女性名 NFTGG",
    "天界人（日本神話）名前表／男性名 NMTGJ ／女性名 NFTGJ",
    "天界人（北欧神話）名前表／男性名 NMTGN ／女性名 NFTGN",
    "天界人（エジプト神話）名前表／男性名 NMTGE ／女性名 NFTGE",
    "天界人（メソポタミア神話）名前表／男性名 NMTGM ／女性名 NFTGM",
    "天界人（インド神話）名前表／男性名 NMTGI ／女性名 NFTGI",
];
static NTTG: Table = Table::from_dice("天界人名前表決定表", 1, 6, NTTG_ITEMS);
/// Ruby `TABLES_MOD_1D["NTTD"]`（亜人名前表決定表 / 1D6）の項目。
static NTTD_ITEMS: &[&str] = &[
    "亜人（ハイファンタジー種族）名前表／男性名 NMTFR ／女性名 NFTFR ／姓  NLTFR",
    "亜人（ハイファンタジー魔物）名前表／男性名 NMTFM ／女性名 NFTFM",
    "亜人（妖怪）名前表／男性名 NMTY ／女性名 NFTY",
    "亜人（ミュータント）名前表／男性名 NMTM ／女性名 NFTM ／姓 NLTM",
    "この表を振り直す NTTD",
    "この表を振り直す NTTD",
];
static NTTD: Table = Table::from_dice("亜人名前表決定表", 1, 6, NTTD_ITEMS);
/// Ruby `TABLES_MOD_1D["NMT2J"]`（古代（日本）名前表／男性名 / 1D6）の項目。
static NMT2J_ITEMS: &[&str] = &[
    "タケル",
    "クマソ",
    "ウジマサ",
    "オオビコ",
    "アリワケ",
    "ヤマト",
];
static NMT2J: Table = Table::from_dice("古代（日本）名前表／男性名", 1, 6, NMT2J_ITEMS);
/// Ruby `TABLES_MOD_1D["NFT2J"]`（古代（日本）名前表／女性名 / 1D6）の項目。
static NFT2J_ITEMS: &[&str] = &["カグヤ", "ヒメカ", "トヨ", "イツセ", "トヨミホ", "ヒミコ"];
static NFT2J: Table = Table::from_dice("古代（日本）名前表／女性名", 1, 6, NFT2J_ITEMS);
/// Ruby `TABLES_MOD_1D["NMTGM"]`（天界人（メソポタミア神話）名前表／男性名 / 1D6）の項目。
static NMTGM_ITEMS: &[&str] = &["アヌ", "アプスー", "エンリル", "ウトゥ", "エンキ", "ラムガ"];
static NMTGM: Table = Table::from_dice(
    "天界人（メソポタミア神話）名前表／男性名",
    1,
    6,
    NMTGM_ITEMS,
);
/// Ruby `TABLES_MOD_1D["NFTGM"]`（天界人（メソポタミア神話）名前表／女性名 / 1D6）の項目。
static NFTGM_ITEMS: &[&str] = &[
    "アントゥアル",
    "ティアマト",
    "イシュタル",
    "イナンナ",
    "アルル",
    "ニダバ",
];
static NFTGM: Table = Table::from_dice(
    "天界人（メソポタミア神話）名前表／女性名",
    1,
    6,
    NFTGM_ITEMS,
);
/// Ruby `TABLES_MOD_1D["NMTGI"]`（天界人（インド神話）名前表／男性名 / 1D6）の項目。
static NMTGI_ITEMS: &[&str] = &[
    "ヴィシュヌ",
    "シヴァ",
    "ブラフマー",
    "ガネーシャ",
    "ハヌマーン",
    "ラーマ",
];
static NMTGI: Table = Table::from_dice("天界人（インド神話）名前表／男性名", 1, 6, NMTGI_ITEMS);
/// Ruby `TABLES_MOD_1D["NFTGI"]`（天界人（インド神話）名前表／女性名 / 1D6）の項目。
static NFTGI_ITEMS: &[&str] = &[
    "ラクシュミ",
    "パールヴァティ",
    "サラスヴァティ",
    "クリシュナ",
    "カーリー",
    "シータ",
];
static NFTGI: Table = Table::from_dice("天界人（インド神話）名前表／女性名", 1, 6, NFTGI_ITEMS);

/// Ruby `TABLES_MOD_1D`。コマンド名 → 表。
static TABLES_MOD_1D: &[(&str, &Table)] = &[
    ("RT", &RT),
    ("CPT", &CPT),
    ("CNT", &CNT),
    ("IT", &IT),
    ("AGT", &AGT),
    ("MCT", &MCT),
    ("SCT", &SCT),
    ("BCT", &BCT),
    ("AMCT", &AMCT),
    ("ASCT1", &ASCT1),
    ("ASCT2", &ASCT2),
    ("CTT0", &CTT0),
    ("CTT1", &CTT1),
    ("CTT2", &CTT2),
    ("CTT3", &CTT3),
    ("CTT4", &CTT4),
    ("CTT5", &CTT5),
    ("CTT6", &CTT6),
    ("CTT7", &CTT7),
    ("CTTD", &CTTD),
    ("CTT4M", &CTT4M),
    ("NTT0", &NTT0),
    ("NTT1", &NTT1),
    ("NTT2", &NTT2),
    ("NTT3", &NTT3),
    ("NTT4", &NTT4),
    ("NTT5", &NTT5),
    ("NTT6", &NTT6),
    ("NTT7", &NTT7),
    ("NTTG", &NTTG),
    ("NTTD", &NTTD),
    ("NMT2J", &NMT2J),
    ("NFT2J", &NFT2J),
    ("NMTGM", &NMTGM),
    ("NFTGM", &NFTGM),
    ("NMTGI", &NMTGI),
    ("NFTGI", &NFTGI),
];

// ---------------------------------------------------------------------------
// Ruby `TABLES_MOD_MINUS`（バタフライエフェクト表（出目 -5〜12 / 2D6+7 を添字にする））
// ---------------------------------------------------------------------------

/// Ruby `TABLES_MOD_MINUS["SBET"]`（重度バタフライエフェクト表 / 2D6）の項目。
static SBET_ITEMS: &[&str] = &[
    "消失。対象の存在自体が時空連続体から完全に消失する。対象を【因縁】としていた全てのPCはその【因縁】の消失欄にチェックを入れ、その対象との【因縁】内容がネガティブなら［疲労度］が「対象の因縁強度+3」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+3」点ずつ増加する。",
    "消失。対象の存在自体が時空連続体から完全に消失する。対象を【因縁】としていた全てのPCはその【因縁】の消失欄にチェックを入れ、その対象との【因縁】内容がネガティブなら［疲労度］が「対象の因縁強度+3」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+3」点ずつ増加する。",
    "消失の可能性。対象の存在自体があいまいになってしまう。表をふったプレイヤーはランダムに選んだ特技を指定特技として判定する。判定に失敗すると対象の存在は消失する。対象を【因縁】としていた全てのPCは対象の【因縁】の消失欄にチェックを入れ、その対象との因縁内容がネガティブなら［疲労度］が「対象の因縁強度+2」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+2」点ずつ増加する。",
    "消失の可能性。対象の存在自体があいまいになってしまう。表をふったプレイヤーはランダムに選んだ特技を指定特技として判定する。判定に失敗すると対象の存在は消失する。対象を【因縁】としていた全てのPCは対象の【因縁】の消失欄にチェックを入れ、その対象との因縁内容がネガティブなら［疲労度］が「対象の因縁強度+2」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+2」点ずつ増加する。",
    "時代変更。対象の存在している時代が変わってしまう。存在する時代が変わってしまえば、もはやPCのことは覚えていないだろう。表を振ったプレイヤーは時代決定表を振って、変更先の時代を決定する。現在と同じ時代となれば、何も起こらない。違う時代になってしまったら、対象を【因縁】としていた全てのPCは対象の【因縁】の消失欄にチェックを入れ、その対象との因縁内容がネガティブなら［疲労度］が「対象の因縁強度+1」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+1」点ずつ増加する。",
    "時代変更。対象の存在している時代が変わってしまう。存在する時代が変わってしまえば、もはやPCのことは覚えていないだろう。表を振ったプレイヤーは時代決定表を振って、変更先の時代を決定する。現在と同じ時代となれば、何も起こらない。違う時代になってしまったら、対象を【因縁】としていた全てのPCは対象の【因縁】の消失欄にチェックを入れ、その対象との因縁内容がネガティブなら［疲労度］が「対象の因縁強度+1」点減少、ポジティブなら［疲労度］と［改変度］が「対象の因縁強度+1」点ずつ増加する。",
    "死亡。対象は死亡してしまう。対象を【因縁】としていた全てのPCは対象の【因縁】の死亡欄にチェックを入れ、［疲労度］と［改変度］が「対象の因縁強度」点ずつ増加する。",
    "死亡。対象は死亡してしまう。対象を【因縁】としていた全てのPCは対象の【因縁】の死亡欄にチェックを入れ、［疲労度］と［改変度］が「対象の因縁強度」点ずつ増加する。",
    "別人化。対象はあなたとの因縁種別は維持したまま、完全な別人になってしまう。表を振ったプレイヤーは名前表、経歴表を用いて新たな設定を決め直すこと。年齢・性別は変化しない。対象を【因縁】としていた全てのPCは［改変度］が「対象の因縁強度」点ずつ増加する。",
    "因縁種別変化。対象との因縁種別が変わってしまう。表を振ったプレイヤ―は因縁種別表を使用して新たな因縁種別を決定する。その結果、元の因縁種別と違うものになったら、表を振ったプレイヤーのPCは［改変度］が「元の因縁強度」点だけ増加する。",
    "忘却。対象はあなたのことを忘れてしまう。表を振ったプレイヤーのPCはその対象との因縁内容がネガティブなら［疲労度］が「対象の因縁強度」点減少、ポジティブなら［疲労度］が「対象の因縁強度」点増加する。",
    "困窮。対象は経済的に困窮してしまい、その生活は荒れ果ててしまう。その対象との因縁内容がポジティブだった場合、表を振ったプレイヤーのPCは《経済》もしくは《心理》を指定特技として判定を行うことができる。判定に失敗した場合、そのPCは［疲労度］を「対象の因縁強度」点増加させたうえ、ネガティブ因縁内容表を使用して新たに決定し直さなければならない。もともと因縁内容がネガティブだった場合は何も起こらない。",
    "病。対象は不治の病に侵されてしまう。表を振ったプレイヤーのPCは《医療》《漢方》《縁起》のいずれかを指定特技として判定を行うことができる。判定に失敗すると対象は不治の病により死亡してしまう。この表の出目「1or2」の効果を適用すること。",
    "年齢変化。対象の年齢が変わってしまう。表を振ったプレイヤーはまず1D6を振る。奇数なら年齢は減り、偶数なら年齢は増えてしまう。何歳変化するかは1D6を振って決定する。ただし、6の目が出た場合は追加で1D6を振る。6が出るたびにこれを繰り返す。最終的に全ての出目の合計だけ年齢が変化する。変化後の年齢が0才未満になってしまった場合は、対象の存在が消えてしまう。この表の出目「-5or-4」の効果を適用すること。一方、変化後の年齢が「寿命＝30+(対象のEL×10)歳」以上になった場合は、寿命を迎えて死亡していないかどうかを決めるため、ランダムに決定した指定特技で判定する。判定に失敗すると対象は死亡してしまう。この表の出目「1or2」の効果を適用すること。また、対象の存在が消えるもしくは死亡しなかった場合でも、結果が矛盾した状態（パラドックス）になったとTAが判断した場合、そのPCの［改変度］が「対象の因縁強度」点増加する。",
    "性別反転。対象の性別が反転してしまう。現在の因縁種別が性別を含むものであれば変更する。例えば次のような形。「実の父親」←→「実の母親」、「実の兄弟」←→「実の姉妹」、「実の祖父」←→「実の祖母」、「養父」←→「養母」、「同性の配偶者」←→「異性の配偶者」など。対象を【因縁】としていた全てのPCのプレイヤーがこの変化を受け入れるのであれば他には何も起こらない。受け入れられないのであれば、そのPCの［疲労度］が「対象の因縁強度」点増加する。また、結果が矛盾した状態（パラドックス）になったとTAが判断した場合、そのPCの［改変度］が「対象の因縁強度」点増加する。",
    "性格反転。対象の性格が反転する。その対象との今の因縁内容がネガティブなら、ポジティブ因縁内容表を使用して決定し直す。今の因縁内容がポジティブなら、ネガティブ因縁内容表を使用して因縁内容を決め直すこと。表を振ったプレイヤーがこの変化を受け入れるのであれば他には何も起こらない。受け入れられないのであれば、そのPCの［疲労度］が「対象の因縁強度」点増加する。",
    "因縁内容変化。対象との因縁内容が変わってしまう。その対象との因縁内容がポジティブだった場合、ポジティブ因縁内容表を、ネガティブだった場合、ネガティブ因縁内容表を使用して因縁内容を決め直すこと。表を振ったプレイヤーがこの変化を受け入れるのであれば他には何も起こらない。受け入れられないのであれば、そのPCの［疲労度］が「対象の因縁強度」点増加する。",
    "宇宙開闢の女神が微笑む。何も変化は起こらなかった。",
];
static SBET: Table = Table::from_dice("重度バタフライエフェクト表", 2, 6, SBET_ITEMS);
/// Ruby `TABLES_MOD_MINUS["MBET"]`（軽度バタフライエフェクト表 / 2D6）の項目。
static MBET_ITEMS: &[&str] = &[
    "激痛。耐え難い激しい痛みが全身を襲う。対象の［疲労度］が「対象の因縁強度」点、［改変度］が「対象の因縁強度+2D6」点増加する。",
    "激痛。耐え難い激しい痛みが全身を襲う。対象の［疲労度］が「対象の因縁強度」点、［改変度］が「対象の因縁強度+2D6」点増加する。",
    "吐血。激しいせき込みの末、吐血してしまう。対象の［疲労度］が「対象の因縁強度」点、［改変度］が「対象の因縁強度+2D6-1」点増加する。",
    "吐血。激しいせき込みの末、吐血してしまう。対象の［疲労度］が「対象の因縁強度」点、［改変度］が「対象の因縁強度+2D6-1」点増加する。",
    "頭痛。頭が割れるような激しい頭痛に襲われる。対象の［疲労度］が「対象の因縁強度の半分」点増加、［改変度］が「対象の因縁強度と同じ値+1D6」点増加する。",
    "頭痛。頭が割れるような激しい頭痛に襲われる。対象の［疲労度］が「対象の因縁強度の半分」点増加、［改変度］が「対象の因縁強度と同じ値+1D6」点増加する。",
    "時間結晶化。身体の一部が時間結晶化する。対象の［改変度］が「対象の因縁強度の半分」点増加する。また、対象がPCだった場合、このセッションの間、好きな【タイムトラベラースキル】を1つだけ追加で修得できる。この【タイムトラベラースキル】はセッション終了時に失われる。",
    "前兆。軽いめまいを感じる。嫌な前兆だ。対象の［疲労度］が1点、対象の［改変度］が「対象の因縁強度の半分」点増加する。",
    "遭遇。自分自身に出会ってしまい時空連続体に亀裂が生じる。何かの事情でこの時代に訪れた別時間軸の自分だろうか。対象の［改変度］が「対象の因縁強度」点増加する。",
    "半透明化。身体がはっきりと半透明になってきた。対象の［改変度］が「対象の因縁強度の半分」点増加する。",
    "外見変化。表を振ったプレイヤーは時代決定表を使用して時代を一つ決定し、その時代の経歴表を使用する。対象はその結果に合った外見・服装に見た目が変化してしまう。対象の［改変度］が「対象の因縁強度の半分」点増加する。",
    "記憶喪失。記憶が混濁し失われていく。対象がPCだった場合、修得している【スキル】のうち、ランダムに選択した【クラススキル】1つがこのセッションの間、使用不能になる。クライマックスフェイズでこのバタフライエフェクトを発生させたPCの【バタフライエフェクト問題】が解決されれば、この効果で使用不能になった【クラススキル】は即座に使用可能になる。対象の［改変度］が「対象の因縁強度の半分」点増加する。",
    "半透明化の兆し。身体が半透明になってきた気がする。対象の［改変度］が1点増加する。",
    "年齢変化。急激に対象の年齢が変化する。表を振ったプレイヤーはまず1D6を振る。奇数なら1D6才若返り、偶数なら1D6才年を取ってしまう。対象の［改変度］が1点増加する。",
    "過去改変。自身の過去が少しだけ書き換わる。対象は新たな経歴を自分の出身時代の経歴表を振って決め直すこと。対象の［改変度］が1点増加する。また、対象がPCだった場合、修得している【クラススキル】を自分のクラスの別の【クラススキル】に変更することができる。",
    "郷愁。ふと意識が宙に浮かび、目の前に故郷の風景が広がる。誰でも郷愁を感じるだろう。対象の［改変度］が1点増加する。",
    "不安。落ち着かない気分になる。対象の［改変度］が1点増加する。",
    "宇宙開闢の女神が微笑む。何も変化は起こらなかった。",
];
static MBET: Table = Table::from_dice("軽度バタフライエフェクト表", 2, 6, MBET_ITEMS);
/// Ruby `TABLES_MOD_MINUS["TBET"]`（タイムトラベラー重度バタフライエフェクト表 / 2D6）の項目。
static TBET_ITEMS: &[&str] = &[
    "消失。PCの存在自体が時空連続体から完全に消失する。PCはロストする。",
    "消失。PCの存在自体が時空連続体から完全に消失する。PCはロストする。",
    "消失の可能性。PCの存在自体があいまいになってしまう。ランダムに選んだ指定特技で判定を行う。判定に失敗すると対象のPCは消失する。PCはロストする。",
    "消失の可能性。PCの存在自体があいまいになってしまう。ランダムに選んだ指定特技で判定を行う。判定に失敗すると対象のPCは消失する。PCはロストする。",
    "時代変更。PCの存在している時代が変わってしまう。時代決定表を振って、新たな出身時代を決定する。違う時代になってしまったら、全ての【因縁】の消失欄にチェックを入れる。また、取得している【クラススキル】を全て失うが、同じ数だけ新たな出身時代の【クラススキル】を取得する。",
    "時代変更。PCの存在している時代が変わってしまう。時代決定表を振って、新たな出身時代を決定する。違う時代になってしまったら、全ての【因縁】の消失欄にチェックを入れる。また、取得している【クラススキル】を全て失うが、同じ数だけ新たな出身時代の【クラススキル】を取得する。",
    "永続時間結晶化。PCの身体の一部が永続的に時間結晶化する。タイムトラベラー特有の能力が強化される。好きな【タイムトラベラースキル】を追加で修得できる。この【スキル】は次回以降のセッションでも修得したままとなる。",
    "死亡。PCは死亡してしまう。PCはロストする。",
    "忘却。PCは記憶を失う。全ての【因縁】の消失欄にチェックを入れる。",
    "改変体質。宇宙開闢の加護の効果が薄まり、PCは歴史改変を受けやすい性質を得てしまう。今後、PCの［改変度］が増加する値が常にプラス1されてしまう。タイムトラベラースキル【時間盾】などによる［改変度］増加量を減少させる効果の前に適用される。",
    "コミュニケーション障害。宇宙開闢の加護の効果が薄まり、PCはタイムトラベル先の言語を理解しにくくなってしまう。今後、PCが行う接近判定に常にマイナス1の修正が付いてしまう。",
    "病。PCは病に侵されてしまう。病が治るまで（次回から3セッションの間）、セッション開始時の［疲労度］が3になり、セッション中も3より小さい値にならない。",
    "年齢変化。急激にPCの年齢が変化する。まず1D6を振る。奇数なら1D6才若返り、偶数なら1D6才年を取ってしまう。",
    "経歴変化。PCの経歴が変化してしまう。自分の時代の経歴表を振って、新しい経歴を決め直すこと。",
    "性別反転。PCの性別が反転してしまう。",
    "語尾変化。PCは時代特有の語尾が口をついて出てしまうようになる。語尾はセッションの舞台となった時代ごとに下記の通り。原始時代：「ウホ」、古代：「であ～る」、中世時代：「ゴザル」、現代：「じゃん」、超情報化時代：「ゼ」、宇宙時代：「ペモ」。",
    "時代侵食。PCの過去に別の時代が少しだけ侵食する。修得している【クラススキル】1つを自分のクラスとは別のクラスの【クラススキル】に変更することができる。",
    "宇宙開闢の女神が微笑む。何も変化は起こらなかった。",
];
static TBET: Table = Table::from_dice(
    "タイムトラベラー重度バタフライエフェクト表",
    2,
    6,
    TBET_ITEMS,
);

/// Ruby `TABLES_MOD_MINUS`。コマンド名 → 表。
static TABLES_MOD_MINUS: &[(&str, &Table)] = &[("SBET", &SBET), ("MBET", &MBET), ("TBET", &TBET)];

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
            .join("test/data/PastFutureParadox.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/PastFutureParadox.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/PastFutureParadox.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("PastFutureParadox.toml must parse");
        assert_eq!(
            data.tests.len(),
            163,
            "case count in test/data/PastFutureParadox.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "PastFutureParadox",
                "unexpected game system in PastFutureParadox.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("PastFutureParadox"), &tc.input, &mut src) {
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
                    "FAIL PastFutureParadox:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} PastFutureParadox cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
