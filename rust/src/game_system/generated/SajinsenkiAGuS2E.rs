//! P4手書き移植: `SajinsenkiAGuS2E.rb`。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::command_parser::Parser;
use crate::dice_table::range_table::RangeTableItem;
use crate::dice_table::{RangeInc, RangeTable, RollableTable, Table};
use crate::enums::RoundType;
use crate::eval::EvalError;
use crate::game_system::int_helpers::IntHelperOps;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SajinsenkiAGuS2E;

impl GameSystem for SajinsenkiAGuS2E {
    fn id(&self) -> &'static str {
        "SajinsenkiAGuS2E"
    }
    fn name(&self) -> &'static str {
        "砂塵戦機アーガス2ndEdition"
    }
    fn sort_key(&self) -> &'static str {
        "さしんせんきああかす2"
    }
    fn help_message(&self) -> &'static str {
        HELP_MESSAGE
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"-?\d*AG",
            "OM",
            "NM",
            "CR",
            r"-?\d*AGW",
            "OMW",
            "NMW",
            "CAP",
            "INT",
            "SAL",
            "DEF",
            "SPE",
        ]
    }
    crate::impl_prefixes_pattern!();
    fn enabled_d9(&self) -> bool {
        true
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
    }
}

const HELP_MESSAGE: &str = r"・一般判定Lv（チャンス出目0→判定0） nAG+x
　　　nは習得レベル、Lv0の場合nの省略可能。xは判定値修正（数式による修正可）、省略した場合はレベル修正0
　　　例）AG:習得レベル0の一般技能、1AG+1:習得レベル1・判定値修正+1の技能、AG+2-1：習得レベル0・判定値修正2-1の技能、(1-1)AG：習得レベル1・レベル修正-1の技能

・適正距離での命中判定（チャンス出目0→判定0、HR算出）OM+y@z
　　　yは命中補正値（数式可）、zはクリティカル値。クリティカル値省略時は0
　　　HRの算出時には、HRが大きくなる場合に出目0を10に読み替えます。
　　　例）OM+18-6@2:命中補正値+18-6でクリティカル値2、適正距離の判定

・非適正距離での命中判定（チャンス出目0→判定0、HR算出）NM+y@z
　　　yは命中補正値（数式可）、zはクリティカル値。クリティカル値省略時は0
　　　HRの算出時には、HRが大きくなる場合に出目0を10に読み替えます。
　　　例）NM+4-3:命中補正値+4-3で非適正距離の判定


・『西風旅徨』で導入されたファンブル・ルールを用いた判定
　判定時にダイスがすべて8以上ならファンブル(自動失敗)です。
　それぞれのコマンドにWを付けると『西風旅徨』モードになります。
　　　・一般判定                nAGW+x
　　　・適正距離での命中判定    OMW+y@z
　　　・非適正距離での命中判定  NMW+y@z



・クリティカル表　　 CR
・鹵獲結果表　　　　 CAP
・幕間クエスト表　　 INT
・サルベージ表　　　 SAL
・赤字ペナルティー表 DEF
・特殊戦況表　　　　 SPE

※通常の1D10などの10面ダイスにおいて出目10の読み替えはしません。コマンドのみです。
　ページ参照は、何もない場合は「ルールブック」、wは「西風旅徨」を示します。

";

struct CommandRoll {
    result: EvalResult,
    dice: Vec<i64>,
}

fn ag_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(-?\d+)?AG((?:[-+]\d+)*)$").expect("valid regex"))
}

fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let west = command
        .replace("AGW", "AG")
        .replace("OMW", "OM")
        .replace("NMW", "NM");
    let is_west = west != command;

    if let Some(mut roll) = roll_ippan(&west, rng)? {
        if is_west {
            change_fumble(&mut roll);
        }
        return Ok(Some(SpecificCommandOutput::result(roll.result)));
    }
    if let Some(mut roll) = roll_hit_check(&west, rng)? {
        if is_west {
            change_fumble(&mut roll);
        }
        return Ok(Some(SpecificCommandOutput::result(roll.result)));
    }

    let output = match command {
        "CR" => Some(CR.roll(rng)?.to_string()),
        "CAP" => Some(CAP.roll(rng)?.to_string()),
        "INT" => Some(INT.roll(rng)?.to_string()),
        "SAL" => Some(SAL.roll(rng)?.to_string()),
        "DEF" => Some(DEF.roll(rng)?.to_string()),
        "SPE" => Some(SPE.roll(rng)?.to_string()),
        _ => None,
    };
    Ok(output.map(SpecificCommandOutput::text))
}

