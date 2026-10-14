# Rồng (`dragon`): character sheet

> Một bé rồng mập màu san hô, mặt Klat, cau có kiểu nóng tính: vỗ cánh phành phạch, đập đuôi xuống đất, phụt khói mũi "hừ" mỗi lần chửi.

![Thức](awake.png) ![Ngủ](asleep.png)

Bảng 10 tư thế: [sheet.png](sheet.png). Code: `src/characters/dragon/DragonRig.ts` (số liệu chính xác ở đó; bảng dưới là bản tóm tắt).

## Brief

- **Ai**: rồng con kiểu phương Tây (con vật tưởng tượng, không phải nhân vật có bản quyền).
- **Dấu nhận diện**: cánh dơi, sừng, đuôi mũi giáo.
- **Nét mới**: tai vây hai bên; giáp bụng và gai lưng; đập đuôi thay gõ chân; khói mũi khi chửi.
- **Tính cách / sắc thái cau có**: nóng tính, bốc hoả nhanh, "hừ" ra khói. Cau có như mèo nhưng ồn ào hơn.
- **Bản quyền**: không áp dụng.

## DNA

| Mục | Giữ nguyên? | Nếu lệch: lệch gì, vì sao |
|---|---|---|
| Dựng khối từ primitive, tỉ lệ chibi, ngồi, 3/4 | Giữ nguyên | Dùng đúng dáng ngồi của mèo |
| Toon 3 bậc, đèn chuẩn | Giữ nguyên | |
| Viền mực inverted hull | Giữ nguyên | Màng cánh, sừng dùng hệ số viền lớn hơn (1.12, 1.18) vì mỏng/nhỏ |
| Khuôn mặt Klat | Giữ nguyên | Mõm nhô hơn mèo; không mũi hồng, thay bằng 2 lỗ mũi mực; không ria |
| Cau có mặc định, 5 trạng thái mặt | Giữ nguyên | |
| Lò xo, squash, thở, nhịp nền | Giữ nguyên | |
| Hiệu ứng chung | Giữ nguyên | Thêm khói mũi (riêng của rồng) |
| Bong bóng thoại, giọng | Giữ nguyên | |

## Bảng màu

| Vai trò | Hex | Ghi chú |
|---|---|---|
| main | `#E58E6B` | San hô, độ sáng ~66%: đủ 3 dải toon |
| shade | `#B3604A` | Cùng tông, tối hơn ~25%: màng cánh, gai, mũi giáo, vạch giáp, chữ Zzz |
| light | `#FBE6C2` | Kem ấm: bụng, mõm, sừng, móng |
| accent | `#E8677A` | Hồng đỏ: mặt trong tai vây |
| iris | `#2EC4B6` | Xanh ngọc, tương phản với thân màu ấm |
| blush | `#F59AA8` | Hồng, nhạt hơn accent |
| *(khói)* | `#DCDBE6` | Xám nhạt có viền |

Đọc rõ trên nền tối `#1E1F24` và nền xám sáng (đã chụp cả hai).

## Khối

Toạ độ world tính từ gốc `root` (x 1.3, quay y −0.22); đầu là head-local.

| Bộ phận | Primitive | Vị trí | Scale / kích thước | Màu | Viền |
|---|---|---|---|---|---|
| Thân, đùi, bàn chân | như mèo | như mèo | như mèo | main | 1.06 |
| Bụng | ellipsoid | (0, 0.85, 0.6) | (0.68, 0.6, 0.5) | light | không |
| Vạch giáp ×3 | torus cung 1.5 rad ôm bụng | y 0.62 / 0.85 / 1.08 | ống 0.035 | shade | không |
| Gai lưng ×3 | cone | sống lưng y 1.6 / 1.25 / 0.85 | r 0.1, cao 0.26 | shade | 1.08 |
| Móng ×6 | sphere | mép trước bàn chân | 0.05 | light | 1.12 |
| Cánh ×2 | ellipsoid dẹt + 3 thuỳ + capsule xương | gốc (±0.6, 1.5, −0.62), vươn qua mép đầu | màng (0.72, 0.5, 0.05), xương dài 1.05 | shade + main | 1.12 / 1.06 |
| Đầu | ellipsoid | (0, 2.05, 0.1) | (1.3, 1, 1.05) | main | 1.06 |
| Sừng ×2 | cone | (±0.42, 1.12, −0.12), ngả sau 0.45 | r 0.13, cao 0.62 | light | 1.18 |
| Tai vây ×2 | cone dẹt + cone trong | gốc (±1.12, 0.3, −0.15), chĩa ngang | r 0.26, dài 0.62 | main + accent | 1.06 / không |
| Lỗ mũi ×2 | sphere | (±0.1, −0.14, 1.05) | 0.03 | INK | không |
| Khói ×4 | sphere | phụt chéo xuống ra ngoài từ lỗ mũi | 0.04 → 0.15 → 0 | khói | 1.12 |
| Đuôi | tube 4 điểm | như mèo, dài hơn | r 0.17 | main | tube |
| Mũi giáo | 2 cone 4 cạnh dẹt | đầu đuôi, theo hướng đuôi | r 0.22 | shade | 1.06 |

