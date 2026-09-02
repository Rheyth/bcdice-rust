//! P4で手書き移植した `lib/bcdice/game_system/Skynauts.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command` → 判定 / 航行 / ダメージ / 砲撃 / 回避
//! - `#get_judge_result`（`2D6<=n` / `SNn`）
//! - `#navigation_result`（`NV+n`）
//! - `#get_fire_result` / `#get_fire_point` / `#get_fire_point_text`
//! - `#get_bomb_result`（`BOMn/D...`）
//! - `#get_avoid_result` / `#scan_fire_point`（`AVOn@m` + 座標）
//!
//! 括弧式（`SN(3+2)` / `2D6<=(6+2-1)` / `AVO(4+2)@6`）は Preprocessor が
//! eval 前に畳む。ここでの正規表現は展開後の `SN5` / `2D6<=7` / `AVO6@6` を見る。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby 側の正規表現を `OnceLock` に載せる。
macro_rules! re {
    ($name:ident, $pattern:literal) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect("valid regex"))
        }
    };
}

// Ruby: `/^2D6<=(\d)$/i` — 目標値は1桁のみ。`2D6<=10` は固有コマンドにならない。
re!(judge_2d6_re, r"(?i)^2D6<=(\d)$");
// Ruby: `/^SN(\d*)$/i` — 空（既定7）と複数桁（`SN10`）を許す。
re!(judge_sn_re, r"(?i)^SN(\d*)$");
// Ruby: `/^NV(\+(\d+))?$/`（`/i` なし。マイナスはマッチしない）
re!(navigation_re, r"^NV(\+(\d+))?$");
// Ruby: `%r{^D([12346789]*)(\[.+\])*/(\d{1,2})(@([2468]))?$}`
// `(\[.+\])*` は `[火災]` / `[大揺れ]` を読み飛ばすだけで座標には使わない。
re!(fire_re, r"^D([12346789]*)(\[.+\])*/(\d{1,2})(@([2468]))?$");
// Ruby: `%r{^BOM(\d*)?/D([12346789]*)(\[.+\])*/(\d+)(@([2468]))?$}i`
re!(
    bomb_re,
    r"(?i)^BOM(\d*)?/D([12346789]*)(\[.+\])*/(\d+)(@([2468]))?$"
);
// Ruby: `command.slice(%r{D([12346789]*)(\[.+\])*/(\d+)(@([2468]))?})`
re!(fire_slice_re, r"D([12346789]*)(\[.+\])*/(\d+)(@([2468]))?");
// Ruby: `/^AVO(\d*)?(@([2468]))(\(?\[縦\d+,横\d+\]\)?,?)+$/`
re!(
    avoid_re,
    r"^AVO(\d*)?(@([2468]))(\(?\[縦\d+,横\d+\]\)?,?)+$"
);
// Ruby: `command.slice(/^AVO(\d*)?(@([2468]))/)`
re!(avoid_judge_re, r"^AVO(\d*)?(@([2468]))");
// Ruby: `command.slice(/(\(?\[縦\d+,横\d+\]\)?,?)+/)`
re!(avoid_point_re, r"(\(?\[縦\d+,横\d+\]\)?,?)+");
// Ruby: `/[^\d]*(\d+),[^\d]*(\d+)/`（縦=y, 横=x）
re!(scan_point_re, r"[^\d]*(\d+),[^\d]*(\d+)");

/// Ruby `BCDice::GameSystem::Skynauts`（ID: `Skynauts`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skynauts;

impl GameSystem for Skynauts {
    fn id(&self) -> &'static str {
        "Skynauts"
    }

    fn name(&self) -> &'static str {
        "歯車の塔の探空士（六畳間幻想空間）"
    }

    fn sort_key(&self) -> &'static str {
        "はくるまのとうのすかいのおつ"
    }

    fn help_message(&self) -> &'static str {
        r"◆判定　(SNn)、(2D6<=n)　n:目標値（省略時:7）
