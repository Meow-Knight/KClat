# Spec: Cửa sổ Settings, Platform/Klat Language, bản macOS

> Nguồn ý định: [`docs/intent/settings-window-and-macos.md`](../intent/settings-window-and-macos.md) (đã chốt 2026-10-13).
> Bổ sung cho [`KLAT_SPEC.md`](../../KLAT_SPEC.md). Chỗ nào mâu thuẫn, file này thắng cho phạm vi của nó
> (§2.3 tray, §7.2 quy tắc 4, §10 settings, §12 dòng "Hỗ trợ macOS").

---

## 0. Capability map

Ba khả năng độc lập kiểm thử được, nên tách module. Id dưới đây cố định, dùng cho beads và commit.

| Module id | Trách nhiệm | Phụ thuộc |
|---|---|---|
| `settings-model` | Schema settings mới (`uiLanguage`, `klatLanguage`), migrate từ `languages`, command sửa settings có validate | — |
| `i18n` | Catalog chữ giao diện en/vi dùng chung Rust + TS; Klat Language trong PhrasePicker | `settings-model` |
| `settings-window` | Cửa sổ Settings (vòng đời, UI, thiết kế) + thu gọn tray | `settings-model`, `i18n` |
| `macos-port` | Đường dẫn, hook adapter, overlay, tray, bundle `.dmg` cho macOS | `settings-model` (đường dẫn data dir) |
| `macos-docs` | Guide build cho dev + guide cài cho người dùng thường | `macos-port` |

Thứ tự build: `settings-model` → `i18n` → `settings-window`. `macos-port` chạy song song sau `settings-model`.
`macos-docs` cuối cùng.

---

## 1. Objective

- **Ai dùng:** người dùng Klat trên Windows (hiện tại) và macOS (mới). Một phần không phải dev.
- **Vấn đề:** mọi tuỳ chỉnh nhét trong tray menu, vài tham số chỉ sửa được bằng tay trong `settings.json`, chữ
  giao diện chỉ có tiếng Anh, ngôn ngữ câu thoại không đổi được, và Klat chỉ chạy trên Windows.
- **Thành công khi:**
  1. Tray còn đúng 5 mục: Ready ☑, Pause notifications ☑, Dismiss all, Settings…, Quit.
  2. Settings… mở một cửa sổ chứa mọi tuỳ chỉnh, đổi là áp dụng ngay và lưu xuống `settings.json`.
  3. Platform Language đổi toàn bộ chữ giao diện (tray, cửa sổ Settings, hộp thoại, tooltip nút Ready) mà không cần
     khởi động lại. Klat Language đổi ngôn ngữ câu thoại; Auto = random cả hai.
  4. Trên Windows mọi hành vi cũ vẫn chạy (gates xanh, checklist `docs/verification.md` đạt).
  5. Có cấu hình bundle và script để build `.dmg` universal (chip Intel + chip M) trên một máy Mac, cùng hai tài liệu.

---

## 2. Module `settings-model`

### 2.1 Schema (`settings.json`)

Thay `languages: string[]` bằng hai field:

```json
{
  "uiLanguage": "en",
  "klatLanguage": "auto"
}
```

| Field | Giá trị hợp lệ | Mặc định | Ghi chú |
|---|---|---|---|
| `uiLanguage` | `"en"`, `"vi"` | `"en"` | Platform Language |
| `klatLanguage` | `"en"`, `"vi"`, `"auto"` | `"auto"` | Klat Language. `auto` = pool gồm cả en và vi |

**Migrate khi load** (chỉ khi file chưa có `klatLanguage`): `languages` = `["en","vi"]` (hoặc bất kỳ chứa cả hai) →
`auto`; `["en"]` → `en`; `["vi"]` → `vi`; thiếu/rỗng/lạ → `auto`. Lần save kế tiếp không ghi `languages` nữa.
Giá trị lạ của hai field mới → default (đúng quy tắc "field nào hỏng dùng default" hiện có).

Giới hạn số (Rust clamp khi load và khi patch; UI dùng cùng giới hạn):

