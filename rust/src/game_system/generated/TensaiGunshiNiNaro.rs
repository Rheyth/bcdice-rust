//! P4で手書き移植した `lib/bcdice/game_system/TensaiGunshiNiNaro.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `TensaiGunshiNiNaro#eval_game_system_specific_command`
//!   （`#roll_judge` → `#roll_damage` → `Base#roll_tables`）
//!
//! # 表・定型文データ
//!
//! Ruby側は `DiceTable::D66Table.from_i18n` / `Table.from_i18n` で
//! `i18n/TensaiGunshiNiNaro/ja_jp.yml` から表を作る。Rust側は同じ値を `static` として
//! 直接持つ。データ部分は同YAML（と汎用文言の `i18n/ja_jp.yml`）から機械的に
//! 書き出したもので、値は1文字も変えていない。
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic::ruby_div;
use crate::command_parser::Parser;
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 定型文（i18n/TensaiGunshiNiNaro/ja_jp.yml, i18n/ja_jp.yml）
// ---------------------------------------------------------------------------

/// i18n `TensaiGunshiNiNaro.JUDGE.critical`
const JUDGE_CRITICAL: &str = "スペシャル";
/// i18n `TensaiGunshiNiNaro.JUDGE.fumble`
const JUDGE_FUMBLE: &str = "ファンブル";
/// i18n `TensaiGunshiNiNaro.NORMAL.critical`
const NORMAL_CRITICAL: &str = "【戦略ポイント】が1点上昇する。";
/// i18n `TensaiGunshiNiNaro.NORMAL.fumble`
const NORMAL_FUMBLE: &str = "天才軍師のプレイヤーは、【イレギュラー】を山札から1枚引く。";
/// i18n `TensaiGunshiNiNaro.BLADE.critical`
const BLADE_CRITICAL: &str =
    "そのラウンドの間、自分の【攻撃力】が1点上昇する。この効果は2回まで重複する。";
/// i18n `TensaiGunshiNiNaro.YOU.critical`
const YOU_CRITICAL: &str =
    "獲得した準英傑に好きな英傑汎用スキルを1つ選んで獲得させることができる。";
/// i18n `success`（`i18n/ja_jp.yml`）
const SUCCESS: &str = "成功";
/// i18n `failure`（`i18n/ja_jp.yml`）
const FAILURE: &str = "失敗";

// ---------------------------------------------------------------------------
// 表（i18n/TensaiGunshiNiNaro/ja_jp.yml の `table`）
// ---------------------------------------------------------------------------

static RELA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("おじやおばのような関係／遠い親戚")),
    (12, TableItem::Text("尊敬できる存在／嫉妬した相手")),
    (
        13,
        TableItem::Text("その能力に心を打たれた／その能力は危険だと感じた"),
    ),
    (
        14,
        TableItem::Text("いつも助かっている／もっと頑張ってほしい"),
    ),
    (15, TableItem::Text("いっしょにいると楽しい／鬱陶しい")),
    (
        16,
        TableItem::Text("国のために働いている忠臣／油断ならない人物"),
    ),
    (22, TableItem::Text("私の宝物／俗物")),
    (23, TableItem::Text("死ぬときは一緒だ／一緒に生き残ろう")),
    (24, TableItem::Text("この剣を預ける／共に研鑽しよう")),
    (25, TableItem::Text("すべてを捧げたい／恩に報いたい")),
    (26, TableItem::Text("守ると決めた／幸せになってほしい")),
    (
        33,
        TableItem::Text("自分にはできない／自分でもできそうだと思う"),
    ),
    (
        34,
        TableItem::Text("からかいがいのある相手／真面目な話ばかりする"),
    ),
    (35, TableItem::Text("盟友！／友達")),
    (36, TableItem::Text("いい人／怖い人")),
    (44, TableItem::Text("半身／仇敵")),
    (
        45,
        TableItem::Text("幼いころからの親友／知り合ったばかりでよくわからない"),
    ),
    (46, TableItem::Text("兄弟姉妹（のようなもの）／仕事仲間")),
    (55, TableItem::Text("憧れる／同類と思われると困る")),
    (
        56,
        TableItem::Text("いいところを知っている／皆に隠している顔を知っているのは自分だけ"),
    ),
    (
        66,
        TableItem::Text("生き別れのきょうだいに似ている……／同郷"),
    ),
];
static RELA: D66Table = D66Table::new("関係決定表", D66SortType::Asc, RELA_ITEMS);

