//! P4で手書き移植した `lib/bcdice/game_system/KimitoYell.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//!
//! 移植したもの:
//! - `KimitoYell#eval_game_system_specific_command`（コマンド振り分け）
//! - `#roll_ky_judge`（判定 `nKY6` / `nKY10`）
//! - `#roll_fumble`（ファンブル表 `FT`）
//! - `#generate_new_encounter`（出会い系表 `NMTA` / `NMT` / `MCT` / `CIT` / `HYT` / `TLT` / `EPT`）
//! - `#generate_new_name`（命名系表 `FNG` / `LNT` / `FNT` / `JLTO` / `JLTT` / `FLT` / `JFTO` / `JFTT` / `FFT`）
//! - 各定数テーブル（`FTABLE` 〜 `TABLES`）

use std::sync::OnceLock;

use regex::Regex;

use crate::dice_table::{D66Table, RollableTable, TableItem};
use crate::enums::D66SortType;
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;
use crate::result::EvalResult;

/// Ruby `BCDice::GameSystem::KimitoYell`（ID: `KimitoYell`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KimitoYell;

impl GameSystem for KimitoYell {
    fn id(&self) -> &'static str {
        "KimitoYell"
    }

    fn name(&self) -> &'static str {
        "キミトエール！"
    }

    fn sort_key(&self) -> &'static str {
        "きみとええる"
    }

    fn help_message(&self) -> &'static str {
        r"■ 判定 （nKY6 / nKY10）
指定された能力値分（n個）のダイスを使って判定を行います。
・nKY6…「有利」を得ていない場合、6面ダイスをn個振って判定します。
・nKY10…「有利」を得ている場合、10面ダイスをn個振って判定します。
6もしくは10の出目があればスペシャル。1の出目があればファンブル。
スペシャルとファンブルは同時に発生した場合、両方の処理を行う。

■ 表
─ ファンブル表（FT）
  ファンブル時の処理を決定します。

─ 新しい出会いを求める
  ─ 一括 新しい出会い表（NMTA） # New Meet Table
    その後の表を含めてすべて同時に決定します。
    ひとつひとつ振る場合には下記のコマンドを使用してください。
  ─ 新しい出会い表（NMT） # New Meet Table
  ─ 偶然出会った表（MCT） # Meet by Chance Table
  ─ 交流のなかった身近な人表（CIT） # someone Close to you but no Interaction Table
  ─ 助けてくれた人表（HYT） # someone Help You table
  ─ どんな人だったか表（TLT） # what's They Like Table
  ─ 変わった人だった表（EPT） # Eccentric Person Table

─ ランダム命名表
  ─ フルネーム一括生成（FNG） # Full Name Generation
  ─ 名字表（LNT） # LastName Table
  ─ 名前表（FNT） # FirstName Table
  ─ 日本名字表1（JLTO） # Japanese Lastname Table One
  ─ 日本名字表2（JLTT） # Japanese Lastname Table Two
  ─ カタカナ名字表（FLT） # Foreien Lastname Table
  ─ 日本名前表1（JFTO） # Japanese Firstname Table One
  ─ 日本名前表2（JFTT） # Japanese Firstname Table Two
  ─ カタカナ名前表（FFT） # Foreien Firstname Table
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            r"\d+KY[6|10]",
            "FT",
            "NMTA",
            "NMT",
            "MCT",
            "CIT",
            "HYT",
            "TLT",
            "EPT",
            "FNG",
            "LNT",
            "FNT",
            "JLTO",
            "JLTT",
            "FLT",
            "JFTO",
            "JFTT",
            "FFT",
        ]
    }

    crate::impl_prefixes_pattern!();

    /// Ruby `KimitoYell#eval_game_system_specific_command`。
    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        // Ruby: if /^(\d+)KY(6|10)$/.match(command).nil? != true
        if ky_pattern().is_match(command) {
            return roll_ky_judge(command, rng).map(|r| r.map(SpecificCommandOutput::result));
        }

        // Ruby: elsif /FT/.match(command).nil? != true && /JFTO|JFTT|FFT/.match(command).nil? == true
        if command.contains("FT")
            && !command.contains("JFTO")
            && !command.contains("JFTT")
            && !command.contains("FFT")
        {
            return roll_fumble(command, rng).map(|s| s.map(SpecificCommandOutput::text));
        }

        // Ruby: elsif /NMTA|NMT|MCT|CIT|HYT|TLT|EPT/.match(command).nil? != true
        if ["NMTA", "NMT", "MCT", "CIT", "HYT", "TLT", "EPT"]
            .iter()
            .any(|k| command.contains(k))
        {
            return generate_new_encounter(command, rng)
                .map(|s| s.map(SpecificCommandOutput::text));
        }

        // Ruby: elsif /FNG|LNT|FNT|JLTO|JLTT|FLT|JFTO|JFTT|FFT/.match(command).nil? != true
        if [
            "FNG", "LNT", "FNT", "JLTO", "JLTT", "FLT", "JFTO", "JFTT", "FFT",
        ]
        .iter()
        .any(|k| command.contains(k))
        {
            return generate_new_name(command, rng).map(|s| s.map(SpecificCommandOutput::text));
        }

        Ok(None)
    }
}

