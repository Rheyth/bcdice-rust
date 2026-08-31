//! 差分ファズ用の決定的乱数源（ラインナップ型）。
//!
//! Ruby側 `bin/fuzz_runner.rb` の `LineupRandomizer` と**完全に同一の式**で出目を返す:
//!
//! ```text
//! state = (state * 48271) % 2147483647   # 面数ごとに独立。初期値 1
//! value = (state % sides) + 1            # 1..=sides
//! ```
//!
//! 面数ごとに独立した状態を持つので「振られた面数と回数が同じなら
//! Ruby/Rustで同一出目」が保証される。出力差分は乱数差異ではなく
//! ロジック差異だけを反映する。
//!
//! **入力1件ごとに新しいインスタンスを作ること。** 状態を引き継ぐと、
//! 1件の不一致が以降の全ケースを巻き添えにして差分が雪崩になる。

use crate::eval::EvalError;
use crate::randomizer::RandSource;

pub const LINEUP_MULTIPLIER: u64 = 48271;
pub const LINEUP_MODULUS: u64 = 2147483647;
pub const LINEUP_SEED: u64 = 1;

/// 差分ファズ用の決定的乱数源（P5成果物・現head RandSource API移植版）。
#[derive(Debug, Clone, Default)]
pub struct LineupSource {
    /// 面数ごとの独立した乱数状態
    states: std::collections::HashMap<i64, u64>,
}

impl LineupSource {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RandSource for LineupSource {
    fn random(&mut self, sides: i64) -> Result<i64, EvalError> {
        // Randomizer#rand_inner は 0 < sides <= 10000 でしか呼ばないが、
        // ゼロ除算を避けるためRuby側と同じ番人を置く
        if sides <= 0 {
            return Ok(1);
        }

        let state = self.states.entry(sides).or_insert(LINEUP_SEED);
        *state = (*state * LINEUP_MULTIPLIER) % LINEUP_MODULUS;
        Ok(((*state % u64::try_from(sides).unwrap_or(u64::MAX)) as i64) + 1)
    }
}
