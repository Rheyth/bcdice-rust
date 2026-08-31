//! P4で手書き移植した `lib/bcdice/game_system/TheOneRing2nd.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TheOneRing2nd#rg_command_exec`（判定コマンド `nRG[x][@y][Az][fiwm...]`）
//! - `TheOneRing2nd#fd_command_exec`（表用コマンド `FD[x][fi...]`）

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `SAURONS_EYE_NUMBER`（サウロンの目）。
const SAURONS_EYE_NUMBER: i64 = 11;
/// Ruby `GANDALF_RUNE_NUMBER`（ガンダルフ・ルーン）。
const GANDALF_RUNE_NUMBER: i64 = 12;
/// Ruby `CHOICE_DIE_MARK`（有利/不利で選択されたダイスにつけるマーク）。
const CHOICE_DIE_MARK: &str = "◎";

/// Ruby `module FavouredState` の3値。
///
/// Ruby側は `-98/-99/-100` の整数だが、値そのものは比較にしか使われないので列挙にした。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FavouredState {
    /// Ruby `FavouredState::NORMAL`
    Normal,
    /// Ruby `FavouredState::FAVOURED`
    Favoured,
    /// Ruby `FavouredState::ILLFAVOURED`
    IllFavoured,
}

/// Ruby `OptionData`。
#[derive(Debug, Clone, Copy)]
struct OptionData {
    favoured_state: FavouredState,
    weary: bool,
    miserable: bool,
}

/// Ruby `String#to_i`（先頭の符号つき数字列を読み、無ければ0）。
///
/// `m[2]`（`-?\d*`）は `"-"` や `""` にもなりうるので、`i64::from_str` ではなく
/// Ruby と同じ「読める所まで読む」挙動が要る。
fn ruby_to_i(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let mut i = 0;
    let negative = match bytes.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };

    let start = i;
    let mut value: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        // Ruby は多倍長なので桁あふれしない。ここでは飽和させる
        // （コマンド書式上 `\d{0,2}` や難易度程度しか来ないので実際には起きない）。
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(bytes[i] - b'0'));
        i += 1;
    }

    if i == start {
        return 0;
    }
    if negative {
        -value
    } else {
        value
    }
}

