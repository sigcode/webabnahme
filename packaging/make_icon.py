#!/usr/bin/env python3
"""Draws the app icon (blue rounded square with a white check mark) as a 1024px PNG.
Only the Python standard library is used. Usage: make_icon.py <out.png>"""
import math
import struct
import sys
import zlib

N = 1024
SS = 2  # supersampling per axis


def rounded_rect_inside(x, y, x0, y0, x1, y1, r):
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r and x0 <= x <= x1 and y0 <= y <= y1


def seg_dist(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    qx, qy = ax + t * dx, ay + t * dy
    return math.hypot(px - qx, py - qy)


def pixel(x, y):
    # macOS-style margin around the tile
    m, r = 100, 185
    if not rounded_rect_inside(x, y, m, m, N - m, N - m, r):
        return (0, 0, 0, 0)
    t = (y - m) / (N - 2 * m)
    top, bot = (84, 132, 255), (38, 86, 222)
    col = tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3))
    # check mark
    p1, p2, p3 = (330, 530), (460, 660), (700, 380)
    w = 62
    d = min(seg_dist(x, y, *p1, *p2), seg_dist(x, y, *p2, *p3))
    if d <= w:
        return (255, 255, 255, 255)
    return col + (255,)


def main(out):
    rows = []
    for y in range(N):
        row = bytearray([0])
        for x in range(N):
            acc = [0, 0, 0, 0]
            for sy in range(SS):
                for sx in range(SS):
                    p = pixel(x + (sx + 0.5) / SS, y + (sy + 0.5) / SS)
                    a = p[3]
                    acc[0] += p[0] * a
                    acc[1] += p[1] * a
                    acc[2] += p[2] * a
                    acc[3] += a
            n = SS * SS
            a = acc[3] / n
            if acc[3]:
                row += bytes([int(acc[0] / acc[3]), int(acc[1] / acc[3]), int(acc[2] / acc[3]), int(a)])
            else:
                row += bytes([0, 0, 0, 0])
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(out, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main(sys.argv[1])
