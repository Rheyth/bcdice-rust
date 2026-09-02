//! P4で手書き移植した `lib/bcdice/game_system/Paradiso.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Paradiso#getJudgeResult`（判定 `nCPt[f1,f2]@c` / `nD20<=t[f1,f2]@c`）
//! - `Paradiso#getDamageResult`（ダメージチェック `DCa[20,30,…]`）
//! - 各種表（`RMT` / `TOT` / `EXT` / `SUT`。いずれも1D20）
//!
//! 表データは同名 `.rb` から機械的に書き出したもので、値は1文字も変えていない
//! （Rubyのダブルクォート文字列なので `\[手がかり\]` は `[手がかり]`、
//! `P\.69` は `P.69` に解決される。改行 `\n` も原典どおり）。

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::RangeInc;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

/// Ruby `get_radiomarietta_table` の項目（ラジオマリエッタ表、1D20）。
static RADIO_MARIETTA_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 1), "「なんてこった！　ここで事故のお知らせだ！」\n通行止め……。ランダムなマス1つを決定する。この一日中、そのマスに移動する事はできない。"),
    (RangeInc::new(2, 4), "「今日はまたずいぶんと湿気てるねぇ……。古傷がある人は要注意だよ」\n天候が悪い。この一日中、「●移動」のアクションで移動できるマス数は常に1マス低くなる。"),
    (RangeInc::new(5, 10), "「日常は至上！　異常は退場！　なんにもないからラジオは以上！　……なんつて」\nいつもどおりの日々。のんびりとした風で、何事もなし。"),
    (RangeInc::new(11, 11), "「それじゃ、本日のメインコーナー。行ってみよう！」\n軽妙なトーク。PC全員の【乗り手コンディション】が1点小さくなる。"),
    (RangeInc::new(12, 12), "「それじゃ、本日のメインコーナー。行ってみよう！」\n軽妙なトーク。PC全員の【乗り手コンディション】が1点小さくなる。"),
    (RangeInc::new(13, 15), "「いーなー、こんな日はボクも飛んでみたい気分だよ！　ブオノあたりまでバーッとね！」\nとんでもなく快晴で絶好のフライト日和。【機体コンディション】【乗り手コンディション】がそれぞれ1点小さくなる。"),
    (RangeInc::new(16, 16), "「店頭で言えば嬉しい値引き。本日のラッキーワードをメモする用意はできたかい？」\nおトクな情報。この一日中、各PCごとに一回ずつ、「価格」の効果値が2低いものとして効果を処理できる（最低値0）"),
    (RangeInc::new(17, 17), "「いっやー……熱演だったね。もーぅ次回が待ちきれなぁい！！」\nラジオドラマが神回だった。その日一日に行う「交流」で獲得できる【キズナ】の点数が+1される。"),
    (RangeInc::new(18, 18), "「イエス！ナイス！エレガンス！あのサーカス団が帰ってくる！」\nサーカス団がやってくる！ランダムなマス1つを決定する。\nこの一日中、そのマスは「娯楽施設：5」（P.55）の効果を得る。"),
    (RangeInc::new(19, 19), "「ラジオネーム、ハプニングさんからのお便り！　おっとぉ、これは興味深い相談だ」\nラジオの話している内容から手がかりが見つかる。[手がかり]が1箇所追加で配置される。"),
    (RangeInc::new(20, 20), "「今夜は素敵なパーリィデイ！　みんな！今夜の仮装を何にするかはもう決めてるかな？」\n酒場でパーティだ！「酒場」のスポット効果を持つスポットに「レストラン：「パーティ」」が追加される。"),
];

