//! P4で手書き移植した `lib/bcdice/game_system/DeadlineHeroes.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DeadlineHeroes#eval_game_system_specific_command`
//! - `#resolute_action` / `#action_result` / `#roll_d100`（行為判定 `DLHx`）
//! - `#roll_death_chart` / `DeathChart`（デスチャート `DCL` / `DCS` / `DCC`）
//! - `#roll_hero_name_chart` とベース／要素表（ヒーローネームチャート `HNC`）
//! - `RealNameChart`（`RNCJ` / `RNCO`。Ruby は `RangeTable` を継承して列を結合する）
//!
//! `DeadlineHeroes_Korean` は親を継承せず、判定・表とも別実装なので
//! ロケール束ね（`SystemTables`）は使わない。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{RangeInc, RangeTable};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::DeadlineHeroes`（ID: `DeadlineHeroes`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineHeroes;

impl GameSystem for DeadlineHeroes {
    fn id(&self) -> &'static str {
        "DeadlineHeroes"
    }

    fn name(&self) -> &'static str {
        "デッドラインヒーローズRPG"
    }

    fn sort_key(&self) -> &'static str {
        "てつとらいんひいろおすRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定（DLHx）
　x：成功率
　例）DLH80
　クリティカル、ファンブルの自動的判定を行います。
　「DLH50+20-30」のように加減算記述も可能。
　成功率は上限100％、下限０％
・デスチャート(DCxY)
　x：チャートの種類。肉体：DCL、精神：DCS、環境：DCC
　Y=マイナス値
　例）DCL5：ライフが -5 の判定
　　　DCS3：サニティーが -3 の判定
　　　DCC0：クレジット 0 の判定
・ヒーローネームチャート（HNC）
・リアルネームチャート　日本（RNCJ）、海外（RNCO）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["DLH", "DC[LSC]", "RNC[JO]", "HNC"]
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

/// Ruby `DeadlineHeroes#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if dlh_head_pattern().is_match(command) {
        return Ok(resolute_action(command, rng)?.map(SpecificCommandOutput::result));
    }
    if dc_head_pattern().is_match(command) {
        return Ok(roll_death_chart(command, rng)?.map(SpecificCommandOutput::text));
    }
    if command == "HNC" {
        return Ok(Some(SpecificCommandOutput::text(roll_hero_name_chart(
            rng,
        )?)));
    }
    Ok(roll_tables(command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `/^DLH/i`。入力は `enabled_upcase_input` で大文字化済みだが原典どおり。
fn dlh_head_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DLH").expect("valid regex"))
}

/// Ruby `/^DC\w/i`。
fn dc_head_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC\w").expect("valid regex"))
}

/// Ruby `/^DLH(\d+([+-]\d+)*)$/`。
fn dlh_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^DLH(\d+([+-]\d+)*)$").expect("valid regex"))
}

/// Ruby `/^DC([LSC])(\d+)$/i`。
fn death_chart_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC([LSC])(\d+)$").expect("valid regex"))
}

/// Ruby `ArithmeticEvaluator.eval(expr)`（不正な式は 0）。
fn arithmetic_evaluator_eval(expr: &str) -> Result<i64, EvalError> {
    Ok(arithmetic::eval(expr, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0))
}

/// Ruby `String#to_i`（先頭の整数。読めなければ 0）。
fn ruby_to_i(s: &str) -> i64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return 0;
    }
    s[..end].parse().unwrap_or_else(|_| {
        if s.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

// ---------------------------------------------------------------------------
// 行為判定
// ---------------------------------------------------------------------------

/// Ruby `SUCCESS_STR` / `FAILURE_STR` / `CRITICAL_STR` / `FUMBLE_STR`。
const SUCCESS_STR: &str = "成功";
const FAILURE_STR: &str = "失敗";
const CRITICAL_STR: &str = "成功 ＞ クリティカル！ パワーの代償１／２";
const FUMBLE_STR: &str = "失敗 ＞ ファンブル！ パワーの代償２倍＆振り直し不可";

/// Ruby `DeadlineHeroes#resolute_action`。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = dlh_pattern().captures(command) else {
        return Ok(None);
    };

    let success_rate = arithmetic_evaluator_eval(&m[1])?;
    let (roll_result, dice10, dice01) = roll_d100(rng)?;
    let roll_result_text = format!("{roll_result:02}");
    let mut result = action_result(roll_result, dice10, dice01, success_rate);
    result.text = format!(
        "行為判定(成功率:{success_rate}％) ＞ 1D100[{dice10},{dice01}]={roll_result_text} ＞ {roll_result_text} ＞ {}",
        result.text
    );
    Ok(Some(result))
}

