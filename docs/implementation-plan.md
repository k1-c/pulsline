# Pulsline 実装計画

> AI エージェントでの作業を **Claude Code Plugin でジャーナリング**し、
> GitHub / Linear の活動と合わせて 1 日のタイムラインとして振り返る、**ローカル完結**のツール。
> クライアントは **TUI** と **デスクトップ（GPUI）** の 2 つ。

- 作成日: 2026-09-26
- 改訂: r3.1（リポジトリ構成を linear-tui 準拠に変更）
- 前回: r3（ローカル完結・エージェントジャーナル中心に方針転換）
- ステータス: Draft

---

## 1. 方針

1. **サーバーを持たない。** データはすべてローカルに置き、外部への送信は GitHub / Linear の API 呼び出しだけにする。
2. **主なデータ源はエージェント。** Claude Code Plugin の hooks で作業を機械的に記録し、Claude 自身に「何をしたか」「何を決めたか」を意味のあるジャーナルとして書かせる。
3. **GitHub と Linear は補助。** 個人トークンで Pull し、エージェントの作業と commit / PR / Issue を結び付ける。
4. **Raw は追記専用の JSONL、SQLite はそこから作る索引。** 索引はいつでも JSONL から作り直せる。
5. **クライアントは 2 つ。** 同じ core を共有する TUI（ratatui）と Desktop（GPUI）。

### r2 から削ったもの

削ったもの:

- クラウドの Connector Hub
- OAuth アプリの公開と審査
- Webhook
- Google Calendar
- Postgres
- ホスティング

これに合わせて、r1 にあった常駐 daemon も不要になる。

- エージェントの記録は hooks の発火に合わせて書き込まれる。
- GitHub / Linear の同期は、クライアントを起動したときと起動中の定期実行で行う。

---

## 2. アーキテクチャ

```
 ┌──────────── Claude Code ────────────┐
 │  Pulsline Plugin                     │
 │   ├─ hooks ──────── pulsline hook <event>  (stdin JSON → spool に追記)
 │   ├─ MCP server ─── pulsline mcp           (journal_record / timeline_query)
 │   └─ skill "journal"  いつ・何を記録するかの指示
 └──────────────────────────────────────┘
                     │ append
                     ▼
   ~/.local/share/pulsline/
     spool/2026-09-26.jsonl      ← 正本（追記専用、人間が読める）
     sync/github.jsonl, linear.jsonl
     index.db (SQLite, WAL)      ← 派生（rebuild 可能）
                     ▲
       ingest + sessionize（どちらのクライアントからも同じ core を呼ぶ）
          ┌──────────┴───────────┐
   pulsline tui (ratatui)   Pulsline Desktop (GPUI)
          └──── GitHub / Linear API（起動時と起動中の定期実行）
```

### 書き込み経路の原則

- **hook はエージェントの作業を絶対に止めない。**
  - hook の処理は「1 行を追記して即終了」だけにし、10ms 程度で終わるようにする。
  - 失敗しても exit 0 で終わる。
  - `pulsline` が PATH に無い場合は何もしない。
- SQLite への取り込み（ingest）はクライアントか CLI が行う。
  - spool のファイルごとに読み込み済みの byte offset を持ち、増分だけを取り込む。
  - hook から直接 DB に書かないのは、ロック競合と、スキーマ変更で hook が壊れることを避けるため。

---

## 3. Claude Code Plugin

```
agent-plugin/
├─ .claude-plugin/plugin.json
├─ .codex-plugin/plugin.json   # Codex 対応時に使う（linear-tui と同じ構成）
├─ hooks/
│  ├─ hooks.json               # command: sh "${CLAUDE_PLUGIN_ROOT}/hooks/record.sh"
│  └─ record.sh                # command -v pulsline || exit 0; exec pulsline hook record
├─ .mcp.json                   # { "pulsline": { "command": "pulsline", "args": ["mcp"] } }
└─ skills/journal/SKILL.md
```