/// Ruby `get_takeoff_table` の項目（移動表、1D20）。
static TAKEOFF_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 1), "エンジンがぶっ壊れた！ただちに【機体コンディション】が「20」となり、このターン中は2つ目のアクションも含め「●移動」することができない。"),
    (RangeInc::new(2, 2), "離水に失敗した！　キミの愛機のダメージマップ上の任意の「翼」部位のダメージボックスに1点のダメージを与え、このターン中は2つ目のアクションも含め「●移動」することができない。"),
    (RangeInc::new(3, 3), "軽いエンジントラブル。このアクションでは移動することができない。"),
    (RangeInc::new(4, 4), "同業者に遭遇。しかし煽られて曲芸飛行につきあわされる。\n任意の方向に強制的に3マス移動し、【物資点】3点を失う。"),
    (RangeInc::new(5, 5), "道を間違えたらしい。【物資点】を5点消費し、ランダムな方向に1マス移動する効果を3回繰り返す。"),
    (RangeInc::new(6, 6), "気づいたらオイル漏れを起こしていた！【物資点】を3点消費する。その後、1マスにつき1点の【物資点】を消費して最大4マスまで移動できる。"),
    (RangeInc::new(7, 7), "あいにくのにわか雨。あまり飛びたくないなあ。1マスにつき1点の【物資点】を消費して最大2マスまで移動できる。"),
    (RangeInc::new(8, 8), "唐突な襲撃。一撃加えたあと、謎の襲撃者はいずこかへ去っていった……。命中判定の達成値が12であると扱う、【火力】3のダメージチェックを受ける。その後、1マスにつき1点の【物資点】を消費して最大4マスまで移動できる。"),
    (RangeInc::new(9, 9), "んー、少し調子が悪いかな？　1マスにつき1点の【物資点】を消費して最大3マスまで移動できる。"),
    (RangeInc::new(10, 12), "順調な空の旅。1マスにつき1点の【物資点】を消費して最大5マスまで移動できる。"),
    (RangeInc::new(13, 13), "島巡りの観光艇と遭遇。ちやほやされていい気分。1マスにつき1点の【物資点】を消費して最大5マスまで移動できる上、キミの【乗り手コンディション】を2点までの任意の点数下げる事ができる。"),
    (RangeInc::new(14, 14), "同業者と遭遇。1マスにつき1点の【物資点】を消費して最大5マスまで移動できる上、同業者は「キミへの【キズナ】」を1点得る。同業者はこのセッション中、キミが望む場面でキミに「判定支援」を行ってくれる。"),
    (RangeInc::new(15, 15), "すごく調子がいいぞ！1マスにつき1点の【物資点】を消費して最大7マスまで移動できる上、キミの【機体コンディション】を2点までの任意の点数下げる事ができる。"),
    (RangeInc::new(16, 16), "すごく調子がいいぞ！1マスにつき1点の【物資点】を消費して最大5マスまで移動できる上、このアクションがこのターンに行う1回目のアクションである場合、2回目のアクションでも続けて「●移動」を行う事ができる。"),
    (RangeInc::new(17, 17), "通りかかった先に思わぬ情報が！1マスにつき1点の【物資点】を消費して最大5マスまで移動できる上、このアクションがこのターンに行う1回目のアクションである場合、2回目のアクションでは今いるマスに[手がかり]が配置されているものとして「●探索」が行える。"),
    (RangeInc::new(18, 18), "酒場が恋しい……。【物資点】を5点消費し、即座に同じ「クエストマップ」内の「酒場」のスポット効果を持つマスに移動する。"),
    (RangeInc::new(19, 19), "アジトが恋しい……。【物資点】を5点消費し、即座に同じ「クエストマップ」内の任意のキミの「アジト」に移動する。"),
    (RangeInc::new(20, 20), "仲間が恋しい……。【物資点】を5点消費し、即座に任意のPC一人のいる場所に移動する。"),
];

/// Ruby `get_exploration_table` の項目（探索表、1D20）。
static EXPLORATION_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 1), "クソっ！このマスに付与されていた[手がかり]を失う。"),
    (RangeInc::new(2, 2), "「ツケ払いやがれ！」見に覚えがあるかないか。キミに詰め寄ってくるヤツがいる。【物資点】を10点消費するか、「ツケを伸ばす」のどちらかを選択する。ツケを伸ばすを選択した場合、次にキミが行う「●探索」のアクションでも、探索表の結果は参照せず、自動的にこの効果が適用される。"),
    (RangeInc::new(3, 3), "謎は深まる。このマスに付与されていた[手がかり]を失い、ランダムな場所に再付与する。【情報点】は得られない。"),
    (RangeInc::new(4, 4), "コネクションは大事だ。「支援チェック」をチェックしていない【キズナ】が1点以上存在すれば、その「支援チェック」を入れたあと、このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。"),
    (RangeInc::new(5, 8), "情報を手に入れるためには、少し骨を折る必要がありそうだ。好きな能力値を2つ組み合わせて｛探索判定｝を行う。成功すればこのマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。"),
    (RangeInc::new(9, 9), "情報を提供してくれるというアイツは見返りを要求してきた。【物資点】を4点消費できる。そうした場合、このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。"),
    (RangeInc::new(10, 13), "危なげなく情報ゲット。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。"),
    (RangeInc::new(14, 14), "手がかりを追っている事を話すと、ソイツは協力を持ちかけてきてくれた。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。さらに、【物資点】を5点獲得する。"),
    (RangeInc::new(15, 15), "手がかりを追っている事を話すと、ソイツは協力を持ちかけてきてくれた。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。さらに、アイテム「チケット」（P.69）を入手する。"),
    (RangeInc::new(16, 16), "昔の仲間から手がかりについて聞くことになった。ついでに積もる話も少々。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。さらに同業者は「キミへの【キズナ】」を1点得る。同業者はこのセッション中、キミが望む場面でキミに「判定支援」を行ってくれる。"),
    (RangeInc::new(17, 17), "空軍にいる友人から手がかりについて聞くことになった。「なあ、お前もフラフラしてないで空軍に入ったらどうだ？」耳に痛い。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。さらに、アイテム「空軍のツテ」（P.69）を入手する。"),
    (RangeInc::new(18, 18), "手がかりを追っていたら他にもボロボロと……。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。さらに、1D20を二回振り、この「クエストマップ」上のランダムなマス2つを求める。それらのマスに[手がかり]が付与されていなければ、[手がかり]を付与する。"),
    (RangeInc::new(19, 19), "あっさり情報を掴むことができてしまった。このマスに付与されていた[手がかり]を失い、【情報点】を1点獲得する。この「●探索」ではアクションを消費せず、追加で別のアクションを宣言する事ができる。"),
    (RangeInc::new(20, 20), "これは重要な手がかりだ！　このマスに付与されていた[手がかり]を失い、【情報点】を2点獲得する。"),
];

