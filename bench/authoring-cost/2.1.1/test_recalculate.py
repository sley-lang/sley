import copy
import json
from pathlib import Path
import unittest
import recalculate as r


def fixture():
    rows=[]
    for language in ['sley_pym','python']:
        for task in ['a','b']:
            rows.append({'run':'sample','model':'model_01','set':'dev','language':language,'task':task,'rep':1,
                         'correct':True,'first_try':True,'infrastructure_failure':False,'attempt_count':1,
                         'tokens':10,'cost_units':0.25})
    return {'schema':r.SCHEMA,'models':['model_01'],'normalization':r.NORMALIZATION,
            'runs':[{'id':'sample','repetitions':1,'sets':[{'id':'dev','tasks':['a','b'],'languages':['sley_pym','python']}],
                     'declared_trials':4}],'rows':rows}

class Validation(unittest.TestCase):
    def test_complete_design(self):
        self.assertEqual(len(r.validate(fixture())['rows']),4)
    def test_missing_or_duplicate_trial(self):
        for mutate in [lambda x:x['rows'].pop(),lambda x:x['rows'].append(x['rows'][0])]:
            x=fixture();mutate(x)
            with self.assertRaises(ValueError):r.validate(x)
    def test_malformed_numbers_and_fields(self):
        for key,value in [('tokens',True),('tokens',-1),('tokens',1.5),('cost_units',float('nan')),
                          ('cost_units',float('inf')),('cost_units',-0.1),('correct',1),('rep',0),
                          ('attempt_count',4),('model','private/model')]:
            x=fixture();x['rows'][0][key]=value
            with self.assertRaises(ValueError,msg=key):r.validate(x)
        x=fixture();x['rows'][0]['prompt']='not allowed'
        with self.assertRaises(ValueError):r.validate(x)
    def test_inconsistent_flags(self):
        for flags in [{'correct':False,'first_try':True},{'correct':True,'infrastructure_failure':True},
                      {'attempt_count':2,'first_try':True}]:
            x=fixture();x['rows'][0].update(flags)
            with self.assertRaises(ValueError):r.validate(x)
    def test_normalization_is_across_runs(self):
        x=fixture();x['rows'][0]['cost_units']=0.5
        with self.assertRaises(ValueError):r.validate(x)
    def test_duplicate_json_key_and_nonfinite_constant(self):
        for raw in ['{"rows":[],"rows":[]}','{"value":NaN}']:
            with self.assertRaises(ValueError):r.parse(raw)

    def test_huge_integer_cost_is_rejected_without_overflow(self):
        x=fixture();x['rows'][0]['cost_units']=10**400
        with self.assertRaises(ValueError):r.validate(x)


class Methods(unittest.TestCase):
    def test_common_task_and_whole_cell_are_distinct(self):
        rows=fixture()['rows'];rows[0]['correct']=False;rows[0]['first_try']=False
        rows[1]['infrastructure_failure']=True;rows[1]['correct']=False;rows[1]['first_try']=False
        # Retain only task a for Sley; Python has both tasks.
        rows[0]['cost_units']=0.2;rows[0]['correct']=True
        rows[2]['cost_units']=0.1;rows[3]['cost_units']=0.7
        paired=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        whole=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',False)
        self.assertAlmostEqual(paired['point'],2)
        self.assertAlmostEqual(whole['point'],0.5)
        self.assertEqual(paired['models'][0]['reference']['trials'],1)
        self.assertEqual(whole['models'][0]['reference']['trials'],2)
    def test_zero_correct_and_zero_reference_cost_are_undefined(self):
        rows=fixture()['rows']
        for x in rows:
            if x['language']=='python':x['correct']=False;x['first_try']=False
        out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        self.assertIsNone(out['point']);self.assertEqual(out['defined_models'],0)
        self.assertEqual(out['models'][0]['undefined_reason'],'zero_reference_correct')
        rows=fixture()['rows']
        for x in rows:
            if x['language']=='python':x['cost_units']=0
        out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        self.assertEqual(out['models'][0]['undefined_reason'],'zero_reference_cost_units')
    def test_no_common_task_is_visible(self):
        rows=fixture()['rows']
        for x in rows:
            if x['language']=='python':x['task']='other'
        out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        self.assertIsNone(out['point']);self.assertEqual(out['models'][0]['undefined_reason'],'no_common_tasks')

    def test_subnormal_means_and_ratios_are_explicitly_undefined(self):
        cases=[(0.25,5e-324,'reference_mean_underflow'),
               (5e-324,0.25,'arm_mean_underflow')]
        for arm,reference,reason in cases:
            rows=fixture()['rows']
            for row in rows:row['cost_units']=0.0
            rows[0]['cost_units']=arm;rows[2]['cost_units']=reference
            out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
            self.assertIsNone(out['point']);self.assertEqual(out['defined_models'],0)
            self.assertEqual(out['models'][0]['undefined_reason'],reason)
        # Means are representable here, but dividing them would overflow.
        rows=[row for row in fixture()['rows'] if row['task']=='a']
        for row in rows:row['cost_units']=0.25 if row['language']=='sley_pym' else 5e-324
        out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        self.assertIsNone(out['point'])
        self.assertEqual(out['models'][0]['undefined_reason'],'ratio_overflow')
        # An extremely small representable mean divided by a larger value can underflow.
        rows[0]['cost_units']=5e-324;rows[1]['cost_units']=2.0
        out=r.compare(rows,rows,['model_01'],'dev','sley_pym','python','cost_units',True)
        self.assertIsNone(out['point'])
        self.assertEqual(out['models'][0]['undefined_reason'],'ratio_underflow')

    def test_huge_expected_integer_is_rejected_without_overflow(self):
        here=Path(__file__).parent
        data=r.validate(r.parse((here/'projection.json').read_text()))
        output=r.calculate(data);expected=r.parse((here/'expected.json').read_text())
        expected['points']['development.c']=10**400
        with self.assertRaises(ValueError):r.verify(output,expected)

    def test_packaged_projection_reproduces_published_points_and_alternative(self):
        here=Path(__file__).parent
        if not (here/'projection.json').exists():self.skipTest('projection not yet exported')
        data=r.validate(r.parse((here/'projection.json').read_text()))
        out=r.calculate(data)
        r.verify(out,r.parse((here/'expected.json').read_text()))
        self.assertEqual(len(out['points']),12)
        self.assertEqual(out['totals']['trials'],5440)
        self.assertEqual(out['totals']['excluded_infrastructure_trials'],199)
        alternative=r.calculate(data,common_task_baseline=True)
        self.assertAlmostEqual(alternative['points']['baseline.val.cost']['point'],0.4666146381646913)
        self.assertAlmostEqual(alternative['points']['baseline.val.tokens']['point'],0.3778654903775837)
        self.assertNotEqual(out['points']['baseline.val.cost']['point'],alternative['points']['baseline.val.cost']['point'])

if __name__=='__main__':unittest.main()