| Field | Min | Max | Đơn vị hiển thị trong UI |
|---|---|---|---|
| `yellIntervalMs` | 5 000 | 600 000 | giây, bước 5 |
| `maxYellsPerWake` | 1 | 50 | lần |
| `staleAfterMs` | 300 000 | 14 400 000 | phút, bước 5 |

> Min hiện tại trong code là 1 000 ms / 60 000 ms. Nâng min chỉ ảnh hưởng file ai đó sửa tay xuống thấp hơn: giá trị
> bị clamp lên, không reset.

### 2.2 Command mới

| Command | Input | Kết quả | Ghi chú |
|---|---|---|---|
| `set_settings` | `patch: object` (một phần của Settings) | `Settings` sau khi áp | Chỉ nhận key trong whitelist: mọi field trừ `position`, `corner`, `startAtLogin`. Ghép patch vào JSON hiện tại rồi chạy lại `Settings::from_object`, nên validate dùng chung với lúc load. Gọi `state::update_settings` (lưu file + emit `cat://settings`). |
| `set_start_at_login` | `on: bool` | `Result<bool, String>` | Chuyển logic `tray::set_autostart` sang đây; trả trạng thái thật sau khi đổi. |
| `reset_position` | — | — | Chuyển từ tray. |
| `wake_test`, `sleep` | — | — | Bọc `state::wake_test`, `state::dismiss_all`. |
| `hooks_status` | — | `{ status, settingsPath, exePath }` | |
| `hooks_apply` | `action: "install" \| "uninstall"` | `{ ok, message }` | Message đã dịch theo `uiLanguage`. Chạy ngoài main thread như hiện tại. |
| `open_settings` | — | — | Mở/đưa lên cửa sổ Settings (tray dùng trực tiếp hàm Rust). |

`cat://settings` vẫn là kênh duy nhất báo settings đổi. Overlay, tray và cửa sổ Settings đều nghe nó, nên đổi ở đâu thì
mọi nơi khớp nhau.

---

## 3. Module `i18n`

### 3.1 Platform Language: catalog dùng chung

- File nguồn: `src/i18n/en.json`, `src/i18n/vi.json`. Object phẳng, key dạng `tray.ready`, `settings.general.sfw.title`.
  Biến chèn dạng `{count}`, `{path}`.
- **TS:** `src/i18n/index.ts` export `t(lang, key, vars?)`. Import JSON tĩnh (Vite gộp vào bundle).
- **Rust:** `src-tauri/src/i18n.rs` dùng `include_str!("../../src/i18n/en.json")` (và vi), parse một lần vào
  `OnceLock<HashMap>`. `i18n::t(lang, key)`; thiếu key trong vi → rơi về en; thiếu cả hai → trả key (và test bắt được).
- Một nguồn duy nhất cho cả hai phía; không có chuỗi giao diện hard-code trong `tray.rs`, `main.rs`, `ReadyButton.ts`.

Chữ phải đi qua catalog: nhãn tray + tooltip ("Klat · 2 sessions waiting"), mọi hộp thoại (`tray::notify`, kết quả hooks,
"Klat is already running"), lỗi `InstallError` hiển thị cho user, tooltip/aria nút Ready trên overlay, toàn bộ cửa sổ
Settings. **Không dịch:** log file, câu thoại (thuộc Klat Language), tên sản phẩm "Klat".

Đổi `uiLanguage`: tray gọi `set_text` cho từng item; overlay và Settings render lại khi nhận `cat://settings`.

### 3.2 Klat Language

- `CatSettings.languages` (TS) đổi thành `klatLanguage: 'en' | 'vi' | 'auto'`.
- `PhrasePicker` nhận `klatLanguage`, quy về danh sách ngôn ngữ: `auto` → `['en','vi']`, còn lại → `[lang]`. Phần còn
  lại của §7.2 giữ nguyên (shuffle bag, SFW, `ignored` từ lần 3).
- Câu `wake` (lần chửi đầu) vẫn bỏ qua bộ lọc ngôn ngữ như hiện tại (D3: luôn là "Wake up, B**CH!!!"). Xem Open
  Question 1.

