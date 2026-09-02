use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeFlippers;

impl GameSystem for EdgeFlippers {
    fn id(&self) -> &'static str {
        "EdgeFlippers"
    }
    fn name(&self) -> &'static str {
        "EDGE FLIPPERS"
    }
    fn sort_key(&self) -> &'static str {
        "えつしふりつはあす"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
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

static HELP_MESSAGE: &str = r"■ 汎用コマンド
  xHD+ySD=t,t,t
  人間ダイスと怪異ダイスをロールして、特定の出目があるか判定します。
  x: 人間ダイスの数
  y: 怪異ダイスの数
  t: 必要な出目（省略化）

■ 拮抗判定
  xHD+ySD=Xi
  人間ダイスと怪異ダイスをロールして、ゾロ目が必要数あるか判定します。
  i: 必要なゾロ目の個数

■ 全力判定
  xHFD>=t
  xSFD>=t
  x: ダイスの数
  t: 目標値（省略時20）

■ 存在判定
  EXIST -> 2HD+2SD

■ 都市判定
  xHCD -> xHD=1,2,3,4

■ 超常判定
  nSPD -> nSD=7,8,9,10

■ 術式判定
  TD

■ ランダム表
  BAET：【人間】にも【怪異】にも影響のある後遺症
  HAET：【人間寄り】の時に影響のある後遺症
  SAET：【怪異寄り】の時に影響のある後遺症
";
static PREFIXES: &[&str] = &[
    r"\d+HD", r"\d+SD", r"\d+HCD", r"\d+SPD", r"\d+HFD", r"\dSFD", "EXIST", "TD", "BAET", "HAET",
    "SAET",
];

fn hs_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(\d+)HD(?:\+(\d+)SD)?|(\d+)SD(?:\+(\d+)HD)?)(?:=(?:([\d,]+)|X(\d+)))?$")
            .expect("valid regex")
    })
}
fn fullpower_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)([HS]FD)(?:>=(\d+))?$").expect("valid regex"))
}
fn alias_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+)(HCD|SPD)$").expect("valid regex"))
}

