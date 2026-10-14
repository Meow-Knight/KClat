# DNA của Klat

Nguồn chuẩn: kit dùng chung `src/kit/` (khuôn mặt `face.ts`, hiệu ứng `fx.ts`, chuyển động `motion.ts`, toon và viền `ToonBuilder.ts`, màu chung `palette.ts`) và con mèo `src/cat/CatRig.ts`. Số liệu dưới đây lấy từ đó. Khi lệch nhau, code thắng; sửa lại file này.

Đơn vị: world units, gốc ở chân nhân vật, y hướng lên, z hướng về người xem. Toạ độ trên mặt là head-local (gốc ở tâm đầu). Bộ phận có cặp trái/phải ghi theo bên phải (+x), bên trái lấy đối xứng.

## 1. Bất biến

Đây là những gì làm nên "chất Klat". Nhân vật mới giữ nguyên tất cả.

### 1.1 Dựng khối

- Chỉ dựng từ primitive: ellipsoid (sphere bán kính 1 rồi scale), cone, torus, capsule, cylinder, tube CatmullRom qua 4 điểm. Không texture, không model ngoài. Hoa văn cũng là khối (decal mỏng), không vẽ lên bề mặt.
- Mọi thứ tròn trịa và mập. Góc nhọn chỉ có ở đầu cone (tai, sừng, gai).
- **Tỉ lệ chibi**: đầu là khối lớn nhất và **rộng hơn thân**. Mèo: thân ellipsoid (1.15, 0.95, 1) tâm y 0.95 (rộng 2.3, cao 0–1.9); đầu (1.3, 1, 1.05) tâm y 2.05, z 0.1 (rộng 2.6, đỉnh y 3.05). Đầu chiếm khoảng 2/3 chiều cao (chưa tính tai). Cho phép lệch ±15%.
- Tư thế **ngồi**: thân hình trứng, hai đùi sau (0.45, 0.4, 0.55) ở (±0.78, 0.42, 0.25), hai bàn chân trước nhỏ màu sáng (0.3, 0.2, 0.34) ở (±0.42, 0.17, 0.9), bụng màu sáng (0.68, 0.6, 0.5) ở (0, 0.85, 0.6) không viền.
- Góc nhìn 3/4: `root.position.x = 1.3`, `root.rotation.y = -0.22` (hơi quay về phía bong bóng thoại bên trái).

### 1.2 Render

- `MeshToonMaterial` với gradient 3 bậc `[95, 180, 255]`, `DataTexture` 3×1, filter `Nearest`. Đúng 3 dải sáng, cạnh dải sắc.
- Đèn: `AmbientLight(0xffffff, 0.95)` + `DirectionalLight(0xffffff, 2.3)` đặt ở (-3, 5, 6). Sáng từ trên-trái-trước.
- **Viền mực kiểu sticker**: inverted hull, bản sao mesh dùng `MeshBasicMaterial({color: INK, side: BackSide})`, scale 1.06 cho khối lớn. Khối nhỏ cần hệ số lớn hơn để nét trông dày ngang nhau: mõm 1.08, cục u 1.1, mống mắt 1.12, mũi 1.18, chóp đuôi 1.25. Decal phẳng (bụng, má hồng, vằn, trong tai) **không viền**.
- Nét vẽ (lông mày, ria, mắt nhắm, miệng ω, mắt ✕) là mesh mảnh `MeshBasicMaterial` màu INK, không bị đổ sáng.
- Bóng đổ: đĩa đen opacity 0.13, bán kính 1.4, co lại khi nhảy (`max(0.5, 1 - jumpY·0.35)`).

### 1.3 Khuôn mặt (dấu nhận diện mạnh nhất)

Mọi nhân vật, kể cả nhân vật gốc có kiểu mắt riêng, đều dùng **khuôn mặt Klat**. Chỉ được đổi màu mống mắt, vị trí bộ mặt cho khớp hình đầu, và kích thước mắt đều cả khối (0.9–1.15).

