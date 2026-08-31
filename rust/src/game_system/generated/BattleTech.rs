//! `lib/bcdice/game_system/BattleTech.rb` の移植。

use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleTech;

impl GameSystem for BattleTech {
    fn id(&self) -> &'static str {
        "BattleTech"
    }
    fn name(&self) -> &'static str {
        "バトルテック"
    }
    fn sort_key(&self) -> &'static str {
        "はとるてつく"
    }
    fn help_message(&self) -> &'static str {
        r"・判定方法
　(回数)BT(ダメージ)(部位)+(基本値)>=(目標値)
　回数は省略時 1固定。
　部位はC（正面）R（右）、L（左）。省略時はC（正面）固定
　U（上半身）、L（下半身）を組み合わせ CU/RU/LU/CL/RL/LLも指定可能
　例）BT3+2>=4
　　正面からダメージ3の攻撃を技能ベース2目標値4で1回判定
　例）2BT3RL+5>=8
　　右下半身にダメージ3の攻撃を技能ベース5目標値8で2回判定
　ミサイルによるダメージは BT(ダメージ) の代わりに SRM2/4/6, LRM5/10/15/20 を指定
　例）3SRM6LU+5>=8
　　左上半身にSRM6連を技能ベース5目標値8で3回判定
  BT(ダメージ) の代わりに PPC を指定するとダメージ10で判定
  例）2PPCR+3>=10
  　右側からPPC（ダメージ10）による攻撃を技能ベース3目標値10で2回判定
・CT：致命的命中表
・DW：転倒後の向き表
・CDx：メック戦士意識維持ロール。ダメージ値x（1〜6）で判定　例）CD3
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*SRM", r"\d*LRM", r"\d*BT", r"\d*PPC", "CT", "DW", "CD"]
    }
    crate::impl_prefixes_pattern!();
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific(command, rng)
    }
}

#[derive(Clone, Copy)]
struct HitPart {
    name: &'static str,
    critical: bool,
}

#[derive(Default)]
struct DamageInfo {
    damages: Vec<i64>,
    criticals: Vec<&'static str>,
}

enum Weapon {
    Fixed(i64),
    Missile(&'static str),
}

fn ppc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(\d+)?PPC([LCR][LU]?)?([+-]\d+)?(?:>=)(\d+)$").unwrap())
}
fn count_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)(.+)$").unwrap())
}
fn tail_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([LCR][LU]?)?(\+\d+)?>=(\d+)").unwrap())
}

fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if command.eq_ignore_ascii_case("CT") {
        let sum = roll_2d6(rng)?;
        return Ok(Some(SpecificCommandOutput::text(format!(
            "致命的命中表({sum}) ＞ {}",
            critical_text(sum)
        ))));
    }
    if command.eq_ignore_ascii_case("DW") {
        let die = rng.roll_once(6)?;
        return Ok(Some(SpecificCommandOutput::text(format!(
            "転倒後の向き表({die}) ＞ {}",
            FALL_TABLE[(die - 1) as usize]
        ))));
    }
    if let Some(m) = ppc_pattern().captures(command) {
        let count = m.get(1).map_or(1, |v| v.as_str().parse().unwrap_or(1));
        let side = m.get(2).map_or("", |v| v.as_str());
        let modify = m.get(3).map_or(0, |v| v.as_str().parse().unwrap_or(0));
        let target = m[4].parse().unwrap_or(0);
        let tail = format!(
            "{side}{}>={target}",
            format::modifier(&crate::Int::from(modify))
        );
        return hit_result(count, Weapon::Fixed(10), &tail, rng)
            .map(|r| r.map(SpecificCommandOutput::result));
    }

    let upper = command.to_ascii_uppercase();
    let (count, body) = count_pattern()
        .captures(&upper)
        .map_or((1, upper.as_str()), |m| {
            (m[1].parse().unwrap_or(1), m.get(2).unwrap().as_str())
        });
    if let Some(damage) = body
        .strip_prefix("CD")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|d| (1..=6).contains(d))
    {
        return Ok(Some(SpecificCommandOutput::result(consciousness(
            damage, rng,
        )?)));
    }
    if let Some(m) = Regex::new(r"^((?:S|L)RM\d+)(.+)$").unwrap().captures(body) {
        let Some(kind) = missile_kind(&m[1]) else {
            return unknown_missile_result(count, &m[2], rng)
                .map(|r| r.map(SpecificCommandOutput::result));
        };
        return hit_result(count, Weapon::Missile(kind), &m[2], rng)
            .map(|r| r.map(SpecificCommandOutput::result));
    }
    if let Some(m) = Regex::new(r"^BT(\d+)(.+)$").unwrap().captures(body) {
        return hit_result(count, Weapon::Fixed(m[1].parse().unwrap_or(0)), &m[2], rng)
            .map(|r| r.map(SpecificCommandOutput::result));
    }
    Ok(None)
}

