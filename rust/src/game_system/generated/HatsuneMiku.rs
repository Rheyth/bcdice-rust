//! P4で手書き移植した `lib/bcdice/game_system/HatsuneMiku.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `HatsuneMiku#judgeRoll`（判定 `Rx±y@z>=t`。ネイロ取得ごとの成功/失敗を列挙）
//! - `HatsuneMiku#getChangedModifyText` / `#check_success`
//! - `HatsuneMiku#eval_game_system_specific_command`（判定 → `roll_tables`）
//!
//! 表データは `i18n/HatsuneMiku/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは [`SystemTables`] に束ね、
//! `HatsuneMiku_Korean`（`ko_kr`）が同じ関数群を使い回す。

use std::sync::OnceLock;

use regex::Regex;

use crate::arithmetic;
use crate::dice_table::{D66Table, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::normalize::{self, CmpOp};
use crate::randomizer::Randomizer;
use crate::Int as I;

static JA_FT_ITEMS: &[&str] = &[
    "周囲から活気が失われる。黒以外のすべてのネイロを一つずつ減らす。",
    "仲間に迷惑をかけてしまう。自分以外のＰＣ全員の【生命力】が１点減少する。",
    "この失敗は後に祟るかもしれない……。自分の【生命力】が１Ｄ６点減少する。",
    "ココロに疲労感が満ちていく。自分がストレスを1点受ける。",
    "１Ｄ６を振ること。そのＰＣのコアが、その出目が１ならダークに、２ならホットに、３ならラブに、４ならエキセントリックに、５ならメランコリーに変化する。６だった場合、コアは変化しない。",
    "ラッキー！特に何も起こらない",
];
static JA_FT: Table = Table::from_dice("ファンブル表", 1, 6, JA_FT_ITEMS);

static JA_CWT_ITEMS: &[&str] = &[
    "絶望的な攻撃を受ける。そのキャラクターは強制退出になる。",
    "苦痛の悲鳴をあげ、無惨にも崩れ落ちる。そのキャラクターは行動不能になる。また、黒のネイロが一つ増える。",
    "オトクイの一撃で、あなたは吹き飛ばされてしまう。そのキャラクターは行動不能になる。また、分類が装備のナンバーにストレスを１点受ける。",
    "強烈な一撃を受けて気絶する。そのキャラクターは行動不能になる。",
    "意識はあるが、立ち上がることができない。そのキャラクターは行動不能になる。次のシーンにまだ【生命力】が０点だった場合、自動的に１点に回復する。",
    "奇跡的に踏みとどまり、持ちこたえる。【生命力】が１点になる。",
];
static JA_CWT: Table = Table::from_dice("致命傷表", 1, 6, JA_CWT_ITEMS);

static JA_BT_ITEMS: &[&str] = &[
    "仲間との楽しい時間。自分の【想い人】のパトスを一つ回復する。",
    "これまでの冒険を思い返す。自分の【能力値】のパトスを一つ回復する。",
    "自分のオトダマと会話する。【協力者】のパトスか、ナンバーのパトスを一つ回復する。",
    "体をゆっくり休める。自分の【生命力】を２Ｄ６点回復する。望むなら、回復を行う前に、自分の【活力】を決め直してもよい。",
    "お、ラッキー！いいもの見つけた！自分のコインを1枚増やす。",
    "ノイズストアに接続できた。各PCは、自分の【頭脳】のダイスの数と同じ個数まで、アプリを購入できる。",
];
static JA_BT: Table = Table::from_dice("休憩表", 1, 6, JA_BT_ITEMS);

static JA_TT_ITEMS: &[&str] = &[
    "悪意。ＰＣの中でもっとも【生命力】の低いもの一人を目標に選ぶ。もっとも低い【生命力】の持ち主が複数いる場合、その中から、ＧＭが自由に一人目標を選ぶ。",
    "狡猾。パラグラフ１〜５の中で、もっとも高い数値のパラグラフにいるＰＣ一人を目標に選ぶ。全員が圏外にいる場合、圏外にいるＰＣ全員を目標に選ぶ。",
    "堅実。ＰＣの中で、その脅威の「判定欄」に書かれた能力値がもっとも低いランクのキャラクター一人を目標に選ぶ。もっとも低いランクのキャラクターが複数いる場合、その中から、もっとも低いモッドのキャラクター一人を目標に選ぶ。モッドも同じ値だった場合、ＧＭが自由に一人目標を選ぶ。",
    "豪快。ＰＣの中でもっとも高いランクの【武勇】の持ち主一人を目標に選ぶ。もっとも高いランクの持ち主が複数いる場合、その中から、もっとも高いモッドの持ち主一人を目標に選ぶ。モッドも同じ値だった場合、ＧＭが自由に一人目標を選ぶ。",
    "単純。パラグラフ１〜５の中で、もっとも低い数値のパラグラフにいるＰＣ一人を目標に選ぶ。全員が圏外にいる場合、圏外にいるＰＣ全員を目標に選ぶ。",
    "乱戦。その脅威のいるパラグラフの数値と数値が１離れたパラグラフにいるＰＣ全員を目標に選ぶ。そのパラグラフにＰＣがいなかった場合、ＧＭが自由に一人目標を選ぶ。",
];
static JA_TT: Table = Table::from_dice("目標表", 1, 6, JA_TT_ITEMS);

static JA_RT_ITEMS: &[&str] = &[
    "恋心（プラス）／殺意（マイナス）",
    "同情（プラス）／侮蔑（マイナス）",
    "憧憬（プラス）／嫉妬（マイナス）",
    "信頼（プラス）／疑い（マイナス）",
    "共感（プラス）／不気味（マイナス）",
    "大切（プラス）／面倒（マイナス）",
];
static JA_RT: Table = Table::from_dice("関係表", 1, 6, JA_RT_ITEMS);

static JA_OT_ITEMS: &[&str] = &[
    "あなたのココロに大きな変化が訪れる。１Ｄ６を振ること。そのＰＣのコアが、その出目が１ならダークに、２ならホットに、３ならラブに、４ならエキセントリックに、５ならメランコリーに変化する。６だった場合、コアは変化しない。",
    "あなたは肉体的に大きなダメージを負う。１Ｄ６点のダメージを受ける。",
    "ノイズの助けを借りて問題を解決する。コインを１Ｄ６枚を支払う必要がある。コインを支払う場合、ほかのPCからコインを譲ってもらってもよい。支払いが足りなかった場合、その差額分だけ自分の【生命力】を減らす。",
    "大きな疲労感を感じる。ストレスを１点受ける。",
    "思わず時間をつかってしまう。【タイム】が１点減少する。",
    "場にイヤな気配が満ちていく。黒のネイロが一つ増える。",
];
static JA_OT: Table = Table::from_dice("障害表", 1, 6, JA_OT_ITEMS);

static JA_RQT_ITEMS: &[&str] = &[
    "そのエリアの風景が、あなたの【情景】へと書き換えられていく。「お前の始まりの物語を語れ。お前はこの地で何を思った？」",
    "あなたは、そのエリアの風景の中に懐かしいものを見つけ、自分の罪を思い出した。「何を見た？なにを悔いている？」",
    "そのエリアの風景が、あなたのコアと同じ色に染まる。あなたは、その風景の中になりたい自分の姿を見つける。「それがお前の望みか？お前は未来に何を求める？」",
    "あなたの脳裏に、人物欄に書かれた人物一人のイメージが浮かぶ。その人物は何かを囁き、あなたのココロが傷ついた。「そいつは誰だ？一体何と言ったのだ？」",
    "あなたは、そのエリアの風景の中に奇妙なものを見つけ、恐怖した。「何を見た？なぜそれを恐れる？」",
    "そのエリアにココロダンジョンの持ち主が現れる。その人物は、お前に質問してくる。「私をどう思ってる？なぜ、私を助ける？」",
    "あなたのオトダマの姿が、あなたのよく知っている人物に変わる「その人物は誰だ？そいつをどう思っている？」",
    "そのエリアに、あなたの持つナンバーが響き渡る。「これがお前のウタか？そのウタの名はなんだ？」",
    "あなたのオトダマの姿が、あなたの好きな人物の姿に変わる。「それがお前が焦がれる人物か。そいつをどうしたい？」",
    "そのエリアの風景にあなたの日常が浮かび上がる。「お前は何をしている？その暮らしをどう思っている？」",
    "あなたの目の前に、あなたの死体が横たわっている。「お前を殺すものは何だ？お前は誰に殺される？」",
];
static JA_RQT: Table = Table::from_dice("リクエスト表", 2, 6, JA_RQT_ITEMS);

static JA_CLT_ITEMS: &[&str] = &[
    "パスワードが抜き取られていた！　所持金が無くなっている！　自分のコインを3枚失う。",
    "過去に同様のオトクイと出会ったことのある人物に出会う。【技術】で判定を行う。成功すると、「特殊アプリの開発」を行うことができる(この間奏アクションには【タイム】は必要ない)。必要なコインは1枚少なくなる。",
    "近所にあるパワースポットを教えてもらう。【霊力】で判定を行う。成功すると、自分の【生命力】を【活力】の値だけ回復することができる。",
    "あなたのことを知る人物に出会う。どんな思い出話をしたのだろうか？　この質問はリクエストとして扱う。",
    "プライベートの友人からメールが届いている。【愛】で判定を行う。成功すると、好きなNPCを協力者として設定することができる。判定に失敗すると苦情のメールだった。ストレスを1点受ける。",
    "ノイズメンバーから応援のメッセージをもらう。好きなネイロを1つ獲得する(この効果で。特定のネイロを7個以上にすることはできない)。",
    "美味しい食べ物屋さんに関する情報を教えてもらう。【日常】で判定を行う。成功すると、自分のストレスを1点回復できる。",
    "オトクイに関する情報を求めているノイズメンバーに出会う。公開されている脅威1つにつき、その情報をコイン1枚で売却できる。このイベントが2度以上起きた場合、すでに売却した脅威の情報を再び売ることはできない。",
    "試作アプリの試験者を募集している。好きなアプリ1つを獲得する。ただし、このアプリを使用するときサイコロを1個振ること。1か2が出ると、そのアプリは効果を発揮しない。セッション中に、試作アプリを使用しているとセッション終了時にレポートを提出できる。【頭脳】で判定を行う。成功すると、コインを1枚獲得できる。",
    "自分に関する悪口を見つける。そこには、どんな悪口が書かれていたのだろうか。　この質問は、リクエストとして扱う。",
    "同じ種類のオトダマと契約しているオトダマ使いと意気投合。このセッションの間、自分のナンバー1つを、修得可能な別のナンバーに変更することができる。",
];
static JA_CLT: Table = Table::from_dice("クロウル表", 2, 6, JA_CLT_ITEMS);

static JA_RWT_ITEMS: &[&str] = &[
    "ノイズからオトクイ退治の報酬をもらうことができる。[倒したオトクイの本体のレベル]枚のコインを獲得する。",
    "ノイズにオトダマの情報を売ることができる。[自分の【頭脳】のダイスの数]枚のコインを獲得する。",
    "冒険を通じて因縁が芽生える。今回登場したキャラクターの中から一人を選ぶ。そのキャラクターを、自分の【想い人】にする。",
    "冒険を通じて絆が結ばれる。今回登場したNPCの中から一人を選ぶ。そのキャラクターを、自分の【協力者】にする。",
    "冒険の思い出が【ウタの欠片】になる。今回の冒険に登場した仲間、情景、出来事などなどから、キーワードを一つ選ぶ。そのキーワードを【ウタの欠片】のキーワード欄に追加する。",
    "戦いの経験が【ウタの欠片】になる。今回の冒険に登場した敵、情景、出来事などなどから、キーワードを一つ選ぶ。そのキーワードを【ウタの欠片】のキーワード欄に追加する。",
];
static JA_RWT: Table = Table::from_dice("報酬表", 1, 6, JA_RWT_ITEMS);

static JA_NMT_ITEMS: &[&str] = &[
    "絶望のウタに知覚を遮断される。背後にオトクイの気配を感じたと思ったときは遅かった。卑劣な攻撃があなたを襲う。好きな能力値で判定を行う。失敗するとあなたのキャラクターは、オトナシとなり、二度と冒険に参加できない。",
    "絶望のウタに混じり、悲痛な叫びが聞こえてくる。ココロダンジョンの持ち主だろうか。あなたは、救えなかったのだ。【日常】で判定を行う。失敗すると、自分の能力値一つを選ぶ。次回のセッションは、その能力値にストレスを受けた状態で始まる。",
    "絶望のウタに混じり、オトクイの笑いがこだまする。それは嘲りの笑いだった。オトクイや仲間たち……何より自分への怒りがわき上がる。【日常】で判定を行う。失敗すると、自分の想い人への【想い】を一つ失う。",
    "絶望のウタの中に一人取り残される。誰もあなたに気づかない。孤独に耐えながら、何とか日常へ帰還したが……そのときの恐怖がぬぐえない。【日常】で判定を行う。失敗すると、次回のセッションは、自分の【生命力】の現在値が通常の半分(端数切り上げ)の状態で始まる。",
    "ココロダンジョンから帰還したあなたを待っていたのは、代わり映えのない日常だった。あなたが任務に失敗しても、世界は変わらない。なら、もう、あんな怖い目をする必要はないんじゃないか？　【日常】で判定を行う。失敗すると、自分のナンバー一つを選ぶ。次回のセッションは、そのナンバーにストレスを受けた状態で始まる。",
    "絶望のウタの中を必死で逃げ出した。背後から仲間の声が聞こえた気がする。しかし、あなたは振り返ることができなかった。【日常】で判定を行う。失敗すると、自分に対して【想い】を持っているPC一人を選び、その自分に対する【想い】が失われる。",
];
static JA_NMT: Table = Table::from_dice("悪夢表", 1, 6, JA_NMT_ITEMS);

static JA_OIT_ITEMS: &[&str] = &[
    "それがし",
    "おいら／あたい",
    "自分の名前",
    "おれ／あたし",
    "わたくし",
    "私",
    "ぼく／うち",
    "自分",
    "俺様／あたくし",
    "余／妾",
    "ミー",
];
static JA_OIT: Table = Table::from_dice("オトダマ一人称表", 2, 6, JA_OIT_ITEMS);

static JA_OYT_ITEMS: &[&str] = &[
    "ユー",
    "（ＰＣの名前）たん／きゅん",
    "同志（ＰＣの名前）",
    "キミ",
    "（ＰＣの名前）くん／ちゃん",
    "マスター",
    "（ＰＣの名前）さん",
    "（ＰＣの名前）様",
    "あなた",
    "（ＰＣの名前）氏／女史",
    "（ＰＣの名前）殿",
];
static JA_OYT: Table = Table::from_dice("オトダマ呼び名表", 2, 6, JA_OYT_ITEMS);

static JA_ORT_ITEMS: &[&str] = &[
    "オトダマの表の性格を表すセリフ",
    "オトダマの裏の性格を表すセリフ",
    "ＰＣを応援するセリフ",
    "ＰＣをからかうセリフ",
    "趣味にまつわるセリフ",
    "攻撃を行うときのセリフ",
];
static JA_ORT: Table = Table::from_dice("リアクション表", 1, 6, JA_ORT_ITEMS);

static JA_OMT_ITEMS: &[&str] = &[
    "名門オトダマ使い。あなたは、代々オトダマを操る一族に生まれました。あなたには、幼い頃から相棒となるオトダマがいます。あなたは、そのオトダマと共に育ちました。",
    "傷ついたオトダマ。ある日、あなたは傷ついたオトダマを発見しました。意識を失い、今にも消えそうなオトダマに触れると、オトダマは意識を取り戻し、あなたを恩人と慕うようになりました。",
    "見えないお友達。あなたは孤独な幼年期を過ごしてきました。そのとき、あなたを導いてくれたのが、あなたのオトダマです。オトダマは、あなたに他人のココロのウタを聞き、人々を助ける術を教えてくれました。",
    "再生。あなたはオトクイに自分のココロのウタを食べられました。オトダマ使いに憑依したオトクイが倒されたとき、自分のココロの中から新たなオトダマが生まれました。",
    "愛するココロ。あなたには、子どもの頃から大好きだったウタがありました。ある日、そのウタを口ずさんでいるとき、突然、後ろから拍手の音が聞こえました。振り向くと、そこにオトダマがいました。",
    "動画。あなたは、動画を通じて歌を聞くのが好きでした。あるとき、聞いたことのないような素敵なウタが聞こえてきたかと思うと、画面の向こうからオトダマが飛び出してきました。",
    "喪失。ある日、あなたは悲劇に見舞われました。そのとき、あなたはとても大切にしていた何かを失いました。その失ったものを補うかのように、あなたの側にオトダマが現れました。",
    "受け継がれるウタ。あなたのオトダマは、あなたが大好きだった人の相棒だったオトダマでした。しかし、その人は悲劇に出会い、あなたの元を去りました。そのとき、あなたにオトダマを託したのです。",
    "謎のメール。ある日、友人からあなたの元に一通のメールが送られてきました。そのメールを開くと、不思議な音楽が流れ出し、オトダマが現れました。その友人とは、それ以来、連絡がつきません。",
    "封印。ある日、あなたは古いレコード屋で一曲の音盤に出会います。その音盤を再生してみると、オトダマが現れました。そして、オトダマは「封印を解いてくれたお礼に、しばらく付き合ってあげる」と言ってきました。",
    "一目惚れ。以前、あなたは様々な楽曲を発表していました。すると、その楽曲に一目惚れしたと言って、あなたの元にオトダマが押しかけてきました。以来、そのオトダマに付きまとわれる毎日です。",
];
static JA_OMT: Table = Table::from_dice("出会い表", 2, 6, JA_OMT_ITEMS);

static JA_ST_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("立ち並ぶ本棚の森")),
    (12, TableItem::Text("夕日が差し込む教室")),
    (13, TableItem::Text("鳴り止まない踏切")),
    (14, TableItem::Text("ビルから見下ろした街並み")),
    (15, TableItem::Text("二人で見た星空")),
    (16, TableItem::Text("液晶画面に映る奇妙な光景")),
    (22, TableItem::Text("ガラス窓に並ぶ雨だれ")),
    (23, TableItem::Text("植物園の温室")),
    (24, TableItem::Text("屋台が並ぶ祭りの風景")),
    (25, TableItem::Text("陽炎が立ちのぼるアスファルト")),
    (26, TableItem::Text("0時を示す時計の針")),
    (33, TableItem::Text("無機質な白い天井")),
    (34, TableItem::Text("暗闇に浮かび上がるヘッドライト")),
    (35, TableItem::Text("後ろからついてくる野良猫")),
    (36, TableItem::Text("一面の花畑")),
    (44, TableItem::Text("あなたを見つめる大勢の観衆")),
    (45, TableItem::Text("降り積もる雪")),
    (46, TableItem::Text("古めかしい洋館の応接間")),
    (55, TableItem::Text("おとぎ話に出てくるような森")),
    (56, TableItem::Text("深夜のコンビニ")),
    (66, TableItem::Text("誰もいない体育館")),
];
static JA_ST: D66Table = D66Table::new("情景表", D66SortType::Asc, JA_ST_ITEMS);

