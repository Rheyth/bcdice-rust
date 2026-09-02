//! P4で手書き移植した `lib/bcdice/game_system/KizunaBullet.rb`
//! （表は `lib/bcdice/game_system/kizuna_bullet/tables.rb`）。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `KizunaBullet#roll_max`（`nDM`）/ `#roll_investigate`（`nIN`）/
//!   `#roll_sedative`（`SEn`）/ `#roll_solve`（`nSO`）
//! - `KizunaBullet::TABLES` と `Base#roll_tables`（各種表）
//!
//! 表と定型文は Ruby が `I18n` から組み立てる（`Table.from_i18n` / `D66Table.from_i18n` /
//! `translate`）。ここでは `ja_jp` ロケールの値を `i18n/KizunaBullet/ja_jp.yml` から
//! 写して `static` に展開した。`KizunaBullet_Korean` が `ko_kr` の値を差し替えられるよう、
//! 判定と表引きは [`SystemTables`] を受け取る関数に切り出してある。

use crate::command_parser::Parser;
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{dice_text, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// `TABLES` の値。
///
/// Ruby の `RollTwiceRandomizerTable` / `Roll4TimesRandomizerTable` は
/// 複数の表を順に振って `"\n"` で連結するだけなので、表の並びだけを持たせた。
/// （`roll` の戻り値が `String` で `RollResult` ではないため、
/// [`RollableTable`] としては表現できない。）
pub(crate) enum TableRef {
    /// 単独の表
    Single(&'static dyn RollableTable),
    /// 順に振って改行で連結する表
    Multi(&'static [&'static dyn RollableTable]),
}

/// 1ロケール分の表と定型文。`KizunaBullet` と `KizunaBullet_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）
    pub(crate) tables: &'static [(&'static str, TableRef)],
    /// i18n `KizunaBullet.INVESTIGATE.success`
    pub(crate) investigate_success: &'static str,
    /// i18n `KizunaBullet.INVESTIGATE.failure`
    pub(crate) investigate_failure: &'static str,
    /// i18n `KizunaBullet.INVESTIGATE.partnerHelp`
    pub(crate) investigate_partner_help: &'static str,
    /// i18n `KizunaBullet.INVESTIGATE.fumble`
    pub(crate) investigate_fumble: &'static str,
    /// i18n `KizunaBullet.SEDATIVE.burst`
    pub(crate) sedative_burst: &'static str,
    /// i18n `KizunaBullet.SEDATIVE.alive`
    pub(crate) sedative_alive: &'static str,
    /// i18n `KizunaBullet.SEDATIVE.success`
    pub(crate) sedative_success: &'static str,
    /// i18n `KizunaBullet.SEDATIVE.failure`（`%{check}` を置換する）
    pub(crate) sedative_failure: &'static str,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `KizunaBullet#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = roll_max(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_investigate(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_sedative(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    if let Some(result) = roll_solve(command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    Ok(roll_tables(sys, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `Base#roll_tables(command, tables)`。
/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = sys.tables.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };

    match table {
        TableRef::Single(table) => Ok(Some(table.roll(rng)?.to_string())),
        TableRef::Multi(tables) => {
            // Ruby `RollTwiceRandomizerTable#roll` / `Roll4TimesRandomizerTable#roll`
            let mut results = Vec::with_capacity(tables.len());
            for table in *tables {
                results.push(table.roll(rng)?.to_string());
            }
            Ok(Some(results.join("\n")))
        }
    }
}

/// Ruby `KizunaBullet#roll_max`（最大値 `nDM`）。
fn roll_max(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["DM"], RoundType::Ceil).has_prefix_number();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `has_prefix_number` なのでパースが通れば必ず入っている
    let times = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let dice_list = rng.roll_barabara(times, 6)?;
    let max = dice_list.iter().max().copied().unwrap_or(0);

    Ok(Some(EvalResult::with_text(format!(
        "{command} ＞ [{}] ＞ {max}",
        dice_text::join_dice(&dice_list)
    ))))
}

/// Ruby `KizunaBullet#roll_investigate`（調査判定 `nIN`）。
fn roll_investigate(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["IN"], RoundType::Ceil).has_prefix_number();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let mut texts: Vec<&str> = Vec::new();
    let mut is_success = false;
    let mut is_fumble = false;

    let times = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let dice_list = rng.roll_barabara(times, 6)?;
    let max = dice_list.iter().max().copied().unwrap_or(0);

    if max >= 5 {
        // 5以上の出目があった場合
        is_success = true;
        texts.push(sys.investigate_success);
    } else if max >= 3 {
        // 3以上の出目があった場合。［パートナーのヘルプ］で成功できる
        texts.push(sys.investigate_failure);
        texts.push(sys.investigate_partner_help);
    } else {
        is_fumble = true;
        texts.push(sys.investigate_failure);
        texts.push(sys.investigate_fumble);
    }

    let mut result = EvalResult::with_text(format!(
        "{command} ＞ [{}] ＞ {}",
        dice_text::join_dice(&dice_list),
        texts.concat()
    ));
    // Ruby: `r.condition = is_success` の後に `r.fumble = is_fumble`
    result.set_condition(is_success);
    result.fumble = is_fumble;

    Ok(Some(result))
}

/// Ruby `KizunaBullet#roll_sedative`（鎮静判定 `SEn`）。
fn roll_sedative(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["SE"], RoundType::Ceil).has_suffix_number();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let target = parsed
        .suffix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let mut is_success = false;

    // Ruby: 合計値は［晶滅］/［生存］の判定より前に必ず振る
    let sum = rng.roll_sum(2, 6)?;

    let text = if target > 12 {
        // すべての［キズナ］が［ヒビワレ］状態
        sys.sedative_burst.to_owned()
    } else if target < 6 {
        sys.sedative_alive.to_owned()
    } else if sum > target {
        is_success = true;
        sys.sedative_success.to_owned()
    } else {
        // ［強制鎮静］に必要な［キズナ］のチェック数
        let dif = target - sum;
        // Ruby `Integer#/`（dif は非負なので床除算と同じ）
        let check = dif / 2 + 1;
        sys.sedative_failure.replace("%{check}", &check.to_string())
    };

    let mut result = EvalResult::with_text(format!("{command} ＞ {sum} ＞ {text}"));
    result.set_condition(is_success);

    Ok(Some(result))
}

