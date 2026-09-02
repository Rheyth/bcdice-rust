//! P4で手書き移植した `lib/bcdice/game_system/FullFace.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `FullFace#resolute_action`（戦闘判定 `x+bFF<=a[,t][&d]`）
//! - `TABLES`（ジャンク表 `JKT`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::FullFace`（ID: `FullFace`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullFace;

impl GameSystem for FullFace {
    fn id(&self) -> &'static str {
        "FullFace"
    }

    fn name(&self) -> &'static str {
        "フルフェイス"
    }

    fn sort_key(&self) -> &'static str {
        "ふるふえいす"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　x+bFF<=a[,t][&d]   x:ヒート(省略時は3) b:判定修正 a:能力値 t:難易度(省略可) d:基本ダメージ(省略可)

例)FF<=2:     能力値2で判定し、その結果(成功数,1の目の数,6の目の数,バースト)を表示。
   6FF<=3:    ヒート6,能力値3で戦闘判定し、その結果( 〃 )を表示。
   8+2FF<=3:  ヒート8,判定修正+2,能力値3で戦闘判定し、その結果( 〃 )を表示。
   FF<=2,1:   能力値2,難易度1で判定し、その結果(成功数,1の目の数,6の目の数,成功・失敗,バースト)を表示。
   6FF<=3,2&1:ヒート6,能力値3,難易度2,基本ダメージ1で戦闘判定し、その結果(成功数,1の目の数,6の目の数,ダメージ,バースト)を表示。

■ジャンク表　JKT

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"([+\d]+)*FF", "JKT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `FullFace#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = resolute_action(self, command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }

        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
    }
}

/// Ruby `resolute_action` のコマンド抽出。
///
/// Ruby: `/^(\d*)([+\d]+)*FF<=(\d)(,(\d))?(&(\d))?$/`
/// 繰り返しつきグループ `([+\d]+)*` は Ruby でも `regex` クレートでも
/// 「最後の繰り返し」を捕獲する。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d*)([+\d]+)*FF<=(\d)(,(\d))?(&(\d))?$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当（先頭の10進数字列を読む。空なら0）。
fn to_i(text: &str) -> i64 {
    if text.is_empty() {
        0
    } else {
        // 桁あふれする入力は Ruby だと Bignum になり、`roll_barabara` の
        // 個数上限で TooManyRandsError になる。i64 に収まらない場合も同じ経路へ落とす。
        text.parse().unwrap_or(i64::MAX)
    }
}