　例）SN5　SN5　SN(3+2)
◆航行チェック　(NV+n)　n:修正値（省略時:0）
　例）NV　NV+1
◆ダメージチェック　(Dx/y@m)　x:ダメージ左側の値、y:ダメージ右側の値
　m:《弾道学》（省略可）上:8、下:2、左:4、右:6
　飛空艇シート外の座標は()が付きます。
　例） D/4　D19/2　D/3@8　D[大揺れ]/2
◆砲撃判定+ダメージチェック　(BOMn/Dx/y@m)　n:目標値（省略時:7）
　x:ダメージ左側の値、y:ダメージ右側の値
　m:《弾道学》（省略可）上:8、下:2、左:4、右:6
　例） BOM/D/4　BOM9/D19/2@4
◆《回避運動》　(AVOn@mXX)　n:目標値（省略時:7）
　m:回避方向。上:8、下:2、左:4、右:6、XX：ダメージチェック結果
　例）
　AVO9@8[縦1,横4],[縦2,横6],[縦3,横8]　AVO@2[縦6,横4],[縦2,横6]
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["D", "2D6<=", "SN", "NV", "AVO", "BOM"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Skynauts#eval_game_system_specific_command`。
    ///
    /// `round_type` は Ruby が `FLOOR` を明示しているが既定値なので上書きしない。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: get_judge_result || navigation_result || get_fire_result ||
        //       get_bomb_result || get_avoid_result
        if let Some(result) = get_judge_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = navigation_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = get_fire_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = get_bomb_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = get_avoid_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `String#to_i`（空文字は 0）。桁あふれは `i64::MAX` へ飽和。
fn to_i(digits: &str) -> i64 {
    if digits.is_empty() {
        0
    } else {
        digits.parse::<i64>().unwrap_or(i64::MAX)
    }
}

/// キャプチャが無い／空なら `""`。Ruby `m[n].to_s`（`nil.to_s == ""`）。
fn cap<'a>(caps: &'a Captures<'a>, index: usize) -> &'a str {
    caps.get(index).map(|m| m.as_str()).unwrap_or("")
}

/// `SN` / `2D6<=` の目標値。空なら 7（Ruby `m[1].empty? ? 7 : m[1].to_i`）。
fn judge_target(raw: &str) -> i64 {
    if raw.is_empty() {
        7
    } else {
        to_i(raw)
    }
}

/// Ruby `Skynauts#get_judge_result`。
fn get_judge_result(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let target = if let Some(caps) = judge_2d6_re().captures(command) {
        judge_target(cap(&caps, 1))
    } else if let Some(caps) = judge_sn_re().captures(command) {
        judge_target(cap(&caps, 1))
    } else {
        return Ok(None);
    };

    Ok(Some(roll_judge(target, rng)?))
}

/// 判定本体。出目はソートしない（`sort_add_dice` 既定 false）。
///
/// ファンブル（合計≦2）は目標値≦2 の成功より優先。
fn roll_judge(target: i64, rng: &mut Randomizer) -> Result<EvalResult, EvalError> {
    let dice_list = rng.roll_barabara(2, 6)?;
    let total: i64 = dice_list.iter().copied().sum();
    let text = format!(
        "(2D6<={target}) ＞ {total}[{}] ＞ {total}",
        dice_text::join_dice(&dice_list)
    );
    if total <= 2 {
        Ok(EvalResult::fumble(format!("{text} ＞ ファンブル")))
    } else if total <= target {
        Ok(EvalResult::success(format!("{text} ＞ 成功")))
    } else {
        Ok(EvalResult::failure(format!("{text} ＞ 失敗")))
    }
}

/// Ruby `Skynauts#navigation_result`。
fn navigation_result(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = navigation_re().captures(command) else {
        return Ok(None);
    };

    // Ruby: m[2].to_i（無いときは nil.to_i == 0）。マイナスは正規表現が弾く。
    let bonus = to_i(cap(&caps, 2));
    let total = rng.roll_once(6)?;
    // Ruby Integer#/ は床除算。出目 1..6 なので Rust の `/` と同じ。
    let half = total / 2;
    let move_point_base = if half <= 0 { 1 } else { half };
    let move_point = move_point_base + bonus;

    // `(最低1)` の直後は全角スペース。` /2+` の前にも半角スペースがある。
    Ok(Some(EvalResult::with_text(format!(
        "航行チェック(最低1)　(1D6/2+{bonus}) ＞ {total} /2+{bonus} ＞ {move_point_base}+{bonus} ＞ {move_point}エリア進む"
    ))))
}

/// Ruby `DIRECTION_INFOS`。5（中央）は載せていない。
fn direction_info(direction: i64) -> Option<(&'static str, i64, i64)> {
    match direction {
        1 => Some(("左下", -1, 1)),
        2 => Some(("下", 0, 1)),
        3 => Some(("右下", 1, 1)),
        4 => Some(("左", -1, 0)),
        6 => Some(("右", 1, 0)),
        7 => Some(("左上", -1, -1)),
        8 => Some(("上", 0, -1)),
        9 => Some(("右上", 1, -1)),
        _ => None,
    }
}

/// Ruby `get_direction_info(direction, :name, "")`。
fn direction_name(direction: i64) -> &'static str {
    direction_info(direction)
        .map(|(name, _, _)| name)
        .unwrap_or("")
}

