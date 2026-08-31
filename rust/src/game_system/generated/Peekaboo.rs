//! P4で手書き移植した `lib/bcdice/game_system/Peekaboo.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Peekaboo#result_2d6`（2D6のスペシャル／ファンブル判定）
//! - `Peekaboo#eval_game_system_specific_command`
//!   （`Base#roll_tables` → `SaiFicSkillTable#roll_command`）
//!
//! 表データ（`RTT` と `TABLES`）は `lib/bcdice/game_system/Peekaboo.rb` から
//! 機械的に書き出したもので、値は1文字も変えていない。
//! Ruby側にロケール差分（`ko_kr` など）は無い。

use crate::dice_table::sai_fic_skill_table::{DEFAULT_RCT_FORMAT, DEFAULT_SKILL_FORMAT};
use crate::dice_table::{RollableTable, SaiFicCategory, SaiFicFormats, SaiFicSkillTable, Table};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

/// Ruby `RTT` の分野「不良」の特技（2D6）。
static RTT_SKILLS1: &[&str] = &[
    "夜更かし",
    "いねむり",
    "無視",
    "ウソつき",
    "悪口",
    "いたずら",
    "ズル",
    "隠れる",
    "ぬすむ",
    "おどす",
    "けんか",
];
/// Ruby `RTT` の分野「運動」の特技（2D6）。
static RTT_SKILLS2: &[&str] = &[
    "泳ぐ",
    "木登り",
    "柔らかい",
    "マラソン",
    "とぶ",
    "かけっこ",
    "バランス",
    "投げる",
    "球技",
    "打ち返す",
    "力持ち",
];
/// Ruby `RTT` の分野「友達」の特技（2D6）。
static RTT_SKILLS3: &[&str] = &[
    "ネット",
    "うわさ話",
    "優しさ",
    "がまん",
    "お手紙",
    "おしゃべり",
    "自転車",
    "勇気",
    "約束",
    "仕切る",
    "秘密基地",
];
/// Ruby `RTT` の分野「遊び」の特技（2D6）。
static RTT_SKILLS4: &[&str] = &[
    "パソコン",
    "ゲーム",
    "集める",
    "絵",
    "音楽",
    "空想",
    "読書",
    "お話づくり",
    "クイズ",
    "手品",
    "占い",
];
/// Ruby `RTT` の分野「勉強」の特技（2D6）。
static RTT_SKILLS5: &[&str] = &[
    "実験",
    "宇宙",
    "生き物",
    "工作",
    "計算",
    "宿題",
    "漢字",
    "作文",
    "歴史",
    "地理",
    "外国語",
];
/// Ruby `RTT` の分野「大人」の特技（2D6）。
static RTT_SKILLS6: &[&str] = &[
    "法律",
    "しかる",
    "手当て",
    "マナー",
    "推理",
    "計画性",
    "お料理",
    "お買い物",
    "オシャレ",
    "恋愛",
    "道楽",
];

/// Ruby `RTT` の分野リスト（1D6の出目順）。
static RTT_CATEGORIES: &[SaiFicCategory] = &[
    SaiFicCategory::new("不良", RTT_SKILLS1),
    SaiFicCategory::new("運動", RTT_SKILLS2),
    SaiFicCategory::new("友達", RTT_SKILLS3),
    SaiFicCategory::new("遊び", RTT_SKILLS4),
    SaiFicCategory::new("勉強", RTT_SKILLS5),
    SaiFicCategory::new("大人", RTT_SKILLS6),
];

