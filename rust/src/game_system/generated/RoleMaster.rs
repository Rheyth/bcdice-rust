//! 自動生成: `lib/bcdice/game_system/RoleMaster.rb` のメタデータから生成した。
//!
//! 手で編集しないこと（`rust/tools/generate_game_systems.rb` が再生成する）。
//! 固有コマンドの中身は P4 で個別移植する。

crate::impl_generated_system! {
    RoleMaster,
    id: "RoleMaster",
    name: "ロールマスター",
    sort_key: "ろおるますたあ",
    help_message: r"上方無限ロール(xUn)の境界値を96にセットします。
",
    prefixes: [],
    settings: {
        upper_dice_reroll_threshold: 96,
    },
}
