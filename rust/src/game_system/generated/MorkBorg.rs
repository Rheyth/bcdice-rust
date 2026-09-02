//! P4で手書き移植した `lib/bcdice/game_system/MorkBorg.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `MorkBorg#eval_game_system_specific_command`
//!   （`#resolute_action` → `#resolute_initiative` → `#resolute_morale` → `Base#roll_tables`）
//! - `#result_dr` / `#with_symbol`
//!
//! # 表データ
//!
//! Ruby側は `DiceTable::Table.from_i18n("MorkBorg.ERT", locale)` で
//! `i18n/MorkBorg/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として直接持つ。
//! データ部分（`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、
//! 値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`MorkBorg_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `MorkBorg_Korean < MorkBorg` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`MorkBorg` と `MorkBorg_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static Table)],
    /// i18n `MorkBorg.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `MorkBorg.critical`
    pub(crate) critical: &'static str,
    /// i18n `MorkBorg.success`
    pub(crate) success: &'static str,
    /// i18n `MorkBorg.failure`
    pub(crate) failure: &'static str,
    /// i18n `MorkBorg.pcs_go_first`
    pub(crate) pcs_go_first: &'static str,
    /// i18n `MorkBorg.enemies_go_first`
    pub(crate) enemies_go_first: &'static str,
    /// i18n `MorkBorg.maintain`
    pub(crate) maintain: &'static str,
    /// i18n `MorkBorg.flee`
    pub(crate) flee: &'static str,
    /// i18n `MorkBorg.surrender`
    pub(crate) surrender: &'static str,
}

/// i18n `MorkBorg.ERT.items`。
static JA_ERT_ITEMS: &[&str] = &[
    "殺す！",
    "殺す！",
    "激昂",
    "激昂",
    "激昂",
    "無関心",
    "無関心",
    "概ね友好的",
    "概ね友好的",
    "協力的",
    "協力的",
];
/// i18n `MorkBorg.ERT`（予期せぬ反応表 / 2D6）。
static JA_ERT: Table = Table::from_dice("予期せぬ反応表", 2, 6, JA_ERT_ITEMS);

/// i18n `MorkBorg.BRO.items`。
static JA_BRO_ITEMS: &[&str] = &[
    "d4ラウンドの間気絶し、d4HPと共に目を覚ます。",
    "d6を振る: 1–5 = 手足の骨折または切断. 6 = 片目を失う。d4ラウンドの間行動不能となり、その後、d4HPと共にまた動けるようになる。",
    "大出血: 処置しなければd2時間以内に死亡する。最初の1時間はすべての判定がDR16。最後の一時間ではDR18となる。",
    "死ぬ。",
];
/// i18n `MorkBorg.BRO`（瀕死表 / 1D4）。
static JA_BRO: Table = Table::from_dice("瀕死表", 1, 4, JA_BRO_ITEMS);

/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: &[("ERT", &JA_ERT), ("BRO", &JA_BRO)],
    fumble: "ファンブル",
    critical: "クリティカル",
    success: "成功",
    failure: "失敗",
    pcs_go_first: "PCたち",
    enemies_go_first: "敵対者ども",
    maintain: "維持された",
    flee: "(逃亡)",
    surrender: "(降伏)",
};

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `MorkBorg#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = resolute_action(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = resolute_initiative(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = resolute_morale(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(table_helpers::roll_table(command, sys.tables, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `MorkBorg#result_dr`。
///
/// クリティカル・ファンブルは修正前の出目（`dice_total`）だけで決まる。
fn result_dr(sys: &SystemTables, total: i64, dice_total: i64, target: i64) -> EvalResult {
    if dice_total <= 1 {
        EvalResult::fumble(sys.fumble)
    } else if dice_total >= 20 {
        EvalResult::critical(sys.critical)
    } else if total >= target {
        EvalResult::success(sys.success)
    } else {
        EvalResult::failure(sys.failure)
    }
}

/// Ruby `MorkBorg#resolute_action` の `/^([+-]?\d+)?DR(\d+)$/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([+-]?\d+)?DR(\d+)$").expect("valid regex"))
}

/// Ruby `MorkBorg#resolute_initiative` の `/^([+-]?\d+)?INS$/`。
fn initiative_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([+-]?\d+)?INS$").expect("valid regex"))
}

/// Ruby `MorkBorg#resolute_morale` の `/^([+-]?\d+)?MOR(\d+)$/`。
fn morale_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([+-]?\d+)?MOR(\d+)$").expect("valid regex"))
}

