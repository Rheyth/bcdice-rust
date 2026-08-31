//! ダイスロールの結果。Ruby `lib/bcdice/result.rb` の移植。
//!
//! P0で用意した [`EvalResult`] をそのまま `BCDice::Result` 相当として使う。
//! Ruby側の `rands` / `detailed_rands` はハーネスが乱数消費で検証するため省略した
//! （P4以降でゲームシステムが参照し始めたら追加する）。
//! `BarabaraDice::Result` の `last_dice_list_list` 等の拡張フィールドも同様に、
//! 参照するのがゲームシステム側だけなのでP4に委ねる。

/// 1件のコマンド評価結果。Ruby `BCDice::Result` に対応する。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvalResult {
    /// 出力テキスト（`text` 相当）
    pub text: String,
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
