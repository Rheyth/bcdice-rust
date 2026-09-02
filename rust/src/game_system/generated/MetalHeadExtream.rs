//! P4で手書き移植した `lib/bcdice/game_system/MetalHeadExtream.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#checkRoll`（判定 `ARn` / `SRn` とロール修正・アクシデント値・高度なロール・ラック・《ミューズ》）
//! - `#get_hit_table`（命中部位表 `(部位)HIT[n]`）
//! - `#get_SUV_table`（戦闘結果表 `SUV(A～Z)n`）
//! - `#get_damageEffect_table`（損傷効果表 `(部位)DMG(種別)`）
//! - `#get_critical_table`（クリティカル表 `CRT[n]`）
//! - `#get_accident_table`（アクシデント表 `(種別)AC[n]`）
//! - `#get_mechanicAccident_table`（メカニック事故表 `(場所)MA[n][+m]`）
//! - `#get_strategyEvent_chart` / `#get_NPCAttack_chart` / `#get_loserDestiny_chart`（マスコンバット）
//! - `#get_randomEncounter_table`（荒野ランダムエンカウント表 `WENC[n]`）
//!
//! 表データ（`static` 群）は原典 rb から機械的に書き出したもので、値は1文字も変えていない。
//!
//! # 浮動小数点の扱い
//!
//! Ruby の `get_value` はロール修正を `Float` で積み上げ、`checkRoll` は
//! `(target * modify / advancedRoll * (2**luckPoint)).to_i` のように `Float` と `Integer` を
//! 混ぜて計算する。Rust でも同じ順序で `f64` 演算し、`Float#to_i` は
//! [`float_to_i_string`] で再現する（非有限なら Ruby と同じく `FloatDomainError`、
//! `i64` を超える値は Ruby の多倍長と同じ十進表記にする）。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::MetalHeadExtream`（ID: `MetalHeadExtream`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalHeadExtream;

impl GameSystem for MetalHeadExtream {
    fn id(&self) -> &'static str {
        "MetalHeadExtream"
    }

    fn name(&self) -> &'static str {
        "メタルヘッドエクストリーム"
    }

    fn sort_key(&self) -> &'static str {
        "めたるへつとえくすとりいむ"
    }

    fn help_message(&self) -> &'static str {
        r"◆判定：ARn or SRn[*/a][@b][Ac][Ld][!M]　　[]内省略可。
「n」で判定値、「*/a」でロール修正を指定。複数回指定可。
「@b」でアクシデント値、省略時は「96」。
「Ac」で高度なロール。「2、4、8」のみ指定可能。
「Ld」でラックポイント、「!M」でパンドラ《ミューズ》。

【書式例】
AR84/2@99!M → 判定値84のAR1/2。アクシデント値99、パンドラ《ミューズ》。
SR40*2A2L1@99 → 判定値80のSR、高度なロール2倍、ラック1点。

◆命中部位表：(命中部位)HIT[n]　　以降、ROC時は[n]を指定。
HU：人間　　BK：バイク　　WA：ワゴン　　SC：シェルキャリア　　BG：バギー
IN：インセクター　　PT：ポケットタンク　　HT：ホバータンク　　TA：戦車
AC：装甲車　　HE：ヘリ　　TR：トレーラー　　VT：VTOL　　BO：ボート
CS：通常、格闘型コンバットシェル　　TH：可変、重コンバットシェル
AM：オートモビル　　GD：ガンドック　　HC：ホバークラフト
BI：自転車　　BT：バトルトレーラー　　AI：エアクラフト
◆戦闘結果表：SUV(A～Z)n　　【書式例】SUVM100
◆損傷効果表：(命中部位)DMG(損傷種別)　　【書式例】TDMGH
H：頭部　　T：胴部　　A：腕部　　L：脚部　　M：心理　　E：電子
B：メカニック本体　　P：パワープラント　　D：ドライブ
(損傷種別)　L：LW　　M：MW　　H：HW　　O：MO
◆クリティカル表：CRT[n]
◆アクシデント表：(種別)AC[n]
G：格闘　　S：射撃、投擲　　M：心理　　E：電子
◆メカニック事故表：(場所)MA[n][+m]　　「+m」で修正を指定。
A：空中　　S：水上、水中　　L：地上

【マスコンバット】
ストラテジーイベントチャート：SEC
NPC攻撃処理チャート：NAC　　敗者運命チャート：LDC

【各種表】
荒野ランダムエンカウント表：WENC[n]
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "[AS]R",
            "(HU|BK|WA|SC|BG|IN|PT|HT|TA|AC|HE|TR|VT|BO|CS|TH|AM|GD|HC|BI|BT|AI)HIT",
            "SUV[A-Z]",
            "[HTALMEBPD]DMG[LMHO]",
            "CRT",
            "[GSME]AC",
            "[ASL]MA",
            "SEC",
            "NAC",
            "LDC",
            "[W]ENC",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific_command(command, rng)?.map(SpecificCommandOutput::text))
    }
}

// ---------------------------------------------------------------------------
// 正規表現（Ruby の `case command.upcase when ...` の各枝）
// ---------------------------------------------------------------------------

/// Ruby の `\d` は ASCII 限定なので、以下では `[0-9]` を明示する。
fn regex(source: &str) -> Regex {
    Regex::new(source).expect("valid regex")
}

/// Ruby `%r{([AS])R(\d+)(([*/]\d+)*)?(((@|A|L)\d+)*)(!M)?$}i`。
fn check_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)([AS])R([0-9]+)(([*/][0-9]+)*)?(((@|A|L)[0-9]+)*)(!M)?$"))
}

/// Ruby `/(HU|BK|…|AI)HIT(\d+)?/i`。
fn hit_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex(
            r"(?i)(HU|BK|WA|SC|BG|IN|PT|HT|TA|AC|HE|TR|VT|BO|CS|TH|AM|GD|HC|BI|BT|AI)HIT([0-9]+)?",
        )
    })
}

/// Ruby `/SUV([A-Z])(\d+)/i`。
fn suv_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)SUV([A-Z])([0-9]+)"))
}

