//! P4で手書き移植した `lib/bcdice/game_system/RyuTuber.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `RyuTuber#eval_game_system_specific_command`（`TABLES` を引く `roll_tables` と
//!   定型文 `TEXTS`）
//! - `TEXTS`（判定ルール・奇跡の演目）と `TABLES`（職業表・趣味表）のデータ
//!
//! データは `lib/bcdice/game_system/RyuTuber.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `BCDice::GameSystem::RyuTuber`（ID: `RyuTuber`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RyuTuber;

impl GameSystem for RyuTuber {
    fn id(&self) -> &'static str {
        "RyuTuber"
    }

    fn name(&self) -> &'static str {
        "リューチューバーとちいさな奇跡"
    }

    fn sort_key(&self) -> &'static str {
        "りゆうちゆうはあとちいさなきせき"
    }

    fn help_message(&self) -> &'static str {
        r"◆判定
　・判定　nB6<=1
　　※　n:サイコロの数　例）12B6<=1　サイコロの数12個の場合
　・判定ルールを表示する　RTB
◆職業　（カッコ内は使えそうな技能）
　・職業表　JT
　・学生表　JST
　・技術・専門職表　JTPT
　・事務・サービス職表　JOST
　・エンタメ職表　JET
◆趣味　（カッコ内は使えそうな技能）
　・趣味表　HT
　・多人数でできる趣味表　HGT
　・一人でできるインドア趣味表A　HIAT
　・一人でできるインドア趣味表B　HIBT
　・一人でできるアウトドア趣味表A　HOAT
　・一人でできるアウトドア趣味表B　HOBT
◆奇跡の演目を表示する
　・幸運の風が吹いている MPW
　・困った時はお互い様 MPT
　・悪い予感は的中する MPF
　・ついていい嘘もある MPL
　・私には星が見えている MPS
　・心は竜と共にあり MPD
　・人は石垣、人は城 MPH
"
    }

    /// Ruby `register_prefix(TEXTS.keys + TABLES.keys)`。
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTB", "MPW", "MPT", "MPF", "MPL", "MPS", "MPD", "MPH", "JT", "JST", "JTPT", "JOST",
            "JET", "HT", "HGT", "HIAT", "HIBT", "HOAT", "HOBT",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