/// Ruby `get_direction_info(direction, :position_diff, {})` の x/y（欠けると 0）。
fn position_diff(direction: i64) -> (i64, i64) {
    direction_info(direction)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((0, 0))
}

/// Ruby `Skynauts#get_move_point`。
fn get_move_point(x: i64, y: i64, direction: i64) -> (i64, i64) {
    let (dx, dy) = position_diff(direction);
    (x + dx, y + dy)
}

/// Ruby `Skynauts#in_map_position?`。
fn in_map_position(x: i64, y: i64) -> bool {
    (1..=6).contains(&y) && (2..=12).contains(&x)
}

/// 着弾点の1グループ。各要素は `[x, y]`（横, 縦）。
type FireGroup = Vec<(i64, i64)>;

/// Ruby `Skynauts#get_fire_result`。フラグは付かない（`Result.new`）。
fn get_fire_result(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = fire_re().captures(command) else {
        return Ok(None);
    };

    let fire_range = cap(&caps, 1);
    let fire_count = to_i(cap(&caps, 3));
    let ballistics = to_i(cap(&caps, 5));

    let fire_point = get_fire_point(fire_range, fire_count, rng)?;
    let mut parts = vec![command.to_owned(), get_fire_point_text(&fire_point, 0)];

    if ballistics != 0 {
        parts.push(format!("《弾道学》:{}\n", direction_name(ballistics)));
        parts.push(get_fire_point_text(&fire_point, ballistics));
    }

    Ok(Some(EvalResult::with_text(parts.join(" ＞ "))))
}

/// Ruby `Skynauts#get_fire_point`。
///
/// 範囲オフセットは毎回 **元の** `(x_pos, y_pos)` に足す（直前の着弾点へは連鎖しない）。
fn get_fire_point(
    fire_range: &str,
    fire_count: i64,
    rng: &mut Randomizer,
) -> Result<Vec<FireGroup>, EvalError> {
    let mut fire_point = Vec::new();

    for _ in 0..fire_count {
        let y_pos = rng.roll_once(6)?;
        let x_pos = rng.roll_sum(2, 6)?;
        let mut group = vec![(x_pos, y_pos)];

        for range_text in fire_range.chars() {
            let direction = to_i(&range_text.to_string());
            let (dx, dy) = position_diff(direction);
            group.push((x_pos + dx, y_pos + dy));
        }

        fire_point.push(group);
    }

    Ok(fire_point)
}

