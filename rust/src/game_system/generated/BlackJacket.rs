//! P4で手書き移植した `lib/bcdice/game_system/BlackJacket.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `resolute_action` / `action_result` / `roll_d100`（行為判定 `BJx`）
//! - `BlackJacket::DeathChart` と `roll_death_chart`（デスチャート `DC[LSC]x`）
//! - `TABLES`（チャレンジ・ペナルティ・チャート `CPC` / サイドトラック・チャート `STC`）
//!
//! # 表データ
//!
//! Ruby側は `I18n.t("BlackJacket.…", locale:)` で `i18n/BlackJacket/ja_jp.yml` から
//! 表と定型文を作る。Rust側は同じ値を `static` として直接持つ。データ部分
//! （`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`BlackJacket_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `BlackJacket_Korean < BlackJacket` なのに対応する）。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::BlackJacket`（ID: `BlackJacket`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlackJacket;

impl GameSystem for BlackJacket {
    fn id(&self) -> &'static str {
        "BlackJacket"
    }

    fn name(&self) -> &'static str {
        "ブラックジャケットRPG"
    }

    fn sort_key(&self) -> &'static str {
        "ふらつくしあけつとRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定（BJx）
　x：成功率
　例）BJ80
　クリティカル、ファンブルの自動的判定を行います。
　「BJ50+20-30」のように加減算記述も可能。
　成功率は上限100％、下限０％
・デスチャート(DCxY)
　x：チャートの種類。肉体：DCL、精神：DCS、環境：DCC
　Y=マイナス値
　例）DCL5：ライフが -5 の判定
　　　DCS3：サニティーが -3 の判定
　　　DCC0：クレジット 0 の判定
・チャレンジ・ペナルティ・チャート（CPC）
・サイドトラック・チャート（STC）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["BJ", "DC[LSC]", "CPC", "STC"]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// i18n `BlackJacket.death_chart_result` の整形。
///
/// 埋め込む値が6個あり、ロケールによって語順を変えられるように関数で持つ
/// （Rubyの `%<name>s` 等の名前付き書式に対応する）。
pub(crate) type DeathChartResultFormatter =
    fn(name: &str, minus: i64, dice: i64, key: i64, key_text: &str, chosen: &str) -> String;

/// 1ロケール分の表と定型文。`BlackJacket` と `BlackJacket_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `DEATH_CHARTS`（キーは `L` / `S` / `C`）
    pub(crate) death_charts: &'static [(&'static str, DeathChart)],
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static Table)],
    /// i18n `BlackJacket.action_judge`（`%<rate>d` を埋める）
    pub(crate) action_judge: fn(rate: i64) -> String,
    /// i18n `BlackJacket.death_chart_result`
    pub(crate) death_chart_result: DeathChartResultFormatter,
    /// i18n `BlackJacket.death_chart_under`
    pub(crate) death_chart_under: &'static str,
    /// i18n `BlackJacket.death_chart_over`
    pub(crate) death_chart_over: &'static str,
    /// i18n `BlackJacket.fumble`
    pub(crate) fumble: &'static str,
    /// i18n `BlackJacket.critical`
    pub(crate) critical: &'static str,
    /// i18n `BlackJacket.misery`
    pub(crate) misery: &'static str,
    /// i18n `success`
    pub(crate) success: &'static str,
    /// i18n `failure`
    pub(crate) failure: &'static str,
}

/// Ruby `BlackJacket::DeathChart`。
///
/// Ruby は `initialize` で項目数が11でなければ `ArgumentError` を投げる。
/// 表は `static` の初期化子として組み立てるので、[`DeathChart::new`] の `assert!` は
/// const評価で走る＝項目数を間違えるとコンパイルが通らない。
pub(crate) struct DeathChart {
    /// Ruby `@name`（i18n `BlackJacket.chart_name.*`）
    name: &'static str,
    /// Ruby `@chart`（キー10〜20に対応する11項目）
    chart: &'static [&'static str],
}