/// Ruby `get_flightsupply_table` の項目（補給表、1D20）。
static FLIGHT_SUPPLY_ITEMS: &[(RangeInc, &str)] = &[
    (RangeInc::new(1, 1), "……えっ？！　キミの【物資点】は0点となる。"),
    (RangeInc::new(2, 2), "おいおい勘弁してくれよ……。このアクションがそのセグメントの一回目のアクションだった場合、キミは2回目のアクションを行えない。その後【物資点】を5点獲得する。"),
    (RangeInc::new(3, 3), "取材に巻き込まれる。【物資点】は獲得できないが、記者の発言からはぽろりとなにかが見えたような?1D20を振り、出た目に対応したマスに「手がかり」を1つ配置する。"),
    (RangeInc::new(4, 4), "成果ゼロ。ま、こんな日もあるかな。【物資点】は獲得できない。"),
    (RangeInc::new(5, 5), "うまいこと補給できなかった。【物資点】を5点獲得する。"),
    (RangeInc::new(6, 6), "「一稼ぎと言ったらこれだろ?」と声をかけてくる悪友たち。「カジノ」(『基本ルールブック』P.55)のスポット効果を即座に適用する。ただしこの処理では判定の失敗により「刑務所」のスポット効果を持つスポットに移動する効果は発生せず、代わりに「酒場」のスポット効果を持つスポットに移動した上で、自身が持つ全ての【キズナ】の「支援チェック」にチェックを入れる。その後、次のセグメントが終了するまでの間アクションは行えない。"),
    (RangeInc::new(7, 9), "のんびり釣りといこう。釣果は運次第だ。1D20を振り、出た目と同じ数だけ【物資点】を獲得する。"),
    (RangeInc::new(10, 12), "なにごともなく補給が完了する。【物資点】を10点獲得する。"),
    (RangeInc::new(13, 13), "ラジオの音が聞こえる。PCが望むなら、1D20を振り、出た目を「ラジオ・マリエッタ表」(『基本ルールブック』P.29)に照らし合わせて、その結果を反映する。これ以後、朝セグメントで振られたラジオ・マリエッタ表の効果は失われる。その後【物資点】を10点獲得する。"),
    (RangeInc::new(14, 14), "補給の合間、ちょっと口寂しくなってしまって露店へ。【物資点】を8点獲得し、アイテム「レモネード」(『基本ルールブック』P.69)を入手する。"),
    (RangeInc::new(15, 15), "補給の合間、軽くメンテナンス。【機体コンディション】を1点下げることができる。その後【物資点】を10点獲得する。"),
    (RangeInc::new(16, 16), "補給の合間、店主と軽く談笑。【乗り手コンデイション】を1点下げることができる。その後【物資点】を10点獲得する。"),
    (RangeInc::new(17, 17), "補給の合間、仲間に軽く挨拶しておこうか。同じマスに他のPCがいた場合、そのPC1人への【キズナ】を1点獲得する。その後【物資点】を10点獲得する。"),
    (RangeInc::new(18, 18), "補給の合間に通りがかった相手と意気投合。相手はキミへの【キズナ】を1点取得する。【物資点】を10点獲得する。"),
    (RangeInc::new(19, 19), "あっさり補給が終わってしまった。どうしようかな。この補給ではアクションを消費せず、【物資点】を10点獲得する。"),
    (RangeInc::new(20, 20), "降って湧いた幸運！【物資点】が20点になる。"),
];

