//! P4で手書き移植した `lib/bcdice/game_system/YearZeroEngine.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `resolute_action`（ダイスプール判定 `nYZEx+x+x±m`）
//! - `resolute_push_action`（プッシュ判定 `nMYZx±x+x`）
//! - `resolute_step_action` / `get_rolling_dice` / `make_dice_a_roll`
//!   （ステップダイス判定 `nYZSx+x±m[A|D]`）
//!
//! ロケール差は定型文だけなので [`SystemStrings`] に束ね、
//! `YearZeroEngine_Korean`（`ko_kr`）が同じ関数群を使い回す
//! （Ruby側で `YearZeroEngine_Korean < YearZeroEngine` なのに対応する）。
//!
//! 定型文は `i18n/YearZeroEngine/ja_jp.yml` から書き写したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::{Captures, Regex};

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::YearZeroEngine`（ID: `YearZeroEngine`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearZeroEngine;

impl GameSystem for YearZeroEngine {
    fn id(&self) -> &'static str {
        "YearZeroEngine"
    }

    fn name(&self) -> &'static str {
        "YearZeroEngine"
    }

    fn sort_key(&self) -> &'static str {
        "いやあせろえんしん"
    }

    fn help_message(&self) -> &'static str {
        r"・ダイスプール判定コマンド(nYZEx+x+x+m)
  (難易度)YZE(能力ダイス数)+(技能ダイス数)+(アイテムダイス数)+(修正値)  # (6のみを数える)
  (難易度)YZE(能力ダイス数)+(技能ダイス数)+(アイテムダイス数)-(修正値)  # (6のみを数える)

・ダイスプール判定コマンド(nMYZx+x+x)
  (難易度)MYZ(能力ダイス数)+(技能ダイス数)+(アイテムダイス数)  # (1と6を数え、プッシュ可能数を表示)
  (難易度)MYZ(能力ダイス数)-(技能ダイス数)+(アイテムダイス数)  # (1と6を数え、プッシュ可能数を表示、技能のマイナス指定)

  ※ 難易度と技能、アイテムダイス数は省略可能

・ステップダイス判定コマンド(nYZSx+x+m+f)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)+(修正値)   # (1,6を数え、プッシュ可能数を表示)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)-(修正値)   # (1,6を数え、プッシュ可能数を表示)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)+(修正値)A  # (1,6を数え、プッシュ可能数を表示、有利)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)-(修正値)A  # (1,6を数え、プッシュ可能数を表示、有利)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)+(修正値)D  # (1,6を数え、プッシュ可能数を表示、不利)
  (難易度)YZS(能力ダイス面数)+(技能ダイス面数)-(修正値)D  # (1,6を数え、プッシュ可能数を表示、不利)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"(\d+)?(YZE|MYZ|YZS)"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_STRINGS, command, rng)
    }
}

// ---------------------------------------------------------------------------
// ロケールごとの定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の定型文。`YearZeroEngine` と `YearZeroEngine_Korean` はこれだけが違う。
///
/// フィールド名は i18n キー（`YearZeroEngine.*`）に対応する。
pub(crate) struct SystemStrings {
    /// i18n `YearZeroEngine.success_count`
    pub(crate) success_count: &'static str,
    /// i18n `YearZeroEngine.difficulty`
    pub(crate) difficulty: &'static str,
    /// i18n `YearZeroEngine.success_msg`
    pub(crate) success_msg: &'static str,
    /// i18n `YearZeroEngine.failure_msg`
    pub(crate) failure_msg: &'static str,
    /// i18n `YearZeroEngine.roll_one`
    pub(crate) roll_one: &'static str,
    /// i18n `YearZeroEngine.ability`
    pub(crate) ability: &'static str,
    /// i18n `YearZeroEngine.skill`
    pub(crate) skill: &'static str,
    /// i18n `YearZeroEngine.item`
    pub(crate) item: &'static str,
    /// i18n `YearZeroEngine.pushable`
    pub(crate) pushable: &'static str,
    /// i18n `YearZeroEngine.dice`
    pub(crate) dice: &'static str,
}

/// `ja_jp` ロケールの定型文（`i18n/YearZeroEngine/ja_jp.yml`）。
static JA_STRINGS: SystemStrings = SystemStrings {
    success_count: "成功数",
    difficulty: "難易度",
    success_msg: "判定成功！",
    failure_msg: "判定失敗！",
    roll_one: "出目1",
    ability: "能力",
    skill: "技能",
    item: "アイテム",
    pushable: "プッシュ可能",
    dice: "ダイス",
};

// ---------------------------------------------------------------------------
// 判定の途中経過
// ---------------------------------------------------------------------------

/// Ruby `dice_info_init` が初期化するインスタンス変数。
///
/// Ruby の `@total_botch_dice` は加算されるだけで出力にも分岐にも使われないため移植していない
/// （`@base_botch_dice` / `@skill_botch_dice` / `@gear_botch_dice` の合計に等しい）。
#[derive(Debug, Default, Clone, Copy)]
struct DiceInfo {
    /// Ruby `@total_success_dice`
    total_success_dice: i64,
    /// Ruby `@base_botch_dice`（能力ダメージ）
    base_botch_dice: i64,
    /// Ruby `@skill_botch_dice`
    skill_botch_dice: i64,
    /// Ruby `@gear_botch_dice`（ギアダメージ）
    gear_botch_dice: i64,
    /// Ruby `@push_dice`
    push_dice: i64,
    /// Ruby `@difficulty`
    difficulty: i64,
}

impl DiceInfo {
    /// Ruby `make_result_with_yze`。
    fn make_result_with_yze(
        &self,
        strings: &SystemStrings,
        dice_count_text: &str,
        dice_text: &str,
    ) -> SpecificCommandOutput {
        let result_text = format!(
            "{dice_count_text} ＞ {dice_text} {}:{}",
            strings.success_count, self.total_success_dice
        );

        if self.difficulty > 0 {
            let head = format!("{result_text} {}={}:", strings.difficulty, self.difficulty);
            return if self.total_success_dice >= self.difficulty {
                SpecificCommandOutput::result(EvalResult::success(format!(
                    "{head}{}",
                    strings.success_msg
                )))
            } else {
                SpecificCommandOutput::result(EvalResult::failure(format!(
                    "{head}{}",
                    strings.failure_msg
                )))
            };
        }

        SpecificCommandOutput::text(result_text)
    }

    /// Ruby `make_result_with_myz`。
    ///
    /// `atter_text` の `[` と `)` の対応が取れていないのは原典どおり
    /// （TOMLの期待値もこの形）。
    fn make_result_with_myz(
        &self,
        strings: &SystemStrings,
        dice_count_text: &str,
        dice_text: &str,
    ) -> SpecificCommandOutput {
        let result_text = format!(
            "{dice_count_text} ＞ {dice_text} {}:{}",
            strings.success_count, self.total_success_dice
        );
        let atter_text = format!(
            "\n{}：[{}：{},{}：{},{}：{}) {}={}{}",
            strings.roll_one,
            strings.ability,
            self.base_botch_dice,
            strings.skill,
            self.skill_botch_dice,
            strings.item,
            self.gear_botch_dice,
            strings.pushable,
            self.push_dice,
            strings.dice,
        );

        if self.difficulty > 0 {
            let head = format!("{result_text} {}={}:", strings.difficulty, self.difficulty);
            return if self.total_success_dice >= self.difficulty {
                SpecificCommandOutput::result(EvalResult::success(format!(
                    "{head}{}{atter_text}",
                    strings.success_msg
                )))
            } else {
                SpecificCommandOutput::result(EvalResult::failure(format!(
                    "{head}{}{atter_text}",
                    strings.failure_msg
                )))
            };
        }

        SpecificCommandOutput::text(format!("{result_text}{atter_text}"))
    }

    /// Ruby `make_dice_a_roll(count, type)`（ステップダイス用）。
    ///
    /// 戻り値は Ruby と同じく `[出目の文字列, botch_dice]`。
    /// `success_level` は10以上の目を二重に数えるので、6以上1個で+1、10以上なら+2になる。
    fn make_dice_a_roll(
        &mut self,
        count: i64,
        sides: i64,
        rng: &mut Randomizer,
    ) -> Result<(String, i64), EvalError> {
        let dice_list = rng.roll_barabara(count, sides)?;
        let botch_dice = count_matching(&dice_list, |v| v == 1);
        let success_dice = count_matching(&dice_list, |v| v >= 6);
        let success_level = success_dice + count_matching(&dice_list, |v| v >= 10);

        self.total_success_dice += success_level;
        self.push_dice += count - (success_dice + botch_dice);

        Ok((
            format!("[{}]", dice_text::join_dice(&dice_list)),
            botch_dice,
        ))
    }
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `YearZeroEngine#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    strings: &SystemStrings,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(output) = resolute_action(strings, command, rng)? {
        return Ok(Some(output));
    }
    if let Some(output) = resolute_push_action(strings, command, rng)? {
        return Ok(Some(output));
    }
    resolute_step_action(strings, command, rng)
}

