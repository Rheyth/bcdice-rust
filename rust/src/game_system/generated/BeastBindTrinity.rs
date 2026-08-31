//! P4で手書き移植した `lib/bcdice/game_system/BeastBindTrinity.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `BeastBindTrinity::BBCommand`（判定 `nBB+m%w@x#y$z&v`）
//! - `TABLES`（邂逅表 `EMO`、暴露表 `EXPO_*`、正体判明チャート `FACE_*`）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{D66GridTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format::modifier;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::BeastBindTrinity`（ID: `BeastBindTrinity`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeastBindTrinity;

impl GameSystem for BeastBindTrinity {
    fn id(&self) -> &'static str {
        "BeastBindTrinity"
    }

    fn name(&self) -> &'static str {
        "ビーストバインド トリニティ"
    }

    fn sort_key(&self) -> &'static str {
        "ひいすとはいんととりにてい"
    }

    fn help_message(&self) -> &'static str {
        r"・判定　(nBB+m%w@x#y$z&v)
　n個のD6を振り、出目の大きい2個から達成値を算出。修正mも可能。

　%w、@x、#y、$z、&vはすべて省略可能。
＞%w：現在の人間性が w であるとして、クリティカル値(C値)を計算。
・省略した場合、C値=12として達成値を算出する。
＞@x：クリティカル値修正。（加減式でも入力可能）
・xに直接数字を書くと、C値をその数字に上書きする。
　「絶対にクリティカルしない」状態は、@13など xを13以上に指定すること。
・xの先頭が「+」か「-」なら、計算したC値にその値を加算。例）@-1、@+2
　この方法でC値をプラスする場合、上限は12となる。
＞#y、#Ay：ファンブル値修正。（加減式でも入力可能）
・yに直接数字を書くと、ファンブル値をその数字に設定。
・yの数字の先頭が「+」か「-」なら、ファンブル値=2にその数字を加算。例）#+2
・※#Ayとすると、ファンブルしても達成値を通常通り算出。　例）#A+1
＞$z：ダイスの出目をzに固定して判定する。複数指定可。
　　　《運命歪曲》など「ダイスの１個を振り直す」効果等に使用する。
　例）2BB$1 →ダイスを2個振る判定で、ダイス1個の出目を1で固定
　例）2BB$16→ダイスを2個振る判定で、ダイスの出目を1と6で固定
＞&v：出目がv未満のダイスがあれば、出目がvだったものとして達成値を計算する。
　例）2BB&3 →出目3未満（→出目1、2）を出目3だったものとして計算。

・D66ダイスあり
・邂逅表：EMO
・暴露表：EXPO_A
・魔獣化暴露表：EXPO_B
・アイドル専用暴露表：EXPO_I
・アイドル専用魔獣化暴露表：EXPO_J
・正体判明チャートA～C：FACE_A, FACE_B, FACE_C
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+BB", r"\d+R6", "EMO", "EXPO_A", "EXPO_B", "EXPO_I", "EXPO_J", "FACE_A", "FACE_B",
            "FACE_C",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> crate::enums::D66SortType {
        crate::enums::D66SortType::Asc
    }

    /// Ruby `BeastBindTrinity#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        // Ruby: bb = BBCommand.new(command); bb.roll(@randomizer)
        //       （パースに失敗していれば roll は nil）
        let Some(bb) = BBCommand::parse(command) else {
            return Ok(None);
        };
        bb.roll(rng)
    }
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

// ---------------------------------------------------------------------------
// 判定コマンド
// ---------------------------------------------------------------------------