---

## 4. Module `settings-window`

### 4.1 Vòng đời cửa sổ

| Thuộc tính | Giá trị |
|---|---|
| Label / trang | `settings` / `settings.html` (Vite multi-page: thêm input trong `vite.config.ts`) |
| Kích thước | 780×540 logical, min 680×480, resizable, có viền + thanh tiêu đề của hệ điều hành |
| Tiêu đề | `Klat Settings` / `Cài đặt Klat` theo `uiLanguage` (đổi khi đổi ngôn ngữ) |
| Mở | Tray "Settings…" (Windows: cả double-click icon tray). Đã mở thì `unminimize` + `show` + `set_focus`, không tạo cửa sổ thứ hai |
| Đóng | Huỷ webview (không ẩn) để trả RAM; mục tiêu RAM toàn app 150 MB (ClauCat-gwd) chỉ tính lúc Settings đóng |
| Vị trí | Giữa màn hình chính |
| Capabilities | Thêm `"settings"` vào `windows` của `src-tauri/capabilities/default.json` (hoặc file capability riêng cho settings) |

### 4.2 Bố cục

```text
┌──────────────────────────────────────────────────────────────────┐
│ ┌───────────────┐ ┌────────────────────────────────────────────┐ │
│ │  (mèo) Klat   │ │ General                                    │ │
│ │  v0.1.0       │ │ Cách Klat chạy trên máy bạn                │ │
│ │               │ │ ┌────────────────────────────────────────┐ │ │
│ │ ▌ General     │ │ │ Start at login                    [●○] │ │ │
│ │   Language    │ │ │ Mở Klat khi đăng nhập                  │ │ │
│ │   Hooks   ●   │ │ ├────────────────────────────────────────┤ │ │
│ │   Advanced    │ │ │ Pause notifications               [○●] │ │ │
│ │               │ │ │ …                                      │ │ │
│ │               │ │ └────────────────────────────────────────┘ │ │
│ └───────────────┘ └────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
  sidebar 208px          nội dung: max-width 560px, cuộn dọc
```

- **Sidebar:** header là mèo nhỏ (SVG tĩnh vẽ đầu mèo theo palette §5.2, không dùng Three.js để giữ RAM thấp) +
  chữ "Klat" + version. Dưới là 4 mục điều hướng có icon; mục đang chọn có thanh nhấn màu aura bên trái và nền nhạt.
  Mục Hooks có chấm trạng thái (xanh / vàng / xám).
- **Nội dung:** tiêu đề mục (20px/600) + một dòng mô tả; các nhóm là card bo góc 10px; mỗi dòng trong card cao tối thiểu
  56px: tên (14px/600) + mô tả (12.5px, màu muted) bên trái, control canh phải, phân cách bằng đường kẻ 1px.

| Mục | Nội dung |
|---|---|
| **General** | Card "Startup": Start at login. Card "Cat": Pause notifications (đồng bộ với tray), Hide cat while sleeping, SFW mode, Reduce motion. Card "Position": Move the cat back to the corner (nút). |
| **Language** | Card "Platform Language": segmented English / Tiếng Việt. Card "Klat Language": segmented English / Tiếng Việt / Auto + **ô xem trước**: bong bóng thoại cùng kiểu bubble overlay, hiện một câu bốc bằng `PhrasePicker` theo `klatLanguage` + `sfwMode`, nút "Câu khác" (↻) bốc câu mới. |
| **Hooks** | Card trạng thái: pill Installed / Needs repair / Not installed, đường dẫn `settings.json` (monospace, có thể chọn copy). Nút chính đổi theo trạng thái (Install / Repair; ẩn khi Installed), nút phụ Uninstall. Kết quả hiện **banner inline** trong card (thành công xanh / lỗi đỏ), không bật hộp thoại. |
| **Advanced** | Card "Timing": khoảng cách giữa hai lần gọi (giây), số lần gọi tối đa, thời gian session bị coi là cũ (phút); mỗi dòng là stepper số có đơn vị. Card "Test": Wake test, Sleep. |

