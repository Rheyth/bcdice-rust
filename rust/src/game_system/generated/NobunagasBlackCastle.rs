//! P4で手書き移植した `lib/bcdice/game_system/NobunagasBlackCastle.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `NobunagasBlackCastle#eval_game_system_specific_command`
//!   （`#resolute_action` → `#resolute_initiative` → `Base#roll_tables` → `#make_npc_status`）
//! - `#result_dr` / `#with_symbol` / `#calc_status`
//!
//! 表データ（`TABLES`）は `lib/bcdice/game_system/NobunagasBlackCastle.rb` から
//! 機械的に書き出したもので、値は1文字も変えていない。
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `TABLES['OSWT']`（その他の奇妙な武器表 / 1D10）の項目。
static OSWT_ITEMS: &[&str] = &[
    "六尺棒（D4）",
    "手槍（D4）",
    "弓矢（D6）",
    "鉄扇（D4）",
    "大鉞（D8）",
    "吹き矢（D2）＋感染",
    "鞭（D3）",
    "熊手（D4）",
    "石つぶて（D3）",
    "丸太（D4）",
];
static OSWT: Table = Table::from_dice("その他の奇妙な武器表", 1, 10, OSWT_ITEMS);

/// Ruby `TABLES['SWT']`（武器表 / 1D12）の項目。
static SWT_ITEMS: &[&str] = &[
    "尖らせた骨の杭（D3）",
    "竹槍（D4）",
    "百姓から奪った鍬（D4）",
    "脇差し（D4）",
    "手裏剣　D6本（D4）",
    "刀（D6）",
    "鎖鎌（D6）",
    "太刀（D8）",
    "種子島銃（2D6）　弾丸（心+5）発",
    "大槍（D8）",
    "爆裂弾（D4）　心+3発",
    "斬馬刀（D10）",
];
static SWT: Table = Table::from_dice("武器表", 1, 12, SWT_ITEMS);

/// Ruby `TABLES['ART']`（鎧表 / 1D6）の項目。
static ART_ITEMS: &[&str] = &[
    "防具は、何もない",
    "防具は、何もない",
    "部分鎧（腹巻き）　-D2ダメージ",
    "お貸し具足　-D3ダメージ",
    "武者鎧　-D4ダメージ",
    "大鎧　-D6ダメージ",
];
static ART: Table = Table::from_dice("鎧表", 1, 6, ART_ITEMS);

/// Ruby `TABLES['ERT']`（遭遇反応表 / 2D6）の項目。
static ERT_ITEMS: &[&str] = &[
    "お前ら、殺す！",
    "お前ら、殺す！",
    "憎悪の視線で睨んでくる。すきを見せれば、攻撃してくる。",
    "憎悪の視線で睨んでくる。すきを見せれば、攻撃してくる。",
    "憎悪の視線で睨んでくる。すきを見せれば、攻撃してくる。",
    "警戒はしているが、特に、戦闘は望んでいない。怒らせなければ、自分たちの目的に沿って動く。",
    "警戒はしているが、特に、戦闘は望んでいない。怒らせなければ、自分たちの目的に沿って動く。",
    "中立。何かを与えたり、取引の材料を提示したりできれば、交渉できそうだ。",
    "中立。何かを与えたり、取引の材料を提示したりできれば、交渉できそうだ。",
    "好意的に会話できそうだ。向こうも取引したがっている。",
    "好意的に会話できそうだ。向こうも取引したがっている。",
];
static ERT: Table = Table::from_dice("遭遇反応表", 2, 6, ERT_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[("OSWT", &OSWT), ("SWT", &SWT), ("ART", &ART), ("ERT", &ERT)];

/// Ruby `NobunagasBlackCastle#resolute_action` の `/^([+-]?\d*)DR(\d+)$/`。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([+-]?\d*)DR(\d+)$").expect("valid regex"))
}

/// Ruby `String#to_i`。
///
/// `([+-]?\d*)` は `"+"` や `""` にもマッチし、Ruby ではどちらも 0 になる。
/// Ruby の `to_i` は多倍長だが、Rustでは `i64` に飽和させる。
fn to_i(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or_else(|_| {
        if text.starts_with('-') && text.len() > 1 {
            i64::MIN
        } else if text.chars().any(|c| c.is_ascii_digit()) {
            i64::MAX
        } else {
            // `""` / `"+"` / `"-"` は Ruby でも 0
            0
        }
    })
}

/// Ruby `NobunagasBlackCastle#with_symbol`。
fn with_symbol(number: i64) -> String {
    if number == 0 {
        "+0".to_owned()
    } else if number > 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}