fn eval_specific(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_hs(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if command == "EXIST" {
        return Ok(Some(SpecificCommandOutput::result(
            roll_hs("2HD+2SD", rng)?.expect("valid alias"),
        )));
    }
    if let Some(m) = alias_pattern().captures(command) {
        let alias = if &m[2] == "HCD" {
            format!("{}HD=1,2,3,4", &m[1])
        } else {
            format!("{}SD=7,8,9,10", &m[1])
        };
        return Ok(Some(SpecificCommandOutput::result(
            roll_hs(&alias, rng)?.expect("valid alias"),
        )));
    }
    if let Some(result) = roll_fullpower(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if command == "TD" {
        let value = rng.roll_once(10)?;
        let success = value <= 2;
        let mut result = if success {
            EvalResult::success("")
        } else {
            EvalResult::failure("")
        };
        result.text = format!(
            "(TD) ＞ {value} ＞ {}",
            if success { "成功" } else { "失敗" }
        );
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    let Some((title, sides, values)) = table(command) else {
        return Ok(None);
    };
    let number = rng.roll_once(sides)?;
    Ok(Some(SpecificCommandOutput::text(format!(
        "{title}({number}) ＞ {}",
        values[(number - 1) as usize]
    ))))
}

fn roll_hs(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = hs_pattern().captures(command) else {
        return Ok(None);
    };
    let h = number(m.get(1)).max(number(m.get(4)));
    let s = number(m.get(2)).max(number(m.get(3)));
    if h == 0 && s == 0 {
        return Ok(None);
    }
    let targets = m.get(5).map(|v| {
        let mut xs = v
            .as_str()
            .split(',')
            .filter_map(|s| s.parse().ok())
            .collect::<Vec<i64>>();
        xs.sort_unstable();
        xs
    });
    let same = m.get(6).and_then(|v| v.as_str().parse::<usize>().ok());
    let mut hv = rng.roll_barabara(h, 6)?;
    let mut sv = rng.roll_barabara(s, 10)?;
    hv.sort_unstable();
    sv.sort_unstable();
    let mut values = hv.clone();
    values.extend_from_slice(&sv);
    values.sort_unstable();
    let success = targets
        .as_ref()
        .map(|target| is_subset(&values, target))
        .or_else(|| {
            same.map(|needed| {
                let mut counts = HashMap::new();
                values.iter().any(|v| {
                    let count = counts.entry(*v).or_insert(0);
                    *count += 1;
                    *count >= needed
                })
            })
        });
    let body = if h != 0 && s != 0 {
        format!("{h}HD+{s}SD")
    } else if h != 0 {
        format!("{h}HD")
    } else {
        format!("{s}SD")
    };
    let cond = if let Some(targets) = &targets {
        format!(
            "={}",
            targets
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    } else if let Some(same) = same {
        format!("=X{same}")
    } else {
        String::new()
    };
    let dice = [
        (!hv.is_empty()).then(|| {
            format!(
                "D6[{}]",
                hv.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            )
        }),
        (!sv.is_empty()).then(|| {
            format!(
                "D10[{}]",
                sv.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            )
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    let text = format!(
        "({body}{cond}) ＞ {dice}{}",
        success
            .map(|v| format!(" ＞ {}", if v { "成功" } else { "失敗" }))
            .unwrap_or_default()
    );
    let mut result = EvalResult::with_text(text);
    if let Some(success) = success {
        result.set_condition(success);
    }
    Ok(Some(result))
}

fn number(value: Option<regex::Match<'_>>) -> i64 {
    value.and_then(|v| v.as_str().parse().ok()).unwrap_or(0)
}

fn is_subset(superset: &[i64], subset: &[i64]) -> bool {
    let mut i = 0;
    for &value in superset {
        let Some(&wanted) = subset.get(i) else {
            return true;
        };
        if value == wanted {
            i += 1;
        } else if value > wanted {
            return false;
        }
    }
    i == subset.len()
}

fn roll_fullpower(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = fullpower_pattern().captures(command) else {
        return Ok(None);
    };
    let count: i64 = m[1].parse().unwrap_or(0);
    if count == 0 {
        return Ok(None);
    }
    let sides = if &m[2] == "HFD" { 6 } else { 10 };
    let target = m.get(3).and_then(|v| v.as_str().parse().ok()).unwrap_or(20);
    let mut total = 0;
    let mut tries = Vec::new();
    while total < target {
        let mut dice = rng.roll_barabara(count, sides)?;
        dice.sort_unstable();
        total += dice.iter().sum::<i64>();
        tries.push(dice);
    }
    let success = tries.len() < 5;
    let rolls = tries
        .iter()
        .map(|ds| {
            format!(
                "{}[{}]",
                ds.iter().sum::<i64>(),
                ds.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(" ＞ ");
    let text = format!(
        "({count}{}>={target}) ＞ {rolls} ＞ {total}({}回) ＞ {}",
        &m[2],
        tries.len(),
        if success { "成功" } else { "失敗" }
    );
    Ok(Some(if success {
        EvalResult::success(text)
    } else {
        EvalResult::failure(text)
    }))
}

fn table(command: &str) -> Option<(&'static str, i64, &'static [&'static str])> {
    match command {
        "BAET" => Some(("【人間】にも【怪異】にも影響のある後遺症", 12, BAET)),
        "HAET" => Some(("【人間寄り】の時に影響のある後遺症", 6, HAET)),
        "SAET" => Some(("【怪異寄り】の時に影響のある後遺症", 6, SAET)),
        _ => None,
    }
}
static BAET: &[&str] = &[
    "鏡に映らない：鏡や水面に姿が映らなくなる。他者から見えなくなるわけではない",
    "部分的に透明：身体のどこかが透明になる",
    "部分的欠落：身体のどこかが消えて触ることもできなくなる。生命維持に影響はない",
    "角が生える：角が生えてくる。元から角がある場合はさらに追加",
    "しっぽ：動物のしっぽが生える",
    "けもみみ：動物の耳が頭に生える",
    "影がなくなる：自分の影がなくなってしまう",
    "涙が止まらない：ずっと涙が流れ続けて止まらなくなる",
    "視界の端に何かいる：常に視界の端に何かがいて、踊っている",
    "耳鳴り：ずっと耳鳴りがやまない。たまに幻聴がする",
    "美しい幻聴：時々他人の声が現実離れした美しい声に変わって聞こえる",
    "皮膚の下のうごめき：皮膚の下でずっと何かが蠢いているように感じられる",
];
static HAET: &[&str] = &[
    "幻肢の感覚：存在しない追加の腕や翼の感覚がある",
    "声変わり：普段とは違う声になってしまう",
    "怪異の片鱗：【怪異寄り】のときの容姿の一部が【人間寄り】の時にも出てしまう",
    "幻覚：時折やけに生々しい幻覚が見えるようになる",
    "恐怖：あらゆるものが怖く見える",
    "怪異言語：【人間】の領域には存在しない謎の言語が理解できる",
];
static SAET: &[&str] = &[
    "吸精体質：他者に触れると生命力を吸収してしまう。特に仲の良い相手だとより強くなる",
    "威圧する視線：あなたの視線は周囲に強烈な威圧感を与えてしまう",
    "キメラ：あなたの容姿の一部が、他の【越境者】の【怪異寄り】のときの容姿のものと同じになる",
    "異なる怪異：あなた本来の【怪異】の姿とは違う姿に変化してしまう",
    "ポルターガイスト：周囲でラップ音や部品の浮遊といった怪現象が多発する",
    "人間そのもの：【怪異寄り】のときでも人間そのものの容姿になってしまう",
];

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "EdgeFlippers",
            "EdgeFlippers.toml",
            33,
        );
    }
}
