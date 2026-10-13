"""Draws src-tauri/icons/tray-template.png: the macOS menu bar icon.

A template image is black on transparent; macOS tints it for light and dark menu bars. This draws the cat's
head (two ears and a round face, sleepy eyes cut out) with 8x8 supersampling for smooth edges, and writes the
PNG with the standard library only. Run: python scripts/make-tray-template.py
"""
import math
import pathlib
import struct
import zlib

SIZE = 44  # 22pt at 2x
SS = 8  # supersamples per axis


def inside_triangle(x: float, y: float, a, b, c) -> bool:
    def side(p, q):
        return (x - q[0]) * (p[1] - q[1]) - (p[0] - q[0]) * (y - q[1])

    d1, d2, d3 = side(a, b), side(b, c), side(c, a)
    neg = d1 < 0 or d2 < 0 or d3 < 0
    pos = d1 > 0 or d2 > 0 or d3 > 0
    return not (neg and pos)


def ink(x: float, y: float) -> bool:
    """True where the silhouette is solid. Coordinates are 0..44."""
    face = ((x - 22) / 17) ** 2 + ((y - 26) / 14.5) ** 2 <= 1
    left_ear = inside_triangle(x, y, (6.5, 21), (8.5, 3.5), (20, 13))
    right_ear = inside_triangle(x, y, (37.5, 21), (35.5, 3.5), (24, 13))
    solid = face or left_ear or right_ear
    if not solid:
        return False
    # Sleepy eyes: two downward arcs cut out of the face, the cat's closed-eye look.
    for cx in (15.5, 28.5):
        r = math.hypot(x - cx, y - 24.5)
        if 3.0 <= r <= 4.6 and y >= 24.5:
            return False
    # A small nose notch.
    if inside_triangle(x, y, (20.4, 30.2), (23.6, 30.2), (22, 32.2)):
        return False
    return True


def main() -> None:
    rows = []
    for py in range(SIZE):
        row = bytearray([0])  # filter: none
        for px in range(SIZE):
            hits = sum(
                ink(px + (sx + 0.5) / SS, py + (sy + 0.5) / SS) for sy in range(SS) for sx in range(SS)
            )
            alpha = round(255 * hits / (SS * SS))
            row += bytes((0, 0, 0, alpha))
        rows.append(bytes(row))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
    png += chunk(b"IEND", b"")
    out = pathlib.Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "tray-template.png"
    out.write_bytes(png)
    print(f"wrote {out} ({len(png)} bytes)")


if __name__ == "__main__":
    main()
