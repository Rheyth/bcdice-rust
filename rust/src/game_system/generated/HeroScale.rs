//! P4で手書き移植した `lib/bcdice/game_system/HeroScale.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `HeroScale#eval_game_system_specific_command`（`#select_origin`）
//! - 超越 `5HS4`（`#origin_great` ＋激情/科学/肉体）
//! - 加護 `4HS6`（`#origin_protection` ＋逆転/安寧/選択）
//! - 契約 `3HS8`（`#origin_vow` ＋奉納/燃焼/収奪/享受）
//! - 呪い `2HS20`（`#origin_curse` ＋破滅/崩壊/歪曲）
//! - 異物 `3HS10`（`#origin_stranger` ＋模造/混血/彼方・`#stranger_effection`）
//! - 報い `1HS60`（`#origin_karma` ＋堕落/忘却/封印）
//! - 同化 `12HS2`（`#origin_absorption` ＋怪物/秘宝/概念）
//! - 存在ロール `1HS12` / `2HS12` / `3HS12`（＋萌芽/変遷/偶然、大神〜元素）
//! - 汎用 `#results_multiplication` / `#result_raoundup` ＋ `*HS*` 乗算ロール

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::HeroScale`（ID: `HeroScale`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroScale;

impl GameSystem for HeroScale {
    fn id(&self) -> &'static str {
        "HeroScale"
    }

    fn name(&self) -> &'static str {
        "英雄の尺度"
    }

    fn sort_key(&self) -> &'static str {
        "えいゆうのしやくと"
    }

    fn help_message(&self) -> &'static str {
        r"同人TRPGシステム『英雄の尺度』用ダイスボット。
基本ルールブック＋サプリメント対応。仮称は非対応。
コマンド一覧は以下の通り。*添え字で内容は[]。†がついていたら添え字必須。
5hs4 超越
5hs4,b 肉体の超越
5hs4,s,* 科学の超越[†達成値への加算値]
5hs4,p 激情の超越
4hs6 加護
4hs6,s 選択の加護
4hs6,p 安寧の加護
4hs6,r 逆転の加護
3hs8,*,* 契約[奉納の出目1,奉納の出目2]
3hs8,a,*,* 享受の契約[†受諾出目1][†受諾出目2]
3hs8,e,* 収奪の契約[†取得出目]
3hs8,b 燃焼の契約
3hs8,o,*,* 奉納の契約[奉納の出目1,奉納の出目2]
2hs20 呪い
2hs20,r 歪曲の呪い
2hs20,c 崩壊の呪い
2hs20,d 破滅の呪い
3hs10 異物
3hs10,i 模造の異物
3hs10,m,* 混血の異物[追加振り基準出目（初期値10）]
3hs10,b,* 彼方の異物[追加振り停止基準値（初期値666）]
1hs60 報い
1hs60,d 堕落の報い
1hs60,o 忘却の報い
1hs60,s,* 封印の報い[出目への係数]
12hs2 同化
12hs2,m,*,*,*,*,*,*,*,* 怪物の同化[*d2,*d4,*d6,*d8,*d10,*d12,*d20,*d60]
12hs2,t,* [†2の枚数宣言]
12hs2,c,* 法則の同化[†1の枚数宣言]
1hs12 下位存在
2hs12 中位存在
2hs12,t 変遷の中位存在
2hs12,c 偶然の中位存在
2hs12,g,* 萌芽の上位存在[加算値]
3hs12 上位存在
3hs12,g 大神の上位存在
3hs12,h 神性の上位存在
3hs12,w 魔性の上位存在
3hs12,m 悪意の上位存在
3hs12,s,* 大罪の上位存在[†確定する目標値]
3hs12,d 破壊の上位存在
3hs12,a 懊悩の上位存在
3hs12,o 試練の上位存在
3hs12,c 創造の上位存在
3hs12,e 元素の上位存在
*hs* 乗算ロール
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+HS\d+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `HeroScale#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        select_origin(command, rng).map(|opt| opt.map(SpecificCommandOutput::Text))
    }
}

// ---------------------------------------------------------------------------
// 分岐本体。Ruby `#select_origin`
// ---------------------------------------------------------------------------

