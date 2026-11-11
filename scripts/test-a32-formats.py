#!/usr/bin/env python3
"""Synthetic format tests; no device or proprietary fixtures required."""
import importlib.util
from pathlib import Path
import struct
import unittest
import contextlib
import io
import tempfile
from unittest.mock import patch
from a32_formats import boot, fdt, scan_fdt, align, MAGIC, tail_evidence, der_item, boot_signature

spec = importlib.util.spec_from_file_location('extract', Path(__file__).with_name('extract-stock-dtb.py'))
extract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(extract)
spec = importlib.util.spec_from_file_location('compare', Path(__file__).with_name('compare-a32-dt.py'))
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)
spec = importlib.util.spec_from_file_location('tail', Path(__file__).with_name('analyze-boot-image-tail.py'))
tail_tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tail_tool)


def tree():
    structure = struct.pack('>I', 1) + bytes(4) + struct.pack('>II', 2, 9)
    return struct.pack('>10I', 0xd00dfeed, 72, 56, 72, 40, 17, 16, 0, 0, 16) + bytes(16) + structure


def image(tail=b'', dtbs=None):
    kernel = bytearray(128)
    struct.pack_into('<III', kernel, 36, 0x016f2818, 0, len(kernel))
    kernel += tree() if dtbs is None else dtbs
    ramdisk = b'ramdisk'
    header = bytearray(2048)
    header[:8] = b'ANDROID!'
    struct.pack_into('<I', header, 8, len(kernel))
    struct.pack_into('<I', header, 16, len(ramdisk))
    struct.pack_into('<I', header, 36, 2048)
    return bytes(header) + kernel + bytes(align(len(kernel), 2048) - len(kernel)) + ramdisk + bytes(2048 - len(ramdisk)) + tail


