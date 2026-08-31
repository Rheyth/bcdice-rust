//! P4で手書き移植した `lib/bcdice/game_system/Nechronica_Korean.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! Ruby側は `Nechronica` を継承し、`TABLES` を `translate_tables(:ko_kr)` に差し替えて
//! `@locale` を `:ko_kr` に変えるだけなので、判定の実装は [`super::Nechronica`] の
//! ものをそのまま使い、ここには `ko_kr` ロケールの表と定型文だけを置く。
//!
//! 表と文言は `i18n/Nechronica/ko_kr.yml` と `i18n/ko_kr.yml`（`success` / `failure`）から
//! 機械的に書き出したもので、値は1文字も変えていない。

use super::Nechronica::{eval_specific_command, result_nd10, SystemTables};
use crate::dice_table::Table;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// i18n `Nechronica.table.NM.items`。
static KO_NM_ITEMS: &[&str] = &[
    "【혐오】[발광:적대인식]전투 동안 적에게 명중하지 않은 공격은 모두, 사정 거리 내에 있다면 혐오의 대상에게 명중한다(방어 측 임의).",
    "【독점】[발광:독점충동]전투 시작 시와 종료 시 각각 1개씩, 대상은 파츠를 선택하여 손상시킨다.",
    "【의존】[발광:유아퇴행]최대 행동치가 감소한다(-2).",
    "【집착】[발광:추적감시]전투 시작 시와 종료 시 각각 1개씩, 대상은 당신에 대한 미련에 광기점을 얻는다.",
    "【연심】[발광:자해행동]전투 시작 시와 종료 시 각각 1개씩, 당신은 파츠를 선택해 손상시킨다.",
    "【대항】[발광:과잉경쟁]전투 시작 시와 종료 시 각각 1개씩, 당신은 임의의 미련에 광기점을 추가로 얻는다.",
    "【우정】[발광:공명의존]세션 종료 시, 대상이 당신보다 손상된 파츠를 더 많이 가지고 있다면, 당신은 대상과 같은 수의 손상된 파츠를 가질 때까지 자신의 파츠를 손상시킨다.",
    "【보호】[발광:상시밀착]전투 동안 미련의 대상과 다른 에리어에 있다면「이동 이외의 효과를 가진 매뉴버」를 선언할 수 없다. 또한「자신과 미련의 대상」이외를 이동 매뉴버의 대상으로 지정할 수 없다.",
    "【동경】[발광:거짓망상]전투 동안 미련의 대상과 같은 에리어에 있다면「이동 이외의 효과를 가진 매뉴버」를 선언할 수 없다. 또한「자신과 미련의 대상」이외를 이동 매뉴버의 대상으로 지정할 수 없다.",
    "【신뢰】[발광:의심암귀]당신을 제외한 모든 자매의 최대 행동치가 감소한다(-1).",
];
/// i18n `Nechronica.table.NM`（자매에 대한 미련표 / 1D10）。
static KO_NM: Table = Table::from_dice("자매에 대한 미련표", 1, 10, KO_NM_ITEMS);

