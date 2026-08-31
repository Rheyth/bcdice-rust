//! P4で手書き移植した `lib/bcdice/game_system/Oukahoushin3rd.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Oukahoushin3rd#eval_game_system_specific_command` と `#replace_dice_notation`
//!   （表の本文に含まれる `nDm` をその場で振って `(=>値)` を添える）
//! - `TABLES`（`BKT` / `KKT` / `NHT` / `SDT` / `SKT` / `STT` / `UKT`）
//!
//! # 表データ
//!
//! 原典の項目は Ruby の double-quoted literal で、`KKT` には `\n`（バックスラッシュと
//! `n` の2文字。改行ではない）が含まれる。ここでは Ruby が評価したあとの値を
//! そのまま持つ（`"…なども）。\\n「次のラウンドに…"`）。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::Oukahoushin3rd`（ID: `Oukahoushin3rd`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Oukahoushin3rd;

impl GameSystem for Oukahoushin3rd {
    fn id(&self) -> &'static str {
        "Oukahoushin3rd"
    }

    fn name(&self) -> &'static str {
        "央華封神RPG 第三版"
    }

    fn sort_key(&self) -> &'static str {
        "おうかほうしんRPG3"
    }

    fn help_message(&self) -> &'static str {
        r"・各種表
　・能力値判定裏成功表（NHT）
　・武器攻撃裏成功表（BKT）
　・受け・回避裏成功表（UKT）
　・仙術行使裏成功表（SKT）
　・仙術抵抗裏成功表（STT）
　・精神値ダメージ悪影響表（SDT）
　・狂気表（KKT）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["BKT", "KKT", "NHT", "SDT", "SKT", "STT", "UKT"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Oukahoushin3rd#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: chosen = roll_tables(command, TABLES); replace_dice_notation(chosen)
        let Some(chosen) = roll_tables(command, rng)? else {
            // Ruby: nil&.gsub は nil
            return Ok(None);
        };

        Ok(Some(SpecificCommandOutput::text(replace_dice_notation(
            &chosen, rng,
        )?)))
    }
}

// BKT: 武器攻撃裏成功表 (2D6) 11 items
static BKT_ITEMS: &[&str] = &[
    "1ポイント清徳値が低下。連続攻撃が行える。この場合の連続攻撃においては、命中判定のサイコロは常にひっくり返して用いるが、2撃目以降はこの表は使わない。",
    "敵に叩きつけると同時に武器が破損。素手や身体に備わった武器（爪、牙など）で攻撃をしていた場合には、自身にも1D6（のみ）ダメージ。",
    "効果的命中。ダメージに1D6加算。ただし極度に疲労するため、精神値に1D6点ダメージを受ける。（2ゾロ）1ポイント仙骨が上昇、体力または機敏（攻撃を行った者が選択する）が1ポイント低下。",
    "ふつうの命中。",
    "不完全な命中、ダメージは半分。（3ゾロ）1ポイント仙骨が低下。",
    "ふつうの命中。",
    "体力または機敏（攻撃を行った者が選択する）が1D6日間、1ポイント上昇。（4ゾロ）能力値の上昇は永遠。",
    "ふつうの命中。",
    "体力または機敏（攻撃を行った者が選択する）が1D6日間、1ポイント低下。（5ゾロ）能力値の低下は永遠。",
    "呼吸を乱す、数瞬間（1D6ラウンド）は仙術を使用できない。",
    "1ポイント清徳値が低下。体力または機敏（攻撃を行った者が選択する）が1ポイント上昇。",
];
static BKT_TABLE: Table = Table::from_dice("武器攻撃裏成功表", 2, 6, BKT_ITEMS);

// KKT: 狂気表 (2D6) 11 items
static KKT_ITEMS: &[&str] = &[
    "心神喪失、生ける屍。",
    "被害妄想。仲間も含め、他者は全て自分を傷つけようとしていると思いこむ。行動はゲームマスターが管理。",
    "重度の不安症。失敗を恐れるあまり、次ラウンドは行動不可。それ以降も、2ラウンドに1回しか行動できない（自動武器や使役獣への命令なども）。\\n「次のラウンドに行動できない」状態では、「割り込み」は行えない。",
    "重度の依存症。自分で行動を決められず、仲間に決めてもらわなければならない。",
    "二重人格。二つ目の人格は狂気。新たに狂気表（KKT）で決定（再度二重人格が出た場合は、振りなおす）。狂気表を使った直後は、この二つ目の人格。\\n1日以上、二重人格が持続している場合、その間に精神値ダメージを受けるたびに、その直後に1Dを振らねばならない。1が出たらこの狂気が顔を出す。\\n二つ目の人格が顔を出している時間は、1Dで決定する（1～3：短時間、4～5：半日、6：1日）。",
    "軽度の依存症。仲間の承認がなければ、思いついた行動を実行できない。",
    "軽度の偏執狂。ある行為や物品などに異常な執着を示す。ただし、行動に大きな影響は与えない。具体的な内容は、ゲームマスターとプレイヤーの相談で決定。",
    "重度の偏執狂。行動に重大な影響を与える。具体的内容は、ゲームマスターが決定。",
    "恐怖症。あるものに対して恐怖。対象からは、ひたすら逃亡しようとする。また、対象に遭遇するたびに、難易度10で意志の能力値判定を行わねばならず、失敗したら1Dの精神値ダメージを受ける。恐怖の対象は、ゲームマスターが決定。",
    "狂暴化。仲間も含め、他者はすべて敵とみなし、傷つけようとする。行動はゲームマスターが管理。",
    "錯乱。行動はゲームマスターが「なるべくでたらめになるように」決定する。",
];
static KKT_TABLE: Table = Table::from_dice("狂気表", 2, 6, KKT_ITEMS);

