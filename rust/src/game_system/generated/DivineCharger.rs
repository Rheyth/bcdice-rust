//! P4で手書き移植した `lib/bcdice/game_system/DivineCharger.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `DivineCharger#eval_game_system_specific_command`
//!   （`resolute_action || resolute_reverse || roll_tables`）
//! - 通常判定 `nDC>=t`（`#resolute_action` ＋ `#action_result`）
//! - 反転判定 `REV[n]>=t`（`#resolute_reverse` ＋ `#reverse_dice`）
//! - `TABLES`: ランダムイベント `RET`（`D66Table`）と神器表 `aksT` 60種（`Table`）
//!
//! # 表データ
//!
//! `TABLES` の名前・項目は Ruby 側の定数から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{dice_text, table_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::DivineCharger`（ID: `DivineCharger`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DivineCharger;

impl GameSystem for DivineCharger {
    fn id(&self) -> &'static str {
        "DivineCharger"
    }

    fn name(&self) -> &'static str {
        "神聖課金RPGディヴァインチャージャー"
    }

    fn sort_key(&self) -> &'static str {
        "しんせいかきんRPGていうあいんちやあしやあ"
    }

    fn help_message(&self) -> &'static str {
        r"■判定　　nDC>=t          n:能力値 t:目標値
例)3DC>=7: ダイスを3個振って、目標値7で判定。その結果(達成値,成功・失敗,クリティカル,ファンブル)を表示
　 3DC>=?: 　同上　目標値が不明なので、達成値,クリティカル,ファンブルのみ表示。

■反転判定　　REV[n]>=t   n:ダイス目(カンマなし) t:目標値
例)REV[123]>=7: 振ったダイスが[1,2,3]で、目標値7で反転判定。その結果(達成値,成功・失敗,クリティカル,ファンブル)を表示


■ランダムイベント　　RET
■神器　　　　　　　　aksT a:表(AかB) k:種別(K:近接, S:射撃, M:魔法, Y:鎧, T:盾, A:装飾品) s:ランク(1～5)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+DC", "REV", "RET", "AK1T", "AK2T", "AK3T", "AK4T", "AK5T", "AS1T", "AS2T", "AS3T",
            "AS4T", "AS5T", "AM1T", "AM2T", "AM3T", "AM4T", "AM5T", "AY1T", "AY2T", "AY3T", "AY4T",
            "AY5T", "AT1T", "AT2T", "AT3T", "AT4T", "AT5T", "AA1T", "AA2T", "AA3T", "AA4T", "AA5T",
            "BK1T", "BK2T", "BK3T", "BK4T", "BK5T", "BS1T", "BS2T", "BS3T", "BS4T", "BS5T", "BM1T",
            "BM2T", "BM3T", "BM4T", "BM5T", "BY1T", "BY2T", "BY3T", "BY4T", "BY5T", "BT1T", "BT2T",
            "BT3T", "BT4T", "BT5T", "BA1T", "BA2T", "BA3T", "BA4T", "BA5T",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `DivineCharger#eval_game_system_specific_command`。
    ///
    /// Ruby: `resolute_action(command) || resolute_reverse(command) || table_helpers::roll_table(command, TABLES, TABLES)`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = resolute_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = resolute_reverse(command) {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::Text))
    }

    /// Ruby `initialize`: `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// 判定。Ruby `#resolute_action` / `#action_result` / `#resolute_reverse` / `#reverse_dice`
// ---------------------------------------------------------------------------

/// Ruby `#resolute_action`（通常判定）: `/^(\d+)DC>=(\d+|\?)$/`。
///
/// Ruby の `\d` はASCII数字のみなので `[0-9]` で書く。
/// ダイスを `num_dice` 個振り、昇順に並べてから [`action_result`] で判定する。
fn resolute_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([0-9]+)DC>=([0-9]+|\?)$").expect("valid regex"));
    let Some(captures) = re.captures(command) else {
        return Ok(None);
    };

    let num_dice = parse_i64_saturating(&captures[1]);
    let target = &captures[2];

    // Ruby: dice = @randomizer.roll_barabara(num_dice, 6).sort
    let mut dice = rng.roll_barabara(num_dice, 6)?;
    dice.sort_unstable();
    let dice_text = dice_text::join_dice(&dice);
    let output = format!("({num_dice}DC>={target}) ＞ {dice_text}");

    Ok(Some(action_result(output, &dice, target)))
}

/// Ruby `#action_result`。`Action_data`（text, dice, target）から `Result` を組み立てる。
///
/// 6が2個以上ならクリティカル、（そうでなく）1が2個以上ならファンブル。
/// どちらでもなければ達成値を出し、目標値が `?` でなければ成功/失敗を判定する。
fn action_result(mut output: String, dice: &[i64], target: &str) -> EvalResult {
    let count6 = dice.iter().filter(|&&d| d == 6).count();
    let count1 = dice.iter().filter(|&&d| d == 1).count();
    let success_num: i64 = dice.iter().sum();

    if count6 >= 2 {
        output.push_str(&format!(" ＞ 達成値{success_num}"));
        output.push_str(" ＞ クリティカル");
        return EvalResult::critical(output);
    } else if count1 >= 2 {
        output.push_str(" ＞ 達成値0");
        output.push_str(" ＞ ファンブル([神聖石]5個)");
        return EvalResult::fumble(output);
    }

    output.push_str(&format!(" ＞ 達成値{success_num}"));

    if target == "?" {
        EvalResult::with_text(output)
    } else if success_num >= parse_i64_saturating(target) {
        output.push_str(" ＞ 成功");
        EvalResult::success(output)
    } else {
        output.push_str(" ＞ 失敗");
        EvalResult::failure(output)
    }
}

