# Klat — Spec triển khai

> **Mục tiêu:** Một desktop companion cho Claude Code trên Windows. Một chú mèo 3D toon cau có nằm ngủ ở góc màn hình. Khi Claude xong lượt hoặc đang chờ bạn, mèo bật dậy và chửi bạn ("Wake up, B**CH!!!") cho tới khi bạn quay lại terminal.
>
> **Đối tượng đọc:** Coding Agent (Claude Code). Đọc hết file trước khi viết code. File này thay thế `CLAUDE_CAT_IMPLEMENTATION_PLAN.md` cũ. Phần nào hai file mâu thuẫn thì theo file này.
>
> **Prototype tham chiếu:** [`docs/prototype/cat-prototype.html`](docs/prototype/cat-prototype.html). Đây là bản Three.js chạy được, cần mạng để tải three.js từ CDN. **Phải mở qua HTTP server**, không mở trực tiếp bằng `file://` vì ES module import sẽ bị chặn và trang sẽ trắng: `cd docs/prototype && python -m http.server 8766`, rồi vào `http://127.0.0.1:8766/cat-prototype.html`. Hình dáng, màu sắc, tham số lò xo và timeline animation trong prototype là **nguồn chuẩn**: khi port sang TypeScript phải giữ đúng cảm giác của nó.

---

## 1. Quyết định đã chốt

| # | Quyết định | Ghi chú |
|---|---|---|
| D1 | **Chỉ 2 trạng thái: `SLEEP` và `ACTIVE`** | ACTIVE khi Claude xong lượt hoặc đang chờ input/permission. Ngoài ra là SLEEP. |
| D2 | **Mèo ngủ luôn hiện trên màn hình** | Đây là thay đổi so với nguyên tắc "hidden-by-default" của plan cũ. Có setting `hideWhileSleeping` (mặc định `false`) cho ai muốn ẩn mèo khi ngủ. |
| D3 | **Khi chuyển SLEEP → ACTIVE, câu đầu tiên luôn là `Wake up, B**CH!!!`** | Nếu bị lờ đi, mèo chửi tiếp bằng câu ngẫu nhiên lấy từ kho câu (§7). |
| D4 | **Chỉ dùng công nghệ miễn phí / mã nguồn mở** | Không dùng Rive (export phải trả phí), Spine (trả phí), Live2D (license có điều kiện). |
| D5 | **Render mèo bằng Three.js, 3D toon dựng bằng code** | Ghép từ các khối cơ bản (sphere, cone, torus, tube) + toon shading + viền bằng inverted hull. Không cần asset hay hoạ sĩ. |
| D6 | **Mọi chuyển động chạy bằng spring physics, không dùng keyframe** | Đổi trạng thái giữa chừng không bị giật. Có thêm quỹ đạo nhảy có trọng lực và squash & stretch. |
| D7 | **Mèo quay đầu theo con trỏ trên toàn màn hình** | Rust đọc vị trí con trỏ và đẩy sang frontend (§9). |
| D8 | **Local-first** | Không telemetry, không cloud. Hook chỉ gửi event qua loopback và không bao giờ tự approve/deny permission. |

---

## 2. Trải nghiệm người dùng

### 2.1 Vòng đời

```text
App khởi động ──► SLEEP (mèo cuộn tròn ngủ ở góc phải dưới, Zzz bay lên)
                    │
   Stop / PermissionRequest / Notification(cần input)
                    ▼
                  WAKE sequence (~1.5s) ──► ACTIVE
                    │                         │ bị lờ → cứ yellIntervalMs lại chửi câu khác
                    │                         │ (tối đa maxYellsPerWake lần, sau đó ngồi cau có im lặng)
   UserPromptSubmit / tool chạy tiếp / PermissionDenied / user dismiss
                    ▼
                  SLEEP sequence (ngáp ~1s rồi nằm ngủ) ──► SLEEP
```

### 2.2 Biểu hiện từng trạng thái

**SLEEP**
- Thân hạ thấp, đầu cúi và nghiêng, tai cụp, mắt nhắm (vòng cung ∪), đuôi quấn ra trước chân.
- Thở chậm (chu kỳ khoảng 4,5 giây), chóp đuôi đung đưa nhẹ, thỉnh thoảng giật tai.
- Chữ "z Z z" bay lên.
- Không nhìn theo chuột.
- Click vào mèo: mèo hé mắt khoảng 0,9 giây, giật tai rồi ngủ lại.

**WAKE (chuyển tiếp)**
- Mắt mở trừng, tai dựng, thụp người lấy đà, bật nhảy, lông xù, đuôi dựng.
- Bong bóng thoại bật ra kèm hiệu ứng rung chữ: **"Wake up, B**CH!!!"**, miệng há.