// NHT: 能力値判定裏成功表 (2D6) 11 items
static NHT_ITEMS: &[&str] = &[
    "1ポイント清徳値が低下。変な癖が身についてしまう。",
    "やりすぎ。過剰な成功をしたり、よけいなことまでして災いが起こりうる。",
    "「気」の爆発。大成功。ただし極度に疲労するため、精神値に1D6点ダメージを受ける。（2ゾロ）1ポイント仙骨が上昇、使用した能力値が1ポイント低下。",
    "ふつうの成功。",
    "不完全な成功、数値的効果は半分ほどに見積もる。（3ゾロ）1ポイント仙骨が低下。",
    "ふつうの成功。",
    "使用した能力値が1D6日間、1ポイント上昇。（4ゾロ）能力値の上昇は永遠。",
    "ふつうの成功。",
    "使用した能力値が1D6日間、1ポイント低下。（5ゾロ）能力値の低下は永遠。",
    "呼吸を乱す、数瞬間（1D6ラウンド）は仙術を使用できない。",
    "1ポイント清徳値が低下。使用した能力値が1ポイント上昇。",
];
static NHT_TABLE: Table = Table::from_dice("能力値判定裏成功表", 2, 6, NHT_ITEMS);

// SDT: 精神値ダメージ悪影響表 (1D6) 6 items
static SDT_ITEMS: &[&str] = &[
    "一瞬の放心。直後の判定は自動的に失敗。精神値を1D6×最大値の10％回復。",
    "一瞬の放心。直後の判定は自動的に失敗。精神値を1D6×最大値の10％回復。",
    "一瞬の放心。直後の判定は自動的に失敗。精神値を1D6×最大値の10％回復。",
    "放心状態。強制され、自動失敗するまで、自発的行動不可。精神値を（1D6+2）×最大値の10％回復。",
    "精神異常（具体的内容は狂気表（KKT）で決定）。短時間のみ。精神値を（1D6+4）×最大値の10％回復。",
    "精神異常（具体的内容は狂気表（KKT）で決定）。期間は1D6を振って決定（1～3：1日、4～5：99日間、6：永遠）。精神値を最大値まで回復。",
];
static SDT_TABLE: Table = Table::from_dice("精神値ダメージ悪影響表", 1, 6, SDT_ITEMS);

// SKT: 仙術行使裏成功表 (2D6) 11 items
static SKT_ITEMS: &[&str] = &[
    "1ポイント清徳値が低下。1ポイント仙骨が上昇。",
    "術の効果は術者にも解除不能になる。精神値に1点ダメージを受ける。",
    "「気」の暴走。効果3倍。ただし極度に疲労するため、精神値に1D6点ダメージを受ける。（2ゾロ）術者は1D6日間、仙術が使用不能になる。1ポイント仙骨が上昇。",
    "術が敵にかけたものの場合、仲間の1人を巻きこむ。精神値に1点ダメージを受ける。",
    "不完全な成功、効果半分。（3ゾロ）持続時間のある術の場合、術者がひたすら精神集中していない限り、術はすぐに解除される。",
    "ふつうの成功。",
    "1ポイント清徳値が低下。（4ゾロ）仙骨以外のいずれかの能力値（術者選択）が1D6日間、1ポイント上昇。",
    "術が味方もしくは自分にかけたものの場合、敵の1人にも同じようにかかる。精神値に1点ダメージを受ける。",
    "仙骨以外のいずれかの能力値（術者選択）が1D6日間、1ポイント低下。（5ゾロ）能力値の低下は永遠。",
    "1D3ポイント清徳値が低下。",
    "1D6ポイント清徳値が低下。仙骨以外のいずれかの能力値（術者選択）が1ポイント上昇。",
];
static SKT_TABLE: Table = Table::from_dice("仙術行使裏成功表", 2, 6, SKT_ITEMS);

