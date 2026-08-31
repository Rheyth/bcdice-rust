//! `lib/bcdice/game_system/GurpsFW.rb` の移植。
//!
//! 原典では表データも同じ Ruby ファイル内に定義されているため、使用する文字列を
//! そのリテラルから `static` へ転記している。

use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GurpsFW;

impl GameSystem for GurpsFW {
    fn id(&self) -> &'static str {
        "GurpsFW"
    }
    fn name(&self) -> &'static str {
        "ガープスフィルトウィズ"
    }
    fn sort_key(&self) -> &'static str {
        "かあふすふいるとういす"
    }
    fn help_message(&self) -> &'static str {
        r"--GURPS汎用コマンド----------
・判定においてクリティカル・ファンブルの自動判別、成功度の自動計算。(3d6<=目標値)
 ・祝福等のダイス目にかかる修正は「3d6-1<=目標値」といった記述で計算されます。
 (ダイス目の修正値はクリティカル・ファンブルに影響を与えません。)
 ・クリティカル値・ファンブル値への修正については現在対応していません。
・クリティカル表 (CRT)
・頭部打撃クリティカル表 (HCRT)
・ファンブル表 (FMB)
・呪文ファンブル表 (MFMB)
・命中部位表 (HIT)
・恐怖表 (FEAR+n)
　nには恐怖判定の失敗度を入れてください。
・反応判定表 (REACT, REACT±n)
　nには反応修正を入れてください。
・D66ダイスあり
--GURPS-FW専用コマンド----------
・ドロップ判定(DROP)/ネームドドロップ判定(DROPN)
 ・ドロップ判定に修正が付く場合は末尾に+xを記述(xは修正値)。(DROP+x、DROPN+x)
・必殺技表(HST)/驚異的必殺技表(KHST)
 ・ホムンクルスの【必殺技！】/【驚異的必殺技！】用コマンド。
・ナンバーワンくじ/ノーマル(LOTN)/プレミアム(LOTP)
--夢幻の迷宮(ver.2013/11/07)----------
・コマンド中のdには難易度を入れてください。(初級：E 中級：N 上級：H 悪夢：L)
・コマンド中のaには地形を入れてください。
 (1：洞窟 2：遺跡 3：断崖 4：水辺 5：森林 6：墓地)
・ランダムイベント(RANDd)/地形固定(RANDda)
・ランダムエンカウント(RENCd)/地形固定(RENCda)
・トラップリスト(TRAPd)
・報酬財宝テーブル(xに到達深度を記述)。 (TRSdx)
 ・財宝テーブルの段階が変動する場合、末尾に±yを記述(yは変動段階)。(TRSdx±y)
  [例：TRSE5-1、TRSH36+2]
・地形決定表(AREA)
・迷宮追加オプション表(RANDOP)
"
    }
    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "CRT",
            "HCRT",
            "FMB",
            "MFMB",
            "HIT",
            "FEAR",
            "REACT",
            "TRAP[ENHL]",
            "TRS[ENHL]",
            "RAND[ENHL]",
            "RENC[ENHL]",
            "AREA",
            "DROPN?",
            "HST",
            "KHST",
            "RANDOP",
            "LOT[NP]",
        ]
    }
    crate::impl_prefixes_pattern!();
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::NoSort
    }

    fn result_nd6(
        &self,
        total: crate::Int,
        dice_total: i64,
        dice: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        let Target::Number(target) = target else {
            return Some(CheckOutcome::Nothing);
        };
        if dice.len() != 3 || cmp_op != CmpOp::Le {
            return None;
        }
        let degree = target.clone() - &total;
        if (dice_total <= 6 && target >= I::from(16))
            || (dice_total <= 5 && target >= I::from(15))
            || dice_total <= 4
        {
            return Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                format!("クリティカル(成功度：{degree})"),
            ))));
        }
        if &target - dice_total <= I::from(-10)
            || (dice_total >= 17 && target <= I::from(15))
            || dice_total >= 18
        {
            return Some(CheckOutcome::Result(Box::new(EvalResult::fumble(format!(
                "ファンブル(失敗度：{degree})"
            )))));
        }
        if dice_total >= 17 {
            return Some(CheckOutcome::Result(Box::new(EvalResult::failure(
                format!("自動失敗(失敗度：{degree})"),
            ))));
        }
        Some(CheckOutcome::Result(Box::new(if total <= target {
            EvalResult::success(format!("成功(成功度：{degree})"))
        } else {
            EvalResult::failure(format!("失敗(失敗度：{degree})"))
        })))
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        Ok(eval_specific(command, rng)?.map(SpecificCommandOutput::text))
    }
}

