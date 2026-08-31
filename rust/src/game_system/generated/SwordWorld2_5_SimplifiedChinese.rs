//! P4で手書き移植した `lib/bcdice/game_system/SwordWorld2_5_SimplifiedChinese.rb`。
//!
//! Ruby側は `SwordWorld2_5` を継承して `@locale = :zh_hans` にするだけなので、
//! ここでも固有コマンドは `SwordWorld2_5::eval_specific_command` に委譲し、
//! `i18n/SwordWorld2_5/zh_hans.yml` にある差分（アビスカース表・分類決定表・属性決定表）だけを持つ。
//! `zh_hans.yml` に無い追加アビスカース表と特殊失敗の文言は、Ruby の
//! `I18n.fallbacks.defaults = [:ja_jp]` と同じく日本語版を使う。

use super::SwordWorld::check_result_2d6;
use super::SwordWorld2_0::ZH_TEXT as ZH_TEXT_2_0;
use super::SwordWorld2_5::{eval_specific_command, SystemText, JA_ADDITIONAL_ABYSS_CURSE_TABLE};
use crate::dice_table::{D66GridTable, D66ParityTable};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

static ZH_ABYSS_CURSE_ITEMS: &[&[&str]] = &[
    &[
        "「自伤的」　装备时　该武具的装备者造成暴击时，装备者的HP减少5点。",
        "「悲叹的」　装备时　周围有敌人时，会紧张到泪流不止。在战斗中无法选取「射程：施术者」和「射程：接触」以外的效果对象。",
        "「仁慈的」　装备时　会对敌人心存怜悯。以敌对角色为对象时，如果对象已经减少了1点以上的HP，则命中力判定、魔法行使判定受到-2的惩罚修正。",
        "「差别的」　装备时　给予特定分类角色的物理伤害・魔法伤害减少2点。分类由「分类决定表」随机决定。",
        "「脆弱的」　装备时　每次受到魔法伤害时，该伤害+1点。",
        "「无谋的」　装备时　防护点减少2点（最低0）。",
    ],
    &[
        "「沉重的」　装备时　被强化的武具的必要力量+2。威力、防护点等保持不变。",
        "「难用的」　装备时　不管使用哪一行威力表，③④栏的数值都与威力无关地变成「0」（不是自动失败）。",
        "「软弱的」　装备时　精神抵抗力判定受到-1的惩罚修正。",
        "「病弱的」　装备时　生命抵抗力判定受到-1的惩罚修正。",
        "「过敏的」　装备时　受到特定属性的物理伤害、魔法伤害上升2点。属性由「属性决定表」随机决定。",
        "「开朗的」　装备时　每次在精神抵抗判定中失败时，就会笑个不停。直到下回合结束时为止，行动判定（『Ⅰ』123页）受到-1的惩罚修正。这个效果会累积。",
    ],
    &[
        "「结巴的」　装备时　说话时容易想不到词或说错。魔法行使判定受到-1的惩罚修正。",
        "「代言的」　装备时　自己的话会被武具原封不动地以难以听清的魔法文明语说出。装备这个武具时无法使用魔法文明语以外的语言对话，无法行使妖精魔法、魔动机术。",
        "「拒绝施舍的」　装备时　战斗中，接受「抵抗：任意」的效果时，生命抵抗力、精神抵抗力受到-2的惩罚修正直到下回合开始。",
        "「接近死亡的」　携带时　生死判定始终受到和「冒险者等级」相同的惩罚修正。",
        "「时髦的」　携带时　始终想把该武具装点得华美。每次获得收入时，必须花费此收入的1成以上用于装饰武具（没有效果之类的变化）。",
        "「吸收玛娜的」　携带时　魔法或练技等，以自己的意思使用消耗MP的效果时，所有MP消耗上升1点。",
    ],
    &[
        "「迟钝的」　携带时　移动力变为一半（小数进位）。",
        "「不准的」　携带时　战斗中回合开始时掷1d，出目为「1」则《目标瞄准》及以此为前提的战斗特技是为没有习得。",
        "「错乱的」　携带时　战斗中回合开始时掷1d，出目为「1」则使用包含近距离攻击在内的「射程：接触」的效果时，对象从和装备者同位置（区域、坐标）的所有角色（包含敌我）中随机选择。",
        "「绊脚的」　携带时　战斗中回合开始时掷1d，出目为「1」则当场摔倒。回合中不能站起来。",
        "「滑落的」　携带时　战斗中回合开始时掷1d，出目为「1」则手部装备及拿着的东西全部掉在原地（该回合可以用主动作捡起）。",
        "「散发恶臭的」　携带时　散发强烈的恶臭。只是携带着就会让其他角色感到不适，隐蔽判定受到-2的惩罚修正。且冒险者阶级（『Ⅱ』137页）视为低1级。",
    ],
    &[
        "「丑恶的」　携带时　武具外观丑陋，毫无魅力。贩卖该武具时只能卖到基本交易价格的4分之1。且冒险者阶级（『Ⅱ』137页）视为低1级。",
        "「嗡鸣的」　携带时　该武具中始终响着飞虫飞舞的声音。隐蔽判定、危险感知判定受到-4的惩罚修正。",
        "「湿软的」　携带时　武具质感像是吸了水一样软塌塌的。追加伤害-2（武器）、防护点-1（铠、盾）。对疾病属性的效果的生命抵抗力、精神抵抗力受到-4的惩罚修正。",
        "「旧伤的」　携带时　受到回复HP的效果时（包含由于休息造成的回复），该回复量减少1点。",
        "「炫目的」　携带时　通过反射光芒等方式发出强烈的光辉。自己始终由于视野不良受到-1的惩罚修正。",
        "「无荣光的」　携带时　行为判定自动成功时，视为没有自动成功，并重投2d，采用新的出目。这个效果1天只生效1次。",
    ],
    &[
        "「正直者的」　携带时　谎言和手段变得很容易暴露。成为真伪判定的对象时，受到-4的惩罚修正。",
        "「晕动的」　携带时　对摇晃的抵抗力减弱。以自己步行之外的方式移动10分钟以上后，1小时内行动判定受到-1的惩罚修正。",
        "「厌恶碧绿的」　携带时　在自然环境中会变得无法冷静。在自然环境（『Ⅰ』108页）中行动判定受到-1的惩罚修正。",
        "「无法忍耐的」　携带时　游戏中每次1天开始时，必须花费「冒険者等级×10」加美尔用于兴趣和嗜好品等的支出。若处于无法消费兴趣和嗜好品的环境中，直到第二天早上为止，HP上限、MP上限减少「冒険者等级」点。",
        "「阴魂不散的」　携带时　这个武具不知不觉就会出现在身边。以该武具以外的武具进行的命中力判定（武器）、魔法行使判定（武器）、回避力判定（铠、盾）会受到-4的惩罚修正。",
        "「慢吞吞的」　携带时　战斗开始时无法进行「战斗准备」。",
    ],
];
static ZH_ABYSS_CURSE_TABLE: D66GridTable = D66GridTable::new("奈落诅咒表", ZH_ABYSS_CURSE_ITEMS);