static JA_DKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("崩壊する楽園")),
    (12, TableItem::Text("空に堕ちる")),
    (13, TableItem::Text("優しい暴力")),
    (14, TableItem::Text("沈黙の掟")),
    (15, TableItem::Text("闇に溺れる")),
    (16, TableItem::Text("こぼれ落ちた命")),
    (22, TableItem::Text("行き止まりの絶望")),
    (23, TableItem::Text("漆黒の翼")),
    (24, TableItem::Text("眠れぬ夜")),
    (25, TableItem::Text("避けられぬ運命")),
    (26, TableItem::Text("斬り裂かれた景色")),
    (33, TableItem::Text("からっぽな自分")),
    (34, TableItem::Text("仮面の奥")),
    (35, TableItem::Text("月光中毒")),
    (36, TableItem::Text("昏い魔術")),
    (44, TableItem::Text("……オブザデッド")),
    (45, TableItem::Text("ココロを殺す")),
    (46, TableItem::Text("感染する破滅")),
    (55, TableItem::Text("愛の鎖")),
    (56, TableItem::Text("残酷な真実")),
    (66, TableItem::Text("デスゲーム")),
];
static JA_DKT: D66Table = D66Table::new("ダーク・キーワード表", D66SortType::Asc, JA_DKT_ITEMS);