/// i18n `Nechronica.table.NMN.items`。
static KO_NMN_ITEMS: &[&str] = &[
    "【기피】[발광:격리의식]당신은 미련의 대상 또는 서번트와 같은 에리어에 있는 동안「이동 이외의 효과를 가진 매뉴버」를 선언할 수 없다. 또한,「자신과 미련의 대상 또는 서번트」를 제외한 다른 대상은 이동 매뉴버의 대상으로 지정할 수 없다.",
    "【질투】[발광:불협화음]모든 자매는 행동 판정에 수정-1을 받는다.",
    "【의존】[발광:유아퇴행]최대 행동치가 감소한다(-2).",
    "【연민】[발광:과다몰입]당신은「서번트」에 대한 공격판정의 결과에 수정-1을 받는다.",
    "【감사】[발광:병적보복]발광 상태가 되었을 때, 당신은 임의의 기본 파츠 2개(없다면 가장 낮은 레벨의 강화 파츠 1개)를 손상시킨다.",
    "【회한】[발광:자업자득]당신이 실패한 공격 판정은 모두 당신 자신의 임의의 파츠에 대미지를 입힌다.",
    "【기대】[발광:희망전결]당신은 광기점을 추가하여 재굴림할때, 그 결과에 -1의 수정을 받는다(이 효과는 누적된다).",
    "【보호】[발광:생전회귀]당신은「레기온」을 매뉴버의 대상으로 선택할 수 없다.",
    "【존경】[발광:신화숭배]당신은「다른 자매들」을 매뉴버의 대상으로 선택할 수 없다.",
    "【신뢰】[발광:의심암귀]당신을 제외한 모든 자매의 최대 행동치가 감소한다(-1).",
];
/// i18n `Nechronica.table.NMN`（중립자에 대한 미련표 / 1D10）。
static KO_NMN: Table = Table::from_dice("중립자에 대한 미련표", 1, 10, KO_NMN_ITEMS);

/// i18n `Nechronica.table.NME.items`。
static KO_NME_ITEMS: &[&str] = &[
    "【공포】[발광:인식거부]당신은 행동 판정 및 광기 판정의 결과에 -1의 수정치를 받는다.",
    "【종속】[발광:조반유리]당신이 실패한 공격 판정은 모두 대실패로 처리된다.",
    "【불안】[발광:거동수상]최대 행동치가 감소한다(-2).",
    "【연민】[발광:과다몰입]당신은「서번트」에 대한 공격판정의 결과에 수정-1을 받는다.",
    "【애증】[발광:흉애심중]당신은 광기 판정 및 공격 판정에서 대성공을 거둘 때마다 [판정값 -10] 개의 자신의 파츠를 선택하여 손상시킨다.",
    "【회한】[발광:자업자득]당신이 실패한 공격 판정은 모두 당신 자신의 임의의 파츠에 대미지를 입힌다.",
    "【경멸】[발광:안중부재]같은 에리어 내의 동료가 당신에게 가하는 공격 판정의 결과는 수정 +1을 받는다.",
    "【분노】[발광:격정폭주]당신은 공격 판정 및 광기 판정의 결과에 수정 -1을 받는다.",
    "【원한】[발광:불구대천]당신은 도주 판정을 할 수 없다. 당신이「자신과 미련의 대상」이외를 대상으로 삼아 매뉴버를 사용할 때, 행동치 1점을 추가로 감소시켜야 한다.",
    "【증오】[발광:흔적파괴]이 미련이 발광할 때, 당신을 제외한 자매 중 한 명을 선택한다. 그 자매는 임의의 파츠 2개를 손상시킨다.",
];
/// i18n `Nechronica.table.NME`（적에 대한 미련표 / 1D10）。
static KO_NME: Table = Table::from_dice("적에 대한 미련표", 1, 10, KO_NME_ITEMS);

/// i18n `Nechronica.hit_location.table`。
static KO_HIT_LOCATION: &[&str] = &[
    "방어측 임의",
    "다리（없으면 공격측 임의）",
    "몸통（없으면 공격측 임의）",
    "팔（없으면 공격측 임의）",
    "머리（없으면 공격측 임의）",
    "공격측 임의",
];

/// `ko_kr` ロケールの表と定型文一式。
static KO_SYSTEM: SystemTables = SystemTables {
    tables: &[("NM", &KO_NM), ("NMN", &KO_NMN), ("NME", &KO_NME)],
    hit_location: KO_HIT_LOCATION,
    additional_damage: "(추가 데미지%{damage})",
    critical: "대성공",
    fumble: "대실패",
    break_all_parts: "사용파츠 전부 손실",
    success: "성공",
    failure: "실패",
};

/// Ruby `BCDice::GameSystem::Nechronica_Korean`（ID: `Nechronica:Korean`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nechronica_Korean;

