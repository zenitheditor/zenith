"""Write the CJK fallback code-point list, one U+XXXX per line."""
import sys


def dbcs(codec, rows, cells=range(0xA1, 0xFF)):
    out = set()
    for r in rows:
        for c in cells:
            try:
                ch = bytes([r, c]).decode(codec)
            except UnicodeDecodeError:
                continue
            if len(ch) == 1:
                out.add(ord(ch))
    return out


def big5_common():
    out = set()
    for hi in range(0xA4, 0xC7):
        for lo in list(range(0x40, 0x7F)) + list(range(0xA1, 0xFF)):
            if hi == 0xC6 and lo > 0x7E:
                break
            try:
                ch = bytes([hi, lo]).decode("big5")
            except UnicodeDecodeError:
                continue
            if len(ch) == 1:
                out.add(ord(ch))
    return out


gb2312 = dbcs("gb2312", range(0xA1, 0xF8))
jis_l1 = dbcs("euc_jp", range(0xB0, 0xD0)) | dbcs("euc_jp", range(0xA1, 0xA9))
hangul = set(range(0xAC00, 0xD7A4)) | set(range(0x3130, 0x3190))
punct = set(range(0x3000, 0x3100)) | set(range(0xFF00, 0xFFF0))
cps = gb2312 | jis_l1 | big5_common() | hangul | punct
sys.stdout.write("".join(f"U+{c:04X}\n" for c in sorted(cps)))
