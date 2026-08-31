//! `lib/bcdice/game_system/FutariSousa.rb` の手書き移植。
//!
//! 表本文は Ruby と同じ i18n YAML をコンパイル時に取り込む。

use std::sync::OnceLock;

use regex::Regex;

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

pub(crate) struct SystemText {
    pub yaml: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub dt_fumble: &'static str,
    pub dt_special: &'static str,
    pub as_fumble: &'static str,
    pub as_special: &'static str,
    pub as_success: &'static str,
    pub consume_shrd_text_rand: bool,
}

static JA_JP: SystemText = SystemText {
    yaml: include_str!("../../../../i18n/FutariSousa/ja_jp.yml"),
    success: "成功",
    failure: "失敗",
    dt_fumble: "ファンブル（変調を受け、助手の心労が1点上昇）",
    dt_special: "スペシャル（助手の余裕を1点獲得）",
    as_fumble: "ファンブル（変調を受け、心労が1点上昇）",
    as_special: "スペシャル（余裕2点と、探偵から助手への感情を獲得）",
    as_success: "成功（余裕1点と、探偵から助手への感情を獲得）",
    consume_shrd_text_rand: true,
};

struct Table<'a> {
    name: &'a str,
    times: i64,
    sides: i64,
    items: Vec<String>,
}

struct D66Table<'a> {
    name: &'a str,
    items: Vec<(i64, String)>,
}

fn indentation(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
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
    let mut index = start + 1;

    while let Some(line) = lines.get(index) {
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
        } else if in_items && trimmed == "- |-" {
            let item_indent = indentation(line);
            index += 1;
            let base_indent = lines
                .get(index)
                .map_or(item_indent + 2, |line| indentation(line));
            let mut body = String::new();
            while let Some(block_line) = lines.get(index) {
                if !block_line.is_empty() && indentation(block_line) <= item_indent {
                    break;
                }
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(block_line.get(base_indent..).unwrap_or(""));
                index += 1;
            }
            items.push(body);
            continue;
        } else if in_items {
            if let Some(value) = trimmed.strip_prefix("- ") {
                items.push(unquote(value).to_string());
            }
        }
        index += 1;
    }

    let (times, sides) = dice?;
    Some(Table {
        name: name?,
        times,
        sides,
        items,
    })
}

fn d66_table<'a>(yaml: &'a str, key: &str) -> Option<D66Table<'a>> {
    let lines: Vec<_> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == format!("{key}:"))?;
    let table_indent = indentation(lines[start]);
    let mut name = None;
    let mut is_d66 = false;
    let mut in_items = false;
    let mut items = Vec::new();

    for line in &lines[start + 1..] {
        if !line.trim().is_empty() && indentation(line) <= table_indent {
            break;
        }
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("name:") {
            name = Some(unquote(value));
        } else if trimmed.starts_with("d66_sort_type:") {
            is_d66 = true;
        } else if trimmed == "items:" {
            in_items = true;
        } else if in_items {
            let (key, value) = trimmed.split_once(':')?;
            if let Ok(key) = key.parse() {
                items.push((key, unquote(value).to_string()));
            }
        }
    }
    is_d66.then_some(D66Table { name: name?, items })
}

fn roll_plain_table(
    yaml: &str,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    if let Some(table) = table(yaml, command) {
        let value = rng.roll_sum(table.times, table.sides)?;
        let item = usize::try_from(value - table.times)
            .ok()
            .and_then(|index| table.items.get(index))
            .map_or("", String::as_str);
        return Ok(Some(format!("{}({value}) ＞ {item}", table.name)));
    }
    if let Some(table) = d66_table(yaml, command) {
        let value = rng.roll_d66(D66SortType::Asc)?;
        let item = table
            .items
            .iter()
            .find(|(key, _)| *key == value)
            .map_or("", |(_, item)| item.as_str());
        return Ok(Some(format!("{}({value}) ＞ {item}", table.name)));
    }
    Ok(None)
}

fn chain_keys(command: &str) -> Option<&'static [Option<&'static str>]> {
    match command {
        "SHRD" => Some(&[
            Some("SHFM"),
            Some("SHBT"),
            Some("SHPI"),
            Some("SHEG"),
            Some("SHWP"),
            Some("SHDS"),
            Some("SHIN"),
            Some("SHEM"),
            Some("SHFT"),
            None,
        ]),
        "SHND" => Some(&[
            Some("SHHE"),
            Some("SHHF"),
            Some("SHMP"),
            Some("SHSB"),
            Some("SHFR"),
            Some("SHIS"),
        ]),
        "SHAD" => Some(&[
            Some("SHSE"),
            Some("SHLM"),
            Some("SHJS"),
            Some("SHAR"),
            Some("SHRM"),
            Some("SHNT"),
        ]),
        "SHKD" => Some(&[
            Some("SHIM"),
            Some("SHNO"),
            Some("SHNE"),
            Some("SHHD"),
            None,
            None,
        ]),
        "SHLD" => Some(&[
            Some("SHGD"),
            Some("SHSA"),
            Some("SHEP"),
            Some("SHXP"),
            None,
            None,
        ]),
        "FLTRA" => Some(&[
            Some("FLTF66"),
            Some("FLTB66"),
            Some("FLTH66"),
            Some("FLTS66"),
            Some("FLTA66"),
            Some("FLTW66"),
        ]),
        _ => None,
    }
}

