//! P4で手書き移植した `lib/bcdice/game_system/DeadlineHeroes_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `DeadlineHeroes` を継承せず `Base` 直下で、行為判定・デスチャート・
//! ネームチャートがすべて別実装になっている。そのため親の [`super::DeadlineHeroes`]
//! を `SystemTables` で使い回すことはできず、こちらに韓語版の評価一式を置く。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::DeadlineHeroes_Korean`（ID: `DeadlineHeroes:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineHeroes_Korean;

impl GameSystem for DeadlineHeroes_Korean {
    fn id(&self) -> &'static str {
        "DeadlineHeroes:Korean"
    }

    fn name(&self) -> &'static str {
        "데드라인 히어로즈 RPG"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:데드라인 히어로즈 RPG"
    }

    fn help_message(&self) -> &'static str {
        r"・행위판정（DLHx）
  x：성공률 (%)
  예) DLH80
  크리티컬, 펌블을 자동으로 판정합니다.
  「DLH50+20-30」처럼 가산, 감산 기재도 가능.
  성공률은 상한 100%, 하한 0%

・데스차트（DCxY）
  x：L=육체 / S=정신 / C=환경
  Y：수치
  예)
    DCL-5 → 라이프 -5로 판정
    DCS-3 → 새니티 -3으로 판정
    DCC0  → 크레딧 0으로 판정

・히어로 네임 차트（HNC）

