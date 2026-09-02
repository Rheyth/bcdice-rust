//! P4で手書き移植した `lib/bcdice/game_system/GardenOrder.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `GardenOrder#eval_game_system_specific_command`（判定 `GOx/y@z` と
//!   負傷表 `DCxxy` / `SExxy`）
//! - `#get_critical_border` / `#check_roll_repeat_attack` / `#check_roll` / `#get_check_result`
//!   （継承先の `GardenOrder_Korean` / `ScreamHighSchool` / `GardenOrderReEdit` も使う）
//! - `#look_up_damage_chart` / `#look_up_damage_se_chart`
//! - `DAMAGE_TABLE` / `DAMAGE_TABLE_SE`
//!
//! # 表データ
//!
//! `DAMAGE_TABLE` / `DAMAGE_TABLE_SE` は `GardenOrder.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。
//!
//! ロケール差のあるデータ（判定結果の文言・負傷表）は [`SystemTables`] に束ね、
//! `GardenOrder_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::eval::EvalError;
use crate::game_system::{str_helpers, GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::GardenOrder`（ID: `GardenOrder`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenOrder;

impl GameSystem for GardenOrder {
    fn id(&self) -> &'static str {
        "GardenOrder"
    }

    fn name(&self) -> &'static str {
        "ガーデンオーダー"
    }

    fn sort_key(&self) -> &'static str {
        "かあてんおおたあ"
    }

    fn help_message(&self) -> &'static str {
        r"・基本判定
　GOx/y@z　x：成功率、y：連続攻撃回数（省略可）、z：クリティカル値（省略可）
　（連続攻撃では1回の判定のみが実施されます）
　例）GO55　GO100/2　GO70@10　GO155/3@44

・負傷表
　DCxxy
　xx：属性（切断：SL，銃弾：BL，衝撃：IM，灼熱：BR，冷却：RF，電撃：EL）
　y：ダメージ
　例）DCSL7　DCEL22

・負傷表(ソウルエンコーダー用)
　SExxy
　xx：属性（切断：SL，銃弾：BL，衝撃：IM，灼熱：BR，冷却：RF，電撃：EL）
　y：ダメージ
　例）SESL7　SEEL22
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["GO", "DC(SL|BL|IM|BR|RF|EL).+", "SE(SL|BL|IM|BR|RF|EL).+"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `GardenOrder#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_TABLES, command, rng)
    }
}

// ---------------------------------------------------------------------------
// 表の部品
// ---------------------------------------------------------------------------

/// Ruby `GardenOrder::DAMAGE_TABLE` の1項目（`[max, {name:, text:, damage:}]`）。
pub(crate) struct DamageRow {
    /// この行が担当するダメージの上限（`get_table_by_number` の比較対象）
    pub(crate) max: i64,
    pub(crate) name: &'static str,
    pub(crate) text: &'static str,
    pub(crate) damage: &'static str,
}

/// Ruby `GardenOrder::DAMAGE_TABLE[type]`（`{name:, table:}`）。
pub(crate) struct DamageTable {
    pub(crate) name: &'static str,
    pub(crate) rows: &'static [DamageRow],
}

/// 属性（`SL` / `BL` / `IM` / `BR` / `RF` / `EL`）ごとの負傷表。
pub(crate) struct DamageTableSet {
    pub(crate) sl: &'static DamageTable,
    pub(crate) bl: &'static DamageTable,
    pub(crate) im: &'static DamageTable,
    pub(crate) br: &'static DamageTable,
    pub(crate) rf: &'static DamageTable,
    pub(crate) el: &'static DamageTable,
}

impl DamageTableSet {
    /// Ruby `DAMAGE_TABLE[type]`。未知の属性は `nil`。
    pub(crate) fn get(&self, damage_type: &str) -> Option<&'static DamageTable> {
        match damage_type {
            "SL" => Some(self.sl),
            "BL" => Some(self.bl),
            "IM" => Some(self.im),
            "BR" => Some(self.br),
            "RF" => Some(self.rf),
            "EL" => Some(self.el),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ロケールごとの表と定型文
// ---------------------------------------------------------------------------

/// 1ロケール分の表と定型文。`GardenOrder` と `GardenOrder_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// `Result.fumble` の文言
    pub(crate) fumble: &'static str,
    /// `Result.critical` の文言
    pub(crate) critical: &'static str,
    /// `Result.success` の文言
    pub(crate) success: &'static str,
    /// `Result.failure` の文言
    pub(crate) failure: &'static str,
    /// 連続攻撃の成功率が50%未満だったときの文言
    pub(crate) repeat_attack_warning: &'static str,
    /// 負傷表の見出し（`負傷表` / `부상 표`）
    pub(crate) damage_chart_title: &'static str,
    /// Ruby `DAMAGE_TABLE`
    pub(crate) damage_table: DamageTableSet,
    /// Ruby `DAMAGE_TABLE_SE`
    pub(crate) damage_table_se: DamageTableSet,
}

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `%r{GO(-?\d+)(/(\d+))?(@(\d+))?}i`（アンカー無し）。
fn go_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)GO(-?\d+)(/(\d+))?(@(\d+))?").expect("valid regex"))
}