/// Ruby `DeadlineHeroes#action_result`。
fn action_result(total: i64, tens: i64, ones: i64, success_rate: i64) -> EvalResult {
    if total == 100 || success_rate <= 0 {
        EvalResult::fumble(FUMBLE_STR)
    } else if total <= success_rate - 100 {
        EvalResult::critical(CRITICAL_STR)
    } else if tens == ones {
        if total <= success_rate {
            EvalResult::critical(CRITICAL_STR)
        } else {
            EvalResult::fumble(FUMBLE_STR)
        }
    } else if total <= success_rate {
        EvalResult::success(SUCCESS_STR)
    } else {
        EvalResult::failure(FAILURE_STR)
    }
}

/// Ruby `DeadlineHeroes#roll_d100`。10 を 0 に読み替える 2d10。
fn roll_d100(rng: &mut Randomizer) -> Result<(i64, i64, i64), EvalError> {
    let mut dice10 = rng.roll_once(10)?;
    if dice10 == 10 {
        dice10 = 0;
    }
    let mut dice01 = rng.roll_once(10)?;
    if dice01 == 10 {
        dice01 = 0;
    }
    let mut roll_result = dice10 * 10 + dice01;
    if roll_result == 0 {
        roll_result = 100;
    }
    Ok((roll_result, dice10, dice01))
}

// ---------------------------------------------------------------------------
// デスチャート
// ---------------------------------------------------------------------------

/// Ruby `DeadlineHeroes::DeathChart`。
struct DeathChart {
    name: &'static str,
    chart: &'static [&'static str],
}

impl DeathChart {
    /// Ruby `DeathChart#roll`。
    fn roll(&self, rng: &mut Randomizer, minus_score: i64) -> Result<String, EvalError> {
        let dice = rng.roll_once(10)?;
        let key_number = dice + minus_score;
        let (key_text, chosen) = self.at(key_number);
        Ok(format!(
            "デスチャート（{}）[マイナス値:{minus_score} + 1D10(->{dice}) = {key_number}] ＞ {key_text} ： {chosen}",
            self.name
        ))
    }

    /// Ruby `DeathChart#at`。key 10..=20 が chart の 0..=10。
    fn at(&self, key_number: i64) -> (&'static str, &'static str) {
        if key_number < 10 {
            ("10以下", self.chart[0])
        } else if key_number > 20 {
            ("20以上", self.chart[self.chart.len() - 1])
        } else {
            let index = (key_number - 10) as usize;
            (static_i64_str(key_number), self.chart[index])
        }
    }
}

/// 10..=20 の十進表記。ヒープを使わずにキー文字列を返す。
fn static_i64_str(value: i64) -> &'static str {
    const NUMS: [&str; 11] = [
        "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
    ];
    NUMS[(value - 10) as usize]
}

/// Ruby `DEATH_CHARTS['L']`。
static DEATH_CHART_L: DeathChart = DeathChart {
    name: "肉体",
    chart: &[
        "何も無し。キミは奇跡的に一命を取り留めた。闘いは続く。",
        "激痛が走る。以後、イベント終了時まで、全ての判定の成功率－10％。",
        "キミは［硬直］ポイント２点を得る。［硬直］ポイントを所持している間、キミは「属性：妨害」のパワーを使用することができない。各ラウンド終了時、キミは所持している［硬直］ポイントを１点減らしてもよい。",
        "渾身の一撃!!　キミは〈生存〉判定を行なう。失敗した場合、［死亡］する。",
        "キミは［気絶］ポイント２点を得る。［気絶］ポイントを所持している間、キミはあらゆるパワーを使用できず、自身のターンを得ることもできない。各ラウンド終了時、キミは所持している［気絶］ポイントを１点減らしてもよい。",
        "以後、イベント終了時まで、全ての判定の成功率－20％。",
        "記録的一撃!!　キミは〈生存〉－20％の判定を行なう。失敗した場合、［死亡］する。",
        "キミは［瀕死］ポイント２点を得る。［瀕死］ポイントを所持している間、キミはあらゆるパワーを使用できず、自身のターンを得ることもできない。各ラウンド終了時、キミは所持している［瀕死］ポイントを１点を失う。全ての［瀕死］ポイントを失う前に戦闘が終了しなかった場合、キミは［死亡］する。",
        "叙事詩的一撃!!　キミは〈生存〉－30％の判定を行なう。失敗した場合、［死亡］する。",
        "以後、イベント終了時まで、全ての判定の成功率－30％。",
        "神話的一撃!!　キミは宙を舞って三回転ほどした後、地面に叩きつけられる。見るも無惨な姿。肉体は原型を留めていない（キミは［死亡］した）。",
    ],
};