static JA_HKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("真夜中をぶっ壊す")),
    (12, TableItem::Text("夢を打ち上げろ")),
    (13, TableItem::Text("譲れない明日")),
    (14, TableItem::Text("あふれ出す衝動")),
    (15, TableItem::Text("獣を解き放て")),
    (16, TableItem::Text("蒸発した涙")),
    (22, TableItem::Text("高らかに叫べ")),
    (23, TableItem::Text("負けられない戦い")),
    (24, TableItem::Text("握りしめた拳")),
    (25, TableItem::Text("疾走する青春")),
    (26, TableItem::Text("ココロに従え")),
    (33, TableItem::Text("がんばれ")),
    (34, TableItem::Text("そのまま進め")),
    (35, TableItem::Text("自分の旗")),
    (36, TableItem::Text("抗い壊し突き進む")),
    (44, TableItem::Text("咲き誇る情熱の花")),
    (45, TableItem::Text("暑苦しい友情")),
    (46, TableItem::Text("オレ色に染まれ")),
    (55, TableItem::Text("世界に八つ当たり")),
    (56, TableItem::Text("消せない炎")),
    (66, TableItem::Text("オーバードライブ")),
];
static JA_HKT: D66Table = D66Table::new("ホット・キーワード表", D66SortType::Asc, JA_HKT_ITEMS);

