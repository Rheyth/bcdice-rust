//! P4で手書き移植した `lib/bcdice/game_system/BadLife.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `BadLife#eval_game_system_specific_command`（`#judgeDice` → `Base#roll_tables`）
//! - `#judgeDice` / `#get_critival_fumble` / `#checkRoll` / `#get_value`
//!
//! 表データ（`TABLES`）は `lib/bcdice/game_system/BadLife.rb` から機械的に
//! 書き出したもので、値は1文字も変えていない。
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::Table;
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{str_helpers, table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `TABLES["SKL"]`（スキル表 / 1D100）の項目。
static SKL_ITEMS: &[&str] = &[
    "一撃離脱",
    "一撃離脱",
    "チェイサー",
    "チェイサー",
    "影の外套",
    "影の外套",
    "二段ジャンプ",
    "二段ジャンプ",
    "韋駄天",
    "韋駄天",
    "手練",
    "手練",
    "ハニーテイスト",
    "ハニーテイスト",
    "先見の明",
    "先見の明",
    "ベテラン",
    "ベテラン",
    "応急手当",
    "応急手当",
    "セラピー",
    "セラピー",
    "緊急治療",
    "緊急治療",
    "ゴールドディガー",
    "ゴールドディガー",
    "デイリーミッション",
    "デイリーミッション",
    "見切り",
    "見切り",
    "鷹の目",
    "鷹の目",
    "しびれ罠",
    "しびれ罠",
    "大逆転",
    "大逆転",
    "武器習熟：○○",
    "武器習熟：○○",
    "百発百中",
    "百発百中",
    "屈強な肉体",
    "屈強な肉体",
    "二刀流",
    "二刀流",
    "クイックリカバリー",
    "クイックリカバリー",
    "体験主義",
    "体験主義",
    "破釜沈船",
    "破釜沈船",
    "想定の範囲内",
    "想定の範囲内",
    "セカンドチャンス",
    "セカンドチャンス",
    "優秀な子分",
    "優秀な子分",
    "時間管理術",
    "時間管理術",
    "連撃術",
    "連撃術",
    "罵詈雑言",
    "罵詈雑言",
    "ケセラセラ",
    "ケセラセラ",
    "ダンス＆ミュージック",
    "ダンス＆ミュージック",
    "フェイント",
    "フェイント",
    "ヘイトコントロール",
    "ヘイトコントロール",
    "惜別",
    "惜別",
    "戦闘マシーン",
    "戦闘マシーン",
    "戦闘マシーン",
    "名医",
    "名医",
    "名医",
    "忍者",
    "忍者",
    "忍者",
    "観察眼",
    "観察眼",
    "観察眼",
    "クレバー",
    "クレバー",
    "クレバー",
    "フェイスマン",
    "フェイスマン",
    "フェイスマン",
    "スポーツマン",
    "スポーツマン",
    "スポーツマン",
    "不屈",
    "不屈",
    "不屈",
    "慎重",
    "慎重",
    "慎重",
    "この表を2回振る",
];
static SKL: Table = Table::from_dice("スキル表", 1, 100, SKL_ITEMS);

/// Ruby `TABLES["TRN"]`（怪盗コードネーム表 / 1D20）の項目。
static TRN_ITEMS: &[&str] = &[
    "フォックス",
    "フォックス",
    "ラット",
    "ラット",
    "キャット",
    "キャット",
    "タイガー",
    "タイガー",
    "シャーク",
    "シャーク",
    "コンドル",
    "コンドル",
    "スパイダー",
    "スパイダー",
    "ウルフ",
    "ウルフ",
    "コヨーテ",
    "コヨーテ",
    "ジャガー",
    "ジャガー",
];
static TRN: Table = Table::from_dice("怪盗コードネーム表", 1, 20, TRN_ITEMS);

/// Ruby `TABLES["DRN"]`（闇医者コードネーム表 / 1D20）の項目。
static DRN_ITEMS: &[&str] = &[
    "キャンサー",
    "キャンサー",
    "ヘッドエイク",
    "ヘッドエイク",
    "ブラッド",
    "ブラッド",
    "ウーンズ",
    "ウーンズ",
    "ポイズン",
    "ポイズン",
    "ペイン",
    "ペイン",
    "スリープ",
    "スリープ",
    "キュア",
    "キュア",
    "デス",
    "デス",
    "リーンカーネイション",
    "リーンカーネイション",
];
static DRN: Table = Table::from_dice("闇医者コードネーム表", 1, 20, DRN_ITEMS);

/// Ruby `TABLES["GRN"]`（博徒コードネーム表 / 1D20）の項目。
static GRN_ITEMS: &[&str] = &[
    "リトルダイス",
    "リトルダイス",
    "プラチナム",
    "プラチナム",
    "プレジデント",
    "プレジデント",
    "ドリーム",
    "ドリーム",
    "アクシデント",
    "アクシデント",
    "グリード",
    "グリード",
    "フォーチュン",
    "フォーチュン",
    "ミラクル",
    "ミラクル",
    "ホープ",
    "ホープ",
    "ビッグヒット",
    "ビッグヒット",
];
static GRN: Table = Table::from_dice("博徒コードネーム表", 1, 20, GRN_ITEMS);

/// Ruby `TABLES["KRN"]`（殺シ屋コードネーム表 / 1D20）の項目。
static KRN_ITEMS: &[&str] = &[
    "ハンマー",
    "ハンマー",
    "アロー",
    "アロー",
    "ボマー",
    "ボマー",
    "キャノン",
    "キャノン",
    "ブレード",
    "ブレード",
    "スティング",
    "スティング",
    "ガロット",
    "ガロット",
    "パイルバンカー",
    "パイルバンカー",
    "レイザー",
    "レイザー",
    "カタナ",
    "カタナ",
];
static KRN: Table = Table::from_dice("殺シ屋コードネーム表", 1, 20, KRN_ITEMS);

/// Ruby `TABLES["SRN"]`（業師コードネーム表 / 1D20）の項目。
static SRN_ITEMS: &[&str] = &[
    "ローズ",
    "ローズ",
    "サクラ",
    "サクラ",
    "ライラック",
    "ライラック",
    "ダンデライオン",
    "ダンデライオン",
    "フリージア",
    "フリージア",
    "カクタス",
    "カクタス",
    "ロータス",
    "ロータス",
    "リリィ",
    "リリィ",
    "ラフレシア",
    "ラフレシア",
    "ヒヤシンス",
    "ヒヤシンス",
];
static SRN: Table = Table::from_dice("業師コードネーム表", 1, 20, SRN_ITEMS);

/// Ruby `TABLES["BRN"]`（遊ビ人コードネーム表 / 1D20）の項目。
static BRN_ITEMS: &[&str] = &[
    "モノポリー",
    "モノポリー",
    "ブリッジ",
    "ブリッジ",
    "チェッカー",
    "チェッカー",
    "アクワイア",
    "アクワイア",
    "ジャンケン",
    "ジャンケン",
    "トランプ",
    "トランプ",
    "ケイドロ",
    "ケイドロ",
    "パンデミック",
    "パンデミック",
    "スゴロク",
    "スゴロク",
    "キャベツカンテイ",
    "キャベツカンテイ",
];
static BRN: Table = Table::from_dice("遊ビ人コードネーム表", 1, 20, BRN_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[
    ("SKL", &SKL),
    ("TRN", &TRN),
    ("DRN", &DRN),
    ("GRN", &GRN),
    ("KRN", &KRN),
    ("SRN", &SRN),
    ("BRN", &BRN),
];

/// Ruby `BadLife#judgeDice` の判定コマンド正規表現。
///
/// Ruby:
/// `/(\d+)?(BAD|BL|GL)([-+\d]*)((C|F)([-+\d]*)?)?((C|F)([-+\d]*))?(@([-+\d]*))?(!(\D*))?/i`
///
/// 先頭が固定されていない（`^` が無い）ので、原典どおり途中一致も許す。
fn judge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(\d+)?(BAD|BL|GL)([-+\d]*)((C|F)([-+\d]*)?)?((C|F)([-+\d]*))?(@([-+\d]*))?(!(\D*))?",
        )
        .expect("valid regex")
    })
}

