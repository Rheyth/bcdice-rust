//! P4で手書き移植した `lib/bcdice/game_system/GranCrest.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `#result_nd6`（nD6の目標値判定でのクリティカル処理）
//! - `#eval_game_system_specific_command` → `roll_tables`（`TABLES` と `ALIASES`）
//!
//! # 表データ
//!
//! Ruby側は `GranCrest.rb` に表をハードコードしている。Rust側は同じ値を
//! `static` として持つ（値は1文字も変えていない）。

use crate::dice_table::{D66RowSplitTable, RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput, Target};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::{CheckOutcome, EvalResult};

// ---------------------------------------------------------------------------
// 表データ（`GranCrest.rb` の `TABLES` から機械的に書き出したもの）
// ---------------------------------------------------------------------------

/// Ruby `TABLES["MT"]` の項目。
static MT_ITEMS_1: &[&str] = &[
    "師匠\nあなたは彼（彼女）から多くのものを学んだ。あなたにとって、彼（彼女）は師であった。",
    "保護者\nあなたは彼（彼女）を兄や姉、あるいは父親のように慕っている。もちろん本当に血縁関係があってもよい。",
    "恩人\nあなたは彼（彼女）に恩義があり、その恩を返したいと思っている。",
    "忠誠\nあなたは彼（彼女）に忠誠を誓っている。それはあなたが望んでそうしているのか、別の事情があるのかは好きに設定すること。",
    "借り\nあなたは彼（彼女）に何らかの借りがある。その借りは必ず返さねばならないものだ。",
    "興味\nあなたは彼（彼女）に興味がある。善悪の問題ではない。どこかで、彼（彼女）を面白いと感じるのだ。",
];
/// Ruby `TABLES["MT"]` の項目。
static MT_ITEMS_2: &[&str] = &[
    "家族\nあなたは彼（彼女）は家族同然の関係だ。もちろん本当に血縁関係があってもよい。",
    "友人\nあなたと彼（彼女）は友人だ。共に時間を過ごし、その友情を育んできた。",
    "仲間\nあなたと彼（彼女）は仲間同士だ。同じ目的、同じ志を持ち、共に協力してこれまで事に当たってきた。",
    "仕事\nあなたと彼（彼女）は仕事上でのつきあいだ。仕事相手として信頼できる相手ではあるが、それ以上でもそれ以下でもない。",
    "腐れ縁\nあなたと彼（彼女）は、何かにつけてよく顔を合わせる腐れ縁だ。長いつきあいと言えるだろう。",
    "忘却\nあなたと彼（彼女）は、どこかで出会ったことがある。だが、それがいつ、どこだったかを思い出すことはできない。",
];
/// Ruby `TABLES["MT"]` の項目。
static MT_ITEMS_3: &[&str] = &[
    "慕情\nあなたは彼（彼女）を慕っている。心の底から、純粋に、それは恋愛感情と呼べるものかもしれない。",
    "貸し\nあなたは彼（彼女）に貸しがある。いつか、この貸しは必ず返してもらわねばならない。",
    "弟妹\nあなたは彼（彼女）を弟や妹のように思っている。もちろん本当に血縁関係があってもよい。",
    "秘密\nあなたと彼（彼女）は秘密を共有している仲だ。一方的に秘密を握られているのかもしれない。",
    "好敵手\nあなたと彼（彼女）は好敵手――ライバルという関係がふさわしい。いつかヤツに勝つため、あなたは研鑽を積む。",
    "仇敵\n彼（彼女）はあなたの仇敵だ。いつかヤツを殺し、雪辱を果たさなければ気が済まない。",
];
/// Ruby `TABLES["MT"]`（`D66OneThirdTable`）。
static MT: D66RowSplitTable =
    D66RowSplitTable::one_third("邂逅表", MT_ITEMS_1, MT_ITEMS_2, MT_ITEMS_3);