impl GameSystem for Nechronica_Korean {
    fn id(&self) -> &'static str {
        "Nechronica:Korean"
    }

    fn name(&self) -> &'static str {
        "네크로니카"
    }

    fn sort_key(&self) -> &'static str {
        "国際化:Korean:네크로니카"
    }

    fn help_message(&self) -> &'static str {
        r"・판정　(nNC+m)
　주사위 수n, 수정치m으로 판정굴림을 행합니다.
　주사위 수가 2개 이상일 때에 파츠파손 수도 표시합니다.
・공격판정　(nNA+m)
　주사위 수n, 수정치m으로 공격판정굴림을 행합니다.
　명중부위와 주사위 수가 2개 이상일 때에 파츠파손 수도 표시합니다.

표
・자매에 대한 미련표 nm
・중립자에 대한 미련표 nmn
・적에 대한 미련표 nme
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d?NC", r"\d?NA", r"\dR10", "NM", "NMN", "NME"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Nechronica#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Nechronica#initialize` の `@sort_barabara_dice = true`。
    fn sort_barabara_dice(&self) -> bool {
        true
    }

    /// Ruby `Nechronica#initialize` の `@default_target_number = 6`。
    fn default_target_number(&self) -> Option<i64> {
        Some(6)
    }

    /// Ruby `Base#result_ndx`（`ko_kr` の定型文で）。
    ///
    /// Ruby側は `translate("success")` が `@locale`（このクラスでは `:ko_kr`）を見るため
    /// `성공` / `실패` になる。トレイトの既定実装は `ja_jp` 固定の
    /// `成功` / `失敗` を返すので、ここで上書きする。
    /// 10面以外のダイス（`result_nd10` を通らない判定）がこの経路を通る。
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        // Ruby: return nil if target.is_a?(String)（目標値 "?"）
        let Target::Number(target) = target else {
            return None;
        };
        if cmp_op.apply(&total, &target) {
            Some(EvalResult::success(KO_SYSTEM.success))
        } else {
            Some(EvalResult::failure(KO_SYSTEM.failure))
        }
    }

    /// Ruby `Nechronica#result_nd10`。
    fn result_nd10(
        &self,
        total: crate::Int,
        _dice_total: i64,
        value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        result_nd10(&KO_SYSTEM, total, value_list, cmp_op, target)
    }

    /// Ruby `Nechronica#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_SYSTEM, command, rng)
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
            .join("test/data/Nechronica_Korean.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Nechronica_Korean.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Nechronica_Korean.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Nechronica_Korean.toml must parse");
        assert_eq!(
            data.tests.len(),
            28,
            "case count in test/data/Nechronica_Korean.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Nechronica:Korean",
                "unexpected game system in Nechronica_Korean.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Nechronica:Korean"), &tc.input, &mut src) {
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
                    "FAIL Nechronica:Korean:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Nechronica:Korean cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 10面以外のダイス判定が `ko_kr` の定型文になること。
    ///
    /// Ruby は `Base#result_ndx` の `translate("success")` が `@locale = :ko_kr` を見るため
    /// `성공` / `실패` になる。TOMLにこの経路のケースが無いのでここで固定する。
    #[test]
    fn result_ndx_uses_ko_kr_wording() {
        let cases = [
            (
                "2D6>=7",
                vec![(4, 6), (5, 6)],
                "(2D6>=7) ＞ 9[4,5] ＞ 9 ＞ 성공",
            ),
            (
                "2D6>=10",
                vec![(4, 6), (5, 6)],
                "(2D6>=10) ＞ 9[4,5] ＞ 9 ＞ 실패",
            ),
        ];
        for (input, rands, expected) in cases {
            let mut src = SeededRandomizer::new(rands);
            let result = eval_command(&GameSystemId::new("Nechronica:Korean"), input, &mut src)
                .expect("eval")
                .expect("result");
            assert_eq!(result.text, expected, "input {input:?}");
            assert!(src.is_empty(), "unconsumed rands for {input:?}");
        }
    }
}
