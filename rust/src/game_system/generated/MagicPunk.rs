//! P4で手書き移植した `lib/bcdice/game_system/MagicPunk.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MagicPunk#roll_mp`（判定 `nMPm` / チャレンジ判定 `nMPmCx` / ダイス数0 `0MPmCx`）
//!
//! 定型文は `i18n/MagicPunk/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `MagicPunk_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// 1ロケール分の定型文。`MagicPunk` と `MagicPunk_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// i18n `MagicPunk.bad_beat`
    pub(crate) bad_beat: &'static str,
    /// i18n `MagicPunk.jackpot`
    pub(crate) jackpot: &'static str,
    /// i18n `MagicPunk.success`（`%<value>d` を含む書式文字列）
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

static JA_SYSTEM: SystemTables = SystemTables {
    bad_beat: "失敗(BB)",
    jackpot: "成功(JP)",
    success: "成功(%<value>d)",
    failure: "失敗",
};

/// Ruby `/^(\d*)MP(\d+)(C?)(\d*)$/`。
fn mp_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d*)MP(\d+)(C?)(\d*)$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当。桁あふれは i64 に飽和させる
/// （Ruby は多倍長になるが、ダイス数ならどのみち振り切れずにエラーになる）。
fn to_i(digits: &str) -> i64 {
    if digits.is_empty() {
        0
    } else {
        digits.parse().unwrap_or(i64::MAX)
    }
}

/// Ruby `MagicPunk#roll_mp`。
pub(crate) fn roll_mp(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = mp_pattern().captures(command) else {
        return Ok(None);
    };

    // 構文解析
    let dices = if m[1].is_empty() { 1 } else { to_i(&m[1]) };
    let spec = to_i(&m[2]);
    let opt1 = &m[3];
    let arg1 = to_i(&m[4]);
    // ダイス数0モードフラグ
    let is_zero = dices == 0;
    // チャレンジ値
    let challenge = if opt1 == "C" { arg1 } else { 0 };
    // ダイスロール
    let dice_list = rng.roll_barabara(if is_zero { 2 } else { dices }, 20)?;

    // 通常は1つ成功なら成功、0ダイス時はすべて成功したとき成功
    // 通常はすべて失敗なら失敗、0ダイス時は1つ失敗したら失敗
    let check_method = |f: &dyn Fn(i64) -> bool| -> bool {
        if is_zero {
            dice_list.iter().all(|&d| f(d))
        } else {
            dice_list.iter().any(|&d| f(d))
        }
    };
    let fail_method = |f: &dyn Fn(i64) -> bool| -> bool {
        if is_zero {
            dice_list.iter().any(|&d| f(d))
        } else {
            dice_list.iter().all(|&d| f(d))
        }
    };

    let mut check = check_method(&|d| d <= spec && challenge <= d); // 通常判定
    let mut is_jp = check_method(&|d| d == spec); // ジャックポット判定
    let is_bb = fail_method(&|d| d == 1); // バッドビート判定

    let result = if is_bb {
        // 自動失敗優先
        is_jp = false;
        check = false;
        sys.bad_beat.to_owned()
    } else if is_jp {
        check = true;
        sys.jackpot.to_owned()
    } else if check {
        let selected = dice_list.iter().copied().filter(|&d| d <= spec);
        let value = if is_zero {
            selected.min()
        } else {
            selected.max()
        };
        // Ruby: `check` が真なら spec 以下の目が必ず1つはあるので nil にはならない
        let value = value.map(|v| v.to_string()).unwrap_or_default();
        sys.success.replace("%<value>d", &value)
    } else {
        sys.failure.to_owned()
    };

    let mut r = EvalResult::with_text(format!(
        "({dices}MP{spec}C{challenge}) > [{}] > {result}",
        dice_text::join_dice(&dice_list)
    ));
    r.fumble = is_bb;
    r.critical = is_jp;
    r.set_condition(check);
    Ok(Some(r))
}

/// Ruby `BCDice::GameSystem::MagicPunk`（ID: `MagicPunk`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicPunk;

impl GameSystem for MagicPunk {
    fn id(&self) -> &'static str {
        "MagicPunk"
    }

    fn name(&self) -> &'static str {
        "マジックパンクTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ましつくはんくTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (nMPm)
nD20のダイスロールをして、m以下の目があれば成功。
mと同じ目があればジャックポット(自動成功)。
すべての目が1ならバッドビート(自動失敗)。
■ チャレンジ判定 (nMPmCx)
通常の判定に加えてチャレンジ値x以上の目が必要になる。
■ ダイス数0 (0MPmCx)
修正によりダイス数が0になった場合は2d20のダイスロールを行う。
2つの目からより悪い結果になる方を採用する。
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"^\d*MP\d+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `MagicPunk#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(roll_mp(&JA_SYSTEM, command, rng)?.map(SpecificCommandOutput::result))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MagicPunk",
            "MagicPunk.toml",
            14,
        );
    }
}