/// Ruby `BCDice::GameSystem::Paradiso`（ID: `Paradiso`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paradiso;

impl GameSystem for Paradiso {
    fn id(&self) -> &'static str {
        "Paradiso"
    }

    fn name(&self) -> &'static str {
        "チェレステ色のパラディーゾ"
    }

    fn sort_key(&self) -> &'static str {
        "ちえれすていろのはらていいそ"
    }

    fn help_message(&self) -> &'static str {
        r"◆判定　(nCPt[f]@c)、(nD20<=t[f]@c)　n:ダイス数（省略時:1）　t:目標値（省略時:14）　f:絶不調の追加ファンブル値　c:人機一体の追加クリティカル値
　例）CP12　CP(13+1)　3CP12[18,19]@7
◆各種表
　・ラジオ・マリエッタ表　RMT
　・移動表　TOT
　・探索表　EXT
　・補給表　SUT
◆ダメージチェック　(DCa[20,30])　a:【攻撃力】、[20]:20mm機銃追加、[30]:30mmガンポッド追加
　例）DC4:【攻撃力】4でダメージチェック　DC5[20]:【攻撃力】5でダメージチェック、うち1つは20mm機銃　DC5[20,30]:【攻撃力】5でダメージチェック、うち1つは20mm機銃、うち1つは30mmガンポッド
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[r"\d*D20<=", r"\d*CP", "RMT", "TOT", "EXT", "SUT", "DC"]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Paradiso#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = judge_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        if let Some(text) = damage_result(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }

        // Ruby は `command.upcase` してから case に掛けるが、`Base#dice_command` が
        // すでに大文字化している（`@enabled_upcase_input` は既定の true）。
        let table = match command {
            "RMT" => Some(("ラジオマリエッタ表", RADIO_MARIETTA_ITEMS)),
            "TOT" => Some(("移動表", TAKEOFF_ITEMS)),
            "EXT" => Some(("探索表", EXPLORATION_ITEMS)),
            "SUT" => Some(("補給表", FLIGHT_SUPPLY_ITEMS)),
            _ => None,
        };
        let Some((name, items)) = table else {
            return Ok(None);
        };

        Ok(Some(SpecificCommandOutput::text(roll_1d20_table(
            name, items, rng,
        )?)))
    }
}

/// Ruby `/^(\d+)?D20<=(\d+)?(\[(\d+)(,(\d+))?\])?(@(\d+))?$/i`。
fn d20_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)?D20<=(\d+)?(\[(\d+)(,(\d+))?\])?(@(\d+))?$").expect("valid regex")
    })
}

/// Ruby `/^(\d+)?CP(\d+)?(\[(\d+)(,(\d+))?\])?(@(\d+))?$/i`。
fn cp_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(\d+)?CP(\d+)?(\[(\d+)(,(\d+))?\])?(@(\d+))?$").expect("valid regex")
    })
}

/// Ruby `Paradiso#getJudgeResult`（通常判定）。
fn judge_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let captures = d20_pattern()
        .captures(command)
        .or_else(|| cp_pattern().captures(command));
    let Some(m) = captures else {
        return Ok(None);
    };

    // Ruby: (Regexp.last_match(n) || 既定値).to_i
    let group = |n: usize, default: i64| m.get(n).map_or(default, |g| ruby_to_i(g.as_str()));
    let number = group(1, 1); // ダイス数。省略時は1
    let target = group(2, 14); // 目標値。省略時は14
    let fumble1 = group(4, 21); // 追加ファンブル値。省略時は21
    let fumble2 = group(6, 21); // 追加ファンブル値。省略時は21
    let critical = group(8, 21); // 追加クリティカル値。省略時は21

    let mut dice_texts: Vec<String> = Vec::new();
    let mut crit_flg = false;
    let mut fumb_flg = false;
    let mut succ_flg = false;

    // Ruby: number.times do ... end（number <= 0 なら1度も回らない）
    let mut i = 0;
    while i < number {
        let dice = rng.roll_once(20)?;
        dice_texts.push(dice.to_string());

        if dice == 1 || dice == critical {
            // パラディーゾではクリティカル優先
            crit_flg = true;
        } else if dice == 20 || dice == fumble1 || dice == fumble2 {
            fumb_flg = true;
        } else if dice <= target {
            succ_flg = true;
        }
        i += 1;
    }

    let result = if crit_flg {
        "クリティカル"
    } else if fumb_flg {
        "ファンブル"
    } else if succ_flg {
        "成功"
    } else {
        "失敗"
    };

    Ok(Some(format!(
        "({number}D20 目標値{target}) ＞ ({}) ＞ {result}",
        dice_texts.join(",")
    )))
}