**ACTIVE**
- Ngồi thẳng, mắt lim dim cau có, lông mày chụm, gõ chân phải sốt ruột, đuôi vẫy nhanh.
- Đầu và đồng tử nhìn theo con trỏ.
- Chớp mắt ngẫu nhiên.
- Cứ `yellIntervalMs` lại nhảy nhẹ và chửi câu khác.
- Click vào mèo: vuốt ve. Mèo nhắm mắt kiểu ^ ^, nghiêng đầu, rung nhẹ như đang kêu rừ rừ.
- Click vào bong bóng: dismiss, mèo đi ngủ.

**SLEEP sequence (từ ACTIVE)**
- Bong bóng ẩn, mèo ngáp (ngửa đầu, há miệng, mắt khép) khoảng 1 giây, rồi hạ người nằm ngủ.

### 2.3 Dismiss và điều khiển

- Click bong bóng (hoặc chọn **Dismiss** trên tray): đánh dấu pending của các session đang hiển thị là đã dismiss, mèo đi ngủ. **Không** gửi gì tới Claude Code.
- Menu tray: `Wake test`, `Sleep`, `Dismiss all`, `SFW mode` ☑, `Pause notifications` ☑, `Hide cat` ☑, `Hooks ▸ Install / Repair / Uninstall`, `Quit`.
- Tooltip của tray hiển thị số session đang chờ, ví dụ `2 sessions waiting`.

---

## 3. Tech stack

Tất cả đều miễn phí. Khi triển khai, pin đúng phiên bản stable hiện hành và commit lockfile.

| Lớp | Công nghệ | Ghi chú |
|---|---|---|
| Desktop shell | **Tauri 2** (Rust) | Tray, cửa sổ overlay, HTTP server loopback, cursor polling, hook installer |
| Overlay frontend | **Vite + TypeScript (vanilla) + Three.js** | Không dùng React cho overlay, để giữ nhẹ |
| Settings UI | Tray menu + file JSON (MVP) | Cửa sổ settings riêng để sau MVP |
| Hook adapter | **Rust sidecar** `klat-hook.exe` | Đọc stdin JSON, POST lên loopback, luôn exit 0, nhanh |
| Event transport | HTTP chỉ trên `127.0.0.1`, cổng ngẫu nhiên, bearer token random 256-bit | Ghi runtime file `{port, token}` vào app data |
| Test | Vitest (TS), `cargo test` (Rust) | |
| Package manager | pnpm | |

**Rủi ro kỹ thuật số 1:** WebGL (`alpha: true`) trong cửa sổ Tauri trong suốt trên Windows/WebView2. Phải spike ngay ở Phase 0 (§11) trước khi đầu tư thêm.

---

## 4. Kiến trúc

```text
Claude Code ──hooks(JSON stdin)──► klat-hook.exe ──POST 127.0.0.1──► Rust event_server
                                                                             │ validate + normalize
                                                                             ▼
                                                                  session_store (Rust, in-memory)
                                                                             │ derive: awake? sessions[]
                                                          emit "cat://state" ▼
Rust cursor poller ──emit "cat://cursor" (30Hz khi ACTIVE)──► Overlay webview (TS)
                                                                 ├─ CatController (state → targets, sequences)
                                                                 ├─ SpringSet (physics)
                                                                 ├─ CatRig (Three.js meshes)
                                                                 ├─ Bubble (HTML overlay)
                                                                 └─ RenderLoop (FPS throttle)
```

Nguyên tắc ranh giới:
- **Session store** là logic thuần, đặt ở Rust, unit test được và không phụ thuộc Tauri.
- **CatController** là logic thuần ở TS. Nó nhận `setAwake(bool)`, `setLook(dx,dy)`, `pet()` và chỉ điều khiển target của spring. Test bằng fake timers.
- **CatRig** chỉ dựng mesh và áp tham số spring lên transform mỗi frame. Không chứa logic trạng thái.

### 4.1 Cấu trúc thư mục đề xuất

```text
ClauCat/
  KLAT_SPEC.md
  README.md
  package.json / pnpm-lock.yaml / vite.config.ts / tsconfig.json
  docs/
    prototype/cat-prototype.html      # nguồn chuẩn về hình + animation
    hooks-troubleshooting.md
  src/
    overlay/
      main.ts                         # entry overlay window: wiring Tauri events ↔ controller
      dev.ts                          # dev page (chạy trên browser, có nút test, không cần Tauri)
    cat/
      palette.ts                      # màu
      materials.ts                    # toon gradient map, outline material
      CatRig.ts                       # dựng mesh, applyPose(params, time)
      tail.ts                         # tail tube preallocated, update in-place
      springs.ts                      # Spring, SpringSet, Ballistic (jump)
      CatController.ts                # state machine animation + sequences + schedulers
      lookTarget.ts                   # cursor px → lx/ly
      RenderLoop.ts                   # rAF + FPS throttle + pause khi hidden
    bubble/
      Bubble.ts                       # DOM bubble, pop + shake, click = dismiss
    phrases/
      phrases.ts                      # dữ liệu câu thoại (§7)
      PhrasePicker.ts                 # shuffle bag + lọc tag/lang/SFW
    bridge/
      tauriBridge.ts                  # listen cat://state, cat://cursor; invoke dismiss
  src-tauri/
    tauri.conf.json
    src/
      main.rs
      tray.rs
      overlay.rs                      # tạo/đặt vị trí window, click-through toggle
      cursor.rs                       # poll vị trí con trỏ
      event_server.rs                 # loopback HTTP + token
      session_store.rs                # logic thuần + tests
      settings.rs                     # đọc/ghi settings JSON
      hooks_installer.rs              # install/repair/uninstall hooks
  crates/
    klat-hook/src/main.rs
  tests/                              # vitest
```

