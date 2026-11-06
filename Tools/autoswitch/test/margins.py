import re, math
src=open('../build_dict.py').read().split("def load")[0]
ns={'__file__': '../build_dict.py'}; exec(src,ns); vn_key=ns['vn_key']; SYL=ns['SYL']
def load(n):
    rows=[(w,int(c)) for w,c in (l.split() for l in open('../'+n,encoding='utf-8'))]; return rows, sum(c for _,c in rows)
vi,vt=load('vi_50k.txt'); en,et=load('en_50k.txt')
sc=lambda c,t: round(10*math.log10(c*1e6/t))
VN={}
for w,c in vi:
    k=vn_key(w)
    if k and SYL.match(k) and len(w)<=7 and c>=5: VN[k]=VN.get(k,0)+c
VN={k:sc(c,vt) for k,c in VN.items()}
EN={w:sc(c,et) for w,c in en if re.fullmatch('[a-z]{2,}',w)}
# pairs where the same raw keys (key form, tone at end) are both: raw English word + VN syllable
pairs=[(k,EN[k],VN[k]) for k in VN if k in EN and k!=re.sub('[sfrxj]$','',k)]
pairs.sort(key=lambda p:p[1]-p[2])
for k,e,v in pairs: print(f"{k:10s} en={e:4d} vn={v:4d} diff(en-vn)={e-v:4d}")
