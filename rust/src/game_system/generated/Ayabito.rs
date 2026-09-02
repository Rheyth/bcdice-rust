//! P4で手書き移植した `lib/bcdice/game_system/Ayabito.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `Ayabito#check_action`（判定 `xAB±y@c$d>=z`）
//! - `Ayabito#eval_game_system_specific_command` → `check_action || roll_tables`
//! - `TABLES`（感情表 / 各シーン表 / 交流表 / ファンブル表 / 封印期間表 / 帝都東京エリア選択）
//!
//! 表データは Ruby の定数から機械的に書き出したもので、値は1文字も変えていない。
//! Ruby の `Array.new(6, "…")` は同じ文字列6個の配列なので、`[…; 6]` で表す。

use std::sync::OnceLock;

use crate::command_parser::Parser;
use crate::dice_table::{D66LeftRangeTable, D66Table, RangeInc, RollableTable, Table, TableItem};
use crate::enums::{D66SortType, RoundType};
use crate::eval::EvalError;
use crate::game_system::{dice_text, table_helpers, GameSystem, SpecificCommandOutput};
use crate::normalize::CmpOp;
use crate::randomizer::Randomizer;
use crate::result::EvalResult;
use crate::Int as I;

// ---------------------------------------------------------------------------
// 表データ
// ---------------------------------------------------------------------------

static ET_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("信頼/不信感")),
    (12, TableItem::Text("好奇心/無関心")),
    (13, TableItem::Text("優越感/劣等感")),
    (14, TableItem::Text("好意/敵意")),
    (15, TableItem::Text("安心感/不安感")),
    (16, TableItem::Text("愛情/偏愛")),
    (21, TableItem::Text("同情/憐憫")),
    (22, TableItem::Text("親近感/疎外感")),
    (23, TableItem::Text("連帯感/隔意")),
    (24, TableItem::Text("尽力/面倒")),
    (25, TableItem::Text("貸し/借り")),
    (26, TableItem::Text("庇護欲/食傷")),
    (31, TableItem::Text("期待/反発")),
    (32, TableItem::Text("熱狂/心酔")),
    (33, TableItem::Text("幸福感/不快感")),
    (34, TableItem::Text("尊敬/軽蔑")),
    (35, TableItem::Text("憧憬/嫉妬")),
    (36, TableItem::Text("忠誠/服従")),
    (41, TableItem::Text("友情/侮蔑")),
    (42, TableItem::Text("競争心/警戒")),
    (43, TableItem::Text("感謝/後悔")),
    (44, TableItem::Text("感服/恐怖")),
    (45, TableItem::Text("興味/屈辱")),
    (46, TableItem::Text("誠意/憎悪")),
    (51, TableItem::Text("羨望/嫌悪")),
    (52, TableItem::Text("共感/懸念")),
    (53, TableItem::Text("傾倒/厭気")),
    (54, TableItem::Text("赦し/怒り")),
    (55, TableItem::Text("有為/苦手")),
    (56, TableItem::Text("恩義/不満")),
    (61, TableItem::Text("予感/困惑")),
    (62, TableItem::Text("懐旧/忘却")),
    (63, TableItem::Text("慕情/執着")),
    (64, TableItem::Text("夢中/退屈")),
    (65, TableItem::Text("贖罪/罪悪感")),
    (66, TableItem::Text("慈愛/殺意")),
];
static ET: D66Table = D66Table::new("感情表", D66SortType::NoSort, ET_ITEMS);

/// Ruby `Array.new(6, "子～巳までの任意の十二支を選択する。")`
static CHOOSE_FIRST_HALF: [&str; 6] = ["子～巳までの任意の十二支を選択する。"; 6];
/// Ruby `Array.new(6, "午～亥まで任意の十二支を選択する。")`
static CHOOSE_SECOND_HALF: [&str; 6] = ["午～亥まで任意の十二支を選択する。"; 6];