/// Ruby `BadLife#get_value`。
///
/// `ArithmeticEvaluator.eval` は `Arithmetic.eval(expr, FLOOR) || 0`、
/// つまり式が不正なら 0 を返す。`nil` も `""` として 0 になる。
fn get_value(text: Option<&str>) -> Result<i64, EvalError> {
    let text = text.unwrap_or("");
    Ok(arithmetic::eval(text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `BadLife#get_critival_fumble`（原典の綴りのまま）。
fn get_critical_fumble(
    critical: i64,
    fumble: i64,
    marker: Option<&str>,
    text: Option<&str>,
) -> Result<(i64, i64), EvalError> {
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる
    // （`Arithmetic.eval` は桁あふれした数値リテラルを `i64::MAX` に丸める）。
    match marker {
        Some("C") => Ok((critical.saturating_add(get_value(text)?), fumble)),
        Some("F") => Ok((critical, fumble.saturating_add(get_value(text)?))),
        _ => Ok((critical, fumble)),
    }
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_max(digits)
}

/// Ruby `BadLife#judgeDice`。
fn judge_dice(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = judge_pattern().captures(command) else {
        return Ok(None);
    };

    let dice_count = m.get(1).map_or(1, |c| to_i(c.as_str()));

    let mut critical = 20;
    let mut fumble = 1;

    // 波乱万丈
    let is_stormy = m.get(2).map(|c| c.as_str()) == Some("GL");
    if is_stormy {
        critical -= 3;
        fumble += 1;
    }

    let modify = get_value(m.get(3).map(|c| c.as_str()))?;

    (critical, fumble) = get_critical_fumble(
        critical,
        fumble,
        m.get(5).map(|c| c.as_str()),
        m.get(6).map(|c| c.as_str()),
    )?;
    (critical, fumble) = get_critical_fumble(
        critical,
        fumble,
        m.get(8).map(|c| c.as_str()),
        m.get(9).map(|c| c.as_str()),
    )?;

    let target = get_value(m.get(11).map(|c| c.as_str()))?;
    let optional_text = m.get(13).map_or("", |c| c.as_str());

    check_roll(
        dice_count,
        modify,
        critical,
        fumble,
        target,
        is_stormy,
        optional_text,
        rng,
    )
}

/// Ruby `BadLife#checkRoll`。
///
/// ダイスを1個も振らない（`0BAD`）と Ruby は `dice_list.max` が `nil` になり
/// `nil >= critical` で NoMethodError を送出してクラッシュする。
/// 本移植は他のコマンドと同じく「解釈できないコマンド＝nil」に畳む。
#[allow(clippy::too_many_arguments)]
fn check_roll(
    dice_count: i64,
    modify: i64,
    critical: i64,
    fumble: i64,
    target: i64,
    is_stormy: bool,
    optional_text: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    // 先見の明
    let is_anticipation = optional_text.contains('A');
    // 重撃
    let is_heavy_attack = optional_text.contains('H');

    let dice_list = rng.roll_barabara(dice_count, 20)?;
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // Ruby: dice_list.max（0個なら nil。上のdocのとおり nil に畳む）
    let Some(mut dice_max) = dice_list.iter().copied().max() else {
        return Ok(None);
    };

    // 重撃
    if is_heavy_attack && dice_max <= 5 {
        dice_max = 5;
    }

    let is_critical = dice_max >= critical;
    let is_fumble = dice_max <= fumble;

    // クリティカル
    if is_critical {
        dice_max = 20;
    }
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる。
    let mut total = dice_max.saturating_add(modify);
    // 先見の明
    if is_anticipation && dice_max <= 7 {
        total = total.saturating_add(5);
    }
    // ファンブル
    if is_fumble {
        total = 0;
    }

    let mut result = format!("{dice_count}D20(C:{critical},F:{fumble}) ＞ ");
    result.push_str(&format!("{dice_max}[{dice_text}]"));
    if modify > 0 {
        result.push('+');
    }
    if modify != 0 {
        result.push_str(&modify.to_string());
    }
    // 先見の明
    if is_anticipation && dice_max <= 7 {
        result.push_str("+5");
    }
    result.push_str(&format!(" ＞ 達成値：{total}"));

    if target > 0 {
        let success = total.saturating_sub(target);
        result.push_str(&format!(">={target} 成功度：{success} ＞ "));

        if is_critical {
            result.push_str("成功（クリティカル）");
        } else if total >= target {
            result.push_str("成功");
        } else {
            result.push_str("失敗");
            if is_fumble {
                result.push_str("（ファンブル）");
            }
        }
    } else {
        if is_critical {
            result.push_str(" クリティカル");
        }
        if is_fumble {
            result.push_str(" ファンブル");
        }
    }

    let mut skill_text = String::new();
    if is_stormy {
        skill_text.push_str("〈波乱万丈〉");
    }
    if is_anticipation {
        skill_text.push_str("〈先見の明〉");
    }
    if is_heavy_attack {
        skill_text.push_str("［重撃］");
    }
    if !skill_text.is_empty() {
        result.push_str(&format!(" {skill_text}"));
    }

    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::BadLife`（ID: `BadLife`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadLife;

impl GameSystem for BadLife {
    fn id(&self) -> &'static str {
        "BadLife"
    }

    fn name(&self) -> &'static str {
        "バッドライフ"
    }

    fn sort_key(&self) -> &'static str {
        "はつとらいふ"
    }

    fn help_message(&self) -> &'static str {
        r"・判定：nBADm[±a][Cb±c][Fd±e][@X±f][!OP]　　[]内のコマンドは省略可。
・BADコマンドは「BL」コマンドで代用可。
・博徒は「GL」コマンドで〈波乱万丈〉の効果を適用。

「n」で振るダイス数、「m」で特性値、「±a」で達成値への修正値、
「Cb±c」でクリティカル値への修正、「Fd±e」でファンブル値への修正、
「@X」で目標難易度を指定。
「±a」「Cb±c」「Fd±e」[@X±f]部分は「4+1-3」などの複数回指定可。
「!OP」部分で、一部のスキルやガジェットの追加効果を指定可。
使用可能なコマンドは以下の通り。順不同、複数同時使用も可。
A：〈先見の明〉　　H：［重撃］

【書式例】
BAD → 1ダイスで達成値を表示。
3BAD10+2-1 → 3ダイスで修正+11の達成値を表示。
BL8@15 → 1ダイスで修正+8、難易度15の判定。
2BL8C-1F1@15 → 2ダイスで修正+8、C値-1、F値+1、難易度15の判定。
GL6@20 → 1ダイスで修正+6、難易度20の判定。〈波乱万丈〉の効果。
GL6@20!HA → 上記に加えて〈先見の明〉［重撃］の効果。

・コードネーム表
怪盗：TRN　　　闇医者：DRN　　博徒：GRN
殺シ屋：KRN　　業師：SRN　　　遊ビ人：BRN

・スキル表：SKL
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d?(BAD|BL|GL)", "[TDGKSB]RN", "SKL"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `BadLife#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = judge_dice(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/BadLife.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/BadLife.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/BadLife.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("BadLife.toml must parse");
        assert_eq!(data.tests.len(), 78, "case count in test/data/BadLife.toml");

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "BadLife",
                "unexpected game system in BadLife.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("BadLife"), &tc.input, &mut src) {
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
                    "FAIL BadLife:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} BadLife cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 桁あふれする修正値でも panic しないこと。
    ///
    /// Ruby の Integer は多倍長なのでそのまま計算されるが、Rustでは
    /// `Arithmetic.eval` が `i64::MAX` に飽和し、以降の加算も飽和演算になる。
    /// TOMLにこの経路のケースが無いのでここで固定する
    /// （デバッグビルドはオーバーフローで panic するため）。
    #[test]
    fn huge_modifier_saturates_instead_of_panicking() {
        let mut src = SeededRandomizer::new(vec![(10, 20)]);
        let result = eval_command(
            &GameSystemId::new("BadLife"),
            "BAD99999999999999999999",
            &mut src,
        )
        .expect("eval")
        .expect("result");
        assert_eq!(
            result.text,
            "1D20(C:20,F:1) ＞ 10[10]+9223372036854775807 ＞ 達成値：9223372036854775807"
        );
        assert!(src.is_empty(), "unconsumed rands");
    }

    /// ダイス0個（`0BAD`）は nil になること。
    ///
    /// Ruby は `dice_list.max` が `nil` になり `nil >= critical` でクラッシュする。
    /// 本移植は「解釈できないコマンド＝nil」に畳む。
    #[test]
    fn zero_dice_returns_nil() {
        let mut src = SeededRandomizer::new(vec![]);
        let result = eval_command(&GameSystemId::new("BadLife"), "0BAD", &mut src).expect("eval");
        assert_eq!(result, None);
        assert!(src.is_empty(), "unconsumed rands");
    }
}