/// Ruby `String#to_i`（符号付きの数字列）。桁あふれは符号側へ飽和させる。
fn to_i(source: &str) -> i64 {
    source.parse().unwrap_or(if source.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（不正な式は 0）。
///
/// Ruby: `Arithmetic.eval(expr, round_type) || 0`。
/// Rust側の [`arithmetic::eval`] はゼロ除算を `Ok(None)` に畳み、それ以外の
/// エラーは伝播させるが、ここに渡る式は `[+\-\d]+` の範囲なので起きない。
fn arithmetic_evaluator_eval(expr: &str) -> i64 {
    crate::randomizer::sat_i64(
        &arithmetic::eval(expr, RoundType::Floor)
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
}

/// Ruby `BeastBindTrinity::BBCommand`（パース済みの判定コマンド）。
struct BBCommand {
    dice_num: i64,
    modify_number: i64,
    critical: i64,
    keep_value_on_fumble: bool,
    fumble: i64,
    dice_pool: Vec<i64>,
    dice_value_lower_limit: i64,
    cmp_op: Option<CmpOp>,
    target_number: Option<i64>,
}

impl BBCommand {
    /// Ruby `BBCommand#parse`。パースに失敗したら `None`（`@parse_error`）。
    fn parse(command: &str) -> Option<Self> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(
                r"^(\d+)(?:R6|BB6?)((?:[+-]\d+)+)?(?:%(-?\d+))?(?:@([+\-\d]+))?(?:#(A)?([+\-\d]+))?(?:\$([1-6]+))?(?:&([1-6]))?(?:([>=]+)(\d+))?$",
            )
            .expect("valid regex")
        });
        let m = re.captures(command)?;

        let dice_num = to_i(&m[1]);
        let modify_number = m
            .get(2)
            .map_or(0, |g| arithmetic_evaluator_eval(g.as_str()));

        let critical = parse_critical(m.get(3).map(|g| g.as_str()), m.get(4).map(|g| g.as_str()));

        let keep_value_on_fumble = m.get(5).is_some();

        let fumble = parse_fumble(m.get(6).map(|g| g.as_str()));

        // Ruby: @dice_pool = m[7] ? m[7].split("").map(&:to_i) : []
        let mut dice_pool: Vec<i64> = m.get(7).map_or_else(Vec::new, |g| {
            g.as_str()
                .chars()
                .map(|c| c.to_digit(10).map_or(0, i64::from))
                .collect()
        });
        // Ruby: @dice_pool.pop(@dice_pool.size - @dice_num) if @dice_pool.size > @dice_num
        if let Ok(len) = i64::try_from(dice_pool.len()) {
            if len > dice_num {
                dice_pool.truncate(usize::try_from(dice_num).unwrap_or(0));
            }
        }

        let dice_value_lower_limit = m.get(8).map_or(0, |g| to_i(g.as_str()));

        let cmp_op = m
            .get(9)
            .and_then(|g| normalize::comparison_operator(g.as_str()));
        let target_number = m.get(10).map(|g| to_i(g.as_str()));

        Some(Self {
            dice_num,
            modify_number,
            critical,
            keep_value_on_fumble,
            fumble,
            dice_pool,
            dice_value_lower_limit,
            cmp_op,
            target_number,
        })
    }

    /// Ruby `BBCommand#roll`。
    fn roll(&self, rng: &mut Randomizer) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let dice_list_org = self.roll_with_dice_pool(rng)?;
        if dice_list_org.is_empty() {
            return Ok(Some(SpecificCommandOutput::text(
                "ERROR:振るダイスの数が0個です",
            )));
        }

        let mut dice_list_filtered: Vec<i64> = dice_list_org
            .iter()
            .map(|&dice| dice.max(self.dice_value_lower_limit))
            .collect();
        dice_list_filtered.sort_unstable();
        // Ruby: @dice_total = dice_list_filtered.last(2).inject(0, :+)
        let dice_total: i64 = dice_list_filtered.iter().rev().take(2).sum();

        let fumble = dice_total <= self.fumble;
        let critical = dice_total >= self.critical;

        let total = self.calc_total(dice_total, fumble, critical);

        let dice_list_org_str = (dice_list_filtered != dice_list_org)
            .then(|| format!("[{}]", join_dice(&dice_list_org)));

        let mut result = self.result_compare(total);
        result.critical = critical;
        result.fumble = fumble;

        let dice_status = if result.fumble {
            Some("ファンブル")
        } else if result.critical {
            Some("クリティカル")
        } else {
            None
        };
        let result_str = if result.success {
            Some("成功")
        } else if result.failure {
            Some("失敗")
        } else {
            None
        };

        let mut sequence = vec![self.command_expr()];
        sequence.extend(dice_list_org_str);
        sequence.push(self.interim_expr(&dice_list_filtered, dice_total, critical));
        sequence.extend(dice_status.map(str::to_owned));
        sequence.push(total.to_string());
        sequence.extend(result_str.map(str::to_owned));
        result.text = sequence.join(" ＞ ");

        Ok(Some(SpecificCommandOutput::result(result)))
    }

    /// Ruby `BBCommand#roll_with_dice_pool`。固定出目（`$z`）の分は振らない。
    fn roll_with_dice_pool(&self, rng: &mut Randomizer) -> Result<Vec<i64>, EvalError> {
        let dice_times = self.dice_num - self.dice_pool.len() as i64;
        let mut dice_list = rng.roll_barabara(dice_times, 6)?;
        dice_list.extend_from_slice(&self.dice_pool);
        dice_list.sort_unstable();
        Ok(dice_list)
    }

    /// Ruby `BBCommand#command_expr`。
    fn command_expr(&self) -> String {
        let cmp_op = self.cmp_op.map(CmpOp::symbol_str).unwrap_or_default();
        let target_number = self
            .target_number
            .map(|n| n.to_string())
            .unwrap_or_default();
        format!(
            "({}BB{}@{}#{}{cmp_op}{target_number})",
            self.dice_num,
            modifier(&crate::Int::from(self.modify_number)),
            self.critical,
            self.fumble
        )
    }

    /// Ruby `BBCommand#interim_expr`。
    fn interim_expr(&self, dice_list: &[i64], dice_total: i64, critical: bool) -> String {
        let mut expr = format!(
            "{dice_total}[{}]{}",
            join_dice(dice_list),
            modifier(&crate::Int::from(self.modify_number))
        );
        if critical {
            expr.push_str("+20");
        }
        expr
    }

    /// Ruby `BBCommand#calc_total`。
    fn calc_total(&self, dice_total: i64, fumble: bool, critical: bool) -> i64 {
        let mut total = dice_total + self.modify_number;
        if fumble {
            if !self.keep_value_on_fumble {
                total = 0;
            }
        } else if critical {
            total += 20;
        }

        if total < 0 {
            total = 0;
        }

        total
    }

    /// Ruby `BBCommand#result_compare`。
    fn result_compare(&self, total: i64) -> EvalResult {
        match (self.cmp_op, self.target_number) {
            // 目標値は比較演算子と同じグループでしか入らないので、対で取り出す
            (Some(cmp_op), Some(target_number)) => {
                if cmp_op.apply(&crate::Int::from(total), &crate::Int::from(target_number)) {
                    EvalResult::success("")
                } else {
                    EvalResult::failure("")
                }
            }
            _ => EvalResult::new(),
        }
    }
}