/// Ruby `TABLES["PFT"]` の項目。
static PFT_ITEMS_1: &[&str] = &[
    "好奇心\nあなたは彼（彼女）に好奇心を感じた。彼（彼女）はとても面白い。もっと知ってみたいと感じた。",
    "憧憬\nあなたは彼（彼女）にあこがれを感じた。彼（彼女）のようになりたいと思った。その思いは今でも続いている。",
    "尊敬\nあなたは彼（彼女）を尊敬している。彼（彼女）を敬い、決して下には見ないだろう。",
    "同志\nあなたは彼（彼女）と同じ志を持つものだと感じている。共に道を行く仲間、もしかしたら好敵手なのかもしれない。",
    "友情\nあなたは彼（彼女）に友情を感じている。彼（彼女）は得がたい友人だ。あなたを助け、そしてあなたも向こうを助けることだろう。",
    "慕情\nあなたは彼（彼女）にほのかな愛情を感じている。それは恋と呼ばれるものかもしれないし、違うかもしれない。",
];
/// Ruby `TABLES["PFT"]` の項目。
static PFT_ITEMS_2: &[&str] = &[
    "庇護\nあなたは彼（彼女）を守ってやらねばならぬ、と感じている。あなたが彼（彼女）を苦難から救うのだ。",
    "幸福感\nあなたは彼（彼女）を見ると、幸福感に包まれる。彼（彼女）がそばにいる、それがあなたにとっては幸せなのだ。",
    "信頼\nあなたは彼（彼女）を信用している。その能力を、もしくは性格を、信じ、頼っている。",
    "尽力\nあなたは彼（彼女）に力を尽くしたいと考えている。彼（彼女）の役に立つ、それがあなたのしたいことだ。",
    "可能性\nあなたは彼（彼女）に、何らかの可能性を感じている。今はまだ未熟でも、彼（彼女）はいつか花開くだろう。",
    "慈愛\nあなたは彼（彼女）に愛情を感じている。見返りなどなくとも、あなたは彼（彼女）の力となるだろう。",
];
/// Ruby `TABLES["PFT"]` の項目。
static PFT_ITEMS_3: &[&str] = &[
    "かわいい\nあなたは彼（彼女）をかわいいと思っている。いつまでも愛でることができたら、それはどんなに幸せだろうか。",
    "同情\nあなたは彼（彼女）に同情している。その過去か、それとも別のものか、あなたは彼（彼女）をかわいそうだと思っている。",
    "連帯感\nあなたは彼（彼女）に連帯感を持っている。彼（彼女）はどこかで自分と似ていると思っている。",
    "親近感\nあなたは彼（彼女）に親近感を抱いている。彼（彼女）は家族のようなものだ。そう接してもよいだろう。",
    "感服\nあなたは彼（彼女）に感服している。その能力か、それとも別の事柄か、彼（彼女）をすごいと感じ、認めている。",
    "誠意\nあなたは彼（彼女）に誠意を感じている。他がどうであろうとも、彼（彼女）はあなたにとってみれば誠実だ。",
];
/// Ruby `TABLES["PFT"]`（`D66OneThirdTable`）。
static PFT: D66RowSplitTable =
    D66RowSplitTable::one_third("ポジティブ感情表", PFT_ITEMS_1, PFT_ITEMS_2, PFT_ITEMS_3);

