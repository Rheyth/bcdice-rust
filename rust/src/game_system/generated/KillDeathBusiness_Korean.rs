//! P4で手書き移植した `lib/bcdice/game_system/KillDeathBusiness_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `KillDeathBusiness` を継承し、`@locale` を `:ko_kr` に変えて表を組み直すだけなので、
//! コマンド解釈・判定・表の引き方は [`super::KillDeathBusiness`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データ（`KO_` 接頭辞の `static` 群）だけを置く。
//!
//! データは `i18n/KillDeathBusiness/ko_kr.yml` から機械抽出したもので、値は1文字も変えていない。
//!
//! # ja_jp へのフォールバック
//!
//! `i18n/KillDeathBusiness/ko_kr.yml` には次のキーが無く、Ruby側は
//! `I18n.fallbacks.defaults = [:ja_jp]` により ja_jp の値を使う。ここでも
//! [`super::KillDeathBusiness`] の `JA_` な `static` をそのまま指す。
//!
//! - `KillDeathBusiness.table.ANSPT` / `MASPT` / `MOSPT` / `PASPT` / `POSPT` / `UMSPT`
//! - `KillDeathBusiness.table.WOTA` / `WOTB` / `WOTC` / `VOT` / `LOT`
//! - `KillDeathBusiness.table.TOT` / `OOT` / `POT` / `NOT`

use super::KillDeathBusiness::{
    check_result_2d6, eval_specific_command, EstSubTable, JdTexts, SystemTables, JA_ANSPT, JA_LOT,
    JA_MASPT, JA_MOSPT, JA_NOT_ITEM1, JA_NOT_ITEM2, JA_NOT_ITEM3, JA_NOT_ITEM4, JA_NOT_ITEM5,
    JA_NOT_ITEM6, JA_NOT_NAME, JA_OOT_ITEM1, JA_OOT_ITEM3, JA_OOT_ITEM5, JA_OOT_NAME, JA_PASPT,
    JA_POSPT, JA_POT_ITEM1, JA_POT_ITEM2, JA_POT_ITEM3, JA_POT_ITEM4, JA_POT_ITEM5, JA_POT_ITEM6,
    JA_POT_NAME, JA_TOT_ITEM1, JA_TOT_ITEM2, JA_TOT_ITEM3, JA_TOT_ITEM4, JA_TOT_ITEM5,
    JA_TOT_ITEM6, JA_TOT_NAME, JA_UMSPT, JA_VOT, JA_WOTA, JA_WOTB, JA_WOTC,
};
use crate::dice_table::{RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `BCDice::GameSystem::KillDeathBusiness_Korean`（ID: `KillDeathBusiness:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillDeathBusiness_Korean;

impl GameSystem for KillDeathBusiness_Korean {
    fn id(&self) -> &'static str {
        "KillDeathBusiness:Korean"
    }

    fn name(&self) -> &'static str {
        "Kill Death Business (한국어)"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:Kill Death Business"
    }

    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }

    fn prefixes(&self) -> &'static [&'static str] {
        PREFIXES
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Base#result_ndx`（`@locale = :ko_kr` のため i18n `success` / `failure` が韓国語）。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        match target {
            Target::Question => None,
            Target::Number(t) => {
                if cmp_op.apply(&total, &t) {
                    Some(EvalResult::success("성공"))
                } else {
                    Some(EvalResult::failure("실패"))
                }
            }
        }
    }

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&KO_SYSTEM, total, dice_total, cmp_op, target)
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
    }
}