fn modifier_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([A-Z]+)([+-]\d+)?$").unwrap())
}

fn eval_specific(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let upper = command.to_ascii_uppercase();
    if let Some((name, table)) = match upper.as_str() {
        "CRT" => Some(("クリティカル表", CRITICAL)),
        "HCRT" => Some(("頭部打撃クリティカル表", HEAD_CRITICAL)),
        "FMB" => Some(("ファンブル表", FUMBLE)),
        "MFMB" => Some(("呪文ファンブル表", MAGIC_FUMBLE)),
        "HIT" => Some(("命中部位表", HIT_LOCATION)),
        _ => None,
    } {
        let sum = roll_sum(rng, 3)?;
        return Ok(Some(format!(
            "{name}({sum})：{}",
            table[(sum - 3) as usize]
        )));
    }

    if let Some(m) = modifier_pattern().captures(&upper) {
        let modify = m.get(2).map_or(0, |v| v.as_str().parse().unwrap_or(0));
        if &m[1] == "FEAR" {
            let number = roll_sum(rng, 3)? + modify;
            return Ok(Some(format!("恐怖表({number})：{}", fear(number))));
        }
        if &m[1] == "REACT" {
            let number = roll_sum(rng, 3)? + modify;
            let result = match number {
                i64::MIN..=0 => "最悪",
                1..=3 => "とても悪い",
                4..=6 => "悪い",
                7..=9 => "良くない",
                10..=12 => "中立",
                13..=15 => "良い",
                16..=18 => "とても良い",
                _ => "最高",
            };
            return Ok(Some(format!("反応表({number})：{result}")));
        }
    }

    match upper.as_str() {
        "RANDE" => {
            let area = rng.roll_once(6)?;
            let event = rng.roll_once(6)?;
            let detail = rng.roll_once(6)?;
            let number = format!("{area}{event}{detail}");
            let result = if (area, event, detail) == (3, 1, 2) {
                RAND_E_312
            } else {
                "1"
            };
            Ok(Some(format!("ランダムイベント表({number})：{result}")))
        }
        "RENCH" => {
            let area = rng.roll_once(6)?;
            let detail = rng.roll_once(6)?;
            let number = format!("{area}4{detail}");
            let result = if (area, detail) == (1, 4) {
                RENC_H_144
            } else {
                "1"
            };
            Ok(Some(format!("ランダムエンカウント表({number})：{result}")))
        }
        "TRAPE" => {
            let sum = roll_sum(rng, 3)?;
            let result = if sum == 7 { TRAP_E_7 } else { "1" };
            Ok(Some(format!("トラップリスト({sum})：初級：{result}")))
        }
        "TRSN28" => {
            let die = rng.roll_once(6)?;
            let result = TREASURE_3[(die - 1) as usize];
            Ok(Some(format!("財宝テーブル({die})：{result}")))
        }
        "AREA" => {
            let die = rng.roll_once(6)?;
            Ok(Some(format!(
                "地形決定表({die})：{}",
                AREA[(die - 1) as usize]
            )))
        }
        "DROP" | "DROPN" => {
            let named = upper == "DROPN";
            let number = roll_sum(rng, 3)?;
            let result = if number <= 3 {
                "レアアイテム1"
            } else if number <= 4 && named {
                "レアアイテム2"
            } else if number < 7 {
                if named {
                    "CL×200GP"
                } else {
                    "CL×100GP"
                }
            } else if named {
                "CL×20GP"
            } else {
                "CL×10GP"
            };
            Ok(Some(format!("ドロップ判定({number})：{result}")))
        }
        "HST" => {
            let die = rng.roll_once(6)?;
            Ok(Some(format!(
                "必殺技表({die})：{}",
                HST[(die - 1) as usize]
            )))
        }
        "KHST" => {
            let sum = roll_sum(rng, 3)?;
            Ok(Some(format!(
                "驚異的必殺技表({sum})：{}",
                KHST[(sum - 3) as usize]
            )))
        }
        "RANDOP" => {
            let number = rng.roll_once(6)? * 10 + rng.roll_once(6)?;
            let result = if number == 31 { RANDOP_31 } else { "1" };
            Ok(Some(format!("迷宮追加オプション表({number})：{result}")))
        }
        "LOTN" => Ok(Some(format!("ナンバーワンノーマルくじ：{}", normal1(rng)?))),
        "LOTP" => Ok(Some(format!(
            "ナンバーワンプレミアムくじ：{}",
            premium1(rng)?
        ))),
        _ => Ok(None),
    }
}