/// Ruby `DEATH_CHARTS['S']`。
static DEATH_CHART_S: DeathChart = DeathChart {
    name: "精神",
    chart: &[
        "何も無し。キミは歯を食いしばってストレスに耐えた。",
        "以後、イベント終了時まで、全ての判定の成功率－10％。",
        "キミは［恐怖］ポイント２点を得る。［恐怖］ポイントを所持している間、キミは「属性：攻撃」のパワーを使用できない。各ラウンド終了時、キミは所持している［恐怖］ポイントを１点減らしてもよい。",
        "とても傷ついた。キミは〈意志〉判定を行なう。失敗した場合、［絶望］してＮＰＣとなる。",
        "キミは［気絶］ポイント２点を得る。［気絶］ポイントを所持している間、キミはあらゆるパワーを使用できず、自身のターンを得ることもできない。各ラウンド終了時、キミは所持している［気絶］ポイントを１点減らしてもよい。",
        "以後、イベント終了時まで、全ての判定の成功率－20％。",
        "信じるものに裏切られたような痛み。キミは〈意志〉－20％の判定を行なう。失敗した場合、［絶望］してＮＰＣとなる。",
        "キミは［混乱］ポイント２点を得る。［混乱］ポイントを所持している間、キミは本来味方であったキャラクターに対して、可能な限り最大の被害を与える様、行動し続ける。各ラウンド終了時、キミは所持している［混乱］ポイントを１点減らしてもよい。",
        "あまりに残酷な現実。キミは〈意志〉－30％の判定を行なう。失敗した場合、［絶望］してＮＰＣとなる。",
        "以後、イベント終了時まで、全ての判定の成功率－30％。",
        "宇宙開闢の理に触れるも、それは人類の認識限界を超える何かであった。キミは［絶望］し、以後ＮＰＣとなる。",
    ],
};

/// Ruby `DEATH_CHARTS['C']`。
static DEATH_CHART_C: DeathChart = DeathChart {
    name: "環境",
    chart: &[
        "何も無し。キミは黒い噂を握りつぶした。",
        "以後、イベント終了時まで、全ての判定の成功率－10％。",
        "ピンチ！　以後、イベント終了時まで、キミは《支援》を使用できない。",
        "裏切り!!　キミは〈経済〉判定を行なう。失敗した場合、キミはヒーローとしての名声を失い、［汚名］を受ける。",
        "以後、シナリオ終了時まで、代償にクレジットを消費するパワーを使用できない。",
        "キミの悪評は大変なもののようだ。協力者からの支援が打ち切られる。以後、シナリオ終了時まで、全ての判定の成功率－20％。",
        "信頼の失墜!!　キミは〈経済〉－20％の判定を行なう。失敗した場合、キミはヒーローとしての名声を失い、［汚名］を受ける。",
        "以後、シナリオ終了時まで、【環境】系の技能のレベルがすべて０となる。",
        "捏造報道!!　身の覚えのない犯罪への荷担が、スクープとして報道される。キミは〈経済〉－30％の判定を行なう。失敗した場合、キミはヒーローとしての名声を失い、［汚名］を受ける。",
        "以後、イベント終了時まで、全ての判定の成功率－30％。",
        "キミの名は史上最悪の汚点として永遠に歴史に刻まれる。もはやキミを信じる仲間はなく、キミを助ける社会もない。キミは［汚名］を受けた。",
    ],
};

/// Ruby `DeadlineHeroes#roll_death_chart`。
fn roll_death_chart(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = death_chart_pattern().captures(command) else {
        return Ok(None);
    };
    let chart = match &m[1] {
        "L" | "l" => &DEATH_CHART_L,
        "S" | "s" => &DEATH_CHART_S,
        "C" | "c" => &DEATH_CHART_C,
        _ => return Ok(None),
    };
    let minus_score = ruby_to_i(&m[2]);
    Ok(Some(chart.roll(rng, minus_score)?))
}

// ---------------------------------------------------------------------------
// リアルネームチャート（RangeTable + 列結合）
// ---------------------------------------------------------------------------

