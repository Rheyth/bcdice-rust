//! P4で手書き移植した `lib/bcdice/game_system/Warhammer.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Warhammer#result_1d100` / `#result_1d100_text`（1D100の成功度・失敗度判定）
//! - `#eval_game_system_specific_command` → `getAttackResult`（`WHx@t` 命中判定）と
//!   `getCriticalResult`（`WH<部位><クリティカル値>` クリティカル表）
//! - `#wh_atpos` / `#get_wh_atpos_message`（命中部位表）

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::floor_div;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::{sat_i64, Randomizer};
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// 表データ（Ruby `getCriticalResult` / `wh_atpos` のローカル配列）
// ---------------------------------------------------------------------------

/// 命中部位表。Ruby は `['二足', 15, '頭部', 35, '右腕', …]` と1次元配列で持つ。
struct HitLocation {
    /// 種別名（Ruby `pos_i[0]`）
    name: &'static str,
    /// `(上限値, 部位名)` の並び（Ruby `pos_i[i]`, `pos_i[i + 1]`）
    entries: &'static [(i64, &'static str)],
}

/// Ruby `whh`（頭部のクリティカル効果）。
static WHH: [&str; 10] = [
    "01:打撃で状況が把握出来なくなる。次ターンは1回の半アクションしか行なえない。",
    "02:耳を強打された為、耳鳴りが酷く目眩がする。1Rに渡って一切のアクションを行なえない。",
    "03:打撃が頭皮を酷く傷つけた。【武器技術度】に-10%。治療を受けるまで継続。",
    "04:鎧が損傷し当該部位のAP-1。修理するには(職能:鎧鍛冶)テスト。鎧を着けていないなら1Rの間アクションを行なえない。",
    "05:転んで倒れ、頭がくらくらする。1Rに渡ってあらゆるテストに-30で、立ち上がるには起立アクションが必要。",
    "06:1d10R気絶。",
    "07:1d10分気絶。以後CTはサドンデス。",
    "08:顔がずたずたになって倒れ、以後無防備状態。治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。【頑強】テストに失敗すると片方の視力を失う。",
    "09:凄まじい打撃により頭蓋骨が粉砕される。死は瞬時に訪れる。",
    "10:死亡する。いかに盛大に出血し、どのような死に様を見せたのかを説明してもよい。",
];

/// Ruby `wha`（腕部のクリティカル効果）。
static WHA: [&str; 10] = [
    "01:手に握っていたものを落とす。盾はくくりつけられている為、影響なし。",
    "02:打撃で腕が痺れ、1Rの間使えなくなる。",
    "03:手の機能が失われ、治療を受けるまで回復できない。手で握っていたもの(盾を除く)は落ちる。",
    "04:鎧が損傷する。当該部位のAP-1。修理するには(職能:鎧鍛冶)テスト。鎧を着けていないなら腕が痺れ、1Rの間使えなくなる。",
    "05:腕の機能が失われ、治療を受けるまで回復できない。手で握っていたもの(盾を除く)は落ちる。",
    "06:腕が砕かれる。手で握っていたもの(盾を除く)は落ちる。出血がひどく、治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。",
    "07:手首から先が血まみれの残骸と化す。手で握っていたもの(盾を除く)は落ちる。出血がひどく、治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。【頑健】テストに失敗すると手の機能を失う。",
    "08:腕は血まみれの肉塊がぶら下がっている状態になる。手で握っていたもの(盾を除く)は落ちる。治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。【頑健】テストに失敗すると肘から先の機能を失う。",
    "09:大動脈に傷が及んだ。コンマ数秒の内に損傷した肩から血を噴出して倒れる。ショックと失血により、ほぼ即死する。",
    "10:死亡する。いかに盛大に出血し、どのような死に様を見せたのかを説明してもよい。",
];

/// Ruby `whb`（胴体のクリティカル効果）。
static WHB: [&str; 10] = [
    "01:打撃で息が詰まる。1Rの間、キャラクターの全てのテストや攻撃に-20%。",
    "02:股間への一撃。苦痛のあまり、1Rに渡って一切のアクションを行なえない。",
    "03:打撃で肋骨がぐちゃぐちゃになる。以後治療を受けるまでの間、【武器技術度】に-10%。",
    "04:鎧が損傷する。当該部位のAP-1。修理するには(職能:鎧鍛冶)テスト。鎧を着けていないなら股間への一撃、1Rに渡って一切のアクションを行なえない。",
    "05:転んで倒れ、息が詰まって悶絶する。1Rに渡ってあらゆるテストに-30の修正、立ち上がるには起立アクションが必要。",
    "06:1d10R気絶。",
    "07:ひどい内出血が起こり、無防備状態。出血がひどく、治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。",
    "08:脊髄が粉砕されて倒れ、以後治療を受けるまで無防備状態。以後CTはサドンデスを適用。【頑強】テストに失敗すると腰から下が不随になる。",
    "09:凄まじい打撃により複数の臓器が破裂し、死は数秒のうちに訪れる。",
    "10:死亡する。いかに盛大に出血し、どのような死に様を見せたのかを説明してもよい。",
];

