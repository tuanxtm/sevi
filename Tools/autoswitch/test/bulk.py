import sys, subprocess, re
src=open('../build_dict.py').read().split("def load")[0]
ns={'__file__': '../build_dict.py'}; exec(src,ns); vn_key=ns['vn_key']; SYL=ns['SYL']
vi=[l.split() for l in open('../vi_50k.txt',encoding='utf-8')]
en=[l.split() for l in open('../en_50k.txt')]
vis=[]; seen=set()
for w,c in vi:
    k=vn_key(w)
    if k and SYL.match(k) and int(c)>=50 and k not in seen and k!=w:
        seen.add(k); vis.append((w,k,int(c)))
ens=[(w,int(c)) for w,c in en if re.fullmatch('[a-z]{2,}',w)][:20000]
def run(lines):
    out=subprocess.run(['./harness','lines'],input='\n'.join(lines)+'\n',capture_output=True,text=True).stdout.splitlines()
    return [o.split('\t') for o in out]
r=run([f"1\t{k} " for w,k,c in vis])
bad=[(vis[i][0],vis[i][1],vis[i][2],x[0]) for i,x in enumerate(r) if x[1]!='0']
tot=sum(c for _,_,c in vis); badc=sum(b[2] for b in bad)
print(f"A VN words in VN mode: {len(bad)}/{len(vis)} switched to EN, {100*badc/tot:.3f}% of VN usage")
print('  ', bad[:40])
r=run([f"0\t{k} " for w,k,c in vis])
ok=sum(1 for i,x in enumerate(r) if x[1]=='1')
okc=sum(vis[i][2] for i,x in enumerate(r) if x[1]=='1')
print(f"D VN words typed in EN mode converted: {ok}/{len(vis)} words, {100*okc/tot:.1f}% of VN usage")
miss=sorted([(vis[i][2],vis[i][0],vis[i][1]) for i,x in enumerate(r) if x[1]!='1'],reverse=True)[:25]
print('   top misses', miss)
r=run([f"0\t{w} " for w,c in ens])
bad=[(ens[i][0],x[0]) for i,x in enumerate(r) if x[1]!='0']
totc=sum(c for _,c in ens); badc=sum(ens[i][1] for i,x in enumerate(r) if x[1]!='0')
print(f"B EN words in EN mode: {len(bad)}/{len(ens)} wrongly converted, {100*badc/totc:.4f}% of EN usage")
print('  ', bad[:60])
r=run([f"1\t{w} " for w,c in ens])
changed=[i for i,x in enumerate(r) if x[0].strip()!=ens[i][0]]
chc=sum(ens[i][1] for i in changed)
print(f"C EN words typed in VN mode: {len(changed)}/{len(ens)} still wrong after space ({100*chc/totc:.2f}% of usage)")
print('   top', [(ens[i][0],r[i][0]) for i in changed[:40]])
