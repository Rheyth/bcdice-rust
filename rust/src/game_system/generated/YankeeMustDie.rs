//! P4で手書き移植した `lib/bcdice/game_system/YankeeMustDie.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `YankeeMustDie#check_action`（判定 `YD+a>=b` / `YD+a>b`。ゾロ目で振り足し）
//! - `YankeeMustDie#eval_game_system_specific_command` → `check_action || roll_tables`
//! - `TABLES`（関係表 `RT` / 場面表 `ST` / ハプニング表 `HT` / 闇堕ち表 `DT`）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

static RT_ITEMS: &[&str] = &[
    "マブダチ：相手とは、マブダチ（親友） だ。いつからマブダチなのかはプレイヤー同士で相談して決めること。",
    "先輩／後輩：相手とは、先輩と後輩の間柄だ。なんの先輩後輩かはプレイヤー同士で相談して決めること。",
    "兄弟：相手とは、血縁であったり契りを交わした兄弟だ。兄弟になった経緯はプレイヤー同士で相談して決めること。",
    "ライバル：相手とは、良きライバル関係にある。どのようなライバル関係かはプレイヤー同士で相談して決めること。",
    "仲間：相手とは、同じチームなどに所属している仲間だ。どんなチームかはプレイヤー同士で相談して決めること。",
    "ジモティー：相手とは、同じ地元の仲間、幼馴染だ。いつから幼馴染なのかはプレイヤー同士で相談して決めること。",
    "おな中：相手とは、出身中学（小学校・高校も可） が同じだ。どんな中学だったのかはプレイヤー同士で決めること。",
    "相棒：相手は、唯一無二の相棒だ。いつから相棒なのかはプレイヤー同士で相談して決めること。",
    "ゾッコン：相手は、唯一無二の相棒だ。いつから相棒なのかはプレイヤー同士で相談して決めること。",
    "犬猿：相手とは、犬猿の仲である。犬猿の仲であるが、なぜ共に行動するのかはプレイヤー同士で相談して決めること。",
];
static RT: Table = Table::from_dice("関係表", 1, 10, RT_ITEMS);

static ST_ITEMS: &[&str] = &[
    "サ店（喫茶店）",
    "クラブ",
    "工業団地",
    "神社／教会",
    "学校",
    "埠頭",
    "繁華街",
    "ゲーセン",
    "公園",
    "河原",
    // 特殊な場合のみ発生する
    // "病院"
];
static ST: Table = Table::from_dice("場面表", 1, 10, ST_ITEMS);

static HT_ITEMS: &[&str] = &[
    "単車ドロ：愛車を盗まれる。次の自身の手番を迎えるまで、愛車が１台使用不能になる。所有している愛車が複数ある場合はランダムに１台を選ぶ。",
    "職質：サツにドウグを取り上げられる。次の自身の手番を迎えるまで、素手を除くドウグが１つ使用不能になる。所有しているドウグが複数ある場合はランダムに１つを選ぶ。",
    "不調：どうにも愛車やドウグが体になじまない次の判定の成功段階がー１される。",
    "乱闘：不良との喧嘩に巻き込まれた。PC は1d10 点のダメージを受ける。",
    "大人：悪辣な大人に遭遇して怒りが募る。PC は不良度が1d10 点上昇する",
    "仲違い：つまらないことで喧嘩になって友情に亀裂が入る。場面に登場している【関係】を結んでいるキャラクターの中からランダムに対象を1 人選ぶ。シナリオが終了するまで対象との【関係】が失われる。",
    "悪名：ボスの悪名が広がることによって自然とボスの取り巻きが増える。次の戦闘フェイズにモブが敵として1 人参加する。モブの種類はGM が決定する。",
    "凶暴化：ボスの思考が先鋭化して凶悪になる。シナリオが終了するまでボスが与えるダメージを+2 する。この効果は累積するが、上昇した能力値は戦力には影響しない。",
    "警戒：ボスは自身の周りでうごめく不穏な気配に警戒を強める。ボスの【HP 最大値】と【HP 現在値】を+10 する。この効果は累積する。",
    "不運：ツキがなくなってきた気がする...。ラッキーナンバーの数値が２下がる（最低１）。すでにラッキーナンバーを使用済みであれば効果を受けない。",
];
static HT: Table = Table::from_dice("ハプニング表", 1, 10, HT_ITEMS);