fn roll_table(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(keys) = chain_keys(command) else {
        return roll_plain_table(text.yaml, command, rng);
    };
    let Some(outer) = table(text.yaml, command) else {
        return Ok(None);
    };
    let value = rng.roll_sum(outer.times, outer.sides)?;
    let Some(index) = usize::try_from(value - outer.times).ok() else {
        return Ok(None);
    };
    let body = if let Some(Some(inner)) = keys.get(index) {
        roll_plain_table(text.yaml, inner, rng)?.unwrap_or_default()
    } else {
        if command != "SHRD" || text.consume_shrd_text_rand {
            let _ = rng.roll_once(10)?;
        }
        outer.items.get(index).cloned().unwrap_or_default()
    };
    Ok(Some(format!("{}({value}) ＞ {body}", outer.name)))
}

fn command_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)?(DT(?:6)?|AS(?:10)?)(?:>=(\d+))?(?:\[(\d+(?:,\d+)*)\])?$")
            .expect("valid FutariSousa command regex")
    })
}

fn roll_check(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(captures) = command_pattern().captures(command) else {
        return Ok(None);
    };
    let count = captures
        .get(1)
        .map_or(2, |value| value.as_str().parse().unwrap_or(i64::MAX));
    let kind = captures.get(2).unwrap().as_str().to_ascii_uppercase();
    let target = captures
        .get(3)
        .map_or(4, |value| value.as_str().parse().unwrap_or(i64::MAX));
    let specials: Vec<i64> = captures.get(4).map_or_else(Vec::new, |value| {
        value
            .as_str()
            .split(',')
            .filter_map(|value| value.parse().ok())
            .collect()
    });
    let detective = kind.starts_with("DT");
    let sides = if kind == "DT6" || kind == "AS" { 6 } else { 10 };
    let dice = rng.roll_barabara(count, sides)?;
    let max = dice.iter().copied().max().unwrap_or(i64::MIN);
    let mut result = if max <= 1 {
        EvalResult::fumble(if detective {
            text.dt_fumble
        } else {
            text.as_fumble
        })
    } else if dice
        .iter()
        .any(|value| *value == 6 || specials.contains(value))
    {
        EvalResult::critical(if detective {
            text.dt_special
        } else {
            text.as_special
        })
    } else if max >= target {
        EvalResult::success(if detective {
            text.success
        } else {
            text.as_success
        })
    } else {
        EvalResult::failure(text.failure)
    };
    let dice = dice
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    result.text = format!("{command}({dice}) ＞ {}", result.text);
    Ok(Some(SpecificCommandOutput::result(result)))
}

pub(crate) fn eval_specific_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_check(text, command, rng)? {
        return Ok(Some(result));
    }
    Ok(roll_table(text, command, rng)?.map(SpecificCommandOutput::text))
}

pub(crate) fn ruby_help(source: &str, terminator: &str) -> String {
    let mut lines = source
        .lines()
        .skip_while(|line| !line.contains("HELP_MESSAGE = <<~"))
        .skip(1)
        .take_while(|line| line.trim() != terminator)
        .collect::<Vec<_>>();
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| indentation(line))
        .min()
        .unwrap_or(0);
    let mut help = lines
        .drain(..)
        .map(|line| line.get(indent..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    help.push('\n');
    help
}

fn help_message() -> &'static str {
    static HELP: OnceLock<String> = OnceLock::new();
    HELP.get_or_init(|| {
        ruby_help(
            include_str!("../../../../lib/bcdice/game_system/FutariSousa.rb"),
            "MESSAGETEXT",
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FutariSousa;

impl GameSystem for FutariSousa {
    fn id(&self) -> &'static str {
        "FutariSousa"
    }
    fn name(&self) -> &'static str {
        "フタリソウサ"
    }
    fn sort_key(&self) -> &'static str {
        "ふたりそうさ"
    }
    fn help_message(&self) -> &'static str {
        help_message()
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }
    crate::impl_prefixes_pattern!();
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_JP, command, rng)
    }
}

pub(crate) static PREFIXES: &[&str] = &[
    r"(\d+)?DT(?:6)?",
    r"(\d+)?AS(?:10)?",
    "SHRD",
    "SHND",
    "SHAD",
    "SHKD",
    "SHLD",
    "SHFM",
    "SHBT",
    "SHPI",
    "SHEG",
    "SHWP",
    "SHDS",
    "SHFT",
    "SHIN",
    "SHEM",
    "SHHE",
    "SHHF",
    "SHMP",
    "SHSB",
    "SHFR",
    "SHIS",
    "SHSE",
    "SHLM",
    "SHJS",
    "SHAR",
    "SHRM",
    "SHNT",
    "SHIM",
    "SHNO",
    "SHNE",
    "SHHD",
    "SHGD",
    "SHSA",
    "SHEP",
    "SHXP",
    "EVS",
    "EVW",
    "EVN",
    "EVC",
    "EVV",
    "EVE",
    "EVD",
    "EVA",
    "EVT",
    "EVH",
    "EVX",
    "EVG",
    "EVQ",
    "EVM",
    "EVP",
    "EVO",
    "EVF",
    "EVB",
    "EVL",
    "EVZ",
    "EVR",
    "EV6S",
    "EV6F",
    "EV8A",
    "EV8N",
    "OBT",
    "ACT",
    "EWT",
    "WMT",
    "BGDD",
    "BGDG",
    "BGDM",
    "BGAJ",
    "BGAP",
    "BGAI",
    "HT",
    "BT",
    "GRT",
    "MIT",
    "MITE",
    "JBT66",
    "JBT10",
    "FST66",
    "FST10",
    "LDT66",
    "LDT10",
    "FLT66",
    "FLT10",
    "FLTL66",
    "FLTD66",
    "FLTRA",
    "FLTF66",
    "FLTB66",
    "FLTH66",
    "FLTS66",
    "FLTA66",
    "FLTW66",
    "FLTC66",
    "FLTU66",
    "FLTO66",
    "FLTI66",
    "NCT66",
    "NCT10",
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        super::super::TokyoNova::assert_toml_cases("FutariSousa", "FutariSousa.toml", 172);
    }
}