static TST_FIRST: &[&str] = &[
    "［子］帝国大学の赤門、学生たちが今日も勉学に励んでいる。",
    "［丑］吉原の歓楽街。昼間は静かだが、夜は活気を見せてくれる。",
    "［寅］上野公園。桜に新緑、紅葉など、季節の顔を見せてくれる。",
    "［卯］浅草六区は今日も賑やか。浅草寺や仲見世には、多くの人々が行き交う。",
    "［辰］凌雲閣。浅草十二階として親しまれる塔に昇り、周囲を一望する。",
    "［巳］丸の内の東京駅。構内の喧騒とは裏腹に、霞ヶ関は静かに時が流れる。",
];
static TST_SECOND: &[&str] = &[
    "［午］銀座をぶらぶらと歩く。百貨店が立ち並ぶこの街では、何でも買うことができるらしい。",
    "［未］日比谷の帝国劇場。演目は、話題のトップスタアによる華やかなる歌劇のようだ。",
    "［申］皇居のほとり。水面にうかぶ蓮の葉が、しずかに揺れている。",
    "［酉］明治神宮の境内。神聖なる雰囲気を味わうことができる。",
    "［戌］新たな東京の名所である新宿。今では東西を二分する街である。",
    "［亥］日本帝国軍駐屯地。妖怪人間共同実働部隊の本部も敷地内にある。",
];
static TST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), TST_FIRST),
    (RangeInc::new(4, 5), TST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static TST: D66LeftRangeTable =
    D66LeftRangeTable::new("帝都東京シーン表", D66SortType::NoSort, TST_ITEMS);

static BST_FIRST: &[&str] = &[
    "［子］人々が寝静まる帝都の夜。月に雲がかかるとともに、魔の香りが漂っている。",
    "［丑］草木も眠る静けさの中、犬の遠吠えだけが聞こえてくる。",
    "［寅］一陣の風が吹き抜ける。風に乗った匂いが、妙に鼻をくすぐってきた。",
    "［卯］霧や朝もやに包まれる。向こうに見える姿は誰だろうか……",
    "［辰］帝都に朝日が射す。人々は起き出し、日々の営みを始める。",
    "［巳］清廉な雰囲気の風景。鳥や虫の声、風にそよぐ木々の音が聞こえてくる。",
];
static BST_SECOND: &[&str] = &[
    "［午］時計の針がある時間を指し示す。刻を告げるチャイムや鐘が響き渡る。",
    "［未］昼間の大通り。自動車や路面電車が走り、行き交う人々の雑踏が至るところで見られる。",
    "［申］夕刻、どこからともなく、定かではない声や物音が漏れてくる。",
    "［酉］瓦斯灯（がすとう）が通りを鮮やかに照らす。夜が街のもうひとつの顔を出し始める。",
    "［戌］星空の下、月明かりが微かに夜道を照らしている。",
    "［亥］光ひとつない暗闇の中。何者かの気配が蠢いている……",
];
static BST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), BST_FIRST),
    (RangeInc::new(4, 5), BST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static BST: D66LeftRangeTable =
    D66LeftRangeTable::new("場面演出シーン表", D66SortType::NoSort, BST_ITEMS);

/// Ruby `Array.new(6, "感情～性格までの任意のテーマを選択する。")`
static CET_CHOOSE_FIRST: [&str; 6] = ["感情～性格までの任意のテーマを選択する。"; 6];
/// Ruby `Array.new(6, "関係性～半生までの任意のテーマを選択する。")`
static CET_CHOOSE_SECOND: [&str; 6] = ["関係性～半生までの任意のテーマを選択する。"; 6];
static CET_FIRST: &[&str] = &[
    "［感情］相手に抱いている感情、伝えるべきか伝えないべきか。",
    "［人間］相手に人間という存在をどう思うか、聞いてみるとしよう。",
    "［友達］相手に友人や仲間について語ろう。話すことで分かる想いもあるだろう。",
    "［告白］相手に話していいか分からないが、自分の秘めたる想いを語ろう。",
    "［思い出］相手に、過去の思い出を話してみよう。相手から昔話をきけるかもしれない。",
    "［性格］相手に自身の性格を語ろう。表向きのみ話すか、奥底まで話してしまうかは、事と次第による。",
];
static CET_SECOND: &[&str] = &[
    "［関係性］相手とは、いつからこうした関係だったのか。相手と関係性について話そう。",
    "［妖怪］相手に妖怪という存在をどう思うか、聞いてみるとしよう。",
    "［あやびと］相手に、自分があやびとである意味や意義を語ってみよう。",
    "［想い］相手が今、何かしら想う人や物事について聞いてみよう。",
    "［夢］相手に自分の夢を語ろう。未来の夢、かつて捨てた夢かもしれない。",
    "［半生］相手に自身の半生を語ろう。半生こそが、今の自分となるきっかけなのだから……",
];
static CET_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CET_CHOOSE_FIRST),
    (RangeInc::new(2, 3), CET_FIRST),
    (RangeInc::new(4, 5), CET_SECOND),
    (RangeInc::new(6, 6), &CET_CHOOSE_SECOND),
];
static CET: D66LeftRangeTable = D66LeftRangeTable::new("交流表", D66SortType::NoSort, CET_ITEMS);