// STT: 仙術抵抗裏成功表 (2D6) 11 items
static STT_ITEMS: &[&str] = &[
    "1ポイント清徳値が低下。",
    "そらされた術の効果が味方に及ぶ。味方の誰にそらされたかは、この表を使ったものが選ぶ。集団攻撃仙術の場合、抵抗に成功したものの中から選ぶこと。ほかの誰も成功に抵抗していなかったときは、ふつうの抵抗成功として扱う。精神値に1点ダメージを受ける。",
    "仙術をかけた敵にその効果が及ぼされる。敵自身はそれに対して、抵抗を試みることができる。（2ゾロ）1ポイント仙骨が上昇。1ポイント知覚が低下。",
    "ふつうの抵抗成功。",
    "不完全な抵抗、ふつうの半分の効果を受ける。（3ゾロ）1ポイント仙骨が低下。",
    "ふつうの抵抗成功。",
    "仙骨または知覚（仙術に抵抗した者が選択する）が1D6日間、1ポイント上昇。（4ゾロ）能力値の上昇は永遠。",
    "ふつうの抵抗成功。",
    "仙骨または知覚（仙術に抵抗した者が選択する）が1D6日間、1ポイント低下。（5ゾロ）能力値の低下は永遠。",
    "呼吸を乱す、数瞬間（1D6ラウンド）は仙術を使用できない。",
    "1ポイント清徳値が低下。仙骨または知覚（仙術に抵抗した者が選択する）が1ポイント上昇。",
];
static STT_TABLE: Table = Table::from_dice("仙術抵抗裏成功表", 2, 6, STT_ITEMS);

// UKT: 受け・回避裏成功表 (2D6) 11 items
static UKT_ITEMS: &[&str] = &[
    "1ポイント清徳値が低下。",
    "転倒する（空を飛んでいるものは落下。乗騎などに乗っていたら転落）。精神値に1点ダメージを受ける。",
    "相手のバランスを崩すのに成功。攻撃を行った敵が転倒（空を飛んでいるものは落下。乗騎などに乗っていたら転落）。（2ゾロ）1ポイント仙骨が上昇、機敏または知覚（攻撃を防御した者が選択する）が1ポイント低下。",
    "ふつうの防御成功。",
    "不完全な防御、通常の半分のダメージを受ける。しかし敵が連続攻撃を行うことは出来ない。攻撃者が裏成功攻撃であってもその反動は決めない。（3ゾロ）1ポイント仙骨が低下。",
    "ふつうの防御成功。",
    "機敏または知覚（攻撃を防御した者が選択する）が1D6日間、1ポイント上昇。（4ゾロ）能力値の上昇は永遠。",
    "ふつうの防御成功。",
    "機敏または知覚（攻撃を防御した者が選択する）が1D6日間、1ポイント低下。（5ゾロ）能力値の低下は永遠。",
    "呼吸を乱す、数瞬間（1D6ラウンド）は仙術を使用できない。",
    "1ポイント清徳値が低下。機敏または知覚（攻撃を防御した者が選択する）が1ポイント上昇。",
];
static UKT_TABLE: Table = Table::from_dice("受け・回避裏成功表", 2, 6, UKT_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[
    ("BKT", &BKT_TABLE),
    ("KKT", &KKT_TABLE),
    ("NHT", &NHT_TABLE),
    ("SDT", &SDT_TABLE),
    ("SKT", &SKT_TABLE),
    ("STT", &STT_TABLE),
    ("UKT", &UKT_TABLE),
];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `/(\d+)D(\d+)/`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn dice_notation_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([0-9]+)D([0-9]+)").expect("valid regex"))
}

/// Ruby `Oukahoushin3rd#replace_dice_notation`。
///
/// 表の本文に含まれる `nDm` をその場で振り、`nDm(=>値)` に置き換える。
/// `Regex::replace_all` のクロージャは `Result` を返せず、置換の中で
/// ダイスを振る必要があるため、`Preprocessor#replace_parentheses` と同じ形の
/// 手書き走査にしている。Ruby の `gsub` は置換後の文字列を再走査しないので、
/// 左から右への1パスが正しい（再走査すると `1D6(=>1)` の `1D6` を無限に拾う）。
fn replace_dice_notation(text: &str, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;

    for caps in dice_notation_pattern().captures_iter(text) {
        let matched = caps.get(0).expect("group 0 always exists");
        out.push_str(&text[last..matched.start()]);

        // Ruby: times, sides = matched.split("D").map(&:to_i)
        let times = to_i_saturating(&caps[1]);
        let sides = to_i_saturating(&caps[2]);
        let value = rng.roll_sum(times, sides)?;

        out.push_str(matched.as_str());
        out.push_str(&format!("(=>{value})"));
        last = matched.end();
    }

    out.push_str(&text[last..]);
    Ok(out)
}

/// Ruby `String#to_i` 相当（桁あふれは飽和させる）。
fn to_i_saturating(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or(i64::MAX)
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
            .join("test/data/Oukahoushin3rd.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Oukahoushin3rd.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Oukahoushin3rd.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Oukahoushin3rd.toml must parse");
        assert_eq!(
            data.tests.len(),
            15,
            "case count in test/data/Oukahoushin3rd.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Oukahoushin3rd",
                "unexpected game system in Oukahoushin3rd.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Oukahoushin3rd"), &tc.input, &mut src) {
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
                    "FAIL Oukahoushin3rd:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Oukahoushin3rd cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