/// Ruby `String#to_i` 相当（`nil.to_i` は 0）。
///
/// Ruby の `to_i` は多倍長だが、Rustでは `i64` に飽和させる
/// （桁あふれする入力は実用上ない）。
fn to_i(text: Option<&str>) -> i64 {
    let Some(text) = text else {
        return 0;
    };
    text.parse::<i64>().unwrap_or(if text.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `MorkBorg#with_symbol`。
fn with_symbol(number: i64) -> String {
    if number == 0 {
        "+0".to_owned()
    } else if number > 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `MorkBorg#resolute_action`（DR判定）。
fn resolute_action(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let num_status = to_i(m.get(1).map(|s| s.as_str()));
    let num_target = to_i(m.get(2).map(|s| s.as_str()));

    let total = rng.roll_once(20)?;
    let total_status = format!("{total}{}", with_symbol(num_status));
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる
    // （`to_i` が飽和した巨大な能力値でも panic しないように）。
    let modified = total.saturating_add(num_status);
    let mut result = result_dr(sys, modified, total, num_target);

    // Ruby: sequence = ["(#{command})", total_status, total + num_status, result.text]
    result.text = format!(
        "({command}) ＞ {total_status} ＞ {modified} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `MorkBorg#resolute_initiative`（イニシアティヴ判定）。
fn resolute_initiative(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = initiative_pattern().captures(command) else {
        return Ok(None);
    };

    let num_status = to_i(m.get(1).map(|s| s.as_str()));

    let die = rng.roll_once(6)?;
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる。
    let total = die.saturating_add(num_status);
    let mut result = if total >= 4 {
        EvalResult::success(sys.pcs_go_first)
    } else {
        EvalResult::failure(sys.enemies_go_first)
    };

    result.text = format!(
        "({command}) ＞ {die}{} ＞ {total} ＞ {}",
        with_symbol(num_status),
        result.text
    );
    Ok(Some(result))
}

/// Ruby `MorkBorg#resolute_morale`（モラル判定）。
fn resolute_morale(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = morale_pattern().captures(command) else {
        return Ok(None);
    };

    let num_status = to_i(m.get(1).map(|s| s.as_str()));
    let num_target = to_i(m.get(2).map(|s| s.as_str()));

    let dice_list = rng.roll_barabara(2, 6)?;
    let dice_total: i64 = dice_list.iter().sum();
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる。
    let total = dice_total.saturating_add(num_status);

    // Ruby: die は文字列 "" で初期化し、崩壊した場合だけ 1D6 の出目になる
    let mut die = String::new();
    let mut result = if total <= num_target {
        EvalResult::failure(sys.maintain)
    } else {
        let rolled = rng.roll_once(6)?;
        die = rolled.to_string();
        if rolled >= 4 {
            EvalResult::success(sys.surrender)
        } else {
            EvalResult::success(sys.flee)
        }
    };

    result.text = format!(
        "({command}) ＞ {dice_total}{} ＞ {total} ＞ {die}{}",
        with_symbol(num_status),
        result.text
    );
    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::MorkBorg`（ID: `MorkBorg`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorkBorg;

impl GameSystem for MorkBorg {
    fn id(&self) -> &'static str {
        "MorkBorg"
    }

    fn name(&self) -> &'static str {
        "MÖRK BORG"
    }

    fn sort_key(&self) -> &'static str {
        "むるくほりい"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　sDRt        s: 能力値(省略時:0) t:目標値

例)+3DR12: 能力値+3、DR12で1d20を振って、その結果を表示(クリティカル・ファンブルも表示)

■イニシアティヴ　sINS s: 能力値(省略時:0. 個別のイニシアティブを使う場合)

例)INS: 1d6を振って、イニシアティヴの結果を表示(PC先行を成功として表示)

■モラル　sMORt s: 能力値(省略時:0) t:相手クリーチャーのモラル値

例)MOR8: 2d6を振って、モラル判定の結果を表示(モラル崩壊を成功として表示)


■各種表

・遭遇反応表 Reaction (ERT)
・破損 Broken (BRO)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"([+-]?\d+)?DR[\d]+",
            r"([+-]?\d+)?INS",
            r"([+-]?\d+)?MOR",
            "ERT",
            "BRO",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `MorkBorg#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/MorkBorg.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/MorkBorg.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/MorkBorg.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("MorkBorg.toml must parse");
        assert_eq!(
            data.tests.len(),
            42,
            "case count in test/data/MorkBorg.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "MorkBorg",
                "unexpected game system in MorkBorg.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("MorkBorg"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push(format!(
                            "eval returned nil, but output was expected: {:?}",
                            tc.output
                        ));
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() {
                        reasons.push(format!("expected nil output, got {:?}", result.text));
                    } else if result.text != tc.output {
                        reasons.push(format!(
                            "output mismatch\n    expected: {:?}\n    actual:   {:?}",
                            tc.output, result.text
                        ));
                    }
                    check_flag(&mut reasons, "secret", tc.secret, result.secret);
                    check_flag(&mut reasons, "success", tc.success, result.success);
                    check_flag(&mut reasons, "failure", tc.failure, result.failure);
                    check_flag(&mut reasons, "critical", tc.critical, result.critical);
                    check_flag(&mut reasons, "fumble", tc.fumble, result.fumble);
                }
            }

            if !src.is_empty() {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL MorkBorg:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} MorkBorg cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 桁あふれする能力値でも panic しないこと。
    ///
    /// Ruby の Integer は多倍長なのでそのまま計算されるが、Rustでは `to_i` が
    /// `i64::MAX` に飽和し、以降の加算も飽和演算になる。TOMLにこの経路のケースが
    /// 無いのでここで固定する（デバッグビルドはオーバーフローで panic するため）。
    #[test]
    fn huge_status_saturates_instead_of_panicking() {
        let cases = [
            (
                "99999999999999999999DR12",
                vec![(12, 20)],
                "(99999999999999999999DR12) ＞ 12+9223372036854775807 ＞ 9223372036854775807 ＞ 成功",
            ),
            (
                "99999999999999999999INS",
                vec![(4, 6)],
                "(99999999999999999999INS) ＞ 4+9223372036854775807 ＞ 9223372036854775807 ＞ PCたち",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("MorkBorg"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