static FT_ITEMS: &[&str] = &[
    "PCの【耐久値】を-5する(最低1)。",
    "PCの【活力値】を-5する(最低1)",
    "PCは戦闘ないしフェイズが終了するまで《アビリティ》を使用できない。",
    "PCは戦闘ないしフェイズが終了するまで[絆]を使用できない。",
    "セッション終了時まで、登場するエネミーすべての【耐久値】を+3する",
    "セッション終了時まで、登場するエネミーすべてのダメージを+2する。",
];
static FT: Table = Table::from_dice("ファンブル表", 1, 6, FT_ITEMS);

static LT_ITEMS: &[&str] = &[
    // "1日",
    "1週間", "1ヶ月", "1年", "10年", "50年", "100年",
    // "500年",
];
static LT: Table = Table::from_dice("封印期間表", 1, 6, LT_ITEMS);

static TET_ITEMS: &[&str] = &[
    "浅草。庶民の盛り場として賑わう商店街と下町。",
    "上野。あらゆる路線の中心である帝都の玄関口。",
    "日本橋。江戸時代から変わらぬ商業の中心地。",
    "銀座。赤煉瓦が立ち並ぶ、帝都一モダンな繁華街。",
    "霞ヶ関。国会議事堂や裁判所がある官庁街。",
    "新宿。関東大震災以降、急速に発展した新しい街。",
];
static TET: Table = Table::from_dice("帝都東京エリア選択", 1, 6, TET_ITEMS);