・리얼 네임 차트:일본（RNCJ） / 해외（RNCO）

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["DLH", "DCL", "DCS", "DCC", "HNC", "RNCO", "RNCJ"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `DeadlineHeroes_Korean#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_check(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(text) = death_chart(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = hero_name_chart(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = real_name_chart_overseas(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = real_name_chart_jp(command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(None)
}

/// Ruby `/^DLH([0-9+-]+)$/i`。
fn dlh_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DLH([0-9+-]+)$").expect("valid regex"))
}

/// Ruby `/^DC([LSC])([+-]?\d+)$/i`。
fn death_chart_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC([LSC])([+-]?\d+)$").expect("valid regex"))
}

/// Ruby `String#to_i`。
fn ruby_to_i(s: &str) -> i64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return 0;
    }
    s[..end].parse().unwrap_or_else(|_| {
        if s.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `Array#[]`（負添字は末尾から。範囲外は nil）。
fn ruby_array_get<T: Copy>(items: &[T], index: i64) -> Option<T> {
    if index >= 0 {
        usize::try_from(index)
            .ok()
            .and_then(|i| items.get(i))
            .copied()
    } else {
        let wrapped = items.len() as i64 + index;
        if wrapped >= 0 {
            usize::try_from(wrapped)
                .ok()
                .and_then(|i| items.get(i))
                .copied()
        } else {
            None
        }
    }
}

/// Ruby `DeadlineHeroes_Korean#sample`。
fn sample<'a>(items: &'a [&'a str], rng: &mut Randomizer) -> Result<&'a str, EvalError> {
    let index = rng.roll_once(items.len() as i64)? - 1;
    Ok(ruby_array_get(items, index).unwrap_or(""))
}

/// Ruby `DeadlineHeroes_Korean#roll_check`。
fn roll_check(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = dlh_pattern().captures(command) else {
        return Ok(None);
    };

    let expr = &m[1];
    let mut target: i64 = 0;
    for part in PartIter::new(expr) {
        target = target.saturating_add(ruby_to_i(part));
    }
    target = target.clamp(0, 100);

    let roll = rng.roll_once(100)?;
    let roll_str = format!("{roll:02}");
    let is_double = roll_str.as_bytes().first() == roll_str.as_bytes().get(1);

    let text = format!("성공률{target}% ＞ {roll_str}");
    let result = if is_double && roll <= target {
        EvalResult::critical(format!("{text} ＞ 크리티컬"))
    } else if is_double && roll > target {
        EvalResult::fumble(format!("{text} ＞ 펌블"))
    } else if roll <= target {
        EvalResult::success(format!("{text} ＞ 성공"))
    } else {
        EvalResult::failure(format!("{text} ＞ 실패"))
    };
    Ok(Some(result))
}

/// Ruby `expr.scan(/[+-]?\d+/)`。
struct PartIter<'a> {
    rest: &'a str,
}

impl<'a> PartIter<'a> {
    fn new(expr: &'a str) -> Self {
        Self { rest: expr }
    }
}

impl<'a> Iterator for PartIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.rest.as_bytes();
        if bytes.is_empty() {
            return None;
        }
        let mut i = 0;
        while i < bytes.len() && !bytes[i].is_ascii_digit() && bytes[i] != b'+' && bytes[i] != b'-'
        {
            i += 1;
        }
        if i >= bytes.len() {
            self.rest = "";
            return None;
        }
        let start = i;
        if bytes[i] == b'+' || bytes[i] == b'-' {
            i += 1;
        }
        if i >= bytes.len() || !bytes[i].is_ascii_digit() {
            // 符号だけの断片は `scan` に掛からない
            self.rest = &self.rest[i.min(bytes.len())..];
            return self.next();
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let item = &self.rest[start..i];
        self.rest = &self.rest[i..];
        Some(item)
    }
}

/// Ruby `DeadlineHeroes_Korean#death_chart`。
fn death_chart(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = death_chart_pattern().captures(command) else {
        return Ok(None);
    };

    let chart_type = m[1].to_uppercase();
    let mut base_value = ruby_to_i(&m[2]);
    base_value = -base_value;
    let roll = rng.roll_once(10)?;
    let mut total = base_value + roll;
    if total > 20 {
        total = 20;
    }

    let table: &[&str] = match chart_type.as_str() {
        "L" => DEATH_CHART_L,
        "S" => DEATH_CHART_S,
        "C" => DEATH_CHART_C,
        _ => return Ok(None),
    };
    let result_text = ruby_array_get(table, total).unwrap_or("(결과 없음)");
    // 原典は閉じ括弧を付けない。TOMLもその文字列を期待する。
    Ok(Some(format!("({chart_type} 데스차트 ＞ {result_text}")))
}

static DEATH_CHART_L: &[&str] = &[
    "",
    "",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "아무 일도 일어나지 않는다. 당신은 기적적으로 목숨을 부지했다. 싸움은 계속된다.",
    "격통이 인다. 이후 이벤트 종료 시까지 모든 판정 성공률 -10%.",
    "당신은 [경직] 포인트 2점을 얻는다. [경직] 포인트를 소지하는 동안, 당신은 모든 파워를 사용할 수 없으며 자신의 턴도 얻을 수 없다. 각 라운드 종료 시, 당신이 소지한 [경직] 포인트를 1점 줄여도 좋다.",
    "혼신의 일격!! 당신은 <생존> 판정을 한다. 실패한 경우 [사망] 한다.",
    "당신은 [기절] 포인트를 2점 얻는다. [기절] 포인트를 소지하는 동안, 당신은 모든 파워를 사용할 수 없으며 자신의 턴도 얻을 수 없다. 각 라운드 종료 시, 당신이 소지한 [기절] 포인트를 1점 줄여도 좋다.",
    "이후 이벤트 종료 시까지 모든 판정의 성공률 -20%.",
    "기록적 일격!! 당신은 <생존>-20% 판정을 한다. 실패한 경우 [사망] 한다.",
    "당신은 [빈사] 포인트 2점을 얻는다. [빈사] 포인트를 소지하는 동안, 당신은 모든 파워를 사용할 수 없으며 자신의 턴도 얻을 수 없다. 각 라운드 종료 시, 당신이 소지한 [빈사] 포인트를 1점 잃는다. 모든 [빈사] 포인트를 잃기 전에 전투가 끝나지 않았을 경우, 당신은 [사망] 한다.",
    "서사시적 일격!! 당신은 <생존>-30% 판정을 한다. 실패한 경우 [사망] 한다.",
    "이후 이벤트 종료 시까지 모든 판정의 성공률 -30%.",
    "신화적 일격!! 당신은 하늘을 날아 3회전 정도 한 뒤, 지면에 내리꽂힌다. 눈뜨고 볼 수 없는 무참한 모습. 육체는 원형을 유지하지 못하고, 당신은 [사망] 했다.",
];

static DEATH_CHART_S: &[&str] = &[
    "",
    "",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "아무 일도 일어나지 않는다. 당신은 이를 악물고 스트레스를 버텼다.",
    "이후 이벤트 종료 시까지 모든 판정 성공률 -10%.",
    "당신은 [공포] 포인트 2점을 얻는다. [공포] 포인트를 소지하는 동안, 당신은 [속성:공격]인 파워를 사용할 수 없다. 각 라운드 종료 시, 당신이 소지한 [공포] 포인트를 1점 줄여도 좋다.",
    "매우 상처입었다. 당신은 <의지> 판정을 한다. 실패한 경우 [절망]해 NPC가 된다.",
    "당신은 [기절] 포인트를 2점 얻는다. [기절] 포인트를 소지하는 동안, 당신은 모든 파워를 사용할 수 없으며 자신의 턴도 얻을 수 없다. 각 라운드 종료 시, 당신이 소지한 [기절] 포인트를 1점 줄여도 좋다.",
    "이후 이벤트 종료 시까지 모든 판정의 성공률 -20%.",
    "믿고 있던 사람에게 배신당한 듯한 아픔. 당신은 <의지>-20% 판정을 한다. 실패한 경우 [절망]해 NPC가 된다.",
    "당신은 [혼란] 포인트 2점을 얻는다. [혼란] 포인트를 소지하는 동안, 당신은 원래 동료였던 캐릭터에게, 가능한 한 최대의 피해를 입히도록 행동을 계속한다. 각 라운드 종료 시, 당신이 소지한 [혼란] 포인트를 1점 줄여도 좋다.",
    "너무나도 잔혹한 현실. 당신은 <의지>-30% 판정을 한다. 실패한 경우 [절망]해 NPC가 된다.",
    "이후 이벤트 종료 시까지 모든 판정의 성공률 -30%.",
    "우주의 섭리를 마주하나, 그것은 인류의 인식 한계를 뛰어넘는 무언가였다. 당신은 [절망]해 이후 NPC가 된다.",
];

static DEATH_CHART_C: &[&str] = &[
    "",
    "",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "아무 일도 일어나지 않는다. 당신은 수상한 소문을 불식시켰다.",
    "이후 이벤트 종료 시까지 모든 판정 성공률 -10%.",
    "핀치! 이후 이벤트 종료 시까지 당신은 《지원》을 사용할 수 없다.",
    "배신!! 당신은 <경제> 판정을 한다. 실패한 경우 당신은 히어로로서의 명성을 잃고 [오명]을 뒤집어쓴다.",
    "이후 시나리오 종료 시까지 대가로 크레딧을 소비하는 파워를 사용할 수 없다.",
    "당신의 악평이 상당한 모양이다. 이후 시나리오 종료 시까지 모든 판정의 성공률 -20%.",
    "신뢰의 실추!! 당신은 <경제>-20% 판정을 한다. 실패한 경우 당신은 히어로로서의 명성을 잃고 [오명]을 뒤집어쓴다.",
    "이후 시나리오 종료 시까지 【환경】계열 기능의 레벨이 모두 0이 된다.",
    "날조 보도!! 저지른 적 없는 범죄에 대한 가담이 특종으로 보도된다. 당신은 <경제>-30% 판정을 한다. 실패한 경우 당신은 히어로로서의 명성을 잃고 [오명]을 뒤집어쓴다.",
    "이후 이벤트 종료 시까지 모든 판정의 성공률 -30%.",
    "당신의 이름은 사상 최악의 오점으로 영원히 역사에 기록된다. 더이상 당신을 믿는 동료는 없으며, 당신을 도와줄 사회도 없다. 당신은 [오명]을 뒤집어쓴다.",
];

/// Ruby `combo_pattern`。
static COMBO_PATTERN: &[&str] = &[
    "베이스A+베이스B",
    "베이스B",
    "베이스B×2회",
    "베이스B+베이스C",
    "베이스A+베이스B+베이스C",
    "베이스A+베이스B×2회",
    "베이스B×2회+베이스C",
    "베이스B·오브·베이스B",
    "베이스B·더·베이스B",
];

/// Ruby `DeadlineHeroes_Korean#hero_name_chart`。
fn hero_name_chart(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "HNC" {
        return Ok(None);
    }

    let combo_roll = rng.roll_once(9)?;
    let combo_pattern = ruby_array_get(COMBO_PATTERN, combo_roll - 1).unwrap_or("");
    // 原典は組み合わせに使わないベースも毎回振る。
    let base_a = get_from_table(NameTable::A, rng)?;
    let base_b1 = get_from_table(NameTable::B, rng)?;
    let base_b2 = get_from_table(NameTable::B, rng)?;
    let base_c = get_from_table(NameTable::C, rng)?;
    let result_name = match combo_roll {
        1 => format!("{base_a}{base_b1}"),
        2 => base_b1,
        3 => format!("{base_b1}{base_b2}"),
        4 => format!("{base_b1}{base_c}"),
        5 => format!("{base_a}{base_b1}{base_c}"),
        6 => format!("{base_a}{base_b1}{base_b2}"),
        7 => format!("{base_b1}{base_b2}{base_c}"),
        8 => format!("{base_b1}·오브·{base_b2}"),
        9 => format!("{base_b1}·더·{base_b2}"),
        _ => String::new(),
    };
    Ok(Some(format!(
        "히어로 네임 차트 ＞ 조합식: {combo_pattern} ＞ 결과: {result_name}"
    )))
}

/// Ruby `get_from_table` の種別。
#[derive(Clone, Copy)]
enum NameTable {
    A,
    B,
    C,
    Color,
    Weapon,
    Body,
    Attack,
    Myth,
    Animal,
    Light,
    Bird,
    Etc,
    Str,
    Bug,
}

/// Ruby `DeadlineHeroes_Korean#get_from_table`。
fn get_from_table(kind: NameTable, rng: &mut Randomizer) -> Result<String, EvalError> {
    match kind {
        NameTable::A => match rng.roll_once(10)? {
            1 => Ok("더·".to_owned()),
            2 => Ok("캡틴·".to_owned()),
            3 => Ok(sample(&["미스터·", "미스·", "미세스·"], rng)?.to_owned()),
            4 => Ok(sample(&["닥터·", "프로페서·"], rng)?.to_owned()),
            5 => Ok(sample(&["로드·", "바론·", "제네럴·"], rng)?.to_owned()),
            6 => Ok("맨·오브·".to_owned()),
            7 => get_from_table(NameTable::Str, rng),
            8 => get_from_table(NameTable::Color, rng),
            9 => Ok(sample(&["마담·", "미들·"], rng)?.to_owned()),
            10 => Ok(rng.roll_once(10)?.to_string()),
            _ => Ok(String::new()),
        },
        NameTable::B => match rng.roll_once(10)? {
            1 => get_from_table(NameTable::Myth, rng),
            2 => get_from_table(NameTable::Weapon, rng),
            3 => get_from_table(NameTable::Animal, rng),
            4 => get_from_table(NameTable::Bird, rng),
            5 => get_from_table(NameTable::Bug, rng),
            6 => get_from_table(NameTable::Body, rng),
            7 => get_from_table(NameTable::Light, rng),
            8 => get_from_table(NameTable::Attack, rng),
            9 => get_from_table(NameTable::Etc, rng),
            10 => Ok(rng.roll_once(10)?.to_string()),
            _ => Ok(String::new()),
        },
        NameTable::C => match rng.roll_once(10)? {
            1 => Ok(sample(&["맨", "우먼"], rng)?.to_owned()),
            2 => Ok(sample(&["보이", "걸"], rng)?.to_owned()),
            3 => Ok(sample(&["마스크", "후드"], rng)?.to_owned()),
            4 => Ok("라이더".to_owned()),
            5 => Ok("마스터".to_owned()),
            6 => Ok(sample(&["파이터", "솔저"], rng)?.to_owned()),
            7 => Ok(sample(&["킹", "퀸"], rng)?.to_owned()),
            8 => get_from_table(NameTable::Color, rng),
            9 => Ok(sample(&["히어로", "스페셜"], rng)?.to_owned()),
            10 => Ok(rng.roll_once(10)?.to_string()),
            _ => Ok(String::new()),
        },
        NameTable::Color => Ok(sample(
            &[
                "블랙",
                "그린",
                "블루",
                "옐로",
                "레드",
                "바이올렛",
                "실버",
                "골드",
                "화이트",
                "클리어",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Weapon => Ok(sample(
            &[
                "나이브스",
                "소드",
                "해머",
                "건",
                "스틸",
                "터스크",
                "뉴",
                "애로",
                "소",
                "레이저",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Body => Ok(sample(
            &[
                "하트",
                "페이스",
                "암",
                "숄더",
                "헤드",
                "아이",
                "피스트",
                "핸드",
                "클로",
                "본",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Attack => Ok(sample(
            &[
                "스트로크",
                "크래시",
                "블로",
                "히트",
                "펀치",
                "킥",
                "슬래시",
                "베네트레이트",
                "샷",
                "킬",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Myth => Ok(sample(
            &[
                "아포칼립스",
                "워",
                "이터널",
                "엔젤",
                "데블",
                "이모탈",
                "데스",
                "드림",
                "고스트",
                "데드",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Animal => Ok(sample(
            &[
                "버니",
                "타이거",
                "샤크",
                "캣",
                "콩",
                "도그",
                "폭스",
                "팬서",
                "애스",
                "배트",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Light => Ok(sample(
            &[
                "라이트",
                "섀도우",
                "파이어",
                "다크",
                "나이트",
                "팬텀",
                "토치",
                "플래시",
                "랜턴",
                "선",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Bird => Ok(sample(
            &[
                "호크",
                "팔콘",
                "캐너리",
                "로빈",
                "이글",
                "아울",
                "레이븐",
                "덕",
                "펭귄",
                "피닉스",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Etc => Ok(sample(
            &[
                "휴먼",
                "에이전트",
                "부스터",
                "아이언",
                "선더",
                "워처",
                "풀",
                "머신",
                "콜드",
                "사이드",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Str => Ok(sample(
            &[
                "슈퍼",
                "원더",
                "얼티밋",
                "판타스틱",
                "마이티",
                "인크레더블",
                "어메이징",
                "와일드",
                "그레이티스트",
                "마벨러스",
            ],
            rng,
        )?
        .to_owned()),
        NameTable::Bug => Ok(sample(
            &[
                "비틀",
                "버터플라이",
                "스네이크",
                "엘리게이터",
                "로커스트",
                "리자드",
                "터틀",
                "스파이더",
                "앤트",
                "맨티스",
            ],
            rng,
        )?
        .to_owned()),
    }
}

/// Ruby `DeadlineHeroes_Korean#real_name_chart_overseas`。
fn real_name_chart_overseas(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    if command != "RNCO" {
        return Ok(None);
    }
    if rng.roll_once(10)? == 1 {
        return Ok(Some(
            "무명(모종의 이유로 이름이 없다. 혹은 잃었다)".to_owned(),
        ));
    }
    let first_male = sample(
        &[
            "알버스",
            "크리스",
            "사뮤엘",
            "시드니",
            "스파이크",
            "데미안",
            "딕",
            "덴젤",
            "돈",
            "니콜라스",
            "네빌",
            "발리",
            "빌리",
            "브루스",
            "마브",
            "라이언",
        ],
        rng,
    )?;
    let first_female = sample(
        &[
            "아이리스",
            "올리브",
            "카라",
            "킬스틴",
            "그웬",
            "사만사",
            "저스티나",
            "타바사",
            "나딘",
            "노엘",
            "할린",
            "마르셀라",
            "라나",
            "린디",
            "로잘리",
            "원더",
        ],
        rng,
    )?;
    let last_name = sample(
        &[
            "알렌",
            "워큰",
            "울프먼",
            "오르센",
            "카터",
            "캐러딘",
            "지겔",
            "존스",
            "파커",
            "프리먼",
            "머피",
            "밀러",
            "무어",
            "리브",
            "레이놀즈",
            "워드",
        ],
        rng,
    )?;
    Ok(Some(format!(
        "이름(남):{first_male}\n이름(여):{first_female}\n성:{last_name}"
    )))
}

/// Ruby `DeadlineHeroes_Korean#real_name_chart_jp`。
fn real_name_chart_jp(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "RNCJ" {
        return Ok(None);
    }
    if rng.roll_once(10)? == 1 {
        return Ok(Some(
            "무명(모종의 이유로 이름이 없다. 혹은 잃었다)".to_owned(),
        ));
    }
    let last_name = sample(
        &[
            "아이카와",
            "아마미야",
            "이부키",
            "오가미",
            "카이",
            "사카키",
            "시시도",
            "타치바나",
            "츠부라야",
            "하야카와",
            "하라다",
            "후지카와",
            "호시",
            "미조구치",
            "야시다",
            "유우키",
        ],
        rng,
    )?;
    let first_male = sample(
        &[
            "아키라",
            "에이지",
            "카즈키",
            "긴가",
            "켄이치로",
            "고우",
            "지로",
            "타케시",
            "츠바사",
            "테츠",
            "히데오",
            "마사무네",
            "야마토",
            "류세이",
            "레츠",
            "렌",
        ],
        rng,
    )?;
    let first_female = sample(
        &[
            "안",
            "이노리",
            "에마",
            "카논",
            "사라",
            "시즈쿠",
            "치즈루",
            "나오미",
            "하루",
            "히카루",
            "베니",
            "마치",
            "미아",
            "유리코",
            "루이",
            "레나",
        ],
        rng,
    )?;
    Ok(Some(format!(
        "성:{last_name}\n이름(남):{first_male}\n이름(여):{first_female}"
    )))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/DeadlineHeroes_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DeadlineHeroes_Korean.toml` 의 전 케이스가 통할 것.
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/DeadlineHeroes_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DeadlineHeroes_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            38,
            "case count in test/data/DeadlineHeroes_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DeadlineHeroes:Korean",
                "unexpected game system in DeadlineHeroes_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("DeadlineHeroes:Korean"),
                &tc.input,
                &mut src,
            ) {
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

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL DeadlineHeroes:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DeadlineHeroes:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