/// Ruby `Array#to_s`（`[1, 2, 6]` のようにカンマ+空白で連結する）。
fn ruby_array_to_s(dice_list: &[i64]) -> String {
    let body = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

/// Ruby `get_adjust_number_text`。
fn get_adjust_number_text(adjust_number: i64) -> String {
    if adjust_number > 0 {
        format!("+{adjust_number}")
    } else if adjust_number < 0 {
        adjust_number.to_string()
    } else {
        String::new()
    }
}

/// Ruby `get_condition_text`。
fn get_condition_text(opts: &OptionData) -> String {
    if opts.favoured_state == FavouredState::Normal && !opts.weary && !opts.miserable {
        return String::new();
    }

    let mut text = "\n状態：".to_owned();
    match opts.favoured_state {
        FavouredState::Favoured => text.push_str("有利 "),
        FavouredState::IllFavoured => text.push_str("不利 "),
        FavouredState::Normal => {}
    }
    if opts.weary {
        text.push_str("疲労 ");
    }
    if opts.miserable {
        text.push_str("絶望 ");
    }

    // Ruby `String#rstrip`
    text.trim_end_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r', '\0'])
        .to_owned()
}

/// Ruby `on_option_switch?`。
fn on_option_switch(opt_value: &str) -> bool {
    // Ruby: opt_value.length == 1（`F` など、スイッチ無し）
    if opt_value.chars().count() == 1 {
        return true;
    }
    // Ruby: opt_value[1..opt_value.length].to_i > 0
    ruby_to_i(&opt_value[1..]) > 0
}

/// Ruby `get_favoured_state`。
fn get_favoured_state(
    option_switch: bool,
    before: FavouredState,
    target: FavouredState,
) -> FavouredState {
    if option_switch {
        if before == target || before == FavouredState::Normal {
            return target;
        }
        return FavouredState::Normal;
    }

    if before == target {
        return FavouredState::Normal;
    }
    before
}

/// Ruby `get_options`。
fn get_options(opt_params: &[&str]) -> OptionData {
    let mut favoured_state = FavouredState::Normal;
    let mut weary = false;
    let mut miserable = false;

    for x in opt_params {
        // Ruby: x[/[WFIM]/]（最初に現れる W/F/I/M の1文字）
        let Some(kind) = x.chars().find(|c| matches!(c, 'W' | 'F' | 'I' | 'M')) else {
            continue;
        };
        match kind {
            'W' => weary = on_option_switch(x),
            'F' => {
                favoured_state =
                    get_favoured_state(on_option_switch(x), favoured_state, FavouredState::Favoured)
            }
            'I' => {
                favoured_state = get_favoured_state(
                    on_option_switch(x),
                    favoured_state,
                    FavouredState::IllFavoured,
                )
            }
            'M' => miserable = on_option_switch(x),
            _ => {}
        }
    }

    OptionData {
        favoured_state,
        weary,
        miserable,
    }
}

/// Ruby `get_specal_die_str`（原典の綴りのまま）。
fn get_specal_die_str(die_number: i64) -> String {
    if die_number == GANDALF_RUNE_NUMBER {
        "ガンダルフ・ルーン".to_owned()
    } else if die_number == SAURONS_EYE_NUMBER {
        "サウロンの目".to_owned()
    } else {
        die_number.to_string()
    }
}

/// Ruby `die_choice`。有利/不利を含めて判定ダイスの結果を選ぶ。
fn die_choice(dice_list: &[i64], favoured_state: FavouredState) -> i64 {
    match favoured_state {
        FavouredState::IllFavoured => {
            if dice_list.contains(&SAURONS_EYE_NUMBER) {
                SAURONS_EYE_NUMBER
            } else {
                // 不利では最小値。`feat_dice_count >= 1` なので必ず要素がある
                dice_list.iter().copied().min().unwrap_or(0)
            }
        }
        FavouredState::Favoured => {
            if dice_list.contains(&GANDALF_RUNE_NUMBER) {
                GANDALF_RUNE_NUMBER
            } else if dice_list
                .iter()
                .filter(|&&d| d == SAURONS_EYE_NUMBER)
                .count()
                == 2
            {
                // どちらもサウロンの目ならサウロンの目
                SAURONS_EYE_NUMBER
            } else {
                // ガンダルフ・ルーンが無ければサウロンの目を除いた最大値
                dice_list
                    .iter()
                    .copied()
                    .filter(|&d| d != SAURONS_EYE_NUMBER)
                    .max()
                    .unwrap_or(0)
            }
        }
        FavouredState::Normal => dice_list[0],
    }
}

/// Ruby `make_featdice_roll` の戻り値 `[feat_result_text, choice_die_number, feat_dice_count]`。
struct FeatDiceRoll {
    result_text: String,
    die_number: i64,
    dice_count: i64,
}

/// Ruby `make_featdice_roll`。
fn make_featdice_roll(
    favoured_state: FavouredState,
    rng: &mut Randomizer,
) -> Result<FeatDiceRoll, EvalError> {
    let feat_dice_count = if favoured_state == FavouredState::Normal {
        1
    } else {
        2
    };
    let dice_list = rng.roll_barabara(feat_dice_count, 12)?;
    let choice_die_number = die_choice(&dice_list, favoured_state);

    let result_text = if feat_dice_count > 1 {
        // Ruby: find_index は最初に一致した位置
        let choice_index = dice_list.iter().position(|&d| d == choice_die_number);
        let mark = |i: usize| {
            if choice_index == Some(i) {
                CHOICE_DIE_MARK
            } else {
                ""
            }
        };
        format!(
            "[{}{}, {}{}]",
            mark(0),
            get_specal_die_str(dice_list[0]),
            mark(1),
            get_specal_die_str(dice_list[1])
        )
    } else {
        format!("[{}]", get_specal_die_str(choice_die_number))
    };

    Ok(FeatDiceRoll {
        result_text,
        die_number: choice_die_number,
        dice_count: feat_dice_count,
    })
}

/// Ruby `make_successdice_roll` の戻り値 `[dice_list.to_s, success_total_number, success_count]`。
struct SuccessDiceRoll {
    result_text: String,
    total_number: i64,
    count: i64,
}

/// Ruby `make_successdice_roll`。
fn make_successdice_roll(
    success_dice_count: i64,
    weary: bool,
    rng: &mut Randomizer,
) -> Result<SuccessDiceRoll, EvalError> {
    let dice_list = rng.roll_barabara(success_dice_count, 6)?;
    let count = dice_list.iter().filter(|&&d| d == 6).count() as i64;
    let total_number: i64 = if weary {
        // Ruby: reject { |i| i <= 3 }.sum（疲労では3以下を数えない）
        dice_list.iter().filter(|&&d| d > 3).sum()
    } else {
        dice_list.iter().sum()
    };

    Ok(SuccessDiceRoll {
        result_text: ruby_array_to_s(&dice_list),
        total_number,
        count,
    })
}

// ---------------------------------------------------------------------------
// FDコマンド
// ---------------------------------------------------------------------------

/// Ruby `/\A(FD)(-?\d*)?([FI]-?\d*)?([FI]-?\d*)?$/`。
fn fd_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(FD)(-?\d*)?([FI]-?\d*)?([FI]-?\d*)?$").expect("valid regex"))
}