/// Ruby `NobunagasBlackCastle#result_dr`。
///
/// クリティカル・ファンブルは修正前の出目（`dice_total`）だけで決まる。
fn result_dr(total: i64, dice_total: i64, target: i64) -> EvalResult {
    if dice_total <= 1 {
        EvalResult::fumble("ファンブル")
    } else if dice_total >= 20 {
        EvalResult::critical("クリティカル")
    } else if total >= target {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

/// Ruby `NobunagasBlackCastle#resolute_action`（DR判定）。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    let num_status = to_i(&m[1]);
    let num_target = to_i(&m[2]);

    let total = rng.roll_once(20)?;
    let total_status = format!("{total}{}", with_symbol(num_status));
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる
    // （`to_i` が飽和した巨大な能力値でも panic しないように）。
    let modified = total.saturating_add(num_status);
    let mut result = result_dr(modified, total, num_target);

    // Ruby: sequence = ["(#{command})", total_status, total + num_status, result.text]
    result.text = format!(
        "({command}) ＞ {total_status} ＞ {modified} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `NobunagasBlackCastle#resolute_initiative`（イニシアティヴ判定）。
fn resolute_initiative(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    if command != "INS" {
        return Ok(None);
    }

    let total = rng.roll_once(6)?;
    let mut result = if total >= 4 {
        EvalResult::success("PC先行")
    } else {
        EvalResult::failure("敵先行")
    };

    result.text = format!("({command}) ＞ {total} ＞ {}", result.text);
    Ok(Some(result))
}

/// Ruby `NobunagasBlackCastle#calc_status`（能力値 → 修正値）。
///
/// Ruby は 21 以上で `nil` を返すが、`3D6` の合計は 18 までなので到達しない。
fn calc_status(st: i64) -> i64 {
    if st <= 4 {
        -3
    } else if st <= 6 {
        -2
    } else if st <= 8 {
        -1
    } else if st <= 12 {
        0
    } else if st <= 14 {
        1
    } else if st <= 16 {
        2
    } else {
        3
    }
}

/// Ruby `NobunagasBlackCastle#make_npc_status`（NPC能力値作成）。
fn make_npc_status(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    if command != "NPCST" {
        return Ok(None);
    }

    let pre = rng.roll_sum(3, 6)?;
    let agi = rng.roll_sum(3, 6)?;
    let str_ = rng.roll_sum(3, 6)?;
    let tgh = rng.roll_sum(3, 6)?;
    let hpd = rng.roll_once(8)?;
    let mut hp = hpd + calc_status(tgh);
    if hp < 1 {
        hp = 1;
    }

    let text = [
        format!("心{}({pre})", with_symbol(calc_status(pre))),
        format!("技{}({agi})", with_symbol(calc_status(agi))),
        format!("体{}({str_})", with_symbol(calc_status(str_))),
        format!("耐久{}({tgh})", with_symbol(calc_status(tgh))),
        format!("HP{hp}({hpd})"),
    ]
    .join(", ");

    Ok(Some(format!("({command}) ＞ {text}")))
}

/// Ruby `BCDice::GameSystem::NobunagasBlackCastle`（ID: `NobunagasBlackCastle`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NobunagasBlackCastle;

impl GameSystem for NobunagasBlackCastle {
    fn id(&self) -> &'static str {
        "NobunagasBlackCastle"
    }

    fn name(&self) -> &'static str {
        "信長の黒い城"
    }

    fn sort_key(&self) -> &'static str {
        "のふなかのくろいしろ"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　sDRt        s: 能力値 t:目標値

例)+3DR12: 能力値+3、DR12で1d20を振って、その結果を表示(クリティカル・ファンブルも表示)

■イニシアティヴ　INS

例)INS: 1d6を振って、イニシアティヴの結果を表示(PC先行を成功として表示)

■NPC能力値作成　NPCST

例)NPCST: 3d6を4回振って、各能力値とHPを表示


■各種表

・遭遇反応表(ERT)
・武器表(SWT)/その他の奇妙な武器表(OSWT)
・鎧表(ART)

"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"[+-]?\d*DR[\d]+",
            "INS",
            "NPCST",
            "OSWT",
            "SWT",
            "ART",
            "ERT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `NobunagasBlackCastle#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `NobunagasBlackCastle#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = resolute_initiative(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(text) = table_helpers::roll_table(command, TABLES, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(make_npc_status(command, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/NobunagasBlackCastle.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/NobunagasBlackCastle.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/NobunagasBlackCastle.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("NobunagasBlackCastle.toml must parse");
        assert_eq!(
            data.tests.len(),
            63,
            "case count in test/data/NobunagasBlackCastle.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "NobunagasBlackCastle",
                "unexpected game system in NobunagasBlackCastle.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("NobunagasBlackCastle"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL NobunagasBlackCastle:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} NobunagasBlackCastle cases failed:\n{}",
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
        let mut src = SeededRandomizer::new(vec![(12, 20)]);
        let result = eval_command(
            &GameSystemId::new("NobunagasBlackCastle"),
            "99999999999999999999DR12",
            &mut src,
        )
        .expect("eval")
        .expect("result");
        assert_eq!(
            result.text,
            "(99999999999999999999DR12) ＞ 12+9223372036854775807 ＞ 9223372036854775807 ＞ 成功"
        );
        assert!(src.is_empty(), "unconsumed rands");
    }
}
