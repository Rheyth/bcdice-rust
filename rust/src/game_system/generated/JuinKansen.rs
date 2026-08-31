//! P4で手書き移植した `lib/bcdice/game_system/JuinKansen.rb`。
//!
//! メタデータ（id/name/sort_key/help_message/prefixes/settings）は
//! `rust/tools/generate_game_systems.rb` が生成したスタブの値をそのまま保っている。
//! 生成スクリプトを再実行するとこのファイルはスタブへ戻るので注意。
//!
//! 移植したもの:
//! - `JuinKansen#eval_game_system_specific_command`（ALIAS 経由の `roll_tables`）
//!
//! 表データは `i18n/JuinKansen/ja_jp.yml` から機械的に書き出したもので、値は1文字も変えていない。
//! ロケール差のあるデータは表スライスとして束ね、`JuinKansen_Korean` が同じ関数群を使い回す。

use crate::dice_table::{RollableTable, Table};
use crate::eval::EvalError;
use crate::game_system::{GameSystem, SpecificCommandOutput};
use crate::randomizer::Randomizer;

static JA_DAILY_ITEMS: &[&str] = &[
    "「入浴中」『内容』PCの自己紹介後、自宅でどのように入浴しているかについて、簡単に説明せよ。『終了条件』GMが「体を洗い終えたので、そろそろあがることにした」と宣言する。",
    "「自炊」『内容』PCの自己紹介後、自宅でどんな食事を作るか、自炊が苦手かどうかなどを説明せよ。『終了条件』GMが「料理ができたので、食事を始めた」と宣言する。",
    "「休憩時間」『内容』PCの自己紹介後、仕事や学業の休憩時間に、どこで何をしているのか簡単に説明せよ。『終了条件』GMが「そろそろ、休憩時間が終わりそうだ」と宣言する。",
    "「帰宅中」『内容』PCの自己紹介後、仕事や学業から帰宅途中、何をしているかを簡単に説明せよ。『終了条件』GMが自宅に到着したと宣言する。",
    "「昼休み」『内容』PCの自己紹介後、仕事や学業の昼休み中、どこでなにを食べているのかを簡単に説明せよ。『終了条件』GMが昼休みがそろそろ終わると宣言する。",
    "「仕事中」『内容』PCの自己紹介後、仕事や学業に対して、どのような態度で臨んでいるのかを簡単に説明せよ。『終了条件』GMが休憩時間を知らせるチャイムなどが鳴ったと宣言する。",
    "「通勤・通学」『内容』PCの自己紹介後、どのような方法で通勤・通学しているかを簡単に説明せよ。『終了条件』GMが職場や学校に到着したと宣言する。",
    "「休日」『内容』PCの自己紹介後、休日に何をしているか簡単に説明せよ。『終了条件』GMが「そろそろ、明日に備えなければ」と宣言する。",
    "「自宅」『内容』PCの自己紹介後、自宅の自室で、どのようにくつろいでいるかを簡単に説明せよ。『終了条件』GMが「そろそろ、寝る時間のようだ」と宣言する。",
    "「寝起き直後」『内容』PCの自己紹介後、自宅で起床直後、どのような行動を取るかについて、簡単に説明せよ。『終了条件』GMが「出勤・通学の準備が整った」と宣言する。",
    "「趣味」『内容』PCの自己紹介後、自室でどんな趣味に没頭しているかを簡単に説明せよ。『終了条件』GMが「名残惜しいが、そろそろ寝る時間のようだ」と宣言する。",
];
static JA_DAILY: Table = Table::from_dice("日常表", 2, 6, JA_DAILY_ITEMS);

