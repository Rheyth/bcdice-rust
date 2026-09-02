//! P4で手書き移植した `lib/bcdice/game_system/WoW.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `WoW#roll_wow`（行為判定 `nWW12@s#f<=x`）
//! - `WoW#roll_gg` / `#roll_table`（ランダムギフトガチャ表 `GG` / `GGx`）
//! - `WoW#roll_fumble_table`（ファンブル表 `FT`）
//! - `TABLES`（`A`〜`H` と `FT`）

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::WoW`（ID: `WoW`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WoW;

impl GameSystem for WoW {
    fn id(&self) -> &'static str {
        "WoW"
    }

    fn name(&self) -> &'static str {
        "ワンダーオブワンダラー"
    }

    fn sort_key(&self) -> &'static str {
        "わんたあおふわんたらあ"
    }

    fn help_message(&self) -> &'static str {
        r"行為判定 nWW12@s#f<=x
n: ダイス数
@s = 大成功値（省略可：デフォルトは1）
#f = 大失敗値（省略可：デフォルトは12）
x = 目標値（省略可：デフォルトは6）
例）1WW12 5WW12<=6 6WW12@5#3<=7+1

ランダムギフトガチャ表 GG
ランダムギフトガチャ表（アルファベット指定） GGx 例）GGA GGB

ファンブル表 FT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d*WW12", "GG", "GGA", "GGB", "GGC", "GGD", "GGE", "GGF", "GGG", "GGH", "FT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `WoW#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if command == "GG" {
            return Ok(Some(SpecificCommandOutput::text(roll_gg(rng)?)));
        }

        if let Some(m) = gg_alphabet_pattern().captures(command) {
            let alphabet = m[1].chars().next().expect("one ASCII letter");
            return Ok(Some(SpecificCommandOutput::text(roll_table(
                alphabet, rng,
            )?)));
        }

        if command == "FT" {
            return Ok(Some(SpecificCommandOutput::text(roll_fumble_table(rng)?)));
        }

        roll_wow(command, rng)
    }
}

/// Ruby `TABLES['A']`（12項目）。
static TABLE_A: &[&str] = &[
    "演者の声",
    "言いくるめ",
    "誤魔化し",
    "代弁者",
    "腕利き弁護人",
    "魔性",
    "魔術",
    "魔法的物理",
    "誤り指摘",
    "専門知識",
    "理力増幅",
    "協力的な有識者",
];

/// Ruby `TABLES['B']`（12項目）。
static TABLE_B: &[&str] = &[
    "百科全書",
    "地道な下調べ",
    "思い…出した！",
    "目星",
    "ハッキング",
    "再考察",
    "迷探偵",
    "逆転の発想",
    "炯眼",
    "安楽椅子探偵",
    "密室トリック解明",
    "丁寧な処置",
];

/// Ruby `TABLES['C']`（12項目）。
static TABLE_C: &[&str] = &[
    "慈愛",
    "クイックヒール",
    "エリアヒール",
    "クリアランス",
    "俯瞰視点",
    "パターン化",
    "瞬時看破",
    "警鐘",
    "賢者の瞳",
    "千里眼",
    "危険感知",
    "リバーサル",
];

/// Ruby `TABLES['D']`（12項目）。
static TABLE_D: &[&str] = &[
    "転禍為福",
    "受け身",
    "九死に一生",
    "軽業",
    "バックドア",
    "着服",
    "闇に隠れる",
    "変装",
    "証拠隠滅",
    "サポート",
    "技師の指",
    "妨害",
];

/// Ruby `TABLES['E']`（12項目）。
static TABLE_E: &[&str] = &[
    "ゴッドハンド",
    "生存者の切り札",
    "狙撃",
    "プラチナ免許",
    "ドライバーズ・ハイ",
    "相乗り",
    "愛車／愛馬",
    "ビーストフレンズ",
    "ドゥ・ライブ",
    "カツアゲ",
    "マッドドッグ",
    "目の上の瘤",
];

/// Ruby `TABLES['F']`（12項目）。
static TABLE_F: &[&str] = &[
    "叱咤激励",
    "ふいに見せた優しさ",
    "スゴ味",
    "達人",
    "必殺技",
    "二刀流",
    "急所狙い",
    "ジャンプショット",
    "パルクール",
    "疾風怒濤",
    "スパート",
    "走為上",
];