/// Ruby `whl`（脚部のクリティカル効果）。
static WHL: [&str; 10] = [
    "01:よろめく。次のターン、1回の半アクションしか行なえない。",
    "02:脚が痺れる。1Rに渡って【移動】は半減し、脚に関連する【敏捷】テストに-20%。回避が出来なくなる。",
    "03:脚の機能が失われ、治療を受けるまで回復しない。【移動】は半減し、脚に関連する【敏捷】テストに-20%。回避が出来なくなる。",
    "04:鎧が損傷する。当該部位のAP-1。修理するには(職能:鎧鍛冶)テスト。鎧を着けていないなら脚が痺れる、1Rに渡って【移動】は半減し、脚に関連する【敏捷】テストに-20%、回避不可になる。",
    "05:転んで倒れ、頭がくらくらする。1Rに渡ってあらゆるテストに-30の修正、立ち上がるには起立アクションが必要。",
    "06:脚が砕かれ、無防備状態。出血がひどく、治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。",
    "07:脚は血まみれの残骸と化し、無防備状態になる。治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。【頑強】テストに失敗すると足首から先を失う。",
    "08:脚は血まみれの肉塊がぶらさがっている状態。以後無防備状態。治療を受けるまで毎Rの被害者のターン開始時に20%で死亡。以後CTはサドンデスを適用。【頑強】テストに失敗すると膝から下を失う。",
    "09:大動脈に傷が及ぶ。コンマ数秒の内に脚の残骸から血を噴出して倒れ、ショックと出血で死は瞬時に訪れる。",
    "10:死亡する。いかに盛大に出血し、どのような死に様を見せたのかを説明してもよい。",
];

/// Ruby `whw`（翼部のクリティカル効果）。
static WHW: [&str; 10] = [
    "01:軽打。1ラウンドに渡って、あらゆるテストに-10％。",
    "02:かすり傷。+10％の【敏捷】テストを行い、失敗なら直ちに高度を1段階失う。地上にいるクリーチャーは、次のターンには飛び立てない。",
    "03:損傷する。【飛行移動力】が2点低下する。-10％の【敏捷】テストを行い、失敗なら直ちに高度を1段階失う。地上にいるクリーチャーは、次のターンには飛び立てない。",
    "04:酷く損傷する。【飛行移動力】が4点低下する。-30％の【敏捷】テストを行い、失敗なら直ちに高度を1段階失う。地上にいるクリーチャーは、1d10ターンが経過するまで飛び立てない。",
    "05:翼が使えなくなる。【飛行移動力】が0に低下する。飛行中のものは落下し、高度に応じたダメージを受ける。地上にいるクリーチャーは、怪我が癒えるまで飛び立てない。",
    "06:翼の付け根に傷が開く。【飛行移動力】が0に低下する。飛行中のものは落下し、高度に応じたダメージを受ける。地上にいるクリーチャーは、怪我が癒えるまで飛び立てない。治療を受けるまで毎R被害者のターン開始時に20％の確率で死亡。以後CTはサドンデスを適用。",
    "07:翼は血まみれの残骸と化し、無防備状態になる。【飛行移動力】が0に低下する。飛行中のものは落下し、高度に応じたダメージを受ける。地上にいるクリーチャーは、怪我が癒えるまで飛び立てない。治療を受けるまで毎R被害者のターン開始時に20％の確率で死亡。以後CTはサドンデスを適用。【頑強】テストに失敗すると飛行能力を失う。",
    "08:翼が千切れてバラバラになり、無防備状態になる。【飛行移動力】が0に低下する。飛行中のものは落下し、高度に応じたダメージを受ける。地上にいるクリーチャーは、怪我が癒えるまで飛び立てない。治療を受けるまで毎R被害者のターン開始時に20％の確率で死亡。以後CTはサドンデスを適用。飛行能力を失う。",
    "09:大動脈が切断された。コンマ数秒の内に血を噴き上げてくずおれる、ショックと出血で死は瞬時に訪れる。",
    "10:死亡する。いかに盛大に出血し、どのような死に様を見せたのかを説明してもよい。",
];