static PTGS_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("サボっている")),
    (12, TableItem::Text("内政の改革を進めている")),
    (13, TableItem::Text("軍の様子を見ている")),
    (14, TableItem::Text("山のような書類を片付けている")),
    (15, TableItem::Text("密偵と影の中で会話をしている")),
    (16, TableItem::Text("戦術書を読んでいる")),
    (22, TableItem::Text("釣りをしている")),
    (23, TableItem::Text("街に出て人々の様子を見ている")),
    (24, TableItem::Text("芸術を嗜んでいる")),
    (25, TableItem::Text("音楽を聴いてリラックスしている")),
    (26, TableItem::Text("よい食事を求めて町を彷徨っている")),
    (33, TableItem::Text("王族や貴族のパーティに参加している")),
    (34, TableItem::Text("近隣諸国を観光しつつ内情を探る")),
    (35, TableItem::Text("自国の経済状況を見つめ直している")),
    (36, TableItem::Text("自国の歴史を改めて確かめている")),
    (44, TableItem::Text("頭を休めるため寝ている")),
    (45, TableItem::Text("一人自然の中に身を置いている")),
    (46, TableItem::Text("自分の軍略を書籍にまとめている")),
    (
        55,
        TableItem::Text("不敵な笑みを浮かべているが何をしているかは秘密"),
    ),
    (56, TableItem::Text("次なる計略を考えてニヤついている")),
    (66, TableItem::Text("よからぬことをしていると噂されている")),
];
static PTGS: D66Table = D66Table::new("平時天才軍師表", D66SortType::Asc, PTGS_ITEMS);

static PTHE_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("堂々とサボっている")),
    (12, TableItem::Text("村同士の諍いを収めている")),
    (13, TableItem::Text("軍の指南をしている")),
    (14, TableItem::Text("自らの技を磨いている")),
    (15, TableItem::Text("仲間と国の未来について話している")),
    (16, TableItem::Text("書類仕事をこなしている")),
    (22, TableItem::Text("狩りをしている")),
    (23, TableItem::Text("街に出て人々と触れ合っている")),
    (24, TableItem::Text("街の食事を楽しんでいる")),
    (25, TableItem::Text("街で買い物を楽しんでいる")),
    (26, TableItem::Text("話を聞き困りごとを解決している")),
    (33, TableItem::Text("自分にできることは何か考えている")),
    (
        34,
        TableItem::Text("教師のような立場となって人に教えている"),
    ),
    (35, TableItem::Text("趣味に走っている")),
    (36, TableItem::Text("誰かと一緒に出かけている")),
    (44, TableItem::Text("自国の観光地を回って楽しむ")),
    (
        45,
        TableItem::Text("自分たちが置かれている状況を再確認する"),
    ),
    (46, TableItem::Text("周辺国を回りどんな状況下確認する")),
    (55, TableItem::Text("自然の中に身を置いて集中する")),
    (56, TableItem::Text("一人で静かに過ごしている")),
    (66, TableItem::Text("軍師のことを見張っている")),
];
static PTHE: D66Table = D66Table::new("平時英傑表", D66SortType::Asc, PTHE_ITEMS);