static JA_PLACECITY_ITEMS: &[&str] = &[
    "「事件現場」ここは以前、怪事件が起こったと噂される事件現場だ。よく見ると、近くには枯れた花束が置かれている。",
    "「帰宅路」ここは帰宅路だ。時間帯のせいか周囲に人気はなく、耳鳴りがするほどの静寂に包まれている。",
    "「近所の公園」ここは自宅・会社・学校いずれかの近くにある公園だ。今いる場所は、奥のあずまやで、近くには自販機が並んでいる。",
    "「近所のコンビニ」ここはPCの自宅近くにある、コンビニエンスストアだ。大きな駐車場のせいか、周囲は実に広々としている。",
    "「飲食店」ここはどこにでもある飲食店だ。しかし、客の入りはまばらで、周囲には空席が目立つ。",
    "「駅の広場」ここは人々が通勤や通学に使う駅の広場だ。が、行き交う人々の視線はどこか冷たく、虚ろにさえ見える。",
    "「神社仏閣」ここは街のなかにある神社・寺社だ。頼めばお祓いをしてくれるという話だが、ご利益があるかは不明だ。",
    "「図書館」ここは街中にある公立の図書館だ。周囲には新旧の様々な書籍が並び、調べものをするにはうってつけだ。",
    "「河川敷の道」ここは近所にある河川敷沿いの道だ。周囲に人気はなく、無機質な道路が遠くまで続いてる。",
    "「病院の前」ここは近所にある総合病院の前だ。通院患者と思しき老人や若者が、時折、真新しい出入り口を行き来している。",
    "「警察署の前」ここは街中にある警察署の前だ。今回の事件、あるいは先ほどの怪現象を話しても……おそらく、信じてはくれないだろう。",
];
static JA_PLACECITY: Table = Table::from_dice("場所表「都市」", 2, 6, JA_PLACECITY_ITEMS);

static JA_PLACECOUNTRYSIDE_ITEMS: &[&str] = &[
    "「事件現場」ここは以前、怪事件が起こったと噂される事件現場だ。よく見ると、近くには枯れた花束が置いてある。",
    "「学校の前」ここは村の奥にある学校の前だ。しかし、何年も前に廃校したらしく、門は固く閉ざされ、校庭には雑草が生い茂っている。",
    "「公園」ここは村の中にある公園だ。中央にたたずむ遊具はいずれも錆びついており、所々に雑草が生えている。",
    "「村役場」ここは村にある役場だ。しかし、役場のドアは締め切られたままで、なかに人の気配はない。",
    "「森の中」ここは道路から離れた場所にある森の中だ。周囲からは、動物と思しき鳴き声が聞こえてくる。",
    "「バス停」ここは村唯一のバス停だ。時刻表を見てみるが、バスは一日に一本しか通ってないらしい。",
    "「神社仏閣」ここは村の中にある神社・寺社だ。境内はさびれており、人の気配は皆無に等しい。",
    "「田畑」ここは村の外れにある田畑だ。農閑期のせいか、畑は放置されたままで、周囲を見渡しても人の姿はない。",
    "「川辺」ここは村の外れにある川の近くだ。目前の川は透き通っており、何匹も魚が泳いでいる。",
    "「病院の前」ここは村にある小さな病院の前だ。しかし、今日は定休日なのか、病院は閉まったままだ。",
    "「交番の前」ここは村唯一の交番だ。しかし、警察官の姿はなく、ドアも締め切られたままだ。",
];
static JA_PLACECOUNTRYSIDE: Table =
    Table::from_dice("場所表「田舎」", 2, 6, JA_PLACECOUNTRYSIDE_ITEMS);

static JA_PLACEFACILITY_ITEMS: &[&str] = &[
    "「礼拝堂」ここは地下に存在する、礼拝堂と思しき場所だ。部屋の奥には奇妙な姿をした石像があり、周囲には瘴気が漂っている。",
    "「隠し部屋」ここは隠し部屋と思しき場所だ。部屋の壁には、血痕のような汚れが染みついている。",
    "「倉庫」ここは中心部に存在する倉庫だ。場所のせいか、はたまた異変のせいか、周囲には不気味なカビが生えている。",
    "「寝台」ここは寝台のある場所だ。内部には簡素なベッドと机が置かれているだけで、非常に殺風景だ。",
    "「応接室」ここは応接室と思しき場所だ。部屋の中央には、大きめのテーブルとソファが置いてある。",
    "「廊下」ここは施設内を通る廊下の1つだ。伝統がついていないせいか、それとも時間帯のせいか、周囲はやけに薄暗い。",
    "「ロビー」ここはロビーだ。扉は相変わらず開く気配がなく、周囲は静まり返っている。",
    "「踊り場」ここは階段の踊り場だ。周囲は非常に薄暗く、これといって何もない。",
    "「地下室」ここは地下室だ。床にはうっすらと埃が積もり、長年使われていないように思える。",
    "「実験室」ここは実験室と思しき場所だ。中央には大きなテーブルが設置されており、どういうわけか血まみれだ。",
    "「研究室」ここは研究室と思しき場所だ。部屋は全体的に暗く、奥の棚には奇妙な姿をした生物のホルマリン漬けが並んでる。",
];
static JA_PLACEFACILITY: Table = Table::from_dice("場所表「施設内」", 2, 6, JA_PLACEFACILITY_ITEMS);

