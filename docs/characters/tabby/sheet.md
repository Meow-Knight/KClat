# Mèo mướp (`tabby`): character sheet

> Đại ca hẻm: một con mèo mướp nâu xám vằn vện, chữ M trên trán, tai to, trán dán miếng băng chéo từ trận đánh tối qua. Bị gọi dậy là cáu kiểu giang hồ xóm chợ: "Ê! Claude kêu kìa, đừng để tao nhắc lần hai."

![Thức](awake.png) ![Ngủ](asleep.png)

Bảng 10 tư thế: [sheet.png](sheet.png). Code: `src/characters/tabby/TabbyRig.ts`, một `CatLook` trên rig mèo `src/characters/cat/CatRig.ts`; vằn dán lên bề mặt bằng `src/characters/cat/markings.ts`.

## Brief

- **Ai**: mèo mướp (mèo ta vằn mackerel tabby), theo yêu cầu của chủ dự án.
- **Dấu nhận diện**: lông nâu xám với vằn sẫm; chữ "M" trên trán; vằn má; đuôi khoanh vằn chóp sẫm.
- **Nét mới**: vằn khắp thân (sườn, đùi), đuôi khoanh 4 vòng; miếng băng dán chéo trên thái dương phải; tai to hơn, thân gọn hơn kiểu mèo hẻm.
- **Tính cách / sắc thái cau có**: đại ca hẻm. Cộc, thẳng, doạ nạt kiểu giang hồ vặt nhưng thật ra là canh chừng cho bạn; nhớ mặt từng người làm phiền.
- **Bản quyền**: không áp dụng.

## DNA

| Mục | Giữ nguyên? | Nếu lệch: lệch gì, vì sao |
|---|---|---|
| Dựng khối từ primitive, tỉ lệ chibi, ngồi, 3/4 | Giữ nguyên | Thân (1.1, 0.95, 0.98) và đầu (1.27, 1, 1.05) gọn hơn mèo ~3%; tai ×1.12 |
| Toon 3 bậc, đèn chuẩn | Giữ nguyên | |
| Viền mực inverted hull | Giữ nguyên | Vằn là decal không viền; băng dán có viền 1.08 |
| Khuôn mặt Klat | Giữ nguyên | Bố cục mặt y hệt mèo (`CAT_FACE`) |
| Cau có mặc định, 5 trạng thái mặt | Giữ nguyên | |
| Lò xo, squash, thở, nhịp nền | Giữ nguyên | Rig mèo dùng chung |
| Hiệu ứng chung | Giữ nguyên | Vị trí như mèo (`CAT_FX`) |
| Bong bóng thoại, giọng | Giữ nguyên | |

## Bảng màu

| Vai trò | Hex | Ghi chú (qua luật màu thế nào) |
|---|---|---|
| main | `#B39E80` | Nâu xám ấm, sáng ~61%: đủ 3 dải toon |
| shade | `#6B5844` | Vằn, chóp đuôi, khoanh đuôi, chữ Zzz. Tối hơn main ~40%, nhiều hơn mức 20–30% để vằn đọc rõ ở cỡ nhỏ (như capybara); vẫn sáng hơn INK nên không lẫn viền |
| light | `#F6EEDF` | Kem ấm: bụng, mõm, bàn chân, băng dán |
| accent | `#E08A7B` | Hồng gạch: mũi, lòng tai |
| iris | `#9BC83A` | Xanh lá vàng, tương phản với lông ấm |
| blush | `#F4B0A2` | Hồng cam nhạt hơn accent |
| *(phụ kiện)* | dùng light | Băng dán chéo |

6 màu riêng. Đọc rõ trên nền tối `#1E1F24` và nền xám (đã chụp cả hai).

## Khối

Như mèo (`CatRig`), trừ:

