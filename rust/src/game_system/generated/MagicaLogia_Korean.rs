//! `lib/bcdice/game_system/MagicaLogia_Korean.rb` の手書き移植。

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::CheckOutcome;

use super::MagicaLogia::{check_result_2d6, eval_specific_command, SystemText, PREFIXES};

static KO_KR: SystemText = SystemText {
    yaml: include_str!("../../../../i18n/MagicaLogia/ko_kr.yml"),
    success: "성공",
    failure: "실패",
    fumble: "펌블",
    rtt: "랜덤 특기 결정표(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
    rct: "랜덤 분야표(%<category_dice>d) ＞ %<category_name>s",
    rttn: "지정특기(%<category_name>s)표(%<row_dice>d) ＞ %<text>s",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct MagicaLogia_Korean;

impl GameSystem for MagicaLogia_Korean {
    fn id(&self) -> &'static str {
        "MagicaLogia:Korean"
    }
    fn name(&self) -> &'static str {
        "마기카로기아"
    }
    fn sort_key(&self) -> &'static str {
        "国際化:Korean:마기카로기아"
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
    fn sort_barabara_dice(&self) -> bool {
        true
    }
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(&KO_KR, total, dice_total, values, cmp_op, target)
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&KO_KR, command, rng)
    }
}

static HELP_MESSAGE: &str = r"・판정
스페셜／펌블／성공／실패
・각종 표
경력 표 BGT/ 초기 앵커 표 DAT/ 운명 속성 표 FAT
소원 표 WIT/ 프라이즈 표 PT
시간의 흐름 표(구판) TPT/ 대형판 시간의 흐름 표 TPTB
사건 표 AT
펌블 표 FT/ 상태이상 표 WT
운명 변전 표 FCT
　전형적 재액 TCT／물리계 재액 PCT／정신계 재액 MCT／광기계 재액 ICT
　사회계 재액 SCT／초자연계 재액 XCT／불가사의계 재액 WCT／코믹계 재액 CCT
　마법사 재액 MGCT
장면표 ST／대형판 장면표 STB
　극한 환경 XEST／내면 세계 IWST／마법 도시 MCST
　사후 세계 WDST／미궁 세계 LWST
　마법 서가 MBST／마법 학원 MAST／크레도의 탑 TCST
　병행 세계 PWST／종말 PAST／이세계 술집 GBST
　별빛 SLST／구 도서관 OLST
세계 법칙 추가 표 WLAT／떠돌이 괴물 표 WMT
랜덤 분야 표 RCT
랜덤 특기 표 RTT
　별 분야 랜덤 특기 표  RTS, RTT1
　짐승 분야 랜덤 특기 표  RTB, RTT2
　힘 분야 랜덤 특기 표  RTF, RTT3
　노래 분야 랜덤 특기 표  RTP, RTT4
　꿈 분야 랜덤 특기 표  RTD, RTT5
　어둠 분야 랜덤 특기 표  RTN, RTT6
백지 비밀 표　BST/
　숙적표　MIT/모략 표　MOT/인연 표　MAT
　기인표　MUT/역장 표　MFT/동맹 표　MLT
낙화 표　FFT
그 후의 전개 표 FLT
・D66 다이스 있음.
";

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        super::super::TokyoNova::assert_toml_cases(
            "MagicaLogia:Korean",
            "MagicaLogia_Korean.toml",
            155,
        );
    }
}