/// Ruby `KizunaBullet#roll_solve`（解決 `nSO`）。
fn roll_solve(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let parser = Parser::new(&["SO"], RoundType::Ceil).enable_prefix_number();
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // Ruby: `parsed.prefix_number.to_i + 2`（未指定なら `nil.to_i` の 0）
    let sum = rng.roll_sum(
        parsed
            .prefix_number
            .as_ref()
            .map(crate::randomizer::sat_i64)
            .unwrap_or(0)
            + 2,
        6,
    )?;

    Ok(Some(EvalResult::with_text(format!("{command} ＞ {sum}"))))
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの表と定型文
// ---------------------------------------------------------------------------

/// i18n `KizunaBullet.table.OP`（ja_jp）。
pub(crate) static JA_TABLE_OP: Table = Table::from_dice(
    "日常表・場所",
    1,
    6,
    &[
        "ケージ：ハウンドの私室にオーナーがお邪魔している。",
        "公園：緑地公園、運動公園、あるいは小さな広場など。",
        "病院：組織の管理下にある病院、あるいは医務室など。",
        "オーナーの家：オーナーの家に、ハウンドがお邪魔している。",
        "訓練場：武道場、ジム、体育館、あるいは射撃場など。",
        "資料室：組織の資料室や書庫、証拠品の保管庫など。",
    ],
);

/// i18n `KizunaBullet.table.OC`（ja_jp）。
pub(crate) static JA_TABLE_OC: Table = Table::from_dice(
    "日常表・内容",
    1,
    6,
    &[
        "仕事／勉強：片方の仕事や勉強を、もう片方が手伝っている。",
        "ゲーム：ふたりでゲームを楽しんでいる。",
        "趣味：片方の趣味にもう片方がつきあっている。",
        "食事：食事をとっている。もしくは料理をしている。",
        "掃除／整頓：ふたりで、その場の掃除や整頓を行なっている。",
        "訓練／手入れ：戦闘訓練や、武器の手入れなどを行なっている。",
    ],
);

/// i18n `KizunaBullet.table.OWP`（ja_jp）。
pub(crate) static JA_TABLE_OWP: Table = Table::from_dice(
    "日常表（仕事）・場所",
    1,
    6,
    &[
        "オフィス：多くの同僚が、一緒に働いているオフィス。",
        "公園：ひろびろとした公園や庭園。",
        "図書館：多くの本や資料が並ぶ図書館。",
        "鉄道駅：多くの人が利用する鉄道の駅",
        "車内：自動車や鉄道など、車輌の中。",
        "病院：医院や病院、あるいは組織の治療室。",
    ],
);

/// i18n `KizunaBullet.table.OWC`（ja_jp）。
pub(crate) static JA_TABLE_OWC: Table = Table::from_dice(
    "日常表（仕事）・内容",
    1,
    6,
    &[
        "休日の相談：次の休みの予定について話し合っている。",
        "ひと休み：仕事を中断し、休憩をとっている。",
        "食事：食事やティーブレイクをとっている。",
        "仕事ぶり：お互いの仕事ぶりについて話し合っている。",
        "報告書作り：上司に提出する報告書を作っている。",
        "手伝い：同僚の手伝いで尋問や交渉などを行なっている。",
    ],
);

/// i18n `KizunaBullet.table.OHP`（ja_jp）。
pub(crate) static JA_TABLE_OHP: Table = Table::from_dice(
    "日常表（休暇）・場所",
    1,
    6,
    &[
        "レストラン：ちょっといい食事どころ。和洋中はお好みで。",
        "街頭：多くの人が行き交う街の中。",
        "水族館：魚や水棲生物が展示されている水族館。",
        "動物園：たくさんの動物が飼育されている動物園。",
        "服飾店：服やアクセサリーを扱っているお店。",
        "ゲーセン：最新機種やレトロゲームまで並ぶゲームセンター。",
    ],
);

/// i18n `KizunaBullet.table.OHC`（ja_jp）。
pub(crate) static JA_TABLE_OHC: Table = Table::from_dice(
    "日常表（休暇）・内容",
    1,
    6,
    &[
        "グルメ：美味しいものを食べている。あるいは順番待ちしている。",
        "次の場所：次はどこに行こうか、何をしようか話し合っている。",
        "プレゼント：パートナーや大切な人へのプレゼントを選んでいる。",
        "今日の服装：今日の服装について、お互いにどう思うかを話している。",
        "季節の行事：クリスマスやハロウィンなど季節の行事を楽しんでいる。",
        "仕事の話：お出かけ中だというのに、仕事について話している。",
    ],
);

/// i18n `KizunaBullet.table.OTP`（ja_jp）。
pub(crate) static JA_TABLE_OTP: Table = Table::from_dice(
    "日常表（出張）・場所",
    1,
    6,
    &[
        "ガソスタ：車に給油したり、洗車したりできるガソリンスタンド。",
        "道の駅：ショップやレストランが併設された道の駅。",
        "バス停：地方のバス停、あるいは高速バスの待機場所など。",
        "林道：山間の静かな林道。人の姿はほとんどない。",
        "車内：自動車や鉄道など、車輌の中。",
        "獣道：人の手で整備されていない、自然の中の獣道。",
    ],
);

/// i18n `KizunaBullet.table.OTC`（ja_jp）。
pub(crate) static JA_TABLE_OTC: Table = Table::from_dice(
    "日常表（出張）・内容",
    1,
    6,
    &[
        "お土産：お土産を選んでいるか、何をしようか話している。",
        "休憩：腰をおちつけて、ひと休みしている。",
        "忘れ物：忘れ物をしてきたことを、相棒に打ち明けている。",
        "グルメ：せっかくなので、美味しいものでも食べようとしている。",
        "仕事の話：せっかくの旅先だが、仕事について話している。",
        "ここはどこ：迷ったらしい……。進む方向について相談している。",
    ],
);

/// i18n `KizunaBullet.table.TT`（ja_jp）。
pub(crate) static JA_TABLE_TT: D66Table = D66Table::new(
    "ターンテーマ表",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("感謝：パートナーへ感謝の言葉を送ろう。円滑な関係には、こういうことも必要だ。")),
        (12, TableItem::Text("協力：パートナーと力を合わせる必要がある。どんな役割分担がよいだろう？")),
        (13, TableItem::Text("思い出作り：調査のついでに、パートナーと思い出を作ろう。いい思い出になるかな？")),
        (14, TableItem::Text("終わったら……：この仕事が終わったら、なにをしようか。パートナーと相談してみよう。")),
        (15, TableItem::Text("相手の調子：パートナーの調子はどうだろうか？　少し注意して、相手を観察してみよう。")),
        (16, TableItem::Text("気になっていたこと：パートナーについて、前から気になっていたこと。この機会に、尋ねてみよう。")),
        (21, TableItem::Text("感謝：パートナーへ感謝の言葉を送ろう。円滑な関係には、こういうことも必要だ。")),
        (22, TableItem::Text("協力：パートナーと力を合わせる必要がある。どんな役割分担がよいだろう？")),
        (23, TableItem::Text("思い出作り：調査のついでに、パートナーと思い出を作ろう。いい思い出になるかな？")),
        (24, TableItem::Text("終わったら……：この仕事が終わったら、なにをしようか。パートナーと相談してみよう。")),
        (25, TableItem::Text("相手の調子：パートナーの調子はどうだろうか？　少し注意して、相手を観察してみよう。")),
        (26, TableItem::Text("気になっていたこと：パートナーについて、前から気になっていたこと。この機会に、尋ねてみよう。")),
        (31, TableItem::Text("感謝：パートナーへ感謝の言葉を送ろう。円滑な関係には、こういうことも必要だ。")),
        (32, TableItem::Text("協力：パートナーと力を合わせる必要がある。どんな役割分担がよいだろう？")),
        (33, TableItem::Text("思い出作り：調査のついでに、パートナーと思い出を作ろう。いい思い出になるかな？")),
        (34, TableItem::Text("終わったら……：この仕事が終わったら、なにをしようか。パートナーと相談してみよう。")),
        (35, TableItem::Text("相手の調子：パートナーの調子はどうだろうか？　少し注意して、相手を観察してみよう。")),
        (36, TableItem::Text("気になっていたこと：パートナーについて、前から気になっていたこと。この機会に、尋ねてみよう。")),
        (41, TableItem::Text("エンジョイ！：どんな時でも、楽しむことが大切だ！　調査も大事だが、楽しいことを探そう。")),
        (42, TableItem::Text("言えなかったこと：いつか言おうと思っていたこと。この機会に、打ち明けられるだろうか。")),
        (43, TableItem::Text("新しい挑戦：苦手なこと、未経験のことでも、挑戦が大切だ。パートナーの力を借りるのもいい。")),
        (44, TableItem::Text("サプライズ：パートナーには秘密で、何かを用意してみよう。驚く顔が見られるかもしれない。")),
        (45, TableItem::Text("まだ知らない一面：パートナーの、まだ知らない一面が見られるかも。相手の様子を観察してみよう。")),
        (46, TableItem::Text("ほしかった物：調査のついでに、ほしかった物を探してみよう。意外な場所で手に入るかも。")),
        (51, TableItem::Text("エンジョイ！：どんな時でも、楽しむことが大切だ！　調査も大事だが、楽しいことを探そう。")),
        (52, TableItem::Text("言えなかったこと：いつか言おうと思っていたこと。この機会に、打ち明けられるだろうか。")),
        (53, TableItem::Text("新しい挑戦：苦手なこと、未経験のことでも、挑戦が大切だ。パートナーの力を借りるのもいい。")),
        (54, TableItem::Text("サプライズ：パートナーには秘密で、何かを用意してみよう。驚く顔が見られるかもしれない。")),
        (55, TableItem::Text("まだ知らない一面：パートナーの、まだ知らない一面が見られるかも。相手の様子を観察してみよう。")),
        (56, TableItem::Text("ほしかった物：調査のついでに、ほしかった物を探してみよう。意外な場所で手に入るかも。")),
        (61, TableItem::Text("エンジョイ！：どんな時でも、楽しむことが大切だ！　調査も大事だが、楽しいことを探そう。")),
        (62, TableItem::Text("言えなかったこと：いつか言おうと思っていたこと。この機会に、打ち明けられるだろうか。")),
        (63, TableItem::Text("新しい挑戦：苦手なこと、未経験のことでも、挑戦が大切だ。パートナーの力を借りるのもいい。")),
        (64, TableItem::Text("サプライズ：パートナーには秘密で、何かを用意してみよう。驚く顔が見られるかもしれない。")),
        (65, TableItem::Text("まだ知らない一面：パートナーの、まだ知らない一面が見られるかも。相手の様子を観察してみよう。")),
        (66, TableItem::Text("ほしかった物：調査のついでに、ほしかった物を探してみよう。意外な場所で手に入るかも。")),
    ],
);