/// Ruby `#select_origin`。`Option<String>`（nil は未解釈）。
fn select_origin(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let order: Vec<&str> = command.split(',').collect();

    match order[0] {
        "5HS4" => origin_great(&order, rng).map(Some),
        "4HS6" => origin_protection(&order, rng).map(Some),
        "3HS8" => origin_vow(&order, rng).map(Some),
        "2HS20" => origin_curse(&order, rng).map(Some),
        "3HS10" => origin_stranger(&order, rng).map(Some),
        "1HS60" => origin_karma(&order, rng).map(Some),
        "12HS2" => origin_absorption(&order, rng).map(Some),
        "1HS12" => origin_normal(rng).map(Some),
        "2HS12" => origin_unique(&order, rng).map(Some),
        "3HS12" => origin_omnipotent(&order, rng).map(Some),
        _ => {
            // Ruby: dice = order[0].rpartition("HS")（右端の "HS" で分割）
            let Some((lhs, rhs)) = order[0].rsplit_once("HS") else {
                return Ok(None);
            };
            if is_digits(lhs) && is_digits(rhs) {
                let times: i64 = lhs.parse().unwrap_or(0);
                let sides: i64 = rhs.parse().unwrap_or(0);
                let natural_result = rng.roll_barabara(times, sides)?;
                let total = results_multiplication(&natural_result);
                let message = format!(
                    "{} ＞ {}[{}]",
                    order[0],
                    total,
                    dice_text::join_dice(&natural_result)
                );
                Ok(Some(message))
            } else {
                Ok(None)
            }
        }
    }
}

/// Ruby `/^\d+$/` 相当。
fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Ruby `#results_multiplication`。全要素の積（Ruby は Bignum だが Rust では飽和乗算）。
fn results_multiplication(result_list: &[i64]) -> i64 {
    result_list
        .iter()
        .copied()
        .fold(1i64, |acc, r| acc.saturating_mul(r))
}

/// Ruby `#result_raoundup`（原文ママ・タイポ）。2で切り上げ。
fn result_roundup(result: i64) -> i64 {
    if result % 2 == 0 {
        result / 2
    } else {
        result / 2 + 1
    }
}

// ---------------------------------------------------------------------------
// 超越 5HS4
// ---------------------------------------------------------------------------

