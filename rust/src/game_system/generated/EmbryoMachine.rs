//! P4で手書き移植した `lib/bcdice/game_system/EmbryoMachine.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `EmbryoMachine#replace_text` / `#checkRoll`（判定ロール `EMt+m@c#f` → `2R10...`）
//! - `EmbryoMachine#result_nd10`（nD10の成功度判定）
//! - 命中部位表 `HLT` / 射撃攻撃ファンブル表 `SFT` / 白兵攻撃ファンブル表 `MFT`

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `get_table_by_number(num, table)`。`number >= index` を満たす最初の項目。
///
/// 見つからなければ Ruby の既定値 `"1"`。
fn get_table_by_number(index: i64, table: &[(i64, &'static str)]) -> &'static str {
    for (number, value) in table {
        if *number >= index {
            return value;
        }
    }
    "1"
}

/// Ruby `get_hit_location_table` の表（2D10）。
static HIT_LOCATION_TABLE: &[(i64, &str)] = &[
    (4, "頭"),
    (7, "左脚"),
    (9, "左腕"),
    (12, "胴"),
    (14, "右腕"),
    (17, "右脚"),
    (20, "頭"),
];

/// Ruby `get_hit_level_table` の表（大きい方のダイス目）。
static HIT_LEVEL_TABLE: &[(i64, &str)] =
    &[(6, "命中レベルC"), (9, "命中レベルB"), (10, "命中レベルA")];

/// Ruby `get_shoot_fumble_table`（射撃攻撃ファンブル表）の項目。添字は `num - 2`。
static SHOOT_FUMBLE_TABLE: &[&str] = &[
    "暴発した。使用した射撃武器が搭載されている部位に命中レベルAで命中する。",
    "あまりに無様な誤射をした。パイロットの精神的負傷が2段階上昇する。",
    "誤射をした。自機に最も近い味方機体に命中レベルAで命中する。",
    "誤射をした。対象に最も近い味方機体に命中レベルAで命中する。",
    "武装が暴発した。使用した射撃武器が破損する。ダメージは発生しない。",
    "転倒した。次のセグメントのアクションが待機に変更される。",
    "弾詰まりを起こした。使用した射撃武器は戦闘終了まで使用できなくなる。",
    "砲身が大きく歪んだ。使用した射撃武器による射撃攻撃の命中値が戦闘終了まで-3される。",
    "熱量が激しく増大した。使用した射撃武器の消費弾薬が戦闘終了まで+3される。",
    "暴発した。使用した射撃武器が搭載されている部位に命中レベルBで命中する。",
    "弾薬が劣化した。使用した射撃武器の全てのダメージが戦闘終了まで-2される。",
    "無様な誤射をした。パイロットの精神的負傷が1段階上昇する。",
    "誤射をした。対象に最も近い味方機体に命中レベルBで命中する。",
    "誤射をした。自機に最も近い味方機体に命中レベルBで命中する。",
    "砲身が歪んだ。使用した射撃武器による射撃攻撃の命中値が戦闘終了まで-2される。",
    "熱量が増大した。使用した射撃武器の消費弾薬が戦闘終了まで+2される。",
    "砲身がわずかに歪んだ。使用した射撃武器による射撃攻撃の命中値が戦闘終了まで-1される。",
    "熱量がやや増大した。使用した射撃武器の消費弾薬が戦闘終了まで+1される。",
    "何も起きなかった。",
];

/// Ruby `get_melee_fumble_table`（白兵攻撃ファンブル表）の項目。添字は `num - 2`。
static MELEE_FUMBLE_TABLE: &[&str] = &[
    "大振りしすぎた。使用した白兵武器が搭載されている部位の反対の部位(右腕に搭載されているなら左側)に命中レベルAで命中する。",
    "激しく頭を打った。パイロットの肉体的負傷が2段階上昇する。",
    "過負荷で部位が爆発した。使用した白兵武器が搭載されている部位が全壊する。ダメージは発生せず、搭載されている武装も破損しない。",
    "大振りしすぎた。使用した白兵武器が搭載されている部位の反対の部位(右腕に搭載されているなら左側)に命中レベルBで命中する。",
    "武装が爆発した。使用した白兵武器が破損する。ダメージは発生しない。",
    "部分的に機能停止した。使用した白兵武器は戦闘終了まで使用できなくなる。",
    "転倒した。次のセグメントのアクションが待機に変更される。",
    "激しい刃こぼれを起こした。使用した白兵武器の全てのダメージが戦闘終了まで-3される。",
    "地面の凹凸にはまった。次の2セグメントは移動を行うことができない。",
    "刃こぼれを起こした。使用した白兵武器の全てのダメージが戦闘終了まで-2される。",
    "大振りしすぎた。使用した白兵武器が搭載されている部位の反対の部位(右腕に搭載されているなら左側)に命中レベルCで命中する。",
    "頭を打った。パイロットの肉体的負傷が1段階上昇する。",
    "駆動系が損傷した。移動力が戦闘終了まで-2される(最低1)。",
    "間合いを取り損ねた。隣接している機体(複数の場合は1機をランダムに決定)に激突する。",
    "機体ごと突っ込んだ。機体が向いている方角へ移動力をすべて消費するまで移動する。",
    "制御系が損傷した。回避値が戦闘終了まで-1される(最低1)。",
    "踏み誤った。機体が向いている方角へ移動力の半分を消費するまで移動する。",
    "たたらを踏んだ。機体が向いている方角へ1の移動力で移動する。",
    "何も起きなかった。",
];

/// Ruby `get_shoot_fumble_table` / `get_melee_fumble_table` の共通部分。
///
/// Ruby は `table[num - dc]` が `nil` なら既定の `'1'` を返す
/// （`Array#[]` の負添字による回り込みは `num < 2` でしか起きず、
/// 2D10 の合計は必ず2以上なので到達しない）。
fn fumble_table_value(num: i64, table: &[&'static str]) -> &'static str {
    let index = num - 2;
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i))
        .copied()
        .unwrap_or("1")
}

/// Ruby `EmbryoMachine#replace_text`。`EMt+m@c#f` を `2R10...>=t[c,f]` に読み替える。
///
/// Ruby は8本の `gsub` を順に適用する。1コマンドに複数の `EM...` が並ぶことは無いので
/// ここでは1本の正規表現でオプション部分を省略可能にし、同じ結果を返す。
/// （`@c` と `#f` の有無で既定値 20 / 2 を埋める点も原典どおり。）
fn em_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)EM(\d+)([+-][+\-\d]+)?(@(\d+))?(#(\d+))?").expect("valid regex")
    })
}