/// Ruby `resolute_action` の正規表現。
///
/// 捕獲グループと Ruby の定数の対応:
/// `1`=`DIFFICULTY_INDEX` / `2`=`COMMAND_TYPE_INDEX` / `3`=`ABILITY_INDEX` /
/// `5`=`SKILL_SIGNED_INDEX` / `6`=`SKILL_INDEX` / `8`=`GEAR_INDEX` /
/// `10`=`MODIFIER_SIGNED_INDEX` / `11`=`MODIFIER_INDEX`。
fn yze_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(\d+)?(YZE)(\d+)((\+)(\d+))?(\+(\d+))?((\+|-)(\d+))?").expect("valid regex")
    })
}

/// Ruby `resolute_push_action` の正規表現。
///
/// グループ番号は `5`=`SKILL_SIGNED_INDEX` / `6`=`SKILL_INDEX` / `8`=`GEAR_INDEX` で
/// [`yze_pattern`] と共通だが、修正値のグループは持たない。
fn myz_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(\d+)?(MYZ)(\d+)((\+|-)(\d+))?(\+(\d+))?").expect("valid regex")
    })
}

/// Ruby `resolute_step_action` の正規表現。
///
/// **グループ番号が YZE/MYZ と違う**。Ruby側も `MODIFIER_*_INDEX` 定数を使わず
/// `m[7]`（修正値のグループ全体。`"+3"` / `"-2"` の形で符号込み）と
/// `m[10]`（`A` / `D`）をリテラルで参照している。
fn yzs_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\A(\d+)?(YZS)(\d+)((\+)(\d+))?((\+|-)(\d+))?(A|D)?").expect("valid regex")
    })
}

