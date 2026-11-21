import numpy as np, pickle, cairosvg, io, sdf
import gen_2d as g
from PIL import Image, ImageDraw, ImageFont
segs=g._sample("logo_simple_curve.3dm")
# chain segments end-to-end into one closed loop
loop=[segs.pop(0)]
while segs:
    e=loop[-1][-1]
    k=min(range(len(segs)),key=lambda i:min(np.linalg.norm(segs[i][0]-e),np.linalg.norm(segs[i][-1]-e)))
    s=segs.pop(k)
    if np.linalg.norm(s[-1]-e)<np.linalg.norm(s[0]-e): s=s[::-1]
    loop.append(s[1:])
P=np.vstack(loop); print("gap",np.linalg.norm(P[0]-P[-1]))
P=P[:-1] if np.linalg.norm(P[0]-P[-1])<1e-6 else P
# Rhino Z-up -> renderer y-up: x=X, y=Z, z=-Y
Q=np.c_[P[:,0],P[:,2],-P[:,1]]; Q=Q-Q.mean(0)
L=np.r_[0,np.cumsum(np.linalg.norm(np.diff(np.vstack([Q,Q[:1]]),axis=0),axis=1))]
t=np.linspace(0,L[-1],800,endpoint=False); Qc=np.vstack([Q,Q[:1]])
Q=np.c_[[np.interp(t,L,Qc[:,k]) for k in range(3)]].T
def view(az,el):
    fwd,right,up=sdf.rot(az,el)
    XY=np.c_[Q@right,Q@up]; D=Q@fwd
    span=(XY.max(0)-XY.min(0)).max()
    return (XY-(XY.min(0)+XY.max(0))/2)/span, D, span
def mark(az,el,w_world=10,gap=True,**kw):
    XY,D,span=view(az,el)
    g.XY,g.D=XY,D; g.X=g.crossings(XY)
    return g.svg(w=w_world/span,gap=gap,**kw)
V=[("B1","Left view: the C",(-90,0)),
   ("B2","Isometric",(-45,35.26)),
   ("B3","Axo, like L3",(-55,25)),
   ("B4","Axo, other side",(-35,25)),
   ("B5","Steeper",(-50,45)),
   ("B6","Flatter",(-60,15)),
   ("B7","From the right",(45,30)),
   ("B8","From behind",(135,30))]
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cw=300; sheet=Image.new("RGB",(4*cw,2*340),"#ECEBE5"); dr=ImageDraw.Draw(sheet)
    for i,(k,n,(az,el)) in enumerate(V):
        s=mark(az,el); open(f"out/flat2d/{k}.svg","w").write(s)
        im=Image.open(io.BytesIO(cairosvg.svg2png(bytestring=s,output_width=270,output_height=270))).convert("RGBA")
        bg=Image.new("RGBA",im.size,"white"); bg.alpha_composite(im)
        x,y=(i%4)*cw,(i//4)*340
        sheet.paste(bg.convert("RGB"),(x+15,y+35)); dr.text((x+15,y+10),f"{k} {n}",fill="#30302E",font=ft)
    sheet.save("out/flat2d/base.png")
