//! P4で手書き移植した `lib/bcdice/game_system/AceKillerGene.rb` と、
//! 親クラス `lib/bcdice/game_system/GardenOrder.rb` のうち本システムが使う部分。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `AceKillerGene#eval_game_system_specific_command`（判定 `AKx/y@z` と負傷表 `DCxxy`）
//! - 親クラス `GardenOrder` の `#get_critical_border` / `#check_roll_repeat_attack` /
//!   `#check_roll` / `#get_check_result` / `#look_up_damage_chart` /
//!   `#get_damage_table_info_by_type` と `DAMAGE_TABLE`
//! - `Base#get_table_by_number`（`default` が `nil` の呼び出し方）
//!
//! # 親クラスの置き場所
//!
//! Ruby側は判定も負傷表も `GardenOrder` にあり、`AceKillerGene` はコマンド名を
//! `GO` から `AK`/`AKG` に差し替えるだけ。Rust側の `GardenOrder.rs` はまだ生成スタブの
//! ままで別バッチの担当なので、ここでは必要な部分をこのファイルに持つ。
//! `GardenOrder` 本体が移植されたら、そちらへ寄せて重複を畳める。
//!
//! `GardenOrder` の `SExxy`（ソウルエンコーダー用負傷表 `DAMAGE_TABLE_SE`）は
//! `AceKillerGene` からは呼ばれないので移植していない。
//!
//! # 表データ
//!
//! `DAMAGE_TABLE`（`JA_` ではなく `DAMAGE_TABLE_` 接頭辞の `static` 群）は
//! `GardenOrder.rb` から機械的に書き出したもので、値は1文字も変えていない。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::AceKillerGene`（ID: `AceKillerGene`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AceKillerGene;

impl GameSystem for AceKillerGene {
    fn id(&self) -> &'static str {
        "AceKillerGene"
    }

    fn name(&self) -> &'static str {
        "エースキラージーン"
    }

    fn sort_key(&self) -> &'static str {
        "ええすきらあしいん"
    }

    fn help_message(&self) -> &'static str {
        r"・基本判定
　AKx/y@z　x：成功率、y：連続攻撃回数（省略可）、z：クリティカル値（省略可）
　（連続攻撃では1回の判定のみが実施されます）
　例）AK55　AK100/2　AK70@10　AK155/3@44
・負傷表
　DCxxy
　xx：属性（切断：SL，銃弾：BL，衝撃：IM，灼熱：BR，冷却：RF，電撃：EL）
　y：ダメージ
　例）DCSL7　DCEL22
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["(AK|AKG)", "DC(SL|BL|IM|BR|RF|EL).+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `AceKillerGene#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `%r{(AK|AKG)(-?\d+)(/(\d+))?(@(\d+))?}i`（アンカー無し）。
fn ak_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(AK|AKG)(-?\d+)(/(\d+))?(@(\d+))?").expect("valid regex"))
}

/// Ruby `/^DC(SL|BL|IM|BR|RF|EL)(\d+)/i`。
///
/// Ruby の `^` は行頭だが、`Preprocessor` が最初の空白より前しか残さないので
/// 実際に来るのは1行の文字列だけ。`regex` の `^`（文字列先頭）で同じ挙動になる。
fn dc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^DC(SL|BL|IM|BR|RF|EL)(\d+)").expect("valid regex"))
}

/// Ruby `AceKillerGene#eval_game_system_specific_command`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(m) = ak_pattern().captures(command) {
        let success_rate = to_i(&m[2]);
        // Ruby: (Regexp.last_match(4) || 1).to_i
        let repeat_count = m.get(4).map_or(1, |v| to_i(v.as_str()));
        let critical_border = get_critical_border(m.get(6).map(|v| v.as_str()), success_rate);

        return check_roll_repeat_attack(success_rate, repeat_count, critical_border, rng);
    }

    if let Some(m) = dc_pattern().captures(command) {
        let damage_value = to_i(&m[2]);
        return Ok(look_up_damage_chart(&m[1], damage_value).map(SpecificCommandOutput::text));
    }

    Ok(None)
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `GardenOrder#get_critical_border`。
fn get_critical_border(critical_border_text: Option<&str>, success_rate: i64) -> i64 {
    match critical_border_text {
        Some(text) => to_i(text),
        // Ruby: [success_rate / 5, 1].max（`Integer#/` は床除算）
        None => (success_rate).div_euclid(5).max(1),
    }
}