---

## 5. Thiết kế mèo (rig)

Nguồn chuẩn là hàm dựng mesh trong `docs/prototype/cat-prototype.html`. Port nguyên các toạ độ, scale và màu sang `CatRig.ts`.

### 5.1 Phong cách

- **Mèo xám xanh mập, mặt cau có**, kiểu British Shorthair chibi. Đầu to, má phồng, mắt hổ phách to, viền mực đậm như sticker.
- Toon shading 3 bậc (`MeshToonMaterial` + `DataTexture` 3×1, giá trị `[95,180,255]`, filter `Nearest`).
- Viền bằng inverted hull: mesh clone với `MeshBasicMaterial({color: INK, side: BackSide})`, scale 1,06 (các chi tiết nhỏ dùng 1,08–1,25).
- Thêm bóng đổ là một đĩa tròn đen, opacity 0,13; co lại khi mèo nhảy lên.

### 5.2 Bảng màu

| Token | Hex | Dùng cho |
|---|---|---|
| `FUR` | `#9AA6BD` | Lông chính, đuôi |
| `FUR2` | `#6B7690` | Vằn trán, chóp đuôi |
| `CREAM` | `#F6F0E6` | Bụng, má/mõm, bàn chân |
| `PINK` | `#F29BB0` | Mũi, trong tai |
| `BLUSH` | `#F7A9BC` | Má hồng (không viền) |
| `AMBER` | `#F5B83D` | Mống mắt |
| `PUPIL` | `#1B1B2F` | Đồng tử |
| `INK` | `#2B2D42` | Viền, lông mày, ria, mắt nhắm, miệng "ω" |
| `MOUTH` | `#5A2A3A` | Miệng há |

### 5.3 Cây phân cấp

```text
scene
├─ shadow (disc)
└─ root                    (x=1.3, rotY=-0.22; jumpY; squash scale; gốc ở chân)
   ├─ bodyG                (puff scale)
   │  ├─ body              ellipsoid (1.15, .95, 1) @ y .95   — thở: scale.y
   │  ├─ belly             CREAM, không viền
   │  └─ haunches ×2       đùi sau
   ├─ pawL, pawR           pawR gõ khi ACTIVE
   ├─ tail (tube) + tailOutline + tip sphere (FUR2)
   └─ headG                (y 2.05; rot = look + sleepy + yawn + happy + purr)
      ├─ head              ellipsoid (1.3, 1, 1.05)
      ├─ stripes ×3, cheeks ×2 (CREAM), blush ×2, nose
      ├─ mouthW (2 torus arc "ω"), mouthO (ellipsoid, scale.y theo mouth)
      ├─ whiskers ×4
      ├─ eye ×2: eg → ig (mống + pupilG[đồng tử, 2 highlight]) + sleepArc(∪) + happyArc(∩) + brow(capsule)
      └─ ear ×2: erg (pivot gốc tai) → cone FUR (scale z .55) + cone PINK
```

---

## 6. Hệ animation

### 6.1 Spring

Mỗi tham số là một spring `{v, vel, t, k, d}`, tích phân semi-implicit Euler, `dt` kẹp tối đa 1/30 s:

```ts
vel += (k * (t - v) - d * vel) * dt;
v   += vel * dt;
```

Tạo hiệu ứng nảy bằng cách cộng thẳng vào `vel` (impulse), không set `v` trực tiếp.

| Param | Init | k | d | Ý nghĩa |
|---|---|---|---|---|
| `sleepy` | 1 | 30 | 9 | 0 = thức, 1 = tư thế ngủ (hạ đầu/thân, đuôi quấn, không nhìn chuột) |
| `eye` | .02 | 420 | 26 | Độ mở mắt. Nhỏ hơn .18 thì hiện mắt nhắm ∪; lớn hơn 1 thì mắt trừng (mống to ra) |
| `ear` | .3 | 200 | 12 | 1 = tai dựng, 0 = cụp |
| `earTw` | 0 | 220 | 10 | Giật tai (chỉ nhận impulse) |
| `mouth` | 0 | 260 | 20 | Miệng há |
| `puff` | 0 | 180 | 9 | Xù lông (thân, tai, đuôi to ra) |
| `squash` | 0 | 260 | 12 | Dương = bẹp, âm = vươn dài |
| `lx`, `ly` | 0 | 70 | 11 | Góc quay đầu theo con trỏ |
| `yawn` | 0 | 60 | 10 | Ngửa đầu khi ngáp |
| `happy` | 0 | 140 | 14 | Mắt ^ ^ khi được vuốt |
| `grumpy` | .3 | 120 | 14 | Lông mày chụm |
| `tailUp` | 0 | 60 | 8 | Đuôi dựng, vẫy nhanh |

