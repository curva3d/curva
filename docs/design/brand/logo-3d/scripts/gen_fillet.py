import numpy as np, rhino3dm as r3
import gen_base as b, gen_duo as d, gen_2d as g2
from PIL import Image, ImageDraw, ImageFont
m=r3.File3dm.Read(str(g2.RHINO/"logo_simple_curve_sharp.3dm"))
segs=[]
for o in m.Objects:
    g=o.Geometry; segs.append(np.array([[g.PointAtStart.X,g.PointAtStart.Y,g.PointAtStart.Z],[g.PointAtEnd.X,g.PointAtEnd.Y,g.PointAtEnd.Z]]))
# chain into corner list
loop=[segs.pop(0)]
while segs:
    e=loop[-1][-1]; k=min(range(len(segs)),key=lambda i:min(np.linalg.norm(segs[i][0]-e),np.linalg.norm(segs[i][1]-e)))
    s=segs.pop(k); loop.append(s if np.linalg.norm(s[0]-e)<np.linalg.norm(s[1]-e) else s[::-1])
C=np.array([s[0] for s in loop]); print(len(C),"corners"); print(C)
def fillet3(C,rad,steps=40):
    out=[]; n=len(C)
    for i in range(n):
        p,c,q=C[i-1],C[i],C[(i+1)%n]
        u=(p-c)/np.linalg.norm(p-c); v=(q-c)/np.linalg.norm(q-c)
        if rad<=0: out.append(c); continue
        ang=np.arccos(np.clip(u@v,-1,1)); rr=min(rad,0.5*min(np.linalg.norm(p-c),np.linalg.norm(q-c))*np.tan(ang/2)*0.999)
        tdist=rr/np.tan(ang/2); a=c+u*tdist; e=c+v*tdist
        bis=(u+v)/np.linalg.norm(u+v); ctr=c+bis*rr/np.sin(ang/2)
        x=(a-ctr)/rr; y=(e-ctr)-((e-ctr)@x)*x; y/=np.linalg.norm(y)
        th=np.arctan2((e-ctr)@y,(e-ctr)@x)
        for s in np.linspace(0,th,steps): out.append(ctr+rr*(np.cos(s)*x+np.sin(s)*y))
    return np.array(out)
def setQ(P):
    Q=np.c_[P[:,0],P[:,2],-P[:,1]]; Q=Q-Q.mean(0)
    Qc=np.vstack([Q,Q[:1]]); L=np.r_[0,np.cumsum(np.linalg.norm(np.diff(Qc,axis=0),axis=1))]
    t=np.linspace(0,L[-1],900,endpoint=False); b.Q=np.c_[[np.interp(t,L,Qc[:,k]) for k in range(3)]].T
R=[0,5,10,15,20,25]
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cw=300; sheet=Image.new("RGB",(6*cw,2*340),"#ECEBE5"); dr=ImageDraw.Draw(sheet)
    for i,rad in enumerate(R):
        setQ(fillet3(C,rad))
        for row,(nm,az) in enumerate((("R3",-60),("R4",-70))):
            im=d.render(az=az,el=35,w_world=20,**d.OS); im.save(f"out/flat2d/F{rad}-{nm}.png")
            sm=im.resize((270,270),Image.LANCZOS); bg=Image.new("RGBA",sm.size,"white"); bg.alpha_composite(sm)
            x,y=i*cw,row*340
            sheet.paste(bg.convert("RGB"),(x+15,y+35)); dr.text((x+15,y+10),f"{nm} · fillet {rad}" if rad else f"{nm} · sharp",fill="#30302E",font=ft)
        print(rad,flush=True)
    sheet.save("out/flat2d/fillets.png")