static JA_LKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("大人の恋")),
    (12, TableItem::Text("ドキドキが止まらない")),
    (13, TableItem::Text("つないだ手")),
    (14, TableItem::Text("世界を敵に回しても")),
    (15, TableItem::Text("重なる声")),
    (16, TableItem::Text("君のためなら死ねる")),
    (22, TableItem::Text("甘い口づけ")),
    (23, TableItem::Text("まぶたをとじて")),
    (24, TableItem::Text("キミとボク")),
    (25, TableItem::Text("好きとか嫌いとか")),
    (26, TableItem::Text("いつまでも")),
    (33, TableItem::Text("抱きしめたい")),
    (34, TableItem::Text("75億と1千五百万人愛してる")),
    (35, TableItem::Text("自動的な恋")),
    (36, TableItem::Text("会いたい")),
    (44, TableItem::Text("伝えたいコトバ")),
    (45, TableItem::Text("ありがとう")),
    (46, TableItem::Text("時間を止めて")),
    (55, TableItem::Text("大好き")),
    (56, TableItem::Text("素敵な贈り物")),
    (66, TableItem::Text("ビューティフルワールド")),
];
static JA_LKT: D66Table = D66Table::new("ラブ・キーワード表", D66SortType::Asc, JA_LKT_ITEMS);

static JA_EKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("シェフのきまぐれニルヴァーナ")),
    (12, TableItem::Text("おかず食べ過ぎ")),
    (13, TableItem::Text("バイバイバイアグラ")),
    (14, TableItem::Text("おふとん王国の攻防")),
    (15, TableItem::Text("ぐるぐるとクルクル")),
    (16, TableItem::Text("ゴリラの千年王国")),
    (22, TableItem::Text("くもん式フランケンシュタイナー")),
    (23, TableItem::Text("宇宙人とデート")),
    (24, TableItem::Text("まいにち寝正月")),
    (25, TableItem::Text("猫がにゃー")),
    (26, TableItem::Text("道草にがい")),
    (33, TableItem::Text("ブシドーロック！サムライパンク！")),
    (34, TableItem::Text("冷やしインド")),
    (35, TableItem::Text("生きててよかった")),
    (36, TableItem::Text("ぷるぷる")),
    (44, TableItem::Text("夜明けのツタンカーメン")),
    (45, TableItem::Text("半額の宴")),
    (46, TableItem::Text("超気持ちいいなにか")),
    (55, TableItem::Text("いあ！いあ！はすたあ！")),
    (56, TableItem::Text("小学生に貯金で負けた")),
    (66, TableItem::Text("秒速１ポロンクセマ")),
];
static JA_EKT: D66Table = D66Table::new(
    "エキセントリック・キーワード表",
    D66SortType::Asc,
    JA_EKT_ITEMS,
);

static JA_MKT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ごめんなさい")),
    (12, TableItem::Text("甘い甘い逃避")),
    (13, TableItem::Text("ひとりぼっち")),
    (14, TableItem::Text("ズルい世界")),
    (15, TableItem::Text("果たせなかった約束")),
    (16, TableItem::Text("取り返しのつかない言葉")),
    (22, TableItem::Text("いっそ死にたい")),
    (23, TableItem::Text("置いてきた夢")),
    (24, TableItem::Text("見あげた青空")),
    (25, TableItem::Text("きみの嘘")),
    (26, TableItem::Text("すれ違う言葉")),
    (33, TableItem::Text("幸せだった昨日")),
    (34, TableItem::Text("こんなはずじゃなかった")),
    (35, TableItem::Text("別れてしまった二つの道")),
    (36, TableItem::Text("また会えたらいいね")),
    (44, TableItem::Text("ここではないどこか")),
    (45, TableItem::Text("青春の終わり")),
    (46, TableItem::Text("大好きだった膝の上")),
    (55, TableItem::Text("誰かぼくをほめて")),
    (56, TableItem::Text("高潔な裏切り")),
    (66, TableItem::Text("ナルシズム")),
];
static JA_MKT: D66Table =
    D66Table::new("メランコリー・キーワード表", D66SortType::Asc, JA_MKT_ITEMS);

