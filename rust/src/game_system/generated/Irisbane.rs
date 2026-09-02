//! P4で手書き移植した `lib/bcdice/game_system/Irisbane.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Irisbane#eval_game_system_specific_command`（`ALIAS` 解決 → `#roll_attack` →
//!   `Base#roll_tables`）
//! - `#roll_attack` / `#make_command_text` / `#parse_operator`
//!
//! # 表・定型文データ
//!
//! Ruby側は `DiceTable::D66LeftRangeTable.from_i18n('Irisbane.SceneSituation', locale)` で
//! `i18n/Irisbane/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として直接持つ。
//! データ部分（`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、
//! 値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`Irisbane_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `Irisbane_Korean < Irisbane` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{D66LeftRangeTable, RangeInc};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::int_helpers::int_clamp;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`Irisbane` と `Irisbane_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static D66LeftRangeTable)],
    /// i18n `Irisbane.zero_dice_count`
    pub(crate) zero_dice_count: &'static str,
    /// i18n `Irisbane.success_dice_count`（`%{count}` を数で置換する）
    pub(crate) success_dice_count: &'static str,
    /// i18n `Irisbane.attack_power`（`%{power}` を数で置換する）
    pub(crate) attack_power: &'static str,
    /// i18n `Irisbane.damage`（`%{damage}` を数で置換する）
    pub(crate) damage: &'static str,
    /// i18n `Irisbane.damage_with_mod`（`%{damage}%{operator}%{mod_value}` を置換する）
    pub(crate) damage_with_mod: &'static str,
}

/// `1..3` の行。
static JA_SCENE_SITUATION_ROW0: &[&str] = &[
    "【日常】何一つ変わることの無い日々の一幕。移ろい易い世界では、それはとても大切である。",
    "【準備】何かを為すための用意をする一幕。情報収集、買物遠征、やるべきことは一杯だ。",
    "【趣味】自分の時間を、有効活用している一幕。必要に追われていない分、心は軽く晴れやかだ。",
    "【喫茶】一息入れ、嗜好品を嗜む時の一幕。穏やかな空気は、だが、往々にして変わりやすい。",
    "【鍛錬】体を鍛え、心を養う修練の一幕。己さえ良ければ、その方法も何だって良い。",
    "【職務】役割の元、仕事に精を出す時の一幕。目的が何であれ、為すべきことに変わりはない。",
];
/// `4..6` の行。
static JA_SCENE_SITUATION_ROW1: &[&str] = &[
    "【移動】何処かから何処かへと向かう一幕。進んでいるなら、手段も目的地も関係あるまい。",
    "【墓前】故人が眠る場所へと赴く一幕。共に眠ることだけは無いように。",
    "【操作】何かを操り、望みを果たしている一幕。運転にせよ何にせよ、脇見には注意が必要だ。",
    "【食事】何かを糧とし、己の力を蓄える一幕。行動すれば消耗する。腹が減っては何とやらだ。",
    "【休息】日々の合間の、憩いの一幕。「何もしない」というのも、立派な行いである。",
    "【夢幻】現実に存在しない何かへと耽る一幕。時間帯に関わらず、何時かは必ず覚めるだろう。",
];
static JA_SCENE_SITUATION_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 3), JA_SCENE_SITUATION_ROW0),
    (RangeInc::new(4, 6), JA_SCENE_SITUATION_ROW1),
];
static JA_SCENE_SITUATION: D66LeftRangeTable = D66LeftRangeTable::new(
    "シチュエーション",
    D66SortType::NoSort,
    JA_SCENE_SITUATION_ITEMS,
);
/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: &[("SCENESITUATION", &JA_SCENE_SITUATION)],
    zero_dice_count: "判定数が 0 です",
    success_dice_count: "成功ダイス数 %{count}",
    attack_power: "× 攻撃力 %{power}",
    damage: "ダメージ %{damage}",
    damage_with_mod: "ダメージ %{damage}%{operator}%{mod_value}",
};

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `ALIAS`（キー・値とも `upcase` 済み）。
static ALIAS: &[(&str, &str)] = &[("SSI", "SCENESITUATION")];

