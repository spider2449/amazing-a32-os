#!/usr/bin/env python3
"""Synthetic tests: uncertainty closes the gate; no proprietary fixtures."""
import struct
import unittest
from a32_boot_critical import (REQUIRED, readiness_gate, pmic_evidence,
    regulator_graph, translate_edge, detect_fixups, memory_overlaps,
    signature_layout, final_block, byte_diff)
from a32_formats import der_item


def cell(n):
    return struct.pack('>I', n)


def tlv(tag, data):
    n = len(data)
    length = bytes([n]) if n < 128 else (b'\x81' + bytes([n]) if n < 256 else b'\x82' + n.to_bytes(2,'big'))
    return bytes([tag]) + length + data


def envelope(cert_length):
    return tlv(0x30, tlv(2,b'\x01') + tlv(0x30, b'a'*cert_length) +
        tlv(0x30,bytes.fromhex('06092a864886f70d01010b0500')) +
        tlv(0x30,tlv(0x13,b'/boot') + tlv(2,b'\x01\x00')) + tlv(4,b's'*256))


class EvidenceTests(unittest.TestCase):
    def test_pmic_not_identified_by_reference(self):
        r=pmic_evidence('mt6328 MT6353 mediatek,mt_pmic')
        self.assertEqual(r['candidates'],['MT6328','MT6353'])
        self.assertEqual(r['hardware_identity'],'UNKNOWN')

    def test_chip_ids(self):
        self.assertEqual(pmic_evidence('#define PMIC6328_E1_CID_CODE 0x2810')['expected_ids'],
                         {'PMIC6328_E1_CID_CODE':0x2810})

    def test_empty_pmic(self):
        self.assertEqual(pmic_evidence('nothing')['candidates'],[])

    def test_cid_register_and_mask(self):
        text='''#define MT6328_PMIC_REG_BASE (0x0000)
#define MT6328_SWCID ((MT6328_PMIC_REG_BASE+0x0202))
#define MT6328_PMIC_SWCID_MASK 0xFFFF
#define MT6328_PMIC_SWCID_SHIFT 0
'''
        self.assertEqual(pmic_evidence(text)['cid_registers'],
                         [{'symbol':'MT6328_SWCID','address':0x202,'mask':65535,'shift':0}])

    def test_cid_missing_base_not_inferred(self):
        text='#define MT6328_SWCID ((MT6328_PMIC_REG_BASE+0x0202))'
        self.assertIsNone(pmic_evidence(text)['cid_registers'][0]['address'])

    def graph(self):
        return {'/mmc':{'vmmc-supply':cell(1)}, '/switch':{'phandle':cell(1),'vin-supply':cell(2)},
                '/rail':{'phandle':cell(2)}}

    def test_indirect_supply(self):
        r=regulator_graph(self.graph(),'/mmc')
        self.assertEqual(r['edges'][1]['to'],'/rail')
        self.assertEqual(r['vqmmc'],'UNKNOWN')

    def test_supply_absent(self):
        self.assertEqual(regulator_graph({'/mmc':{}},'/mmc')['vmmc'],'UNKNOWN')

    def test_missing_handle(self):
        with self.assertRaises(ValueError): regulator_graph({'/mmc':{'vmmc-supply':cell(9)}},'/mmc')

    def test_cycle(self):
        n=self.graph(); n['/rail']['vin-supply']=cell(1)
        with self.assertRaises(ValueError): regulator_graph(n,'/mmc')

    def test_duplicate_handle(self):
        n=self.graph(); n['/rail']['phandle']=cell(1)
        with self.assertRaises(ValueError): regulator_graph(n,'/mmc')

    def test_conflicting_phandle_aliases(self):
        n=self.graph(); n['/rail']['linux,phandle']=cell(3)
        with self.assertRaises(ValueError): regulator_graph(n,'/mmc')

    def test_supply_multiple_handles(self):
        n=self.graph(); n['/mmc']['vmmc-supply']=cell(1)+cell(2)
        with self.assertRaises(ValueError): regulator_graph(n,'/mmc')

    def test_unproven_translation(self):
        self.assertIsNone(translate_edge(b'\x01',None,{0:'rising',1:'falling'})['value'])

    def test_proven_translation(self):
        semantics={0:'rising',1:'falling'}
        self.assertEqual(translate_edge(b'\x01',semantics,semantics)['value'],1)

    def test_u32_not_u8(self):
        with self.assertRaises(ValueError): translate_edge(cell(1))

    def test_bad_edge(self):
        with self.assertRaises(ValueError): translate_edge(b'\x02')

    def test_fixup_comments_strings(self):
        self.assertEqual(detect_fixups('/* fdt_setprop(x) */ "fdt_pack(x)"; // fdt_open_into(x)\n'),[])

    def test_fixup_leads(self):
        r=detect_fixups('\nfdt_setprop_string(x);\ncustom_fixup(x);')
        self.assertEqual([x['line'] for x in r],[2,3])
        self.assertTrue(all(x['status']=='REQUIRES_CALL_PATH_AUDIT' for x in r))

    def test_nested_memory(self):
        self.assertEqual(len(memory_overlaps([{'start':0,'size':100,'owner':'a'},
                       {'start':10,'size':10,'owner':'b'}])),1)

    def test_adjacent_memory(self):
        self.assertEqual(memory_overlaps([{'start':0,'size':10,'owner':'a'},
                       {'start':10,'size':10,'owner':'b'}]),[])

    def test_bad_memory(self):
        for start,size in [(-1,1),(0,0),(2**64-1,2)]:
            with self.assertRaises(ValueError): memory_overlaps([{'start':start,'size':size,'owner':'x'}])

    def test_variable_der_offsets(self):
        for n in (10,127,128,255,256,1000):
            d=envelope(n); r=signature_layout(d)
            c=next(x for x in r['sections'] if x['name']=='certificate')
            self.assertEqual(d[c['content_offset']:c['content_offset']+c['content_length']],b'a'*n)
            self.assertEqual(r['sections'][0]['length'],len(d))

    def test_truncated_der(self):
        with self.assertRaises(ValueError): signature_layout(envelope(256)[:-1])

    def test_negative_der_offset(self):
        with self.assertRaises(ValueError): der_item(b'\x02\x01\x01',-1,3)

    def test_final_size_not_rsa_claim(self):
        self.assertEqual(final_block(bytes(range(256)),{'x':b'hello'})['classification'],'UNKNOWN')

    def test_padding(self):
        self.assertTrue(final_block(bytes(256),{})['all_zero_or_ff'])

    def test_wrong_block_length(self):
        with self.assertRaises(ValueError): final_block(bytes(255),{})

    def test_byte_diff(self):
        self.assertEqual(byte_diff(b'abc',b'axc'),[{'offset':1,'a':98,'b':120,'xor':26}])

    def test_no_digest_match(self):
        r=final_block(bytes(256),{'x':b'hello'})
        self.assertFalse(r['candidate_comparisons']['x']['digest_equals_block'])

    def rows(self):
        return [{'component':n,'ready':'YES','evidence':'synthetic proof'} for n in REQUIRED]

    def timing(self):
        return {'ready':'PARTIAL','stock_conservative_proven':True,'evidence':'synthetic proof'}

    def test_gate_success(self):
        self.assertTrue(readiness_gate(self.rows(),self.timing()))

    def test_gate_missing(self):
        self.assertFalse(readiness_gate(self.rows()[:-1],self.timing()))

    def test_gate_partial(self):
        r=self.rows(); r[0]['ready']='PARTIAL'
        self.assertFalse(readiness_gate(r,self.timing()))

    def test_gate_no_evidence(self):
        r=self.rows(); r[0]['evidence']=''
        self.assertFalse(readiness_gate(r,self.timing()))

    def test_gate_duplicate(self):
        r=self.rows(); r.append(r[0])
        self.assertFalse(readiness_gate(r,self.timing()))

    def test_gate_timing_guess(self):
        t=self.timing(); t['stock_conservative_proven']=False
        self.assertFalse(readiness_gate(self.rows(),t))

    def test_gate_malformed(self):
        for rows,timing in [(None,{}),(['YES'],{}),({},{}),(self.rows(),None)]:
            self.assertFalse(readiness_gate(rows,timing))


if __name__=='__main__':
    unittest.main()