/// Ruby `HELP_MESSAGE`。
static HELP_MESSAGE: &str = r"・판정
　JDx or JDx+y or JDx-y or JDx,z or JDx+y,z JDx-y,z
　（x＝난이도、y＝보정、z＝펌블도(리스크)）
・이력표 (HST、HSTx) x에 숫자(1,2)로 표를 개별 롤
・소원표 (-WT)
　죽음(DWT)、복수(RWT)、승리(VWT)、획득(PWT)、지배(CWT)、번영(FWT)
　강화(IWT)、건강(HWT)、안전(SAWT)、장생(LWT)、삶(EWT)
・만능이름표 (NAME) x에 숫자(1,2,3)로 표를 개별 롤
・서브플롯표 (-SPT)
　오컬트(OSPT)、가족(FSPT)、연애(LOSPT)、정의(JSPT)、수행(TSPT)
　웃음(BSPT)、심술쟁이(MASPT)、원한(UMSPT)、인기(POSPT)、구분(PASPT)
　돈벌이(MOSPT)、대(対)악마(ANSPT)
・씬 표 (ST)、서비스 씬 표 (EST)
・CM표 (CMT)
・소생 부작용 표 (ERT)
・일주일간 표（WKT)
・소울 방출표 (SOUL)
・범용연출표 (STGT)
・헬 스타일리스트 매도표 (HSAT、HSATx) x에 숫자(1,2)로 표를 개별 롤
・지정특기 랜덤 결정표 (RTT, SKLT)、지정특기 분야 랜덤 결정표 (RCT, SKLJ)
・엑스트라 표 (EXT、EXTx) x에 숫자(1,2,3,4)로 표를 개별 롤
・제작위원 결정표　PCDT/실제 어떠했는가 표　OHT
・태스크 표　헬 라이온　PCT1/헬 크로우　PCT2/헬 스네이크　PCT3/
　헬 드래곤　PCT4/헬 플라이　PCT5/헬 갓　PCT6/헬 베어　PCT7
・D66 다이스 지원
";

/// Ruby `register_prefix(...)`。スタブの配列をそのまま維持する。
static PREFIXES: &[&str] = &[
    "ST[1-2]?",
    "NAME[1-3]?",
    "EST",
    "sErviceST",
    "HSAT[1-2]?",
    "EXT[1-4]?",
    "JD",
    "TOT",
    "OOT",
    "WOT",
    "POT",
    "NOT",
    "DEATHWT",
    "REVENGEWT",
    "VICTORYWT",
    "POSSESIONWT",
    "CONTROLWT",
    "FLOURISHWT",
    "INTENSIFYWT",
    "HEALTHWT",
    "SAFETYWT",
    "LONGEVITYWT",
    "EXISTWT",
    "OCCULTSPT",
    "FAMILYSPT",
    "LOVESPT",
    "JUSTICESPT",
    "TRAININGSPT",
    "BEAMSPT",
    "HST",
    "DWT",
    "RWT",
    "VWT",
    "PWT",
    "CWT",
    "FWT",
    "IWT",
    "HWT",
    "SAWT",
    "LWT",
    "EWT",
    "OSPT",
    "FSPT",
    "LOSPT",
    "JSPT",
    "TSPT",
    "BSPT",
    "CMT",
    "ERT",
    "WKT",
    "SOUL",
    "STGT",
    "PCDT",
    "OHT",
    "PCT1",
    "PCT2",
    "PCT3",
    "PCT4",
    "PCT5",
    "PCT6",
    "PCT7",
    "ANSPT",
    "MASPT",
    "MOSPT",
    "PASPT",
    "POSPT",
    "UMSPT",
    "WOTA",
    "WOTB",
    "WOTC",
    "VOT",
    "LOT",
    "ST[1-2]?",
    "NAME[1-3]?",
    "EST",
    "sErviceST",
    "HSAT[1-2]?",
    "EXT[1-4]?",
    "JD",
    "TOT",
    "OOT",
    "WOT",
    "POT",
    "NOT",
    "DEATHWT",
    "REVENGEWT",
    "VICTORYWT",
    "POSSESIONWT",
    "CONTROLWT",
    "FLOURISHWT",
    "INTENSIFYWT",
    "HEALTHWT",
    "SAFETYWT",
    "LONGEVITYWT",
    "EXISTWT",
    "OCCULTSPT",
    "FAMILYSPT",
    "LOVESPT",
    "JUSTICESPT",
    "TRAININGSPT",
    "BEAMSPT",
    "RTT[1-6]?",
    "RCT",
    "SKLT",
    "SKLJ",
];

