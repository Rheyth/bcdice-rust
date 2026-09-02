//! P4で手書き移植した `lib/bcdice/game_system/BloodMoon.rb` と、
//! `lib/bcdice/game_system/BloodCrusade.rb` のうち本システムが使う部分。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `BloodMoon#result_2d6`（2以下でファンブル、12以上でスペシャル、目標値 `?` は `Result.nothing`）
//! - `#eval_game_system_specific_command` → `roll_tables` と `BloodCrusade::RTT.roll_command`
//! - `TABLES`（`BloodCrusade::TABLES_WITH_BLOOD_MOON` を `merge` した後の形）
//!
//! # 親クラスではない依存の置き場所
//!
//! `BloodMoon` は `Base` 直継承だが、`RTT`（ランダム全特技表）と
//! `TABLES_WITH_BLOOD_MOON`（`IST` / `BRT`）は `BloodCrusade` のクラス定数を参照している。
//! Rust側の `BloodCrusade.rs` はまだ生成スタブのままで別バッチの担当なので、
//! ここでは必要な部分をこのファイルに持つ。`BloodCrusade` 本体が移植されたら、
//! そちらへ寄せて重複を畳める。
//!
//! # 表データ
//!
//! `TABLE_` 接頭辞の `static` 群は上記2つの `.rb` から機械的に書き出したもので、
//! 値は1文字も変えていない。

use crate::dice_table::{
    D66Table, RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table, TableItem,
};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{table_helpers, GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::sat_i64;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};
use crate::Int as I;

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `BloodMoon#result_2d6`。
fn check_result_2d6(
    dice_total: crate::Int,
    total: i64,
    cmp_op: CmpOp,
    target: Target,
) -> Option<CheckOutcome> {
    // Ruby: return nil unless cmp_op == :>=
    if cmp_op != CmpOp::Ge {
        return None;
    }

    if dice_total <= I::from(2) {
        Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
            "ファンブル(【余裕】が 0 に)",
        ))))
    } else if dice_total >= I::from(12) {
        Some(CheckOutcome::Result(Box::new(EvalResult::critical(
            "スペシャル(【余裕】+3）",
        ))))
    } else {
        match target {
            // Ruby: elsif target == '?' -> Result.nothing
            Target::Question => Some(CheckOutcome::Nothing),
            Target::Number(target) if total >= crate::randomizer::sat_i64(&target) => {
                Some(CheckOutcome::Result(Box::new(EvalResult::success("成功"))))
            }
            Target::Number(_) => Some(CheckOutcome::Result(Box::new(EvalResult::failure("失敗")))),
        }
    }
}

