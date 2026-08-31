//! P4で手書き移植した `lib/bcdice/game_system/ShinkuuGakuen.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#eval_game_system_specific_command`（`(武器記号)(技能ベース)(>=目標値)` の判定）
//! - `#rollJudge` / `#getJudgeDiceList` / `#getSuccessText` / `#getSkillText`
//! - `#getWeaponTable` と `#getWeaponTableXxx` 21種、`#getWeaponSkillText`、
//!   `#getRandMartialArtCounter`
//!
//! # 表データ
//!
//! `WEAPON_` 接頭辞の `static` 群は `.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない（誤記に見える `五月雨斬り」` 等もそのまま）。

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `/^([A-Z]+)([+-]?\d+)?(?:>=(\d+))?$/i`。
///
/// Ruby の `/i` 付き `[A-Z]` はASCIIのみを畳むが、`regex` クレートの `(?i)` は
/// Unicode ケースフォールディングを行い `K`(U+212A) 等も拾ってしまう。
/// 挙動を合わせるため `(?i)` を使わず `[A-Za-z]` と書く。
fn command_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Za-z]+)([+-]?\d+)?(?:>=(\d+))?$").expect("valid regex"))
}

/// Ruby `ShinkuuGakuen#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = command_pattern().captures(command) else {
        return Ok(None);
    };

    let weapon_command = &m[1];
    // Ruby: base = m[2].to_i（`nil.to_i` は 0）
    let base = m.get(2).map_or(0, |v| to_i(v.as_str()));
    let diff = m.get(3).map(|v| v.as_str());

    let weapon_info = get_weapon_table(weapon_command, rng)?;
    let output_msg = roll_judge(base, diff, &weapon_info, rng)?;

    Ok(Some(SpecificCommandOutput::text(output_msg)))
}