/// Ruby `MatchData#[i..]` + `compact`（未マッチのグループを落とす）。
fn captured_options<'a>(m: &Captures<'a>, start: usize) -> Vec<&'a str> {
    (start..m.len())
        .filter_map(|i| m.get(i).map(|g| g.as_str()))
        .collect()
}

/// Ruby `fd_command_exec`。
fn fd_command_exec(command: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let Some(m) = fd_pattern().captures(command) else {
        // Ruby: return ''（`Base#dice_command` が空文字列を nil に畳む）
        return Ok(String::new());
    };

    // Ruby `get_fd_params`
    let adjust_number = ruby_to_i(m.get(2).map_or("", |g| g.as_str()));
    let opts = get_options(&captured_options(&m, 3));

    let feat = make_featdice_roll(opts.favoured_state, rng)?;
    let result_header_text = format!("({}D12", feat.dice_count);

    Ok(get_fd_roll_result(
        &result_header_text,
        &feat.result_text,
        feat.die_number,
        feat.dice_count,
        adjust_number,
    ))
}

/// Ruby `get_fd_adjust`。
fn get_fd_adjust(feat_die_number: i64, adjust_number: i64) -> (i64, String) {
    if feat_die_number == SAURONS_EYE_NUMBER || feat_die_number == GANDALF_RUNE_NUMBER {
        return (feat_die_number, get_adjust_number_text(adjust_number));
    }

    // Ruby: 10より上は10、1より下は1に丸める
    let res_total_num = (feat_die_number + adjust_number).clamp(1, 10);
    (res_total_num, get_adjust_number_text(adjust_number))
}

/// Ruby `get_fd_roll_result`。
fn get_fd_roll_result(
    result_header_text: &str,
    feat_result_text: &str,
    feat_die_number: i64,
    feat_dice_count: i64,
    adjust_number: i64,
) -> String {
    let (reslt_die_number, adjust_number_text) = get_fd_adjust(feat_die_number, adjust_number);

    let header = format!(
        "{result_header_text}{adjust_number_text}) ＞ {feat_result_text}{adjust_number_text}"
    );
    if adjust_number != 0 || feat_dice_count != 1 {
        return format!("{header} ＞ [{}]", get_specal_die_str(reslt_die_number));
    }

    header
}

// ---------------------------------------------------------------------------
// RGコマンド
// ---------------------------------------------------------------------------

/// Ruby `/\A(\d+)(RG)(\d*)(@(\d{0,2}))?(A(-?\d*))?([WFIM]-?\d*)?{4}$/`。
fn rg_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^(\d+)(RG)(\d*)(@(\d{0,2}))?(A(-?\d*))?([WFIM]-?\d*)?([WFIM]-?\d*)?([WFIM]-?\d*)?([WFIM]-?\d*)?$",
        )
        .expect("valid regex")
    })
}

/// Ruby `get_rg_roll_result` の引数一式。
///
/// Ruby はメソッド引数7個で渡すが、clippy の `too_many_arguments` を避けて構造体にした。
struct RgRollContext {
    result_header_text: String,
    difficulty: i64,
    feat_die_number: i64,
    piercing_blows_number: i64,
    total_dice_number: i64,
    success_count: i64,
    opts: OptionData,
}

