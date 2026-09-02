//! ゲームシステム。Ruby `BCDice::Base`（lib/bcdice/base.rb）とその継承クラス群に対応する。
//!
//! # 構造
//!
//! Ruby側は「1つの `Base` インスタンスが設定（`initialize` のインスタンス変数）と
//! フック（`result_1d100` などのオーバーライド可能なメソッド）の両方を持つ」構造をしている。
//! Rustではこれを [`GameSystem`] トレイト1本で表し、
//!
//! - 設定値 → 既定実装つきのアクセサ（[`GameSystem::sort_add_dice`] など）
//! - フック → 既定実装つきのメソッド（[`GameSystem::result_1d100`] など）
//!
//! として、ゲームシステム側が必要なものだけを上書きする。
//! 既定実装は Ruby `Base` の既定値・空メソッドと1対1に対応する。
//!
//! [`GameSystemConfig`] は同じ設定値を実行時に組み替えられる struct 版で、
//! それ自身が [`GameSystem`] を実装する（`Base` を直接インスタンス化したものに相当）。
//! 「既定値以外の設定を通る分岐」をゲームシステム移植を待たずに検証するために使う
//! （rust/tests/config_variants.rs）。
//!
//! # 登録済みのシステム
//!
//! [`registry`] には Ruby本家の全336システムが登録されており、
//! TOMLハーネス（`test/data/*.toml` 348ファイル・19,864ケース）が全パスしている。
//! 各システムの実装状況と残課題は docs/rust_port_plan.md および
//! docs/refactor_candidates_20260901.md を参照。

pub mod dice_bot;
pub mod dummy_system;
pub mod generated;
pub mod int_helpers;
pub mod registry;

use std::borrow::Cow;

use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// マクロ [`impl_prefixes_pattern!`](crate::impl_prefixes_pattern) から参照するための再エクスポート。
pub use regex::Regex;
pub use registry::{game_system_class, game_systems};

/// ゲームシステムID（TOML `game_system`、例: "AFF2e"）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameSystemId(String);

impl GameSystemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GameSystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Ruby `Base#eval_game_system_specific_command` の戻り値。
///
/// Ruby側は `String` / `Result` / `nil` / `RollResult`（`to_s` される）を返しうる。
/// `Base#dice_command` の分岐（`Result` ならそのまま、文字列なら空・`"1"` を `nil` 扱い）
/// を型で表現するため、文字列と `Result` を区別して持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecificCommandOutput {
    /// 文字列を返した場合。空文字列と `"1"` は `Base#dice_command` が `nil` に畳む。
    Text(String),
    /// `Result` を返した場合。`secret` は `dice_command` が OR で立てる。
    Result(Box<EvalResult>),
}

impl SpecificCommandOutput {
    /// 文字列出力を作る。
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// `Result` 出力を作る。
    pub fn result(result: EvalResult) -> Self {
        Self::Result(Box::new(result))
    }
}

/// 目標値。Ruby側は `Integer` か文字列 `"?"`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Number(crate::Int),
    /// 目標値未定（`"?"`）
    Question,
}

/// ゲームシステム。Ruby `BCDice::Base` とその継承クラスに対応する。
///
/// 静的レジストリ（[`registry`]）に `&'static dyn GameSystem` として置くため
/// [`Sync`] を要求する。実装は原則としてフィールドを持たないユニット構造体にし、
/// 設定値はアクセサの上書きで表す（Rubyの定数・`initialize` 代入に対応する）。
pub trait GameSystem: Sync {
    // ----- Ruby: クラス定数 -----

    /// Ruby `ID` 定数。
    fn id(&self) -> &'static str;

    /// Ruby `NAME` 定数。
    fn name(&self) -> &'static str;

    /// Ruby `SORT_KEY` 定数。
    fn sort_key(&self) -> &'static str;

    /// Ruby `HELP_MESSAGE` 定数。
    fn help_message(&self) -> &'static str;