impl DeathChart {
    /// Ruby `DeathChart#initialize`。
    ///
    /// # Panics
    ///
    /// 項目数が11でない場合（Ruby の `ArgumentError` に対応）。
    /// `static` の初期化子から呼ぶ限りコンパイル時に落ちる。
    pub(crate) const fn new(name: &'static str, chart: &'static [&'static str]) -> Self {
        assert!(
            chart.len() == 11,
            "unexpected chart size (expected 11 items)"
        );
        Self { name, chart }
    }

    /// Ruby `DeathChart#roll(randomizer, minus_score, locale)`。
    fn roll(
        &self,
        tables: &SystemTables,
        minus_score: i64,
        rng: &mut Randomizer,
    ) -> Result<String, EvalError> {
        let dice = rng.roll_once(10)?;
        let key_number = dice + minus_score;
        let (key_text, chosen) = self.at(tables, key_number);

        Ok((tables.death_chart_result)(
            self.name,
            minus_score,
            dice,
            key_number,
            &key_text,
            chosen,
        ))
    }

    /// Ruby `DeathChart#at`。key_numberの10から20がindexの0から10に対応する。
    fn at(&self, tables: &SystemTables, key_number: i64) -> (Cow<'static, str>, &'static str) {
        if key_number < 10 {
            (
                Cow::Borrowed(tables.death_chart_under),
                self.chart.first().copied().unwrap_or(""),
            )
        } else if key_number > 20 {
            (
                Cow::Borrowed(tables.death_chart_over),
                self.chart.last().copied().unwrap_or(""),
            )
        } else {
            let index = usize::try_from(key_number - 10).unwrap_or(0);
            (
                Cow::Owned(key_number.to_string()),
                self.chart.get(index).copied().unwrap_or(""),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `BlackJacket#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = resolute_action(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(text) = roll_death_chart(tables, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(roll_tables(tables, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = tables.tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `resolute_action` の正規表現。
///
/// Ruby の `^` / `$` は行アンカーだが、`Preprocessor` が最初の空白より前しか残さないので
/// 改行は届かず、Rust の（既定の）文字列アンカーと一致する。
fn action_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^BJ(\d+([+-]\d+)*)$").expect("valid regex"))
}

/// Ruby `roll_death_chart` の正規表現（`/^DC([LSC])(\d+)$/i`）。
fn death_chart_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC([LSC])(\d+)$").expect("valid regex"))
}

/// Ruby `resolute_action`（行為判定 `BJx`）。
fn resolute_action(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = action_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: ArithmeticEvaluator.eval(m[1])
    //       = Arithmetic.eval(expr, RoundType::FLOOR) || 0
    let success_rate = arithmetic::eval(&m[1], RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);

    let (roll_result, dice10, dice01) = roll_d100(rng)?;
    // Ruby: format('%02d', roll_result)。100 は3桁のまま。
    let roll_result_text = format!("{roll_result:02}");

    let mut result = action_result(tables, roll_result, dice10, dice01, success_rate);

    result.text = [
        (tables.action_judge)(success_rate),
        format!("1D100[{dice10},{dice01}]={roll_result_text}"),
        roll_result_text.clone(),
        result.text,
    ]
    .join(" ＞ ");

    Ok(Some(result))
}

/// Ruby `action_result(total, tens, ones, success_rate)`。
fn action_result(
    tables: &SystemTables,
    total: i64,
    tens: i64,
    ones: i64,
    success_rate: i64,
) -> EvalResult {
    if total == 100 {
        EvalResult::fumble(tables.misery)
    } else if success_rate <= 0 {
        EvalResult::fumble(tables.fumble)
    } else if total <= success_rate - 100 {
        // 成功率が100を超えている分だけ自動クリティカル
        EvalResult::critical(tables.critical)
    } else if tens == ones {
        // ゾロ目はクリティカルかファンブル
        if total <= success_rate {
            EvalResult::critical(tables.critical)
        } else {
            EvalResult::fumble(tables.fumble)
        }
    } else if total <= success_rate {
        EvalResult::success(tables.success)
    } else {
        EvalResult::failure(tables.failure)
    }
}

/// Ruby `roll_d100`。10面を2回振り、10の目を0として読む（`00` は100）。
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

/// Ruby `roll_death_chart`（デスチャート `DC[LSC]x`）。
fn roll_death_chart(
    tables: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(m) = death_chart_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: DEATH_CHARTS[m[1]]。正規表現が `/i` なので小文字にもマッチしうるが、
    // `enabled_upcase_input` により入力は大文字化済みで、キーは必ず見つかる
    // （Ruby側は見つからなければ `nil.roll` で NoMethodError になる経路）。
    let Some((_, chart)) = tables.death_charts.iter().find(|(key, _)| *key == &m[1]) else {
        return Ok(None);
    };
    let minus_score = m[2].parse::<i64>().unwrap_or(0);

    Ok(Some(chart.roll(tables, minus_score, rng)?))
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの定型文
// ---------------------------------------------------------------------------

/// i18n `ja_jp.BlackJacket.action_judge`。
fn ja_action_judge(rate: i64) -> String {
    format!("行為判定(成功率:{rate}％)")
}

/// i18n `ja_jp.BlackJacket.death_chart_result`。
fn ja_death_chart_result(
    name: &str,
    minus: i64,
    dice: i64,
    key: i64,
    key_text: &str,
    chosen: &str,
) -> String {
    format!("デスチャート（{name}）[マイナス値:{minus} + 1D10(->{dice}) = {key}] ＞ {key_text} ： {chosen}")
}

// ---------------------------------------------------------------------------
// 表データ（i18n/BlackJacket/ja_jp.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// i18n `BlackJacket.death_charts.physical`。
static JA_DEATH_CHART_PHYSICAL: &[&str] = &[
    "何も無し。キミは奇跡的に一命を取り留めた。闘いは続く。",
    "激痛が走る。以後、イベント終了時まで、全ての判定の成功率－10％。",
    "もう、体が動かない……。キミは［硬直２］を受ける。",
    "渾身の一撃!!　キミは〈生存〉判定を行なう。失敗した場合、［死亡］する。",
    "突然、目の前が真っ暗になった。キミは［気絶２］を受ける。",
    "以後、イベント終了時まで、全ての判定の成功率－20％。",
    "記録的一撃!!　キミは〈生存〉－20％の判定を行なう。失敗した場合、［死亡］する。",
    "生きているのか死んでいるのか。キミは［瀕死２］を受ける。",
    "叙事詩的一撃!!　キミは〈生存〉－30％の判定を行なう。失敗した場合、［死亡］する。",
    "以後、イベント終了時まで、全ての判定の成功率－30％。",
    "神話的一撃!!　キミは宙を舞って三回転ほどした後、地面に叩きつけられる。見るも無惨な姿。肉体は原型を留めていない（キミは［死亡］した）。",
];

/// i18n `BlackJacket.death_charts.mental`。
static JA_DEATH_CHART_MENTAL: &[&str] = &[
    "何も無し。キミは歯を食いしばってストレスに耐えた。",
    "以後、イベント終了時まで、全ての判定の成功率－10％。",
    "云い知れぬ恐怖がキミを襲う。キミは［恐怖２］を受ける。",
    "とても傷ついた。キミは〈意思〉判定を行なう。失敗した場合、［絶望］してNPCとなる。",
    "キミは意識を失った。キミは［気絶２］を受ける。",
    "以後、イベント終了時まで、全ての判定の成功率－20％。",
    "信じる者にだまされたような痛み。キミは〈意思〉－20％の判定を行なう。失敗した場合、［絶望］してＮＰＣとなる。",
    "仲間に裏切られたのかも知れない。キミは［混乱２］を受ける。",
    "あまりに残酷な現実。キミは〈意思〉－30％の判定を行なう。失敗した場合、［絶望］してＮＰＣとなる。",
    "以後、イベント終了時まで、全ての判定の成功率－30％。",
    "宇宙開闢の理に触れるも、それは人類の認識限界を超える何かであった。キミは［絶望］し、以後ＮＰＣとなる。",
];

/// i18n `BlackJacket.death_charts.social`。
static JA_DEATH_CHART_SOCIAL: &[&str] = &[
    "何も無し。キミは黒い噂を握りつぶした。",
    "以後、イベント終了時まで、全ての判定の成功率－10％。",
    "ピンチ！　以後、ラウンド終了時まで、キミはカルマを使用できない。",
    "悪い噂が流れる。キミは〈交渉〉判定を行なう。失敗した場合、キミは仲間からの信頼を失って［無縁］され、ＮＰＣとなる。",
    "以後、イベント終了時まで、代償にクレジットを消費するパワーを使用できない。",
    "キミの悪評が世間に知れ渡る。協力者からの支援が打ち切られる。以後、シナリオ終了時まで、全ての判定の成功率－20％。",
    "裏切り!!　キミは〈経済〉－20％の判定を行なう。失敗した場合、キミは周囲からの信頼を失い、［無縁］され、ＮＰＣとなる。",
    "以後、シナリオ終了時まで、【環境】系の技能のレベルがすべて０となる。",
    "捏造報道？　身に覚えのない背信行為がスクープとして報道される。キミは〈心理〉－30％の判定を行なう。失敗した場合、キミは人としての尊厳を失い、［無縁］を受ける。",
    "以後、イベント終了時まで、全ての判定の成功率－30％。",
    "キミの名は史上最悪の汚点として歴史に刻まれる。もはらキミを信じる仲間はなく、キミを助ける社会もない。キミは［無縁］され、以後ＮＰＣとなる。",
];

/// i18n `BlackJacket.table.CPC.items`。
static JA_CPC_ITEMS: &[&str] = &[
    "逝去\n助けるべきＮＰＣ（ヒロインなど）が死亡する。",
    "黒星\n敵が目的を成就し、事件はPCの敗北で終了する。そのまま余韻フェイズへ。",
    "活性\n敵のボスのライフを２倍にしたうえで決戦フェイズを開始する。",
    "攻勢\n敵ボスの与ダメージに＋２D6の修正を与えたうえで決戦フェイズを開始する。",
    "大挙\n敵の数（ボス以外）を２倍にしたうえで決戦フェイズを開始する。",
    "暗黒\nすべてのエリアを［暗闇］にしたうえで決戦フェイズを開始する。",
    "猛火\n２つの戦場エリアを［ダメージゾーン２］にして、決戦フェイズを開始する。",
    "伏兵\n敵の半分をエリア１とエリア２に移動させた状態で決戦フェイズを開始する。",
    "満腹\nボス以外の敵のライフをすべて２倍にしたうえで決戦フェイズを開始する。",
    "封印\n決戦フェイズの間、PCはカルマを使用できない。決戦フェイズを開始する。",
];

/// Ruby `TABLES["CPC"]`（`1D10`）。
static JA_CPC: Table = Table::from_dice("チャレンジ・ペナルティ・チャート", 1, 10, JA_CPC_ITEMS);

/// i18n `BlackJacket.table.STC.items`。
static JA_STC_ITEMS: &[&str] = &[
    "邂逅\n偶然、ＮＰＣと出会う。どのＮＰＣが現れるかはGMが決定すること。",
    "事故\n交通事故に出くわす。周囲ではパニックが起きているかも知れない。",
    "午睡\n強烈な睡魔に襲われる。まさか、新手のヴィランの能力か？",
    "告白\nＮＰＣのひとりから、今まで秘めていた思いを吐露される。",
    "設定\n新たな設定が明かされる。実はＮＰＣの父だったとか、生来目が見えん、とか。",
    "刺客\n何者かから攻撃を受ける。第３勢力か？",
    "会敵\n偶然、仇敵のひとりと出くわす。追うべきか？　無視すべきか？",
    "不審\n怪しい人物を見かける。追うべきか？　無視すべきか？",
    "遭遇\nシナリオと関係のないヴィラン組織と遭遇する。",
    "平和\n特に何も起きなかった。",
];

/// Ruby `TABLES["STC"]`（`1D10`）。
static JA_STC: Table = Table::from_dice("サイドトラック・チャート", 1, 10, JA_STC_ITEMS);

/// Ruby `DEATH_CHARTS`（i18n `BlackJacket.chart_name.*` が表名）。
static JA_DEATH_CHARTS: &[(&str, DeathChart)] = &[
    ("L", DeathChart::new("肉体", JA_DEATH_CHART_PHYSICAL)),
    ("S", DeathChart::new("精神", JA_DEATH_CHART_MENTAL)),
    ("C", DeathChart::new("環境", JA_DEATH_CHART_SOCIAL)),
];

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static JA_TABLES: &[(&str, &Table)] = &[("CPC", &JA_CPC), ("STC", &JA_STC)];

/// `ja_jp` ロケールの表と定型文。
static JA_SYSTEM: SystemTables = SystemTables {
    death_charts: JA_DEATH_CHARTS,
    tables: JA_TABLES,
    action_judge: ja_action_judge,
    death_chart_result: ja_death_chart_result,
    death_chart_under: "10以下",
    death_chart_over: "20以上",
    fumble: "失敗 ＞ ファンブル！ パワーの代償２倍＆振り直し不可",
    critical: "成功 ＞ クリティカル！ パワーの代償１／２",
    misery: "失敗 ＞ ミザリー！ パワーの代償２倍＆振り直し不可",
    success: "成功",
    failure: "失敗",
};

#[cfg(test)]
mod tests {
    /// `test/data/BlackJacket.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// nil を返すケース（2, 41）の `rands` は上流のTOMLに残った書き換え漏れで、
    /// 出目のオラクルにならない。nil経路ではダイスを消費しないため全量が余る。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "BlackJacket",
            "BlackJacket.toml",
            89,
            &[(2, 2), (41, 1)],
        );
    }
}