static SCOU_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("仮面で顔を隠した怪しげな騎士が仲間に入れてほしいと言ってきた。その素性はわからない。いったい何者なんだ……。")),
    (12, TableItem::Text("酒場や飯屋で出会った人物と国や世界について語り合う機会があり、その問答によって同志になりえると確信した。")),
    (13, TableItem::Text("兵の中に、目立つ者がいたので実力を試してみた。試験の結果は散々だったが、一点だけ優れたところがあり、これは使えるかもしれないと考えた。")),
    (14, TableItem::Text("PCや準英傑の師匠や先生が自分にもできることはないかとやってきた。現役を退いて長い人物ではあるが、その経験は役に立つだろう。")),
    (15, TableItem::Text("PCの親戚を名乗る者がやってきた。その真偽はともかく（PCが真偽を決めても構いません）、実力はあるようだ。")),
    (16, TableItem::Text("見聞を広めるために旅行に出かけていた者が帰ってきた。外の世界で身につけた技能を使い、この国に尽くしたいという。")),
    (22, TableItem::Text("王族が前線で働きたいと申し出てきた。覚悟は本物のようで、何度言っても申し出を変えなかった。")),
    (23, TableItem::Text("準英傑の一族の者が現れた。その人物は自分も準英傑のように働きたいという。")),
    (24, TableItem::Text("暴れっぷりで街で噂になっていた人物がいたので、会いに行ってみると中々気持ちの良い人物であった。声をかけてみることにする。")),
    (25, TableItem::Text("賊のアジトを殲滅したら、その賊に囚われていた者が感謝をしながら協力を申し出てきた。賊を一人でなんとかしようとしていたらしい。")),
    (26, TableItem::Text("敵軍に雇われていた部隊の中に、PCや準英傑の親族がいた。PCや準英傑の説得によって、仲間になってくれた。")),
    (33, TableItem::Text("釣りをしていると、声をかけてきた人がいた。その人物との問答でただ者ではないと直感し、軍に誘った。")),
    (34, TableItem::Text("古い知り合いが危機のただ中にいるPCを心配して訪ねてきた。逃げ出すことを勧めているが、逆に協力を要請できないだろうか。")),
    (35, TableItem::Text("敵や賊の奇襲に遭ったが、腕の立つ傭兵に偶然にも助けられた。その人物の腕を見込んで、これからも協力してもらえないか交渉する。")),
    (36, TableItem::Text("若い騎士たちの中で、頭角を現した者がいる。会ってみると中々気が利く真面目な人物で、好意が持てた。")),
    (44, TableItem::Text("（海や湖に近いなら）漁師や船員と出会い、（山に近いなら）山師や山に住む者たちと出会い、彼の助力を受けた。")),
    (45, TableItem::Text("義憤に燃えてやってきた国民がいた。情熱は確かだが、ただの農民で本当に役に立つのかはまだわからない。")),
    (46, TableItem::Text("国の貴族が自ら志願をしてきてくれた。高貴な精神を持っており、その精神は誉れ高い。ただ、少しお坊ちゃん（お嬢様）育ちすぎる。")),
    (55, TableItem::Text("「あえて不利な方につきたい」と言う物好きな人がやってきて、協力を願い出てくれた。変わった人物だが、助かる申し出だ。")),
    (56, TableItem::Text("敵国の兵だったが、こちらの行動に胸を打たれてやって来た者がいる。こちら側に歩み寄ってくれたこの人物を説得して仲間にできないだろうか。")),
    (66, TableItem::Text("PCや準英傑に惚れているという人物がやってきて、協力を約束してくれた。その想いはまだ秘めいているようだが、いつか明かす日が来るのだろうか。")),
];
static SCOU: D66Table = D66Table::new("スカウト表", D66SortType::Asc, SCOU_ITEMS);

static BDST_ITEMS: &[&str] = &[
    "加重：決戦フェイズ中、自分は「移動」を行えない。スキルの効果によってエリアを移動することはできる。",
    "毒：ラウンド終了時に、自分は【HP】が1点減少する。また、自分が参加した判定ではスペシャルが発生しない",
    "暴走：自分キャラクターがこの変調を受けている場合、天才軍師が【イレギュラー】を公開するとき、「好きな【イレギュラー】を1枚選んで公開する」ではなく「ランダムに1枚選んで公開する」となる。敵軍がこの変調を受けている場合、【防御力】が2点減少する。",
    "激昂：自分が参加した判定でファンブルが発生した場合、通常のファンブルの処理に加え山札から【イレギュラー】を1枚引いて公開する。敵軍がこの変調を受けている場合、【攻撃力】が1点減少する。",
    "猛暑：自分は【防御力】が2点減少する。また、自分が使用するスキルはコストが1点上昇する。",
    "凍え：自分は【攻撃力】が1点減少する。また、自分は「移動」を行うたびに【戦略ポイント】が1点減少する。",
];
static BDST: Table = Table::from_dice("変調表", 1, 6, BDST_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("RELA", &RELA),
    ("PTGS", &PTGS),
    ("PTHE", &PTHE),
    ("SCOU", &SCOU),
    ("BDST", &BDST),
];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `TensaiGunshiNiNaro#roll_judge` の `/^(\d*)TN(6|10)([ABCKSTY]*)$/`。
fn judge_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d*)TN(6|10)([ABCKSTY]*)$").expect("valid regex"))
}

