use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GundamSentinel;

impl GameSystem for GundamSentinel {
    fn id(&self) -> &'static str {
        "GundamSentinel"
    }
    fn name(&self) -> &'static str {
        "ガンダム・センチネルRPG"
    }
    fn sort_key(&self) -> &'static str {
        "かんたむせんちねる"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }
    crate::impl_prefixes_pattern!();
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

static HELP_MESSAGE: &str = r"・基本戦闘(BB, BBM)
　BB[+修正][>回避値]で基本戦闘を判定します。回避値を指定すると、命中・回避も表示します。
　BBM[+修正][>回避値]でモブ用の基本戦闘を判定します。クリティカルを判定します。回避値を指定すると、命中・回避も表示します。

　例）BB BBM BB+5>14 BBM+5>15

・一般技能(GS)
　GS[+修正][>目標値]で一般技能を判定します。目標値を指定しない場合は、目標値10で判定します。

　例）GS GS+5 GS+5>10


・各種表
　敵MSクリティカルヒットチャート　(ECHC)
　PC用脱出判定チャート　　　　　　(PEJC[+m] m:修正)
　艦船追加ダメージ決定チャート　　(ASDC)
　対空砲結果チャート　　　　　　　(AARC[+m]=t m:修正, t:対空防御力)
　リハビリ判定チャート　　　　　　(RTJC[+m] m:修正)
　二次被害判定チャート　　　　　　(SDDC)
";
static PREFIXES: &[&str] = &["BBM?", "GS", "AARC", "PEJC", "RTJC", "ECHC", "ASDC", "SDDC"];

fn bb_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^BB(M)?([-+][-+\d]+)?(?:>([-+\d]+))?").expect("valid regex"))
}
fn gs_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^GS([-+][-+\d]+)?(?:>([-+\d]+))?").expect("valid regex"))
}
fn aarc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^AARC([+-]\d+)?=(\d+)$").expect("valid regex"))
}
fn chart_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(PEJC|RTJC)([+-]\d+)?$").expect("valid regex"))
}

fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = basic_battle(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = general_skill(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = anti_aircraft(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(m) = chart_pattern().captures(command) {
        let modify = eval_optional(m.get(2))?;
        let dice = rng.roll_sum(2, 6)?;
        let total = (dice + modify).clamp(2, 12);
        let shown = if modify == 0 {
            total.to_string()
        } else {
            format!("{dice}{}={total}", format_modifier(modify))
        };
        let (title, result) = if &m[1] == "PEJC" {
            ("PC用脱出判定チャート", ESCAPE[(total - 2) as usize])
        } else {
            ("リハビリ判定チャート", REHABILITATION[(total - 2) as usize])
        };
        return Ok(Some(SpecificCommandOutput::text(format!(
            "{title}({shown}) ＞ {result}"
        ))));
    }
    if command == "ECHC" || command == "ASDC" {
        let number = rng.roll_sum(2, 6)?;
        let (title, values) = if command == "ECHC" {
            ("敵MSクリティカルヒットチャート", ECHC)
        } else {
            ("艦船追加ダメージ決定チャート", ASDC)
        };
        return Ok(Some(SpecificCommandOutput::text(format!(
            "{title}({number}) ＞ {}",
            values[(number - 2) as usize]
        ))));
    }
    if command == "SDDC" {
        let tens = rng.roll_once(6)?;
        let ones = rng.roll_once(6)?;
        let number = tens * 10 + ones;
        return Ok(Some(SpecificCommandOutput::text(format!(
            "二次被害判定チャート({number}) ＞ {}",
            SDDC[((tens - 1) * 6 + ones - 1) as usize]
        ))));
    }
    Ok(None)
}

fn basic_battle(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = bb_pattern().captures(command) else {
        return Ok(None);
    };
    let mob = m.get(1).is_some();
    let modify = eval_optional(m.get(2))?;
    let avoid = m
        .get(3)
        .map(|v| arithmetic::eval(v.as_str(), RoundType::Ceil))
        .transpose()?
        .flatten();
    let high = rng.roll_once(6)?;
    let low = rng.roll_once(6)?;
    let original = high * 10 + low;
    let shifted = low + modify - 1;
    let total_high = high + (shifted).div_euclid(6);
    let total_low = shifted.rem_euclid(6) + 1;
    let total = (total_high * 10 + total_low).max(11);
    let mut parts = vec![format!("({command})")];
    if m.get(2).is_some() {
        parts.push(format!("{original}{}", format_modifier(modify)));
    }
    parts.push(total.to_string());
    let mut result = EvalResult::new();
    if let Some(avoid) = avoid {
        if total > crate::randomizer::sat_i64(&avoid) {
            parts.push(format!(
                "命中(+{})",
                count_success(total, crate::randomizer::sat_i64(&avoid))
            ));
            result.success = true;
        } else {
            parts.push("回避".to_string());
            result.failure = true;
        }
    }
    if mob && total >= 66 {
        parts.push("クリティカル".to_string());
        result.critical = true;
    }
    result.text = parts.join(" ＞ ");
    Ok(Some(result))
}

fn count_success(dice: i64, avoid: i64) -> i64 {
    (dice / 10 * 6 + dice % 10) - (avoid / 10 * 6 + avoid % 10)
}

fn general_skill(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = gs_pattern().captures(command) else {
        return Ok(None);
    };
    let modify = eval_optional(m.get(1))?;
    let target = m
        .get(2)
        .map(|v| arithmetic::eval(v.as_str(), RoundType::Ceil))
        .transpose()?
        .flatten()
        .unwrap_or(I::from(10));
    let dice = rng.roll_sum(2, 6)?;
    let total = dice + modify;
    let success = total > crate::randomizer::sat_i64(&target);
    let mut parts = vec![format!("({command})")];
    if m.get(1).is_some() {
        parts.push(format!("{dice}{}", format_modifier(modify)));
    }
    parts.push(total.to_string());
    parts.push(if success { "成功" } else { "失敗" }.to_string());
    Ok(Some(if success {
        EvalResult::success(parts.join(" ＞ "))
    } else {
        EvalResult::failure(parts.join(" ＞ "))
    }))
}

fn anti_aircraft(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = aarc_pattern().captures(command) else {
        return Ok(None);
    };
    let modify = eval_optional(m.get(1))?;
    let target: i64 = m[2].parse().unwrap_or(0).clamp(1, 6);
    let dice = rng.roll_sum(2, 6)?;
    let total = (dice + modify).clamp(1, 13);
    let shown = if modify == 0 {
        total.to_string()
    } else {
        format!("({dice}{}={total})", format_modifier(modify))
    };
    let value = GUN[((target - 1) * 13 + total - 1) as usize];
    let text = format!("対空砲結果チャート({shown}vs{target}) ＞ 結果「{value}」");
    Ok(Some(if value.parse::<i64>().is_ok() {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }))
}

fn eval_optional(value: Option<regex::Match<'_>>) -> Result<i64, EvalError> {
    match value {
        Some(v) => Ok(arithmetic::eval(v.as_str(), RoundType::Ceil)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)),
        None => Ok(0),
    }
}
fn format_modifier(value: i64) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

static GUN: &[&str] = &[
    "D", "H", "H", "H", "10", "8", "6", "5", "4", "2", "1", "-", "-", "D", "H", "H", "H", "12",
    "10", "9", "8", "6", "5", "3", "2", "-", "D", "D", "H", "H", "H", "12", "10", "9", "7", "6",
    "4", "3", "1", "D", "D", "H", "H", "H", "14", "13", "12", "10", "8", "6", "5", "3", "D", "D",
    "D", "H", "H", "H", "14", "13", "11", "9", "7", "6", "4", "D", "D", "D", "H", "H", "H", "H",
    "16", "14", "12", "11", "8", "6",
];
static ESCAPE: &[&str] = &[
    "無傷で脱出",
    "無傷で脱出",
    "無傷で脱出",
    "軽傷で脱出「１Ｄ６ダメージ。」",
    "中傷で脱出「２Ｄ６ダメージ。」",
    "重傷で脱出「３Ｄ６ダメージ。」",
    "重体で脱出「１Ｄ３の耐久力が残る。」",
    "戦死「二階級特進。」",
    "戦死「二階級特進。」",
    "戦死「二階級特進。」",
    "戦死「二階級特進。」",
];
static REHABILITATION: &[&str] = &[
    "なし",
    "１ヶ月",
    "２ヶ月",
    "３ヶ月",
    "４ヶ月",
    "５ヶ月",
    "６ヶ月",
    "１０ヶ月",
    "１年",
    "１年６ヶ月",
    "１年と、もう一度このチャートで振った結果分を足した期間",
];
static ECHC: &[&str] = &["コックピット直撃：目標ＭＳは残骸となる。","腕破損：同時に携帯武器も失う。携帯武装の交換も行えない。直ちにモラル判定を－４で行う。","射撃武装破損：目標ＭＳはその時点で使用しているナンバーの若い武装を１つ失う。全ての武装を失った場合、モラル判定を行う。","頭部直撃：目標ＭＳはメインカメラを失い、以後射撃、格闘の命中判定に－６の修正を受ける。頭部に装備されている武装も失われる。","パイロット気絶：目標ＭＳは回復するまで行動不能。","目標ＭＳへのダメージ２倍。","目標ＭＳへのダメージ２倍。","目標ＭＳへのダメージ３倍。","脚破損：目標ＭＳは、以後の回避値に－６の修正を受ける。","コントロール不能：目標ＭＳは１Ｄ６ラウンドの間、行動不能。","熱核ジェネレーター直撃：目標ＭＳは直ちに爆発（耐久力０）する。"];
static ASDC: &[&str] = &[
    "ブリッジ損傷「複数ある艦は、総てのブリッジが損傷すると以後の対空防御は修正を＋５する。」",
    "カタパルト損傷「複数ある艦は、総てのカタパルトが損傷すると、ＭＳの発着艦ができなくなる。」",
    "追加ダメージ「追加２Ｄ６×２ダメージ。」",
    "主砲大破「主砲１門を失う。」",
    "副砲大破「副砲１門を失う。」",
    "追加ダメージ「追加２Ｄ６ダメージ。」",
    "追加ダメージ「追加２Ｄ６ダメージ。」",
    "追加ダメージ「追加２Ｄ６ダメージ。」",
    "１ターン行動不能「１ターンはその艦は何も行動ができない。」",
    "航行不能「その艦はそのヘックスから動けなくなる。」",
    "エンジン誘爆「１Ｄ６×１０％の耐久力を失う。」",
];
static SDDC: &[&str] = &[
    "奇蹟的に無傷「不発！？今回のダメージは0。」",
    "メインカメラ破損「以後、射撃、格闘の命中判定に－３の修正を受ける。」",
    "コクピット破損「以後の追加ダメージ判定に－１の修正を受ける。」",
    "右腕損傷「携帯していた武装も失う。また右腕での武器の使用はできなくなる。」",
    "左腕損傷「携帯していた武装も失う。また左腕での武器の使用はできなくなる。」",
    "気絶「気絶判定の余地無く、必ず気絶する。」",
    "気絶「気絶判定を－６の修正で行う。」",
    "気絶「気絶判定を－４の修正で行う。」",
    "気絶「気絶判定を－２の修正で行う。」",
    "気絶「気絶判定を行う。」",
    "予備弾倉破損「携帯している予備弾倉かＥパックを１つ失う。」",
    "サブカメラ破損「以後、射撃、格闘の命中判定に－１の修正を受ける。」",
    "固定武装破損「固定されている武装を１つ失う。」",
    "予備武装破損「携帯している以外の武装を１つ失う。」",
    "頭部破損「メインカメラも失い、以後、射撃、格闘の命中判定に－３の修正を受ける。」",
    "右脚破損「以後、回避値が１Ｄ３低下する。」",
    "左脚破損「以後、回避値が１Ｄ３低下する。」",
    "操縦機構破損「以後、すべての行動は消費行動ポイントを１ポイント余分に消費する。」",
    "軽傷「パイロットは１Ｄ６のダメージを受ける。また気絶判定を行う。」",
    "中傷「パイロットは２Ｄ６のダメージを受ける。また気絶判定を－６修正で行う。」",
    "重傷「パイロットは３Ｄ６のダメージを受ける。また気絶判定を－９修正で行う。」",
    "操縦伝達部破損「以後すべての射撃、格闘の命中判定と回避値に－１の修正を受ける。」",
    "センサー破損「イニシアティブ決定に－１の修正を受ける。」",
    "脱出機構破損「脱出判定に＋３の修正を受ける。」",
    "熱核ジェネレーター損傷「行動の「追加移動」が行えなくなる。」",
    "右腕の携帯武装破損「右腕に持っていた武装を１つ失う。」",
    "左腕の携帯武装破損「左腕に持っていた武装を１つ失う。」",
    "サブスラスター破損「回避値が１低下する。」",
    "プロペラントタンク破損「プロペラントタンクを１つ失う。」",
    "バックパック破損「推進剤３Ｄ６ポイント失う。」",
    "メインスラスター破損「回避値が１Ｄ６低下する。」",
    "動力パイプ破損「以後、行動ポイント決定のダイスに－１の修正を受ける。」",
    "動力伝達機構破損「以後、行動ポイント決定のサイコロに－１Ｄ３の修正を受ける。」",
    "サブスラスター破損「旋回が１２０度までしかできなくなる。」",
    "メインスラスター破損「旋回が６０度までしかできなくなる。」",
    "熱核ジェネレーター直撃「そのＭＳは爆発する。ＰＣは直ちに脱出判定を行う。」",
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "GundamSentinel",
            "GundamSentinel.toml",
            34,
        );
    }
}