/// Ruby `TABLES["NFT"]` の項目。
static NFT_ITEMS_1: &[&str] = &[
    "憤憑\nあなたは彼（彼女）に憤憑を覚えている。性格か、その癖か、何かにあなたは怒りを覚えているのだ。",
    "悲哀\nあなたは彼（彼女）に悲哀を感じている。その過去か、それとも別のものか、あなたは彼（彼女）をかわいそうだと感じている。",
    "寂しさ\nあなたは彼（彼女）を見ると寂しさを覚える。もっと距離を近づけたいのか、別の理由なのか、ともあれ、どこか寂しいのだ。",
    "食傷\nあなたは彼（彼女）に食傷を覚えている。口癖や考え方、あるいは別のものにうんざりしているのだ。",
    "敵愾心\nあなたは彼（彼女）に敵愾心を覚えている。ここだけは彼（彼女）に負けたくないと思っているのだ。",
    "不快感\nあなたは彼（彼女）に不快感を覚えている。他のところはともかく、どうしてもここだけはイヤだ、という部分があるのだ。",
];
/// Ruby `TABLES["NFT"]` の項目。
static NFT_ITEMS_2: &[&str] = &[
    "猜疑心\nあなたは彼（彼女）に猜疑心を覚えている。なぜか、彼（彼女）のことを信じられない。どこかで疑ってしまうのだ。",
    "嫌悪\nあなたは彼（彼女）のことを嫌っている。考え方なのか、癖なのか、どうしても拒否感を覚えずにはいられない。",
    "隔意\nあなたは彼（彼女）とは隔たりがあると感じている。何かの分野について、向こうと自分には距離があると思っている。",
    "憎悪\nあなたは彼（彼女）を憎んでいる。ここだけは許せない、という部分について、憎しみを燃やさずにはいられない。",
    "偏愛\nあなたは彼（彼女）を愛している。だが、その愛はどこか偏りがある、一方通行気味のものだ。",
    "疎外感\nあなたは彼（彼女）に疎外感を覚えている。なぜか仲間はずれにされているような、そんな感覚がまとわりつく。",
];
/// Ruby `TABLES["NFT"]` の項目。
static NFT_ITEMS_3: &[&str] = &[
    "劣等感\nあなたは彼（彼女）に劣等感を覚えている。彼（彼女）には敵わない、なぜか上に行かれる、そんなコンプレックスを持っている。",
    "不安\nあなたは彼（彼女）を見ると、どこか不安を覚える。理由は無いのかもしれない、なぜかイヤな予感を覚えるのだ。",
    "恐怖\nあなたは彼（彼女）を怖がっている。その能力や考え方、あるいは彼（彼女）自身ではなく、失うことを恐れているのかもしれない。",
    "嫉妬\nあなたは彼（彼女）に嫉妬している。その過去や能力、あるいは環境を羨ましいと思わずにはいられない。",
    "驚異\nあなたは彼（彼女）に脅威を感じている。自分にとって致命的な存在になるかもしれないと考えている。",
    "侮蔑\nあなたは彼（彼女）を侮り、蔑んでいる。彼（彼女）は自分より下の存在であると思っている。",
];
/// Ruby `TABLES["NFT"]`（`D66OneThirdTable`）。
static NFT: D66RowSplitTable =
    D66RowSplitTable::one_third("ネガティブ感情表", NFT_ITEMS_1, NFT_ITEMS_2, NFT_ITEMS_3);

/// Ruby `TABLES["CT"]` の項目。
static CT_ITEMS: &[&str] = &[
    "地形（TCT）\n森林、山岳、河川など、その国に存在する地勢を表す。",
    "産業（ICT）\n農耕が盛ん、鍛冶技術に優れるなど、その国の産業を表す。",
    "人物（PCT）\n腕のよい鍛冶屋がいる、知識豊富な薬師がいるなど、その国にいる人物を表す。",
    "組織（OCT）\n狩人の組合がある、キャラバンがいるなど、その国にある組織を表す。",
    "拠点（BCT）\n砦や関所などの軍用拠点の他、市場や街道など、その国にある拠点を表す。",
    "文化（CCT）\n食のバリエーションが多い、森に造詣が深いなどのその国の文化や風習を表す。",
];
/// Ruby `TABLES["CT"]`（`Table`, `1D6`）。
static CT: Table = Table::from_dice("国特徴・カテゴリー表", 1, 6, CT_ITEMS);