| Bộ phận | Primitive | Vị trí | Scale / kích thước | Màu | Viền |
|---|---|---|---|---|---|
| Thân | ellipsoid | (0, 0.95, 0) | (1.1, 0.95, 0.98) | main | 1.06 |
| Đầu | ellipsoid | (0, 2.05, 0.1) | (1.27, 1, 1.05) | main | 1.06 |
| Tai ×2 | cone mèo ×1.12 | gốc (±0.7, 0.7, 0), nghiêng ra 0.28 | | main / lòng accent | 1.06 / không |
| Chữ M (4 nét) | ellipsoid decal | trán, x ±0.3 / ±0.1, y 0.7 / 0.67; nét ngoài nghiêng ±0.3, nét trong ±0.5 | (0.055, 0.17 / 0.15, 0.05) | shade | không |
| Vằn má ×2 mỗi bên | ellipsoid decal | θ ±1.05 / ±1.1, y 0.03 / −0.12 | (0.22 / 0.2, 0.035, 0.05) | shade | không |
| Vằn sườn ×4 mỗi bên | ellipsoid decal | thân, θ ±1.0 / 1.45 / 1.9 / 2.35, y +0.22 | (0.08, 0.42, 0.06) | shade | không |
| Vằn đùi ×2 mỗi bên | ellipsoid decal | đùi, θ ±1.25 / 1.75, y +0.06 | (0.06, 0.27, 0.05) | shade | không |
| Khoanh đuôi ×4 | torus dẹt bám theo đuôi | u 0.3 / 0.48 / 0.66 / 0.84 dọc đuôi | mép ngoài = r đuôi + 0.032 (nằm trong viền), rộng ~0.11 | shade | không |
| Băng dán | 2 capsule dẹt bắt chéo ±0.75 | thái dương phải, θ 0.82, y 0.5 | r 0.065, dài 0.3, sâu ×0.35 | light | 1.08 |
| Đuôi | tube 4 điểm như mèo | | r 0.15 | main, chóp shade | +0.04 |

Decal đặt theo pháp tuyến bề mặt ellipsoid (θ tính từ trước ra, dương về +x). Vằn vượt mức "khoảng 5 mảng" của DNA: vằn là chính dấu nhận diện của mèo mướp.

## Ánh xạ pose

Như mèo (rig dùng chung):

| Tham số | Nơi nhận | Cách áp (công thức) |
|---|---|---|
| sleepy | Đầu gục, đuôi khoanh cuộn quanh chân | như mèo; khoanh đuôi bám theo đuôi mỗi frame |
| ear | Tai xoay ở gốc | `rotation.z = −s·(0.28 + (1 − ear)·0.5)` |
| earTw | Tai giật | như mèo |
| tailUp | Đuôi dựng, vẫy nhanh | như mèo |
| swish | Đuôi quất khi bị chọc lúc ngủ | như mèo |
| puff | Thân +8%, tai +15%, đuôi và khoanh to ra | như mèo |
| tapping | Chân trước phải gõ | như mèo |

## Hiệu ứng

Như mèo. Hitbox thức cao tới y 3.7 cho tai to.

## Giọng

Chưa đưa vào app (câu thoại vẫn dùng chung). Gợi ý cho sau này:

- "Wake up, B**CH!!! Claude's calling. Don't make me say it twice."
- "I've got a reputation on this street. Answer the damn prompt."
- "Ê! Dậy! Claude kêu kìa, đừng để tao nhắc lần hai."
- "Cả hẻm này ai cũng biết tao. Trả lời Claude đi cho tao còn ngủ."
- SFW: "Oi. Claude's waiting. Move it."

## Bài test Klat

| # | Mục | Đạt? | Ghi chú |
|---|---|---|---|
| 1 | Che thân → nhận ra mặt Klat | Đạt | Mặt mèo Klat y nguyên |
| 2 | Che mặt → không nhầm với mèo | Đạt | Tai to hơn, đuôi khoanh, vằn sườn và đùi, băng dán; khác hẳn mèo xám trơn |
| 3 | Tỉ lệ | Đạt | Đầu rộng hơn thân, ngồi, quay 3/4 |
| 4 | Viền | Đạt | Khối đặc và băng dán có viền; vằn, khoanh đuôi không; khoanh đuôi không cắt viền đuôi |
| 5 | Toon | Đạt | 3 dải phẳng |
| 6 | Cau có | Đạt | Mắt lim dim, lông mày chụm |
| 7 | Màu | Đạt | 6 vai trò; shade sẫm hơn luật để vằn rõ, vẫn tách khỏi INK; rõ trên nền tối và sáng |
| 8 | Pose | Đạt | Mọi tham số có nơi nhận; ảnh ngủ thấy rõ đuôi khoanh cuộn |
| 9 | Dấu nhận diện | Đạt | Vằn, chữ M, vằn má, đuôi khoanh đều thấy |