/// Ruby `TABLES['G']`（12項目）。
static TABLE_G: &[&str] = &[
    "ヒット＆アウェイ",
    "ウーバー",
    "割れもの注意",
    "もしもの備え",
    "アブダクション",
    "追加機材",
    "自在配送",
    "不屈の精神",
    "防壁",
    "心頭滅却",
    "三時間しか寝てない",
    "βエンドルフィン",
];

/// Ruby `TABLES['H']`（12項目）。
static TABLE_H: &[&str] = &[
    "怒髪天",
    "頭の体操",
    "精神統一",
    "リトルラック",
    "いいね！",
    "幻視",
    "慎重性",
    "バレットストッパー",
    "褪せぬ想い",
    "アピール上手",
    "土俵際の魔術師",
    "真実の愛",
];

/// Ruby `TABLES['FT']`（12項目）。
static TABLE_FT: &[&str] = &[
    "何も起きなかった！　ラッキー（？）",
    "ランダムに武器または防具が外れる。該当箇所に何も装備していなければ1点のダメージ（軽減無効）を受ける。",
    "GMの指定したLOVEの【深度】が1増加する。誰かに対するLOVEを新規取得させても良い。",
    "GMの指定したハンドアウト1つの強度が［自身のソウルLV／2］増加する。",
    "1点のダメージ（軽減無効）を受ける。",
    "プレイス内のPCが所持している消耗品からGMが1つ指定し、破壊する。破壊したくない場合、かわりに自身のHPを最大値の1／3（切り捨て）減らす。",
    "不調強度［自身のソウルLV／2］のランダムな不調を受ける。",
    "ファンブル表を2回振る。この効果は判定につき1度までで、以降は1点のダメージ（軽減無効）を受ける。",
    "ランダムなLOVEの【深度】が1減少する。",
    "ランダムなLOVEの【エモ】が2増加する。",
    "トラブルが発生する。ランダムトラブル表を使用し、場にトラブルのハンドアウトを追加する。",
    "ランダムなギフト1つのMPが0になる。",
];

/// Ruby `TABLES` のうちアルファベット指定で引く分（`'A'`〜`'H'`）。
static ALPHABET_TABLES: &[(char, &[&str])] = &[
    ('A', TABLE_A),
    ('B', TABLE_B),
    ('C', TABLE_C),
    ('D', TABLE_D),
    ('E', TABLE_E),
    ('F', TABLE_F),
    ('G', TABLE_G),
    ('H', TABLE_H),
];

/// Ruby `/^GG([A-H])$/`。
fn gg_alphabet_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^GG([A-H])$").expect("valid regex"))
}

/// Ruby `/^(\d+)WW12(?:@(\d+))?(?:#(\d+))?(?:<=(\d+))?$/`。
///
/// Rubyの `\d` はASCII限定なので `[0-9]` に置き換える（Rustの `regex` は既定でUnicode）。
fn wow_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([0-9]+)WW12(?:@([0-9]+))?(?:#([0-9]+))?(?:<=([0-9]+))?$")
            .expect("valid regex")
    })
}

/// Ruby `String#to_i`。`i64` に収まらない指定は `i64::MAX`に飽和。
fn to_i_saturating(text: &str) -> i64 {
    str_helpers::to_i_max(text)
}