fn unknown_missile_result(
    count: i64,
    tail: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = tail_pattern().captures(tail) else {
        return Ok(None);
    };
    let base = m.get(2).map_or(0, |v| v.as_str().parse().unwrap_or(0));
    let target = m[3].parse::<i64>().unwrap_or(0);
    let mut lines = Vec::new();
    for _ in 0..count {
        let die1 = rng.roll_once(6)?;
        let die2 = rng.roll_once(6)?;
        let total = die1 + die2 + base;
        if total >= target {
            return Ok(None);
        }
        lines.push(format!(
            "{total}[{die1},{die2}{}]>={target} ＞ 外れ",
            if base > 0 {
                format!("+{base}")
            } else {
                String::new()
            }
        ));
    }
    lines.push(" ＞ 0回命中".to_owned());
    Ok(Some(EvalResult::failure(lines.join("\n"))))
}

fn hit_result(
    count: i64,
    weapon: Weapon,
    tail: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = tail_pattern().captures(tail) else {
        return Ok(None);
    };
    let side = m.get(1).map_or("C", |v| v.as_str());
    let base = m
        .get(2)
        .map_or(0, |v| v.as_str().parse::<i64>().unwrap_or(0));
    let target = m[3].parse::<i64>().unwrap_or(0);
    let mut lines = Vec::new();
    let mut damages: BTreeMap<&'static str, DamageInfo> = BTreeMap::new();
    let mut hits = 0;

    for _ in 0..count {
        let die1 = rng.roll_once(6)?;
        let die2 = rng.roll_once(6)?;
        let total = die1 + die2 + base;
        let mut line = format!(
            "{total}[{die1},{die2}{}]>={target} ＞ ",
            if base > 0 {
                format!("+{base}")
            } else {
                String::new()
            }
        );
        if total >= target {
            hits += 1;
            line.push_str("命中 ＞ ");
            line.push_str(&damage_text(&weapon, side, &mut damages, rng)?);
        } else {
            line.push_str("外れ");
        }
        lines.push(line);
    }
    let summary = if hits > 0 {
        format!(" ＞ {hits}回命中 命中箇所：{}", total_damage(&damages))
    } else {
        format!(" ＞ {hits}回命中")
    };
    lines.push(summary);
    let text = lines.join("\n");
    Ok(Some(if hits > 0 {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }))
}

fn damage_text(
    weapon: &Weapon,
    side: &str,
    damages: &mut BTreeMap<&'static str, DamageInfo>,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let (damage, cluster_roll, lrm) = match weapon {
        Weapon::Fixed(damage) => (*damage, None, false),
        Weapon::Missile(kind) => {
            let roll = roll_2d6(rng)?;
            let hits = missile_hits(kind, roll);
            (
                if kind.starts_with('L') {
                    hits
                } else {
                    hits * 2
                },
                Some(roll),
                kind.starts_with('L'),
            )
        }
    };
    let parts = if lrm { (damage + 4) / 5 } else { 1 };
    let mut text = if lrm {
        format!("[{}] {damage}点", cluster_roll.unwrap())
    } else {
        String::new()
    };
    for index in 0..parts {
        let current = if lrm {
            (damage - 5 * index).min(5)
        } else {
            damage
        };
        let damage_label = match cluster_roll {
            None => damage.to_string(),
            Some(roll) if !lrm => format!("[{roll}] {damage}"),
            Some(_) => current.to_string(),
        };
        let (part_text, part, critical) = hit_one(&damage_label, side, rng)?;
        if lrm {
            text.push(' ')
        }
        text.push_str(&part_text);
        let entry = damages.entry(part).or_default();
        entry.damages.push(current);
        if let Some(critical) = critical {
            entry.criticals.push(critical)
        }
    }
    Ok(text)
}

