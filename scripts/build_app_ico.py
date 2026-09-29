#!/usr/bin/env python3
"""
Task Cleaner - Multi-Resolution Windows .ico Builder
Creates a production-grade 7-layer .ico file conforming to Microsoft Windows 11 icon specs.
Layers: 256x256 (PNG), 128x128, 64x64, 48x48, 32x32, 24x24, 16x16 (32-bit BGRA DIB).
"""

import os
import sys
import struct
import shutil
from PIL import Image

def build_windows_ico(layer_map, output_path):
    """
    Assembles multiple resolution PNGs into a standard Windows .ico container.
    256x256 is compressed as standard PNG.
    <= 128x128 are uncompressed 32-bit BGRA DIBs for maximum Win32 GDI & Shell compatibility.
    """
    sorted_sizes = sorted(layer_map.keys(), reverse=True)
    count = len(sorted_sizes)
    header = struct.pack('<HHH', 0, 1, count)
    
    entries = []
    data_blobs = []
    offset = 6 + count * 16
    
    for s in sorted_sizes:
        png_path = layer_map[s]
        im = Image.open(png_path).convert('RGBA')
        w, h = im.size
        assert w == s and h == s, f"Dimension mismatch: expected {s}x{s}, got {w}x{h}"
        
        if s == 256:
            with open(png_path, 'rb') as f:
                blob = f.read()
            bWidth = 0
            bHeight = 0
        else:
            bWidth = s
            bHeight = s
            
            # XOR mask: BGRA bottom-up
            xor_bytes = bytearray()
            for y in reversed(range(h)):
                for x in range(w):
                    r, g, b, a = im.getpixel((x, y))
                    xor_bytes.extend([b, g, r, a])
            
            # AND mask: 1-bpp row padded to 4 bytes
            row_bytes = (w + 31) // 32 * 4
            and_bytes = b'\x00' * (row_bytes * h)
            
            bih = struct.pack(
                '<IIIHHIIIIII',
                40,              # biSize
                w,               # biWidth
                h * 2,           # biHeight (doubled for XOR + AND in ICO)
                1,               # biPlanes
                32,              # biBitCount
                0,               # biCompression (BI_RGB)
                len(xor_bytes),  # biSizeImage
                0, 0, 0, 0       # resolution & palette colors
            )
            blob = bih + xor_bytes + and_bytes
            
        entry = struct.pack(
            '<BBBBHHII',
            bWidth,
            bHeight,
            0,             # bColorCount
            0,             # bReserved
            1,             # wPlanes
            32,            # wBitCount
            len(blob),     # dwBytesInRes
            offset         # dwImageOffset
        )
        entries.append(entry)
        data_blobs.append(blob)
        offset += len(blob)
        
    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, 'wb') as f:
        f.write(header)
        for e in entries:
            f.write(e)
        for d in data_blobs:
            f.write(d)
            
    file_size_kb = os.path.getsize(output_path) / 1024.0
    print(f"[SUCCESS] Generated: {output_path} ({file_size_kb:.1f} KB, {count} layers)")

def main():
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    raw_dir = os.path.join(repo_root, "assets", "raw_icons")
    
    # Run swift generator if raw icons don't exist
    swift_script = os.path.join(repo_root, "scripts", "generate_win_app_icon.swift")
    if not os.path.exists(os.path.join(raw_dir, "icon_256.png")):
        print("[INFO] Rendering raw icon layers via Swift...")
        ret = os.system(f"swift '{swift_script}' '{raw_dir}'")
        if ret != 0:
            print("[ERROR] Failed to run swift script.")
            sys.exit(1)
            
    ico_sizes = [256, 128, 64, 48, 32, 24, 16]
    layer_map = {}
    for s in ico_sizes:
        p = os.path.join(raw_dir, f"icon_{s}.png")
        if not os.path.exists(p):
            print(f"[ERROR] Missing layer: {p}")
            sys.exit(1)
        layer_map[s] = p
        
    # Destinations
    dest_ico = os.path.join(repo_root, "assets", "app.ico")
    build_windows_ico(layer_map, dest_ico)
    
    # Copy to root and crates/task-cleaner-gui
    root_ico = os.path.join(repo_root, "app.ico")
    gui_ico = os.path.join(repo_root, "crates", "task-cleaner-gui", "app.ico")
    shutil.copyfile(dest_ico, root_ico)
    shutil.copyfile(dest_ico, gui_ico)
    print(f"[INFO] Copied to: {root_ico}")
    print(f"[INFO] Copied to: {gui_ico}")
    
    # Copy high-res PNGs to assets
    for s in [1024, 512, 256]:
        src = os.path.join(raw_dir, f"icon_{s}.png")
        dst = os.path.join(repo_root, "assets", f"app_icon_{s}.png")
        if os.path.exists(src):
            shutil.copyfile(src, dst)
            print(f"[INFO] Exported high-res PNG: {dst}")

if __name__ == "__main__":
    main()