/// Ruby `rg_command_exec`。
fn rg_command_exec(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = rg_pattern().captures(command) else {
        // Ruby: return ''
        return Ok(None);
    };

    let mut success_count = 0;

    // Ruby `get_rg_params`
    let difficulty = ruby_to_i(&m[1]);
    let success_dice_count = ruby_to_i(&m[3]);
    let adjust_number = ruby_to_i(m.get(7).map_or("", |g| g.as_str()));
    // Ruby: `m[5]&.to_i || -1`。`@` 自体が無ければ -1（痛打判定を行わない）、
    // `@` だけで数字が無ければ `"".to_i` の 0 になる（Ruby では 0 も truthy）。
    let piercing_blows_number = m.get(5).map_or(-1, |g| ruby_to_i(g.as_str()));
    let opts = get_options(&captured_options(&m, 8));

    let feat = make_featdice_roll(opts.favoured_state, rng)?;
    let mut total_dice_number = if feat.die_number != SAURONS_EYE_NUMBER {
        feat.die_number
    } else {
        0
    };

    let mut result_header_text = format!("({}D12", feat.dice_count);
    let mut result_dice_text = feat.result_text.clone();

    if success_dice_count > 0 {
        let success = make_successdice_roll(success_dice_count, opts.weary, rng)?;
        success_count = success.count;
        total_dice_number += success.total_number;

        result_header_text.push_str(&format!("+{success_dice_count}D6"));
        result_dice_text.push_str(&format!("+{}", success.result_text));
    }

    // Ruby `get_rg_adjust`
    total_dice_number += adjust_number;
    if total_dice_number < 0 {
        total_dice_number = 0;
    }
    let adjust_number_text = get_adjust_number_text(adjust_number);

    let result_header_text = format!(
        "{result_header_text}{adjust_number_text}) ＞ {result_dice_text}{adjust_number_text}"
    );

    Ok(Some(get_rg_roll_result(&RgRollContext {
        result_header_text,
        difficulty,
        feat_die_number: feat.die_number,
        piercing_blows_number,
        total_dice_number,
        success_count,
        opts,
    })))
}

/// Ruby `get_rg_roll_result` 内の `piercing_blows` lambda。
fn piercing_blows(
    feat_die_number: i64,
    piercing_blows_number: i64,
    res_text: &str,
    cond_text: &str,
) -> String {
    let mut text = res_text.to_owned();
    if piercing_blows_number > 0
        && feat_die_number >= piercing_blows_number
        && feat_die_number != SAURONS_EYE_NUMBER
    {
        text.push_str(" 痛打発生！");
    }
    text.push_str(cond_text);
    text
}

/// Ruby `get_rg_roll_result`。
fn get_rg_roll_result(ctx: &RgRollContext) -> EvalResult {
    let condition_text = get_condition_text(&ctx.opts);
    let success_count_text = format!("成功度 {}", ctx.success_count);

    if ctx.feat_die_number == GANDALF_RUNE_NUMBER {
        let text = format!("{}：自動成功[{success_count_text}]", ctx.result_header_text);
        let text = piercing_blows(
            ctx.feat_die_number,
            ctx.piercing_blows_number,
            &text,
            &condition_text,
        );
        return if ctx.success_count >= 2 {
            EvalResult::critical(text)
        } else {
            EvalResult::success(text)
        };
    } else if ctx.opts.miserable && ctx.feat_die_number == SAURONS_EYE_NUMBER {
        return EvalResult::failure(format!(
            "{}：自動失敗{condition_text}",
            ctx.result_header_text
        ));
    }

    let result_detail_text = format!("難易度 {} 達成値 {}", ctx.difficulty, ctx.total_dice_number);
    if ctx.difficulty > ctx.total_dice_number {
        return EvalResult::failure(format!(
            "{} {result_detail_text}：失敗{condition_text}",
            ctx.result_header_text
        ));
    }

    let success_text = format!(
        "{} {result_detail_text}：成功[{success_count_text}]",
        ctx.result_header_text
    );
    let success_text = piercing_blows(
        ctx.feat_die_number,
        ctx.piercing_blows_number,
        &success_text,
        &condition_text,
    );
    if ctx.success_count >= 2 {
        EvalResult::critical(success_text)
    } else {
        EvalResult::success(success_text)
    }
}

/// Ruby `/^\d+RG/i`。
fn rg_prefix_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\d+RG").expect("valid regex"))
}