const NO_RTTN_ALIASES: &[&str] = &[];

include!("KillDeathBusiness_ko_data.rs");

static KO_EST_TABLES: &[EstSubTable] = &[
    EstSubTable {
        name: KO_EST_UNDRESSING_NAME,
        items: KO_EST_UNDRESSING_ITEMS,
    },
    EstSubTable {
        name: KO_EST_VIOLENCE_NAME,
        items: KO_EST_VIOLENCE_ITEMS,
    },
    EstSubTable {
        name: KO_EST_TRAVEL_NAME,
        items: KO_EST_TRAVEL_ITEMS,
    },
    EstSubTable {
        name: KO_EST_LOVE_NAME,
        items: KO_EST_LOVE_ITEMS,
    },
    EstSubTable {
        name: KO_EST_EMOTION_NAME,
        items: KO_EST_EMOTION_ITEMS,
    },
    EstSubTable {
        name: KO_EST_OTHER_GENRE_NAME,
        items: KO_EST_OTHER_GENRE_ITEMS,
    },
];

static KO_RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new(KO_RTT_CAT1_NAME, KO_RTT_CAT1_SKILLS),
    SaiFicCategory::new(KO_RTT_CAT2_NAME, KO_RTT_CAT2_SKILLS),
    SaiFicCategory::new(KO_RTT_CAT3_NAME, KO_RTT_CAT3_SKILLS),
    SaiFicCategory::new(KO_RTT_CAT4_NAME, KO_RTT_CAT4_SKILLS),
    SaiFicCategory::new(KO_RTT_CAT5_NAME, KO_RTT_CAT5_SKILLS),
    SaiFicCategory::new(KO_RTT_CAT6_NAME, KO_RTT_CAT6_SKILLS),
];

static KO_RTT: SaiFicSkillTable = SaiFicSkillTable::new(KO_RTT_CATEGORIES)
    .with_commands(Some("SKLT"), Some("SKLJ"), NO_RTTN_ALIASES)
    .with_formats(SaiFicFormats {
        rtt: KO_RTT_RTT_FORMAT,
        rct: KO_RTT_RCT_FORMAT,
        rttn: KO_RTT_RTTN_FORMAT,
        skill: KO_RTT_S_FORMAT,
    });

static KO_ROLL_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("HST", &KO_HST),
    ("DWT", &KO_DWT),
    ("RWT", &KO_RWT),
    ("VWT", &KO_VWT),
    ("PWT", &KO_PWT),
    ("CWT", &KO_CWT),
    ("FWT", &KO_FWT),
    ("IWT", &KO_IWT),
    ("HWT", &KO_HWT),
    ("SAWT", &KO_SAWT),
    ("LWT", &KO_LWT),
    ("EWT", &KO_EWT),
    ("OSPT", &KO_OSPT),
    ("FSPT", &KO_FSPT),
    ("LOSPT", &KO_LOSPT),
    ("JSPT", &KO_JSPT),
    ("TSPT", &KO_TSPT),
    ("BSPT", &KO_BSPT),
    ("CMT", &KO_CMT),
    ("ERT", &KO_ERT),
    ("WKT", &KO_WKT),
    ("SOUL", &KO_SOUL),
    ("STGT", &KO_STGT),
    ("PCDT", &KO_PCDT),
    ("OHT", &KO_OHT),
    ("PCT1", &KO_PCT1),
    ("PCT2", &KO_PCT2),
    ("PCT3", &KO_PCT3),
    ("PCT4", &KO_PCT4),
    ("PCT5", &KO_PCT5),
    ("PCT6", &KO_PCT6),
    ("PCT7", &KO_PCT7),
    ("ANSPT", &JA_ANSPT),
    ("MASPT", &JA_MASPT),
    ("MOSPT", &JA_MOSPT),
    ("PASPT", &JA_PASPT),
    ("POSPT", &JA_POSPT),
    ("UMSPT", &JA_UMSPT),
    ("WOTA", &JA_WOTA),
    ("WOTB", &JA_WOTB),
    ("WOTC", &JA_WOTC),
    ("VOT", &JA_VOT),
    ("LOT", &JA_LOT),
];