/// Ruby `RealNameChart#mix_column` 済みの `RNCJ`。
static RNCJ_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(1, 6),
        "\n姓: アイカワ／相川、愛川\n名（男）: アキラ／晶、章\n名（女）: アン／杏",
    ),
    (
        RangeInc::new(7, 12),
        "\n姓: アマミヤ／雨宮\n名（男）: エイジ／映司、英治\n名（女）: イノリ／祈鈴、祈",
    ),
    (
        RangeInc::new(13, 18),
        "\n姓: イブキ／伊吹\n名（男）: カズキ／和希、一輝\n名（女）: エマ／英真、恵茉",
    ),
    (
        RangeInc::new(19, 24),
        "\n姓: オガミ／尾上\n名（男）: ギンガ／銀河\n名（女）: カノン／花音、観音",
    ),
    (
        RangeInc::new(25, 30),
        "\n姓: カイ／甲斐\n名（男）: ケンイチロウ／健一郎\n名（女）: サラ／沙羅",
    ),
    (
        RangeInc::new(31, 36),
        "\n姓: サカキ／榊、阪木\n名（男）: ゴウ／豪、剛\n名（女）: シズク／雫",
    ),
    (
        RangeInc::new(37, 42),
        "\n姓: シシド／宍戸\n名（男）: ジロー／次郎、治郎\n名（女）: チズル／千鶴、千尋",
    ),
    (
        RangeInc::new(43, 48),
        "\n姓: タチバナ／橘、立花\n名（男）: タケシ／猛、武\n名（女）: ナオミ／直美、尚美",
    ),
    (
        RangeInc::new(49, 54),
        "\n姓: ツブラヤ／円谷\n名（男）: ツバサ／翼\n名（女）: ハル／華、波留",
    ),
    (
        RangeInc::new(55, 60),
        "\n姓: ハヤカワ／早川\n名（男）: テツ／鉄、哲\n名（女）: ヒカル／光",
    ),
    (
        RangeInc::new(61, 66),
        "\n姓: ハラダ／原田\n名（男）: ヒデオ／英雄\n名（女）: ベニ／紅",
    ),
    (
        RangeInc::new(67, 72),
        "\n姓: フジカワ／藤川\n名（男）: マサムネ／正宗、政宗\n名（女）: マチ／真知、町",
    ),
    (
        RangeInc::new(73, 78),
        "\n姓: ホシ／星\n名（男）: ヤマト／大和\n名（女）: ミア／深空、美杏",
    ),
    (
        RangeInc::new(79, 84),
        "\n姓: ミゾグチ／溝口\n名（男）: リュウセイ／流星\n名（女）: ユリコ／由里子",
    ),
    (
        RangeInc::new(85, 90),
        "\n姓: ヤシダ／矢志田\n名（男）: レツ／烈、裂\n名（女）: ルイ／瑠衣、涙",
    ),
    (
        RangeInc::new(91, 96),
        "\n姓: ユウキ／結城\n名（男）: レン／連、錬\n名（女）: レナ／玲奈",
    ),
    (
        RangeInc::new(97, 100),
        "名無し（何らかの理由で名前を持たない、もしくは失った）",
    ),
];

/// Ruby `TABLES['RNCJ']`。
static RNCJ: RangeTable = RangeTable::from_dice("リアルネームチャート（日本）", 1, 100, RNCJ_ITEMS);

/// Ruby `RealNameChart#mix_column` 済みの `RNCO`。
static RNCO_ITEMS: &[(RangeInc, &str)] = &[
    (
        RangeInc::new(1, 6),
        "\n名（男）: アルバス\n名（女）: アイリス\n姓: アレン",
    ),
    (
        RangeInc::new(7, 12),
        "\n名（男）: クリス\n名（女）: オリーブ\n姓: ウォーケン",
    ),
    (
        RangeInc::new(13, 18),
        "\n名（男）: サミュエル\n名（女）: カーラ\n姓: ウルフマン",
    ),
    (
        RangeInc::new(19, 24),
        "\n名（男）: シドニー\n名（女）: キルスティン\n姓: オルセン",
    ),
    (
        RangeInc::new(25, 30),
        "\n名（男）: スパイク\n名（女）: グウェン\n姓: カーター",
    ),
    (
        RangeInc::new(31, 36),
        "\n名（男）: ダミアン\n名（女）: サマンサ\n姓: キャラダイン",
    ),
    (
        RangeInc::new(37, 42),
        "\n名（男）: ディック\n名（女）: ジャスティナ\n姓: シーゲル",
    ),
    (
        RangeInc::new(43, 48),
        "\n名（男）: デンゼル\n名（女）: タバサ\n姓: ジョーンズ",
    ),
    (
        RangeInc::new(49, 54),
        "\n名（男）: ドン\n名（女）: ナディン\n姓: パーカー",
    ),
    (
        RangeInc::new(55, 60),
        "\n名（男）: ニコラス\n名（女）: ノエル\n姓: フリーマン",
    ),
    (
        RangeInc::new(61, 66),
        "\n名（男）: ネビル\n名（女）: ハーリーン\n姓: マーフィー",
    ),
    (
        RangeInc::new(67, 72),
        "\n名（男）: バリ\n名（女）: マルセラ\n姓: ミラー",
    ),
    (
        RangeInc::new(73, 78),
        "\n名（男）: ビリー\n名（女）: ラナ\n姓: ムーア",
    ),
    (
        RangeInc::new(79, 84),
        "\n名（男）: ブルース\n名（女）: リンジー\n姓: リーヴ",
    ),
    (
        RangeInc::new(85, 90),
        "\n名（男）: マーヴ\n名（女）: ロザリー\n姓: レイノルズ",
    ),
    (
        RangeInc::new(91, 96),
        "\n名（男）: ライアン\n名（女）: ワンダ\n姓: ワード",
    ),
    (
        RangeInc::new(97, 100),
        "名無し（何らかの理由で名前を持たない、もしくは失った）",
    ),
];

/// Ruby `TABLES['RNCO']`。
static RNCO: RangeTable = RangeTable::from_dice("リアルネームチャート（海外）", 1, 100, RNCO_ITEMS);

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let table = match command {
        "RNCJ" => &RNCJ,
        "RNCO" => &RNCO,
        _ => return Ok(None),
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

// ---------------------------------------------------------------------------
// ヒーローネームチャート
// ---------------------------------------------------------------------------

/// Ruby `HERO_NAME_TEMPLATES` の1件。
#[derive(Clone, Copy)]
struct HeroNameTemplate {
    text: &'static str,
    elements: &'static [&'static str],
}