配布方法:

- linear-tui と同じく、リポジトリの直下に `.claude-plugin/marketplace.json`（Codex 用は `.agents/plugins/marketplace.json`）を置き、このリポジトリ自体を marketplace にする。
- hook の本体はシェルスクリプトで包む。`pulsline` が入っていない環境では何もせずに終わるので、エラーで作業を妨げない。
- `pulsline` バイナリは `cargo install pulsline` か GitHub Releases で入れてもらう。

### 3.1 hooks: 機械的な記録

| hook | 記録する内容 | kind |
|---|---|---|
| `SessionStart` | session_id、cwd、git branch / HEAD、source（startup / resume など） | `agent.session.started` |
| `UserPromptSubmit` | prompt の先頭 N 文字（設定で全文 / なし / 先頭のみを選べる） | `agent.prompt` |
| `PostToolUse` | tool_name と要約（Edit / Write はファイルパス、Bash はコマンド先頭、Read はパス） | `agent.tool` |
| `Stop` / `SubagentStop` | ターンの終了 | `agent.turn.ended` |
| `PreCompact` | compact の発生 | `agent.compacted` |
| `SessionEnd` | 終了理由 | `agent.session.ended` |

- 本文は既定で記録しない。`tool_input` / `tool_response` はパスやコマンドなどの要約だけを持つ。
- hook の入力と出力の最新仕様は、M1 の着手時に公式ドキュメントで確認する。

### 3.2 ジャーナル: 意味のある記録（Pulsline の核）

プラグインに同梱する MCP server が次の tool を提供する。

```
journal_record(kind, title, body?, refs?)
  kind: "progress" | "decision" | "discovery" | "blocked" | "done"
  refs: { issue?: "INT-132", pr?: 412, files?: [...] }

timeline_query(range, project?)   # 「昨日何してた？」に答えるための読み取り
```

skill `journal` では、Claude が次の場面で `journal_record` を呼ぶように指示する。

- ひとまとまりの作業を終えたとき（`done`）
- 設計や実装の判断をしたとき（`decision`、却下した案とその理由も含める）
- 原因や制約が判明したとき（`discovery`）
- 作業が詰まったとき（`blocked`）

進捗の実況のような細かい報告は記録させない。

**ねらい:** Session のタイトルと要約を、後から LLM API で生成するのではなく、**作業中の Claude 自身に書かせる**。
追加の API 費用がかからず、データも外部に出ず、文脈を最もよく知っている主体が書くので要約の精度も高い。

### 3.3 過去分の取り込み

プラグインを導入する前の作業も取り込めるようにする。
`pulsline import claude-transcripts` で `~/.claude/projects/**/*.jsonl` を読み、spool と同じ形式に変換する。
transcript は保持期間を過ぎると消えるので、初回セットアップ時に実行する。

---

## 4. GitHub / Linear 同期

| | 認証 | 取得するもの | cursor |
|---|---|---|---|
| GitHub | `gh auth token` を流用し、無ければ PAT | 自分の commit（登録した repo）、PR の open / merge、review | `updatedAt` |
| Linear | Personal API key（`keyring` に保存） | 自分が assignee か作成者の Issue、状態遷移、コメント | `updatedAt` |

- 起動時と 15 分ごとに増分を同期する。`pulsline sync` で手動でも実行できる。
- **エージェント作業との結び付け（ここが価値になる）:**
  - hook で記録した branch / HEAD と、GitHub の commit SHA / PR を照合する。
  - branch 名、prompt、commit message、`journal_record` の `refs.issue` から Linear の Issue キー（`[A-Z]+-\d+`）を抽出する。
  - これで Session に「INT-132 / PR #412」を自動で付ける。

---

## 5. データモデル

### spool の 1 行（正本）

