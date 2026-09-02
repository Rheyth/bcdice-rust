//! P4で手書き移植した `lib/bcdice/game_system/Garako.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`roll_tables` → `roll_gr` → `roll_damage_chart` → `roll_attack_hit`）
//! - `#roll_gr`（判定 `GR+n#f>=X`。戻りは String であり Result ではない）
//! - `#roll_attack_hit`（`GHAn`）
//! - `#roll_damage_chart`（`xDCy` / `xDTy`）
//! - `DAMAGE_CHARTS` と `TABLES`（個性表 `IDI` は RangeTable）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::command_parser::Parser;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::int_helpers::int_clamp;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::Garako`（ID: `Garako`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Garako;

impl GameSystem for Garako {
    fn id(&self) -> &'static str {
        "Garako"
    }

    fn name(&self) -> &'static str {
        "ガラコと破界の塔"
    }

    fn sort_key(&self) -> &'static str {
        "からことはかいのとう"
    }

    fn help_message(&self) -> &'static str {
        r"・判定 GR+n#f>=X （+n：判定値、#f：不安定による自動失敗基準値、X：目標値、それぞれ省略可能）
・部位決定チャート：HIT
・ダメージ+部位決定：GHAn（n：火力）
・ダメージチャート：xDCy（CDC/EDC/FDC/ADC/LDC )
・ダメージチャートver2：xDTy（CDT/EDT/FDT/ADT/LDT）
　xは C：コックピット、E：エンジン、F：フレーム、A：アーム、L：レッグ
　yはダメージ値
各種表
・個性表：IDI／動機決定表：MTV
・名前表
ピグマー族　　男：PNM　女：PNF　　エレメント族　男：ENM　女：ENF
ノーマッド族　男：NNM　女：NNF　　ラット族　　　男：RNM　女：RNF
ブレイン族　　１：BN1　２：BN2　　テンタクル族　１：TN1　２：TN2
・ガラコ改造チャート表：GCC
・武器改造チャート表：WCC
・イベントチャート表：EVC
・戦闘開始距離：BSD

デフォルトダイス：10面
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "GR",
            "[CEFAL]D[CT]",
            "GHA",
            "PNM",
            "PNF",
            "ENM",
            "ENF",
            "NNM",
            "NNF",
            "RNM",
            "RNF",
            "BN1",
            "BN2",
            "TN1",
            "TN2",
            "MTV",
            "HIT",
            "GCC",
            "WCC",
            "EVC",
            "BSD",
            "IDI",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sides_implicit_d = 10`。
    fn sides_implicit_d(&self) -> i64 {
        10
    }

    /// Ruby `Garako#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `#eval_game_system_specific_command`。
///
/// `roll_tables(command, TABLES) || roll_gr(command) || roll_damage_chart(command) || roll_attack_hit(command)`
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = roll_tables(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_gr(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_damage_chart(command)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = roll_attack_hit(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(None)
}

/// Ruby `Base#roll_tables(command, TABLES)`。
///
/// `RangeTable` は `RollableTable` を実装しない（結果型が `RangeRollResult`）ので、
/// 個性表 `IDI` だけ別経路で `.roll().to_s` する。既定の整形は `{name}({sum}) ＞ {content}`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command == "IDI" {
        return Ok(Some(TABLE_IDI.roll(rng)?.to_string()));
    }
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `Garako#roll_gr`。
///
/// 判定結果は String（`SpecificCommandOutput::text`）であり、成功/失敗フラグは立たない。
fn roll_gr(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("GR", round_type: round_type).enable_fumble.restrict_cmp_op_to(nil, :>=)
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["GR"], RoundType::Floor)
            .enable_fumble()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `Parsed#modify_number` は欠落時 0。Ruby の `cmd.modify_number || 0` に相当。
    let modify_number = parsed.modify_number;
    let auto_failure_number = parsed
        .fumble
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(1);
    let target_number = parsed.target_number;

    let dice = rng.roll_once(10)?;
    let total = dice + modify_number.clone();

    let result = if dice == 1 {
        Some("ファンブル")
    } else if dice <= auto_failure_number {
        // 公式FAQより、ファンブルと自動失敗を区別する可能性があるので分岐
        Some("自動失敗")
    } else if dice == 10 {
        Some("クリティカル")
    } else if let Some(ref target) = target_number {
        Some(if total >= *target { "成功" } else { "失敗" })
    } else {
        // 目標値なし・出目 2〜9 は結果文言なし（compact で落ちる）
        None
    };

    let formated_modifier = modifier(&modify_number);
    let formated_auto_failure = if auto_failure_number >= 2 {
        format!("#{auto_failure_number}")
    } else {
        String::new()
    };
    let format_target = target_number.map(|t| format!(">={t}")).unwrap_or_default();

    // Ruby: sequence.compact.join(" ＞ ")
    let mut sequence = vec![
        format!("(1D10{formated_modifier}{formated_auto_failure}{format_target})"),
        format!("{dice}[{dice}]{formated_modifier}"),
        total.to_string(),
    ];
    if let Some(result) = result {
        sequence.push(result.to_owned());
    }

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `/^GHA([-+\d]+)$/i`。
fn attack_hit_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^GHA([+\-\d]+)$").expect("valid regex"))
}

