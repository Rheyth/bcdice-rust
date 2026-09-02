//! P4で手書き移植した `lib/bcdice/game_system/Gorilla.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Gorilla#change_text`（`G` を `2D6` に展開するショートカット）
//! - `Gorilla#result_2d6`（出目 `[5,5]` のゴリティカル）

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::game_system::{GameSystem, Target};
use crate::normalize::CmpOp;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::Gorilla`（ID: `Gorilla`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gorilla;

impl GameSystem for Gorilla {
    fn id(&self) -> &'static str {
        "Gorilla"
    }

    fn name(&self) -> &'static str {
        "ゴリラTRPG"
    }

    fn sort_key(&self) -> &'static str {
        "こりらTRPG"
    }

    fn help_message(&self) -> &'static str {
        r"2D6ロール時のゴリティカル自動判定を行います。

G = 2D6のショートカット

例) G>=7 : 2D6して7以上なら成功
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["G"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Gorilla#change_text`。
    ///
    /// Ruby: `string.gsub(/^(S)?G/i) { "#{Regexp.last_match(1)}2D6" }`
    /// 先頭の `S`（シークレットロール）は残したまま `G` だけ `2D6` にする。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        // Rustの `$1` は「グループ12」と読まれ得るので `${1}` と明示する。
        shortcut_pattern().replace_all(text, "${1}2D6")
    }

    /// Ruby `Gorilla#result_2d6`。
    ///
    /// `[5,5]` 以外は `nil` を返して `Base#result_ndx` の成功/失敗判定へ落とす。
    fn result_2d6(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        if value_list == [5, 5] {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "ゴリティカル（自動的成功）",
            ))))
        } else {
            None
        }
    }
}

/// Ruby `/^(S)?G/i`。
fn shortcut_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?mi)^(S)?G").expect("valid regex"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Gorilla", "Gorilla.toml", 15);
    }
}
