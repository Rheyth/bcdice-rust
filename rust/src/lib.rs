//! bcdice-rust: BCDice (Ruby) を Rust へ移植するプロジェクト。
//!
//! # 現在の到達点（P3-Batch1: GameSystemインフラ）
//!
//! P1（コア移植）:
//! - [`randomizer`]: Ruby `BCDice::Randomizer` の移植と、TOML `rands` を注入する
//!   [`SeededRandomizer`](randomizer::SeededRandomizer)
//! - [`arithmetic`] / [`command_parser`] / [`common_command`]: Racc文法の
//!   手書き再帰下降移植（選定理由は [`arithmetic`] のモジュールdoc参照）
//! - [`eval`]: Ruby `BCDice::Base#eval` のパイプライン
//! - [`toml_test`]: `test/data/*.toml` 348ファイルを読んで実行するテストハーネス
//!
//! P3-Batch1（GameSystemインフラ）:
//! - [`game_system`]: Ruby `BCDice::Base` に対応する [`GameSystem`](game_system::GameSystem)
//!   トレイトと、Ruby `BCDice.game_system_class` に対応する
//!   [`registry`](game_system::registry)
//! - [`dice_table`]: Ruby `BCDice::DiceTable::*` の表11種
//!
//! # 未実装
//!
//! レジストリに登録済みなのは [`DiceBot`](game_system::dice_bot::DiceBot) と、
//! インフラ検証用の [`DummySystem`](game_system::dummy_system::DummySystem) だけ。
//! 残り335システムは後続バッチで `game_system/generated/` へコード生成する
//! （docs/rust_port_plan.md の P3 節を参照）。
//! 未登録のIDは [`eval::eval_command`] が
//! [`EvalError::SystemNotImplemented`](eval::EvalError::SystemNotImplemented) を返し、
//! ハーネスがfail理由として表示する。

pub type Int = num_bigint::BigInt;

pub mod arithmetic;
pub mod command_parser;
pub mod common_command;
pub mod dice_table;
pub mod enums;
pub mod eval;
pub mod format;
pub mod game_system;
pub mod lineup_source;
pub mod normalize;
pub mod preprocessor;
pub mod randomizer;
pub mod result;
pub mod toml_test;
