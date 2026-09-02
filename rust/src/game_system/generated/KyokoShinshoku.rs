//! P4で手書き移植した `lib/bcdice/game_system/KyokoShinshoku.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KyokoShinshoku#eval_game_system_specific_command`
//!   （`#roll_check` → `#roll_kansoku` → `#roll_shusoku`）
//! - `#roll_check_once` / `#format_rolls` と対応表（`DICE_SIZE_TO_SIDES` など）
//!
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `DICE_SIZE_TO_SIDES`（ダイスサイズ → 面数）。
static DICE_SIZE_TO_SIDES: &[(i64, i64)] = &[
    (1, 4),
    (2, 4),
    (3, 4),
    (4, 4),
    (6, 6),
    (8, 8),
    (10, 10),
    (12, 12),
    (20, 20),
];

/// Ruby `GENJITU_KAIRI_TO_SIDES`（［現実乖離］の段階 → 面数）。
static GENJITU_KAIRI_TO_SIDES: &[i64] = &[4, 6, 8, 10, 12, 20];

/// Ruby `REALITY_LINE_TO_TIMES`（［リアリティライン］のレベル → ダイス個数）。
static REALITY_LINE_TO_TIMES: &[(i64, i64)] = &[(3, 1), (2, 2), (1, 3)];

/// Ruby `Hash#[]`（未登録なら `nil`）。
fn lookup(table: &[(i64, i64)], key: i64) -> Option<i64> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Ruby `Array#[]` の添字参照。負の添字は末尾から数える。
///
/// `KR(0)` のように `dice_size` が 0 だと Ruby は `GENJITU_KAIRI_TO_SIDES[-1]`（＝20）を
/// 読むので、その挙動をそのまま再現する。
fn ruby_at(values: &[i64], index: i64) -> Option<i64> {
    let index = if index < 0 {
        index.checked_add(values.len() as i64)?
    } else {
        index
    };
    usize::try_from(index)
        .ok()
        .and_then(|i| values.get(i))
        .copied()
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `roll_check_once` の戻り値 `{dice_list:, value:}`。
struct CheckRoll {
    dice_list: Vec<i64>,
    value: i64,
}

/// Ruby `KyokoShinshoku#roll_check` の判定コマンド正規表現。
///
/// Ruby: `/^KS(?:\(([-+\d]+),([-+\d]+)?\)|(\d+))([AD]?)(?:>=([-+\d]+))?$/`
fn check_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^KS(?:\(([-+\d]+),([-+\d]+)?\)|(\d+))([AD]?)(?:>=([-+\d]+))?$")
            .expect("valid regex")
    })
}

/// Ruby `KyokoShinshoku#roll_kansoku` の `/^KR(?:(\d+)|\((\d),(\d)\))$/`。
fn kansoku_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^KR(?:(\d+)|\((\d),(\d)\))$").expect("valid regex"))
}

/// Ruby `KyokoShinshoku#roll_shusoku` の `/^KRS(?:\((\d),([-+\d]+)\))$/`。
fn shusoku_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^KRS(?:\((\d),([-+\d]+)\))$").expect("valid regex"))
}

/// Ruby `KyokoShinshoku#roll_check_once`。
///
/// ダイス数が1未満のときは2個振って最小、そうでなければ指定数振って最大を取り、
/// `1..dice_size` にクランプする。
fn roll_check_once(
    times: i64,
    dice_size: i64,
    sides: i64,
    rng: &mut Randomizer,
) -> Result<CheckRoll, EvalError> {
    let (dice_list, value) = if times < 1 {
        let mut dice_list = rng.roll_barabara(2, sides)?;
        dice_list.sort_unstable();
        let value = dice_list.iter().copied().min().unwrap_or(0);
        (dice_list, value)
    } else {
        let mut dice_list = rng.roll_barabara(times, sides)?;
        dice_list.sort_unstable();
        let value = dice_list.iter().copied().max().unwrap_or(0);
        (dice_list, value)
    };

    Ok(CheckRoll {
        dice_list,
        // Ruby: value.clamp(1, dice_size)（dice_size は対応表のキーなので必ず1以上）
        value: value.clamp(1, dice_size),
    })
}