/// Ruby `BCDice::GameSystem::TheOneRing2nd`（ID: `TheOneRing2nd`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheOneRing2nd;

impl GameSystem for TheOneRing2nd {
    fn id(&self) -> &'static str {
        "TheOneRing2nd"
    }

    fn name(&self) -> &'static str {
        "一つの指輪：指輪物語TRPG2版"
    }

    fn sort_key(&self) -> &'static str {
        "ひとつのゆひわゆひわものかたりTRPG2"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(nRG[x][@y][Az][f[0|1]][i[0|1]][w[0|1]][m[0|1]])
 判定用に難易度nを指定して判定ダイスを振る。技量ダイスx、痛打判定値y、修正値zを指定可能。
 技量ダイス、痛打判定値、修正値は0、または未指定（0と同じ）にできる。
 痛打判定値の0、未指定は痛打判定を行わない。
 修正値は判定合計値に加算され、「ガンダルフ・ルーン」や「サウロンの目」はその影響を受けない。
 例1: 13RG     (難易度13 技量ダイス0個)
 例2: 13RG3    (難易度13 技量ダイス3個)
 例3: 13RG3@10A1  (難易度13 技量ダイス3個、痛打判定10、結果に1を加算)

・表用コマンド(FD[x][f[0|1]][i[0|1]])
 表用に判定ダイスを振る。修正値xが指定可能。修正値は0、あるいは未指定(0と同じ)にできる。
 「ガンダルフ・ルーン」や「サウロンの目」は修正値の影響を受けず、値が10を越えることもない。
 例1: FD      (1d12で判定)
 例2: FD1     (1d12で判定し、ダイス目に+1修正)

・コマンドオプション
オプションは、判定コマンドなら4個まで、表用コマンドなら2個まで、順不同で指定可能（重複可）。
  f: 有利(favoured)オプション。不利と同時指定時は相殺。選択された値に◎。
  i: 不利(ill-favoured)オプション。有利と同時指定時は相殺。選択された値に◎。
 例1: 13RG3f   (難易度13 技量ダイス3個、有利)
 例2: FD1f     (1修正、有利)
 例3: 13RG3if   (難易度13 技量ダイス3個、不利、有利)
      ※有利/不利は相殺。

 判定コマンドでは更に下記のオプションを同じ条件で指定可能。
  w: 疲労(weary)状態オプション。
  m: 絶望(miserable)状態オプション。
 例1: 13RG3wf   (難易度13 技量ダイス3個、疲労状態、有利)
 例2: 13RG3fiwm (難易度13 技量ダイス3個、有利、不利、疲労状態、絶望状態)
      ※有利/不利は相殺。最大オプション数である4つを指定。

・オプションスイッチ
 指定したオプションのon/offを1/0で指定可能。1はon、0はoffを表す。
 複数の同じオプションへのスイッチ指定は、最後のスイッチが有効となる。
 例1: 13RG3if0  (難易度13 技量ダイス3個、不利はon、有利はoff)
      ※ 有利指定がoffのため、相殺されず不利となる。
 例2: 13RG3f1f0 (難易度13 技量ダイス3個、有利は最終的にoff)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+RG", "FD"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `TheOneRing2nd#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if rg_prefix_pattern().is_match(command) {
            return Ok(rg_command_exec(command, rng)?
                .map(SpecificCommandOutput::result)
                // Ruby: 正規表現に合わなければ空文字列（`dice_command` が nil に畳む）
                .or_else(|| Some(SpecificCommandOutput::text(""))));
        }
        if command.to_uppercase().starts_with("FD") {
            return Ok(Some(SpecificCommandOutput::text(fd_command_exec(
                command, rng,
            )?)));
        }
        // Ruby: 到達しないはずだが、念のため
        Ok(Some(SpecificCommandOutput::text("Error")))
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
            .join("test/data/TheOneRing2nd.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TheOneRing2nd.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TheOneRing2nd.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TheOneRing2nd.toml must parse");
        assert_eq!(
            data.tests.len(),
            130,
            "case count in test/data/TheOneRing2nd.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TheOneRing2nd",
                "unexpected game system in TheOneRing2nd.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("TheOneRing2nd"), &tc.input, &mut src) {
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
                    "FAIL TheOneRing2nd:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TheOneRing2nd cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
