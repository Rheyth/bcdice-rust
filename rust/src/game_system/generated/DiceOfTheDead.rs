//! P4で手書き移植した `lib/bcdice/game_system/DiceOfTheDead.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DiceOfTheDead#eval_game_system_specific_command`（感染度表 `BIOx` / ゾンビ化表 `ZMB+x`）
//! - `checkInfection` / `rollZombie` とそれぞれの表
//!
//! # 表データ
//!
//! 原典はどちらの表もメソッド内のローカル変数として毎回組み立てるが、内容は定数なので
//! `static` に持ち上げた（値は1文字も変えていない）。

use std::sync::OnceLock;

use regex::Regex;

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::DiceOfTheDead`（ID: `DiceOfTheDead`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiceOfTheDead;

impl GameSystem for DiceOfTheDead {
    fn id(&self) -> &'static str {
        "DiceOfTheDead"
    }

    fn name(&self) -> &'static str {
        "ダイス・オブ・ザ・デッド"
    }

    fn sort_key(&self) -> &'static str {
        "たいすおふさてつと"
    }

    fn help_message(&self) -> &'static str {
        r"・ゾンビ化表　ZMB+x
（x=オープン中の感染度マスの数。+xは省略可能、省略時は0）
・感染度表　BIOx
（xは被弾回数。xは省略可能、省略時は1）
（上記二つは最初からシークレットダイスで行われます）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["ZMB", "BIO"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DiceOfTheDead#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `DiceOfTheDead#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `DiceOfTheDead#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: when /^BIO(\d+)?$/
        if let Some(captures) = infection_pattern().captures(command) {
            // Ruby: (Regexp.last_match(1) || 1).to_i
            let roll_times = captures.get(1).map_or(1, |m| to_i(m.as_str()));
            let mut result = EvalResult::with_text(check_infection(roll_times, rng)?);
            result.secret = true;
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        // Ruby: when /^ZMB(\+(\d+))?$/
        if let Some(captures) = zombie_pattern().captures(command) {
            // Ruby: Regexp.last_match(2).to_i（nil.to_i == 0）
            let value = captures.get(2).map_or(0, |m| to_i(m.as_str()));
            let mut result = EvalResult::with_text(roll_zombie(value, rng)?);
            result.secret = true;
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(None)
    }
}

/// Ruby `/^BIO(\d+)?$/`。
fn infection_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^BIO(\d+)?$").expect("valid regex"))
}

/// Ruby `/^ZMB(\+(\d+))?$/`。
fn zombie_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^ZMB(\+(\d+))?$").expect("valid regex"))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない入力は飽和させる。
///
/// 被弾回数は繰り返し回数、感染度は合計値の加算にしか使わない。
/// 桁あふれする入力は Ruby でも実質的に応答不能（巨大ループ）なので、
/// 飽和させても意味のある差は生じない。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `checkInfection` の表（行=左のダイス、列=右のダイス）。
static INFECTION_TABLE: &[&[&str]] = &[
    &[
        "「右下（【足】＋１）」",
        "「右中（【足】＋１）」",
        "「右上（【足】＋１）」",
    ],
    &[
        "「中下（【腕】＋１）」",
        "「真中（【腕】＋１）」",
        "「中上（【腕】＋１）」",
    ],
    &[
        "「左下（【頭】＋１）」",
        "「左中（【頭】＋１）」",
        "「左上（【頭】＋１）」",
    ],
];

/// Ruby `rollZombie` の表（合計値 → 内容）。
static ZOMBIE_TABLE: &[(i64, &str)] = &[
    (5, "５以下：影響なし"),
    (6, "６：任意の部位を１点回復"),
    (7, "７：〈アイテム〉武器を１つその場に落とす"),
    (8, "８：〈アイテム〉便利道具１つをその場に落とす"),
    (9, "９：〈アイテム〉消耗品１つをその場に落とす"),
    (10, "１０：腕の傷が広がる。「部位：【腕】」１点ダメージ"),
    (11, "１１：足の傷が広がる。「部位：【足】」１点ダメージ"),
    (12, "１２：頭の傷が広がる。「部位：【頭】」１点ダメージ"),
    (13, "１３：【ゾンビ化表】が新たに適用されるまで「【感染度】＋１マス」の効果を受ける"),
    (14, "１４：即座に自分以外の味方１人のスロット内の〈アイテム〉１つをランダムに捨てさせる"),
    (15, "１５：味方１人に素手で攻撃を行う"),
    (16, "１６：即座に感染度が１上昇する"),
    (17, "１７：次のターンのみ、すべての【能力値】を２倍にする"),
    (
        18,
        "１８以上：自分以外の味方１人にできる限り全力で攻撃を行う。〈アイテム〉も可能な限り使用する",
    ),
];

/// Ruby `DiceOfTheDead#checkInfection`（感染度表）。
fn check_infection(roll_times: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut result = String::from("感染度表");

    for _ in 0..roll_times {
        let d1 = rng.roll_once(6)?;
        let d2 = rng.roll_once(6)?;

        result.push_str(&format!("　＞　出目：{d1}、{d2}　"));

        let index1 = infection_index(d1);
        let index2 = infection_index(d2);

        // Ruby: table[index1][index2]（範囲外は nil ＝ 補間すると空文字列）
        result.push_str(
            INFECTION_TABLE
                .get(index1)
                .and_then(|row| row.get(index2))
                .copied()
                .unwrap_or(""),
        );
    }

    Ok(result)
}

/// Ruby `(d / 2.0).ceil - 1`。
///
/// `roll_once(6)` の戻り値 1〜6 では `(d - 1) / 2`（0, 0, 1, 1, 2, 2）と一致する。
/// 添字なので `usize` に落とし、負値（面数が不正で0が返る場合）は範囲外として扱う。
fn infection_index(dice: i64) -> usize {
    usize::try_from((dice - 1).div_euclid(2)).unwrap_or(usize::MAX)
}

/// Ruby `DiceOfTheDead#rollZombie`（ゾンビ化表）。
fn roll_zombie(value: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let d1 = rng.roll_once(6)?;
    let d2 = rng.roll_once(6)?;

    let dice_total = d1.saturating_add(d2).saturating_add(value);

    // Ruby: minDice = table.first.first / maxDice = table.last.first
    let min_dice = ZOMBIE_TABLE.first().map_or(0, |(n, _)| *n);
    let max_dice = ZOMBIE_TABLE.last().map_or(0, |(n, _)| *n);
    let index = dice_total.max(min_dice).min(max_dice);

    // Ruby: _number, text = table.assoc(index)（該当なしなら nil ＝ 空文字列）
    let text = ZOMBIE_TABLE
        .iter()
        .find(|(n, _)| *n == index)
        .map_or("", |(_, t)| *t);

    Ok(format!(
        "ゾンビ化表　＞　出目：{d1}＋{d2}　感染度：{value}　合計値：{dice_total}　＞　{text}"
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DiceOfTheDead",
            "DiceOfTheDead.toml",
            12,
        );
    }
}