/// Ruby `TABLES["SET"]`（学校イベント表 / 2D6）の項目。
static SET_ITEMS: &[&str] = &[
    "持ち物検査が行われる！イノセント全員は、《隠れる/不良9》の判定を行うこと。失敗したキャラクターは、GMがアイテム1個を選んで没収することができる（セッション終了時に返してもらえる）",
    "クラスで流行っている遊びに誘われる。GMは、「遊び」の分野の中からランダムに特技一つを選ぶ。イノセント全員は、その判定を行う。失敗したキャラクターは、【眠気】が1d6点増える。",
    "とても退屈な授業が始まった。イノセント全員は、《いねむり/不良3》の判定を行う。失敗したキャラクターは、【眠気】が1d6+1点増える。",
    "明日までの宿題を出される。イノセント全員は、明日までに宿題を終わらせないといけない。宿題を終わらせるためには、各サイクルの終わりに、《宿題／勉強7》の判定を行い、それに成功しなければいけない。宿題を次の日の学校フェイズまでに終わらせることができなかった場合は、居残り勉強させられる。その日の放課後フェイズの最初のサイクルは、1回休み。",
    "今日も今日とて楽しい授業。GMは、「勉強」の分野の中からランダムに特技1つを選ぶ。イノセント全員は、その判定を行う。失敗したキャラクターは【眠気】が1d6点増える。",
    "特に変わったこともなく、おだやかな一日だった。イノセント全員は、【眠気】が1点増える。",
    "今日の体育の時間はハードだった！　GMは、「運動」の分野の中からランダムに特技1つを選ぶ。イノセント全員は、その判定を行う。失敗したキャラクターは、【眠気】が1d6点増える。",
    "自習の時間だ！GMは、「勉強」の分野の中からランダムに特技1つを選ぶ。イノセント全員は、その判定を行う。成功したキャラクターは、1回だけ自由行動ができる。",
    "抜き打ちテストだ！GMは、「勉強」の分野の中からランダムに特技1つを選ぶ。イノセント全員は、-2の修正をつけて、その特技の判定を行う。成功したキャラクターはよい点をゲット！家にかえってそれを親に見せるとおこづかいを1個もらえる。失敗したキャラクターは、親にこっぴどく怒られ、【眠気】が1d6点増えるうえに、それ以降「みんなで遊ぶ」ことが出来なくなる。",
    "体操服や水着、宿題に提出物などなど、今日は学校に持ってこないといけないものがあったはず！《計画性/大人7》で判定を行う。失敗すると、先生に怒られてしょんぼり。【眠気】が1d6点増える。",
    "それぞれに色々なことがあった。イノセントは、各自1回ずつ2d6を振り、個別学校イベント表の指示に従うこと。",
];
static SET: Table = Table::from_dice("学校イベント表", 2, 6, SET_ITEMS);

/// Ruby `TABLES["PSET"]`（個別学校イベント表 / 2D6）の項目。
static PSET_ITEMS: &[&str] = &[
    "クラスの中に気になるコが現れる。《恋愛/大人11》の判定を行う。成功すると、その子と仲良くなって経験値を1点獲得する。",
    "お腹の調子が悪くなり、トイレに行きたくなる。《がまん/友達5》か《隠れる/不良9》の判定を行うこと。失敗すると、不名誉なあだ名をつけられ、それ以降、「友達」の分野の判定に-1の修正を受ける。",
    "今日の給食には、どうしても苦手な食べ物が出てきた。《勇気/友達9》で判定を行うこと。成功すると、苦手な食べ物を克服し、気分爽快！【眠気】を1d6点回復するか、【元気】を1点回復することができる。",
    "友達から遊ぼうと誘われる。その日の放課後フェイズに、クラスメイト1d6人と「みんなで遊ぶ」ことができる。これを断る場合は、《優しさ/友達4》で判定を行うこと。失敗するとこれ以降、「みんなで遊ぶ」を行うとき、友達を誘うことが出来なくなる。",
    "今日はクラブ活動があった。次のサイクルは行動できなくなる。その代わり、好きな特技1つを選ぶ。このセッションの間、その特技の判定を行うとき+1の修正がつく。",
    "授業中、先生がとても難しい問題を出してくる。GMは、「勉強」の分野からランダムに特技を1つ選ぶ。その判定を行うこと。成功すると、経験値を1点獲得する。",
    "クラスでオバケの噂を耳にする。《うわさ話/友達3》の判定に成功すると、GMからそのセッションで出てくるオバケの外見や情報を教えてもらうことができる。",
    "校庭や体育館など、自分達の遊び場が他のグループに占拠されている。《けんか/不良12》で判定を行うこと。成功すると、それ以降「友達」の分野の判定に+1の修正を受ける。失敗すると遊び場を失ってしまう。1ダメージを受け、それ以降、「友達と遊ぶ」ことができなくなってしまう。",
    "いじめの現場に出くわす！　《勇気/友達10》の判定に成功すると、いじめっこを撃退することができる。いじめられていた子が、お礼にお菓子を1個くれる。失敗すると1点のダメージを受ける。",
    "今日は全校集会があった。《がまん/友達5》で判定を行う。失敗すると貧血で倒れ次のサイクルは行動できなくなる。",
    "図書室で面白そうな本を発見する。《読書/遊び8》で判定を行うこと。成功すると、経験値を1点獲得する。",
];
static PSET: Table = Table::from_dice("個別学校イベント表", 2, 6, PSET_ITEMS);

