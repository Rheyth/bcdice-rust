//! P4で手書き移植した `lib/bcdice/game_system/LiverLabyrinth.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `LiverLabyrinth#check_roll`（`xLL+y@c$d>=z`）
//! - `LiverLabyrinth#roll_table_command`（表の複数回・出目指定）
//! - `TABLES`

use std::sync::OnceLock;

use regex::Regex;

use crate::command_parser::Parser;
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

/// Ruby `BCDice::GameSystem::LiverLabyrinth`（ID: `LiverLabyrinth`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiverLabyrinth;

impl GameSystem for LiverLabyrinth {
    fn id(&self) -> &'static str {
        "LiverLabyrinth"
    }

    fn name(&self) -> &'static str {
        "ライバー＆ラビリンス"
    }

    fn sort_key(&self) -> &'static str {
        "らいはああんとらひりんす"
    }

    fn help_message(&self) -> &'static str {
        r#"同人TRPGシステム『ライバー＆ラビリンス』用ダイスボット。
・判定コマンド(xLL+y@c$d>=z)
  x：能力値
  +y：ダメージ判定時の攻撃力(省略可。省略時は0)
  c：クリティカル値(省略可。省略時は10)
  d：クリティカル時の加算値(省略可。省略時は1)
  z：難易度(4以下のとき5に。11以上は10になり、サイコロの数が減る）
  (例) 6LL@8>=6
       10LL>=5
       4LL+5@10$2>=10
・各種表 ：
    コマンド末尾に数字を入れると複数回の一括実行が可能　例）GETCT4
    コマンド末尾に"="(イコール)と数字を入れると、特定のダイス目の結果の実行が可能　例）CRITICALT=5
  ・クリティカル表(CriticalT)
  ・命中ファンブル表(FumbleT)
  ・致命傷表(FatalT)
  ・休憩表(RestT)
  ・痛恨表(TerribleT)
  ・お宝表(レベル1~4)(GetCT)
  ・お宝表(レベル5~8)(GetRT)
  ・お宝表(レベル9~14)(GetSRT)
  ・お宝表(レベル15~99)(GetURT)
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+LL",
            "CRITICALT",
            "FUMBLET",
            "FATALT",
            "RESTT",
            "TERRIBLET",
            "GETCT",
            "GETRT",
            "GETSRT",
            "GETURT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `LiverLabyrinth#initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `LiverLabyrinth#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_roll(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = roll_table_command(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(None)
    }
}

/// Ruby `LiverLabyrinth#check_roll`。
fn check_roll(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["LL"], RoundType::Ceil)
        .has_prefix_number()
        .enable_critical()
        .enable_dollar()
        .restrict_cmp_op_to(&[Some(CmpOp::Ge)]);
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let mut dice_cnt = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let modify = parsed.modify_number;
    let critical_target = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(10);
    let critical_addition = parsed
        .dollar
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(1);
    let Some(mut target) = parsed.target_number else {
        return Ok(None);
    };

    let mut text = String::new();
    if target < I::from(5) {
        text.push_str(&format!(
            "【{command}】 ＞ あらゆる難易度は5未満にはならないため、難易度は5になる！\n"
        ));
        target = I::from(5);
    } else if target >= I::from(11) {
        text.push_str(&format!(
            "【{command}】 ＞ 難易度が11を超えたため、超過分、ダイスの数が減少！\n"
        ));
        let over = target - I::from(10);
        target = I::from(10);
        dice_cnt -= crate::randomizer::sat_i64(&over);
    }

    if dice_cnt < 0 {
        dice_cnt = 0;
    }

    let crit_add = if critical_addition > 1 {
        format!("(+{critical_addition})")
    } else {
        String::new()
    };
    let atk = if modify > I::ZERO {
        format!("、攻撃力{modify}")
    } else {
        String::new()
    };
    text.push_str(&format!(
        "【ダイスの数{dice_cnt}、難易度{target}、クリティカル{critical_target}{crit_add}{atk}】"
    ));

    let dice_arr = rng.roll_barabara(dice_cnt, 10)?;
    let mut counts = [0i64; 11];
    for v in &dice_arr {
        if (1..=10).contains(v) {
            counts[*v as usize] += 1;
        }
    }

    let mut success_cnt = 0i64;
    let mut critical_cnt = 0i64;
    for (idx, &count) in counts.iter().enumerate().skip(1) {
        if count == 0 {
            continue;
        }
        let face = idx as i64;
        if face >= crate::randomizer::sat_i64(&target) {
            success_cnt += count;
        }
        if face >= critical_target {
            success_cnt += count * critical_addition;
            critical_cnt += count;
        }
    }

    let mut dice_count_strs = Vec::new();
    for (idx, &count) in counts.iter().enumerate().skip(1) {
        if count == 0 {
            continue;
        }
        dice_count_strs.push(format!("[{idx}]×{count}"));
    }

    let mut has_critical = critical_cnt >= 3;
    let half = (dice_cnt as f64 / 2.0).ceil() as i64;
    let has_fumble = dice_cnt > 0 && counts[1] >= half;
    if has_fumble {
        has_critical = false;
        success_cnt = 0;
    }
    let result = success_cnt > 0;

    text.push_str(&format!(
        " ＞ {} ＞ 成功度{success_cnt} ＞ {}{}{}",
        dice_count_strs.join(","),
        if result { "成功" } else { "失敗" },
        if has_critical {
            "(クリティカル)"
        } else {
            ""
        },
        if has_fumble { "(ファンブル)" } else { "" },
    ));

    if result && modify > I::ZERO {
        text.push_str(&format!(" ＞ {}ダメージ", success_cnt + modify));
    }

    let mut r = EvalResult::with_text(text);
    r.critical = has_critical;
    r.fumble = has_fumble;
    r.success = result;
    r.failure = !result;
    Ok(Some(r))
}

