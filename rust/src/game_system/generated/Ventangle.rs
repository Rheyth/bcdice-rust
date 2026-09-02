//! P4で手書き移植した `lib/bcdice/game_system/Ventangle.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Ventangle#eval_game_system_specific_command`（判定 `VTn@s#f$g>=T`。
//!   3個以上振ったときは出目順を保ったまま上位2つを採用）
//! - `Ventangle#compare`（ファンブル → スペシャル → 目標値の順で判定）
//!
//! ロケール差のある定型文は [`SystemTexts`] に束ね、
//! `Ventangle_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `DEFAULT_SPECIAL_VALUE`。
const DEFAULT_SPECIAL_VALUE: i64 = 12;
/// Ruby `DEFAULT_FUMBLE_VALUE`。
const DEFAULT_FUMBLE_VALUE: i64 = 2;
/// Ruby `DEFAULT_DICE_NUM`。
const DEFAULT_DICE_NUM: i64 = 2;

/// 1ロケール分の定型文。
pub(crate) struct SystemTexts {
    /// `Ventangle.special`
    pub(crate) special: &'static str,
    /// `Ventangle.level_gap`（`%<gap>d` を差し替える）
    pub(crate) level_gap: &'static str,
    /// `fumble`
    pub(crate) fumble: &'static str,
    /// `success`
    pub(crate) success: &'static str,
    /// `failure`
    pub(crate) failure: &'static str,
}

pub(crate) static JA_TEXTS: SystemTexts = SystemTexts {
    special: "スペシャル",
    level_gap: "ギャップボーナス(%<gap>d)",
    fumble: "ファンブル",
    success: "成功",
    failure: "失敗",
};

/// Ruby `Array#to_s`（`[5, 2]`）。
fn array_to_s(list: &[i64]) -> String {
    format!(
        "[{}]",
        list.iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Ruby `Ventangle#compare`。判定結果（フラグ）と表示文。
fn compare(
    texts: &SystemTexts,
    dice_total: i64,
    total: I,
    special: i64,
    fumble: i64,
    cmp_op: Option<CmpOp>,
    target_number: Option<I>,
) -> (EvalResult, Option<&'static str>) {
    if dice_total <= fumble {
        return (EvalResult::fumble(""), Some(texts.fumble));
    } else if dice_total >= special {
        return (EvalResult::critical(""), Some(texts.special));
    }

    match (target_number, cmp_op) {
        (Some(target), Some(op)) => {
            if op.apply(&total, &target) {
                (EvalResult::success(""), Some(texts.success))
            } else {
                (EvalResult::failure(""), Some(texts.failure))
            }
        }
        // Ruby: Result.new(nil)
        _ => (EvalResult::new(), None),
    }
}

/// Ruby `Ventangle#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    texts: &SystemTexts,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new('VT', round_type: round_type)（Base の既定 :floor）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["VT"], RoundType::Floor)
            .enable_critical()
            .enable_fumble()
            .enable_dollar()
            .enable_suffix_number()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let Some(cmd) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_num = cmd
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(DEFAULT_DICE_NUM);
    if dice_num < DEFAULT_DICE_NUM {
        return Ok(None);
    }

    let dice_list = rng.roll_barabara(dice_num, 6)?;
    let using_list: Vec<i64> = if dice_num > 2 {
        // Ruby: 値の昇順（同値は添字順）に並べて reverse → 上位2つ → 添字順に戻す。
        // つまり値の降順、同値なら添字の降順で2つ選び、元の出目順で並べる。
        let mut indices: Vec<usize> = (0..dice_list.len()).collect();
        indices.sort_by(|&a, &b| dice_list[b].cmp(&dice_list[a]).then(b.cmp(&a)));
        let mut top: Vec<usize> = indices.into_iter().take(2).collect();
        top.sort_unstable();
        top.into_iter().map(|i| dice_list[i]).collect()
    } else {
        dice_list.clone()
    };
    let dice_total: i64 = using_list.iter().sum();
    let total = dice_total + cmd.modify_number.clone();

    let special = cmd
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(DEFAULT_SPECIAL_VALUE);
    let fumble = cmd
        .fumble
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(DEFAULT_FUMBLE_VALUE);
    let (mut result, result_text) = compare(
        texts,
        dice_total,
        total.clone(),
        special,
        fumble,
        cmd.cmp_op,
        cmd.target_number.clone(),
    );

    let advantage_str = (dice_num > 2).then(|| array_to_s(&using_list));

    let modifier_str = (cmd.modify_number > I::ZERO)
        .then(|| format!("{dice_total}{}", format::modifier(&cmd.modify_number)));

    let gap_bonus_str = match (cmd.target_number.as_ref(), cmd.dollar.as_ref()) {
        (Some(target), Some(dollar)) if result.success => {
            let gap = &total - target;
            (gap >= *dollar).then(|| texts.level_gap.replace("%<gap>d", &gap.to_string()))
        }
        _ => None,
    };

    let mut sequence: Vec<String> = vec![
        cmd.to_s(SuffixPosition::AfterCommand),
        array_to_s(&dice_list),
    ];
    sequence.extend(advantage_str);
    sequence.extend(modifier_str);
    sequence.push(total.to_string());
    sequence.extend(result_text.map(str::to_owned));
    sequence.extend(gap_bonus_str);

    result.text = sequence.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `BCDice::GameSystem::Ventangle`（ID: `Ventangle`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ventangle;

impl GameSystem for Ventangle {
    fn id(&self) -> &'static str {
        "Ventangle"
    }

    fn name(&self) -> &'static str {
        "Ventangle"
    }

    fn sort_key(&self) -> &'static str {
        "うえんたんくる"
    }

    fn help_message(&self) -> &'static str {
        r"基本書式 VTn@s#f$g>=T n=ダイス数（省略時2） s=スペシャル値（省略時12） f=ファンブル値（省略時2） g=レベルギャップ判定値（省略可） T=目標値（省略可）

例：
VT        デフォルトのスペシャル値・ファンブル値の判定を行う
VT@10#3   スペシャル値10、ファンブル値3の判定を行う
VT3@10#3  スペシャル値10、ファンブル値3の判定を、アドバンテージを1点消費してダイス3つで行う

VT>=5         デフォルトのスペシャル値・ファンブル値で目標値5の判定を行う
VT@10#3>=5    スペシャル値10、ファンブル値3で目標値5の判定を行う
VT@10#3$5>=5  スペシャル値10、ファンブル値3で目標値5の判定を行う。この際達成値が目標値より5以上大きい場合、ギャップボーナスを表示する
VT3@10#3>=5   スペシャル値10、ファンブル値3で目標値5の判定を、アドバンテージを1点消費してダイス3つで行う
VT3@10#3$4>=5 スペシャル値10、ファンブル値3で目標値5の判定を、アドバンテージを1点消費してダイス3つで行う。この際達成値が目標値より4以上大きい場合、ギャップボーナスを表示する
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["VT"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_TEXTS, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "Ventangle",
            "Ventangle.toml",
            47,
        );
    }
}