fn hit_one(
    damage: &str,
    side: &str,
    rng: &mut Randomizer,
) -> Result<(String, &'static str, Option<&'static str>), EvalError> {
    let (sum, part) = hit_part(side, rng)?;
    let mut text = format!(
        "[{sum}] {}{} {damage}点",
        part.name,
        if part.critical {
            "（致命的命中）"
        } else {
            ""
        }
    );
    let mut critical = None;
    if part.critical {
        let roll = roll_2d6(rng)?;
        let content = critical_text(roll);
        if roll > 7 {
            critical = Some(content)
        }
        text.push_str(&format!(" ＞ [{roll}] {content}"));
    }
    Ok((text, part.name, critical))
}

fn hit_part(side: &str, rng: &mut Randomizer) -> Result<(i64, HitPart), EvalError> {
    let upper = side.to_ascii_uppercase();
    let die = if upper.len() == 2 {
        rng.roll_once(6)?
    } else {
        roll_2d6(rng)?
    };
    let part = match upper.as_str() {
        "L" => match die {
            2 => p("左胴", true),
            3 | 6 => p("左脚", false),
            4 | 5 => p("左腕", false),
            7 => p("左胴", false),
            8 => p("胴中央", false),
            9 => p("右胴", false),
            10 => p("右腕", false),
            11 => p("右脚", false),
            _ => p("頭", false),
        },
        "R" => match die {
            2 => p("右胴", true),
            3 | 6 => p("右脚", false),
            4 | 5 => p("右腕", false),
            7 => p("右胴", false),
            8 => p("胴中央", false),
            9 => p("左胴", false),
            10 => p("左腕", false),
            11 => p("左脚", false),
            _ => p("頭", false),
        },
        "LU" => match die {
            1 | 2 => p("左胴", false),
            3 => p("胴中央", false),
            4 | 5 => p("左腕", false),
            _ => p("頭", false),
        },
        "CU" => match die {
            1 => p("左腕", false),
            2 => p("左胴", false),
            3 => p("胴中央", false),
            4 => p("右胴", false),
            5 => p("右腕", false),
            _ => p("頭", false),
        },
        "RU" => match die {
            1 | 2 => p("右胴", false),
            3 => p("胴中央", false),
            4 | 5 => p("右腕", false),
            _ => p("頭", false),
        },
        "LL" => p("左脚", false),
        "CL" => {
            if die <= 3 {
                p("右脚", false)
            } else {
                p("左脚", false)
            }
        }
        "RL" => p("右脚", false),
        _ => match die {
            2 => p("胴中央", true),
            3 | 4 => p("右腕", false),
            5 => p("右脚", false),
            6 => p("右胴", false),
            7 => p("胴中央", false),
            8 => p("左胴", false),
            9 => p("左脚", false),
            10 | 11 => p("左腕", false),
            _ => p("頭", false),
        },
    };
    Ok((die, part))
}

const fn p(name: &'static str, critical: bool) -> HitPart {
    HitPart { name, critical }
}

fn total_damage(damages: &BTreeMap<&'static str, DamageInfo>) -> String {
    let mut all = 0;
    let mut texts = Vec::new();
    for part in [
        "頭",
        "胴中央",
        "右胴",
        "左胴",
        "右脚",
        "左脚",
        "右腕",
        "左腕",
    ] {
        let Some(info) = damages.get(part) else {
            continue;
        };
        let damage = info.damages.iter().sum::<i64>();
        all += damage;
        let mut text = format!("{part}({}回) {damage}点", info.damages.len());
        if !info.criticals.is_empty() {
            text.push_str(&format!(" {}", info.criticals.join(" ")))
        }
        texts.push(text);
    }
    format!("{} ＞ 合計ダメージ {all}点", texts.join(" ／ "))
}