static JA_FIRSTLOOK_ITEMS: &[&str] = &[
    "「襲撃からの救出」『内容』PC同士が再会後、【命運】の低いPCが突如、不良などに絡まれ、【命運】の高いPCが実力や機転で救う場面を演出せよ。『終了条件』【命運】の低いPCが、【命運】の高いPCに対して感謝の言葉を述べる。",
    "「落とし物」『内容』PC同士が再会後、【幸運】の低いPCが荷物を落とし、それを【幸運】の高いPCが拾い、手渡す場面を演出せよ。『終了条件』【幸運】の低いPCが【幸運】の高いPCの[呪印]に気付く。",
    "「黒い影」『内容』PC同士が再会後、【精神】の高いPCが、【精神】の低いPCの背後に黒い影を目撃する場面を演出せよ。『終了条件』【精神】の高いPCが「さっきの……なに？」と問う。",
    "「<なにか>を見た」『内容』PC同士が再会後、【幸運】の低いPCが暗がりの奥で<なにか>を目撃。顔面蒼白となる場面を演出せよ。『終了条件』【幸運】の高いPCが【幸運】の低いPCを気遣う。",
    "「呼び声」『内容』PC同士は再会後、遠くから不気味な呼び声を聞き、周囲を見回すうちに、目が合う場面を演出せよ。『終了条件』PCのいずれかが「今の……聞こえた？」と問いかけ、いずれかが返答する。",
    "「知り合い」『内容』PC同士が再会後、実は知り合いの知り合いだったことが判明する場面を演出せよ。『終了条件』PC同士がどんな関係かについて相談・決定する。",
    "「事故からの救出」『内容』落下物や交通事故など、【希望】の低いPCが突如、不可解な事故に巻き込まれ、【希望】の高いPCに救われる場面を演出せよ。『終了条件』【希望】の低いPCが、【希望】の高いPCに感謝の言葉を述べる。",
    "「SNS」『内容』【知性】の高いPCがSNS上で事件に関する発信を行い、【知性】の低いPCと知り合う場面を演出せよ。『終了条件』【知性】の低いPCが「また今度、お会いしましょう」と返信する。",
    "「<なにか>が見える」『内容』PC同士が再会後、暗がりの向こうで<なにか>を目撃、戦慄する場面を演出せよ。『終了条件』<なにか>が消え去り、PCのいずれかが「……さっきの見た？」と問う。",
    "「介抱」『内容』【体力】の低いPCが突如、<なにか>に襲われる幻覚を見て取り乱し、偶然現れた【体力】の高いPCに助けられる場面を演出せよ。『終了条件』【体力】の低いPCが感謝の言葉を述べる。",
    "「呪印」『内容』PC同士が再会後、【敏捷】の高いPCが、【敏捷】の低いPCの持つ[呪印]に気付く場面を演出せよ。『終了条件』【敏捷】の高いPCが自身にも同じものがあると告げる。",
];
static JA_FIRSTLOOK: Table = Table::from_dice("初見表", 2, 6, JA_FIRSTLOOK_ITEMS);