    /// Ruby `Base.prefixes`（`register_prefix` で登録された接頭辞）。
    ///
    /// 各要素は正規表現の断片（例: `"\\d*BT"`）。既定は空＝固有コマンドを持たない。
    fn prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    /// Ruby `Base.prefixes_pattern`（`/^(S)?(prefix|...)/i`）。
    ///
    /// 既定の `None` は Ruby側の `/(?!)/`（何にもマッチしない）に対応する
    /// ＝ [`prefixes`](Self::prefixes) が空の場合。
    ///
    /// # 上書きするときの注意
    ///
    /// キャッシュ用の `static OnceLock<Regex>` は **必ず各 impl のメソッド本体**に置くこと。
    /// このトレイト既定実装の本体に置くと、単相化されても `static` は1つしか作られず、
    /// 全ゲームシステムが最初に評価された1つの正規表現を共有してしまう。
    /// 正しい形は [`impl_prefixes_pattern!`](crate::impl_prefixes_pattern) が1行で書ける。
    fn prefixes_pattern(&self) -> Option<&'static Regex> {
        None
    }

    // ----- Ruby: Base#initialize が設定するインスタンス変数 -----

    /// Ruby `sort_add_dice?`。加算ダイスでダイス目をソートするか。
    fn sort_add_dice(&self) -> bool {
        false
    }

    /// Ruby `sort_barabara_dice?`。バラバラダイスでダイス目をソートするか。
    fn sort_barabara_dice(&self) -> bool {
        false
    }

    /// Ruby `d66_sort_type`。D66の入れ替え方法。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    /// Ruby `enabled_d9?`。D9ダイスが有効か。
    fn enabled_d9(&self) -> bool {
        false
    }

    /// Ruby `round_type`。割り算の端数処理。
    fn round_type(&self) -> RoundType {
        RoundType::Floor
    }

    /// Ruby `sides_implicit_d`。面数省略時のダイス面数。
    fn sides_implicit_d(&self) -> i64 {
        6
    }

    /// Ruby `upper_dice_reroll_threshold`。UpperDiceの振り足し閾値の既定値。
    fn upper_dice_reroll_threshold(&self) -> Option<i64> {
        None
    }

    /// Ruby `reroll_dice_reroll_threshold`。RerollDiceの振り足し閾値の既定値。
    fn reroll_dice_reroll_threshold(&self) -> Option<i64> {
        None
    }

    /// Ruby `default_cmp_op`。目標値が空欄の場合の比較演算子。
    fn default_cmp_op(&self) -> Option<CmpOp> {
        None
    }

    /// Ruby `default_target_number`。目標値が空欄の場合の目標値。
    fn default_target_number(&self) -> Option<i64> {
        None
    }

    /// Ruby `@enabled_upcase_input`。入力を大文字化するか。
    fn enabled_upcase_input(&self) -> bool {
        true
    }

    // ----- Ruby: オーバーライド可能なフック -----

    /// Ruby `Base#eval_game_system_specific_command`。ゲームシステム固有コマンドの評価。
    ///
    /// 既定は Ruby の空メソッド（`nil`）に対応する `Ok(None)`。
    fn eval_game_system_specific_command(
        &self,
        _command: &str,
        _rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(None)
    }

    /// Ruby `Base#change_text`。ゲームシステムごとの入力前処理。既定は恒等関数。
    fn change_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(text)
    }

    /// Ruby `Base#grich_text`（シャドウラン用グリッチ判定）。既定は `nil`。
    fn grich_text(
        &self,
        _count_one: usize,
        _dice_total_count: usize,
        _count_success: i64,
    ) -> Option<String> {
        None
    }

    /// Ruby `Base#result_1d100`。既定は空メソッド（`nil`）。
    fn result_1d100(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        None
    }

    /// Ruby `Base#result_1d20`。既定は空メソッド（`nil`）。
    fn result_1d20(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        None
    }

    /// Ruby `Base#result_2d6`。既定は空メソッド（`nil`）。
    fn result_2d6(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        _value_list: &[i64],
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        None
    }

    /// Ruby `Base#result_2d6` のうち、フック内で `@randomizer` を使うもの。
    ///
    /// Ruby の `result_*` は `@randomizer` を握っているのでフック内でもダイスを振れる
    /// （例: `CardRanker#result_2d6` はスペシャル時にランダムモンスター選択を振る）。
    /// Rust の [`result_2d6`](Self::result_2d6) には [`Randomizer`] が渡らないので、
    /// 振る必要があるシステムはこちらを上書きする。既定は [`result_2d6`](Self::result_2d6)
    /// へそのまま委譲するので、振らないシステムは従来どおり `result_2d6` だけを書けばよい。
    ///
    /// ここで振るダイスは Ruby と同じく **加算ロールの `rand_results` には入らない**
    /// （Ruby も `AddDice::Randomizer` ではなくゲームシステムの `@randomizer` を使う）。
    fn result_2d6_with_randomizer(
        &self,
        total: crate::Int,
        dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
        _rng: &mut Randomizer,
    ) -> Result<Option<CheckOutcome>, EvalError> {
        Ok(self.result_2d6(total, dice_total, value_list, cmp_op, target))
    }

    /// Ruby `Base#result_nd10`。既定は空メソッド（`nil`）。
    fn result_nd10(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        _value_list: &[i64],
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        None
    }

    /// Ruby `Base#result_nd6`。既定は空メソッド（`nil`）。
    fn result_nd6(
        &self,
        _total: crate::Int,
        _dice_total: i64,
        _value_list: &[i64],
        _cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        None
    }

    // ----- Ruby: Base に実装があり、通常は上書きしないもの -----

    /// Ruby `Base#result_ndx`。成功/失敗を返す。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        match target {
            // Ruby: target.is_a?(String) -> nil （目標値 "?" のケース）
            Target::Question => None,
            Target::Number(t) => {
                if cmp_op.apply(&total, &t) {
                    Some(EvalResult::success(translate_success()))
                } else {
                    Some(EvalResult::failure(translate_failure()))
                }
            }
        }
    }

    /// Ruby `Base#check_result`。
    ///
    /// `rand_results` は `(sides, value)` の列
    /// （`CommonCommand::AddDice::Randomizer::RandResult` 相当）。
    ///
    /// `rng` は [`result_2d6_with_randomizer`](Self::result_2d6_with_randomizer) 用。
    /// Ruby の `@randomizer` に対応し、加算ロールの `rand_results` を記録しない生の方を渡す。
    fn check_result(
        &self,
        total: crate::Int,
        rand_results: &[(i64, i64)],
        cmp_op: CmpOp,
        target: Target,
        rng: &mut Randomizer,
    ) -> Result<Option<EvalResult>, EvalError> {
        let sides_list: Vec<i64> = rand_results.iter().map(|r| r.0).collect();
        let value_list: Vec<i64> = rand_results.iter().map(|r| r.1).collect();
        let dice_total: i64 = value_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));

        let ret = match sides_list.as_slice() {
            [100] => self.result_1d100(total.clone(), dice_total, cmp_op, target.clone()),
            [20] => self.result_1d20(total.clone(), dice_total, cmp_op, target.clone()),
            [6, 6] => self.result_2d6_with_randomizer(
                total.clone(),
                dice_total,
                &value_list,
                cmp_op,
                target.clone(),
                rng,
            )?,
            _ => None,
        };
        match ret {
            // Ruby: return nil if ret == Result.nothing
            Some(CheckOutcome::Nothing) => return Ok(None),
            Some(CheckOutcome::Result(r)) => return Ok(Some(*r)),
            None => {}
        }

        // Ruby `Array#uniq` は「出現順を保った重複除去」。隣接除去の `dedup` とは異なる。
        let mut uniq: Vec<i64> = Vec::new();
        for s in &sides_list {
            if !uniq.contains(s) {
                uniq.push(*s);
            }
        }

        let ret = match uniq.as_slice() {
            [10] => self.result_nd10(
                total.clone(),
                dice_total,
                &value_list,
                cmp_op,
                target.clone(),
            ),
            [6] => self.result_nd6(
                total.clone(),
                dice_total,
                &value_list,
                cmp_op,
                target.clone(),
            ),
            _ => None,
        };
        match ret {
            Some(CheckOutcome::Nothing) => return Ok(None),
            Some(CheckOutcome::Result(r)) => return Ok(Some(*r)),
            None => {}
        }

        Ok(self.result_ndx(total, cmp_op, target))
    }
}

