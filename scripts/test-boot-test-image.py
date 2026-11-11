#!/usr/bin/env python3
"""Offline format/preservation tests. Fixtures are temporary, never boot targets."""
import contextlib
import importlib.util
import io
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("boot", Path(__file__).with_name("boot-test-image.py"))
boot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boot)


def kernel(size):
    data = bytearray(size)
    struct.pack_into("<III", data, 36, 0x016F2818, 0, size)
    return bytes(data)


def fixture(second=b"second-stage"):
    header = bytearray(2048)
    header[:8] = b"ANDROID!"
    sections = [kernel(3000), b"original-ramdisk", second]
    for field, value in zip(range(8, 48, 4),
                           (3000, 0x40008000, len(sections[1]), 0x44000000,
                            len(second), 0x40F00000, 0x4E000000, 2048, 0, 123)):
        struct.pack_into("<I", header, field, value)
    header[48:52] = b"test"
    header[64:71] = b"cmdline"
    header[608:613] = b"extra"
    header[576:608] = boot.image_id(sections)
    result = bytes(header)
    for section in sections:
        result += section + bytes(boot.align(len(section), 2048) - len(section))
    return result + bytes(16384)


class RepackTests(unittest.TestCase):
    def run_prepare(self, original, replacement=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stock, new, output = (root / n for n in ("stock.img", "kernel", "test.img"))
            stock.write_bytes(original)
            new.write_bytes(replacement or kernel(5000))
            with contextlib.redirect_stdout(io.StringIO()):
                boot.prepare(stock, new, output)
            return output.read_bytes()

    def test_only_kernel_size_and_id_change_in_header(self):
        old = fixture()
        result = self.run_prepare(old)
        h, sections, info = boot.parse(result)
        before = boot.parse(old)
        self.assertEqual(sections[1:], before[1][1:])
        self.assertEqual(sections[0], kernel(5000))
        self.assertTrue(info["id_matches"])
        self.assertEqual(len(result), len(old))
        for i in range(len(h)):
            if i not in range(8, 12) and i not in range(576, 608):
                self.assertEqual(h[i], before[0][i])

    def test_empty_second_stage(self):
        self.assertEqual(boot.parse(self.run_prepare(fixture(b"")))[1][2], b"")

    def test_reject_signed_trailer(self):
        with self.assertRaisesRegex(ValueError, "signed/vendor"):
            self.run_prepare(fixture() + b"signature")

    def test_reject_versions_and_legacy_dt(self):
        for version in (1, 2, 3, 4, 60245):
            data = bytearray(fixture())
            struct.pack_into("<I", data, 40, version)
            with self.assertRaisesRegex(ValueError, "unsupported header"):
                self.run_prepare(data)

    def test_reject_corruption_truncation_padding_and_oversize(self):
        for data in (b"bad", fixture()[:2050], fixture()[:1632]):
            with self.assertRaises(ValueError):
                self.run_prepare(data)
        data = bytearray(fixture())
        data[2047] = 1
        with self.assertRaisesRegex(ValueError, "header extension"):
            self.run_prepare(data)
        data = bytearray(fixture())
        data[576] ^= 1
        with self.assertRaisesRegex(ValueError, "ID mismatch"):
            self.run_prepare(data)
        with self.assertRaisesRegex(ValueError, "exceeds stock"):
            self.run_prepare(fixture(), kernel(40000))

    def test_reject_wrong_kernel_and_dt_strategy(self):
        with self.assertRaisesRegex(ValueError, "ARM32"):
            self.run_prepare(fixture(), b"wrong architecture")
        dtb = bytearray(40)
        struct.pack_into(">II", dtb, 0, 0xD00DFEED, 40)
        with self.assertRaisesRegex(ValueError, "DTB strategy"):
            self.run_prepare(fixture(), kernel(3000) + dtb)


if __name__ == "__main__":
    unittest.main()