/// Ruby `Subject#roll_tables(command, TABLES)`。
fn roll_tables(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let Some((_, table)) = TABLES.iter().find(|(key, _)| *key == command) else {
        return Ok(None);
    };
    Ok(Some(table.roll(rng)?.to_string()))
}

// ---------------------------------------------------------------------------
// 表
// ---------------------------------------------------------------------------

/// Ruby `FTABLE`。
static FTABLE: &[&str] = &[
    "とんでもない大失敗！　魔法でないと取り返しがつかない！　出目にかかわらず、「魔法の提案」をしない限り判定は失敗になる。",
    "もうちょっと何かが足りない。自分の【がんばり】を1点消費することで、出目にかかわらず判定を成功にできる。【がんばり】を消費しなければ出目にかかわらず判定が失敗になる。",
    "トラブルが発生したけど「大切な想い」を思い出して何とか乗り切った。大切だと思う世界から学んだこと、大切だと思いたい世界に対する気持ちを思い出して、なんとかしよう。自分の持っているカードの「大切な想い」を1つ選んで〇で囲む。〇で囲めない場合、判定は出目にかかわらず失敗になる。",
    "トラブルが発生した。こんな自分を、あの人が見たらどう思うかな。自分が持っているカードに、「大切な想い」を考えて1つ書き込む。",
    "トラブルが発生したけど、偶然にも自分の「守りたい人」が助けてくれた。あるいは、「守りたい人」の教えてくれたことが役立った。ありがとう……。",
    "ちょっとヒヤリとする瞬間があったけど、何も起こらなかった。よかった。",
];

/// Ruby `NMTABLE`。
static NMTABLE: &[&str] = &[
    "新しい出会い表",
    "「偶然出会った表（MCT）」と「どんな人だったか表（TLT）」を使用してNPCを作成する。",
    "「偶然出会った表（MCT）」と「変わった人だった表（EPT）」を使用してNPCを作成する。",
    "「交流のなかった身近な人表（CIT）」と「どんな人だったか表（TLT）」を使用してNPCを作成する。",
    "「交流のなかった身近な人表（CIT）」と「変わった人だった表（EPT）」を使用してNPCを作成する。",
    "「助けてくれた人表（HYT）」と「どんな人だったか表（TLT）」を使用してNPCを作成する。",
    "「助けてくれた人表（HYT）」と「変わった人だった表（EPT）」を使用してNPCを作成する。",
];

/// Ruby `MCTABLE`。
static MCTABLE: &[&str] = &[
    "偶然出会った表",
    "何らかの事件や事故が起こり、それに巻き込まれた人を助けるために動いた。そのお礼をしたいと声をかけられた。",
    "急に振り出した雨。屋根のある所に雨宿りをした際に、同じく雨宿りをしていた人物と話をした。",
    "図書館の資料を集めていたところ、偶然にも同じ資料を借りようとしていた人物とバッティングしてしまった。どちらが先にするか話し合った。",
    "魔法やオカルトの事を調べるのが趣味らしく、ちょっとした魔法の事件が起こった場所をうろついていた。巻き込まれないように声をかけたら、自分が怪しまれた。",
    "街を歩いている時に、うずくまっている人を見つけた。何があったのかと声をかけてみると、何か困っていることがあるらしい。それを助けた。",
    "偶然にも稼働していない魔具を見つけたので、回収するために持ち主と話をすることになった。",
    "以前、魔具関係の事件で駆け回った時の自分を見かけた人がいたらしい。その時の顔が印象に残ったらしく、何をしていたのか聞かれた。",
    "「MAGIA」にたちよったところ、知らない店員がいた。新しく雇ったアルバイトらしく、何をしていたのか聞かれた。",
    "たまたま立ち寄った飲食店の店長から、試供品が提供されて、味の感想を求められた。どうやら新メニューを作りたいらしく、いろいろな人の意見を聞いているらしい。",
    "「守りたい人」に会いに行ったら、「守りたい人」の親友と名乗る人と出会った。自分の知らない「守りたい人」について話してくれた。",
];