/// Ruby `EmbryoMachine#replace_text`。
fn replace_text(string: &str) -> Cow<'_, str> {
    em_pattern().replace_all(string, |caps: &Captures<'_>| {
        let target = &caps[1];
        let modifier = caps.get(2).map_or("", |m| m.as_str());
        let critical = caps.get(4).map_or("20", |m| m.as_str());
        let fumble = caps.get(6).map_or("2", |m| m.as_str());
        format!("2R10{modifier}>={target}[{critical},{fumble}]")
    })
}

/// Ruby `checkRoll` の判定ロール書式。
///
/// Ruby: `/(^|\s)S?(2[rR]10([+\-\d]+)?([>=]+(\d+))(\[(\d+),(\d+)\]))(\s|$)/i`
fn check_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)S?(2R10([+\-\d]+)?([>=]+(\d+))(\[(\d+),(\d+)\]))(\s|$)")
            .expect("valid regex")
    })
}

/// Ruby `EmbryoMachine#checkRoll`。
fn check_roll(string: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let string = replace_text(string);

    let Some(m) = check_roll_pattern().captures(&string) else {
        return Ok(None);
    };

    let matched = m[2].to_owned();
    // Ruby: `Regexp.last_match(5)` などは必ずマッチするグループなので `to_i` が通る。
    let diff: i64 = m[5].parse().unwrap_or(0);
    let crit: i64 = m[7].parse().unwrap_or(20);
    let fumble: i64 = m[8].parse().unwrap_or(2);
    // Ruby: `mod = ArithmeticEvaluator.eval(modText) if modText`（不正な式は0）
    let mod_value = match m.get(3) {
        Some(text) => arithmetic::eval(text.as_str(), RoundType::Floor)?
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0),
        None => 0,
    };

    let mut dice_arr = rng.roll_barabara(2, 10)?;
    dice_arr.sort_unstable();
    let dice_now: i64 = dice_arr.iter().sum();
    let dice_str = dice_arr
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // Ruby: 命中部位のダイスは成否によらず必ず振る
    let dice_loc = rng.roll_sum(2, 10)?;
    let big_dice = dice_arr[1];

    let mut output = format!("{dice_now}[{dice_str}]");
    let total_n = dice_now + mod_value;
    if mod_value > 0 {
        output.push_str(&format!("+{mod_value}"));
    } else if mod_value < 0 {
        output.push_str(&mod_value.to_string());
    }

    // Ruby: `output =~ /[^\d\[\]]+/`（数字と角括弧以外を含むか）
    let mut output = if output
        .chars()
        .any(|c| !c.is_ascii_digit() && c != '[' && c != ']')
    {
        format!("({matched}) ＞ {output} ＞ {total_n}")
    } else {
        format!("({matched}) ＞ {output}")
    };

    // 成功度判定
    if dice_now <= fumble {
        output.push_str(" ＞ ファンブル");
    } else if dice_now >= crit {
        output.push_str(&format!(
            " ＞ クリティカル ＞ {}(ダメージ+10) ＞ [{dice_loc}]{}",
            get_table_by_number(big_dice, HIT_LEVEL_TABLE),
            get_table_by_number(dice_loc, HIT_LOCATION_TABLE)
        ));
    } else if total_n >= diff {
        output.push_str(&format!(
            " ＞ 成功 ＞ {} ＞ [{dice_loc}]{}",
            get_table_by_number(big_dice, HIT_LEVEL_TABLE),
            get_table_by_number(dice_loc, HIT_LOCATION_TABLE)
        ));
    } else {
        output.push_str(" ＞ 失敗");
    }

    Ok(Some(output))
}