### 4.3 Hệ thiết kế

Token màu trên `:root`, đổi theo `prefers-color-scheme` (không có toggle riêng):

| Token | Light | Dark | Dùng cho |
|---|---|---|---|
| `--bg` | `#f4f5f8` | `#17181f` | nền cửa sổ |
| `--sidebar` | `#eceef3` | `#1d1f28` | sidebar |
| `--card` | `#ffffff` | `#23252f` | card |
| `--line` | `#e2e5ec` | `#2f3240` | đường kẻ, viền card |
| `--ink` | `#2b2d42` | `#eceef5` | chữ chính (INK của mèo) |
| `--muted` | `#6b7690` | `#9aa6bd` | mô tả (FUR2 / FUR) |
| `--accent` | `#f5b83d` | `#f5b83d` | màu aura: switch bật, thanh mục đang chọn, focus ring |
| `--on-accent` | `#2b2d42` | `#2b2d42` | chữ/icon trên nền accent (không dùng trắng: tương phản kém) |
| `--ok` / `--warn` / `--danger` | `#2f9e6a` / `#c98a12` / `#d64545` | `#4cc38a` / `#f5b83d` / `#ff6b6b` | pill trạng thái, banner |

- Font: `"Segoe UI Variable Text", "Segoe UI", -apple-system, BlinkMacSystemFont, system-ui, sans-serif`. Chữ tiếng
  Việt có dấu phải hiển thị đúng trên cả hai OS.
- Control tự vẽ bằng HTML/CSS, không thêm thư viện UI: switch (`role="switch"`), segmented control (radio group),
  stepper (`<input type="number">` + nút −/+), button primary (accent) / secondary (viền) / danger (chữ đỏ).
- Chuyển động: chuyển mục 120ms fade; switch 150ms. Tắt hết khi `reduceMotion` hoặc `prefers-reduced-motion`.
- Bàn phím: Tab đi hết control, ↑/↓ trong sidebar, focus ring 2px accent. Mọi control có label đọc được bằng screen reader.
- Không tài nguyên ngoài (CSP `'self'`): không CDN, không Google Fonts.
- Vanilla TS + CSS như phần còn lại của dự án; tách logic thuần (đổi đơn vị, clamp, chọn nút Hooks theo trạng thái) ra
  module riêng để test bằng vitest.

**Cổng duyệt thiết kế:** trước khi nối Rust, dựng `settings.html` chạy được trong `pnpm dev` với bridge giả
(`?mock` trong URL), chụp 4 ảnh (light/dark × en/vi) và để chủ dự án duyệt. Chỉ nối vào app sau khi được duyệt.

### 4.4 Tray sau khi thu gọn

```text
☐ Ready: wake me when Claude is done
☐ Pause notifications
  Dismiss all
──────────────
  Settings…
──────────────
  Quit
```

Wake test, Sleep, Move the cat back to the corner, SFW, Hide, Start at login, Hooks ▸ rời tray. Tick của Ready và Pause
cập nhật khi đổi từ nơi khác (overlay, cửa sổ Settings). Tooltip giữ số session đang chờ, theo `uiLanguage`.

---

## 5. Module `macos-port`

Code Windows-only hiện có và cách xử lý trên macOS:

| Chỗ | Hiện tại | macOS |
|---|---|---|
| Data dir (`settings.rs`, `state.rs`, `event_server.rs`, `crates/klat-hook`) | `%APPDATA%\Klat` | `~/Library/Application Support/Klat`. Một hàm `data_dir()` theo `cfg(target_os)`. `klat-hook` là crate riêng nên có bản sao của hàm, kèm test khẳng định hai bên ra cùng đường dẫn. `migrate_legacy_dir` chỉ chạy trên Windows. |
| Tên hook adapter (`hooks_installer.rs`) | `MARKER = "klat-hook.exe"` | Tên file = `"klat-hook" + std::env::consts::EXE_SUFFIX`. `is_ours` khớp theo `klat-hook` (không phân biệt hoa thường), nên entry cũ có `.exe` vẫn được nhận. |
| Vị trí adapter khi bundle | cạnh `klat.exe` | `Klat.app/Contents/MacOS/klat-hook` (Tauri đặt sidecar cạnh binary chính; `current_exe().with_file_name` vẫn đúng) |
| Overlay trong suốt | WebView2 | Bật `app.macOSPrivateApi: true` trong `tauri.conf.json` + feature `macos-private-api` của crate `tauri`. Thêm `visible_on_all_workspaces(true)` để mèo theo qua mọi Space. |
| Biểu tượng Dock | không có | `set_activation_policy(Accessory)` trong `setup`: chỉ có icon trên menu bar. Mở Settings vẫn phải lấy được focus (xem Rủi ro). |
| Tray icon | icon màu | Thêm `icons/tray-template.png` (đơn sắc, 22pt @1x/@2x), `icon_as_template(true)` trên macOS để tự đổi theo menu bar sáng/tối. |
| Kéo mèo bằng Space (`drag.rs`) | low-level keyboard hook Win32 | **v1: không có** (stub `cfg(not(windows))` đã có sẵn). Settings > General ẩn dòng hướng dẫn kéo; nút "Move the cat back to the corner" vẫn chạy. Xem Open Question 2. |
| Start at login | HKCU Run | `tauri-plugin-autostart` dùng LaunchAgent sẵn; không cần code riêng. |
| Font CSS (`dev.html`, `stage.css`) | `'Segoe UI'` | Thêm `-apple-system, BlinkMacSystemFont` vào stack |
| Vite `build.target` | `chrome105` | `['chrome105', 'safari15']` (WKWebView) |
| Bundle | `targets: ["nsis"]` | `src-tauri/tauri.macos.conf.json`: `targets: ["app", "dmg"]`, `macOS.minimumSystemVersion: "11.0"`, `macOS.signingIdentity: "-"` (ký ad-hoc, bắt buộc để chạy trên chip M dù không có Developer ID). |
| Sidecar universal (`scripts/stage-sidecar.mjs`) | build theo host | Nhận `--target universal-apple-darwin`: build `klat-hook` cho `aarch64-apple-darwin` + `x86_64-apple-darwin`, ghép bằng `lipo -create`, đặt tên `klat-hook-universal-apple-darwin`. |
| Script | `pnpm release` | Thêm `pnpm release:mac` = build hook universal + `tauri build --target universal-apple-darwin --config src-tauri/tauri.release.conf.json` |
| Chữ hiển thị đường dẫn | `%USERPROFILE%\.claude\settings.json` | Hiện đường dẫn thật (`~/.claude/settings.json` trên mac), qua catalog i18n |

Không đổi gì trên Windows ngoài những chỗ bảng trên ghi rõ.

## 6. Module `macos-docs`

| File | Đối tượng | Nội dung |
|---|---|---|
| `docs/macos-build.md` | Dev | Yêu cầu (macOS 11+, Xcode Command Line Tools, Rust + 2 target apple, Node 20 + pnpm 8); clone → `pnpm install` → `pnpm tauri dev`; build `.dmg` universal bằng `pnpm release:mac`; file ra ở đâu; mục **"Nếu lỗi X thì làm Y"** cho các lỗi dự đoán được (thiếu target, `lipo` không thấy, overlay không trong suốt, hook không gọi được). |
| `docs/macos-install.md` | Người dùng thường | Tải `.dmg` → kéo Klat vào Applications → lần mở đầu bị chặn → System Settings → Privacy & Security → **Open Anyway** → nếu báo "damaged" thì chạy `xattr -cr /Applications/Klat.app` (giải thích bằng lời thường, có ô copy lệnh) → mở Settings > Hooks > Install. Có chỗ cho ảnh chụp `docs/images/macos/*.png`. |

Ảnh chụp màn hình macOS **không** tạo được trên máy Windows: guide ghi rõ chỗ đặt ảnh và danh sách ảnh cần chụp ở lần
build đầu trên Mac. Không dùng ảnh giả.

