# bcdice-rust

[BCDice](https://github.com/bcdice/BCDice)（本家 BCDice 3.17.0 時点）のフォーク。Ludorium プロジェクトでの利用を目的としている。

## 現状

- **これは Ruby 製のまま**。Rust への移植は未着手。リポジトリ名 `bcdice-rust` は Ludorium 側の ADR（§4 ダイスエンジン）が想定する Rust 版ダイスエンジンの受け皿として名付けたもの。
- 想定される将来像: 本家 Ruby 実装を参照実装として、ダイスコアを Rust に移植する（`lib/bcdice` 配下）。
- 移植完了までの当面の用途: 本家同等の Ruby 版として動作させる（bcdice-api 相当のサイドカーまたは gem 利用）。

## 上流追従

```
git remote add upstream https://github.com/bcdice/BCDice.git   # 済み
git fetch upstream
git merge upstream/master
```

本家は活発（直近リリース 3.17.0）。移植開始前に最新へ追従すること。

## ライセンス

本家 [BSD-3-Clause](./LICENSE)（Copyright (c) 2011, Faceless and たいたい竹流）を継承。本フォークでの改変・追加コードも同ライセンスで公開する（public 化時の前提）。

## 元リポジトリ

- 上流: https://github.com/bcdice/BCDice
- フォーク元コミット: `8eced50f`（Release BCDice 3.17.0）