fn roll_sum(rng: &mut Randomizer, count: i64) -> Result<i64, EvalError> {
    Ok(rng.roll_barabara(count, 6)?.iter().sum())
}

fn fear(number: i64) -> &'static str {
    match number {
        18 => "生命力判定を行い、失敗すると1点の負傷を受ける。さらに1D分間意識を失う。以後、1分ごとに生命力判定を行い、成功すると回復。",
        19 => "1点負傷。2D分間意識を失う。以後、1分ごとに生命力判定を行い、成功すると回復。",
        _ => "1ターン朦朧状態。2ターン目に自動回復。",
    }
}

static CRITICAL: &[&str] = &[
    "体を狙っていたら、相手は気絶(回復は30分後に生命力判定)。他はダメージ3倍。",
    "相手の防御点を無視。",
    "ダメージ3倍。",
    "ダメージ2倍。",
    "相手は生命力判定を行い、失敗すると朦朧状態となる。",
    "四肢を狙っていたら、6ターンそこが使えなくなる。通常ダメージ。",
    "通常ダメージ。",
    "通常ダメージ。",
    "通常ダメージ。",
    "四肢を狙っていたら、6ターンそこが使えなくなる。通常ダメージ。",
    "相手の防御点を無視。",
    "四肢を狙っていたら、そこが使えなくなる(通常ダメージ)。他は2倍ダメージ。",
    "相手は武器を落とす。通常ダメージ。",
    "ダメージ2倍。",
    "ダメージ3倍。",
    "体を狙っていたら、相手は気絶(回復は30分後に生命力判定)。他はダメージ3倍。",
];
static HEAD_CRITICAL: &[&str] = &[
    "敵は即死する", "敵は意識を失う。30分ごとに生命力判定をして、成功すると意識を回復する。", "敵は意識を失う。30分ごとに生命力判定をして、成功すると意識を回復する。", "敵は両目を負傷する。朦朧状態になる。目が見えないので、敏捷力-10。", "敵は片目を負傷する。朦朧状態になる。敏捷力-2。", "敵はバランスを失う。次のターンまで、防御しかできない。", "通常ダメージのみ。", "通常ダメージのみ。", "通常ダメージのみ。", "「叩き」攻撃なら、敵は24時間のあいだ耳が聞こえなくなる。「切り」「刺し」なら、1点しかダメージを与えられないが、傷跡が残る。", "「叩き」攻撃なら、敵は耳が聞こえなくなる。「切り」「刺し」なら、2点しかダメージを与えられないが、傷跡が残る。", "敵は逃げ腰になって武器を落とす(両手に武器を持っていたらランダムに決定)。", "敵は通常のダメージを受け、朦朧状態になる。", "敵は通常のダメージを受け、朦朧状態になる。", "敵は通常のダメージを受け、朦朧状態になる。", "敵は通常のダメージを受け、朦朧状態になる。",
];
static FUMBLE: &[&str] = &[
    "武器が壊れる。ただし、メイスなど固い\"叩き\"武器は壊れない(ふりなおし)。", "武器が壊れる。ただし、フレイルなど固い\"叩き\"武器は壊れない(ふりなおし)。", "自分の腕か足に命中(通常ダメージ)。ただし\"刺し\"武器や射撃ならふりなおし。", "自分の腕か足に命中(半分ダメージ)。ただし\"刺し\"武器や射撃ならふりなおし。", "バランスを失い、次ターンは行動不可。次ターンの行動の番まで、能動防御-2。", "使った武器が非準備状態になる。1ターンよぶんに準備行動を行わないと、準備状態にならない。", "武器を落とす。", "武器を落とす。", "武器を落とす。", "使った武器が非準備状態になる。1ターンよぶんに準備行動を行わないと、準備状態にならない。", "バランスを失い、次ターンは行動不可。次ターンの行動の番まで、能動防御-2。", "前か後ろ(ランダム)に武器が1メートル飛んでいく。その場にいるキャラクターは敏捷力判定を行い、失敗するとダメージ(通常の半分)を受ける。ただし、\"刺し\"武器や弓矢はその場に落ちるだけ。", "利き腕をくじいてしまう。30分間、攻撃にも防御にも使えない。", "利き腕をくじいてしまう。30分間、攻撃にも防御にも使えない。", "足をすべらせ、その場に倒れる。", "武器が壊れる。ただし、金属バットなど固い\"叩き\"武器は壊れない(ふりなおし)。",
];
static MAGIC_FUMBLE: &[&str] = &[
    "呪文が完全に失敗する。術者は1D点のダメージを受ける。", "呪文が術者にかかる。", "呪文が術者の仲間にかかる(対象はランダムに決定)。", "呪文が近くの敵にかかる(対象はランダムに決定)。", "哀れな物音があがり、硫黄のひどい匂いが立ち込める。", "呪文が目標以外のもの(仲間、敵、品物)にかかる。対象はランダムに決定するか、おもしろくなるようにGMが決定する。", "呪文が完全に失敗する。術者は1点のダメージを受ける。", "呪文が完全に失敗する。術者は朦朧状態になる(立ち直るには知力判定を行う)。", "大きな物音があがり、色とりどりの閃光が走る。", "見せ掛けの効果があらわれるが、弱くてとても役に立たない。", "意図した効果と逆の効果があらわれる。", "違った目標に、意図した効果とは逆の効果があらわれる(対象はランダムに決定)。", "何も起こらないが、術者は一時的にその呪文を忘れてしまう。思い出すまで、1週間ごとに知力判定を行う。", "呪文がかかったように思えるが、役に立たないただの見せかけだけ。", "呪文が完全に失敗し、術者の右腕が損なわれる。回復に1週間を要する。", "呪文が完全に失敗する。GMから見て、術者や呪文が純粋で善良なものでなければ、悪魔(第3版文庫版P.384参照)があらわれ、術者を攻撃する。",
];
static HIT_LOCATION: &[&str] = &[
    "脳",
    "脳",
    "頭",
    "遠い腕",
    "手首(左右ランダム)",
    "近い腕",
    "胴体",
    "胴体",
    "胴体",
    "遠い足",
    "近い足",
    "近い足",
    "足首(左右ランダム)",
    "足首(左右ランダム)",
    "重要機関(胴体の)",
    "武器",
];
static HST: &[&str] = &[
    "命中判定に[1,1,1]でクリティカル(クリティカル表も参照)。",
    "命中判定に+20のボーナス。",
    "ダメージを与えると「生命力-2」で気絶判定。",
    "ダメージを与えると「敏捷力-4」で転倒判定。",
    "致傷力+2D。",
    "命中判定に[6,6,6]でファンブル(ファンブル表も参照)。",
];
static KHST: &[&str] = &["命中判定に[1,1,1]でクリティカル。クリティカル表は参照せず、相手は即死。「分類：ネームド」「分類：魔将」に対しては最大HPの半分のダメージを与える。", "命中判定に[1,1,1]でクリティカル。クリティカル表は参照せず、致傷力3倍。", "命中判定に[1,1,1]でクリティカル。クリティカル表は参照せず、致傷力2倍。", "命中判定に[1,1,1]でクリティカル(クリティカル表も参照)。", "命中判定に+40のボーナス。", "致傷力+4D(火炎特性)。", "致傷力+3D(雷撃特性)。", "与えたダメージに等しいHPを回復する。回避に-3のペナルティを与える。", "1点でもダメージを与えた場合、対象を転倒状態にする。回避に-3のペナルティを与える。", "致傷力+3D。", "致傷力+4D(冷気特性)。", "1点でもダメージを与えた場合、-6のペナルティで気絶判定。", "致傷力+4D。防護点無視。", "致傷力+6D。回避に-3のペナルティを与える。", "命中判定に[6,6,5]でファンブル(ファンブル表も参照)。目標値が16以上だった場合は自動失敗。", "命中判定に[6,6,6]でファンブル(ファンブル表も参照)。"];