static AST_FIRST: &[&str] = &[
    "［子］浅草寺。浅草の顔ともいえる寺。7世紀に建造された都内でも最古の寺。",
    "［丑］武神一刀流道場。浅草でも有名な剣術道場。柔術や薙刀など、剣以外の実践的な技も教えている。",
    "［寅］待乳の渡し。隅田川を橋で渡る代わりに、乗せて渡ってくれる小舟。",
    "［卯］帝都観光案内館。帝都東京内観光の案内所。ガイドつきのバス観光も行っている。",
    "［辰］神谷バー。老舗の飲食店であり、電気ブランを提供していることで有名。",
    "［巳］雷門。浅草寺の南側に建てられた門で、風神と雷神の像が安置されている。",
];
static AST_SECOND: &[&str] = &[
    "［午］仲見世。雷門から浅草寺本堂に続く仁王門まで立ち並ぶ商店街。",
    "［未］仰天堂。やたらと大盛りの食事を提供してくれる人気店。",
    "［申］混沌興行。妖怪たちが演じることで有名な見世物小屋。いつも混み合っている。",
    "［酉］浅草六区。浅草の中心街であり、店舗や演芸場、活動写真館が並んでいる。",
    "［戌］凌雲閣。浅草十二階とも呼ばれる塔。関東大震災でも崩れることなく、そびえ立っている。",
    "［亥］花屋敷。日本最初の遊園地であり、園内には動物園も完備している。",
];
static AST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), AST_FIRST),
    (RangeInc::new(4, 5), AST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static AST: D66LeftRangeTable =
    D66LeftRangeTable::new("浅草シーン表", D66SortType::NoSort, AST_ITEMS);

static UST_FIRST: &[&str] = &[
    "［子］上野恩賜公園。桜の名所でもある巨大な公園。敷地内では、四季折々の自然を楽しむことができる。",
    "［丑］東京帝室博物館。宮内省所管の博物館であり、珍しい品々が展示されている。",
    "［寅］上野駅。帝都と地方の路線を繋ぐ駅。地方から上京してくる人々を多く見かける。",
    "［卯］精養軒。本格的な洋食が楽しめる老舗のレストラン。人間、妖怪に限らず上流階級が通っている。",
    "［辰］御徒町。高架下と周辺に所狭しと民家や長屋、様々な店がひしめきあう下町の歓楽街。",
    "［巳］一鉄工場。偏屈で頑固だが有能な老人が経営する工場。自作のラジオが置かれている。",
];
static UST_SECOND: &[&str] = &[
    "［午］帝国大学。象徴の赤門が有名な、日本の最高学府。校内には、妖怪研究室がある。",
    "［未］鳳明館。明治創業の旅館。文士が執筆する際に、よく利用している。",
    "［申］黄龍門学園。帝国大学敷地の横にある私立学園。妖怪や半妖も多く通っている。",
    "［酉］不忍池。弁天堂と大黒天堂を構える池で、河童が隠れ住んでいる。",
    "［戌］きさらぎ長屋。行く宛のない者や、行き場をなくした者が集まる長屋。",
    "［亥］上野恩賜公園動物園。ホッキョクグマ舎やサル山をはじめ、珍しい動物が飼われている。",
];
static UST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), UST_FIRST),
    (RangeInc::new(4, 5), UST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static UST: D66LeftRangeTable =
    D66LeftRangeTable::new("上野シーン表", D66SortType::NoSort, UST_ITEMS);

static NST_FIRST: &[&str] = &[
    "［子］日本銀行本店。西洋建築の先駆けといえる建築物。帝都事変では大きな被害が出た。",
    "［丑］三越。日本橋に居を構える有数の大型百貨店。この店で揃わぬ物はないとされている。",
    "［寅］日本橋。五つ街道の起点であり、東海道五十三次の出発点としてよく知られる橋。",
    "［卯］メイゾン妖の巣。妖怪や半妖の文学者たちが集うカフェー。",
    "［辰］大正座。明治座の兄弟とよばれる演芸場。歌舞伎や芝居などが上演されている。",
    "［巳］多々良堂。江戸時代より続く退魔具の専門店。多くのあやびとが訪れる。",
];
static NST_SECOND: &[&str] = &[
    "［午］妖艶大世界。上海にある上海大世界を真似て、妖怪や半妖が集まってできた新興の花街。",
    "［未］二丁巴里。丸ノ内の一丁倫敦に対して創られた大規模な問屋街。",
    "［申］皇居外苑。桔梗門の前は大広場があり、皇居を訪れる者が集まっている。",
    "［酉］東京駅。皇居に対面する形で作られた煉瓦造りの壮麗な駅。",
    "［戌］丸ノ内ビルヂング。大正12年に竣工した東洋一ともいわれる巨大なビルヂング。",
    "［亥］将門塚。平将門公の御首が祀られる塚。大手町にひっそりと存在している。",
];
static NST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), NST_FIRST),
    (RangeInc::new(4, 5), NST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static NST: D66LeftRangeTable =
    D66LeftRangeTable::new("日本橋シーン表", D66SortType::NoSort, NST_ITEMS);

static GST_FIRST: &[&str] = &[
    "［子］ダンスホウル・ガアデン。モダンボーイやモダンガール御用達の帝都東京有数のダンスホウル。",
    "［丑］服部時計店。銀座のシンボルとなっている時計塔と、併設された店舗。",
    "［寅］松屋。大正14年に開店した百貨店。地上7階までの吹き抜けステンドグラスで知られる。",
    "［卯］歌舞伎座。歌舞伎の殿堂。古式ゆかしい意匠を取り入れた最新建築の劇場。",
    "［辰］倫敦橋。文士や芸術家が好んで訪れる2階建ての洋式建築のカフェー。",
    "［巳］資生堂パーラー。ソーダ水やアイスクリンを提供したことで有名な喫茶店。",
];
static GST_SECOND: &[&str] = &[
    "［午］カフェープランタン。日本初のカフェーといわれる老舗。富裕層やインテリ層が多く訪れる。",
    "［未］新橋絢爛花街。関東大震災をきっかけに再建された花街。政府高官なども利用している。",
    "［申］鹿鳴館。明治時代を代表する西洋建築。華族や資産家が利用できる施設となっている。",
    "［酉］帝国ホテル。フランク・ロイド・ライトが設計した最新鋭の技術が詰め込まれたホテル。",
    "［戌］妖務省本部。妖務省と妖怪人間共同実働部隊の本部が置かれている。",
    "［亥］帝国劇場。明治時代を代表する日本最大の大劇場で\"帝劇\"の愛称で知られている。",
];
static GST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), GST_FIRST),
    (RangeInc::new(4, 5), GST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static GST: D66LeftRangeTable =
    D66LeftRangeTable::new("銀座シーン表", D66SortType::NoSort, GST_ITEMS);

static KST_FIRST: &[&str] = &[
    "［子］桜田門。内堀にある門のひとつで皇居に通じている。",
    "［丑］警視庁庁舎。警視庁の総合本部。庁舎内に妖鬼対策本部がある。",
    "［寅］日比谷公園。市民の憩いの場所であり、図書館や音楽堂をそなえる大規模な公園。",
    "［卯］大審院庁舎。司法裁判所の中における最上級審の裁判所。",
    "［辰］私立聖ロザリオ女学園。愛宕山の森に囲まれた都内随一のお嬢様学校。",
    "［巳］片倉組。武家屋敷を改装した本邸であり、帝都屈指のヤクザの根城となっている。",
];
static KST_SECOND: &[&str] = &[
    "［午］妖人史料編纂局。妖怪の史料の収集と編纂を目的として設置された官立組織。",
    "［未］鰐淵金融。利子こそ高いが、誰にでも門戸を開いている貸金業者。",
    "［申］料亭山王園。政治家や官僚御用達の料亭。十二真鬼も出入りしている。",
    "［酉］日枝神社。山王祭で知られる神社。数多くの刀剣が納められている。",
    "［戌］国会議事堂。現在建設中の国会議事堂。大正25年に竣工予定で、現在は木造の仮議事堂がある。",
    "［亥］帝国図書館。国立図書館で、国内で出版されたあらゆる書籍が収蔵されている。",
];
static KST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), KST_FIRST),
    (RangeInc::new(4, 5), KST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static KST: D66LeftRangeTable =
    D66LeftRangeTable::new("霞ヶ関シーン表", D66SortType::NoSort, KST_ITEMS);

static SST_FIRST: &[&str] = &[
    "［子］武蔵野館。座席数1,200を誇る国内有数の活動大写真館。",
    "［丑］二幸。食料品専門百貨店。あやびとたちに友好的で、伝奇事件が解決すると飲食が提供される。",
    "［寅］高野フルーツパーラー。マスクメロンが有名な果物専門店と、併設された果物が楽しめる飲食店。",
    "［卯］紀伊國屋書店。大正16年に創業した、文士たちが集うサロンのような大型書店。",
    "［辰］新宿御苑。宮内省が管理する皇室のための庭園。封印具の保管庫である浄玻璃ノ宮がある。",
    "［巳］明治神宮。明治天皇と昭憲皇太后を祭神とした神社。境内は厳かな雰囲気に満ちている。",
];
static SST_SECOND: &[&str] = &[
    "［午］新宿駅。多摩や小田原からの玄関口となる駅。1日あたりの乗降客数は日本一。",
    "［未］酩酊横丁。長屋が連なる路地に、200軒以上もの居酒屋やバーがひしめいている。",
    "［申］歌楽騒戯通り。関東大震災後に作られた真新しい建物がひしめく歓楽街。",
    "［酉］淀橋浄水場。コレラ流行後に、水道を近代化させるために作られた浄水場。",
    "［戌］新宿大通り。急速な発展を遂げた新宿のメイン通り。",
    "［亥］ほたる屋。妖狐と妖狸が経営している衣料品専門の百貨店。",
];
static SST_ITEMS: &[(RangeInc, &[&str])] = &[
    (RangeInc::new(1, 1), &CHOOSE_FIRST_HALF),
    (RangeInc::new(2, 3), SST_FIRST),
    (RangeInc::new(4, 5), SST_SECOND),
    (RangeInc::new(6, 6), &CHOOSE_SECOND_HALF),
];
static SST: D66LeftRangeTable =
    D66LeftRangeTable::new("新宿シーン表", D66SortType::NoSort, SST_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, &dyn RollableTable)] = &[
    ("ET", &ET),
    ("TST", &TST),
    ("BST", &BST),
    ("CET", &CET),
    ("FT", &FT),
    ("LT", &LT),
    ("TET", &TET),
    ("AST", &AST),
    ("UST", &UST),
    ("NST", &NST),
    ("GST", &GST),
    ("KST", &KST),
    ("SST", &SST),
];

