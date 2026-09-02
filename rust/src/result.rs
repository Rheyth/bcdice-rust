//! ダイスロールの結果。Ruby `lib/bcdice/result.rb` の移植。
//!
//! P0で用意した [`EvalResult`] をそのまま `BCDice::Result` 相当として使う。
//! Ruby側の `attr_accessor` と Rust側フィールドの対応:
//!
//! | Ruby (`result.rb`)      | Rust                    |
//! |-------------------------|-------------------------|
//! | `text`                  | `text`                  |
//! | `rands`                 | `rands`                 |
//! | `detailed_rands`        | `detailed_rands`        |
//! | `secret?`               | `secret`                |
//! | `success?`              | `success`               |
//! | `failure?`              | `failure`               |
//! | `critical?`             | `critical`              |
//! | `fumble?`               | `fumble`                |
//!
//! `rands` / `detailed_rands` は Ruby本家の `Base#eval`（lib/bcdice/base.rb:173-174）と
//! 同様に、`dice_command` / `eval_common_command` の戻り値に対して
//! ランダマイザの記録を詰める形で接続する（`eval::eval_raw` 参照）。
//! Rubyでは同じ配列オブジェクトを参照代入するが、Rustでは参照コピー（`Vec` の clone）
//! であり、乱数の再ロールは発生しない。この「eval 戻り値への一括詰め」は
//! `common_command::reroll_dice::REROLL_LIMIT` と同様に本家由来の挙動であり、
//! 個別コマンド側での再実装・黙示的な是正はしない。
//!
//! `BarabaraDice::Result` の `last_dice_list_list` 等の拡張フィールドは、
//! 参照するのがゲームシステム側だけなのでP4に委ねる。
//!
//! 注: Rubyは `@rands = nil` 初期化で「eval を通らなかった結果」と「空」を区別するが、
//! Rust版は eval 経由でしか `EvalResult` を観測できないため空 `Vec` で統一する。

use crate::randomizer::DetailedRandResult;

/// 1件のコマンド評価結果。Ruby `BCDice::Result` に対応する。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvalResult {
    /// 出力テキスト（`text` 相当）
    pub text: String,
    /// 消費したダイス出目一覧（`(value, sides)` の列）。Ruby `rands` 相当
    /// （Ruby側は `[value, sides]` の配列。順序も同一）。
    pub rands: Vec<(i64, i64)>,
    /// 消費したダイスロールの詳細。Ruby `detailed_rands` 相当。
    pub detailed_rands: Vec<DetailedRandResult>,
    /// シークレットロールか（`secret?` 相当）
    pub secret: bool,
    /// 成功フラグ（`success?` 相当）
    pub success: bool,
    /// 失敗フラグ（`failure?` 相当）
    pub failure: bool,
    /// クリティカルフラグ（`critical?` 相当）
    pub critical: bool,
    /// ファンブルフラグ（`fumble?` 相当）
    pub fumble: bool,
}

impl EvalResult {
    /// Ruby `Result.new` 相当（テキストなし）。
    pub fn new() -> Self {
        Self::default()
    }

    /// Ruby `Result.new(text)` 相当。
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Ruby `Result.success(text)` 相当。
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            success: true,
            ..Self::default()
        }
    }

    /// Ruby `Result.failure(text)` 相当。
    pub fn failure(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            failure: true,
            ..Self::default()
        }
    }

    /// Ruby `Result.critical(text)` 相当（`critical` と `success` が立つ）。
    pub fn critical(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            critical: true,
            success: true,
            ..Self::default()
        }
    }

    /// Ruby `Result.fumble(text)` 相当（`fumble` と `failure` が立つ）。
    pub fn fumble(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fumble: true,
            failure: true,
            ..Self::default()
        }
    }

    /// Ruby `Result#condition=` 相当。
    pub fn set_condition(&mut self, condition: bool) {
        self.success = condition;
        self.failure = !condition;
    }
}

/// Ruby `Result.nothing`（`:nothing`）と `nil` を区別するための型。
///
/// `Base#check_result` は「フックが `:nothing` を返したら以降の判定を打ち切って `nil`」
/// 「`nil` を返したら次のフックへ進む」という区別をするため、`Option<CheckOutcome>` で
/// 三値（None / Some(Nothing) / Some(Result)）を表現する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Ruby `Result.nothing`（`:nothing`）
    Nothing,
    /// 判定結果
    Result(Box<EvalResult>),
}