static JA_APPRECIATIVEFRIEND_ITEMS: &[&str] = &[
    "「大切な人」『内容』PC同士が再会後、事件に関する相談をするうちに、PCが互いに、自身の[大切な人]について語る場面を演出せよ。『終了条件』いずれかのPCが「なんにせよ、その人のためにも、生き残らないといけませんね」と言う。",
    "「個人の印象」『内容』PC同士が出会い、事件の相談をするうちに、いずれかが片方のPCの印象や人柄について、感想を述べる場面を演出せよ。『終了条件』感想を言われたPCが「そうかな？」と言う。",
    "「呪印について」『内容』PC同士が出会い、お互いの体に刻まれた[呪印]の正体について考える場面を演出せよ。『終了条件』いずれかのPCが、「……結局、謎だらけだということですね。また考えましょうか」と言う。",
    "「不安と恐れ」『内容』PC同士が出会い、相談をするうちに【命運】の低いPCが不安を抱き、【命運】の高いPCが気遣う場面を演出せよ。『終了条件』【命運】の低いPCが「大丈夫です」と返す。",
    "「情報交換」『内容』PC同士が出会い、現状を整理するために、情報交換を行う場面を演出せよ。『終了条件』いずれかのPCが「……また、新しい情報が手に入ったら連絡します」と言う。",
    "「過去と秘密」『内容』PC同士が出会い、事件の相談を行った際、いずれかのPCが[関係性]を結んだ≪過去や秘密≫について語る。『終了条件』≪過去や秘密≫を語ったPCが「巻き込んでしまって、すいません」と言う。",
    "「推理」『内容』PC同士が出会い、今回の事件の解決方法について、推理する場面を演出せよ。『終了条件』いずれかのPCが「もっと詳しく調べてみましょう。きっと、なにか良い方法があるはずです」と言う。",
    "「協力関係」『内容』PC同士が出会い、いずれかのPCが今回の事件を生き抜くために、より協力関係を強めようと、提案する場面を演出せよ。『終了条件』提案されたほうのPCが、提案に承諾する。",
    "「願いについて」『内容』PC同士が出会い、事件について推理するうちに、いずれかのPCが自身の[願い]について語る場面を演出せよ。『終了条件』[願い]を知ったPCが、それに対して個人的な感想を述べる。",
    "「作戦会議」『内容』PC同士が出会い、【知性】の低いPCが【知性】の高いPCに良い知恵はないかと相談する場面を演出せよ。『終了条件』【知性】の高いPCが「……上手くいくかわかりませんが、ある方法を試してみます」と言う。",
    "「方法はあるはずだ」『内容』PC同士が出会い、いずれかのPCが「生き残る方法は、必ずあるはずだ」と鼓舞する場面を演出せよ。『終了条件』鼓舞されたPCが、「そうですね……その方法を探してみましょう」と言う。",
];
static JA_APPRECIATIVEFRIEND: Table = Table::from_dice("知己表", 2, 6, JA_APPRECIATIVEFRIEND_ITEMS);

static JA_FORESHADOW_ITEMS: &[&str] = &[
    "PCとの会話のなかで、ふと閃くものがあった。",
    "謎の金属片が落ちていた。胸ポケットにしまっておこう。",
    "会話の最中……突如、ある真実に気づいてしまった。",
    "PCからの激励を耳にして、やけに気合が入った。",
    "不安な情報を耳にした瞬間、大切な人の顔が浮かんだ。",
    "そのとき、不思議な力が身のうちに宿った。",
];
static JA_FORESHADOW: Table = Table::from_dice("伏線表", 1, 6, JA_FORESHADOW_ITEMS);

static JA_EMOTION_ITEMS: &[&str] = &["尊敬", "好意", "友情", "庇護", "信頼", "安心"];
static JA_EMOTION: Table = Table::from_dice("感情表", 1, 6, JA_EMOTION_ITEMS);

