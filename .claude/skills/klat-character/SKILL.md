---
name: klat-character
description: DNA thiết kế nhân vật của Klat (mèo toon cau có). Dùng khi tạo nhân vật mới cho Klat (con vật khác, hoặc "Klat hoá" một nhân vật người dùng yêu thích), làm biến thể của con mèo, hoặc kiểm tra một thiết kế còn "chất Klat" không.
---

# Tạo nhân vật Klat

Mục tiêu: một nhân vật **mới** nhưng ai nhìn cũng biết là người nhà Klat. Công thức: giữ nguyên **DNA** (dựng khối, toon, viền mực, khuôn mặt, chuyển động, hiệu ứng, giọng), và dồn toàn bộ sự mới mẻ vào **trục tự do** (dáng, bộ phận đặc trưng loài, bảng màu, phụ kiện, tật riêng, giọng thoại).

DNA và trục tự do, kèm số đo chính xác: [DNA.md](DNA.md). Đọc hết trước bước 2.

Sản phẩm: `docs/characters/<id>/` gồm `sheet.md` (theo [SHEET-TEMPLATE.md](SHEET-TEMPLATE.md)), `awake.png`, `asleep.png`, `sheet.png`; và rig `src/characters/<id>/<Name>Rig.ts`. Mẫu đã làm: `docs/characters/snake`, `docs/characters/dragon`.

## Bước 1: Brief

Chốt bốn điều, hỏi người dùng những điều còn thiếu (gộp vào một lần hỏi):

- **Ai**: loài vật, hoặc nhân vật gốc.
- **Dấu nhận diện**: 2–3 đặc điểm khiến người ta nhận ra ngay (với nhân vật gốc: bộ phận dáng, mảng màu, món phụ kiện kinh điển). Đây là những gì được mang sang; phần còn lại theo DNA.
- **Nét mới**: ít nhất một yếu tố đặc trưng mà con mèo không có (bộ phận, phụ kiện hoặc tật riêng).
- **Tính cách**: vẫn cau có, phiền phức, chửi đổng vui (DNA), nhưng có sắc thái riêng: cau có kiểu nào?

Xong khi phần Brief trong sheet không còn ô trống. Nếu nhân vật gốc có bản quyền, ghi một dòng trong sheet để chủ dự án quyết định trước khi phát hành.

## Bước 2: Thiết kế (sheet)

Copy [SHEET-TEMPLATE.md](SHEET-TEMPLATE.md) thành `docs/characters/<id>/sheet.md` rồi điền:

- Mỗi dòng DNA ghi "giữ nguyên"; chỗ nào lệch khỏi DNA phải có lý do (hiếm).
- Bảng màu đủ 6 vai trò, qua hết các luật màu trong DNA.md.
- Bảng **ánh xạ pose**: mỗi tham số trong DNA.md có một bộ phận nhận nó, hoặc ghi rõ "bỏ" kèm lý do.
- Danh sách khối: mỗi bộ phận là primitive nào, toạ độ, scale, có viền hay không.

Xong khi sheet không còn `TBD` và mọi tham số pose đều có nơi nhận.

## Bước 3: Dựng rig

Rig là bản dựng thật, dựng trên kit dùng chung `src/kit/` (DNA nằm sẵn trong đó). Lấy `src/characters/snake/SnakeRig.ts` hoặc `src/characters/dragon/DragonRig.ts` làm mẫu, cái nào gần dáng nhân vật mới hơn.

1. Tạo `src/characters/<id>/<Name>Rig.ts`, implement `CharacterRig`. Dùng `ToonBuilder` (`mk()` có toon + viền), `Face` với `FaceLayout` của nhân vật, `CharacterFx` với `FxLayout`, các hàm trong `kit/motion.ts`, `ToonTube` cho đuôi/khăn/râu. Chỉ tự viết phần riêng của loài.
2. Trong `applyPose()`, nối từng tham số pose vào bộ phận nhận nó theo bảng ánh xạ. Chuyển động trang trí (rung, vỗ, khói) tắt khi `pose.auraFlicker` là false (reduceMotion).
3. Đăng ký: thêm id vào `CHARACTERS` (`src/settings.ts`) và `RIGS` (`src/characters/index.ts`).

Chụp ảnh bằng trang dev `characters.html`: 10 tư thế cố định (ngủ, thức, chửi, được vuốt, ngáp, bị chọc, 4 vật knockout), cùng thời điểm nên lần chạy nào cũng ra một ảnh.

```bash
npx vite --port 5199 --strictPort &
CH="/c/Program Files/Google/Chrome/Application/chrome.exe"   # hoặc msedge.exe
shot() { "$CH" --headless=new --use-angle=swiftshader --enable-unsafe-swiftshader --hide-scrollbars   --virtual-time-budget=15000 --window-size=$1 --screenshot="$2" "http://localhost:5199/characters.html?c=<id>$3"; }
shot 720,660 docs/characters/<id>/asleep.png "&px=2&poses=0"
shot 720,660 docs/characters/<id>/awake.png "&px=2&poses=1"
shot 1800,660 docs/characters/<id>/sheet.png ""
shot 1800,660 /tmp/<id>-dark.png "&bg=dark"                 # luật màu: nền tối
```

Xem cả bảng 10 tư thế, không chỉ hai ảnh. Xong khi mọi tư thế hiện đúng nhân vật, không khối nào lơ lửng hay xuyên qua nhau, không bộ phận nào bị đầu che mất (cánh, đuôi), và hiệu ứng (aura, sao, dấu giận, vật bay vào đầu) nằm đúng chỗ.

Sửa `src/kit/` là sửa mọi nhân vật. Sau khi sửa kit, so hash các ô của mèo trên `characters.html?c=cat` (in dưới ảnh) trước và sau: phải giống hệt.

## Bước 4: Bài test "còn là Klat không?"

Chấm từng mục trong phần **Bài test Klat** của DNA.md dựa trên các ảnh chụp, ghi kết quả đạt hoặc trượt vào cuối sheet. Mục nào trượt thì sửa rig, chụp lại, chấm lại. Xong khi mọi mục đều đạt.

## Bước 5: Đưa vào Settings

Nhân vật mới chỉ cần một nhãn hiển thị: thêm `settings.character.<id>` vào `src/i18n/en.json` và `vi.json`, và thêm biến thể vào `enum Character` trong `src-tauri/src/settings.rs`. Thẻ chọn có hình trong Settings, việc lưu và đổi nhân vật lúc đang chạy đã tự lấy theo `CHARACTERS`. Ảnh trên thẻ được vẽ từ chính rig lúc mở Settings (`src/characters/pictures.ts`, tư thế `GRUMPY_POSE` và `ASLEEP_POSE`): mở `settings.html?mock` qua `pnpm dev` và xem nhân vật mới nằm gọn trong khung, cánh hay sừng không bị cắt. Chạy `pnpm typecheck`, `pnpm test`, `pnpm lint`, `cargo test --workspace`; `tests/characters.test.ts` tự kiểm tra mọi nhân vật đã đăng ký.

## Bước 6: Bàn giao

Gửi người dùng ảnh thức, ảnh ngủ và bảng 10 tư thế, tóm tắt DNA đã giữ và các nét mới, kèm đường dẫn sheet và rig.
