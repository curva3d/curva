"""Final logo set (21 Nov 2026): amber, black and white, tile-free floating icons with 3.5% grain."""
import os, pathlib
import gen_final as gfin, gen_duo as d, gen_graphite as gg
from grain import grain
from PIL import Image, ImageDraw, ImageFont
SET={"amber":dict(cl="#F6C35B",cs="#E0603F"),                 # colour also flows along the loop (flow 0.35)
     "black":dict(cl="#E8E8E6",cs="#060606",flow=0.0),        # lit by light only
     "white":dict(cl="#FFFFFF",cs="#9E9C95",flow=0.0)}
GRAIN=dict(amt=0.035,size=1,shade=0.4)
OUT=os.environ.get("LOGO_OUT",str(pathlib.Path(__file__).resolve().parent.parent/"renders"/"set"))
def marks(S=2048,out=1024):
    return {k:grain(d.render(az=gfin.AZ,el=gfin.EL,w_world=gfin.W,S=S,out=out,pad=0.1,**kw),**GRAIN) for k,kw in SET.items()}
if __name__=="__main__":
    os.makedirs(OUT,exist_ok=True)
    M=marks()
    for k,m in M.items():
        m.save(f"{OUT}/{k}-mark.png")
        for s in (1024,512,256,128,64,32,16): gg.bare(m,s).save(f"{OUT}/{k}-icon-{s}.png")
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",17)
    W=1540; sh=Image.new("RGB",(W,1700),"#ECEBE5"); dr=ImageDraw.Draw(sh)
    def put(k,x,y,bgc,size):
        bg=Image.new("RGBA",(size,size),bgc); ic=gg.bare(M[k],int(size*0.8)); bg.alpha_composite(ic,((size-ic.width)//2,(size-ic.height)//2)); sh.paste(bg.convert("RGB"),(x,y))
    for i,(k,lab,bgs) in enumerate((("amber","Amber · main icon",("#FFFFFF","#30302E")),("black","Black · for light backgrounds",("#FFFFFF","#F4F3EE")),("white","White · for dark backgrounds",("#30302E","#1E1E1C")))):
        x=20+i*500; dr.text((x,14),lab,fill="#30302E",font=ft); put(k,x,44,bgs[0],230); put(k,x+240,44,bgs[1],230)
    y=300; dr.text((20,y),"In the dock",fill="#30302E",font=ft); y+=30; x=20
    for k,light in (("amber",False),("amber",True),("black",True),("white",False)):
        s=gg.seg(gg.bare(M[k],96),light); s=s.resize((360,int(s.height*360/s.width)),Image.LANCZOS); sh.paste(s.convert("RGB"),(x,y)); x+=370
    y+=170; dr.text((20,y),"Lockups",fill="#30302E",font=ft); y+=30
    L=[gfin.lockup(M["amber"]),gfin.lockup(M["black"]),gfin.lockup(M["amber"],ink="#F4F3EE",bg="#30302E"),gfin.lockup(M["white"],ink="#F4F3EE",bg="#30302E")]
    for i,l in enumerate(L):
        l=l.convert("RGB"); l=l.resize((720,int(l.height*720/l.width)),Image.LANCZOS); sh.paste(l,(20+(i%2)*740,y+(i//2)*(l.height+20))); lh=l.height
    y+=2*(lh+20)+10; dr.text((20,y),"Sizes: 256 · 128 · 64 · 32 · 16",fill="#30302E",font=ft); y+=30; x=20
    for k,bgc in (("amber","#FFFFFF"),("black","#FFFFFF"),("white","#30302E")):
        strip=Image.new("RGB",(500,280),bgc); xx=10
        for s in (180,128,64,32,16):
            ic=gg.bare(M[k],s); strip.paste(ic,(xx,140-s//2),ic); xx+=s+14
        sh.paste(strip,(x,y)); x+=505
    sh.crop((0,0,W,y+300)).save(f"{OUT}/board.png")