/// Ruby `/([HTALMEBPD])DMG([LMHO])/i`。
fn damage_effect_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)([HTALMEBPD])DMG([LMHO])"))
}

/// Ruby `/CRT(\d+)?/i`。
fn critical_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)CRT([0-9]+)?"))
}

/// Ruby `/([GSME])AC(\d+)?/i`。
fn accident_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)([GSME])AC([0-9]+)?"))
}

/// Ruby `/([ASL])MA(\d+)?(\+(\d+))?/i`。
fn mechanic_accident_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)([ASL])MA([0-9]+)?(\+([0-9]+))?"))
}

/// Ruby `/(W)ENC(\d+)?/i`。
fn random_encounter_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(?i)(W)ENC([0-9]+)?"))
}

/// Ruby `get_value` の `scan(%r{[*/]\d*})`。
fn modify_token_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"[*/][0-9]*"))
}

/// Ruby `paramText.scan(/(.)(\d+)/)`。
fn roll_parameter_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| regex(r"(.)([0-9]+)"))
}

// ---------------------------------------------------------------------------
// 数値の補助
// ---------------------------------------------------------------------------

/// Ruby `String#to_i`（ここに来るのは `\d+` にマッチした文字列だけ）。
///
/// Ruby は多倍長だが、i64 に収まらない入力は飽和させる。
fn to_i(digits: &str) -> i64 {
    digits.bytes().fold(0i64, |acc, b| {
        acc.saturating_mul(10).saturating_add(i64::from(b - b'0'))
    })
}

/// `mantissa * 2^shift` を十進表記にする（多倍長の再現用）。
fn shifted_decimal(mantissa: u64, shift: u32) -> String {
    // 十進の桁を下位から持つ
    let mut digits: Vec<u8> = Vec::new();
    let mut m = mantissa;
    while m > 0 {
        digits.push((m % 10) as u8);
        m /= 10;
    }
    if digits.is_empty() {
        digits.push(0);
    }
    for _ in 0..shift {
        let mut carry = 0u8;
        for d in digits.iter_mut() {
            let v = *d * 2 + carry;
            *d = v % 10;
            carry = v / 10;
        }
        if carry > 0 {
            digits.push(carry);
        }
    }
    digits.iter().rev().map(|d| char::from(b'0' + d)).collect()
}

/// Ruby `Float#to_i` を文字列で再現する。
///
/// 非有限（`Infinity` / `NaN`）なら Ruby と同じく `FloatDomainError`。
/// `i64` に収まらない値は、Ruby が返す多倍長整数と同じ十進表記を組み立てる
/// （`f64` の整数部は `仮数 * 2^指数` で厳密に表せる）。
fn float_to_i_string(value: f64) -> Result<String, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::FloatDomain);
    }
    let truncated = value.trunc();
    // 2^63 未満なら i64 で表せる（`as` は切り捨て・飽和だが、この範囲では正確）
    if truncated.abs() < 9_223_372_036_854_775_808.0 {
        return Ok((truncated as i64).to_string());
    }
    let bits = truncated.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64 - 1075;
    let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    // |truncated| >= 2^63 なので指数は正
    let mut text = shifted_decimal(mantissa, u32::try_from(exponent).unwrap_or(0));
    if truncated < 0.0 {
        text.insert(0, '-');
    }
    Ok(text)
}

/// Ruby `2**luckPoint` の `Float` 換算（`Float * Integer` は Integer を `to_f` する）。
///
/// `2**1024` 以上は Ruby でも `Infinity` になる。
fn pow2_f64(luck_point: i64) -> f64 {
    match i32::try_from(luck_point) {
        Ok(exp) if exp < 1024 => 2f64.powi(exp),
        _ => f64::INFINITY,
    }
}

/// Ruby `2**luckPoint` の十進表記（`Integer` なので多倍長）。
///
/// ここに来る時点で `checkRoll` の `rollTarget` 計算が通っている
/// （= `luckPoint < 1024`）ので、桁数は高々 309 桁。
fn pow2_string(luck_point: i64) -> String {
    shifted_decimal(1, u32::try_from(luck_point).unwrap_or(u32::MAX))
}

// ---------------------------------------------------------------------------
// 表の部品
// ---------------------------------------------------------------------------

/// Ruby の `[[番号, '本文'], …]` 形式の表。
struct NumberedTable {
    name: &'static str,
    items: &'static [(i64, &'static str)],
}

/// Ruby `Base#get_table_by_number(index, table, default = "1")`。
/// 番号が `index` 以上の最初の項目を返す。
fn get_table_by_number(index: i64, table: &NumberedTable) -> &'static str {
    table
        .items
        .iter()
        .find(|(number, _)| *number >= index)
        .map_or("1", |(_, text)| text)
}

/// Ruby `#get_roc_dice(roc, diceMax)`。
///
/// `roc`（出目指定）が `diceMax` を超えていれば `diceMax` に丸め、0 なら振る。
fn get_roc_dice(roc: i64, dice_max: i64, rng: &mut Randomizer) -> Result<i64, EvalError> {
    let mut dice = roc;
    if dice > dice_max {
        dice = dice_max;
    }
    if dice == 0 {
        dice = rng.roll_once(dice_max)?;
    }
    Ok(dice)
}

/// Ruby `#get_MetalHeadExtream_1dX_table_result(name, table, roc, diceMax)`。
fn table_result(
    table: &NumberedTable,
    roc: i64,
    dice_max: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice = get_roc_dice(roc, dice_max, rng)?;
    let text = get_table_by_number(dice, table);
    Ok(format!("{}({}) ＞ {}", table.name, dice, text))
}

