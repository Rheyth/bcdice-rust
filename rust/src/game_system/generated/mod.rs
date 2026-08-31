//! Ruby本家のメタデータから自動生成したゲームシステム群。
//!
//! 中身は `lib/bcdice/game_system/*.rb` の**定数と設定フラグだけ**を写したスタブで、
//! `eval_game_system_specific_command` の本体とダイス表はまだ無い（P4で個別移植する）。
//! 接頭辞を持つシステムは、接頭辞にマッチした入力に対して
//! [`EvalError::NotImplemented`](crate::eval::EvalError::NotImplemented) を返す。
//! 既定の `Ok(None)` にすると未実装の固有コマンドが黙って汎用コマンドへ
//! フォールスルーし、誤った出力を返してしまうため。
//!
//! # 再生成
//!
//! ```sh
//! # 1. メタデータ抽出（docker ruby:3.2。手順は docs/rust_port_plan.md の P3 節）
//! ruby -Ilib rust/tools/extract_game_systems.rb .scratch/game_systems.json
//! # 2. Rustソース生成（ホストで実行可）
//! ruby rust/tools/generate_game_systems.rb .scratch/game_systems.json
//! ```
//!
//! このファイルは「手書き部分（マクロ定義まで）」と「生成部分」に分かれており、
//! 生成スクリプトは目印コメントより下だけを書き換える。マクロを直すときは
//! 目印より上を編集すること。
//!
//! # 命名
//!
//! 型名・モジュール名は Ruby のクラス名をそのまま使う（`lib/bcdice/game_system/` の
//! ファイル名と1対1で対応させ、移植状況を追えるようにするため）。
//! IDとクラス名は64件で食い違うので（ID `Arianrhod:Korean` → クラス名
//! `Arianrhod_Korean`）、[`GameSystem::id`](crate::game_system::GameSystem::id) は
//! 元のIDを返す。Rustの命名規則から外れる名前になるため、この生成物に限って
//! `non_snake_case` / `non_camel_case_types` を許可する。

#![allow(non_snake_case, non_camel_case_types)]

/// 生成されたゲームシステム1件分の `impl GameSystem` を展開する。
///
/// `prefixes` が空かどうかで2つのアームに分かれる。
/// 空のシステムは Ruby `Base` と同一挙動（固有コマンドを持たない）なので
/// `prefixes` 関連と `eval_game_system_specific_command` を一切上書きしない。
///
/// ```ignore
/// impl_generated_system! {
///     Cthulhu7th,
///     id: "Cthulhu7th",
///     name: "新クトゥルフ神話TRPG",
///     sort_key: "しんくとうるふしんわTRPG",
///     help_message: r"...",
///     prefixes: ["CC", "CBR"],
///     settings: { sort_barabara_dice: true, },
/// }
/// ```
#[macro_export]
macro_rules! impl_generated_system {
    // --- 固有コマンドを持たないシステム（Ruby側で register_prefix していない） ---
    (
        $ty:ident,
        id: $id:literal,
        name: $name:literal,
        sort_key: $sort_key:literal,
        help_message: $help:expr,
        prefixes: [],
        settings: { $($settings:tt)* } $(,)?
    ) => {
        #[doc = concat!("Ruby `BCDice::GameSystem::", stringify!($ty), "`（ID: `", $id, "`）。")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $ty;

        impl $crate::game_system::GameSystem for $ty {
            $crate::__generated_system_constants!($id, $name, $sort_key, $help);
            $crate::__generated_system_settings!($($settings)*);
        }
    };

    // --- 固有コマンドを持つシステム ---
    (
        $ty:ident,
        id: $id:literal,
        name: $name:literal,
        sort_key: $sort_key:literal,
        help_message: $help:expr,
        prefixes: [$($prefix:literal),+ $(,)?],
        settings: { $($settings:tt)* } $(,)?
    ) => {
        #[doc = concat!("Ruby `BCDice::GameSystem::", stringify!($ty), "`（ID: `", $id, "`）。")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $ty;

        impl $crate::game_system::GameSystem for $ty {
            $crate::__generated_system_constants!($id, $name, $sort_key, $help);

            fn prefixes(&self) -> &'static [&'static str] {
                &[$($prefix),+]
            }

            $crate::impl_prefixes_pattern!();

            /// P4で個別移植するまで、固有コマンドは評価できない。
            fn eval_game_system_specific_command(
                &self,
                _command: &str,
                _rng: &mut $crate::randomizer::Randomizer,
            ) -> ::core::result::Result<
                ::core::option::Option<$crate::game_system::SpecificCommandOutput>,
                $crate::eval::EvalError,
            > {
                ::core::result::Result::Err($crate::eval::EvalError::NotImplemented)
            }

            $crate::__generated_system_settings!($($settings)*);
        }
    };
}