fn roll_ippan(command: &str, rng: &mut Randomizer) -> Result<Option<CommandRoll>, EvalError> {
    let Some(caps) = ag_pattern().captures(command) else {
        return Ok(None);
    };
    let level = caps.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let x = arithmetic::eval(caps.get(2).map_or("", |m| m.as_str()), RoundType::Floor)?
        .unwrap_or(I::ZERO);
    let target = if level <= 0 { 7 + x } else { 10 + level + x };
    let dice = rng
        .roll_barabara(2, 10)?
        .into_iter()
        .map(|d| if d == 10 { 0 } else { d })
        .collect::<Vec<_>>();
    let total: i64 = dice.iter().sum();
    let success_level = 1 + dice.iter().filter(|&&d| d <= level).count();
    let success = total <= crate::randomizer::sat_i64(&target);
    let mut sequence = vec![
        format!("(2D10<={target})"),
        format!("{total}[{},{}]", dice[0], dice[1]),
    ];
    if dice.contains(&0) {
        sequence.push("チャンス".to_owned());
    }
    sequence.push(if success {
        format!("成功(+{success_level})")
    } else {
        "失敗".to_owned()
    });
    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.set_condition(success);
    Ok(Some(CommandRoll { result, dice }))
}

fn roll_hit_check(command: &str, rng: &mut Randomizer) -> Result<Option<CommandRoll>, EvalError> {
    let Some(parsed) = Parser::new(&["OM", "NM"], RoundType::Floor)
        .enable_critical()
        .parse(command)
    else {
        return Ok(None);
    };
    let count = if parsed.command == "OM" { 2 } else { 3 };
    let mut dice = rng
        .roll_barabara(count, 10)?
        .into_iter()
        .map(|d| if d == 10 { 0 } else { d })
        .collect::<Vec<_>>();
    dice.sort_unstable_by(|a, b| b.cmp(a));
    let chosen = &dice[..2];
    let total: i64 = chosen.iter().sum();
    let target = parsed.modify_number;
    let criticals = chosen
        .iter()
        .filter(|&&d| {
            d <= parsed
                .critical
                .as_ref()
                .map(crate::randomizer::sat_i64)
                .unwrap_or(0)
        })
        .count();
    let success = total <= crate::randomizer::sat_i64(&target);
    let hr = (target.clone() - total).abs_int().max(
        (target.clone() - total - chosen.iter().filter(|&&d| d == 0).count() as i64 * 10).abs_int(),
    );
    let dice_text = if count == 2 {
        format!("{},{}", dice[0], dice[1])
    } else {
        format!("{},{}&{}", dice[0], dice[1], dice[2])
    };
    let mut sequence = vec![
        format!("({count}D10<={target})"),
        format!("{total}[{dice_text}]"),
    ];
    if chosen.contains(&0) {
        sequence.push("チャンス".to_owned());
    }
    sequence.push(if success {
        format!("成功（HR={hr}、クリティカル{criticals}）")
    } else {
        "失敗".to_owned()
    });
    let mut result = EvalResult::with_text(sequence.join(" ＞ "));
    result.set_condition(success);
    result.critical = criticals >= 1;
    Ok(Some(CommandRoll { result, dice }))
}

fn change_fumble(roll: &mut CommandRoll) {
    if roll.dice.iter().filter(|&&d| d >= 8).count() < 2 {
        return;
    }
    if let Some(index) = roll.result.text.rfind(" ＞ ") {
        roll.result.text.truncate(index);
        roll.result.text.push_str(" ＞ ファンブル");
    }
    roll.result.success = false;
    roll.result.failure = true;
    roll.result.fumble = true;
}

