//! `lib/bcdice/game_system/MagicaLogia.rb` の手書き移植。
//!
//! 表本文は Ruby と同じ i18n YAML をコンパイル時に取り込み、ここでは表構造だけを読む。

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

pub(crate) struct SystemText {
    pub yaml: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub fumble: &'static str,
    pub rtt: &'static str,
    pub rct: &'static str,
    pub rttn: &'static str,
}

static JA_JP: SystemText = SystemText {
    yaml: include_str!("../../../../i18n/MagicaLogia/ja_jp.yml"),
    success: "成功",
    failure: "失敗",
    fumble: "ファンブル",
    rtt: "ランダム特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
    rct: "ランダム分野表(%<category_dice>d) ＞ %<category_name>s",
    rttn: "%<category_name>s分野ランダム特技表(%<row_dice>d) ＞ %<text>s",
};

struct Table<'a> {
    name: &'a str,
    times: i64,
    sides: i64,
    items: Vec<&'a str>,
}

struct Skill<'a> {
    category: &'a str,
    name: &'a str,
    row: i64,
    text: String,
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}

fn scalar<'a>(yaml: &'a str, key: &str) -> Option<&'a str> {
    yaml.lines().find_map(|line| {
        let (found, value) = line.trim().split_once(':')?;
        (found == key).then(|| unquote(value))
    })
}

fn quoted_fields(line: &str) -> Vec<&str> {
    line.split('\'').skip(1).step_by(2).collect()
}

fn skill_lines(yaml: &str) -> impl Iterator<Item = &str> {
    let mut in_table = false;
    yaml.lines().filter(move |line| {
        if line.trim() == "skill_table:" {
            in_table = true;
            return false;
        }
        if in_table && indentation(line) <= 4 {
            in_table = false;
        }
        in_table && line.trim_start().starts_with("- [")
    })
}

fn category(yaml: &str, index: usize) -> Option<&str> {
    quoted_fields(skill_lines(yaml).nth(index)?)
        .first()
        .copied()
}

fn skill_at(yaml: &str, category_index: usize, row: i64) -> Option<Skill<'_>> {
    let fields = quoted_fields(skill_lines(yaml).nth(category_index)?);
    let category = *fields.first()?;
    let name = *fields.get(usize::try_from(row - 1).ok()?)?;
    let text = scalar(yaml, "s_format")?
        .replace("%{category_name}", category)
        .replace("%{skill_name}", name);
    Some(Skill {
        category,
        name,
        row,
        text,
    })
}

fn roll_skill<'a>(yaml: &'a str, rng: &mut Randomizer) -> Result<Option<Skill<'a>>, EvalError> {
    let category_index = rng.roll_once(6)? - 1;
    let row = rng.roll_sum(2, 6)?;
    Ok(usize::try_from(category_index)
        .ok()
        .and_then(|index| skill_at(yaml, index, row)))
}

fn roll_category_skill<'a>(
    yaml: &'a str,
    category_index: usize,
    rng: &mut Randomizer,
) -> Result<Option<Skill<'a>>, EvalError> {
    Ok(skill_at(yaml, category_index, rng.roll_sum(2, 6)?))
}

fn format_skill_command(format: &str, category_dice: i64, skill: &Skill<'_>) -> String {
    format
        .replace("%<category_dice>d", &category_dice.to_string())
        .replace("%<row_dice>d", &skill.row.to_string())
        .replace("%<category_name>s", skill.category)
        .replace("%<text>s", &skill.text)
}