/// Ruby `BBCommand#parse_critical`。
fn parse_critical(humanity: Option<&str>, atmark: Option<&str>) -> i64 {
    let humanity = humanity.map_or(99, to_i);
    let atmark_value = atmark.map_or(0, arithmetic_evaluator_eval);

    match atmark {
        Some(atmark) if atmark.starts_with(['+', '-']) => {
            (critical_from_humanity(humanity) + atmark_value).min(12)
        }
        Some(_) => atmark_value,
        None => critical_from_humanity(humanity),
    }
}

/// Ruby `BBCommand#critical_from_humanity`。
fn critical_from_humanity(humanity: i64) -> i64 {
    if humanity <= 0 {
        9
    } else if humanity <= 20 {
        10
    } else if humanity <= 40 {
        11
    } else {
        12
    }
}

/// Ruby `BBCommand#parse_fumble`。
fn parse_fumble(sharp: Option<&str>) -> i64 {
    let sharp_value = sharp.map_or(0, arithmetic_evaluator_eval);

    match sharp {
        Some(sharp) if sharp.starts_with(['+', '-']) => 2 + sharp_value,
        Some(_) => sharp_value,
        None => 2,
    }
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `TABLES['EMO']`（邂逅表）。
static TABLE_EMO: D66GridTable = D66GridTable::new(
    "邂逅表",
    &[
        &["家族", "家族", "信頼", "信頼", "忘却", "忘却"],
        &["慈愛", "慈愛", "憧憬", "憧憬", "感銘", "感銘"],
        &["同志", "同志", "幼子", "幼子", "興味", "興味"],
        &["ビジネス", "ビジネス", "師事", "師事", "好敵手", "好敵手"],
        &["友情", "友情", "忠誠", "忠誠", "恐怖", "恐怖"],
        &["執着", "執着", "軽蔑", "軽蔑", "憎悪", "憎悪"],
    ],
);

/// Ruby `TABLES['EXPO_A']`（暴露表）。
static TABLE_EXPO_A: Table = Table::from_dice(
    "暴露表",
    1,
    6,
    &[
        "噂になるがすぐ忘れられる",
        "都市伝説として処理される",
        "ワイドショーをにぎわす",
        "シナリオ中［迫害状態］になる",
        "絆の対象ひとりに正体が知られる",
        "魔獣化暴露表へ",
    ],
);

/// Ruby `TABLES['EXPO_B']`（魔獣化暴露表）。
static TABLE_EXPO_B: Table = Table::from_dice(
    "魔獣化暴露表",
    1,
    6,
    &[
        "トンデモ業界の伝説になる",
        "シナリオ中［迫害状態］になる",
        "シナリオ中［迫害状態］になる",
        "絆の対象ひとりに正体が知られる",
        "絆の対象ひとりに正体が知られる",
        "自衛隊退魔部隊×2D6体の襲撃",
    ],
);

/// Ruby `TABLES['EXPO_I']`（アイドル専用暴露表）。
static TABLE_EXPO_I: Table = Table::from_dice(
    "アイドル専用暴露表",
    1,
    6,
    &[
        "愉快な伝説として人気になる",
        "ワイドショーをにぎわす",
        "炎上。シナリオ中［迫害状態］",
        "所属事務所に2D6時間説教される",
        "絆の対象ひとりに正体が知られる",
        "アイドル専用魔獣化暴露表へ",
    ],
);

/// Ruby `TABLES['EXPO_J']`（アイドル専用魔獣化暴露表）。
static TABLE_EXPO_J: Table = Table::from_dice(
    "アイドル専用魔獣化暴露表",
    1,
    6,
    &[
        "シナリオ中［迫害状態］になる",
        "シナリオ中［迫害状態］になる",
        "絆の対象ひとりに正体が知られる",
        "事務所から契約を解除される",
        "絆の対象ひとりに正体が知られる",
        "1D6本のレギュラー番組を失う",
    ],
);

/// Ruby `TABLES['FACE_A']`（正体判明チャートA）。
static TABLE_FACE_A: Table = Table::from_dice(
    "正体判明チャートA",
    1,
    6,
    &[
        "あなたを受け入れてくれる",
        "あなたを受け入れてくれる",
        "絆が（拒絶）に書き換わる",
        "絆がエゴに書き換わる",
        "気絶しその事実を忘れる",
        "精神崩壊する",
    ],
);

/// Ruby `TABLES['FACE_B']`（正体判明チャートB）。
static TABLE_FACE_B: Table = Table::from_dice(
    "正体判明チャートB",
    1,
    6,
    &[
        "あなたを受け入れてくれる",
        "狂乱し攻撃してくる",
        "退場。その場から逃亡。暴露表へ",
        "絆がエゴに書き換わる",
        "精神崩壊する",
        "精神崩壊する",
    ],
);

/// Ruby `TABLES['FACE_C']`（正体判明チャートC）。
static TABLE_FACE_C: Table = Table::from_dice(
    "正体判明チャートC",
    1,
    6,
    &[
        "あなたを受け入れてくれる",
        "退場。その場から逃亡。暴露表へ",
        "退場。その場から逃亡。暴露表へ",
        "絆がエゴに書き換わる",
        "精神崩壊する",
        "精神崩壊する",
    ],
);

/// Ruby `TABLES`。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("EMO", &TABLE_EMO),
    ("EXPO_A", &TABLE_EXPO_A),
    ("EXPO_B", &TABLE_EXPO_B),
    ("EXPO_I", &TABLE_EXPO_I),
    ("EXPO_J", &TABLE_EXPO_J),
    ("FACE_A", &TABLE_FACE_A),
    ("FACE_B", &TABLE_FACE_B),
    ("FACE_C", &TABLE_FACE_C),
];

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
            .join("test/data/BeastBindTrinity.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/BeastBindTrinity.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/BeastBindTrinity.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("BeastBindTrinity.toml must parse");
        assert_eq!(
            data.tests.len(),
            46,
            "case count in test/data/BeastBindTrinity.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "BeastBindTrinity",
                "unexpected game system in BeastBindTrinity.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("BeastBindTrinity"), &tc.input, &mut src) {
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
                    "FAIL BeastBindTrinity:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} BeastBindTrinity cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