/// Ruby `criticalTable`。10行×10列（行=1D100の十の位、列=クリティカル値1〜10）。
static CRITICAL_TABLE: [usize; 100] = [
    5, 7, 9, 10, 10, 10, 10, 10, 10, 10, // 01-10
    5, 6, 8, 9, 10, 10, 10, 10, 10, 10, // 11-20
    4, 6, 8, 9, 9, 10, 10, 10, 10, 10, // 21-30
    4, 5, 7, 8, 9, 9, 10, 10, 10, 10, // 31-40
    3, 5, 7, 8, 8, 9, 9, 10, 10, 10, // 41-50
    3, 4, 6, 7, 8, 8, 9, 9, 10, 10, // 51-60
    2, 4, 6, 7, 7, 8, 8, 9, 9, 10, // 61-70
    2, 3, 5, 6, 7, 7, 8, 8, 9, 9, // 71-80
    1, 3, 5, 6, 6, 7, 7, 8, 8, 9, // 81-90
    1, 2, 4, 5, 6, 6, 7, 7, 8, 8, // 91-00
];

/// Ruby `pos_2l`。
static POS_2L: HitLocation = HitLocation {
    name: "二足",
    entries: &[
        (15, "頭部"),
        (35, "右腕"),
        (55, "左腕"),
        (80, "胴体"),
        (90, "右脚"),
        (100, "左脚"),
    ],
};

/// Ruby `pos_2lw`。
static POS_2LW: HitLocation = HitLocation {
    name: "有翼二足",
    entries: &[
        (15, "頭部"),
        (25, "右腕"),
        (35, "左腕"),
        (45, "右翼"),
        (55, "左翼"),
        (80, "胴体"),
        (90, "右脚"),
        (100, "左脚"),
    ],
};

/// Ruby `pos_4l`。
static POS_4L: HitLocation = HitLocation {
    name: "四足",
    entries: &[
        (15, "頭部"),
        (60, "胴体"),
        (70, "右前脚"),
        (80, "左前脚"),
        (90, "右後脚"),
        (100, "左後脚"),
    ],
};

/// Ruby `pos_4la`。
static POS_4LA: HitLocation = HitLocation {
    name: "半人四足",
    entries: &[
        (10, "頭部"),
        (20, "右腕"),
        (30, "左腕"),
        (60, "胴体"),
        (70, "右前脚"),
        (80, "左前脚"),
        (90, "右後脚"),
        (100, "左後脚"),
    ],
};

/// Ruby `pos_4lw`。
static POS_4LW: HitLocation = HitLocation {
    name: "有翼四足",
    entries: &[
        (10, "頭部"),
        (20, "右翼"),
        (30, "左翼"),
        (60, "胴体"),
        (70, "右前脚"),
        (80, "左前脚"),
        (90, "右後脚"),
        (100, "左後脚"),
    ],
};

/// Ruby `pos_b`。
static POS_B: HitLocation = HitLocation {
    name: "鳥",
    entries: &[
        (15, "頭部"),
        (35, "右翼"),
        (55, "左翼"),
        (80, "胴体"),
        (90, "右脚"),
        (100, "左脚"),
    ],
};

/// Ruby `wh_pos = [pos_2l, pos_2lw, pos_4l, pos_4la, pos_4lw, pos_b]`。
static WH_POS: [&HitLocation; 6] = [&POS_2L, &POS_2LW, &POS_4L, &POS_4LA, &POS_4LW, &POS_B];

// ---------------------------------------------------------------------------
// 判定
// ---------------------------------------------------------------------------

/// Ruby `Warhammer#result_1d100` のうち `Result` を返す部分。
///
/// `result_1d100`（トレイトのフック）と `getAttackResult` の両方から呼ぶため、
/// 目標値 `"?"`（Ruby の `Result.nothing`）の分岐は呼び出し側に残してある。
fn check_1d100(total: crate::Int, cmp_op: CmpOp, target: crate::Int) -> Option<EvalResult> {
    // Ruby: return nil unless cmp_op == :<=
    if cmp_op != CmpOp::Le {
        return None;
    }

    if total <= target {
        Some(EvalResult::success(format!(
            "成功(成功度{})",
            floor_div(target - total, crate::Int::from(10))
        )))
    } else {
        Some(EvalResult::failure(format!(
            "失敗(失敗度{})",
            floor_div(total - target, crate::Int::from(10))
        )))
    }
}