```json
{"v":1,"id":"01J...","at":"2026-09-26T00:02:11Z","source":"claude_code",
 "kind":"agent.tool","session":"b3f...","cwd":"/home/k1nix/dev/integral",
 "git":{"branch":"feat/INT-132-oauth","head":"a1b2c3"},
 "data":{"tool":"Edit","path":"src/auth/oauth.rs"}}
```

### SQLite（派生）

```sql
CREATE TABLE events (            -- spool / sync の各行
  id TEXT PRIMARY KEY, source TEXT, kind TEXT, at TEXT,
  agent_session TEXT, project_id TEXT, data TEXT);          -- JSON
CREATE INDEX events_at ON events(at);

CREATE TABLE journal (           -- journal_record のみ（UI で強調表示する）
  id TEXT PRIMARY KEY, at TEXT, agent_session TEXT, project_id TEXT,
  kind TEXT, title TEXT, body TEXT, refs TEXT);

CREATE TABLE work_sessions (     -- Sessionizer の出力
  id TEXT PRIMARY KEY, project_id TEXT, starts_at TEXT, ends_at TEXT,
  title TEXT, stats TEXT, links TEXT, algo_version INTEGER);

CREATE TABLE projects (id TEXT PRIMARY KEY, name TEXT, root_path TEXT, github_repo TEXT, linear_team TEXT);
CREATE TABLE ingest_offsets (file TEXT PRIMARY KEY, offset INTEGER);
CREATE TABLE sync_cursors (source TEXT PRIMARY KEY, cursor TEXT, last_success_at TEXT, last_error TEXT);
```

- Project は、cwd の git root から自動生成する。remote URL から `github_repo` も自動で推定する。
- `linear_team` は設定で指定する。

### Sessionizer v1

- 基本単位は Claude Code の `session_id`。
- 同じ Project で、間隔が 20 分未満の agent session、commit、PR、Linear の変更をまとめて 1 つの **work session** にする。
- タイトルは次の優先順位で決める: `done` / `progress` のジャーナル → Linear Issue のタイトル → PR のタイトル → branch 名。
- `stats` には tool 呼び出しの回数、編集したファイル数、commit 数などを入れる。

---

## 6. リポジトリ構成