/// Ruby `GardenOrder#check_roll_repeat_attack`。
///
/// 連続攻撃で1回あたりの成功率が50%未満なら、ダイスを振らずに注意文を返す。
fn check_roll_repeat_attack(
    success_rate: i64,
    repeat_count: i64,
    critical_border: i64,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    // Ruby は `success_rate / 0` で ZeroDivisionError を送出してクラッシュする。
    // 本移植は他のコマンドと同じく「解釈できないコマンド＝nil」に畳む。
    if repeat_count == 0 {
        return Ok(None);
    }

    // Ruby: success_rate / repeat_count（床除算）
    let success_rate_per_one = (success_rate).div_euclid(repeat_count);
    // 連続攻撃は最終的な成功率が50%以上であることが必要 cf. p217
    if repeat_count > 1 && success_rate_per_one < 50 {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "D100<={success_rate_per_one}@{critical_border} ＞ 連続攻撃は成功率が50％以上必要です"
        ))));
    }

    Ok(Some(SpecificCommandOutput::result(check_roll(
        success_rate_per_one,
        critical_border,
        rng,
    )?)))
}

/// Ruby `GardenOrder#check_roll`。
fn check_roll(
    success_rate: i64,
    critical_border: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let success_rate = success_rate.max(0);
    let fumble_border = if success_rate < 100 { 96 } else { 99 };

    let dice_value = rng.roll_once(100)?;
    let mut result = get_check_result(dice_value, success_rate, critical_border, fumble_border);

    result.text = format!(
        "D100<={success_rate}@{critical_border} ＞ {dice_value} ＞ {}",
        result.text
    );
    Ok(result)
}

/// Ruby `GardenOrder#get_check_result`。
///
/// クリティカルとファンブルが重なった場合は、ファンブルとなる。 cf. p175
fn get_check_result(
    dice_value: i64,
    success_rate: i64,
    critical_border: i64,
    fumble_border: i64,
) -> EvalResult {
    if dice_value >= fumble_border {
        EvalResult::fumble("ファンブル")
    } else if dice_value <= critical_border {
        EvalResult::critical("クリティカル")
    } else if dice_value <= success_rate {
        EvalResult::success("成功")
    } else {
        EvalResult::failure("失敗")
    }
}

// ---------------------------------------------------------------------------
// 負傷表
// ---------------------------------------------------------------------------

/// Ruby `GardenOrder::DAMAGE_TABLE` の1項目（`[上限値, {name:, text:, damage:}]`）。
struct DamageRow {
    /// Ruby `item[0]`（この値以下のダメージがこの行に当たる）
    max: i64,
    /// Ruby `row[:name]`
    name: &'static str,
    /// Ruby `row[:text]`
    text: &'static str,
    /// Ruby `row[:damage]`
    damage: &'static str,
}

/// Ruby `GardenOrder::DAMAGE_TABLE[type]`（`{name:, table:}`）。
struct DamageTable {
    /// Ruby `data[:name]`（属性名）
    name: &'static str,
    /// Ruby `data[:table]`
    rows: &'static [DamageRow],
}

/// Ruby `GardenOrder#get_damage_table_info_by_type(type, "DC")`。
fn damage_table_by_type(damage_type: &str) -> Option<&'static DamageTable> {
    match damage_type {
        "SL" => Some(&DAMAGE_TABLE_SL),
        "BL" => Some(&DAMAGE_TABLE_BL),
        "IM" => Some(&DAMAGE_TABLE_IM),
        "BR" => Some(&DAMAGE_TABLE_BR),
        "RF" => Some(&DAMAGE_TABLE_RF),
        "EL" => Some(&DAMAGE_TABLE_EL),
        _ => None,
    }
}