/// [`GameSystem::prefixes_pattern`] を正しい形（impl ごとの `static`）で実装する。
///
/// ```ignore
/// impl GameSystem for Foo {
///     fn prefixes(&self) -> &'static [&'static str] { &["FOO"] }
///     bcdice::impl_prefixes_pattern!();
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! impl_prefixes_pattern {
    () => {
        fn prefixes_pattern(&self) -> ::core::option::Option<&'static $crate::game_system::Regex> {
            // この `static` は展開先の impl ごとに1つ作られる。
            // トレイト既定実装の本体に置くと全 impl で共有されてしまうので、
            // 必ずこのマクロ（＝各 impl 側）で定義すること。
            static RE: ::std::sync::OnceLock<$crate::game_system::Regex> =
                ::std::sync::OnceLock::new();
            ::core::option::Option::Some(
                RE.get_or_init(|| $crate::game_system::build_prefixes_pattern(self.prefixes())),
            )
        }
    };
}

/// Ruby `Base.prefixes_pattern` の正規表現を組み立てる。
///
/// Ruby: `/^(S)?(#{@prefixes.join('|')})/i`
///
/// - `join('|')` 全体を1つのグループで包む（個別に包むとマッチ範囲が変わる）。
/// - Rubyの `^` は行頭にもマッチするが、`Preprocessor` が最初の空白（改行を含む）より
///   前しか残さないため、ここに改行が来ることはない。`(?m)` は付けない。
///
/// # Panics
///
/// 接頭辞が `regex` クレートで解釈できない正規表現だった場合にパニックする
/// （Rubyでは正規表現リテラルの構文エラーに相当する）。
pub fn build_prefixes_pattern(prefixes: &[&str]) -> Regex {
    let source = format!("(?i)^(S)?({})", prefixes.join("|"));
    Regex::new(&source).unwrap_or_else(|e| panic!("invalid prefixes pattern {source:?}: {e}"))
}

