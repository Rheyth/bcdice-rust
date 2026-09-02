//! P4で手書き移植した `lib/bcdice/game_system/Nechronica.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Nechronica#eval_game_system_specific_command`（`Base#roll_tables` → `#nechronica_check`）
//! - `#result_nd10`（1D10の後方互換）/ `#result_nechronica` / `#get_hit_location`
//! - `#r_backward_compatibility`（`nR10` 記法の後方互換）
//!
//! # 表データ
//!
//! Ruby側は `DiceTable::Table.from_i18n("Nechronica.table.…", locale)` で
//! `i18n/Nechronica/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として直接持つ。
//! データ部分（`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、
//! 値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`Nechronica_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `Nechronica_Korean < Nechronica` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::Table;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{dice_text, table_helpers, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`Nechronica` と `Nechronica_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static Table)],
    /// i18n `Nechronica.hit_location.table`
    pub(crate) hit_location: &'static [&'static str],
    /// i18n `Nechronica.hit_location.additional_damage`（`%{damage}` を数で置換する）
    pub(crate) additional_damage: &'static str,
    /// i18n `Nechronica.critical`
    pub(crate) critical: &'static str,
    /// i18n `Nechronica.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `Nechronica.break_all_parts`
    pub(crate) break_all_parts: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Nechronica#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = table_helpers::roll_table(command, sys.tables, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(nechronica_check(sys, command, rng)?.map(SpecificCommandOutput::result))
}

/// Ruby `Nechronica#result_nd10`（後方互換を維持するため、1d10>=nを目標値nの1NCとして処理）。
pub(crate) fn result_nd10(
    sys: &SystemTables,
    total: crate::Int,
    value_list: &[i64],
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    // Ruby: value_list.count != 1 || cmp_op != :>= || target.nil? || target == "?"
    // 目標値なしで `check_result` が呼ばれることはないので、`target.nil?` は `"?"` だけで足りる。
    let Target::Number(target) = target else {
        return None;
    };
    if value_list.len() != 1 || cmp_op != CmpOp::Ge {
        return None;
    }

    Some(CheckOutcome::Result(Box::new(result_nechronica(
        sys,
        &[crate::randomizer::sat_i64(&total)],
        crate::randomizer::sat_i64(&target),
    ))))
}

/// Ruby `Nechronica#result_nechronica`。
fn result_nechronica(sys: &SystemTables, value_list: &[i64], target: i64) -> EvalResult {
    let max = value_list.iter().copied().max().unwrap_or(0);
    if max >= target {
        if max >= 11 {
            EvalResult::critical(sys.critical)
        } else {
            EvalResult::success(sys.success)
        }
    } else if value_list.iter().filter(|i| **i <= 1).count() == 0 {
        EvalResult::failure(sys.failure)
    } else if value_list.len() > 1 {
        EvalResult::fumble(format!("{} ＞ {}", sys.fumble, sys.break_all_parts))
    } else {
        EvalResult::fumble(sys.fumble)
    }
}

/// Ruby `/^(\d)?R10([+\-\d]+)?(\[(\d+)\])?$/`。
fn r_backward_compatibility_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(\d)?R10([+\-\d]+)?(\[(\d+)\])?\z").expect("valid regex"))
}

/// Ruby `Nechronica#r_backward_compatibility`（Rコマンドの後方互換）。
fn r_backward_compatibility(command: &str) -> String {
    let Some(m) = r_backward_compatibility_pattern().captures(command) else {
        return command.to_owned();
    };

    // Ruby: `#{m[1]}` / `#{m[2]}` は nil なら空文字列
    let prefix = m.get(1).map_or("", |x| x.as_str());
    let modifier = m.get(2).map_or("", |x| x.as_str());
    // Ruby: m[4] == "1" なら攻撃判定（NA）、それ以外（nil を含む）は判定（NC）
    if m.get(4).map(|x| x.as_str()) == Some("1") {
        format!("{prefix}NA{modifier}")
    } else {
        format!("{prefix}NC{modifier}")
    }
}