/// Ruby `TABLES["TCT"]` の項目。
static TCT_ITEMS_1: &[&str] = &[
    "水辺\nあなたの国は大河や海洋に面しており、水運の便に優れている。\n技術＋２、資金＋１",
    "森林\nあなたの国には大きな森がいくつもある。ただ風光明媚なだけでなく、木材資源にも恵まれている。\n森林＋３",
    "山岳\nあなたの国は大きな山に囲まれている。山は国にさまざまな実りをもたらしてくれる。\n鉱物＋２、森林＋１",
    "草原\nあなたの国は見渡す限りの大草原から構成されている。馬を育てたり交易を行なうには、絶好の土地と言えるだろう。\n食料＋１、馬＋２",
    "沼地\nあなたの国は小さな川と、それが流れ込む池や沼地から構成されている。\n食料＋２、森林＋１",
    "荒野\nあなたの国のほとんどは荒野からなっている。野生のごつごつした荒々しさが国土の特徴だ。\n鉱物＋２、馬＋２、資金－１",
];
/// Ruby `TABLES["TCT"]` の項目。
static TCT_ITEMS_2: &[&str] = &[
    "砂漠\nあなたの国は砂漠に取り巻かれている。灼熱の昼と極寒の夜があなたたちの日常だ。\n技術＋２、資金＋２、森林－１",
    "寒冷地\nあなたの国は雪と氷を友としている。夏は短く、冬は長い。鉛色の雲の下であなたたちは育った。\n鉱物＋２、資金＋１",
    "熱帯雨林\nあなたの国は焦熱の地である。雨期になれば大量の雨が降り注ぎ、巨大な密林を構成する。\n食料＋１、森林＋２",
    "火山\nあなたの国は火山地帯にある。山は火を噴き大地を揺らすが、それ故の豊かさもある。\n鉱物＋３",
    "諸島群\nあなたの国はいくつもの小さな島によって構成されている。交易にこれほど適した土地もないだろう。\n資金＋３",
    "秘境\nあなたの国は、混沌の影響によってこの世ならぬ光景を持っている。空を飛ぶ島や形を取った虹などだ。商才はＧＭと相談せよ。\n国資源ひとつを＋３",
];
/// Ruby `TABLES["TCT"]`（`D66HalfGridTable`）。
static TCT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・地形表", TCT_ITEMS_1, TCT_ITEMS_2);

/// Ruby `TABLES["ICT"]` の項目。
static ICT_ITEMS_1: &[&str] = &[
    "農業\nあなたの国は農業生産力に優れている。大地の恵みが国の基だ。\n食料＋３",
    "手工業\nあなたの国は織物や細工物といった人間の手作業から生み出される道具作りに秀でている。\n技術＋３",
    "鉱業\nあなたの国は大地そのものに眠る鉱物資源が豊富なことで知られている。\n鉱物＋４、森林－１",
    "牧畜\nあなたの国は広大な領地を生かして、牛や馬のような牧畜産業が豊かである。\n食料＋１、馬＋２",
    "漁業\nあなたの国は海または河川を生かした大規模な漁業で名を轟かせている。\n技術＋１、食料＋２",
    "貿易\nあなたの国は国と国とを結びつけ、その仲立ちをして利益を得ている。\n資金＋３",
];
/// Ruby `TABLES["ICT"]` の項目。
static ICT_ITEMS_2: &[&str] = &[
    "金融\nあなたの国は他社の金を預かり、その金を貸し付けることで富を得ている。\n資金＋３",
    "金属加工\nあなたの国は刀剣鍛冶や金細工といった、金属を加工して別の何かに変える産業を有している。\n技術＋２、鉱物＋２、森林－１",
    "ガラス\nあなたの国はガラス工芸技術を保持している。列国でも稀な技術だ。\n技術＋２、資金＋２、森林－１",
    "香辛料\nあなたの国は胡椒や唐辛子のようなスパイスの産地である。\n食料＋１、資金＋２",
    "酒造\nあなたの国は葡萄酒や蜂蜜酒のようなアルコールの産地として知られている。\n食料＋２、資金＋１",
    "サービス業\nあなたの国は演劇や酒場、レストランといった文化的な産業で名を馳せている。\n技術＋１、資金＋２",
];
/// Ruby `TABLES["ICT"]`（`D66HalfGridTable`）。
static ICT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・産業表", ICT_ITEMS_1, ICT_ITEMS_2);