/// Ruby `HERO_NAME_TEMPLATES`。
static HERO_NAME_TEMPLATES: &[HeroNameTemplate] = &[
    HeroNameTemplate {
        text: "ベースＡ＋ベースＡ",
        elements: &["ベースＡ", "ベースＢ"],
    },
    HeroNameTemplate {
        text: "ベースＢ",
        elements: &["ベースＢ"],
    },
    HeroNameTemplate {
        text: "ベースＢ×2回",
        elements: &["ベースＢ", "ベースＢ"],
    },
    HeroNameTemplate {
        text: "ベースＢ＋ベースＣ",
        elements: &["ベースＢ", "ベースＣ"],
    },
    HeroNameTemplate {
        text: "ベースＡ＋ベースＢ＋ベースＣ",
        elements: &["ベースＡ", "ベースＢ", "ベースＣ"],
    },
    HeroNameTemplate {
        text: "ベースＡ＋ベースＢ×2回",
        elements: &["ベースＡ", "ベースＢ", "ベースＢ"],
    },
    HeroNameTemplate {
        text: "ベースＢ×2回＋ベースＣ",
        elements: &["ベースＢ", "ベースＢ", "ベースＣ"],
    },
    HeroNameTemplate {
        text: "（ベースＢ）・オブ・（ベースＢ）",
        elements: &["ベースＢ", "・オブ・", "ベースＢ"],
    },
    HeroNameTemplate {
        text: "（ベースＢ）・ザ・（ベースＢ）",
        elements: &["ベースＢ", "・ザ・", "ベースＢ"],
    },
    HeroNameTemplate {
        text: "任意",
        elements: &["任意"],
    },
];