/// [`impl_generated_system!`] の内部ヘルパ。Ruby のクラス定数を展開する。
#[doc(hidden)]
#[macro_export]
macro_rules! __generated_system_constants {
    ($id:literal, $name:literal, $sort_key:literal, $help:expr) => {
        fn id(&self) -> &'static str {
            $id
        }
        fn name(&self) -> &'static str {
            $name
        }
        fn sort_key(&self) -> &'static str {
            $sort_key
        }
        fn help_message(&self) -> &'static str {
            $help
        }
    };
}

/// [`impl_generated_system!`] の内部ヘルパ。
///
/// Ruby `Base#initialize` の既定値と異なる設定だけを上書きする。
/// 各項目は省略可能だが、**順序はこの定義どおりに並べる**こと
/// （`macro_rules!` の省略可能グループは順序どおりにしか照合できない）。
/// 生成スクリプトはこの順序で出力する。
#[doc(hidden)]
#[macro_export]
macro_rules! __generated_system_settings {
    (
        $(sort_add_dice: $sort_add_dice:expr,)?
        $(sort_barabara_dice: $sort_barabara_dice:expr,)?
        $(d66_sort_type: $d66_sort_type:expr,)?
        $(enabled_d9: $enabled_d9:expr,)?
        $(round_type: $round_type:expr,)?
        $(sides_implicit_d: $sides_implicit_d:expr,)?
        $(upper_dice_reroll_threshold: $upper_dice_reroll_threshold:expr,)?
        $(reroll_dice_reroll_threshold: $reroll_dice_reroll_threshold:expr,)?
        $(default_cmp_op: $default_cmp_op:expr,)?
        $(default_target_number: $default_target_number:expr,)?
        $(enabled_upcase_input: $enabled_upcase_input:expr,)?
    ) => {
        $(fn sort_add_dice(&self) -> bool { $sort_add_dice })?
        $(fn sort_barabara_dice(&self) -> bool { $sort_barabara_dice })?
        $(fn d66_sort_type(&self) -> $crate::enums::D66SortType { $d66_sort_type })?
        $(fn enabled_d9(&self) -> bool { $enabled_d9 })?
        $(fn round_type(&self) -> $crate::enums::RoundType { $round_type })?
        $(fn sides_implicit_d(&self) -> i64 { $sides_implicit_d })?
        $(fn upper_dice_reroll_threshold(&self) -> ::core::option::Option<i64> {
            ::core::option::Option::Some($upper_dice_reroll_threshold)
        })?
        $(fn reroll_dice_reroll_threshold(&self) -> ::core::option::Option<i64> {
            ::core::option::Option::Some($reroll_dice_reroll_threshold)
        })?
        $(fn default_cmp_op(&self) -> ::core::option::Option<$crate::normalize::CmpOp> {
            ::core::option::Option::Some($default_cmp_op)
        })?
        $(fn default_target_number(&self) -> ::core::option::Option<i64> {
            ::core::option::Option::Some($default_target_number)
        })?
        $(fn enabled_upcase_input(&self) -> bool { $enabled_upcase_input })?
    };
}

// ============================================================================
// ここから下は rust/tools/generate_game_systems.rb が生成する。手で編集しない。
// ============================================================================

