//! P4で手書き移植した `lib/bcdice/game_system/DetatokoSaga.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DetatokoSaga#eval_game_system_specific_command`
//!   （`#checkRoll` → `#checkJudgeValue` → `Base#roll_tables`）
//! - `#getRollResult` / `#getSuccess` / `#getCheckFlagResult` / `#getDownWill`
//! - `#getModifyText` / `#getTotalResultValue` / `#getTotalResultValueWhenSlash`
//! - `ALIAS`（表の長い別名）と `TABLES`（`SST` / `WST` / `SBET` / `WBET`）
//!
//! # 表データ
//!
//! Ruby側は `DiceTable::Table.from_i18n("DetatokoSaga.table.…", locale)` で
//! `i18n/DetatokoSaga/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として直接持つ。
//! データ部分（`JA_` 接頭辞の `static` 群）は同YAMLから機械的に書き出したもので、
//! 値は1文字も変えていない。
//!
//! ロケール差は [`SystemTables`] に束ね、`DetatokoSaga_Korean`（`ko_kr`）が
//! 同じ関数群を使い回す（Ruby側で `DetatokoSaga_Korean < DetatokoSaga` なのに対応する）。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{RollableTable, Table};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`DetatokoSaga` と `DetatokoSaga_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, &'static Table)],
    /// i18n `DetatokoSaga.DS.input_options`（`%{skill}` / `%{flag}` / `%{target}`）
    pub(crate) ds_input_options: &'static str,
    /// i18n `DetatokoSaga.DS.modifier`（`%{modifier}`）
    pub(crate) ds_modifier: &'static str,
    /// i18n `DetatokoSaga.DS.success`
    pub(crate) ds_success: &'static str,
    /// i18n `DetatokoSaga.DS.failure`
    pub(crate) ds_failure: &'static str,
    /// i18n `DetatokoSaga.JD.input_options`（`%{skill}` / `%{flag}`）
    pub(crate) jd_input_options: &'static str,
    /// i18n `DetatokoSaga.JD.modifier`（`%{modifier}`）
    pub(crate) jd_modifier: &'static str,
    /// i18n `DetatokoSaga.total_value`（`%{total}`）
    pub(crate) total_value: &'static str,
    /// i18n `DetatokoSaga.less_than_flag`（`%{will}`）
    pub(crate) less_than_flag: &'static str,
    /// i18n `DetatokoSaga.division_by_zero_error`
    pub(crate) division_by_zero_error: &'static str,
}

/// i18n の `%{name}` 置換。
fn interpolate(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in pairs {
        out = out.replace(&format!("%{{{name}}}"), value);
    }
    out
}

/// Ruby `ALIAS`（キーは `transform_keys(&:upcase)` 済み）。
static ALIAS: &[(&str, &str)] = &[
    ("STRENGTHSTIGMATABLE", "SST"),
    ("WILLSTIGMATABLE", "WST"),
    ("STRENGTHBADENDTABLE", "SBET"),
    ("WILLBADENDTABLE", "WBET"),
];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `%r{^(\d+)DS(\d+)?(([+-/])(\d+))?(?:>=(\d+))?$}i`。
///
/// `[+-/]` は Ruby では `+`(0x2B) から `/`(0x2F) までの**文字範囲**
/// （`+` `,` `-` `.` `/` の5文字）。`regex` クレートも同じ解釈なので、
/// `[+\-/]` に「直さず」原典どおりの表記のまま使う。
fn ds_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\A(\d+)DS(\d+)?(([+-/])(\d+))?(?:>=(\d+))?\z").expect("valid regex")
    })
}

/// Ruby `%r{^(\d+)JD(\d+)?(([+-/])(\d+))?$}i`。
fn jd_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\A(\d+)JD(\d+)?(([+-/])(\d+))?\z").expect("valid regex"))
}