static JA_DNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("ダーク／濁、搦　ネロ／音呂、寝路")),
    (12, TableItem::Text("クロト／黒斗、玄徒　ヤミ／夜美、闇")),
    (13, TableItem::Text("ネクロ／根黒、寝喰　マコ／魔子、混乎")),
    (
        14,
        TableItem::Text("カゲオ／影男、陰夫　オニコ／鬼子、隠忍呼"),
    ),
    (15, TableItem::Text("アクタ／芥、悪太　ホタル／蛍、歩足")),
    (
        16,
        TableItem::Text("マオウ／魔王、万凹　ミダラ／淫、美堕裸"),
    ),
    (
        22,
        TableItem::Text("マミヤ／魔美也、狸夜　ジャミ／邪美、蛇実"),
    ),
    (23, TableItem::Text("ドクロ／髑髏、毒炉　ヨミ／黄泉、詠")),
    (24, TableItem::Text("マクラ／枕、真暗　サツキ／殺鬼、五月")),
    (25, TableItem::Text("ゲドウ／外道、戯堂　サヤ／小夜、鞘")),
    (26, TableItem::Text("ジゴク／地獄、慈極　ウマル／埋、兎丸")),
    (33, TableItem::Text("エンド／怨人、終　ヨハネ／夜羽、世刎")),
    (34, TableItem::Text("ノロイ／呪、鈍　カバネ／屍、椛音")),
    (35, TableItem::Text("アクム／悪夢、飽夢　クサリ／腐、鎖")),
    (36, TableItem::Text("バツ／罰、×　ニエ／贄、沸")),
    (
        44,
        TableItem::Text("ネガ／音我、願　リリス／璃々子、離里素"),
    ),
    (45, TableItem::Text("ウツロ／虚、洞　ネタミ／妬美、寝多実")),
    (46, TableItem::Text("ハジメ／始、創　ホロビ／滅、亡")),
    (
        55,
        TableItem::Text("ザイン／罪印、沙陰　リンボ／淋墓、辺獄"),
    ),
    (
        56,
        TableItem::Text("ハラワタ／腑、祓輪太　ユガミ／歪、由神"),
    ),
    (
        66,
        TableItem::Text("イミ／忌、逝美　ムイミ／無意味、無為巳"),
    ),
];
static JA_DNT: D66Table = D66Table::new("ダーク・名前表", D66SortType::Asc, JA_DNT_ITEMS);

static JA_HNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("レッド／烈怒、煉集　アカネ／赤音、茜")),
    (12, TableItem::Text("アツシ／熱、純志　カンナ／神奈、柑菜")),
    (13, TableItem::Text("カケル／駆、賭　ハル／晴、春")),
    (14, TableItem::Text("ガッツ／牙突、勝　アカリ／紅莉、明里")),
    (15, TableItem::Text("ケン／剣、拳　アスカ／明日香、飛鳥")),
    (16, TableItem::Text("ゴウ／豪、剛　ヒミコ／日美子、卑弥呼")),
    (22, TableItem::Text("ヒイロ／火色、陽彩　アキラ／晶、爽")),
    (23, TableItem::Text("タケル／武、猛　ヒトミ／瞳、仁美")),
    (
        24,
        TableItem::Text("グレン／紅蓮、九煉　ナツコ／夏子、懐子"),
    ),
    (25, TableItem::Text("アラシ／嵐、荒　ヒカル／光、晃")),
    (
        26,
        TableItem::Text("エンジョウ／炎上、円定　コマチ／小町、小真知"),
    ),
    (33, TableItem::Text("レツ／烈、裂　リズム／理澄、李珠夢")),
    (34, TableItem::Text("リキ／力、陸希　キョウカ／響歌、驚花")),
    (35, TableItem::Text("ホムラ／焔、吠叢　カグヤ／輝夜、赫映")),
    (36, TableItem::Text("ジョウ／情、丈　アオリ／煽、亜織")),
    (
        44,
        TableItem::Text("ロック／六句、麓　フォルテ／鳳流弖、彫照"),
    ),
    (
        45,
        TableItem::Text("ヤマト／大和、岳斗　イサミ／伊佐美、勇美"),
    ),
    (
        46,
        TableItem::Text("リュウセイ／流星、龍盛　ミライ／未来、美良依"),
    ),
    (
        55,
        TableItem::Text("イカル／怒、鵤　ヒマワリ／向日葵、火回"),
    ),
    (56, TableItem::Text("ツトム／努、勉　ハナビ／花火、羽夏妃")),
    (66, TableItem::Text("レオ／伶央、獅王　マツリ／祭、茉莉")),
];
static JA_HNT: D66Table = D66Table::new("ホット・名前表", D66SortType::Asc, JA_HNT_ITEMS);

static JA_LNT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("シアン／詩庵、思杏　アオイ／葵、蒼生")),
    (
        12,
        TableItem::Text("ソナタ／奏名太、其方　イズミ／泉、出海"),
    ),
    (13, TableItem::Text("ツナグ／繋、継　カレン／可憐、歌恋")),
    (14, TableItem::Text("ミノル／実、稔　コイ／恋、鯉")),
    (15, TableItem::Text("ユウ／優、悠　ラブ／良舞、羅步")),
    (
        16,
        TableItem::Text("レイン／玲音、霊印　アマミ／甘味、天海"),
    ),
    (22, TableItem::Text("ソウヤ／想夜、添也　フミ／文、芙美")),
    (
        23,
        TableItem::Text("イトシ／糸糸、意俊　コイシ／恋志、小石"),
    ),
    (24, TableItem::Text("エガオ／笑顔、描生　オモイ／想、念")),
    (25, TableItem::Text("マコト／誠、真実　マナ／真菜、愛")),
    (26, TableItem::Text("ユウリ／有理、悠里　ケイ／恵、佳")),
    (33, TableItem::Text("チヒロ／千尋、茅紘　ウララ／麗、占")),
    (34, TableItem::Text("トモ／友、杜望　ヒナ／雛、比奈")),
    (35, TableItem::Text("ソラ／空、宙　ツユ／露、梅雨")),
    (
        36,
        TableItem::Text("ユウダイ／雄大、優大　ノゾミ／望、希海"),
    ),
    (44, TableItem::Text("ハグ／剥、抱　キス／喜好、口吻")),
    (45, TableItem::Text("ショウタ／翔太、祥太　アイ／愛、藍")),
    (46, TableItem::Text("ジュン／純、潤　ミサオ／美沙緒、操")),
    (55, TableItem::Text("リョウ／涼、猟　イチズ／一途、意地図")),
    (
        56,
        TableItem::Text("シグレ／時雨、紫暮　アオバ／青葉、碧羽"),
    ),
    (
        66,
        TableItem::Text("ロミオ／路美雄、露澪　ロマン／浪漫、絽萬"),
    ),
];
static JA_LNT: D66Table = D66Table::new("ラブ・名前表", D66SortType::Asc, JA_LNT_ITEMS);