pub mod AFF2e;
pub mod AceKillerGene;
pub mod Agnostos;
pub mod Ainecadette;
pub mod Aionia;
pub mod Airgetlamh;
pub mod Airgetlamh_Korean;
pub mod AlchemiaStruggle;
pub mod Alsetto;
pub mod Alsetto_Korean;
pub mod Alshard;
pub mod AlterRaise;
pub mod Amadeus;
pub mod Amadeus_Korean;
pub mod AngelGear;
pub mod AniMalus;
pub mod AnimaAnimus;
pub mod AnimaAnimus_Korean;
pub mod Aoharubaan;
pub mod Arianrhod;
pub mod Arianrhod_Korean;
pub mod ArknightsFan;
pub mod ArsMagica;
pub mod AssaultEngine;
pub mod Avandner;
pub mod Ayabito;
pub mod BBN;
pub mod BadLife;
pub mod Bakenokawa;
pub mod BarnaKronika;
pub mod BattleTech;
pub mod BeastBindTrinity;
pub mod BeginningIdol;
pub mod BeginningIdol2022;
pub mod BeginningIdol_Korean;
pub mod BlackJacket;
pub mod BlackJacket_Korean;
pub mod BladeOfArcana;
pub mod BlindMythos;
pub mod BloodCrusade;
pub mod BloodMoon;
pub mod Bloodorium;
pub mod Bloodorium_Korean;
pub mod CardRanker;
pub mod CastleInGray;
pub mod ChaosFlare;
pub mod CharonSanctions;
pub mod Chill;
pub mod Chill3;
pub mod ChroniclesOfDarkness2e;
pub mod CodeLayerd;
pub mod ColossalHunter;
pub mod Comes;
pub mod ConvictorDrive;
pub mod CrashWorld;
pub mod Cthulhu;
pub mod Cthulhu7th;
pub mod Cthulhu7th_ChineseTraditional;
pub mod Cthulhu7th_Korean;
pub mod CthulhuTech;
pub mod Cthulhu_ChineseTraditional;
pub mod Cthulhu_English;
pub mod Cthulhu_Korean;
pub mod Cthulhu_SimplifiedChinese;
pub mod CyberpunkRed;
pub mod CyberpunkRed_Korean;
pub mod DarkBlaze;
pub mod DarkDaysDrive;
pub mod DarkSouls;
pub mod DeadlineHeroes;
pub mod DeadlineHeroes_Korean;
pub mod DemonParasite;
pub mod DemonSpike;
pub mod DesperateRun;
pub mod DetatokoSaga;
pub mod DetatokoSaga_Korean;
pub mod DiceOfTheDead;
pub mod DivineCharger;
pub mod DoubleCross;
pub mod DoubleCross_Korean;
pub mod Dracurouge;
pub mod Dracurouge_Korean;
pub mod DungeonsAndDragons;
pub mod DungeonsAndDragons5;
pub mod DungeonsAndDragons5_Korean;
pub mod DungeonsAndDragons_Korean;
pub mod EarthDawn;
pub mod EarthDawn3;
pub mod EarthDawn4;
pub mod EclipsePhase;
pub mod EdgeFlippers;
pub mod Elric;
pub mod Elysion;
pub mod EmbryoMachine;
pub mod Emoklore;
pub mod Emoklore_Korean;
pub mod EndBreaker;
pub mod EtrianOdysseySRS;
pub mod FateCoreSystem;
pub mod Fiasco;
pub mod Fiasco_Korean;
pub mod FilledWith;
pub mod FinalFantasyXIV;
pub mod FinalFantasyXIV_English;
pub mod FullFace;
pub mod FullMetalPanic;
pub mod FullMetalPanic_Korean;
pub mod FutariSousa;
pub mod FutariSousa_Korean;
pub mod GURPS;
pub mod GaiaCare;
pub mod Garactier;
pub mod Garako;
pub mod GardenOrder;
pub mod GardenOrderReEdit;
pub mod GardenOrder_Korean;
pub mod GehennaAn;
pub mod GeishaGirlwithKatana;
pub mod GhostLive;
pub mod GoblinSlayer;
pub mod GoldenSkyStories;
pub mod Gorilla;
pub mod GranCrest;
pub mod GundamSentinel;
pub mod Gundog;
pub mod GundogRevised;
pub mod GundogZero;
pub mod GurpsFW;
pub mod HarnMaster;
pub mod HatsuneMiku;
pub mod HatsuneMiku_Korean;
pub mod HeroScale;
pub mod Hieizan;
pub mod HouraiGakuen;
pub mod HunterTheReckoning5th;
pub mod HuntersMoon;
pub mod IfIfIf;
pub mod Illusio;
pub mod InfiniteBabeL;
pub mod InfiniteFantasia;
pub mod Insane;
pub mod Insane_Korean;
pub mod InvisibleLiar;
pub mod Irisbane;
pub mod Irisbane_Korean;
pub mod IthaWenUa;
pub mod JamesBond;
pub mod JekyllAndHyde;
pub mod JuinKansen;
pub mod JuinKansen_Korean;
pub mod Kamigakari;
pub mod Kamigakari_Korean;
pub mod KamitsubakiCityUnderConstructionNarrative;
pub mod KanColle;
pub mod Karukami;
pub mod KemonoNoMori;
pub mod KemonoNoMori_Korean;
pub mod KillDeathBusiness;
pub mod KillDeathBusiness_Korean;
pub mod KimitoYell;
pub mod KinAriel;
pub mod KizunaBullet;
pub mod KizunaBullet_Korean;
pub mod KurayamiCrying;
pub mod Kutulu;
pub mod KutuluRevised;
pub mod KyokoShinshoku;
pub mod Liminal;
pub mod LiverLabyrinth;
pub mod LiveraDoll;
pub mod LogHorizon;
pub mod LogHorizon_Korean;
pub mod Lost;
pub mod LostRecord;
pub mod LostRoyal;
pub mod MagicPunk;
pub mod MagicPunk_Korean;
pub mod MagicaLogia;
pub mod MagicaLogia_Korean;
pub mod MagicaLogia_SimplifiedChinese;
pub mod Magius;
pub mod Magius_3rdNewTokyoCity;
pub mod MamonoScramble;
pub mod MarvelHeroicRoleplaying;
pub mod MeikyuDays;
pub mod MeikyuKingdom;
pub mod MeikyuKingdomBasic;
pub mod MetalHead;
pub mod MetalHeadExtream;
pub mod MetallicGuardian;
pub mod MetallicGuardian_Korean;
pub mod MonotoneMuseum;
pub mod MonotoneMuseum_Korean;
pub mod MorkBorg;
pub mod MorkBorg_Korean;
pub mod NRR;
pub mod NSSQ;
pub mod NanimonaiMura;
pub mod Nechronica;
pub mod Nechronica_Korean;
pub mod NegikureNegimaki;
pub mod NegikureNegimaki_Korean;
pub mod NeonUnderRealm;
pub mod NervWhitePaper;
pub mod NeverCloud;
pub mod NightWizard;
pub mod NightWizard3rd;
pub mod NightmareHunterDeep;
pub mod NinjaSlayer;
pub mod NinjaSlayer2;
pub mod NjslyrBattle;
pub mod NobunagasBlackCastle;
pub mod Nuekagami;
pub mod Nuekagami_Korean;
pub mod OneWayHeroics;
pub mod OracleEngine;
pub mod OrgaRain;
pub mod Oukahoushin3rd;
pub mod Paradiso;
pub mod Paranoia;
pub mod ParanoiaPerfect;
pub mod ParanoiaRebooted;
pub mod ParasiteBlood;
pub mod PastFutureParadox;
pub mod Pathfinder;
pub mod Peekaboo;
pub mod Pendragon;
pub mod PersonaO;
pub mod PhantasmAdventure;
pub mod Postman;
pub mod PreciousDays;
pub mod PulpCthulhu;
pub mod Raisondetre;
pub mod RecordOfLodossWar;
pub mod RecordOfSteam;
pub mod Revulture;
pub mod Revulture_Korean;
pub mod RogueLikeHalf;
pub mod RokumonSekai2;
pub mod RoleMaster;
pub mod RuinBreakers;
pub mod RuneQuest;
pub mod RuneQuestRoleplayingInGlorantha;
pub mod RyuTuber;
pub mod Ryutama;
pub mod SRS;
pub mod SRS_Korean;
pub mod SajinsenkiAGuS;
pub mod SajinsenkiAGuS2E;
pub mod SamsaraBallad;
pub mod Satasupe;
pub mod ScreamHighSchool;
pub mod Sengensyou;
pub mod SevenFortressMobius;
pub mod ShadowRun;
pub mod ShadowRun4;
pub mod ShadowRun5;
pub mod SharedFantasia;
pub mod ShinMegamiTenseiKakuseihen;
pub mod ShinkuuGakuen;
pub mod ShinobiGami;
pub mod ShinobiGami_Korean;
pub mod Shiranui;
pub mod ShoujoTenrankai;
pub mod ShuumatsuBargainWars;
pub mod ShuumatsuKikou;
pub mod Siren;
pub mod Skynauts;
pub mod SkynautsBouken;
pub mod SkynautsBouken_Korean;
pub mod StarryDolls;
pub mod StarryDolls_Korean;
pub mod SteamPunkers;
pub mod StellarKnights;
pub mod StellarKnights_Korean;
pub mod StellarLife;
pub mod StrangerOfSwordCity;
pub mod StratoShout;
pub mod StratoShout_Korean;
pub mod Strave;
pub mod SwordWorld;
pub mod SwordWorld2_0;
pub mod SwordWorld2_0_SimplifiedChinese;
pub mod SwordWorld2_5;
pub mod SwordWorld2_5_SimplifiedChinese;
pub mod SwordWorld_SimplifiedChinese;
pub mod TacticalExorcist;
pub mod TalesFromTheLoop;
pub mod TenkaRyouran;
pub mod TenkaRyouran_Korean;
pub mod TensaiGunshiNiNaro;
pub mod TheIndieHack;
pub mod TheOneRing2nd;
pub mod TheUnofficialHollowKnightRPG;
pub mod TherapieSein;
pub mod TokumeiTenkousei;
pub mod TokyoGhostResearch;
pub mod TokyoNova;
pub mod Torg;
pub mod Torg1_5;
pub mod TorgEternity;
pub mod ToshiakiHolyGrailWar;
pub mod TrailOfCthulhu;
pub mod TrinitySeven;
pub mod TunnelsAndTrolls;
pub mod TwilightGunsmoke;
pub mod UnsungDuet;
pub mod UnsungDuet_Korean;
pub mod Utakaze;
pub mod VampireTheMasquerade5th;
pub mod Ventangle;
pub mod Ventangle_Korean;
pub mod Villaciel;
pub mod VisionConnect;
pub mod WARPS;
pub mod WaresBlade;
pub mod Warhammer;
pub mod Warhammer4;
pub mod WerewolfTheApocalypse5th;
pub mod WitchQuest;
pub mod WoW;
pub mod WorldEndScrapyard;
pub mod WorldOfDarkness;
pub mod WorldsEndFrontline;
pub mod WorldsEndFrontline_Korean;
pub mod YankeeMustDie;
pub mod YankeeYogSothoth;
pub mod YearZeroEngine;
pub mod YearZeroEngine_Korean;
pub mod Yggdrasill;
pub mod Yotabana;
pub mod YuMyoKishi;
pub mod ZettaiReido;
pub mod ZombiLine;
pub mod ZombiLine_Korean;