/// ヒーローネームのベース表／要素表の項目。
#[derive(Clone, Copy)]
enum HeroNameItem {
    Text(&'static str),
    Element(&'static str),
}

/// Ruby `HeroNameBaseChart`。
struct HeroNameBaseChart {
    name: &'static str,
    items: &'static [HeroNameItem],
}

/// Ruby `HeroNameElementChart` の1項目。
#[derive(Clone, Copy)]
struct HeroNameElement {
    element: &'static str,
    mean: &'static str,
}

/// Ruby `HeroNameElementChart`。
struct HeroNameElementChart {
    name: &'static str,
    items: &'static [HeroNameElement],
}

impl HeroNameElementChart {
    /// Ruby `HeroNameElementChart#roll`。戻り値は `(result, chosen)`。
    fn roll(&self, rng: &mut Randomizer) -> Result<(String, &'static str), EvalError> {
        let dice = rng.roll_once(10)?;
        let chosen = ruby_array_get(self.items, dice - 1).unwrap_or(HeroNameElement {
            element: "",
            mean: "",
        });
        Ok((
            format!(
                "{}({dice}) ＞ {} （意味：{}）",
                self.name, chosen.element, chosen.mean
            ),
            chosen.element,
        ))
    }
}

impl HeroNameBaseChart {
    /// Ruby `HeroNameBaseChart#roll`。戻り値は `(result, chosen)`。
    fn roll(&self, rng: &mut Randomizer) -> Result<(String, String), EvalError> {
        let dice = rng.roll_once(10)?;
        let chosen = match ruby_array_get(self.items, dice - 1) {
            Some(HeroNameItem::Text(text)) => {
                return Ok((
                    format!("{}({dice}) ＞ {text}", self.name),
                    (*text).to_owned(),
                ));
            }
            Some(HeroNameItem::Element(element_type)) => element_type,
            None => return Ok((format!("{}({dice}) ＞ ", self.name), String::new())),
        };

        let element_chart = element_chart(chosen).ok_or(EvalError::Internal(
            "DeadlineHeroes: missing hero name element chart",
        ))?;
        let (element_result, element) = element_chart.roll(rng)?;
        Ok((
            format!("{}({dice}) ＞ ［{chosen}］ ＞ {element_result}", self.name),
            element.to_owned(),
        ))
    }
}

/// Ruby `Array#[]`（負添字は末尾から。範囲外は nil）。
fn ruby_array_get<T: Copy>(items: &[T], index: i64) -> Option<T> {
    if index >= 0 {
        usize::try_from(index)
            .ok()
            .and_then(|i| items.get(i))
            .copied()
    } else {
        let wrapped = items.len() as i64 + index;
        if wrapped >= 0 {
            usize::try_from(wrapped)
                .ok()
                .and_then(|i| items.get(i))
                .copied()
        } else {
            None
        }
    }
}

static BASE_A: HeroNameBaseChart = HeroNameBaseChart {
    name: "ベースＡ",
    items: &[
        HeroNameItem::Text("ザ・"),
        HeroNameItem::Text("キャプテン・"),
        HeroNameItem::Text("ミスター／ミス／ミセス・"),
        HeroNameItem::Text("ドクター／プロフェッサー・"),
        HeroNameItem::Text("ロード／バロン／ジェネラル・"),
        HeroNameItem::Text("マン・オブ・"),
        HeroNameItem::Element("強さ"),
        HeroNameItem::Element("色"),
        HeroNameItem::Text("マダム／ミドル・"),
        HeroNameItem::Text("数字（1～10）・"),
    ],
};

static BASE_B: HeroNameBaseChart = HeroNameBaseChart {
    name: "ベースＢ",
    items: &[
        HeroNameItem::Element("神話／夢"),
        HeroNameItem::Element("武器"),
        HeroNameItem::Element("動物"),
        HeroNameItem::Element("鳥"),
        HeroNameItem::Element("虫／爬虫類"),
        HeroNameItem::Element("部位"),
        HeroNameItem::Element("光"),
        HeroNameItem::Element("攻撃"),
        HeroNameItem::Element("その他"),
        HeroNameItem::Text("数字（1～10）・"),
    ],
};

static BASE_C: HeroNameBaseChart = HeroNameBaseChart {
    name: "ベースＣ",
    items: &[
        HeroNameItem::Text("マン／ウーマン"),
        HeroNameItem::Text("ボーイ／ガール"),
        HeroNameItem::Text("マスク／フード"),
        HeroNameItem::Text("ライダー"),
        HeroNameItem::Text("マスター"),
        HeroNameItem::Text("ファイター／ソルジャー"),
        HeroNameItem::Text("キング／クイーン"),
        HeroNameItem::Element("色"),
        HeroNameItem::Text("ヒーロー／スペシャル"),
        HeroNameItem::Text("ヒーロー／スペシャル"),
    ],
};

static ELEM_BODY: HeroNameElementChart = HeroNameElementChart {
    name: "部位",
    items: &[
        HeroNameElement {
            element: "ハート",
            mean: "心臓",
        },
        HeroNameElement {
            element: "フェイス",
            mean: "顔",
        },
        HeroNameElement {
            element: "アーム",
            mean: "腕",
        },
        HeroNameElement {
            element: "ショルダー",
            mean: "肩",
        },
        HeroNameElement {
            element: "ヘッド",
            mean: "頭",
        },
        HeroNameElement {
            element: "アイ",
            mean: "眼",
        },
        HeroNameElement {
            element: "フィスト",
            mean: "拳",
        },
        HeroNameElement {
            element: "ハンド",
            mean: "手",
        },
        HeroNameElement {
            element: "クロウ",
            mean: "爪",
        },
        HeroNameElement {
            element: "ボーン",
            mean: "骨",
        },
    ],
};

static ELEM_WEAPON: HeroNameElementChart = HeroNameElementChart {
    name: "武器",
    items: &[
        HeroNameElement {
            element: "ナイヴス",
            mean: "短剣",
        },
        HeroNameElement {
            element: "ソード",
            mean: "剣",
        },
        HeroNameElement {
            element: "ハンマー",
            mean: "鎚",
        },
        HeroNameElement {
            element: "ガン",
            mean: "銃",
        },
        HeroNameElement {
            element: "スティール",
            mean: "刃",
        },
        HeroNameElement {
            element: "タスク",
            mean: "牙",
        },
        HeroNameElement {
            element: "ニューク",
            mean: "核",
        },
        HeroNameElement {
            element: "アロー",
            mean: "矢",
        },
        HeroNameElement {
            element: "ソウ",
            mean: "ノコギリ",
        },
        HeroNameElement {
            element: "レイザー",
            mean: "剃刀",
        },
    ],
};

static ELEM_COLOR: HeroNameElementChart = HeroNameElementChart {
    name: "色",
    items: &[
        HeroNameElement {
            element: "ブラック",
            mean: "黒",
        },
        HeroNameElement {
            element: "グリーン",
            mean: "緑",
        },
        HeroNameElement {
            element: "ブルー",
            mean: "青",
        },
        HeroNameElement {
            element: "イエロー",
            mean: "黃",
        },
        HeroNameElement {
            element: "レッド",
            mean: "赤",
        },
        HeroNameElement {
            element: "バイオレット",
            mean: "紫",
        },
        HeroNameElement {
            element: "シルバー",
            mean: "銀",
        },
        HeroNameElement {
            element: "ゴールド",
            mean: "金",
        },
        HeroNameElement {
            element: "ホワイト",
            mean: "白",
        },
        HeroNameElement {
            element: "クリア",
            mean: "透明",
        },
    ],
};

static ELEM_ANIMAL: HeroNameElementChart = HeroNameElementChart {
    name: "動物",
    items: &[
        HeroNameElement {
            element: "バニー",
            mean: "ウサギ",
        },
        HeroNameElement {
            element: "タイガー",
            mean: "虎",
        },
        HeroNameElement {
            element: "シャーク",
            mean: "鮫",
        },
        HeroNameElement {
            element: "キャット",
            mean: "猫",
        },
        HeroNameElement {
            element: "コング",
            mean: "ゴリラ",
        },
        HeroNameElement {
            element: "ドッグ",
            mean: "犬",
        },
        HeroNameElement {
            element: "フォックス",
            mean: "狐",
        },
        HeroNameElement {
            element: "パンサー",
            mean: "豹",
        },
        HeroNameElement {
            element: "アス",
            mean: "ロバ",
        },
        HeroNameElement {
            element: "バット",
            mean: "蝙蝠",
        },
    ],
};

static ELEM_MYTH: HeroNameElementChart = HeroNameElementChart {
    name: "神話／夢",
    items: &[
        HeroNameElement {
            element: "アポカリプス",
            mean: "黙示録",
        },
        HeroNameElement {
            element: "ウォー",
            mean: "戦争",
        },
        HeroNameElement {
            element: "エターナル",
            mean: "永遠",
        },
        HeroNameElement {
            element: "エンジェル",
            mean: "天使",
        },
        HeroNameElement {
            element: "デビル",
            mean: "悪魔",
        },
        HeroNameElement {
            element: "イモータル",
            mean: "死なない",
        },
        HeroNameElement {
            element: "デス",
            mean: "死神",
        },
        HeroNameElement {
            element: "ドリーム",
            mean: "夢",
        },
        HeroNameElement {
            element: "ゴースト",
            mean: "幽霊",
        },
        HeroNameElement {
            element: "デッド",
            mean: "死んでいる",
        },
    ],
};

static ELEM_ATTACK: HeroNameElementChart = HeroNameElementChart {
    name: "攻撃",
    items: &[
        HeroNameElement {
            element: "ストローク",
            mean: "一撃",
        },
        HeroNameElement {
            element: "クラッシュ",
            mean: "壊す",
        },
        HeroNameElement {
            element: "ブロウ",
            mean: "吹き飛ばす",
        },
        HeroNameElement {
            element: "ヒット",
            mean: "打つ",
        },
        HeroNameElement {
            element: "パンチ",
            mean: "殴る",
        },
        HeroNameElement {
            element: "キック",
            mean: "蹴る",
        },
        HeroNameElement {
            element: "スラッシュ",
            mean: "斬る",
        },
        HeroNameElement {
            element: "ペネトレイト",
            mean: "貫く",
        },
        HeroNameElement {
            element: "ショット",
            mean: "撃つ",
        },
        HeroNameElement {
            element: "キル",
            mean: "殺す",
        },
    ],
};

static ELEM_ETC: HeroNameElementChart = HeroNameElementChart {
    name: "その他",
    items: &[
        HeroNameElement {
            element: "ヒューマン",
            mean: "人間",
        },
        HeroNameElement {
            element: "エージェント",
            mean: "代理人",
        },
        HeroNameElement {
            element: "ブースター",
            mean: "泥棒",
        },
        HeroNameElement {
            element: "アイアン",
            mean: "鉄",
        },
        HeroNameElement {
            element: "サンダー",
            mean: "雷",
        },
        HeroNameElement {
            element: "ウォッチャー",
            mean: "監視者",
        },
        HeroNameElement {
            element: "プール",
            mean: "水たまり",
        },
        HeroNameElement {
            element: "マシーン",
            mean: "機械",
        },
        HeroNameElement {
            element: "コールド",
            mean: "冷たい",
        },
        HeroNameElement {
            element: "サイド",
            mean: "側面",
        },
    ],
};

static ELEM_BIRD: HeroNameElementChart = HeroNameElementChart {
    name: "鳥",
    items: &[
        HeroNameElement {
            element: "ホーク",
            mean: "鷹",
        },
        HeroNameElement {
            element: "ファルコン",
            mean: "隼",
        },
        HeroNameElement {
            element: "キャナリー",
            mean: "カナリア",
        },
        HeroNameElement {
            element: "ロビン",
            mean: "コマツグミ",
        },
        HeroNameElement {
            element: "イーグル",
            mean: "鷲",
        },
        HeroNameElement {
            element: "オウル",
            mean: "フクロウ",
        },
        HeroNameElement {
            element: "レイブン",
            mean: "ワタリガラス",
        },
        HeroNameElement {
            element: "ダック",
            mean: "アヒル",
        },
        HeroNameElement {
            element: "ペンギン",
            mean: "ペンギン",
        },
        HeroNameElement {
            element: "フェニックス",
            mean: "不死鳥",
        },
    ],
};

static ELEM_LIGHT: HeroNameElementChart = HeroNameElementChart {
    name: "光",
    items: &[
        HeroNameElement {
            element: "ライト",
            mean: "光",
        },
        HeroNameElement {
            element: "シャドウ",
            mean: "影",
        },
        HeroNameElement {
            element: "ファイアー",
            mean: "炎",
        },
        HeroNameElement {
            element: "ダーク",
            mean: "暗い",
        },
        HeroNameElement {
            element: "ナイト",
            mean: "夜",
        },
        HeroNameElement {
            element: "ファントム",
            mean: "幻影",
        },
        HeroNameElement {
            element: "トーチ",
            mean: "灯火",
        },
        HeroNameElement {
            element: "フラッシュ",
            mean: "閃光",
        },
        HeroNameElement {
            element: "ランタン",
            mean: "手さげランプ",
        },
        HeroNameElement {
            element: "サン",
            mean: "太陽",
        },
    ],
};

static ELEM_BUG: HeroNameElementChart = HeroNameElementChart {
    name: "虫／爬虫類",
    items: &[
        HeroNameElement {
            element: "ビートル",
            mean: "甲虫",
        },
        HeroNameElement {
            element: "バタフライ／モス",
            mean: "蝶／蛾",
        },
        HeroNameElement {
            element: "スネーク／コブラ",
            mean: "蛇",
        },
        HeroNameElement {
            element: "アリゲーター",
            mean: "ワニ",
        },
        HeroNameElement {
            element: "ローカスト",
            mean: "バッタ",
        },
        HeroNameElement {
            element: "リザード",
            mean: "トカゲ",
        },
        HeroNameElement {
            element: "タートル",
            mean: "亀",
        },
        HeroNameElement {
            element: "スパイダー",
            mean: "蜘蛛",
        },
        HeroNameElement {
            element: "アント",
            mean: "アリ",
        },
        HeroNameElement {
            element: "マンティス",
            mean: "カマキリ",
        },
    ],
};

static ELEM_STR: HeroNameElementChart = HeroNameElementChart {
    name: "強さ",
    items: &[
        HeroNameElement {
            element: "スーパー／ウルトラ",
            mean: "超",
        },
        HeroNameElement {
            element: "ワンダー",
            mean: "驚異的",
        },
        HeroNameElement {
            element: "アルティメット",
            mean: "究極の",
        },
        HeroNameElement {
            element: "ファンタスティック",
            mean: "途方もない",
        },
        HeroNameElement {
            element: "マイティ",
            mean: "強い",
        },
        HeroNameElement {
            element: "インクレディブル",
            mean: "凄い",
        },
        HeroNameElement {
            element: "アメージング",
            mean: "素晴らしい",
        },
        HeroNameElement {
            element: "ワイルド",
            mean: "狂乱の",
        },
        HeroNameElement {
            element: "グレイテスト",
            mean: "至高の",
        },
        HeroNameElement {
            element: "マーベラス",
            mean: "驚くべき",
        },
    ],
};

fn base_chart(kind: &str) -> Option<&'static HeroNameBaseChart> {
    match kind {
        "ベースＡ" => Some(&BASE_A),
        "ベースＢ" => Some(&BASE_B),
        "ベースＣ" => Some(&BASE_C),
        _ => None,
    }
}

fn element_chart(kind: &str) -> Option<&'static HeroNameElementChart> {
    match kind {
        "部位" => Some(&ELEM_BODY),
        "武器" => Some(&ELEM_WEAPON),
        "色" => Some(&ELEM_COLOR),
        "動物" => Some(&ELEM_ANIMAL),
        "神話／夢" => Some(&ELEM_MYTH),
        "攻撃" => Some(&ELEM_ATTACK),
        "その他" => Some(&ELEM_ETC),
        "鳥" => Some(&ELEM_BIRD),
        "光" => Some(&ELEM_LIGHT),
        "虫／爬虫類" => Some(&ELEM_BUG),
        "強さ" => Some(&ELEM_STR),
        _ => None,
    }
}