/// キー付きの表群から `key` の表を探す（Ruby の `case hitPart when 'HU' …`）。
fn find_table(
    tables: &'static [(&'static str, NumberedTable)],
    key: &str,
) -> Option<&'static NumberedTable> {
    tables
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, table)| table)
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `MetalHeadExtream#eval_game_system_specific_command`。
///
/// `case command.upcase when …` を同じ順序で試す。どの枝にも当たらなければ `nil`。
fn eval_specific_command(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let command = command.to_uppercase();

    if let Some(m) = check_roll_pattern().captures(&command) {
        let roll_type = &m[1];
        let target = to_i(&m[2]);
        let modify = get_value(1.0, m.get(3).map_or("", |g| g.as_str()));
        let param_text = m.get(5).map_or("", |g| g.as_str());
        // パンドラ《ミューズ》
        let is_muse = m.get(8).is_some();

        let mut accident_value = 96;
        let mut advanced_roll = 1;
        let mut luck_point = 0;

        for params in roll_parameter_pattern().captures_iter(param_text) {
            let marker = &params[1];
            let value = to_i(&params[2]);
            // Ruby `get_roll_parameter`
            match marker {
                "@" => accident_value = value,
                "A" => {
                    if [2, 4, 8].contains(&value) {
                        advanced_roll = value;
                    }
                }
                "L" => luck_point = value,
                _ => {}
            }
        }

        return check_roll(
            roll_type,
            target,
            modify,
            accident_value,
            advanced_roll,
            luck_point,
            is_muse,
            rng,
        )
        .map(Some);
    }

    if let Some(m) = hit_pattern().captures(&command) {
        let hit_part = &m[1];
        let roc = m.get(2).map_or(0, |g| to_i(g.as_str()));
        return get_hit_table(hit_part, roc, rng);
    }

    if let Some(m) = suv_pattern().captures(&command) {
        let armor_grade = &m[1];
        let damage = to_i(&m[2]);
        return Ok(get_suv_table(armor_grade, damage));
    }

    if let Some(m) = damage_effect_pattern().captures(&command) {
        let hit_part = &m[1];
        let damage_stage = &m[2];
        return Ok(get_damage_effect_table(hit_part, damage_stage));
    }

    if let Some(m) = critical_pattern().captures(&command) {
        let roc = m.get(1).map_or(0, |g| to_i(g.as_str()));
        return table_result(&CRITICAL_TABLE, roc, 10, rng).map(Some);
    }

    if let Some(m) = accident_pattern().captures(&command) {
        let damage_type = &m[1];
        let roc = m.get(2).map_or(0, |g| to_i(g.as_str()));
        return get_accident_table(damage_type, roc, rng);
    }

    if let Some(m) = mechanic_accident_pattern().captures(&command) {
        let location_type = &m[1];
        let roc = m.get(2).map_or(0, |g| to_i(g.as_str()));
        let correction = m.get(4).map_or(0, |g| to_i(g.as_str()));
        return get_mechanic_accident_table(location_type, roc, correction, rng);
    }

    if command == "SEC" {
        return table_result(&STRATEGY_EVENT_CHART, 0, 100, rng).map(Some);
    }

    if command == "NAC" {
        return table_result(&NPC_ATTACK_CHART, 0, 10, rng).map(Some);
    }

    if command == "LDC" {
        return table_result(&LOSER_DESTINY_CHART, 0, 10, rng).map(Some);
    }

    if let Some(m) = random_encounter_pattern().captures(&command) {
        let location_type = &m[1];
        let roc = m.get(2).map_or(0, |g| to_i(g.as_str()));
        return get_random_encounter_table(location_type, roc, rng);
    }

    Ok(None)
}

/// Ruby `#get_value(originalValue, calculateText)`。
///
/// 「端数が使いたいので、parren_killer未使用」——`*n` / `/n` を `Float` で順に適用する。
fn get_value(original_value: f64, calculate_text: &str) -> f64 {
    let mut result = original_value;
    for token in modify_token_pattern().find_iter(calculate_text) {
        let token = token.as_str();
        // Ruby: `i =~ %r{([*/])(\d*)}i` → `Regexp.last_match(2).to_i`（空なら 0）
        let value = to_i(&token[1..]) as f64;
        match &token[..1] {
            "*" => result *= value,
            "/" => result /= value,
            _ => {}
        }
    }
    result
}

/// Ruby `#checkRoll(rollText, target, modify, accidentValue, advancedRoll, luckPoint, isMuse)`。
#[allow(clippy::too_many_arguments)]
fn check_roll(
    roll_text: &str,
    target: i64,
    modify: f64,
    accident_value: i64,
    advanced_roll: i64,
    luck_point: i64,
    is_muse: bool,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    // Ruby: (target * modify / advancedRoll * (2**luckPoint)).to_i
    //       Integer * Float → Float、以後は Float 演算（Integer は to_f される）
    let roll_target_f = target as f64 * modify / advanced_roll as f64 * pow2_f64(luck_point);
    let roll_target = float_to_i_string(roll_target_f)?;

    let dice = rng.roll_once(100)?;
    let (mut result_text, success_value) = get_roll_result_text_and_success_value(
        dice,
        advanced_roll,
        roll_target_f.trunc(),
        accident_value,
        is_muse,
    );

    result_text += &format!(" 達成値：{success_value}");

    let mut complement_text = format!("ACC:{accident_value}");
    if advanced_roll > 1 {
        complement_text += &format!(", ADV:*{advanced_roll}");
    }
    if luck_point > 0 {
        complement_text += &format!(", LUC:{luck_point}");
    }

    let modify_text = if modify >= 1.0 {
        float_to_i_string(modify)?
    } else {
        format!("1/{}", float_to_i_string(1.0 / modify)?)
    };

    let formula_text = get_formula_text(target, modify, advanced_roll, luck_point)?;

    let mut result = format!(
        "{roll_text}R{modify_text}({complement_text})：1D100<={roll_target}{formula_text} ＞ [{dice}] {result_text}"
    );
    if is_muse {
        result += " 《ミューズ》";
    }

    Ok(result)
}

