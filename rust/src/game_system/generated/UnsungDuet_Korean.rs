//! P4で手書き移植した `lib/bcdice/game_system/UnsungDuet_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `UnsungDuet` を継承し `@locale = :ko_kr` で表を組み直すだけなので、
//! コマンド解釈は [`super::UnsungDuet`] の実装をそのまま使い、
//! ここには `ko_kr` ロケールの表データと定型文だけを置く。
//! `shifter>=5` のような判定は `AddDice` → `Base#result_ndx` を通るので、
//! `성공` / `실패` を返すために `result_ndx` も上書きする。

use super::UnsungDuet::{eval_specific_command, result_ndx_localized, SystemTables};
use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

static KO_HIN_ITEMS: &[&str] = &[
    "얼굴의 상처 → 얼굴에 생긴 상처. 천천히 피가 배어 나온다",
    "큰 부상 → 빨리 조치를 취하지 않으면 생명이 위험",
    "아프지 않은 상처 → 큰 상처인데 왠지 아프지 않다",
    "상실 → 몸의 어딘가가 사라져 버린다",
    "글씨 같은 상처 → 읽을 수 없는 글씨 같은 상처",
    "문양 같은 멍 → 무늬나 모양 같은 멍",
];
static KO_HIN: Table = Table::from_dice("변이표: 상처", 1, 6, KO_HIN_ITEMS);

static KO_HPH_ITEMS: &[&str] = &[
    "눈 앞이 흐려짐 → 눈의 초점이 맞지 않는다",
    "이명 → 고음이 계속 울리는 것 같다",
    "이상한 한기 → 얼어버릴 것처럼 춥게 느껴진다",
    "발한 → 덥지도 않은데 땀이 잔뜩 난다",
    "환각 → 이것이 진짜인지 환상인지 구별이 가지 않는다",
    "주마등 → 과거의 일을 계속해서 떠올리게 된다",
];
static KO_HPH: Table = Table::from_dice("변이표: 몸 상태의 변화", 1, 6, KO_HPH_ITEMS);

static KO_HFE_ITEMS: &[&str] = &[
    "불안 → 막연한 불안감이 마음을 좀먹는다",
    "좁은 장소가 두려움 → 좁은 곳에 들어가고 싶지 않다",
    "몸이 떨림 → 도무지 진정이 되지 않는다",
    "소리가 두려움 → 정말 작은 소리에도 겁이 난다",
    "어두운 곳이 두려움 → 빛이 없는 장소가 엄청나게 무섭다",
    "누가 뒤따라 옴 → 누군가가 뒤에 있는 것만 같다…",
];
static KO_HFE: Table = Table::from_dice("변이표: 공포", 1, 6, KO_HFE_ITEMS);

static KO_HFA_ITEMS: &[&str] = &[
    "유리화 → 몸의 일부가 유리처럼 투명해진다",
    "깃털 → 몸의 어딘가에서 깃털이 자라난다",
    "식물화 → 몸에서 덩굴이나 잎이 자라난다",
    "동물의 눈 → 눈 모양이 동물처럼 변한다",
    "뿔 → 이마나 머리 옆쪽에 뿔이 돋아난다",
    "도기화 → 피부가 도자기처럼 변한다",
];
static KO_HFA: Table = Table::from_dice("변이표: 환상화", 1, 6, KO_HFA_ITEMS);

static KO_HMI_ITEMS: &[&str] = &[
    "기억의 혼란 → 여기는 어디? 어쩌다 이런 데 왔지?",
    "유년기의 기억 → 말투나 태도가 어려진다",
    "솔직 → 생각한 것을 전부 말해 버린다",
    "만용 → 파트너를 지키기 위해 무모한 짓만 한다",
    "의심 → 무엇이든 안 좋은 방향으로만 생각한다",
    "먹어버리고 싶다 → 파트너를 베어 먹고 싶어진다",
];
static KO_HMI: Table = Table::from_dice("변이표: 정신", 1, 6, KO_HMI_ITEMS);

static KO_HOT_ITEMS: &[&str] = &[
    "그림자 → 몸의 일부가 그림자가 된다",
    "어항 → 몸의 일부가 어항처럼 된다",
    "눈물이 멈추지 않음 → 왠지 계속 눈물이 난다",
    "갈퀴 → 손이나 발에 짐승 같은 갈퀴손톱/발톱이 난다",
    "미래가 보임 → 앞일이 보이지만 진짜인지는 불명",
    "돌아가고 싶지 않다 → 현실에 돌아가기 싫다는 생각이 어렴풋이 든다",
];
static KO_HOT: Table = Table::from_dice("변이표: 기타", 1, 6, KO_HOT_ITEMS);

/// Ruby `TABLES`（`translate_tables(:ko_kr)`）。
static KO_TABLES: &[(&str, &Table)] = &[
    ("HIN", &KO_HIN),
    ("HPH", &KO_HPH),
    ("HFE", &KO_HFE),
    ("HFA", &KO_HFA),
    ("HMI", &KO_HMI),
    ("HOT", &KO_HOT),
];

/// i18n `ko_kr` の表と定型文。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: KO_TABLES,
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::UnsungDuet_Korean`（ID: `UnsungDuet:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsungDuet_Korean;

impl GameSystem for UnsungDuet_Korean {
    fn id(&self) -> &'static str {
        "UnsungDuet:Korean"
    }

    fn name(&self) -> &'static str {
        "언성 듀엣"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:언성 듀엣"
    }

    fn help_message(&self) -> &'static str {
        r"■ 시프터 용 판정 (shifter, UDS)
  1D10을 다이스롤 해서 판정을 행합니다.
  예） shifter, UDS, shifter>=5, shifter+1>=6

■ 바인더 용 판정 (binder, UDB)
  2D6을 다이스롤 해서 판정을 행합니다.
  예） binder, UDB, binder>=5, binder+1>=6

■ 변이표
  ・상처 (HIN, HInjury)
  ・몸 상태의 변화 (HPH, HPhysical)
  ・공포 (HFE, HFear)
  ・환상화 (HFA, HFantasy)
  ・정신 (HMI, HMind)
  ・기타 (HOT, HOther)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "shifter",
            "UDS",
            "binder",
            "UDB",
            "HINJURY",
            "HPHYSICAL",
            "HFEAR",
            "HFANTASY",
            "HMIND",
            "HOTHER",
            "HIN",
            "HPH",
            "HFE",
            "HFA",
            "HMI",
            "HOT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// `shifter>=5` / `binder>=9` は `AddDice.eval(…, self, …)` 経由でここに来る。
    /// Ruby は `translate("success")` が `@locale = :ko_kr` を見るので `성공` / `실패` になる。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        result_ndx_localized(&KO_SYSTEM, total, cmp_op, target)
    }

    /// Ruby `UnsungDuet#eval_game_system_specific_command`（`ko_kr` の表で）。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, self, command, rng)
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
            .join("test/data/UnsungDuet_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/UnsungDuet_Korean.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/UnsungDuet_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("UnsungDuet_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            28,
            "case count in test/data/UnsungDuet_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "UnsungDuet:Korean",
                "unexpected game system in UnsungDuet_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("UnsungDuet:Korean"), &tc.input, &mut src) {
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
                    "FAIL UnsungDuet_Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} UnsungDuet_Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