class Formats(unittest.TestCase):
    def test_valid_fdt(self):
        self.assertEqual(fdt(tree())['nodes'], {'/': {}})
        self.assertEqual(extract.extract(image())[0], tree())
        self.assertEqual(scan_fdt(b'arbitrary-prefix' + tree())[0][0]['offset'], 16)

    def test_invalid_fdt(self):
        with self.assertRaises(ValueError):
            fdt(bytes(72))

    def test_truncated_fdt(self):
        with self.assertRaises(ValueError):
            fdt(tree()[:-1])

    def test_multiple_fdt(self):
        self.assertEqual(len(scan_fdt(tree() * 2)[0]), 2)
        with self.assertRaises(ValueError):
            extract.extract(image(dtbs=tree() * 2))

    def test_false_magic(self):
        self.assertEqual(len(scan_fdt(MAGIC + bytes(40))[1]), 1)
        with self.assertRaises(ValueError):
            extract.extract(image(dtbs=tree() + MAGIC))

    def test_overlapping_blocks(self):
        data = bytearray(tree())
        struct.pack_into('>I', data, 16, 56)
        with self.assertRaises(ValueError):
            fdt(data)

    def test_bad_token(self):
        data = bytearray(tree())
        struct.pack_into('>I', data, 68, 99)
        with self.assertRaises(ValueError):
            fdt(data)

    def test_padded_sections(self):
        info = boot(image())
        self.assertEqual(info['sections']['kernel']['padded_end'], 4096)
        self.assertEqual(info['logical_end'], 6144)

    def test_nonzero_padding_rejected(self):
        data = bytearray(image())
        data[4095] = 1
        with self.assertRaises(ValueError):
            boot(data)

    def test_unknown_trailer(self):
        info = boot(image(b'unknown SIGN'))
        self.assertEqual(info['trailer_classification'], 'UNKNOWN')
        self.assertEqual(info['trailer_size'], 12)
        self.assertIn('5349474e', tail_evidence(b'unknown SIGN')['magic_hits_relative'])

    def test_no_trailer(self):
        self.assertEqual(boot(image())['trailer_size'], 0)

    def test_zero_trailer(self):
        self.assertIsNone(boot(image(bytes(50)))['nonzero_span'])

    def test_malformed_header(self):
        for data in (b'', b'ANDROID!', bytes(2048), image()[:-1]):
            with self.assertRaises(ValueError):
                boot(data)
        for offset, value in ((36, 0), (40, 1), (8, 0xffffffff)):
            data = bytearray(image())
            struct.pack_into('<I', data, offset, value)
            with self.assertRaises(ValueError):
                boot(data)

    def test_container_gap_rejected(self):
        with self.assertRaises(ValueError):
            extract.extract(image(dtbs=bytes(4) + tree()))

    def test_bad_der_lengths(self):
        for data in (b'', b'\x30\x80', b'\x30\x81\x01\0', b'\x30\x82\0\x80', b'\x30\x10\0'):
            with self.assertRaises(ValueError):
                der_item(data, 0, len(data))

    def test_unknown_der_schema(self):
        with self.assertRaises(ValueError):
            boot_signature(b'\x30\x03\x02\x01\x01')

    def test_der_signature_schema_and_truncation(self):
        def tlv(tag, value):
            n = len(value)
            size = bytes([n]) if n < 128 else b'\x82' + n.to_bytes(2, 'big')
            return bytes([tag]) + size + value
        attrs = tlv(0x30, tlv(0x13, b'/boot') + tlv(2, b'\x18\0'))
        raw = tlv(0x30, tlv(2, b'\1') + tlv(0x30, b'') +
                  tlv(0x30, bytes.fromhex('06092a864886f70d01010b')) + attrs + tlv(4, bytes(256)))
        parsed = boot_signature(raw + b'unknown')
        self.assertEqual(parsed['target'], '/boot')
        self.assertEqual(parsed['signed_length'], 6144)
        self.assertEqual(parsed['after_der_classification'], 'UNKNOWN')
        for end in (1, 3, len(raw) - 1):
            with self.assertRaises(ValueError):
                boot_signature(raw[:end])

    def test_resolved_reference_ignores_numeric_handle(self):
        def make(handle):
            return {'nodes': {'/': {'interrupt-parent': struct.pack('>I', handle)},
                              '/gic': {'phandle': struct.pack('>I', handle)}},
                    'handles': {handle: '/gic'}}
        self.assertEqual(compare.canonical(make(1)), compare.canonical(make(123)))

    def test_reference_specifier_cell_count(self):
        t = {'nodes': {'/clock': {'#clock-cells': struct.pack('>I', 1)}}, 'handles': {7: '/clock'}}
        refs = compare.inventory.references('clocks', struct.pack('>IIII', 7, 5, 7, 9), t)
        self.assertEqual(refs, ['/clock args=[5]', '/clock args=[9]'])
        self.assertIn('TRUNCATED', compare.inventory.references('clocks', struct.pack('>I', 7), t)[0])

    def test_symbolic_overlay_target(self):
        base = {'nodes': {'/': {}, '/cpu': {'status': b'disabled\0'},
                          '/__symbols__': {'cpus': b'/cpu\0'}}, 'handles': {}}
        overlay = {'nodes': {'/': {}, '/fragment@0': {'target': bytes.fromhex('ffffffff')},
                             '/fragment@0/__overlay__': {'status': b'okay\0'},
                             '/__fixups__': {'cpus': b'/fragment@0:target:0\0'}}, 'handles': {}}
        view, targets = compare.overlay_view(base, overlay)
        self.assertEqual(view['/cpu']['status'], '["okay"]')
        self.assertEqual(targets, {'/fragment@0': '/cpu'})
        del base['nodes']['/__symbols__']['cpus']
        with self.assertRaises(ValueError):
            compare.overlay_view(base, overlay)

    def test_mmc_path_aliases(self):
        self.assertEqual(compare.alias('/mtk-msdc.0/msdc0@11230000'), '/msdc@11230000')
        self.assertEqual(compare.alias('/bus/pinctrl@0x10211000/mmc0@default'),
                         '/bus/pinctrl@0x10211000/msdc0@default')

    def test_missing_dtc_fails_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output = root / 'fixture.bin', root / 'out'
            source.write_bytes(image())
            with patch('sys.argv', ['extract', str(source), '--out', str(output)]), \
                    patch.object(extract.shutil, 'which', return_value=None), \
                    contextlib.redirect_stderr(io.StringIO()) as error:
                self.assertEqual(extract.main(), 2)
            self.assertIn('dependency missing: dtc', error.getvalue())
            self.assertFalse(output.exists())

    def test_malformed_cli_input_creates_no_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output = root / 'fixture.bin', root / 'out'
            source.write_bytes(b'ANDROID! truncated')
            with patch('sys.argv', ['tail', str(source), '--out', str(output)]), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(tail_tool.main(), 2)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
