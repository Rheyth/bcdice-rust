use super::SwordWorld::{check_result_2d6, SystemText as SwordWorldText};
use crate::arithmetic::{self, Node, ParenMode};
use crate::command_parser::{Parser, SuffixPosition};
use crate::common_command::lexer::{self, Cursor, Tok};
use crate::dice_table::{RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::format;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

static RATING_TABLE: [&str; 101] = [
    "*,0,0,0,1,2,2,3,3,4,4",
    "*,0,0,0,1,2,3,3,3,4,4",
    "*,0,0,0,1,2,3,4,4,4,4",
    "*,0,0,1,1,2,3,4,4,4,5",
    "*,0,0,1,2,2,3,4,4,5,5",
    "*,0,1,1,2,2,3,4,5,5,5",
    "*,0,1,1,2,3,3,4,5,5,5",
    "*,0,1,1,2,3,4,4,5,5,6",
    "*,0,1,2,2,3,4,4,5,6,6",
    "*,0,1,2,3,3,4,4,5,6,7",
    "*,1,1,2,3,3,4,5,5,6,7",
    "*,1,2,2,3,3,4,5,6,6,7",
    "*,1,2,2,3,4,4,5,6,6,7",
    "*,1,2,3,3,4,4,5,6,7,7",
    "*,1,2,3,4,4,4,5,6,7,8",
    "*,1,2,3,4,4,5,5,6,7,8",
    "*,1,2,3,4,4,5,6,7,7,8",
    "*,1,2,3,4,5,5,6,7,7,8",
    "*,1,2,3,4,5,6,6,7,7,8",
    "*,1,2,3,4,5,6,7,7,8,9",
    "*,1,2,3,4,5,6,7,8,9,10",
    "*,1,2,3,4,6,6,7,8,9,10",
    "*,1,2,3,5,6,6,7,8,9,10",
    "*,2,2,3,5,6,7,7,8,9,10",
    "*,2,3,4,5,6,7,7,8,9,10",
    "*,2,3,4,5,6,7,8,8,9,10",
    "*,2,3,4,5,6,8,8,9,9,10",
    "*,2,3,4,6,6,8,8,9,9,10",
    "*,2,3,4,6,6,8,9,9,10,10",
    "*,2,3,4,6,7,8,9,9,10,10",
    "*,2,4,4,6,7,8,9,10,10,10",
    "*,2,4,5,6,7,8,9,10,10,11",
    "*,3,4,5,6,7,8,10,10,10,11",
    "*,3,4,5,6,8,8,10,10,10,11",
    "*,3,4,5,6,8,9,10,10,11,11",
    "*,3,4,5,7,8,9,10,10,11,12",
    "*,3,5,5,7,8,9,10,11,11,12",
    "*,3,5,6,7,8,9,10,11,12,12",
    "*,3,5,6,7,8,10,10,11,12,13",
    "*,4,5,6,7,8,10,11,11,12,13",
    "*,4,5,6,7,9,10,11,11,12,13",
    "*,4,6,6,7,9,10,11,12,12,13",
    "*,4,6,7,7,9,10,11,12,13,13",
    "*,4,6,7,8,9,10,11,12,13,14",
    "*,4,6,7,8,10,10,11,12,13,14",
    "*,4,6,7,9,10,10,11,12,13,14",
    "*,4,6,7,9,10,10,12,13,13,14",
    "*,4,6,7,9,10,11,12,13,13,15",
    "*,4,6,7,9,10,12,12,13,13,15",
    "*,4,6,7,10,10,12,12,13,14,15",
    "*,4,6,8,10,10,12,12,13,15,15",
    "*,5,7,8,10,10,12,12,13,15,15",
    "*,5,7,8,10,11,12,12,13,15,15",
    "*,5,7,9,10,11,12,12,14,15,15",
    "*,5,7,9,10,11,12,13,14,15,16",
    "*,5,7,10,10,11,12,13,14,16,16",
    "*,5,8,10,10,11,12,13,15,16,16",
    "*,5,8,10,11,11,12,13,15,16,17",
    "*,5,8,10,11,12,12,13,15,16,17",
    "*,5,9,10,11,12,12,14,15,16,17",
    "*,5,9,10,11,12,13,14,15,16,18",
    "*,5,9,10,11,12,13,14,16,17,18",
    "*,5,9,10,11,13,13,14,16,17,18",
    "*,5,9,10,11,13,13,15,17,17,18",
    "*,5,9,10,11,13,14,15,17,17,18",
    "*,5,9,10,12,13,14,15,17,18,18",
    "*,5,9,10,12,13,15,15,17,18,19",
    "*,5,9,10,12,13,15,16,17,19,19",
    "*,5,9,10,12,14,15,16,17,19,19",
    "*,5,9,10,12,14,16,16,17,19,19",
    "*,5,9,10,12,14,16,17,18,19,19",
    "*,5,9,10,13,14,16,17,18,19,20",
    "*,5,9,10,13,15,16,17,18,19,20",
    "*,5,9,10,13,15,16,17,19,20,21",
    "*,6,9,10,13,15,16,18,19,20,21",
    "*,6,9,10,13,16,16,18,19,20,21",
    "*,6,9,10,13,16,17,18,19,20,21",
    "*,6,9,10,13,16,17,18,20,21,22",
    "*,6,9,10,13,16,17,19,20,22,23",
    "*,6,9,10,13,16,18,19,20,22,23",
    "*,6,9,10,13,16,18,20,21,22,23",
    "*,6,9,10,13,17,18,20,21,22,23",
    "*,6,9,10,14,17,18,20,21,22,24",
    "*,6,9,11,14,17,18,20,21,23,24",
    "*,6,9,11,14,17,19,20,21,23,24",
    "*,6,9,11,14,17,19,21,22,23,24",
    "*,7,10,11,14,17,19,21,22,23,25",
    "*,7,10,12,14,17,19,21,22,24,25",
    "*,7,10,12,14,18,19,21,22,24,25",
    "*,7,10,12,15,18,19,21,22,24,26",
    "*,7,10,12,15,18,19,21,23,25,26",
    "*,7,11,13,15,18,19,21,23,25,26",
    "*,7,11,13,15,18,20,21,23,25,27",
    "*,8,11,13,15,18,20,22,23,25,27",
    "*,8,11,13,16,18,20,22,23,25,28",
    "*,8,11,14,16,18,20,22,23,26,28",
    "*,8,11,14,16,19,20,22,23,26,28",
    "*,8,12,14,16,19,20,22,24,26,28",
    "*,8,12,15,16,19,20,22,24,27,28",
    "*,8,12,15,17,19,20,22,24,27,29",
    "*,8,12,15,18,19,20,22,24,27,30",
];

pub(crate) struct SystemText {
    pub(crate) common: SwordWorldText,
    pub(crate) transcendent_critical_too_small: &'static str,
    pub(crate) super_success: &'static str,
    pub(crate) keynumber_exceeds: &'static str,
    pub(crate) infinite_critical: &'static str,
    pub(crate) round_suffix: &'static str,
    pub(crate) growth: &'static Table,
    pub(crate) fumble: &'static Table,
    pub(crate) tangle: &'static Table,
}

static JA_GROWTH_ITEMS: &[&str] = &["器用度", "敏捷度", "筋力", "生命力", "知力", "精神力"];
static JA_FUMBLE_ITEMS: &[&str] = &[
    "この表を2回振り、その両方を適用する。（同じ出目による影響は累積しない）。この自動失敗により得られる経験点は、+50点される",
    "ダメージに、攻撃者を強化している「剣のかけら」の数が追加される",
    "ダメージに、攻撃者の「レベル」が追加される",
    "ダメージ決定を2回行い、より高い方を採用する",
    "合算ダメージを2倍する",
    "防護点無効",
];
static JA_TANGLE_ITEMS: &[&str] = &[
    "頭や顔：牙や噛みつきなどにおける命中力判定及び、魔法の行使やブレスに-2のペナルティ修正を受ける",
    "武器や盾：武器の使用不可、又は盾の回避力修正及び防護点を無効化する",
    "腕や手：武器や爪などにおける命中力判定に-2のペナルティ修正、盾を持つ腕方の腕ならその盾の回避力修正及び防護点を無効化する",
    "脚や足：移動不可、更に回避力判定に-2のペナルティ修正を受ける ※両足に絡んでも累積しない",
    "胴体：生命・精神抵抗力を基準値に用いる判定を除き、あらゆる行為判定に-1のペナルティ修正を受ける",
    "特殊：尻尾や翼などに命中。絡められた部位を使用する判定において-2のペナルティ修正、またはそこが使えていたことによるボーナス修正を失う ※存在しない場合は決め直し",
];

static ZH_GROWTH_ITEMS: &[&str] = &["灵巧度", "敏捷度", "力量", "生命力", "智力", "精神力"];
static ZH_FUMBLE_ITEMS: &[&str] = &[
    "骰本表2次，适用两边的效果。（同出目带来的影响不会累积）。本次自动失败获得的经验点+50点",
    "伤害追加强化攻击者的「剑之碎片」个数点",
    "伤害追加攻击者的「等级」点",
    "进行2次伤害决定，采用更高的结果",
    "合算伤害变为2倍",
    "防护点无效",
];
static ZH_TANGLE_ITEMS: &[&str] = &[
    "头或脸：用牙和咬的攻击命中力判定，以及魔法行使和祝福术受到-2的惩罚修正。",
    "武器或盾：武器不能使用，且盾的回避力修正和防护点无效",
    "胳膊或手：武器和爪之类的命中力判定受到-2的惩罚修正，如果持盾的手被缠绕则该盾的回避力修正以及防护点无效",
    "脚：无法移动，且回避力判定受到-2的惩罚修正 ※两脚被缠绕时效果不累积",
    "躯干：除使用生命、精神抵抗力为基础值的判定外，所有行为判定受到-1的惩罚修正",
    "特殊：命中尾巴或翅膀等部位。被缠上的部位使用的判定，受到-2的惩罚修正。且失去该部位提供的加值。 ※不存在这样的部位时重新决定",
];

static JA_GROWTH_TABLE: Table = Table::from_dice("成長表", 1, 6, JA_GROWTH_ITEMS);
static JA_FUMBLE_TABLE: Table = Table::from_dice("防御ファンブル表", 1, 6, JA_FUMBLE_ITEMS);
static JA_TANGLE_TABLE: Table = Table::from_dice("絡み効果表", 1, 6, JA_TANGLE_ITEMS);
static ZH_GROWTH_TABLE: Table = Table::from_dice("成长表", 1, 6, ZH_GROWTH_ITEMS);
static ZH_FUMBLE_TABLE: Table = Table::from_dice("防御大失败表", 1, 6, ZH_FUMBLE_ITEMS);
static ZH_TANGLE_TABLE: Table = Table::from_dice("缠绕效果表", 1, 6, ZH_TANGLE_ITEMS);

pub(crate) static JA_TEXT: SystemText = SystemText {
    common: SwordWorldText {
        success: "成功",
        failure: "失敗",
        critical: "自動的成功",
        fumble: "自動的失敗",
        keynumber_exceeds: "キーナンバーは100までです",
        infinite_critical: "C値を3以上にしてください",
        round_suffix: "回転",
    },
    transcendent_critical_too_small:
        "(%{expression}) ＞ クリティカル値が小さすぎます。3以上を指定してください。",
    super_success: "超成功",
    keynumber_exceeds: "キーナンバーは%{keyMax}までです",
    infinite_critical: "C値を%{min_critical}以上にしてください",
    round_suffix: "回転",
    growth: &JA_GROWTH_TABLE,
    fumble: &JA_FUMBLE_TABLE,
    tangle: &JA_TANGLE_TABLE,
};

pub(crate) static ZH_TEXT: SystemText = SystemText {
    common: SwordWorldText {
        success: "成功",
        failure: "失败",
        critical: "自动成功",
        fumble: "自动失败",
        keynumber_exceeds: "威力最大为%{keyMax}",
        infinite_critical: "请输入%{min_critical}以上的C值",
        round_suffix: "暴击",
    },
    transcendent_critical_too_small: "(%{expression}) ＞ 暴击值过小。请输入3以上的暴击值。",
    super_success: "超成功",
    keynumber_exceeds: "威力最大为%{keyMax}",
    infinite_critical: "请输入%{min_critical}以上的C值",
    round_suffix: "暴击",
    growth: &ZH_GROWTH_TABLE,
    fumble: &ZH_FUMBLE_TABLE,
    tangle: &ZH_TANGLE_TABLE,
};

#[derive(Debug, Clone, Copy, Default)]
enum FirstAdjust {
    #[default]
    None,
    To(i64),
    Modify(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Non2dRoll {
    None,
    GreatestFortune,
    SemiFixed(i64),
    TmpFixed(i64),
}

#[derive(Debug, Clone, Copy)]
struct RatingCommand {
    rate: i64,
    critical: i64,
    modifier: i64,
    first_adjust: FirstAdjust,
    rateup: i64,
    non_2d_roll: Non2dRoll,
    half: bool,
    modifier_after_half: i64,
}

impl RatingCommand {
    fn min_critical(self) -> i64 {
        match self.non_2d_roll {
            Non2dRoll::SemiFixed(value) if value > 1 => (value + 2).clamp(3, 13),
            _ => 3,
        }
    }

    fn label(self) -> String {
        let mut output = format!("KeyNo.{}", self.rate);
        if self.critical < 13 {
            output.push_str(&format!("c[{}]", self.critical));
        }
        match self.first_adjust {
            FirstAdjust::Modify(value) if value != 0 => {
                output.push_str(&format!("m[{}]", format::modifier(&value.into())));
            }
            FirstAdjust::To(value) if value != 0 => output.push_str(&format!("m[{value}]")),
            _ => {}
        }
        if self.rateup != 0 {
            output.push_str(&format!("r[{}]", self.rateup));
        }
        match self.non_2d_roll {
            Non2dRoll::GreatestFortune => output.push_str("gf"),
            Non2dRoll::SemiFixed(value) => output.push_str(&format!("sf[{value}]")),
            Non2dRoll::TmpFixed(value) => output.push_str(&format!("tf[{value}]")),
            Non2dRoll::None => {}
        }
        output.push_str(&format::modifier(&self.modifier.into()));
        output
    }
}

/// 次のトークンが `NUMBER` ならそれを取り出す。
pub(crate) fn take_number(cursor: &mut Cursor<'_>) -> Option<i64> {
    match cursor.peek() {
        Some(Tok::Number(value)) => {
            let value = crate::randomizer::sat_i64(value);
            cursor.advance();
            Some(value)
        }
        _ => None,
    }
}

/// Ruby `rating_parser.y` の `modifier` 規則（`PLUS mul | MINUS mul | modifier PLUS mul | modifier MINUS mul`）。
pub(crate) fn parse_rating_modifier(cursor: &mut Cursor<'_>) -> Option<Node> {
    let mut node = if cursor.accept(&Tok::Plus) {
        arithmetic::parse_mul(cursor, ParenMode::Drop)?
    } else if cursor.accept(&Tok::Minus) {
        Node::Negative(Box::new(arithmetic::parse_mul(cursor, ParenMode::Drop)?))
    } else {
        return None;
    };

    loop {
        let op = if cursor.accept(&Tok::Plus) {
            arithmetic::ArithOp::Add
        } else if cursor.accept(&Tok::Minus) {
            arithmetic::ArithOp::Sub
        } else {
            return Some(node);
        };
        let rhs = arithmetic::parse_mul(cursor, ParenMode::Drop)?;
        node = Node::BinaryOp {
            lhs: Box::new(node),
            op,
            rhs: Box::new(rhs),
        };
    }
}

fn parse_rating_options(
    cursor: &mut Cursor<'_>,
    critical: &mut Option<Node>,
    first_adjust: &mut FirstAdjust,
    half_after: &mut Option<Node>,
    rateup: &mut Option<Node>,
    non_2d_roll: &mut Non2dRoll,
) -> Option<bool> {
    let mut found = false;
    loop {
        if cursor.accept(&Tok::BracketL) {
            if critical.is_some() {
                return None;
            }
            *critical = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            if !cursor.accept(&Tok::BracketR) {
                return None;
            }
            found = true;
            continue;
        }
        if cursor.accept(&Tok::At) {
            if critical.is_some() {
                return None;
            }
            *critical = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            found = true;
            continue;
        }
        if cursor.accept_sym("$") {
            if !matches!(*first_adjust, FirstAdjust::None) {
                return None;
            }
            *first_adjust = if cursor.accept(&Tok::Plus) {
                FirstAdjust::Modify(take_number(cursor)?)
            } else if cursor.accept(&Tok::Minus) {
                FirstAdjust::Modify(-take_number(cursor)?)
            } else {
                FirstAdjust::To(take_number(cursor)?)
            };
            found = true;
            continue;
        }
        if cursor.accept_sym("H") {
            if half_after.is_some() {
                return None;
            }
            *half_after = Some(
                if matches!(cursor.peek(), Some(Tok::Plus) | Some(Tok::Minus))
                    || cursor.peek_starts_term()
                {
                    arithmetic::parse_unary(cursor, ParenMode::Drop)?
                } else {
                    Node::Number(0.into())
                },
            );
            found = true;
            continue;
        }
        if cursor.accept_sym("R") {
            if rateup.is_some() {
                return None;
            }
            *rateup = Some(arithmetic::parse_unary(cursor, ParenMode::Drop)?);
            found = true;
            continue;
        }
        if cursor.accept_sym("G") {
            if !cursor.accept_sym("F") || !matches!(*non_2d_roll, Non2dRoll::None) {
                return None;
            }
            *non_2d_roll = Non2dRoll::GreatestFortune;
            found = true;
            continue;
        }
        if cursor.accept_sym("S") {
            if !cursor.accept_sym("F") || !matches!(*non_2d_roll, Non2dRoll::None) {
                return None;
            }
            *non_2d_roll = Non2dRoll::SemiFixed(take_number(cursor)?.clamp(1, 6));
            found = true;
            continue;
        }
        if cursor.accept_sym("T") {
            if !cursor.accept_sym("F") || !matches!(*non_2d_roll, Non2dRoll::None) {
                return None;
            }
            *non_2d_roll = Non2dRoll::TmpFixed(take_number(cursor)?.clamp(1, 6));
            found = true;
            continue;
        }
        return Some(found);
    }
}

fn parse_rating(source: &str) -> Option<RatingCommand> {
    let lexed = lexer::lex(source);
    let mut cursor = Cursor::new(&lexed.tokens);
    let prefix_half = cursor.accept_sym("H");
    if !cursor.accept_sym("K") {
        return None;
    }

    let rate = take_number(&mut cursor)?;
    let mut critical = None;
    let mut first_adjust = FirstAdjust::None;
    let mut half_after = None;
    let mut rateup = None;
    let mut non_2d_roll = Non2dRoll::None;

    let had_leading_options = parse_rating_options(
        &mut cursor,
        &mut critical,
        &mut first_adjust,
        &mut half_after,
        &mut rateup,
        &mut non_2d_roll,
    )?;

    let modifier = if matches!(cursor.peek(), Some(Tok::Plus) | Some(Tok::Minus)) {
        Some(parse_rating_modifier(&mut cursor)?)
    } else {
        None
    };

    if modifier.is_some() && !had_leading_options {
        parse_rating_options(
            &mut cursor,
            &mut critical,
            &mut first_adjust,
            &mut half_after,
            &mut rateup,
            &mut non_2d_roll,
        )?;
    }
    if !cursor.at_eof() {
        return None;
    }

    let modifier = match modifier {
        Some(node) => crate::randomizer::sat_i64(&node.eval(RoundType::Ceil).ok()?),
        None => 0,
    };
    let critical = match critical {
        Some(node) => crate::randomizer::sat_i64(&node.eval(RoundType::Ceil).ok()?).clamp(0, 13),
        None => {
            if prefix_half || half_after.is_some() {
                13
            } else {
                10
            }
        }
    };
    let rateup = match rateup {
        Some(node) => crate::randomizer::sat_i64(&node.eval(RoundType::Ceil).ok()?),
        None => 0,
    };
    let half = prefix_half || half_after.is_some();
    let modifier_after_half = match half_after {
        Some(node) => crate::randomizer::sat_i64(&node.eval(RoundType::Ceil).ok()?),
        None => 0,
    };

    Some(RatingCommand {
        rate,
        critical,
        modifier,
        first_adjust,
        rateup,
        non_2d_roll,
        half,
        modifier_after_half,
    })
}

/// Ruby `SwordWorld2_0#rollDice`。`SwordWorld2_5` も同じ振り方を使う。
pub(crate) fn roll_dice(
    non_2d_roll: Non2dRoll,
    round: i64,
    rng: &mut Randomizer,
) -> Result<(i64, String), EvalError> {
    match non_2d_roll {
        Non2dRoll::SemiFixed(fixed) => {
            let dice = rng.roll_once(6)?;
            Ok((dice + fixed, format!("{dice},{fixed}")))
        }
        Non2dRoll::TmpFixed(fixed) if round == 0 => {
            let dice = rng.roll_once(6)?;
            Ok((dice + fixed, format!("{dice},{fixed}")))
        }
        Non2dRoll::GreatestFortune => {
            let dice = rng.roll_once(6)?;
            Ok((dice * 2, format!("{dice},{dice}")))
        }
        Non2dRoll::None | Non2dRoll::TmpFixed(_) => {
            let values = rng.roll_barabara(2, 6)?;
            let total = values.iter().sum::<i64>();
            let text = values
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            Ok((total, text))
        }
    }
}

/// レーティング表の値。`key` はキーナンバー（0〜100）、`dice` は出目（2以下は0）。
pub(crate) fn rating_value(key: i64, dice: i64) -> i64 {
    if dice <= 2 {
        return 0;
    }
    RATING_TABLE[key as usize]
        .split(',')
        .nth((dice - 2) as usize)
        .expect("complete SwordWorld rating row")
        .parse()
        .expect("numeric SwordWorld rating value")
}

fn rating(
    text: &SystemText,
    source: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let Some(command) = parse_rating(source) else {
        return Ok(None);
    };
    if command.rate > 100 {
        let message = text.keynumber_exceeds.replace("%{keyMax}", "100");
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            message,
        ))));
    }

    let min_critical = command.min_critical();
    if command.critical < min_critical {
        let message = text
            .infinite_critical
            .replace("%{min_critical}", &min_critical.to_string());
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            message,
        ))));
    }

    let mut dice_texts = Vec::new();
    let mut dice_totals = Vec::new();
    let mut rate_results = Vec::new();
    let mut rating_total = 0_i64;
    let mut dice_only_total = 0_i64;
    let mut round = 0_i64;
    let mut first_adjust = command.first_adjust;

    loop {
        let (raw_value, dice_text) = roll_dice(command.non_2d_roll, round, rng)?;
        let mut raw = raw_value;
        let mut dice = raw_value;

        match first_adjust {
            FirstAdjust::To(value) if round == 0 && value != 0 => {
                raw = value;
                dice = value;
            }
            FirstAdjust::Modify(value) if round == 0 => {
                dice = dice.saturating_add(value);
            }
            _ => {}
        }
        first_adjust = FirstAdjust::None;

        dice_texts.push(dice_text);
        if raw <= 2 {
            dice_totals.push(raw.to_string());
            rate_results.push("**".to_string());
            round += 1;
            break;
        }

        dice = dice.clamp(2, 12);
        let current_key = (command.rate + round * command.rateup).clamp(0, 100);
        let rate_value = rating_value(current_key, dice);
        rating_total = rating_total.saturating_add(rate_value);
        dice_only_total = dice_only_total.saturating_add(dice);
        dice_totals.push(dice.to_string());
        rate_results.push(if dice > 2 {
            rate_value.to_string()
        } else {
            "**".to_string()
        });
        round += 1;

        if dice < command.critical {
            break;
        }
    }

    let mut sequence = vec![format!(
        "2D:[{}]={}",
        dice_texts.join(" "),
        dice_totals.join(",")
    )];
    let mut result = EvalResult::new();

    if dice_only_total <= 2 {
        sequence.push(rate_results.join(","));
        sequence.push(text.common.fumble.to_string());
        result.fumble = true;
    } else {
        if rate_results.len() > 1 || command.modifier != 0 {
            let mut calculation = format!(
                "{}{}",
                rate_results.join(","),
                format::modifier(&command.modifier.into())
            );
            if command.half {
                calculation = format!("({calculation})/2");
                calculation.push_str(&format::modifier(&command.modifier_after_half.into()));
            }
            sequence.push(calculation);
        } else if command.half {
            sequence.push(format!(
                "{}/2{}",
                rate_results[0],
                format::modifier(&command.modifier_after_half.into())
            ));
        }

        if round > 1 {
            sequence.push(format!("{}{}", round - 1, text.round_suffix));
        }

        let mut total = rating_total.saturating_add(command.modifier);
        if command.half {
            total = (total + 1)
                .div_euclid(2)
                .saturating_add(command.modifier_after_half);
        }
        sequence.push(total.to_string());
        result.critical = round > 1;
    }

    result.text = format!("{} ＞ {}", command.label(), sequence.join(" ＞ "));
    Ok(Some(SpecificCommandOutput::result(result)))
}

