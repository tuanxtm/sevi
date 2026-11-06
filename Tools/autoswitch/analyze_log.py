#!/usr/bin/env python3
"""Summarize an Sevi typing log (typing-log.jsonl) to find what to improve.

usage: analyze_log.py typing-log.jsonl [--context 3]

Prints counts, then every manual switch and undo with the words typed just before
and after it: each one is a case where auto switch did the wrong thing or nothing.
"""
import json, sys, collections

path = sys.argv[1]
ctx = int(sys.argv[sys.argv.index('--context') + 1]) if '--context' in sys.argv else 3
rows = [json.loads(l) for l in open(path, encoding='utf-8') if l.strip()]

c = collections.Counter(r['e'] if r['e'] != 'auto' else 'auto_' + r['rule'] for r in rows)
words = c['word']
print(f"words {words}  manual {c['manual']}  undo {c['undo']}  context {c['context']}")
print('auto:', {k: v for k, v in c.items() if k.startswith('auto_')})
if words:
    print(f"manual switches per 100 words: {100 * c['manual'] / words:.2f}")

per_app = collections.Counter(r.get('site') or r.get('app') for r in rows if r['e'] in ('manual', 'undo'))
print('manual/undo by app or site:', per_app.most_common(10))

print('\n--- manual switches and undos in context ---')
for i, r in enumerate(rows):
    if r['e'] not in ('manual', 'undo'):
        continue
    before = [x for x in rows[max(0, i - 15):i] if x['e'] in ('word', 'auto')][-ctx:]
    after = [x for x in rows[i + 1:i + 15] if x['e'] == 'word'][:ctx]
    fmt = lambda x: (f"{x['keys']}({x['lang']})" if x['e'] == 'word' else f"<auto {x['rule']}>")
    print(f"{r['t'][5:19]} {r.get('site') or r.get('app')}: {' '.join(map(fmt, before))}  ==[{r['e']} -> {r['to']}]==>  {' '.join(map(fmt, after))}")