/// Ruby `CITABLE`。
static CITABLE: &[&str] = &[
    "交流のなかった身近な人表",
    "その人は自分の親戚で、家の用事で出かけたときに親などから紹介をされた。話をしてみたら、自分の興味と同じものを研究していた。",
    "親などに頼まれて、ご近所に挨拶へ伺った。その人は近所で見かけることがあったが、不思議と今まで交流がなく、よくわからない人だった。",
    "「守りたい人」がたまに話題に出す友人と、「守りたい人」に紹介される形で会う機会ができた。この人はどんな人なのか、見てみよう。",
    "きょうだいの知り合いで、挨拶ぐらいはしていたけど、二人きりになったのは初めてだった。きょうだいも用事でいないし、どう話したものか。",
    "学業やスポーツの関係で、遠くの国で暮らしていたきょうだい（あるいはいとこ）が帰ってきた。優秀な成績を修めて、世間からも注目されている人物にどう接しようか。",
    "昔はずっと一緒に遊んでいた幼馴染だったけど、事情があって最近まで遠くに出かけていた。数日前に帰ってきたらしく、挨拶しにやってきた。",
    "「MAGIA」に立ち寄ったところ、店長から話しかけられた。どうも今は暇らしく、話し相手になってほしいとのことだ。",
    "SNSなどで知り合い、趣味が合ってネット上の友人となれた人物と、外でも会うことになった。",
    "クラスや職場で人気の人が、たまたま一人でいるところを見かけた。向こうもこちらに気づいたらしく、話しかけてきた。さて、どうするかな。",
    "趣味の集いに行ったら、クラスメイトや元クラスメイトがいた。趣味の場で会ってみると教室との印象が違った。",
];

/// Ruby `HYTABLE`。
static HYTABLE: &[&str] = &[
    "助けてくれた人表",
    "忘れ物をしたが、それを届けてくれた。その際に少し話をしたが、優しく丁寧で好感の持てる人だった。",
    "自分が転びそうになった、あるいは轢かれそうになった時に、助けてくれた。",
    "用事があって普段行かない場所に行ったとき、迷子になった自分を助けてくれた。",
    "自分がケガをした子供の対処に困っている時、一緒になって子供の面倒を診てくれた。",
    "自分は幼い頃、外でケガをしてしまったことがある。その時に応急処置をして、病院まで運んでくれた人がいる。その人と偶然再会した。",
    "自分は幼い頃、何かしらの事情で孤独になり、辛かった時期がある。そんな時に、声をかけて一緒に遊んでくれた人がいた。その人と再会した。",
    "自分が不良や話しかけられたくないタイプの人に絡まれた時、声をかけて助けてくれた人がいる。",
    "図書館などで資料集めをしている際に、声をかけて助けてくれた人がいた。",
    "魔具や財布など重要なものを失くしてしまい、探している必死そうな自分を見て助けてくれた。",
    "昔、魔法関係で困った時に、助けてくれた魔法使いがいた。その人が何かの用事で「MAGIA」に立ち寄っており、その時に声をかけた。",
];

/// Ruby `TLTABLE`。
static TLTABLE: &[&str] = &[
    "どんな人だったか表",
    "「守りたい人」によく似ている。",
    "不良っぽいファッションだけど、単にそういう格好が好きなだけで丁寧な人だった。",
    "パリッとした服装、きちんとした身なりで優等生あるいは真面目な人という感じ。",
    "活発そうな人物で、スポーツに打ち込んでいそうな体格と口調。いかにも体育会系。",
    "サバサバとした性格で、いろいろな人の悩み事を聞いては解決しようとしていた兄貴分（姉貴分）。",
    "細かいことが気になるタイプのようで、何かとチェックしてそうな視線を感じた。",
    "優しい性格で、自分の言葉を待って聞いてくれる人だった。",
    "おしゃべりな人物で、いろいろなことをしゃべってくれる。そのうえで、こちらの話も聞いてくれた。",
    "不思議と小動物のような印象を受けた。懐いてきて、自分の反応を楽しみにしているような、そんな人だ。",
    "「守りたい人」の知り合いで、共通の知り合いの話題ができた。",
];