/// Ruby `table[index]`（範囲外は `nil` ＝ 文字列補間では空文字列）。
fn table_item(table: &[&'static str], number: i64) -> &'static str {
    usize::try_from(number - 1)
        .ok()
        .and_then(|i| table.get(i))
        .copied()
        .unwrap_or("")
}

/// Ruby `TABLES[alphabet]`。
fn alphabet_table(alphabet: char) -> &'static [&'static str] {
    ALPHABET_TABLES
        .iter()
        .find(|(key, _)| *key == alphabet)
        .map_or(&[][..], |(_, table)| *table)
}

/// Ruby `WoW#roll_gg`（ランダムギフトガチャ表）。
fn roll_gg(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice_results = rng.roll_barabara(2, 12)?;
    let first_roll = dice_results[0];
    let second_roll = dice_results[1];

    if first_roll >= 9 {
        return Ok("GG ＞ 自由（アルファベットを決めてGGXを振る）".to_owned());
    }

    // Ruby: (64 + first_roll).chr（1..8 なら 'A'..'H'）
    let alphabet = char::from(64u8 + first_roll as u8);
    let table = alphabet_table(alphabet);
    Ok(format!(
        "ランダムギフトガチャ {alphabet}-{second_roll} ＞ {}",
        table_item(table, second_roll)
    ))
}

/// Ruby `WoW#roll_table`（アルファベット指定のランダムギフトガチャ表）。
fn roll_table(alphabet: char, rng: &mut Randomizer) -> Result<String, EvalError> {
    let table = alphabet_table(alphabet);
    let dice_result = rng.roll_once(12)?;
    Ok(format!(
        "ランダムギフトガチャ {alphabet}-{dice_result} ＞ {}",
        table_item(table, dice_result)
    ))
}

/// Ruby `WoW#roll_fumble_table`（ファンブル表）。
fn roll_fumble_table(rng: &mut Randomizer) -> Result<String, EvalError> {
    let dice_result = rng.roll_once(12)?;
    Ok(format!(
        "FT({dice_result}) ＞ {}",
        table_item(TABLE_FT, dice_result)
    ))
}

/// Ruby `WoW#roll_wow`（行為判定）。
fn roll_wow(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(m) = wow_pattern().captures(command) else {
        return Ok(None);
    };

    // 振るダイスの数
    let num_dice = to_i_saturating(&m[1]);
    // 大成功の値（デフォルトは1）
    let critical_success_value = m.get(2).map_or(1, |g| to_i_saturating(g.as_str()));
    // 大失敗の値（デフォルトは12）
    let critical_fail_value = m.get(3).map_or(12, |g| to_i_saturating(g.as_str()));
    // 成功の閾値（デフォルトは6）
    let success_threshold = m.get(4).map_or(6, |g| to_i_saturating(g.as_str()));

    // Ruby: 目標値が省略された場合だけコマンドを組み直す。
    //       `@s` / `#f` は復元されないので `4WW12@5` は `(4WW12<=6)` と表示される
    //       （原典どおりの挙動）。
    let command_with_defaults = if m.get(4).is_none() {
        format!("{}WW12<={success_threshold}", &m[1])
    } else {
        command.to_owned()
    };

    // ダイスを振る
    let dice_results = rng.roll_barabara(num_dice, 12)?;

    // 出目を分類
    let mut critical_success = dice_results
        .iter()
        .filter(|r| **r <= critical_success_value)
        .count() as i64;
    let mut critical_fail = dice_results
        .iter()
        .filter(|r| **r >= critical_fail_value)
        .count() as i64;
    let normal_success = dice_results
        .iter()
        .filter(|r| {
            **r > critical_success_value && **r <= success_threshold && **r < critical_fail_value
        })
        .count() as i64;

    let critical_success_first = critical_success;
    let critical_fail_first = critical_fail;

    // 大成功と大失敗の相殺
    let offset = critical_success.min(critical_fail);
    critical_success -= offset;
    critical_fail -= offset;

    // 成功数とファンブルの判定
    let successes = normal_success + critical_success * 2;
    let is_fumble = critical_fail > 0;

    let dice_text = dice_results
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let fumble_text = if is_fumble {
        " ＞ ファンブル！"
    } else {
        ""
    };
    let text = format!(
        "({command_with_defaults}) ＞ [{dice_text}] ＞ 成功数{successes}（大成功{critical_success_first}個、大失敗{critical_fail_first}個）{fumble_text}"
    );

    // Ruby: BCDice::Result.new.tap { ... }（4フラグを個別に立てる）
    Ok(Some(SpecificCommandOutput::result(EvalResult {
        critical: critical_success > 0,
        fumble: is_fumble,
        // 成功数が0より大きく、ファンブルがない場合に成功
        success: successes > 0 && !is_fumble,
        // 成功数が0、またはファンブルがある場合に失敗
        failure: successes == 0 || is_fumble,
        ..EvalResult::with_text(text)
    })))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases("WoW", "WoW.toml", 19, &[(14, 3)]);
    }
}
