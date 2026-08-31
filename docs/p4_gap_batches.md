# P4 穴埋めバッチ計画（lia・2026-08-30）

B02〜B09で未計画の57システム（旧B01残り11＋無計画46）をB10〜B21に割り当てる。
起票は親カード（t_5b014172）のワーカーが docs/p4_batch_card_template.md v1.0 に従って行う。
網羅性チェック必須: 各バッチ完了時、本表と突き合わせて漏れゼロを確認すること。

## B10: 旧B01残り・大ケース系（5システム・約1,700cases）
- MeikyuKingdom (622) / MeikyuKingdomBasic (373) / Satasupe (346) / LogHorizon (268, バリアント LogHorizon_Korean) / GundogZero (259)

## B11: 旧B01残り・中ケース系（5システム・約1,300cases）
- Warhammer4 (246) / Warhammer (241) / SwordWorld (230, バリアント SwordWorld_SimplifiedChinese) / MeikyuDays (229) / HuntersMoon (175)
- ※YggdrasillはB02〜B09に既存のため除外

## B12: Cthulhuファミリー（9システム）
- Cthulhu / Cthulhu_English / Cthulhu_Korean / Cthulhu_SimplifiedChinese / Cthulhu_ChineseTraditional / Cthulhu7th / Cthulhu7th_Korean / Cthulhu7th_ChineseTraditional / Gorilla

## B13: SwordWorld & Dracurouge系（6システム）
- SwordWorld2_5 / SwordWorld2_5_SimplifiedChinese / Dracurouge / Dracurouge_Korean / Chill3 / Elric

## B14: StarryDolls & 大作系（8システム）
- StarryDolls / StarryDolls_Korean / RuneQuest（B02〜B09に既存か要確認・なければここ）/ Pathfinder / Pendragon / RoleMaster / EclipsePhase / IthaWenUa

## B15: MagicaLogia & FutariSousa系（8システム）
- MagicaLogia / MagicaLogia_Korean / MagicaLogia_SimplifiedChinese / FutariSousa / FutariSousa_Korean / TokyoNova / WARPS / WaresBlade

## B16: キルビジネス & 小規模系（8システム）
- KillDeathBusiness / KillDeathBusiness_Korean / JamesBond / ShadowRun4 / SharedFantasia / ShinMegamiTenseiKakuseihen / InfiniteFantasia / Hieizan

## B17: その他残余（7システム）
- Arianrhod / Arianrhod_Korean / NjslyrBattle / PhantasmAdventure / LostRecord / IthaWenUa（B14と重複時はB14優先）/ DungeonsAndDragons_Korean

## 検証
- 全バッチ完了後、generated/ 336ファイルのうち Notimplented スタブ残存が 0 であることを確認する
- 確認コマンド: grep -l "NotImplemented" rust/src/game_system/generated/*.rs | wc -l → 0