| Bộ phận | Cấu tạo (mèo, head-local) |
|---|---|
| Mắt | Nhóm ở (±0.45, 0.08, 0.9), quay y ±0.32 theo độ cong đầu. Mống: ellipsoid (0.24, 0.27, 0.12) màu iris, viền 1.12. Đồng tử: (0.17, 0.21, 0.08) ở z 0.06, màu PUPIL. **Hai đốm sáng trắng**: đốm to (0.06, 0.06, 0.03) ở (-0.07, 0.09, 0.13), đốm nhỏ (0.03, 0.03, 0.02) ở (0.06, -0.08, 0.13). Hai mắt cùng phía đốm (theo hướng đèn). |
| Mắt nhắm ∪ / vui ∩ | Torus nửa vòng r 0.19, ống 0.035, màu INK. ∩ hạ xuống 0.08. |
| Mắt bất tỉnh ✕ | Hai capsule INK chéo ±45°, scale (1.3, 1.25, 1). |
| Lông mày | Capsule INK r 0.035, dài 0.2, y 0.36, góc `π/2 + s·(grumpy·0.45 − 0.12)`: chụm lại khi cau có. |
| Miệng ω | Hai torus nửa vòng r 0.07, ống 0.02, màu INK, ở (±0.07, −0.33, 1.1). |
| Miệng há | Ellipsoid MOUTH (0.13, 0.02 + 0.16·mouth, 0.06) ở (0, −0.42, 1.06), không viền. |
| Má hồng | Ellipsoid dẹt (0.17, 0.08, 0.05) ở (±0.78, −0.18, 0.8), không viền. |
| Mõm *(tuỳ loài)* | Hai khối màu sáng (0.3, 0.22, 0.22) ở (±0.22, −0.32, 0.9), viền 1.08. |
| Mũi *(tuỳ loài)* | Ellipsoid nhỏ màu accent (0.1, 0.07, 0.06) ở (0, −0.17, 1.05), viền 1.18. |

Bố cục: mắt to (cao khoảng 1/4 chiều cao đầu), hơi trên tâm đầu, cách nhau khoảng 1/3 bề rộng đầu; mũi, miệng, má dồn xuống nửa dưới. Đồng tử lệch theo hướng nhìn: (lx·0.08, −ly·0.06).

### 1.4 Biểu cảm và tính cách

- Mặc định khi thức là **cau có**: mắt lim dim (`eye` 0.62), lông mày chụm (`grumpy` 1). Chửi thì mắt trừng (`eye` 1.25, mống phình ngang), lông mày bật lên, miệng há.
- Đủ 5 trạng thái mặt: mở, ∪ ngủ, ∩ vui khi được vuốt, ✕ bất tỉnh, há miệng (chửi, ngáp).
- Mắt chuyển: `eye < 0.18` thành ∪; `eye > 1` phình mống `(1 + (eye−1)·0.6, min(1.35, eye))`.

### 1.5 Chuyển động

- **Chỉ dùng lò xo**, không keyframe. `vel += (k·(t − v) − d·vel)·dt; v += vel·dt`, dt tối đa 1/30. Nảy bằng cách cộng vào `vel`.
- Giữ nguyên tên, giá trị và ý nghĩa các tham số pose ở §3. Bộ não `src/cat/CatController.ts` dùng chung cho mọi nhân vật.
- Nhảy: trọng lực 18, chạm đất với v < −1.2 thì `squash.vel += −v·1.3`.
- Squash & stretch giữ thể tích, neo ở chân: `scale = (1 + sq·0.55, 1 − sq, 1 + sq·0.55)`.
- Thở: thân `scale.y` dao động ±0.014, tần số 1.4 rad/s khi ngủ, 2.4 khi thức; đầu nhấp nhô ±0.012 lệch pha −0.6.
- Đầu: `rotation = (ly·awake + sleepy·0.3 − yawn·0.38, lx·awake, sleepy·0.18 + happy·0.2 + purr)`, với purr = `sin(70t)·0.014`.
- Ngủ: đầu hạ 0.4, nhô ra 0.18, nghiêng, cuộn bộ phận dài (đuôi) quanh chân.
- Nhịp nền: chớp mắt mỗi 1.6–5 s khi thức; giật tai mỗi 2–6.5 s.
- `reduceMotion`: tắt nhảy, squash, puff, rung; lò xo chuyển critically damped; vẫn đổi biểu cảm.