/// Ruby `TABLES["OET"]`（お化け屋敷イベント表 / 2D6）の項目。
static OET_ITEMS: &[&str] = &[
    "謎かけ守護者が門を護っている。未行動のキャラクターは、《クイズ/遊び10》の判定を行うことができる。判定したキャラクターは行動済みになる。失敗したキャラクターは、1点のダメージを受ける。誰かが成功すれば、イベントはクリアできる。",
    "天井から血の雨が降ってくる！　この雨に触れると火傷しちゃうみたいだ！【防御力】が0のスプーキーとイノセントは、《かけっこ/運動7》の判定を行う。失敗すると、イノセントは1ダメージ、スプーキーは1d6ダメージを受ける。成功・失敗にかかわらず、イベントはクリアできる。",
    "トンガリ族の妖精がいる。彼は、先へ行くための通行料として、アイテムを要求してくる。何か好きなアイテム1個を渡すか、未行動のキャラクターは、《お話づくり/遊び9》の判定を行うことができる。判定したキャラクターは行動済みになる。誰かが判定に成功するか、アイテムを渡すかしたら、イベントはクリアできる。",
    "行き止まりだ。先に進む方法が分からない。未行動のキャラクターは、《推理/大人6》の判定を行うことができる。判定したキャラクターは行動済みになる。誰かが成功したら、イベントはクリアできる。",
    "まっくらで、何も見えない部屋だ。一行は不安におちいる。未行動のキャラクターは、《仕切る/友達11》の判定を行うことができる。判定したキャラクターは行動済みになる。誰かが成功すれば、イベントはクリアできる。",
    "ジメジメした通路だ。特に何もしなくても、イベントはクリアだ。誰かがのぞむなら、自由行動を1回行うことができるぞ。",
    "通路が途中で途切れて崖のようになっている。誰かが飛ぶことが出来れば、向こう岸にあるはしごを断崖にかけられそうだけど……。未行動のキャラクターは、《とぶ/運動6》の判定を行うことができる。判定したキャラクターは行動済みになる。誰かが成功すれば、イベントはクリアできる。",
    "まぼろしの部屋だ。各キャラクターの大好物のまぼろしがつぎつぎ現れる。未行動のキャラクターは、《がまん/友達5》の判定を行うことができる。判定したキャラクターは行動済みになる。誰かが成功したら、イベントはクリアできる。",
    "通路がいくつにも分岐している……。未行動のキャラクターのうち1人が《地理/勉強11》、もしくは《絵/遊び5》の判定を行う。判定したキャラクターは行動済みになる。失敗すると、そのオバケ屋敷の部屋数が1d6点上昇する。成功失敗にかかわらず、イベントはクリアできる。",
    "シャドウが見回りをしている。未行動のキャラクターのうち1人が、《隠れる/不良9》の判定を行う。成功すれば、イベントはクリアできる。失敗すると、プレイヤーと同じ人数のシャドウと戦闘を行うこと。勝利すればイベントはクリアできる。",
    "足下からシャドウが現れ、みんなに襲いかかる！プレイヤーと同じ人数のシャドウと戦闘を行うこと。勝利すればイベントはクリアできる。",
];
static OET: Table = Table::from_dice("お化け屋敷イベント表", 2, 6, OET_ITEMS);