/// Ruby `Irisbane::ATTACK_ROLL_REG`。
///
/// Ruby: `%r{^AT(TACK|K)?([+\-*/()\d]+)@([+\-*/()\d]+)<=([+\-*/()\d]+)(\[([+-])([+\-*/()\d]+)\])?}i`
fn attack_roll_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^AT(TACK|K)?([+\-*/()\d]+)@([+\-*/()\d]+)<=([+\-*/()\d]+)(\[([+-])([+\-*/()\d]+)\])?")
            .expect("valid regex")
    })
}

/// Ruby `Irisbane#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: command = ALIAS[command] || command
    let command = ALIAS
        .iter()
        .find(|(key, _)| *key == command)
        .map_or(command, |(_, value)| *value);

    if let Some(m) = attack_roll_pattern().captures(command) {
        return roll_attack(
            sys,
            &m[2],
            &m[3],
            &m[4],
            m.get(6).map(|c| c.as_str()),
            m.get(7).map(|c| c.as_str()),
            rng,
        );
    }
    Ok(roll_tables(sys, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, sys.tables)`。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    table_helpers::roll_table(command, sys.tables, rng)
}

/// Ruby `Irisbane#make_command_text`。
fn make_command_text(
    power: &I,
    dice_count: &I,
    border: &I,
    modification_operator: Option<&str>,
    modification_value: Option<&I>,
) -> String {
    let mut text = format!("(ATTACK{power}@{dice_count}<={border}");
    if let Some(op) = modification_operator {
        // Ruby: modification_value は operator があれば必ず非nil（上で早期returnしている）
        text.push_str(&format!("[{op}{}]", modification_value.unwrap_or(&I::ZERO)));
    }
    text.push(')');
    text
}

/// Ruby `Irisbane#parse_operator`。
fn apply_operator(operator: &str, x: I, y: &I) -> I {
    match operator {
        // Ruby の Integer は多倍長なので桁あふれしない（B18でBigInt化済み）。
        "+" => x + y,
        "-" => x - y,
        // Ruby: どちらでもなければ `nil.call` で NoMethodError。
        // 正規表現が `[+-]` に限定しているので到達しない。
        _ => unreachable!("modification operator is restricted to + or - by the regexp"),
    }
}

/// Ruby `Irisbane#roll_attack`。
#[allow(clippy::too_many_arguments)]
fn roll_attack(
    sys: &SystemTables,
    power_expression: &str,
    dice_count_expression: &str,
    border_expression: &str,
    modification_operator: Option<&str>,
    modification_expression: Option<&str>,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby: Arithmetic.eval(..., RoundType::CEIL)
    let power = arithmetic::eval(power_expression, RoundType::Ceil)?;
    let dice_count = arithmetic::eval(dice_count_expression, RoundType::Ceil)?;
    let border = arithmetic::eval(border_expression, RoundType::Ceil)?;
    let modification_value = match modification_expression {
        None => None,
        Some(expr) => arithmetic::eval(expr, RoundType::Ceil)?,
    };

    // Ruby: return if power.nil? || dice_count.nil? || border.nil?
    let (Some(mut power), Some(dice_count), Some(border)) = (power, dice_count, border) else {
        return Ok(None);
    };
    // Ruby: return if modification_operator && modification_value.nil?
    if modification_operator.is_some() && modification_value.is_none() {
        return Ok(None);
    }

    if power < I::ZERO {
        power = I::ZERO;
    }
    // Ruby: border.clamp(1, 6)
    let border = int_clamp(&border, &crate::Int::from(1), &crate::Int::from(6));

    let command = make_command_text(
        &power,
        &dice_count,
        &border,
        modification_operator,
        modification_value.as_ref(),
    );

    if dice_count <= crate::Int::ZERO {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "{command} ＞ {}",
            sys.zero_dice_count
        ))));
    }

    let mut dices = rng.roll_barabara(crate::randomizer::sat_i64(&dice_count), 6)?;
    dices.sort_unstable();

    let success_dice_count = dices
        .iter()
        .filter(|d| **d <= crate::randomizer::sat_i64(&border))
        .count() as i64;
    // Ruby の Integer は多倍長なので桁あふれしない（B18でBigInt化済み）。
    let mut damage = I::from(success_dice_count) * &power;

    let mut message_elements: Vec<String> = Vec::new();
    message_elements.push(command);
    message_elements.push(
        dices
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    message_elements.push(
        sys.success_dice_count
            .replace("%{count}", &success_dice_count.to_string()),
    );
    if success_dice_count > 0 {
        message_elements.push(sys.attack_power.replace("%{power}", &power.to_string()));
    }

    if success_dice_count > 0 {
        match (modification_operator, modification_value) {
            (Some(op), Some(mod_value)) => {
                message_elements.push(
                    sys.damage_with_mod
                        .replace("%{damage}", &damage.to_string())
                        .replace("%{operator}", op)
                        .replace("%{mod_value}", &mod_value.to_string()),
                );
                damage = apply_operator(op, damage, &mod_value);
                if damage < I::ZERO {
                    damage = I::ZERO;
                }
                message_elements.push(damage.to_string());
            }
            _ => {
                message_elements.push(sys.damage.replace("%{damage}", &damage.to_string()));
            }
        }
    }

    let mut result = EvalResult::with_text(message_elements.join(" ＞ "));
    result.set_condition(success_dice_count > 0);
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// Ruby `BCDice::GameSystem::Irisbane`（ID: `Irisbane`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Irisbane;

impl GameSystem for Irisbane {
    fn id(&self) -> &'static str {
        "Irisbane"
    }

    fn name(&self) -> &'static str {
        "瞳逸らさぬイリスベイン"
    }

    fn sort_key(&self) -> &'static str {
        "ひとみそらさぬいりすへいん"
    }

    fn help_message(&self) -> &'static str {
        r"■攻撃判定（ ATTACKx@y<=z ）
