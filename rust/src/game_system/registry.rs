//! ゲームシステムの静的レジストリ。Ruby `BCDice.game_system_class` /
//! `BCDice.all_game_systems`（lib/bcdice/loader.rb）に対応する。
//!
//! Ruby側は `BCDice::GameSystem` 名前空間の定数を列挙して動的に集めるが、
//! Rustでは「ビルド時に全システムを確定させる」方針（P3のG1決定）なので、
//! ここに `&'static dyn GameSystem` の表を静的に持つ。
//!
//! 後続バッチは `register_game_systems!` の引数へ生成済みシステムを追加するだけでよい。

use std::collections::HashMap;
use std::sync::OnceLock;

use super::dice_bot::DiceBot;
use super::dummy_system::DummySystem;
use super::GameSystem;

/// 表駆動のレジストリを定義する。
///
/// `handwritten` は手書き実装のユニット構造体の並び。定数昇格（const promotion）に
/// より `&Foo` はそのまま `'static` になる。
/// `generated` は生成物側が持つスライス（`rust/tools/generate_game_systems.rb` が出力）。
///
/// 静的スライス同士はコンパイル時に連結できないので、初回参照時に一度だけ
/// 連結した `Vec` を作って以後使い回す。
macro_rules! register_game_systems {
    (
        handwritten: [$($system:expr),* $(,)?],
        generated: $generated:expr $(,)?
    ) => {
        /// 手書き実装のゲームシステム。
        static HANDWRITTEN_GAME_SYSTEMS: &[&'static dyn GameSystem] = &[$(&$system),*];

        /// 登録済みゲームシステムの一覧（手書き分 → 生成分の順）。
        fn registered() -> &'static [&'static dyn GameSystem] {
            static ALL: OnceLock<Vec<&'static dyn GameSystem>> = OnceLock::new();
            ALL.get_or_init(|| {
                let mut all = HANDWRITTEN_GAME_SYSTEMS.to_vec();
                all.extend_from_slice($generated);
                all
            })
        }
    };
}

register_game_systems! {
    // TODO(P4): インフラ検証用の DummySystem は個別移植が終わったら外す。
    handwritten: [DiceBot, DummySystem],
    generated: crate::game_system::generated::GENERATED_GAME_SYSTEMS,
}

/// 登録済みゲームシステムの一覧（登録順）。
///
/// Ruby `BCDice.all_game_systems` に対応するが、割り当てを避けるためスライスを返す。
pub fn game_systems() -> &'static [&'static dyn GameSystem] {
    registered()
}

/// Ruby `BCDice.all_game_systems`。
pub fn all_game_systems() -> Vec<&'static dyn GameSystem> {
    registered().to_vec()
}

/// IDからゲームシステムを引く。Ruby `BCDice.game_system_class(id)`。
///
/// Ruby側は `all_game_systems.find { ... }` の線形探索だが、TOMLハーネスが
/// 2万件のケースごとに引くので索引を1度だけ作る。
/// ID重複時に先勝ちになる点も `Array#find` と揃えてある
/// （重複がないことは本モジュールの `ids_are_unique` テストで担保する）。
pub fn game_system_class(id: &str) -> Option<&'static dyn GameSystem> {
    index().get(id).copied()
}

fn index() -> &'static HashMap<&'static str, &'static dyn GameSystem> {
    static INDEX: OnceLock<HashMap<&'static str, &'static dyn GameSystem>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut map: HashMap<&'static str, &'static dyn GameSystem> =
            HashMap::with_capacity(registered().len());
        for system in registered() {
            map.entry(system.id()).or_insert(*system);
        }
        map
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dice_bot_is_registered() {
        let system = game_system_class("DiceBot").expect("DiceBot must be registered");
        assert_eq!(system.name(), "DiceBot");
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(game_system_class("NoSuchSystem").is_none());
        // 大文字小文字は区別する（Ruby も `id == game_system::ID` の完全一致）
        assert!(game_system_class("dicebot").is_none());
    }

    #[test]
    fn ids_are_unique() {
        let mut seen: Vec<&'static str> = Vec::new();
        for system in game_systems() {
            assert!(
                !seen.contains(&system.id()),
                "duplicate game system id: {}",
                system.id()
            );
            seen.push(system.id());
        }
        assert_eq!(all_game_systems().len(), seen.len());
    }
}