/// Ruby `Garako#roll_attack_hit`。
fn roll_attack_hit(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(captures) = attack_hit_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: ArithmeticEvaluator.eval(m[1])（失敗時 0）
    let modifier_number = arithmetic::eval(&captures[1], RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let attack = rng.roll_once(10)?;
    let total = attack + modifier_number;

    // total <= 0 では `hit_dice` が未代入。Ruby はメソッド内代入を見た時点で nil になり、補間は空文字。
    let (hit_dice, hit_text) = if total > 0 {
        let hit = TABLE_HIT.roll(rng)?;
        (
            format!(", HIT[{}]", hit.value()),
            format!("{}に {total} -【部位装甲】", hit.last_body()),
        )
    } else {
        (String::new(), format!("{total}（ダメージを受けない）"))
    };

    let formated_modifier = modifier(&crate::Int::from(modifier_number));
    // Ruby: sequence.join(" ＞ ")（compact しない）
    let sequence = [
        format!("(1D10{formated_modifier})"),
        format!("{attack}[{attack}]{formated_modifier}{hit_dice}"),
        hit_text,
    ];

    Ok(Some(sequence.join(" ＞ ")))
}

/// Ruby `/^([CEFAL]D[CT])([-+\d]+)$/i`。
fn damage_chart_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^([CEFAL]D[CT])([+\-\d]+)$").expect("valid regex"))
}

/// Ruby `Garako#roll_damage_chart`。
///
/// 数値なし（`CDC` など）は正規表現不一致で `nil`。
fn roll_damage_chart(command: &str) -> Result<Option<String>, EvalError> {
    let Some(captures) = damage_chart_pattern().captures(command) else {
        return Ok(None);
    };

    let Some((_, chart_name, table)) = DAMAGE_CHARTS
        .iter()
        .find(|(key, _, _)| *key == &captures[1])
    else {
        return Ok(None);
    };

    // Ruby: ArithmeticEvaluator.eval(m[2]).clamp(0, 10)
    let damage = int_clamp(
        &arithmetic::eval(&captures[2], RoundType::Floor)?.unwrap_or(I::ZERO),
        &I::ZERO,
        &I::from(10),
    );
    if damage <= I::ZERO {
        return Ok(Some("ダメージを受けない".to_owned()));
    }

    let result = table[(sat_i64(&damage) as usize) - 1];
    Ok(Some(format!("{chart_name}({damage}) ＞ {result}")))
}

