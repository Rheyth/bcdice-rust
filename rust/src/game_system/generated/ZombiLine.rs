//! P4で手書き移植した `lib/bcdice/game_system/ZombiLine.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `ZombiLine#check_action`（判定 `xZL<=y`）
//! - `ZombiLine#eval_game_system_specific_command` → `check_action || roll_tables`
//! - `translate_tables`（ストレス症状表 `SST` / 食材表 `IT`）
//!
//! 表データと定型文は `i18n/ZombiLine/ja_jp.yml` から機械的に書き出したもので、
//! 値は1文字も変えていない。ロケール差のあるデータは [`SystemTables`] に束ね、
//! `ZombiLine_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use crate::command_parser::{Parser, SuffixPosition};
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

// ---------------------------------------------------------------------------
// 表の部品
// ---------------------------------------------------------------------------

/// 引くと文字列を返す表。
///
/// `TABLES` は `DiceTable::Table`（`RollResult`）と `DiceTable::RangeTable`
/// （`RangeTable::RollResult`）が混在し、Ruby側はどちらも `to_s` して使う。
pub(crate) trait RollText: Sync {
    /// Ruby `#roll(randomizer).to_s`。
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError>;
}

impl RollText for Table {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng)?.to_string())
    }
}

impl RollText for RangeTable {
    fn roll_text(&self, rng: &mut Randomizer) -> Result<String, EvalError> {
        Ok(self.roll(rng)?.to_string())
    }
}

/// 1ロケール分の表と定型文。`ZombiLine` と `ZombiLine_Korean` はこれだけが違う。
pub(crate) struct SystemTables {
    /// Ruby `TABLES`
    pub(crate) tables: &'static [(&'static str, &'static dyn RollText)],
    /// i18n `ZombiLine.success_critical`
    pub(crate) success_critical: &'static str,
    /// i18n `ZombiLine.success_fumble`
    pub(crate) success_fumble: &'static str,
    /// i18n `ZombiLine.success`
    pub(crate) success: &'static str,
    /// i18n `ZombiLine.failure_fumble`
    pub(crate) failure_fumble: &'static str,
    /// i18n `ZombiLine.failure`
    pub(crate) failure: &'static str,
}

// ---------------------------------------------------------------------------
// ja_jp ロケールの表データ（i18n/ZombiLine/ja_jp.yml から機械的に書き出したもの）
// ---------------------------------------------------------------------------

static JA_SST_ITEMS: &[&str] = &[
    "憤怒：一番近い敵を攻撃（成功率+20%）しにいきます。近くに敵がいない場合、誰かのストレスを＋１させます。　頭に血が上り、誰かに怒りをぶつけます。",
    "逃避：落下してでも敵から逃げるように移動します。周囲に敵が居ない場合、現実逃避します。　耐えられなくなり、逃げ出します。",
    "幻覚：戦闘中は、「行動放棄（全AP）」します。戦闘以外なら、幻覚を見て笑います。　自分が望む幻覚が見えます。",
    "絶叫：戦闘中は、「注目を集める（2AP）」をします。戦闘以外なら、無意味に叫びます。　思わず叫んでしまいます。",
    "自傷：自ら【怪我】を負います。戦闘中は「自傷行為（1AP）」をして自分が【怪我】します。　思わず自分を傷つけます。",
    "不安：誰かのストレスを１上げます。近くに誰も居ない場合、泣き出します。　不安にかられて余計なことを言います。",
    "忌避：その場から一番近い対象に「石（1AP）」を投げます。それができない場合、【転倒】してうずくまります。　嫌悪感から全てを拒みます。",
    "暴走：一番近い敵を攻撃しにいきます。近くに敵がいない場合、周りの意見も聞かずに安直な行動をします。　冷静でいられなくなり、直情的になります。",
    "混乱：近くにいるランダムな対象に格闘で攻撃しにいきます。それができない場合、「行動放棄（全 AP）」します。　世界全てが敵に見えて攻撃します。",
    "開眼：ストレスは0まで下がります。あなたは教祖となって教義をひとつつくって「布教」できます。次の症状が出るまで効果は続きます。　ゾンビだらけの世界の真理を見つけます。",
];
static JA_SST: Table = Table::from_dice("ストレス症状表", 1, 10, JA_SST_ITEMS);

static JA_IT_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 50), "生モノ食材"),
    (RangeInc::new(51, 80), "怪しい食材"),
    (RangeInc::new(81, 100), "危ない食材"),
];
static JA_IT: RangeTable = RangeTable::from_dice("食材表", 1, 100, JA_IT_ITEMS);

static JA_TABLES: &[(&str, &dyn RollText)] = &[("SST", &JA_SST), ("IT", &JA_IT)];

static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    success_critical: "成功(クリティカル)",
    success_fumble: "成功(ファンブル)",
    success: "成功",
    failure_fumble: "失敗(ファンブル)",
    failure: "失敗",
};

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `ZombiLine#eval_game_system_specific_command`。
///
/// `check_action(command) || roll_tables(command, self.class::TABLES)`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = check_action(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::result(result)));
    }
    match sys.tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(SpecificCommandOutput::text(table.roll_text(rng)?))),
    }
}

/// Ruby `ZombiLine#check_action`（判定 `xZL<=y`）。
fn check_action(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: round_type: @round_type（既定の FLOOR）
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["ZL"], RoundType::Floor)
            .enable_prefix_number()
            .disable_modifier()
            .restrict_cmp_op_to(&[Some(CmpOp::Le)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    let dice_count = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(1);
    // 比較演算子が `<=` に限定されているので目標値は必ず入る
    let Some(ref target_num) = parsed.target_number else {
        return Ok(None);
    };

    let mut dice_list = rng.roll_barabara(dice_count, 100)?;
    dice_list.sort_unstable();
    let is_success = dice_list
        .iter()
        .any(|&i| i <= crate::randomizer::sat_i64(target_num));
    let mut is_critical = dice_list.iter().any(|&i| i <= 5);
    let mut is_fumble = dice_list
        .iter()
        .any(|&i| i >= 96 && i > crate::randomizer::sat_i64(target_num));

    if is_critical && is_fumble {
        is_critical = false;
        is_fumble = false;
    }

    let success_message = if is_success && is_critical {
        sys.success_critical
    } else if is_success && is_fumble {
        sys.success_fumble
    } else if is_success {
        sys.success
    } else if is_fumble {
        sys.failure_fumble
    } else {
        sys.failure
    };

    let sequence = [
        format!("({})", parsed.to_s(SuffixPosition::AfterCommand)),
        format!("[{}]", join_dice(&dice_list)),
        success_message.to_owned(),
    ];

    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.set_condition(is_success);
    result.critical = is_critical;
    result.fumble = is_fumble;
    Ok(Some(result))
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice_list: &[i64]) -> String {
    dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ruby `BCDice::GameSystem::ZombiLine`（ID: `ZombiLine`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZombiLine;

impl GameSystem for ZombiLine {
    fn id(&self) -> &'static str {
        "ZombiLine"
    }

    fn name(&self) -> &'static str {
        "ゾンビライン"
    }

    fn sort_key(&self) -> &'static str {
        "そんひらいん"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 (xZL<=y)
　x：ダイス数(省略時は1)
　y：成功率

■ 各種表
　ストレス症状表 SST
　食材表 IT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*ZL", "SST", "IT"]
    }

    crate::impl_prefixes_pattern!();

    fn sides_implicit_d(&self) -> i64 {
        10
    }

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
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "ZombiLine",
            "ZombiLine.toml",
            23,
        );
    }
}
