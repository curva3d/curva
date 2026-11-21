import numpy as np, sdf, cairosvg, io
from PIL import Image, ImageDraw, ImageFont
import pathlib, rhino3dm as _r3
RHINO=pathlib.Path(__file__).resolve().parent.parent/"rhino"
def _sample(name,k=40):
    out=[]
    for o in _r3.File3dm.Read(str(RHINO/name)).Objects:
        c=o.Geometry; d=c.Domain
        out.append(np.array([[c.PointAt(t).X,c.PointAt(t).Y,c.PointAt(t).Z] for t in np.linspace(d.T0,d.T1,k)]))
    return out
P=_sample("logo_simple_curve_v1.3dm",400)[0]
Q=np.c_[-P[:,1],P[:,2],-P[:,0]]; Q=Q-(Q.min(0)+Q.max(0))/2
Q=np.vstack([Q,Q[:1]])
L=np.r_[0,np.cumsum(np.linalg.norm(np.diff(Q,axis=0),axis=1))]
t=np.linspace(0,L[-1],600,endpoint=False); Q=np.c_[[np.interp(t,L,Q[:,k]) for k in range(3)]].T
fwd,right,up=sdf.rot(35,25)
XY=np.c_[Q@right,Q@up]; D=Q@fwd           # D larger = further away
mn,mx=XY.min(0),XY.max(0); XY=(XY-(mn+mx)/2)/(mx-mn).max()
def pts(seg,S,pad): 
    q=seg*S*(1-2*pad)+S/2; q[:,1]=S-q[:,1]; return q
def path(q,closed=False):
    s="M"+" L".join(f"{x:.2f},{y:.2f}" for x,y in q); return s+("Z" if closed else "")
def crossings(xy):
    n=len(xy); out=[]
    for i in range(n):
        a,b=xy[i],xy[(i+1)%n]
        for j in range(i+2,n):
            if (j+1)%n==i: continue
            c,d=xy[j],xy[(j+1)%n]
            den=(b[0]-a[0])*(d[1]-c[1])-(b[1]-a[1])*(d[0]-c[0])
            if abs(den)<1e-12: continue
            u=((c[0]-a[0])*(d[1]-c[1])-(c[1]-a[1])*(d[0]-c[0]))/den
            v=((c[0]-a[0])*(b[1]-a[1])-(c[1]-a[1])*(b[0]-a[0]))/den
            if 0<=u<=1 and 0<=v<=1: out.append((i,j))
    return out
X=crossings(XY); print("crossings",len(X))
def svg(w=0.07,gap=True,ink="#30302E",bg="#FFFFFF",tile=None,S=512,pad=0.12):
    q=pts(XY,S,pad); sw=w*S
    body=[]
    if tile: body.append(f'<rect width="{S}" height="{S}" rx="{S*52/256:.1f}" fill="{tile}"/>')
    body.append(f'<path d="{path(q,True)}" fill="none" stroke="{ink}" stroke-width="{sw:.1f}" stroke-linejoin="round"/>')
    if gap:
        n=len(q); k=int(0.06*n)
        for i,j in X:
            front=i if D[i]<D[j] else j
            idx=[(front+o)%n for o in range(-k,k+1)]
            body.append(f'<path d="{path(q[idx])}" fill="none" stroke="{tile or bg}" stroke-width="{sw*1.9:.1f}" stroke-linecap="butt"/>')
            body.append(f'<path d="{path(q[[(front+o)%n for o in range(-k-2,k+3)]])}" fill="none" stroke="{ink}" stroke-width="{sw:.1f}" stroke-linecap="butt"/>')
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" width="{S}" height="{S}">'+"".join(body)+"</svg>"
V=[("A1","Line, crossings joined",dict(w=0.07,gap=False)),
   ("A2","Line, over and under",dict(w=0.07)),
   ("A3","Bold, over and under",dict(w=0.11)),
   ("A4","Thin, over and under",dict(w=0.04)),
   ("A5","Bold, joined",dict(w=0.11,gap=False)),
   ("A6","Icon, light",dict(w=0.09,tile="#F4F3EE")),
   ("A7","Icon, dark",dict(w=0.09,ink="#F4F3EE",tile="#30302E")),
   ("A8","Small sizes",None)]
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cw=300; sheet=Image.new("RGB",(4*cw,2*340),"#ECEBE5"); dr=ImageDraw.Draw(sheet)
    for i,(k,n,kw) in enumerate(V):
        x,y=(i%4)*cw,(i//4)*340
        dr.text((x+15,y+10),f"{k} {n}",fill="#30302E",font=ft)
        if kw is None:
            tile=Image.new("RGB",(270,270),"white"); xx=10
            for sz in (96,64,32,16):
                s=svg(w=0.11); im=Image.open(io.BytesIO(cairosvg.svg2png(bytestring=s,output_width=sz,output_height=sz))).convert("RGBA")
                bg=Image.new("RGBA",im.size,"white"); bg.alpha_composite(im); tile.paste(bg.convert("RGB"),(xx,135-sz//2)); xx+=sz+14
            sheet.paste(tile,(x+15,y+35)); continue
        s=svg(**kw); open(f"out/flat2d/{k}.svg","w").write(s)
        im=Image.open(io.BytesIO(cairosvg.svg2png(bytestring=s,output_width=270,output_height=270))).convert("RGBA")
        bg=Image.new("RGBA",im.size,"white"); bg.alpha_composite(im); sheet.paste(bg.convert("RGB"),(x+15,y+35))
    sheet.save("out/flat2d/sheet.png")