/// Ruby `DAMAGE_CHARTS`（キー → 表名と10件の本文）。
static DAMAGE_CHARTS: &[(&str, &str, &[&str])] = &[
    (
        "CDC",
        "部位ダメージチャート：コックピット",
        &[
            "小破（アーマー損傷）：以後、この部位の【部位装甲】-1。",
            "小破（視界不良）：モニターやハッチの歪み等により、視界を大きく遮られる。以後、【視認性】-1、【部位装甲】-1。",
            "小破（強震）：大きく揺さぶられる。キミは【身体】10の判定を行う。失敗した場合、次のターンを失う。【部位装甲】-1。",
            "小破（収納直撃）：アイテム収納スペースに直撃！　所持品一つにつき1d10を振れ。出目が5以下だった所持品は破壊される。【部位装甲】-1。",
            "中破（計器損傷）：コンソールの一部が停止する。［弱体1］を受ける。",
            "中破（制御不能）：コントロールが効かなくなる。キミは次のターンを失う。［弱体1］を受ける。",
            "中破（貫通！）：パイロットに被害が！　キミはHPダメージ（1d10-【身体】）に加え、［弱体1］を受ける。",
            "大破（故障）：コックピットが完全にいかれる。キミは次のラウンド終了時まで、あらゆる判定に自動的にファンブルする。［弱体1］を受ける。",
            "大破（貫通！）：パイロットに被害が！　キミはHPダメージ（1d10+3-【身体】）に加え、［弱体1］を受ける。",
            "修復不能（破壊）：コックピットが［修復不能］となる。キミは2d10-【身体】点のHPダメージを受ける。ガラコはすべての機能を停止する。コックピットのハッチが自動的に開く。",
        ],
    ),
    (
        "EDC",
        "部位ダメージチャート：エンジン",
        &[
            "小破（アーマー損傷）：以後、この部位の【部位装甲】-1。",
            "小破（アーマー損傷）：以後、この部位の【部位装甲】-1。",
            "小破（燃料漏れ）：タンクから燃料が漏れる。燃料-1。この部位の【部位装甲】-1。",
            "小破（燃料漏れ）：タンクから燃料が漏れる。燃料-2。この部位の【部位装甲】-1。",
            "中破（エンジン不調）：時々エンジンが動かなくなる。［弱体1］を受ける。",
            "中破（燃料漏れ）：タンクから燃料が漏れる。燃料-2。［弱体1］を受ける。",
            "中破（ヒート）：オーバーヒートする。次のターンの終了時まで、移動と攻撃を行えない。［弱体1］を受ける。",
            "大破（エンジン不調）：キミは次のターンを失う。［弱体1］を受ける。",
            "大破（故障）：以後、この部位の【部位装甲】が0になる。［弱体1］を受ける。",
            "修復不能（エンジン停止）：エンジンが停止する。ガラコはすべての機能を停止する。コックピットのハッチが自動的に開く。【操作性】10の判定を行うこと。失敗するとエンジンが爆発する。その場合、すべての部位が［修復不能］となり、キミは2d10-【身体】点のダメージを受ける。",
        ],
    ),
    (
        "FDC",
        "部位ダメージチャート：フレーム",
        &[
            "小破（不安定）：体勢を崩す。次のターン、キミは攻撃を行えない。この部位の【部位装甲】-1。",
            "小破（スクラッチ！）：フレームに醜い傷が残る。この部位の【部位装甲】-1。",
            "小破（アーマー損傷）：フレームが歪む。この部位の【部位装甲】-1。",
            "小破（アーマー損傷）：フレームがきしみ始め、ガラコの動きを阻害し始める。【移動力】-1。さらに、この部位の【部位装甲】-1。",
            "中破（放熱板損傷）：熱を機体外に逃すことができなくなる。［弱体1］を受ける。",
            "中破（スタビライザー損傷）：機体のバランス調整装置が故障する。【身体】10の判定を行うこと。失敗した場合、キミは次のターンを失う。［弱体1］を受ける。",
            "中破（貫通！）：パイロットに被害が！　キミはHPダメージ（1d10-【身体】）を受ける。［弱体1］を受ける。",
            "大破（停止）：フレームが動かない。キミは次のターンを失う。［弱体1］を受ける。",
            "大破（アーマー損傷）：フレームに甚大なダメージを受ける。以後、この部位の【部位装甲】に-3。［弱体1］を受ける。",
            "修復不能（フレーム崩壊）：フレームが［修復不能］となる。フレームの大部分が剥がれ落ち、ガラコの内部が晒される。以後、キミに対して部位狙いが行われる場合、その命中判定に対する修正（p21）は発生しなくなる。［弱体2］を受ける。",
        ],
    ),
    (
        "ADC",
        "部位ダメージチャート：アーム",
        &[
            "小破（アーマー損傷）：アームの装甲にヒビが入る。【部位装甲】-1。",
            "小破（武器落とし！）：【身体】8の判定を行う。失敗した場合、ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。【部位装甲】-1。",
            "小破（マニュピレータ損傷）：指が何本かちぎれ飛んだ。【操作性】-1、【部位装甲】-1。",
            "小破（機能停止）：次のターンの終了時まで、このアームを使った攻撃はできない。以後、この部位の【部位装甲】-1。",
            "中破（痙攣）：アームの動きがぶれ始める。［弱体1］を受ける。",
            "中破（武器落とし！）：ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。［弱体1］を受ける。",
            "中破（スピン）：機体が大きく回転する。【身体】10の判定を行うこと。失敗した場合、［伏せ］状態となった上、次のターンを失う。［弱体1］を受ける。",
            "大破（アーマー損傷）：以後、この部位の【部位装甲】を-3。［弱体1］を受ける。",
            "大破（武器落とし！）：ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。以後、この部位の【部位装甲】が0になる。［弱体1］を受ける。",
            "修復不能（破壊）：ダメージを受けた側のアームが［修復不能］となる。［弱体2］を受ける。",
        ],
    ),
    (
        "LDC",
        "部位ダメージチャート：レッグ",
        &[
            "小破（アーマー損傷）：以後、この部位の【部位装甲】-1。",
            "小破（よろめき）：以後、この部位の【部位装甲】-1。次のターン終了時まで、キミは移動できない。",
            "小破（スネア）：足元をすくわれる。【部位装甲】-1。さらに【身体】8の判定を行うこと。失敗した場合、キミは［伏せ］状態になる。",
            "小破（跛足）：以後、【移動力】-1、【部位装甲】-1。",
            "中破（シャフト損傷）：脚部の軸に歪みが生じる。［弱体1］を受ける。",
            "中破（アクチュエータ損傷）：脚部のアクチュエータに大きな損傷を受ける。【移動力】-1。［弱体1］を受ける。",
            "中破（スピン）：機体が大きく回転する。【身体】10の判定を行うこと。失敗した場合、［伏せ］状態となった上、次のターンを失う。［弱体1］を受ける。",
            "大破（アーマー損傷）：以後、この部位の【部位装甲】を-3。［弱体1］を受ける。",
            "大破（跛足）：以後、【移動力】-2。この部位の【部位装甲】が0になる。［弱体1］を受ける。",
            "修復不能（破壊）：ダメージを受けた側のレッグが［修復不能］となる。【移動力】-2。［弱体2］を受ける。",
        ],
    ),
    (
        "CDT",
        "部位ダメージチャートv2: コックピット",
        &[
            "アーマー損傷（小破/弱体0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "視界不良（小破/弱体0/装甲ダメージ1）モニターやハッチの歪み等により、視界を大きく遮られる。以後、【視認性】-1。",
            "強震（小破/弱体0/装甲ダメージ1）大きく揺さぶられる。キミは【身体】10 の判定を行なう。失敗した場合、次のターンを失う。",
            "貫通！（小破/弱体0/装甲ダメージ1）パイロットに被害が！キミはＨＰダメージ（1d10-【身体】）を受ける。",
            "計器損傷（中破/弱体1/装甲ダメージ1）コンソールの一部が停止する。",
            "制御不能（中破/弱体1/装甲ダメージ1）コントロールが効かなくなる。キミは次のターンを失う。",
            "貫通！（中破/弱体1/装甲ダメージ1）パイロットに被害が！キミはＨＰダメージ（1d10-【身体】）を受ける。",
            "故障（大破/弱体1/装甲ダメージ2）コックピットが完全にいかれる。キミは次のラウンド終了時まで、あらゆる判定に自動的にファンブルする。",
            "貫通！（大破/弱体1/装甲ダメージ2）パイロットに被害が！キミはＨＰダメージ（1d10+3-【身体】）を受ける。",
            "破壊（修復不能/弱体2/装甲ダメージ3）コックピットが「修復不能」となる。キミは2d10-【身体】点のＨＰダメージを受ける。ガラコはすべての機能を停止する。コックピットのハッチが自動的に開く。",
        ],
    ),
    (
        "EDT",
        "部位ダメージチャートv2: エンジン",
        &[
            "アーマー損傷（小破/弱体0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "アーマー損傷（小破/弱体0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "燃料漏れ（小破/弱体0/装甲ダメージ1）タンクから燃料が漏れる。燃料-1。",
            "燃料漏れ（小破/弱体0/装甲ダメージ1）タンクから燃料が漏れる。燃料-2。",
            "エンジン不調（中破/弱体1/装甲ダメージ1）エンジンの調子が安定しない。",
            "オーバーヒート（中破/弱体1/装甲ダメージ1）オーバーヒートする。次のターンの終了時まで、移動と攻撃を行えない。",
            "エンジン不調（中破/弱体1/装甲ダメージ1）なんだか調子悪い。キミは次のターンを失う。",
            "燃料漏れ（大破/弱体1/装甲ダメージ2）タンクから燃料が漏れる。燃料-3。",
            "貫通！（大破/弱体1/装甲ダメージ2）パイロットに被害が！キミはＨＰダメージ（1d10+3-【身体】）を受ける。",
            "エンジン停止（修復不能/弱体2/装甲ダメージ3）エンジンが停止する。ガラコはすべての機能を停止する。コックピットのハッチが自動的に開く。【操作性】10の判定を行なうこと。失敗するとエンジンが爆発する。その場合、すべての部位が［修復不能］となり、キミは2d10-【身体】点のＨＰダメージを受ける",
        ],
    ),
    (
        "FDT",
        "部位ダメージチャートv2: フレーム",
        &[
            "アーマー損傷 （小破 /0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "スクラッチ！（小破/弱体0/装甲ダメージ1）フレームに醜い傷が残る。",
            "歪み（小破/弱体0/装甲ダメージ1）フレームが歪み、ガラコの動きを阻害し始める。【移動力】-1。",
            "ハードポイント損傷（小破/弱体0/装甲ダメージ1）このフレームに装着している武器、及びオプションをすべて落とす。装着していた武器やオプションが外れかかる。キミは【身体】8の判定を行なう。失敗した場合、",
            "放熱板損傷（中破/弱体1/装甲ダメージ1）熱を機体外に逃すことができなくなる。これはヤバい。",
            "スタビライザー損傷（中破/弱体1/装甲ダメージ1）ターンを失う。機体のバランス調整装置が故障する。【身体】10の判定を行なうこと。失敗した場合、キミは次の",
            "貫通！（中破/弱体1/装甲ダメージ1）パイロットに被害が！キミはＨＰダメージ（1d10-【身体】）を受ける。",
            "停止（大破/弱体1/装甲ダメージ2）フレームが動かない。キミは次のターンを失う。",
            "ハードポイント破壊（大破/弱体1/装甲ダメージ2）武器やオプションを取り付ける箇所が破壊される。以後、このフレームに（スロットを消費して）装着している武器やオプションは使用できない（常時効果のあるものも、効果を失う）。",
            "フレーム崩壊（修復不能/弱体2/装甲ダメージ3）フレームが「修復不能」となる。フレームの大部分が剥がれ落ち、ガラコの内部が晒される。以後キミに対して部位狙いが行われる場合、その命中判定に対する修正（『GHT』p21）は発生しない。",
        ],
    ),
    (
        "ADT",
        "部位ダメージチャートv2: アーム",
        &[
            "アーマー損傷（小破/弱体0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "武器落とし！（小破/弱体0/装甲ダメージ1）キミは【身体】8の判定を行う。失敗した場合、ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。",
            "マニピュレータ損傷（小破/弱体0/装甲ダメージ1）指が何本かちぎれ飛んだ。【操作性】-1。",
            "機能停止（小破/弱体0/装甲ダメージ1）次のターンの終了時まで、このアームを使った攻撃はできない。",
            "痙攣（中破/弱体1/装甲ダメージ1）アームの動きがぶれ始める。",
            "武器落とし！（中破/弱体1/装甲ダメージ1）ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。",
            "スピン（中破/弱体1/装甲ダメージ1）機体が大きく回転する。【身体】10の判定を行うこと。失敗した場合、［伏せ］状態となった上、次のターンを失う。",
            "武器落とし！（大破/弱体1/装甲ダメージ2）ダメージを受けた側のアームに（スロットを消費して）装着していた武器を落とす。",
            "ハードポイント破壊（大破/弱体1/装甲ダメージ2）している武器やオプションは使用できない（常時効果のあるものも、効果を失う）。武器やオプションを取り付ける箇所が破壊される。以後、このアームに（スロットを消費して）装着している武器やオプションは使用できない（常時効果のあるものも、効果を失う）。",
            "破壊（修復不能/2/装甲ダメージ3）ダメージを受けた側のアームが「修復不能」となる。",
        ],
    ),
    (
        "LDT",
        "部位ダメージチャートv2: レッグ",
        &[
            "アーマー損傷 （小破 /弱体0/装甲ダメージ1）装甲に軽いひび割れが走る。",
            "よろめき （小破 /弱体0/装甲ダメージ1）次のターンの終了時まで、キミは移動できない。",
            "スネア （小破 /弱体0/装甲ダメージ1）足元をすくわれる。【身体】8 の判定を行うこと。失敗した場合、キミは［伏せ］状態になる。",
            "跛足 （小破 /弱体0/装甲ダメージ1）以後、【移動力】-1。",
            "シャフト損傷 （中破 /弱体1/装甲ダメージ1）脚部の軸に歪みが生じる。",
            "アクチュエータ損傷 （中破 /弱体1/装甲ダメージ1）脚部のアクチュエータに大きな損傷を受ける。【移動力】-1。",
            "スピン （中破 /弱体1/装甲ダメージ1）機体が大きく回転する。【身体】10 の判定を行うこと。失敗した場合、［伏せ］状態となった上、次のターンを失う。",
            "跛足 （大破 /弱体1/装甲ダメージ2）以後、【移動力】-2。",
            "ハードポイント破壊 （大破 /弱体1/装甲ダメージ2）している武器やオプションは使用できない（常時効果のあるものも、効果を失う）。 武器やオプションを取り付ける箇所が破壊される。以後、このレッグに（スロットを消費して）装着している武器やオプションは使用できない（常時効果のあるものも、効果を失う）。",
            "破壊 （修復不能 /弱体2/装甲ダメージ3）ダメージを受けた側のレッグが「修復不能」となる。【移動力】-2。",
        ],
    ),
];