/// Ruby `EPTABLE`。
static EPTABLE: &[&str] = &[
    "変わった人だった表",
    "元気すぎる。声も大きいし、自分は振り回されるし。ちょっと疲れる。",
    "優しそうだけど、どこか底知れない、何を考えているのかわからない人物だった。",
    "見るからに遊び人で、言動も軽く見える。どこか一定以上親しくなるのを恐れている部分が垣間見えた。",
    "ぶっきらぼうで一見とっつきづらい印象だけど、こちらのことをよく見ていて、助けようとしている。たぶん、寂しがり屋。",
    "トレンドを着こなしていて、いかにもおしゃれな人だった。ファッションだけでなく、あらゆることを調べて理解しようと動いている。努力の人なんだと思う。",
    "自分がその人にとって大事な人に似ているらしく、世話を焼こうとしてくれている。",
    "お菓子職人や料理人を目指しているらしく、試食を頼んでくる。",
    "誰かを助けなければならない、という理念があり、とにかく人助けをして回っている人だった。自分のことは二の次のようだ。",
    "すごい資産家の一族らしく、身に着けている物はすべて高級で、教養もあった。",
    "常に何かのアルバイトをしている。掛け持ちだから忙しくしているらしい。",
];

/// Ruby `LNTABLE`。
static LNTABLE: &[&str] = &[
    "名字表",
    "日本名字表1（JLTO）を使用する。",
    "日本名字表1（JLTO）を使用する。",
    "日本名字表2（JLTT）を使用する。",
    "日本名字表2（JLTT）を使用する。",
    "カタカナ名字表（FLT）を使用する。",
    "カタカナ名字表（FLT）を使用する。",
];

/// Ruby `FNTABLE`。
static FNTABLE: &[&str] = &[
    "名前表",
    "日本名前表1（JFTO）を使用する。",
    "日本名前表1（JFTO）を使用する。",
    "日本名前表2（JFTT）を使用する。",
    "日本名前表2（JFTT）を使用する。",
    "カタカナ名前表（FFT）を使用する。",
    "カタカナ名前表（FFT）を使用する。",
];

/// Ruby `JLTO`（`DiceTable::D66Table`・D66SortType::ASC）。
static JLTO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("有栖（ありす）")),
    (12, TableItem::Text("佐藤（さとう）")),
    (13, TableItem::Text("鈴木（すずき）")),
    (14, TableItem::Text("葉月（はづき）")),
    (15, TableItem::Text("如月（きさらぎ）")),
    (16, TableItem::Text("皐月（さつき）")),
    (22, TableItem::Text("九重（ここのえ）")),
    (23, TableItem::Text("高橋（たかはし）")),
    (24, TableItem::Text("田中（たなか）")),
    (25, TableItem::Text("右京（うきょう）")),
    (26, TableItem::Text("七海（ななみ）")),
    (33, TableItem::Text("小春（こはる）")),
    (34, TableItem::Text("伊藤（いとう）")),
    (35, TableItem::Text("渡辺（わたなべ）")),
    (36, TableItem::Text("飛鳥（あすか）")),
    (44, TableItem::Text("渡井（わたらい）")),
    (45, TableItem::Text("井上（いのうえ）")),
    (46, TableItem::Text("氷室（ひむろ）")),
    (55, TableItem::Text("錦（にしき）")),
    (56, TableItem::Text("柳（やなぎ）")),
    (66, TableItem::Text("蓬莱（ほうらい）")),
];
static JLTO: D66Table = D66Table::new("日本名字表1", D66SortType::Asc, JLTO_ITEMS);