// ---------------------------------------------------------------------------
// コマンド評価
// ---------------------------------------------------------------------------

/// Ruby `Ayabito#check_action`（判定 `xAB±y@c$d>=z`）。
fn check_action(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    static PARSER: OnceLock<Parser> = OnceLock::new();
    let parser = PARSER.get_or_init(|| {
        Parser::new(&["AB"], RoundType::Ceil)
            .has_prefix_number()
            .enable_critical()
            .enable_dollar()
            .restrict_cmp_op_to(&[None, Some(CmpOp::Ge)])
    });
    let Some(parsed) = parser.parse(command) else {
        return Ok(None);
    };
    // has_prefix_number なので必ず入る
    let Some(prefix_number) = parsed.prefix_number else {
        return Ok(None);
    };

    let (dice_cnt, over_modify) = if prefix_number < I::from(10) {
        (prefix_number.clone(), I::ZERO)
    } else {
        (I::from(9), prefix_number - I::from(9))
    };
    let modify = parsed.modify_number;
    let critical_target = parsed
        .critical
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(6);
    let addition_target = parsed
        .dollar
        .as_ref()
        .map(crate::randomizer::sat_i64)
        .unwrap_or(6);
    let target = parsed.target_number;

    let mut dice_arr = rng.roll_barabara(crate::randomizer::sat_i64(&dice_cnt), 6)?;
    dice_arr.sort_unstable();
    let dice_str = dice_text::join_dice(&dice_arr);
    let has_critical = dice_arr.iter().any(|&x| x >= critical_target);
    let mut success_cnt: i64 = dice_arr.iter().filter(|&&x| x >= 4).count() as i64
        + dice_arr.iter().filter(|&&x| x >= addition_target).count() as i64
        + crate::randomizer::sat_i64(&(over_modify.clone() * 2));
    let has_fumble = success_cnt == 0 && dice_arr.contains(&1);
    if has_fumble {
        success_cnt = 0;
    } else {
        success_cnt = success_cnt.saturating_add(crate::randomizer::sat_i64(&modify));
    }
    let result = match target {
        None => success_cnt >= 1,
        Some(target) => success_cnt >= crate::randomizer::sat_i64(&target),
    };

    // Ruby: `over_modify > 0 ? "+#{over_modify * 2}" : ''`（式と出目の両方に付く）
    let over_text = if over_modify > I::ZERO {
        format!("+{}", over_modify * 2)
    } else {
        String::new()
    };
    let text = format!(
        "({}B6>=4){over_text} ＞ [{dice_str}]{over_text} ＞ 成功数{success_cnt} ＞ {}{}{}",
        crate::randomizer::sat_i64(&dice_cnt),
        if result { "成功" } else { "失敗" },
        if has_critical {
            "(クリティカル)"
        } else {
            ""
        },
        if has_fumble { "(ファンブル)" } else { "" },
    );

    Ok(Some(EvalResult {
        text,
        critical: has_critical,
        fumble: has_fumble,
        success: result,
        failure: !result,
        ..EvalResult::default()
    }))
}