/// Ruby `BloodMoon#eval_game_system_specific_command`。
///
/// `table_helpers::roll_table(command, TABLES, TABLES) || BloodCrusade::RTT.roll_command(randomizer, command)`。
fn eval_specific_command(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = table_helpers::roll_table(command, TABLES, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(RTT
        .roll_command(rng, command)?
        .map(SpecificCommandOutput::text))
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `BloodMoon::TABLES["ST"]`（シーン表）。
static TABLE_ST: Table = Table::from_dice(
    "シーン表",
    2,
    6,
    &[
        "どこまでも広がる荒野。風が吹き抜けていく。",
        "血まみれの惨劇の跡。いったい誰がこんなことを？",
        "都市の地下。かぼそい明かりがコンクリートを照らす。",
        "豪華な調度が揃えられた室内。くつろぎの空間を演出。",
        "普通の道端。様々な人が道を行き交う。",
        "明るく浮かぶ月の下。暴力の気配が満ちていく。",
        "打ち捨てられた廃墟。荒れ果てた景色に心も荒む。",
        "生活の様子が色濃く残る部屋の中。誰の部屋だろう？",
        "にぎやかな飲食店。騒ぐ人々に紛れつつ事態は進行する。",
        "ざわめく木立。踊る影。",
        "高い塔の上。都市を一望できる。",
    ],
);

/// Ruby `BloodMoon::TABLES["MIT"]`（軽度狂気表）。
static TABLE_MIT: Table = Table::from_dice(
    "軽度狂気表",
    1,
    6,
    &[
        "【誇大妄想】（判定に失敗するたびに【テンション】が１増加する。）",
        "【記憶喪失】（【幸福】の修復判定にマイナス２の修正。）",
        "【こだわり】（戦闘中の行動を「パス」以外で一つ選択し、その行動をすると【テンション】が６増加する。）",
        "【お守り中毒】（「幸運のお守り」を装備していない場合、全ての2d6判定にマイナス１の修正。）",
        "【不死幻想】（自分が受けるダメージが全て１増加する。）",
        "【血の飢え】（戦闘中、ラウンドごとに他のキャラクターにダメージを与えないと、ラウンド終了時に【テンション】４増加。）",
    ],
);

/// Ruby `BloodMoon::TABLES["SIT"]`（重度狂気表）。
static TABLE_SIT: Table = Table::from_dice(
    "重度狂気表",
    1,
    6,
    &[
        "【幸福依存】（【幸福】を一つ選択し、その【幸福】が結果フェイズに失われたとき、死亡する。）",
        "【見えない友達】（交流判定にマイナス３の修正がつく。）",
        "【臆病】（自分の行う妨害判定にマイナス２の修正がつく。）",
        "【陰謀論】（休息判定にマイナス３の修正がつく。）",
        "【指令受信】（メインフェイズの３サイクル目の自分のシーンでは、可能な範囲でGMが行動を決定する。）",
        "【猜疑心】（自分が「連携攻撃」を行うとき、関係の【深度】をダメージに加えられない。）",
    ],
);

/// Ruby `BloodMoon::TABLES["CHT"]`（自信幸福表）。
static TABLE_CHT: Table = Table::from_dice(
    "自信幸福表",
    1,
    6,
    &[
        "【戦闘能力】あなたはハンターとしての自分の戦闘能力に自信を持っています。たとえ負けようとも、それは運か相手か仲間が悪かったので、あなたの戦闘能力が低いわけではありません。",
        "【美貌】あなたは自分が美しいことを知っています。他人もあなたを美しいと思っているはず。鏡を見るたびに、あなたは自分の美しさに惚れ惚れしてしまいます。",
        "【血筋】あなたは名家の血を引く者です。祖先の栄光を背負い、家門の名誉を更に増すために、偉業をなす運命にあります。または、普通にいい家族に恵まれているのかもしれません。",
        "【趣味の技量】あなたは趣味の分野では第一人者です。必ずしも名前が知れ渡っているわけではありませんが、どんな相手にも負けない自信があります。どんな趣味かは自由です。",
        "【仕事の技量】職場で最も有能なもの、それがあなたです。誰もあなたの仕事の量とクオリティを超えられません。どんな仕事をしているかは自由に決めて構いません。",
        "【長生き】あなたはハンターとしてかなりの年月を過ごしてきたが、まだ死んでいません。これは誇るべきことです。そこらの若造には、まだまだ負けていません。",
    ],
);

/// Ruby `BloodMoon::TABLES["SHT"]`（地位幸福表）。
static TABLE_SHT: Table = Table::from_dice(
    "地位幸福表",
    1,
    6,
    &[
        "【役職】あなたは職場、あるいはハンターの組織のなかで高い階級についています。そのため、下にいるものには命令でき、相応の敬意を払われます。",
        "【英雄】あなたはかつて偉業を成し遂げたことがあり、誰でもそれを知っています。少々くすぐったい気もしますが、英雄として扱われるのは悪くありません。",
        "【お金持ち】あなたには財産があります。それも生半可な財産ではなく、人が敬意を払うだけの財産です。あなたはお金に困ることはなく、その幸せを知っています",
        "【特権階級】あなたは国が定める特権階級の一員です。王族や貴族をイメージするとわかりやすいでしょう。あなたは、どこに行っても、それ相応の扱いを受けることになります。",
        "【人格者】誰もが認める人格者としての評判を持っているため、あなたのところには悩みを抱えた人々が引きも切らずに押しかけてきます。大変ですが、ちょっと楽しい",
        "【リーダー】あなたは所属している何らかの組織を率いる立場にあります。会社の社長や、部活動の部長などです。あなたは求められてその地位にあります",
    ],
);

/// Ruby `BloodMoon::TABLES["DHT"]`（日常幸福表）。
static TABLE_DHT: Table = Table::from_dice(
    "日常幸福表",
    1,
    6,
    &[
        "【家】あなたの家はとても快適な空間です。コストと時間をかけて作り上げられた、あなたが居住するための空間。それはあなたの幸せの源なのです。",
        "【職場】あなたは仕事が楽しくて仕方ありません。意義ある仕事で払いも悪くなく、チームの仲間はみんないい奴ばかりです。残業は……ちょっとあるかもしれません。",
        "【行きつけの店】あなたには休みの日や職場帰りに立ち寄る行きつけの店があり、そこにいる時間は安らぎを感じることができます。店員とも顔見知りです。",
        "【ベッド】あなたは動物を飼っています。よく懐いた可愛い、またはかっこいい動物です。一緒に過ごす時間はあなたに幸せを感じさせてくれます",
        "【親しい隣人】おとなりさんやお向かいさん。よくお土産を渡したり、小さな子供を預かったりするような仲です。風邪を引いたときには、家事を手伝ってくれることも。",
        "【思い出】あなたは昔の思い出を心の支えにしています。何らかの幸せな記憶……それがあれば、この先にどんなつらいことが待っていても大丈夫でしょう。",
    ],
);

/// Ruby `BloodMoon::TABLES["LHT"]`（人脈幸福表）。
static TABLE_LHT: Table = Table::from_dice(
    "人脈幸福表",
    1,
    6,
    &[
        "【理解ある家族】あなたの家族は、あなたがハンターであることを知ったうえで協力してくれます。これがどれほど稀なことかは、仲間に聞けば分かるでしょう。",
        "【有能な友人】あなたの友人は、吸血鬼の存在とあなたの本当の仕事を知っています。そして、直接戦うだけの技量はないものの、あなたの探索をサポートしてくれます。",
        "【愛する恋人】あなたには愛する人がいます。見つめあうだけで、あなたの心は舞い上がり……帰ってきません。この恋人を失うなんて、考えるだけでも恐ろしいことです。",
        "【同志の権力者】あなたには吸血鬼の存在を知りながら、奴らに屈していない権力者との繋がりがあります。様々な違法行為をはたらく際に、役に立つでしょう。",
        "【得がたい師匠】あなたは使う武器を学んだ師匠がいて、それを通して兄弟弟子とも繋がりがあります。過酷な訓練を経て、彼らとあなたには強い絆ができています。",
        "【可愛い子供】あなたには子供がいます。聡明で魅力的、しかも健康な……将来を嘱望される子供です。子供が掴む幸せな未来を思う時、あなたの顔には笑みが広がります。",
    ],
);

/// Ruby `BloodMoon::TABLES["EHT"]`（退路幸福表）。
static TABLE_EHT: Table = Table::from_dice(
    "退路幸福表",
    1,
    6,
    &[
        "【故郷の町】あなたは生まれ育った街を離れてハンターとして活動しています。いつの日かあの町へ帰る……その思いがあなたを戦いのなかで支えています。",
        "【待っている人】あなたがハンターをやめて、普通の暮らしに戻ることを待ちわびている人がいます。そして、あなたはその思いに応えたいと思っています。",
        "【就職先】あなたは吸血鬼狩りの報酬がなくなっても、すぐに入ることができる就職先があるので安心です。有能なのか過疎地域なのかは分かりませんが。",
        "【配偶者】あなたはハンターをやめたあとに家庭に入ろうと考えています。暮らしの設計はすでに済み、あとは実行するだけなのですが、なかなかそうはいきません。",
        "【大志】あなたがハンターとして活動しているのは、やむにやまれぬ事情があるからです。あなたには「本当にやりたかったこと」があり、いつかその夢をかなえる気でいます。",
        "【空想の王国】あなたには辛いことがあると白昼夢にふける、あるいは物語に没入する癖があり、そのときには非常に幸せな気分になることができます。",
    ],
);

/// Ruby `BloodMoon::TABLES["IDT"]`（導入タイプ決定表(ノーマル)）。
static TABLE_IDT: Table = Table::from_dice(
    "導入タイプ決定表(ノーマル)",
    1,
    6,
    &[
        "依頼\n《概要》 ハンターは任意のキャラクターに他のハンターの【幸福】を守るように依頼され、その依頼を受ける。\n《目的》 他のハンターの【幸福】のうち一つを結果フェイズまで破壊されないこと。この【幸福】は、ゲームマスターが指定する。\n《報酬》　経験値2",
        "防衛\n《概要》 ハンターは今回の敵となるモンスターに【幸福】を狙われている。モンスターを倒さなければ【幸福】を守る事は出来ない。\n《目的》 自分の獲得している【幸福】のうち一つを結果フェイズで失わないこと。この【幸福】はゲームマスターが指定する。\n《報酬》 経験値2",
        "復讐\n《概要》 ハンターは今回の敵となるモンスターに負けたことがある。戦闘に敗北したのか、それとも【幸福】を壊されたのか。いずれにせよ、復讐の時だ。\n《目的》 結果フェイズまでにモンスターを無力化すること。\n《報酬》 経験値２",
        "関係\n《概要》 ハンターは、特定の人物が参加しているから、という理由で狩りに参加する。憧れているのかライバルなのか、単に仲がいいのかは自由。\n《目的》 結果フェイズの時点で他のハンターのうち一人との関係が、お互いに【深度】3以上になっていること。対象のハンターはシーンプレイヤーが決定する。\n《報酬》 経験値２",
        "挑戦\n《概要》 ハンターは今回の敵となるモンスターのことをなんらかの理由で知り、自分から戦いに赴く。\n《目的》 結果フェイズまでハンター全員が生き残り、かつ、フォロワーやモンスターに変化していないこと。\n《報酬》 経験値２",
        "救済\n《概要》 ハンターは今回の敵となるフォロワーのうち一人を救うために戦う。\n《目的》 結果フェイズまでに対象のフォロワーを「説得」で無力化する。このフォロワーはシーンプレイヤーが決定する。\n《報酬》 経験値2",
    ],
);

/// Ruby `BloodCrusade::TABLES_WITH_BLOOD_MOON["IST"]`（先制判定指定特技表）。
static TABLE_IST: Table = Table::from_dice(
    "先制判定指定特技表",
    1,
    6,
    &[
        "《自信/社会5》",
        "《地位/社会9》",
        "《日常/環境3》",
        "《人脈/環境7》",
        "《退路/環境11》",
        "《心臓/胴部7》",
    ],
);

/// Ruby `BloodCrusade::TABLES_WITH_BLOOD_MOON["BRT"]`（身体部位決定表）。
static TABLE_BRT: Table = Table::from_dice(
    "身体部位決定表",
    2,
    6,
    &[
        "《脳》",
        "《利き腕》",
        "《利き脚》",
        "《消化器》",
        "《感覚器》",
        "《攻撃したキャラクターの任意》",
        "《口》",
        "《呼吸器》",
        "《逆脚》",
        "《逆腕》",
        "《心臓》",
    ],
);

/// Ruby `BloodMoon::TABLES["ID2T"]`（導入タイプ決定表(ハード込み)）。
static TABLE_ID2T: D66Table = D66Table::new(
    "導入タイプ決定表(ハード込み)",
    D66SortType::Asc,
    &[
        (11, TableItem::Text("依頼\n《概要》 ハンターは任意のキャラクターに他のハンターの【幸福】を守るように依頼され、その依頼を受ける。\n《目的》 他のハンターの【幸福】のうち一つを結果フェイズまで破壊されないこと。この【幸福】は、ゲームマスターが指定する。\n《報酬》　経験値2")),
        (12, TableItem::Text("防衛\n《概要》 ハンターは今回の敵となるモンスターに【幸福】を狙われている。モンスターを倒さなければ【幸福】を守る事は出来ない。\n《目的》 自分の獲得している【幸福】のうち一つを結果フェイズで失わないこと。この【幸福】はゲームマスターが指定する。\n《報酬》 経験値2")),
        (13, TableItem::Text("復讐\n《概要》 ハンターは今回の敵となるモンスターに負けたことがある。戦闘に敗北したのか、それとも【幸福】を壊されたのか。いずれにせよ、復讐の時だ。\n《目的》 結果フェイズまでにモンスターを無力化すること。\n《報酬》 経験値２")),
        (14, TableItem::Text("関係\n《概要》 ハンターは、特定の人物が参加しているから、という理由で狩りに参加する。憧れているのかライバルなのか、単に仲がいいのかは自由。\n《目的》 結果フェイズの時点で他のハンターのうち一人との関係が、お互いに【深度】3以上になっていること。対象のハンターはシーンプレイヤーが決定する。\n《報酬》 経験値２")),
        (15, TableItem::Text("挑戦\n《概要》 ハンターは今回の敵となるモンスターのことをなんらかの理由で知り、自分から戦いに赴く。\n《目的》 結果フェイズまでハンター全員が生き残り、かつ、フォロワーやモンスターに変化していないこと。\n《報酬》 経験値２")),
        (16, TableItem::Text("救済\n《概要》 ハンターは今回の敵となるフォロワーのうち一人を救うために戦う。\n《目的》 結果フェイズまでに対象のフォロワーを「説得」で無力化する。このフォロワーはシーンプレイヤーが決定する。\n《報酬》 経験値2")),
        (22, TableItem::Text("復調\n 《概要》 ハンターは正気を取り戻し、【狂気】を癒すために戦う。\n《目的》 結果フェイズまでに自分の【狂気】を2減らす。\n《報酬》 経験値２")),
        (23, TableItem::Text("撃滅 \n《概要》 ハンターは狩りの対象であるモンスターを倒すために育成されていたり、モンスターに【幸福】を全て破壊された過去を持っている。\n《目的》 モンスターを自分で無力化する。\n《報酬》　経験値6")),
        (24, TableItem::Text("競争 \n《概要》 ハンターは自分で決めたライバルに勝つために狩りを行う。\n《目的》 他のプレイヤーのハンターからライバルを一人選ぶ。結果フェイズの段階で、ライバルよりも多くのモンスターとフォロワーを攻撃で倒している事。このライバルはシーンプレイヤーが選択する。\n《報酬》 経験値6")),
        (25, TableItem::Text("育成 \n《概要》 ハンターは仲間を成長させるために狩りに出る。\n《目的》 他の狩人すべてに導入タイプの目的を達成させる。\n《報酬》 達成した人数+2の経験値")),
        (26, TableItem::Text("窮乏 \n《概要》 ハンターは貧乏なので、金のために狩りをしなければならない。\n《目的》 自分が装備しているアイテムから一つを対象として選ぶ。対象は即座に破壊される。そのうえで、結果フェイズまで対象が書いてあったアイテム欄を使用しない。この対象はシーンプレイヤーが選択する。\n《報酬》 経験値6")),
        (33, TableItem::Text("泰然 \n《概要》 ハンターはクールでかっこいい自分のスタイルを守るために狩りをする。\n《目的》 結果フェイズまで【激情】を使用しない。\n《報酬》 経験値8")),
        (34, TableItem::Text("対話 \n《概要》 ハンターはモンスターと話をするために追いかけていく。\n《目的》 モンスターに対する関係【深度】が2以上になっている状態で決戦フェイズに入る。\n《報酬》 経験値8")),
        (35, TableItem::Text("完勝 \n《概要》 ハンターは今回の敵となるモンスターに勝ったことがある。今度こそ、とどめを刺すのだ。\n《目的》 部位ダメージを受けずにモンスターを無力化する。\n《報酬》 経験値4")),
        (36, TableItem::Text("依頼(ハード) \n《概要》 ハンターは任意のキャラクターに他のハンターの【幸福】を守るように依頼され、その依頼を受ける。\n《目的》 他のハンターの【幸福】を一つも結果フェイズまで破壊されないこと。対象となるハンターは、ゲームマスターが指定する。\n《報酬》 経験値4")),
        (44, TableItem::Text("防衛(ハード) \n《概要》 ハンターは今回の敵となるモンスターに自分の【幸福】を狙われている。モンスターを倒さなければ、【幸福】を守ることはできない。\n《目的》 自分の獲得している【幸福】を一つも結果フェイズで失わないこと。\n《報酬》 経験値4")),
        (45, TableItem::Text("復讐(ハード) \n《概要》 ハンターは今回の敵となるモンスターに負けたことがある。戦闘に敗北したのか、それとも、【幸福】を壊されたのか。いずれにせよ、復讐の時だ。\n《目的》 結果フェイズまでにモンスターとフォロワー全てを攻撃で倒すこと。自分の攻撃でなくてもかまわない。\n《報酬》 経験値6")),
        (46, TableItem::Text("関係(ハード) \n《概要》 ハンターは、特定の人物が参加しているから、という理由で狩りに参加する。憧れているのかライバルなのか、単に仲がいいのかは自由。\n《目的》 結果フェイズの時点で他のハンターのうち一人との関係が、お互いに【深度】５になっていること。対象のハンターはシーンプレイヤーが決定する。\n《報酬》 経験値4")),
        (55, TableItem::Text("挑戦(ハード) \n《概要》 ハンターは今回の敵となるモンスターのことをなんらかの理由で知り、自分から戦いに赴く。\n《目的》 結果フェイズまでハンター全員が一度も無力化されずに生き残り、かつ、フォロワーやモンスターに変化していないこと。\n《報酬》 経験値6")),
        (56, TableItem::Text("救済(ハード) \n《概要》 ハンターは今回の敵となるフォロワー全員を救うために戦う。\n《目的》 結果フェイズまでにフォロワー全員を「説得」で無力化する。\n《報酬》 経験値6")),
        (66, TableItem::Text("振り直し")),
    ],
);

/// Ruby `BloodMoon::TABLES["RAT"]`（関係属性表）。
static TABLE_RAT: D66Table = D66Table::new(
    "関係属性表",
    D66SortType::NoSort,
    &[
        (11, TableItem::Text("愛情")),
        (12, TableItem::Text("憧れ")),
        (13, TableItem::Text("怒り")),
        (14, TableItem::Text("悲しみ")),
        (15, TableItem::Text("感謝")),
        (16, TableItem::Text("期待")),
        (21, TableItem::Text("憧れ")),
        (22, TableItem::Text("共感")),
        (23, TableItem::Text("恐怖")),
        (24, TableItem::Text("嫌悪")),
        (25, TableItem::Text("困惑")),
        (26, TableItem::Text("罪悪感")),
        (31, TableItem::Text("怒り")),
        (32, TableItem::Text("恐怖")),
        (33, TableItem::Text("殺意")),
        (34, TableItem::Text("嫉妬")),
        (35, TableItem::Text("憎悪")),
        (36, TableItem::Text("忠義")),
        (41, TableItem::Text("悲しみ")),
        (42, TableItem::Text("嫌悪")),
        (43, TableItem::Text("嫉妬")),
        (44, TableItem::Text("不信感")),
        (45, TableItem::Text("侮蔑")),
        (46, TableItem::Text("保護欲")),
        (51, TableItem::Text("感謝")),
        (52, TableItem::Text("困惑")),
        (53, TableItem::Text("憎悪")),
        (54, TableItem::Text("侮蔑")),
        (55, TableItem::Text("満足感")),
        (56, TableItem::Text("友情")),
        (61, TableItem::Text("期待")),
        (62, TableItem::Text("罪悪感")),
        (63, TableItem::Text("忠義")),
        (64, TableItem::Text("保護欲")),
        (65, TableItem::Text("友情")),
        (66, TableItem::Text("喜び")),
    ],
);

/// Ruby `BloodCrusade::RTT` の分野1（社会）。
static RTT_SKILLS1: &[&str] = &[
    "怯える",
    "脅す",
    "考えない",
    "自信",
    "黙る",
    "伝える",
    "だます",
    "地位",
    "笑う",
    "話す",
    "怒る",
];
/// Ruby `BloodCrusade::RTT` の分野2（頭部）。
static RTT_SKILLS2: &[&str] = &[
    "聴く",
    "感覚器",
    "見る",
    "反応",
    "考える",
    "脳",
    "閃く",
    "予感",
    "叫ぶ",
    "口",
    "噛む",
];
/// Ruby `BloodCrusade::RTT` の分野3（腕部）。
static RTT_SKILLS3: &[&str] = &[
    "締める",
    "殴る",
    "斬る",
    "利き腕",
    "撃つ",
    "操作",
    "刺す",
    "逆腕",
    "振る",
    "掴む",
    "投げる",
];
/// Ruby `BloodCrusade::RTT` の分野4（胴部）。
static RTT_SKILLS4: &[&str] = &[
    "塞ぐ",
    "呼吸器",
    "止める",
    "受ける",
    "測る",
    "心臓",
    "逸らす",
    "かわす",
    "耐える",
    "消化器",
    "落ちる",
];
/// Ruby `BloodCrusade::RTT` の分野5（脚部）。
static RTT_SKILLS5: &[&str] = &[
    "走る",
    "迫る",
    "蹴る",
    "利き脚",
    "跳ぶ",
    "仕掛ける",
    "踏む",
    "逆脚",
    "這う",
    "伏せる",
    "歩く",
];
/// Ruby `BloodCrusade::RTT` の分野6（環境）。
static RTT_SKILLS6: &[&str] = &[
    "休む",
    "日常",
    "隠れる",
    "待つ",
    "現れる",
    "人脈",
    "捕らえる",
    "開ける",
    "逃げる",
    "退路",
    "休まない",
];

/// Ruby `BloodCrusade::RTT` の特技リスト（分野は1D6の出目順）。
static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("社会", RTT_SKILLS1),
    SaiFicCategory::new("頭部", RTT_SKILLS2),
    SaiFicCategory::new("腕部", RTT_SKILLS3),
    SaiFicCategory::new("胴部", RTT_SKILLS4),
    SaiFicCategory::new("脚部", RTT_SKILLS5),
    SaiFicCategory::new("環境", RTT_SKILLS6),
];

/// Ruby `BloodCrusade::RTT`
/// （`SaiFicSkillTable.new(..., rtt: 'AST', rtt_format: ...)`）。
static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES)
    .with_commands(Some("AST"), None, &[])
    .with_formats(SaiFicFormats {
        rtt: "ランダム全特技表(%<category_dice>d) ＞ %<category_name>s(%<row_dice>d) ＞ %<skill_name>s",
        rct: crate::dice_table::sai_fic_skill_table::DEFAULT_RCT_FORMAT,
        rttn: crate::dice_table::sai_fic_skill_table::DEFAULT_RTTN_FORMAT,
        skill: crate::dice_table::sai_fic_skill_table::DEFAULT_SKILL_FORMAT,
    });

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
///
/// Ruby は `BloodMoon::TABLES` に `BloodCrusade::TABLES_WITH_BLOOD_MOON` を
/// `merge` したもの。`Hash#merge` は新しいキーを末尾に足すので、この並びになる。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("ST", &TABLE_ST),
    ("MIT", &TABLE_MIT),
    ("SIT", &TABLE_SIT),
    ("CHT", &TABLE_CHT),
    ("SHT", &TABLE_SHT),
    ("DHT", &TABLE_DHT),
    ("LHT", &TABLE_LHT),
    ("EHT", &TABLE_EHT),
    ("ID2T", &TABLE_ID2T),
    ("IDT", &TABLE_IDT),
    ("RAT", &TABLE_RAT),
    ("IST", &TABLE_IST),
    ("BRT", &TABLE_BRT),
];