static JA_SITUATION_ITEMS: &[&str] = &[
    "「不運の連鎖」事件が関係しているのか、PCたちの周囲で次々と不幸が起こり始めている。なんとかして、この連鎖を止めなければ……。『使用可能能力値』【幸運】",
    "「見えない恐怖」事件が発生してから、PCたちは誰かから監視されているような気がする。こんなときこそ、気を強く持たなければ……。『使用可能能力値』【精神】",
    "「冷静な行動」立て続けに起こる怪奇現象のせいで、PCたちはやや混乱気味だ。しかし、こんなときこそ冷静に知恵を絞らねば……。『使用可能能力値』【知性】",
    "「震えよ、止まれ」迫りくる恐怖のせいで、PCたちの心身は萎縮し、衰弱している。だが、こんなときこそ冷静に行動しなければ……。『使用可能能力値』【敏捷】",
    "「危機はすぐそこ」得体のしれない<なにか>は、間違いなくそこまで迫ってきている。頼れるのはもはや、自分たちの肉体だけだ……。『使用可能能力値』【体力】",
    "「運が味方」状況は逼迫しつつある。だが、幸運にも状況がPCたちに味方し始めた。なんとしてでも、このチャンスを活かさねば……。『使用可能能力値』各PCの任意の【能力値】",
];
static JA_SITUATION: Table = Table::from_dice("状況表", 1, 6, JA_SITUATION_ITEMS);

static JA_TARGET_ITEMS: &[&str] = &[
    "【体力】が最も高い順",
    "【敏捷】が最も高い順",
    "【知性】が最も高い順",
    "【精神】が最も高い順",
    "【幸運】が最も高い順",
    "【希望】が最も低い順",
];
static JA_TARGET: Table = Table::from_dice("対象表", 1, 6, JA_TARGET_ITEMS);

static JA_INSANITY_ITEMS: &[&str] = &[
    "「逃げたい」次[フェイズ]の終了を迎えるまで、PCは恐怖のため、その場から一刻も早く立ち去りたいという衝動に駆られる様子を演出せよ。",
    "「記憶の混濁」次[フェイズ]の終了を迎えるまで、PCはショックのあまり、見たものや聞いたものが何だったかを思い出せない様子を演出せよ。",
    "「興奮状態」次[フェイズ]の終了を迎えるまで、PCは恐怖のあまり、些細なことでイライラし、PCに辛くあたった後、「……ごめんなさい」と、後悔する状況を演出せよ。",
    "「怯えている」次[フェイズ]の終了を迎えるまで、PCは恐怖のあまり、先ほどから怯え続けている状況を演出せよ。",
    "「緊張状態」次[フェイズ]の終了を迎えるまで、PCは極度の緊張状態で体の自由が利かず、転んだり、萎縮したりする状況を演出せよ。",
    "「一時的発狂」次[フェイズ]の終了を迎えるまで、PCは衝撃のあまり、一時的に発狂してしまい、うわごとを何度も何度も繰り返してしまう状況を演出せよ。",
];
static JA_INSANITY: Table = Table::from_dice("狂気表", 1, 6, JA_INSANITY_ITEMS);

static JA_DEATH_ITEMS: &[&str] = &[
    "「猟奇殺人」その後、PCは郊外の廃屋のなかでバラバラ死体となって発見された。警察は懸命に捜査を行ったが、結局、手がかりは見つからず、事件は迷宮入りした。",
    "「変死」その後、PCが自室で変死しているのが発見された。PCの死因は出血多量。出血を引き起こした外傷は、まるで大きな獣に噛まれたような痕だった。",
    "「自殺」その後、PCが郊外に放置された廃車のなかで、自殺しているのが発見された。だが、遺書は見つかっておらず、その理由は最後までわからずじまいであった。",
    "「衰弱死」その後、PCは突如として体調を崩し、入院を余儀なくされた。検査を繰り返したものの、原因は不明のまま……やがて、PCは体重が半減して、衰弱死した。",
    "「事故死」その後、PCは不慮の事故に遭い、この世を去った。だが、遺体解剖の結果、事故による外傷ではなく、直前に内臓破裂を起こしていたことが判明した。",
    "「行方不明」その後、PCは行方不明となった。家族はPCの捜索届を出したが、結局、最後までPCが見つかることはなかった。",
];
static JA_DEATH: Table = Table::from_dice("終焉表", 1, 6, JA_DEATH_ITEMS);