static DT_ITEMS: &[&str] = &[
    "出奔：すべての人間関係を捨ててどこか遠くへ旅に出る。",
    "半グレ：半グレ集団とつるむようになり、悪事に手を染めるようになる。",
    "指名手配：重大な犯罪を起こして指名手配されて逃亡者となる。",
    "事故：大事故に遭い意識不明の重体となり長期入院する。",
    "ヤク中：薬物中毒者となり、薬を得るためなら何でもするようになる。",
    "借金：イカれた人間を信奉するようになり多額の借金を背負わされる。",
    "傀儡：悪意を持って人間を利用しようとする勢力に祭り上げられ傀儡と化す。",
    "身代わり：犯罪を犯した人間の身代わりにされて追われる身となる。",
    "逮捕：度を越えた暴力沙汰を度々起こして警察に逮捕される。",
    "失踪：ヤバい事件に首を突っ込んで謎の失踪を遂げる。",
];
static DT: Table = Table::from_dice("闇堕ち表", 1, 10, DT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[("RT", &RT), ("ST", &ST), ("HT", &HT), ("DT", &DT)];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    match TABLES.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `YankeeMustDie#check_action`。
fn check_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: round_type: @round_type（既定の FLOOR）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["YD"], RoundType::Floor).restrict_cmp_op_to(&[
            Some(CmpOp::Ge),
            Some(CmpOp::Gt),
            None,
        ])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: loop { 3d10 を振ってソート; ゾロ目（uniq.one?）なら振り足す }
    let mut dice_all: Vec<Vec<i64>> = Vec::new();
    loop {
        let mut dice_list = rng.roll_barabara(3, 10)?;
        dice_list.sort_unstable();
        let is_all_same = dice_list.iter().all(|&d| d == dice_list[0]);
        dice_all.push(dice_list);
        if !is_all_same {
            break;
        }
    }

    // Ruby: dice_all.flatten.sum(parsed.modify_number)
    let sum: i64 = dice_all.iter().flatten().sum();
    let achievement_value: crate::Int = crate::Int::from(sum) + &parsed.modify_number.clone();
    let success_level = if achievement_value <= 9.into() {
        0
    } else if achievement_value <= 19.into() {
        1
    } else if achievement_value <= 29.into() {
        2
    } else if achievement_value <= 39.into() {
        3
    } else if achievement_value <= 49.into() {
        4
    } else if achievement_value <= 59.into() {
        5
    } else if achievement_value <= 69.into() {
        6
    } else if achievement_value <= 79.into() {
        7
    } else if achievement_value <= 89.into() {
        8
    } else if achievement_value <= 99.into() {
        9
    } else {
        10
    };

    let (is_success, is_failure, success_message) =
        match (parsed.cmp_op, parsed.target_number.clone()) {
            (Some(CmpOp::Gt), Some(target)) => {
                let is_success = crate::Int::from(success_level) > target;
                (is_success, !is_success, Some(is_success))
            }
            (Some(CmpOp::Ge), Some(target)) => {
                let is_success = crate::Int::from(success_level) >= target;
                (is_success, !is_success, Some(is_success))
            }
            _ => (false, false, None),
        };

    let dice_to_message_arr: Vec<String> = dice_all
        .iter()
        .map(|arr| {
            let sum: i64 = arr.iter().fold(0i64, |a, b| a.wrapping_add(*b));
            format!("{sum}[{}]", join_dice(arr))
        })
        .collect();

    let mut sequence = vec![
        parsed.to_s(SuffixPosition::AfterCommand),
        // Ruby: format("#{...} %+d", parsed.modify_number)
        format!(
            "{} {:+}",
            dice_to_message_arr.join(" + "),
            parsed.modify_number
        ),
        achievement_value.to_string(),
        format!("成功段階{success_level}"),
    ];

    if let Some(is_success) = success_message {
        sequence.push((if is_success { "成功" } else { "失敗" }).to_owned());
    }

    Ok(Some(EvalResult {
        text: sequence.join(" ＞ "),
        success: is_success,
        failure: is_failure,
        ..EvalResult::default()
    }))
}

/// Ruby `arr.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::YankeeMustDie`（ID: `YankeeMustDie`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YankeeMustDie;

impl GameSystem for YankeeMustDie {
    fn id(&self) -> &'static str {
        "YankeeMustDie"
    }

    fn name(&self) -> &'static str {
        "ヤンキーマストダイ"
    }

    fn sort_key(&self) -> &'static str {
        "やんきいますとたい"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定の方法
  基本形式：
  (YD+a>=b) または (YD+a>b)
  a：能力値、技能レベル、ドウグ等による修正値(複数指定可)
  b：目標となる成功段階

■ 成功の条件
  >=b：目標となる成功段階b以上の場合に成功となります。この条件では、目標となる成功段階と同じ数値でも成功とみなされます。
  >b：目標となる成功段階bより高い成功段階を出した場合に成功となります。この条件では、目標となる成功段階と同じ数値では失敗となります。

■ 各種表
　関係表 RT
　場面表 ST
　ハプニング表 HT
　闇堕ち表 DT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["YD", "RT", "ST", "HT", "DT"]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn sides_implicit_d(&self) -> i64 {
        10
    }

    /// Ruby `YankeeMustDie#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/YankeeMustDie.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/YankeeMustDie.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/YankeeMustDie.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("YankeeMustDie.toml must parse");
        assert_eq!(
            data.tests.len(),
            17,
            "case count in test/data/YankeeMustDie.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "YankeeMustDie",
                "unexpected game system in YankeeMustDie.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("YankeeMustDie"), &tc.input, &mut src) {
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
                    "FAIL YankeeMustDie:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} YankeeMustDie cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