/// Ruby `String#to_i`（空文字列は 0）。
///
/// Ruby の `to_i` は多倍長だが、Rustでは `i64` に飽和させる。桁あふれする個数を
/// 指定した場合は Ruby でも `roll_barabara` が上限超過で落ちるため経路は変わらない。
fn to_i(digits: &str) -> i64 {
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

/// Ruby `Array#intersection` が空でないこと（`dice_list` 側に `values` の要素があるか）。
fn intersects(dice_list: &[i64], values: &[i64]) -> bool {
    dice_list.iter().any(|d| values.contains(d))
}

/// Ruby `TensaiGunshiNiNaro#roll_judge`（行為判定）。
fn roll_judge(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = judge_pattern().captures(command) else {
        return Ok(None);
    };

    // 成功となる出目
    let mut success_dices: Vec<i64> = vec![4, 5, 6, 7, 8, 9, 10];
    // スペシャルとなる出目
    let mut special_dices: Vec<i64> = vec![6, 10];
    // ファンブルとなる出目
    let fumble_dices: [i64; 1] = [1];

    // 有利
    let advantage = &m[2] == "10";

    let options = &m[3];
    // 不調 気づかぬうちの不満
    let complaints = options.contains('C');
    // 軍師スキル 〇〇サポート
    let support = options.contains('S');
    // 英傑スキル/武人 煌めく刃
    let blade = options.contains('B');
    // 英傑スキル/武人 必殺の剣
    let killer = options.contains('K');
    // 英傑スキル/武人 二刀流
    let twin = options.contains('T');
    // 英傑スキル/カリスマ 御身のためならば
    let you = options.contains('Y');
    // 英傑スキル/弓取り 愛用の弓、英傑スキル/英傑汎用 凄腕エージェント
    let agent = options.contains('A');

    // 二刀流の適用時、成功となる出目に2を追加
    if twin {
        success_dices.push(2);
    }

    // 〇〇サポート、煌めく刃、愛用の弓、御身のためならば、凄腕エージェントいずれかの適用時、
    // 成功となる出目に3を追加
    if support || blade || you || agent {
        success_dices.push(3);
    }

    // 煌めく刃、御身のためならば、愛用の弓、凄腕エージェントいずれかの適用時、
    // スペシャルとなる出目に3を追加
    if blade || you || agent {
        special_dices.push(3);
    }

    // 必殺の剣の適用時、スペシャルとなる出目に4，5を追加
    if killer {
        special_dices.push(4);
        special_dices.push(5);
    }

    // 気づかぬうちの不満適用時、成功となる出目から4を削除
    if complaints {
        success_dices.retain(|d| *d != 4);
    }

    // 英傑スキル/武人 力ずく
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる
    // （飽和した個数は `roll_barabara` が上限超過で弾く）。
    let times = to_i(&m[1]).saturating_add(2);
    let dice_size = if advantage { 10 } else { 6 };
    let dice_list = rng.roll_barabara(times, dice_size)?;

    let mut texts: Vec<String> = Vec::new();
    let mut is_critical = false;
    let mut is_fumble = false;
    let mut is_success = false;

    // Ruby `dice_list.count != dice_list.uniq.count`（同じ出目が2つ以上あるか）
    let has_duplicate = {
        let mut uniq: Vec<i64> = Vec::new();
        for d in &dice_list {
            if !uniq.contains(d) {
                uniq.push(*d);
            }
        }
        dice_list.len() != uniq.len()
    };

    // スペシャルとなる出目を含む、または、二刀流の適用時かつ同じ出目のサイコロが2つ以上出ている場合
    if intersects(&dice_list, &special_dices) || (twin && has_duplicate) {
        is_critical = true;
        texts.push(JUDGE_CRITICAL.to_owned());

        let mut special_effects: Vec<&str> = Vec::new();
        // 通常時の追加効果
        special_effects.push(NORMAL_CRITICAL);
        // 英傑スキル/武人 煌めく刃による追加効果
        if blade {
            special_effects.push(BLADE_CRITICAL);
        }
        // 英傑スキル/カリスマ 御身のためならばによる追加効果
        if you {
            special_effects.push(YOU_CRITICAL);
        }
        texts.push(format!("（{}）", special_effects.join("")));
    }

    // ファンブルとなる出目を含む場合
    if intersects(&dice_list, &fumble_dices) {
        is_fumble = true;
        texts.push(JUDGE_FUMBLE.to_owned());
        texts.push(format!("（{NORMAL_FUMBLE}）"));
    }

    if intersects(&dice_list, &success_dices) {
        is_success = true;
        texts.push(SUCCESS.to_owned());
    } else {
        texts.push(FAILURE.to_owned());
    }

    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut result = EvalResult::new();
    result.text = format!("{command} ＞ [{dice_text}] ＞ {}", texts.join(""));
    result.set_condition(is_success);
    result.critical = is_critical;
    result.fumble = is_fumble;
    Ok(Some(result))
}

/// Ruby `TensaiGunshiNiNaro#roll_damage`（ダメージ計算）。
fn roll_damage(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    // Ruby: Command::Parser.new("DM", round_type: @round_type).has_prefix_number.restrict_cmp_op_to(:>=)
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["DM"], RoundType::Floor)
            .has_prefix_number()
            .restrict_cmp_op_to(&[Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };

    // `has_prefix_number` / `restrict_cmp_op_to(:>=)` によりどちらも必ず埋まる
    let prefix_number = parsed
        .prefix_number
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let target_number = parsed.target_number.unwrap_or(crate::Int::from(0));

    // ダメージ計算
    // Ruby の Integer は多倍長なので桁あふれしない。Rustでは飽和させる。
    let damage = rng
        .roll_sum(prefix_number, 6)?
        .saturating_add(crate::randomizer::sat_i64(&parsed.modify_number));
    // HP減少量計算（Ruby `Integer#/` は床除算。除数0は ZeroDivisionError）
    let mut dec = ruby_div(crate::Int::from(damage), target_number)?;

    // HP減少量の最大値は3
    if dec > I::from(3) {
        dec = I::from(3);
    }

    let is_success = dec > I::ZERO;
    let text = if is_success {
        // i18n `TensaiGunshiNiNaro.DAMAGE.success`
        format!("{damage}ダメージ, 成功, 【HP】が{dec}点減少する")
    } else {
        // i18n `TensaiGunshiNiNaro.DAMAGE.failure`
        format!("{damage}ダメージ, 失敗, 【HP】はそのまま")
    };

    let mut result = EvalResult::new();
    result.text = format!("{command} ＞ {damage} ＞ {text}");
    result.set_condition(is_success);
    Ok(Some(result))
}

/// Ruby `BCDice::GameSystem::TensaiGunshiNiNaro`（ID: `TensaiGunshiNiNaro`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TensaiGunshiNiNaro;

impl GameSystem for TensaiGunshiNiNaro {
    fn id(&self) -> &'static str {
        "TensaiGunshiNiNaro"
    }

    fn name(&self) -> &'static str {
        "天才軍師になろう"
    }

    fn sort_key(&self) -> &'static str {
        "てんさいくんしになろう"
    }

    fn help_message(&self) -> &'static str {
        r"・行為判定