/// Ruby `TABLES["NET"]`（日中ブラブラ表 / 2D6）の項目。
static NET_ITEMS: &[&str] = &[
    "相棒のスプーキーと喧嘩する。1d6サイクルの間、自分の相棒のスプーキーは、自分に対して【お助け力】を使用することができなくなる。",
    "ついついネットやゲームで時間をつぶす。特に何もなし。。",
    "街をブラブラしていたら、突然シッポ族のオバケに襲われる。「お前、大人のくせに俺が見えるのか？」GMは「運動」の分野の中から、ランダムに特技を1つ選ぶ。この表の使用者は、その判定を行う。成功したら、オバケに気に入られ、セッション中、好きな時に一度だけ、シッポ族の魔法をかけてもらうことができるようになる。失敗すると1点のダメージを受ける。",
    "…暇だ。たまには、タメになりそうな本でも読んでみるか。GMは「勉強」の分野の中から、ランダムに特技1つを選ぶ。この表の使用者はその判定を行う。成功したら、セッション中、好きな時に一度だけ、判定を自動的に成功することができるようになる。失敗すると退屈のあまり【眠気】が1ｄ6点上昇する。",
    "ああ、まずい。会いたくないヤツに会っちまったなぁ。どうやって誤魔化そう？GMは「不良」の分野の中から、ランダムに特技1つを選ぶ。この表の使用者は、その判定を行う。成功したら、うまく誤魔化してそのシナリオに登場するハグレオバケの噂を手に入れることができる。失敗するとお説教をくらって【眠気】が1ｄ6点上昇する。",
    "ふわわわわ。眠いなぁ。……寝るか。【眠気】を2ｄ6点回復する。",
    "うーん。腹減った。何か食べたいけど……。GMは「大人」の分野の中から、ランダムに特技を1つ選ぶ。この表の使用者はその判定を行う。成功したら、【お菓子】を1ｄ6個手に入れる。失敗すると、持っていれば【おこづかい】を一個減らす。",
    "うーん。そうだ。あいつに連絡してみるか……。GMは「友達」分野の中からランダムに特技1つを選ぶ。この表の使用者は、その判定を行う。成功したら友達の力を借りて、それ以降の自分の手番に二回行動することができるようになる。失敗すると、誰にも捕まらず、寂しさのあまり【眠気】が1ｄ6点上昇する。",
    "臨時のバイト。久々の労働だ。GMは「遊び」の分野の中から、ランダムに特技を1つ選ぶ。この表の使用者は、その判定を行う。成功したら、【おこづかい】を1つ獲得できる。失敗すると【眠気】が1ｄ6点上昇する。",
    "あ、これ欲しかったんだよな。つい無駄な買い物をしてしまう。持っていれば【おこづかい】を一個減らす。",
    "相棒のスプーキーと、素敵な時間を過ごす。そのセッションの間だけ、「武装契約」「守護契約」「強化契約」「同調契約」のいずれか一つを結ぶことができる。",
];
static NET: Table = Table::from_dice("日中ブラブラ表", 2, 6, NET_ITEMS);

/// Ruby `TABLES["IBT"]`（イノセント用バタンキュー！表 / 1D6）の項目。
static IBT_ITEMS: &[&str] = &[
    "悲しい別れ。病院につれていくことができれば、1d6日入院したあとに目覚めます。その間は、行動不能です。目覚めたときに【眠気】も【元気】もすべて回復しますが、スプーキーを見ることができなくなっています。そのキャラクターはスプーキーと一緒に冒険を続けることはできません……。",
    "大けがをして昏睡状態。病院につれていくことができれば、1d6日入院したあとに目覚めます。その間は、行動不能です。目覚めたときに【眠気】はすべて回復し、【元気】が3点回復します。",
    "気絶しちゃった！　1d6サイクル後に目覚めます。気絶している間は、行動不能です。目覚めたときに【眠気】が1d6点、【元気】が1点回復します。",
    "体が動かない！　何かを見たり、話したりといった簡単な行動ならできますが、自由行動や戦闘行動といった通常の行動は行えません。1d6サイクル後に【元気】が1点回復し、通常通り行動できるようになります。",
    "かろうじて意識はあるものの、朦朧としてきた。【眠気】が2d6点増えます。それで行動不能になっていなければ、【元気】が1点回復します。そうでなければ、気絶してしまい、1d6サイクル後に目覚めます。気絶している間は、行動不能です。目覚めたときに【眠気】が1d6点減少し、【元気】が1点回復します。",
    "なんという幸運！アイテムがキミを護ってくれた。もし持ち物にアイテムがあった場合、それが1個破壊され、受けたダメージを無効化します。アイテムがなければ行動不能になります。",
];
static IBT: Table = Table::from_dice("イノセント用バタンキュー！表", 1, 6, IBT_ITEMS);