/// Ruby `BCDice::GameSystem::BloodMoon`（ID: `BloodMoon`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodMoon;

impl GameSystem for BloodMoon {
    fn id(&self) -> &'static str {
        "BloodMoon"
    }

    fn name(&self) -> &'static str {
        "ブラッドムーン"
    }

    fn sort_key(&self) -> &'static str {
        "ふらつとむうん"
    }

    fn help_message(&self) -> &'static str {
        r"・各種表
　・関係属性表　RAT
　・導入タイプ決定表(ノーマル)　IDT
　・導入タイプ決定表(ハード込み)　ID2T
　・シーン表           ST
　・先制判定指定特技表 IST
　・身体部位決定表　　 BRT
　・自信幸福表　　　　 CHT
　・地位幸福表　　　　 SHT
　・日常幸福表　　　　 DHT
　・人脈幸福表　　　　 LHT
　・退路幸福表　　　　 EHT
　・ランダム全特技表　 AST
　・軽度狂気表　　　　 MIT
　・重度狂気表　　　　 SIT
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTT[1-6]?",
            "RCT",
            "AST",
            "ST",
            "MIT",
            "SIT",
            "CHT",
            "SHT",
            "DHT",
            "LHT",
            "EHT",
            "ID2T",
            "IDT",
            "RAT",
            "IST",
            "BRT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `BloodMoon#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `BloodMoon#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `BloodMoon#initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `BloodMoon#result_2d6`。
    fn result_2d6(
        &self,
        total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        check_result_2d6(I::from(dice_total), sat_i64(&total), cmp_op, target)
    }

    /// Ruby `BloodMoon#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(command, rng)
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
            .join("test/data/BloodMoon.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/BloodMoon.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/BloodMoon.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("BloodMoon.toml must parse");
        assert_eq!(
            data.tests.len(),
            43,
            "case count in test/data/BloodMoon.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "BloodMoon",
                "unexpected game system in BloodMoon.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("BloodMoon"), &tc.input, &mut src) {
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
                    "FAIL BloodMoon:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} BloodMoon cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