/// Ruby `Warhammer#result_1d100_text`。
fn result_1d100_text(total: crate::Int, cmp_op: CmpOp, target: crate::Int) -> String {
    match check_1d100(total, cmp_op, target) {
        None => String::new(),
        Some(result) => format!(" ＞ {}", result.text),
    }
}

// ---------------------------------------------------------------------------
// クリティカル表
// ---------------------------------------------------------------------------

/// Ruby `getCriticalResult(string)`。
///
/// 戻り値の `None` は Ruby の `return '1'`（＝`dice_command` が nil に畳む番兵）に対応する。
fn critical_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"WH([HABTLW])(\d+)").expect("valid regex"));

    let Some(caps) = re.captures(command) else {
        return Ok(None);
    };
    let parts_word = &caps[1];
    // Ruby: Regexp.last_match(2).to_i（桁あふれは i64 に飽和させる）
    let critical_value = caps[2].parse::<i64>().unwrap_or(i64::MAX).clamp(1, 10);

    let (parts_name, effects): (&str, &[&str; 10]) = match parts_word {
        "H" => ("頭部", &WHH),
        "A" => ("腕部", &WHA),
        "T" | "B" => ("胴体", &WHB),
        "L" => ("脚部", &WHL),
        // 正規表現の文字クラスが [HABTLW] なので、残るのは W だけ
        _ => ("翼部", &WHW),
    };

    let dice_now = rng.roll_once(100)?;

    // Ruby: crit_no = ((dice_now - 1) / 10).to_i * 10
    let crit_no = sat_i64(&floor_div(
        crate::Int::from(dice_now - 1),
        crate::Int::from(10),
    )) * 10;
    let crit_num = CRITICAL_TABLE[(crit_no + critical_value - 1) as usize];

    let suffix = if crit_num >= 5 {
        "サドンデス×"
    } else {
        "サドンデス○"
    };
    let result_text = format!("{}{}", effects[crit_num - 1], suffix);

    Ok(Some(format!(
        "{parts_name}CT表({dice_now}+{critical_value}) ＞ {result_text}"
    )))
}

// ---------------------------------------------------------------------------
// 命中判定と命中部位表
// ---------------------------------------------------------------------------

/// Ruby `get_wh_atpos_message(pos_i, pos_num)`。
fn wh_atpos_message(pos: &HitLocation, pos_num: i64) -> String {
    let mut output = format!(" {}:", pos.name);
    for (threshold, label) in pos.entries {
        if pos_num <= *threshold {
            output.push_str(label);
            break;
        }
    }
    output
}

/// Ruby `wh_atpos(pos_num, pos_type)`。
///
/// `pos_type` は `getAttackResult` が切り出した `@` 以降（例: `"@4W"`）。
/// Ruby の `case pos_type when /@(2W|W2)/i` は非アンカーの検索なので、
/// ここでも部分一致で判定する。
fn wh_atpos(pos_num: i64, pos_type: &str) -> String {
    // Ruby: pos_t = 0 のまま「二足」。-1 は全種別を並べる。
    let mut pos_t: i64 = 0;
    if !pos_type.is_empty() {
        pos_t = if contains_any(pos_type, &["@2W", "@W2"]) {
            1
        } else if contains_any(pos_type, &["@4W", "@W4"]) {
            4
        } else if contains_any(pos_type, &["@4H", "@H4"]) {
            3
        } else if pos_type.contains("@4") {
            2
        } else if pos_type.contains("@W") {
            5
        } else if contains_any(pos_type, &["@2H", "@H2", "@2"]) {
            0
        } else {
            -1
        };
    }

    if pos_t < 0 {
        return WH_POS
            .iter()
            .map(|pos| wh_atpos_message(pos, pos_num))
            .collect();
    }
    wh_atpos_message(WH_POS[pos_t as usize], pos_num)
}