/// Ruby `#getRollResultTextAndSuccesValue(dice, advancedRoll, rollTarget, accidentValue, isMuse)`。
///
/// `roll_target` は `Float#to_i` 後の値（`f64` の整数値）。出目は高々 100 なので
/// `f64` 同士の比較で Ruby の `Integer` 比較と一致する。
fn get_roll_result_text_and_success_value(
    dice: i64,
    advanced_roll: i64,
    roll_target: f64,
    accident_value: i64,
    is_muse: bool,
) -> (String, i64) {
    let success_value = 0;

    if dice >= accident_value {
        return ("失敗（アクシデント）".to_owned(), success_value);
    }

    if dice as f64 > roll_target {
        return ("失敗".to_owned(), success_value);
    }

    // Ruby: dig1 = dice - ((dice / 10).to_i * 10)
    let dig1 = dice - (dice / 10) * 10;

    let is_critical = if is_muse { dig1 <= 1 } else { dig1 == 1 };

    let mut result_text = "成功".to_owned();
    if is_critical {
        result_text += "（クリティカル）";
    }

    let success_value = dice.saturating_mul(advanced_roll);

    (result_text, success_value)
}

/// Ruby `#getFormulaText(target, modify, advancedRoll, luckPoint)`。
fn get_formula_text(
    target: i64,
    modify: f64,
    advanced_roll: i64,
    luck_point: i64,
) -> Result<String, EvalError> {
    let target_text = target.to_string();
    let mut formula_text = target_text.clone();
    if modify > 1.0 {
        formula_text += &format!("*{}", float_to_i_string(modify)?);
    }
    if modify < 1.0 {
        formula_text += &format!("/{}", float_to_i_string(1.0 / modify)?);
    }
    if advanced_roll > 1 {
        formula_text += &format!("/{advanced_roll}");
    }
    if luck_point > 0 {
        formula_text += &format!("*{}", pow2_string(luck_point));
    }

    if formula_text == target_text {
        return Ok(String::new());
    }

    Ok(format!("[{formula_text}]"))
}

/// Ruby `#get_hit_table(hitPart, roc)`。
fn get_hit_table(
    hit_part: &str,
    roc: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(table) = find_table(HIT_TABLES, hit_part) else {
        return Ok(None);
    };
    table_result(table, roc, 10, rng).map(Some)
}

/// Ruby `#get_SUV_table(armorGrade, damage)`。
fn get_suv_table(armor_grade: &str, damage: i64) -> Option<String> {
    let name = "戦闘結果表";

    // Ruby: ('A'..'Z').to_a.index(armorGrade)
    let armor_index = armor_grade
        .chars()
        .next()
        .filter(|c| c.is_ascii_uppercase())
        .map(|c| (c as usize) - ('A' as usize))?;
    let damage_info = SUV_TABLE.get(armor_index)?;

    let wound_ranks = [
        "無傷",
        "LW(軽傷)",
        "MW(中傷)",
        "HW(重傷)",
        "MO(致命傷)",
        "KL(死亡)",
    ];

    let mut wound_text = "";

    for (rate, rank) in damage_info.iter().zip(wound_ranks.iter()) {
        if *rate > damage {
            break;
        }
        wound_text = rank;
    }

    Some(format!("{name}({armor_grade})：{damage} ＞ {wound_text}"))
}

/// Ruby `#get_damageEffect_table(hitPart, damageStage)`。
fn get_damage_effect_table(hit_part: &str, damage_stage: &str) -> Option<String> {
    let damage_infos = [("L", "(LW)"), ("M", "(MW)"), ("H", "(HW)"), ("O", "(MO)")];

    let index = damage_infos
        .iter()
        .position(|(stage, _)| *stage == damage_stage)?;

    let damage_index = index as i64 + 1;
    let damage_text = damage_infos[index].1;

    let table = find_table(DAMAGE_EFFECT_TABLES, hit_part)?;

    let text = get_table_by_number(damage_index, table);
    Some(format!("{}{} ＞ {}", table.name, damage_text, text))
}

/// Ruby `#get_accident_table(damageType, roc)`。
fn get_accident_table(
    damage_type: &str,
    roc: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(table) = find_table(ACCIDENT_TABLES, damage_type) else {
        return Ok(None);
    };
    table_result(table, roc, 10, rng).map(Some)
}

/// Ruby `#get_mechanicAccident_table(locationType, roc, correction)`。
fn get_mechanic_accident_table(
    location_type: &str,
    roc: i64,
    correction: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(table) = find_table(MECHANIC_ACCIDENT_TABLES, location_type) else {
        return Ok(None);
    };

    let mut dice = get_roc_dice(roc, 10, rng)?;
    let mut dice_text = dice.to_string();

    let original_dice = dice;
    dice = dice.saturating_add(correction);
    if dice > 10 {
        dice = 10;
    }
    if correction > 0 {
        dice_text = format!("{dice}[{original_dice}+{correction}]");
    }

    let table_text = get_table_by_number(dice, table);
    Ok(Some(format!(
        "{}({}) ＞ {}",
        table.name, dice_text, table_text
    )))
}

/// Ruby `#get_randomEncounter_table(locationType, roc)`。
fn get_random_encounter_table(
    location_type: &str,
    roc: i64,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(table) = find_table(RANDOM_ENCOUNTER_TABLES, location_type) else {
        return Ok(None);
    };
    table_result(table, roc, 100, rng).map(Some)
}

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