/// i18n `KizunaBullet.table.TTI`（ja_jp）。
pub(crate) static JA_TABLE_TTI: D66Table = D66Table::new(
    "ターンテーマ表・親密",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("次の約束：この事件が終わったら何をするか、パートナーと相談してみよう。")),
        (12, TableItem::Text("頼りどき：パートナーを頼ってみよう。ちょっといいとこ見てみたい！")),
        (13, TableItem::Text("世話焼き：あれやこれや、パートナーの世話を焼いてみよう。面白い反応が見られるかも？")),
        (14, TableItem::Text("コンビネーション：互いの力を合わせて事件に挑もう。今こそコンビネーションの見せ所だ！")),
        (15, TableItem::Text("成長：自分がどれだけ成長したか、パートナーに見せつけるいい機会かも知れない。")),
        (16, TableItem::Text("エンジョイ：仕事はきっちりやるとして、せっかくだから、いまの状況を精一杯楽しもう。")),
        (21, TableItem::Text("次の約束：この事件が終わったら何をするか、パートナーと相談してみよう。")),
        (22, TableItem::Text("頼りどき：パートナーを頼ってみよう。ちょっといいとこ見てみたい！")),
        (23, TableItem::Text("世話焼き：あれやこれや、パートナーの世話を焼いてみよう。面白い反応が見られるかも？")),
        (24, TableItem::Text("コンビネーション：互いの力を合わせて事件に挑もう。今こそコンビネーションの見せ所だ！")),
        (25, TableItem::Text("成長：自分がどれだけ成長したか、パートナーに見せつけるいい機会かも知れない。")),
        (26, TableItem::Text("エンジョイ：仕事はきっちりやるとして、せっかくだから、いまの状況を精一杯楽しもう。")),
        (31, TableItem::Text("次の約束：この事件が終わったら何をするか、パートナーと相談してみよう。")),
        (32, TableItem::Text("頼りどき：パートナーを頼ってみよう。ちょっといいとこ見てみたい！")),
        (33, TableItem::Text("世話焼き：あれやこれや、パートナーの世話を焼いてみよう。面白い反応が見られるかも？")),
        (34, TableItem::Text("コンビネーション：互いの力を合わせて事件に挑もう。今こそコンビネーションの見せ所だ！")),
        (35, TableItem::Text("成長：自分がどれだけ成長したか、パートナーに見せつけるいい機会かも知れない。")),
        (36, TableItem::Text("エンジョイ：仕事はきっちりやるとして、せっかくだから、いまの状況を精一杯楽しもう。")),
        (41, TableItem::Text("甘えどき：たまにはパートナーに甘えてみるのもいいかもしれない。どんな顔をするかな？")),
        (42, TableItem::Text("感謝：いつも世話になっているパートナーに、感謝の気持ちを伝えよう。")),
        (43, TableItem::Text("思い出作り：せっかくだし、なにか思い出になるようなことに挑戦してみよう。")),
        (44, TableItem::Text("気遣い：疲れてはいないだろうか？　パートナーを気遣ってみよう。")),
        (45, TableItem::Text("約束：なにかひとつ、約束をしよう。生き残って、それを果たすのだ。")),
        (46, TableItem::Text("悩みごと：この機会に、悩みごとを打ち明けるのもいいかもしれない。")),
        (51, TableItem::Text("甘えどき：たまにはパートナーに甘えてみるのもいいかもしれない。どんな顔をするかな？")),
        (52, TableItem::Text("感謝：いつも世話になっているパートナーに、感謝の気持ちを伝えよう。")),
        (53, TableItem::Text("思い出作り：せっかくだし、なにか思い出になるようなことに挑戦してみよう。")),
        (54, TableItem::Text("気遣い：疲れてはいないだろうか？　パートナーを気遣ってみよう。")),
        (55, TableItem::Text("約束：なにかひとつ、約束をしよう。生き残って、それを果たすのだ。")),
        (56, TableItem::Text("悩みごと：この機会に、悩みごとを打ち明けるのもいいかもしれない。")),
        (61, TableItem::Text("甘えどき：たまにはパートナーに甘えてみるのもいいかもしれない。どんな顔をするかな？")),
        (62, TableItem::Text("感謝：いつも世話になっているパートナーに、感謝の気持ちを伝えよう。")),
        (63, TableItem::Text("思い出作り：せっかくだし、なにか思い出になるようなことに挑戦してみよう。")),
        (64, TableItem::Text("気遣い：疲れてはいないだろうか？　パートナーを気遣ってみよう。")),
        (65, TableItem::Text("約束：なにかひとつ、約束をしよう。生き残って、それを果たすのだ。")),
        (66, TableItem::Text("悩みごと：この機会に、悩みごとを打ち明けるのもいいかもしれない。")),
    ],
);