/// Ruby `JLTT`（`DiceTable::D66Table`・D66SortType::ASC）。
static JLTT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("蜂須賀（はちすか）")),
    (12, TableItem::Text("山本（やまもと）")),
    (13, TableItem::Text("中村（なかむら）")),
    (14, TableItem::Text("御影（みかげ）")),
    (15, TableItem::Text("四季（しき）")),
    (16, TableItem::Text("常磐（ときわ）")),
    (22, TableItem::Text("栗栖（くりす）")),
    (23, TableItem::Text("小林（こばやし）")),
    (24, TableItem::Text("加藤（かとう）")),
    (25, TableItem::Text("花野井（はなのい）")),
    (26, TableItem::Text("綾瀬（あやせ）")),
    (33, TableItem::Text("乙女（おとめ）")),
    (34, TableItem::Text("吉田（よしだ）")),
    (35, TableItem::Text("山田（やまだ）")),
    (36, TableItem::Text("桐葉（きりは）")),
    (44, TableItem::Text("桔梗（ききょう）")),
    (45, TableItem::Text("松本（まつもと）")),
    (46, TableItem::Text("音羽（おとわ）")),
    (55, TableItem::Text("蓮見（はすみ）")),
    (56, TableItem::Text("桜森（さくらもり）")),
    (66, TableItem::Text("百合園（ゆりぞの）")),
];
static JLTT: D66Table = D66Table::new("日本名字表2", D66SortType::Asc, JLTT_ITEMS);

/// Ruby `FLT`（`DiceTable::D66Table`・D66SortType::ASC）。
static FLT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("レイエス")),
    (12, TableItem::Text("スミス")),
    (13, TableItem::Text("ジョンソン")),
    (14, TableItem::Text("ウィリアム")),
    (15, TableItem::Text("ブラウン")),
    (16, TableItem::Text("ジョーンズ")),
    (22, TableItem::Text("シュルツ")),
    (23, TableItem::Text("エメリヒ")),
    (24, TableItem::Text("ファル")),
    (25, TableItem::Text("クルツ")),
    (26, TableItem::Text("マイアー")),
    (33, TableItem::Text("コスキネン")),
    (34, TableItem::Text("モロー")),
    (35, TableItem::Text("ルノー")),
    (36, TableItem::Text("ロベール")),
    (44, TableItem::Text("ラ")),
    (45, TableItem::Text("エン")),
    (46, TableItem::Text("キョウ")),
    (55, TableItem::Text("ハン")),
    (56, TableItem::Text("ユン")),
    (66, TableItem::Text("ホン")),
];
static FLT: D66Table = D66Table::new("カタカナ名字表", D66SortType::Asc, FLT_ITEMS);

/// Ruby `JFTO`（`DiceTable::D66Table`・D66SortType::ASC）。
static JFTO_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("涼太（りょうた）／八重（やえ）")),
    (12, TableItem::Text("蒼（あおい）／雅（みやび）")),
    (13, TableItem::Text("樹（いつき）／凛（りん）")),
    (14, TableItem::Text("蓮（れん）詩（うた）")),
    (15, TableItem::Text("翔（しょう）／舞（まい）")),
    (16, TableItem::Text("翼（つばさ）／鈴（すず）")),
    (22, TableItem::Text("遼（りょう）／瑠華（るか）")),
    (23, TableItem::Text("陽翔（はると）／結菜（ゆな）")),
    (24, TableItem::Text("律（りつ）／莉子（りこ）")),
    (25, TableItem::Text("輝（ひかる）／陽葵（ひまり）")),
    (26, TableItem::Text("仁（じん）／乃愛（のあ）")),
    (33, TableItem::Text("大夢（ひろむ）／阿澄（あすみ）")),
    (34, TableItem::Text("朝陽（あさひ）／結月（ゆづき）")),
    (35, TableItem::Text("大翔（ひろと）／結愛（ゆあ）")),
    (36, TableItem::Text("隼人（はやと）／萌花（もか）")),
    (44, TableItem::Text("公太（こうた）／春歌（はるか）")),
    (45, TableItem::Text("大和（やまと）／澪（みお）")),
    (46, TableItem::Text("拓真（たくま）／奈々（なな）")),
    (55, TableItem::Text("雄大（ゆうだい）／明日香（あすか）")),
    (56, TableItem::Text("悠（ゆう）／彩（あや）")),
    (66, TableItem::Text("秀助（しゅうすけ）／那留（なる）")),
];
static JFTO: D66Table = D66Table::new("日本名前表1", D66SortType::Asc, JFTO_ITEMS);