fn table_command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([A-Z]+)(\d+)?(=)?(\d+)?").expect("valid regex"))
}

/// Ruby `LiverLabyrinth#roll_table_command`。
fn roll_table_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let command = command.to_ascii_uppercase();
    let Some(m) = table_command_pattern().captures(&command) else {
        return Ok(None);
    };
    let table_name = &m[1];
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == table_name) else {
        return Ok(None);
    };

    let counts = m.get(2).map_or(1, |v| ruby_to_i_digits(v.as_str()));
    let operator = m.get(3).map(|v| v.as_str());
    let value = m.get(4).map_or(0, |v| ruby_to_i_digits(v.as_str()));

    // Ruby: return nil if !operator.nil? && value <= 0 || value >= 11
    if (operator.is_some() && value <= 0) || value >= 11 {
        return Ok(None);
    }

    let mut result_texts = Vec::new();
    for _ in 0..counts {
        let text = if operator == Some("=") {
            table.choice(value).to_string()
        } else {
            table.roll(rng)?.to_string()
        };
        result_texts.push(text);
    }

    Ok(Some(EvalResult::with_text(result_texts.join("\n"))))
}

fn ruby_to_i_digits(text: &str) -> i64 {
    text.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `TABLES["CRITICALT"]`。
static CRITICALT_ITEMS: &[&str] = &[
        "視聴者が沸き立つ一撃！閲覧数を1D10点増加させる。",
        "致命的な一撃！最終的に与えるダメージが2倍になる。",
        "肉体を変容させる一撃！ランダムで対象にバステを付与する。",
        "魔力の消費を最小限に抑えることに成功！最終的にこのアクションで消費する《EP》が0になる。",
        "取れ高発生！《トレダカ》を1点増加させる。",
        "相手の動きを阻害することに成功！対象の《行動値》を0にする。",
        "華麗に素材をゲット！《クレジット》を1D10点獲得する。",
        "狙いが的確に決まった！対象のスキル、アプリ、ツールのうちどれか一つ、この戦闘の間、使用不能にする。",
        "意識の外から刈り取る一撃！このアクションに対して、対象は防御判定を行えない。また、スキル、アプリ、ツールによるダメージ減少も無視する。",
        "次の動作への連携が決まる！次に行う自身のアクションのクリティカル値を2点減少させる。",
    ];
static CRITICALT: Table = Table::from_dice("クリティカル表", 1, 10, CRITICALT_ITEMS);

/// Ruby `TABLES["FUMBLET"]`。
static FUMBLET_ITEMS: &[&str] = &[
        "急にコメントが荒れて攻撃を外してしまう。「炎上」のバステを受ける。",
        "攻撃が自分に命中。1D10点のダメージを受ける（防御判定不可）",
        "アクション中に盛大にすっころぶ。「ストップ」のバステを受ける。",
        "アクションが大失敗。配信の空気が冷える。《トレダカ》が1点減少する。",
        "魔力の消費が爆増！このアクションで消費した《EP》を再度消費する。",
        "タンマツの調子が悪い。「オフライン」のバステを受ける。",
        "敵のカウンターを受ける。1D10点のダメージを受ける（防御判定不可）",
        "うっかり武器を落としてしまう。支援行動で武器を拾うまで、汎用アクション以外のアクションを行うことができない。",
        "仲間との連携に失敗。ランダムな味方一人の《EP》を1D10点減少する。",
        "攻撃は失敗だが、ネタとして大ウケ。《閲覧数》が1D10点増加する。",
    ];
static FUMBLET: Table = Table::from_dice("命中ファンブル表", 1, 10, FUMBLET_ITEMS);

/// Ruby `TABLES["FATALT"]`。
static FATALT_ITEMS: &[&str] = &[
        "行動不能。ダンジョンに身体を侵食される。異形トロフィーを1つ獲得する。",
        "ドラマチックなやられ方で配信が盛り上がる。《閲覧数》が2D10点増加する。自身は行動不能になる。",
        "お前も道連れだ！自分にダメージを与えた対象に同じダメージを与える。このダメージ減少できない。自身は行動不能になる。",
        "奇跡が起きた！？〔幸運〕で難易度10の判定に成功すると受けたダメージを０にする。",
        "致命傷だがまだ動ける！《EP》を1にする。「スリップ」のバステを受ける。",
        "行動不能。ダンジョンに身体を侵食される。異形トロフィーを1つ獲得する。",
        "行動不能。だが、タンマツにはまだエネルギーが残っている。１ラウンド後、《EP》を1にして戦線に復帰する。",
        "走馬灯が過る！走馬灯に回避のアイデアが！〔反応〕で難易度10の判定に成功すると、受けたダメージを０にする。",
        "死んだかと思ったが、ギリギリのところで持ちこたえる。《EP》を1にする。",
        "行動不能。ダンジョンに身体を侵食される。異形トロフィーを1つ獲得する。",
    ];
static FATALT: Table = Table::from_dice("致命傷表", 1, 10, FATALT_ITEMS);

/// Ruby `TABLES["RESTT"]`。
static RESTT_ITEMS: &[&str] = &[
        "辺りを探索すると、ツールを発見する。誰かがここに残していたのだろうか？お宝表(レベル1~4)を一回振る。",
        "希少な鉱床を発見。【ダンジョン資源(中級)】を一つ獲得。",
        "自身の存在が大きくブレる。任意のアクションを一つ、別のアクションに変更してもよい。",
        "素晴らしい戦術を思いつく。次回のバトルフェイズでの行動値判定で振ることができるダイスが１つ増える。",
        "視聴者の無茶振りについつい応えてしまう。調子に乗りすぎて体力が…。 《EP》が1D10点減少する。《トレダカ》を1点獲得。",
        "何気ない雑談配信。だが危うくリテラシーのない発言をしてしまい…。 〔魅力〕で難易度8の判定を行う。閲覧数が成功度分増加。判定に失敗した場合、「炎上」のバステを受ける。",
        "休憩の合間にネットサーフィン。うわ！なんか変なリンク踏んだ！？〔技術〕で難易度9の判定を行う。失敗した場合、「フリーズ」のバステを受ける。成功した場合、奇跡的に冒険者用の通販サイトに繋がる。買い物を行うことができる。",
        "急にタンマツのアプリのアップデートがはじまる。アップデートが重すぎて他の通信がうまくいかない！？〔幸運〕で難易度9の判定を行う。失敗した場合、「オフライン」のバステを受ける。成功した場合、タンマツのアプデが成功し、〔EP〕が全回復する。",
        "バッチリ熟睡。しっかりとした休憩を取ることができた。〔EP〕が2D10点回復する。",
        "やたら魔力の巡りがいい。絶好調ってやつか！？このセッションの間、すべての主能力が1点増加する。副能力の再計算を行うこと。",
    ];
static RESTT: Table = Table::from_dice("休憩表", 1, 10, RESTT_ITEMS);

/// Ruby `TABLES["TERRIBLET"]`。
static TERRIBLET_ITEMS: &[&str] = &[
        "脳が揺さぶられた！「ブライン」のバステを付与する。",
        "痛恨の一撃！最終的に与えるダメージが2倍になる。",
        "肉体の動きを阻害する一撃！対象の《行動値》を0にする。",
        "致命的な一撃！ダメージを与える代わりに、対象の《EP》を1にする。",
        "追撃を決められてしまった！ダメージを2D10点追加する。",
        "場外へ吹っ飛ばした！対象を戦場から取り除く。取り除かれた対象は、ラウンド終了時に最後尾に再配置する。",
        "悔しいが見栄えする一撃だ！《閲覧数》が1D10点増加する。",
        "衝撃が貫通する！アクションの対象になっていないキャラ1体を選択し、そのキャラにもダメージを与える。",
        "意識の外から刈り取る一撃！このアクションに対して、対象は防御判定を行えない。また、スキル、アプリ、ツールによるダメージ減少も無視する。",
        "魔力を奪う一撃！与えたダメージと同じ値だけ《EP》が回復する。",
    ];
static TERRIBLET: Table = Table::from_dice("痛恨表", 1, 10, TERRIBLET_ITEMS);

/// Ruby `TABLES["GETCT"]`。
static GETCT_ITEMS: &[&str] = &[
    "携帯食料を1つ手に入れた！ ⇒54頁参照",
    "エアバッグを1つ手に入れた！ ⇒53頁参照",
    "携帯テントを1つ手に入れた！ ⇒54頁参照",
    "特効薬を1つ手に入れた！ ⇒52頁参照",
    "ダンジョン資源（低級）を1つ手に入れた！ ⇒55頁参照",
    "スモークボールを1つ手に入れた！ ⇒53頁参照",
    "ポーションを1つ手に入れた！ ⇒52頁参照",
    "クイックポーションを1つ手に入れた！ ⇒52頁参照",
    "ダンジョン資源（低級）を1つ手に入れた！ ⇒55頁参照",
    "素晴らしい戦果で配信が盛り上がる！現在の閲覧数が1D10点上昇する。",
];
static GETCT: Table = Table::from_dice("お宝表(レベル1~4)", 1, 10, GETCT_ITEMS);

/// Ruby `TABLES["GETRT"]`。
static GETRT_ITEMS: &[&str] = &[
    "携帯保健室を1つ手に入れた！ ⇒54頁参照",
    "マショウストーンを1つ手に入れた！ ⇒55頁参照",
    "ぬいぐるみ爆弾を1つ手に入れた！ ⇒54頁参照",
    "生命の粉塵を1つ手に入れた！ ⇒52頁参照",
    "ダンジョン資源（中級）を1つ手に入れた！ ⇒55頁参照",
    "パワーポーションを1つ手に入れた！ ⇒52頁参照",
    "クリティカッターを1つ手に入れた！ ⇒53頁参照",
    "ダンジョン資源（中級）を1つ手に入れた！ ⇒55頁参照",
    "ハイポーションを1つ手に入れた！ ⇒52頁参照",
    "素晴らしい戦果で配信が盛り上がる！現在の閲覧数が2D10点上昇する。",
];
static GETRT: Table = Table::from_dice("お宝表(レベル5~8)", 1, 10, GETRT_ITEMS);

/// Ruby `TABLES["GETSRT"]`。
static GETSRT_ITEMS: &[&str] = &[
    "携帯食料を1つ手に入れた！ ⇒54頁参照",
    "フウマスリケンを1つ手に入れた！ ⇒55頁参照",
    "ダンジョン資源（上級）を1つ手に入れた！ ⇒55頁参照",
    "生命の粉塵を1つ手に入れた！ ⇒52頁参照",
    "コンティニューコインを1つ手に入れた！ ⇒52頁参照",
    "ダンジョン資源（低級）を1D10個手に入れた！ ⇒55頁参照",
    "フィリピンバクチクを1つ手に入れた！ ⇒55頁参照",
    "ダンジョン資源（上級）を1つ手に入れた！ ⇒55頁参照",
    "携帯病院を1つ手に入れた！ ⇒54頁参照",
    "素晴らしい戦果で配信が盛り上がる！現在の閲覧数が4D10点上昇する。",
];
static GETSRT: Table = Table::from_dice("お宝表(レベル9~14)", 1, 10, GETSRT_ITEMS);

/// Ruby `TABLES["GETURT"]`。
static GETURT_ITEMS: &[&str] = &[
    "ダンジョン資源（伝説）を1つ手に入れた！ ⇒55頁参照",
    "マショウストーンを1D10個手に入れた！ ⇒55頁参照",
    "エリキシルを1つ手に入れた！ ⇒54頁参照",
    "ダンジョン資源（伝説）を1つ手に入れた！ ⇒55頁参照",
    "盗賊の鍵を1つ手に入れた！ ⇒53頁参照",
    "コンティニューコインを1つ手に入れた！ ⇒52頁参照",
    "経験値を1つ手に入れた！ ⇒55頁参照",
    "ダイナマイトを1つ手に入れた！ ⇒55頁参照",
    "エリキシルを1つ手に入れた！ ⇒54頁参照",
    "素晴らしい戦果で配信が盛り上がる！現在の閲覧数が8D10点上昇する。",
];
static GETURT: Table = Table::from_dice("お宝表(レベル15~99)", 1, 10, GETURT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &Table)] = &[
    ("CRITICALT", &CRITICALT),
    ("FUMBLET", &FUMBLET),
    ("FATALT", &FATALT),
    ("RESTT", &RESTT),
    ("TERRIBLET", &TERRIBLET),
    ("GETCT", &GETCT),
    ("GETRT", &GETRT),
    ("GETSRT", &GETSRT),
    ("GETURT", &GETURT),
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
            .join("test/data/LiverLabyrinth.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/LiverLabyrinth.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/LiverLabyrinth.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("LiverLabyrinth.toml must parse");
        assert_eq!(
            data.tests.len(),
            56,
            "case count in test/data/LiverLabyrinth.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "LiverLabyrinth",
                "unexpected game system in LiverLabyrinth.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("LiverLabyrinth"), &tc.input, &mut src) {
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
                    "FAIL LiverLabyrinth:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} LiverLabyrinth cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