/// Ruby `FullFace#resolute_action`（戦闘判定）。
fn resolute_action(
    system: &FullFace,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    // Ruby: return nil unless m
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let mut heat_level = to_i(&m[1]);
    if heat_level == 0 {
        heat_level = 3;
    }
    // Ruby: Arithmetic.eval("0#{m[2]}", @round_type)
    // 式として壊れている場合（`8+FF<=3` など）Ruby は nil を返し、直後の
    // `modify > 0` が NoMethodError でクラッシュする。ここでは 0 として扱う。
    let modify_expr = format!("0{}", m.get(2).map_or("", |x| x.as_str()));
    let modify = arithmetic::eval(&modify_expr, system.round_type())?.unwrap_or(I::ZERO);
    let status_no = to_i(&m[3]);
    let target_no = m.get(5).map_or(0, |x| to_i(x.as_str()));
    let damage_no = m.get(7).map_or(0, |x| to_i(x.as_str()));

    let mut dice_array: Vec<String> = Vec::new();

    let dice = roll_sorted(rng, heat_level)?;
    let mut ones = count_of(&dice, 1);
    let sixs = count_of(&dice, 6);
    let mut success_num = dice.iter().filter(|val| **val <= status_no).count() as i64;
    dice_array.push(join_dice(&dice));

    if modify > I::ZERO {
        let dice = roll_sorted(rng, sat_i64(&modify))?;
        ones += count_of(&dice, 1);
        success_num += dice.iter().filter(|val| **val <= status_no).count() as i64;
        dice_array.push(join_dice(&dice));
    }
    let mut ones_total = ones;

    while ones > 0 {
        let dice = roll_sorted(rng, ones)?;
        ones = count_of(&dice, 1);
        ones_total += ones;
        success_num += dice.iter().filter(|val| **val <= status_no).count() as i64;
        dice_array.push(join_dice(&dice));
    }

    // Ruby: Result.new.tap do |result| ... end（フラグの代入順をそのまま再現する）
    let mut result = EvalResult::new();
    let mut command_out = format!("({heat_level}{}FF<={status_no}", modifier(&modify));
    if sixs >= 2 {
        result.fumble = true;
        result.set_condition(false);
    } else {
        result.set_condition(success_num > 0);
        result.critical = ones_total > 0;
    }

    let mut result_txt: Vec<String> = Vec::new();
    result_txt.push(format!("成功度({success_num})"));
    if ones_total > 0 {
        result_txt.push(format!("1の目({ones_total})"));
    }
    if sixs > 0 {
        result_txt.push(format!("6の目({sixs})"));
    }
    if target_no > 0 {
        command_out += &format!(",{target_no}");
        if success_num >= target_no {
            result_txt.push("成功".to_owned());
            result.set_condition(true);
        } else {
            result_txt.push("失敗".to_owned());
            result.set_condition(false);
        }
    }
    if damage_no > 0 {
        command_out += &format!("&{damage_no}");
        let damage = damage_no + ones_total;
        result_txt.push(format!("ダメージ({damage})"));
    }
    if result.fumble {
        result_txt.push("バースト".to_owned());
    }
    command_out += ")";

    let sequence = [command_out, dice_array.join("+"), result_txt.join(",")];
    result.text = sequence.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `@randomizer.roll_barabara(times, 6).sort`。
fn roll_sorted(rng: &mut Randomizer, times: i64) -> Result<Vec<i64>, EvalError> {
    let mut dice = rng.roll_barabara(times, 6)?;
    dice.sort_unstable();
    Ok(dice)
}

/// Ruby `Array#count(value)`。
fn count_of(dice: &[i64], value: i64) -> i64 {
    dice.iter().filter(|d| **d == value).count() as i64
}

/// Ruby `dice.join(",")`。
fn join_dice(dice: &[i64]) -> String {
    dice.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "JKT" {
        return Ok(None);
    }
    Ok(Some(JKT.roll(rng)?.to_string()))
}

/// Ruby `TABLES["JKT"]`（ジャンク表）の項目。
static JKT_ITEMS: &[&str] = &[
    "命欲しさに重要な情報を吐いた。セッションのボスに関する情報を得る。",
    "ユニットの機密文書だ。この戦闘で獲得したユニットがあるなら、そのうち好きなユニット1つを経験点を消費せずに常備化できる。",
    "違法アップロードされた個人情報のデータだ。このセッション中、エネミーと出会ったとき、詳細なデータが即座に公開される（GMはできるだけ拒否しないこと）。",
    "鍵を持っていた。近くに施錠された扉や箱などがあるなら、そのうちの1つを開けることができる。",
    "何も見つけられなかったが、敵を倒したことによって自信を得た。このセッション中のみ、1回だけ自身の判定で出た6の目を1つ消すことができる。",
    "自爆装置だ。何も残らなかった。",
    "使い捨ての武器を手に入れた。このセッション中のみ、1回だけ「近接攻撃5『本能』」の攻撃アクションを行える。",
    "敵が改心した。倒されたキャラクターがまだ生きているなら、そのうちの一人が君たちに協力を申し込んでくる（このセッション限定の恩恵「人脈：協力者」を得る）。",
    "金を手に入れた。キャラクター全員、アフターフェイズに配布される経験点が1増加する。",
    "強力なユニットを隠し持っていた。好きな組織専用ユニットを1つ獲得する。",
    "大金を手に入れた。キャラクター全員、アフターフェイズに配布される経験点が3増加する。",
];

/// Ruby `DiceTable::Table.new("ジャンク表", "2D6", …)`。
static JKT: Table = Table::from_dice("ジャンク表", 2, 6, JKT_ITEMS);

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("FullFace", "FullFace.toml", 14);
    }
}
