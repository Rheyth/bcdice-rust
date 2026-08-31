# P4 バッチ子カード標準テンプレート（v1.0・lia設計）

この文書は bcdice-rust ボードの P4 バッチ子カードの **正** である。
ワーカーはこの形式に従って起票・実行する。改造・省略は禁止（改善提案はコメントでliaに返す）。

## 1. 起票仕様（親カードのワーカーが従う）

- タイトル: `P4-B##: <代表システム名> 他Nシステム（M cases）`
  - 例: `P4-B01: MeikyuKingdom 他19システム（4880 cases）`
- assignee: `bcdice-rust-dev`、workspace: `worktree:/home/rheyth/works/bcdice-rust`
- parent: `t_5b014172`（P4親）
- body: 下記テンプレートの `{...}` を plans/p4_batches.json の該当バッチで置換して埋める

## 2. 子カード body テンプレート

```markdown
【対象】{バッチに含まれるシステムID一覧（改行区切り、各IDの原典rbとTOMLケース数を併記）}

【統合ブランチ】
- 作業ブランチ: wt/p4-integration（head {起票時のa33c8c2b等のコミットハッシュ}）
- ベース状態: P1〜P3統合済み・cargo test 147パス・generatedメタデータ336システム込み
- 本カードの実装先: rust/src/game_system/generated/{SystemId}.rs の eval 部を実装
  （スタブの「固有多コマンドはP4で個別移植」コメント部分を置き換える形）

【参照原典（正）】
- lib/bcdice/game_system/{SystemId}.rb（各システム。クラス継承・定数・eval_game_system・テーブル定義）
- 親クラスの機能を使う場合はそのrbも読むこと（Base / サブディレクトリ型はlib/bcdice/game_system/{Dir}/配下）
- 期待値の真実: test/data/{SystemId}.toml

【作業手順（この順で・変更はこの順で記録すること）】
1. cd /home/rheyth/works/bcdice-rust && git checkout wt/p4-integration（作業ブランチ確認）
2. 対象システムのrbを読み、コマンド解析→eval_game_system→出力フォーマットの流れを把握
3. claude -p に委譲（規約は下記【委譲プロトコル】）
4. 検証（下記【検証コマンド】をこの順で実行、全パスを確認）
5. git commit（メッセージ: `P4-B##: {SystemId,...} 移植`）。1システム単位でも可、ただしコミットごとにテストをパスさせること

【委譲プロトコル（全バッチ共通）】
- コマンド: claude -p "<タスク全文>" --model opus --effort max --max-turns 90 --dangerously-skip-permissions --output-format json
  （workdirは /home/rheyth/works/bcdice-rust、タスク全文には対象システムID・原典rbパス・TOMLパス・完了条件を含める）
- error_max_turns の場合: JSONのsession_idを --resume <id> で継続（最大3回）。3回で完了しなければカードをblockedにし、summaryに進捗と残タスクを書く
- 委譲1回ごとにカードコメントへ記録: 使用したプロンプト要点・session_id・結果（success/error_max_turns）
- 90/90で中断→resumeは禁止する例外なし。コミットゼロの中断を繰り返させないため、resume時に「まず cargo test を実行して現状を把握してから続きを作る」をプロンプトに含める

【検証コマンド（全パスが完了条件。この順で実行し、結果を報告に貼る）】
cd /home/rheyth/works/bcdice-rust/rust
cargo test 2>&1 | grep -E "test result"
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
TOMLハーネスでの対象システム別パス率（ハーネスのシステム別集計機能を使う。無ければ cargo test 全体のパス数と失敗リスト）

【完了条件】
1. 対象システムのTOMLケースが全パス（失敗がある場合、原因がTOML側のバグでない限り完了としない）
2. cargo test 全体が既存パス数以上（他システムを壊していないこと）
3. clippy -D warnings 0
4. コミット済み（worktreeのmainブランチ wt/p4-integration 上）

【報告形式（kanban_complete のsummaryに書くこと）】
- パス率: {対象ケース数}/{対象ケース数}
- cargo test: {N} passed / clippy: 0 warning
- コミット: {ハッシュ一覧}
- 既知差分・特記事項（あれば）
```

## 3. テーブル系バッチ（table_only 48システム）の扱い

- eval系15バッチ完了後に起票する（親カードが管理）
- テーブル系は`dice_table`機構（P3実装済み）+ メタデータスタブの`settings`/テーブルデータで完結する想定
- テンプレートは上記と同一。相違点のみ: 作業対象が「`generated/{SystemId}.rs` のテーブルデータ実装」となる
- 難所（例: SwordWorld2.5系の複雑なテーブル）が判明したら、そのシステムを単独カードに切り出してよい（起票前にliaへコメントで報告）

## 4. 変更管理

- このテンプレートの変更はliaのみが行う（versionを上げる）
- ワーカーからの改善提案は、実行中カードのコメントで「テンプレート改善案:」の見出しで書くこと
- 起票済みカードのbodyを後から書き換えない。差分はコメントで補足する