static JA_FEAR_ITEMS: &[&str] = &[
    "「不気味なモノ」血液、人骨のようなもの、大量の粘液など……奇妙なモノが突如、足元に落ちてきた。[効果算出]PC全員に5+[呪印÷2]d。",
    "「早まる鼓動」突然のチャイム、突然の電話、突然の呼び声、突然の物音……不安のせいか、ただそれだけで、鼓動が早まる。[効果算出]PC3体に4+[呪印÷2]d。",
    "「不安の種」無言電話、差出人不明のメッセージ、鍵穴についた深い傷、排水溝の長い髪の毛など……不安になるものを見つけてしまう。[効果算出]PC1体に4+[呪印÷2]d。",
    "「異臭」生臭い匂い、鉄臭い匂い、花のような芳香など……普段嗅がない異臭が、一瞬だけ鼻孔をついた。[効果算出]PC2体に3+[呪印÷2]d。",
    "「隙間」建物の影、ベッドの下、ふすまの隙間など……様々な死角から<なにか>の息遣い、気配が漂う。[効果算出]PC2体に2+[呪印÷2]d。",
    "「視線」どこからか、強い視線を感じる……だが、そちらに目をやると、気配はすぐに消えてしまった。[効果算出]PC2体に2+[呪印÷2]d。",
    "「異音」遠くから不思議な金属音や鐘の音、肉を潰すような音や粘着質な音などが聞こえた気がした。[効果算出]PC2体に3+[呪印÷2]d。",
    "「接触」つい先ほど……なにかに触られたような、あるいは服を引っ張られたような気がした。[効果算出]PC3体に3+[呪印÷2]d。",
    "「さっきのは？」隣を通り過ぎた人物、声をかけてきた人物、何かを渡してきた人物……しかし、ここには2人以外、誰もいないはずだ。[効果算出]PC2体に4+[呪印÷2]d。",
    "「見てはいけない」次の行動に移ろうとしたそのとき、一瞬、通路の向こうに<なにか>が佇んでいるような気がした。[効果算出]PC1体に5+[呪印÷2]d。",
    "「暗闇のなかへ」PCは突如として<なにか>に手足をつかまれ、暗闇へと引きずり込まれそうになる！[効果算出]PC1体に6+[呪印÷2]d。",
];
static JA_FEAR: Table = Table::from_dice("恐怖表", 2, 6, JA_FEAR_ITEMS);

pub(crate) static JA_TABLES: &[(&str, &dyn RollableTable)] = &[
    ("DAILY", &JA_DAILY),
    ("PLACECITY", &JA_PLACECITY),
    ("PLACECOUNTRYSIDE", &JA_PLACECOUNTRYSIDE),
    ("PLACEFACILITY", &JA_PLACEFACILITY),
    ("FIRSTLOOK", &JA_FIRSTLOOK),
    ("APPRECIATIVEFRIEND", &JA_APPRECIATIVEFRIEND),
    ("FORESHADOW", &JA_FORESHADOW),
    ("EMOTION", &JA_EMOTION),
    ("SITUATION", &JA_SITUATION),
    ("TARGET", &JA_TARGET),
    ("INSANITY", &JA_INSANITY),
    ("DEATH", &JA_DEATH),
    ("FEAR", &JA_FEAR),
];

/// Ruby `JuinKansen::ALIAS`。
pub(crate) fn resolve_alias(command: &str) -> &str {
    match command {
        "DAI" => "DAILY",
        "PCI" => "PLACECITY",
        "PCO" => "PLACECOUNTRYSIDE",
        "PFA" => "PLACEFACILITY",
        "FL" => "FIRSTLOOK",
        "AF" => "APPRECIATIVEFRIEND",
        "FOR" => "FORESHADOW",
        "EMO" => "EMOTION",
        "SIT" => "SITUATION",
        "TAR" => "TARGET",
        "INS" => "INSANITY",
        "DEA" => "DEATH",
        "FEA" => "FEAR",
        other => other,
    }
}