/// Ruby `JFTT`（`DiceTable::D66Table`・D66SortType::ASC）。
static JFTT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("一郎（いちろう）／かぐや")),
    (12, TableItem::Text("太一（たいち）／さくら")),
    (13, TableItem::Text("颯太（そうた）／あかり")),
    (14, TableItem::Text("瑛斗（えいと）／こはる")),
    (15, TableItem::Text("俊輔（しゅんすけ）／ひなた")),
    (16, TableItem::Text("大地（だいち）／すみれ")),
    (22, TableItem::Text("健太（けんた）／里奈（りな）")),
    (23, TableItem::Text("歩（あゆむ）／春菜（はるな）")),
    (24, TableItem::Text("伊織（いおり）／芽衣（めい）")),
    (25, TableItem::Text("航（わたる）／愛美（あいみ）")),
    (26, TableItem::Text("優希（ゆうき）／綾乃（あやの）")),
    (33, TableItem::Text("直樹（なおき）／茜（あかね）")),
    (34, TableItem::Text("煌（こう）／もも")),
    (35, TableItem::Text("陽向（ひなた）／ひかり")),
    (36, TableItem::Text("将吾（しょうご）／ほのか")),
    (44, TableItem::Text("和也（かずや）／美穂（みほ）")),
    (45, TableItem::Text("巧（たくみ）／未来（みらい）")),
    (46, TableItem::Text("直哉（なおや）／朱里（しゅり）")),
    (55, TableItem::Text("亮（りょう）／瞳（ひとみ）")),
    (56, TableItem::Text("陸人（りくと）／心音（ここね）")),
    (66, TableItem::Text("康平（こうへい）／沙織（さおり）")),
];
static JFTT: D66Table = D66Table::new("日本名前表2", D66SortType::Asc, JFTT_ITEMS);

/// Ruby `FFT`（`DiceTable::D66Table`・D66SortType::ASC）。
static FFT_ITEMS: &[(i64, TableItem)] = &[
    (11, TableItem::Text("カルロ／ビアンカ")),
    (12, TableItem::Text("リアム／オリビア")),
    (13, TableItem::Text("イライジャ／エイヴァ")),
    (14, TableItem::Text("オリバー／ミア")),
    (15, TableItem::Text("ジェームズ／アメリア")),
    (16, TableItem::Text("メイソン／シャーロット")),
    (22, TableItem::Text("オネスト／カルメン")),
    (23, TableItem::Text("ブルーノ／アンネ")),
    (24, TableItem::Text("エーミール／クラーラ")),
    (25, TableItem::Text("ラインハルト／エッダ")),
    (26, TableItem::Text("テオ／リア")),
    (33, TableItem::Text("ロメオ／ルチア")),
    (34, TableItem::Text("セドリック／マリアンヌ")),
    (35, TableItem::Text("コーム／リーズ")),
    (36, TableItem::Text("ギー／カトリーヌ")),
    (44, TableItem::Text("ハオユー／ルーシー")),
    (45, TableItem::Text("ハオラン／イーラン")),
    (46, TableItem::Text("イーチェン／シンイー")),
    (55, TableItem::Text("ウヌ／ハユン")),
    (56, TableItem::Text("ソジュン／ソア")),
    (66, TableItem::Text("ジュウォン／スピン")),
];
static FFT: D66Table = D66Table::new("カタカナ名前表", D66SortType::Asc, FFT_ITEMS);

/// Ruby `TABLES`。
static TABLES: &[(&str, D66Table)] = &[
    ("JLTO", JLTO),
    ("JLTT", JLTT),
    ("FLT", FLT),
    ("JFTO", JFTO),
    ("JFTT", JFTT),
    ("FFT", FFT),
];

// ---------------------------------------------------------------------------
// 判定コマンド
// ---------------------------------------------------------------------------

/// Ruby `/^(\d+)KY(6|10)$/`。
fn ky_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A(\d+)KY(6|10)\z").expect("valid regex"))
}