/// Ruby `TABLES["PCT"]` の項目。
static PCT_ITEMS_1: &[&str] = &[
    "哲学者\nあなたの国には高名な哲学者がいる。あなたの国には知的な国家だと考えられている。\n技術＋１、資金＋２",
    "科学者\nあなたの国には混沌に立ち向かう科学者（メイジではない）がおり、技術に優れている。\n技術＋３",
    "高名な騎士\nあなたの国には高名な騎士がおり、その武勇を慕ってさまざまな武芸者が集まってくる。\n技術＋１、",
    "芸能者\nあなたの国には偉大な芸術家や歌姫がおり、その人気は他国にも響いている。\n技術＋１、資金＋２",
    "大商人\nあなたの国に居を構えている大商人の富は、一国にも匹敵する。その税収はばかにならない。\n資金＋３",
    "聖人\nあなたの国には聖人と呼ばれる偉大な宗教的指導者がいる。その名声は素晴らしい。\n食料＋２、資金＋１",
];
/// Ruby `TABLES["PCT"]` の項目。
static PCT_ITEMS_2: &[&str] = &[
    "森林官\nあなたの国には森林資源を管理する森林官がおり、その技術を弟子たちに分け与えている。\n森林＋３",
    "名工\nあなたの国には非常に優秀な鍛冶職人がおり、その技術を弟子たちに分け与えている。\n技術＋２、資金＋１",
    "売国奴\nあなたの国には極めて油断ならない臣下がいる。大変有能だが、裏切りを考えていることは明白なのだ。\n資金＋４、技術－１",
    "農民指導者\nあなたの国には農民たちを指導する偉大な英雄がいる。彼は常に農民の味方だ。\n食料＋３",
    "亡命者\nあなたの国には他国から亡命してきた優秀な技術者がいる。その事がいずれ災厄を呼び込むかもしれない\n国資源ふたつに＋２",
    "かつての英雄\nあなたの国土の一部は、かつての英雄たるアーティストと融合している。彼は時折あなたに力を貸してくれる。\n食料に＋１、森林＋２",
];
/// Ruby `TABLES["PCT"]`（`D66HalfGridTable`）。
static PCT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・人物表", PCT_ITEMS_1, PCT_ITEMS_2);

/// Ruby `TABLES["OCT"]` の項目。
static OCT_ITEMS_1: &[&str] = &[
    "運搬業者\nあなたの国には水運や運搬など、輸送に携わる人々が多い。\n食料＋３",
    "職人学校\nあなたの国は技術を伝授し発展させる学校を有している。\n技術＋３",
    "森番\nあなたの国は森を守るための独自の警察を有している。\n森林＋３",
    "金掘り衆\nあなたの国はいわゆる山師を組織化しており、鉱山開発に熱心だ。\n鉱物＋３",
    "民会\nあなたの国は豪商や大農場主からなる議会を持ち、意見を取り入れている。\n資金＋３",
    "飛脚\nあなたの国は飛脚や早馬といった高速通信網が発達している。\n馬＋３",
];
/// Ruby `TABLES["OCT"]` の項目。
static OCT_ITEMS_2: &[&str] = &[
    "放浪民\nあなたの国には、騎馬民族や放浪民といった人々が訪れることが多い。\n馬＋２、資金＋１",
    "宗教結社\nあなたの国には巨大な宗教結社が存在する。悩んだらクレスト教団とせよ。\n技術＋２、資金＋１",
    "学術団体\nあなたの国にはアカデミーの分校、あるいは何らかの研究施設が存在する。\n食料＋１、技術＋２",
    "異民族\nあなたの国には主流民族とは別の異民族が発生しており、目下のところ共存している。\n技術＋１、資金＋２",
    "難民\nあなたの国には他国の戦乱を逃れてきた難民たちが避難してきている。しかし、安価な労働力の供給源であることも確かだ。\n食料－２、技術＋２、資金＋３",
    "犯罪結社\nあなたの国は豊かで、それゆえに犯罪結社が跳梁している。\n資金＋３",
];
/// Ruby `TABLES["OCT"]`（`D66HalfGridTable`）。
static OCT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・組織表", OCT_ITEMS_1, OCT_ITEMS_2);