/// Ruby `#resolute_reverse`（反転判定）: `/^REV\[([\d,]+)\]>=(\d+|\?)$/`。
///
/// ダイスは振らず、入力の出目（カンマは除去）を反転させて [`action_result`] で判定する。
fn resolute_reverse(command: &str) -> Option<EvalResult> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"^REV\[([0-9,]+)\]>=([0-9]+|\?)$").expect("valid regex"));
    let captures = re.captures(command)?;

    // Ruby: raw_dice = m[1].delete(',')
    let raw_dice: String = captures[1].chars().filter(|&c| c != ',').collect();
    let target = &captures[2];

    let dice = reverse_dice(&raw_dice);
    let dice_text = dice_text::join_dice(&dice);
    let output = format!("(REV[{raw_dice}]>={target}) ＞ {dice_text}");

    Some(action_result(output, &dice, target))
}

/// Ruby `#reverse_dice`。出目を 1 <-> 6, 2 <-> 5, 3 <-> 4 で反転させ、昇順に並べる。
///
/// Ruby: `array_dice.map(&:to_i).filter { |v| 1 <= v && v <= 6 }.map { |v| 7 - v }.sort`
/// （1文字ずつ `to_i` するので、0 や 7〜9 は捨てられる）。
fn reverse_dice(raw_dice: &str) -> Vec<i64> {
    let mut dice: Vec<i64> = raw_dice
        .chars()
        .map(|c| c.to_digit(10).map_or(0, i64::from))
        .filter(|v| (1..=6).contains(v))
        .map(|v| 7 - v)
        .collect();
    dice.sort_unstable();
    dice
}

