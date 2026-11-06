import re, glob, unicodedata
VN = re.compile(r'[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]', re.I)
out = []
for f in sorted(glob.glob('viblo/*.md')):
    s = open(f, encoding='utf-8').read()
    s = re.sub(r'```.*?```', ' ', s, flags=re.S)        # code blocks
    s = re.sub(r'`[^`]*`', ' ', s)                       # inline code
    s = re.sub(r'!\[[^\]]*\]\([^)]*\)', ' ', s)          # images
    s = re.sub(r'\[([^\]]*)\]\([^)]*\)', r'\1', s)       # links -> text
    s = re.sub(r'https?://\S+', ' ', s)
    s = re.sub(r'<[^>]+>', ' ', s)
    for line in s.split('\n'):
        line = line.strip()
        if not line or line.startswith('|') or line.startswith('>'): continue
        line = re.sub(r'^[#*\-+\d.)\s]+', '', line)
        line = re.sub(r'[*_~]', '', line)
        line = unicodedata.normalize('NFC', line)
        if not VN.search(line): continue
        # keep only characters we can type in the simulator
        line = re.sub(r"[^\w\s,.;:?!'()/-]", ' ', line)
        line = re.sub(r'\s+', ' ', line).strip()
        for sent in re.split(r'(?<=[.?!])\s+', line):
            if 4 <= len(sent.split()) <= 60: out.append(sent)
open('viblo_sentences.txt', 'w', encoding='utf-8').write('\n'.join(out))
words = sum(len(x.split()) for x in out)
print(len(out), 'sentences', words, 'words')
