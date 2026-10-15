#!/usr/bin/env python3
"""Offline numeric authoring-cost recomputation; standard library only."""
import argparse
import itertools
import json
import math
from pathlib import Path
import re
import statistics
import sys

SCHEMA='sley.authoring-cost-numeric-projection.v1'
NORMALIZATION='one_shared_positive_divisor_per_model_across_all_runs'
ROW_FIELDS={'run','model','set','language','task','rep','correct','first_try',
            'infrastructure_failure','attempt_count','tokens','cost_units'}
MAX_BYTES=16*1024*1024


def need(ok,message):
    if not ok:raise ValueError(message)


def parse(text):
    def pairs(items):
        result={}
        for key,value in items:
            need(key not in result,'duplicate JSON key');result[key]=value
        return result
    def constant(_):raise ValueError('nonfinite JSON constant')
    need(len(text.encode('utf-8'))<=MAX_BYTES,'JSON size bound')
    return json.loads(text,object_pairs_hook=pairs,parse_constant=constant)


def finite_number(value,minimum=0,maximum=sys.float_info.max):
    # Bound arbitrary-precision JSON integers before conversion by isfinite.
    return type(value) in (int,float) and minimum<=value<=maximum and math.isfinite(value)


def integer(value,minimum,maximum):
    return type(value) is int and minimum<=value<=maximum


def names(value,pattern,maximum):
    need(isinstance(value,list) and 0<len(value)<=maximum,'invalid name list')
    need(all(type(x) is str and re.fullmatch(pattern,x) for x in value),'invalid identifier')
    need(len(set(value))==len(value),'duplicate identifier')


def validate(data):
    need(isinstance(data,dict) and set(data)=={'schema','models','normalization','runs','rows'},'projection fields differ')
    need(data['schema']==SCHEMA and data['normalization']==NORMALIZATION,'projection contract differs')
    names(data['models'],r'model_[0-9]{2}',64)
    need(isinstance(data['runs'],list) and 0<len(data['runs'])<=16,'run bound')
    expected=set();run_names=set()
    for run in data['runs']:
        need(isinstance(run,dict) and set(run)=={'id','repetitions','sets','declared_trials'},'run fields differ')
        names([run['id']],r'[a-z][a-z0-9_]{0,31}',1)
        need(run['id'] not in run_names,'duplicate run');run_names.add(run['id'])
        need(integer(run['repetitions'],1,16) and integer(run['declared_trials'],1,100000),'run dimensions')
        need(isinstance(run['sets'],list) and 0<len(run['sets'])<=16,'set bound')
        set_names=set();run_count=0
        for group in run['sets']:
            need(isinstance(group,dict) and set(group)=={'id','tasks','languages'},'set fields differ')
            names([group['id']],r'[a-z][a-z0-9_]{0,31}',1)
            need(group['id'] not in set_names,'duplicate set');set_names.add(group['id'])
            names(group['tasks'],r'[a-z][a-z0-9_]{0,63}',1000)
            names(group['languages'],r'[a-z][a-z0-9_]{0,31}',32)
            count=len(data['models'])*len(group['tasks'])*len(group['languages'])*run['repetitions']
            run_count+=count
            need(len(expected)+count<=100000,'trial design too large')
            expected.update((run['id'],model,group['id'],language,task,rep)
              for model,language,task,rep in itertools.product(data['models'],group['languages'],group['tasks'],range(1,run['repetitions']+1)))
        need(run_count==run['declared_trials'],'declared trials differ from design')
    need(isinstance(data['rows'],list) and len(data['rows'])==len(expected),'missing or extra trials')
    seen=set();costs={model:[] for model in data['models']}
    for row in data['rows']:
        need(isinstance(row,dict) and set(row)==ROW_FIELDS,'row fields differ')
        need(all(type(row[k]) is str for k in ['run','model','set','language','task']),'row identity types')
        need(integer(row['rep'],1,16),'repetition invalid')
        key=tuple(row[k] for k in ['run','model','set','language','task','rep'])
        need(key in expected and key not in seen,'unknown or duplicate trial');seen.add(key)
        need(all(type(row[k]) is bool for k in ['correct','first_try','infrastructure_failure']),'flag must be boolean')
        need(integer(row['attempt_count'],1,3) and integer(row['tokens'],0,10**10),'attempt/token bound')
        cost=row['cost_units']
        need(finite_number(cost,0,1),'invalid normalized cost')
        need(row['first_try']==(row['correct'] and row['attempt_count']==1),'first-try flag inconsistent')
        need(not(row['correct'] and row['infrastructure_failure']),'correct infrastructure trial inconsistent')
        costs[row['model']].append(cost)
    need(seen==expected,'incomplete design')
    need(all(math.isclose(math.fsum(values),1.0,rel_tol=0,abs_tol=1e-9) for values in costs.values()),'cost normalization must total one per model across all runs')
    return data


def totals(rows,field):
    return {'trials':len(rows),'correct':sum(x['correct'] for x in rows),
            'first_try_correct':sum(x['first_try'] for x in rows),
            'tasks':sorted({x['task'] for x in rows}),'value_sum':math.fsum(x[field] for x in rows)}