static TREASURE_3: &[&str] = &[
    "ミスリル武器(4000GPまでのもの)",
    "最高級能力増加ポーション(消耗品)",
    "高級クイック再生ポーション(消耗品)",
    "魔法1つ(5000GPまでのもの)",
    "防具1つ(5000GPまでのもの)",
    "3000GP",
];
static TRAP_E_7: &str = "スロット：スロットが揃うまで開かない宝箱。スロットを1回まわすには100GPが必要。行動を消費して「視覚-5」判定に成功すればスロットはそろう。「反射神経」があれば「視覚」そのままで判定可能。";
static RAND_E_312: &str = "断崖(初級)：休憩に丁度いい広場を見つけた。FPが2D回復するが、「意志」判定を行うこと。PCの半数以上が失敗すると居心地が良すぎて離れづらくなり次の深度判定と地形変化が起きなくなる。その場合次もこのイベントを行うこと。";
static RENC_H_144: &str = "洞窟(上級)：デビルホイール(CL26)とエンカウント、防護点+4、HP+24。トロッコに乗って逃げつつの戦闘になり、2ラウンド以内に倒せなければ轢かれてPC全員が8Dの防護点無視ダメージを受ける。また、1ラウンドに1人誰かが体力判定を行ってトロッコを運転する必要があり、これに失敗すると即座に轢かれてしまう。轢かれると戦闘は終了する。パーテイー内にケイヴウォーカーがいればこの判定は不要。";
static RANDOP_31: &str = "「暗闇の迷宮」　初期深度+5\n「暗視」がなければ視覚判定に-5のペナルティを受ける。\nストームコーザーはペナルティが2倍。";
static AREA: &[&str] = &[
    "洞窟\n「ん、暗くて先が見えないって？そりゃこのフィルトウィズのことかい？」\n姿を様々に変える洞窟。ケイヴウォーカーがいれば有利に探索可能。非常に暗く「暗視」がなければ満足に進むことはできないだろう。\n☆深度判定：体力判定(「暗視」があれば深度判定に+3のボーナスを受ける)\n☆屋内(飛行不可)\n☆薄暗い(ストームコーザー「鳥目」を適用)",
    "遺跡\n「どんな仕掛けにだって意味はある。人が作ったものだからな」\n人為的に作られた様々な建造物の内部。\n様々な恐ろしい仕掛けが行く手を阻む。\n☆深度判定：<探索>\n☆屋内(飛行不可)",
    "断崖\n「うーん、とっても気持ちのいい風ね。ん？何を震えてるの？」\n一歩踏み外せば奈落の底。過酷な自然の要塞。\nストームコーザーなどの飛行可能な仲間がいると心強いだろう。\n☆深度判定：<軽業>\n☆屋外",
    "水辺\n「人間とは何かと不便なことが多い種族ですな」\n川、湖などを泳いだりして進んでいくダンジョン。\nリザードやワイズマンがその力を発揮するだろう。\n☆深度判定：<水泳>\n(水泳に「自動的に成功」するキャラクターは敏捷力+4で判定可能。\n 《水泳》のかかっているキャラクターがいた場合も同様。\n 【ミズグモ】があれば敏捷力+2で深度判定可能)\n☆屋外 ",
    "森林\n「ここが危険だと思う？それはアナタがこの森では『異質』だからよ」\n鬱蒼とした森林は、人間にはとても過酷な環境となっている。\nフラウなどの自然と共に生きる者の力が助けになるだろう。\n☆深度判定：<生存>\n☆屋外",
    "墓地\n「客人とは珍しい・・・『死者の王』に出会わぬよう、ゆめゆめご注意を・・・」\n死者どもの彷徨う、暗く冷たい墓地。\nローブをかぶった得体の知れない墓守を<追跡>して脱出せよ。\n☆深度判定：<追跡>\n☆屋外\n☆薄暗い(ストームコーザー「鳥目」を適用)",
];