/// Ruby `BCDice::GameSystem::EmbryoMachine`（ID: `EmbryoMachine`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbryoMachine;

impl GameSystem for EmbryoMachine {
    fn id(&self) -> &'static str {
        "EmbryoMachine"
    }

    fn name(&self) -> &'static str {
        "エムブリオマシンRPG"
    }

    fn sort_key(&self) -> &'static str {
        "えむふりおましんRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定ロール(EMt+m@c#f)
　目標値t、修正値m、クリティカル値c(省略時は20)、ファンブル値f(省略時は2)で攻撃判定を行います。
　命中した場合は命中レベルと命中部位も自動出力します。
　Rコマンドに読み替えされます。
・各種表
　・命中部位表　HLT
　・白兵攻撃ファンブル表　MFT
　・射撃攻撃ファンブル表　SFT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["EM", "HLT", "MFT", "SFT", "2R10"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `EmbryoMachine#result_nd10`（ゲーム別成功度判定）。
    fn result_nd10(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :>=
        if cmp_op != CmpOp::Ge {
            return None;
        }

        let result = if dice_total <= 2 {
            EvalResult::fumble("ファンブル")
        } else if dice_total >= 20 {
            EvalResult::critical("クリティカル")
        } else {
            // Ruby: target == "?" なら Result.nothing
            let Target::Number(target) = target else {
                return Some(CheckOutcome::Nothing);
            };
            if total >= target {
                EvalResult::success("成功")
            } else {
                EvalResult::failure("失敗")
            }
        };

        Some(CheckOutcome::Result(Box::new(result)))
    }

    /// Ruby `EmbryoMachine#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(result)));
        }

        // Ruby: `case command when /HLT/i ... when /SFT/i ... when /MFT/i`（部分一致）
        let (kind, table): (&str, &[&'static str]) = if command.contains("HLT") {
            ("命中部位", &[])
        } else if command.contains("SFT") {
            ("射撃ファンブル", SHOOT_FUMBLE_TABLE)
        } else if command.contains("MFT") {
            ("白兵ファンブル", MELEE_FUMBLE_TABLE)
        } else {
            // Ruby: output は '1' のまま（`dice_command` が nil に畳む）
            return Ok(Some(SpecificCommandOutput::text("1")));
        };

        let number = rng.roll_sum(2, 10)?;
        let output = if table.is_empty() {
            get_table_by_number(number, HIT_LOCATION_TABLE)
        } else {
            fumble_table_value(number, table)
        };

        if output == "1" {
            return Ok(Some(SpecificCommandOutput::text("1")));
        }
        Ok(Some(SpecificCommandOutput::text(format!(
            "{kind}表({number}) ＞ {output}"
        ))))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EmbryoMachine",
            "EmbryoMachine.toml",
            133,
        );
    }
}