/// Ruby `DetatokoSaga#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = check_roll(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    if let Some(text) = check_judge_value(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }

    // Ruby: roll_tables(ALIAS[command] || command, TABLES)
    let key = ALIAS
        .iter()
        .find(|(alias, _)| *alias == command)
        .map_or(command, |(_, name)| *name);
    Ok(roll_tables(sys, key, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = sys.tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `DetatokoSaga#checkRoll`（通常判定 `xDS`）。
fn check_roll(
    sys: &SystemTables,
    string: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(m) = ds_pattern().captures(string) else {
        return Ok(None);
    };

    let skill = to_i(&m[1]);
    // Ruby: m[2].to_i（nil.to_i == 0）
    let flag = m.get(2).map_or(0, |x| to_i(x.as_str()));
    let operator = m.get(4).map(|x| x.as_str());
    let value = m.get(5).map_or(0, |x| to_i(x.as_str()));
    // Ruby: m[6]&.to_i || 8
    let target = m.get(6).map_or(8, |x| to_i(x.as_str()));

    let mut result = interpolate(
        sys.ds_input_options,
        &[
            ("skill", &skill.to_string()),
            ("flag", &flag.to_string()),
            ("target", &target.to_string()),
        ],
    );

    let modify_text = get_modify_text(operator, value);
    if !modify_text.is_empty() {
        result.push_str(&interpolate(sys.ds_modifier, &[("modifier", &modify_text)]));
    }

    let (mut total, roll_text) = get_roll_result(skill, rng)?;
    result.push_str(&format!(" ＞ {total}[{roll_text}]{modify_text}"));
    result.push_str(&format!(
        " ＞ {}",
        get_total_result_value(sys, total, value, operator)
    ));

    // Ruby: 修正表記が空でない場合だけ、加減算を判定値へ反映する（÷は反映しない）
    if !modify_text.is_empty() {
        match operator {
            Some("+") => total = total.saturating_add(value),
            Some("-") => total = total.saturating_sub(value),
            _ => {}
        }
    }

    result.push_str(&format!(" ＞ {}", get_success(sys, total, target)));
    result.push_str(&get_check_flag_result(sys, total, flag, rng)?);

    Ok(Some(result))
}

/// Ruby `DetatokoSaga#checkJudgeValue`（スキル判定値 `xJD`）。
fn check_judge_value(
    sys: &SystemTables,
    string: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some(m) = jd_pattern().captures(string) else {
        return Ok(None);
    };

    let skill = to_i(&m[1]);
    let flag = m.get(2).map_or(0, |x| to_i(x.as_str()));
    let operator = m.get(4).map(|x| x.as_str());
    let value = m.get(5).map_or(0, |x| to_i(x.as_str()));

    let mut result = interpolate(
        sys.jd_input_options,
        &[("skill", &skill.to_string()), ("flag", &flag.to_string())],
    );

    let modify_text = get_modify_text(operator, value);
    if !modify_text.is_empty() {
        result.push_str(&interpolate(sys.jd_modifier, &[("modifier", &modify_text)]));
    }

    let (total, roll_text) = get_roll_result(skill, rng)?;
    result.push_str(&format!(" ＞ {total}[{roll_text}]{modify_text}"));
    result.push_str(&format!(
        " ＞ {}",
        get_total_result_value(sys, total, value, operator)
    ));

    // Ruby: `checkRoll` と違い、判定値へ加減算を反映しないまま旗の判定に使う
    result.push_str(&get_check_flag_result(sys, total, flag, rng)?);

    Ok(Some(result))
}

/// Ruby `DetatokoSaga#getRollResult`。戻り値は `[total, diceText]`。
fn get_roll_result(skill: i64, rng: &mut Randomizer) -> Result<(i64, String), EvalError> {
    // Ruby: diceCount = skill + 1; diceCount = 3 if skill == 0
    let dice_count = if skill == 0 {
        3
    } else {
        skill.saturating_add(1)
    };

    let dice = rng.roll_barabara(dice_count, 6)?;
    // 表示に使う出目は振った順のまま
    let dice_text = dice
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut sorted = dice;
    sorted.sort_unstable();
    // Ruby: dice = dice.reverse if skill != 0（スキル0だけ小さい2つを使う）
    if skill != 0 {
        sorted.reverse();
    }

    let total = sorted[0] + sorted[1];
    Ok((total, dice_text))
}

/// Ruby `DetatokoSaga#getSuccess`。
fn get_success(sys: &SystemTables, check: i64, target: i64) -> &'static str {
    if check >= target {
        sys.ds_success
    } else {
        sys.ds_failure
    }
}

/// Ruby `DetatokoSaga#getCheckFlagResult`。
fn get_check_flag_result(
    sys: &SystemTables,
    total: i64,
    flag: i64,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    if total > flag {
        return Ok(String::new());
    }

    let will = get_down_will(flag, rng)?;
    Ok(interpolate(sys.less_than_flag, &[("will", &will)]))
}

/// Ruby `DetatokoSaga#getDownWill`。フラグが10以上なら1D6を振らない。
fn get_down_will(flag: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    if flag >= 10 {
        return Ok("6".to_owned());
    }

    let dice = rng.roll_once(6)?;
    Ok(format!("1D6->{dice}"))
}

/// Ruby `DetatokoSaga#getModifyText`。
fn get_modify_text(operator: Option<&str>, value: i64) -> String {
    // Ruby: return '' if value == 0（演算子より先に判定する）
    if value == 0 {
        return String::new();
    }

    let operator_text = match operator {
        Some("+") => "＋",
        Some("-") => "－",
        Some("/") => "÷",
        // Ruby: case の else は `return ""`
        _ => return String::new(),
    };

    format!("{operator_text}{value}")
}

/// Ruby `DetatokoSaga#getTotalResultValue`。
///
/// 修正値が0でも演算子があればこちらは展開形になる（`getModifyText` と違って
/// 0の早期リターンが無い）。原典どおりの非対称をそのまま写す。
fn get_total_result_value(
    sys: &SystemTables,
    total: i64,
    value: i64,
    operator: Option<&str>,
) -> String {
    match operator {
        Some("+") => format!(
            "{total}+{value} ＞ {}",
            total_value(sys, total.saturating_add(value))
        ),
        Some("-") => format!(
            "{total}-{value} ＞ {}",
            total_value(sys, total.saturating_sub(value))
        ),
        Some("/") => get_total_result_value_when_slash(sys, total, value),
        _ => total_value(sys, total),
    }
}

/// Ruby `DetatokoSaga#getTotalResultValueWhenSlash`。
fn get_total_result_value_when_slash(sys: &SystemTables, total: i64, value: i64) -> String {
    if value == 0 {
        return sys.division_by_zero_error.to_owned();
    }

    // Ruby: ((1.0 * total) / value).ceil（浮動小数点除算の切り上げ）
    let quotient = ((total as f64) / (value as f64)).ceil() as i64;
    format!("{total}÷{value} ＞ {}", total_value(sys, quotient))
}

/// i18n `DetatokoSaga.total_value`。
fn total_value(sys: &SystemTables, total: i64) -> String {
    interpolate(sys.total_value, &[("total", &total.to_string())])
}

/// Ruby の `String#to_i`（ここに来るのは `\d+` にマッチした文字列だけ）。
///
/// 桁あふれは Ruby だと Bignum になるので、`i64` に収まらない場合は飽和させ、
/// ダイス個数なら `roll_barabara` の上限（TooManyRandsError）へ落ちるようにする。
fn to_i(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの表と定型文
// ---------------------------------------------------------------------------

/// i18n `DetatokoSaga.table.SST.items`。
static JA_SST_ITEMS: &[&str] = &[
    "あなたは【烙印】を２つ受ける。この表をさらに２回振って受ける【烙印】を決める（その結果、再びこの出目が出ても【烙印】は増えない）。",
    "【痛手】手負い傷を負った。何とか戦えているが……。",
    "【流血】血があふれ出し、目がかすむ……。",
    "【衰弱】体が弱り、その心さえも萎えてしまいそうだ……。",
    "【苦悶】痛みと苦しみ、情けなさ。目に涙がにじむ。",
    "【衝撃】吹き飛ばされ、壁や樹木にめりこむ。早く起き上がらねば。",
    "【疲労】あなたの顔に疲労の色が強まる……この戦いがつらくなってきた。",
    "【怒号】うっとうしい攻撃に怒りの叫びを放つ。怒りが戦いを迷わせるか？",
    "【負傷】手傷を負わされた……。",
    "【軽症】あなたの肌に傷が残った。これだけなら何ということもない。",
    "奇跡的にあなたは【烙印】を受けなかった。",
];
/// i18n `DetatokoSaga.table.SST`（体力烙印表 / 2D6）。
static JA_SST: Table = Table::from_dice("体力烙印表", 2, 6, JA_SST_ITEMS);

/// i18n `DetatokoSaga.table.WST.items`。
static JA_WST_ITEMS: &[&str] = &[
    "あなたは【烙印】を２つ受ける。この表をさらに２回振って受ける【烙印】を決める（その結果、再びこの出目が出ても【烙印】は増えない）。",
    "【絶望】どうしようもない状況。希望は失われ……膝を付くことしかできない。",
    "【号泣】あまりの理不尽に、子供のように泣き叫ぶことしかできない。",
    "【後悔】こんなはずじゃなかったのに。しかし現実は非情だった。",
    "【恐怖】恐怖に囚われてしまった！敵が、己の手が、恐ろしくてならない！",
    "【葛藤】本当にこれでいいのか？何度も自身への問いかけが起こる……。",
    "【憎悪】怒りと憎しみに囚われたあなたは、本来の力を発揮できるだろうか？",
    "【呆然】これは現実なのか？ぼんやりとしながらあなたは考える。",
    "【迷い】迷いを抱いてしまった。それは戦う意志を鈍らせるだろうか？",
    "【悪夢】これから時折、あなたはこの時を悪夢に見ることだろう。",
    "奇跡的にあなたは【烙印】を受けなかった。",
];
/// i18n `DetatokoSaga.table.WST`（気力烙印表 / 2D6）。
static JA_WST: Table = Table::from_dice("気力烙印表", 2, 6, JA_WST_ITEMS);

/// i18n `DetatokoSaga.table.SBET.items`。
static JA_SBET_ITEMS: &[&str] = &[
    "【死亡】あなたは死んだ。次のセッションに参加するには、クラス１つを『モンスター』か『暗黒』にクラスチェンジしなくてはいけない。",
    "【命乞】あなたは恐怖に駆られ、命乞いをしてしまった！次のセッション開始時に、クラス１つが『ザコ』に変更される！",
    "【忘却】あなたは記憶を失い、ぼんやりと立ち尽くす。次のセッションに参加するには、クラス１つを変更しなくてはならない。",
    "【悲劇】あなたの攻撃は敵ではなく味方を撃った！全てが終わるまであなたは立ち尽くしていた。任意の味方の【体力】を１Ｄ６点減少させる。",
    "【暴走】あなたは正気を失い、衝動のまま暴走する！同じシーンにいる全員の【体力】を１Ｄ６点減少させる。",
    "【転落】あなたは断崖絶壁から転落した。",
    "【虜囚】あなたは敵に囚われた。",
    "【逃走】あなたは恐れをなし、仲間を見捨てて逃げ出した。",
    "【重症】あなたはどうしようもない痛手を負い、倒れた。",
    "【気絶】あなたは気を失った。そして目覚めれば全てが終わっていた。",
    "それでもまだ立ち上がる！あなたはバッドエンドを迎えなかった。体力の【烙印】を１つ打ち消してよい。",
];
/// i18n `DetatokoSaga.table.SBET`（体力バッドエンド表 / 2D6）。
static JA_SBET: Table = Table::from_dice("体力バッドエンド表", 2, 6, JA_SBET_ITEMS);

/// i18n `DetatokoSaga.table.WBET.items`。
static JA_WBET_ITEMS: &[&str] = &[
    "【自害】あなたは自ら死を選んだ。次のセッションに参加するには、クラス１つを『暗黒』にクラスチェンジしなくてはいけない。",
    "【堕落】あなたは心の中の闇に飲まれた。次のセッション開始時に、クラス１つが『暗黒』か『モンスター』に変更される！",
    "【隷属】あなたは敵の言うことに逆らえない。次のセッションであなたのスタンスは『従属』になる。",
    "【裏切】裏切りの衝動。任意の味方の【体力】を１Ｄ６点減少させ、その場から逃げ出す。",
    "【暴走】あなたは正気を失い、衝動のまま暴走する！同じシーンにいる全員の【体力】を１Ｄ６点減少させる。",
    "【呪い】心の闇が顕在化したのか。敵の怨嗟か。呪いに蝕まれたあなたは、のたうちまわることしかできない。",
    "【虜囚】あなたは敵に囚われ、その場から連れ去られる。",
    "【逃走】あなたは恐れをなし、仲間を見捨てて逃げ出した。",
    "【放心】あなたはただぼんやりと立ち尽くすしかなかった。我に返った時、全ては終わっていた。",
    "【気絶】あなたは気を失った。そして目覚めれば全てが終わっていた。",
    "それでもまだ諦めない！あなたはバッドエンドを迎えなかった。あなたは気力の【烙印】を１つ打ち消してよい。",
];
/// i18n `DetatokoSaga.table.WBET`（気力バッドエンド表 / 2D6）。
static JA_WBET: Table = Table::from_dice("気力バッドエンド表", 2, 6, JA_WBET_ITEMS);

/// `ja_jp` ロケールの表と定型文一式。
pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: &[
        ("SST", &JA_SST),
        ("WST", &JA_WST),
        ("SBET", &JA_SBET),
        ("WBET", &JA_WBET),
    ],
    ds_input_options: "判定！　スキルランク：%{skill}　フラグ：%{flag}　目標値：%{target}",
    ds_modifier: "　修正値：%{modifier}",
    ds_success: "目標値以上！【成功】",
    ds_failure: "目標値未満…【失敗】",
    jd_input_options: "判定！　スキルランク：%{skill}　フラグ：%{flag}",
    jd_modifier: "　修正値：%{modifier}",
    total_value: "判定値：%{total}",
    less_than_flag: "、フラグ以下！【気力%{will}点減少】【判定値変更不可】",
    division_by_zero_error: "0では割れません",
};

/// Ruby `BCDice::GameSystem::DetatokoSaga`（ID: `DetatokoSaga`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetatokoSaga;

impl GameSystem for DetatokoSaga {
    fn id(&self) -> &'static str {
        "DetatokoSaga"
    }

    fn name(&self) -> &'static str {
        "でたとこサーガ"
    }

    fn sort_key(&self) -> &'static str {
        "てたとこさあか"
    }

    fn help_message(&self) -> &'static str {
        r"・通常判定　xDS or xDSy or xDS>=t or xDSy>=t or xDS+z>=t or xDSy+z>=t
