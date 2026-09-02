//! P4で手書き移植した `lib/bcdice/game_system/MarvelHeroicRoleplaying.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MarvelHeroicRoleplaying#resolute_action`（判定 `MHRnDx[+nDx]`）
//! - `#result_prioritize_the_sum`（合計値優先）/ `#result_prioritize_effect_dice`（効果ダイス優先）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `DiceBlock = Struct.new(:counts, :sides)`。
#[derive(Debug, Clone, Copy)]
struct DiceBlock {
    counts: i64,
    sides: i64,
}

/// Ruby `DiceStats = Struct.new(:value, :sides, :used)`。
#[derive(Debug, Clone, Copy)]
struct DiceStats {
    value: i64,
    sides: i64,
    used: bool,
}

/// Ruby `/MHR((\d+D\d+)(\+\d+D\d+)*)/`。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"MHR((\d+D\d+)(\+\d+D\d+)*)").expect("valid regex"))
}

/// Ruby `/(\d+)D(\d+)/`（`split("+")` した各ブロックの解析）。
fn block_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d+)D(\d+)").expect("valid regex"))
}

/// Ruby `MarvelHeroicRoleplaying#resolute_action`。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_str = &m[1];
    let mut dice_block_arr: Vec<DiceBlock> = Vec::new();
    for d in dice_str.split('+') {
        // `m[1]` の形（`\d+D\d+` を `+` で連ねたもの）から必ずマッチする。
        let Some(n) = block_pattern().captures(d) else {
            continue;
        };
        dice_block_arr.push(DiceBlock {
            counts: to_i(&n[1]),
            sides: to_i(&n[2]),
        });
    }

    // Ruby: group_by(&:sides).map { |sides, group| DiceBlock.new(group.sum(&:counts), sides) }
    //         .sort_by { |item| -item.sides }
    // `group_by` はキーの初出順を保つが、直後に面数降順へ並べ替えるので順序は一意。
    let mut grouped: Vec<DiceBlock> = Vec::new();
    for db in &dice_block_arr {
        match grouped.iter_mut().find(|g| g.sides == db.sides) {
            Some(g) => g.counts = g.counts.saturating_add(db.counts),
            None => grouped.push(*db),
        }
    }
    grouped.sort_by_key(|db| std::cmp::Reverse(db.sides));

    let mut output_parts: Vec<String> = Vec::new();
    let mut stats: Vec<DiceStats> = Vec::new();
    for db in &grouped {
        // Ruby: roll_barabara(...).sort.reverse（降順）
        let mut dices = rng.roll_barabara(db.counts, db.sides)?;
        dices.sort_unstable();
        dices.reverse();

        for &value in &dices {
            stats.push(DiceStats {
                value,
                sides: db.sides,
                used: false,
            });
        }

        let dice_text = join_dice(&dices);
        output_parts.push(format!("D{}[{dice_text}]", db.sides));
    }
    // Ruby: 先頭に付いた "," を `delete_prefix(",")` で落とす＝カンマ区切りの連結。
    let mut output = output_parts.join(",");

    // Ruby: sort_by { |item| [-item.value, item.sides] }
    stats.sort_by(|a, b| b.value.cmp(&a.value).then(a.sides.cmp(&b.sides)));

    let chance = stats.iter().filter(|s| s.value == 1).count() as i64;
    for s in stats.iter_mut() {
        if s.value == 1 {
            s.used = true;
        }
    }

    let (add_dice, effect_die) = result_prioritize_the_sum(&stats);
    if add_dice <= 0 {
        return Ok(None);
    }
    let output_prioritize_the_sum = format!("合計値{add_dice},効果ダイスD{effect_die}");

    let (add_dice, effect_die) = result_prioritize_effect_dice(&stats);
    let output_prioritize_effect_dice = format!("合計値{add_dice},効果ダイスD{effect_die}");

    if add_dice == 0 || output_prioritize_the_sum == output_prioritize_effect_dice {
        output.push_str(&format!(" ＞ {output_prioritize_the_sum}"));
    } else {
        output.push_str(&format!(
            " ＞ {output_prioritize_the_sum} or {output_prioritize_effect_dice}"
        ));
    }

    if chance > 0 {
        output.push_str(&format!(" ＞ チャンス{chance}"));
        Ok(Some(EvalResult::failure(output)))
    } else {
        Ok(Some(EvalResult::success(output)))
    }
}

/// Ruby `#result_prioritize_the_sum`（合計値優先）。
///
/// 上位2つは `used`（＝出目1）かどうかを問わず無条件に取る。
/// 出目1しか残っていない場合に効果ダイスがD4へ落ちるのはこのため。
fn result_prioritize_the_sum(dice_stats_arr: &[DiceStats]) -> (i64, i64) {
    let mut arr = dice_stats_arr.to_vec();

    if arr.len() < 2 {
        return (0, 4);
    }
    let add_dice = arr[0].value + arr[1].value;
    arr[0].used = true;
    arr[1].used = true;

    // Ruby: reject(&:used).max_by(&:sides) → 面数だけを読む
    let effect_die = arr
        .iter()
        .filter(|s| !s.used)
        .map(|s| s.sides)
        .max()
        .unwrap_or(4);

    (add_dice, effect_die)
}

/// Ruby `#result_prioritize_effect_dice`（効果ダイス優先）。
fn result_prioritize_effect_dice(dice_stats_arr: &[DiceStats]) -> (i64, i64) {
    let mut arr = dice_stats_arr.to_vec();

    // Ruby: reject(&:used).min_by { |item| [-item.sides, item.value] }
    // Ruby の `min_by` も Rust の `min_by_key` も同値なら最初の要素を返す。
    let picked = arr
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.used)
        .min_by_key(|(_, s)| (-s.sides, s.value))
        .map(|(i, _)| i);

    let mut effect_die = match picked {
        Some(i) => {
            arr[i].used = true;
            arr[i].sides
        }
        None => 4,
    };

    let mut rest: Vec<DiceStats> = arr.iter().copied().filter(|s| !s.used).collect();
    let add_dice = if rest.len() >= 2 {
        rest.sort_by(|a, b| b.value.cmp(&a.value).then(a.sides.cmp(&b.sides)));
        rest[0].value + rest[1].value
    } else {
        effect_die = 4;
        0
    };

    (add_dice, effect_die)
}

/// Ruby `String#to_i`。i64に収まらない値は飽和させる（Rubyでは Bignum）。
fn to_i(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// Ruby `dices.join(",")`。
fn join_dice(dices: &[i64]) -> String {
    dices
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::MarvelHeroicRoleplaying`（ID: `MarvelHeroicRoleplaying`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarvelHeroicRoleplaying;

impl GameSystem for MarvelHeroicRoleplaying {
    fn id(&self) -> &'static str {
        "MarvelHeroicRoleplaying"
    }

    fn name(&self) -> &'static str {
        "MarvelヒロイックRPG"
    }

    fn sort_key(&self) -> &'static str {
        "まあへるひろいつくRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　MHRnDx[+nDx]        n: ダイス数 x:ダイスの面数

例)MHR3D10+2D8+1D6: 10面ダイスを3個・8面ダイスを2個・6面ダイスを1個振って、その結果を表示(合計値,効果ダイス,チャンス)
   合計値を優先した場合と効果ダイスを優先した場合で結果が変わるケースでは双方を表示。

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["MHR"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(resolute_action(command, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MarvelHeroicRoleplaying",
            "MarvelHeroicRoleplaying.toml",
            8,
        );
    }
}