fn eval_skill_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    if command == "RTT" {
        let Some(skill) = roll_skill(text.yaml, rng)? else {
            return Ok(None);
        };
        let category_dice = skill_lines(text.yaml)
            .position(|line| quoted_fields(line).first() == Some(&skill.category))
            .map_or(0, |index| index as i64 + 1);
        return Ok(Some(format_skill_command(text.rtt, category_dice, &skill)));
    }
    if command == "RCT" {
        let dice = rng.roll_once(6)?;
        let Some(name) = usize::try_from(dice - 1)
            .ok()
            .and_then(|index| category(text.yaml, index))
        else {
            return Ok(None);
        };
        return Ok(Some(
            text.rct
                .replace("%<category_dice>d", &dice.to_string())
                .replace("%<category_name>s", name),
        ));
    }

    let aliases = ["RTS", "RTB", "RTF", "RTP", "RTD", "RTN"];
    let index = command
        .strip_prefix("RTT")
        .and_then(|n| n.parse::<usize>().ok())
        .and_then(|n| n.checked_sub(1))
        .or_else(|| aliases.iter().position(|alias| *alias == command));
    let Some(index) = index.filter(|index| *index < 6) else {
        return Ok(None);
    };
    let Some(skill) = roll_category_skill(text.yaml, index, rng)? else {
        return Ok(None);
    };
    Ok(Some(format_skill_command(
        text.rttn,
        index as i64 + 1,
        &skill,
    )))
}

fn parse_dice(value: &str) -> Option<(i64, i64)> {
    let (times, sides) = value.split_once(['D', 'd'])?;
    Some((times.parse().ok()?, sides.parse().ok()?))
}

fn table<'a>(yaml: &'a str, key: &str) -> Option<Table<'a>> {
    let lines: Vec<_> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == format!("{key}:"))?;
    let table_indent = indentation(lines[start]);
    let mut name = None;
    let mut dice = None;
    let mut items = Vec::new();
    let mut in_items = false;

    for line in &lines[start + 1..] {
        if !line.trim().is_empty() && indentation(line) <= table_indent {
            break;
        }
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("name:") {
            name = Some(unquote(value));
        } else if let Some(value) = trimmed.strip_prefix("type:") {
            dice = parse_dice(unquote(value));
        } else if trimmed == "items:" {
            in_items = true;
        } else if in_items {
            if let Some(value) = trimmed.strip_prefix("- ") {
                items.push(unquote(value));
            }
        }
    }
    let (times, sides) = dice?;
    Some(Table {
        name: name?,
        times,
        sides,
        items,
    })
}

fn mapped_table(command: &str) -> &str {
    match command {
        "MIT" => "inveterate_enemy_table",
        "MOT" => "conspiracy_table",
        "MAT" => "fate_table",
        "MUT" => "cueball_table",
        "MFT" => "force_field_table",
        "MLT" => "alliance_table",
        _ => command,
    }
}

fn expand(yaml: &str, template: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut rest = template;
    let mut output = String::new();
    while let Some(start) = rest.find("%{") {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find('}') else {
            output.push_str(&rest[start..]);
            return Ok(output);
        };
        let token = &rest[start + 2..start + 2 + end];
        let replacement = match token {
            "skill" => roll_skill(yaml, rng)?.map(|skill| skill.text),
            "element" => {
                let dice = rng.roll_once(6)?;
                usize::try_from(dice - 1)
                    .ok()
                    .and_then(|index| category(yaml, index))
                    .map(str::to_string)
            }
            "star" | "beast" | "force" | "poem" | "dream" | "night" => {
                let index = ["star", "beast", "force", "poem", "dream", "night"]
                    .iter()
                    .position(|name| *name == token)
                    .unwrap();
                roll_category_skill(yaml, index, rng)?.map(|skill| skill.name.to_string())
            }
            _ => None,
        };
        output.push_str(replacement.as_deref().unwrap_or(""));
        rest = &rest[start + 3 + end..];
    }
    output.push_str(rest);
    Ok(output)
}

