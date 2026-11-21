import numpy as np, gen_tube2d as tb
from PIL import Image, ImageDraw, ImageFont
# reuse render() but replace shading with a two-colour, light-driven blend
def render(az,el,cl,cs,w_world=16,flow=0.35,S=1024,pad=0.1,tile=None,lightdir=(-0.6,-0.7),out=512):
    b=tb.b; g2=tb.g2
    XY,D,span=b.view(az,el); n=len(XY)
    rr=w_world/span/2; ext=(XY.max(0)-XY.min(0))+2*rr; c=(XY.max(0)+XY.min(0))/2
    k=S*(1-2*pad)/ext.max(); q=(XY-c)*k+S/2; q[:,1]=S-q[:,1]; r=rr*k; zk=k/span
    X=g2.crossings(XY); i0,j0=X[0]; bk=i0 if D[i0]>D[j0] else j0
    t=((np.arange(n)-bk)%n)/n
    zb=np.full((S,S),np.inf); U=np.zeros((S,S)); V=np.zeros((S,S)); I=np.full((S,S),-1,int)
    for i in range(n):
        x0,x1=int(max(0,q[i,0]-r-1)),int(min(S,q[i,0]+r+2)); y0,y1=int(max(0,q[i,1]-r-1)),int(min(S,q[i,1]+r+2))
        yy,xx=np.mgrid[y0:y1,x0:x1]; dx=xx+0.5-q[i,0]; dy=yy+0.5-q[i,1]; d2=dx*dx+dy*dy
        m=d2<r*r; z=D[i]*zk-np.sqrt(np.clip(r*r-d2,0,None))
        sub=zb[y0:y1,x0:x1]; upd=m&(z<sub); sub[upd]=z[upd]
        U[y0:y1,x0:x1][upd]=(dx/r)[upd]; V[y0:y1,x0:x1][upd]=(dy/r)[upd]; I[y0:y1,x0:x1][upd]=i
    hit=I>=0
    nz=np.sqrt(np.clip(1-U*U-V*V,0,1))
    L=np.array([lightdir[0],lightdir[1],0.75]); L/=np.linalg.norm(L)
    lam=np.clip(U*L[0]+V*L[1]+nz*L[2],0,1)
    lam=lam*lam*(3-2*lam)
    tt=t[I.clip(0)]; tt=0.5-0.5*np.cos(np.pi*tt)            # smooth run along the loop
    wgt=np.clip((1-flow)*lam+flow*tt,0,1)[...,None]
    A,B=tb.hx(cs),tb.hx(cl)
    col=A+(B-A)*wgt
    img=np.zeros((S,S,4)); img[...,:3]=col; img[...,3]=hit*255
    im=Image.fromarray(img.clip(0,255).astype(np.uint8),"RGBA").resize((out,out),Image.LANCZOS)
    if tile:
        bg=Image.new("RGBA",(out,out),(0,0,0,0)); mk=Image.new("L",(out,out),0)
        ImageDraw.Draw(mk).rounded_rectangle([0,0,out-1,out-1],radius=out*52/256,fill=255)
        bg.paste(Image.new("RGBA",(out,out),tile),(0,0),mk); bg.alpha_composite(im); im=bg
    return im
ISO=35.26
YP=dict(cl="#F7EE6A",cs="#F48A9A")     # the reference: yellow light side, pink shadow side
OS=dict(cl="#FBD9A6",cs="#E0782F")     # brand orange
OP=dict(cl="#F6C35B",cs="#E0603F")     # amber / vermilion
GR=dict(cl="#E4E2DA",cs="#4A4945")
V=[("D1","Yellow / pink · 16",dict(az=-45,el=ISO,w_world=16,**YP)),
   ("D2","Yellow / pink · 12",dict(az=-45,el=ISO,w_world=12,**YP)),
   ("D3","Yellow / pink · 9",dict(az=-45,el=ISO,w_world=9,**YP)),
   ("D4","Yellow / pink · 45°, 14",dict(az=-45,el=45,w_world=14,**YP)),
   ("D5","Orange / sand · 14",dict(az=-45,el=ISO,w_world=14,**OS)),
   ("D6","Amber / vermilion · 14",dict(az=-45,el=ISO,w_world=14,**OP)),
   ("D7","Grey · 14",dict(az=-45,el=ISO,w_world=14,**GR)),
   ("D8","Orange / sand · dark tile",dict(az=-45,el=ISO,w_world=14,tile="#30302E",pad=0.17,**OS))]
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cw=300; sheet=Image.new("RGB",(4*cw,2*340),"#ECEBE5"); dr=ImageDraw.Draw(sheet)
    for i,(kk,nm,kw) in enumerate(V):
        im=render(**kw); im.save(f"out/flat2d/{kk}.png")
        sm=im.resize((270,270),Image.LANCZOS); bg=Image.new("RGBA",sm.size,"white"); bg.alpha_composite(sm)
        x,y=(i%4)*cw,(i//4)*340
        sheet.paste(bg.convert("RGB"),(x+15,y+35)); dr.text((x+15,y+10),f"{kk} {nm}",fill="#30302E",font=ft)
        print(kk,flush=True)
    sheet.save("out/flat2d/duo.png")