/// i18n `KizunaBullet.table.TTC`（ja_jp）。
pub(crate) static JA_TABLE_TTC: D66Table = D66Table::new(
    "ターンテーマ表・クール",
    D66SortType::NoSort,
    &[
        (
            11,
            TableItem::Text("優先順位：この事件を解決するための優先順位をパートナーと整理しよう。"),
        ),
        (
            12,
            TableItem::Text(
                "そういえば：前から気になっていることがあった。せっかくだから、尋ねてみよう。",
            ),
        ),
        (
            13,
            TableItem::Text("観察：あらためてパートナーを観察してみよう。なにか発見があるかも。"),
        ),
        (
            14,
            TableItem::Text(
                "トライアル：パートナーと新しい調査方法や道具を試してみる、いい機会だ。",
            ),
        ),
        (
            15,
            TableItem::Text(
                "張り合い：どちらが捜査に貢献できるだろうか。負けたくない気分の日もある。",
            ),
        ),
        (
            16,
            TableItem::Text(
                "共通点：この機会に互いの共通点を探してみるのも悪くない。意外なものがあるかも。",
            ),
        ),
        (
            21,
            TableItem::Text("優先順位：この事件を解決するための優先順位をパートナーと整理しよう。"),
        ),
        (
            22,
            TableItem::Text(
                "そういえば：前から気になっていることがあった。せっかくだから、尋ねてみよう。",
            ),
        ),
        (
            23,
            TableItem::Text("観察：あらためてパートナーを観察してみよう。なにか発見があるかも。"),
        ),
        (
            24,
            TableItem::Text(
                "トライアル：パートナーと新しい調査方法や道具を試してみる、いい機会だ。",
            ),
        ),
        (
            25,
            TableItem::Text(
                "張り合い：どちらが捜査に貢献できるだろうか。負けたくない気分の日もある。",
            ),
        ),
        (
            26,
            TableItem::Text(
                "共通点：この機会に互いの共通点を探してみるのも悪くない。意外なものがあるかも。",
            ),
        ),
        (
            31,
            TableItem::Text("優先順位：この事件を解決するための優先順位をパートナーと整理しよう。"),
        ),
        (
            32,
            TableItem::Text(
                "そういえば：前から気になっていることがあった。せっかくだから、尋ねてみよう。",
            ),
        ),
        (
            33,
            TableItem::Text("観察：あらためてパートナーを観察してみよう。なにか発見があるかも。"),
        ),
        (
            34,
            TableItem::Text(
                "トライアル：パートナーと新しい調査方法や道具を試してみる、いい機会だ。",
            ),
        ),
        (
            35,
            TableItem::Text(
                "張り合い：どちらが捜査に貢献できるだろうか。負けたくない気分の日もある。",
            ),
        ),
        (
            36,
            TableItem::Text(
                "共通点：この機会に互いの共通点を探してみるのも悪くない。意外なものがあるかも。",
            ),
        ),
        (
            41,
            TableItem::Text("感謝：たまには……感謝を述べてみるのも悪くない。そう、たまには。"),
        ),
        (
            42,
            TableItem::Text("相違点：相手と自分の違う点。しっかり確認しておいて、損はないだろう。"),
        ),
        (
            43,
            TableItem::Text(
                "好きなもの：パートナーの好きなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            44,
            TableItem::Text("腹を割って：この機会に、前から言いたかったことを話してみよう。"),
        ),
        (
            45,
            TableItem::Text(
                "嫌いなもの：パートナーの嫌いなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            46,
            TableItem::Text("仲直り：ケンカをしてしまった。仲直りのきっかけを見つけないと……。"),
        ),
        (
            51,
            TableItem::Text("感謝：たまには……感謝を述べてみるのも悪くない。そう、たまには。"),
        ),
        (
            52,
            TableItem::Text("相違点：相手と自分の違う点。しっかり確認しておいて、損はないだろう。"),
        ),
        (
            53,
            TableItem::Text(
                "好きなもの：パートナーの好きなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            54,
            TableItem::Text("腹を割って：この機会に、前から言いたかったことを話してみよう。"),
        ),
        (
            55,
            TableItem::Text(
                "嫌いなもの：パートナーの嫌いなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            56,
            TableItem::Text("仲直り：ケンカをしてしまった。仲直りのきっかけを見つけないと……。"),
        ),
        (
            61,
            TableItem::Text("感謝：たまには……感謝を述べてみるのも悪くない。そう、たまには。"),
        ),
        (
            62,
            TableItem::Text("相違点：相手と自分の違う点。しっかり確認しておいて、損はないだろう。"),
        ),
        (
            63,
            TableItem::Text(
                "好きなもの：パートナーの好きなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            64,
            TableItem::Text("腹を割って：この機会に、前から言いたかったことを話してみよう。"),
        ),
        (
            65,
            TableItem::Text(
                "嫌いなもの：パートナーの嫌いなものは何だろうか。まだ知らないものがあるのかも。",
            ),
        ),
        (
            66,
            TableItem::Text("仲直り：ケンカをしてしまった。仲直りのきっかけを見つけないと……。"),
        ),
    ],
);

/// i18n `KizunaBullet.table.TTH`（ja_jp）。
pub(crate) static JA_TABLE_TTH: D66Table = D66Table::new(
    "ターンテーマ表・主従",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("お願いごと：従者、あるいは主人にしてほしいことを言ってみよう。叶うかどうかは別として。")),
        (12, TableItem::Text("レッスン：この調査をレッスンとして利用しよう。行儀？　勉強？　それとも戦い？")),
        (13, TableItem::Text("サプライズ：従者を、あるいは主人を驚かせてみよう。どんな方法がいいだろうか。")),
        (14, TableItem::Text("次の予定：この戦いが終わったら、何をしようか。次の予定を決めておこう。")),
        (15, TableItem::Text("弱点探し：主人の弱点を探そう。あるいは従者の弱点を見つけるのもいい。")),
        (16, TableItem::Text("感謝：ときには感謝の言葉も悪くない。いつかは言えなくなるのだから。")),
        (21, TableItem::Text("お願いごと：従者、あるいは主人にしてほしいことを言ってみよう。叶うかどうかは別として。")),
        (22, TableItem::Text("レッスン：この調査をレッスンとして利用しよう。行儀？　勉強？　それとも戦い？")),
        (23, TableItem::Text("サプライズ：従者を、あるいは主人を驚かせてみよう。どんな方法がいいだろうか。")),
        (24, TableItem::Text("次の予定：この戦いが終わったら、何をしようか。次の予定を決めておこう。")),
        (25, TableItem::Text("弱点探し：主人の弱点を探そう。あるいは従者の弱点を見つけるのもいい。")),
        (26, TableItem::Text("感謝：ときには感謝の言葉も悪くない。いつかは言えなくなるのだから。")),
        (31, TableItem::Text("お願いごと：従者、あるいは主人にしてほしいことを言ってみよう。叶うかどうかは別として。")),
        (32, TableItem::Text("レッスン：この調査をレッスンとして利用しよう。行儀？　勉強？　それとも戦い？")),
        (33, TableItem::Text("サプライズ：従者を、あるいは主人を驚かせてみよう。どんな方法がいいだろうか。")),
        (34, TableItem::Text("次の予定：この戦いが終わったら、何をしようか。次の予定を決めておこう。")),
        (35, TableItem::Text("弱点探し：主人の弱点を探そう。あるいは従者の弱点を見つけるのもいい。")),
        (36, TableItem::Text("感謝：ときには感謝の言葉も悪くない。いつかは言えなくなるのだから。")),
        (41, TableItem::Text("心配ごと：この機会に心配ごとを打ち明けてみよう。いい解決策が見つかるかも？")),
        (42, TableItem::Text("秘密：実は秘密にしていたことが……。打ち明ける機会を見つけよう。")),
        (43, TableItem::Text("探し物：調査のついでに、前から気になっていたものを探してみよう。")),
        (44, TableItem::Text("意外な一面：主従といえど、知らない一面もある。それを知る機会かもしれない。")),
        (45, TableItem::Text("不満：いい機会なので、言わせていただきたいことがある！")),
        (46, TableItem::Text("世話焼き：いつものように主の世話を焼こう。あるいは逆というのも面白い。")),
        (51, TableItem::Text("心配ごと：この機会に心配ごとを打ち明けてみよう。いい解決策が見つかるかも？")),
        (52, TableItem::Text("秘密：実は秘密にしていたことが……。打ち明ける機会を見つけよう。")),
        (53, TableItem::Text("探し物：調査のついでに、前から気になっていたものを探してみよう。")),
        (54, TableItem::Text("意外な一面：主従といえど、知らない一面もある。それを知る機会かもしれない。")),
        (55, TableItem::Text("不満：いい機会なので、言わせていただきたいことがある！")),
        (56, TableItem::Text("世話焼き：いつものように主の世話を焼こう。あるいは逆というのも面白い。")),
        (61, TableItem::Text("心配ごと：この機会に心配ごとを打ち明けてみよう。いい解決策が見つかるかも？")),
        (62, TableItem::Text("秘密：実は秘密にしていたことが……。打ち明ける機会を見つけよう。")),
        (63, TableItem::Text("探し物：調査のついでに、前から気になっていたものを探してみよう。")),
        (64, TableItem::Text("意外な一面：主従といえど、知らない一面もある。それを知る機会かもしれない。")),
        (65, TableItem::Text("不満：いい機会なので、言わせていただきたいことがある！")),
        (66, TableItem::Text("世話焼き：いつものように主の世話を焼こう。あるいは逆というのも面白い。")),
    ],
);

/// i18n `KizunaBullet.table.EP`（ja_jp）。
pub(crate) static JA_TABLE_EP: Table = Table::from_dice(
    "遭遇表・場所",
    1,
    6,
    &[
        "事件現場：事件が起きた現場（のひとつ）で遭遇した。",
        "資料の在処：事件の証拠や資料が保管されている場所で遭遇した。",
        "病院／家：被害者がいる病院や、家族が住む家の前で遭遇した。",
        "道路／店：現場へ向かう道や、近くの店で遭遇した。",
        "情報源：情報屋や事情通の知り合いに会いに行って遭遇した。",
        "本拠地：バレットAの組織へ、バレットBが情報収集に現れた。",
    ],
);

/// i18n `KizunaBullet.table.EO`（ja_jp）。
pub(crate) static JA_TABLE_EO: Table = Table::from_dice(
    "遭遇表・登場順",
    1,
    6,
    &[
        "バレットA：バレットAが先に場面に登場する。",
        "バレットA：バレットAが先に場面に登場する。",
        "バレットA：バレットAが先に場面に登場する。",
        "バレットB：バレットBが先に場面に登場する。",
        "バレットB：バレットBが先に場面に登場する。",
        "バレットB：バレットBが先に場面に登場する。",
    ],
);

/// i18n `KizunaBullet.table.EF`（ja_jp）。
pub(crate) static JA_TABLE_EF: Table = Table::from_dice(
    "遭遇表・状況（初対面）",
    1,
    6,
    &[
        "警戒姿勢：思わぬ相手の出現だ。先に登場していたバレットが、もう一方へ所属と目的を尋ねる。",
        "自己紹介(A)：不要な諍いを避けよう。先にバレットAが自らの所属と目的を明かす。",
        "気安い態度：どうやら相手もバレットのようだ。後に登場したバレットが、もう一方へ所属と目的を尋ねる。",
        "自己紹介(B)：情報を開示した方が得策だろう。先にバレットBが自らの所属と目的を明かす。",
        "丁寧な態度：無用なトラブルを避けるため、先に登場していたバレットが丁寧な態度で話しかける。",
        "攻撃：敵と認識し、先に登場したバレットが攻撃を仕掛ける。ただし、誤解はすぐに解ける。",
    ],
);

/// i18n `KizunaBullet.table.EA`（ja_jp）。
pub(crate) static JA_TABLE_EA: Table = Table::from_dice(
    "遭遇表・状況（知り合い）",
    1,
    6,
    &[
        "先日の礼：先日は世話になった。先に登場したバレットが、もう一方に礼を述べ、自分たちの目的を話す。",
        "預かりもの：預かったものを返そうと思っていた。後に登場したバレットが預かりものを返し、自分たちの目的を話す。",
        "思わぬ幸運：ちょうど連絡しようと思っていたところだ。バレットBから、自分たちの目的を話す。",
        "待ち合わせ：すでにバレット同士で連絡をとり、待ち合わせていた。バレットAから自分たちの目的を話す。",
        "またお前らか：腐れ縁というやつか。苦笑いしつつ、後に登場したバレットから自分たちの目的を話す。",
        "攻撃：先に登場したバレットが、もう一方へ攻撃を仕掛ける。なに、挨拶みたいなものさ。",
    ],
);

/// i18n `KizunaBullet.table.EE`（ja_jp）。
pub(crate) static JA_TABLE_EE: Table = Table::from_dice(
    "遭遇表・決着",
    1,
    6,
    &[
        "同行しよう：互いの状況を確認した結果、まずは一緒に行動するのが得策と判断した。",
        "相互監視：互いの状況は理解した。しかし完全に信用はできない。同行して監視しよう。",
        "分散連携：情報を整理した結果、密に連絡をとりつつ手分けして調査を進めることになった。",
        "自由にやろう：自由に動く方が効率がよい。最低限の連絡だけとって、独自に調査を進めることになった。",
        "利益重視：協力することが利益を生む。互いを利用する……もとい助け合うことになった。",
        "上層部の伝達：上層部に先に連絡を交わしており、協力体制をとるよう双方に指令が下った。",
    ],
);

/// i18n `KizunaBullet.table.CP`（ja_jp）。
pub(crate) static JA_TABLE_CP: Table = Table::from_dice(
    "交流表・場所",
    1,
    6,
    &[
        "カフェ：お洒落なカフェ。一息つくには丁度いい。",
        "路地裏：薄暗い路地裏。少なくとも人目は気にならない。",
        "公園：解放感のある公園。自動販売機もある。",
        "拠点：組織が管理している隠れ家。安全な場所だ。",
        "車内：車や電車の中。他の人がいるなら声量には気をつけて。",
        "屋上：街を見下ろせるビルの屋上。風が気持ちいい。",
    ],
);

/// i18n `KizunaBullet.table.CC`（ja_jp）。
pub(crate) static JA_TABLE_CC: Table = Table::from_dice(
    "交流表・内容",
    1,
    6,
    &[
        "下準備：次の調査や、戦いに向けた下準備を進めよう。",
        "被害者：事件や標的に被害を受けた人について話そう。",
        "ひと休み：円滑な任務遂行のためには、ひと休みも必要だ。",
        "次の予定：この事件が終わった後のスケジュールを決めよう。",
        "気付いたこと：調査活動の中で気付いたことを話し合ってみよう。",
        "敵：事件の犯人や標的などについて話し合ってみよう。",
    ],
);

/// i18n `KizunaBullet.table.IB`（ja_jp）。
pub(crate) static JA_TABLE_IB: D66Table = D66Table::new(
    "調査表・ベーシック",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("遺留品：現場に残された遺留品や、押収された品などを、しっかり調べてみよう。")),
        (12, TableItem::Text("聞き込み（繁華街）：繁華街の店員や通行人に聞き込みしてみよう。小さな違和感や気がかりがヒントになるかも。")),
        (13, TableItem::Text("過去の洗い直し：標的が起こした過去の事件や、活動の履歴を、しっかり洗い直してみよう。")),
        (14, TableItem::Text("情報屋（取引）：裏社会の事情に通じている情報屋と会おう。うまく折り合いがつけば情報を手に入れられるかも。")),
        (15, TableItem::Text("ウェブ調査：インターネットを使って情報を集めてみよう。SNSの書き込みや噂も、なかなか馬鹿にできない。")),
        (16, TableItem::Text("報告の整理：バックアップの調査員から報告が集まっている。話を聞きに行くか、書類に目を通してみよう。")),
        (21, TableItem::Text("遺留品：現場に残された遺留品や、押収された品などを、しっかり調べてみよう。")),
        (22, TableItem::Text("聞き込み（繁華街）：繁華街の店員や通行人に聞き込みしてみよう。小さな違和感や気がかりがヒントになるかも。")),
        (23, TableItem::Text("過去の洗い直し：標的が起こした過去の事件や、活動の履歴を、しっかり洗い直してみよう。")),
        (24, TableItem::Text("情報屋（取引）：裏社会の事情に通じている情報屋と会おう。うまく折り合いがつけば情報を手に入れられるかも。")),
        (25, TableItem::Text("ウェブ調査：インターネットを使って情報を集めてみよう。SNSの書き込みや噂も、なかなか馬鹿にできない。")),
        (26, TableItem::Text("報告の整理：バックアップの調査員から報告が集まっている。話を聞きに行くか、書類に目を通してみよう。")),
        (31, TableItem::Text("遺留品：現場に残された遺留品や、押収された品などを、しっかり調べてみよう。")),
        (32, TableItem::Text("聞き込み（繁華街）：繁華街の店員や通行人に聞き込みしてみよう。小さな違和感や気がかりがヒントになるかも。")),
        (33, TableItem::Text("過去の洗い直し：標的が起こした過去の事件や、活動の履歴を、しっかり洗い直してみよう。")),
        (34, TableItem::Text("情報屋（取引）：裏社会の事情に通じている情報屋と会おう。うまく折り合いがつけば情報を手に入れられるかも。")),
        (35, TableItem::Text("ウェブ調査：インターネットを使って情報を集めてみよう。SNSの書き込みや噂も、なかなか馬鹿にできない。")),
        (36, TableItem::Text("報告の整理：バックアップの調査員から報告が集まっている。話を聞きに行くか、書類に目を通してみよう。")),
        (41, TableItem::Text("ハッキング：標的に関係がありそうな組織や施設のコンピュータに、ハッキングを仕掛けてみよう。")),
        (42, TableItem::Text("聞き込み（学生）：街で学生たちに聞き込みをしてみよう。彼らの情報網は意外と馬鹿にならないものだ。")),
        (43, TableItem::Text("専門家を訪問：事件や標的に関連のある専門家を訪ねよう。うまく話せば有益なヒントが得られるかも。")),
        (44, TableItem::Text("現場検証：事件の現場や、標的が目撃された場所に、なにか手がかりが残っていないか調べてみよう。")),
        (45, TableItem::Text("聞き込み（港湾部）：港や倉庫が集まる地域で聞き込みをしてみよう。不審な積み荷や業者などの情報が得られるかも。")),
        (46, TableItem::Text("謎の電話：関係者を名乗る謎の電話がかかってきた。うまく話を聞き出すことはできるだろうか。")),
        (51, TableItem::Text("ハッキング：標的に関係がありそうな組織や施設のコンピュータに、ハッキングを仕掛けてみよう。")),
        (52, TableItem::Text("聞き込み（学生）：街で学生たちに聞き込みをしてみよう。彼らの情報網は意外と馬鹿にならないものだ。")),
        (53, TableItem::Text("専門家を訪問：事件や標的に関連のある専門家を訪ねよう。うまく話せば有益なヒントが得られるかも。")),
        (54, TableItem::Text("現場検証：事件の現場や、標的が目撃された場所に、なにか手がかりが残っていないか調べてみよう。")),
        (55, TableItem::Text("聞き込み（港湾部）：港や倉庫が集まる地域で聞き込みをしてみよう。不審な積み荷や業者などの情報が得られるかも。")),
        (56, TableItem::Text("謎の電話：関係者を名乗る謎の電話がかかってきた。うまく話を聞き出すことはできるだろうか。")),
        (61, TableItem::Text("ハッキング：標的に関係がありそうな組織や施設のコンピュータに、ハッキングを仕掛けてみよう。")),
        (62, TableItem::Text("聞き込み（学生）：街で学生たちに聞き込みをしてみよう。彼らの情報網は意外と馬鹿にならないものだ。")),
        (63, TableItem::Text("専門家を訪問：事件や標的に関連のある専門家を訪ねよう。うまく話せば有益なヒントが得られるかも。")),
        (64, TableItem::Text("現場検証：事件の現場や、標的が目撃された場所に、なにか手がかりが残っていないか調べてみよう。")),
        (65, TableItem::Text("聞き込み（港湾部）：港や倉庫が集まる地域で聞き込みをしてみよう。不審な積み荷や業者などの情報が得られるかも。")),
        (66, TableItem::Text("謎の電話：関係者を名乗る謎の電話がかかってきた。うまく話を聞き出すことはできるだろうか。")),
    ],
);

/// i18n `KizunaBullet.table.ID`（ja_jp）。
pub(crate) static JA_TABLE_ID: D66Table = D66Table::new(
    "調査表・ダイナミック",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("突然の抗争：聞き込みに訪れた店で銃撃戦が発生した。やめさせるか、全員ぶちのめして話を聞くとしよう。")),
        (12, TableItem::Text("色仕掛け：重要な情報を持っていそうな人物を見つけた。ここはひとつ、色仕掛けで話を聞き出してみよう。")),
        (13, TableItem::Text("返り討ち（個人）：あなたたちに因縁のある人物が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (14, TableItem::Text("血の代償：情報の代価に邪魔な人物の“処分”を提示された。実行するか、別の条件を提示する必要がある。")),
        (15, TableItem::Text("聞き込み（裏社会）：裏社会に属する犯罪組織へ、聞き込みに行こう。ちょっと荒っぽい行動が必要になるかもしれない。")),
        (16, TableItem::Text("返り討ち（集団）：あなたたちに因縁のある集団が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (21, TableItem::Text("突然の抗争：聞き込みに訪れた店で銃撃戦が発生した。やめさせるか、全員ぶちのめして話を聞くとしよう。")),
        (22, TableItem::Text("色仕掛け：重要な情報を持っていそうな人物を見つけた。ここはひとつ、色仕掛けで話を聞き出してみよう。")),
        (23, TableItem::Text("返り討ち（個人）：あなたたちに因縁のある人物が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (24, TableItem::Text("血の代償：情報の代価に邪魔な人物の“処分”を提示された。実行するか、別の条件を提示する必要がある。")),
        (25, TableItem::Text("聞き込み（裏社会）：裏社会に属する犯罪組織へ、聞き込みに行こう。ちょっと荒っぽい行動が必要になるかもしれない。")),
        (26, TableItem::Text("返り討ち（集団）：あなたたちに因縁のある集団が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (31, TableItem::Text("突然の抗争：聞き込みに訪れた店で銃撃戦が発生した。やめさせるか、全員ぶちのめして話を聞くとしよう。")),
        (32, TableItem::Text("色仕掛け：重要な情報を持っていそうな人物を見つけた。ここはひとつ、色仕掛けで話を聞き出してみよう。")),
        (33, TableItem::Text("返り討ち（個人）：あなたたちに因縁のある人物が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (34, TableItem::Text("血の代償：情報の代価に邪魔な人物の“処分”を提示された。実行するか、別の条件を提示する必要がある。")),
        (35, TableItem::Text("聞き込み（裏社会）：裏社会に属する犯罪組織へ、聞き込みに行こう。ちょっと荒っぽい行動が必要になるかもしれない。")),
        (36, TableItem::Text("返り討ち（集団）：あなたたちに因縁のある集団が襲いかかってきた。返り討ちのついでに、何か話を聞き出せるだろうか。")),
        (41, TableItem::Text("力試し：情報の代価に、腕試しや度胸試しを提示された。提案に乗るか、他の条件を提示する必要がある。")),
        (42, TableItem::Text("襲撃（アジト）：情報を持っていそうな犯罪者や犯罪組織。そのアジトを襲撃して、情報を漁ることにした。")),
        (43, TableItem::Text("聞き込み（裏市場）：盗品や横流し品などを扱う裏市場。ここでなら有益な情報を得られるかもしれない。")),
        (44, TableItem::Text("情報屋（脅迫）：いい情報を持っているかもしれない情報屋。弱みを握って脅迫すれば、簡単に口を割るだろう。")),
        (45, TableItem::Text("襲撃（取引現場）：偶然にも、密輸品の取引現場に遭遇した。こいつらをぶちのめしたら、情報を得られるかも。")),
        (46, TableItem::Text("逃走劇：情報を持っていそうな人物が逃げ出した。追いかけて捕まえて、お話しをしてもらおう。")),
        (51, TableItem::Text("力試し：情報の代価に、腕試しや度胸試しを提示された。提案に乗るか、他の条件を提示する必要がある。")),
        (52, TableItem::Text("襲撃（アジト）：情報を持っていそうな犯罪者や犯罪組織。そのアジトを襲撃して、情報を漁ることにした。")),
        (53, TableItem::Text("聞き込み（裏市場）：盗品や横流し品などを扱う裏市場。ここでなら有益な情報を得られるかもしれない。")),
        (54, TableItem::Text("情報屋（脅迫）：いい情報を持っているかもしれない情報屋。弱みを握って脅迫すれば、簡単に口を割るだろう。")),
        (55, TableItem::Text("襲撃（取引現場）：偶然にも、密輸品の取引現場に遭遇した。こいつらをぶちのめしたら、情報を得られるかも。")),
        (56, TableItem::Text("逃走劇：情報を持っていそうな人物が逃げ出した。追いかけて捕まえて、お話しをしてもらおう。")),
        (61, TableItem::Text("力試し：情報の代価に、腕試しや度胸試しを提示された。提案に乗るか、他の条件を提示する必要がある。")),
        (62, TableItem::Text("襲撃（アジト）：情報を持っていそうな犯罪者や犯罪組織。そのアジトを襲撃して、情報を漁ることにした。")),
        (63, TableItem::Text("聞き込み（裏市場）：盗品や横流し品などを扱う裏市場。ここでなら有益な情報を得られるかもしれない。")),
        (64, TableItem::Text("情報屋（脅迫）：いい情報を持っているかもしれない情報屋。弱みを握って脅迫すれば、簡単に口を割るだろう。")),
        (65, TableItem::Text("襲撃（取引現場）：偶然にも、密輸品の取引現場に遭遇した。こいつらをぶちのめしたら、情報を得られるかも。")),
        (66, TableItem::Text("逃走劇：情報を持っていそうな人物が逃げ出した。追いかけて捕まえて、お話しをしてもらおう。")),
    ],
);

/// i18n `KizunaBullet.table.HA`（ja_jp）。
pub(crate) static JA_TABLE_HA: Table = Table::from_dice(
    "ハザード表",
    1,
    6,
    &[
        "不活性化：【励起値】がもっとも高いPCが。【励起値】を1点失う。",
        "キセキ増強：［決戦］でエネミーが与えるダメージに+3する。",
        "小さな事故：PCひとり（任意に決定）が【耐久値】を［1D］失う。",
        "強まるネガイ：［決戦］でエネミーの初期【生命値】が1点増加する。",
        "大きな事故：PC全員が【耐久値】を［1D］失う。",
        "悪運強し：なんたる幸運！　何も発生しない。",
    ],
);

/// i18n `KizunaBullet.table.NI1`（ja_jp）。
pub(crate) static JA_TABLE_NI1: Table = Table::from_dice(
    "その事件の内容は……",
    1,
    6,
    &[
        "凄惨な殺人",
        "人がこつ然と消えた",
        "突然言動がおかしくなる人間が増えた",
        "不自然な溺死／焼死／転落死／の遺体",
        "不可解な火災／爆発／水害／の発生",
        "意識不明の人間が増えた",
    ],
);

/// i18n `KizunaBullet.table.NI2`（ja_jp）。
pub(crate) static JA_TABLE_NI2: Table = Table::from_dice(
    "捜査に向かった場所は……",
    1,
    6,
    &[
        "裏社会の緩衝地帯である夜の繁華街",
        "密輸品の取引場になっている貨物船",
        "多くの学生が行き交う大学",
        "一般人が暮らす住宅街",
        "上流階級の集まる社交場",
        "犯罪結社が携わる地下の大賭博場",
    ],
);

/// i18n `KizunaBullet.table.NI3`（ja_jp）。
pub(crate) static JA_TABLE_NI3: Table = Table::from_dice(
    "犯人のキセキ使いは……",
    1,
    6,
    &[
        "殺された家族の復讐を誓っていた",
        "人間の壊し方を探究していた",
        "亡き主人の命令を守り続けていた",
        "金を集めることが目的だった",
        "身勝手な善行を続けていた",
        "気紛れに力を振るい続けていた",
    ],
);

/// i18n `KizunaBullet.table.NI4`（ja_jp）。
pub(crate) static JA_TABLE_NI4: Table = Table::from_dice(
    "起きた出来事は……",
    1,
    6,
    &[
        "キセキ使いによって次の被害が出た",
        "裏社会の人間と乱闘になった",
        "オーナーの古い知人に偶然出会った",
        "ハウンドの苦手なことを強いられた",
        "ハウンドが得意なことで活躍した",
        "キセキ使いによる次の被害を阻止した",
    ],
);

/// i18n `KizunaBullet.table.NI5`（ja_jp）。
pub(crate) static JA_TABLE_NI5: Table = Table::from_dice(
    "バレットの間では……",
    1,
    6,
    &[
        "少しだけ本音を明かし合った",
        "新たな一面を知って隔たりを感じた",
        "そらぞらしい希望を語り合った",
        "キセキ使いを自分たちに重ねた",
        "一緒に行動する時間を楽しんだ",
        "理解し合えないのかと悩みが深まった",
    ],
);

/// i18n `KizunaBullet.table.NI6`（ja_jp）。
pub(crate) static JA_TABLE_NI6: Table = Table::from_dice(
    "戦いの結末は……",
    1,
    6,
    &[
        "順調に倒すことができた。：破壊（任意）×1、チェック（任意）×1",
        "敵の攻撃が激しくオーナーが消耗した。：破壊（任意）×1、破壊（オーナー）×1",
        "しぶとく長時間の戦いを強いられた。：破壊（任意）×2",
        "敵の抵抗が激しくハウンドが無理をした。：破壊（任意）×1、破壊（ハウンド）×1",
        "敵が意地を見せて手痛い一撃を食らった。：破壊（任意）×2、チェック（任意）×1",
        "不運が重なり、限界近くまで消耗した。：破壊（任意）×3",
    ],
);

/// i18n `KizunaBullet.table.NT1`（ja_jp）。
pub(crate) static JA_TABLE_NT1: Table = Table::from_dice(
    "その場所とは……",
    1,
    6,
    &[
        "奇妙な風習が残る和風屋敷",
        "絶海の孤島に建てられたお洒落な洋館",
        "試験航海中の豪華客船",
        "森の奥に建てられた怪しげなホテル",
        "いわくのある山奥の廃病院",
        "地図に存在しない謎の村",
    ],
);

/// i18n `KizunaBullet.table.NT2`（ja_jp）。
pub(crate) static JA_TABLE_NT2: Table = Table::from_dice(
    "そこで始まったのは……",
    1,
    6,
    &[
        "伝承歌になぞらえた殺人事件が起きた",
        "マスクを被った殺人鬼が現れた",
        "人間がひとりずつ消えていった",
        "奇怪な姿の怪物が襲いかかってきた",
        "洗脳された一般人が人間狩りを始めた",
        "生存をかけたデスゲームが宣言された",
    ],
);

/// i18n `KizunaBullet.table.NT3`（ja_jp）。
pub(crate) static JA_TABLE_NT3: Table = Table::from_dice(
    "極限状態のなかで……",
    1,
    6,
    &[
        "警察官が自分を犠牲に仲間を助けた",
        "生存者にキセキ使いの仲間がいた",
        "自分の命惜しさに裏切る金持ちがいた",
        "謎の組織の関与が明らかになってきた",
        "建物に爆弾が仕掛けられていた",
        "容疑者がすでに死んでいた",
    ],
);

/// i18n `KizunaBullet.table.NT4`（ja_jp）。
pub(crate) static JA_TABLE_NT4: Table = Table::from_dice(
    "犯人のキセキ使いは……",
    1,
    6,
    &[
        "人間の恐怖心を分析しようとしていた",
        "恋人の無念を晴らそうとしていた",
        "罰を与えることに執心していた",
        "悪趣味な脅かしで心を満たしていた",
        "隠された何を守ろうとしていた",
        "注目されることに快感を覚えていた",
    ],
);

/// i18n `KizunaBullet.table.NT5`（ja_jp）。
pub(crate) static JA_TABLE_NT5: Table = Table::from_dice(
    "バレットの間では……",
    1,
    6,
    &[
        "生きて帰れたらやりたいことを話し合った",
        "少し前にやらかしたことを告白して謝った",
        "遠慮していたことを思いきり言い合った",
        "お互いの力を信頼して背中を預けることにした",
        "こんな時にも（こんな時だからこそ）本心を隠した",
        "なんだかんだで一番頼りになる相手だと感じた",
    ],
);

/// i18n `KizunaBullet.table.NT6`（ja_jp）。
pub(crate) static JA_TABLE_NT6: Table = Table::from_dice(
    "戦いの結末は……",
    1,
    6,
    &[
        "順調に倒すことができた。：破壊（任意）×1、チェック（任意）×1",
        "敵の攻撃が激しくオーナーが消耗した。：破壊（任意）×1、破壊（オーナー）×1",
        "しぶとく長時間の戦いを強いられた。：破壊（任意）×2",
        "敵の抵抗が激しくハウンドが無理をした。：破壊（任意）×1、破壊（ハウンド）×1",
        "敵が意地を見せて手痛い一撃を食らった。：破壊（任意）×2、チェック（任意）×1",
        "不運が重なり、限界近くまで消耗した。：破壊（任意）×3",
    ],
);

/// i18n `KizunaBullet.table.HH1`（ja_jp）。
pub(crate) static JA_TABLE_HH1: Table = Table::from_dice(
    "その場所とは……",
    1,
    6,
    &[
        "ランチブッフェ／スイーツバイキング",
        "水族館／海浜公園",
        "動物園／植物園",
        "美術館／博物館",
        "プール／レジャー施設",
        "ショッピングモール",
    ],
);

/// i18n `KizunaBullet.table.HH2`（ja_jp）。
pub(crate) static JA_TABLE_HH2: Table = Table::from_dice(
    "待ち合わせをしたら……",
    1,
    6,
    &[
        "ハウンドはそこそこお洒落した",
        "オーナーはそこそこお洒落した",
        "ハウンドは気合をいれてお洒落した",
        "オーナーは気合をいれてお洒落した",
        "ハウンドはいつも通りの格好だった",
        "オーナーはいつも通りの格好だった",
    ],
);

/// i18n `KizunaBullet.table.HH3`（ja_jp）。
pub(crate) static JA_TABLE_HH3: Table = Table::from_dice(
    "そしてなんと……",
    1,
    6,
    &[
        "なぜかカラオケ大会に出ることに",
        "通りすがりのキセキ使いに出くわした",
        "財布／家の鍵／身分証／を落とした",
        "強盗が出現！　立てこもり事件発生",
        "時限爆弾が設置された！？",
        "ファッションコンテストに出ることに",
    ],
);

/// i18n `KizunaBullet.table.HH4`（ja_jp）。
pub(crate) static JA_TABLE_HH4: Table = Table::from_dice(
    "ふたりが決めたのは……",
    1,
    6,
    &[
        "なんとか協力して乗り切ることにした",
        "その場から脱出することにした",
        "オーナーの力をアテにすることにした",
        "ハウンドの力をアテにすることにした",
        "ノリと勢いでやってみることにした",
        "諦めて流れに身を任せることにした",
    ],
);

/// i18n `KizunaBullet.table.HH5`（ja_jp）。
pub(crate) static JA_TABLE_HH5: Table = Table::from_dice(
    "結果的に……",
    1,
    6,
    &[
        "ものすごい幸運が重なってなんとかなった",
        "オーナーが隠れた才能を発揮してなんとかなった",
        "ハウンドが力技でなんとか誤魔化した",
        "組織（知人）に連絡をとって解決してもらった",
        "親切な人に助けてもらってなんとかなった",
        "ふたりの天才的な閃きで見事に解決した",
    ],
);

/// i18n `KizunaBullet.table.HH6`（ja_jp）。
pub(crate) static JA_TABLE_HH6: Table = Table::from_dice(
    "バレットは最後に……",
    1,
    6,
    &[
        "これもいい経験だったと割り切ることにした",
        "失態を繰り返さないようリベンジを誓った",
        "ふたりだけの秘密にしようと約束した",
        "今後のため帰って復習することにした",
        "仲良く喧嘩をしながら帰るのだった",
        "こっそり買っていたプレゼントを渡し合った",
    ],
);

/// i18n `KizunaBullet.table.HC1`（ja_jp）。
pub(crate) static JA_TABLE_HC1: Table = Table::from_dice(
    "その場所とは……",
    1,
    6,
    &[
        "大都市の中にある綺麗な病院",
        "海沿いにある静かな公園",
        "大勢の人で賑わう遊園地",
        "活気にあふれた商店街",
        "たくさんの人が行き交う駅",
        "山奥にあるキャンプ場",
    ],
);

/// i18n `KizunaBullet.table.HC2`（ja_jp）。
pub(crate) static JA_TABLE_HC2: Table = Table::from_dice(
    "起きた事件は……",
    1,
    6,
    &[
        "人びとが「バブー」しか話せなくなる",
        "あらゆる食べ物の味がド甘くなる",
        "話したいことと逆の言葉が出てくる",
        "薄着になりたくて仕方なくなる",
        "強制的に笑顔になってしまう",
        "腹が減って腹が減って仕方なくなる",
    ],
);

/// i18n `KizunaBullet.table.HC3`（ja_jp）。
pub(crate) static JA_TABLE_HC3: Table = Table::from_dice(
    "犯人のキセキ使いは……",
    1,
    6,
    &[
        "他人が困る姿に快楽を覚えいた",
        "自分の芸術を見せびらかしていた",
        "世間に怒りを抱えていた",
        "気の向くままに遊んでいた",
        "自分なりの正義を執行していた",
        "実は深遠な計画を進めていた",
    ],
);

/// i18n `KizunaBullet.table.HC4`（ja_jp）。
pub(crate) static JA_TABLE_HC4: Table = Table::from_dice(
    "犯人を追い詰めるべく……",
    1,
    6,
    &[
        "地道な聞き込みをすることにした",
        "目立って誘き寄せることにした",
        "犯人の気持ちになってみることにした",
        "頭を冷やすため遊びに出かけた",
        "犯人の裏をかいてみることにした",
        "とにかく暴力で解決することにした",
    ],
);

/// i18n `KizunaBullet.table.HC5`（ja_jp）。
pub(crate) static JA_TABLE_HC5: Table = Table::from_dice(
    "戦いの結果は……",
    1,
    6,
    &[
        "なんだかんだ一方的に勝利した",
        "犯人が策に溺れて自滅していった……",
        "通りすがりの知り合いが倒してくれた",
        "ちょっと苦戦したが倒すことができた",
        "犯人は意味深な言葉を残して自爆した",
        "通りすがりの戸山紅果（『第2巻』P135）が犯人を殺した",
    ],
);

/// i18n `KizunaBullet.table.HC6`（ja_jp）。
pub(crate) static JA_TABLE_HC6: Table = Table::from_dice(
    "バレットは最後に……",
    1,
    6,
    &[
        "気を取り直して遊びに行くことにした",
        "これもいい経験だったと思うことにした",
        "まだ事件を終わっていないように感じた",
        "美味しいものでも食べて帰ることにした",
        "次はどこか遊びに行こうと約束した",
        "すぐ新たな事件に巻き込まれるのであった",
    ],
);

/// Ruby `OPC`（OP・OC を順に振って改行で連結する）。
static JA_MULTI_OPC: &[&dyn RollableTable] = &[&JA_TABLE_OP, &JA_TABLE_OC];

/// Ruby `OWPC`（OWP・OWC を順に振って改行で連結する）。
static JA_MULTI_OWPC: &[&dyn RollableTable] = &[&JA_TABLE_OWP, &JA_TABLE_OWC];

/// Ruby `OHPC`（OHP・OHC を順に振って改行で連結する）。
static JA_MULTI_OHPC: &[&dyn RollableTable] = &[&JA_TABLE_OHP, &JA_TABLE_OHC];

/// Ruby `OTPC`（OTP・OTC を順に振って改行で連結する）。
static JA_MULTI_OTPC: &[&dyn RollableTable] = &[&JA_TABLE_OTP, &JA_TABLE_OTC];

/// Ruby `EFA`（EP・EO・EF・EE を順に振って改行で連結する）。
static JA_MULTI_EFA: &[&dyn RollableTable] =
    &[&JA_TABLE_EP, &JA_TABLE_EO, &JA_TABLE_EF, &JA_TABLE_EE];

/// Ruby `EAA`（EP・EO・EA・EE を順に振って改行で連結する）。
static JA_MULTI_EAA: &[&dyn RollableTable] =
    &[&JA_TABLE_EP, &JA_TABLE_EO, &JA_TABLE_EA, &JA_TABLE_EE];

/// Ruby `CPC`（CP・CC を順に振って改行で連結する）。
static JA_MULTI_CPC: &[&dyn RollableTable] = &[&JA_TABLE_CP, &JA_TABLE_CC];

/// Ruby `IBD`（IB・ID を順に振って改行で連結する）。
static JA_MULTI_IBD: &[&dyn RollableTable] = &[&JA_TABLE_IB, &JA_TABLE_ID];

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static JA_TABLES: &[(&str, TableRef)] = &[
    ("OP", TableRef::Single(&JA_TABLE_OP)),
    ("OC", TableRef::Single(&JA_TABLE_OC)),
    ("OPC", TableRef::Multi(JA_MULTI_OPC)),
    ("OWP", TableRef::Single(&JA_TABLE_OWP)),
    ("OWC", TableRef::Single(&JA_TABLE_OWC)),
    ("OWPC", TableRef::Multi(JA_MULTI_OWPC)),
    ("OHP", TableRef::Single(&JA_TABLE_OHP)),
    ("OHC", TableRef::Single(&JA_TABLE_OHC)),
    ("OHPC", TableRef::Multi(JA_MULTI_OHPC)),
    ("OTP", TableRef::Single(&JA_TABLE_OTP)),
    ("OTC", TableRef::Single(&JA_TABLE_OTC)),
    ("OTPC", TableRef::Multi(JA_MULTI_OTPC)),
    ("TT", TableRef::Single(&JA_TABLE_TT)),
    ("TTI", TableRef::Single(&JA_TABLE_TTI)),
    ("TTC", TableRef::Single(&JA_TABLE_TTC)),
    ("TTH", TableRef::Single(&JA_TABLE_TTH)),
    ("EP", TableRef::Single(&JA_TABLE_EP)),
    ("EO", TableRef::Single(&JA_TABLE_EO)),
    ("EF", TableRef::Single(&JA_TABLE_EF)),
    ("EA", TableRef::Single(&JA_TABLE_EA)),
    ("EE", TableRef::Single(&JA_TABLE_EE)),
    ("EFA", TableRef::Multi(JA_MULTI_EFA)),
    ("EAA", TableRef::Multi(JA_MULTI_EAA)),
    ("CP", TableRef::Single(&JA_TABLE_CP)),
    ("CC", TableRef::Single(&JA_TABLE_CC)),
    ("CPC", TableRef::Multi(JA_MULTI_CPC)),
    ("IB", TableRef::Single(&JA_TABLE_IB)),
    ("ID", TableRef::Single(&JA_TABLE_ID)),
    ("IBD", TableRef::Multi(JA_MULTI_IBD)),
    ("HA", TableRef::Single(&JA_TABLE_HA)),
    ("NI1", TableRef::Single(&JA_TABLE_NI1)),
    ("NI2", TableRef::Single(&JA_TABLE_NI2)),
    ("NI3", TableRef::Single(&JA_TABLE_NI3)),
    ("NI4", TableRef::Single(&JA_TABLE_NI4)),
    ("NI5", TableRef::Single(&JA_TABLE_NI5)),
    ("NI6", TableRef::Single(&JA_TABLE_NI6)),
    ("NT1", TableRef::Single(&JA_TABLE_NT1)),
    ("NT2", TableRef::Single(&JA_TABLE_NT2)),
    ("NT3", TableRef::Single(&JA_TABLE_NT3)),
    ("NT4", TableRef::Single(&JA_TABLE_NT4)),
    ("NT5", TableRef::Single(&JA_TABLE_NT5)),
    ("NT6", TableRef::Single(&JA_TABLE_NT6)),
    ("HH1", TableRef::Single(&JA_TABLE_HH1)),
    ("HH2", TableRef::Single(&JA_TABLE_HH2)),
    ("HH3", TableRef::Single(&JA_TABLE_HH3)),
    ("HH4", TableRef::Single(&JA_TABLE_HH4)),
    ("HH5", TableRef::Single(&JA_TABLE_HH5)),
    ("HH6", TableRef::Single(&JA_TABLE_HH6)),
    ("HC1", TableRef::Single(&JA_TABLE_HC1)),
    ("HC2", TableRef::Single(&JA_TABLE_HC2)),
    ("HC3", TableRef::Single(&JA_TABLE_HC3)),
    ("HC4", TableRef::Single(&JA_TABLE_HC4)),
    ("HC5", TableRef::Single(&JA_TABLE_HC5)),
    ("HC6", TableRef::Single(&JA_TABLE_HC6)),
];

/// i18n `KizunaBullet.INVESTIGATE.success`（ja_jp）。
const JA_INVESTIGATE_SUCCESS: &str =
    "［成功］。［調査進行度］が2点増加。［シーンプレイヤー］の【励起値】が1点増加。";
/// i18n `KizunaBullet.INVESTIGATE.failure`（ja_jp）。
const JA_INVESTIGATE_FAILURE: &str = "［失敗］。［調査進行度］が1点増加。";
/// i18n `KizunaBullet.INVESTIGATE.partnerHelp`（ja_jp）。
const JA_INVESTIGATE_PARTNER_HELP: &str =
    "［シーンプレイヤー］の［パートナー］の【励起値】を1点消費すると成功（［パートナーのヘルプ］）";
/// i18n `KizunaBullet.INVESTIGATE.fumble`（ja_jp）。
const JA_INVESTIGATE_FUMBLE: &str = "［パートナーのヘルプ］使用不可。";
/// i18n `KizunaBullet.SEDATIVE.burst`（ja_jp）。
const JA_SEDATIVE_BURST: &str = "すべての［キズナ］が［ヒビワレ］状態のため［晶滅］（［死亡］）。以降のセッションで継続して使用不可。";
/// i18n `KizunaBullet.SEDATIVE.alive`（ja_jp）。
const JA_SEDATIVE_ALIVE: &str =
    "［ヒビワレ］状態の［キズナ］が6個未満のため［生存］。次のセッションに継続して使用可能。";
/// i18n `KizunaBullet.SEDATIVE.success`（ja_jp）。
const JA_SEDATIVE_SUCCESS: &str = "［生存］。次のセッションに継続して使用可能。";
/// i18n `KizunaBullet.SEDATIVE.failure`（ja_jp。`%{check}` を置換する）。
const JA_SEDATIVE_FAILURE: &str = "［残響体］化（NPC化）。パートナーは自分の任意の［キズナ］に%{check}つチェックを入れると［生存］（［強制鎮静］）";

/// `ja_jp` ロケールの表と定型文一式。
static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    investigate_success: JA_INVESTIGATE_SUCCESS,
    investigate_failure: JA_INVESTIGATE_FAILURE,
    investigate_partner_help: JA_INVESTIGATE_PARTNER_HELP,
    investigate_fumble: JA_INVESTIGATE_FUMBLE,
    sedative_burst: JA_SEDATIVE_BURST,
    sedative_alive: JA_SEDATIVE_ALIVE,
    sedative_success: JA_SEDATIVE_SUCCESS,
    sedative_failure: JA_SEDATIVE_FAILURE,
};

/// Ruby `BCDice::GameSystem::KizunaBullet`（ID: `KizunaBullet`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KizunaBullet;

impl GameSystem for KizunaBullet {
    fn id(&self) -> &'static str {
        "KizunaBullet"
    }

    fn name(&self) -> &'static str {
        "キズナバレット"
    }

    fn sort_key(&self) -> &'static str {
        "きすなはれつと"
    }

    fn help_message(&self) -> &'static str {
        r"・ダイスロール
nDM…n個の6面ダイスを転がして、一番高い出目を採用します。
・［調査判定］
nIN…n個の6面ダイスを転がして、一番高い出目が5以上なら成功します。（［パートナーのヘルプ］使用可）
・［鎮静判定］
SEn…2個の6面ダイスを転がして、出目の合計値がn（［ヒビワレ］状態の［キズナ］の個数）より高いと成功します。（［強制鎮静］使用可）
・［解決］ ［アクション］のダメージと［アクシデント］のダメージ軽減
nSO…2+n個の6面ダイスを転がして、出目をすべて合計します。（nは減らした【励起値】。省略可能）
・各種表
日常表・場所 OP
日常表・内容 OC
日常表・場所と内容 OPC
日常表（仕事）・場所 OWP
日常表（仕事）・内容 OWC
日常表（仕事）・場所と内容 OWPC
日常表（休暇）・場所 OHP
日常表（休暇）・内容 OHC
日常表（休暇）・場所と内容 OHPC
日常表（出張）・場所 OTP
日常表（出張）・内容 OTC
日常表（出張）・場所と内容 OTPC
ターンテーマ表 TT
ターンテーマ表・親密 TTI
ターンテーマ表・クール TTC
ターンテーマ表・主従 TTH
遭遇表・場所 EP
遭遇表・登場順 EO
遭遇表・状況（初対面） EF
遭遇表・状況（知り合い） EA
遭遇表・決着 EE
遭遇表・場所と登場順と状況（初対面）と決着 EFA
遭遇表・場所と登場順と状況（知り合い）と決着 EAA
交流表・場所 CP
交流表・内容 CC
交流表・場所と内容 CPC
調査表・ベーシック IB
調査表・ダイナミック ID
調査表・ベーシックとダイナミック IBD
ハザード表 HA
通常ダイジェスト　キミたちに新しい命令が下った（調査が依頼された）。
1:その事件の内容は…… NI1
2:捜査に向かった場所は…… NI2
3:犯人のキセキ使いは…… NI3
4:起きた出来事は…… NI4
5:バレットの間では…… NI5
6:戦いの結末は…… NI6
通常ダイジェスト　キミたちは旅行（出張）である場所を訪れた。
1:その場所とは…… NT1
2:そこで始まったのは…… NT2
3:極限状態のなかで…… NT3
4:犯人のキセキ使いは…… NT4
5:バレットの間では…… NT5
6:戦いの結末は…… NT6
ホリデーダイジェスト　キミたちは休日に出かけることにした。
1:その場所とは…… HH1
2:待ち合わせをしたら…… HH2
3:そしてなんと…… HH3
4:ふたりが決めたのは…… HH4
5:結果的に…… HH5
6:バレットは最後に…… HH6
ホリデーダイジェスト　キミたちは奇妙な事件に出くわした。
1:その場所とは…… HC1
2:起きた事件は…… HC2
3:犯人のキセキ使いは…… HC3
4:犯人を追い詰めるべく…… HC4
5:戦いの結果は…… HC5
6:バレットは最後に…… HC6
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+DM", r"\d+IN", r"SE\d+", r"\d*SO", "OP", "OC", "OPC", "OWP", "OWC", "OWPC", "OHP",
            "OHC", "OHPC", "OTP", "OTC", "OTPC", "TT", "TTI", "TTC", "TTH", "EP", "EO", "EF", "EA",
            "EE", "EFA", "EAA", "CP", "CC", "CPC", "IB", "ID", "IBD", "HA", "NI1", "NI2", "NI3",
            "NI4", "NI5", "NI6", "NT1", "NT2", "NT3", "NT4", "NT5", "NT6", "HH1", "HH2", "HH3",
            "HH4", "HH5", "HH6", "HC1", "HC2", "HC3", "HC4", "HC5", "HC6",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn round_type(&self) -> crate::enums::RoundType {
        crate::enums::RoundType::Ceil
    }

    /// Ruby `KizunaBullet#eval_game_system_specific_command`。
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
            .join("test/data/KizunaBullet.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/KizunaBullet.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/KizunaBullet.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KizunaBullet.toml must parse");
        assert_eq!(
            data.tests.len(),
            63,
            "case count in test/data/KizunaBullet.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KizunaBullet",
                "unexpected game system in KizunaBullet.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KizunaBullet"), &tc.input, &mut src) {
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
                    "FAIL KizunaBullet:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KizunaBullet cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