/// Ruby `BCDice::GameSystem::Ayabito`（ID: `Ayabito`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ayabito;

impl GameSystem for Ayabito {
    fn id(&self) -> &'static str {
        "Ayabito"
    }

    fn name(&self) -> &'static str {
        "あやびと"
    }

    fn sort_key(&self) -> &'static str {
        "あやひと"
    }

    fn help_message(&self) -> &'static str {
        r"・判定コマンド(xAB±y@c$d>=z)
  x：サイコロの数(10以上の場合9個振り、それ以降を成功数2として加算する)
  ±y：成功数への補正(省略可)
  c：クリティカル値(@ごと省略可。省略時は6)
  d：出目を2として数える数の最小値($ごと省略可。省略時は6)
  z：目標値(妨害値など。>=ごと省略可)
  (例) 4AB
       11AB>=5
       5AB+1
       6AB@5>=3

・各種表
  感情表 ET
  帝都東京シーン表 TST / 場面演出シーン表 BST
  交流表 CET
  ファンブル表 FT
  封印期間表 LT

  帝都東京エリア選択 TET
  　浅草シーン表 AST
  　上野シーン表 UST
  　日本橋シーン表 NST
  　銀座シーン表 GST
  　霞ヶ関シーン表 KST
  　新宿シーン表 SST
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d*AB", "ET", "TST", "BST", "CET", "FT", "LT", "TET", "AST", "UST", "NST", "GST",
            "KST", "SST",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn sort_barabara_dice(&self) -> bool {
        true
    }

    fn round_type(&self) -> RoundType {
        RoundType::Ceil
    }

    /// Ruby `Ayabito#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        if let Some(result) = check_action(command, rng)? {
            return Ok(Some(SpecificCommandOutput::result(result)));
        }
        Ok(table_helpers::roll_table(command, TABLES, rng)?.map(SpecificCommandOutput::text))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_toml_cases_pass() {
        crate::game_system::test_support::assert_toml_cases_strict("Ayabito", "Ayabito.toml", 41);
    }
}