/// Ruby `Base` の設定値（`initialize` が代入するインスタンス変数）を struct にしたもの。
///
/// それ自身が [`GameSystem`] を実装しており、`Base` を直接インスタンス化したものとして
/// 振る舞う。ゲームシステムを移植せずに「既定値以外の設定を通る分岐」を検証するために使う。
///
/// 実際のゲームシステムはユニット構造体 + アクセサ上書きで表現するので、
/// 移植コードがこの struct を持つことはない。
///
/// なお Ruby の `prefixes` はクラス属性（`register_prefix`）でありインスタンス変数では
/// ないため、ここには含めない（[`GameSystem::prefixes`] 側で表す）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSystemConfig {
    /// Ruby `ID` 定数相当。既定は空文字列。
    pub id: &'static str,
    /// Ruby `NAME` 定数相当。既定は空文字列。
    pub name: &'static str,
    /// Ruby `SORT_KEY` 定数相当。既定は空文字列。
    pub sort_key: &'static str,
    /// Ruby `HELP_MESSAGE` 定数相当。既定は空文字列。
    pub help_message: &'static str,
    /// 加算ダイスでダイス目をソートするか（`sort_add_dice?`）
    pub sort_add_dice: bool,
    /// バラバラダイスでダイス目をソートするか（`sort_barabara_dice?`）
    pub sort_barabara_dice: bool,
    /// D66の入れ替え方法（`d66_sort_type`）
    pub d66_sort_type: D66SortType,
    /// D9ダイスが有効か（`enabled_d9?`）
    pub enabled_d9: bool,
    /// 割り算の端数処理（`round_type`）
    pub round_type: RoundType,
    /// 面数省略時のダイス面数（`sides_implicit_d`）
    pub sides_implicit_d: i64,
    /// UpperDiceの振り足し閾値の既定値（`upper_dice_reroll_threshold`）
    pub upper_dice_reroll_threshold: Option<i64>,
    /// RerollDiceの振り足し閾値の既定値（`reroll_dice_reroll_threshold`）
    pub reroll_dice_reroll_threshold: Option<i64>,
    /// 目標値が空欄の場合の比較演算子（`default_cmp_op`）
    pub default_cmp_op: Option<CmpOp>,
    /// 目標値が空欄の場合の目標値（`default_target_number`）
    pub default_target_number: Option<i64>,
    /// 入力を大文字化するか（`enabled_upcase_input`）
    pub enabled_upcase_input: bool,
}