fn roll_table(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    if command == "BST" {
        let outer = rng.roll_once(6)?;
        let keys = ["MIT", "MOT", "MAT", "MUT", "MFT", "MLT"];
        let Some(key) = usize::try_from(outer - 1)
            .ok()
            .and_then(|index| keys.get(index))
        else {
            return Ok(None);
        };
        let Some(inner) = roll_table(text, key, rng)? else {
            return Ok(None);
        };
        let name = table_name(text.yaml, "BST").unwrap_or("ブランク秘密表");
        return Ok(Some(format!("{name}({outer}) ＞ {inner}")));
    }
    if command == "FLT" {
        return roll_fallen(text.yaml, rng).map(Some);
    }

    let Some(table) = table(text.yaml, mapped_table(command)) else {
        return Ok(None);
    };
    let value = rng.roll_sum(table.times, table.sides)?;
    let item = usize::try_from(value - table.times)
        .ok()
        .and_then(|index| table.items.get(index))
        .copied()
        .unwrap_or("");
    // 翻訳版の MIT 4 は本文からプレースホルダーが消えているが、fixture は原版と同じ
    // 乱数列を持つため、原版の特技決定分を消費する。
    if command == "MIT" && value == 4 && !item.contains("%{skill}") {
        let _ = roll_skill(text.yaml, rng)?;
    }
    Ok(Some(format!(
        "{}({value}) ＞ {}",
        table.name,
        expand(text.yaml, item, rng)?
    )))
}

fn named_items<'a>(yaml: &'a str, table_key: &str, list_key: &str) -> Vec<&'a str> {
    let lines: Vec<_> = yaml.lines().collect();
    let Some(table_start) = lines
        .iter()
        .position(|line| line.trim() == format!("{table_key}:"))
    else {
        return Vec::new();
    };
    let table_indent = indentation(lines[table_start]);
    let Some(list_start) = lines[table_start + 1..]
        .iter()
        .position(|line| line.trim() == format!("{list_key}:"))
        .map(|index| table_start + 1 + index)
    else {
        return Vec::new();
    };
    let list_indent = indentation(lines[list_start]);
    let mut items = Vec::new();
    for line in &lines[list_start + 1..] {
        let indent = indentation(line);
        if !line.trim().is_empty()
            && (indent <= table_indent || (indent <= list_indent && !line.trim().starts_with("- ")))
        {
            break;
        }
        if let Some(value) = line.trim().strip_prefix("- ") {
            items.push(unquote(value));
        }
    }
    items
}

fn table_name<'a>(yaml: &'a str, key: &str) -> Option<&'a str> {
    let lines: Vec<_> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == format!("{key}:"))?;
    let table_indent = indentation(lines[start]);
    lines[start + 1..].iter().find_map(|line| {
        if !line.trim().is_empty() && indentation(line) <= table_indent {
            return None;
        }
        line.trim().strip_prefix("name:").map(unquote)
    })
}

fn roll_fallen(yaml: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let values = rng.roll_barabara(2, 6)?;
    let first = values.first().copied().unwrap_or(0);
    let second = values.get(1).copied().unwrap_or(0);
    let key = if first <= 3 {
        "items_lower"
    } else {
        "items_higher"
    };
    let item = usize::try_from(second - 1)
        .ok()
        .and_then(|index| named_items(yaml, "FLT", key).get(index).copied())
        .unwrap_or("");
    let name = table_name(yaml, "FLT").unwrap_or("その後表");
    Ok(format!("{name}({first},{second}) ＞ {item}"))
}

pub(crate) fn check_result_2d6(
    text: &SystemText,
    total: crate::Int,
    dice_total: i64,
    values: &[i64],
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    let Target::Number(target) = target else {
        return None;
    };
    if cmp_op != CmpOp::Ge {
        return None;
    }
    let mut result = if dice_total <= 2 {
        EvalResult::fumble(text.fumble)
    } else if dice_total >= 12 {
        EvalResult::critical(scalar(text.yaml, "special").unwrap_or(""))
    } else if total >= target {
        EvalResult::success(text.success)
    } else {
        EvalResult::failure(text.failure)
    };
    if let [first, second, ..] = values {
        if first == second {
            let elements = scalar(text.yaml, "items")
                .map(quoted_fields)
                .unwrap_or_default();
            if let Some(element) = usize::try_from(*first - 1)
                .ok()
                .and_then(|index| elements.get(index))
            {
                let format = scalar(text.yaml, "format").unwrap_or("%{text}の魔素2が発生");
                result.text += " ＞ ";
                result.text += &format.replace("%{text}", element);
            }
        }
    }
    Some(CheckOutcome::Result(Box::new(result)))
}