fn pick<'a>(rng: &mut Randomizer, table: &'a [&'a str]) -> Result<&'a str, EvalError> {
    Ok(table[(rng.roll_once(6)? - 1) as usize])
}
fn normal1(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1..=3 => Ok("イレブンチキン"),
        4 | 5 => normal2(rng),
        _ => normal3(rng),
    }
}
fn normal2(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("バロールたわし"),
        2 => Ok("イグニスジッポ"),
        3 => Ok("ヤコ仮面or梟の文鎮(選択可)"),
        4 => Ok("ナレッジのハンモックorジンジャビースト"),
        _ => normal3(rng),
    }
}
fn normal3(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("特性HPポーション"),
        2 => Ok("特性MPポーション"),
        3 => Ok("黒い甲冑"),
        4 => Ok("天体望遠鏡"),
        5 => Ok("金獅子の剥製"),
        _ => normal4(rng),
    }
}
fn normal4(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("特性スタミナポーション"),
        2 => Ok("戦乙女の兜"),
        3 => Ok("フェンリルの首輪"),
        4 => Ok("フェニックスカーペット"),
        5 => Ok("動くアダマンゴーレム"),
        _ => normal5(rng),
    }
}
fn normal5(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("キャンディークッション"),
        2 => Ok("屑鉄の金床"),
        3 => Ok("薪割り王の斧"),
        4 => Ok("ロジエの水差し"),
        5 => Ok("箱舟の模型"),
        _ => premium5(rng),
    }
}
fn premium1(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1..=3 => Ok("プレミアムチキン"),
        4 => normal3(rng),
        _ => premium2(rng),
    }
}
fn premium2(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("親衛隊バッジ"),
        2 => Ok("ハタモトチャブダイ"),
        3 => Ok("星のコンパス"),
        4 => Ok("白銀の甲冑"),
        5 => normal4(rng),
        _ => premium3(rng),
    }
}
fn premium3(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("特性クイックHPポーション"),
        2 => Ok("特性クイックMPポーション"),
        3 => Ok("特製クイックスタミナポーション"),
        4 => Ok("火龍のフィギュアor氷龍のフィギュア(選択可)"),
        5 => Ok("ヒメショーグンドレス"),
        _ => premium4(rng),
    }
}
fn premium4(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("クイックユグドラポーション"),
        2 => Ok("銀河龍のフィギュア/ドラゴン"),
        3 => Ok("銀河龍のフィギュア/魔族"),
        4 => Ok("魔族チェスセット"),
        5 => Ok("イグニスコンロ"),
        _ => premium5(rng),
    }
}
fn premium5(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    match rng.roll_once(6)? {
        1 => Ok("グレヴディバリウス"),
        2 => Ok("天使の望遠鏡orデスの目覚まし時計(選択可)"),
        3 => Ok("世界樹の蔦"),
        4 => Ok("死神の飾りドレス"),
        5 => Ok("ザバーニヤ等身大フィギュア"),
        _ => premium6(rng),
    }
}
fn premium6(rng: &mut Randomizer) -> Result<&'static str, EvalError> {
    pick(
        rng,
        &[
            "イレブンチキン",
            "イレブンチキン(2ピース)",
            "イレブンチキン(3ピース)",
            "イレブンチキン(6ピース)",
            "イレブンチキン(12ピース)",
            "wish star",
        ],
    )
}

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
            .join("test/data/GurpsFW.toml");
        path.exists().then_some(path)
    }
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else { return };
        let data = TestDataFile::load(&path).expect("GurpsFW.toml must parse");
        assert_eq!(data.tests.len(), 36, "case count in test/data/GurpsFW.toml");
        let mut failures = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(tc.game_system, "GurpsFW");
            let mut src = SeededRandomizer::new(tc.rands.iter().map(|r| (r.value, r.sides)));
            match eval_command(&GameSystemId::new("GurpsFW"), &tc.input, &mut src) {
                Ok(Some(result)) if !tc.expects_nil() => {
                    if result.text != tc.output
                        || result.secret != tc.secret
                        || result.success != tc.success
                        || result.failure != tc.failure
                        || result.critical != tc.critical
                        || result.fumble != tc.fumble
                    {
                        failures.push(format!(
                            "{}:{}\nexpected: {:?}\nactual: {:?}",
                            i + 1,
                            tc.input,
                            tc.output,
                            result
                        ));
                    }
                }
                Ok(None) if tc.expects_nil() => {}
                other => failures.push(format!("{}:{}: {other:?}", i + 1, tc.input)),
            }
            if !src.is_empty() {
                failures.push(format!(
                    "{}:{}: {} unconsumed rands",
                    i + 1,
                    tc.input,
                    src.remaining()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