　(x＝スキルランク、y＝現在フラグ値(省略時0)、z＝修正値(省略時０)、t＝目標値(省略時８))
　例）3DS　2DS5　0DS　3DS>=10　3DS7>=12 2DS3+1 3DS2+1>=10
・判定値　xJD or xJDy or xJDy+z or xJDy-z or xJDy/z
　(x＝スキルランク、y＝現在フラグ値(省略時0)、z＝修正値(省略時０))
　例）3JD　2JD5　3JD7+1　4JD/3
・体力烙印表　SST (StrengthStigmaTable)
・気力烙印表　WST (WillStigmaTable)
・体力バッドエンド表　SBET (StrengthBadEndTable)
・気力バッドエンド表　WBET (WillBadEndTable)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+DS",
            r"\d+JD",
            "SST",
            "WST",
            "SBET",
            "WBET",
            "STRENGTHSTIGMATABLE",
            "WILLSTIGMATABLE",
            "STRENGTHBADENDTABLE",
            "WILLBADENDTABLE",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DetatokoSaga#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `DetatokoSaga#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `DetatokoSaga#eval_game_system_specific_command`。
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
            .join("test/data/DetatokoSaga.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/DetatokoSaga.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/DetatokoSaga.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("DetatokoSaga.toml must parse");
        assert_eq!(
            data.tests.len(),
            46,
            "case count in test/data/DetatokoSaga.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "DetatokoSaga",
                "unexpected game system in DetatokoSaga.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("DetatokoSaga"), &tc.input, &mut src) {
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
                    "FAIL DetatokoSaga:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} DetatokoSaga cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