[k1-c/linear-tui](https://github.com/k1-c/linear-tui) の構成を踏襲する。

- crate を細かく分割しない。1 つの crate の中で、**層ごとにディレクトリを分ける**（core / interface / infra）。
- 依存の向きは `tests/architecture.rs` で機械的に検査する。

例外として、**GPUI 製の Desktop だけは別 package に切り出す**。理由は 2 つある。

- GPUI は依存が重く、API もよく変わる。
- Linux ではシステムライブラリが必要になる。

これらを `cargo install pulsline`（CLI、TUI、hook、MCP）に持ち込まないため。

```
pulsline/
├─ Cargo.toml                   # [workspace] members = [".", "desktop"]、root は package "pulsline"（bin + lib）
├─ src/
│  ├─ main.rs                   # エントリポイント、サブコマンドの振り分け
│  ├─ lib.rs                    # desktop から core / infra を使うための公開面
│  ├─ runtime.rs                # TUI の main loop（interface と infra をつなぐ）
│  ├─ config.rs                 # ~/.config/pulsline/config.toml
│  ├─ logging.rs
│  ├─ core/                     # I/O なし（端末・ネットワーク・ファイルに触れない）
│  │  ├─ entity/                # 集約ごとに 1 ファイル
│  │  │  ├─ ids.rs
│  │  │  ├─ event.rs            # spool レコード（形式は外部との契約。VERSION を持つ）
│  │  │  ├─ agent_session.rs    # Claude Code / Codex のセッション
│  │  │  ├─ journal.rs          # journal_record のエントリ（done / decision / discovery / blocked）
│  │  │  ├─ work_session.rs     # work session と sessionize（純粋関数）
│  │  │  ├─ project.rs          # cwd / git root / repo / Linear team の対応
│  │  │  ├─ code.rs             # commit, pull request, review
│  │  │  ├─ task.rs             # Linear Issue の参照と状態遷移
│  │  │  └─ timeline.rs         # DayTimeline（2 つのクライアントが描画する view model）
│  │  ├─ store/                 # 読み込み済みの日と Session のメモリ上の状態、整合性のルール
│  │  ├─ usecase/               # 仕様書となる層。集約ごとに 1 モジュール
│  │  │  ├─ agent_session.rs    # hook イベントの記録
│  │  │  ├─ journal.rs          # 記録と一覧
│  │  │  ├─ timeline.rs         # 日単位・期間単位の問い合わせ、rebuild
│  │  │  ├─ project.rs
│  │  │  ├─ code.rs             # GitHub との同期要求
│  │  │  └─ task.rs             # Linear との同期要求
│  │  └─ message.rs             # infra から返ってくる結果
│  ├─ interface/                # 入口
│  │  ├─ cli/                   # hook / journal / sync / import / today / report / rebuild
│  │  ├─ mcp/                   # stdio MCP server（journal_record / timeline_query）
│  │  └─ tui/                   # ratatui（app/ = 状態遷移、ui/ = 描画、keys.rs）
│  └─ infra/                    # 呼び出す外部の仕組み
│     ├─ spool/                 # JSONL への追記と、offset を使った読み取り
│     ├─ index/                 # SQLite（rusqlite）、migration、ingest
│     ├─ transcript/            # ~/.claude/projects の jsonl の import
│     ├─ github/                # GraphQL クライアントと認証（gh auth token を流用し、無ければ PAT）
│     ├─ linear/                # GraphQL クライアントと API key
│     └─ dispatch.rs            # usecase::Request を実行する
├─ desktop/                     # package "pulsline-desktop"（GPUI）。pulsline の lib に依存する
│  ├─ Cargo.toml
│  └─ src/{main.rs, timeline_element.rs, ...}
├─ agent-plugin/                # Claude Code / Codex plugin（§3）
├─ .claude-plugin/marketplace.json
├─ .agents/plugins/marketplace.json
├─ tests/
│  ├─ architecture.rs           # 依存が内向きであること、core が I/O 系の crate を使わないこと
│  ├─ usecase_spec.rs           # usecase が仕様として読めること、各 use case にシナリオがあること
│  └─ fixtures/                 # hook 入力、transcript、GitHub / Linear のレスポンス
├─ docs/                        # README.md を目次にする（agent-plugin.md, cli.md, data-format.md, development.md …）
├─ AGENTS.md / CLAUDE.md
├─ mise.toml                    # verify = fmt → clippy -D warnings → test → build
├─ release-plz.toml / cliff.toml
└─ .github/workflows/           # ci / pr-title / release-plz（uses: は commit SHA で固定）
```

### linear-tui から引き継ぐ不変条件

- **依存は内向きだけ。**
  - `interface/` と `infra/` は互いを参照しない。両者をつなぐのは `runtime.rs` / `main.rs` だけ。
  - 例外は `interface/cli` と `interface/mcp`。各サブコマンドが必要な infra を自分で組み立てる。
- **core は I/O をしない。**
  - use case は `Request` を返し、実行するのは `infra::dispatch`。
  - sessionize と Project 解決も core の純粋関数にし、fixture だけでテストできるようにする。
- **TUI の UI スレッドは await しない。**
  - `Request → dispatch → Message` のループで処理する。
  - Desktop は GPUI の `background_spawn` で同じ `Request` を実行する。
- **spool の形式と DayTimeline は外部との契約。**
  - フィールドは追加だけにする。互換性を壊す変更をするときは `event::VERSION` を上げる。
  - hook と MCP が書き込み、2 つのクライアントが読むため。
- **2 つのクライアントは `core::usecase::timeline` が返す `DayTimeline` を描画するだけにする。** 描画ロジックを 2 か所に重複させない。

### 開発環境

- `mise install` と `mise run verify` を検証のゲートにする。
  - TLS は rustls にする。
  - SQLite は `rusqlite` の `bundled` feature を使う。
  - これで OpenSSL と pkg-config は不要になる。
- Desktop だけは Linux で Vulkan / Wayland / xkbcommon / fontconfig が必要になる。
  - 手順は `docs/development.md` に書く。
  - 開発シェルは `desktop/shell.nix` で用意し、`desktop/with-deps.sh` 経由で mise のタスクから呼ぶ（NixOS 以外ではシステムのパッケージを使う）。
  - `mise run verify` は root package だけを対象にし、Desktop は `mise run verify:desktop` で別に検証する。

---

## 7. マイルストーン

| # | 内容 | 完了条件 |
|---|---|---|
| **M0** 土台（〜1 週） | linear-tui 準拠の骨格（core / interface / infra、`tests/architecture.rs`、`mise.toml`、AGENTS.md / CLAUDE.md、CI、release-plz）。spool の形式、SQLite の migration。`desktop/` で GPUI の hello world を NixOS 上で起動確認する | `mise run verify` が通り、`cargo run -p pulsline-desktop` でウィンドウが開く |
| **M1** Plugin の hooks（〜1 週） | `pulsline hook`、plugin の雛形、marketplace、ingest、`pulsline today`（テキスト出力）、transcript の import | プラグインを入れて作業すると、`pulsline today` にその日の agent session が出る |
| **M2** ジャーナル（〜1 週） | `pulsline mcp`（`journal_record` / `timeline_query`）、skill `journal`、Sessionizer v1 | 1 日の作業が「タイトル付きの work session」として出て、Claude に「昨日何した？」と聞くと答えられる |
| **M3** TUI（〜1〜2 週） | 日表示（時間軸と Session のバー）、Session の詳細、日の移動、ジャーナルの強調表示、spool の変化に追従する更新 | `pulsline tui` で 1 日を快適に振り返れる |
| **M4** GitHub / Linear（〜1〜2 週） | 同期、keyring への保存、Issue キーと SHA による結び付け | Session に PR と Issue が自動で付く |
| **M5** Desktop（〜2〜3 週） | GPUI の独自 Timeline Element（ズーム・パン）、Session の詳細、Week 表示 | **MVP 完了** |

TUI を先に作るのは、実装が速く、データモデルと Sessionizer の質を早く検証できるため。
GPUI では、その検証が済んだ view model を描画するだけになる。

### MVP 後の候補

- **日報と週報の Markdown 出力**（`pulsline report --day`。Slack や Linear に貼る用途）
- Codex 対応（transcript の import と、可能であれば hook）
- ジャーナルを Linear Issue にコメントとして書き戻す（手元の decision-journal-writer をローカル版として置き換えられる）

---

## 8. リスク

| リスク | 対策 |
|---|---|
| hook の仕様変更や入力形式の変化 | spool には受け取った入力の要約とバージョンを記録する。解釈は ingest 側で吸収する |
| hook が遅いと作業体験を損なう | 追記のみにして DB には触らない。起動の速い単一バイナリにする。処理時間を計測する |
| Claude が `journal_record` を適切に呼ばない、または呼びすぎる | skill の指示を調整し、`Stop` hook で「未記録の完了作業」を促すかを検討する。記録の頻度を統計で確認する |
| prompt などの機微な情報がローカルに残る | 既定では記録を最小限にする。記録する範囲は設定で選べるようにする。データは `~/.local/share` の中だけに置く |
| GPUI の API 変更 | crates.io 版（`gpui = "0.2.2"`）をバージョン固定で使い、更新は意図して行う。先に TUI で価値を確立しておく |

---

## 9. 未決事項

- **UserPromptSubmit で記録する範囲の既定値:** 全文 / 先頭 N 文字 / なし
- **対象エージェント:** MVP では Claude Code のみとし、Codex は MVP の後に回すか
- **マルチマシン:** 複数の PC の spool を統合するか。やるなら spool の同期（Syncthing や git）で対応でき、サーバーは不要