/// Ruby `TABLES["BCT"]` の項目。
static BCT_ITEMS_1: &[&str] = &[
    "城塞\nあなたの首都はその難攻不落の防壁で知られている。\n技術＋３",
    "長城\nあなたの国境線は堅牢な長城で守られている。それが国境線の全域に及ぶかどうかはＧＭと相談せよ。\n技術＋２、資金＋１",
    "良港\nあなたの国には誰もがうらやむ非常に安定した広い港がある。\n資金＋３",
    "運河\nあなたの国には巨大な運河があり、人々に流通と治水とをもたらしている。\n食料＋２、技術＋１",
    "図書館\nあなたの国には図書館があり、膨大な知識を過去から蓄積している。\n技術＋３",
    "訓練所\nあなたの国には訓練所があり、兵士たちを常に鍛えることができる。\n技術＋２、馬＋１",
];
/// Ruby `TABLES["BCT"]` の項目。
static BCT_ITEMS_2: &[&str] = &[
    "下町\nあなたの国には貧しい人々が住まう下町（悪い言い方をすれば貧民街）がある。豊かではないが、そこには活気がある。\n食料＋２、資金＋１",
    "高級住宅街\nあなたの国には豊かな人々が多く、そこに住みたいと考える人も多い。\n資金＋３",
    "保養地\nあなたの国には風光明媚で過ごしやすい土地がある。外国人であっても、そこで休暇を取ることを好むのだ。\n森林＋１、資金＋２",
    "宗教的聖地\nあなたの国には、それに実際的な効果があるかどうかはともかく宗教的な聖地があり、人々を集めている。\n食料＋２、森林＋１",
    "鉱山街\nあなたの国には鉱山があり、その鉱山で働く人たちのための酒場や学校や病院が完備されている。\n鉱物＋３",
    "歓楽街\nあなたの国は楽しく過ごせる酒場や劇場といったもので評判だ。人々の笑顔がさらなる富をもたらすのだ。\n食料＋１、資金＋２",
];
/// Ruby `TABLES["BCT"]`（`D66HalfGridTable`）。
static BCT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・拠点表", BCT_ITEMS_1, BCT_ITEMS_2);

/// Ruby `TABLES["CCT"]` の項目。
static CCT_ITEMS_1: &[&str] = &[
    "芸術指向\nあなたの国の人々は芸術に高い価値を与えている。\n資金＋３",
    "享楽的\nあなたの国民は、今日を楽しく過ごすことが大切だと考えている。\n食料＋１、資金＋２",
    "禁欲的\nあなたの国民は、道徳を重んじ、常に自分の欲望を制限することが理想的だと考えている。\n食料＋４、資金－１",
    "好戦的\nあなたの国民は好戦的だ。物事は剣と暴力によって解決するべきだと考えている。\n技術＋１、馬＋２",
    "平和主義\nあなたの国民は平和を愛している。まず話し合うことが大事だと思っているのだ。\n食料＋１、森林＋２",
    "理知的\nあなたの国民は論理性を重んじる。道理が通っているかが問題だ。\n技術＋３",
];
/// Ruby `TABLES["CCT"]` の項目。
static CCT_ITEMS_2: &[&str] = &[
    "情念深い\nあなたの国民は感情を重視する。恩は忘れず、恨みもまた決して忘れない。\n鉱物＋１、森林＋２",
    "礼儀重視\nあなたの国民は建前としての礼儀を重んじる。礼儀こそ平和と繁栄への道だ。\n技術＋２、馬＋１",
    "拝金主義\nあなたの国民にとって、もっとも重要なのは金である。金がなくてどうして生きて行けよう。\n森林－１、資金＋４",
    "農本主義\nあなたの国では農民が国の基である。大地を耕すことが理想的なのだ。\n食料＋３",
    "富国強兵\nあなたの国は技術を発展させ軍隊を近代化させるべく一丸となっている。\n技術＋２、馬＋１",
    "呑気\nあなたの国は豊かだ。人々は思い煩うことなく、日々をのんびりと楽しく生きている。\n技術－１、食料＋４",
];
/// Ruby `TABLES["CCT"]`（`D66HalfGridTable`）。
static CCT: D66RowSplitTable =
    D66RowSplitTable::half_grid("国特徴・文化表", CCT_ITEMS_1, CCT_ITEMS_2);