/// Ruby の `String#to_i`（多倍長）。`i64` に収まらない指定は飽和させる。
///
/// ここに来るのは `[+-]?\d+` か `\d+` にマッチした部分文字列だけ。
fn to_i(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or_else(|_| {
        if digits.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Ruby `#rollJudge`。
fn roll_judge(
    base: i64,
    diff: Option<&str>,
    weapon_info: &WeaponInfo,
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice_list = get_judge_dice_list(rng)?;
    let total = dice_list.iter().fold(0i64, |a, b| a.wrapping_add(*b));
    let all_total = total.wrapping_add(base);

    let diff_text = diff.map_or(String::new(), |d| format!(">={d}"));
    let mut result = format!(
        "({}：{base}{diff_text}) ＞ 1D100+{base} ＞ {total}",
        weapon_info.name
    );
    if dice_list.len() >= 2 {
        let joined = dice_list
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        result += &format!("[{joined}]");
    }
    // Ruby: result += "+#{base}"（base が負なら "+-5" になるが、そのまま合わせる）
    result += &format!("+{base}");
    result += &format!(" ＞ {all_total}");
    result += &get_success_text(all_total, diff, &dice_list, weapon_info.table.is_some());
    // Ruby: diceList は必ず1個以上あるので max は nil にならない
    let max = dice_list.iter().copied().max().unwrap_or(0);
    result += &get_weapon_skill_text(weapon_info.table.as_deref(), max);

    Ok(result)
}

/// Ruby `#getJudgeDiceList`。
///
/// 1の位が0（＝10の倍数）である限り振り足す。
/// 振り足しが止まらなくても `Randomizer` の総回数上限で必ずエラーになる。
fn get_judge_dice_list(rng: &mut Randomizer) -> Result<Vec<i64>, EvalError> {
    let mut dice_list = Vec::new();
    loop {
        let value = rng.roll_once(100)?;
        dice_list.push(value);

        let rank01 = value % 10;
        if rank01 != 0 {
            break;
        }
    }
    Ok(dice_list)
}

/// Ruby `#getSuccessText`。
///
/// `is_weapon` は Ruby の `isWeapon`（＝`weaponInfo[:table]`）の真偽。
fn get_success_text(
    all_total: i64,
    diff: Option<&str>,
    dice_list: &[i64],
    is_weapon: bool,
) -> String {
    let Some(first) = dice_list.first().copied() else {
        return String::new();
    };

    if first <= 9 {
        return " ＞ ファンブル".to_string();
    }

    if diff.is_none() && first != 10 {
        return String::new();
    }

    let skill_text = get_skill_text(first, diff, is_weapon);
    let mut result = skill_text.clone();

    if let Some(diff) = diff {
        // Ruby: result += ' ＞ ' if skillText.empty?
        if skill_text.is_empty() {
            result += " ＞ ";
        }
        result += if all_total >= to_i(diff) {
            "成功"
        } else {
            "失敗"
        };
    }

    result
}

/// Ruby `#getSkillText`。
///
/// 武器の表があるときは空文字列。無い（`判定`）ときは必ず `' ＞ '` から始まる
/// **非空** の文字列を返す。この `' ＞ '` が成功/失敗の前の区切りを兼ねている。
fn get_skill_text(first: i64, diff: Option<&str>, is_weapon: bool) -> String {
    if is_weapon {
        return String::new();
    }

    let mut result = " ＞ ".to_string();
    if first != 10 {
        return result;
    }

    result += "技能なし：ファンブル";
    if diff.is_none() {
        return result;
    }

    result += "／技能あり：";
    result
}

// ---------------------------------------------------------------------------
// 武器表
// ---------------------------------------------------------------------------

/// Ruby の表の1行 `[index, name, effect]`。`nil` は `None`（直前の行から引き継ぐ）。
type StaticRow = (i64, Option<&'static str>, Option<&'static str>);

/// Ruby `getWeaponTableXxx` の戻り値 `{name:, table:}`。
struct WeaponInfo {
    /// Ruby `weaponInfo[:name]`
    name: &'static str,
    /// Ruby `weaponInfo[:table]`。武器記号に当たらなければ `nil`（＝`判定`）。
    ///
    /// Ruby はメソッド呼び出しのたびに配列を作り直す。体術カウンターの `[99]` は
    /// その場でD10を振った結果を効果へ連結するので、Rust側も毎回組み立てる。
    table: Option<Vec<WeaponRow>>,
}

/// [`StaticRow`] を1行分展開したもの。
struct WeaponRow {
    /// Ruby `index`
    index: i64,
    /// Ruby `name`
    name: Option<&'static str>,
    /// Ruby `effect`
    effect: Option<Cow<'static, str>>,
}

/// `static` の表をそのまま行へ展開する。
fn build_rows(rows: &'static [StaticRow]) -> Vec<WeaponRow> {
    rows.iter()
        .map(|&(index, name, effect)| WeaponRow {
            index,
            name,
            effect: effect.map(Cow::Borrowed),
        })
        .collect()
}

/// Ruby `#getWeaponTableMartialArtCounter`。
///
/// Ruby は `[99]` の効果を
/// `'Ｄ１０で振った必殺技によるカウンター' + getRandMartialArtCounter` として組み立てる。
/// つまり **この表を引くたびに必ずD10を1回振る**（`[99]` に当たらなくても振る）。
fn martial_art_counter_rows(rng: &mut Randomizer) -> Result<Vec<WeaponRow>, EvalError> {
    let rand_text = get_rand_martial_art_counter(rng)?;
    Ok(WEAPON_MARTIAL_ART_COUNTER
        .iter()
        .map(|&(index, name, effect)| WeaponRow {
            index,
            name,
            effect: match (index, effect) {
                (99, Some(e)) => Some(Cow::Owned(format!("{e}{rand_text}"))),
                _ => effect.map(Cow::Borrowed),
            },
        })
        .collect())
}

/// Ruby `#getRandMartialArtCounter`。
fn get_rand_martial_art_counter(rng: &mut Randomizer) -> Result<String, EvalError> {
    let value = rng.roll_once(10)?;
    let mut dice = value * 10 + value;
    // Ruby: dice = 100 if value == 110
    // `value` は 1〜10 なので到達しない分岐だが、Ruby の記述をそのまま残す。
    if value == 110 {
        dice = 100;
    }

    let weapon_table = build_rows(WEAPON_MARTIAL_ART);

    let mut result = format!(" ＞ ({value})");
    result += &get_weapon_skill_text(Some(&weapon_table), dice);

    Ok(result)
}

/// Ruby `#getWeaponTable`。
fn get_weapon_table(weapon_command: &str, rng: &mut Randomizer) -> Result<WeaponInfo, EvalError> {
    // Ruby: case weaponCommand.upcase
    // `Base#dice_command` が `upcase` 済みの文字列を渡すので実際には変わらない。
    let upper = weapon_command.to_ascii_uppercase();

    if upper == "CMA" {
        return Ok(WeaponInfo {
            name: "体術カウンター",
            table: Some(martial_art_counter_rows(rng)?),
        });
    }

    let (name, table) = match upper.as_str() {
        "SW" => ("剣", WEAPON_SWORD),
        "CSW" => ("剣カウンター", WEAPON_SWORD_COUNTER),
        "LS" => ("大剣", WEAPON_LONG_SWORD),
        "CLS" => ("大剣カウンター", WEAPON_LONG_SWORD_COUNTER),
        "SS" => ("小剣", WEAPON_SHORT_SWORD),
        "CSS" => ("小剣カウンター", WEAPON_SHORT_SWORD_COUNTER),
        "SP" => ("槍", WEAPON_SPEAR),
        "CSP" => ("槍カウンター", WEAPON_SPEAR_COUNTER),
        "AX" => ("斧", WEAPON_AX),
        "CAX" => ("斧カウンター", WEAPON_AX_COUNTER),
        "CL" => ("棍棒", WEAPON_CLUB),
        "CCL" => ("棍棒カウンター", WEAPON_CLUB_COUNTER),
        "BW" => ("弓", WEAPON_BOW),
        "MA" => ("体術", WEAPON_MARTIAL_ART),
        "BX" => ("ボクシング", WEAPON_BOXING),
        "CBX" => ("ボクシングカウンター", WEAPON_BOXING_COUNTER),
        "PR" => ("プロレス", WEAPON_PRO_WRESTLING),
        "CPR" => ("プロレスカウンター", WEAPON_PRO_WRESTLING_COUNTER),
        "ST" => ("幽波紋", WEAPON_STAND),
        "CST" => ("幽波紋カウンター", WEAPON_STAND_COUNTER),
        // Ruby: return {name: '判定', table: nil}
        _ => {
            return Ok(WeaponInfo {
                name: "判定",
                table: None,
            })
        }
    };

    Ok(WeaponInfo {
        name,
        table: Some(build_rows(table)),
    })
}

/// Ruby `#getWeaponSkillText`。
///
/// `name` / `effect` が `nil` の行は直前の行の値を引き継ぐ。
fn get_weapon_skill_text(weapon_table: Option<&[WeaponRow]>, dice: i64) -> String {
    let Some(weapon_table) = weapon_table else {
        return String::new();
    };

    let mut pre_name: &str = "";
    let mut pre_effect: &str = "";

    for row in weapon_table {
        let name = row.name.unwrap_or(pre_name);
        pre_name = name;

        let effect = row.effect.as_deref().unwrap_or(pre_effect);
        pre_effect = effect;

        if row.index != dice % 100 {
            continue;
        }

        return format!(" ＞ 「{name}」{effect}");
    }

    String::new()
}

/// Ruby `getWeaponTableSword` の `table`（剣）。
static WEAPON_SWORD: &[StaticRow] = &[
    (11, Some("失礼剣"), Some("成功度＋５")),
    (22, Some("隼斬り"), Some("回避不可")),
    (33, Some("みじん斬り"), Some("攻撃量２倍")),
    (44, Some("天地二段"), Some("２連続攻撃")),
    (55, Some("波動剣"), Some("カウンター不可、Ｂ・Ｄ")),
    (66, Some("疾風剣"), Some("攻撃量３倍､盾受けー１００")),
    (77, Some("残像剣"), Some("全体攻撃、Ｂ・Ｄ")),
    (88, Some("五月雨斬り」"), Some("回避不可．ダメージ３倍")),
    (
        99,
        Some("ライジングノヴア」"),
        Some("２連続攻撃・２撃目敵無防備、Ｂ・Ｄ"),
    ),
    (
        0,
        Some("光速剣"),
        Some("攻撃量3倍､盾受け不可､カウンター不可、Ｂ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableSwordCounter` の `table`（剣カウンター）。
static WEAPON_SWORD_COUNTER: &[StaticRow] = &[
    (33, Some("パリィ"), Some("攻撃の無効化")),
    (44, None, None),
    (55, None, None),
    (66, Some("かすみ青眼"), Some("カウンター")),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (
        0,
        Some("不動剣"),
        Some("クロスカウンター、Ｂ・Ｄ、ダメージ２倍"),
    ),
];

/// Ruby `getWeaponTableLongSword` の `table`（大剣）。
static WEAPON_LONG_SWORD: &[StaticRow] = &[
    (11, Some("スマッシュ"), Some("敵防御半分")),
    (22, Some("峰打ち"), Some("麻痺硬化「根性」０")),
    (33, Some("水鳥剣"), Some("敵防御判定ー５０")),
    (44, Some("ブルクラッシュ"), Some("敵防御力無視")),
    (55, Some("逆風の太刀"), Some("カウンター不可、ダメージ２倍")),
    (66, Some("濁流剣"), Some("回避不可、カウンター不可、Ｂ・Ｄ")),
    (77, Some("清流剣"), Some("回避不可、カウンター不可、Ｂ・Ｄ")),
    (
        88,
        Some("燕返し"),
        Some("２連続攻撃・２撃目カウンター不可、Ｂ・Ｄ"),
    ),
    (
        99,
        Some("地ずり残月"),
        Some("盾受け不可、ダメージ３倍、Ｂ・Ｄ"),
    ),
    (
        0,
        Some("乱れ雪月花"),
        Some("３連続攻撃・三撃目敵無防備、ダメージ３倍、防御力無視、Ｂ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableLongSwordCounter` の `table`（大剣カウンター）。
static WEAPON_LONG_SWORD_COUNTER: &[StaticRow] = &[
    (22, Some("無形の位"), Some("攻撃の無効化")),
    (33, None, None),
    (44, None, None),
    (55, Some("双破"), Some("クロスカウンター、Ｂ・Ｄ")),
    (66, None, None),
    (77, None, None),
    (88, Some("喪心無想"), Some("カウンター、攻撃量６倍")),
    (99, None, None),
    (0, None, None),
];

/// Ruby `getWeaponTableShortSword` の `table`（小剣）。
static WEAPON_SHORT_SWORD: &[StaticRow] = &[
    (11, Some("乱れ突き"), Some("２連続攻撃")),
    (22, Some("フェイクタング"), Some("スタン効果「注意力」５")),
    (33, Some("マインドステア"), Some("麻痺効果「注意力」０")),
    (44, Some("サイドワインダー"), Some("成功度＋３、盾受け不可")),
    (
        55,
        Some("スクリュードライバー"),
        Some("防御力無視、ダメージ２倍"),
    ),
    (66, Some("ニードルロンド"), Some("３連続攻撃")),
    (
        77,
        Some("プラズマブラスト"),
        Some("麻痺効果「根性」０、Ｂ・Ｄ"),
    ),
    (
        88,
        Some("サザンクロス"),
        Some("麻痺効果「根性」５、攻撃量２倍"),
    ),
    (
        99,
        Some("ファイナルレター"),
        Some("気絶効果「根性」０、回避不可、カウンター不可、Ｂ・Ｄ"),
    ),
    (
        0,
        Some("百花繚乱"),
        Some("回避不可、盾受け不可、攻撃量３倍、Ｂ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableShortSwordCounter` の `table`（小剣カウンター）。
static WEAPON_SHORT_SWORD_COUNTER: &[StaticRow] = &[
    (11, Some("リポスト"), Some("カウンター")),
    (22, None, None),
    (33, None, None),
    (44, None, None),
    (55, None, None),
    (66, None, None),
    (77, None, None),
    (
        88,
        Some("マタドール"),
        Some("カウンター、麻痺効果「注意力」５"),
    ),
    (99, None, None),
    (0, Some("マリオネット"), Some("攻撃の相手を変える")),
];

/// Ruby `getWeaponTableSpear` の `table`（槍）。
static WEAPON_SPEAR: &[StaticRow] = &[
    (11, Some("チャージ"), Some("ダメージ１．５倍、盾受けー３０")),
    (22, Some("稲妻突き"), Some("回避不可")),
    (33, Some("脳削り"), Some("麻痺効果「根性」０")),
    (44, Some("大車輪"), Some("全体攻撃")),
    (55, Some("狂乱撃"), Some("二回攻撃")),
    (
        66,
        Some("スパイラルチャージ"),
        Some("盾受け不可、ダメージ２倍、Ｂ・Ｄ"),
    ),
    (
        77,
        Some("双龍波"),
        Some("スタン効果「注意力」５、盾受け不可、Ｂ・Ｄ"),
    ),
    (
        88,
        Some("流星衝"),
        Some("カウンター不可、ダメージ３倍、次行動まで攻撃対象にならない"),
    ),
    (
        99,
        Some("ランドスライサー"),
        Some("全体攻撃、回避不可、カウンター不可、Ｂ・Ｄ"),
    ),
    (
        0,
        Some("無双三段"),
        Some("三段攻撃、二段目Ｂ・Ｄ、三段目ダメージ２倍、Ｂ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableSpearCounter` の `table`（槍カウンター）。
static WEAPON_SPEAR_COUNTER: &[StaticRow] = &[
    (55, Some("風車"), Some("カウンター、ダメージ２倍")),
    (66, None, None),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (0, None, None),
];

/// Ruby `getWeaponTableAx` の `table`（斧）。
static WEAPON_AX: &[StaticRow] = &[
    (11, Some("一人時間差"), Some("防御行動ー１００")),
    (22, Some("トマホーク"), Some("カウンター不可")),
    (33, Some("大木断"), Some("ダメージ２倍")),
    (44, Some("ブレードロール"), Some("全体攻撃")),
    (55, Some("マキ割りスペシャル"), Some("盾受け不可、Ｂ・Ｄ")),
    (66, Some("ヨーヨー"), Some("カウンター不可、２連続攻撃")),
    (
        77,
        Some("メガホーク"),
        Some("カウンター不可、全体攻撃、攻撃量２倍"),
    ),
    (88, Some("デッドリースピン"), Some("回避不可、攻撃量５倍")),
    (
        99,
        Some("マキ割りダイナミック"),
        Some("盾受け不可、ダメージ２倍、Ｂ・Ｄ、ターンの最後に命中"),
    ),
    (
        0,
        Some("高速ナブラ"),
        Some("回避不可、カウンター不可、攻撃量３倍、Ｂ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableAxCounter` の `table`（斧カウンター）。
static WEAPON_AX_COUNTER: &[StaticRow] = &[
    (44, Some("真っ向唐竹割り"), Some("クロスカウンター、Ｂ・Ｄ")),
    (55, None, None),
    (66, None, None),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (0, None, None),
];

/// Ruby `getWeaponTableClub` の `table`（棍棒）。
static WEAPON_CLUB: &[StaticRow] = &[
    (11, Some("ハードヒット"), Some("防御力無視")),
    (22, Some("ダブルヒット"), Some("２連続攻撃")),
    (33, Some("回転撃"), Some("防御判定ー１００")),
    (44, Some("飛翔脳天撃"), Some("麻痺効果「根性」５")),
    (55, Some("削岩撃"), Some("盾受け不可、攻撃量３倍")),
    (
        66,
        Some("地裂撃"),
        Some("防御力無視、カウンター不可、盾受け不可、スタン効果「注意力」０"),
    ),
    (77, Some("トリプルヒット"), Some("３連続攻撃")),
    (
        88,
        Some("亀甲羅割り"),
        Some("防御力半分、盾受け不可、Ｂ・Ｄ"),
    ),
    (
        99,
        Some("叩きつぶす"),
        Some("防御力無視、防御行動、カウンター不可、Ｂ・Ｄ"),
    ),
    (
        0,
        Some("グランドクロス"),
        Some("防御無視、盾、カウンター不可、ダメージ２倍、Ｂ・Ｄ、全体攻撃"),
    ),
];

/// Ruby `getWeaponTableClubCounter` の `table`（棍棒カウンター）。
static WEAPON_CLUB_COUNTER: &[StaticRow] = &[
    (11, Some("ブロッキング"), Some("攻撃の無効化")),
    (22, None, None),
    (33, None, None),
    (44, None, None),
    (55, None, None),
    (66, Some("ジャストミート"), Some("飛び道具のみカウンター")),
    (77, None, None),
    (88, None, None),
    (
        99,
        Some("ホームラン"),
        Some("すべての攻撃に対するカウンター"),
    ),
    (0, None, None),
];

/// Ruby `getWeaponTableBow` の `table`（弓）。
static WEAPON_BOW: &[StaticRow] = &[
    (11, Some("影縫い"), Some("麻痺効果「注意力」０")),
    (22, Some("アローレイン"), Some("全体攻撃・回避ー５０")),
    (33, Some("速射"), Some("２連続攻撃")),
    (44, Some("瞬速の矢"), Some("防御不可")),
    (
        55,
        Some("バラージシュート"),
        Some("全体攻撃・盾受け不可・攻撃量２倍"),
    ),
    (66, Some("貫きの矢"), Some("防御力無視、Ｂ・Ｄ")),
    (77, Some("落鳳波"), Some("回避不可、Ｂ・Ｄ")),
    (88, Some("皆死ね矢"), Some("全体攻撃、気絶効果「根性」５")),
    (99, Some("ミリオンダラー"), Some("三連続攻撃")),
    (0, Some("夢想弓"), Some("Ｂ・Ｄ、ダメージ３倍")),
];

/// Ruby `getWeaponTableMartialArt` の `table`（体術）。
static WEAPON_MARTIAL_ART: &[StaticRow] = &[
    (11, Some("集気法"), Some("通常ダメージ分自分のＨＰ回復")),
    (22, Some("コンビネーション"), Some("２連続攻撃")),
    (
        33,
        Some("逆一本"),
        Some("盾受け不可、防御力半分、スタン効果「根性」０"),
    ),
    (
        44,
        Some("コークスクリューブロー"),
        Some("防御力無視、ダメージ３倍"),
    ),
    (55, Some("練気拳"), Some("全体攻撃・回避不可")),
    (66, Some("バベルクランプル"), Some("盾受け不可、Ｂ・Ｄ")),
    (77, Some("マシンガンジャブ"), Some("３連続攻撃")),
    (
        88,
        Some("ナイアガラフォール"),
        Some("盾受け不可、Ｂ・Ｄ、ダメージ２倍"),
    ),
    (
        99,
        Some("羅刹掌"),
        Some("防御力無視、防御不可、Ｂ・Ｄ、ダメージ３倍"),
    ),
    (
        0,
        Some("千手観音"),
        Some("５連続攻撃、すべてカウンター不可"),
    ),
];

/// Ruby `getWeaponTableMartialArtCounter` の `table`（体術カウンター）。
static WEAPON_MARTIAL_ART_COUNTER: &[StaticRow] = &[
    (11, Some("スウェイバック"), Some("攻撃の無効化")),
    (22, None, None),
    (33, Some("当て身投げ"), Some("カウンター")),
    (44, None, None),
    (55, None, None),
    (
        66,
        Some("ジョルトカウンター"),
        Some("クロスカウンター、Ｂ・Ｄ"),
    ),
    (77, None, None),
    (88, None, None),
    (
        99,
        Some("ガードキャンセル"),
        Some("Ｄ１０で振った必殺技によるカウンター"),
    ), // Ruby はここに getRandMartialArtCounter の結果を連結する
    (0, None, None),
];

/// Ruby `getWeaponTableBoxing` の `table`（ボクシング）。
static WEAPON_BOXING: &[StaticRow] = &[
    (
        11,
        Some("ワン・ツー"),
        Some("２連続攻撃・２攻撃目盾受け、回避不可"),
    ),
    (22, Some("リバーブロー"), Some("麻痺効果「根性」５")),
    (
        33,
        Some("フリッカー"),
        Some("２連続攻撃・全て盾受け、カウンター不可"),
    ),
    (
        44,
        Some("コークスクリューブロー"),
        Some("防御力無視、ダメージ３倍"),
    ),
    (
        55,
        Some("レイ・ガン"),
        Some("全体攻撃、Ｂ・Ｄ、陽属性魔法攻撃"),
    ),
    (66, Some("ショットガンブロー"), Some("攻撃量１０倍")),
    (
        77,
        Some("ハートブレイクショット"),
        Some("２連続攻撃・１攻撃目防御力無視、ダメージ３倍・２撃目敵無防備"),
    ),
    (88, Some("デンプシーロール"), Some("３連続攻撃・全てＢ・Ｄ")),
    (
        99,
        Some("フラッシュピストンマッハパンチ"),
        Some("全体攻撃、Ｂ・Ｄ、気絶効果「根性」５"),
    ),
    (0, Some("右"), Some("防御力無視、ダメージ１０倍")),
];

/// Ruby `getWeaponTableBoxingCounter` の `table`（ボクシングカウンター）。
static WEAPON_BOXING_COUNTER: &[StaticRow] = &[
    (11, Some("ダッキングブロー"), Some("カウンター")),
    (
        22,
        Some("ジョルトカウンター"),
        Some("クロスカウンター、Ｂ・Ｄ"),
    ),
    (33, None, None),
    (44, None, None),
    (55, None, None),
    (66, None, None),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (
        0,
        Some("ノーガード戦法"),
        Some("攻撃の無効化、次ターン以降は自分の盾受け、回避不可、全ての攻撃にＢ・Ｄ"),
    ),
];

/// Ruby `getWeaponTableProWrestling` の `table`（プロレス）。
static WEAPON_PRO_WRESTLING: &[StaticRow] = &[
    (11, Some("ボディスラム"), Some("盾受け不可")),
    (22, Some("ドロップキック"), Some("Ｂ・Ｄ")),
    (33, Some("水車落とし"), Some("盾受け不可、成功度＋５")),
    (
        44,
        Some("ナックルアロー"),
        Some("Ｂ・Ｄ、麻痺効果「根性」５"),
    ),
    (55, Some("ワン・ツー・エルボー"), Some("２連続攻撃")),
    (66, Some("バックドロップ"), Some("盾受け不可、ダメージ２倍")),
    (
        77,
        Some("投げっ放しジャーマン"),
        Some("盾受け不可、防御力無視、成功度＋５"),
    ),
    (
        88,
        Some("パワーボム"),
        Some("盾受け不可、ダメージ２倍、Ｂ・Ｄ"),
    ),
    (
        99,
        Some("デスバレーボム"),
        Some("盾受け不可、防御力無視、ダメージ２倍、気絶効果「根性」５"),
    ),
    (
        0,
        Some("ジャックハマー"),
        Some("盾受け不可、防御力無視、ダメージ３倍、成功度＋１０"),
    ),
];

/// Ruby `getWeaponTableProWrestlingCounter` の `table`（プロレスカウンター）。
static WEAPON_PRO_WRESTLING_COUNTER: &[StaticRow] = &[
    (22, Some("パワースラム"), Some("カウンター")),
    (55, Some("アックスボンバー"), Some("カウンター、Ｂ・Ｄ")),
    (66, None, None),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (0, None, None),
];

/// Ruby `getWeaponTableStand` の `table`（幽波紋）。
static WEAPON_STAND: &[StaticRow] = &[
    (
        11,
        Some("SILER CHARIOT"),
        Some("攻撃量５倍、刺しタイプ攻撃"),
    ),
    (22, Some("TOWER OF GRAY"), Some("防御力無視")),
    (
        33,
        Some("DARK BLUE MOON"),
        Some("全体攻撃、攻撃量２倍、水属性斬りタイプ攻撃"),
    ),
    (
        44,
        Some("EMPEROR"),
        Some("回避不可、盾受け不可、カウンター不可、飛び道具攻撃"),
    ),
    (
        55,
        Some("MAGICIAN's RED"),
        Some("ダメージ２倍、Ｂ・Ｄ、火属性魔法攻撃"),
    ),
    (
        66,
        Some("DEATH 13"),
        Some("ダメージ０、全体攻撃、気絶効果「根性」５"),
    ),
    (
        77,
        Some("HIEROPHANT GREEN"),
        Some("全体攻撃、Ｂ・Ｄ、水属性攻撃"),
    ),
    (
        88,
        Some("VANILLA ICE CREAM"),
        Some("盾受け不可、カウンター不可、防御力無視、ダメージ３倍、Ｂ・Ｄ"),
    ),
    (99, Some("THE WORLD"), Some("５連続攻撃、全て敵無防備")),
    (0, Some("STAR PLATINUM"), Some("攻撃量１５倍、Ｂ・Ｄ")),
];

/// Ruby `getWeaponTableStandCounter` の `table`（幽波紋カウンター）。
static WEAPON_STAND_COUNTER: &[StaticRow] = &[
    (
        11,
        Some("ANUBIS"),
        Some("技のみカウンター、ダメージ（カウンターした回数の２乗）倍、斬りタイプ攻撃"),
    ),
    (22, None, None),
    (33, None, None),
    (44, None, None),
    (55, None, None),
    (
        66,
        Some("YELLOW TEMPERANE"),
        Some("魔法・飛び道具含めて全ての攻撃を無効化"),
    ),
    (77, None, None),
    (88, None, None),
    (99, None, None),
    (0, None, None),
];

/// Ruby `BCDice::GameSystem::ShinkuuGakuen`（ID: `ShinkuuGakuen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShinkuuGakuen;

impl GameSystem for ShinkuuGakuen {
    fn id(&self) -> &'static str {
        "ShinkuuGakuen"
    }

    fn name(&self) -> &'static str {
        "真空学園"
    }

    fn sort_key(&self) -> &'static str {
        "しんくうかくえん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
RLx：技能ベースｘで技能チェックのダイスロール
RLx>=y：この書式なら目標値 ｙ で判定結果出力
　例）RL10　　RL22>=50

・武器攻撃
（武器記号）（技能ベース値）
　例）SW10　BX30
武器を技能ベースでダイスロール。技発動までチェック。
武器記号は以下の通り
　SW：剣、LS：大剣、SS：小剣、SP：槍、
　AX：斧、CL：棍棒、BW：弓、MA：体術、
　BX：ボクシング、PR：プロレス、ST：幽波紋

・カウンター攻撃
カウンター技は武器記号の頭に「C」をつけるとロール可能。
　例）CSW10　CBX76
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "CRL", "CSW", "CLS", "CSS", "CSP", "CAX", "CCL", "CMA", "CBX", "CPR", "CST", "RL",
            "SW", "LS", "SS", "SP", "AX", "CL", "BW", "MA", "BX", "PR", "ST",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `ShinkuuGakuen#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
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
            .join("test/data/ShinkuuGakuen.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/ShinkuuGakuen.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/ShinkuuGakuen.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("ShinkuuGakuen.toml must parse");
        assert_eq!(
            data.tests.len(),
            45,
            "case count in test/data/ShinkuuGakuen.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "ShinkuuGakuen",
                "unexpected game system in ShinkuuGakuen.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("ShinkuuGakuen"), &tc.input, &mut src) {
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
                    "FAIL ShinkuuGakuen:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} ShinkuuGakuen cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
