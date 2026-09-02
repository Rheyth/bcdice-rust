//! P4で手書き移植した `lib/bcdice/game_system/TokyoGhostResearch.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#tgr_opening_table`（導入表 `OP`）
//! - `#tgr_common_trouble_table`（一般トラブル表 `TB`）
//! - `#getCheckResult`（`TK` 系。原典のバグをそのまま再現する。下記参照）

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `#tgr_opening_table`（導入表 1D10）。
static OPENING_TABLE: Table = Table::from_dice(
    "導入表",
    1,
    10,
    &[
        "【病休中断】体調不良または怪我で療養中だったが強制召喚された。",
        "【忙殺中】別の業務で忙殺中であった。",
        "【出張帰り】遠方での仕事から戻ったばかり。",
        "【休暇取り消し】休暇中だったが呼び戻された。",
        "【平常運転】いつもどおりの仕事中だった。",
        "【休暇明け】十分に休養をとったあとで、心身ともに充実している。",
        "【人生の岐路】人生の岐路にまさに差し掛かったところであった。",
        "【同窓会】かつての同級生に会い、差を実感したばかりだった。",
        "【転職活動中】転職を考えて求人サイトを見ているところだった。",
        "【サボリ中】仕事をサボっているところに呼び出しがあった。",
    ],
);

/// Ruby `#tgr_common_trouble_table`（一般トラブル表 1D10）。
static COMMON_TROUBLE_TABLE: Table = Table::from_dice(
    "一般トラブル表",
    1,
    10,
    &[
        "トラブルが生じたが、間一髪、危機を脱した。【ダメージなし】",
        "どうにかタスクを処理したが、非常に疲労してしまった。【肉体ダメージ1点】",
        "タスク処理の過程で負傷してしまった。【肉体ダメージ1点】",
        "恐怖や混乱、ストレスなどで精神の均衡を崩してしまった。【精神ダメージ1点】",
        "過去のトラウマなどを思い出し、気分が沈んでしまった。【精神ダメージ1点】",
        "自身の信用をキズつけたり、汚名を背負ってしまった。【環境ダメージ1点】",
        "会社や上司の不興を買ってしまった。【環境ダメージ1点】",
        "疲労困憊で動くこともままならない。【肉体ダメージ1点＋精神ダメージ1点】",
        "負傷したうえ、会社に損害を与えてしまった。【肉体ダメージ1点＋環境ダメージ1点】",
        "上司から厳しく叱責され、まずい立場になった。【精神ダメージ1点＋環境ダメージ1点】",
    ],
);

pub struct TokyoGhostResearch;

impl GameSystem for TokyoGhostResearch {
    fn id(&self) -> &'static str {
        "TokyoGhostResearch"
    }

    fn name(&self) -> &'static str {
        "東京ゴーストリサーチ"
    }

    fn sort_key(&self) -> &'static str {
        "とうきようこおすとりさあち"
    }

    fn help_message(&self) -> &'static str {
        r"判定
・タスク処理は目標値以上の値で成功となります。
  1d10>={目標値}
  例：目標値「5」の場合、5～0で成功
各種表
  ・導入表  OP
  ・一般トラブル表  TB
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["OP", "TB", "TK?"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: case command.upcase when /TK/i
        if command.contains("TK") {
            // Ruby `#getCheckResult` は `/TK?<=(\d+)/i` に1つしかグループが無いのに
            // `Regexp.last_match(2)` を読むため `nil.to_i` → `diff = 0` となり、
            // `if diff > 0` に入らず常に空文字列を返す（原典のバグ）。
            // 空文字列は `dice_command` が nil に畳むので、共通コマンドへ抜ける。
            return Ok(Some(SpecificCommandOutput::text(String::new())));
        }

        let table = match command {
            "OP" => &OPENING_TABLE,
            "TB" => &COMMON_TROUBLE_TABLE,
            _ => return Ok(None),
        };

        Ok(Some(SpecificCommandOutput::text(
            table.roll(rng)?.to_string(),
        )))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "TokyoGhostResearch",
            "TokyoGhostResearch.toml",
            27,
        );
    }
}