README: thêm mục macOS trỏ tới hai file trên.

---

## 7. Tech stack

Không thêm dependency mới cho phạm vi đã chốt. Đang dùng: Tauri 2.12 (`tray-icon`, thêm `macos-private-api`),
`tauri-plugin-dialog` 2.8, `tauri-plugin-autostart` 2.7, `tauri-plugin-single-instance` 2.5, serde/serde_json,
TypeScript 6, Vite 8, Vitest 4, three 0.186 (chỉ overlay).

## 8. Commands

```bash
pnpm install
pnpm tauri dev                 # chạy app (Windows)
pnpm dev                       # chỉ frontend; mở http://localhost:1420/settings.html?mock để duyệt UI
pnpm lint && pnpm typecheck && pnpm test
pnpm lint:rust && pnpm test:rust
pnpm release                   # bộ cài NSIS (Windows)
pnpm release:mac               # .dmg universal (chỉ chạy trên macOS)
rustup target add aarch64-apple-darwin && cargo check -p klat --target aarch64-apple-darwin   # kiểm tra chéo từ Windows, best effort
```

## 9. Project structure (phần thêm/đổi)

```text
settings.html                      trang của cửa sổ Settings
src/i18n/{en,vi}.json, index.ts    catalog Platform Language + t()
src/settings-window/               main.ts, sections/*.ts, controls/*.ts, settings.css, logic thuần (viewModel.ts)
src-tauri/src/i18n.rs              đọc catalog chung
src-tauri/src/settings_window.rs   tạo/focus/huỷ cửa sổ
src-tauri/src/platform.rs          data_dir() theo OS
src-tauri/tauri.macos.conf.json    bundle macOS
src-tauri/icons/tray-template.png  icon menu bar macOS
docs/macos-build.md, docs/macos-install.md, docs/images/macos/
tests/i18n.test.ts, tests/settingsViewModel.test.ts
```

## 10. Code style

Theo code hiện có: doc comment `//!`/`///` nói *vì sao*, hàm nhỏ, không `unwrap` trên dữ liệu từ ngoài, logic thuần tách
khỏi Tauri để test được. Ví dụ hướng đi cho `set_settings`:

```rust
/// Settings window edits: merge the patch into the current settings and re-run the load-time validation,
/// so a value the UI should never send is clamped or dropped the same way a hand-edited file would be.
pub fn apply_patch(current: &Settings, patch: &Map<String, Value>) -> Settings {
    let mut obj = to_object(current);
    for (k, v) in patch.iter().filter(|(k, _)| PATCHABLE.contains(&k.as_str())) {
        obj.insert(k.clone(), v.clone());
    }
    Settings::from_object(&obj)
}
```

TS: module nhỏ, type rõ, không framework; DOM dựng bằng hàm trả `HTMLElement`; chuỗi giao diện chỉ qua `t()`.

## 11. Testing strategy

| Mức | Cái gì |
|---|---|
| Rust unit | migrate `languages` → `klatLanguage` (mọi nhánh); giá trị lạ → default; `apply_patch` bỏ key ngoài whitelist và clamp; catalog en/vi cùng tập key, không chuỗi rỗng, biến `{x}` khớp nhau; `t()` rơi về en; `data_dir()` theo OS; `is_ours` khớp `klat-hook` và `klat-hook.exe`; `klat-hook` và app ra cùng đường dẫn runtime. |
| Vitest | `t()` + parity catalog; PhrasePicker với `klatLanguage` en / vi / auto (vi không ra câu en ngoài câu wake); viewModel: đổi ms ↔ giây/phút, clamp, nút Hooks theo trạng thái. |
| Thủ công (Windows) | Thêm mục vào `docs/verification.md`: mở/đóng Settings nhiều lần không sinh cửa sổ thứ hai; đổi từng setting thấy hiệu lực ngay; đổi Platform Language thấy tray + Settings + tooltip Ready đổi; RAM khi Settings đóng. Không tự động click tray/taskbar (chủ dự án click). |
| Duyệt thiết kế | 4 ảnh light/dark × en/vi ở cổng §4.3. |
| macOS | `cargo check` chéo (best effort, ghi lại kết quả). Chạy thật do chủ dự án làm theo `docs/macos-build.md`. |