/// 与えられた部分文字列のいずれかを含むか（Ruby の `/(A|B)/` 相当）。
fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Ruby `getAttackResult(string)`。
///
/// 戻り値の `None` は Ruby の `return '1'` に対応する。
fn attack_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"WH(\d+)").expect("valid regex"));

    // Ruby: if /(.+)(@.*)/ =~ string
    // `.+` は貪欲なので **最後の** `@` で分かれる。`.+` は1文字以上を要求するので、
    // 先頭が `@` の場合（前半が空）は分割しない。
    let (body, pos_type) = match command.rsplit_once('@') {
        Some((head, tail)) if !head.is_empty() => (head, format!("@{tail}")),
        _ => (command, String::new()),
    };

    let Some(caps) = re.captures(body) else {
        return Ok(None);
    };
    // Ruby: Regexp.last_match(1).to_i（桁あふれは i64 に飽和させる）
    let diff = caps[1].parse::<i64>().unwrap_or(i64::MAX);

    let total_n = rng.roll_once(100)?;

    let mut output = format!("({body}) ＞ {total_n}");
    output.push_str(&result_1d100_text(
        crate::Int::from(total_n),
        CmpOp::Le,
        crate::Int::from(diff),
    ));

    // Ruby: pos_num = (total_n % 10) * 10 + (total_n / 10).to_i
    let mut pos_num =
        (total_n % 10) * 10 + sat_i64(&floor_div(crate::Int::from(total_n), crate::Int::from(10)));
    if total_n >= 100 {
        pos_num = 100;
    }

    if total_n <= diff {
        output.push_str(&wh_atpos(pos_num, &pos_type));
    }

    Ok(Some(output))
}

/// Ruby `BCDice::GameSystem::Warhammer`（ID: `Warhammer`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warhammer;

impl GameSystem for Warhammer {
    fn id(&self) -> &'static str {
        "Warhammer"
    }

    fn name(&self) -> &'static str {
        "ウォーハンマー"
    }

    fn sort_key(&self) -> &'static str {
        "うおおはんまあ"
    }

    fn help_message(&self) -> &'static str {
        r#"・クリティカル表(whHxx/whAxx/whBxx/whLxx)
　"WH部位 クリティカル値"の形で指定します。部位は「H(頭部)」「A(腕)」「B(胴体)」「L(足)」の４カ所です。
　例）whH10 whA5 WHL4
・命中判定(WHx@t)
　"WH(命中値)@(種別)"の形で指定します。
　種別は脚の数を数字、翼が付いているものは「W」、手が付いているものは「H」で書きます。
　「2H(二足)」「2W(有翼二足)」「4(四足)」「4H(半人四足)」「4W(有翼四足)」「W(鳥類)」となります。
　命中判定を行って、当たれば部位も表示します。
　なお、種別指定を省略すると「二足」、「@」だけにすると全種別の命中部位を表示します。(コマンドを忘れた時の対応です)
　例）wh60　　wh43@4W　　WH65@
"#
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["WH"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> crate::enums::RoundType {
        crate::enums::RoundType::Ceil
    }

    /// Ruby `Warhammer#result_1d100`。
    fn result_1d100(
        &self,
        total: crate::Int,
        _dice_total: i64,
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return Result.nothing if target == '?'
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        check_1d100(total, cmp_op, target).map(|r| CheckOutcome::Result(Box::new(r)))
    }

    /// Ruby `Warhammer#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        static ATTACK: OnceLock<Regex> = OnceLock::new();
        static CRITICAL: OnceLock<Regex> = OnceLock::new();
        // Ruby は `command.upcase` に対してマッチさせるが、`enabled_upcase_input` が
        // 既定の true なので、ここへ来る時点で command は大文字化済み。
        let attack = ATTACK.get_or_init(|| Regex::new(r"^(WH\d+(?:@[\dWH]*)?)").expect("valid"));
        let critical = CRITICAL.get_or_init(|| Regex::new(r"^(WH[HABTLW]\d+)").expect("valid"));

        let output = if let Some(caps) = attack.captures(command) {
            attack_result(&caps[1], rng)?
        } else if let Some(caps) = critical.captures(command) {
            critical_result(&caps[1], rng)?
        } else {
            // Ruby: どの when にも該当しなければ output_msg は nil のまま
            return Ok(None);
        };

        // Ruby の `return '1'`（該当なし）はそのまま返して dice_command 側で nil に畳ませる
        Ok(Some(SpecificCommandOutput::text(
            output.unwrap_or_else(|| "1".to_owned()),
        )))
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
            .join("test/data/Warhammer.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Warhammer.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Warhammer.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Warhammer.toml must parse");
        assert_eq!(
            data.tests.len(),
            241,
            "case count in test/data/Warhammer.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Warhammer",
                "unexpected game system in Warhammer.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Warhammer"), &tc.input, &mut src) {
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
                    "FAIL Warhammer:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Warhammer cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