/// Ruby `get_hit_table` の命中部位表（キー → 表）。
static HIT_TABLES: &[(&str, NumberedTable)] = &[
    (
        "HU",
        NumberedTable {
            name: "命中部位表：人間",
            items: &[
                (1, "胴部（クリティカル）"),
                (2, "頭部"),
                (3, "左腕部"),
                (4, "右腕部"),
                (5, "胴部"),
                (6, "胴部"),
                (7, "胴部"),
                (8, "胴部"),
                (9, "脚部"),
                (10, "脚部"),
            ],
        },
    ),
    (
        "BK",
        NumberedTable {
            name: "命中部位表：バイク",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "パワープラント"),
                (6, "ドライブ"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "乗員"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "WA",
        NumberedTable {
            name: "命中部位表：ワゴン",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "パワープラント"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "SC",
        NumberedTable {
            name: "命中部位表：シェルキャリア",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "パワープラント"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "BG",
        NumberedTable {
            name: "命中部位表：バギー",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "IN",
        NumberedTable {
            name: "命中部位表：インセクター",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "ドライブ"),
                (9, "ドライブ"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "PT",
        NumberedTable {
            name: "命中部位表：ポケットタンク",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "パワープラント"),
                (8, "ドライブ"),
                (9, "ドライブ"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "HT",
        NumberedTable {
            name: "命中部位表：ホバータンク",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "パワープラント"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "TA",
        NumberedTable {
            name: "命中部位表：戦車",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "AC",
        NumberedTable {
            name: "命中部位表：装甲車",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "HE",
        NumberedTable {
            name: "命中部位表：ヘリ",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "パワープラント"),
                (6, "ドライブ"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "TR",
        NumberedTable {
            name: "命中部位表：トレーラー",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "パワープラント"),
                (6, "ドライブ"),
                (7, "兵装・カーゴ"),
                (8, "兵装・カーゴ"),
                (9, "兵装・カーゴ"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "VT",
        NumberedTable {
            name: "命中部位表：VTOL",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "BO",
        NumberedTable {
            name: "命中部位表：ボート",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "本体"),
                (8, "パワープラント"),
                (9, "ドライブ"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "CS",
        NumberedTable {
            name: "命中部位表：通常・格闘型コンバットシェル",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "ザック"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "TH",
        NumberedTable {
            name: "命中部位表：可変・重コンバットシェル",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "本体"),
                (7, "ドライブ"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "兵装・貨物"),
            ],
        },
    ),
    (
        "AM",
        NumberedTable {
            name: "命中部位表：オートモビル",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "GD",
        NumberedTable {
            name: "命中部位表：ガンドック",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "ドライブ"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "HC",
        NumberedTable {
            name: "命中部位表：ホバークラフト",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "パワープラント"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "乗員"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "BI",
        NumberedTable {
            name: "命中部位表：自転車",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "BT",
        NumberedTable {
            name: "命中部位表：バトルトレーラー",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
    (
        "AI",
        NumberedTable {
            name: "命中部位表：エアクラフト",
            items: &[
                (1, "本体（クリティカル）"),
                (2, "本体"),
                (3, "本体"),
                (4, "本体"),
                (5, "本体"),
                (6, "パワープラント"),
                (7, "ドライブ"),
                (8, "兵装・貨物"),
                (9, "兵装・貨物"),
                (10, "乗員"),
            ],
        },
    ),
];

/// Ruby `get_damageEffect_table` の損傷効果表（命中部位 → 表）。
static DAMAGE_EFFECT_TABLES: &[(&str, NumberedTable)] = &[
    (
        "H",
        NumberedTable {
            name: "対人損傷効果表：頭部",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。【PER】のAR、【PER】がベースアビリティのスキルのSRにSR1/2のロール修正。"),
                (3, "ダメージ修正+20。【PER】のAR、【PER】がベースアビリティのスキルのSRにSR1/4のロール修正。"),
                (4, "ダメージ修正+30。［死亡］。頭部がサイバーの場合は［戦闘不能］。"),
            ],
        },
    ),
    (
        "T",
        NumberedTable {
            name: "対人損傷効果表：胴部",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。【DEX】のAR、【DEX】がベースアビリティのスキルのSRにSR1/2のロール修正。"),
                (3, "ダメージ修正+20。【DEX】のAR、【DEX】がベースアビリティのスキルのSRにSR1/4のロール修正。"),
                (4, "ダメージ修正+30。［戦闘不能］。"),
            ],
        },
    ),
    (
        "A",
        NumberedTable {
            name: "対人損傷効果表：腕部",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。損傷した腕を使用する、また両腕を使用する行動にSR1/2のロール修正。"),
                (3, "ダメージ修正+20。損傷した腕を使用する、また両腕を使用する行動にSR1/4のロール修正。"),
                (4, "ダメージ修正+30。損傷した腕を使用する、また両腕を使用する行動不可。"),
            ],
        },
    ),
    (
        "L",
        NumberedTable {
            name: "対人損傷効果表：脚部",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。【REF】のAR、【REF】がベースアビリティのスキルのSRにSR1/2のロール修正。"),
                (3, "ダメージ修正+20。【REF】のAR、【REF】がベースアビリティのスキルのSRにSR1/4のロール修正。【MV】が1/2。"),
                (4, "ダメージ修正+30。［戦闘不能］。"),
            ],
        },
    ),
    (
        "M",
        NumberedTable {
            name: "心理損傷効果表",
            items: &[
                (1, "ダメージ修正+10。焦り。効果は特になし。シーン終了で自然回復。"),
                (2, "ダメージ修正+20。混乱。1シーン、すべてのロールがSR1/2となる。シーン終了で自然回復。"),
                (3, "ダメージ修正+30。恐怖。1シーン、すべてのロールがSR1/4となる。シーン終了で自然回復。"),
                (4, "ダメージ修正+50。喪失。［戦闘不能］。シーン終了で自然回復。"),
            ],
        },
    ),
    (
        "E",
        NumberedTable {
            name: "電子損傷効果表",
            items: &[
                (1, "ダメージ修正+10。処理落ち。効果は特になし。"),
                (2, "ダメージ修正+20。ノイズ。1シーン、キャラクターならすべてのロールが、アイテムならそれを使用したロールが1/2となる。"),
                (3, "ダメージ修正+30。恐怖。1シーン、キャラクターならすべてのロールが、アイテムならそれを使用したロールが1/4となる。"),
                (4, "ダメージ修正+50。クラッシュ。キャラクターなら［戦闘不能］。アイテムなら1シナリオ中、使用不可。"),
            ],
        },
    ),
    (
        "B",
        NumberedTable {
            name: "メカニック損傷効果表：本体",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正シフト1。修理費がフレーム価格の1/4かかる。"),
                (3, "ダメージ修正シフト2。修理費がフレーム価格の1/2かかる。"),
                (4, "ダメージ修正シフト3。移動不能。修理費がフレーム価格と同じだけかかる。走行中なら事故表を振ること。"),
            ],
        },
    ),
    (
        "P",
        NumberedTable {
            name: "メカニック損傷効果表：パワープラント",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。メカニックの【MV】が1/2になる。修理費がパワープラント価格の1/4かかる。"),
                (3, "ダメージ修正+20。メカニックの【MV】が1/4になる。修理費がパワープラント価格の1/2かかる。"),
                (4, "ダメージ修正+30。移動不能。修理費がパワープラント価格と同じだけかかる。走行中なら事故表を振ること。"),
            ],
        },
    ),
    (
        "D",
        NumberedTable {
            name: "メカニック損傷効果表：ドライブ",
            items: &[
                (1, "ダメージ修正+10。"),
                (2, "ダメージ修正+10。メカニックの【REF】が1/2になる。［メカニック］スキルにSR1/2の修正。修理費がドライブ価格の1/4かかる。"),
                (3, "ダメージ修正+20。メカニックの【REF】が1/2になる。［メカニック］スキルにSR1/4の修正。修理費がドライブ価格の1/2かかる。"),
                (4, "ダメージ修正+30。移動不能。修理費がドライブ価格と同じだけかかる。走行中なら事故表を振ること。"),
            ],
        },
    ),
];

/// Ruby `get_critical_table` のクリティカル表（1D10）。
static CRITICAL_TABLE: NumberedTable = NumberedTable {
    name: "クリティカル表",
    items: &[
        (1, "特に追加被害は発生しない。"),
        (2, "対象はバランスを崩す。クリンナッププロセスまで、対象は命中ロールにSR1/2のロール修正を受ける。"),
        (3, "対象に隙を作る。クリンナッププロセスまで、対象はリアクションにSR1/2のロール修正を受ける。"),
        (4, "激しい一撃。最終火力に+20してダメージを算出すること。"),
        (5, "多大なダメージ。最終火力に+20してダメージを算出すること。"),
        (6, "弱点に直撃。対象の装甲値を無視してダメージを算出すること。"),
        (7, "効果的な一撃。対象の受ける損傷段階をシフト1する。"),
        (8, "致命的な一撃。対象の受ける損傷段階をシフト2する。"),
        (9, "中枢に直撃。対象の【SUV】を3ランク低いものとしてダメージを算出する。"),
        (10, "中枢を破壊。対象の装甲値を無視し、【SUV】を3ランク低いものとしてダメージを算出する。"),
    ],
};

/// Ruby `get_accident_table` のアクシデント表（種別 → 表）。
static ACCIDENT_TABLES: &[(&str, NumberedTable)] = &[
    (
        "G",
        NumberedTable {
            name: "格闘アクシデント表",
            items: &[
                (1, "体勢を崩す。その攻撃は失敗する。"),
                (2, "体勢を崩す。その攻撃は失敗する。"),
                (3, "体勢を崩す。その攻撃は失敗する。"),
                (4, "転倒。格闘回避と機動回避にSR1/4、【MV】が半分に。"),
                (5, "転倒。格闘回避と機動回避にSR1/4、【MV】が半分に。"),
                (6, "転倒。格闘回避と機動回避にSR1/4、【MV】が半分に。"),
                (7, "武器が足下（0m離れたところ）に落ちる。素手のときは何もなし。"),
                (8, "武器が足下（0m離れたところ）に落ちる。素手のときは何もなし。"),
                (9, "武器が5m離れたところに落ちる。素手のときは関係ない。"),
                (10, "使用武器が壊れ、1シーン使用不可。"),
            ],
        },
    ),
    (
        "S",
        NumberedTable {
            name: "射撃／投擲アクシデント表",
            items: &[
                (1, "ささいなミス。その攻撃は失敗する。"),
                (2, "ささいなミス。その攻撃は失敗する。"),
                (3, "ささいなミス。その攻撃は失敗する。"),
                (4, "射撃武器はジャム。投擲武器ならば武器が取り出せないなど、マイナーアクションを消費しなければその武器を使用できない。"),
                (5, "射撃武器はジャム。投擲武器ならば武器が取り出せないなど、マイナーアクションを消費しなければその武器を使用できない。"),
                (6, "射撃武器はジャム。投擲武器ならば武器が取り出せないなど、マイナーアクションを消費しなければその武器を使用できない。"),
                (7, "故障。メジャーアクションで【DEX】のSR1のロールに成功しなければ、その武器を使用できない。"),
                (8, "故障。メジャーアクションで【DEX】のSR1のロールに成功しなければ、その武器を使用できない。"),
                (9, "破壊。以後、その武器は使用できない。"),
                (10, "武器の暴発。固定火力100のダメージを、装甲値無視で武器を持っていた腕（両手なら両手）、または兵装・貨物に受ける。"),
            ],
        },
    ),
    (
        "M",
        NumberedTable {
            name: "心理攻撃アクシデント表",
            items: &[
                (1, "集中失敗。攻撃は失敗する。"),
                (2, "集中失敗。攻撃は失敗する。"),
                (3, "集中失敗。攻撃は失敗する。"),
                (4, "思考ノイズ。クリンナップまですべてのリアクションにSR1/2。"),
                (5, "思考ノイズ。クリンナップまですべてのリアクションにSR1/2。"),
                (6, "思考ノイズ。クリンナップまですべてのリアクションにSR1/2。"),
                (7, "EXの暴走。頭部に装甲値無視、固定火力60のダメージを受ける。"),
                (8, "EXの暴走。頭部に装甲値無視、固定火力60のダメージを受ける。"),
                (9, "感情暴走。攻撃に使用したマニューバが1シーン使用不可。"),
                (10, "トラウマの再現。装甲値無視、固定火力100の心理ダメージを受ける。"),
            ],
        },
    ),
    (
        "E",
        NumberedTable {
            name: "電子攻撃アクシデント表",
            items: &[
                (1, "ショック。攻撃は失敗する。"),
                (2, "ショック。攻撃は失敗する。"),
                (3, "ショック。攻撃は失敗する。"),
                (4, "ノイズ発生。クリンナップまで電子攻撃のリアクションにSR1/2。"),
                (5, "ノイズ発生。クリンナップまで電子攻撃のリアクションにSR1/2。"),
                (6, "ノイズ発生。クリンナップまで電子攻撃のリアクションにSR1/2。"),
                (7, "ソフトウェア障害。攻撃に使用したソフトが1シーン使用不可。"),
                (8, "ソフトウェア障害。攻撃に使用したソフトが1シーン使用不可。"),
                (9, "ハードウェア障害。装甲値無視、固定火力80の電子ダメージを受ける。"),
                (10, "信号逆流。装甲値無視、固定火力100の心理ダメージを受ける。"),
            ],
        },
    ),
];

/// Ruby `get_mechanicAccident_table` のメカニック事故表（場所 → 表）。
static MECHANIC_ACCIDENT_TABLES: &[(&str, NumberedTable)] = &[
    (
        "A",
        NumberedTable {
            name: "空中メカニック事故表",
            items: &[
                (3, "兵装／貨物。メカニックが装備している一番ENCの大きい武器ひとつが戦闘終了時まで使用不能になる。武器がない場合はメカニックオプションが使用不能になり、それもない場合は一番ENCの重い貨物（乗客をのぞく）が失われる。"),
                (6, "操作不能。メカニック本体にMWダメージ。操縦者は適切な［メカニック］スキルでSR1/4のロールを行い、成功したら体勢を立て直せる。失敗した場合、次のクリンナッププロセスまで、回避をふくめた一切の行動を取ることができない。"),
                (8, "不時着。メカニック本体にHWダメージ。次のクリンナッププロセスまで、回復をふくめた一切の行動を取ることができない。"),
                (9, "墜落。メカニック本体にMOダメージ。すべての乗員は、墜落のショックによってランダムな部位に〈物〉155の固定ダメージを受ける。このダメージは機動回避可能である。"),
                (10, "爆発。メカニックが爆発し、完全に破壊される。すべての乗員は、爆発と落下によって胴体に〈熱〉205の固定ダメージを受ける。このダメージは機動回避可能だが、SRに1/4の修正がある。"),
            ],
        },
    ),
    (
        "S",
        NumberedTable {
            name: "水上／水中メカニック事故表",
            items: &[
                (3, "横揺れ。次のクリンナッププロセスまで、このメカニックに乗っているキャラクターの行うすべての［メカニック］ロールに1/2の修正が与えられる。"),
                (6, "兵装／貨物。メカニックが装備している一番ENCの大きい武器ひとつが戦闘終了時まで使用不能になる。武器がない場合はメカニックオプションが使用不能になり、それもない場合は一番ENCの重い貨物（乗客をのぞく）が失われる。"),
                (8, "横転。メカニック本体にMWダメージ。操縦者は適切な［メカニック］スキルでSR1/4のロールを行い、成功したら体勢を立て直せる。失敗した場合、次のクリンナッププロセスまで、回避をふくめた一切の行動を取ることができない。"),
                (9, "激突。メカニック本体に〈物〉255の固定ダメージ。"),
                (10, "爆発。メカニックが爆発し、完全に破壊される。すべての乗員は、爆発によって胴体に〈熱〉155の固定ダメージを受ける。このダメージは機動回避可能だが、SRに1/4の修正がある。"),
            ],
        },
    ),
    (
        "L",
        NumberedTable {
            name: "地上メカニック事故表",
            items: &[
                (3, "接触。メカニック本体にLWダメージ。"),
                (6, "兵装／貨物。メカニックが装備している一番ENCの大きい武器ひとつが戦闘終了時まで使用不能になる。武器がない場合はメカニックオプションが使用不能になり、それもない場合は一番ENCの重い貨物（乗客をのぞく）が失われる。"),
                (8, "スピン。メカニック本体にMWダメージ。操縦者は適切な［メカニック］スキルでSR1/4のロールを行い、成功したら体勢を立て直せる。失敗した場合、次のクリンナッププロセスまで、回避をふくめた一切の行動を取ることができない。"),
                (9, "激突。メカニック本体に〈物〉255の固定ダメージ。次のクリンナッププロセスまで、回避をふくめた一切の行動を取ることができない。"),
                (10, "爆発。メカニックが爆発し、完全に破壊される。すべての乗員は、爆発によって胴体に〈熱〉155の固定ダメージを受ける。このダメージは機動回避可能だが、SRに1/4の修正がある。"),
            ],
        },
    ),
];

/// Ruby `get_strategyEvent_chart` のストラテジーイベントチャート（1D100）。
static STRATEGY_EVENT_CHART: NumberedTable = NumberedTable {
    name: "ストラテジーイベントチャート",
    items: &[
        (50, "特に何事もなかった。"),
        (53, "スコール。種別：レーザーを装備している部隊の戦力はこのターン半減する。この効果は重複しない。"),
        (55, "ただよう不安。味方ユニットはWILのAR1を行い、失敗すると士気の10%を失う。"),
        (57, "狙撃！　司令官キャラクターは胴体に〈物〉155点の固定ダメージを受ける。機動回避は可能。"),
        (60, "敵の猛烈な反撃！　味方ユニットはREFのAR1を行い、失敗するとこのターン、移動力がマイナス1。"),
        (63, "敵弾幕の隙を見いだす。このターン、味方ユニットは突破判定がSR2に。"),
        (65, "突破のチャンス。このターン、味方ユニットは移動力が1点上昇する。"),
        (67, "士気高揚。味方ユニットの士気がそれぞれ現在値の10%だけ回復する。"),
        (70, "敵陣崩壊。敵ユニットの中で士気がもっとも低いユニットが戦場から撤退する。複数いた場合、すべて撤退。PC、ゲストには効果なし。"),
        (73, "大声援。戦闘がどこかのハッカーによって衛星中継され、喝采を浴びる。"),
        (75, "雨／雪。種別；レーザーを部隊の戦力はこのターン半減する。この効果は重複しない。"),
        (77, "磁気嵐。このターン、種別：ミサイルは戦力に数えず、突撃に使用することもできない。"),
        (80, "膠着した戦況。このターン、味方ユニットは突破判定がSR1/2に。"),
        (83, "メタルホッパー！　金属イナゴの襲来で視界をふさがれ、このラウンドは全てのMC射程が0となる。"),
        (85, "大竜巻！　飛行しているユニットの移動力は0となり、飛行ユニットはこのターン自分から突撃を行えない。"),
        (87, "通信の混乱。味方ユニットはINTのAR1を行い、失敗するとこのターン、移動力がマイナス1。"),
        (90, "幸運が微笑む。味方ユニットのラックポイントが1点ずつ回復。NPCには無効。"),
        (93, "致命的な狙撃！　司令官キャラクターは胴体に〈物〉205点の固定ダメージを受ける。機動回避は可能。"),
        (95, "敵の罠に落ちた。このターン、敵軍ユニットは移動力が1点上昇する。"),
        (97, "勝利の予感。味方ユニットの士気がそれぞれの現在値の20%だけ回復する。"),
        (99, "天変地異が襲いかかる！　このターン、すべてのユニットは移動できない。"),
        (100, "大混乱。後2回振る。"),
    ],
};

/// Ruby `get_NPCAttack_chart` のNPC攻撃処理チャート（1D10）。
static NPC_ATTACK_CHART: NumberedTable = NumberedTable {
    name: "NPC攻撃処理チャート",
    items: &[
        (5, "戦力の低い側だけが一方的に除去される。"),
        (8, "双方、一番戦力の少ないユニットひとつを除去する。"),
        (10, "戦力の高い側が最大戦力のユニットひとつを除去する。"),
    ],
};

/// Ruby `get_loserDestiny_chart` の敗者運命チャート（1D10）。
static LOSER_DESTINY_CHART: NumberedTable = NumberedTable {
    name: "敗者運命チャート",
    items: &[
        (
            1,
            "奇跡的に無傷で生き延びた。いずれ復讐の機会もあるだろう。",
        ),
        (2, "ランダムな部位にLWを負う。"),
        (3, "戦力決定に使っていた武器が破壊される。"),
        (4, "ランダムな部位にMWを負う。"),
        (5, "外見に影響するような傷を負う。治療するなら$3000。"),
        (6, "ランダムな部位にHWを負う。"),
        (7, "着用している防具すべてが破壊される。衣服は壊れない。"),
        (8, "ランダムな部位にMOを負う。"),
        (9, "ランダムに決定した能力値ひとつを、永久に1点失う。"),
        (10, "残念ながら、君は死んでしまった。"),
    ],
};

/// Ruby `get_randomEncounter_table` のランダムエンカウント表（場所 → 表）。
static RANDOM_ENCOUNTER_TABLES: &[(&str, NumberedTable)] = &[(
    "W",
    NumberedTable {
        name: "荒野ランダムエンカウント表",
        items: &[
            (80, "特に遭遇は発生しなかった"),
            (85, "1d10名のバンデッド"),
            (
                87,
                "ヴェーダ・バウンサー1名に率いられた1d10+2（最低1）のヴェーダ・ソルジャー",
            ),
            (89, "1d10+2体のウェーブコヨーテ"),
            (91, "1d10÷2体（最低1）のレーザーアント"),
            (93, "1d10-5体（最低1）のライトニングホーク"),
            (96, "1d10体のメタルホッパー"),
            (98, "1体のブラスビースト"),
            (100, "1d10÷3体（最低1）のサンドワーム"),
        ],
    },
)];

/// Ruby `get_SUV_table` の戦闘結果表。添字は装甲グレード（A=0 .. Z=25）、
/// 各行は `woundRanks` と同じ順（無傷 / LW / MW / HW / MO / KL）の閾値。
static SUV_TABLE: [[i64; 6]; 26] = [
    [0, 1, 6, 16, 26, 36],
    [0, 1, 6, 26, 36, 46],
    [0, 1, 16, 26, 46, 56],
    [1, 6, 26, 36, 56, 76],
    [1, 16, 36, 46, 66, 76],
    [1, 26, 36, 56, 76, 86],
    [1, 36, 56, 66, 76, 96],
    [1, 56, 76, 86, 96, 106],
    [1, 66, 86, 96, 106, 116],
    [1, 66, 86, 96, 116, 136],
    [1, 76, 96, 106, 126, 156],
    [1, 76, 96, 116, 146, 166],
    [1, 86, 106, 126, 166, 176],
    [1, 106, 126, 136, 176, 196],
    [1, 106, 126, 146, 186, 206],
    [1, 116, 136, 156, 196, 206],
    [1, 126, 146, 166, 206, 226],
    [1, 126, 146, 176, 226, 246],
    [1, 136, 156, 186, 246, 266],
    [1, 156, 176, 206, 246, 286],
    [1, 156, 176, 206, 266, 306],
    [1, 166, 186, 206, 286, 346],
    [1, 176, 196, 246, 326, 366],
    [1, 196, 226, 266, 346, 386],
    [1, 206, 226, 286, 366, 406],
    [1, 226, 246, 306, 386, 406],
];

#[cfg(test)]
mod tests {

    /// `Float#to_i` の再現。i64 に収まらない値は Ruby の多倍長と同じ十進表記になること。
    #[test]
    fn float_to_i_string_matches_ruby() {
        use super::{float_to_i_string, pow2_string, shifted_decimal};
        use crate::eval::EvalError;

        assert_eq!(float_to_i_string(12.5).unwrap(), "12");
        assert_eq!(float_to_i_string(0.25).unwrap(), "0");
        assert_eq!(float_to_i_string(-1.5).unwrap(), "-1");
        // Ruby: (20.0 * 2**64).to_i => 368934881474191032320
        assert_eq!(
            float_to_i_string(20.0 * 2f64.powi(64)).unwrap(),
            "368934881474191032320"
        );
        // Ruby: 1e22.to_i => 10000000000000000000000
        assert_eq!(float_to_i_string(1e22).unwrap(), "10000000000000000000000");
        assert_eq!(
            float_to_i_string(f64::INFINITY),
            Err(EvalError::FloatDomain)
        );
        assert_eq!(float_to_i_string(f64::NAN), Err(EvalError::FloatDomain));

        assert_eq!(shifted_decimal(0, 5), "0");
        assert_eq!(pow2_string(0), "1");
        assert_eq!(pow2_string(10), "1024");
        // Ruby: 2**64 => 18446744073709551616
        assert_eq!(pow2_string(64), "18446744073709551616");
    }

    /// `test/data/MetalHeadExtream.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MetalHeadExtream",
            "MetalHeadExtream.toml",
            106,
        );
    }
}