static JA_ENT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("ライム／来夢、雷鵡　ミドリ／緑、美登里"),
    ),
    (
        12,
        TableItem::Text("ランポ／乱歩、蘭舗　ビビリ／恐、美々裏"),
    ),
    (
        13,
        TableItem::Text("シラズ／不知、調頭　ヒスイ／翡翠、陽彗"),
    ),
    (14, TableItem::Text("ムウ／夢生、無　キノコ／茸、紀乃子")),
    (
        15,
        TableItem::Text("ネコヒコ／猫彦、寝子日子　イヌコ／犬子、夷猫"),
    ),
    (16, TableItem::Text("ダダ／駄々、蛇陀　キリコ／切子、霧湖")),
    (
        22,
        TableItem::Text("イケメン／活面、逝麺　ラムネ／来夢音、螺旨"),
    ),
    (
        23,
        TableItem::Text("キョウスケ／狂介、京助　ランマ／乱麻、爛漫"),
    ),
    (
        24,
        TableItem::Text("ネジ／螺子、寝児　アリス／有栖、亜梨子"),
    ),
    (25, TableItem::Text("マワル／回、環　タタミ／畳、多々実")),
    (26, TableItem::Text("キュウ／球、Ｑ　ズキン／頭巾、厨琴")),
    (
        33,
        TableItem::Text("サバン／沙蛮、裂卍　マニア／摩尼亜、間合"),
    ),
    (
        34,
        TableItem::Text("カエル／帰、蛙　エリマキ／襟巻、絵里真希"),
    ),
    (
        35,
        TableItem::Text("ナゾウ／謎宇、何造　カンノン／観音、疳暢"),
    ),
    (
        36,
        TableItem::Text("イッキュウ／一休、逸宮　ミロク／弥勒、診録"),
    ),
    (
        44,
        TableItem::Text("シュール／酒潤、終琉　カプリ／華降、噛布里"),
    ),
    (
        45,
        TableItem::Text("キジン／奇人、鬼神　フシギ／不思議、節黄"),
    ),
    (
        46,
        TableItem::Text("カブキ／歌舞伎、傾　メロン／芽論、女侖"),
    ),
    (
        55,
        TableItem::Text("ジョーカー／冗歌、浄化　ピエロ／秘絵呂、道化師"),
    ),
    (
        56,
        TableItem::Text("ウイロウ／外郎、初弄　マッチャ／抹茶、末耶"),
    ),
    (66, TableItem::Text("ビックリ／吃驚、！　ハテナ／果菜、？")),
];
static JA_ENT: D66Table = D66Table::new("エキセントリック・名前表", D66SortType::Asc, JA_ENT_ITEMS);

static JA_MNT_ITEMS: &[(i64, TableItem)] = &[
    (
        11,
        TableItem::Text("ヴァイス／灰主、唄守　マシロ／真白、万代"),
    ),
    (12, TableItem::Text("キズ／傷、疵　ダレカ／誰香、惰麗華")),
    (13, TableItem::Text("ユレル／揺、遊玲流　エモ／絵萌、恵面")),
    (14, TableItem::Text("オボロ／朧、憶露　ホノカ／仄、穂乃香")),
    (15, TableItem::Text("メロ／夢露、芽朗　シズ／静、志津")),
    (
        16,
        TableItem::Text("ヒイラギ／柊、氷刺木　カタミ／形見、片実"),
    ),
    (
        22,
        TableItem::Text("リネン／理然、離念　スノウ／素皇、珠瑙"),
    ),
    (23, TableItem::Text("セツナ／切、刹那　シノブ／偲、忍")),
    (24, TableItem::Text("ナミダ／涙、波太　カスカ／霞歌、幽")),
    (25, TableItem::Text("ムスビ／結、息日　カコ／過去、寡子")),
    (26, TableItem::Text("ウソ／嘘、宇曽　アイカ／哀歌、愛香")),
    (
        33,
        TableItem::Text("ペイン／閉音、病印　ツラミ／辛美、貫実"),
    ),
    (
        34,
        TableItem::Text("ヨリミチ／寄道、頼道　シラユキ／白雪、知由樹"),
    ),
    (35, TableItem::Text("ヒトリ／独、一人　オトナ／音鳴、乙菜")),
    (36, TableItem::Text("スバル／昴、透遙　ハルカ／遥、晴香")),
    (
        44,
        TableItem::Text("バイバイ／梅云、吠々　バニラ／香子蘭、芭韮"),
    ),
    (45, TableItem::Text("トオル／透、通　リツ／律、慄")),
    (46, TableItem::Text("タビ／旅、足袋　チギリ／契、千切")),
    (55, TableItem::Text("サイゴ／彩吾、最期　サクラ／桜、咲良")),
    (56, TableItem::Text("アワレ／憐、哀　ヒメイ／悲鳴、姫衣")),
    (
        66,
        TableItem::Text("ヘヴン／戸聞、天国　ガラス／硝子、枯州"),
    ),
];
static JA_MNT: D66Table = D66Table::new("メランコリー・名前表", D66SortType::Asc, JA_MNT_ITEMS);

static JA_OPA_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("さわやか")),
    (12, TableItem::Text("単純")),
    (13, TableItem::Text("目立ちたがり")),
    (14, TableItem::Text("笑い上戸")),
    (15, TableItem::Text("P大好き")),
    (16, TableItem::Text("がんばり屋")),
    (22, TableItem::Text("ひょうきん")),
    (23, TableItem::Text("ほれっぽい")),
    (24, TableItem::Text("勇敢")),
    (25, TableItem::Text("好奇心旺盛")),
    (26, TableItem::Text("優しい")),
    (33, TableItem::Text("八方美人")),
    (34, TableItem::Text("博愛")),
    (35, TableItem::Text("感情的")),
    (36, TableItem::Text("おしゃべり")),
    (44, TableItem::Text("無鉄砲")),
    (45, TableItem::Text("元気")),
    (46, TableItem::Text("楽観的")),
    (55, TableItem::Text("自信家")),
    (56, TableItem::Text("自由")),
    (66, TableItem::Text("好戦的")),
];
static JA_OPA: D66Table = D66Table::new("オトダマ性格表A", D66SortType::Asc, JA_OPA_ITEMS);

