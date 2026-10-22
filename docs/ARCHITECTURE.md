# Kiến trúc Klat

> **Vai trò của file này:** cấu trúc chuẩn của project. Mọi tính năng mới, mọi refactor đều đặt code theo đây.
> Nếu code và file này mâu thuẫn: code **cũ** là nợ cần trả (xem §11), code **mới** phải theo file này.
> Muốn đổi một quy tắc ở đây thì sửa file này trước (kèm lý do, §13), rồi mới viết code.
>
> **Phạm vi:** cấu trúc code, ranh giới tầng, pattern. Hành vi sản phẩm nằm ở [`KLAT_SPEC.md`](../KLAT_SPEC.md) và
> [`docs/specs/`](specs/).
>
> **Cập nhật:** 2026-10-16 · **Trạng thái migrate:** §11 (code hiện tại chưa theo cấu trúc đích).

---

## Mục lục

1. [Cách dùng file này khi làm tính năng](#1-cách-dùng-file-này-khi-làm-tính-năng)
2. [Nguyên tắc](#2-nguyên-tắc)
3. [Bức tranh tổng thể](#3-bức-tranh-tổng-thể)
4. [Quy tắc phụ thuộc](#4-quy-tắc-phụ-thuộc)
5. [Vai trò từng tầng](#5-vai-trò-từng-tầng)
6. [Luồng dữ liệu chuẩn](#6-luồng-dữ-liệu-chuẩn)
7. [Pattern bắt buộc và anti-pattern cấm](#7-pattern-bắt-buộc-và-anti-pattern-cấm)
8. [Đặt code ở đâu](#8-đặt-code-ở-đâu)
9. [Công thức thêm tính năng](#9-công-thức-thêm-tính-năng)
10. [Những gì cố ý không làm](#10-những-gì-cố-ý-không-làm)
11. [Lộ trình migrate và giai đoạn chuyển tiếp](#11-lộ-trình-migrate-và-giai-đoạn-chuyển-tiếp)
12. [Checklist trước khi merge](#12-checklist-trước-khi-merge)
13. [Sửa file này](#13-sửa-file-này)

---

## 1. Cách dùng file này khi làm tính năng

1. Xác định tính năng chạm vào **tầng nào** bằng bảng §8.
2. Nếu có công thức sẵn ở §9, làm theo đúng thứ tự trong đó (luôn đi từ trong ra ngoài: protocol → core → infra → app → frontend).
3. Viết test ở **tầng trong cùng** mà logic đó thuộc về (logic nằm ở `klat-core` thì test ở `klat-core`, không test qua Tauri).
4. Chạy checklist §12.
5. Nếu tính năng không vừa với cấu trúc này: **dừng lại**, đề xuất sửa §-tương-ứng của file này, đừng "lách" bằng một ngoại lệ trong code.

---

## 2. Nguyên tắc

| # | Nguyên tắc | Lý do |
|---|---|---|
| P1 | **Phụ thuộc chỉ đi một chiều, từ ngoài vào trong.** Lõi nghiệp vụ không biết Tauri, file, socket, đồng hồ hệ thống. | Logic thay đổi nhiều nhất (khi nào mèo thức, Ready, Pause…) phải rẻ nhất để sửa và test. |
| P2 | **Ranh giới được compiler ép, không dựa vào trí nhớ.** Tầng = crate Rust; quy ước chỉ dùng khi crate là quá mức. | Quy ước bằng lời sẽ mục dần; `use` sai chiều mà không compile được thì không thể mục. |
| P3 | **Chỉ tách crate khi có lợi cụ thể:** (a) ép chiều phụ thuộc, (b) test không cần compile Tauri, (c) dùng chung với `klat-hook` mà không làm nó phình. | 10k dòng không cần 10 crate. Mỗi crate thêm là thêm manifest, thêm thời gian link. |
| P4 | **Mỗi mẩu state có đúng một chủ.** Không global mutable, trừ callback OS bắt buộc (§7.2). | Biết ai được ghi thì mới suy luận được về race và lock. |
| P5 | **Hợp đồng giữa hai process có một nguồn sự thật**: hook ↔ app (crate `klat-protocol`), Rust ↔ TypeScript (type sinh tự động). | Đây là ranh giới compiler không tự nhìn thấy; drift ở đây hỏng âm thầm lúc chạy. |
| P6 | **Dùng type, không dùng chuỗi và số trần.** Enum thay chuỗi, newtype cho đơn vị (ms, px vật lý/logic/CSS). | Lịch sử project có nhiều bug DPI/scale: đúng loại bug mà newtype chặn được lúc compile. |
| P7 | **Riêng tư và bảo mật là một phần kiến trúc**, không phải lớp phủ: hook chỉ gửi tên, event server là bề mặt tấn công duy nhất và đứng riêng. | Spec D8: local-first, không bao giờ gửi prompt/lệnh/tool input. |
| P8 | **Đơn giản trước, pattern sau.** Chỉ thêm trait/abstraction khi có ≥ 2 implementation thật hoặc test cần. | Xem §10. |

---

## 3. Bức tranh tổng thể

```
 Claude Code ──(stdin JSON)──▶ klat-hook ──(HTTP loopback + token)──┐
                                                                    ▼
 ┌──────────────────────────────── klat (src-tauri) ─────────────────────────────────┐
 │  bootstrap/  composition root: Builder, plugins, .manage(), setup, lifecycle        │
 │  ipc/        #[tauri::command] mỏng + events (tên + payload có type)                │
 │  services/   CatService (mèo + settings, một lock), settings_service, hooks_service │
 │  ui/         overlay, settings_window, tray: subscribe state, gửi Command          │
 │  tasks/      cursor_poller, drag_loop, wait_ticker                                  │
 │  platform/   windows.rs | macos.rs | fallback.rs, webview_memory: mọi unsafe/FFI   │
 └───────────────┬──────────────────────────────────────┬────────────────────────────┘
                 ▼                                      ▼
        ┌──────────────────┐                   ┌──────────────────┐
        │   klat-infra     │─────────────────▶│    klat-core     │
        │ file, socket, log│                   │ logic thuần, 0 IO│
        └────────┬─────────┘                   └────────┬─────────┘
                 ▼                                      ▼
        ┌──────────────────────────────────────────────────────────┐        ┌────────────┐
        │                     klat-protocol                         │◀──────│ klat-hook  │
        │ HookEvent, HookName, RuntimeInfo, data_dir, hằng số wire  │        │  (binary)  │
        └──────────────────────────────────────────────────────────┘        └────────────┘

 Frontend (src/, WebView) ◀──── events cat://… / invoke(command) ────▶ ipc/
```

### Bố cục workspace đích

```
ClauCat/
├─ Cargo.toml                 # [workspace] + [workspace.dependencies] + [workspace.lints]
├─ crates/
│  ├─ klat-protocol/          # deps: serde (derive tuỳ feature). KHÔNG serde_json bắt buộc.
│  ├─ klat-core/              # deps: klat-protocol, serde, serde_json
│  ├─ klat-infra/             # deps: klat-core, klat-protocol, getrandom, log
│  └─ klat-hook/              # deps: klat-protocol, serde_json
├─ src-tauri/                 # crate `klat`
│  └─ src/
│     ├─ main.rs              # ≤ 10 dòng: platform::scrub_env(); bootstrap::run()
│     ├─ bootstrap/
│     ├─ ipc/{commands/, events.rs, error.rs}
│     ├─ services/
│     ├─ ui/
│     ├─ tasks/
│     └─ platform/
├─ src/                       # frontend (§5.6)
└─ scripts/fake-cc/           # công cụ dev, ngoài kiến trúc
```

---

## 4. Quy tắc phụ thuộc

### 4.1 Rust

| Crate / module | Được phụ thuộc | **Cấm** |
|---|---|---|
| `klat-protocol` | `serde`, `std` (trừ `fs`/`net` ngoài `data_dir`) | mọi crate klat khác, `tauri`, `serde_json` bắt buộc |
| `klat-core` | `klat-protocol`, `serde`, `serde_json`, `thiserror` | `tauri`, `std::fs`, `std::net`, `std::thread`, `SystemTime::now`, `Instant::now`, `getrandom`, `log` (core không log: trả kết quả để tầng ngoài log) |
| `klat-infra` | `klat-core`, `klat-protocol`, `getrandom`, `log` (facade), `thiserror`, `std::{fs,net,thread}` | `tauri`, bất kỳ thứ gì trong `src-tauri` |
| `klat-hook` | `klat-protocol`, `serde_json` | `klat-core`, `klat-infra`, async runtime, crate làm tăng binary > 10% |
| `klat` / `ipc` | `services`, type từ core | gọi thẳng `infra`, giữ state |
| `klat` / `services` | core, infra, `platform` (khi cần) | `ui`, `ipc`, `tasks` |
| `klat` / `ui`, `tasks` | `services` (subscribe + `dispatch`), `platform`, core (type) | gọi chéo nhau (`ui` ↔ `tasks`), tự giữ state nghiệp vụ |
| `klat` / `platform` | `std`, crate OS (`windows-sys`, `core-graphics`…), `tauri` (type cửa sổ) | `services`, `ui`, logic nghiệp vụ |
| `klat` / `bootstrap` | mọi thứ (đây là nơi nối dây) | logic |

### 4.2 Cách ép buộc

- Ranh giới crate: compiler tự ép.
- `[workspace.lints]`: `unsafe_code = "forbid"` cho `klat-protocol`, `klat-core`, `klat-infra`, `klat-hook`; trong `klat` chỉ `platform/` có `#![allow(unsafe_code)]`.
- `clippy.toml` của `klat-core`: `disallowed-methods` cho `std::time::SystemTime::now`, `std::time::Instant::now`, `std::thread::spawn`.
- Trong `klat`: chưa có công cụ ép giữa các module, nên review theo bảng 4.1 (checklist §12).
- Frontend: `no-restricted-imports` trong `eslint.config.js` sinh luật từ bảng §4.3 (`LAYERS`), chạy trong `pnpm lint`. Thêm thư mục cấp cao mới trong `src/` thì thêm nó vào `LAYERS`.

### 4.3 Frontend (`src/`)

Thứ tự tầng, trong ra ngoài. Một thư mục chỉ được import từ thư mục **cùng tầng hoặc tầng trong hơn**.

| Tầng | Thư mục | Vai trò | Ghi chú |
|---|---|---|---|
| 0 | `settings.ts`, `i18n/`, `phrases/` | Dữ liệu và hàm thuần | Không DOM, không Three.js, không Tauri |
| 1 | `kit/` | Bộ dựng toon dùng chung: mesh, viền, mặt, fx, chuyển động, kiểu `CharacterRig`; `kit/pose.ts`: `CatPose`, `POSE_PARAMS`, mốc thời gian knockout | Không biết nhân vật cụ thể nào |
| 2 | `characters/` (`cat/`, `snake/`, `dragon/`, `capybara/`) | Mỗi nhân vật một rig + registry `characters/index.ts` | Chỉ import `kit/` + tầng 0 |
| 3 | `cat/` (bộ não + stage), `bubble/` | `CatController` (thuần), `CatStage`, `RenderLoop`, bong bóng | Lấy rig qua registry, không import rig cụ thể |
| 4 | `bridge/` | **Nơi duy nhất** import `@tauri-apps/*`; type sinh từ Rust | `bridge/generated.ts` chỉ có type, không code: mọi tầng được `import type` từ nó (coi như tầng 0) |
| 5 | `overlay/`, `settings-window/` | Entry point, composition root của từng cửa sổ | Nối bridge với tầng 0–3 |

---

## 5. Vai trò từng tầng

### 5.1 `klat-protocol`: hợp đồng giữa hai process

**Chứa:** `HookEvent` (body `POST /event`, field `agent` cho Codex), `enum HookName` + `fn is_installed(self, agent)`, `enum Agent` (Claude Code, Codex), `RuntimeInfo { port, token, pid }`, `fn is_valid_token`, `data_dir()`/`runtime_path()`, hằng số `EVENT_PATH`, `MAX_BODY`.

**Không chứa:** logic nghiệp vụ (`classify` thuộc core), I/O mạng.

**Lý do:**
- `klat-hook` và app là hai binary có thể lệch version (app đã cập nhật, `~/.claude/settings.json` vẫn trỏ hook cũ). Một type duy nhất để compiler bắt drift.
- Một `enum HookName` thay cho hai danh sách chuỗi (`ALLOWED_HOOKS` 10 tên, `HOOK_EVENTS` 9 tên); quan hệ "SubagentStop được nhận nhưng không cài" thành code tường minh.
- `klat-hook` chạy ở **mỗi** hook của Claude: crate này phải nhẹ. Mọi thay đổi đo lại kích thước và thời gian khởi động hook, ghi vào `docs/verification.md`.

**Tương thích:** field mới trong `HookEvent` luôn là `Option` + `#[serde(default)]`; không đổi tên/xoá field đã phát hành. App phải nhận event từ hook cũ hơn nó.

### 5.2 `klat-core`: nghiệp vụ thuần, có tính quyết định

| Module | Nội dung |
|---|---|
| `session` | `SessionStore`, `classify`, `Signal`, `Gate`, dedupe, `MAX_SESSIONS` |
| `cat` | `CatModel` (reducer), `Command`, `Ctx`, `CatState`, `Attention`, `SleepCause` |
| `settings` | `Settings` + enum, đọc khoan dung (`from_object`), `SettingsPatch`, clamp, migrate legacy |
| `geometry` | `Point<U, N>` / `Rect<U, N>` với đơn vị `Css` / `Logical` / `Physical` (§7.5), `WindowPos`, `OverlayFrame`, `to_overlay`, `hit_test`, `clamp_window`, `is_on_screen`, `keep_bottom_right`, `bottom_right`, `LookTracker` |
| `agent_hooks` | `status` / `install_into` / `uninstall_from` trên `serde_json::Value` (không đọc ghi file) cho từng `Agent` (Claude Code: exec form; Codex: một dòng lệnh shell), lỗi `UnexpectedShape`; golden test trong `testdata/claude_settings/` và `testdata/codex_hooks/` |
| `i18n` | Catalog + `t(lang, key)`, `tf(...)`; không phụ thuộc `AppHandle` |
| `ratelimit` | `RejectLog` |
| `time` | `UnixMs`, `DurationMs` |

Hình dạng chuẩn của state machine:

```rust
pub enum Command<'a> {
    Hook(&'a HookEvent),
    SetReady(bool),
    DismissAll,
    WakeTest,
    ExpireStale { stale_after: DurationMs },
    ClearAnswered,
    SettingsChanged,
}

pub struct Ctx { pub now: UnixMs, pub paused: bool }

/// Core không log: trả đủ để tầng ngoài log và quyết định publish.
pub struct Handled { pub publish: bool, pub hook: Option<Outcome> }

impl CatModel {
    pub fn handle(&mut self, cmd: Command<'_>, ctx: Ctx) -> Handled;
    /// Snapshot chỉ cần Pause, không cần thời gian.
    pub fn snapshot(&self, paused: bool) -> CatState;
}
```

**Lý do:** mọi quy tắc "thức/ngủ/la lại/sleep_cause" trở thành bảng test `(state, command, now) → (state', snapshot)` chạy trong mili giây bằng `cargo test -p klat-core`, không compile Tauri. Thời gian truyền vào nên test không bao giờ flaky.

### 5.3 `klat-infra`: adapter I/O, không biết Tauri

| Module | Vai trò |
|---|---|
| `settings_repo` | load/save atomic (tmp + rename), đổi tên file hỏng `.corrupt.<ts>`, migrate thư mục cũ |
| `runtime_file` | ghi `runtime.json` 0600, `create_new`, xoá khi thoát |
| `event_server` | HTTP loopback: deadline cả request, token so sánh thời gian hằng, allowlist, giới hạn kết nối, giới hạn log từ chối (theo từng server, không `static`). `start(token, clock: impl Fn() -> UnixMs, on_event: impl Fn(HookEvent))`; lỗi là `enum Reject` → `status()`; test 2000 request rác seed cố định |
| `hook_files` | đọc/ghi `~/.claude/settings.json` và `~/.codex/hooks.json` (`$CODEX_HOME`) có backup; biến đổi bằng `core::agent_hooks`; `InstallError` (thiserror) |
| `log` | backend của facade `log` (`FileLogger`): file có giới hạn dung lượng, một dòng mỗi bản ghi, không bao giờ ghi nội dung prompt; chỉ nhận target `klat`/`klat_*`, bỏ record của tauri/wry/tao |

**Lý do:** `event_server` là bề mặt tấn công duy nhất; đứng riêng thì test được `handle()` bằng input rác/ngẫu nhiên, test socket thật không cần WebView, review bảo mật cô lập. Tách khỏi core để core không có `std::fs`/`std::net`.

### 5.4 `klat` (`src-tauri`): lớp vỏ

| Module | Làm gì | Không làm gì |
|---|---|---|
| `bootstrap/` | `tauri::Builder`, plugin, `.manage()`, `setup`, khởi động task, dọn dẹp lúc `RunEvent::Exit` | Logic |
| `ipc/commands/` | Validate input → gọi service → trả kết quả. Mỗi command ≤ ~10 dòng. Chia file theo miền: `cat.rs`, `settings.rs`, `hooks.rs`, `overlay.rs`, `drag.rs` | Lock state, I/O trực tiếp, `if` nghiệp vụ |
| `ipc/events.rs` | Tên event `cat://…` (const) + struct payload; nguồn sinh type TS | |
| `ipc/error.rs` | `Message { key, vars }` (key trong `src/i18n` + giá trị placeholder) và `IpcError { code, message }`, serialize được | Chuỗi lỗi tự do, chữ hiển thị soạn sẵn |
| `services/` | **Chủ duy nhất** của state: `Mutex<Model>` + `watch::Sender<Snapshot>`; `dispatch(Command)` | Gọi `ui`/`tray` trực tiếp |
| `ui/` | `overlay`, `settings_window`, `tray`: subscribe snapshot, cập nhật hiển thị; hành động người dùng → `service.dispatch(...)` | Giữ state nghiệp vụ, gọi sang `ui` khác |
| `tasks/` | Vòng lặp nền: `cursor_poller`, `drag_loop`, `wait_ticker` | Như `ui/` |
| `platform/` | API chung ở `mod.rs`; mỗi OS một file. Toàn bộ `unsafe`, FFI, `#[cfg(target_os)]`, `webview_memory`, `scrub_env` | Logic nghiệp vụ |

**Lý do:** command là public API cho JavaScript; giữ mỏng thì nhìn một chỗ thấy toàn bộ bề mặt webview gọi được (quan trọng với capabilities). `platform/` gom `unsafe` về một chỗ để audit, port OS mới chỉ thêm một file.

### 5.5 `klat-hook`

Đọc stdin (tối đa 256 KB), giữ đúng các field cần, POST tới app, **luôn in rỗng và exit 0**, tổng thời gian ≤ 300 ms. Không bao giờ gửi `tool_input`, lệnh, transcript, đường dẫn đầy đủ (chỉ `basename(cwd)`). Dựng payload bằng type `HookEvent` của `klat-protocol`, không dùng `json!{}`.

### 5.6 Frontend

- `CatController` là bộ não **thuần** (không Three.js, không DOM); rig chỉ quyết định thân hình hiển thị `CatPose` thế nào (pattern Bridge). Giữ nguyên ranh giới này.
- `bridge/` là nơi duy nhất nói chuyện với Tauri; type lấy từ file sinh (`src/bridge/generated.ts`), không viết tay lại struct Rust.
- Logic của Settings window nằm ở `viewModel.ts` (hàm thuần, có test); `sections/` chỉ render.

---

## 6. Luồng dữ liệu chuẩn

### 6.1 Hook event → mèo thức

```
Claude Code
  └─▶ klat-hook: stdin → HookEvent (protocol) → POST /event
        └─▶ infra::event_server: kiểm token, Origin, kích thước, allowlist → HookEvent
              └─▶ services::CatService::dispatch(Command::Hook(ev))
                    ├─ lock Model (một lock duy nhất)
                    ├─ core::CatModel::handle(cmd, Ctx { now, paused })   // thuần
                    ├─ core::CatModel::snapshot(ctx) → CatState
                    └─ watch::Sender::send_if_modified(snapshot)
                          ├─▶ ui::tray        (số phiên chờ, tick Ready)
                          ├─▶ platform::webview_memory (mức bộ nhớ khi thức/ngủ)
                          ├─▶ ipc::events     emit cat://state → frontend
                          └─▶ tasks::cursor_poller đọc rx.borrow().awake
```

### 6.2 Hành động người dùng

```
Frontend click / tray menu
  └─▶ ipc::commands::dismiss  (hoặc ui::tray handler)
        └─▶ CatService::dispatch(Command::DismissAll)   // cùng một đường như 6.1
```

### 6.3 Đổi settings

```
Settings window ─invoke set_settings(patch: SettingsPatch)─▶ ipc → CatService::update_settings
  │   (một lock cho cả mèo và settings: không ai thấy settings mới với sleep_cause cũ)
  ├─ core::settings::Settings::apply (đọc khoan dung + clamp)
  ├─ infra::settings_repo::save (atomic)
  ├─ core::CatModel::handle(Command::SettingsChanged)
  └─ watch<Settings> → tray (nhãn, ngôn ngữ), settings_window (tiêu đề), emit cat://settings
```

`services/settings_service.rs` giữ phần quanh model: thư mục dữ liệu, đọc file lúc khởi động, Start at login (sự thật
là entry của OS).

**Quy tắc:** mọi thay đổi state đi qua `dispatch`. Không có đường tắt "sửa field rồi gọi publish".

---

## 7. Pattern bắt buộc và anti-pattern cấm

### 7.1 Bắt buộc

| Pattern | Áp dụng | Lý do |
|---|---|---|
| **Functional core / imperative shell** | Logic ở `klat-core`, I/O ở `klat-infra`/`klat` | Test nhanh, có tính quyết định |
| **Command + reducer** | Mọi mutation state = một biến thể `Command`; `CatModel::handle` | Một chỗ để lock, log, publish; thêm hành động = thêm biến thể, compiler chỉ ra mọi `match` cần sửa |
| **Observer qua `tokio::sync::watch`** | Service phát snapshot; `ui`/`tasks`/emit là subscriber | Phá vòng phụ thuộc; thêm phản ứng mới = thêm subscriber, không sửa lõi; subscriber chậm chỉ bỏ qua trạng thái trung gian |
| **Anti-corruption layer** | `classify(HookEvent) -> Option<Signal>` | Từ vựng của Claude Code không lọt vào nghiệp vụ |
| **Specification / policy object** | `Gate::lets_through` | Quy tắc "ai được đánh thức mèo" có tên, một chỗ |
| **Clock injection** | `now: UnixMs` truyền vào core; infra nhận `clock: impl Fn()` | Test thời gian không flaky |
| **Inject bằng closure trước, trait sau** | `event_server::start(token, on_event)`, `data_dir_for(os, env)` | Đủ để test, không cần `dyn` |
| **Tolerant reader** | Đọc settings từng field, mặc định khi thiếu/sai, clamp khi ngoài khoảng | File hỏng không bao giờ làm app chết |
| **Atomic write** | tmp + `create_new` + `rename`; 0600 cho file chứa bí mật | Không ghi dở, không đi theo symlink cài sẵn |
| **Parse, don't validate** | Input từ ngoài (HTTP, IPC, file) được parse thành type đã hợp lệ ngay ở biên | Code bên trong không cần kiểm tra lại |
| **Bridge (frontend)** | `CatController` ↔ `CharacterRig` | Thêm nhân vật không đụng animation |
| **Registry + factory (frontend)** | `characters/index.ts` với `Record<CharacterId, …>` | Quên đăng ký nhân vật là lỗi compile |

### 7.2 State và `static`

- State nghiệp vụ: trong service, đăng ký bằng `app.manage()`.
- State UI/OS (zoom overlay, đang preview, đang kéo, hitbox, menu tray): struct riêng (`ui::overlay::OverlayState`, `ui::tray::TrayHandles`), tạo trong `bootstrap`, `app.manage()` cho command và truyền `Arc` vào task.
- `static` mutable **chỉ** được dùng trong `platform/`, khi callback OS không nhận con trỏ user data (Windows `WH_KEYBOARD_LL`, macOS `CGEventTap`). Bắt buộc có comment nêu lý do. Dạng cho phép: `OnceLock<Sender<…>>`, atomic.
- Một service = một `Mutex`. Không giữ hai lock cùng lúc; nếu buộc phải giữ, ghi thứ tự lock trong doc comment của struct.

### 7.3 Lỗi

- `klat-core`, `klat-infra`: `enum` lỗi bằng `thiserror`, một enum cho mỗi miền (`InstallError`, `SettingsError`…).
- `ipc`: chuyển thành `IpcError { code, message: Message { key, vars } }` (`impl From<…> for IpcError` trong `ipc/error.rs`); kết quả cần nói thành lời cũng là `Message`. Frontend dịch bằng `tMessage` theo ngôn ngữ đang dùng: Rust không gửi chữ hiển thị.
- Cấm `Result<_, String>` ở API mới. Cấm `unwrap()`/`expect()` trên dữ liệu từ ngoài; chỉ chấp nhận `expect` khi khởi động (spawn thread, build app) với thông điệp rõ.
- `Mutex::lock().unwrap()` được chấp nhận (poison = đã panic ở chỗ khác).

### 7.4 Log và riêng tư

- Dùng facade `log` (`log::info!`, `log::warn!`); backend là `klat-infra::log`.
- Chỉ log tên event, kết quả, 8 ký tự đầu session id, tên thư mục project. **Không bao giờ** log prompt, lệnh, tool input, token, đường dẫn đầy đủ.
- Nguồn có thể bị spam (request bị từ chối…) phải đi qua `ratelimit`.

### 7.5 Type thay cho chuỗi và số trần

| Thay vì | Dùng |
|---|---|
| `hook_event_name: String` | `HookName` |
| `corner: String` | `enum Corner` |
| `Map<String, Value>` + `PATCHABLE` | `struct SettingsPatch { field: Option<T>, … }`, đọc khoan dung như file (key lạ bỏ qua, giá trị sai kiểu thành default): không `deny_unknown_fields`, để cửa sổ Settings khác phiên bản vẫn lưu được |
| `Result<_, u16>` cho HTTP | `enum Reject` → `fn status(self) -> u16` |
| `u64` cho thời gian | `UnixMs`, `DurationMs` |
| `f64`/tuple cho toạ độ | `Css<Point>`, `Logical<Point>`, `Physical<Point>`; chuyển đổi chỉ qua hàm có tên nhận `zoom`/`scale` |
| Chuỗi id menu tray | `enum TrayAction` |

### 7.6 Hợp đồng IPC với frontend

- Struct đi qua IPC: `#[derive(Serialize)] #[serde(rename_all = "camelCase")]` + `ts_rs::TS`. Trong `klat-core`/`klat-infra`: `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]`; trong app: `#[cfg_attr(test, derive(ts_rs::TS))]`. Feature `ts` chỉ được bật qua dev-dependency của `src-tauri`, nên bản release không có ts-rs.
- Type mới qua IPC: thêm một dòng `declare::<T>()` trong `src-tauri/src/ipc/bindings.rs`, chạy `pnpm gen:types`, commit cả `src/bridge/generated.ts`. Test trong `bindings.rs` so file với type Rust, nên `pnpm check` (bước `test:rust`) fail khi file cũ (repo không có remote/CI: gate chạy cục bộ).
- Type có tham số đơn vị (`Point<U>`, `Rect<U>`): TS nhận hình dạng qua `klat_core::geometry::wire`, có test khẳng định serialize giống nhau.
- Tên event: const trong `ipc/events.rs`, tiền tố `cat://`.

### 7.7 Cấm

| Anti-pattern | Thay bằng |
|---|---|
| Service locator `app.state::<T>()` trong core/infra hoặc trong code ngoài `ipc`/`bootstrap` | Truyền service/handle cụ thể vào hàm/struct |
| Fan-out cứng (`publish` gọi thẳng tray, webview…) | Subscriber `watch` |
| Hàm `pub fn do_x(app: &AppHandle)` tự lock + mutate + publish | `dispatch(Command::X)` |
| Global mutable ngoài `platform/` | §7.2 |
| `#[path = …]` để dùng chung file giữa crate | Crate dùng chung |
| Viết tay struct Rust sang TS | Type sinh |
| `#[cfg(target_os)]` ngoài `platform/` và `Cargo.toml` | API trong `platform/mod.rs` |

---

## 8. Đặt code ở đâu

| Bạn đang viết… | Đặt ở |
|---|---|
| Quy tắc quyết định (khi nào thức, ai được đánh thức, dedupe, hết hạn) | `klat-core::cat` / `klat-core::session` |
| Ánh xạ một hook/notification mới của Claude Code | `klat-protocol::HookName` + `klat-core::session::classify` |
| Field mới trong payload hook | `klat-protocol::HookEvent` (Option) + `klat-hook` |
| Setting mới: mặc định, khoảng hợp lệ, migrate | `klat-core::settings` |
| Tính toán hình học (hit test, clamp vào màn hình, đổi đơn vị) | `klat-core::geometry` |
| Đọc/ghi file, socket, sinh token | `klat-infra` |
| Biến đổi JSON `~/.claude/settings.json`, `~/.codex/hooks.json` | `klat-core::agent_hooks` (thuần) + `klat-infra::hook_files` (I/O) |
| Gọi API OS, `unsafe`, FFI | `src-tauri/src/platform/<os>.rs` |
| Hàm frontend gọi được | `src-tauri/src/ipc/commands/<miền>.rs` |
| Event đẩy sang frontend | `src-tauri/src/ipc/events.rs` |
| Phản ứng khi state đổi (tray, âm thanh, notification) | Subscriber mới trong `src-tauri/src/ui/` |
| Vòng lặp nền | `src-tauri/src/tasks/` |
| Cửa sổ mới | `src-tauri/src/ui/<cửa_sổ>.rs` + entry `src/<cửa-sổ>/` + `*.html` |
| Chữ hiển thị (tray, dialog, Settings) | `src/i18n/{en,vi}.json` (Rust đọc chung) |
| Câu mèo chửi | `src/phrases/phrases.ts` |
| Nhân vật mới | `src/characters/<id>/` (§9.5) |
| Hoạt ảnh/biểu cảm dùng chung mọi nhân vật | `src/kit/pose.ts` (`POSE_PARAMS`) + spring trong `src/cat/CatController.ts` (`SPRINGS`, compiler bắt thiếu) + `src/kit/` |

---

## 9. Công thức thêm tính năng

Luôn theo thứ tự trong → ngoài; mỗi bước có test ở tầng của nó.

### 9.1 Phản ứng với một hook / notification mới của Claude Code hoặc Codex

1. `klat-protocol`: thêm biến thể `HookName`; quyết định `is_installed(agent)` cho từng agent.
2. `klat-core::session::classify`: ánh xạ sang `Signal`; test bảng trong `session`.
3. Nếu cần field mới: `HookEvent` (Option) → `klat-hook::build_event` → test `crates/klat-hook/tests/cli.rs`.
4. `klat-infra::event_server`: không cần sửa (allowlist lấy từ `HookName`); chạy lại test.
5. Cài lại hook: installer tự lấy danh sách từ `HookName::installed(agent)`; golden `testdata/` đổi theo.
6. Cập nhật bảng ánh xạ ở doc comment đầu `session` và `docs/hooks-troubleshooting.md`.

### 9.2 Thêm một setting

1. `klat-core::settings`: field + default + clamp/enum; `from_object` đọc khoan dung; nếu người dùng sửa được thì thêm vào `SettingsPatch`. Test: thiếu, sai kiểu, ngoài khoảng, file cũ.
2. Nếu ảnh hưởng quyết định thức/ngủ: đưa vào `Ctx` và test reducer.
3. `ipc`: không cần command mới nếu đi qua `set_settings`.
4. Sinh lại type TS; `src/settings.ts` dùng type sinh.
5. UI: `settings-window/viewModel.ts` (+ test) → `sections/<mục>.ts`; chữ ở `src/i18n/{en,vi}.json`.
6. Cập nhật `KLAT_SPEC.md` §10.

### 9.3 Thêm một hành động người dùng (nút, menu tray)

1. `klat-core::cat::Command`: thêm biến thể; xử lý trong `handle`; test.
2. `ipc/commands/<miền>.rs`: command mỏng gọi `dispatch`; đăng ký trong `bootstrap`; thêm vào `capabilities/default.json` nếu cần quyền.
3. Tray: thêm biến thể `TrayAction` trong `ui/tray.rs`.
4. Frontend: hàm trong `bridge/`, gọi từ entry tương ứng.

### 9.4 Thêm một phản ứng khi state đổi (âm thanh, notification hệ thống…)

1. File mới `src-tauri/src/ui/<tên>.rs` với `pub fn follow(app: &AppHandle, rx: watch::Receiver<CatState>)`, chạy
   vòng lặp bằng `services::on_each(rx, |state| …)`.
2. Gọi `follow(…, cat.subscribe())` trong `bootstrap`, trước khi event server khởi động.
3. Không sửa `services/` hay `klat-core`. Nếu thấy phải sửa, phản ứng đó đang cần dữ liệu chưa có trong snapshot: thêm vào `CatState` trước (bước riêng, có test).

### 9.5 Thêm một nhân vật

1. Thiết kế theo skill `klat-character`; sheet ở `docs/characters/<id>/`.
2. `src/characters/<id>/<Id>Rig.ts` implement `CharacterRig`, chỉ dùng `kit/`.
3. Đăng ký trong `characters/index.ts`; thêm id vào `CharacterId` (TS) và `enum Character` (`klat-core::settings`).
4. Test trong `tests/characters.test.ts`.
5. Không sửa `CatController` trừ khi cần param pose mới cho mọi nhân vật (thêm vào `kit/pose.ts` `POSE_PARAMS`, rồi spring trong `SPRINGS`).
6. Kiểm ảnh: `pnpm dev` → `/characters.html?c=<id>` in hash pixel từng ô; refactor không được đổi hash của nhân vật khác.

### 9.6 Thêm hỗ trợ một hệ điều hành

1. `src-tauri/src/platform/<os>.rs` implement đủ API của `platform/mod.rs`.
2. `crates/klat-protocol::data_dir`: nhánh OS + test qua `data_dir_for`.
3. Dependency OS dưới `[target.'cfg(...)'.dependencies]`, pin cùng version Tauri đang kéo.
4. `docs/<os>-build.md`, `docs/<os>-install.md`, bổ sung `docs/verification.md`.

### 9.7 Thêm một agent khác (Cursor…)

Codex đã đi theo công thức này (2026-10): `Agent::Codex`, `klat-hook --agent codex`, `hooks.json`, một card riêng
trong Settings → Hooks.

1. `klat-protocol`: thêm biến thể `Agent`; `HookEvent.agent` là chuỗi trên wire (thiếu = Claude Code, để tương thích),
   đọc bằng `HookEvent::agent()`. Quyết định `HookName::is_installed(agent)`.
2. Adapter: nếu agent gửi cùng field như Claude Code thì chỉ cần `--agent <tên>`; nếu khác thì thêm chế độ trong
   `klat-hook` dịch định dạng đó sang `HookEvent`.
3. `classify`: event của agent lạ bị bỏ qua; tên giống nhau mang nghĩa giống nhau. Lõi phía sau (`Signal` trở đi) không
   đổi.
4. `agent_hooks::our_entry` (dạng entry agent đó hiểu) + golden; `hook_files::hooks_file_path`.
5. Settings → Hooks tự có thêm card (`Agent::ALL`); chữ `settings.hooks.agent.<tên>` trong `src/i18n`.

---

## 10. Những gì cố ý không làm

| Không làm | Lý do |
|---|---|
| Thay event server bằng axum/hyper | Server nhỏ, đã harden và test (deadline, slow-drip, giới hạn kết nối). Framework web thêm hàng trăm dependency vào bề mặt tấn công và binary |
| Async/tokio trong `klat-hook` | Thời gian khởi động hook là chỉ số sống còn |
| Async hoá thread nền | Polling chu kỳ cố định; async không lợi gì, khó đọc hơn |
| Trait Repository / `dyn Clock` / DI container | Closure và tham số `now` đủ để test; composition root viết tay rõ hơn container |
| Event sourcing / CQRS | State nhỏ, không cần lịch sử; Command + snapshot là đủ |
| Actor framework | `Mutex` + `watch` giải quyết được, không thêm dependency |
| Tách `platform/` thành crate | Cần type cửa sổ của Tauri; tách lúc này chưa có lợi (xét lại khi có ≥ 3 OS) |

---

## 11. Lộ trình migrate và giai đoạn chuyển tiếp

### 11.1 Trạng thái

Theo dõi trong beads: epic **`ClauCat-dv6`** (`bd show ClauCat-dv6`). Mỗi phase là một issue con, có task con với
design, acceptance và OUTPUT cụ thể; quy trình chất lượng bắt buộc cho mọi task nằm trong phần design của epic.

| Phase | Bead | Nội dung | Tiêu chí xong | Trạng thái |
|---|---|---|---|---|
| 0 | `ClauCat-dv6.1` | Baseline: chốt thay đổi đang dở, gate một lệnh `pnpm check` (§12), đo `klat-hook` + số test, kịch bản hồi quy | Test xanh, có số liệu baseline và kịch bản hồi quy | Xong 2026-10-16 |
| 1 | `ClauCat-dv6.2` | `klat-protocol`; bỏ `#[path]`; `HookName` | Hook và app dùng chung type; hook không tăng > 10% | Xong 2026-10-16 |
| 2 | `ClauCat-dv6.3` | `klat-core`: `session`, `geometry` (newtype đơn vị), `settings` (phần thuần), `claude_hooks`, `i18n`, `CatModel` | `cargo test -p klat-core` không compile Tauri | Xong 2026-10-16 |
| 3 | `ClauCat-dv6.4` | `klat-infra`: facade `log`, `event_server`, `runtime_file`, `settings_repo`, `claude_settings` | Không còn `crate::` trong event server; test socket xanh | Xong 2026-10-16 |
| 4 | `ClauCat-dv6.5` | `klat`: `CatService` + `watch`, subscriber, xoá `static`, chia `ipc/ui/tasks/platform` | Không còn `crate::tray::` trong service; hồi quy Windows pass, macOS do owner | Phần agent xong 2026-10-16 (4.1–4.4, `pnpm regress` 18/18); chờ owner chạy O1–O10 trên Windows và O7 trên Mac |
| 5 | `ClauCat-dv6.6` | Type TS sinh tự động, `IpcError`; frontend: `CatPose` sang `kit/`, phá vòng `cat` ↔ `characters`, gộp `settings-window/api.ts` vào `bridge/`, ESLint ép tầng | `tauriBridge.ts` dùng type sinh; `pnpm check` kiểm drift | Xong 2026-10-16 |

Mỗi phase = một chuỗi commit **không đổi hành vi**, test xanh ở mọi commit. Cập nhật cột "Trạng thái" khi phase xong.

Sau Phase 4 còn lại, có chủ ý:
- `tasks/` vẫn dùng `ui::overlay` (`get`, `work_area`, `OverlayState`): cursor poller và drag loop làm việc trực
  tiếp trên cửa sổ overlay.
- `services::settings_service` nhận `AppHandle` vì plugin autostart cần nó; `CatService` thì không.
- Event giờ do subscriber gửi, sau khi command trả về; snapshot hoặc settings giống hệt không bị gửi lại.

### 11.2 Ánh xạ file cũ → vị trí mới (lịch sử)

Các file bên trái không còn; bảng giữ lại để đọc commit và bead cũ.

| Hiện tại (`src-tauri/src/`) | Đích |
|---|---|
| `session_store.rs` | `klat-core::session` (`HookEvent`, `ALLOWED_HOOKS` → `klat-protocol`) |
| `state.rs` | `Inner`/`snapshot`/`bump_attention` → `klat-core::cat`; `AppState`/`publish_state`/`update_settings` → `services/` |
| `settings.rs` | Model + validate → `klat-core::settings`; load/save/migrate → `klat-infra::settings_repo` |
| `event_server.rs` | `klat-infra::event_server`; `RuntimeInfo` → `klat-protocol`; ghi file → `klat-infra::runtime_file`; `RejectLog` → `klat-core::ratelimit` |
| `hooks_installer.rs` | Biến đổi JSON → `klat-core::claude_hooks`; I/O → `klat-infra::claude_settings` (từ 2026-10: `agent_hooks` / `hook_files`, thêm Codex); `HOOK_EVENTS` → `HookName::is_installed` |
| `hooks_ui.rs` | `services/hooks_service.rs` |
| `i18n.rs` | `klat-core::i18n` (`current(app)` → service settings) |
| `log.rs` | `klat-infra::log` (backend facade `log`) |
| `cursor.rs` | Hình học → `klat-core::geometry`; vòng lặp → `tasks/cursor_poller.rs` |
| `drag.rs`, `drag_macos.rs` | Hình học → `klat-core::geometry`; vòng lặp → `tasks/drag_loop.rs`; hook bàn phím → `platform/{windows,macos}.rs` |
| `overlay.rs`, `settings_window.rs`, `tray.rs` | `ui/` (`static` → `OverlayState`, `TrayHandles`) |
| `webview_memory.rs` | `platform/webview_memory.rs` (subscriber) |
| `autostart.rs` | `services/settings_service.rs` + plugin trong `bootstrap/` |
| `commands.rs` | `ipc/commands/<miền>.rs` |
| `main.rs` | `bootstrap/` (`main.rs` còn ≤ 10 dòng) |
| `crates/klat-hook/src/data_dir.rs` | `klat-protocol::data_dir` |

### 11.3 Quy tắc trong giai đoạn chuyển tiếp (hết hiệu lực)

Hết hiệu lực từ 2026-10-16: mọi tầng đích đã tồn tại, code mới đặt thẳng vào tầng của nó theo §8 và không còn dấu
`// target:`. Giữ lại để đọc lịch sử. Trước đó, khi tầng đích của tính năng **chưa tồn tại**:

1. Viết code mới **như thể** nó đã ở tầng đích: logic thuần trong hàm/struct không chạm `AppHandle`, I/O, đồng hồ; thời gian truyền vào; type thay chuỗi.
2. Đặt nó trong module hiện tại gần nhất theo §11.2, kèm comment `// target: klat-core::<module>` để phase migrate nhặt ra.
3. Không thêm `static` mutable mới, không thêm lệnh gọi thẳng vào `publish_state` từ module mới, không thêm phụ thuộc ngược (module "thuần" import module có `AppHandle`).
4. Nếu chạm vào một file cũ, được phép (không bắt buộc) dời phần bị chạm sang hình dạng đích, **trong commit riêng** tách khỏi commit đổi hành vi.

---

## 12. Checklist trước khi merge

- [ ] Code nằm đúng tầng theo §8; không vi phạm bảng phụ thuộc §4.
- [ ] Logic mới có test ở tầng trong cùng chứa nó.
- [ ] Mọi mutation state đi qua `dispatch(Command)` (sau Phase 4).
- [ ] Không thêm `static` mutable ngoài `platform/`; không thêm `#[cfg(target_os)]` ngoài `platform/`.
- [ ] Không `Result<_, String>` mới; không chuỗi thay enum; thời gian/toạ độ dùng newtype (sau Phase 2).
- [ ] Payload IPC/hook thay đổi: type TS sinh lại; field hook mới là `Option`.
- [ ] Log không chứa prompt, lệnh, tool input, token, đường dẫn đầy đủ.
- [ ] Đổi `klat-hook`: đo lại kích thước + thời gian khởi động.
- [ ] Gate chất lượng xanh (`scripts/check.mjs`):

```bash
pnpm check              # fmt, clippy -D warnings, cargo test, eslint, tsc, vitest, cargo audit; bảng kết quả, exit 1 nếu fail
pnpm check --mac        # thêm type-check cho macOS khi chạm code #[cfg(target_os = "macos")]
pnpm check --skip audit # khi offline (cargo audit cần tải advisory DB); cấu hình bỏ qua ở .cargo/audit.toml
```

- [ ] Hành vi người dùng thấy được: `pnpm regress` xanh (Windows, tắt Klat đã cài trước) + các dòng owner (O1–O8) mà thay đổi chạm tới, theo `docs/verification.md` ("Regression script").
- [ ] Đổi `klat-hook` hoặc hứa hẹn về hiệu năng: `pnpm measure`, so với bảng baseline trong `docs/verification.md`.
- [ ] Spec/doc liên quan đã cập nhật (`KLAT_SPEC.md`, `docs/specs/`, file này nếu đổi cấu trúc).

---

## 13. Sửa file này

- Đổi quy tắc, thêm tầng, cho phép ngoại lệ: sửa file này **trong cùng PR** với code cần nó, ghi lý do ngay tại mục bị sửa (một câu "vì sao" là đủ).
- Quyết định lớn (đổi framework, tách/gộp crate, đổi giao thức hook): thêm một mục ngắn vào bảng dưới đây.
- Hoàn thành một phase migrate: cập nhật §11.1.

| Ngày | Quyết định | Lý do |
|---|---|---|
| 2026-10-16 | Áp dụng kiến trúc phân tầng protocol / core / infra / app | Tách logic khỏi Tauri để test và mở rộng; một nguồn sự thật cho giao thức hook; xoá phụ thuộc vòng và global state |
| 2026-10-16 | Migrate xong (Phase 0–5, epic `ClauCat-dv6`) | So với baseline: `klat-hook` nhỏ hơn 2.2%, vòng sửa-test nhanh 5.3×, test Rust 124 → 176, TS 108 → 121 (docs/verification.md). Phase 4 còn chờ owner kiểm tay trên Windows và Mac |
| 2026-10-16 | Type TS sinh bằng `ts-rs` 12 (chỉ lúc test), không dùng `tauri-specta` | ts-rs đã ổn định (12.0.1) và không đổi cách gọi `invoke`; tauri-specta vẫn là bản RC (2.0.0-rc.25) cho Tauri 2. Đổi khi tauri-specta ổn định và ta muốn sinh luôn wrapper command |