/// Ruby `String#to_i`（`[0-9]+` にマッチした文字列用）。
///
/// i64 を超える桁数は飽和させる（Ruby は Bignum になり、
/// `roll_barabara` が `TooManyRandsError` を投げる／目標値比較が失敗になる）。
fn parse_i64_saturating(text: &str) -> i64 {
    text.parse().unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// 表。Ruby `TABLES`
// ---------------------------------------------------------------------------

/// Ruby `TABLES["RET"]`（ランダムイベント）の項目。
static RET_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。近くに宝箱がある。[予測]:こういう場所では運動神経を試される罠が仕掛けてあることが多い。宝箱の中には、当然ながら金目のものが眠っているはずだ。[探索時間:4]")),
    (12, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。近くに宝箱がある。[予測]:こういう場所では運動神経を試される罠が仕掛けてあることが多い。宝箱の中には、当然ながら金目のものが眠っているはずだ。[探索時間:4]")),
    (13, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。向こう側に宝箱がある。[予測]:悪い予感がする。妙なトラップに遭遇するかもしれない。心の準備をしておいた方がいいだろう。宝箱には何かアイテムがあるような気がする。[探索時間:4]")),
    (14, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。向こう側に宝箱がある。[予測]:悪い予感がする。妙なトラップに遭遇するかもしれない。心の準備をしておいた方がいいだろう。宝箱には何かアイテムがあるような気がする。[探索時間:4]")),
    (15, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。中央に宝箱がある。[予測]:一見何もないように見える場所こそ注意が必要だ。いつでも立ち回れるようにした方がいいだろう。宝箱の中から光がにじみ出している。まさか〈神聖石〉が入っているのでは。[探索時間:4]")),
    (16, TableItem::Text("[描写]:辺りには何もなく、がらんとした部屋だ。中央に宝箱がある。[予測]:一見何もないように見える場所こそ注意が必要だ。いつでも立ち回れるようにした方がいいだろう。宝箱の中から光がにじみ出している。まさか〈神聖石〉が入っているのでは。[探索時間:4]")),
    (21, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁には幾何学的な模様が彫ってある。天井も無数の石のブロックで形成されている。[予測]:天井が崩れそうな予感がする。素早く探索しないと怪我しそうだ。ここには〈神聖石〉がある気がする。[探索時間:5]")),
    (22, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁には幾何学的な模様が彫ってある。天井も無数の石のブロックで形成されている。[予測]:天井が崩れそうな予感がする。素早く探索しないと怪我しそうだ。ここには〈神聖石〉がある気がする。[探索時間:5]")),
    (23, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁には様々なe壁画が彫ってある。[予測]:何か違和感がある。己の感覚を研ぎ澄まし注意した方がいいだろう。部屋の中央には宝箱が置いてある。[探索時間:5]")),
    (24, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁には様々なe壁画が彫ってある。[予測]:何か違和感がある。己の感覚を研ぎ澄まし注意した方がいいだろう。部屋の中央には宝箱が置いてある。[探索時間:5]")),
    (25, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁は光苔に覆われて輝いている。探せば何かあるかもしれない。[予測]:何か違和感を覚える。この違和感を押さえ込まないと、今後の行動に支障が出てきそうだ。部屋の端には薬棚があり、魔法薬が置いてある。[探索時間:5]")),
    (26, TableItem::Text("[描写]:石の壁で覆われた部屋だ。壁は光苔に覆われて輝いている。探せば何かあるかもしれない。[予測]:何か違和感を覚える。この違和感を押さえ込まないと、今後の行動に支障が出てきそうだ。部屋の端には薬棚があり、魔法薬が置いてある。[探索時間:5]")),
    (31, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。ガラクタから、何かを見つけることができるかもしれない。[予測]:こういうところでこそ、油断してはいけない。隙を突くようなトラップが仕掛けている場合がある。俊敏に動こう。ガラクタの中には、魔法薬の瓶がある。[探索時間:4]")),
    (32, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。ガラクタから、何かを見つけることができるかもしれない。[予測]:こういうところでこそ、油断してはいけない。隙を突くようなトラップが仕掛けている場合がある。俊敏に動こう。ガラクタの中には、魔法薬の瓶がある。[探索時間:4]")),
    (33, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。ガラクタの中から、何かいいものが落ちているかもしれない。[予測]:こういう場所には大体トラップが置いてあるはずだが、今のところその気配はない。感覚を研ぎ澄ませて慎重に行こう。隅に光る石がある。〈神聖石〉だろうか。[探索時間:3]")),
    (34, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。ガラクタの中から、何かいいものが落ちているかもしれない。[予測]:こういう場所には大体トラップが置いてあるはずだが、今のところその気配はない。感覚を研ぎ澄ませて慎重に行こう。隅に光る石がある。〈神聖石〉だろうか。[探索時間:3]")),
    (35, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。隅には宝箱が見える。[予測]:何やらすえた匂いがする。酸を使ったトラップがあるかもしれない。機敏に動こう。宝箱には金目の物があるだろう。[探索時間:3]")),
    (36, TableItem::Text("[描写]:小さな部屋だ。雑多に物がちらかっている。隅には宝箱が見える。[予測]:何やらすえた匂いがする。酸を使ったトラップがあるかもしれない。機敏に動こう。宝箱には金目の物があるだろう。[探索時間:3]")),
    (41, TableItem::Text("[描写]:光が差し込みにくい、薄暗い部屋だ。伸ばした自分の手の先もよく見えない。[予測]:このような場所ではうかつに動くと怪我をしてしまう。感覚を研ぎ澄まして動いた方がいいだろう。ここには〈神聖石〉がある気がする。[探索時間:4]")),
    (42, TableItem::Text("[描写]:光が差し込みにくい、薄暗い部屋だ。伸ばした自分の手の先もよく見えない。[予測]:このような場所ではうかつに動くと怪我をしてしまう。感覚を研ぎ澄まして動いた方がいいだろう。ここには〈神聖石〉がある気がする。[探索時間:4]")),
    (43, TableItem::Text("[描写]:光が差し込みにくい暗い部屋だ。探索には骨が折れるかもしれない。[予測]:このような場所では何が起きるかわからない。何が起きても動じない心構えが必要だ。身につけてるものもちゃんと管理しておこう。ここには宝がある気がする。[探索時間:4]")),
    (44, TableItem::Text("[描写]:光が差し込みにくい暗い部屋だ。探索には骨が折れるかもしれない。[予測]:このような場所では何が起きるかわからない。何が起きても動じない心構えが必要だ。身につけてるものもちゃんと管理しておこう。ここには宝がある気がする。[探索時間:4]")),
    (45, TableItem::Text("[描写]:光が差し込みにくい薄暗い部屋だ。何やら生き物の気配も感じる。[予測]:どんな生物がいるのか、探っておく必要があるだろう。対処方法につながる。ここには霊薬が置いてある気がする。[探索時間:4]")),
    (46, TableItem::Text("[描写]:光が差し込みにくい薄暗い部屋だ。何やら生き物の気配も感じる。[予測]:どんな生物がいるのか、探っておく必要があるだろう。対処方法につながる。ここには霊薬が置いてある気がする。[探索時間:4]")),
    (51, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。四方の壁には棚が置かれている。何か見つかればいいのだが。[予測]:魔法陣は明らかに怪しい。いつでも対応できるよう感覚を研ぎ澄ませ、装備にも気を配っておこう。棚には魔法の薬が置いてあるようだ。[探索時間:4]")),
    (52, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。四方の壁には棚が置かれている。何か見つかればいいのだが。[予測]:魔法陣は明らかに怪しい。いつでも対応できるよう感覚を研ぎ澄ませ、装備にも気を配っておこう。棚には魔法の薬が置いてあるようだ。[探索時間:4]")),
    (53, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。探索すれば何かあるかもしれない。[予測]:魔法陣は明らかに怪しい。これに罠があるとするなら、知性を試されるようなものに違いない。注意しておこう。この部屋には〈神聖石〉がある気がする。[探索時間:4]")),
    (54, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。探索すれば何かあるかもしれない。[予測]:魔法陣は明らかに怪しい。これに罠があるとするなら、知性を試されるようなものに違いない。注意しておこう。この部屋には〈神聖石〉がある気がする。[探索時間:4]")),
    (55, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。探索すれば何かあるかもしれない。[予測]:この魔法陣が罠であるのは間違いない。いつでも対応できるように俊敏に行動しよう。部屋の隅には宝箱があり、金目の物が入ってそうだ。[探索時間:4]")),
    (56, TableItem::Text("[描写]:床や天井におどろおどろしい魔法陣が描かれている部屋だ。探索すれば何かあるかもしれない。[予測]:この魔法陣が罠であるのは間違いない。いつでも対応できるように俊敏に行動しよう。部屋の隅には宝箱があり、金目の物が入ってそうだ。[探索時間:4]")),
    (61, TableItem::Text("[描写]:静謐な部屋だ。中央には泉があり、清らかな空気を放っている。泉のそばにはキノコが生えている。慎重に食べた方がいいだろう。[予測]:キノコは魔法のキノコで、何かしらの効果が期待されるが、キノコの魔法成分を受け止める精神力が必要だ。また、泉の中には金貨が見える。[探索時間:3]")),
    (62, TableItem::Text("[描写]:静謐な部屋だ。中央には泉があり、清らかな空気を放っている。泉のそばにはキノコが生えている。慎重に食べた方がいいだろう。[予測]:キノコは魔法のキノコで、何かしらの効果が期待されるが、キノコの魔法成分を受け止める精神力が必要だ。また、泉の中には金貨が見える。[探索時間:3]")),
    (63, TableItem::Text("[描写]:神聖な雰囲気の漂う部屋だ。中央には泉があり、清らかな空気を放っている。とりあえず飲んでみるべきだろう。[予測]:泉の水には何らかの効果が期待できそうだが、もしもの時のために体力があった方がいいだろう。また、泉の中には〈神聖石〉が見える。[探索時間:3]")),
    (64, TableItem::Text("[描写]:神聖な雰囲気の漂う部屋だ。中央には泉があり、清らかな空気を放っている。とりあえず飲んでみるべきだろう。[予測]:泉の水には何らかの効果が期待できそうだが、もしもの時のために体力があった方がいいだろう。また、泉の中には〈神聖石〉が見える。[探索時間:3]")),
    (65, TableItem::Text("[描写]:静謐な部屋だ。中央には泉があり、清らかな空気を放っている。泉の中には何かあるかも知れない。[予測]:泉の中には何かが潜んでいるかもしれない。俊敏に対応できるように注意するべきだろう。また、泉の中には薬瓶が見える。[探索時間:3]")),
    (66, TableItem::Text("[描写]:静謐な部屋だ。中央には泉があり、清らかな空気を放っている。泉の中には何かあるかも知れない。[予測]:泉の中には何かが潜んでいるかもしれない。俊敏に対応できるように注意するべきだろう。また、泉の中には薬瓶が見える。[探索時間:3]")),
];

/// Ruby `TABLES["RET"]`。`D66SortType::NO_SORT` で振る。
static RET: D66Table = D66Table::new("ランダムイベント", D66SortType::NoSort, RET_ITEMS);

/// Ruby `TABLES["AK1T"]`（1D6）。
static AK1T: Table = Table::from_dice(
    "[神器:近接]表A☆1",
    1,
    6,
    &[
        "フレイムソード P.82",
        "サンダースピア P.82",
        "ディフェンダー P.82",
        "ビッグロック P.82",
        "ブラックジャック P.82",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AK2T"]`（1D6）。
static AK2T: Table = Table::from_dice(
    "[神器:近接]表A☆2",
    1,
    6,
    &[
        "イチモンジブレード P.82",
        "レオソード P.82",
        "ブラッドアックス P.82",
        "ウッドバスター P.82",
        "クラブライブ P.82",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AK3T"]`（1D6）。
static AK3T: Table = Table::from_dice(
    "[神器:近接]表A☆3",
    1,
    6,
    &[
        "ソニックブレード P.83",
        "アブソリュートゼロ P.83",
        "ブライトアックス P.83",
        "ブレスメイス P.83",
        "レッドソード P.83",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AK4T"]`（1D6）。
static AK4T: Table = Table::from_dice(
    "[神器:近接]表A☆4",
    1,
    6,
    &[
        "ディヴァインブレード P.83",
        "ゴリラソード P.83",
        "ジャホコ P.83",
        "ゴローマサムネ P.83",
        "ドンキードンキ P.83",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AK5T"]`（1D1）。
static AK5T: Table = Table::from_dice("[神器:近接]表A☆5", 1, 1, &["カタストロフ P.83"]);

/// Ruby `TABLES["AS1T"]`（1D6）。
static AS1T: Table = Table::from_dice(
    "[神器:射撃]表A☆1",
    1,
    6,
    &[
        "ライトボウ P.84",
        "ウィンドブレイカー P.84",
        "マシンクロスボウ P.84",
        "マッハボウ P.84",
        "シャープチャクラム P.84",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AS2T"]`（1D6）。
static AS2T: Table = Table::from_dice(
    "[神器:射撃]表A☆2",
    1,
    6,
    &[
        "ラストシューター P.84",
        "マグネットボウ P.84",
        "ビッグブーメラン P.84",
        "ホーミングシューター P.84",
        "アサシンチャクラム P.84",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AS3T"]`（1D6）。
static AS3T: Table = Table::from_dice(
    "[神器:射撃]表A☆3",
    1,
    6,
    &[
        "パワーライフル P.85",
        "フレイムガン P.85",
        "エレクトロスター P.85",
        "ナパームシューター P.85",
        "ラインヒーラー P.85",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AS4T"]`（1D6）。
static AS4T: Table = Table::from_dice(
    "[神器:射撃]表A☆4",
    1,
    6,
    &[
        "ストーカーボウ P.85",
        "ビームチャクラム P.85",
        "アストラルボウ P.85",
        "フォーチュンガン P.85",
        "ウォークライボウ P.85",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AS5T"]`（1D1）。
static AS5T: Table = Table::from_dice("[神器:射撃]表A☆5", 1, 1, &["オートリピーター P.85"]);

/// Ruby `TABLES["AM1T"]`（1D6）。
static AM1T: Table = Table::from_dice(
    "[神器:魔法]表A☆1",
    1,
    6,
    &[
        "スカーレットワンド P.86",
        "クラウドスタッフ P.86",
        "アイスジュエル P.86",
        "パワーワンド P.86",
        "ジーニアスブック P.86",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AM2T"]`（1D6）。
static AM2T: Table = Table::from_dice(
    "[神器:魔法]表A☆2",
    1,
    6,
    &[
        "ヘルアポカリプス P.86",
        "シャーマニックスカル P.86",
        "カーズタスク P.86",
        "バリアロッド P.86",
        "ホーリーベル P.86",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AM3T"]`（1D6）。
static AM3T: Table = Table::from_dice(
    "[神器:魔法]表A☆3",
    1,
    6,
    &[
        "オーシャンワンド P.87",
        "ダーククラウド P.87",
        "ワイズマン P.87",
        "エンシェントワンド P.87",
        "ゴッドゴブレット P.87",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AM4T"]`（1D6）。
static AM4T: Table = Table::from_dice(
    "[神器:魔法]表A☆4",
    1,
    6,
    &[
        "テンペストロッド P.87",
        "セイバースタッフ P.87",
        "ダークスカッター P.87",
        "ルーラーズレイ P.87",
        "デモンズホーン P.87",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AM5T"]`（1D1）。
static AM5T: Table = Table::from_dice("[神器:魔法]表A☆5", 1, 1, &["スターコンプレッサ P.87"]);

/// Ruby `TABLES["AY1T"]`（1D6）。
static AY1T: Table = Table::from_dice(
    "[神器:鎧]表A☆1",
    1,
    6,
    &[
        "ハードアーマー P.88",
        "シーヴスローブ P.88",
        "マジックアーマー P.88",
        "ナイトアーマー P.88",
        "フェザーキルト P.88",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AY2T"]`（1D6）。
static AY2T: Table = Table::from_dice(
    "[神器:鎧]表A☆2",
    1,
    6,
    &[
        "トワイライトアーマー P.88",
        "ヒューマンガーター P.88",
        "ソルトメイル P.88",
        "ライトムーヴ P.88",
        "キャプテンアーマー P.88",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AY3T"]`（1D6）。
static AY3T: Table = Table::from_dice(
    "[神器:鎧]表A☆3",
    1,
    6,
    &[
        "ジェットアーマー P.89",
        "ドラゴンアーマー P.89",
        "ホーリーケープ P.89",
        "ビーストエイジ P.89",
        "クロスフォートレス P.89",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AY4T"]`（1D6）。
static AY4T: Table = Table::from_dice(
    "[神器:鎧]表A☆4",
    1,
    6,
    &[
        "フェニックスアーマー P.89",
        "マジックゲイナー P.89",
        "インシュランスメイル P.89",
        "ジャイアントメイル P.89",
        "バンブーメイル P.89",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AY5T"]`（1D1）。
static AY5T: Table = Table::from_dice("[神器:鎧]表A☆5", 1, 1, &["ディヴァインクロス P.89"]);

/// Ruby `TABLES["AT1T"]`（1D6）。
static AT1T: Table = Table::from_dice(
    "[神器:盾]表A☆1",
    1,
    6,
    &[
        "スパイクシールド P.90",
        "ウェーブシールド P.90",
        "レアメタルシールド P.90",
        "バインドシールド P.90",
        "ゲイルシールド P.90",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AT2T"]`（1D6）。
static AT2T: Table = Table::from_dice(
    "[神器:盾]表A☆2",
    1,
    6,
    &[
        "アースシールド P.90",
        "フレイムレジスター P.90",
        "ポラリゼーショナー P.90",
        "グレートシールド P.90",
        "センサーシールド P.90",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AT3T"]`（1D6）。
static AT3T: Table = Table::from_dice(
    "[神器:盾]表A☆3",
    1,
    6,
    &[
        "ヒールボード P.91",
        "フレッシュガーダー P.91",
        "タフシールド P.91",
        "ワイズモノリス P.91",
        "エールポンポン P.91",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AT4T"]`（1D6）。
static AT4T: Table = Table::from_dice(
    "[神器:盾]表A☆4",
    1,
    6,
    &[
        "ガッデスミラー P.91",
        "ラックエムブレム P.91",
        "バトルドレイナー P.91",
        "グランドソーサー P.91",
        "オートドール P.91",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AT5T"]`（1D1）。
static AT5T: Table = Table::from_dice("[神器:盾]表A☆5", 1, 1, &["ダークマター P.91"]);

/// Ruby `TABLES["AA1T"]`（1D6）。
static AA1T: Table = Table::from_dice(
    "[神器:装飾品]表A☆1",
    1,
    6,
    &[
        "エナジーブレス P.92",
        "ホークガントレット P.92",
        "ライトアミュレット P.92",
        "センシングブレス P.92",
        "レジストマント P.92",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["AA2T"]`（1D6）。
static AA2T: Table = Table::from_dice(
    "[神器:装飾品]表A☆2",
    1,
    6,
    &[
        "ドリルブレス P.92",
        "ルーンマント P.92",
        "バランスビット P.92",
        "ベストサングラス P.92",
        "シルバーペンダント P.92",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["AA3T"]`（1D6）。
static AA3T: Table = Table::from_dice(
    "[神器:装飾品]表A☆3",
    1,
    6,
    &[
        "ミスティックマスク P.93",
        "ガードブレス P.93",
        "マジックピアス P.93",
        "ミラージュブレス P.93",
        "キャットフード P.93",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["AA4T"]`（1D6）。
static AA4T: Table = Table::from_dice(
    "[神器:装飾品]表A☆4",
    1,
    6,
    &[
        "ショルダーアーム P.93",
        "ナイトコート P.93",
        "エンジェルバックル P.93",
        "オラクルピアス P.93",
        "センサーリング P.93",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["AA5T"]`（1D1）。
static AA5T: Table = Table::from_dice("[神器:装飾品]表A☆5", 1, 1, &["ノーブルスフィア P.93"]);

/// Ruby `TABLES["BK1T"]`（1D6）。
static BK1T: Table = Table::from_dice(
    "[神器:近接]表B☆1",
    1,
    6,
    &[
        "マンイーター P.94",
        "アイスメイス P.94",
        "エクステンダー P.94",
        "スラッグカッター P.94",
        "フィアーギロチン P.94",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BK2T"]`（1D6）。
static BK2T: Table = Table::from_dice(
    "[神器:近接]表B☆2",
    1,
    6,
    &[
        "ツインランサー P.94",
        "メディシンランス P.94",
        "レイスラッシャー P.94",
        "シザーソード P.94",
        "エナジーヨーヨー P.94",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BK3T"]`（1D6）。
static BK3T: Table = Table::from_dice(
    "[神器:近接]表B☆3",
    1,
    6,
    &[
        "ラディオランサー P.95",
        "マシンキラー P.95",
        "ニンジャハンマー P.95",
        "ストームブレード P.95",
        "オートバランサー P.95",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BK4T"]`（1D6）。
static BK4T: Table = Table::from_dice(
    "[神器:近接]表B☆4",
    1,
    6,
    &[
        "ラムダセイバー P.95",
        "エクスカリアックス P.95",
        "ゴッドロック P.95",
        "バスターメイス P.95",
        "グルメランサー P.95",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BK5T"]`（1D1）。
static BK5T: Table = Table::from_dice("[神器:近接]表B☆5", 1, 1, &["カタストロフ P.95"]);

/// Ruby `TABLES["BS1T"]`（1D6）。
static BS1T: Table = Table::from_dice(
    "[神器:射撃]表B☆1",
    1,
    6,
    &[
        "ホーリースリング P.96",
        "ハンターボウ P.96",
        "ミラージュダーツ P.96",
        "ベストダーツ P.96",
        "エイミングボウ P.96",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BS2T"]`（1D6）。
static BS2T: Table = Table::from_dice(
    "[神器:射撃]表B☆2",
    1,
    6,
    &[
        "ラビットボウ P.96",
        "スティングダーツ P.96",
        "ビジネスカード P.96",
        "エクスプロードボウ P.96",
        "インパクトエアガン P.96",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BS3T"]`（1D6）。
static BS3T: Table = Table::from_dice(
    "[神器:射撃]表B☆3",
    1,
    6,
    &[
        "オーガシューター P.97",
        "マーダーボウガン P.97",
        "メンタルドレイナー P.97",
        "スタナーガン P.97",
        "ニードルシューター P.97",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BS4T"]`（1D6）。
static BS4T: Table = Table::from_dice(
    "[神器:射撃]表B☆4",
    1,
    6,
    &[
        "ガーディアンボウ P.97",
        "フォトンブーメラン P.97",
        "ダンスマシンガン P.97",
        "スリリングスリング P.97",
        "アルケミストガン P.97",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BS5T"]`（1D1）。
static BS5T: Table = Table::from_dice("[神器:射撃]表B☆5", 1, 1, &["オートリピーター P.97"]);

/// Ruby `TABLES["BM1T"]`（1D6）。
static BM1T: Table = Table::from_dice(
    "[神器:魔法]表B☆1",
    1,
    6,
    &[
        "ソニックスタッフ P.98",
        "ホーリーワンド P.98",
        "ライフメイカー P.98",
        "ゴリラワンド P.98",
        "コンセントレイター P.98",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BM2T"]`（1D6）。
static BM2T: Table = Table::from_dice(
    "[神器:魔法]表B☆2",
    1,
    6,
    &[
        "ポイズンワンド P.98",
        "キーンタロー P.98",
        "マジックビースト P.98",
        "オープニングスタッフ P.98",
        "ディヴァイドジュエル P.98",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BM3T"]`（1D6）。
static BM3T: Table = Table::from_dice(
    "[神器:魔法]表B☆3",
    1,
    6,
    &[
        "サイクロンアイ P.99",
        "クリムゾンオーブ P.99",
        "ルーインスタッフ P.99",
        "アジリティオーブ P.99",
        "キングステッキ P.99",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BM4T"]`（1D6）。
static BM4T: Table = Table::from_dice(
    "[神器:魔法]表B☆4",
    1,
    6,
    &[
        "ディヴァインドラム P.99",
        "エンリッチオーブ P.99",
        "ヒールアミュレット P.99",
        "マナインヘイラー P.99",
        "ライトライト P.99",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BM5T"]`（1D1）。
static BM5T: Table = Table::from_dice("[神器:魔法]表B☆5", 1, 1, &["スターコンプレッサ P.99"]);

/// Ruby `TABLES["BY1T"]`（1D6）。
static BY1T: Table = Table::from_dice(
    "[神器:鎧]表B☆1",
    1,
    6,
    &[
        "ダンボールボックス P.100",
        "マーチャントクロス P.100",
        "ウィザードローブ P.100",
        "バランスアーマー P.100",
        "レジストローブ P.100",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BY2T"]`（1D6）。
static BY2T: Table = Table::from_dice(
    "[神器:鎧]表B☆2",
    1,
    6,
    &[
        "スカウトローブ P.100",
        "ランプアーマー P.100",
        "フィールドローブ P.100",
        "ケミカルアーマー P.100",
        "ビジネススーツ P.100",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BY3T"]`（1D6）。
static BY3T: Table = Table::from_dice(
    "[神器:鎧]表B☆3",
    1,
    6,
    &[
        "セージアーマー P.101",
        "トータスアーマー P.101",
        "ブレスブレスト P.101",
        "ラビットスーツ P.101",
        "サクリファイスメイル P.101",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BY4T"]`（1D6）。
static BY4T: Table = Table::from_dice(
    "[神器:鎧]表B☆4",
    1,
    6,
    &[
        "ファルコンアーマー P.101",
        "ガッツアーマー P.101",
        "ゴットカーボン P.101",
        "パワードアーマー P.101",
        "グラビティガード P.101",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BY5T"]`（1D1）。
static BY5T: Table = Table::from_dice("[神器:鎧]表B☆5", 1, 1, &["ディヴァインクロス P.101"]);

/// Ruby `TABLES["BT1T"]`（1D6）。
static BT1T: Table = Table::from_dice(
    "[神器:盾]表B☆1",
    1,
    6,
    &[
        "ルーンボード P.102",
        "ラブリーペット P.102",
        "ハリケーンシールド P.102",
        "ジュークシールド P.102",
        "ミラーシールド P.102",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BT2T"]`（1D6）。
static BT2T: Table = Table::from_dice(
    "[神器:盾]表B☆2",
    1,
    6,
    &[
        "フリーズカウンター P.102",
        "ゲイルガーダー P.102",
        "フォースバックラー P.102",
        "ロードシールド P.102",
        "ビッグドーナツ P.102",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BT3T"]`（1D6）。
static BT3T: Table = Table::from_dice(
    "[神器:盾]表B☆3",
    1,
    6,
    &[
        "ナイスボード P.103",
        "シニスターシールド P.103",
        "ノーブルソーサー P.103",
        "ミサイルシールド P.103",
        "ゴリラシールド P.103",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BT4T"]`（1D6）。
static BT4T: Table = Table::from_dice(
    "[神器:盾]表B☆4",
    1,
    6,
    &[
        "ビームバックラー P.103",
        "オールドシールド P.103",
        "サニーガード P.103",
        "ゴッドクロック P.103",
        "ミストシールド P.103",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BT5T"]`（1D1）。
static BT5T: Table = Table::from_dice("[神器:盾]表B☆5", 1, 1, &["ダークマター P.103"]);

/// Ruby `TABLES["BA1T"]`（1D6）。
static BA1T: Table = Table::from_dice(
    "[神器:装飾品]表B☆1",
    1,
    6,
    &[
        "ラックデビルアイ P.104",
        "ビームリング P.104",
        "ダッシューズ P.104",
        "エレガンダイ P.104",
        "スキルブック P.104",
        "ランクアップ？([神聖石]10個)",
    ],
);

/// Ruby `TABLES["BA2T"]`（1D6）。
static BA2T: Table = Table::from_dice(
    "[神器:装飾品]表B☆2",
    1,
    6,
    &[
        "ブレスアイドル P.104",
        "アロマピアス P.104",
        "スピードリング P.104",
        "ハイホーリーシンボル P.104",
        "ノーブルマント P.104",
        "ランクアップ？([神聖石]20個)",
    ],
);

/// Ruby `TABLES["BA3T"]`（1D6）。
static BA3T: Table = Table::from_dice(
    "[神器:装飾品]表B☆3",
    1,
    6,
    &[
        "シュルダーバード P.105",
        "フェアリードール P.105",
        "フレッシュブレス P.105",
        "ラビットフット P.105",
        "ファストブレス P.105",
        "ランクアップ？([神聖石]30個)",
    ],
);

/// Ruby `TABLES["BA4T"]`（1D6）。
static BA4T: Table = Table::from_dice(
    "[神器:装飾品]表B☆4",
    1,
    6,
    &[
        "ヒールグラス P.105",
        "テレポートブレス P.105",
        "ブラッドチェンジャー P.105",
        "ルーンブレス P.105",
        "コンディショナー P.105",
        "ランクアップ？([神聖石]40個)",
    ],
);

/// Ruby `TABLES["BA5T"]`（1D1）。
static BA5T: Table = Table::from_dice("[神器:装飾品]表B☆5", 1, 1, &["ノーブルスフィア P.105"]);
/// Ruby `TABLES`（コマンド → 表）。`RET` は `D66Table`、神器表は `Table`。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("RET", &RET),
    ("AK1T", &AK1T),
    ("AK2T", &AK2T),
    ("AK3T", &AK3T),
    ("AK4T", &AK4T),
    ("AK5T", &AK5T),
    ("AS1T", &AS1T),
    ("AS2T", &AS2T),
    ("AS3T", &AS3T),
    ("AS4T", &AS4T),
    ("AS5T", &AS5T),
    ("AM1T", &AM1T),
    ("AM2T", &AM2T),
    ("AM3T", &AM3T),
    ("AM4T", &AM4T),
    ("AM5T", &AM5T),
    ("AY1T", &AY1T),
    ("AY2T", &AY2T),
    ("AY3T", &AY3T),
    ("AY4T", &AY4T),
    ("AY5T", &AY5T),
    ("AT1T", &AT1T),
    ("AT2T", &AT2T),
    ("AT3T", &AT3T),
    ("AT4T", &AT4T),
    ("AT5T", &AT5T),
    ("AA1T", &AA1T),
    ("AA2T", &AA2T),
    ("AA3T", &AA3T),
    ("AA4T", &AA4T),
    ("AA5T", &AA5T),
    ("BK1T", &BK1T),
    ("BK2T", &BK2T),
    ("BK3T", &BK3T),
    ("BK4T", &BK4T),
    ("BK5T", &BK5T),
    ("BS1T", &BS1T),
    ("BS2T", &BS2T),
    ("BS3T", &BS3T),
    ("BS4T", &BS4T),
    ("BS5T", &BS5T),
    ("BM1T", &BM1T),
    ("BM2T", &BM2T),
    ("BM3T", &BM3T),
    ("BM4T", &BM4T),
    ("BM5T", &BM5T),
    ("BY1T", &BY1T),
    ("BY2T", &BY2T),
    ("BY3T", &BY3T),
    ("BY4T", &BY4T),
    ("BY5T", &BY5T),
    ("BT1T", &BT1T),
    ("BT2T", &BT2T),
    ("BT3T", &BT3T),
    ("BT4T", &BT4T),
    ("BT5T", &BT5T),
    ("BA1T", &BA1T),
    ("BA2T", &BA2T),
    ("BA3T", &BA3T),
    ("BA4T", &BA4T),
    ("BA5T", &BA5T),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;

    fn eval(input: &str, rands: &[(i64, i64)]) -> Option<EvalResult> {
        let mut src = SeededRandomizer::new(rands.to_vec());
        let result =
            eval_command(&GameSystemId::new("DivineCharger"), input, &mut src).expect("eval");
        assert!(src.is_empty(), "unconsumed rands");
        result
    }

    #[test]
    fn reverse_check_strips_commas_and_flips_dice() {
        let r = eval("REV[1,2,3]>=7", &[]).expect("result");
        assert_eq!(r.text, "(REV[123]>=7) ＞ 4,5,6 ＞ 達成値15 ＞ 成功");
        assert!(r.success && !r.failure && !r.critical && !r.fumble);
    }

    #[test]
    fn reverse_check_critical_and_fumble() {
        // 1,1 → 6,6 でクリティカル（目標値 "?" でも判定する）
        let r = eval("REV[11]>=?", &[]).expect("result");
        assert_eq!(r.text, "(REV[11]>=?) ＞ 6,6 ＞ 達成値12 ＞ クリティカル");
        assert!(r.critical && r.success);

        // 6,6 → 1,1 でファンブル（達成値は0になる）
        let r = eval("REV[66]>=3", &[]).expect("result");
        assert_eq!(
            r.text,
            "(REV[66]>=3) ＞ 1,1 ＞ 達成値0 ＞ ファンブル([神聖石]5個)"
        );
        assert!(r.fumble && r.failure);
    }

    #[test]
    fn reverse_check_ignores_digits_out_of_range() {
        // Ruby: filter { |v| 1 <= v && v <= 6 } で 0 と 7〜9 は捨てられる
        let r = eval("REV[0789]>=?", &[]).expect("result");
        assert_eq!(r.text, "(REV[0789]>=?) ＞  ＞ 達成値0");
        assert!(!r.success && !r.failure);
    }

    #[test]
    fn action_check_with_secret_prefix() {
        let r = eval("S3DC>=7", &[(4, 6), (6, 6), (5, 6)]).expect("result");
        assert_eq!(r.text, "(3DC>=7) ＞ 4,5,6 ＞ 達成値15 ＞ 成功");
        assert!(r.secret && r.success);
    }

    #[test]
    fn unknown_table_command_is_nil() {
        assert!(eval("AK6T", &[]).is_none());
        assert!(eval("CK1T", &[]).is_none());
    }

    /// `test/data/DivineCharger.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "DivineCharger",
            "DivineCharger.toml",
            71,
        );
    }
}