static ZH_ABYSS_CURSE_CATEGORY_ODD: &[&str] = &[
    "蛮族",
    "动物",
    "植物",
    "不死生物",
    "魔法生物",
    "在「蛮族」「动物」「植物」「不死生物」「魔法生物」中任选一个",
];
static ZH_ABYSS_CURSE_CATEGORY_EVEN: &[&str] = &[
    "魔动机",
    "幻兽",
    "妖精",
    "魔神",
    "人族",
    "在「魔动机」「幻兽」「妖精」「魔神」「人族」中任选一个",
];
static ZH_ABYSS_CURSE_CATEGORY_TABLE: D66ParityTable = D66ParityTable::new(
    "奈落诅咒分类决定表",
    ZH_ABYSS_CURSE_CATEGORY_ODD,
    ZH_ABYSS_CURSE_CATEGORY_EVEN,
);

static ZH_ABYSS_CURSE_ATTR_ODD: &[&str] = &["土", "水・冰", "炎", "风", "雷", "纯能量"];
static ZH_ABYSS_CURSE_ATTR_EVEN: &[&str] = &["断空", "冲击", "毒", "疾病", "诅咒", "精神效果"];
static ZH_ABYSS_CURSE_ATTR_TABLE: D66ParityTable = D66ParityTable::new(
    "奈落诅咒属性决定表",
    ZH_ABYSS_CURSE_ATTR_ODD,
    ZH_ABYSS_CURSE_ATTR_EVEN,
);

static ZH_TEXT: SystemText = SystemText {
    base: &ZH_TEXT_2_0,
    // zh_hans.yml に `special_failure` が無く、ja_jp へ fallback する
    special_failure: "特殊失敗",
    abyss_curse: &ZH_ABYSS_CURSE_TABLE,
    abyss_curse_category: &ZH_ABYSS_CURSE_CATEGORY_TABLE,
    abyss_curse_attr: &ZH_ABYSS_CURSE_ATTR_TABLE,
    // zh_hans.yml に `AdditionalAbyssCurseTable` が無く、ja_jp へ fallback する
    additional_abyss_curse: &JA_ADDITIONAL_ABYSS_CURSE_TABLE,
};

/// Ruby `BCDice::GameSystem::SwordWorld2_5_SimplifiedChinese`（ID: `SwordWorld2.5:SimplifiedChinese`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwordWorld2_5_SimplifiedChinese;