/// Ruby `/^(DC|SE)(SL|BL|IM|BR|RF|EL)(\d+)/i`。
fn damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(DC|SE)(SL|BL|IM|BR|RF|EL)(\d+)").expect("valid regex"))
}

/// Ruby `GardenOrder#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(m) = go_pattern().captures(command) {
        let success_rate = to_i(&m[1]);
        let repeat_count = m.get(3).map_or(1, |v| to_i(v.as_str()));
        let critical_border = get_critical_border(m.get(5).map(|v| v.as_str()), success_rate);
        return check_roll_repeat_attack(sys, success_rate, repeat_count, critical_border, rng);
    }

    if let Some(m) = damage_pattern().captures(command) {
        // `Base#dice_command` が入力を大文字化しているので、`chart_type` / `type` は大文字
        let damage_value = to_i(&m[3]);
        let text = match &m[1] {
            "DC" => look_up_damage_chart(sys, &m[2], damage_value),
            "SE" => look_up_damage_se_chart(sys, &m[2], damage_value),
            _ => None,
        };
        return Ok(text.map(SpecificCommandOutput::text));
    }

    Ok(None)
}

/// Ruby `String#to_i`。`i64` に収まらない指定は 符号方向に飽和。
///
/// `GardenOrderReEdit` / `ScreamHighSchool` から `super::GardenOrder::to_i`
/// として借用されるため `pub(crate)` を維持する。
pub(crate) fn to_i(digits: &str) -> i64 {
    str_helpers::to_i_signed_saturating(digits)
}

/// Ruby `GardenOrder#get_critical_border`。
pub(crate) fn get_critical_border(critical_border_text: Option<&str>, success_rate: i64) -> i64 {
    match critical_border_text {
        Some(text) => to_i(text),
        // Ruby `Integer#/` は床除算
        None => (success_rate).div_euclid(5).max(1),
    }
}

/// Ruby `GardenOrder#check_roll_repeat_attack`。
///
/// `repeat_count == 0` は Ruby だと `ZeroDivisionError` で落ちる入力
/// （`GO50/0`）。ここでは `GardenOrderReEdit` と同じく `None` に畳む。
pub(crate) fn check_roll_repeat_attack(
    sys: &SystemTables,
    success_rate: i64,
    repeat_count: i64,
    critical_border: i64,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if repeat_count == 0 {
        return Ok(None);
    }

    let success_rate_per_one = (success_rate).div_euclid(repeat_count);
    // 連続攻撃は最終的な成功率が50%以上であることが必要 cf. p217
    if repeat_count > 1 && success_rate_per_one < 50 {
        return Ok(Some(SpecificCommandOutput::text(format!(
            "D100<={success_rate_per_one}@{critical_border} ＞ {}",
            sys.repeat_attack_warning
        ))));
    }

    Ok(Some(SpecificCommandOutput::result(check_roll(
        sys,
        success_rate_per_one,
        critical_border,
        rng,
    )?)))
}

/// Ruby `GardenOrder#check_roll`。
pub(crate) fn check_roll(
    sys: &SystemTables,
    success_rate: i64,
    critical_border: i64,
    rng: &mut Randomizer,
) -> Result<EvalResult, EvalError> {
    let success_rate = success_rate.max(0);
    let fumble_border = if success_rate < 100 { 96 } else { 99 };

    let dice_value = rng.roll_once(100)?;
    let mut result = get_check_result(
        sys,
        dice_value,
        success_rate,
        critical_border,
        fumble_border,
    );

    result.text = format!(
        "D100<={success_rate}@{critical_border} ＞ {dice_value} ＞ {}",
        result.text
    );
    Ok(result)
}