/// Ruby `KimitoYell#roll_ky_judge`。
fn roll_ky_judge(command: &str, rng: &mut Randomizer) -> Result<Option<EvalResult>, EvalError> {
    let Some(m) = ky_pattern().captures(command) else {
        return Ok(None);
    };

    // d6、d10の設定
    let n_of_diceside: i64 = if &m[2] == "10" { 10 } else { 6 };
    // 振るさいころの数
    let n_of_rolldice: i64 = m[1].parse().unwrap_or(i64::MAX);

    // ダイスを振る
    let dice_list = rng.roll_barabara(n_of_rolldice, n_of_diceside)?;

    // 結果チェック（成功: 4〜10、スペシャル: 6/10、ファンブル: 1）
    let is_special = dice_list.iter().any(|&d| d == 6 || d == 10);
    let is_fumble = dice_list.contains(&1);
    let is_success = dice_list.iter().any(|&d| (4..=10).contains(&d));
    let is_failure = !is_success;

    // 結果用テキストの生成
    let mut result_txts: Vec<&str> = Vec::new();
    if is_success {
        result_txts.push("成功");
    }
    if is_failure {
        result_txts.push("失敗");
    }
    if is_special {
        result_txts.push("スペシャル（がんばりが1点上昇！）");
    }
    if is_fumble {
        result_txts.push("ファンブル（ファンブル表：FTを振る）");
    }

    let mut r = EvalResult::new();
    r.text = format!(
        "({command}) ＞ [{}] ＞ {}",
        join_dice(&dice_list),
        result_txts.join("・")
    );
    r.success = is_success;
    r.failure = is_failure;
    r.critical = is_special;
    r.fumble = is_fumble;
    Ok(Some(r))
}

/// Ruby `KimitoYell#roll_fumble`。
fn roll_fumble(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let fumbledice = rng.roll_once(6)?;
    let fumbletext = FTABLE[(fumbledice - 1) as usize];
    Ok(Some(format!(
        "ファンブル表({command}:{fumbledice}) ＞ {fumbletext}"
    )))
}