/// Ruby `TABLES["PNM"]`（名前表：ピグマー族（男） / 1D10）。
static TABLE_PNM: Table = Table::from_dice(
    "名前表：ピグマー族（男）",
    1,
    10,
    &[
        "バビロン",
        "グリニッジ",
        "デトロイト",
        "ヨコスカ",
        "ボルドー",
        "テキサス",
        "シチリア",
        "チェルノブイリ",
        "グンマ",
        "サマルトリア",
    ],
);

/// Ruby `TABLES["PNF"]`（名前表：ピグマー族（女） / 1D10）。
static TABLE_PNF: Table = Table::from_dice(
    "名前表：ピグマー族（女）",
    1,
    10,
    &[
        "ルアンダ",
        "ローマ",
        "フロリダ",
        "ホノルル",
        "ツガル",
        "ゲルニカ",
        "シャンハイ",
        "モナコ",
        "チグリス",
        "オーサカ",
    ],
);

/// Ruby `TABLES["ENM"]`（名前表：エレメント族（男） / 1D10）。
static TABLE_ENM: Table = Table::from_dice(
    "名前表：エレメント族（男）",
    1,
    10,
    &[
        "アポロン",
        "ミキストリ",
        "アザゼル",
        "フマクト",
        "マサカド",
        "ククルカン",
        "ルシフェル",
        "ザギグ",
        "フェムト",
        "マイトレーヤ",
    ],
);

