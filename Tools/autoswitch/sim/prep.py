#!/usr/bin/env python3
"""Turns mixed Vietnamese/English text into keystroke units for sim.cpp.

usage: prep.py corpus.txt [more.txt ...] > units.txt
Each output line: W<TAB>keys<TAB>isvn  |  S<TAB>separator chars  |  N (Enter)
Vietnamese words are typed in Telex the way people do: modifiers right after their
letter, the tone key at the end of the word (60%), right after its vowel (25%) or after
the vowel cluster (15%); ươ typed as uwow or uow.
"""
import re, sys, random, unicodedata
random.seed(42)
TONES = {'́': 's', '̀': 'f', '̉': 'r', '̃': 'x', '̣': 'j'}
VOWELS = set('aeiouy')
SEPS = set(" ,.;:?!'\"()-/0123456789")

def telex(word):
    letters = []          # list of (keys for this letter, is_vowel)
    tone, tone_at = '', -1
    for ch in word:
        d = unicodedata.normalize('NFD', ch)
        base, marks = d[0], d[1:]
        if base in 'đĐ':
            letters.append((base.replace('đ', 'd').replace('Đ', 'D') + 'd', False)); continue
        k = base
        low = base.lower()
        for m in marks:
            if m in TONES: tone, tone_at = TONES[m], len(letters)
            elif m == '̂': k += low          # â ê ô
            elif m in '̛̆': k += 'w'    # ă ơ ư
            else: return None
        letters.append((k, low in VOWELS))
    keys = [k for k, _ in letters]
    s = ''.join(keys)
    if 'uwow' in s.lower() and random.random() < 0.5:   # ươ as "uow"
        for i in range(len(keys) - 1):
            if keys[i].lower() == 'uw' and keys[i + 1].lower() == 'ow':
                keys[i] = keys[i][0]
    if tone:
        r = random.random()
        if r < 0.6 or tone_at < 0:
            keys.append(tone)
        elif r < 0.85:
            keys[tone_at] += tone
        else:
            last_vowel = max(i for i, (k, v) in enumerate(letters) if v)
            keys[last_vowel] += tone
    out = ''.join(keys)
    if len(word) > 1 and word.isupper(): out = out.upper()   # all caps: typed with Caps Lock, modifier keys too
    return out

VN_CHARS = re.compile(r'[^\x00-\x7f]')
TYPO_RATE = float(sys.argv[sys.argv.index('--typos') + 1]) if '--typos' in sys.argv else 0.0
NEIGHBORS = {c: n for row in ['qwertyuiop', 'asdfghjkl', 'zxcvbnm'] for i, c in enumerate(row)
             for n in [row[max(0, i - 1):i] + row[i + 1:i + 2]]}

def typo(keys):
    """One realistic slip: a neighbouring key instead of the right one, a dropped or a doubled letter."""
    i = random.randrange(len(keys))
    c = keys[i]
    if not c.isalpha(): return keys
    r = random.random()
    if r < 0.6:
        n = NEIGHBORS.get(c.lower(), '')
        if not n: return keys
        rep = random.choice(n)
        return keys[:i] + (rep.upper() if c.isupper() else rep) + keys[i + 1:]
    if r < 0.8 and len(keys) > 2:
        return keys[:i] + keys[i + 1:]
    return keys[:i] + c + keys[i:]
files = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith('--') and sys.argv[i - 1] != '--typos']
for path in files:
    for line in open(path, encoding='utf-8'):
        line = unicodedata.normalize('NFC', line.strip())
        if not line: continue
        for tok in re.findall(r'[^\W\d_]+|[^\w]|\d|_', line):
            if tok[0].isalpha():
                if VN_CHARS.search(tok):
                    k = telex(tok)
                    if k is None or not k.isascii(): print('S\t '); continue
                    if TYPO_RATE and random.random() < TYPO_RATE:
                        k = typo(k)      # the user meant this as Vietnamese; it should stay Vietnamese
                    print(f'W\t{k}\t1')
                else:
                    print(f'W\t{tok}\t0')
            else:
                print('S\t' + (tok if tok in SEPS else ' '))
        print('N')
