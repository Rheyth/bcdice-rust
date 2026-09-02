//! P4で手書き移植した `lib/bcdice/game_system/GaiaCare.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Emoklore` を継承し、`register_prefix_from_super_class` で親の
//! 接頭辞を引き継ぐだけ（`@locale` も `:ja_jp` のまま）なので、判定の実装は
//! [`super::Emoklore`] のものをそのまま使い、ここにはメタデータだけを置く。
//! `Base#result_ndx`（`1D10>=5` などの汎用判定）も親と同じ既定実装で足りる。

use super::Emoklore::{eval_specific_command, JA_SYSTEM};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::GaiaCare`（ID: `GaiaCare`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaiaCare;

impl GameSystem for GaiaCare {
    fn id(&self) -> &'static str {
        "GaiaCare"
    }

    fn name(&self) -> &'static str {
        "ガイアケアTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "かいあけあTRPG"
    }

    fn help_message(&self) -> &'static str {
        r#"・技能値判定（xDM<=y / xDM<=yEz）
  "(個数)DM<=(判定値)"で指定します。
  ダイスの個数は省略可能で、省略した場合1個になります。
  個数や判定値には四則演算（+-*/）を使用できます。
  末尾にEzを付けるとダイス数にzを加算します。E-zで減算も可能です。
  例）2DM<=5 DM<=8 2+2DM<=5 → 4個で判定値5
      2DM<=5E2 → 4個で判定値5 / 3DM<=5E-1 → 2個で判定値5
  ※ダイス数が0以下になる場合は確定失敗

・技能値判定（sDAa+z)
  "(技能レベル)DA(能力値)+(ダイスボーナス)"で指定します。
  ダイスボーナスの個数は省略可能で、省略した場合0になります。
  技能レベルは1～3の数値、またはベース技能の場合"b"が入ります。
  ダイスの個数は技能レベルとダイスボーナスの個数により決定し、s+z個のダイスを振ります。（s="b"の場合はs=1）
  判定値はs+aとなります。（s="b"の場合はs=0）
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"[-+*/\d]*DM<=", r"(B|\d*)DA"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("GaiaCare", "GaiaCare.toml", 6);
    }
}