/// Ruby `TABLES["ENF"]`（名前表：エレメント族（女） / 1D10）。
static TABLE_ENF: Table = Table::from_dice(
    "名前表：エレメント族（女）",
    1,
    10,
    &[
        "クシナダ",
        "アルテミス",
        "ゼノビア",
        "フレイヤ",
        "イシュタム",
        "ベルゼバブ",
        "マイシェラ",
        "バステト",
        "スクルド",
        "アテナ",
    ],
);

/// Ruby `TABLES["NNM"]`（名前表：ノーマッド族（男） / 1D10）。
static TABLE_NNM: Table = Table::from_dice(
    "名前表：ノーマッド族（男）",
    1,
    10,
    &[
        "ドラム",
        "カホン",
        "ハレルヤ",
        "トリノウタ",
        "スリラー",
        "シンバル",
        "リュート",
        "ウクレレ",
        "タンバリン",
        "ユメコウネン",
    ],
);

/// Ruby `TABLES["NNF"]`（名前表：ノーマッド族（女） / 1D10）。
static TABLE_NNF: Table = Table::from_dice(
    "名前表：ノーマッド族（女）",
    1,
    10,
    &[
        "ピアノ",
        "テルミン",
        "ソバカス",
        "イマジン",
        "ツナミ",
        "ピッコロ",
        "ハープ",
        "シャミセン",
        "ミザルー",
        "ドナドナ",
    ],
);

/// Ruby `TABLES["RNM"]`（名前表：ラット族（男） / 1D10）。
static TABLE_RNM: Table = Table::from_dice(
    "名前表：ラット族（男）",
    1,
    10,
    &[
        "ポチ",
        "シシマル",
        "ポンタ",
        "コテツ",
        "アルフォンス",
        "パトラッシュ",
        "ミッキー",
        "ジジ",
        "サカモト",
        "オンソクマル",
    ],
);

/// Ruby `TABLES["RNF"]`（名前表：ラット族（女） / 1D10）。
static TABLE_RNF: Table = Table::from_dice(
    "名前表：ラット族（女）",
    1,
    10,
    &[
        "タマ",
        "ココ",
        "ラブ",
        "ピーコ",
        "モカ",
        "オリガミ",
        "ヒメ",
        "ミィ",
        "ルナ",
        "ク・メル",
    ],
);

/// Ruby `TABLES["BN1"]`（名前表：ブレイン族（その１） / 1D10）。
static TABLE_BN1: Table = Table::from_dice(
    "名前表：ブレイン族（その１）",
    1,
    10,
    &[
        "マリファナ",
        "バファリン",
        "タミフル",
        "セーロガン",
        "モルヒネ",
        "ハルシオン",
        "トリカブト",
        "バイアグラ",
        "エリクサー",
        "クラレ",
    ],
);

/// Ruby `TABLES["BN2"]`（名前表：ブレイン族（その２） / 1D10）。
static TABLE_BN2: Table = Table::from_dice(
    "名前表：ブレイン族（その２）",
    1,
    10,
    &[
        "ニトロ",
        "ダイオキシン",
        "タウリン",
        "コイーバ",
        "マールボロ",
        "キャメル",
        "ドクダミ",
        "アブサン",
        "ドブロク",
        "マティーニ",
    ],
);