fn growth(text: &SystemText, count: i64, rng: &mut Randomizer) -> Result<String, EvalError> {
    let mut outputs = Vec::new();
    for _ in 0..count {
        let first = text.growth.roll(rng)?;
        let second = text.growth.roll(rng)?;
        let body = if first.value() != second.value() {
            format!(
                "[{},{}]->({} or {})",
                first.value(),
                second.value(),
                first.last_body(),
                second.last_body()
            )
        } else {
            format!(
                "[{},{}]->({})",
                first.value(),
                second.value(),
                first.last_body()
            )
        };
        outputs.push(body);
    }
    Ok(outputs.join(" | "))
}

fn growth_count(command: &str) -> Option<i64> {
    let suffix = command.strip_prefix("GR")?;
    if suffix.is_empty() {
        Some(1)
    } else if suffix.chars().all(|c| c.is_ascii_digit()) {
        suffix.parse().ok()
    } else {
        None
    }
}

fn transcendent_expression(mut expression: String) -> String {
    if expression.starts_with("2D@") {
        expression.insert(2, '6');
    }
    expression
}

fn transcendent_test(
    text: &SystemText,
    source: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let parser = Parser::new(&["2D6?"], RoundType::Ceil)
        .enable_critical()
        .restrict_cmp_op_to(&[None, Some(CmpOp::Ge), Some(CmpOp::Gt)]);
    let Some(parsed) = parser.parse(source) else {
        return Ok(None);
    };
    let Some(critical_node) = &parsed.critical else {
        return Ok(None);
    };
    let critical = crate::randomizer::sat_i64(critical_node);
    let expression = transcendent_expression(parsed.to_s(SuffixPosition::AfterCommand));

    if critical < 3 {
        let message = text
            .transcendent_critical_too_small
            .replace("%{expression}", &expression);
        return Ok(Some(SpecificCommandOutput::result(EvalResult::with_text(
            message,
        ))));
    }

    let first = rng.roll_barabara(2, 6)?;
    let fumble = first.len() == 2 && first[0] == 1 && first[1] == 1;
    let critical_hit = first.len() == 2 && first[0] == 6 && first[1] == 6;
    let mut groups = vec![first];

    while groups
        .last()
        .map(|values| values.iter().sum::<i64>() >= critical)
        .unwrap_or(false)
    {
        groups.push(rng.roll_barabara(2, 6)?);
    }

    let sum = groups.iter().flatten().copied().sum::<i64>();
    let total: crate::Int = crate::Int::from(sum) + &parsed.modify_number;
    let modifier = format::modifier(&parsed.modify_number);
    let mut result = EvalResult::new();

    if let (Some(cmp_op), Some(target)) = (parsed.cmp_op, &parsed.target_number) {
        if fumble {
            result = EvalResult::fumble(text.common.fumble);
        } else if critical_hit {
            result = EvalResult::critical(text.common.critical);
        } else if cmp_op.apply(&total, target) {
            result = if groups.len() >= 2 && total >= 41.into() {
                EvalResult::success(text.super_success)
            } else {
                EvalResult::success(text.common.success)
            };
        } else {
            result = EvalResult::failure(text.common.failure);
        }
    }

    let group_text = groups
        .iter()
        .map(|values| {
            format!(
                "[{}]",
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let mut parts = vec![
        format!("({expression})"),
        format!("{sum}{group_text}{modifier}"),
        total.to_string(),
    ];
    if !result.text.is_empty() {
        parts.push(result.text.clone());
    }
    result.text = parts.join(" ＞ ");
    Ok(Some(SpecificCommandOutput::result(result)))
}

/// レーティング表以外の固有コマンド（成長 / 超越判定 / 防御ファンブル表 / 絡み効果表）。
///
/// 該当するコマンドでなければ `None` を返し、呼び出し側がレーティング表へ進む
/// （Ruby `SwordWorld2_0#eval_game_system_specific_command` の `else super` に対応）。
/// `SwordWorld2_5` はこれを呼んでから v2.5 版のレーティング表を評価する。
pub(crate) fn eval_non_rating_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Option<Result<Option<SpecificCommandOutput>, EvalError>> {
    if let Some(count) = growth_count(command) {
        return Some(growth(text, count, rng).map(|body| Some(SpecificCommandOutput::text(body))));
    }
    if command.starts_with("2D@") || command.starts_with("2D6@") {
        return Some(transcendent_test(text, command, rng));
    }
    let table = match command {
        "FT" => text.fumble,
        "TT" => text.tangle,
        _ => return None,
    };
    Some(
        table
            .roll(rng)
            .map(|result| Some(SpecificCommandOutput::text(result.to_string()))),
    )
}

pub(crate) fn eval_specific_command(
    text: &SystemText,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(result) = eval_non_rating_command(text, command, rng) {
        return result;
    }
    rating(text, command, rng)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwordWorld2_0;

impl GameSystem for SwordWorld2_0 {
    fn id(&self) -> &'static str {
        "SwordWorld2.0"
    }
    fn name(&self) -> &'static str {
        "ソード・ワールド2.0"
    }
    fn sort_key(&self) -> &'static str {
        "そおとわあると2.0"
    }
    fn help_message(&self) -> &'static str {
        r#"自動的成功、成功、失敗、自動的失敗の自動判定を行います。

・レーティング表　(Kx)
　"Kキーナンバー+ボーナス"の形で記入します。
　ボーナスの部分に「K20+K30」のようにレーティングを取ることは出来ません。
　また、ボーナスは複数取ることが出来ます。
　レーティング表もダイスロールと同様に、他のプレイヤーに隠れてロールすることも可能です。
　例）K20　　　K10+5　　　k30　　　k10+10　　　Sk10-1　　　k10+5+2

・クリティカル値の設定
　クリティカル値は"[クリティカル値]"で指定します。
　指定しない場合はクリティカル値10とします。
　クリティカル処理が必要ないときは13などとしてください。(防御時などの対応)
　またタイプの軽減化のために末尾に「@クリティカル値」でも処理するようにしました。
　例）K20[10]　　　K10+5[9]　　　k30[10]　　　k10[9]+10　　　k10-5@9

・レーティング表の半減 (HKx, KxH+N)
　レーティング表の先頭または末尾に"H"をつけると、レーティング表を振って最終結果を半減させます。
　末尾につけた場合、直後に修正ををつけることで、半減後の加減算を行うことができます。
　この際、複数の項による修正にはカッコで囲うことが必要です（カッコがないとパースに失敗します）
　クリティカル値を指定しない場合、クリティカルなしと扱われます。
　例）HK20　　K20h　　HK10-5@9　　K10-5@9H　　K20gfH　　K20+8H+2　　K20+8H(1+1)

・ダイス目の修正（運命変転やクリティカルレイ用）
　末尾に「$修正値」でダイス目に修正がかかります。
　$＋１と修正表記ならダイス目に＋修正、＄９のように固定値ならダイス目をその出目に差し替え。
　クリティカルした場合でも固定値や修正値の適用は最初の一回だけです。
　例）K20$+1　　　K10+5$9　　　k10-5@9$+2　　　k10[9]+10$9

・首切り刀用レーティング上昇 r10
　例）K20r10　K30+24@8R10　K40+24@8$12r10

・グレイテストフォーチュンは末尾に gf
　例）K20gf　K30+24@8GF　K40+24@8$12r10gf

・威力表を1d+sfで参照 クリティカル後も継続 sf4
　例）k10sf4　k0+5SF4@13　k70+26sf3@9

・威力表を1d+tfで参照 クリティカル後は2dで参照 tf3
　例）k10tf3　k0+5TF4@13　k70+26tf3@9

・超越判定用に2d6ロールに 2D6@10 書式でクリティカル値付与が可能に。
　例）2D6@10　2D6@10+11>=30

・成長　(Gr)
　末尾に数字を付加することで、複数回の成長をまとめて行えます。
　例）Gr3

・防御ファンブル表　(FT)
　防御ファンブル表を出すことができます。

・絡み効果表　(TT)
　絡み効果表を出すことができます。
"#
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &["H?K", "Gr", r"2D6?@\d+", "FT", "TT"]
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
            &JA_TEXT.common,
            total,
            crate::Int::from(dice_total),
            cmp_op,
            target,
        )
    }
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_TEXT, command, rng)
    }
}

#[cfg(test)]
mod tests {
    /// `test/data/SwordWorld2_0.toml` の全ケースが通ること（共通ハーネス）。
    ///
    /// ケース 60（無効コマンド `green` の暴発確認fixture）は出目が消費されない
    /// 既知のTOML不整合。
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases(
            "SwordWorld2.0",
            "SwordWorld2_0.toml",
            75,
            &[(60, 2)],
        );
    }
}