/// Ruby `RyuTuber#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(ret) = table_helpers::roll_table(command, TABLES, rng)? {
        return Ok(Some(SpecificCommandOutput::text(ret)));
    }

    // Ruby: text.chomp（heredoc 末尾の改行を落とす）。
    // `TEXTS` のデータは既に chomp 済みの形で持っている。
    if let Some((_, text)) = TEXTS.iter().find(|(key, _)| *key == command) {
        return Ok(Some(SpecificCommandOutput::text(*text)));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// 表データ（lib/bcdice/game_system/RyuTuber.rb から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `TEXTS`。値は `chomp` 済み（Ruby は参照時に `chomp` する）。
static TEXTS: &[(&str, &str)] = &[
    ("RTB", "判定ルール表示\n①枠主が判定内容を宣言、判定参加者が行動宣言\n②サイコロは竜の巫女なら6個、技能レベルか指定魅力の値個、奇跡の演目を1つ以上クリアで+6個、スパの消費数個\n③振ったサイコロの「1の目」の数が目標値以上なら華麗に成功、目標値未満ならちょっと残念な結果"),
    ("MPW", "幸運の風が吹いている\n奇跡　以降ゲーム終了まで、サイコロ+1\n①健気に頑張る姿を見せる。\n②報われることはなく、さらに最悪の展開に。\n③それでも健気なところを見せる。"),
    ("MPT", "困った時はお互い様\n奇跡　そのプレイヤーの判定サイコロを1回振り直しできる\n①けちな様子を見せる。\n②困っている人に施しをする姿を見られる。\n③窮地に陥る。"),
    ("MPF", "悪い予感は的中する\n奇跡　1判定だけ、サイコロ+3\n①犠牲者が悪い噂を耳にする。\n②犠牲者が悪い冗談を言う。\n③犠牲者が悪い予感に心さざめき、誰かに悪い予感を話す。"),
    ("MPL", "ついていい嘘もある\n奇跡　ついた（ささやかな）嘘が本当になる　枠主判断でいつか発動する。\n①嘘を言う。\n②嘘によって窮地に立つ。\n③嘘を嘘にしないためにあがく。"),
    ("MPS", "私には星が見えている\n奇跡　指定したキャラクターの次の行動がわかる\n①少し先のことを言い当てる。\n②気味が悪いと噂になる。\n③言い当てる力を人間観察に用いる。"),
    ("MPD", "心は竜と共にあり\n奇跡　起こりうる不幸を阻止する\n①心清いひとに助けられる。\n②自分の性根悪さを悲しむ。\n③自分なりのやり方で心清い行いをする。"),
    ("MPH", "人は石垣、人は城\n奇跡　感化された周りの人が手伝うようになる\n①人々の不幸を見て、親切にしてしまう。\n②けなげに頑張る姿を見られる。\n③見ていた人々が集まってくる。"),
];

/// Ruby `TABLES["JT"]`（職業表）。
static JT_ITEMS: &[&str] = &[
    "学生表へ",
    "技術・専門職表へ",
    "技術・専門職表へ",
    "事務・サービス職表へ",
    "事務・サービス職表へ",
    "エンタメ職表へ",
];
static JT: Table = Table::from_dice("職業表", 1, 6, JT_ITEMS);

/// Ruby `TABLES["JST"]`（学生表）。
static JST_ITEMS: &[&str] = &[
    "中学生　（ゲーム　運動する）",
    "高校生（文系）　（仲良くする　文章を書く）",
    "高校生（理系）　（仲良くする　科学の知識）",
    "専門学校生　（ものづくり　設計する）",
    "大学生（文系）　（社会の仕組み　外国語）",
    "大学生（理系）　（すごい技術　科学の知識）",
];
static JST: Table = Table::from_dice("学生表", 1, 6, JST_ITEMS);

/// Ruby `TABLES["JTPT"]`（技術・専門職表）。
static JTPT_ITEMS: &[&str] = &[
    "勝負師・山師　（洞察力　精神力）",
    "漁師/猟師　（自然の知識　料理する）",
    "建築家、大工　（設計する　運転する）",
    "料理人　（料理する　ものづくり）",
    "職人　（ものづくり　丁寧）",
    "農家　（自然の知識　育てる）",
    "医療・福祉関係（医師、薬剤師、介護職）　（治す　科学の知識）",
    "美容、スタイリスト　（見た目を整える　仲良くする）",
    "プログラマー　（プログラム　設計する）",
    "士業（税理士、弁護士、行政書士等）　（社会の仕組み　事務仕事）",
    "研究者　（教える　すごい技術）",
];
static JTPT: Table = Table::from_dice("技術・専門職表", 2, 6, JTPT_ITEMS);

/// Ruby `TABLES["JOST"]`（事務・サービス職表）。
static JOST_ITEMS: &[&str] = &[
    "宗教関係（巫女、僧侶など）　（お祈りする　地元知識）",
    "観光、旅行　（外国語　地元知識）",
    "教師、保育士　（教える　育てる）",
    "運転手、配達員　（運転する　地元知識）",
    "自宅警備員　（ゲーム　想像力）",
    "サラリーマン　（事務仕事　仲良くする）",
    "店員　（丁寧　商品知識）",
    "公務員　（事務仕事　地元知識）",
    "警察、自衛隊、消防士　（社会の仕組み　戦う）",
    "投資家、金融業、不動産　（プレゼンする　事務仕事）",
    "経営者　（社会の仕組み　仲良くする）",
];
static JOST: Table = Table::from_dice("事務・サービス職表", 2, 6, JOST_ITEMS);

/// Ruby `TABLES["JET"]`（エンタメ職表）。
static JET_ITEMS: &[&str] = &[
    "ゲーム制作　（プログラム　ものづくり）",
    "写真家　（自然の知識　絵を描く）",
    "デザイナー　（設計する 見た目を整える）",
    "ライター　（文章を書く　想像力）",
    "イラストレーター　（絵を描く　見た目を整える）",
    "専業配信者　（プレゼンする　カリスマ）",
    "声優　（声を出す　演技する）",
    "ミュージシャン　（声を出す　音楽）",
    "アイドル・芸能人　（演技する　カリスマ）",
    "プロゲーマー　（ゲーム　戦う）",
    "プロスポーツ選手　（運動する　精神力）",
];
static JET: Table = Table::from_dice("エンタメ職表", 2, 6, JET_ITEMS);

/// Ruby `TABLES["HT"]`（趣味表）。
static HT_ITEMS: &[&str] = &[
    "多人数でできる趣味表へ",
    "多人数でできる趣味表へ",
    "一人でできるインドア趣味表Aへ",
    "一人でできるインドア趣味表Bへ",
    "一人でできるアウトドア趣味表Aへ",
    "一人でできるアウトドア趣味表Bへ",
];
static HT: Table = Table::from_dice("趣味表", 1, 6, HT_ITEMS);

/// Ruby `TABLES["HGT"]`（多人数でできる趣味表）。
static HGT_ITEMS: &[&str] = &[
    "家族サービス　（仲良くする　育てる）",
    "野球・フットサル　（仲良くする　運動する）",
    "ボードゲーム／ＴＲＰＧ／囲碁／将棋　（ゲーム　想像する）",
    "ボランティア　（忍耐力　カリスマ）",
    "サバイバルゲーム　（戦う　隠れる）",
    "バンド　（音楽　見た目を整える）",
];
static HGT: Table = Table::from_dice("多人数でできる趣味表", 1, 6, HGT_ITEMS);

/// Ruby `TABLES["HIAT"]`（一人でできるインドア趣味表A）。
static HIAT_ITEMS: &[&str] = &[
    "工芸　（ものづくり　想像力）",
    "編み物　（丁寧　見た目を整える）",
    "陶芸　（ものづくり　想像力）",
    "プラモ　（ものづくり　見た目を整える）",
    "同人　（絵を描く　文章を書く）",
    "読書　（外国語　社会の仕組み）",
];
static HIAT: Table = Table::from_dice("一人でできるインドア趣味表A", 1, 6, HIAT_ITEMS);

/// Ruby `TABLES["HIBT"]`（一人でできるインドア趣味表B）。
static HIBT_ITEMS: &[&str] = &[
    "仕事　（事務仕事　忍耐力）",
    "資格集め　（社会の仕組み　商品知識）",
    "お絵かき　（絵を描く　想像力）",
    "料理　（料理する　設計する）",
    "筋トレ　（運動する　忍耐力）",
    "コンピューターゲーム　（ゲーム　プログラム）",
];
static HIBT: Table = Table::from_dice("一人でできるインドア趣味表B", 1, 6, HIBT_ITEMS);

/// Ruby `TABLES["HOAT"]`（一人でできるアウトドア趣味表A）。
static HOAT_ITEMS: &[&str] = &[
    "スポーツ観戦　（忍耐力　お祈りする）",
    "水泳　（運動する　泳ぐ）",
    "旅行／鉄道　（移動する　外国語）",
    "写真　（自然の知識　想像力）",
    "ジグソーパズル　（ゲーム　忍耐力）",
    "マラソン　（運動する　忍耐力）",
];
static HOAT: Table = Table::from_dice("一人でできるアウトドア趣味表A", 1, 6, HOAT_ITEMS);

/// Ruby `TABLES["HOBT"]`（一人でできるアウトドア趣味表B）。
static HOBT_ITEMS: &[&str] = &[
    "スキー・スノーボード　（運動する　自然の知識）",
    "自転車　（移動する　運動する）",
    "盆栽・生花　（丁寧　育てる）",
    "キャンプ　（自然の知識　精神力）",
    "映画鑑賞　（演技する　想像力）",
    "恋愛　（仲良くする　見た目を整える）",
];
static HOBT: Table = Table::from_dice("一人でできるアウトドア趣味表B", 1, 6, HOBT_ITEMS);

static TABLES: &[(&str, &Table)] = &[
    ("JT", &JT),
    ("JST", &JST),
    ("JTPT", &JTPT),
    ("JOST", &JOST),
    ("JET", &JET),
    ("HT", &HT),
    ("HGT", &HGT),
    ("HIAT", &HIAT),
    ("HIBT", &HIBT),
    ("HOAT", &HOAT),
    ("HOBT", &HOBT),
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
            .join("test/data/RyuTuber.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/RyuTuber.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/RyuTuber.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("RyuTuber.toml must parse");
        assert_eq!(
            data.tests.len(),
            19,
            "case count in test/data/RyuTuber.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "RyuTuber",
                "unexpected game system in RyuTuber.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("RyuTuber"), &tc.input, &mut src) {
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
                    "FAIL RyuTuber:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} RyuTuber cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