**Jump** dùng quỹ đạo vật lý riêng: `gravity = 18`. Khi chạm đất với `v < -1.2` thì `squash.vel += -v * 1.3`.

### 6.2 Timeline các sequence (ms tính từ lúc bắt đầu)

**wake()** (chỉ chạy khi đang SLEEP)
| t | Hành động |
|---|---|
| 0 | `sleepy→0`, `yawn→0`, `ear→1` (+vel 9), `eye→1.25` (+vel 14), `squash→.22` (lấy đà) |
| 150 | `squash→0` (vel −4), `jumpV = 4.4`, `puff→1` (+vel 7) |
| 240 | `yell(first=true)` |
| 750 | `puff→0` |
| 1500 | bật gõ chân |
| mỗi `yellIntervalMs` | `yell(false)` (tối đa `maxYellsPerWake`) |

**yell(first)**
| t | Hành động |
|---|---|
| 0 | Chọn câu (first → câu wake), hiện bubble và rung chữ; `mouth→1`, `grumpy→0`, `eye→1.25` (+6), `ear` +6, `tailUp→1`. Nếu không phải lần đầu: `jumpV = 2.6`, `squash` +2 |
| 1100 | `mouth→0`, `grumpy→1`, `eyeBase = .62` (lim dim), `eye→eyeBase`, `tailUp→.4` |

**sleep()** (chỉ chạy khi đang ACTIVE)
| t | Hành động |
|---|---|
| 0 | Huỷ timer, ẩn bubble, tắt gõ chân; `mouth→.85`, `yawn→1`, `eye→.1`, `tailUp→0` |
| 1000 | `mouth→0`, `yawn→0`, `sleepy→1`, `ear→.3`, `eye→.02`, `grumpy→.3` |

**Scheduler nền**
- Blink: chỉ khi ACTIVE, không đang happy hay há miệng. Mỗi 1,6–5 giây: `eye→.03` trong 110ms rồi về `eyeBase`.
- Ear twitch: mỗi 2–6,5 giây, `earTw.vel += ±7` (chạy ở cả hai trạng thái).
- Pet: khi ACTIVE, `happy→1` trong 1300ms và đầu rung `sin(70t)·0.014` quanh trục z. Khi SLEEP: `eye→.4` trong 900ms và `earTw` +9.

### 6.3 Áp tham số lên mesh mỗi frame

Port nguyên khối `frame()` trong prototype. Các điểm chính:
- `headG.rotation = (ly·(1−sleepy) + sleepy·.3 − yawn·.38, lx·(1−sleepy), sleepy·.18 + happy·.2 + purr)`.
- `root.scale = (1+sq·.55, 1−sq, 1+sq·.55)`. Gốc của root ở chân nên squash không làm mèo lún xuống đất.
- Thở: `body.scale.y = .95·(1−sleepy·.1) + sin(t·ω)·.014`, với ω = 1,4 khi ngủ và 2,4 khi thức. Đầu dao động lệch pha −0,6.
- **Đuôi:** CatmullRom qua 4 điểm, nội suy giữa pose ACTIVE `A` và pose SLEEP `Z` theo `sleepy`, cộng thêm dao động `sw`. **Khác prototype:** không tạo lại `TubeGeometry` mỗi frame. Cấp phát sẵn một lần, mỗi frame tính lại vị trí đỉnh trực tiếp vào buffer (`attributes.position.needsUpdate = true`) để không tạo rác cho GC.

### 6.4 Nhìn theo con trỏ

```ts
// dx, dy: px từ vị trí đầu mèo trên màn hình tới con trỏ (logical px)
lx.t = clamp(dx / 260, -1, 1) * 0.75;
ly.t = clamp(dy / 200, -1, 1) * 0.45;
// đồng tử lệch thêm: pupil.x = lx * .08, pupil.y = -ly * .06 (đều nhân (1 - sleepy))
```

Khi không có dữ liệu con trỏ (con trỏ ở màn hình khác, hoặc vừa vào ACTIVE), mèo nhìn về phía bubble: `lx=-.35, ly=-.08`. Vị trí đầu mèo trên màn hình lấy bằng `headG.getWorldPosition().project(camera)`, rồi cộng thêm vị trí cửa sổ overlay.

### 6.5 Giảm chuyển động (`reduceMotion`)

- Tắt nhảy, squash, puff, rung chữ, rung khi rừ rừ và Zzz bay.
- Mọi spring chuyển sang critically damped (`d = 2·√k`).
- Vẫn giữ việc đổi biểu cảm (mắt, tai, tư thế) và bubble.

---

## 7. Câu thoại

### 7.1 Data model