Gates trước mỗi commit: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm lint:rust`, `pnpm test:rust`.

## 12. Boundaries

- **Always:** chạy đủ gates trước commit; giữ `settings.json` cũ đọc được (migrate, không reset); mọi chuỗi giao diện
  qua catalog; commit **không** có dòng `Co-Authored-By` hay "Generated with Claude Code".
- **Ask first:** thêm dependency (kể cả crate macOS như `core-graphics`); đổi vị trí `settings.json` trên Windows; bỏ
  hay thêm mục tray ngoài danh sách §4.4; đổi câu thoại hay quy tắc chọn câu ngoài §3.2; commit dính vào thay đổi đang
  dở của `ClauCat-mvx`.
- **Never:** tự động click vào tray/taskbar; tải tài nguyên ngoài trong cửa sổ Settings; ký bằng danh tính Apple của
  bất kỳ ai; tạo ảnh chụp macOS giả.

## 13. Success criteria

1. Tray đúng như §4.4, tick Ready/Pause luôn khớp trạng thái thật.
2. Cửa sổ Settings có đủ nội dung §4.2; mỗi control đổi được, ghi xuống `settings.json`, overlay phản ứng ngay; mở lại
   app thấy giá trị cũ.
3. File settings cũ có `"languages": ["vi"]` mở lên thành Klat Language = Tiếng Việt.
4. Đổi Platform Language sang Tiếng Việt: không còn chữ tiếng Anh nào trên tray, Settings, hộp thoại, tooltip Ready
   (trừ "Klat" và đường dẫn).
5. Klat Language = Tiếng Việt: sau câu wake, mọi câu thoại là tiếng Việt; Auto: có cả hai.
6. Bộ 4 ảnh thiết kế được chủ dự án duyệt.
7. Gates xanh; checklist Windows trong `docs/verification.md` đạt.
8. `tauri.macos.conf.json`, `pnpm release:mac`, `docs/macos-build.md`, `docs/macos-install.md` có mặt; `cargo check`
   chéo cho `aarch64-apple-darwin` đã chạy và kết quả được ghi lại (kể cả nếu fail vì toolchain).

## 13.1 Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Không test được trên Mac | Tách mọi khác biệt OS vào `platform.rs` + cấu hình; guide build có mục xử lý lỗi; issue riêng "chạy thử trên Mac thật" giao cho chủ dự án |
| App kiểu Accessory trên macOS mở cửa sổ Settings mà không lấy được focus | Khi mở Settings gọi `set_focus`; nếu không đủ thì tạm chuyển `ActivationPolicy::Regular` khi Settings mở, trả lại khi đóng. Ghi vào guide để kiểm khi chạy thật |
| Webview thứ hai đội RAM | Tạo khi mở, huỷ khi đóng; không dùng Three.js trong Settings |
| Thay đổi đang dở của `ClauCat-mvx` đụng cùng file (`tray.rs`, `settings.rs`, `main.ts`) | Chốt/commit mvx trước khi bắt đầu module nào sửa các file đó |

## 14. Open questions

1. **Câu wake khi Klat Language = Tiếng Việt.** Hiện câu đầu luôn là "Wake up, B**CH!!!" (tiếng Anh, D3). Đề xuất: giữ
   nguyên vì là câu đặc trưng. Nếu muốn có câu wake tiếng Việt thì cần bạn đưa câu.
2. **Kéo mèo bằng Space trên macOS.** Đề xuất: v1 không có (tránh thêm crate và quyền Accessibility khi chưa test được);
   tạo issue P3 riêng làm bằng `CGEventTap` sau khi bản Mac đầu tiên chạy.
3. **"Move the cat back to the corner"** chuyển từ tray vào Settings > General. Đồng ý?
4. **Pause notifications** có ở cả tray lẫn Settings > General (đồng bộ hai chiều). Đồng ý?