/// Ruby `Skynauts#get_fire_point_text`（`.text` だけ使うので String を返す）。
///
/// グループ内は区切りなし連結、グループ間はカンマ（空白なし）。
fn get_fire_point_text(fire_point: &[FireGroup], direction: i64) -> String {
    let mut fire_text_list = Vec::with_capacity(fire_point.len());

    for point in fire_point {
        let mut text = String::new();
        for &(x, y) in point {
            let (x, y) = get_move_point(x, y, direction);
            if in_map_position(x, y) {
                text.push_str(&format!("[縦{y},横{x}]"));
            } else {
                text.push_str(&format!("([縦{y},横{x}])"));
            }
        }
        fire_text_list.push(text);
    }

    fire_text_list.join(",")
}

/// Ruby `Skynauts#get_bomb_result`。
///
/// 失敗（ファンブル含む。Ruby `Result#failure?`）ならダメージチェックをしない。
/// 成功時のフラグは SN 判定の success のまま。
fn get_bomb_result(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = bomb_re().captures(command) else {
        return Ok(None);
    };

    let target = cap(&caps, 1);
    let mut sn = get_judge_result(&format!("SN{target}"), rng)?
        .expect("SN + BOM target always matches get_judge_result");

    if sn.failure {
        sn.text = format!("{command} ＞ {}", sn.text);
        return Ok(Some(sn));
    }

    let fire_command = fire_slice_re()
        .find(command)
        .map(|m| m.as_str())
        .expect("BOM match guarantees a D... slice");
    let fire = get_fire_result(fire_command, rng)?.expect("sliced D... matches get_fire_result");
    sn.text = format!("{command} ＞ {}\n ＞ {}", sn.text, fire.text);
    Ok(Some(sn))
}

/// Ruby `Skynauts#get_avoid_result`。
fn get_avoid_result(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(caps) = avoid_re().captures(command) else {
        return Ok(None);
    };

    let direction = to_i(cap(&caps, 3));
    let judge_command = avoid_judge_re()
        .find(command)
        .map(|m| m.as_str())
        .expect("AVO match guarantees the judge prefix");
    // Ruby: get_judge_result("SN" + Regexp.last_match(1).to_s)
    let judge_caps = avoid_judge_re()
        .captures(command)
        .expect("AVO match guarantees the judge prefix");
    let mut sn = get_judge_result(&format!("SN{}", cap(&judge_caps, 1)), rng)?
        .expect("SN + AVO target always matches get_judge_result");

    if sn.failure {
        sn.text = format!("{judge_command} ＞ 《回避運動》{}", sn.text);
        return Ok(Some(sn));
    }

    let point_command = avoid_point_re()
        .find(command)
        .map(|m| m.as_str())
        .expect("AVO match guarantees coordinate text");
    let fire_point = scan_fire_point(point_command);

    Ok(Some(EvalResult::success(
        [
            judge_command,
            &format!("《回避運動》{}\n", sn.text),
            point_command,
            &format!("《回避運動》:{}\n", direction_name(direction)),
            &get_fire_point_text(&fire_point, direction),
        ]
        .join(" ＞ "),
    )))
}

/// Ruby `Skynauts#scan_fire_point`。
///
/// `],` で砲撃1発（グループ）に分け、`]` で着弾点に分ける。
fn scan_fire_point(command: &str) -> Vec<FireGroup> {
    // Ruby: command.gsub(/\(|\)/, "")
    let command = command.replace(['(', ')'], "");
    let mut fire_point = Vec::new();

    for point_text in command.split("],") {
        let mut group = FireGroup::new();
        for point in point_text.split(']') {
            let Some(caps) = scan_point_re().captures(point) else {
                continue;
            };
            let y = to_i(&caps[1]);
            let x = to_i(&caps[2]);
            group.push((x, y));
        }
        fire_point.push(group);
    }

    fire_point
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Skynauts", "Skynauts.toml", 47);
    }
}