/// Ruby `TABLES["SBT"]`（スプーキー用バタンキュー！表 / 1D6）の項目。
static SBT_ITEMS: &[&str] = &[
    "封印状態！　オバケは封印されてしまいます。1d6*1年後になれば、そのオバケは復活します。それまでは、イノセントと一緒に冒険することはできません。できたとしても、そのときイノセントはあなたを見ることができなくなっているかもしれませんが……。",
    "休眠状態！　オバケは休眠状態になります。1d6日が経過すると目覚めます。その間は、行動不能です。目覚めたときに【魔力】はすべて回復しています。",
    "コバケ状態！　体は小さく縮んてしまい、重戦闘も戦闘行動も行うことはできません。【魔力】が1点以上になると、通常通り行動できるようになります。",
    "混沌変化！　自分のリングのからだリストを使って、ランダムにからだを1つ選びます。自分のからだが、それに変化します。1d6サイクルの間、行動不能になります。その後、【魔力】が1d6点回復して通常通り行動できるようになります。",
    "魔力変質！　自分のリングの衣装リストを使って、ランダムに衣装を1つ選びます。自分の衣装1つが、それに変化します。そして、1d6サイクルの間、行動不能になります。その後、【魔力】が1d6点回復して通常通り行動できるようになります。",
    "魔法暴発！　自分の持っている魔法をランダムに1つ選んで、その効果が発動します。魔法の対象が選べる場合は、スプーキーのプレイヤーが選んで構いません。そして、1d6サイクルの間、行動不能になります。その後、【魔力】が1d6点回復して通常通り行動できるようになります。",
];
static SBT: Table = Table::from_dice("スプーキー用バタンキュー！表", 1, 6, SBT_ITEMS);

/// Ruby `TABLES["STT"]`（オバケぶらり旅表 / 2D6）の項目。
static STT_ITEMS: &[&str] = &[
    "オバケ狩りに遭遇してしまいますオバケ判定を行ってください。失敗すると2d6点のダメージを受けます。",
    "リングの上司からレポートを提出しろといわれます。セッション終了に、今回の冒険がどんな話だったかをまとめ、GMに報告してください。GMが、その報告がうまくまとまっていると感じたら、そのスプーキーとイノセントのコンビがもらえる経験点が1点上昇します。",
    "近所のお家に忍び込み、めぼしいものを探しました。オバケ判定を行ってください。成功すると、ルールブック228頁に書いてあるアイテム表を使ってランダムで決めたアイテム1個を入手します。",
    "街の優しいおばけたちに頼まれます。そのシナリオに登場するはぐれオバケを説得し、悪事をやめさせることができれば、そのスプーキーとイノセントのコンビがもらえる経験点が1点上昇します。",
    "オバケたちとギャンブルに興じます♪好きなだけ【魔力】を減少してください。サイコロを1個振ります。奇数が出たらギャンブルに勝利！減少した【魔力】の2倍の値だけ魔力が回復します。偶数が出たらギャンブルに敗北(しょぼん)。減少した魔力はかえってきません。",
    "オバケたちのウワサを耳にします。GMは、シナリオ上で重要な情報を1つ選んで下さい。その情報を入手することができます。",
    "人間について学習します。オバケ判定を行って下さい。成功すると、ランダムに好きな特技を1つ選びます。そのゲームの間、その特技を習得しているものとして、行為判定を行うことができるようになります。",
    "リングのオバケ学校で再修行。自分の魔法1つを未取得にします。そして、共用か自分のリングの魔法リストから、ランダムに魔法を1つ選び、新たに修得します。",
    "突如、誰かの役に立ちたいと思いました。このセッションが終わるまでに、そのスプーキーとイノセントのコンビに対して、5人以上のキャラクターが「ありがとう」と言ったら、コンビのもらえる経験点が1点上昇します。",
    "道端で拾ったものがオバケ市場で売れました。ラッキー♪お小遣いを1つ入手できます。",
    "オバケ狩りから身を隠すために変装します。自分の衣装1つを未取得にします。そして、共用か自分のリングの衣装から、ランダムに衣装を1つ選び、新たに修得します。",
];
static STT: Table = Table::from_dice("オバケぶらり旅表", 2, 6, STT_ITEMS);

