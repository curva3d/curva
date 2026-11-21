import numpy as np
import gen_base as b, gen_2d as g2
from PIL import Image, ImageDraw, ImageFont
def hx(h): return np.array([int(h[i:i+2],16) for i in (1,3,5)],float)
def render(az,el,c0,c1,w_world=24,S=1024,pad=0.08,tile=None,dark=0.35,light=0.4,lightdir=(-0.55,-0.8),phase=0.0,out=512,smooth=False,amb=0.55,spec=0.25):
    XY,D,span=b.view(az,el); n=len(XY)
    rr=w_world/span/2; ext=(XY.max(0)-XY.min(0))+2*rr; c=(XY.max(0)+XY.min(0))/2
    k=S*(1-2*pad)/ext.max(); q=(XY-c)*k+S/2; q[:,1]=S-q[:,1]; r=rr*k; zk=k/span
    T=np.roll(q,-3,0)-np.roll(q,3,0); T/=np.linalg.norm(T,axis=1)[:,None]
    N=np.c_[-T[:,1],T[:,0]]; L=np.array(lightdir)/np.linalg.norm(lightdir); N*=np.sign(N@L+1e-9)[:,None]
    X=g2.crossings(XY); i0,j0=X[0]; bk=i0 if D[i0]>D[j0] else j0
    t=((np.arange(n)-bk)%n)/n
    t=t*t*(3-2*t) if smooth else t
    A,B=hx(c0),hx(c1); base=A[None]+(B-A)[None]*t[:,None]
    zb=np.full((S,S),np.inf); U=np.zeros((S,S)); V2=np.zeros((S,S)); I=np.full((S,S),-1,int)
    for i in range(n):
        x0,x1=int(max(0,q[i,0]-r-1)),int(min(S,q[i,0]+r+2)); y0,y1=int(max(0,q[i,1]-r-1)),int(min(S,q[i,1]+r+2))
        yy,xx=np.mgrid[y0:y1,x0:x1]; dx=xx+0.5-q[i,0]; dy=yy+0.5-q[i,1]; d2=dx*dx+dy*dy
        m=d2<r*r
        z=D[i]*zk-np.sqrt(np.clip(r*r-d2,0,None))
        sub=zb[y0:y1,x0:x1]; upd=m&(z<sub)
        sub[upd]=z[upd]
        U[y0:y1,x0:x1][upd]=(dx/r)[upd]; V2[y0:y1,x0:x1][upd]=(dy/r)[upd]
        I[y0:y1,x0:x1][upd]=i
    hit=I>=0
    col=base[I.clip(0)]
    # smooth tube normal: offset from the centre plus the bulge toward the viewer
    nz=np.sqrt(np.clip(1-U*U-V2*V2,0,1))
    Lv=np.array([lightdir[0],lightdir[1],0.9]); Lv/=np.linalg.norm(Lv)
    lam=np.clip(U*Lv[0]+V2*Lv[1]+nz*Lv[2],0,1)
    shade=amb+(1-amb)*lam                      # 0.55 .. 1
    hl=spec*np.clip((lam-0.75)/0.25,0,1)**2
    col=col*(0.55+0.6*shade[...,None]); col=col+(255-col)*hl[...,None]
    img=np.zeros((S,S,4)); img[...,:3]=col; img[...,3]=hit*255
    im=Image.fromarray(img.clip(0,255).astype(np.uint8),"RGBA").resize((out,out),Image.LANCZOS)
    if tile:
        bg=Image.new("RGBA",(out,out),(0,0,0,0)); m=Image.new("L",(out,out),0)
        ImageDraw.Draw(m).rounded_rectangle([0,0,out-1,out-1],radius=out*52/256,fill=255)
        bg.paste(Image.new("RGBA",(out,out),tile),(0,0),m); bg.alpha_composite(im); im=bg
    return im
ISO=35.26
V=[("T1","Isometric · graphite / grey",dict(az=-45,el=ISO,c0="#30302E",c1="#B3B1AA")),
   ("T2","45° · graphite / grey",dict(az=-45,el=45,c0="#30302E",c1="#B3B1AA")),
   ("T3","25° · graphite / grey",dict(az=-45,el=25,c0="#30302E",c1="#B3B1AA")),
   ("T4","Isometric · orange / sand",dict(az=-45,el=ISO,c0="#E0782F",c1="#F7E3CF")),
   ("T5","Isometric · graphite / orange",dict(az=-45,el=ISO,c0="#30302E",c1="#E0782F")),
   ("T6","Isometric · pink / blue",dict(az=-45,el=ISO,c0="#F49AC1",c1="#6FB7F2")),
   ("T7","Dark tile · orange / sand",dict(az=-45,el=ISO,c0="#E0782F",c1="#F7E3CF",tile="#30302E",pad=0.15)),
   ("T8","Light tile · graphite / grey",dict(az=-45,el=ISO,c0="#30302E",c1="#B3B1AA",tile="#F4F3EE",pad=0.15))]
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cw=300; sheet=Image.new("RGB",(4*cw,2*340),"#ECEBE5"); dr=ImageDraw.Draw(sheet)
    for i,(kk,nm,kw) in enumerate(V):
        im=render(**kw); im.save(f"out/flat2d/{kk}.png")
        sm=im.resize((270,270),Image.LANCZOS); bg=Image.new("RGBA",sm.size,"white"); bg.alpha_composite(sm)
        x,y=(i%4)*cw,(i//4)*340
        sheet.paste(bg.convert("RGB"),(x+15,y+35)); dr.text((x+15,y+10),f"{kk} {nm}",fill="#30302E",font=ft)
        print(kk,flush=True)
    sheet.save("out/flat2d/tube2d.png")
