# wasmchi-rs2 docs

この `docs/` は wasmchi（Rust新規実装）の設計・仕様・開発ログ置き場。

- 目的: **Web(ブラウザ/Node) → hot loop → CLI(WASI) → server/edge** の順で実用レベルまで育てる
- 価値観: **minimal / fast / 書きやすい / 型はガチ**（`any` なし、危険操作は明示）
- 開発スタイル: **t-wada流TDD**（小さい赤→緑→リファクタを回して仕様を固定）

## ドキュメント一覧
- [spec-v0.md](./spec-v0.md): v0の言語仕様（まずここが真実）
- [roadmap.md](./roadmap.md): 実装ロードマップ（TDDの赤リスト付き）
- [testing.md](./testing.md): テスト戦略（Rust unit + Node/Browser E2E）

## 重要な制約（現実）
「無限に実装し続ける」は物理的にできないので、代わりに **終わりのないTODOを明文化して、区切りごとに前進**する。

- 進捗は `roadmap.md` のチェックを増やしていく
- 破壊的変更は `spec-v0.md` を更新し、テストで固定する