/// Ruby `KyokoShinshoku#format_rolls`。
///
/// 1回振りでダイスも1個なら `nil`（＝出力から落とす）。
fn format_rolls(rolls: &[CheckRoll]) -> Option<String> {
    if rolls.len() == 1 && rolls[0].dice_list.len() == 1 {
        return None;
    }

    Some(
        rolls
            .iter()
            .map(|v| {
                if v.dice_list.len() == 1 {
                    v.value.to_string()
                } else {
                    format!(
                        "{}[{}]",
                        v.value,
                        v.dice_list
                            .iter()
                            .map(|d| d.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Ruby `KyokoShinshoku#roll_check`（判定）。
fn roll_check(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = check_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1] ? Arithmetic.eval(m[1], @round_type) : Arithmetic.eval(m[3], @round_type).to_i
    //       後者は `nil.to_i == 0`（対応表に 0 は無いので下で nil に落ちる）。
    let dice_size = match m.get(1) {
        Some(c) => arithmetic::eval(c.as_str(), RoundType::Floor)?,
        None => Some(
            arithmetic::eval(m.get(3).map_or("", |c| c.as_str()), RoundType::Floor)?
                .unwrap_or(I::ZERO),
        ),
    };
    // Ruby: m[2] ? Arithmetic.eval(m[2], @round_type) : 1
    let times = match m.get(2) {
        Some(c) => arithmetic::eval(c.as_str(), RoundType::Floor)?,
        None => Some(I::ONE),
    };
    // Ruby: m[5] && Arithmetic.eval(m[5], @round_type)
    let target = match m.get(5) {
        Some(c) => arithmetic::eval(c.as_str(), RoundType::Floor)?,
        None => None,
    };

    // Ruby: `([AD]?)` なので必ず文字列（空文字列もありうる）
    let advantage = m.get(4).map_or("", |c| c.as_str());

    // Ruby: sides = DICE_SIZE_TO_SIDES[dice_size]（dice_size が nil でも nil）
    let sides = dice_size
        .as_ref()
        .and_then(|size| lookup(DICE_SIZE_TO_SIDES, crate::randomizer::sat_i64(size)));

    // Ruby: return nil if sides.nil? || times.nil?
    let (Some(sides), Some(times), Some(dice_size)) = (sides, times, dice_size) else {
        return Ok(None);
    };

    // Ruby: Array.new(advantage.empty? ? 1 : 2) { roll_check_once(...) }
    let roll_count = if advantage.is_empty() { 1 } else { 2 };
    let mut rolls: Vec<CheckRoll> = Vec::with_capacity(roll_count);
    for _ in 0..roll_count {
        rolls.push(roll_check_once(
            crate::randomizer::sat_i64(&times),
            crate::randomizer::sat_i64(&dice_size),
            sides,
            rng,
        )?);
    }
    let values: Vec<i64> = rolls.iter().map(|v| v.value).collect();

    // ［有利］は大きい方、［不利］は小さい方
    let value = match advantage {
        "A" => values.iter().copied().max().unwrap_or(0),
        "D" => values.iter().copied().min().unwrap_or(0),
        _ => values[0],
    };

    // Ruby: Result.new() は text が nil なので、下の `compact` で落ちる
    let (mut result, has_text) = if value == 1 {
        (EvalResult::fumble("ファンブル"), true)
    } else if target
        .as_ref()
        .is_some_and(|t| value < crate::randomizer::sat_i64(t))
    {
        (EvalResult::failure("失敗"), true)
    } else if target.is_some() && value == sides {
        (EvalResult::critical("クリティカル"), true)
    } else if target
        .as_ref()
        .is_some_and(|t| value >= crate::randomizer::sat_i64(t))
    {
        (EvalResult::success("成功"), true)
    } else {
        (EvalResult::new(), false)
    };

    let head = match target {
        Some(t) => format!(
            "(KS({dice_size},{}){advantage}>={t})",
            crate::randomizer::sat_i64(&times)
        ),
        None => format!(
            "(KS({dice_size},{}){advantage})",
            crate::randomizer::sat_i64(&times)
        ),
    };

    // Ruby: [head, format_rolls(rolls), value, result.text].compact.join(" ＞ ")
    let mut parts: Vec<String> = vec![head];
    if let Some(text) = format_rolls(&rolls) {
        parts.push(text);
    }
    parts.push(value.to_string());
    if has_text {
        parts.push(result.text.clone());
    }
    result.text = parts.join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `KyokoShinshoku#roll_kansoku`（観測ロール）。
fn roll_kansoku(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = kansoku_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[1]&.to_i || m[2].to_i
    let dice_size = match m.get(1) {
        Some(c) => to_i(c.as_str()),
        None => to_i(m.get(2).map_or("", |c| c.as_str())),
    };
    let reality_line = m.get(3).map(|c| to_i(c.as_str()));

    if let Some(line) = reality_line {
        if !(1..=3).contains(&line) {
            return Ok(None);
        }
    }

    let sides = ruby_at(GENJITU_KAIRI_TO_SIDES, dice_size - 1);
    let times = reality_line
        .and_then(|line| lookup(REALITY_LINE_TO_TIMES, line))
        .unwrap_or(1);

    // Ruby: return nil unless sides
    let Some(sides) = sides else {
        return Ok(None);
    };

    let mut dice_list = rng.roll_barabara(times, sides)?;
    dice_list.sort_unstable();
    let value = dice_list.iter().copied().max().unwrap_or(0);

    let cmd = match reality_line {
        Some(line) => format!("KR({dice_size},{line})"),
        None => format!("KR({dice_size})"),
    };

    if times == 1 {
        Ok(Some(format!("({cmd}) ＞ {value}")))
    } else {
        Ok(Some(format!(
            "({cmd}) ＞ {value}[{}] ＞ {value}",
            dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )))
    }
}

/// Ruby `KyokoShinshoku#roll_shusoku`（虚構の収束の侵蝕度減少ロール）。
fn roll_shusoku(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = shusoku_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_size = to_i(&m[1]);
    let times = arithmetic::eval(&m[2], RoundType::Floor)?;

    let sides = ruby_at(GENJITU_KAIRI_TO_SIDES, dice_size - 1);
    // Ruby: return nil if sides.nil? || times.nil?
    let (Some(sides), Some(times)) = (sides, times) else {
        return Ok(None);
    };

    // Ruby側はここだけソートしない
    let dice_list = rng.roll_barabara(crate::randomizer::sat_i64(&times), sides)?;
    let value: i64 = dice_list.iter().sum();

    if times == I::ONE {
        Ok(Some(format!("(KRS({dice_size},{times})) ＞ {value}")))
    } else {
        Ok(Some(format!(
            "(KRS({dice_size},{times})) ＞ {value}[{}] ＞ {value}",
            dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )))
    }
}

/// Ruby `BCDice::GameSystem::KyokoShinshoku`（ID: `KyokoShinshoku`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KyokoShinshoku;

impl GameSystem for KyokoShinshoku {
    fn id(&self) -> &'static str {
        "KyokoShinshoku"
    }

    fn name(&self) -> &'static str {
        "虚構侵蝕TRPG"
    }

    fn sort_key(&self) -> &'static str {
        "きよこうしんしよくTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　ダイスを指定数ダイスロールして、最も高い出目を出力します。難易度を指定すると成否を判定します。オプションでA、Dをつけると、［有利］［不利］の条件で振れます（A=［有利］、D=［不利］）。
KS(x,y)
x：ダイスサイズ。1=D4（能力値1、2以上の出目が出ていたとしても最大1）／2=D4（能力値2、3以上の出目が出ていたとしても最大2）／3=D4（能力値3、出目4が出ていたとしても最大3）／4=D4／6=D6／8=D8／10=D10／12=D12／20=D20
y：ダイス数（省略：1）

KS(x,y)>=z
x：ダイスサイズ。1=D4（能力値1、2以上の出目が出ていたとしても最大1）／2=D4（能力値2、3以上の出目が出ていたとしても最大2）／3=D4（能力値3、出目4が出ていたとしても最大3）／4=D4／6=D6／8=D8／10=D10／12=D12／20=D20
y：ダイス数（省略：1）
z：難易度

KS(x,y)A>=z（［有利］：KS(x,y)の判定を２回行い、それぞれの結果のより大きい方が結果となります）
x：ダイスサイズ。1=D4（能力値1、2以上の出目が出ていたとしても最大1）／2=D4（能力値2、3以上の出目が出ていたとしても最大2）／3=D4（能力値3、出目4が出ていたとしても最大3）／4=D4／6=D6／8=D8／10=D10／12=D12／20=D20
y：ダイス数（省略：1）
z：難易度

KS(x,y)D>=z（［不利］：KS(x,y)の判定を２回行い、それぞれの結果のより小さい方が結果となります）
x：ダイスサイズ。1=D4（能力値1、2以上の出目が出ていたとしても最大1）／2=D4（能力値2、3以上の出目が出ていたとしても最大2）／3=D4（能力値3、出目4が出ていたとしても最大3）／4=D4／6=D6／8=D8／10=D10／12=D12／20=D20
y：ダイス数（省略：1）
z：難易度

・観測ロール
　［現実乖離］の段階に応じたダイスを指定数ダイスロールして、最も高い出目を出力します。
KR(x)
x=［現実乖離］の段階（1=D4／2=D6／3=D8／4=D10／5=D12／6=D20）

KR(x,y)　観測ロール（リアリティラインあり）
x=［現実乖離］の段階（1=D4／2=D6／3=D8／4=D10／5=D12／6=D20）
y=［リアリティライン］のレベル（3=1個／2=2個／1=3個）

・虚構の収束の侵蝕度減少ロール
　［現実乖離］の段階に応じたダイスを指定数ダイスロールして、その合計を出力します。
KRS(x,y)
x=［現実乖離］の段階（1=D4／2=D6／3=D8／4=D10／5=D12／6=D20）
y=ダイスの個数
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["KS", "KR", "KRS"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `KyokoShinshoku#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_check(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = roll_kansoku(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(roll_shusoku(command, rng)?.map(SpecificCommandOutput::text))
    }
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
            .join("test/data/KyokoShinshoku.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/KyokoShinshoku.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/KyokoShinshoku.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KyokoShinshoku.toml must parse");
        assert_eq!(
            data.tests.len(),
            66,
            "case count in test/data/KyokoShinshoku.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KyokoShinshoku",
                "unexpected game system in KyokoShinshoku.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KyokoShinshoku"), &tc.input, &mut src) {
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
                    "FAIL KyokoShinshoku:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KyokoShinshoku cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