/// Ruby `DeadlineHeroes#roll_hero_name_chart`。
fn roll_hero_name_chart(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice = rng.roll_once(10)?;
    let template = ruby_array_get(HERO_NAME_TEMPLATES, dice - 1).unwrap_or(HeroNameTemplate {
        text: "",
        elements: &[],
    });
    let template_result = format!("ヒーローネームチャート({dice}) ＞ {}", template.text);
    if template.text == "任意" {
        return Ok(template_result);
    }

    let mut results = vec![template_result];
    let mut elements = Vec::new();
    for kind in template.elements {
        if let Some(base) = base_chart(kind) {
            let (result, element) = base.roll(rng)?;
            results.push(result);
            elements.push(element);
        } else {
            elements.push((*kind).to_owned());
        }
    }

    let hero_name = compose_hero_name(&elements);
    results.push(format!("ヒーローネーム ＞ {hero_name}"));
    Ok(results.join("\n"))
}

/// Ruby `elements.join("").gsub(/・{2,}/, "・").sub(/・$/, "")`。
fn compose_hero_name(elements: &[String]) -> String {
    let joined = elements.join("");
    let collapsed = collapse_middle_dots(&joined);
    collapsed
        .strip_suffix('・')
        .unwrap_or(&collapsed)
        .to_owned()
}

fn collapse_middle_dots(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut prev_dot = false;
    for ch in source.chars() {
        if ch == '・' {
            if !prev_dot {
                out.push(ch);
            }
            prev_dot = true;
        } else {
            out.push(ch);
            prev_dot = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DeadlineHeroes",
            "DeadlineHeroes.toml",
            113,
        );
    }
}
