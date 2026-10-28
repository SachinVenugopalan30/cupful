# SPDX-License-Identifier: AGPL-3.0-or-later
"""Rebuild data/staples.csv from data/tools/staples_spec.txt and USDA FoodData Central downloads.

Spec lines: name|fdc_id|is_prepared (0/1)|portion hint ("-" for none; else a word in the USDA portion text).
Download and unzip the Foundation, SR Legacy and Survey (FNDDS) JSON files from
https://fdc.nal.usda.gov/download-datasets, then:

    python3 data/tools/build_staples.py data/tools/staples_spec.txt data/staples.csv FOUNDATION.json SR_LEGACY.json SURVEY.json

Every number comes from USDA; nothing is typed by hand. Add aliases for new rows in data/aliases.csv.
"""
import csv, json, sys
spec, out, *paths = sys.argv[1:]
idx = {}
for p in paths:
    for key, foods in json.load(open(p)).items():
        for f in filter(None, foods):
            idx[f['fdcId']] = f

def nutrients(f):
    n = {}
    for fn in f.get('foodNutrients', []):
        nu = fn.get('nutrient', {})
        if 'amount' in fn:
            n[(nu.get('number'), nu.get('unitName', '').lower())] = fn['amount']
    kcal = n.get(('208', 'kcal'), n.get(('958', 'kcal'), n.get(('957', 'kcal'))))
    carbs = n.get(('205', 'g'), n.get(('205.2', 'g')))
    return kcal, n.get(('203', 'g')), carbs, n.get(('204', 'g'))

def portions(f):
    for p in f.get('foodPortions', []):
        if p.get('portionDescription'):
            text = p['portionDescription']
        else:
            unit = (p.get('measureUnit') or {}).get('name', '')
            unit = '' if unit == 'undetermined' else unit
            text = ' '.join(str(x) for x in [p.get('amount', ''), unit, p.get('modifier', '')] if x)
        yield text, p['gramWeight']

w = csv.writer(open(out, 'w', newline=''), lineterminator='\n')
w.writerow(['name', 'is_prepared', 'kcal', 'protein_g', 'carbs_g', 'fat_g', 'default_portion_g', 'portion_text', 'fdc_id', 'fdc_description'])
for line in open(spec):
    name, fdc, prep, hint = line.rstrip('\n').split('|')
    f = idx[int(fdc)]
    vals = nutrients(f)
    if None in vals:
        print('MISSING', name, fdc, vals, file=sys.stderr)
        continue
    ps = list(portions(f))
    pick = next(((t, g) for t, g in ps if hint and hint.lower() in t.lower()), None)
    if hint == "-":
        pick = ("", "")
    elif pick is None:
        pick = next(((t, g) for t, g in ps if 'Quantity not specified' not in t), (ps[0] if ps else ('', '')))
    print(f'{name:40} | {pick[0]} = {pick[1]}  || {[t for t, _ in ps][:6]}', file=sys.stderr)
    w.writerow([name, prep, *[round(float(v), 2) for v in vals], round(float(pick[1]), 1) if pick[1] != '' else '', pick[0], fdc, f['description']])