/// Ruby `TABLES["SAT"]`（オバケ行動表 / 2D6）の項目。
static SAT_ITEMS: &[&str] = &[
    "「えー。お前なんか嫌いだ!」スプーキーは、イノセントとけんかしてしまいます。そのサイクルは行動せず、【魔力】も回復しません。",
    "「疲れたよーん」スプーキーは、行動をパスして【魔力】を回復します。",
    "「ムムム。気になる、気になるぞー」スプーキーは、戦闘フェイズなら「ひみつを暴く」を行います。それ以外なら「オバケ占い」を行います。",
    "「やっぱ定番はこうでしょう」スプーキーは、戦闘フェイズなら「攻撃」を行います。それ以外なら「調査」を行います。",
    "「オバケ的にはどう思う？」スプーキーは、ほかのスプーキーのいうことをききます。",
    "「うん、そうするー」スプーキーは自分の契約しているイノセントのいうことをききます。",
    "「うーん。そうかなぁ？」スプーキーは、ほかのイノセントのいうことをききます。",
    "「トリック・オア・トリート!」スプーキーは、何かのアイテム(お菓子やおこづかいが適当でしょう)をもらえたら、イノセントのいうことをききます。もらったアイテムは、即座に消費されます。",
    "「ここは派手にいきたいぜ！」スプーキーは、自分の修得している魔法をランダムに1個選んで、それを使用します。対象を決める場合は、自由に選んで構いません。",
    "「ここは様子見だなぁ」スプーキーは、行動をパスして【魔力】を回復します。",
    "「こうなりゃ本気モードだ!」スプーキーは自分の契約しているイノセントのいうことをききます。また、そのサイクルの間、そのスプーキーは魔力の消費なしで、魔法を使用することができます。",
];
static SAT: Table = Table::from_dice("オバケ行動表", 2, 6, SAT_ITEMS);

/// Ruby `TABLES["NST"]`（仲間効果表 / 1D6）の項目。
static NST_ITEMS: &[&str] = &[
    "用心棒。この仲間の効果は、誰かがダメージを受けた時に使用できる。そのダメージを無効化する。",
    "パトロン。この仲間の効果は、好きな時に使用できる。【おこづかい】一つか、ランダムに選んだアイテム一つを獲得できる。",
    "助言。この仲間の効果は、誰かが判定を行うときに使用できる。そのNPCが人間の場合、その判定に使う特技が、そのNPCが修得している特技なら、自動的に成功になる。そのNPCがオバケの場合、その判定にプラス2かマイナス2の修正をつけることができる。",
    "師匠。この仲間の効果は、好きな時に使用できる。そのNPCが人間の場合、ランダムに選んだ才能を一つ獲得する。この効果は、その日の終りまで持続する。そのNPCがオバケの場合、そのオバケが修得している好きな魔法を一つ使用する。",
    "時間稼ぎ。好きな行動済みのキャラクター1人を未行動にしてくれる。",
    "乱入行動。自分が攻撃に成功した時に使用できる。そのダメージを1d6点上昇してくれる。",
];
static NST: Table = Table::from_dice("仲間効果表", 1, 6, NST_ITEMS);

/// Ruby `TABLES`（`roll_tables` が引くコマンド名 → 表）。
static TABLES: &[(&str, &Table)] = &[
    ("SET", &SET),
    ("PSET", &PSET),
    ("OET", &OET),
    ("NET", &NET),
    ("IBT", &IBT),
    ("SBT", &SBT),
    ("STT", &STT),
    ("SAT", &SAT),
    ("NST", &NST),
];