def compare(left,right,models,split,arm,reference,field,paired):
    details=[];values=[]
    for model in models:
        a=[x for x in left if not x['infrastructure_failure'] and (x['model'],x['set'],x['language'])==(model,split,arm)]
        b=[x for x in right if not x['infrastructure_failure'] and (x['model'],x['set'],x['language'])==(model,split,reference)]
        common=sorted({x['task'] for x in a}&{x['task'] for x in b})
        if paired:a=[x for x in a if x['task'] in common];b=[x for x in b if x['task'] in common]
        ta,tb=totals(a,field),totals(b,field)
        reason=('no_common_tasks' if paired and not common else
                'zero_arm_correct' if not ta['correct'] else
                'zero_reference_correct' if not tb['correct'] else
                'zero_reference_'+field if not tb['value_sum'] else None)
        point=None
        if reason is None:
            arm_mean=ta['value_sum']/ta['correct']
            reference_mean=tb['value_sum']/tb['correct']
            if reference_mean==0:
                reason='reference_mean_underflow'
            elif arm_mean==0 and ta['value_sum']>0:
                reason='arm_mean_underflow'
            else:
                point=arm_mean/reference_mean
                if not math.isfinite(point):
                    point=None;reason='ratio_overflow'
                elif point==0 and arm_mean>0:
                    point=None;reason='ratio_underflow'
        if point is not None:values.append(point)
        details.append({'model':model,'arm':ta,'reference':tb,'common_tasks':common,'point':point,'undefined_reason':reason})
    return {'point':statistics.median(values) if values else None,'defined_models':len(values),
            'total_models':len(models),'method':'common_tasks' if paired else 'whole_retained_cells',
            'field':field,'models':details}


def calculate(data,common_task_baseline=False):
    validate(data)
    rows=data['rows'];models=data['models']
    need({x['id'] for x in data['runs']}=={'base210','final211','final2'},'headline calculation requires three registered runs')
    pooled=[x for x in rows if x['run'] in ('final211','final2')]
    final=[x for x in rows if x['run']=='final2'];base=[x for x in rows if x['run']=='base210']
    points={}
    for lang in ['c','cpp','rust','python','go']:
        points['development.'+lang]=compare(pooled,pooled,models,'dev','sley_pym',lang,'cost_units',True)
    for split in ['dev','val']:
        for field,label in [('cost_units','cost'),('tokens','tokens')]:
            points[f'baseline.{split}.{label}']=compare(final,base,models,split,'sley_pym','sley_f',field,common_task_baseline)
    for lang in ['python','go','rust']:
        points['held2.'+lang]=compare(final,final,models,'held2','sley_pym',lang,'cost_units',True)
    cells=[]
    for key in sorted({(x['run'],x['model'],x['set'],x['language']) for x in rows}):
        allrows=[x for x in rows if (x['run'],x['model'],x['set'],x['language'])==key]
        retained=[x for x in allrows if not x['infrastructure_failure']]
        cells.append(dict(zip(['run','model','set','language'],key),planned_trials=len(allrows),
          excluded_infrastructure_trials=len(allrows)-len(retained),retained_trials=len(retained),
          correct=sum(x['correct'] for x in retained),first_try_correct=sum(x['first_try'] for x in retained),
          cost_units=math.fsum(x['cost_units'] for x in retained),tokens=sum(x['tokens'] for x in retained)))
    held={}
    for arm in ['sley_pym','sley_py']:
        kept=[x for x in final if x['set']=='held2' and x['language']==arm and not x['infrastructure_failure']]
        held[arm]={'retained_trials':len(kept),'correct':sum(x['correct'] for x in kept),'first_try_correct':sum(x['first_try'] for x in kept)}
    return {'schema':'sley.authoring-cost-recomputation.v1','baseline_method':'common_tasks_alternative' if common_task_baseline else 'published_whole_retained_cells',
            'points':points,'cells':cells,'held2_correctness':held,
            'totals':{'trials':len(rows),'attempts':sum(x['attempt_count'] for x in rows),
                      'excluded_infrastructure_trials':sum(x['infrastructure_failure'] for x in rows)},
            'limits':'Normalized list-price arithmetic over recorded correctness flags; no price, model, invoice, oracle or runtime validation.'}


def verify(output,expected):
    need(output['baseline_method']=='published_whole_retained_cells','alternative method is not the published calculation')
    need(set(expected)=={'schema','points','totals','held2_correctness'} and expected['schema']=='sley.authoring-cost-expected.v1','expected contract differs')
    need(set(output['points'])==set(expected['points']),'expected point set differs')
    for name,value in expected['points'].items():
        actual=output['points'][name]
        need(actual['defined_models']==8 and actual['total_models']==8 and finite_number(value)
             and finite_number(actual['point'])
             and math.isclose(actual['point'],value,rel_tol=0,abs_tol=1e-12),'published point mismatch')
    need(output['totals']==expected['totals'] and output['held2_correctness']==expected['held2_correctness'],'published denominator mismatch')


def read_json(path):
    with path.open('rb') as stream:raw=stream.read(MAX_BYTES+1)
    need(len(raw)<=MAX_BYTES,'input size bound')
    return parse(raw.decode('utf-8'))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('projection',type=Path)
    parser.add_argument('--verify',type=Path,metavar='EXPECTED_JSON')
    parser.add_argument('--common-task-baseline',action='store_true')
    args=parser.parse_args()
    output=calculate(read_json(args.projection),args.common_task_baseline)
    if args.verify:verify(output,read_json(args.verify));output['published_values_verified']=True
    print(json.dumps(output,sort_keys=True,indent=2,allow_nan=False))

if __name__=='__main__':
    try:main()
    except (ValueError,OSError,UnicodeError,KeyError,TypeError):
        print('Offline recomputation refused: invalid, incomplete or mismatching inputs.',file=sys.stderr)
        raise SystemExit(1) from None