/// Ruby `GardenOrder#get_check_result`。
pub(crate) fn get_check_result(
    sys: &SystemTables,
    dice_value: i64,
    success_rate: i64,
    critical_border: i64,
    fumble_border: i64,
) -> EvalResult {
    // クリティカルとファンブルが重なった場合は、ファンブルとなる。 cf. p175
    if dice_value >= fumble_border {
        EvalResult::fumble(sys.fumble)
    } else if dice_value <= critical_border {
        EvalResult::critical(sys.critical)
    } else if dice_value <= success_rate {
        EvalResult::success(sys.success)
    } else {
        EvalResult::failure(sys.failure)
    }
}

/// Ruby `Base#get_table_by_number(index, table, nil)`。
fn get_table_by_number(index: i64, table: &'static DamageTable) -> Option<&'static DamageRow> {
    table.rows.iter().find(|row| row.max >= index)
}

/// Ruby `GardenOrder#look_up_damage_chart`。
pub(crate) fn look_up_damage_chart(
    sys: &SystemTables,
    damage_type: &str,
    damage_value: i64,
) -> Option<String> {
    let table = sys.damage_table.get(damage_type)?;
    let row = get_table_by_number(damage_value, table)?;

    Some(format!(
        "{}：{}[{damage_value}] ＞ {} ｜ {} … {}",
        sys.damage_chart_title, table.name, row.damage, row.name, row.text
    ))
}

/// Ruby `GardenOrder#look_up_damage_se_chart`。
pub(crate) fn look_up_damage_se_chart(
    sys: &SystemTables,
    damage_type: &str,
    damage_value: i64,
) -> Option<String> {
    let table = sys.damage_table_se.get(damage_type)?;
    let row = get_table_by_number(damage_value, table)?;

    Some(format!(
        "{}(SE)：{}[{damage_value}] ＞ {} ｜ {} … {}",
        sys.damage_chart_title, table.name, row.damage, row.name, row.text
    ))
}

// ---------------------------------------------------------------------------
// ja_jp の表と定型文
// ---------------------------------------------------------------------------

/// `ja_jp` ロケールの表と定型文。
pub(crate) static JA_TABLES: SystemTables = SystemTables {
    fumble: "ファンブル",
    critical: "クリティカル",
    success: "成功",
    failure: "失敗",
    repeat_attack_warning: "連続攻撃は成功率が50％以上必要です",
    damage_chart_title: "負傷表",
    damage_table: DamageTableSet {
        sl: &JA_DC_SL,
        bl: &JA_DC_BL,
        im: &JA_DC_IM,
        br: &JA_DC_BR,
        rf: &JA_DC_RF,
        el: &JA_DC_EL,
    },
    damage_table_se: DamageTableSet {
        sl: &JA_SE_SL,
        bl: &JA_SE_BL,
        im: &JA_SE_IM,
        br: &JA_SE_BR,
        rf: &JA_SE_RF,
        el: &JA_SE_EL,
    },
};

/// Ruby `GardenOrder::DAMAGE_TABLE["SL"]`（切断）。
static JA_DC_SL: DamageTable = DamageTable {
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
static JA_DC_BL: DamageTable = DamageTable {
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
static JA_DC_IM: DamageTable = DamageTable {
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
static JA_DC_BR: DamageTable = DamageTable {
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
static JA_DC_RF: DamageTable = DamageTable {
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
static JA_DC_EL: DamageTable = DamageTable {
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

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["SL"]`（切断）。
static JA_SE_SL: DamageTable = DamageTable {
    name: "切断",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い衝撃",
            text: "たいした傷ではないが、衝撃を受ける。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "小さな傷",
            text: "外装に傷がつく。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 13,
            name: "大きな傷",
            text: "外装に大きな傷がつく。",
            damage: "軽傷2",
        },
        DamageRow {
            max: 16,
            name: "とても大きな傷",
            text: "外装にさらに大きな傷がつき、内部もダメージを受ける。",
            damage: "軽傷3/DOT:軽傷1",
        },
        DamageRow {
            max: 19,
            name: "外観破損",
            text: "外装の一部が欠ける。",
            damage: "軽傷4",
        },
        DamageRow {
            max: 22,
            name: "内部破損",
            text: "外装の一部が破損し、内部もダメージを受ける。",
            damage: "重傷1/DOT:軽傷1",
        },
        DamageRow {
            max: 25,
            name: "内部大破損",
            text: "内部の一部が大きく破損する。",
            damage: "重傷2",
        },
        DamageRow {
            max: 28,
            name: "一時不全",
            text: "外装の一部が壊れ、内部も大きなダメージを受ける。",
            damage: "重傷2/DOT:軽傷2",
        },
        DamageRow {
            max: 31,
            name: "裂傷",
            text: "傷をつけられ、内部が顔をのぞかせる。",
            damage: "重傷3",
        },
        DamageRow {
            max: 34,
            name: "視界不良",
            text: "取りつけられているカメラに不都合が生じる",
            damage: "重傷3/スタン",
        },
        DamageRow {
            max: 37,
            name: "大裂傷",
            text: "大きく切り裂かれ、内部が露わになる。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 39,
            name: "機能不全",
            text: "重要な部品が壊れ、このままで機能に大きな障害が出る。",
            damage: "致命傷1/DOT:軽傷3",
        },
        DamageRow {
            max: 9999,
            name: "致命的損傷",
            text: "機能が停止しかねないほどの大きな損傷を受ける。",
            damage: "致命傷2",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["BL"]`（銃弾）。
static JA_SE_BL: DamageTable = DamageTable {
    name: "銃弾",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い衝撃",
            text: "銃弾ははじいたが、衝撃を受ける。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "小さな銃創",
            text: "銃弾ははじいたが、外装に凹みができた。",
            damage: "軽傷2",
        },
        DamageRow {
            max: 13,
            name: "大きな銃創",
            text: "かろうじて銃弾ははじいたが、外装に大きな凹みができた。",
            damage: "軽傷3",
        },
        DamageRow {
            max: 16,
            name: "機能低下",
            text: "銃弾の衝撃で、内部の機能に不都合が講じた。",
            damage: "軽傷4/スロウ:-3",
        },
        DamageRow {
            max: 19,
            name: "とても大きな銃創",
            text: "銃弾をはじけず、外装に食い込んだ。",
            damage: "重傷1",
        },
        DamageRow {
            max: 22,
            name: "銃弾停止",
            text: "銃弾が貫通した。かろうじて内部にダメージはないようだ。",
            damage: "重傷2",
        },
        DamageRow {
            max: 25,
            name: "内部損傷",
            text: "銃弾が貫通した時、内部の機能に衝撃を受ける。",
            damage: "重傷2/放心",
        },
        DamageRow {
            max: 28,
            name: "内部破壊",
            text: "銃弾が貫通した時、内部にも大きなダメージを受ける。",
            damage: "重傷3",
        },
        DamageRow {
            max: 31,
            name: "内部大破壊",
            text: "銃弾が貫通した時、その衝撃で内部に一時的な損傷を受ける。",
            damage: "重傷3/スロウ:-5",
        },
        DamageRow {
            max: 34,
            name: "機能一部停止",
            text: "銃弾によって内部の機能が一時的に役に立たなくなっている。",
            damage: "致命傷1/スタン",
        },
        DamageRow {
            max: 37,
            name: "破損",
            text: "銃弾によって外装が吹き飛び、内部の一部が破壊される。",
            damage: "致命傷1/DOT:軽傷1",
        },
        DamageRow {
            max: 39,
            name: "大破損",
            text: "銃弾によって外装ごと内部の一部が吹き飛ぶ。",
            damage: "致命傷2",
        },
        DamageRow {
            max: 9999,
            name: "致命的な一撃",
            text: "銃弾が重要な部位に命中。衝撃で機能の大部分が一時的に停止する。",
            damage: "致命傷2/放心",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["IM"]`（衝撃）。
static JA_SE_IM: DamageTable = DamageTable {
    name: "衝撃",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い衝撃",
            text: "傷も凹みもないが、衝撃を受ける。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "衝撃",
            text: "衝撃を受けて外装が凹む。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 13,
            name: "大きな衝撃",
            text: "衝撃を受けて外装が大きく凹む。",
            damage: "軽傷2",
        },
        DamageRow {
            max: 16,
            name: "とても大きな衝撃",
            text: "衝撃を受けて外装が大きく凹み、機能が瞬間的に停止する。",
            damage: "軽傷2/放心",
        },
        DamageRow {
            max: 19,
            name: "内部圧迫",
            text: "外装が凹み、内部を圧迫している。",
            damage: "軽傷3/DOT:疲労3",
        },
        DamageRow {
            max: 22,
            name: "痛打",
            text: "当たり所が悪く、カメラ機能が一時的に停止する。",
            damage: "軽傷4/スタン",
        },
        DamageRow {
            max: 25,
            name: "内部衝撃",
            text: "当たり所が悪く、内部に大きなダメージを与える。機能が一時的に停止する。",
            damage: "軽傷5/放心",
        },
        DamageRow {
            max: 28,
            name: "機能障害",
            text: "衝撃によって機能の動作に障害が発生する。",
            damage: "重傷1/スロウ:-5",
        },
        DamageRow {
            max: 31,
            name: "外裝損傷",
            text: "外装が損傷し、その破片が内部に突き刺さる。",
            damage: "重傷1/放心、スタン",
        },
        DamageRow {
            max: 34,
            name: "外装大損傷",
            text: "外装が大きく損傷し、内部もいくつか破壊される。",
            damage: "重傷2/放心",
        },
        DamageRow {
            max: 37,
            name: "外装破壊",
            text: "外装の一部が吹き飛び、内部も破壊される。",
            damage: "重傷3/放心、スタン",
        },
        DamageRow {
            max: 39,
            name: "外装大破壊",
            text: "外装の一部が吹き飛び、内部も一緒に吹き飛ばされる。",
            damage: "致命傷1、疲労3",
        },
        DamageRow {
            max: 9999,
            name: "致命的破壊",
            text: "外装のほとんどが破壊され、内部もひしゃげ、潰される。",
            damage: "致命傷1/放心、スタン",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["BR"]`（灼熱）。
static JA_SE_BR: DamageTable = DamageTable {
    name: "灼熱",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い溶解",
            text: "外装が少しだけ溶け、機能が低下する。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "溶解",
            text: "外装が溶ける。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 13,
            name: "温度上昇",
            text: "熱によって、外装だけでなく内部も少しだけ溶解する。",
            damage: "軽傷2、疲労1",
        },
        DamageRow {
            max: 16,
            name: "温度大上昇",
            text: "熱によって、外装だけでなく内部が溶解する。",
            damage: "軽傷3/放心",
        },
        DamageRow {
            max: 19,
            name: "発火",
            text: "外装に火が燃え移る。",
            damage: "軽傷3/DOT:軽傷1",
        },
        DamageRow {
            max: 22,
            name: "爆発",
            text: "爆発により外装の一部が吹き飛ばされる。",
            damage: "重傷1/スタン",
        },
        DamageRow {
            max: 25,
            name: "大溶解",
            text: "痕が残るほどの大きく外装が溶解する。",
            damage: "重傷2",
        },
        DamageRow {
            max: 28,
            name: "熱波",
            text: "強力な熱により内部機能が低下する。",
            damage: "重傷2/スタン",
        },
        DamageRow {
            max: 31,
            name: "大爆発",
            text: "激しい爆発で外装が大きく吹き飛ばされ、内部にもダメージを受ける。",
            damage: "重傷3/放心",
        },
        DamageRow {
            max: 34,
            name: "大発火",
            text: "広範囲に火が燃え移る。",
            damage: "重傷3/DOT:軽傷1",
        },
        DamageRow {
            max: 37,
            name: "炭化",
            text: "高熱のあまり、焼けた部分が炭化してしまう。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 39,
            name: "内部溶解",
            text: "内部に熱や炎が回り、大きく溶解する。",
            damage: "致命傷1/DOT:軽傷1",
        },
        DamageRow {
            max: 9999,
            name: "致命的溶解",
            text: "外装や内部の大部分が溶解する。",
            damage: "致命傷2",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["RF"]`（冷却）。
static JA_SE_RF: DamageTable = DamageTable {
    name: "冷却",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い冷却",
            text: "冷気で機能が低下する。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "冷気",
            text: "外装が薄い氷で覆われる。",
            damage: "軽傷1",
        },
        DamageRow {
            max: 13,
            name: "霜の衣",
            text: "外装が薄い氷で覆われ、動作が鈍る。",
            damage: "軽傷2/疲労1",
        },
        DamageRow {
            max: 16,
            name: "軽い凍結",
            text: "凍結によって外装の一部が痛む。",
            damage: "軽傷3",
        },
        DamageRow {
            max: 19,
            name: "温度低下",
            text: "冷気によって機能が低下する。",
            damage: "軽傷3/DOT:疲労1",
        },
        DamageRow {
            max: 22,
            name: "氷の枷",
            text: "可動部などが氷で覆われ、動きが取りにくくなる。",
            damage: "重傷1/スタン",
        },
        DamageRow {
            max: 25,
            name: "大凍結",
            text: "外装の大部分が凍結する。",
            damage: "重傷1/DOT:疲労2",
        },
        DamageRow {
            max: 28,
            name: "氷の束縛",
            text: "可動部などが完全に凍りつき、動くことができない。",
            damage: "重傷2/放心",
        },
        DamageRow {
            max: 31,
            name: "視界不良",
            text: "カメラのレンズにも氷が張り、視界がふさがれる。",
            damage: "重傷2/スタン",
        },
        DamageRow {
            max: 34,
            name: "動作不良",
            text: "凍結によって一部動作に不都合が生じている。",
            damage: "重傷3/放心",
        },
        DamageRow {
            max: 37,
            name: "重度凍結",
            text: "さらに温度が低下し、内部にも深刻なダメージを受ける。",
            damage: "致命傷1",
        },
        DamageRow {
            max: 39,
            name: " 全身凍結",
            text: "全体が凍りづけになる。",
            damage: "致命傷1/DOT:疲労2",
        },
        DamageRow {
            max: 9999,
            name: "致命的凍結",
            text: "外装だけでなく、内部も致命的なダメージを受ける。",
            damage: "致命傷2",
        },
    ],
};

/// Ruby `GardenOrder::DAMAGE_TABLE_SE["EL"]`（電撃）。
static JA_SE_EL: DamageTable = DamageTable {
    name: "電撃",
    rows: &[
        DamageRow {
            max: 5,
            name: "軽い電撃",
            text: "電撃で機能が低下する。",
            damage: "スタン",
        },
        DamageRow {
            max: 10,
            name: "帯電",
            text: "帯電により軽いダメージを受ける。",
            damage: "疲労3",
        },
        DamageRow {
            max: 13,
            name: "電熱傷",
            text: "電流によって外装が傷つく。",
            damage: "疲労1、軽傷1",
        },
        DamageRow {
            max: 16,
            name: "軽い感電",
            text: "電流で傷つくと共に、内部に軽いダメージを受ける。",
            damage: "疲労2、軽傷2",
        },
        DamageRow {
            max: 19,
            name: "閃光",
            text: "激しい閃光により、一時的にカメラ機能がマヒする。",
            damage: "軽傷3/スタン",
        },
        DamageRow {
            max: 22,
            name: "感電",
            text: "電流により内部がダメージを受け、一時的に動作がマヒする。",
            damage: "重傷1/放心",
        },
        DamageRow {
            max: 25,
            name: "大電熱傷",
            text: "外装の各所が電流によって傷つく。",
            damage: "疲労2、重傷2",
        },
        DamageRow {
            max: 28,
            name: "感電による負傷",
            text: "外装だけでなく、内部も大きなダメージを受け、機能がマヒする。",
            damage: "軽傷1、重傷2/放心",
        },
        DamageRow {
            max: 31,
            name: "大感電",
            text: "電流によって機能のほとんどがマヒして、動作しなくなる。",
            damage: "重傷2/放心、スタン",
        },
        DamageRow {
            max: 34,
            name: "大電流",
            text: "強力な電撃のショックにより、内部にも多大なダメージを受ける。",
            damage: "疲労3、重傷3",
        },
        DamageRow {
            max: 37,
            name: "一時停止",
            text: "全身に電流が駆け巡り、機能の大部分が動作不良を起こす。",
            damage: "重傷3/放心、スタン",
        },
        DamageRow {
            max: 39,
            name: "致命電熱傷",
            text: "全体が電流によって傷つく。",
            damage: "重傷1、致命傷",
        },
        DamageRow {
            max: 9999,
            name: "機能停止",
            text: "強力な電撃のショックにより、故障寸前のダメージを受ける。",
            damage: "疲労3、重傷1、致命傷1",
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
            .join("test/data/GardenOrder.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/GardenOrder.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。本体のハーネスは
    /// まだ DiceBot しか assert していないので、移植したシステムの回帰は
    /// ここで押さえる。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/GardenOrder.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("GardenOrder.toml must parse");
        assert_eq!(
            data.tests.len(),
            52,
            "case count in test/data/GardenOrder.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "GardenOrder",
                "unexpected game system in GardenOrder.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("GardenOrder"), &tc.input, &mut src) {
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
                    "FAIL GardenOrder:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} GardenOrder cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