/// 生成された全ゲームシステム（クラス名の昇順、335件）。
///
/// 手書き実装（`DiceBot` / `DummySystem`）は含まない。
/// レジストリ（`crate::game_system::registry`）がこのスライスと手書き分を連結する。
pub static GENERATED_GAME_SYSTEMS: &[&'static dyn crate::game_system::GameSystem] = &[
    &AFF2e::AFF2e,
    &AceKillerGene::AceKillerGene,
    &Agnostos::Agnostos,
    &Ainecadette::Ainecadette,
    &Aionia::Aionia,
    &Airgetlamh::Airgetlamh,
    &Airgetlamh_Korean::Airgetlamh_Korean,
    &AlchemiaStruggle::AlchemiaStruggle,
    &Alsetto::Alsetto,
    &Alsetto_Korean::Alsetto_Korean,
    &Alshard::Alshard,
    &AlterRaise::AlterRaise,
    &Amadeus::Amadeus,
    &Amadeus_Korean::Amadeus_Korean,
    &AngelGear::AngelGear,
    &AniMalus::AniMalus,
    &AnimaAnimus::AnimaAnimus,
    &AnimaAnimus_Korean::AnimaAnimus_Korean,
    &Aoharubaan::Aoharubaan,
    &Arianrhod::Arianrhod,
    &Arianrhod_Korean::Arianrhod_Korean,
    &ArknightsFan::ArknightsFan,
    &ArsMagica::ArsMagica,
    &AssaultEngine::AssaultEngine,
    &Avandner::Avandner,
    &Ayabito::Ayabito,
    &BBN::BBN,
    &BadLife::BadLife,
    &Bakenokawa::Bakenokawa,
    &BarnaKronika::BarnaKronika,
    &BattleTech::BattleTech,
    &BeastBindTrinity::BeastBindTrinity,
    &BeginningIdol::BeginningIdol,
    &BeginningIdol2022::BeginningIdol2022,
    &BeginningIdol_Korean::BeginningIdol_Korean,
    &BlackJacket::BlackJacket,
    &BlackJacket_Korean::BlackJacket_Korean,
    &BladeOfArcana::BladeOfArcana,
    &BlindMythos::BlindMythos,
    &BloodCrusade::BloodCrusade,
    &BloodMoon::BloodMoon,
    &Bloodorium::Bloodorium,
    &Bloodorium_Korean::Bloodorium_Korean,
    &CardRanker::CardRanker,
    &CastleInGray::CastleInGray,
    &ChaosFlare::ChaosFlare,
    &CharonSanctions::CharonSanctions,
    &Chill::Chill,
    &Chill3::Chill3,
    &ChroniclesOfDarkness2e::ChroniclesOfDarkness2e,
    &CodeLayerd::CodeLayerd,
    &ColossalHunter::ColossalHunter,
    &Comes::Comes,
    &ConvictorDrive::ConvictorDrive,
    &CrashWorld::CrashWorld,
    &Cthulhu::Cthulhu,
    &Cthulhu7th::Cthulhu7th,
    &Cthulhu7th_ChineseTraditional::Cthulhu7th_ChineseTraditional,
    &Cthulhu7th_Korean::Cthulhu7th_Korean,
    &CthulhuTech::CthulhuTech,
    &Cthulhu_ChineseTraditional::Cthulhu_ChineseTraditional,
    &Cthulhu_English::Cthulhu_English,
    &Cthulhu_Korean::Cthulhu_Korean,
    &Cthulhu_SimplifiedChinese::Cthulhu_SimplifiedChinese,
    &CyberpunkRed::CyberpunkRed,
    &CyberpunkRed_Korean::CyberpunkRed_Korean,
    &DarkBlaze::DarkBlaze,
    &DarkDaysDrive::DarkDaysDrive,
    &DarkSouls::DarkSouls,
    &DeadlineHeroes::DeadlineHeroes,
    &DeadlineHeroes_Korean::DeadlineHeroes_Korean,
    &DemonParasite::DemonParasite,
    &DemonSpike::DemonSpike,
    &DesperateRun::DesperateRun,
    &DetatokoSaga::DetatokoSaga,
    &DetatokoSaga_Korean::DetatokoSaga_Korean,
    &DiceOfTheDead::DiceOfTheDead,
    &DivineCharger::DivineCharger,
    &DoubleCross::DoubleCross,
    &DoubleCross_Korean::DoubleCross_Korean,
    &Dracurouge::Dracurouge,
    &Dracurouge_Korean::Dracurouge_Korean,
    &DungeonsAndDragons::DungeonsAndDragons,
    &DungeonsAndDragons5::DungeonsAndDragons5,
    &DungeonsAndDragons5_Korean::DungeonsAndDragons5_Korean,
    &DungeonsAndDragons_Korean::DungeonsAndDragons_Korean,
    &EarthDawn::EarthDawn,
    &EarthDawn3::EarthDawn3,
    &EarthDawn4::EarthDawn4,
    &EclipsePhase::EclipsePhase,
    &EdgeFlippers::EdgeFlippers,
    &Elric::Elric,
    &Elysion::Elysion,
    &EmbryoMachine::EmbryoMachine,
    &Emoklore::Emoklore,
    &Emoklore_Korean::Emoklore_Korean,
    &EndBreaker::EndBreaker,
    &EtrianOdysseySRS::EtrianOdysseySRS,
    &FateCoreSystem::FateCoreSystem,
    &Fiasco::Fiasco,
    &Fiasco_Korean::Fiasco_Korean,
    &FilledWith::FilledWith,
    &FinalFantasyXIV::FinalFantasyXIV,
    &FinalFantasyXIV_English::FinalFantasyXIV_English,
    &FullFace::FullFace,
    &FullMetalPanic::FullMetalPanic,
    &FullMetalPanic_Korean::FullMetalPanic_Korean,
    &FutariSousa::FutariSousa,
    &FutariSousa_Korean::FutariSousa_Korean,
    &GURPS::GURPS,
    &GaiaCare::GaiaCare,
    &Garactier::Garactier,
    &Garako::Garako,
    &GardenOrder::GardenOrder,
    &GardenOrderReEdit::GardenOrderReEdit,
    &GardenOrder_Korean::GardenOrder_Korean,
    &GehennaAn::GehennaAn,
    &GeishaGirlwithKatana::GeishaGirlwithKatana,
    &GhostLive::GhostLive,
    &GoblinSlayer::GoblinSlayer,
    &GoldenSkyStories::GoldenSkyStories,
    &Gorilla::Gorilla,
    &GranCrest::GranCrest,
    &GundamSentinel::GundamSentinel,
    &Gundog::Gundog,
    &GundogRevised::GundogRevised,
    &GundogZero::GundogZero,
    &GurpsFW::GurpsFW,
    &HarnMaster::HarnMaster,
    &HatsuneMiku::HatsuneMiku,
    &HatsuneMiku_Korean::HatsuneMiku_Korean,
    &HeroScale::HeroScale,
    &Hieizan::Hieizan,
    &HouraiGakuen::HouraiGakuen,
    &HunterTheReckoning5th::HunterTheReckoning5th,
    &HuntersMoon::HuntersMoon,
    &IfIfIf::IfIfIf,
    &Illusio::Illusio,
    &InfiniteBabeL::InfiniteBabeL,
    &InfiniteFantasia::InfiniteFantasia,
    &Insane::Insane,
    &Insane_Korean::Insane_Korean,
    &InvisibleLiar::InvisibleLiar,
    &Irisbane::Irisbane,
    &Irisbane_Korean::Irisbane_Korean,
    &IthaWenUa::IthaWenUa,
    &JamesBond::JamesBond,
    &JekyllAndHyde::JekyllAndHyde,
    &JuinKansen::JuinKansen,
    &JuinKansen_Korean::JuinKansen_Korean,
    &Kamigakari::Kamigakari,
    &Kamigakari_Korean::Kamigakari_Korean,
    &KamitsubakiCityUnderConstructionNarrative::KamitsubakiCityUnderConstructionNarrative,
    &KanColle::KanColle,
    &Karukami::Karukami,
    &KemonoNoMori::KemonoNoMori,
    &KemonoNoMori_Korean::KemonoNoMori_Korean,
    &KillDeathBusiness::KillDeathBusiness,
    &KillDeathBusiness_Korean::KillDeathBusiness_Korean,
    &KimitoYell::KimitoYell,
    &KinAriel::KinAriel,
    &KizunaBullet::KizunaBullet,
    &KizunaBullet_Korean::KizunaBullet_Korean,
    &KurayamiCrying::KurayamiCrying,
    &Kutulu::Kutulu,
    &KutuluRevised::KutuluRevised,
    &KyokoShinshoku::KyokoShinshoku,
    &Liminal::Liminal,
    &LiverLabyrinth::LiverLabyrinth,
    &LiveraDoll::LiveraDoll,
    &LogHorizon::LogHorizon,
    &LogHorizon_Korean::LogHorizon_Korean,
    &Lost::Lost,
    &LostRecord::LostRecord,
    &LostRoyal::LostRoyal,
    &MagicPunk::MagicPunk,
    &MagicPunk_Korean::MagicPunk_Korean,
    &MagicaLogia::MagicaLogia,
    &MagicaLogia_Korean::MagicaLogia_Korean,
    &MagicaLogia_SimplifiedChinese::MagicaLogia_SimplifiedChinese,
    &Magius::Magius,
    &Magius_3rdNewTokyoCity::Magius_3rdNewTokyoCity,
    &MamonoScramble::MamonoScramble,
    &MarvelHeroicRoleplaying::MarvelHeroicRoleplaying,
    &MeikyuDays::MeikyuDays,
    &MeikyuKingdom::MeikyuKingdom,
    &MeikyuKingdomBasic::MeikyuKingdomBasic,
    &MetalHead::MetalHead,
    &MetalHeadExtream::MetalHeadExtream,
    &MetallicGuardian::MetallicGuardian,
    &MetallicGuardian_Korean::MetallicGuardian_Korean,
    &MonotoneMuseum::MonotoneMuseum,
    &MonotoneMuseum_Korean::MonotoneMuseum_Korean,
    &MorkBorg::MorkBorg,
    &MorkBorg_Korean::MorkBorg_Korean,
    &NRR::NRR,
    &NSSQ::NSSQ,
    &NanimonaiMura::NanimonaiMura,
    &Nechronica::Nechronica,
    &Nechronica_Korean::Nechronica_Korean,
    &NegikureNegimaki::NegikureNegimaki,
    &NegikureNegimaki_Korean::NegikureNegimaki_Korean,
    &NeonUnderRealm::NeonUnderRealm,
    &NervWhitePaper::NervWhitePaper,
    &NeverCloud::NeverCloud,
    &NightWizard::NightWizard,
    &NightWizard3rd::NightWizard3rd,
    &NightmareHunterDeep::NightmareHunterDeep,
    &NinjaSlayer::NinjaSlayer,
    &NinjaSlayer2::NinjaSlayer2,
    &NjslyrBattle::NjslyrBattle,
    &NobunagasBlackCastle::NobunagasBlackCastle,
    &Nuekagami::Nuekagami,
    &Nuekagami_Korean::Nuekagami_Korean,
    &OneWayHeroics::OneWayHeroics,
    &OracleEngine::OracleEngine,
    &OrgaRain::OrgaRain,
    &Oukahoushin3rd::Oukahoushin3rd,
    &Paradiso::Paradiso,
    &Paranoia::Paranoia,
    &ParanoiaPerfect::ParanoiaPerfect,
    &ParanoiaRebooted::ParanoiaRebooted,
    &ParasiteBlood::ParasiteBlood,
    &PastFutureParadox::PastFutureParadox,
    &Pathfinder::Pathfinder,
    &Peekaboo::Peekaboo,
    &Pendragon::Pendragon,
    &PersonaO::PersonaO,
    &PhantasmAdventure::PhantasmAdventure,
    &Postman::Postman,
    &PreciousDays::PreciousDays,
    &PulpCthulhu::PulpCthulhu,
    &Raisondetre::Raisondetre,
    &RecordOfLodossWar::RecordOfLodossWar,
    &RecordOfSteam::RecordOfSteam,
    &Revulture::Revulture,
    &Revulture_Korean::Revulture_Korean,
    &RogueLikeHalf::RogueLikeHalf,
    &RokumonSekai2::RokumonSekai2,
    &RoleMaster::RoleMaster,
    &RuinBreakers::RuinBreakers,
    &RuneQuest::RuneQuest,
    &RuneQuestRoleplayingInGlorantha::RuneQuestRoleplayingInGlorantha,
    &RyuTuber::RyuTuber,
    &Ryutama::Ryutama,
    &SRS::SRS,
    &SRS_Korean::SRS_Korean,
    &SajinsenkiAGuS::SajinsenkiAGuS,
    &SajinsenkiAGuS2E::SajinsenkiAGuS2E,
    &SamsaraBallad::SamsaraBallad,
    &Satasupe::Satasupe,
    &ScreamHighSchool::ScreamHighSchool,
    &Sengensyou::Sengensyou,
    &SevenFortressMobius::SevenFortressMobius,
    &ShadowRun::ShadowRun,
    &ShadowRun4::ShadowRun4,
    &ShadowRun5::ShadowRun5,
    &SharedFantasia::SharedFantasia,
    &ShinMegamiTenseiKakuseihen::ShinMegamiTenseiKakuseihen,
    &ShinkuuGakuen::ShinkuuGakuen,
    &ShinobiGami::ShinobiGami,
    &ShinobiGami_Korean::ShinobiGami_Korean,
    &Shiranui::Shiranui,
    &ShoujoTenrankai::ShoujoTenrankai,
    &ShuumatsuBargainWars::ShuumatsuBargainWars,
    &ShuumatsuKikou::ShuumatsuKikou,
    &Siren::Siren,
    &Skynauts::Skynauts,
    &SkynautsBouken::SkynautsBouken,
    &SkynautsBouken_Korean::SkynautsBouken_Korean,
    &StarryDolls::StarryDolls,
    &StarryDolls_Korean::StarryDolls_Korean,
    &SteamPunkers::SteamPunkers,
    &StellarKnights::StellarKnights,
    &StellarKnights_Korean::StellarKnights_Korean,
    &StellarLife::StellarLife,
    &StrangerOfSwordCity::StrangerOfSwordCity,
    &StratoShout::StratoShout,
    &StratoShout_Korean::StratoShout_Korean,
    &Strave::Strave,
    &SwordWorld::SwordWorld,
    &SwordWorld2_0::SwordWorld2_0,
    &SwordWorld2_0_SimplifiedChinese::SwordWorld2_0_SimplifiedChinese,
    &SwordWorld2_5::SwordWorld2_5,
    &SwordWorld2_5_SimplifiedChinese::SwordWorld2_5_SimplifiedChinese,
    &SwordWorld_SimplifiedChinese::SwordWorld_SimplifiedChinese,
    &TacticalExorcist::TacticalExorcist,
    &TalesFromTheLoop::TalesFromTheLoop,
    &TenkaRyouran::TenkaRyouran,
    &TenkaRyouran_Korean::TenkaRyouran_Korean,
    &TensaiGunshiNiNaro::TensaiGunshiNiNaro,
    &TheIndieHack::TheIndieHack,
    &TheOneRing2nd::TheOneRing2nd,
    &TheUnofficialHollowKnightRPG::TheUnofficialHollowKnightRPG,
    &TherapieSein::TherapieSein,
    &TokumeiTenkousei::TokumeiTenkousei,
    &TokyoGhostResearch::TokyoGhostResearch,
    &TokyoNova::TokyoNova,
    &Torg::Torg,
    &Torg1_5::Torg1_5,
    &TorgEternity::TorgEternity,
    &ToshiakiHolyGrailWar::ToshiakiHolyGrailWar,
    &TrailOfCthulhu::TrailOfCthulhu,
    &TrinitySeven::TrinitySeven,
    &TunnelsAndTrolls::TunnelsAndTrolls,
    &TwilightGunsmoke::TwilightGunsmoke,
    &UnsungDuet::UnsungDuet,
    &UnsungDuet_Korean::UnsungDuet_Korean,
    &Utakaze::Utakaze,
    &VampireTheMasquerade5th::VampireTheMasquerade5th,
    &Ventangle::Ventangle,
    &Ventangle_Korean::Ventangle_Korean,
    &Villaciel::Villaciel,
    &VisionConnect::VisionConnect,
    &WARPS::WARPS,
    &WaresBlade::WaresBlade,
    &Warhammer::Warhammer,
    &Warhammer4::Warhammer4,
    &WerewolfTheApocalypse5th::WerewolfTheApocalypse5th,
    &WitchQuest::WitchQuest,
    &WoW::WoW,
    &WorldEndScrapyard::WorldEndScrapyard,
    &WorldOfDarkness::WorldOfDarkness,
    &WorldsEndFrontline::WorldsEndFrontline,
    &WorldsEndFrontline_Korean::WorldsEndFrontline_Korean,
    &YankeeMustDie::YankeeMustDie,
    &YankeeYogSothoth::YankeeYogSothoth,
    &YearZeroEngine::YearZeroEngine,
    &YearZeroEngine_Korean::YearZeroEngine_Korean,
    &Yggdrasill::Yggdrasill,
    &Yotabana::Yotabana,
    &YuMyoKishi::YuMyoKishi,
    &ZettaiReido::ZettaiReido,
    &ZombiLine::ZombiLine,
    &ZombiLine_Korean::ZombiLine_Korean,
];