/// Ruby `Nechronica#nechronica_check`。
fn nechronica_check(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let command = r_backward_compatibility(command);

    // Ruby: Command::Parser.new(/N[CA](10)?/, round_type: round_type).enable_prefix_number
    //       歴史的経緯で10を受理する
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER
        .get_or_init(|| Parser::new(&["N[CA](10)?"], RoundType::Floor).enable_prefix_number());
    let Some(cmd) = parser.parse(&command) else {
        return Ok(None);
    };

    // Ruby: [1, cmd.prefix_number.to_i].max（nil.to_i == 0）
    let dice_count = std::cmp::max(
        1,
        cmd.prefix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
    );
    let modify_number = cmd.modify_number.clone();

    let mut dice = rng.roll_barabara(dice_count, 10)?;
    dice.sort_unstable();
    let dice_mod: Vec<i64> = dice
        .iter()
        .map(|i| i.saturating_add(crate::randomizer::sat_i64(&modify_number)))
        .collect();
    let total = dice_mod.iter().copied().max().unwrap_or(0);

    // Ruby: na = get_hit_location(total) if cmd.command.start_with?("NA")
    let na = if cmd.command.starts_with("NA") {
        get_hit_location(sys, total)
    } else {
        None
    };

    // Ruby: result_nechronica(dice_mod, 6)（目標値6は直値で、@default_target_number 経由ではない）
    let mut result = result_nechronica(sys, &dice_mod, 6);

    let mut sequence = vec![
        format!("({})", cmd.to_s(SuffixPosition::AfterCommand)),
        format!(
            "[{}]{}",
            dice_text::join_dice(&dice),
            modifier(&modify_number)
        ),
        format!("{total}[{}]", dice_text::join_dice(&dice_mod)),
        result.text.clone(),
    ];
    if let Some(na) = na {
        sequence.push(na);
    }

    result.text = sequence.join(" ＞ ");
    Ok(Some(result))
}

/// Ruby `Nechronica#get_hit_location`（命中部位）。乱数は消費しない。
fn get_hit_location(sys: &SystemTables, value: i64) -> Option<String> {
    if value <= 5 {
        return None;
    }

    // Ruby: table[(value - 6).clamp(0, 5)]
    let index = (value - 6).clamp(0, 5) as usize;
    let text = sys.hit_location[index];

    if value > 10 {
        Some(format!(
            "{text}{}",
            sys.additional_damage
                .replace("%{damage}", &(value - 10).to_string())
        ))
    } else {
        Some(text.to_owned())
    }
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの表と定型文
// ---------------------------------------------------------------------------

/// i18n `Nechronica.table.NM.items`。
static JA_NM_ITEMS: &[&str] = &[
    "【嫌悪】[発狂:敵対認識]敵に命中しなかった攻撃は全て、射程内にいるなら嫌悪の対象に命中する。(防御側任意)",
    "【独占】[発狂:独占衝動]戦闘開始時と終了時に１つずつ、対象はパーツを選んで損傷する。",
    "【依存】[発狂:幼児退行]最大行動値が減少する(-2)",
    "【執着】[発狂:追尾監視]戦闘開始時と終了時に1つずつ、対象はあなたへの未練に狂気点を得る。",
    "【恋心】[発狂:自傷行動]戦闘開始時と終了時に1つずつ、あなたはパーツを選んで損傷する。",
    "【対抗】[発狂:過剰競争]戦闘開始時と終了時に1つずつ、あなたは任意の未練に狂気点を追加で得る。",
    "【友情】[発狂:共鳴依存]セッション終了時、対象にあなたよりも多く損傷したパーツがある際、あなたは損傷パーツ数が対象と同じになるまで、パーツを損傷させる。",
    "【保護】[発狂:常時密着]あなたが対象と別エリアにいるなら「移動以外の効果を持つマニューバ」を宣言できない。「自身と対象」以外を移動マニューバの対象にできない。",
    "【憧憬】[発狂:贋作妄想]あなたが対象と同エリアにいるなら「移動以外の効果を持つマニューバ」を宣言できない。「自身と対象」以外を移動マニューバの対象にできない。",
    "【信頼】[発狂:疑心暗鬼]あなた以外の全ての姉妹の最大行動値が減少する(-1)",
];
/// i18n `Nechronica.table.NM`（姉妹への未練表 / 1D10）。
static JA_NM: Table = Table::from_dice("姉妹への未練表", 1, 10, JA_NM_ITEMS);

/// i18n `Nechronica.table.NMN.items`。
static JA_NMN_ITEMS: &[&str] = &[
    "【忌避】[発狂:隔絶意識]あなたは未練の対象ないしサヴァントと同じエリアにいる間、「移動以外の効果を持つマニューバ」を宣言できない。また、「自身と未練の対象ないしサヴァント」以外を移動マニューバの対象にできない。",
    "【嫉妬】[発狂:不協和音]全ての姉妹は行動判定に修正-1を受ける。",
    "【依存】[発狂:幼児退行]最大行動値が減少する(-2)",
    "【憐憫】[発狂:過情移入]あなたは「サヴァント」に対する攻撃判定の出目に修正-1を受ける。",
    "【感謝】[発狂:病的返礼]発狂した際、あなたは任意の基本パーツ2つ（なければ最もレベルの低い強化パーツ1つ）を損傷する。",
    "【悔恨】[発狂:自業自棄]あなたが失敗した攻撃判定は全て、あなた自身の任意の箇所にダメージを与える。",
    "【期待】[発狂:希望転結]あなたは狂気点を追加して振り直しを行う際、出目に修正-1を受ける。（この効果は累積する）",
    "【保護】[発狂:生前回帰]あなたは「レギオン」をマニューバの対象に選べない。",
    "【尊敬】[発狂:神化崇拝]あなたは「他の姉妹」をマニューバの対象に選べない。",
    "【信頼】[発狂:疑心暗鬼]あなた以外の全ての姉妹の最大行動値が減少する(-1)",
];
/// i18n `Nechronica.table.NMN`（中立者への未練表 / 1D10）。
static JA_NMN: Table = Table::from_dice("中立者への未練表", 1, 10, JA_NMN_ITEMS);

/// i18n `Nechronica.table.NME.items`。
static JA_NME_ITEMS: &[&str] = &[
    "【恐怖】[発狂:認識拒否]あなたは、行動判定・狂気判定の出目に修正-1を受ける。",
    "【隷属】[発狂:造反有理]あなたが失敗した攻撃判定は全て、大失敗として扱う。",
    "【不安】[発狂:挙動不審]最大行動値が減少する(-2)",
    "【憐憫】[発狂:感情移入]あなたは「サヴァント」に対する攻撃判定の出目に修正-1を受ける。",
    "【愛憎】[発狂:凶愛心中]あなたは狂気判定・攻撃判定で大成功するごとに[判定値-10]個の自身のパーツを選び、損傷させる。",
    "【悔恨】[発狂:自業自棄]あなたが失敗した攻撃判定は全て、あなた自身の任意の箇所にダメージを与える。",
    "【軽蔑】[発狂:眼中不在]同エリアの手駒があなたに対して行う攻撃判定の出目は修正+1を受ける。",
    "【憤怒】[発狂:激情暴走]あなたは、攻撃判定・狂気判定の出目に修正-1を受ける。",
    "【怨念】[発狂:不倶戴天]あなたは逃走判定ができない。あなたが「自身と未練の対象」以外を対象にしたマニューバを使用する際、行動値1点を追加で減らさなくてはいけない。",
    "【憎悪】[発狂:痕跡破壊]この未練を発狂した際、あなた以外の姉妹から1人選ぶ。その姉妹は任意のパーツを2つ損傷する。",
];
/// i18n `Nechronica.table.NME`（敵への未練表 / 1D10）。
static JA_NME: Table = Table::from_dice("敵への未練表", 1, 10, JA_NME_ITEMS);

/// i18n `Nechronica.hit_location.table`。
static JA_HIT_LOCATION: &[&str] = &[
    "防御側任意",
    "脚（なければ攻撃側任意）",
    "胴（なければ攻撃側任意）",
    "腕（なければ攻撃側任意）",
    "頭（なければ攻撃側任意）",
    "攻撃側任意",
];

/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: &[("NM", &JA_NM), ("NMN", &JA_NMN), ("NME", &JA_NME)],
    hit_location: JA_HIT_LOCATION,
    additional_damage: "(追加ダメージ%{damage})",
    critical: "大成功",
    fumble: "大失敗",
    break_all_parts: "使用パーツ全損",
    success: "成功",
    failure: "失敗",
};