/// Ruby `TABLES`。`roll_tables` が引く表の一覧。
static TABLES: &[(&str, &'static dyn RollableTable)] = &[
    ("MT", &MT),
    ("PFT", &PFT),
    ("NFT", &NFT),
    ("CT", &CT),
    ("TCT", &TCT),
    ("ICT", &ICT),
    ("PCT", &PCT),
    ("OCT", &OCT),
    ("BCT", &BCT),
    ("CCT", &CCT),
];

/// Ruby `ALIASES`（`transform_keys(&:upcase)` 済み）。
static ALIASES: &[(&str, &str)] = &[
    ("POSITIVEFT", "PFT"),
    ("NEGATIVEFT", "NFT"),
    ("CATEGORYCT", "CT"),
];

/// Ruby `Base#roll_tables(command, tables)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    for (key, table) in TABLES {
        if *key == command {
            return Ok(Some(table.roll(rng)?.to_string()));
        }
    }
    Ok(None)
}

/// Ruby `BCDice::GameSystem::GranCrest`（ID: `GranCrest`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GranCrest;

impl GameSystem for GranCrest {
    fn id(&self) -> &'static str {
        "GranCrest"
    }

    fn name(&self) -> &'static str {
        "グランクレストRPG"
    }

    fn sort_key(&self) -> &'static str {
        "くらんくれすとRPG"
    }

    fn help_message(&self) -> &'static str {
        r"・2D6の目標値判定でクリティカル処理
　例）3d6>=19 3d6+5>=24
・邂逅表（MT）
・感情表（-FT）
　ポジティブ感情表（PFT）、ネガティブ感情表（NFT）
・国特徴表（-CT）
　カテゴリー表（CT）、地形表（TCT）、産業表（ICT）、人物表（PCT）
　組織表（OCT）、拠点表（BCT）、文化表（CCT）
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "MT",
            "PFT",
            "NFT",
            "CT",
            "TCT",
            "ICT",
            "PCT",
            "OCT",
            "BCT",
            "CCT",
            "POSITIVEFT",
            "NEGATIVEFT",
            "CATEGORYCT",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sort_add_dice(&self) -> bool {
        true
    }

    /// Ruby `#result_nd6`。ゲーム別成功度判定(nD6)。
    ///
    /// `critical` は `success` / `failure` とは独立に立つので
    /// `Result.critical` 生成子は使えない。
    fn result_nd6(
        &self,
        total: crate::Int,
        _dice_total: i64,
        dice_list: &[i64],
        cmp_op: CmpOp,
        target: Target,
    ) -> Option<CheckOutcome> {
        if cmp_op != CmpOp::Ge {
            return None;
        }

        let mut total = total;
        let mut result = EvalResult::new();
        let mut sequence: Vec<String> = Vec::new();

        if dice_list.iter().filter(|d| **d == 6).count() >= 2 {
            total += 10;
            result.critical = true;
            sequence.push("（クリティカル）".to_owned());
            sequence.push(total.to_string());
        }

        // Ruby: if target != '?'
        if let Target::Number(target) = target {
            if total >= target {
                sequence.push("成功".to_owned());
                result.success = true;
            } else {
                sequence.push("失敗".to_owned());
                result.failure = true;
            }
        }

        if sequence.is_empty() {
            return None;
        }

        result.text = sequence.join(" ＞ ");
        Some(CheckOutcome::Result(Box::new(result)))
    }

    /// Ruby `#eval_game_system_specific_command`。
    ///
    /// Ruby: `roll_tables(ALIASES[command] || command, TABLES)`
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        let key = ALIASES
            .iter()
            .find(|(alias, _)| *alias == command)
            .map_or(command, |(_, key)| *key);

        Ok(roll_tables(key, rng)?.map(SpecificCommandOutput::text))
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
            .join("test/data/GranCrest.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/GranCrest.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/GranCrest.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("GranCrest.toml must parse");
        assert_eq!(
            data.tests.len(),
            32,
            "case count in test/data/GranCrest.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "GranCrest",
                "unexpected game system in GranCrest.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("GranCrest"), &tc.input, &mut src) {
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
                    "FAIL GranCrest:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} GranCrest cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