static JA_OPB_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("悲観的")),
    (12, TableItem::Text("大人しい")),
    (13, TableItem::Text("臆病")),
    (14, TableItem::Text("クール")),
    (15, TableItem::Text("のんき")),
    (16, TableItem::Text("マジメ")),
    (22, TableItem::Text("夢想家")),
    (23, TableItem::Text("常識人")),
    (24, TableItem::Text("サイコ")),
    (25, TableItem::Text("おおらか")),
    (26, TableItem::Text("平和主義者")),
    (33, TableItem::Text("慎重")),
    (34, TableItem::Text("合理主義者")),
    (35, TableItem::Text("無口")),
    (36, TableItem::Text("照れ屋")),
    (44, TableItem::Text("おひとよし")),
    (45, TableItem::Text("なまけもの")),
    (46, TableItem::Text("腰が低い")),
    (55, TableItem::Text("疑い深い")),
    (56, TableItem::Text("謙虚")),
    (66, TableItem::Text("嘘つき")),
];
static JA_OPB: D66Table = D66Table::new("オトダマ性格表B", D66SortType::Asc, JA_OPB_ITEMS);

static JA_OHT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("散歩")),
    (12, TableItem::Text("うわさ話")),
    (13, TableItem::Text("寝る")),
    (14, TableItem::Text("読書")),
    (15, TableItem::Text("アイドル")),
    (16, TableItem::Text("甘味")),
    (22, TableItem::Text("飲み会")),
    (23, TableItem::Text("温泉")),
    (24, TableItem::Text("ギャンブル")),
    (25, TableItem::Text("動物")),
    (26, TableItem::Text("アニメ")),
    (33, TableItem::Text("ガーデニング")),
    (34, TableItem::Text("漫画")),
    (35, TableItem::Text("ドラマ")),
    (36, TableItem::Text("オークション")),
    (44, TableItem::Text("パズル")),
    (45, TableItem::Text("占い")),
    (46, TableItem::Text("焼き肉")),
    (55, TableItem::Text("スポーツ観戦")),
    (56, TableItem::Text("ゲーム")),
    (66, TableItem::Text("動画配信")),
];
static JA_OHT: D66Table = D66Table::new("オトダマ趣味表", D66SortType::Asc, JA_OHT_ITEMS);

static JA_OLT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("デフォルト")),
    (12, TableItem::Text("王子様／お姫様")),
    (13, TableItem::Text("和装")),
    (14, TableItem::Text("獣系")),
    (15, TableItem::Text("ゴス")),
    (16, TableItem::Text("眼鏡")),
    (22, TableItem::Text("スポーツ")),
    (23, TableItem::Text("軍服")),
    (24, TableItem::Text("天使／悪魔の羽")),
    (25, TableItem::Text("学生服")),
    (26, TableItem::Text("メガホン")),
    (33, TableItem::Text("スポーツ系")),
    (34, TableItem::Text("パンク")),
    (35, TableItem::Text("フォーマル")),
    (36, TableItem::Text("ジャージ")),
    (44, TableItem::Text("季節イベント")),
    (45, TableItem::Text("白衣")),
    (46, TableItem::Text("童話コス")),
    (55, TableItem::Text("バニー")),
    (56, TableItem::Text("水着")),
    (66, TableItem::Text("戦隊コス")),
];
static JA_OLT: D66Table = D66Table::new("オトダマ外見表", D66SortType::Asc, JA_OLT_ITEMS);
/// Ruby `TABLES`（`translate_tables(:ja_jp)`）。
pub(crate) static JA_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("FT", &JA_FT),
    ("CWT", &JA_CWT),
    ("BT", &JA_BT),
    ("TT", &JA_TT),
    ("RT", &JA_RT),
    ("OT", &JA_OT),
    ("RQT", &JA_RQT),
    ("CLT", &JA_CLT),
    ("RWT", &JA_RWT),
    ("NMT", &JA_NMT),
    ("OIT", &JA_OIT),
    ("OYT", &JA_OYT),
    ("ORT", &JA_ORT),
    ("OMT", &JA_OMT),
    ("ST", &JA_ST),
    ("DKT", &JA_DKT),
    ("HKT", &JA_HKT),
    ("LKT", &JA_LKT),
    ("EKT", &JA_EKT),
    ("MKT", &JA_MKT),
    ("DNT", &JA_DNT),
    ("HNT", &JA_HNT),
    ("LNT", &JA_LNT),
    ("ENT", &JA_ENT),
    ("MNT", &JA_MNT),
    ("OPA", &JA_OPA),
    ("OPB", &JA_OPB),
    ("OHT", &JA_OHT),
    ("OLT", &JA_OLT),
];

/// 1ロケール分の表と定型文。
pub(crate) struct SystemTables {
    pub(crate) tables: &'static [(&'static str, &'static dyn RollableTable)],
    /// `HatsuneMiku.special`
    pub(crate) special: &'static str,
    /// `HatsuneMiku.neiro_acquire`（`%{pickup_dice}` / `%{color}` / `%{total}` / `%{result}`）
    pub(crate) neiro_acquire: &'static str,
    /// `HatsuneMiku.color_black` 〜 `color_any`（出目1〜6の順）
    pub(crate) colors: [&'static str; 6],
    /// `fumble`
    pub(crate) fumble: &'static str,
    /// `success`
    pub(crate) success: &'static str,
    /// `failure`
    pub(crate) failure: &'static str,
}

pub(crate) static JA_SYSTEM: SystemTables = SystemTables {
    tables: JA_TABLES,
    special: "スペシャル",
    neiro_acquire: "　ネイロに%{pickup_dice}(%{color})を取得した場合 %{total}:%{result}",
    colors: ["黒", "赤", "青", "緑", "白", "任意"],
    fumble: "ファンブル",
    success: "成功",
    failure: "失敗",
};

/// Ruby `HatsuneMiku#getChangedModifyText`。`++` → `+2`、`+` → `+1`。
fn get_changed_modify_text(text: &str) -> String {
    let mut modify_text = String::new();
    for value in text.split(',') {
        match value {
            "++" => modify_text.push_str("+2"),
            "+" => modify_text.push_str("+1"),
            _ => modify_text.push_str(value),
        }
    }
    modify_text
}

/// Ruby `HatsuneMiku#check_success`。
fn check_success(
    sys: &SystemTables,
    total: i64,
    dice: i64,
    cmp_op: CmpOp,
    target: i64,
    special: i64,
) -> &'static str {
    if dice == 1 {
        return sys.fumble;
    }
    if dice >= special {
        return sys.special;
    }
    if cmp_op.apply(&I::from(total), &I::from(target)) {
        sys.success
    } else {
        sys.failure
    }
}

