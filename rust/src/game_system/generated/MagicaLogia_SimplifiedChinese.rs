//! `lib/bcdice/game_system/MagicaLogia_SimplifiedChinese.rb` の手書き移植。

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::CheckOutcome;

use super::MagicaLogia::{check_result_2d6, eval_specific_command, SystemText, PREFIXES};

static ZH_HANS: SystemText = SystemText {
    yaml: include_str!("../../../../i18n/MagicaLogia/zh_hans.yml"),
    success: "成功",
    failure: "失败",
    fumble: "大失败",
    rtt: "随机特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
    rct: "ランダム分野表(%<category_dice>d) ＞ %<category_name>s",
    rttn: "%<category_name>s领域随机特技表(%<row_dice>d) ＞ %<text>s",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct MagicaLogia_SimplifiedChinese;

impl GameSystem for MagicaLogia_SimplifiedChinese {
    fn id(&self) -> &'static str {
        "MagicaLogia:SimplifiedChinese"
    }
    fn name(&self) -> &'static str {
        "魔导书大战"
    }
    fn sort_key(&self) -> &'static str {
        "国際化:Simplified Chinese:魔导书大战"
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
        check_result_2d6(&ZH_HANS, total, dice_total, values, cmp_op, target)
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&ZH_HANS, command, rng)
    }
}

static HELP_MESSAGE: &str = r"・判定
可以判定大成功／大失败／成功／失败
・各种表
经历表　BGT/初期锚点表　DAT/命运属性表　FAT
愿望表　WIT/战利品表　PT
时间流逝表　TPT/大判时间流逝表　TPTB
事件表　AT
大失败表　FT／变调表　WT
命运转变表表　FCT
　典型性灾厄 TCT／物理性灾厄 PCT／精神性灾厄 MCT／狂气性灾厄 ICT
　社会性灾厄 SCT／超自然灾厄 XCT／不可思议的灾厄 WCT／喜剧性灾厄 CCT
　魔法使的灾厄 MGCT
场景表　ST／大判场景表　STB
　极限环境 XEST／内心世界 IWST／魔法都市 MCST
　死后世界 WDST／迷宫世界 LWST
　魔法书架 MBST／魔法学院 MAST／克雷德塔 TCST
　平行世界 PWST／终末世界 PAST／异世界酒吧 GBST
　星影 SLST／旧图书馆 OLST
世界法则追加表 WLAT/徘徊怪物表 WMT
随机领域表　RCT
随机特技表　RTT
　星领域随机特技表  RTS, RTT1
　兽领域随机特技表  RTB, RTT2
　力领域随机特技表  RTF, RTT3
　歌领域随机特技表  RTP, RTT4
　梦领域随机特技表  RTD, RTT5
　暗领域随机特技表  RTN, RTT6
空白秘密表　BST
　宿敌表　MIT/谋略表　MOT/因缘表　MAT
　奇人表　MUT/力场表　MFT/同盟表　MLT
落花表　FFT
那之后表 FLT
・可以使用D66
";

#[cfg(test)]
mod tests {
    /// `test/data/MagicaLogia_SimplifiedChinese.toml` の全ケースが通ること（共通ハーネス）。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "MagicaLogia:SimplifiedChinese",
            "MagicaLogia_SimplifiedChinese.toml",
            155,
        );
    }
}
