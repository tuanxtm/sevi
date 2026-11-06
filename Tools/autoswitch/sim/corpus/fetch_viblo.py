import json, time, urllib.request, re, os, sys
def get(url):
    req = urllib.request.Request(url, headers={'Accept': 'application/json', 'User-Agent': 'Mozilla/5.0 (research; Sevi typing test)'})
    return json.load(urllib.request.urlopen(req, timeout=20))
slugs = []
for page in range(1, 16):
    d = get(f"https://viblo.asia/api/posts/newest?limit=20&page={page}")
    slugs += [p['slug'] for p in d['data']]
    time.sleep(0.8)
os.makedirs('viblo', exist_ok=True)
n = 0
for s in slugs:
    out = f'viblo/{s}.md'
    if os.path.exists(out): continue
    try:
        d = get(f"https://viblo.asia/api/p/{s}")
        open(out, 'w').write(d.get('title', '') + '\n\n' + (d.get('contents') or ''))
        n += 1
    except Exception as e:
        print('err', s, e)
    time.sleep(0.8)
print('fetched', n, 'of', len(slugs))