TN6…「有利」を得ていない場合、6面ダイスを2つ振って判定します。
TN10…「有利」を得ている場合、10面ダイスを2つ振って判定します。
不調 気づかぬうちの不満【C】…このセッションの間、「4」の出目を出しても判定は成功になりません。数字の後ろに【C】をつけます。
　例）TN6C
軍師スキル 〇〇サポート【S】…決戦フェイズの判定中「3」の出目を出しても判定に成功します。数字の後ろに【S】をつけます。
　例）TN6S
英傑スキル/武人 煌めく刃【B】…決戦フェイズの判定中「3」の出目を出しても判定に成功となり、スペシャルが発生します。数字の後ろに【B】をつけます。
　例）TN6B
英傑スキル/武人 力ずく…その判定のサイコロをすべて振った後、[使用者の【攻撃力】]個サイコロを振る。先頭に使用者の【攻撃力】をつけます。
　例）4TN6
英傑スキル/武人 必殺の剣【D】…《戦技》を使用している判定中「4」「5」の出目を出してもスペシャルが発生します。数字の後ろに【D】をつけます。
　例）TN6K
英傑スキル/武人 二刀流【T】…「攻撃」のスキルの判定中「2」の出目を出しても判定に成功となり、同じ出目のサイコロが2つ以上出ているとスペシャルが発生します。数字の後ろに【T】をつけます。
　例）TN6T
英傑スキル/カリスマ 御身のためならば【Y】…「交流」「スカウト」の判定中「3」の出目を出しても判定に成功となり、スペシャルが発生します。数字の後ろに【Y】をつけます。
　例）TN6Y
英傑スキル/弓取り 愛用の弓【A】…「攻撃」のスキルの判定中「3」の出目を出しても判定に成功となり、スペシャルが発生します。数字の後ろに【A】をつけます。
　例）TN6A
英傑スキル/ヤンキー&マイルドヤンキー その辺の物を武器に【C】…「4」の出目を出しても判定は成功になりません。数字の後ろに【C】をつけます。
　例）TN6C
英傑スキル/ヤンキー&マイルドヤンキー 熱血判定【C】…「4」の出目を出しても判定は成功になりません。数字の後ろに【C】をつけます。
　例）TN6C
英傑スキル/英傑汎用 凄腕エージェント【A】…活動フェイズの判定中「3」の出目を出しても判定に成功となり、スペシャルが発生します。数字の後ろに【A】をつけます。
　例）TN6A
数字の後ろに複数のコマンドを追加できます。
　例）TN10CYA
・ダメージ計算 xDM+y>=t
　[ダメージ計算]を行う。成否と【HP】の減少量を表示する。
　x: 6面ダイス数
　y: 補正値（省略可能）
　t: 防御力
・各種表
関係決定表 RELA
平時天才軍師表 PTGS
平時英傑表 PTHE
スカウト表 SCOU
変調表 BDST
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d*TN(6|10)[ABCKSTY]*",
            r"\d+DM",
            "RELA",
            "PTGS",
            "PTHE",
            "SCOU",
            "BDST",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `TensaiGunshiNiNaro#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> crate::enums::D66SortType {
        crate::enums::D66SortType::Asc
    }

    /// Ruby `TensaiGunshiNiNaro#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = roll_judge(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        if let Some(result) = roll_damage(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/TensaiGunshiNiNaro.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/TensaiGunshiNiNaro.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/TensaiGunshiNiNaro.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("TensaiGunshiNiNaro.toml must parse");
        assert_eq!(
            data.tests.len(),
            84,
            "case count in test/data/TensaiGunshiNiNaro.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "TensaiGunshiNiNaro",
                "unexpected game system in TensaiGunshiNiNaro.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(
                &GameSystemId::new("TensaiGunshiNiNaro"),
                &tc.input,
                &mut src,
            ) {
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
                    "FAIL TensaiGunshiNiNaro:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} TensaiGunshiNiNaro cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }

    /// 桁あふれするダイス個数でもオーバーフローせず `TooManyRandsError` になること。
    ///
    /// Ruby の Integer は多倍長で、`roll_barabara` が上限超過で
    /// `TooManyRandsError` を上げる。Rustでは `to_i` が `i64::MAX` に飽和したうえで
    /// 同じ経路へ落ちる。TOMLにこの経路のケースが無いのでここで固定する。
    #[test]
    fn huge_dice_count_saturates_into_too_many_rands() {
        let mut src = SeededRandomizer::new(vec![]);
        let err = eval_command(
            &GameSystemId::new("TensaiGunshiNiNaro"),
            "99999999999999999999TN6",
            &mut src,
        )
        .expect_err("too many rands");
        assert_eq!(err.to_string(), "TooManyRandsError");
    }
}