x: 攻撃力
y: 判定数
z: 目標値
（※ ATTACK は ATK または AT と簡略化可能）
例） ATTACK2@3<=5
例） ATK10@2<=4
例） AT8@3<=2

上記 x y z にはそれぞれ四則演算を指定可能。
例） ATTACK2+7@3*2<=5-1

□攻撃判定のダメージ増減（ ATTACKx@y<=z[+a]  ATTACKx@y<=z[-a]）
末尾に [+a] または [-a] と指定すると、最終的なダメージを増減できる。
a: 増減量
例） ATTACK2@3<=5[+10]
例） ATK10@2<=4[-8]
例） AT8@3<=2[-8+5]

■シチュエーション（p115）
SceneSituation, SSi
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["AT(TACK|K)?", "SCENESITUATION", "SSI"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Irisbane#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Irisbane#initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `Irisbane#eval_game_system_specific_command`。
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
            .join("test/data/Irisbane.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Irisbane.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Irisbane.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Irisbane.toml must parse");
        assert_eq!(
            data.tests.len(),
            41,
            "case count in test/data/Irisbane.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Irisbane",
                "unexpected game system in Irisbane.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Irisbane"), &tc.input, &mut src) {
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
                    "FAIL Irisbane:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Irisbane cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 桁あふれする攻撃力・増減量でも panic しないこと。
    ///
    /// B18（多倍長整数化）により、Ruby 本家と同じく多倍長のまま計算される。
    /// 期待値は Ruby 本家（`bin/fuzz_runner.rb` と同一の SeedRandomizer で
    /// `ATTACK99999999999999999999@2<=5[+99999999999999999999]` を実行）した実測値。
    #[test]
    fn huge_power_saturates_instead_of_panicking() {
        let mut src = SeededRandomizer::new(vec![(1, 6), (3, 6)]);
        let result = eval_command(
            &GameSystemId::new("Irisbane"),
            "ATTACK99999999999999999999@2<=5[+99999999999999999999]",
            &mut src,
        )
        .expect("eval")
        .expect("result");
        assert_eq!(
            result.text,
            concat!(
                "(ATTACK99999999999999999999@2<=5[+99999999999999999999])",
                " ＞ 1,3 ＞ 成功ダイス数 2 ＞ × 攻撃力 99999999999999999999",
                " ＞ ダメージ 199999999999999999998+99999999999999999999",
                " ＞ 299999999999999999997"
            )
        );
        assert!(src.is_empty(), "unconsumed rands");
    }
}