### 1.6 Hiệu ứng chung

Mọi nhân vật có đủ bộ này, cùng kiểu dáng; chỉ đổi vị trí cho khớp thân hình:

- **Aura Super Saiyan** vàng khi thức: hai lớp gai năng lượng, vệt dọc chạy lên, tia sét trắng. Gai giật khoảng 14 bước/giây kiểu hoạt hình vẽ tay.
- **Dấu giận** kiểu anime (bốn nét cong đỏ `#E5383B`, viền trắng) ở góc trên đầu khi bị chọc lúc ngủ.
- **Zzz** bay lên chéo phải, màu shade của nhân vật, viền trắng.
- **Knockout**: que, dép tổ ong, sầu riêng (rơi từ trên xuống), chảo chống dính bay vào đầu; chớp nổ trắng lõi vàng; 3 ngôi sao vàng viền mực quay quanh đầu; cục u màu chính có đốm sáng trắng; mắt ✕.

### 1.7 Giọng và UI

- Bong bóng thoại: nền trắng, viền 2px INK, bo 16px, font **Grandstander** 700 cỡ 15px; bật ra từ `scale(0) rotate(-8deg)` có overshoot, chữ rung khi chửi.
- UI phụ dùng **Nunito**.
- Giọng: cau có, xấc, chửi đổng mà buồn cười, song ngữ Anh/Việt, có bản SFW. Câu đầu khi thức luôn kiểu "Wake up, B**CH!!!".

## 2. Trục tự do

Dồn sự mới mẻ vào đây. Mỗi nhân vật phải có **ít nhất một nét mới** con mèo không có.

| Trục | Được đổi gì | Giới hạn |
|---|---|---|
| Bộ phận loài | Tai cụp/tròn/dài, sừng, ăng-ten, mào tóc, mỏ, vây, cánh, đuôi (tube 4 điểm), áo choàng, khăn | Dựng từ primitive, có viền, bám đúng nơi nhận pose (§3) |
| Dáng thân | Mập hơn/gầy hơn, tai cao hơn, chân ngắn hơn | Trong ±15% tỉ lệ chibi, vẫn ngồi, đầu vẫn rộng hơn thân |
| Bảng màu | Cả 6 vai trò | Theo luật màu (§2.1) |
| Hoa văn | Vằn, đốm, mảng mặt nạ, sọc | Decal mỏng không viền, màu shade/light, tối đa khoảng 5 mảng |
| Phụ kiện | Mũ, kính, khăn, nơ, chuông, món đồ kinh điển của nhân vật gốc | Tối đa 2 món, primitive có viền, không che mắt hay miệng |
| Mõm, mũi, ria | Bỏ hoặc đổi dáng | Bộ mặt trong §1.3 vẫn giữ |
| Tật riêng | Thay cho gõ chân khi sốt ruột: gõ ngón tay, vỗ cánh, đập đuôi, xoay ăng-ten… | Nhận tham số `tapping`, nhịp `sin(10t)` |
| Giọng | Thêm câu cửa miệng, sắc thái chửi riêng | Vẫn cau có, vẫn có bản SFW |

### 2.1 Luật màu

Mèo làm mẫu: main `#9AA6BD`, shade `#6B7690`, light `#F6F0E6`, accent `#F29BB0`, iris `#F5B83D`, blush `#F7A9BC`. Chung cho mọi nhân vật: INK `#2B2D42`, PUPIL `#1B1B2F`, MOUTH `#5A2A3A`, đốm sáng `#FFFFFF`.

