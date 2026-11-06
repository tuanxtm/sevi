import subprocess, sys
rows=[l.rstrip('\n').split('\t') for l in open('cases.tsv',encoding='utf-8') if l.strip() and not l.startswith('#')]
# a case's expected text may contain a newline (Enter); rows were split by line, so rejoin those
fixed=[]; i=0
while i < len(rows):
    r=rows[i]
    if len(r)==3 and i+1<len(rows) and len(rows[i+1])==2 and rows[i+1][0]=='':
        fixed.append([r[0],r[1],r[2]+'\n',rows[i+1][1]]); i+=2; continue
    fixed.append(r); i+=1
inp='\n'.join(f"{r[0]}\t{r[1]}" for r in fixed)+'\n'
out=subprocess.run(['./harness','lines'],input=inp,capture_output=True,text=True).stdout
# output lines may contain newlines from Enter: parse by fields
parts=out.split('\t'); res=[]; text=parts[0]
for k in range(1,len(parts)):
    seg=parts[k]
    if k%2==1: sw=seg
    else:
        lang,_,rest=seg.partition('\n'); res.append((text,lang)); text=rest
fail=0
for r,(text,lang) in zip(fixed,res):
    ok = text==r[2] and lang==r[3]
    if not ok: fail+=1
    print(('PASS ' if ok else 'FAIL ')+repr(r[1])+' -> '+repr(text)+f' lang={lang}'+('' if ok else f'   expected {r[2]!r} lang={r[3]}'))
print(f"{len(fixed)-fail}/{len(fixed)} passed"); sys.exit(1 if fail else 0)