fn consciousness(damage: i64, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let command = format!("CD{damage}");
    if damage == 6 {
        return Ok(EvalResult::fumble(format!("{command} ＞ 死亡")));
    }
    let target = [0, 3, 5, 7, 10, 11][damage as usize];
    let values = rng.roll_barabara(2, 6)?;
    let sum = values.iter().sum::<i64>();
    let success = sum >= target;
    let text = format!(
        "{command} ＞ (2D6>={target}) ＞ {sum}[{},{}] ＞ {sum} ＞ {}",
        values[0],
        values[1],
        if success { "成功" } else { "失敗" }
    );
    Ok(if success {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    })
}

fn roll_2d6(rng: &mut Randomizer) -> Result<i64, EvalError> {
    Ok(rng.roll_once(6)? + rng.roll_once(6)?)
}

fn critical_text(sum: i64) -> &'static str {
    match sum {
        2..=7 => "致命的命中はなかった",
        8..=9 => "1箇所の致命的命中",
        10..=11 => "2箇所の致命的命中",
        _ => "その部位が吹き飛ぶ（腕、脚、頭）または3箇所の致命的命中（胴）",
    }
}

fn missile_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "SRM2" => Some("SRM2"),
        "SRM4" => Some("SRM4"),
        "SRM6" => Some("SRM6"),
        "LRM5" => Some("LRM5"),
        "LRM10" => Some("LRM10"),
        "LRM15" => Some("LRM15"),
        "LRM20" => Some("LRM20"),
        _ => None,
    }
}
fn missile_hits(kind: &str, roll: i64) -> i64 {
    match kind {
        "SRM2" => {
            if roll <= 7 {
                1
            } else {
                2
            }
        }
        "SRM4" => match roll {
            2 => 1,
            3..=6 => 2,
            7..=10 => 3,
            _ => 4,
        },
        "SRM6" => match roll {
            2..=3 => 2,
            4..=5 => 3,
            6..=8 => 4,
            9..=10 => 5,
            _ => 6,
        },
        "LRM5" => match roll {
            2 => 1,
            3..=4 => 2,
            5..=8 => 3,
            9..=10 => 4,
            _ => 5,
        },
        "LRM10" => match roll {
            2..=3 => 3,
            4 => 4,
            5..=8 => 6,
            9..=10 => 8,
            _ => 10,
        },
        "LRM15" => match roll {
            2..=3 => 5,
            4 => 6,
            5..=8 => 9,
            9..=10 => 12,
            _ => 15,
        },
        "LRM20" => match roll {
            2..=3 => 6,
            4 => 9,
            5..=8 => 12,
            9..=10 => 16,
            _ => 20,
        },
        _ => 0,
    }
}

static FALL_TABLE: &[&str] = &[
    "同じ（前面から転倒） 正面／背面",
    "1ヘクスサイド右（側面から転倒） 右側面",
    "2ヘクスサイド右（側面から転倒） 右側面",
    "180度逆（背面から転倒） 正面／背面",
    "2ヘクスサイド左（側面から転倒） 左側面",
    "1ヘクスサイド左（側面から転倒） 左側面",
];

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::{Path, PathBuf};
    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/BattleTech.toml");
        path.exists().then_some(path)
    }
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else { return };
        let data = TestDataFile::load(&path).expect("BattleTech.toml must parse");
        assert_eq!(
            data.tests.len(),
            37,
            "case count in test/data/BattleTech.toml"
        );
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "BattleTech");
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new("BattleTech"), &tc.input, &mut src) {
                Ok(Some(result)) if !tc.expects_nil() => {
                    if result.text != tc.output
                        || result.secret != tc.secret
                        || result.success != tc.success
                        || result.failure != tc.failure
                        || result.critical != tc.critical
                        || result.fumble != tc.fumble
                    {
                        failures.push(format!(
                            "{}:{}\nexpected: {:?}\nactual: {:?}",
                            i + 1,
                            tc.input,
                            tc.output,
                            result
                        ));
                    }
                }
                Ok(None) if tc.expects_nil() => {}
                other => failures.push(format!("{}:{}: {other:?}", i + 1, tc.input)),
            }
            if !src.is_empty() {
                failures.push(format!(
                    "{}:{}: {} unconsumed rands",
                    i + 1,
                    tc.input,
                    src.remaining()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