/// Ruby `resolute_action`（ダイスプール判定 `nYZEx+x+x±m`）。
fn resolute_action(
    strings: &SystemStrings,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = yze_pattern().captures(command) else {
        return Ok(None);
    };

    let mut info = DiceInfo {
        difficulty: group_to_i(&m, 1),
        ..DiceInfo::default()
    };

    let mut attribute = group_to_i(&m, 3);
    let mut skill = group_to_i(&m, 6);
    let mut gear = group_to_i(&m, 8);
    let mut modifier = group_to_i(&m, 11);

    if m.get(10).map(|x| x.as_str()) == Some("-") {
        // マイナス修正は 技能 → アイテム → 能力 の順に削る
        if skill >= modifier {
            skill -= modifier;
        } else {
            modifier -= skill;
            skill = 0;
            if gear >= modifier {
                gear -= modifier;
            } else {
                modifier -= gear;
                gear = 0;
                if attribute >= modifier {
                    attribute -= modifier;
                } else {
                    attribute = 0;
                }
            }
        }
    } else {
        // 技能ダイスが省略されていても加算される（その場合ダイスは振られず捨てられる）
        skill += modifier;
    }

    let dice_pool = attribute;
    let (ability_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;
    info.total_success_dice += success_dice;
    info.base_botch_dice += botch_dice; // 能力ダメージ
    info.push_dice += dice_pool - (success_dice + botch_dice);

    let mut dice_count_text = format!("({dice_pool}D6)");
    let mut dice_text = ability_dice_text;

    if m.get(6).is_some() {
        let dice_pool = skill;
        let (skill_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;
        info.total_success_dice += success_dice;
        // 技能ダイスの1はpushで振り直し可能（例えマイナス技能でも）
        info.skill_botch_dice += botch_dice;
        // 技能ダイスのみ1を含むので、ここでは1を計算に入れない
        info.push_dice += dice_pool - success_dice;

        dice_count_text += &format!("+({dice_pool}D6)");
        dice_text += &format!("+{skill_dice_text}");
    }

    if m.get(8).is_some() {
        let dice_pool = gear;
        let (gear_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;
        info.total_success_dice += success_dice;
        info.gear_botch_dice += botch_dice; // ギアダメージ
        info.push_dice += dice_pool - (success_dice + botch_dice);

        dice_count_text += &format!("+({dice_pool}D6)");
        dice_text += &format!("+{gear_dice_text}");
    }

    Ok(Some(info.make_result_with_yze(
        strings,
        &dice_count_text,
        &dice_text,
    )))
}

/// Ruby `resolute_push_action`（プッシュ判定 `nMYZx±x+x`）。
fn resolute_push_action(
    strings: &SystemStrings,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = myz_pattern().captures(command) else {
        return Ok(None);
    };

    let mut info = DiceInfo {
        difficulty: group_to_i(&m, 1),
        ..DiceInfo::default()
    };

    let dice_pool = group_to_i(&m, 3);
    let (ability_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;
    info.total_success_dice += success_dice;
    info.base_botch_dice += botch_dice; // 能力ダメージ
    info.push_dice += dice_pool - (success_dice + botch_dice);

    let mut dice_count_text = format!("({dice_pool}D6)");
    let mut dice_text = ability_dice_text;

    if m.get(6).is_some() {
        let dice_pool = group_to_i(&m, 6);
        let (skill_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;

        let skill_unsigned = m.get(5).map_or("", |x| x.as_str());
        if skill_unsigned == "-" {
            // マイナス技能の成功は通常の成功と相殺される
            info.total_success_dice -= success_dice;
        } else {
            info.total_success_dice += success_dice;
        }

        // 技能ダイスの1はpushで振り直し可能（例えマイナス技能でも）
        info.skill_botch_dice += botch_dice;
        // 技能ダイスのみ1を含むので、ここでは1を計算に入れない
        info.push_dice += dice_pool - success_dice;

        dice_count_text += &format!("{skill_unsigned}({dice_pool}D6)");
        dice_text += &format!("{skill_unsigned}{skill_dice_text}");
    }

    if m.get(8).is_some() {
        let dice_pool = group_to_i(&m, 8);
        let (gear_dice_text, success_dice, botch_dice) = make_dice_roll(dice_pool, rng)?;
        info.total_success_dice += success_dice;
        info.gear_botch_dice += botch_dice; // ギアダメージ
        info.push_dice += dice_pool - (success_dice + botch_dice);

        dice_count_text += &format!("+({dice_pool}D6)");
        dice_text += &format!("+{gear_dice_text}");
    }

    Ok(Some(info.make_result_with_myz(
        strings,
        &dice_count_text,
        &dice_text,
    )))
}

/// Ruby `resolute_step_action`（ステップダイス判定 `nYZSx+x±m[A|D]`）。
fn resolute_step_action(
    strings: &SystemStrings,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = yzs_pattern().captures(command) else {
        return Ok(None);
    };

    let mut info = DiceInfo {
        difficulty: group_to_i(&m, 1),
        ..DiceInfo::default()
    };

    let attribute = group_to_i(&m, 3);
    let skill = group_to_i(&m, 6);
    // m[7] は修正値のグループ全体。Ruby `String#to_i` は先頭の符号を読むので `"-2"` → -2。
    let modifier = group_to_i(&m, 7);
    let advantage = m.get(10).map(|x| x.as_str());

    let mut dice_count_text = String::new();
    let mut dice_text = String::new();

    let (dice_type1, dice_type2) = get_rolling_dice(attribute, skill, modifier);

    if dice_type1 <= dice_type2 {
        match advantage {
            // 不利（`D`）は能力ダイスを振らない
            Some(advantage) => {
                if advantage == "A" && dice_type1 > 4 {
                    let (ability_dice_text, botch_dice) =
                        info.make_dice_a_roll(2, dice_type1, rng)?;
                    info.base_botch_dice += botch_dice; // 能力ダメージ
                    dice_count_text = format!("(2D{dice_type1})");
                    dice_text = ability_dice_text;
                }
            }
            None => {
                if dice_type1 > 4 {
                    let (ability_dice_text, botch_dice) =
                        info.make_dice_a_roll(1, dice_type1, rng)?;
                    info.base_botch_dice += botch_dice; // 能力ダメージ
                    dice_count_text = format!("(1D{dice_type1})");
                    dice_text = ability_dice_text;
                }
            }
        }
        if dice_type2 > 4 {
            let (skill_dice_text, botch_dice) = info.make_dice_a_roll(1, dice_type2, rng)?;
            info.skill_botch_dice += botch_dice;
            if !dice_count_text.is_empty() {
                dice_count_text.push('+');
            }
            if !dice_text.is_empty() {
                dice_text.push('+');
            }
            dice_count_text += &format!("(1D{dice_type2})");
            dice_text += &skill_dice_text;
        }
    } else {
        if dice_type1 > 4 {
            let (ability_dice_text, botch_dice) = info.make_dice_a_roll(1, dice_type1, rng)?;
            info.base_botch_dice += botch_dice; // 能力ダメージ
            dice_count_text = format!("(1D{dice_type1})");
            dice_text = ability_dice_text;
        }
        match advantage {
            Some(advantage) => {
                if advantage == "A" && dice_type2 > 4 {
                    let (skill_dice_text, botch_dice) =
                        info.make_dice_a_roll(2, dice_type2, rng)?;
                    info.skill_botch_dice += botch_dice;
                    dice_count_text += &format!("+(2D{dice_type2})");
                    dice_text += &format!("+{skill_dice_text}");
                }
            }
            None => {
                if dice_type2 > 4 {
                    let (skill_dice_text, botch_dice) =
                        info.make_dice_a_roll(1, dice_type2, rng)?;
                    info.skill_botch_dice += botch_dice;
                    dice_count_text += &format!("+(1D{dice_type2})");
                    dice_text += &format!("+{skill_dice_text}");
                }
            }
        }
    }

    Ok(Some(info.make_result_with_myz(
        strings,
        &dice_count_text,
        &dice_text,
    )))
}

/// Ruby `get_rolling_dice`。修正値の分だけ大きい方／小さい方の面数を2ずつ上下させる。
///
/// 面数の下限は4、上限は12。両方が4になった場合だけ能力側を6へ引き上げる。
fn get_rolling_dice(mut dice_type1: i64, mut dice_type2: i64, mut dice_upgrade: i64) -> (i64, i64) {
    if dice_type1 < 4 {
        dice_type1 = 4;
    }
    if dice_type2 < 4 {
        dice_type2 = 4;
    }

    while dice_upgrade > 0 {
        if dice_type1 >= dice_type2 {
            if dice_type2 < 12 {
                dice_type2 += 2;
            }
        } else if dice_type1 < 12 {
            dice_type1 += 2;
        }
        dice_upgrade -= 1;
    }

    while dice_upgrade < 0 {
        if dice_type1 <= dice_type2 {
            if dice_type2 > 4 {
                dice_type2 -= 2;
            }
        } else if dice_type1 > 4 {
            dice_type1 -= 2;
        }
        dice_upgrade += 1;
    }

    if dice_type1 == 4 && dice_type2 == 4 {
        dice_type1 = 6;
    }

    (dice_type1, dice_type2)
}

/// Ruby `make_dice_roll(dice_pool)`。戻り値は `[出目の文字列, success_dice, botch_dice]`。
fn make_dice_roll(dice_pool: i64, rng: &mut Randomizer) -> Result<(String, i64, i64), EvalError> {
    let dice_list = rng.roll_barabara(dice_pool, 6)?;
    let success_dice = count_matching(&dice_list, |v| v == 6);
    let botch_dice = count_matching(&dice_list, |v| v == 1);

    Ok((
        format!("[{}]", dice_text::join_dice(&dice_list)),
        success_dice,
        botch_dice,
    ))
}

/// Ruby `dice_list.count { ... }`。
fn count_matching(dice_list: &[i64], pred: impl Fn(i64) -> bool) -> i64 {
    dice_list.iter().filter(|&&d| pred(d)).count() as i64
}

/// Ruby `m[index].to_i`（グループ未マッチの `nil.to_i` は 0）。
///
/// 符号付きのグループ（YZSの `m[7]`）もそのまま渡せるように、Ruby `String#to_i` と同じく
/// 先頭の `+` / `-` を読む。
fn group_to_i(captures: &Captures<'_>, index: usize) -> i64 {
    captures
        .get(index)
        .map_or(0, |x| x.as_str().parse::<i64>().unwrap_or(0))
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
            .join("test/data/YearZeroEngine.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/YearZeroEngine.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/YearZeroEngine.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("YearZeroEngine.toml must parse");
        assert_eq!(
            data.tests.len(),
            90,
            "case count in test/data/YearZeroEngine.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "YearZeroEngine",
                "unexpected game system in YearZeroEngine.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("YearZeroEngine"), &tc.input, &mut src) {
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
                    "FAIL YearZeroEngine:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} YearZeroEngine cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