Vị trí khuôn mặt: mắt `(0.45, 0.1, 0.9)` yaw `0.32` scale `1`; miệng `(0, −0.33, 1.14)`, há `(0, −0.42, 1.1)`; má `(0.78, −0.18, 0.8)`; mõm: có (±0.21, nhô hơn mèo); mũi: không (lỗ mũi mực).

## Ánh xạ pose

| Tham số | Nơi nhận | Cách áp |
|---|---|---|
| sleepy | Đầu cúi, đuôi cuộn quanh chân, cánh gập về sau | như mèo; cánh quay về sau thêm 1.0 rad, nhỏ lại 20% |
| ear | Tai vây | `rotation.z = s·(0.25 − (1 − ear)·0.75)`: dựng ngang khi cảnh giác, cụp xuống khi ngủ |
| earTw | Tai vây giật | như tai mèo |
| tailUp | Cánh vỗ + đuôi dựng | vỗ `sin(14t)·0.35·tailUp`; đuôi nâng 0.35 |
| swish | Đuôi quất khi bị chọc lúc ngủ | như mèo |
| puff | Xoè cánh, tai vây và thân phồng | cánh +0.3 rad, +10%; tai +15%; thân +8% |
| tapping | Đập đuôi | mũi đuôi nâng `tapBeat(t)·0.35` rồi đập xuống |

Khói mũi: khi há miệng chửi (không phải ngáp), có chuyển động, đang thức.

## Hiệu ứng

Như mèo: Zzz `(2.1, 2.6, 0.3)`, dấu giận `(0.86, 0.5, 1.0)`, sao `(1.0, 1.2, 0.55)` (cao hơn để tránh sừng), cục u `(0.18, 0.9, 0.38)`, bóng r 1.45.

## Giọng

Chưa đưa vào app (câu thoại vẫn dùng chung). Gợi ý cho sau này:

- "HMPH! Claude's done. Don't make me breathe fire."
- "I hoard gold, not your unread output. MOVE."
- "Dậy! Không là tao nướng bàn phím đấy!"
- "Hừ! Xong rồi kìa, ra mà nhận!"
- SFW: "Snort. Your turn, human."

## Bài test Klat

| # | Mục | Đạt? | Ghi chú |
|---|---|---|---|
| 1 | Che thân → nhận ra mặt Klat | Đạt | Mắt hai đốm sáng, lông mày mực, ω, má hồng, mõm kem |
| 2 | Che mặt → không nhầm với mèo | Đạt | Cánh vươn ngang, sừng, tai vây, đuôi mũi giáo, màu ấm |
| 3 | Tỉ lệ | Đạt | Dáng ngồi của mèo, đầu rộng hơn thân, quay 3/4 |
| 4 | Viền | Đạt | Mọi khối đặc có viền; bụng, vạch giáp, mặt trong tai không |
| 5 | Toon | Đạt | 3 dải phẳng |
| 6 | Cau có | Đạt | Ảnh thức: mắt lim dim, lông mày chụm |
| 7 | Màu | Đạt | 6 vai trò, qua luật màu, rõ trên nền tối và sáng |
| 8 | Pose | Đạt | Mọi tham số có nơi nhận; ảnh ngủ thấy dáng cuộn, cánh gập |
| 9 | Dấu nhận diện | Đạt | Cánh, sừng, đuôi mũi giáo đều thấy |