/// Ruby `/^DC(\d+)(\[(\d+)(,(\d+))?(,(\d+))?(,(\d+))?(,(\d+))?(,(\d+))?\])?$/i`。
fn damage_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^DC(\d+)(\[(\d+)(,(\d+))?(,(\d+))?(,(\d+))?(,(\d+))?(,(\d+))?\])?$")
            .expect("valid regex")
    })
}

/// Ruby `Paradiso#getDamageResult`（ダメージチェック）。
fn damage_result(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some(m) = damage_pattern().captures(command) else {
        return Ok(None);
    };

    // Ruby: attack = (Regexp.last_match(1) || 1).to_i（`(\d+)` は必須なので必ずある）
    let attack = m.get(1).map_or(1, |g| ruby_to_i(g.as_str()));

    // Ruby は `biggun = [0, 0, 0]` に index 5 まで代入するので、
    // 配列が自動で6要素へ伸びる。その結果をそのまま6要素で持つ。
    let biggun: Vec<i64> = [3usize, 5, 7, 9, 11, 13]
        .iter()
        .map(|&n| m.get(n).map_or(0, |g| ruby_to_i(g.as_str())))
        .collect();

    let mut tripledam = biggun.iter().filter(|&&bg| bg == 30).count();
    let mut doubledam = biggun.iter().filter(|&&bg| bg == 20).count();

    let mut dice_texts: Vec<String> = Vec::new();
    let mut damage = [0i64; 20];

    // Ruby: attack.times do ... end
    let mut i = 0;
    while i < attack {
        let dice = rng.roll_once(20)?;
        let mut dice_text = dice.to_string();

        let add = if tripledam >= 1 {
            tripledam -= 1;
            dice_text.push_str("【30mm】");
            3
        } else if doubledam >= 1 {
            doubledam -= 1;
            dice_text.push_str("【20mm】");
            2
        } else {
            1
        };

        // Ruby: damage[dice - 1] += add（出目は 1..20 なので添字は 0..19）。
        // Ruby の配列は範囲外の添字でも自動で伸びて例外にならないので、
        // ここも範囲外は「表示されない位置への加算」として黙って捨てる。
        if let Some(slot) = usize::try_from(dice - 1)
            .ok()
            .and_then(|index| damage.get_mut(index))
        {
            *slot += add;
        }

        dice_texts.push(dice_text);
        i += 1;
    }

    // Ruby: "\n" 区切りで5マスずつ4行
    let mut result = String::new();
    for row in damage.chunks(5) {
        result.push('\n');
        for value in row {
            result.push_str(&value.to_string());
        }
    }

    Ok(Some(format!(
        "攻撃力{attack}ダメージチェック ＞ ({}) ＞ {result}",
        dice_texts.join(",")
    )))
}

/// Ruby の各種表（`get_radiomarietta_table` 等）。
///
/// いずれも 1D20 を振り、`case` の範囲で本文を選び、
/// `"表名" + "(" + 出目 + ")：" + 本文` を返す（区切りは全角コロン）。
fn roll_1d20_table(
    name: &str,
    items: &[(RangeInc, &'static str)],
    rng: &mut Randomizer,
) -> Result<String, EvalError> {
    let dice = rng.roll_once(20)?;
    // 原典の `case` は 1..20 を隙間なく覆っているので、該当なしにはならない。
    let text = items
        .iter()
        .find(|(range, _)| range.includes(dice))
        .map_or("", |(_, text)| *text);
    Ok(format!("{name}({dice})：{text}"))
}

/// Ruby `String#to_i`。ここに来るのは `\d+` なので符号や空白は現れない。
fn ruby_to_i(s: &str) -> i64 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        // Ruby: "".to_i == 0
        return 0;
    }
    // 桁あふれは Ruby だと Bignum になる。i64 に収まらない場合は飽和させ、
    // ダイス個数なら `roll_once` の呼び出し回数上限（TooManyRandsError）へ落ちる。
    digits.parse().unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Paradiso", "Paradiso.toml", 12);
    }
}