/// Ruby `#origin_great`。
fn origin_great(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(5, 4)?;
    let message = match order.get(1).copied() {
        Some("P") => fate_passion(&natural_result),
        Some("S") => fate_science(&natural_result, order),
        Some("B") => fate_body(&natural_result, rng)?,
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "超越 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_passion`。
fn fate_passion(natural_result: &[i64]) -> String {
    let number_of_1 = natural_result.iter().filter(|&&r| r == 1).count() as i64;
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| r.saturating_add(number_of_1))
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "激情の超越 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_science`。
fn fate_science(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    if order.len() > 2 && is_digits(order[2]) {
        let science: i64 = order[2].parse().unwrap_or(0);
        if science < 1024 {
            let total = subtotal.saturating_add(science);
            let mut message = format!(
                "科学の超越 ＞ {}[{}] ＞ {}",
                subtotal,
                dice_text::join_dice(natural_result),
                total
            );
            if total > 1023 {
                message += "(科学臨界)";
            }
            message
        } else {
            "エラー：科学力が1024を超えています。".to_string()
        }
    } else {
        "エラー：科学力を設定してください。".to_string()
    }
}

/// Ruby `#fate_body`。
///
/// Ruby の `each` は `modified_result` に後から追加した要素（4の再振り）も
/// 反復するため、Rust でもインデックスループで同じ挙動を再現する
/// （出目4は最大2連鎖: 元の5個+追加2個、追加された4はさらに振られない）。
fn fate_body(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    let mut i = 0;
    while i < modified_result.len() {
        if modified_result[i] == 4 {
            modified_result.push(rng.roll_once(4)?);
        }
        i += 1;
    }
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    Ok(format!(
        "肉体の超越 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

// ---------------------------------------------------------------------------
// 加護 4HS6
// ---------------------------------------------------------------------------

/// Ruby `#origin_protection`。
fn origin_protection(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(4, 6)?;
    let message = match order.get(1).copied() {
        Some("R") => fate_reversal(&natural_result),
        Some("P") => fate_peace(&natural_result),
        Some("S") => fate_choice(&natural_result, rng)?,
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "加護 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_reversal`。
fn fate_reversal(natural_result: &[i64]) -> String {
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| if r < 4 { 7 - r } else { r })
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "逆転の加護 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_peace`。
fn fate_peace(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    let total = subtotal.saturating_add(250);
    format!(
        "安寧の加護 ＞ {}[{}] ＞ {}",
        subtotal,
        dice_text::join_dice(natural_result),
        total
    )
}

/// Ruby `#fate_choice`。
fn fate_choice(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    modified_result.extend(rng.roll_barabara(3, 6)?);
    let mut even_total: i64 = 1;
    let mut odd_total: i64 = 1;
    for &result in &modified_result {
        if result % 2 == 0 {
            even_total = even_total.saturating_mul(result);
        } else {
            odd_total = odd_total.saturating_mul(result);
        }
    }
    let total = even_total.max(odd_total);
    let subtotal = results_multiplication(natural_result);
    Ok(format!(
        "選択の加護 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

// ---------------------------------------------------------------------------
// 契約 3HS8
// ---------------------------------------------------------------------------

/// Ruby `#origin_vow`。
fn origin_vow(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(3, 8)?;
    let message = match order.get(1).copied() {
        Some("O") => fate_offering(&natural_result, order),
        Some("B") => fate_burning(&natural_result),
        Some("E") => fate_exploitation(&natural_result, order),
        Some("A") => fate_acceptance(&natural_result, order),
        _ => {
            let subtotal = results_multiplication(&natural_result);
            if order.len() > 2 && is_digits(order[1]) && is_digits(order[2]) {
                let mut modified_result = natural_result.to_vec();
                modified_result.push(order[1].parse::<i64>().unwrap_or(0));
                modified_result.push(order[2].parse::<i64>().unwrap_or(0));
                let total = results_multiplication(&modified_result);
                format!(
                    "契約 ＞ {}[{}] ＞ {}[{}]",
                    subtotal,
                    dice_text::join_dice(&natural_result),
                    total,
                    dice_text::join_dice(&modified_result)
                )
            } else {
                format!(
                    "契約 ＞ {}[{}]",
                    subtotal,
                    dice_text::join_dice(&natural_result)
                )
            }
        }
    };
    Ok(message)
}

/// Ruby `#fate_offering`。
fn fate_offering(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    let mut message = if order.len() > 3 && is_digits(order[2]) && is_digits(order[3]) {
        let mut modified_result = natural_result.to_vec();
        modified_result.push(order[2].parse::<i64>().unwrap_or(0));
        modified_result.push(order[3].parse::<i64>().unwrap_or(0));
        let total = results_multiplication(&modified_result);
        format!(
            "奉納の契約 ＞ {}[{}] ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    } else {
        format!(
            "奉納の契約 ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result)
        )
    };

    // Ruby: offering_result.sort!.reverse!.shift(1)（最大値1個を除去した残りを降順表示）
    let mut offering_result = natural_result.to_vec();
    offering_result.sort_unstable_by(|a, b| b.cmp(a));
    offering_result.remove(0);
    message += &format!("(奉納：{})", dice_text::join_dice(&offering_result));
    message
}

/// Ruby `#fate_burning`。
fn fate_burning(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    let total = subtotal.saturating_mul(6);
    format!(
        "燃焼の契約 ＞ {}[{}] ＞ {}",
        subtotal,
        dice_text::join_dice(natural_result),
        total
    )
}

/// Ruby `#fate_exploitation`。
fn fate_exploitation(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    if order.len() > 2 && is_digits(order[2]) {
        let mut modified_result = natural_result.to_vec();
        let min = *modified_result.iter().min().unwrap_or(&1);
        if let Some(pos) = modified_result.iter().position(|&r| r == min) {
            modified_result[pos] = order[2].parse::<i64>().unwrap_or(0);
        }
        let total = results_multiplication(&modified_result);
        format!(
            "収奪の契約 ＞ {}[{}] ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    } else {
        "エラー：収奪数を指定してください。".to_string()
    }
}

/// Ruby `#fate_acceptance`。
fn fate_acceptance(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    if order.len() > 3 && is_digits(order[2]) && is_digits(order[3]) {
        // Ruby: change_result = natural_result.min(2)
        let mut smallest2: Vec<i64> = natural_result.to_vec();
        smallest2.sort_unstable();
        smallest2.truncate(2);
        let mut modified_result = natural_result.to_vec();
        if let Some(pos) = modified_result.iter().position(|&r| r == smallest2[0]) {
            modified_result[pos] = order[2].parse::<i64>().unwrap_or(0);
        }
        if let Some(pos) = modified_result.iter().position(|&r| r == smallest2[1]) {
            modified_result[pos] = order[3].parse::<i64>().unwrap_or(0);
        }
        let total = results_multiplication(&modified_result);
        format!(
            "享受の契約 ＞ {}[{}] ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    } else {
        "エラー：享受数を指定してください。".to_string()
    }
}

// ---------------------------------------------------------------------------
// 呪い 2HS20
// ---------------------------------------------------------------------------

/// Ruby `#origin_curse`。
fn origin_curse(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(2, 20)?;
    let message = match order.get(1).copied() {
        Some("R") => fate_ruin(&natural_result, rng)?,
        Some("C") => fate_collapse(&natural_result),
        Some("D") => fate_distortion(&natural_result),
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "呪い ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_ruin`（破滅の呪い）。
fn fate_ruin(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    modified_result.extend(rng.roll_barabara(2, 20)?);
    let mut total: i64 = 1;
    for &result in &modified_result {
        if result > 10 {
            total = total.saturating_mul(result);
        }
    }
    let subtotal = results_multiplication(natural_result);
    Ok(format!(
        "破滅の呪い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

/// Ruby `#fate_collapse`（崩壊の呪い）。
fn fate_collapse(natural_result: &[i64]) -> String {
    let mut modified_result = natural_result.to_vec();
    let max = natural_result.iter().max().copied().unwrap_or(1);
    let collapse_result = result_roundup(max);
    if modified_result[0] == modified_result[1] {
        modified_result[0] = collapse_result;
        modified_result[1] = collapse_result;
        modified_result.push(collapse_result);
        modified_result.push(collapse_result);
    } else {
        // Ruby: index(max) は最初に見つかった位置
        let pos = natural_result.iter().position(|&r| r == max).unwrap_or(0);
        modified_result[pos] = collapse_result;
        modified_result.insert(pos, collapse_result);
    }
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "崩壊の呪い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_distortion`（歪曲の呪い）。
fn fate_distortion(natural_result: &[i64]) -> String {
    let mut modified_result = natural_result.to_vec();
    if modified_result[0] == modified_result[1] {
        modified_result[0] = modified_result[0].saturating_add(13);
        modified_result[1] = modified_result[1].saturating_add(13);
    } else {
        let min = natural_result.iter().min().copied().unwrap_or(1);
        let pos = natural_result.iter().position(|&r| r == min).unwrap_or(0);
        modified_result[pos] = modified_result[pos].saturating_add(13);
    }
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "歪曲の呪い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

// ---------------------------------------------------------------------------
// 異物 3HS10
// ---------------------------------------------------------------------------

/// Ruby `#origin_stranger`。
fn origin_stranger(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(3, 10)?;
    // Ruby: すべての出目から1を引く
    let natural_result: Vec<i64> = natural_result.iter().map(|&r| r - 1).collect();
    let message = match order.get(1).copied() {
        Some("I") => fate_imitation(&natural_result),
        Some("M") => fate_mixed(&natural_result, order, rng)?,
        Some("B") => fate_beyond(&natural_result, order, rng)?,
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "異物 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_imitation`（模造の異物）。
fn fate_imitation(natural_result: &[i64]) -> String {
    let mut modified_result = natural_result.to_vec();
    modified_result.sort_unstable();
    let v = modified_result[0].saturating_add(modified_result[1].saturating_mul(10));
    modified_result[0] = if v == 0 { 100 } else { v };
    modified_result[1] = modified_result[2];
    modified_result.remove(2);
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "模造の異物 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_mixed`（混血の異物）。
fn fate_mixed(
    natural_result: &[i64],
    order: &[&str],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let subtotal = results_multiplication(natural_result);
    let mixed_score: i64 = if order.len() > 2 && is_digits(order[2]) {
        order[2].parse().unwrap_or(1)
    } else {
        1
    };
    let mut modified_result = natural_result.to_vec();
    let message = if mixed_score <= *natural_result.iter().min().unwrap_or(&1) {
        modified_result.push(rng.roll_once(12)?);
        let total = results_multiplication(&modified_result);
        format!(
            "混血の異物 ＞ {}[{}] ＞ {}[{}](追加振り)",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    } else {
        let min = natural_result.iter().min().copied().unwrap_or(1);
        let pos = natural_result.iter().position(|&r| r == min).unwrap_or(0);
        modified_result[pos] = 10;
        let total = results_multiplication(&modified_result);
        format!(
            "混血の異物 ＞ {}[{}] ＞ {}[{}](10置換)",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    };
    Ok(message)
}

/// Ruby `#fate_beyond`（彼方の異物）。
fn fate_beyond(
    natural_result: &[i64],
    order: &[&str],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    let subtotal = results_multiplication(natural_result);
    let mut total = subtotal;
    let mut beyond_limit: i64 = 666;
    if order.len() > 2 && is_digits(order[2]) {
        let v: i64 = order[2].parse().unwrap_or(666);
        if v < 666 {
            beyond_limit = v;
        }
    }
    // Ruby: while total != 0 && total <= beyond_limit
    while total != 0 && total <= beyond_limit {
        modified_result.push(rng.roll_d9()?);
        total = results_multiplication(&modified_result);
    }
    Ok(format!(
        "彼方の異物 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

// ---------------------------------------------------------------------------
// 報い 1HS60
// ---------------------------------------------------------------------------

/// Ruby `#origin_karma`。
fn origin_karma(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(1, 60)?;
    let message = match order.get(1).copied() {
        Some("D") => fate_depravity(&natural_result),
        Some("O") => fate_oblivion(&natural_result, rng)?,
        Some("S") => fate_sealed(&natural_result, order),
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "報い ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_depravity`（堕落の報い）。
fn fate_depravity(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    let mut modified_result = natural_result.to_vec();
    let v = natural_result[0];
    let depravity_num1 = v % 10;
    let depravity_num10 = v / 10;
    if depravity_num10 > 1 {
        modified_result.push(depravity_num10);
    }
    if depravity_num1 > 1 {
        modified_result.push(depravity_num1);
    }
    let total = results_multiplication(&modified_result);
    format!(
        "堕落の報い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_oblivion`（忘却の報い）。
fn fate_oblivion(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    modified_result.push(rng.roll_once(60)?);
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result) / 2;
    Ok(format!(
        "忘却の報い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

/// Ruby `#fate_sealed`（封印の報い）。
fn fate_sealed(natural_result: &[i64], order: &[&str]) -> String {
    let mut modified_result = natural_result.to_vec();
    let subtotal = results_multiplication(natural_result);
    let mut sealed_break: i64 = 1;
    if order.len() > 2 && is_digits(order[2]) {
        sealed_break = order[2].parse().unwrap_or(1);
    }
    modified_result[0] = modified_result[0].saturating_mul(sealed_break);
    let total = subtotal.saturating_mul(sealed_break);
    let mut message = format!(
        "封印の報い ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    );
    if total <= 30 {
        sealed_break = sealed_break.saturating_mul(4);
        message += &format!("(封印解除成功：{})", sealed_break);
    } else if total <= 60 {
        sealed_break = sealed_break.saturating_mul(2);
        message += &format!("(封印解除成功：{})", sealed_break);
    } else {
        message += &format!("(封印解除失敗：{})", sealed_break);
    }
    message
}

// ---------------------------------------------------------------------------
// 同化 12HS2
// ---------------------------------------------------------------------------

/// Ruby `#origin_absorption`。
fn origin_absorption(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(12, 2)?;
    let message = match order.get(1).copied() {
        Some("M") => fate_monster(order, rng)?,
        Some("T") => fate_treasure(&natural_result, order),
        Some("C") => fate_concept(&natural_result, order),
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "同化 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_monster`（怪物の同化）。
fn fate_monster(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result: Vec<i64> = Vec::new();
    if order.len() > 9
        && [2, 3, 4, 5, 6, 7, 8, 9]
            .iter()
            .all(|&i| is_digits(order[i]))
    {
        let d2: i64 = order[2].parse().unwrap_or(0);
        if d2 > 0 {
            modified_result.extend(rng.roll_barabara(d2, 2)?);
        }
        let d4: i64 = order[3].parse().unwrap_or(0);
        if d4 > 0 {
            modified_result.extend(rng.roll_barabara(d4, 4)?);
        }
        let d6: i64 = order[4].parse().unwrap_or(0);
        if d6 > 0 {
            modified_result.extend(rng.roll_barabara(d6, 6)?);
        }
        let d8: i64 = order[5].parse().unwrap_or(0);
        if d8 > 0 {
            modified_result.extend(rng.roll_barabara(d8, 8)?);
        }
        let count_of_10: i64 = order[6].parse().unwrap_or(0);
        for _ in 0..count_of_10 {
            modified_result.push(rng.roll_d9()?);
        }
        let d12: i64 = order[7].parse().unwrap_or(0);
        if d12 > 0 {
            modified_result.extend(rng.roll_barabara(d12, 12)?);
        }
        let d20: i64 = order[8].parse().unwrap_or(0);
        if d20 > 0 {
            modified_result.extend(rng.roll_barabara(d20, 20)?);
        }
        let d60: i64 = order[9].parse().unwrap_or(0);
        if d60 > 0 {
            modified_result.extend(rng.roll_barabara(d60, 60)?);
        }
        let total = results_multiplication(&modified_result);
        let subtotal: i64 = modified_result.iter().sum();
        let mut message = format!(
            "怪物の同化 ＞ {}[{}] 浸蝕値：{}",
            total,
            dice_text::join_dice(&modified_result),
            subtotal
        );
        if [2, 4, 6, 8, 10, 12, 20, 60].contains(&subtotal) {
            message += "(変異進行)";
        }
        if modified_result.contains(&1) {
            message += "(人間性喪失)";
        }
        Ok(message)
    } else {
        Ok("エラー：変異状態を指定してください。".to_string())
    }
}

/// Ruby `#fate_treasure`（秘宝の同化）。
fn fate_treasure(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    let mut total = subtotal;
    if order.len() > 2 && is_digits(order[2]) {
        let treasure_point: i64 = order[2].parse().unwrap_or(0);
        let count_of_2 = natural_result.iter().filter(|&&r| r == 2).count() as i64;
        if count_of_2 >= treasure_point {
            total = total.saturating_mul(treasure_point);
            format!(
                "秘宝の同化 ＞ {}[{}] ＞ {}(同調成功)",
                subtotal,
                dice_text::join_dice(natural_result),
                total
            )
        } else {
            format!(
                "秘宝の同化 ＞ {}[{}] ＞ {}(同調失敗)",
                subtotal,
                dice_text::join_dice(natural_result),
                total
            )
        }
    } else {
        "エラー：解放率を指定してください。".to_string()
    }
}

/// Ruby `#fate_concept`（概念の同化）。
fn fate_concept(natural_result: &[i64], order: &[&str]) -> String {
    let mut modified_result = natural_result.to_vec();
    let subtotal = results_multiplication(natural_result);
    if order.len() > 2 && is_digits(order[2]) {
        let existence_scale: i64 = order[2].parse().unwrap_or(0);
        let n = (existence_scale.clamp(0, 12)) as usize;
        for item in modified_result.iter_mut().take(n) {
            *item = 2;
        }
        let total = results_multiplication(&modified_result);
        format!(
            "概念の同化 ＞ {}[{}] ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result),
            total,
            dice_text::join_dice(&modified_result)
        )
    } else {
        "エラー：事象強度を指定してください。".to_string()
    }
}

// ---------------------------------------------------------------------------
// 下位・中位・上位存在
// ---------------------------------------------------------------------------

/// Ruby `#origin_normal`（下位存在）。
fn origin_normal(rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(1, 12)?;
    let total = results_multiplication(&natural_result);
    Ok(format!(
        "下位存在 ＞ {}[{}]",
        total,
        dice_text::join_dice(&natural_result)
    ))
}

/// Ruby `#origin_unique`（中位存在）。
fn origin_unique(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(2, 12)?;
    let message = match order.get(1).copied() {
        Some("G") => fate_growth(&natural_result, order),
        Some("T") => fate_transition(&natural_result, rng)?,
        Some("C") => fate_chance(&natural_result),
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "中位存在 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };
    Ok(message)
}

/// Ruby `#fate_growth`（萌芽の中位存在）。
fn fate_growth(natural_result: &[i64], order: &[&str]) -> String {
    let subtotal = results_multiplication(natural_result);
    let mut total = subtotal;
    let mut message;
    if order.len() > 2 && is_digits(order[2]) {
        total = total.saturating_add(order[2].parse::<i64>().unwrap_or(0));
        message = format!(
            "萌芽の中位存在 ＞ {}[{}] ＞ {}",
            subtotal,
            dice_text::join_dice(natural_result),
            total
        );
    } else {
        message = format!(
            "萌芽の中位存在 ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result)
        );
    }
    // Ruby: order[2].to_i（数字以外は0・nilは0）
    let growth_level = order
        .get(2)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        + 50;
    message += &format!("(成長段階：{})", growth_level);
    message
}

/// Ruby `#fate_transition`（変遷の中位存在）。
fn fate_transition(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    modified_result.push(rng.roll_d9()?);
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    Ok(format!(
        "変遷の中位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

/// Ruby `#fate_chance`（偶然の中位存在）。
fn fate_chance(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    if natural_result[0] == natural_result[1] {
        let total = subtotal.saturating_mul(24);
        format!(
            "偶然の中位存在 ＞ {}[{}] ＞ {}",
            subtotal,
            dice_text::join_dice(natural_result),
            total
        )
    } else {
        format!(
            "偶然の中位存在 ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result)
        )
    }
}

/// Ruby `#origin_omnipotent`（上位存在）。
fn origin_omnipotent(order: &[&str], rng: &mut Randomizer) -> Result<String, EvalError> {
    let natural_result = rng.roll_barabara(3, 12)?;

    let message = match order.get(1).copied() {
        Some("G") => fate_god(&natural_result),
        Some("H") => fate_holy(&natural_result),
        Some("W") => fate_wicked(&natural_result),
        Some("M") => fate_malice(&natural_result),
        Some("S") => fate_sin(&natural_result, order, rng)?,
        Some("D") => fate_destruction(&natural_result, rng)?,
        Some("A") => fate_anguish(&natural_result),
        Some("O") => fate_ordeal(&natural_result),
        Some("C") => fate_creation(&natural_result),
        Some("E") => fate_element(rng)?,
        _ => {
            let total = results_multiplication(&natural_result);
            format!(
                "上位存在 ＞ {}[{}]",
                total,
                dice_text::join_dice(&natural_result)
            )
        }
    };

    Ok(message)
}

/// Ruby `#fate_god`（大神の上位存在）。
fn fate_god(natural_result: &[i64]) -> String {
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| r.saturating_add(3))
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "大神の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_holy`（神性の上位存在）。
fn fate_holy(natural_result: &[i64]) -> String {
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| r.saturating_add(1))
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "神性の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_wicked`（魔性の上位存在）。
fn fate_wicked(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    let total = subtotal.saturating_add(120);
    format!(
        "魔性の上位存在 ＞ {}[{}] ＞ {}",
        subtotal,
        dice_text::join_dice(natural_result),
        total
    )
}

/// Ruby `#fate_malice`（悪意の上位存在）。
fn fate_malice(natural_result: &[i64]) -> String {
    let subtotal = results_multiplication(natural_result);
    let total = subtotal.saturating_mul(2);
    format!(
        "悪意の上位存在 ＞ {}[{}] ＞ {}",
        subtotal,
        dice_text::join_dice(natural_result),
        total
    )
}

/// Ruby `#fate_sin`（大罪の上位存在）。
fn fate_sin(
    natural_result: &[i64],
    order: &[&str],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let subtotal = results_multiplication(natural_result);
    if order.len() > 2 && is_digits(order[2]) {
        let mut message = format!(
            "大罪の上位存在 ＞ {}[{}]",
            subtotal,
            dice_text::join_dice(natural_result)
        );
        let sin_weight: i64 = order[2].parse().unwrap_or(0);
        let mut total = subtotal;
        let mut sin_count = 0;
        while sin_count < 3 && total < sin_weight {
            let modified_result = rng.roll_barabara(3, 12)?;
            total = results_multiplication(&modified_result);
            message += &format!(" ＞ {}[{}]", total, dice_text::join_dice(&modified_result));
            sin_count += 1;
        }
        Ok(message)
    } else {
        Ok("エラー：罪の重さを指定してください。".to_string())
    }
}

/// Ruby `#fate_destruction`（破壊の上位存在）。
fn fate_destruction(natural_result: &[i64], rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut modified_result = natural_result.to_vec();
    modified_result.push(rng.roll_once(12)?);
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result).saturating_sub(300);
    Ok(format!(
        "破壊の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    ))
}

/// Ruby `#fate_anguish`（懊悩の上位存在）。
fn fate_anguish(natural_result: &[i64]) -> String {
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| if r < 7 { 7 } else { r })
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "懊悩の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_ordeal`（試練の上位存在）。
fn fate_ordeal(natural_result: &[i64]) -> String {
    let modified_result: Vec<i64> = natural_result
        .iter()
        .map(|&r| if r < 9 { 9 } else { r })
        .collect();
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "試練の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_creation`（創造の上位存在）。
fn fate_creation(natural_result: &[i64]) -> String {
    let mut modified_result = natural_result.to_vec();
    let mut temporary_result = natural_result.to_vec();
    temporary_result.sort_unstable();
    modified_result.push(temporary_result[1]);
    let subtotal = results_multiplication(natural_result);
    let total = results_multiplication(&modified_result);
    format!(
        "創造の上位存在 ＞ {}[{}] ＞ {}[{}]",
        subtotal,
        dice_text::join_dice(natural_result),
        total,
        dice_text::join_dice(&modified_result)
    )
}

/// Ruby `#fate_element`（元素の上位存在）。
fn fate_element(rng: &mut Randomizer) -> Result<String, EvalError> {
    let modified_result = vec![
        rng.roll_once(4)?,
        rng.roll_once(6)?,
        rng.roll_once(8)?,
        rng.roll_once(12)?,
        rng.roll_once(20)?,
    ];
    let total = results_multiplication(&modified_result);
    Ok(format!(
        "元素の上位存在 ＞ {}[{}]",
        total,
        dice_text::join_dice(&modified_result)
    ))
}

// ---------------------------------------------------------------------------
// テスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::PathBuf;

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    fn toml_path() -> Option<PathBuf> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test/data/HeroScale.toml");
        path.exists().then_some(path)
    }

    /// `test/data/HeroScale.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    ///
    /// ケース41 (`3hs10,b,20`) は彼方の異物の上限 `20 < subtotal 81` のため
    /// 追加の d9 を1回も振らない（Ruby `#fate_beyond` の while 条件）。
    /// TOML 側には直前ケースと同じ系列の d9 出目が余分に記録されており、
    /// Ruby の RandomizerMock は未消費 rand を許容するため、Rust 側も許容する
    /// （Aionia.rs の `SURPLUS_RANDS_ALLOWED` 前例と同じ対処）。
    const SURPLUS_RANDS_ALLOWED: &[(usize, usize)] = &[
        (41, 1), // 3hs10,b,20 — beyond_limit に届かず d9 不振り
    ];

    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/HeroScale.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("HeroScale.toml must parse");
        assert_eq!(
            data.tests.len(),
            92,
            "case count in test/data/HeroScale.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "HeroScale",
                "unexpected game system in HeroScale.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("HeroScale"), &tc.input, &mut src) {
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

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL HeroScale:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} HeroScale cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