/// Ruby `TABLES["TN1"]`（名前表：テンタクル族（その１） / 1D10）。
static TABLE_TN1: Table = Table::from_dice(
    "名前表：テンタクル族（その１）",
    1,
    10,
    &[
        "アップル",
        "プリン",
        "ビフテキ",
        "ガンモ",
        "レバニラ",
        "カボチャ",
        "コロッケ",
        "マトン",
        "ギョーザ",
        "タバスコ",
    ],
);

/// Ruby `TABLES["TN2"]`（名前表：テンタクル族（その２） / 1D10）。
static TABLE_TN2: Table = Table::from_dice(
    "名前表：テンタクル族（その２）",
    1,
    10,
    &[
        "キノコ",
        "セロリ",
        "ラザニア",
        "ユドーフ",
        "ニンジン",
        "カイワレ",
        "ボルシチ",
        "ハクサイ",
        "キャラメル",
        "ワタアメ",
    ],
);

/// Ruby `TABLES["MTV"]`（動機決定表 / 1D10）。
static TABLE_MTV: Table = Table::from_dice(
    "動機決定表",
    1,
    10,
    &[
        "金。お宝の臭いがした。",
        "正義。破界の塔は災いのもと。絶たねばならない。",
        "友情。この破界の塔のせいで友人が困っている。助けなくちゃ。",
        "探究心。破界の塔のことをもっと知りたい。",
        "戦闘狂。もっと戦いたい。",
        "暇つぶし。退屈な日常を忘れたい。",
        "自殺願望。なんかもう死にたい。",
        "冒険家。ワクワクしたい。",
        "山男。シティが肌に合わない。",
        "特に動機らしい動機はない。",
    ],
);

/// Ruby `TABLES["HIT"]`（部位決定チャート / 1D10）。
static TABLE_HIT: Table = Table::from_dice(
    "部位決定チャート",
    1,
    10,
    &[
        "コックピット",
        "エンジン",
        "フレーム",
        "フレーム",
        "フレーム",
        "フレーム",
        "ライトアーム",
        "レフトアーム",
        "ライトレッグ",
        "レフトレッグ",
    ],
);

/// Ruby `TABLES["GCC"]`（ガラコ改造チャート表 / 1D10）。
static TABLE_GCC: Table = Table::from_dice(
    "ガラコ改造チャート表",
    1,
    10,
    &[
        "【命中+】価格+200。【操作性】+1。［不安定］1。",
        "【回避+】価格+200。【機動性】+1。［不安定］1。",
        "【視界+】価格+200。【視認性】+2。［不安定］1。",
        "【移動+】価格+100。【移動力】+1。",
        "【火力+】価格+200。その部位に装着した武器の火力を常に+2する。",
        "【部位装甲+】価格+100。【部位装甲】+2。",
        "【限界重量+】価格+100。【限界重量】+1000。",
        "【安定性+】価格+50。［不安定］-1。",
        "【スロット+】価格+500。【スロット】+1。",
        "【弱体無効】価格+500。このパーツへの部位ダメージによる[弱体]の効果を無視する。",
    ],
);

/// Ruby `TABLES["WCC"]`（武器改造チャート表 / 1D10）。
static TABLE_WCC: Table = Table::from_dice(
    "武器改造チャート表",
    1,
    10,
    &[
        "【命中+】価格+200。【操作性】+1。",
        "【火力+】価格+200。【火力】+2。",
        "【射程】価格+200。【射程】+3。「射程：近接」の場合、「射程:3 or 近接」となる(攻撃する度にどちらかを選ぶ)。",
        "【範囲+】価格+200。1シーンにつき1回、この武器の目標を「範囲2」に変更してもよい(フリーアクション)。もともと範囲攻撃できる武器の場合は、「範囲n+1」にできる(1シーン1回、フリーアクション)。",
        "【部位変更】価格+200。装着できる部位がランダムに追加される。部位決定チャート(『GHT』p21)を使用して決めること。",
        "【部位装甲+】価格+100。装着した部位の【部位装甲】+2。",
        "【精度+】価格+100。この武器を使って狙い撃ちをする場合、命中判定に+1。",
        "【装飾+】価格+500。特に効果はないが、売却した時の金額が上昇する。",
        "【幸運+】価格+500。この武器による命中判定の出目が1だった場合、判定を振り直しても良い(1シーン1回まで)。",
        "【回数無限】価格+500。武器の使用回数制限がなくなる。",
    ],
);

/// Ruby `TABLES["EVC"]`（イベントチャート表 / 1D10）。
static TABLE_EVC: Table = Table::from_dice(
    "イベントチャート表",
    1,
    10,
    &[
        "【クリーチャー】スタートル(『GTD』p30)が1d10+3体現れる。戦闘開始。",
        "【ビット】コーンノーズ(『GTD』p23)が1d10+3体現れる。戦闘開始。",
        "【ノーマッド】ノーマッド族のランドクローラーと遭遇する。このシーンはノーマッドからアイテムを購入しても良い。ノーマッド族は天蓋都市で購入できるすべてのアイテムを販売している(ただし金額は20%増し)。",
        "【ピグマー族】君達の目的地方面から、ボロボロになったピグマー族のNPCが歩いてくる。NPCに何があったのかはGMが決めよ。ピグマー族を天蓋都市まで送った場合、謝礼として200クレジットを受け取ることが出来る。NPCは重量50のアイテムとして扱う。",
        "【ビット】ダスクウォッチ(『GTD』p23)が1d10+3体現れる。戦闘開始。",
        "【異常気象】嵐、竜巻、豪雨など、異常な気象によって行動を阻害される。PCのうち代表者1名が【視認】10の判定を行うこと。失敗した場合、次のシーンはスポットを移動できない。現在のスポットに留まることになる。",
        "【クリーチャー】ナグ(『GTD』p31)が1d10+4体現れる。戦闘開始。",
        "【ビット】ランオーバー(『GTD』p25)が3体現れる。戦闘開始。",
        "【猛毒の霧】付近に毒の霧が立ち込める。全てのキャラクターは毒によって1d10のHPダメージを受ける。",
        "【最悪の敵】ズルワーン(『GTD』p29)が1体現れる。戦闘開始。",
    ],
);