/// Ruby `Base#get_table_by_number(index, table, nil)`。
///
/// 先頭から見て最初に `上限値 >= index` になった行を返す。`default` が `nil` の
/// 呼び出し方なので、どの行にも当たらなければ `None`。
fn get_table_by_number(index: i64, table: &'static DamageTable) -> Option<&'static DamageRow> {
    table.rows.iter().find(|row| row.max >= index)
}

/// Ruby `GardenOrder#look_up_damage_chart`。
fn look_up_damage_chart(damage_type: &str, damage_value: i64) -> Option<String> {
    // 正規表現が属性を6種に限っているので、ここが `None` になることはない。
    let table = damage_table_by_type(damage_type)?;
    let row = get_table_by_number(damage_value, table)?;

    Some(format!(
        "負傷表：{}[{damage_value}] ＞ {} ｜ {} … {}",
        table.name, row.damage, row.name, row.text
    ))
}

/// Ruby `GardenOrder::DAMAGE_TABLE["SL"]`（切断）。
static DAMAGE_TABLE_SL: DamageTable = DamageTable {
    name: "切断",
    rows: &[
        DamageRow {
            max: 5,
            name: "切り傷",
            text: "皮膚が切り裂かれる。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 10,
            name: "脚部負傷",
            text: "足が切り裂かれ、思わずひざまずく。",
            damage: "軽傷２／マヒ",
        },
        DamageRow {
            max: 13,
            name: "出血",
            text: "斬り裂かれた傷から出血が続く。",
            damage: "軽傷３／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 16,
            name: "胴部負傷",
            text: "胴部に大きな傷を受ける。",
            damage: "軽傷４",
        },
        DamageRow {
            max: 19,
            name: "腕部負傷",
            text: "腕に大きな傷を受ける。",
            damage: "重傷1／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 22,
            name: "腹部負傷",
            text: "腹部を深く切り裂かれる。",
            damage: "重傷２",
        },
        DamageRow {
            max: 25,
            name: "大量出血",
            text: "傷は深く、そこから大量に出血する。",
            damage: "重傷２／ＤＯＴ：軽傷２",
        },
        DamageRow {
            max: 28,
            name: "裂傷",
            text: "治りにくい傷をつけられる。",
            damage: "重傷３",
        },
        DamageRow {
            max: 31,
            name: "視界不良",
            text: "頭部に受けた傷から血が流れ、視界がふさがれる。",
            damage: "重傷３／スタン",
        },
        DamageRow {
            max: 34,
            name: "胸部負傷",
            text: "胸から腰にかけて大きく切り裂かれる。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 37,
            name: "動脈切断",
            text: "動脈が切り裂かれ、噴き出るように出血する。",
            damage: "致命傷1／ＤＯＴ：軽傷３",
        },
        DamageRow {
            max: 39,
            name: "胸部切断",
            text: "傷が肺にまで達し、喀血する。",
            damage: "致命傷２",
        },
        DamageRow {
            max: 9999,
            name: "脊髄損傷",
            text: "脊髄が損傷する。",
            damage: "致命傷２／放心、スタン、マヒ",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE["BL"]`（銃弾）。
static DAMAGE_TABLE_BL: DamageTable = DamageTable {
    name: "銃弾",
    rows: &[
        DamageRow {
            max: 5,
            name: "腕部損傷",
            text: "銃弾が腕をかすめた。",
            damage: "軽傷２",
        },
        DamageRow {
            max: 10,
            name: "腕部貫通",
            text: "銃弾が腕を貫く。痛みはあるが動作に支障はない。",
            damage: "軽傷３",
        },
        DamageRow {
            max: 13,
            name: "胴部負傷",
            text: "胴部に銃弾をくらう。痛みで動きが鈍くなる。",
            damage: "軽傷４／スロウ：－３",
        },
        DamageRow {
            max: 16,
            name: "肩負傷",
            text: "肩を貫かれる。骨が砕けたようだ。",
            damage: "重傷1",
        },
        DamageRow {
            max: 19,
            name: "腹部負傷",
            text: "腹部が貫かれる。かろうじて内臓にダメージはないようだ。",
            damage: "重傷２",
        },
        DamageRow {
            max: 22,
            name: "脚部貫通",
            text: "脚を銃弾に貫かれ、その場でひざまずく。",
            damage: "重傷２／マヒ",
        },
        DamageRow {
            max: 25,
            name: "消化器系損傷",
            text: "胃などの消化器官にダメージを受ける。",
            damage: "重傷３",
        },
        DamageRow {
            max: 28,
            name: "盲管銃弾",
            text: "身体に弾丸が深々と刺さる。激痛が走る。",
            damage: "重傷３／スロウ：－5",
        },
        DamageRow {
            max: 31,
            name: "内臓損傷",
            text: "いくつかの内臓にダメージを受ける。",
            damage: "致命傷1／スタン",
        },
        DamageRow {
            max: 34,
            name: "胴部貫通",
            text: "腹部への攻撃が貫通し、出血する。",
            damage: "致命傷1／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 37,
            name: "胸部負傷",
            text: "銃弾で肺を貫かれる。",
            damage: "致命傷２",
        },
        DamageRow {
            max: 39,
            name: "致命的な一撃",
            text: "銃弾が頭部に命中。ショックで意識を飛ばされる。",
            damage: "致命傷２／放心",
        },
        DamageRow {
            max: 9999,
            name: "必殺の一撃",
            text: "銃弾が心臓の近くを貫く。動脈にダメージを受けたようだ。",
            damage: "致命傷２／ＤＯＴ：重傷1",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE["IM"]`（衝撃）。
static DAMAGE_TABLE_IM: DamageTable = DamageTable {
    name: "衝撃",
    rows: &[
        DamageRow {
            max: 5,
            name: "打撲",
            text: "攻撃を受けた箇所がどす黒く腫れ上がる。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 10,
            name: "転倒",
            text: "衝撃で転倒する。",
            damage: "軽傷1／マヒ",
        },
        DamageRow {
            max: 13,
            name: "平衡感覚喪失",
            text: "衝撃で三半規管にダメージを受ける。",
            damage: "軽傷２、疲労２",
        },
        DamageRow {
            max: 16,
            name: "ボディーブロー",
            text: "腹部に直撃。痛みが継続し、体力を奪う。",
            damage: "軽傷３／ＤＯＴ：疲労３",
        },
        DamageRow {
            max: 19,
            name: "痛打",
            text: "胴部や脚部などに打撃を受ける。",
            damage: "軽傷４／スタン",
        },
        DamageRow {
            max: 22,
            name: "頭部痛打",
            text: "頭部にクリーンヒット。意識がもうろうとする。",
            damage: "軽傷5／放心",
        },
        DamageRow {
            max: 25,
            name: "脚部骨折",
            text: "攻撃が足に命中し、骨折する。",
            damage: "重傷1／スロウ：－5",
        },
        DamageRow {
            max: 28,
            name: "大転倒",
            text: "激しい衝撃によって、負傷すると共に大きく体勢を崩す。",
            damage: "重傷1／マヒ、スタン",
        },
        DamageRow {
            max: 31,
            name: "脳震盪",
            text: "脳が大きく揺さぶられ、意識が飛びそうになる。",
            damage: "重傷２／放心",
        },
        DamageRow {
            max: 34,
            name: "複雑骨折",
            text: "攻撃を受けた部分が大きくひしゃげ、複雑骨折したようだ。",
            damage: "重傷３／放心、スタン",
        },
        DamageRow {
            max: 37,
            name: "頭部裂傷",
            text: "頭部に命中。皮膚が大きく裂ける。",
            damage: "致命傷1、疲労３",
        },
        DamageRow {
            max: 39,
            name: "肋骨負傷",
            text: "折れた肋骨が肺に突き刺さり、まともに呼吸を行なうことができない。",
            damage: "致命傷1／放心、スタン",
        },
        DamageRow {
            max: 9999,
            name: "内臓損傷",
            text: "衝撃が身体の芯まで届き、内臓がいくつか傷ついたようだ。",
            damage: "致命傷２／ＤＯＴ：重傷1",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE["BR"]`（灼熱）。
static DAMAGE_TABLE_BR: DamageTable = DamageTable {
    name: "灼熱",
    rows: &[
        DamageRow {
            max: 5,
            name: "火傷",
            text: "皮膚に小さな火傷を負う。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 10,
            name: "温度上昇",
            text: "熱によって、怪我だけではなく体力も奪われる。",
            damage: "軽傷２、疲労1",
        },
        DamageRow {
            max: 13,
            name: "恐怖",
            text: "燃え上がる炎に恐怖を感じ、身体がすくんで動きが止まる。",
            damage: "軽傷３／放心",
        },
        DamageRow {
            max: 16,
            name: "発火",
            text: "衣服や身体の一部に火が燃え移る。",
            damage: "軽傷３／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 19,
            name: "爆発",
            text: "爆発により吹き飛ばされ、転倒する。",
            damage: "重傷1／マヒ",
        },
        DamageRow {
            max: 22,
            name: "大火傷",
            text: "痕が残るほどの大きな火傷を負う。",
            damage: "重傷２",
        },
        DamageRow {
            max: 25,
            name: "熱波",
            text: "火傷と強力な熱により意識がもうろうとする。",
            damage: "重傷２／スタン",
        },
        DamageRow {
            max: 28,
            name: "大爆発",
            text: "激しい爆発で吹き飛ばされ、ダメージと共に転倒する。",
            damage: "重傷３／マヒ",
        },
        DamageRow {
            max: 31,
            name: "大発火",
            text: "広範囲に火が燃え移る。",
            damage: "重傷３／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 34,
            name: "炭化",
            text: "高熱のあまり、焼けた部分が炭化してしまう。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 37,
            name: "内臓火傷",
            text: "高温の空気を吸い込む、気道にも火傷を負ってしまう。",
            damage: "致命傷1／ＤＯＴ：軽傷1",
        },
        DamageRow {
            max: 39,
            name: "全身火傷",
            text: "身体の各所に深い火傷を負う。",
            damage: "致命傷２",
        },
        DamageRow {
            max: 9999,
            name: "致命的火傷",
            text: "身体の大部分に焼けどを負う。",
            damage: "致命傷２／スタン",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE["RF"]`（冷却）。
static DAMAGE_TABLE_RF: DamageTable = DamageTable {
    name: "冷却",
    rows: &[
        DamageRow {
            max: 5,
            name: "冷気",
            text: "軽い凍傷を受ける。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 10,
            name: "霜の衣",
            text: "身体が薄い氷で覆われ、動きが鈍る。",
            damage: "軽傷1／疲労1",
        },
        DamageRow {
            max: 13,
            name: "凍傷",
            text: "凍傷により身体が傷つけられる。",
            damage: "軽傷3",
        },
        DamageRow {
            max: 16,
            name: "体温低下",
            text: "冷気によって体温を奪われる。",
            damage: "軽傷３／ＤＯＴ：疲労1",
        },
        DamageRow {
            max: 19,
            name: "氷の枷",
            text: "肘や膝などが氷で覆われ、動きが取りにくくなる。",
            damage: "重傷1／マヒ",
        },
        DamageRow {
            max: 22,
            name: "大凍傷",
            text: "身体の各所に凍傷を受ける。",
            damage: "重傷1／ＤＯＴ：疲労２",
        },
        DamageRow {
            max: 25,
            name: "氷の束縛",
            text: "下半身が凍りつき、動くことができない。",
            damage: "重傷２／マヒ",
        },
        DamageRow {
            max: 28,
            name: "視界不良",
            text: "頭部にも氷が張り、視界がふさがれる。",
            damage: "重傷２／スタン",
        },
        DamageRow {
            max: 31,
            name: "腕部凍結",
            text: "腕が凍りづけになり、動かすことができない。",
            damage: "重傷３／放心",
        },
        DamageRow {
            max: 34,
            name: "重度凍傷",
            text: "さらに体温が低下し、深刻な凍傷を受ける。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 37,
            name: "全身凍結",
            text: "全身が凍りづけになる。",
            damage: "致命傷1／ＤＯＴ：疲労２",
        },
        DamageRow {
            max: 39,
            name: "致命的凍傷",
            text: "身体全身に凍傷を受ける。",
            damage: "致命傷２",
        },
        DamageRow {
            max: 9999,
            name: "氷の棺",
            text: "完全に氷に閉じ込められる。",
            damage: "致命傷２／スタン、マヒ",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE["EL"]`（電撃）。
static DAMAGE_TABLE_EL: DamageTable = DamageTable {
    name: "電撃",
    rows: &[
        DamageRow {
            max: 5,
            name: "静電気",
            text: "全身の毛が逆立つ。",
            damage: "疲労３",
        },
        DamageRow {
            max: 10,
            name: "電熱傷",
            text: "電流によって傷つく。",
            damage: "疲労1、軽傷1",
        },
        DamageRow {
            max: 13,
            name: "感電",
            text: "電流で傷つくと共に、身体が軽くしびれる。",
            damage: "疲労２、軽傷２",
        },
        DamageRow {
            max: 16,
            name: "閃光",
            text: "激しい電光により、一時的に視界がふさがれる。",
            damage: "軽傷３／スタン",
        },
        DamageRow {
            max: 19,
            name: "脚部感電",
            text: "電流により脚がしびれ、動けなくなる。",
            damage: "重傷1／マヒ",
        },
        DamageRow {
            max: 22,
            name: "大電熱傷",
            text: "身体の各所が電流によって傷つく。",
            damage: "疲労２、重傷２",
        },
        DamageRow {
            max: 25,
            name: "腕部負傷",
            text: "電流で腕がしびれ、動けなくなる。",
            damage: "軽傷1、重傷２／放心",
        },
        DamageRow {
            max: 28,
            name: "大感電",
            text: "電流によって身体中がしびれ、動けなくなる。",
            damage: "重傷２／スタン、マヒ",
        },
        DamageRow {
            max: 31,
            name: "一時心停止",
            text: "強力な電撃のショックにより、心臓がほんの一瞬だけ止まる。",
            damage: "疲労３、重傷３",
        },
        DamageRow {
            max: 34,
            name: "大電流",
            text: "全身に電流が駆け巡る。",
            damage: "重傷３／放心、マヒ",
        },
        DamageRow {
            max: 37,
            name: "致命電熱傷",
            text: "全身が電流によって傷つく。",
            damage: "重傷1、致命傷1",
        },
        DamageRow {
            max: 39,
            name: "心停止",
            text: "強力な電撃のショックにより、心臓が一時的に止まる。死の淵が見える。",
            damage: "疲労３、重傷1、致命傷1",
        },
        DamageRow {
            max: 9999,
            name: "組織炭化",
            text: "全身が電流で焼かれ、あちこちの組織が炭化する。",
            damage: "致命傷２／スタン",
        },
    ],
};
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
            .join("test/data/AceKillerGene.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/AceKillerGene.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/AceKillerGene.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("AceKillerGene.toml must parse");
        assert_eq!(
            data.tests.len(),
            43,
            "case count in test/data/AceKillerGene.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "AceKillerGene",
                "unexpected game system in AceKillerGene.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("AceKillerGene"), &tc.input, &mut src) {
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
                    "FAIL AceKillerGene:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} AceKillerGene cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// TOMLに無い経路の固定。
    ///
    /// - `AKG` 別名（`(AK|AKG)` のバックトラック）
    /// - `SE` 負傷表は `AceKillerGene` では引けない（`GardenOrder` 側のコマンド）
    /// - 負傷表の最終行（上限9999）を超えるダメージは `nil`
    #[test]
    fn alias_and_out_of_range_paths() {
        let mut src = SeededRandomizer::new(vec![(10, 100)]);
        let result = eval_command(&GameSystemId::new("AceKillerGene"), "AKG50", &mut src)
            .expect("AKG50 must not error")
            .expect("AKG50 must produce output");
        assert_eq!(result.text, "D100<=50@10 ＞ 10 ＞ クリティカル");

        let mut src = SeededRandomizer::new(vec![]);
        assert!(
            eval_command(&GameSystemId::new("AceKillerGene"), "SESL7", &mut src)
                .expect("SESL7 must not error")
                .is_none()
        );

        let mut src = SeededRandomizer::new(vec![]);
        assert!(
            eval_command(&GameSystemId::new("AceKillerGene"), "DCSL10000", &mut src)
                .expect("DCSL10000 must not error")
                .is_none()
        );
    }
}