/// Ruby `RTT`（`SaiFicSkillTable.new(..., rttn:, rtt_format:, rttn_format:)`）。
///
/// `rtt` / `rct` の別名は指定されていないので、`RTT` / `RCT` のみが引ける。
/// `rct_format` と `s_format` は既定のまま。
static RTT: SaiFicSkillTable = SaiFicSkillTable::new(RTT_CATEGORIES)
    .with_commands(None, None, &["TNT", "TET", "TFT", "TPT", "TST", "TAT"])
    .with_formats(SaiFicFormats {
        rtt: "ランダム指定特技表(%<category_dice>d,%<row_dice>d) ＞ %<text>s",
        rct: DEFAULT_RCT_FORMAT,
        rttn: "指定特技(%<category_name>s)表(%<row_dice>d) ＞ %<text>s",
        skill: DEFAULT_SKILL_FORMAT,
    });

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

/// Ruby `BCDice::GameSystem::Peekaboo`（ID: `Peekaboo`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peekaboo;

impl GameSystem for Peekaboo {
    fn id(&self) -> &'static str {
        "Peekaboo"
    }

    fn name(&self) -> &'static str {
        "ピーカーブー"
    }

    fn sort_key(&self) -> &'static str {
        "ひいかあふう"
    }

    fn help_message(&self) -> &'static str {
        r"・判定
　判定時にクリティカルとファンブルを自動判定します。
・各種表
　・学校イベント表　　　　　　　　SET
　・個別学校イベント表　　　　　　PSET
　・オバケ屋敷イベント表　　　　　OET
　・イノセント用バタンキュー！表　IBT
　・スプーキー用バタンキュー！表　SBT
　・日中ブラブラ表　　　　　　　　NET
　・オバケぶらり旅表　　　　　　　STT
　・仲間効果表　　　　　　　　　　NST
　・ランダム特技決定表　　　　　　RTT
　・ランダム分野決定表　　　　　　RCT
　・指定特技(不良)表　　　　　　　RTT1, TNT
　・指定特技(運動)表　　　　　　　RTT2, TET
　・指定特技(友達)表　　　　　　　RTT3, TFT
　・指定特技(遊び)表　　　　　　　RTT4, TPT
　・指定特技(勉強)表　　　　　　　RTT5, TST
　・指定特技(大人)表　　　　　　　RTT6, TAT
・D66ダイスあり
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "RTT[1-6]?",
            "RCT",
            "TNT",
            "TET",
            "TFT",
            "TPT",
            "TST",
            "TAT",
            "SET",
            "PSET",
            "OET",
            "NET",
            "IBT",
            "SBT",
            "STT",
            "SAT",
            "NST",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `Peekaboo#initialize` の `@sort_add_dice = true`。
    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `Peekaboo#initialize` の `@d66_sort_type = D66SortType::ASC`。
    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    /// Ruby `Peekaboo#initialize` の `@round_type = RoundType::CEIL`。
    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `Peekaboo#result_2d6`（ゲーム別成功度判定）。
    ///
    /// スペシャル／ファンブル以外は Ruby も `nil` を返し、`Base#result_ndx` に落ちる。
    fn result_2d6(
        &self,
        _total: crate::Int,
        dice_total: i64,
        _value_list: &[i64],
        cmp_op: CmpOp,
        _target: Target,
    ) -> Option<CheckOutcome> {
        // Ruby: return nil unless cmp_op == :>=
        if cmp_op != CmpOp::Ge {
            return None;
        }

        if dice_total <= 2 {
            Some(CheckOutcome::Result(Box::new(EvalResult::fumble(
                "ファンブル(【眠気】が1d6点上昇)",
            ))))
        } else if dice_total >= 12 {
            Some(CheckOutcome::Result(Box::new(EvalResult::critical(
                "スペシャル(【魔力】あるいは【眠気】が1d6点回復)",
            ))))
        } else {
            None
        }
    }

    /// Ruby `Peekaboo#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(text) = roll_tables(command, rng)? {
            return Ok(Some(SpecificCommandOutput::text(text)));
        }
        Ok(RTT
            .roll_command(rng, command)?
            .map(SpecificCommandOutput::text))
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
            .join("test/data/Peekaboo.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/Peekaboo.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/Peekaboo.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("Peekaboo.toml must parse");
        assert_eq!(
            data.tests.len(),
            80,
            "case count in test/data/Peekaboo.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "Peekaboo",
                "unexpected game system in Peekaboo.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("Peekaboo"), &tc.input, &mut src) {
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
                    "FAIL Peekaboo:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} Peekaboo cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