/// Ruby `BCDice::GameSystem::Nechronica`（ID: `Nechronica`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nechronica;

impl GameSystem for Nechronica {
    fn id(&self) -> &'static str {
        "Nechronica"
    }

    fn name(&self) -> &'static str {
        "ネクロニカ"
    }

    fn sort_key(&self) -> &'static str {
        "ねくろにか"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　(nNC+m)
　ダイス数n、修正値mで判定ロールを行います(省略=1)
　ダイス数が2以上の時のパーツ破損数も表示します。
・攻撃判定　(nNA+m)
　ダイス数n、修正値mで攻撃判定ロールを行います(省略=1)
　命中部位とダイス数が2以上の時のパーツ破損数も表示します。

表
・姉妹への未練表 nm
・中立者への未練表 nmn
・敵への未練表 nme
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d?NC", r"\d?NA", r"\dR10", "NM", "NMN", "NME"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Nechronica#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Nechronica#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Nechronica#initialize` の `@default_target_number = 6`。
    fn default_target_number(&self) -> Option<i64> {
        Some(6)
    }

    /// Ruby `Nechronica#result_nd10`。
    fn result_nd10(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd10(&JA_SYSTEM, total, value_list, cmp_op, target)
    }

    /// Ruby `Nechronica#eval_game_system_specific_command`。
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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Nechronica",
            "Nechronica.toml",
            37,
        );
    }
}