impl GameSystem for SwordWorld2_5_SimplifiedChinese {
    fn id(&self) -> &'static str {
        "SwordWorld2.5:SimplifiedChinese"
    }
    fn name(&self) -> &'static str {
        "剑世界2.5"
    }
    fn sort_key(&self) -> &'static str {
        "国際化:Simplified Chinese:剑世界2.5"
    }
    fn help_message(&self) -> &'static str {
        r"自动成功、成功、失败、自动失败会自动判定。

  ・威力表　(Kx)
  　指令为“K威力值+加值”的格式。
  　加值的部分不能是像“K20+K30”这样的威力的写法。
  　另外，加值可以有多个。
  　威力表和骰点一样，可以对其他玩家隐藏。
  　例）K20　　　K10+5　　　k30　　　k10+10　　　Sk10-1　　　k10+5+2

  ・暴击值的设定
  　暴击值通过“[暴击值]”指定。
  　没有指定暴击值时默认为10。
  　如果不需要发生暴击，请设定为13。（例如防御时）
  　另外也可写成结尾的“@暴击值”的形式。
  　例）K20[10]　　　K10+5[9]　　　k30[10]　　　k10[9]+10　　　k10-5@9

  ・威力表的减半 (HKx, KxH+N)
  　在威力表开头或末尾加上“H”，骰威力表的最终结果就会减半。
  　H写在威力表末尾的情况下可以在后面直接跟着修正，会在减半后进行加减运算。
  　这种情况下，多个修正需要用括号括起来（否则会解析失败）
  　没有指定暴击值的情况下，则视为没有暴击值。
  　例）HK20　　K20h　　HK10-5@9　　K10-5@9H　　K20gfH　　K20+8H+2　　K20+8H+(1+1)

  ・骰子出目修正（命运变转或重击光辉用）
  　在指令末尾添加“$修正值”来改变骰子出目。
  　可以使用$+1的格式在骰子出目上+修正，或使用$9的格式把骰子出目替换为固定值。
  　暴击时只有第一次出目应用这个修正。
  　例）K20$+1　　　K10+5$9　　　k10-5@9$+2　　　k10[9]+10$9

  ・骰子出目修正（必杀攻击用）
  　写成“#修正值“的格式来增加骰子出目。
  　暴击时后续骰子也会继续应用修正值。
  　例）K20#1　　　k10-5@9#2

  ・威力上升（斩首刀用） r5
  　r后跟上升值，暴击后威力上升r后所填写的上升值点
  　例）K20r5　K30+24@8R5　K40+24@8$12r5

  ・极限命运在末尾加上 gf
  　例）K20gf　K30+24@8GF　K40+24@8$12r5gf

  ・威力表使用1d+固定值对应 暴击后仍继续 sf4
  　例）k10sf4　k0+5sf4@13　k70+26sf3@9

  ・威力表使用1d+固定值对应 暴击后变回使用2d对应 tf3
  　例）k10tf3　k0+5tf4@13　k70+26tf3@9

  ・超越判定用2d6可写成 2d6@10 的格式加上暴击值。
  　例）2D6@10　2D6@10+11>=30

  ・成长　(Gr)
  　在Gr后面加上数字可以进行多次成长。
  　例）Gr3

  ・防御大失败表　(FT)
  　抽取防御大失败表。

  ・缠绕效果表　(TT)
  　抽取缠绕效果表。

  ・德鲁伊用物理魔法表 (Dru[2~6的结果，7~9的结果，10~12的结果])
  　例）Dru[0,3,6]+10-3

  ・奈落诅咒表　(ABT)
  　抽取奈落诅咒表。
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "H?K",
            "OHK",
            "DBK",
            "Gr",
            r"2D6?@\d+",
            "FT",
            "TT",
            "Dru",
            "Bib",
            "ABT",
            "AABT",
        ]
    }
    crate::impl_prefixes_pattern!();

    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _values: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(
            &ZH_TEXT_2_0.common,
            total,
            crate::Int::from(dice_total),
            cmp_op,
            target,
        )
    }
    fn result_ndx(&self, total: crate::Int, cmp_op: CmpOp, target: Target) -> Option<EvalResult> {
        match target {
            Target::Question => None,
            Target::Number(target) => {
                if cmp_op.apply(&total, &target) {
                    Some(EvalResult::success("成功"))
                } else {
                    Some(EvalResult::failure("失败"))
                }
            }
        }
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&ZH_TEXT, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::generated::SwordWorld2_0::assert_toml_cases(
            "SwordWorld2.5:SimplifiedChinese",
            "SwordWorld2_5_SimplifiedChinese.toml",
            92,
        );
    }
}