/// Ruby `KimitoYell#generate_new_encounter`。
fn generate_new_encounter(
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<String>, EvalError> {
    // 「新しい出会い表（一括生成用もいったんまとめて振る）」
    let table0dice;
    let mut table0: &[&str];
    let mut table0txt;
    let mut table1: Option<&[&str]> = None;
    let mut table2: Option<&[&str]> = None;

    if command.contains("NMTA") || command.contains("NMT") {
        table0 = NMTABLE;
        table0dice = rng.roll_once(6)?;
        table0txt = table0[table0dice as usize];
        match table0dice {
            1 => {
                table1 = Some(MCTABLE);
                table2 = Some(TLTABLE);
            }
            2 => {
                table1 = Some(MCTABLE);
                table2 = Some(EPTABLE);
            }
            3 => {
                table1 = Some(CITABLE);
                table2 = Some(TLTABLE);
            }
            4 => {
                table1 = Some(CITABLE);
                table2 = Some(EPTABLE);
            }
            5 => {
                table1 = Some(HYTABLE);
                table2 = Some(TLTABLE);
            }
            _ => {
                table1 = Some(HYTABLE);
                table2 = Some(EPTABLE);
            }
        }
    } else {
        // その他の表だけ用ダイス
        table0dice = rng.roll_once(10)?;
        table0 = &[];
        table0txt = "";
    }

    // 新しい出会いを求める際のランダム表個別
    if command.contains("MCT") {
        table0 = MCTABLE;
        table0txt = table0[table0dice as usize];
    } else if command.contains("CIT") {
        table0 = CITABLE;
        table0txt = table0[table0dice as usize];
    } else if command.contains("HYT") {
        table0 = HYTABLE;
        table0txt = table0[table0dice as usize];
    } else if command.contains("TLT") {
        table0 = TLTABLE;
        table0txt = table0[table0dice as usize];
    } else if command.contains("EPT") {
        table0 = EPTABLE;
        table0txt = table0[table0dice as usize];
    }

    // 新しい出会い表の一括振り分残りの表決定と結果用テキスト生成
    let resulttxt = if command.contains("NMTA") {
        let table1 = table1.expect("NMTA branch sets table1");
        let table2 = table2.expect("NMTA branch sets table2");
        let table1dice = rng.roll_once(10)?;
        let table1txt = table1[table1dice as usize];
        let table2dice = rng.roll_once(10)?;
        let table2txt = table2[table2dice as usize];

        format!(
            "{}({table0dice}) ＞ {table0txt}\n{}({table1dice}) ＞ {table1txt}\n{}({table2dice}) ＞ {table2txt}",
            table0[0], table1[0], table2[0]
        )
    } else {
        // 一括じゃない場合は表1枚分なので結果用テキスト生成処理まとめて
        format!("{}({table0dice}) ＞ {table0txt}", table0[0])
    };

    Ok(Some(resulttxt))
}

/// Ruby `KimitoYell#generate_new_name`。
fn generate_new_name(command: &str, rng: &mut Randomizer) -> Result<Option<String>, EvalError> {
    let mut result1: Option<String> = None;
    let mut result2: Option<String> = None;
    let mut result3: Option<String> = None;
    let mut result4: Option<String> = None;

    // フルネーム一括生成
    if command.contains("FNG") {
        let nametabledice1 = rng.roll_once(6)?;
        result1 = Some(format!(
            "{}({nametabledice1}) ＞ {}",
            LNTABLE[0], LNTABLE[nametabledice1 as usize]
        ));
        let nametabledice2 = rng.roll_once(6)?;
        result2 = Some(format!(
            "{}({nametabledice2}) ＞ {}",
            FNTABLE[0], FNTABLE[nametabledice2 as usize]
        ));
        result3 = if (1..=2).contains(&nametabledice1) {
            roll_tables("JLTO", rng)?
        } else if (3..=4).contains(&nametabledice1) {
            roll_tables("JLTT", rng)?
        } else {
            roll_tables("FLT", rng)?
        };
        result4 = if (1..=2).contains(&nametabledice1) {
            roll_tables("JFTO", rng)?
        } else if (3..=4).contains(&nametabledice1) {
            roll_tables("JFTT", rng)?
        } else {
            roll_tables("FFT", rng)?
        };
    }

    // 名字表or名前表（その後の表も振る）
    if command.contains("LNT") {
        let nametabledice1 = rng.roll_once(6)?;
        result1 = Some(format!(
            "{}({nametabledice1}) ＞ {}",
            LNTABLE[0], LNTABLE[nametabledice1 as usize]
        ));
        result2 = if (1..=2).contains(&nametabledice1) {
            roll_tables("JLTO", rng)?
        } else if (3..=4).contains(&nametabledice1) {
            roll_tables("JLTT", rng)?
        } else {
            roll_tables("FLT", rng)?
        };
    } else if command.contains("FNT") {
        let nametabledice1 = rng.roll_once(6)?;
        result1 = Some(format!(
            "{}({nametabledice1}) ＞ {}",
            FNTABLE[0], FNTABLE[nametabledice1 as usize]
        ));
        result2 = if (1..=2).contains(&nametabledice1) {
            roll_tables("JFTO", rng)?
        } else if (3..=4).contains(&nametabledice1) {
            roll_tables("JFTT", rng)?
        } else {
            roll_tables("FFT", rng)?
        };
    }

    // 各表単発
    if ["JLTO", "JLTT", "FLT", "JFTO", "JFTT", "FFT"]
        .iter()
        .any(|k| command.contains(k))
    {
        result1 = roll_tables(command, rng)?;
    }

    // 結果表示用テキスト生成
    let resulttxt = if let Some(result4) = result4 {
        format!(
            "{}\n{}\n{}\n{}",
            result1.expect("FNG sets result1"),
            result3.expect("FNG sets result3"),
            result2.expect("FNG sets result2"),
            result4
        )
    } else if let Some(result2) = result2 {
        format!("{}\n{}", result1.expect("LNT/FNT sets result1"), result2)
    } else {
        result1.expect("some name command matched")
    };

    Ok(Some(resulttxt))
}

/// Ruby `dice_list.join(',')`。
fn join_dice(dice: &[i64]) -> String {
    dice.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use crate::eval::eval_command;
    use crate::game_system::GameSystemId;
    use crate::randomizer::SeededRandomizer;
    use crate::toml_test::TestDataFile;
    use std::path::PathBuf;

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    fn toml_path() -> Option<PathBuf> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test/data/KimitoYell.toml");
        path.exists().then_some(path)
    }

    /// `test/data/KimitoYell.toml` の全ケースが通ること。
    ///
    /// 判定項目は `rust/tests/toml_harness.rs::run_case` と同じ
    /// （出力文字列・5フラグ・注入乱数を使い切ったか）。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            // worktree外でクレート単体ビルドされた場合
            eprintln!("skip: test/data/KimitoYell.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("KimitoYell.toml must parse");
        assert_eq!(
            data.tests.len(),
            67,
            "case count in test/data/KimitoYell.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "KimitoYell",
                "unexpected game system in KimitoYell.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("KimitoYell"), &tc.input, &mut src) {
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

            if src.remaining() != 0 {
                reasons.push(format!("unconsumed rands remain ({})", src.remaining()));
            }

            if !reasons.is_empty() {
                failures.push(format!(
                    "FAIL KimitoYell:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} KimitoYell cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