```ts
type PhraseTag = 'wake' | 'done' | 'permission' | 'input' | 'ignored' | 'general';
interface Phrase { id: string; text: string; lang: 'en' | 'vi'; tags: PhraseTag[]; explicit: boolean; }
```

### 7.2 Quy tắc chọn câu

1. Lần chửi đầu tiên của mỗi lần wake: câu có tag `wake`. Với SFW mode tắt, đó là `Wake up, B**CH!!!`; với SFW mode bật, đó là `Wake up, human!!!`.
2. Những lần sau: pool = câu `general` + câu khớp lý do đánh thức (`done` / `permission` / `input`). Từ lần chửi thứ 3 trở đi, thêm các câu `ignored`.
3. Bốc theo **shuffle bag**: xáo pool, rút lần lượt, hết thì xáo lại. Không được lặp câu vừa nói khi chuyển sang bag mới.
4. Lọc theo `languages` (mặc định `['en','vi']`) và bỏ câu `explicit` khi bật `sfwMode`.
5. Bubble có thể hiện thêm tên project ở dòng phụ nếu có hơn 1 session đang chờ.

### 7.3 Dữ liệu

| id | text | lang | tags | explicit |
|---|---|---|---|---|
| wake-1 | Wake up, B**CH!!! | en | wake | ✔ |
| wake-sfw | Wake up, human!!! | en | wake | |
| en-01 | Claude's done. Your turn, lazy a**! | en | done | ✔ |
| en-02 | HEY! The robot is waiting on YOU! | en | general | |
| en-03 | Get your butt back here, human! | en | general | |
| en-04 | I was dreaming about tuna, you sh*t! | en | general | ✔ |
| en-05 | Stop scrolling. Read the d*mn output! | en | done | ✔ |
| en-06 | Tick tock, meatbag! | en | ignored | |
| en-07 | Approve it or your coffee hits the floor. | en | permission | |
| en-08 | Human.exe has stopped responding. | en | ignored | |
| en-09 | Earth to human! Claude's waiting! | en | general | |
| en-10 | The AI did the work. Go take the credit. | en | done | |
| en-11 | Y or N. It's not rocket science, d*mmit! | en | permission | ✔ |
| en-12 | Did you die? Blink twice. | en | ignored | |
| en-13 | I'm a cat, not your f*cking alarm clock. | en | general | ✔ |
| en-14 | Back to work, you overpaid button-pusher! | en | general | |
| en-15 | Read. Approve. Let me sleep. In that order. | en | permission | |
| en-16 | Last warning before I sit on your keyboard. | en | ignored | |
| en-17 | Holy sh*t, it's done. Go look! | en | done | ✔ |
| en-18 | You call this pair programming?! | en | general | |
| en-19 | Stop pretending you're in a meeting! | en | ignored | |
| en-20 | Hssss! Terminal! NOW! | en | general | |
| en-21 | Meow. Translation: get your a** over here. | en | general | ✔ |
| en-22 | Your terminal is lonely, jerk. | en | input | |
| en-23 | Ship it or I pee on your keyboard. | en | done | ✔ |
| en-24 | Who's the senior dev here? Not you. MOVE. | en | general | |
| en-25 | My tail is twitching. Bad news for you. | en | ignored | |
| en-26 | Claude finished. You haven't even started. | en | done | |
| en-27 | Feed me, then press Enter. | en | input | |
| vi-01 | Dậy đi đồ lười!!! | vi | general | |
| vi-02 | Claude xong rồi kìa, còn ngồi đó à?! | vi | done | |
| vi-03 | Duyệt lẹ đi, tao còn ngủ tiếp! | vi | permission | |
| vi-04 | Đang mơ cá ngừ thì bị gọi dậy. Cảm ơn nhé! | vi | general | |
| vi-05 | Lướt Facebook ít thôi, vào code đi! | vi | ignored | |
| vi-06 | Ê! Terminal đợi mày nãy giờ đó! | vi | input | |
| vi-07 | Bấm Enter giùm cái, mỏi mắt lắm rồi! | vi | input | |
| vi-08 | Không duyệt là tao gạt ly cà phê đấy! | vi | permission | |
| vi-09 | Xong rồi đó, ra nhận công đi sếp! | vi | done | |
| vi-10 | Còn sống thì nhấp chuột một cái coi! | vi | ignored | |
| vi-11 | Mèo này không làm báo thức miễn phí đâu! | vi | general | |
| vi-12 | Sắp deadline rồi mà còn ngồi đó à?! | vi | ignored | |

---

## 8. Claude Code hooks và mapping sự kiện

