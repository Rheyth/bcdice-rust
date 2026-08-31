//! Ruby `BCDice::GameSystem::DiceBot`（lib/bcdice/game_system/DiceBot.rb）の移植。
//!
//! `Base` の既定値をそのまま使い、固有コマンド（`prefixes`）を持たない。
//! 汎用コマンドだけを提供する「素のダイスボット」で、TOMLテストの327ケースがこれに属する。

use super::GameSystem;

/// Ruby `BCDice::GameSystem::DiceBot`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiceBot;

impl GameSystem for DiceBot {
    fn id(&self) -> &'static str {
        "DiceBot"
    }

    fn name(&self) -> &'static str {
        "DiceBot"
    }

    fn sort_key(&self) -> &'static str {
        "*たいすほつと"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    // 設定値・フックはすべて `Base` の既定値のまま（trait の既定実装）。
}

/// Ruby `HELP_MESSAGE` 定数（ヒアドキュメント `<<~HELP` を展開したもの）。
const HELP_MESSAGE: &str = "\
3D6+1>=9 ：3d6+1で目標値9以上かの判定
1D100<=50 ：D100で50％目標の下方ロールの例
3U6[5] ：3d6のダイス目が5以上の場合に振り足しして合計する(上方無限)
3B6 ：3d6のダイス目をバラバラのまま出力する（合計しない）
10B6>=4 ：10d6を振り4以上のダイス目の個数を数える
2R6[>3]>=5 ：2D6のダイス目が3より大きい場合に振り足して、5以上のダイス目の個数を数える
(8/2)D(4+6)<=(5*3)：個数・ダイス・達成値には四則演算も使用可能
c(10-4*3/2+2)：c(計算式）で計算だけの実行も可能
choice[a,b,c]：列挙した要素から一つを選択表示。ランダム攻撃対象決定などに
S3d6 ： 各コマンドの先頭に「S」を付けると他人結果の見えないシークレットロール
3d6/2 ： ダイス出目を割り算（端数処理はゲームシステム依存）。切り上げは /2C、四捨五入は /2R、切り捨ては /2F
D66 ： D66ダイス。順序はゲームに依存。D66N：そのまま、D66A：昇順、D66D：降順

詳細は下記URLのコマンドガイドを参照
https://docs.bcdice.org/
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{D66SortType, RoundType};

    #[test]
    fn constants_match_ruby() {
        assert_eq!(DiceBot.id(), "DiceBot");
        assert_eq!(DiceBot.name(), "DiceBot");
        assert_eq!(DiceBot.sort_key(), "*たいすほつと");
        assert!(DiceBot.help_message().starts_with("3D6+1>=9 ："));
        assert!(DiceBot
            .help_message()
            .ends_with("https://docs.bcdice.org/\n"));
    }

    #[test]
    fn settings_are_base_defaults() {
        assert!(DiceBot.prefixes().is_empty());
        assert!(DiceBot.prefixes_pattern().is_none());
        assert!(!DiceBot.sort_add_dice());
        assert!(!DiceBot.sort_barabara_dice());
        assert_eq!(DiceBot.d66_sort_type(), D66SortType::NoSort);
        assert_eq!(DiceBot.round_type(), RoundType::Floor);
        assert_eq!(DiceBot.sides_implicit_d(), 6);
        assert!(DiceBot.enabled_upcase_input());
    }
}
