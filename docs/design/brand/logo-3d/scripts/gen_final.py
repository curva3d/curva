import numpy as np
import gen_fillet as gf, gen_duo as d
from PIL import Image, ImageDraw, ImageFont
gf.setQ(gf.fillet3(gf.C,25))
AZ,EL,W=-70,35,20
import pathlib; OUT=str(pathlib.Path(__file__).resolve().parent.parent/'renders')
PAL=[("Orange / sand",dict(cl="#FBD9A6",cs="#E0782F")),
     ("Amber / vermilion",dict(cl="#F6C35B",cs="#E0603F")),
     ("Yellow / pink",dict(cl="#F7EE6A",cs="#F48A9A")),
     ("Graphite / grey",dict(cl="#E4E2DA",cs="#4A4945")),
     ("Ink / graphite",dict(cl="#8E8C84",cs="#1E1E1C")),
     ("Violet / blue",dict(cl="#7FD3F5",cs="#8A6FF0")),
     ("Teal / lime",dict(cl="#D8F56A",cs="#2FB7A4")),
     ("Sand / white on dark",dict(cl="#FFFFFF",cs="#D9C3A5",tile="#30302E",pad=0.16))]
def mk(**kw): return d.render(az=AZ,el=EL,w_world=W,**kw)
def sheet(items,cols,name,cw=300,ch=340):
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    rows=(len(items)+cols-1)//cols
    sh=Image.new("RGB",(cols*cw,rows*ch),"#ECEBE5"); dr=ImageDraw.Draw(sh)
    for i,(lab,im) in enumerate(items):
        x,y=(i%cols)*cw,(i//cols)*ch
        sm=im.resize((270,270),Image.LANCZOS); bg=Image.new("RGBA",sm.size,"white"); bg.alpha_composite(sm)
        sh.paste(bg.convert("RGB"),(x+15,y+35)); dr.text((x+15,y+10),lab,fill="#30302E",font=ft)
    sh.save(name)
def tile(im,col,size=512,pad=0.16):
    out=Image.new("RGBA",(size,size),(0,0,0,0)); m=Image.new("L",(size,size),0)
    ImageDraw.Draw(m).rounded_rectangle([0,0,size-1,size-1],radius=size*52/256,fill=255)
    out.paste(Image.new("RGBA",(size,size),col),(0,0),m)
    s=int(size*(1-2*pad)); out.alpha_composite(im.resize((s,s),Image.LANCZOS),((size-s)//2,(size-s)//2)); return out
def mono(im,col):
    a=np.array(im)[...,3]; o=np.zeros((*a.shape,4),np.uint8); o[...,:3]=Image.new("RGB",(1,1),col).getpixel((0,0)); o[...,3]=a
    return Image.fromarray(o,"RGBA")
def crop(im):
    bb=im.getbbox(); return im.crop(bb)
def lockup(mark,stacked=False,ink="#30302E",bg="#FFFFFF",H=300):
    f=ImageFont.truetype("/usr/share/fonts/opentype/inter/Inter-SemiBold.otf",int(H*0.42))
    m=crop(mark); m=m.resize((int(m.width*H/m.height),H),Image.LANCZOS)
    def word(dr,x,y):
        tr=-0.03*f.size; cx=x
        for ch in "Curva":
            dr.text((cx,y),ch,font=f,fill=ink); cx+=f.getlength(ch)+tr
        return cx
    tw=sum(f.getlength(c) for c in "Curva")-0.03*f.size*4
    if not stacked:
        W_,H_=int(m.width+H*0.28+tw+H*0.6),int(H*1.5)
        img=Image.new("RGBA",(W_,H_),bg); img.alpha_composite(m,(int(H*0.3),int(H*0.25)))
        dr=ImageDraw.Draw(img); asc,desc=f.getmetrics()
        word(dr,int(H*0.3)+m.width+int(H*0.28),int(H_/2-(asc*0.72)/2-asc*0.28))
    else:
        W_=int(max(m.width,tw)+H*0.6); H_=int(H+f.size*1.4+H*0.6)
        img=Image.new("RGBA",(W_,H_),bg); img.alpha_composite(m,((W_-m.width)//2,int(H*0.3)))
        word(ImageDraw.Draw(img),int((W_-tw)/2),int(H*0.3+H+f.size*0.15))
    return img
if __name__=="__main__":
    import os; os.makedirs(OUT,exist_ok=True)
    imgs=[]
    for i,(n,kw) in enumerate(PAL):
        im=mk(**kw); im.save(fOUT+"/P{i+1}.png"); imgs.append((f"P{i+1} · {n}",im))
    sheet(imgs,4,OUT+"/colours.png")
    base=Image.open(OUT+"/P1.png")
    var=[("V1 · Light tile",tile(base,"#F4F3EE")),
         ("V2 · Dark tile",tile(base,"#30302E")),
         ("V3 · Orange tile",tile(mk(cl="#FFFFFF",cs="#F3C9A0"),"#E0782F")),
         ("V4 · Flat, ink",mono(base,"#30302E")),
         ("V5 · Flat, white on dark",tile(mono(base,"#FFFFFF"),"#30302E")),
         ("V6 · Graphite tile, sand",tile(Image.open(OUT+"/P8.png"),"#30302E",pad=0.0))]
    sheet(var,3,OUT+"/variants.png")
    lockup(base).save(OUT+"/lockup.png"); lockup(base,True).save(OUT+"/lockup-stacked.png")
    lockup(Image.open(OUT+"/P8.png"),ink="#F4F3EE",bg="#30302E").save(OUT+"/lockup-dark.png")
    sm=Image.new("RGB",(600,140),"white"); x=20
    for s in (128,64,32,16):
        t=tile(base,"#30302E").resize((s,s),Image.LANCZOS); sm.paste(t,(x,70-s//2),t); x+=s+30
    for s in (32,16):
        t=mono(base,"#30302E").resize((s,s),Image.LANCZOS); sm.paste(t,(x,70-s//2),t); x+=s+30
    sm.save(OUT+"/small.png")
