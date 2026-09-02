//! P4で手書き移植した `lib/bcdice/game_system/WorldsEndFrontline.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! # 親クラス（`Bloodorium`）の扱い
//!
//! Ruby側の `WorldsEndFrontline < Bloodorium` は `register_prefix_from_super_class()` を
//! 呼ぶだけで、判定処理（`dicecheck` / `total_expr`）は親 `Bloodorium` のものをそのまま使う。
//! 親 `Bloodorium` は Rust ではまだ未移植のスタブなので、`generated/Bloodorium.rs` は触らず、
//! 必要な処理をこのファイルへ取り込んである（親が移植されたら整理する前提）。
//!
//! ロケール差（i18n `Bloodorium.triumph`）は [`SystemTexts`] に束ね、
//! `WorldsEndFrontline_Korean`（`ko_kr`）が同じ関数を使い回す。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::WorldsEndFrontline`（ID: `WorldsEndFrontline`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldsEndFrontline;

impl GameSystem for WorldsEndFrontline {
    fn id(&self) -> &'static str {
        "WorldsEndFrontline"
    }

    fn name(&self) -> &'static str {
        "ワールドエンドフロントライン"
    }

    fn sort_key(&self) -> &'static str {
        "わあるとえんとふろんとらいん"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    /// Ruby `register_prefix_from_super_class()`（親 `Bloodorium` の `'\d+DC'`）。
    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d+DC"]
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

/// Ruby `HELP_MESSAGE`（親 `Bloodorium` から引き継いだ文面）。
pub(crate) const HELP_MESSAGE: &str = r"・ダイスチェック xDC+y
　【ダイスチェック】を行う。《トライアンフ》を結果に自動反映する。
　x: ダイス数
　y: 結果への修正値 （省略可）
";

/// 1ロケール分の定型文。`WorldsEndFrontline` と `WorldsEndFrontline_Korean` はこれだけが違う。
///
/// i18n `Bloodorium.triumph`（`"《トライアンフ》(*%{triumph})"`）を
/// `%{triumph}` の前後で機械的に分割して持つ。
pub(crate) struct SystemTexts {
    /// `%{triumph}` より前の部分。
    pub(crate) triumph_before: &'static str,
    /// `%{triumph}` より後の部分。
    pub(crate) triumph_after: &'static str,
}

/// i18n `i18n/Bloodorium/ja_jp.yml`。
static JA_TEXTS: SystemTexts = SystemTexts {
    triumph_before: "《トライアンフ》(*",
    triumph_after: ")",
};

/// Ruby `Bloodorium#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    texts: &SystemTexts,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    Ok(dicecheck(texts, command, rng)?.map(SpecificCommandOutput::result))
}

/// Ruby `Bloodorium#dicecheck`（【ダイスチェック】`xDC+y`）。
fn dicecheck(
    texts: &SystemTexts,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("DC", round_type: @round_type)
    //         .has_prefix_number.restrict_cmp_op_to(nil)
    //       `round_type` は Base の既定（:floor）のまま。
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["DC"], RoundType::Floor)
            .has_prefix_number()
            .restrict_cmp_op_to(&[None])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let prefix_number = parsed
        .prefix_number
        .clone()
        .expect("has_prefix_number guarantees a prefix number");
    let mut dice_list = rng.roll_barabara(crate::randomizer::sat_i64(&prefix_number), 6)?;
    dice_list.sort_unstable();

    // Ruby: dice_list.max / values_count.values.max
    //       ダイス数が `UPPER_LIMIT_DICE_TIMES` を超えると `roll_barabara` が空配列を返し、
    //       Ruby は nil に対する演算で NoMethodError になる。ここでは内部エラーとして表面化させる。
    let Some(&dice_value) = dice_list.last() else {
        return Err(EvalError::Internal(
            "Bloodorium: dice check rolled no dice (too many dice)",
        ));
    };
    // Ruby: dice_list.group_by(&:itself).transform_values(&:length).values.max
    //       ソート済みなので同じ値の連なりを数えれば同じ結果になる。
    let mut triumph = 1;
    let mut run = 1;
    for w in dice_list.windows(2) {
        if w[0] == w[1] {
            run += 1;
        } else {
            run = 1;
        }
        triumph = triumph.max(run);
    }

    let total = dice_value * triumph + parsed.modify_number.clone();

    // Ruby: [..., (translate(...) if triumph > 1), (total_expr(...) if total != dice_value), total].compact
    let mut sequence = vec![
        format!("({})", parsed.to_s(SuffixPosition::AfterCommand)),
        format!(
            "[{}]{}",
            dice_list
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(","),
            modifier(&parsed.modify_number)
        ),
    ];
    if triumph > 1 {
        sequence.push(format!(
            "{}{triumph}{}",
            texts.triumph_before, texts.triumph_after
        ));
    }
    if total != crate::Int::from(dice_value) {
        sequence.push(total_expr(
            dice_value,
            triumph,
            crate::randomizer::sat_i64(&parsed.modify_number),
        ));
    }
    sequence.push(total.to_string());

    // Ruby: Result.new.tap { |r| r.critical = triumph > 1; ... }
    //       `critical=` は success を立てない（`Result.critical` とは別物）。
    Ok(Some(EvalResult {
        text: sequence.join(" ＞ "),
        critical: triumph > 1,
        ..EvalResult::default()
    }))
}

/// Ruby `Bloodorium#total_expr`。
fn total_expr(dice_value: i64, triumph: i64, modify_number: i64) -> String {
    // Ruby: formated_triumph = triumph > 1 ? "*#{triumph}" : nil
    let formated_triumph = if triumph > 1 {
        format!("*{triumph}")
    } else {
        String::new()
    };

    format!(
        "{dice_value}{formated_triumph}{}",
        modifier(&crate::Int::from(modify_number))
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "WorldsEndFrontline",
            "WorldsEndFrontline.toml",
            10,
        );
    }
}