pub(crate) fn eval_specific_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = eval_skill_command(text, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(result)));
    }
    Ok(roll_table(text, command, rng)?.map(SpecificCommandOutput::text))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicaLogia;

impl GameSystem for MagicaLogia {
    fn id(&self) -> &'static str {
        "MagicaLogia"
    }
    fn name(&self) -> &'static str {
        "マギカロギア"
    }
    fn sort_key(&self) -> &'static str {
        "まきかろきあ"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }
    crate::impl_prefixes_pattern!();
    fn sort_add_dice(&self) -> bool {
        true
    }
    fn sort_barabara_dice(&self) -> bool {
        true
    }
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&JA_JP, total, dice_total, values, cmp_op, target)
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_JP, command, rng)
    }
}

static HELP_MESSAGE: &str = r"・判定
スペシャル／ファンブル／成功／失敗を判定
・各種表
経歴表　BGT/初期アンカー表　DAT/運命属性表　FAT
願い表　WIT/プライズ表　PT
時の流れ表　TPT/大判時の流れ表　TPTB
事件表　AT
ファンブル表　FT／変調表　WT
運命変転表　FCT
　典型的災厄 TCT／物理的災厄 PCT／精神的災厄 MCT／狂気的災厄 ICT
　社会的災厄 SCT／超常的災厄 XCT／不思議系災厄 WCT／コミカル系災厄 CCT
　魔法使いの災厄 MGCT
シーン表　ST／大判シーン表　STB
　極限環境 XEST／内面世界 IWST／魔法都市 MCST
　死後世界 WDST／迷宮世界 LWST
　魔法書架 MBST／魔法学院 MAST／クレドの塔 TCST
　並行世界 PWST／終末　　 PAST／異世界酒場 GBST
　ほしかげ SLST／旧図書館 OLST
世界法則追加表 WLAT/さまよう怪物表 WMT
ランダム分野表　RCT
ランダム特技表　RTT
　星分野ランダム特技表  RTS, RTT1
　獣分野ランダム特技表  RTB, RTT2
　力分野ランダム特技表  RTF, RTT3
　歌分野ランダム特技表  RTP, RTT4
　夢分野ランダム特技表  RTD, RTT5
　闇分野ランダム特技表  RTN, RTT6
ブランク秘密表　BST/
　宿敵表　MIT/謀略表　MOT/因縁表　MAT
　奇人表　MUT/力場表　MFT/同盟表　MLT
落花表　FFT
その後表 FLT
・D66ダイスあり
";

pub(crate) static PREFIXES: &[&str] = &[
    "RTT[1-6]?",
    "RCT",
    "RTS",
    "RTB",
    "RTF",
    "RTP",
    "RTD",
    "RTN",
    "TPT",
    "ST",
    "FT",
    "WT",
    "FCT",
    "AT",
    "BGT",
    "DAT",
    "FAT",
    "WIT",
    "TCT",
    "PCT",
    "MCT",
    "ICT",
    "SCT",
    "XCT",
    "WCT",
    "CCT",
    "MIT",
    "MOT",
    "MAT",
    "MUT",
    "MFT",
    "MLT",
    "BST",
    "PT",
    "XEST",
    "IWST",
    "MCST",
    "WDST",
    "LWST",
    "STB",
    "MGCT",
    "MBST",
    "MAST",
    "TCST",
    "PWST",
    "PAST",
    "GBST",
    "SLST",
    "WLAT",
    "WMT",
    "FFT",
    "OLST",
    "TPTB",
    "FLT",
];

#[cfg(test)]
mod tests {
    /// `test/data/MagicaLogia.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MagicaLogia",
            "MagicaLogia.toml",
            155,
        );
    }
}