> Trước khi code phần này, **đọc docs hooks chính thức mới nhất** (https://code.claude.com/docs/en/hooks) và xác minh tên event, `notification_type` và schema stdin của phiên bản Claude Code đang cài. Bảng dưới đây là ý định, không phải schema đã xác minh.

### 8.1 Mapping

| Hook | Điều kiện | Event nội bộ | Hiệu ứng |
|---|---|---|---|
| `Stop` | — | `attention_needed{reason: done}` | Session → pending |
| `PermissionRequest` | — | `attention_needed{reason: permission, toolName}` | Session → pending |
| `Notification` | `permission_prompt` | `attention_needed{reason: permission}` | Fallback, bị dedupe với `PermissionRequest` |
| `Notification` | `idle_prompt` / loại "cần input" khác | `attention_needed{reason: input}` | Session → pending |
| `UserPromptSubmit` | — | `attention_cleared{reason: prompt_submitted}` | Session → not pending |
| `PostToolUse` | session đang pending vì permission và cùng `toolName` | `attention_cleared{reason: tool_resumed}` | User đã duyệt, Claude chạy tiếp |
| `PermissionDenied` (nếu có) | — | `attention_cleared{reason: permission_denied}` | |
| `SessionEnd` | — | `session_ended` | Xoá session |

Notification type lạ hoặc event lạ thì bỏ qua và chỉ ghi log ở mức debug.

### 8.2 Session store (logic thuần, Rust)

```text
sessions: Map<sessionId, { pending: bool, reason, since, projectName, toolName?, dismissed: bool }>
awake = !paused && sessions.any(s => s.pending && !s.dismissed)
```

- **Dedupe:** cùng session, cùng reason, trong vòng 2 giây thì bỏ qua (không wake lại, không reset bộ đếm chửi).
- Mèo đang ACTIVE mà một session khác vừa thành pending: không chạy lại `wake()`, chỉ gọi ngay `yell(false)` (kèm tên project mới).
- Event của session A không được làm thay đổi session B.
- Dismiss: set `dismissed=true` cho các session đang pending. Một `attention_needed` mới của session đó thì reset `dismissed=false`.
- Pending quá `staleAfterMs` (mặc định 30 phút) thì tự clear.
- Mỗi khi `awake` hoặc danh sách session đổi, emit `cat://state { awake, reason, sessions: [{projectName, reason}] }`.

### 8.3 Event contract (hook → server)

```ts
interface HookEvent {
  eventId: string;          // adapter sinh (uuid)
  hookEventName: string;    // Stop | PermissionRequest | Notification | ...
  notificationType?: string;
  sessionId: string;
  projectName?: string;     // basename(cwd); không gửi đường dẫn đầy đủ
  toolName?: string;        // KHÔNG gửi tool_input / command / transcript
  occurredAt: number;       // unix ms
}
```

### 8.4 Bảo mật và an toàn

- Server chỉ bind `127.0.0.1`, cổng ngẫu nhiên, token 256-bit tạo lại mỗi lần app chạy. Runtime file `%APPDATA%\Klat\runtime.json` bị xoá khi app quit.
- Server từ chối khi: token sai, body lớn hơn 64 KB, `hookEventName` không có trong allowlist, hoặc JSON hỏng.
- Hook adapter:
  - Đọc stdin tối đa 256 KB và chỉ trích các field ở §8.3.
  - Timeout HTTP 300ms.
  - **Luôn exit 0 và không in gì ra stdout**, để không bao giờ bị hiểu là một decision allow/deny.
  - App không chạy hoặc lỗi thì thoát im lặng.
- Log local có giới hạn dung lượng và không chứa nội dung lệnh.

### 8.5 Hook installer

1. Target duy nhất: `%USERPROFILE%\.claude\settings.json` (cấp user). Không sửa settings cấp project.
2. Trước khi sửa: parse JSON. Nếu hỏng thì **dừng**, báo lỗi, không ghi gì.
3. Backup sang `settings.json.bak.<timestamp>`.
4. Merge: thêm entry cho từng event ở §8.1 với command `"<path>\klat-hook.exe" <event>` và `timeout: 1`. Nhận diện entry của mình bằng việc command chứa `klat-hook.exe`, để cài lại không bị trùng.
5. Uninstall chỉ xoá entry của mình, giữ nguyên mọi thứ khác (kể cả thứ tự).
6. Path có dấu cách phải được quote và escape JSON đúng. Kiểm tra file exe tồn tại trước khi ghi.
7. Chỉ chạy khi user bấm Install/Repair/Uninstall trên tray. Không tự cài lúc khởi động.

---

## 9. Cửa sổ overlay (Windows)

| Yêu cầu | Cách làm (cần xác minh trên Windows 10/11) |
|---|---|
| Trong suốt, không viền, không bóng | `transparent: true`, `decorations: false`, `shadow: false`; `html, body { background: transparent }`; renderer `alpha: true`, `setClearColor(0x000000, 0)` |
| Không có nút trên taskbar | `skipTaskbar: true` |
| Luôn ở trên cùng | `alwaysOnTop: true` |
| Không cướp focus | `focus: false` khi tạo. Nếu `show()` vẫn lấy focus thì thêm `WS_EX_NOACTIVATE` bằng crate `windows` |
| Vị trí | Góc phải dưới của **work area** màn hình chính (tránh taskbar), lề 16px. Kích thước khoảng **360×280 logical px** (mèo bên phải, bubble bên trái). Xử lý đúng DPI scale |
| Click xuyên qua vùng trống | Mặc định `set_ignore_cursor_events(true)`. Cursor poller kiểm tra con trỏ có nằm trong hitbox của mèo hoặc bubble không (frontend gửi hitbox lên khi layout đổi); nếu có thì tắt ignore để nhận click, ra ngoài thì bật lại |
| Vị trí con trỏ toàn màn hình | `cursor.rs` poll vị trí con trỏ (API cursor position của Tauri, fallback `GetCursorPos`) ở **30Hz khi ACTIVE**, **5Hz khi SLEEP** (chỉ phục vụ hitbox). Emit `cat://cursor {x,y}` theo toạ độ logical so với góc cửa sổ overlay |
| `hideWhileSleeping` | Nếu bật: `window.hide()` khi vào SLEEP (sau khi sleep sequence chạy xong), `show()` trước khi gọi `wake()` |

### 9.1 Ngân sách hiệu năng

- ACTIVE: render 60fps. SLEEP: **giới hạn ở 20fps** (bỏ bớt frame trong rAF). Cửa sổ ẩn: dừng hẳn vòng lặp.
- `renderer.setPixelRatio(Math.min(devicePixelRatio, 2))`.
- Không cấp phát object trong vòng lặp frame (đuôi cập nhật in-place, dùng lại `Vector3`).
- Mục tiêu đo ở Phase 6: CPU khi SLEEP dưới khoảng 1% trên máy dev, RAM toàn app dưới khoảng 150 MB. Ghi lại số đo thực tế vào README.

---

## 10. Settings (`%APPDATA%\Klat\settings.json`)

```json
{
  "notificationsEnabled": true,
  "hideWhileSleeping": false,
  "sfwMode": false,
  "languages": ["en", "vi"],
  "yellIntervalMs": 30000,
  "maxYellsPerWake": 10,
  "staleAfterMs": 1800000,
  "reduceMotion": false,
  "startAtLogin": false,
  "corner": "bottom-right"
}
```

Validate khi load: field nào thiếu thì dùng default. File hỏng thì đổi tên sang `.corrupt.<ts>` rồi tạo lại default, không được crash. (Prototype dùng `yellIntervalMs` 6500 để demo; mặc định thật là 30 giây.)

---

## 11. Các phase triển khai

> **Quy trình bắt buộc:** làm từng phase như một lát cắt dọc. Hết mỗi phase: chạy lint, typecheck và test, sửa lỗi, rồi **dừng lại báo cáo cho user** (đã làm gì, lệnh đã chạy và kết quả, cách tự kiểm tra) và **chờ user xác nhận** trước khi sang phase sau. Các quyết định nhỏ thì tự chọn phương án đơn giản, an toàn và ghi vào README, không cần hỏi.

### Phase 0: Scaffold + spike độ trong suốt
1. Scaffold Tauri 2 + Vite + TypeScript (vanilla) bằng pnpm. Thêm scripts `dev`, `build`, `lint`, `typecheck`, `test`.
2. **Spike:** cửa sổ overlay trong suốt render một khối cầu toon xoay bằng Three.js. Xác minh trên Windows: không có nền đen hay trắng, không viền, không có trên taskbar, luôn ở trên cùng, không cướp focus.
3. Nếu WebGL trong suốt không chạy: ghi rõ triệu chứng, thử các cách khắc phục (premultipliedAlpha, CSS, cấu hình WebView2), rồi báo user trước khi đi tiếp.

**Acceptance:** `pnpm tauri dev` mở ra một khối cầu nổi trên desktop, không có nền.

### Phase 1: Port mèo sang TypeScript
1. Viết `springs.ts`, `CatRig.ts`, `tail.ts` (in-place), `CatController.ts`, `RenderLoop.ts`, `Bubble.ts`, `phrases.ts`, `PhrasePicker.ts` theo §5–§7.
2. `dev.ts`: trang chạy trên browser (`pnpm dev`) có nút Wake / Sleep / Next phrase / Pet, toggle SFW và reduceMotion. Chuột trong trang dùng làm cursor.
3. Unit test:
   - Spring hội tụ về target.
   - PhrasePicker: câu đầu luôn là câu wake; không lặp câu cho tới khi hết bag; lọc SFW và ngôn ngữ đúng; câu `ignored` chỉ xuất hiện từ lần chửi thứ 3.
   - CatController (fake timers): `wake()` khi đang ACTIVE không làm gì; `sleep()` huỷ hết timer; dừng ở `maxYellsPerWake`.

**Acceptance:** trang dev trông và chuyển động giống prototype (so sánh song song), không có cấp phát object trong vòng lặp frame (kiểm tra bằng Performance tab), test xanh.

### Phase 2: Overlay thật + tray
1. Gắn mèo vào cửa sổ overlay ở góc phải dưới của work area, DPI đúng.
2. Tray menu theo §2.3 (Hooks để disable cho tới Phase 5).
3. FPS throttle ở SLEEP. Click-through cho vùng trống, nhận click trong vùng hitbox.
4. Settings load/save (§10).

**Acceptance:** khởi động app thấy mèo ngủ ở góc; `Wake test` trên tray làm mèo bật dậy chửi; click vùng trống xuyên xuống app bên dưới; click bubble dismiss được; focus không rời khỏi terminal.

### Phase 3: Nhìn theo con trỏ toàn màn hình
1. Viết `cursor.rs` theo §9 và bridge sang `lookTarget.ts`.
2. Kiểm tra với nhiều màn hình và DPI 100/125/150%.

**Acceptance:** khi ACTIVE, đầu mèo quay theo con trỏ ở bất kỳ đâu trên màn hình chính, mượt, không giật.

### Phase 4: Event bridge
1. `event_server.rs` (loopback + token + allowlist + giới hạn kích thước) và `session_store.rs` (§8.2) kèm unit test.
2. Crate `klat-hook`: đọc stdin, POST, luôn exit 0, không in stdout.
3. Script giả lập: `scripts/send-test-event.ps1 <HookName> [sessionId]` để test mà không cần chạy Claude Code.
4. Test:
   - Token sai, body quá lớn, event lạ, JSON hỏng đều bị từ chối.
   - Dedupe hoạt động.
   - Event của session A không clear session B.
   - App tắt thì hook thoát nhanh hơn 300ms.

**Acceptance:** chạy script giả lập `Stop` thì mèo dậy; `UserPromptSubmit` thì mèo ngủ.

### Phase 5: Hook installer
1. Viết `hooks_installer.rs` theo §8.5 và nối với menu tray.
2. Test: giữ nguyên hook có sẵn; cài 2 lần không bị trùng; uninstall chỉ xoá entry của mình; JSON hỏng thì không ghi gì.
3. Test end-to-end với Claude Code thật:
   - Một lượt trả lời xong thì mèo dậy.
   - Xin permission thì mèo dậy gần như ngay lập tức.
   - Gửi prompt mới thì mèo ngủ.

### Phase 6: Multi-session, hardening, release
1. Tooltip tray hiện số session; bubble hiện tên project khi có hơn 1 session.
2. Stale timeout, Pause notifications, autostart (opt-in).
3. Đo CPU/RAM (§9.1) và ghi vào README.
4. README hoàn chỉnh: cài đặt, dev, build, cài/gỡ hooks, troubleshooting, privacy, các giới hạn đã biết.

---

## 12. Backlog sau MVP (đừng làm trong MVP)

- **Ngồi trên thanh tiêu đề của cửa sổ terminal đang cần bạn.** Hook gửi PID cha, sau đó dùng Win32 tìm cửa sổ terminal. Click mèo thì focus terminal đó.
- **Vồ con trỏ** khi con trỏ đến gần; rình rồi lao theo con trỏ khi bị lờ lâu.
- **Vuốt quá lâu thì mèo cắn** (mắt trừng, há miệng, bubble "HSSS").
- **Túm gáy kéo thả mèo:** chân lủng lẳng có vật lý; thả xuống thì tiếp đất rồi liếm lông với vẻ khó chịu; nhớ vị trí mới.
- **Leo thang khi bị lờ lâu:** gạt cái cốc ở mép taskbar rơi xuống; cuối cùng nằm chắn giữa màn hình (có giới hạn, tắt được).
- **Nhiều session thì có thêm mèo con,** mỗi con đeo thẻ tên project.
- **Bong bóng mơ khi ngủ,** hiện icon tool Claude đang chạy (cần hook `PreToolUse`, chỉ lấy `tool_name`).
- **Tamagotchi:** mỗi lượt hoàn tất được thưởng một con cá, mèo mập dần trong ngày.
- **Âm thanh** (meow, tiếng rừ rừ), mặc định tắt.
- Model chi tiết hơn làm bằng **Blender → glTF**, giữ nguyên CatController và tên các tham số.
- Hỗ trợ macOS/Linux.

---

## 13. Definition of Done (MVP)

1. App Tauri chạy nền trên Windows, có tray. Mèo ngủ hiện ở góc màn hình, hoặc bị ẩn nếu bật `hideWhileSleeping`.
2. Claude xong lượt hoặc chờ input/permission thì mèo bật dậy với "Wake up, B**CH!!!". Bị lờ thì mèo chửi tiếp bằng câu ngẫu nhiên. Gửi prompt mới thì mèo ngáp rồi ngủ lại.
3. Khi ACTIVE, đầu mèo quay theo con trỏ trên toàn màn hình; chuyển động mượt nhờ spring.
4. Overlay không cướp focus, vùng trống click xuyên qua được, CPU khi ngủ thấp.
5. Installer an toàn, idempotent và giữ nguyên các hook khác. Hook không bao giờ làm chậm hay chặn Claude Code và không bao giờ đưa ra quyết định permission.
6. Có đủ test cho spring, phrase picker, controller, session store, event server và installer; tất cả đều xanh. README đầy đủ.