/// Ruby `TABLES["BSD"]`（戦闘開始距離 / 1D10）。
static TABLE_BSD: Table = Table::from_dice(
    "戦闘開始距離",
    1,
    10,
    &[
        "3マス", "3マス", "6マス", "6マス", "9マス", "9マス", "12マス", "12マス", "15マス",
        "15マス",
    ],
);

/// Ruby `TABLES['IDI']`（個性表 / 1D100）。
static TABLE_IDI: RangeTable = RangeTable::from_dice(
    "個性表",
    1,
    100,
    &[
        (RangeInc::new(1, 5), "〈近接武器熟練〉 近接攻撃の火力+1。"),
        (RangeInc::new(6, 10), "〈遠隔武器熟練〉 遠隔攻撃の火力+1。"),
        (RangeInc::new(11, 15), "〈天才〉 【技術】+1。"),
        (RangeInc::new(16, 20), "〈頑強〉 【身体】+1。"),
        (RangeInc::new(21, 25), "〈早業〉 【速度】+1。"),
        (RangeInc::new(26, 30), "〈スイフトフット〉 【移動力】+1。"),
        (RangeInc::new(31, 35), "〈超反応〉 行動判定値+2。"),
        (RangeInc::new(36, 40), "〈警戒心〉 罠を発見するための判定に+2。"),
        (RangeInc::new(41, 45), "〈解除屋〉 罠を解除するための判定に+2。"),
        (RangeInc::new(46, 50), "〈タフガイ〉 最大HP+5。"),
        (RangeInc::single(51), "〈踏み込み〉 キミが使用する近接武器のデータを「射程：2」に変更する。"),
        (RangeInc::single(52), "〈不動〉 キミは強制移動の効果を受けない。"),
        (RangeInc::single(53), "〈ペイローダー〉 ガラコの【限界重量】+2000。"),
        (RangeInc::single(54), "〈魅力〉 キミがHPを回復するアイテム、もしくは超能力の目標になった時、キミのHPを追加で1点回復する。"),
        (RangeInc::single(55), "〈ダブルタップ〉 キミのターン開始時に使用。このターンの間、キミは追加で1回の遠隔攻撃を行うことができる。"),
        (RangeInc::single(56), "〈薙ぎ払い〉 キミのターン開始時に使用。このターンの間、キミが行う近接攻撃の目標を「周囲1マス以内にいるすべての敵」に変更する。"),
        (RangeInc::single(57), "〈武器落とし〉 キミは部位ひとつを指定する。目標は指定された部位に（スロットを消費して）装着している武器すべてを地面に落とす。"),
        (RangeInc::single(58), "〈切り払い〉 キミが行う回避判定の直前に使用。その判定を、【機動性】ではなく【操作性】で判定してよい。ただし、キミは近接武器を装着していなければならない。"),
        (RangeInc::single(59), "〈体崩しの達人〉 キミが目標のレッグに攻撃を命中させる度、その目標は【機動性】10の判定を行う。失敗した場合、目標は［伏せ］状態になる。"),
        (RangeInc::single(60), "〈超分解術〉 アイテムひとつを目標にする。目標のアイテムの重量を1/4にする。ただし、そのアイテムは使用できなくなる。再度〈超分解術〉の判定に成功することで、元に戻せる（重量が元に戻り、アイテムが使用可能になる）。"),
        (RangeInc::single(61), "〈即時換装〉 キミは、ガラコのパーツ換装を（ベースアクションではなく）ムーブアクションで行ってもよい。"),
        (RangeInc::single(62), "〈ノックバック〉 キミが目標に5点以上の最終ダメージを与えた直後に使用。目標を1マス、任意の方向に強制移動させる。近接武器で攻撃した場合のみ使用できる。"),
        (RangeInc::single(63), "〈照準〉 このターンの間、次に行う攻撃の命中判定+1。"),
        (RangeInc::single(64), "〈燃料節約術〉 戦闘時以外、キミは燃料を消費しなくてよい。"),
        (RangeInc::single(65), "〈追撃〉 キミの敵が、隣接するマスから離れるような移動を宣言した直後に使用。キミはその敵に対して近接攻撃を行う。近接攻撃の後、敵は移動を行うこと。"),
        (RangeInc::single(66), "〈連撃〉 キミが敵の部位を［修復不能］にした直後に使用。キミは再度、その敵に対して攻撃を行う。"),
        (RangeInc::single(67), "〈殺し屋〉 キミがコックピットに攻撃を命中させる度、そのガラコの操縦者は2点のHPを失う。"),
        (RangeInc::single(68), "〈極大射程〉 キミが扱う遠隔武器の射程を2倍にする。"),
        (RangeInc::single(69), "〈援護射撃〉 目標が回避判定を行った直後に使用。目標の回避判定の達成値-1。その後、キミは準備済みの遠隔武器ひとつの使用回数を1減らすこと。"),
        (RangeInc::single(70), "〈鉄壁〉 キミがダメージを受けた直後に使用。そのダメージを無効化する。"),
        (RangeInc::single(71), "〈心臓狙い〉 キミが部位狙いを行い、コックピット、もしくはエンジンに対して攻撃を行う際、命中判定+1。"),
        (RangeInc::single(72), "〈四肢狙い〉 キミが部位狙いを行い、アーム、もしくはレッグに対して攻撃を行う際、命中判定+1。"),
        (RangeInc::single(73), "〈窮地逆転〉 キミの判定の出目が1だった時、その出目を10に変更する。"),
        (RangeInc::single(74), "〈防御重視〉 ラウンド開始時に使用。【操作性】-1。【機動性】+2。ラウンド終了時まで。"),
        (RangeInc::single(75), "〈チアガール〉 目標は即座に追加のターンを得る。"),
        (RangeInc::single(76), "〈毒半減〉 キミが［毒］状態になった時、毎回失うHPを1点減らす。ノーマッド族はこの個性を取得できない。"),
        (RangeInc::single(77), "〈毒無効〉 キミは［毒］状態にならない。この個性はノーマッド族だけが取得できる。"),
        (RangeInc::single(78), "〈生存術〉 キミは各シーン終了時、HPを減らさなくてよい。"),
        (RangeInc::single(79), "〈平衡感覚〉 キミは［不安定］状態のペナルティを受けない。"),
        (RangeInc::single(80), "〈不屈〉 キミのターン開始時に使用。このターンの間、キミはガラコの損傷による［弱体］の効果を受けない。"),
        (RangeInc::single(81), "〈プレデターセンス〉 近接攻撃の命中判定+2。この個性はラット族だけが取得できる。"),
        (RangeInc::single(82), "〈鷹の目〉 遠隔攻撃の命中判定+2。この個性はラット族だけが取得できる。"),
        (RangeInc::single(83), "〈超リペア術〉 部位をひとつ選択する。目標の部位の被ダメージすべてを一時的に回復する（修復不能を除く）。回復したダメージは、シーン終了時に元に戻る（再度壊れる）。この個性はブレイン族のみが取得できる。"),
        (RangeInc::single(84), "〈浮遊術〉 キミは［飛行］状態になる。シーン終了時まで。この個性はテンタクル人のみが取得できる。"),
        (RangeInc::single(85), "〈瞬間移動術〉 キミは任意のマスに瞬間移動する。この個性はテンタクル人のみが取得できる。"),
        (RangeInc::single(86), "〈ハイボルテージ〉 4ラウンドめ以降、キミが持つすべての武器の火力を+2する。"),
        (RangeInc::single(87), "〈スライドディフェンス〉 キミが部位決定チャートを振った直後に使用。チャートの結果を+1する。"),
        (RangeInc::single(88), "〈カーブアタック〉 目標が部位決定チャートを振った直後に使用。チャートの結果を-1する。"),
        (RangeInc::single(89), "〈サイコショット〉 目標に［火力0］の攻撃を行う（自動命中）。（超能力）"),
        (RangeInc::single(90), "〈ファイア〉 目標に［火力5］の攻撃を行う。（超能力）"),
        (RangeInc::single(91), "〈アイス〉 目標に［火力3］の攻撃を行う。（超能力）"),
        (RangeInc::single(92), "〈サンダー〉 目標に［火力2］の攻撃を行う。（超能力）"),
        (RangeInc::single(93), "〈テレパシー〉 キミは念話によって会話することができる。（超能力）"),
        (RangeInc::single(94), "〈ミラー〉 目標が超能力の使用を宣言した直後に使用。超能力の目標を使用者自身に変更する。（超能力）"),
        (RangeInc::single(95), "〈バインド〉 目標のターン開始時に使用。目標の移動力-3。ターン終了時まで。（超能力）"),
        (RangeInc::single(96), "〈アーマー〉 目標のすべての部位装甲+2。シーン終了時まで。（超能力）"),
        (RangeInc::single(97), "〈バリア〉 目標がダメージを受けた直後に使用。ダメージを3点軽減する。（超能力）"),
        (RangeInc::single(98), "〈ヒール〉 目標のHPを［1d10-4］点回復する。出目が低いとHPを失う可能性があることに注意。（超能力）"),
        (RangeInc::single(99), "〈カース〉 目標が判定を行った直後に使用。その判定の達成値を-3する。"),
        (RangeInc::single(100), "〈リザレクション〉 死んだ目標を生き返らせる。生き返った目標のHPは10になる。このシーンの間に死亡したキャラクターのみ目標にできる。（超能力）"),
    ],
);

/// Ruby `TABLES`（`IDI` 以外。`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[
    ("PNM", &TABLE_PNM),
    ("PNF", &TABLE_PNF),
    ("ENM", &TABLE_ENM),
    ("ENF", &TABLE_ENF),
    ("NNM", &TABLE_NNM),
    ("NNF", &TABLE_NNF),
    ("RNM", &TABLE_RNM),
    ("RNF", &TABLE_RNF),
    ("BN1", &TABLE_BN1),
    ("BN2", &TABLE_BN2),
    ("TN1", &TABLE_TN1),
    ("TN2", &TABLE_TN2),
    ("MTV", &TABLE_MTV),
    ("HIT", &TABLE_HIT),
    ("GCC", &TABLE_GCC),
    ("WCC", &TABLE_WCC),
    ("EVC", &TABLE_EVC),
    ("BSD", &TABLE_BSD),
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Garako", "Garako.toml", 52);
    }
}
