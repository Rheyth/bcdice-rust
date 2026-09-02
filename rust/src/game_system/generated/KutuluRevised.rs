//! P4で手書き移植した `lib/bcdice/game_system/KutuluRevised.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KutuluRevised#resolute_action`（アクティヴ能力の判定 `nKU`）
//! - `KutuluRevised#resolute_competition`（対抗判定 `nKR`）
//!
//! Ruby でも `Kutulu` を継承せず `Base` を直接継承した別クラスなので、
//! `Kutulu` とは共有せずここに複製してある（原典どおり）。
//! `Kutulu` との差分は `resolute_action` のギリギリ成功判定の条件と文言だけ。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::KutuluRevised`（ID: `KutuluRevised`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KutuluRevised;

impl GameSystem for KutuluRevised {
    fn id(&self) -> &'static str {
        "KutuluRevised"
    }

    fn name(&self) -> &'static str {
        "Kutulu リバイズド"
    }

    fn sort_key(&self) -> &'static str {
        "くとうるうりはいすと"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　nKU        n: ダイス数(1～9)

例)3KU: ダイスを3個振って、その結果を表示(ギリギリでの成功も表示)

■対抗判定　nKR        n: ダイス数(1～9)

例)2KR: ダイスを2個振って、その結果を表示。対抗判定用の3桁の数字も出力。(大きい方が勝利)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\dK[UR]"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `KutuluRevised#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: resolute_action(command) || resolute_competition(command)
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = resolute_competition(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `/(\d)KU/`（アンカーなし）。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d)KU").expect("valid regex"))
}

/// Ruby `/(\d)KR/`（アンカーなし）。
fn competition_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d)KR").expect("valid regex"))
}

/// Ruby `KutuluRevised#resolute_action`（アクティヴ能力の判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    // 1桁の数字なのでパースは必ず成功する。
    let num_dices: i64 = m[1].parse().expect("single digit");

    let mut dices = rng.roll_barabara(num_dices, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);

    let mut output = format!("({num_dices}KU) ＞ {dice_text}");

    let success_num = dices.iter().filter(|&&val| val >= 4).count();
    if success_num > 0 {
        output.push_str(&format!(" ＞ 成功数{success_num}"));
        // Ruby `Kutulu` は `success_num == 1 && counts_4 == 1` だが、こちらは出目4の数を見ない
        if success_num == 1 {
            output.push_str(" ＞ *ギリギリの成功？");
        }
        Ok(Some(EvalResult::success(output)))
    } else {
        output.push_str(" ＞ 失敗");
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `KutuluRevised#resolute_competition`（対抗判定用出力）。
fn resolute_competition(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = competition_pattern().captures(command) else {
        return Ok(None);
    };

    let num_dices: i64 = m[1].parse().expect("single digit");

    let mut dices = rng.roll_barabara(num_dices, 6)?;
    dices.sort_unstable();
    let dice_text = join_dice(&dices);

    let counts_6 = dices.iter().filter(|&&val| val == 6).count();
    let counts_5 = dices.iter().filter(|&&val| val == 5).count();
    let success_num = dices.iter().filter(|&&val| val >= 4).count();
    // Ruby: format("(%d%d%d)", success_num, counts_6, counts_5)
    let com_text = format!("({success_num}{counts_6}{counts_5})");

    let output = format!("({num_dices}KR) ＞ {dice_text} ＞ {com_text}");

    if success_num > 0 {
        Ok(Some(EvalResult::success(output)))
    } else {
        Ok(Some(EvalResult::failure(output)))
    }
}

/// Ruby `dices.join(",")`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "KutuluRevised",
            "KutuluRevised.toml",
            13,
        );
    }
}