static KO_SYSTEM: SystemTables = SystemTables {
    fumble: KO_FUMBLE,
    special: KO_SPECIAL,
    jd: JdTexts {
        name: KO_JD_NAME,
        warn_over_target: KO_JD_WARN_OVER_TARGET,
        warn_min_target: KO_JD_WARN_MIN_TARGET,
        warn_over_fumble: KO_JD_WARN_OVER_FUMBLE,
        options: KO_JD_OPTIONS,
        dice_value: KO_JD_DICE_VALUE,
        fumble: KO_JD_FUMBLE,
        special: KO_JD_SPECIAL,
        less_than_fumble: KO_JD_LESS_THAN_FUMBLE,
        failure: KO_JD_FAILURE,
        success: KO_JD_SUCCESS,
    },
    st_name: KO_ST_NAME,
    st_format: KO_ST_FORMAT,
    st_table1: KO_ST_TABLE1,
    st_table2: KO_ST_TABLE2,
    name_name: KO_NAME_NAME,
    name_table1: KO_NAME_TABLE1,
    name_table2: KO_NAME_TABLE2,
    name_table3: KO_NAME_TABLE3,
    est_name: KO_EST_NAME,
    est_format: KO_EST_FORMAT,
    est_tables: KO_EST_TABLES,
    hsat_name: KO_HSAT_NAME,
    hsat_abuse1: KO_HSAT_ABUSE1,
    hsat_abuse2: KO_HSAT_ABUSE2,
    hsat_prefix: KO_HSAT_PREFIX,
    hsat_suffix: KO_HSAT_SUFFIX,
    ext_name: KO_EXT_NAME,
    ext_table1: KO_EXT_TABLE1,
    ext_table2: KO_EXT_TABLE2,
    ext_table3: KO_EXT_TABLE3,
    ext_table4: KO_EXT_TABLE4,
    tables: KO_ROLL_TABLES,
    rtt: &KO_RTT,
    wota: &JA_WOTA,
    wotb: &JA_WOTB,
    wotc: &JA_WOTC,
    vot: &JA_VOT,
    lot: &JA_LOT,
    tot_name: JA_TOT_NAME,
    tot_item1: JA_TOT_ITEM1,
    tot_item2: JA_TOT_ITEM2,
    tot_item3: JA_TOT_ITEM3,
    tot_item4: JA_TOT_ITEM4,
    tot_item5: JA_TOT_ITEM5,
    tot_item6: JA_TOT_ITEM6,
    oot_name: JA_OOT_NAME,
    oot_item1: JA_OOT_ITEM1,
    oot_item3: JA_OOT_ITEM3,
    oot_item5: JA_OOT_ITEM5,
    pot_name: JA_POT_NAME,
    pot_item1: JA_POT_ITEM1,
    pot_item2: JA_POT_ITEM2,
    pot_item3: JA_POT_ITEM3,
    pot_item4: JA_POT_ITEM4,
    pot_item5: JA_POT_ITEM5,
    pot_item6: JA_POT_ITEM6,
    not_name: JA_NOT_NAME,
    not_item1: JA_NOT_ITEM1,
    not_item2: JA_NOT_ITEM2,
    not_item3: JA_NOT_ITEM3,
    not_item4: JA_NOT_ITEM4,
    not_item5: JA_NOT_ITEM5,
    not_item6: JA_NOT_ITEM6,
};

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "KillDeathBusiness:Korean",
            "KillDeathBusiness_Korean.toml",
            133,
        );
    }
}