/// Ruby `JuinKansen#eval_game_system_specific_command`。
pub(crate) fn eval_specific_command(
    tables: &'static [(&'static str, &'static dyn RollableTable)],
    command: &str,
    rng: &mut Randomizer,
) -> Result<Option<SpecificCommandOutput>, EvalError> {
    let key = resolve_alias(command);
    let Some((_, table)) = tables.iter().find(|(k, _)| *k == key) else {
        return Ok(None);
    };
    Ok(Some(SpecificCommandOutput::text(
        table.roll(rng)?.to_string(),
    )))
}

/// Ruby `BCDice::GameSystem::JuinKansen`（ID: `JuinKansen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JuinKansen;

impl GameSystem for JuinKansen {
    fn id(&self) -> &'static str {
        "JuinKansen"
    }

    fn name(&self) -> &'static str {
        "呪印感染"
    }

    fn sort_key(&self) -> &'static str {
        "しゆいんかんせん"
    }

    fn help_message(&self) -> &'static str {
        r"■ 表
  ・日常表 (DAI, Daily)
  ・場所表
    ・「都市」 (PCI, PlaceCity)
    ・「田舎」 (PCO, PlaceCountryside)
    ・「施設内」 (PFA, PlaceFacility)
  ・初見表 (FL, FirstLook)
  ・知己表 (AF, AppreciativeFriend)
  ・伏線表 (FOR, Foreshadow)
  ・感情表 (EMO, Emotion)
  ・状況表 (SIT, Situation)
  ・対象表 (TAR, Target)
  ・狂気表 (INS, Insanity)
  ・終焉表 (DEA, Death)
  ・恐怖表 (FEA, Fear)
"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[
            "DAILY",
            "PLACECITY",
            "PLACECOUNTRYSIDE",
            "PLACEFACILITY",
            "FIRSTLOOK",
            "APPRECIATIVEFRIEND",
            "FORESHADOW",
            "EMOTION",
            "SITUATION",
            "TARGET",
            "INSANITY",
            "DEATH",
            "FEAR",
            "DAI",
            "PCI",
            "PCO",
            "PFA",
            "FL",
            "AF",
            "FOR",
            "EMO",
            "SIT",
            "TAR",
            "INS",
            "DEA",
            "FEA",
        ]
    }

    crate::impl_prefixes_pattern!();

    fn eval_game_system_specific_command(
        &self,
        command: &str,
        rng: &mut Randomizer,
    ) -> Result<Option<SpecificCommandOutput>, EvalError> {
        eval_specific_command(JA_TABLES, command, rng)
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
            .join("test/data/JuinKansen.toml");
        path.exists().then_some(path)
    }

    fn check_flag(reasons: &mut Vec<String>, name: &str, expected: bool, actual: bool) {
        if expected != actual {
            reasons.push(format!(
                "{name} flag mismatch: expected {expected}, actual {actual}"
            ));
        }
    }

    /// `test/data/JuinKansen.toml` の全ケースが通ること。
    #[test]
    fn all_toml_cases_pass() {
        let Some(path) = toml_path() else {
            eprintln!("skip: test/data/JuinKansen.toml not found");
            return;
        };

        let data = TestDataFile::load(&path).expect("JuinKansen.toml must parse");
        assert_eq!(
            data.tests.len(),
            52,
            "case count in test/data/JuinKansen.toml"
        );

        let mut failures: Vec<String> = Vec::new();
        for (i, tc) in data.tests.iter().enumerate() {
            assert_eq!(
                tc.game_system, "JuinKansen",
                "unexpected game system in JuinKansen.toml"
            );

            let mut reasons: Vec<String> = Vec::new();
            let rands: Vec<(i64, i64)> = tc.rands.iter().map(|r| (r.value, r.sides)).collect();
            let mut src = SeededRandomizer::new(rands);

            match eval_command(&GameSystemId::new("JuinKansen"), &tc.input, &mut src) {
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
                    "FAIL JuinKansen:{}:{}\n  - {}",
                    i + 1,
                    tc.input,
                    reasons.join("\n  - ")
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{}/{} JuinKansen cases failed:\n{}",
            failures.len(),
            data.tests.len(),
            failures.join("\n")
        );
    }
}