/// Ruby `HatsuneMiku#judgeRoll`。
fn judge_roll(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^(R([A-DS]|\d+)([+\-\d,]*))(@(\d))?((>(=)?)([+\-\d]*))?(@(\d))?$")
            .expect("valid regex")
    });
    let Some(m) = re.captures(command) else {
        return Ok(None);
    };

    let skill_rank = m.get(2).map_or("", |x| x.as_str());
    let modify_text = m.get(3).map_or("", |x| x.as_str());
    let sign_of_inequality = m.get(7).map_or(">=", |x| x.as_str());
    let target_text = m.get(9).map_or("4", |x| x.as_str());

    // Ruby: specialNum = m[5] || m[11] || 6
    let special_num: i64 = m
        .get(5)
        .or_else(|| m.get(11))
        .and_then(|x| x.as_str().parse().ok())
        .unwrap_or(6);
    let special_text = if special_num == 6 {
        String::new()
    } else {
        format!("@{special_num}")
    };

    let modify_text = get_changed_modify_text(modify_text);

    let command_text = format!("R{skill_rank}{modify_text}");

    let dice_count = match skill_rank {
        "S" => 4,
        "A" => 3,
        "B" => 2,
        "C" => 1,
        "D" => 2,
        digits => match digits.parse::<i64>() {
            Ok(n) => n,
            // Ruby では nil のまま roll_barabara に渡って例外になる。Rust では判定なしとする。
            Err(_) => return Ok(None),
        },
    };

    // Ruby: ArithmeticEvaluator.eval（不正な式は 0）
    let modify = arithmetic::eval(&modify_text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let target = arithmetic::eval(target_text, RoundType::Floor)?
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(0);
    let cmp_op = normalize::comparison_operator(sign_of_inequality).unwrap_or(CmpOp::Ge);

    let mut dice_list = rng.roll_barabara(dice_count, 6)?;
    dice_list.sort_unstable();
    let dice_text = dice_list
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if skill_rank == "D" {
        if let Some(min) = dice_list.iter().min().copied() {
            dice_list = vec![min];
        }
    }

    let mut message = format!(
        "({command_text}{special_text}{sign_of_inequality}{target_text}) ＞ [{dice_text}]{modify_text} ＞ "
    );

    if dice_list.len() <= 1 {
        let Some(dice) = dice_list.first().copied() else {
            return Ok(None);
        };
        let total = dice + modify;
        let result = check_success(sys, total, dice, cmp_op, target, special_num);
        message.push_str(&format!("{total}:{result}"));
    } else {
        let mut texts: Vec<String> = Vec::new();
        for (index, pickup_dice) in dice_list.iter().copied().enumerate() {
            let mut rests = dice_list.clone();
            rests.remove(index);
            let dice = rests.iter().max().copied().unwrap_or(0);
            let total = dice + modify;
            let result = check_success(sys, total, dice, cmp_op, target, special_num);

            let color = usize::try_from(pickup_dice - 1)
                .ok()
                .and_then(|i| sys.colors.get(i))
                .copied()
                .unwrap_or("");
            let text = sys
                .neiro_acquire
                .replace("%{pickup_dice}", &pickup_dice.to_string())
                .replace("%{color}", color)
                .replace("%{total}", &total.to_string())
                .replace("%{result}", result);
            // Ruby: texts.uniq!
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
        message.push('\n');
        message.push_str(&texts.join("\n"));
    }

    Ok(Some(message))
}

/// Ruby `Base#roll_tables(command, TABLES)`。
fn roll_tables(
    tables: &'static [(&'static str, &'static dyn RollableTable)],
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    match tables.iter().find(|(key, _)| *key == command) {
        None => Ok(None),
        Some((_, table)) => Ok(Some(table.roll(rng)?.to_string())),
    }
}

/// Ruby `HatsuneMiku#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    sys: &SystemTables,
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    if let Some(text) = judge_roll(sys, command, rng)? {
        return Ok(Some(SpecificCommandOutput::text(text)));
    }
    Ok(roll_tables(sys.tables, command, rng)?.map(SpecificCommandOutput::text))
}

/// Ruby `BCDice::GameSystem::HatsuneMiku`（ID: `HatsuneMiku`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HatsuneMiku;

impl GameSystem for HatsuneMiku {
    fn id(&self) -> &'static str {
        "HatsuneMiku"
    }

    fn name(&self) -> &'static str {
        "初音ミクTRPG ココロダンジョン"
    }

    fn sort_key(&self) -> &'static str {
        "はつねみくTRPGこころたんしよん"
    }

    fn help_message(&self) -> &'static str {
        r"・判定(Rx±y@z>=t)
　能力値のダイスごとに成功・失敗の判定を行います。
　x：能力ランク(S,A～D)。数字指定で直接その個数のダイスが振れます
　y：修正値。A+2 あるいは A++ のように表記。混在時は A++,+1 のように記述も可能
　z：スペシャル最低値（省略：6）　t：目標値（省略：4）
　　例） RA　R2　RB+1　RC++　RD+,+2　RA>=5　RS-1@5>=6
　結果はネイロを取得した残りで最大値を表示
例） RB
　HatsuneMiku : (RB>=4) ＞ [3,5] ＞
　　ネイロに3(青)を取得した場合 5:成功
　　ネイロに5(白)を取得した場合 3:失敗
・各種表
　ファンブル表 FT／致命傷表 CWT／休憩表 BT／目標表 TT／関係表 RT
　障害表 OT／リクエスト表 RQT／クロウル表 CLT／報酬表 RWT／悪夢表 NMT／情景表 ST
・キーワード表
　ダーク DKT／ホット HKT／ラブ LKT／エキセントリック EKT／メランコリー MKT
・名前表 NT
　コア別　ダーク DNT／ホット HNT／ラブ LNT／エキセントリック ENT／メランコリー MNT
・オトダマ各種表
　性格表A OPA／性格表B OPB／趣味表 OHT／外見表 OLT／一人称表 OIT／呼び名表 OYT
　リアクション表 ORT／出会い表 OMT
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "R[A-DS]?", "FT", "CWT", "BT", "TT", "RT", "OT", "RQT", "CLT", "RWT", "NMT", "OIT",
            "OYT", "ORT", "OMT", "ST", "DKT", "HKT", "LKT", "EKT", "MKT", "DNT", "HNT", "LNT",
            "ENT", "MNT", "OPA", "OPB", "OHT", "OLT",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn d66_sort_type(&self) -> D66SortType {
        D66SortType::Asc
    }

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(&JA_SYSTEM, command, rng)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict(
            "HatsuneMiku",
            "HatsuneMiku.toml",
            57,
        );
    }
}