- **main** sáng vừa, hơi ngả xám (độ sáng khoảng 55–75%) để cả 3 dải toon đều đọc được. Tránh trắng tinh (dải sáng cháy) và tránh tối gần INK (mất viền).
- **shade** cùng tông với main, tối hơn khoảng 20–30%.
- **light** trắng ngà ấm, không dùng `#FFFFFF`.
- **iris** bão hoà, tương phản với tông của main (lông lạnh thì mắt ấm, lông ấm thì mắt lạnh hoặc xanh lá), sáng hơn hẳn PUPIL.
- **accent** và **blush** là hồng hoặc đỏ ấm; blush nhạt hơn accent.
- Tổng cộng tối đa 6 màu riêng (cộng màu phụ kiện) ngoài INK, PUPIL, MOUTH.
- Phải đọc rõ trên cả desktop tối lẫn sáng: kiểm tra trên nền `#1E1F24` và nền ca-rô sáng.

## 3. Ánh xạ pose

Bộ não chung đẩy ra các tham số này mỗi frame. Nhân vật mới phải cho mỗi tham số một nơi nhận nhìn thấy được. Nếu không có tai hay đuôi thì chọn bộ phận khác mang cùng cảm xúc.

| Tham số | Cảm xúc | Mèo dùng cho | Gợi ý nơi nhận khác |
|---|---|---|---|
| `sleepy` 0–1 | Tư thế ngủ | Đầu hạ, nghiêng; đuôi cuộn quanh chân; tai cụp | Cuộn khăn/áo choàng/cánh quanh người |
| `eye` | Độ mở mắt | Mắt (§1.3) | Không đổi |
| `grumpy` | Cau có | Lông mày | Không đổi |
| `mouth` | Há miệng | Miệng | Không đổi; mỏ thì tách hai nửa |
| `happy`, `purring` | Được vuốt | Mắt ∩, nghiêng đầu, rung | Không đổi |
| `lx`, `ly` | Nhìn theo chuột | Xoay đầu + lệch đồng tử | Không đổi |
| `yawn` | Ngáp | Ngửa đầu | Không đổi |
| `ear` 0–1 | Cảnh giác (1 = dựng) | Tai xoay ở gốc | Ăng-ten, mào tóc, sừng mềm, vây |
| `earTw` | Giật | Tai giật | Ăng-ten rung, mào tóc nảy |
| `tailUp` | Hăng, sốt ruột | Đuôi dựng, vẫy nhanh | Áo choàng tung, khăn bay, cánh vỗ |
| `swish` | Bị chọc lúc ngủ | Đuôi quất | Như trên, biên độ nhỏ |
| `puff` | Xù lên khi giật mình | Thân +8%, tai +15%, đuôi to ra | Lông/gai/tóc dựng, cánh xoè |
| `squash`, `jumpY` | Nhảy, tiếp đất | Toàn thân (§1.5) | Không đổi |
| `tapping` | Sốt ruột | Chân phải gõ | Tật riêng (§2) |
| `aura`, `annoyed`, `dizzy`, `bump`, `zzz`, `bonk*` | Hiệu ứng | §1.6 | Chỉ dời vị trí |

## 4. Bài test Klat

Chấm trên ảnh `awake.png` và `asleep.png`. Mọi mục phải đạt.

1. **Che thân, chỉ nhìn mặt**: nhận ra ngay khuôn mặt Klat (mắt hai đốm sáng, lông mày mực, ω, má hồng).
2. **Che mặt, chỉ nhìn bóng dáng**: không nhầm với con mèo. Có nét mới rõ ràng.
3. **Tỉ lệ**: đầu rộng hơn thân, ngồi, quay 3/4.
4. **Viền**: mọi khối đặc có viền mực dày đều; decal thì không.
5. **Toon**: đúng 3 dải phẳng, không gradient mềm, không texture.
6. **Cau có**: ảnh thức có mắt lim dim hoặc trừng và lông mày chụm, không phải mặt hiền.
7. **Màu**: đủ 6 vai trò, qua luật §2.1, đọc rõ trên nền tối và sáng.
8. **Pose**: mọi tham số §3 có nơi nhận; ảnh ngủ thấy rõ tư thế cuộn.
9. **Dấu nhận diện** (nhân vật gốc): 2–3 dấu trong brief đều nhìn thấy được.