static CR_ITEMS: &[&str] = &[
    "1：「小破」ダメージ+［5］。耐久値-［1］",
    "2：「小破」ダメージ+［5］。耐久値-［1］",
    "3：「小破」ダメージ+［5］。耐久値-［1］",
    "4：「小破」ダメージ+［5］。耐久値-［1］",
    "5：「兵装」損壊を受けるごとに［1D10］を振り、出目に応じた部位の兵装とオプションが《脱落》",
    "6：「上体」攻撃系能力［白兵/ 火器/ 索敵］は各［- 損壊Lv］",
    "7：「脚部」行動系・防御系能力［Iv 値（イニシア値）/ 最大MP/ 回避］は各［- 損壊Lv］",
    "8：「搭乗者」搭乗者の〈最大HP〉および〈HP〉は［-（4 ×損壊Lv）］",
    "9：「搭乗者」搭乗者の〈最大HP〉および〈HP〉は［-（4 ×損壊Lv）］",
    "0：「小破」ダメージ+［5］。耐久値-［1］",
];
static CR: Table = Table::from_dice("クリティカル表", 1, 10, CR_ITEMS);
static CAP_ITEMS: &[&str] = &[
    "0:敵A:GuS を完全な状態で鹵獲︕ ※総合価格÷ 2 で売却可。",
    "1:敵A:GuS を完全な状態で鹵獲︕ ※総合価格÷ 2 で売却可。",
    "2:敵A:GuS を完全な状態で鹵獲︕ ※総合価格÷ 2 で売却可。",
    "3:敵A:GuS の兵装を鹵獲︕ ※敵A:GuS の装備している任意の兵装1つを獲得。",
    "4:敵A:GuS の兵装を鹵獲︕ ※敵A:GuS の装備している任意の兵装1つを獲得。",
    "5:敵A:GuS の兵装を鹵獲︕ ※敵A:GuS の装備している任意の兵装1つを獲得。",
    "6:敵A:GuS の兵装を鹵獲︕ ※敵A:GuS の装備している任意の兵装1つを獲得。",
    "7:敵A:GuS の兵装を鹵獲︕ ※敵A:GuS の装備している任意の兵装1つを獲得。",
    "8:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "9:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "10:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "11:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "12:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "13:使えそうな兵装を発見︕ ※1D10 を振り、出目の部位の兵装1つを獲得。",
    "14:残念、完全にスクラップだ……。※部品代として［バランス値×300］cdtを獲得。",
    "15:残念、完全にスクラップだ……。※部品代として［バランス値×300］cdtを獲得。",
    "16:残念、完全にスクラップだ……。※部品代として［バランス値×300］cdtを獲得。",
    "17:残念、完全にスクラップだ……。※部品代として［バランス値×300］cdtを獲得。",
    "18:残念、完全にスクラップだ……。※部品代として［バランス値×300］cdtを獲得。",
];
static CAP: Table = Table::from_dice("鹵獲結果表", 2, 10, CAP_ITEMS);
static INT_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 1), "慰労 PC/クルー1名が、労ってくれる。 最大HP+4"),
    (RangeInc::new(2, 3), "感謝 PC/クルー1名が、感謝の気持ちを伝える。 最大HP+4"),
    (RangeInc::new(4, 5), "安堵 PC/ クルー1名が、安堵の気持ちを伝える。 最大HP+4"),
    (RangeInc::new(6, 7), "治療 戦闘中の怪我や急な病気で医療班のお世話になることに。 最大HP+4"),
    (RangeInc::new(8, 9), "日常 PC/クルー1名と、他愛のない日常の会話をする。 最大HP+4"),
    (RangeInc::new(10, 11), "遊興 PC/クルー1名と遊びに興じ、楽しい時を過ごす。 XP+1"),
    (RangeInc::new(12, 13), "勤労 PC/クルー1名と協力し、船内の仕事を行う。 XP+1"),
    (RangeInc::new(14, 15), "評価 PC/クルー1名が、仕事の出来を評価してくれる。 XP+1"),
    (RangeInc::new(16, 17), "調達 PC/クルー1名とともに生活品の買い出しを行うことに。 XP+1"),
    (RangeInc::new(18, 19), "社交 取引や補給などの仕事を通し、船外での社会経験を得る。 XP+1"),
    (RangeInc::new(20, 21), "注意 PC/クルー1名が、君の危険な戦闘行動について指摘する。 SP+1"),
    (RangeInc::new(22, 23), "反省 PC/クルー1名と、作戦行動の反省会を行う。 SP+1"),
    (RangeInc::new(24, 25), "鍛錬 PC/クルー1名に、模擬戦に付き合ってもらう。 SP+1"),
    (RangeInc::new(26, 27), "感心 PC/クルー1名の仕事や戦闘行動に対し、感銘を受ける。 SP+1"),
    (RangeInc::new(28, 29), "改良 整備班と協力し、A:GuSのプログラムの改良に努める。 SP+1"),
    (RangeInc::new(30, 31), "割引 兵装が割引されているのを発見し、格安で購入できる。 基本兵装1つを半額で購入可。"),
    (RangeInc::new(32, 33), "発見 クルー1名が、兵装を入手した。 基本兵装1つを半額で購入可。"),
    (RangeInc::new(34, 35), "発明 クルー1名が、兵装を開発した。 基本兵装1つを半額で購入可。"),
    (RangeInc::new(36, 37), "大発見 クルー1名が、強力な兵装を入手した！ 上級兵装1つを購入可。（p37参照）"),
    (RangeInc::new(38, 39), "大発明 クルー1名が、新たな兵装を開発した！ 上級兵装1つを購入可。（p37参照）"),
    (RangeInc::new(40, 41), "昔話 PC/クルー1名に、自分の過去について語ってしまう。 最大LP+1"),
    (RangeInc::new(42, 43), "願望 PC/クルー1名に、自分の夢や未来について語ってしまう。 最大LP+1"),
    (RangeInc::new(44, 45), "家族 PC/クルー1名に、自分の家族について語ってしまう。 最大LP+1"),
    (RangeInc::new(46, 47), "望郷 PC/クルー1名に、自分の故郷について語ってしまう。 最大LP+1"),
    (RangeInc::new(48, 49), "知人 PC/クルー1名に、自分の知人を重ね合わせてしまう。 最大LP+1"),
    (RangeInc::new(50, 51), "個人収入 チームとは関係ない個人的な商売や取引で利益を得る。 4,000cdtを獲得。"),
    (RangeInc::new(52, 53), "臨時収入 思いがけない臨時の収入が入る。 4,000cdt を獲得。"),
    (RangeInc::new(54, 55), "取引 クルー1 名と取引を行い、予算を獲得することに成功する。 4,000cdtを獲得。"),
    (RangeInc::new(56, 57), "裏取引 クルー1 名と秘密の取引を行い、見返りとして予算を獲得。 6,000cdtを獲得。"),
    (RangeInc::new(58, 59), "賞与 オーナーが特別に報酬を用意してくれた！ 6,000cdtを獲得。"),
    (RangeInc::new(60, 61), "改造 整備班とともに機体の改造に明け暮れる。 任意の改造Lv+1。（上限：2Lv）"),
    (RangeInc::new(62, 63), "鹵獲 鹵獲品の中から機体の改造に使えるものを発見。 任意の改造Lv+1。（上限：2Lv）"),
    (RangeInc::new(64, 65), "強化 案機体を強化するための画期的なアイディアを思いつく。 任意の改造Lv+1。（上限：2Lv）"),
    (RangeInc::new(66, 67), "懇願 整備班に頼みこみ、機体の改造をしてもらう。 任意の改造Lv+1。（上限：3Lv）"),
    (RangeInc::new(68, 69), "掘出物 掘出物を発見、整備班が早速機体に取り付けてくれた。 任意の改造Lv+1。（上限：3Lv）"),
    (RangeInc::new(70, 71), "募集 クルーの募集を行ったところ、何名か候補が現れた。 クルー1名を割安（20,000cdt）で雇用可。"),
    (RangeInc::new(72, 73), "勧誘 有能な人材を発見した。ぜひ雇い入れたいものだが。 クルー1名を割安（20,000cdt）で雇用可。"),
    (RangeInc::new(74, 75), "推薦 依頼人からの推薦で、クルーを1名紹介される。 クルー1名を割安（20,000cdt）で雇用可。"),
    (RangeInc::new(76, 77), "志願 クルーとして雇って欲しい、という人物が現れる。クルー1名を割安（15,000cdt）で雇用可。"),
    (RangeInc::new(78, 79), "成長 見習いクルーが大分成長してきた。もう1人前と見てもいい。クルー1名を割安（15,000cdt）で雇用可。"),
    (RangeInc::new(80, 81), "交渉 依頼人との交渉がうまくいき、少し報酬を割増ししてもらえる。 チーム予算を8,000cdt獲得。"),
    (RangeInc::new(82, 83), "節約 経費が思ったよりも節約できた。経理やオーナーの機嫌が良い。 チーム予算を8,000cdt獲得。"),
    (RangeInc::new(84, 85), "賞金 今回の敵は賞金がかかっていたようで臨時収入が入った。 チーム予算を8,000cdt獲得。"),
    (RangeInc::new(86, 87), "名声 チームの名声が高まっており、クルーの自尊心が刺激される。 チーム予算を12,000cdt獲得。"),
    (RangeInc::new(88, 89), "一致団結 オーナーからの労いがあり、クルー一同の結束力が高まる。 チーム予算を12,000cdt獲得。"),
    (RangeInc::new(90, 91), "点検 PC/クルー1名とシップに異状がないか点検作業を行う。 拠点AP+10。"),
    (RangeInc::new(92, 93), "補修 整備班とともにくたびれたシップの改装作業を行う。 拠点AP+10。"),
    (RangeInc::new(94, 95), "全面改修 艦内の問題箇所を全面的に改修する。 拠点AP+10。"),
    (RangeInc::new(96, 97), "自由 行動自由気ままに好きなことをして過ごす。 00～49の任意の効果を適用可。（p69参照）"),
    (RangeInc::new(98, 99), "歓迎 街の住民にたいへんな歓迎を受ける。 50～95の任意の効果を適用可。（p69参照）"),
    (RangeInc::new(100, 100), "慰労 PC/クルー1名が、労ってくれる。 最大HP+4"),
];
static INT: RangeTable = RangeTable::from_dice("幕間クエスト表", 1, 100, INT_ITEMS);
static SAL_ITEMS: &[RangeTableItem] = &[
    (RangeInc::new(1, 2), "大失敗…。大変な損失を出してしまった…。 -5,000cdt"),
    (RangeInc::new(3, 5), "失敗…。かなりの損失を出してしまった…。 -3,000cdt"),
    (RangeInc::new(6, 9), "失敗…。損失を出してしまった…。 -1,000cdt"),
    (RangeInc::new(10, 10), "大成功！大きな収益を上げることができた！ +5,000cdt"),
    (RangeInc::new(11, 19), "空振り……何の成果も得られなかった…。 0cdt"),
    (RangeInc::new(20, 20), "掘り出し物を発見！2Lv改造済の【基本携行兵装】一つを獲得！ 一般兵装（→p34）"),
    (RangeInc::new(21, 22), "ジャンク品を発見。船の装甲強化くらいには使えそうだ。 拠点AP+［5］"),
    (RangeInc::new(23, 25), "ジャンク品を発見。少し赤字だがまあやむを得まい。 +1,000cdt"),
    (RangeInc::new(26, 29), "ジャンク品を発見。手間賃くらいにはなった。 +2,000cdt"),
    (RangeInc::new(30, 30), "掘り出し物を発見！2Lv改造済の【基本外装兵装】一つを獲得！ 一般兵装（→p35）"),
    (RangeInc::new(31, 32), "成功！少しだが利益を出すことができた！ +3,000cdt"),
    (RangeInc::new(33, 35), "成功！まずまずの利益を出すことができた！ +5,000cdt"),
    (RangeInc::new(36, 39), "大成功！大きな利益を出すことができた！ +7,000cdt"),
    (RangeInc::new(40, 41), "良質のバッテリーを獲得。 EP+［4］"),
    (RangeInc::new(42, 43), "良質の装甲版を獲得。 AP+［4］"),
    (RangeInc::new(44, 44), "大失敗…。大きな損失を出してしまった…。 -5,000cdt"),
    (RangeInc::new(45, 45), "ブレードを獲得。 →p34"),
    (RangeInc::new(46, 46), "ランスを獲得。 →p34"),
    (RangeInc::new(47, 47), "アンカーブレードを獲得。 →p34"),
    (RangeInc::new(48, 48), "パイルバンカーを獲得。 →p34"),
    (RangeInc::new(49, 49), "ハンドガンを獲得。 →p35"),
    (RangeInc::new(50, 50), "ヘビーハンドガンを獲得。 →p35"),
    (RangeInc::new(51, 51), "ライフルを獲得。 →p35"),
    (RangeInc::new(52, 52), "アンカーショットを獲得。 →p35"),
    (RangeInc::new(53, 53), "マシンガンSを獲得。 →p35"),
    (RangeInc::new(54, 54), "マシンガンLを獲得。 →p35"),
    (RangeInc::new(55, 55), "ミサイルポッドSを獲得。 →p35"),
    (RangeInc::new(56, 56), "ミサイルポッドLを獲得。 →p35"),
    (RangeInc::new(57, 57), "バズーカを獲得。 →p35"),
    (RangeInc::new(58, 58), "カノンを獲得。 →p35"),
    (RangeInc::new(59, 59), "ライトシールドを獲得。 →p35"),
    (RangeInc::new(60, 60), "ミドルシールドを獲得。 →p35"),
    (RangeInc::new(61, 61), "ヘビーシールドを獲得。 →p35"),
    (RangeInc::new(62, 62), "レーダーユニットを獲得。 →p35"),
    (RangeInc::new(63, 63), "ECMユニットを獲得。 →p35"),
    (RangeInc::new(64, 64), "サブブースターを獲得。 →p36"),
    (RangeInc::new(65, 65), "ディフェンスサポートを獲得。 →p36"),
    (RangeInc::new(66, 66), "コンバットサポートを獲得。 →p36"),
    (RangeInc::new(67, 67), "大失敗…。大きな損失を出してしまった…。 -5,000cdt"),
    (RangeInc::new(68, 68), "ショットサポートを獲得。 →p36"),
    (RangeInc::new(69, 69), "パワーローダーを獲得。 →p36"),
    (RangeInc::new(70, 70), "サブバッテリーを獲得。 →p36"),
    (RangeInc::new(71, 71), "サブバッテリー+を獲得。 →p36"),
    (RangeInc::new(72, 72), "ファランクスを獲得。 →p36"),
    (RangeInc::new(73, 73), "リアクティブアーマーを獲得。 →p36"),
    (RangeInc::new(74, 74), "強化装甲版を獲得。 →p36"),
    (RangeInc::new(75, 75), "ヘビーマシンガンSを獲得。 →p35"),
    (RangeInc::new(76, 76), "ヘビーマシンガンLを獲得。 →p35"),
    (RangeInc::new(77, 77), "掘り出し物を発見！25,000cdt以下の【上級外装兵装】一つを獲得！ 上級兵装 (→p37）"),
    (RangeInc::new(78, 79), "失敗…。かなりの損失を出してしまった…。 -3,000cdt"),
    (RangeInc::new(80, 80), "医療用品を獲得！ 調息値+［1］（全PC）"),
    (RangeInc::new(81, 81), "大型ソーラーパネルを獲得！ 整備値+［1］（全PC）"),
    (RangeInc::new(82, 82), "艦内用の環境設備を獲得！ サポート使用回数+［1］"),
    (RangeInc::new(83, 83), "リニアガンを獲得。 →p37"),
    (RangeInc::new(84, 84), "リニアマシンガンを獲得。 →p37"),
    (RangeInc::new(85, 85), "ジャマ―ユニットを獲得。 →p37"),
    (RangeInc::new(86, 86), "センサー+を獲得。 →p37"),
    (RangeInc::new(87, 87), "パワーローダー++を獲得。 →p37"),
    (RangeInc::new(88, 88), "サブバッテリー++を獲得。 →p37"),
    (RangeInc::new(89, 89), "フレームカバーを獲得。 →p37"),
    (RangeInc::new(90, 90), "空振り……何の成果も得られなかった…。 0cdt"),
    (RangeInc::new(91, 92), "良質なA:GuSのパーツを獲得！機体の改造や予備パーツとして使えそうだ！ 改造Lv+［1］（上限：3Lv）"),
    (RangeInc::new(93, 95), "A:GuSのパーツを獲得！機体の改造や予備パーツとして使えそうだ。 改造Lv+［1］（上限：1Lv）"),
    (RangeInc::new(96, 98), "多少傷ついているが、A:GuS1機を獲得！→10,000cdtで売却可能→10,000cdt支払えば補修して取得が可能。 （→p30～33）（→w23）"),
    (RangeInc::new(99, 99), "A:GuS1機をほぼ完全な状態で獲得！ （→p30～33）（→w23）"),
    (RangeInc::new(100, 100), "掘り出し物を発見！25,000cdt以下の【上級携行兵装】一つを獲得！ 上級兵装 (→p37）"),
];
static SAL: RangeTable = RangeTable::from_dice("サルベージ表", 1, 100, SAL_ITEMS);
static DEF_ITEMS: &[RangeTableItem] = &[
    (
        RangeInc::new(1, 1),
        "解雇 クルー1名を失う。10,000cdtを得る。",
    ),
    (
        RangeInc::new(2, 3),
        "劣化 任意のチーム能力一つは-1Lv。10,000cdtを得る。",
    ),
    (
        RangeInc::new(4, 5),
        "借金 次回の維持費が+20,000cdt。10,000cdtを得る。",
    ),
    (
        RangeInc::new(6, 7),
        "酷使 各PCは最大HP-4。10,000cdtを得る。",
    ),
    (
        RangeInc::new(8, 9),
        "売却 各PCはオプション以外の兵装を一つずつ廃棄。10,000cdtを得る。",
    ),
    (
        RangeInc::new(10, 10),
        "解雇 クルー1名を失う。10,000cdtを得る。",
    ),
];
static DEF: RangeTable = RangeTable::from_dice("赤字ペナルティー表", 1, 10, DEF_ITEMS);
static SPE_ITEMS: &[&str] = &[
    "混戦 以下のエリアのユニットをシャッフルする。♠：A⇔C ♣：B⇔D ♦：A⇔D ♥：B⇔C",
    "乱戦 R中、すべての攻撃は［距離：○］になる。",
    "逸失 敵拠点エリアのユニットを［♠♣：A ♦♥：B］に移動。味方拠点エリアのユニットを［♠♣：D ♦♥：C］に移動。",
    "突風 艦船、オブジェクト以外の全ユニットを「風向き」方向に移動。",
    "流砂 以下のエリアのユニットは脱出のため、MPとEPを［3］点失う。［♠：A ♣：B ♦：C ♥：D］",
    "混乱 母船内でトラブル発生。R中、【整備】は行えない。［♠♥：味方側 ♣♦：敵側］",
    "岩盤 以下のエリアのユニットは岩盤に乗り上げ、《クリティカル》が1回発生。［♠：A ♣：B ♦：C ♥：D］",
    "混乱 R中、イニシア値を逆順で処理する。 ※【エイミング】等の高低も逆として処理する。",
    "飛礫 飛礫によって、すべてのユニットはAPを［1D10］（以下のエリアでは［2D10］）点失う。［♠：A ♣：B ♦：C ♥：D］",
    "雨 雨は砂を土へと変えてしまう。R中、全ユニット移動/突撃不可。",
];
static SPE: Table = Table::from_dice("特殊戦況表", 1, 10, SPE_ITEMS);

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::{Path, PathBuf};

    fn toml_path() -> Option<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("test/data/SajinsenkiAGuS2E.toml");
        path.exists().then_some(path)
    }
    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            return;
        };
        let data = TestDataFile::load(&path).expect("TOML must parse");
        assert_eq!(
            data.tests.len(),
            49,
            "case count in test/data/SajinsenkiAGuS2E.toml"
        );
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "SajinsenkiAGuS2E");
            let mut reasons = Vec::new();
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new("SajinsenkiAGuS2E"), &tc.input, &mut src) {
                Err(e) => reasons.push(format!("eval error: {e}")),
                Ok(None) => {
                    if !tc.expects_nil() {
                        reasons.push("unexpected nil".to_owned());
                    }
                }
                Ok(Some(result)) => {
                    if tc.expects_nil() || result.text != tc.output {
                        reasons.push(format!(
                            "expected {:?}, actual {:?}",
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
                reasons.push(format!("unconsumed rands: {}", src.remaining()));
            }
            if !reasons.is_empty() {
                failures.push(format!("{}:{}: {}", i + 1, tc.input, reasons.join("; ")));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