impl Default for GameSystemConfig {
    /// Ruby `BCDice::Base#initialize` の既定値。
    fn default() -> Self {
        Self {
            id: "",
            name: "",
            sort_key: "",
            help_message: "",
            sort_add_dice: false,
            sort_barabara_dice: false,
            d66_sort_type: D66SortType::NoSort,
            enabled_d9: false,
            round_type: RoundType::Floor,
            sides_implicit_d: 6,
            upper_dice_reroll_threshold: None,
            reroll_dice_reroll_threshold: None,
            default_cmp_op: None,
            default_target_number: None,
            enabled_upcase_input: true,
        }
    }
}

impl GameSystem for GameSystemConfig {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn sort_key(&self) -> &'static str {
        self.sort_key
    }
    fn help_message(&self) -> &'static str {
        self.help_message
    }
    fn sort_add_dice(&self) -> bool {
        self.sort_add_dice
    }
    fn sort_barabara_dice(&self) -> bool {
        self.sort_barabara_dice
    }
    fn d66_sort_type(&self) -> D66SortType {
        self.d66_sort_type
    }
    fn enabled_d9(&self) -> bool {
        self.enabled_d9
    }
    fn round_type(&self) -> RoundType {
        self.round_type
    }
    fn sides_implicit_d(&self) -> i64 {
        self.sides_implicit_d
    }
    fn upper_dice_reroll_threshold(&self) -> Option<i64> {
        self.upper_dice_reroll_threshold
    }
    fn reroll_dice_reroll_threshold(&self) -> Option<i64> {
        self.reroll_dice_reroll_threshold
    }
    fn default_cmp_op(&self) -> Option<CmpOp> {
        self.default_cmp_op
    }
    fn default_target_number(&self) -> Option<i64> {
        self.default_target_number
    }
    fn enabled_upcase_input(&self) -> bool {
        self.enabled_upcase_input
    }
}

/// i18n `ja_jp.success`。P1ではロケールを ja_jp 固定にしている。
fn translate_success() -> &'static str {
    "成功"
}

/// i18n `ja_jp.failure`。
fn translate_failure() -> &'static str {
    "失敗"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defaults_match_ruby() {
        let c = GameSystemConfig::default();
        assert!(!c.sort_add_dice());
        assert!(!c.sort_barabara_dice());
        assert_eq!(c.d66_sort_type(), D66SortType::NoSort);
        assert!(!c.enabled_d9());
        assert_eq!(c.round_type(), RoundType::Floor);
        assert_eq!(c.sides_implicit_d(), 6);
        assert_eq!(c.upper_dice_reroll_threshold(), None);
        assert_eq!(c.reroll_dice_reroll_threshold(), None);
        assert_eq!(c.default_cmp_op(), None);
        assert_eq!(c.default_target_number(), None);
        assert!(c.enabled_upcase_input());
        assert!(c.prefixes().is_empty());
        assert!(c.prefixes_pattern().is_none());
    }

    #[test]
    fn prefixes_pattern_matches_ruby_shape() {
        // Ruby: /^(S)?(\d*BT|CT)/i
        let re = build_prefixes_pattern(&["\\d*BT", "CT"]);
        assert_eq!(re.as_str(), "(?i)^(S)?(\\d*BT|CT)");

        let m = re.captures("S2BT3+2>=4").expect("matches");
        assert_eq!(m.get(1).map(|x| x.as_str()), Some("S"));
        assert_eq!(m.get(2).map(|x| x.as_str()), Some("2BT"));

        // 選択肢を個別に包むと `^` の効き方が変わる。全体を1グループで包むこと。
        assert!(re.captures("CT").is_some());
        assert!(re.captures("XCT").is_none());
    }
}
