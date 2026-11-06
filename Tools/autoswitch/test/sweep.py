import subprocess, re, sys, itertools
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
E='../../../Engine'
def build(p,vf,ef):
    subprocess.run(f'clang++ -std=c++17 -O2 -w -DAS_PREFER={p} -DAS_VN_FLOOR={vf} -DAS_EN_FLOOR={ef} -I{E} -I{E}/platforms -o h_sweep harness.cpp {E}/Engine.cpp {E}/Vietnamese.cpp {E}/Macro.cpp {E}/SmartSwitchKey.cpp {E}/ConvertTool.cpp',shell=True,check=True)
def run(lines):
    out=subprocess.run(['./h_sweep','lines'],input='\n'.join(lines)+'\n',capture_output=True,text=True).stdout.splitlines()
    return [o.split('\t') for o in out]
vt=sum(c for *_,c in vis); et=sum(c for _,c in ens)
def metrics(show=False):
    rA=run([f"1\t{k} " for w,k,c in vis]); A=[vis[i] for i,x in enumerate(rA) if x[1]!='0']
    rD=run([f"0\t{k} " for w,k,c in vis]); D=[vis[i] for i,x in enumerate(rD) if x[1]!='1']
    rB=run([f"0\t{w} " for w,c in ens]); B=[(ens[i],x[0]) for i,x in enumerate(rB) if x[1]!='0']
    rC=run([f"1\t{w} " for w,c in ens]); C=[ens[i] for i,x in enumerate(rC) if x[0].strip()!=ens[i][0]]
    m=dict(A=100*sum(v[2] for v in A)/vt, B=100*sum(b[0][1] for b in B)/et, C=100*sum(c[1] for c in C)/et, D=100*sum(d[2] for d in D)/vt)
    if show:
        print(' A wrong VN->EN:', [(v[0],v[1]) for v in A][:30])
        print(' B wrong EN->VN:', [(b[0][0],b[1].strip()) for b in B][:40])
    return m, len(A), len(B)
for p,vf,ef in [(10,0,0),(10,100,100),(10,15,5),(10,10,5),(10,10,0),(10,20,5),(15,15,5),(10,15,10)]:
    build(p,vf,ef); m,a,b=metrics()
    print(f"prefer={p} vnFloor={vf} enFloor={ef}: wrongVN->EN {a} ({m['A']:.3f}%)  wrongEN->VN {b} ({m['B']:.4f}%)  | missed EN-in-VN {m['C']:.2f}%  missed VN-in-EN {m['D']:.2f}%")
