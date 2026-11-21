import gen_final as gfin, gen_duo as d, gen_graphite as gg
from PIL import Image, ImageDraw, ImageFont
T={"amber":dict(cl="#F6C35B",cs="#E0603F"),
   "white":dict(cl="#FFFFFF",mid="#E9E7E0",cs="#8E8C84",con=1,sheen=0.4),
   "black":dict(cl="#A3A19A",mid="#3A3936",cs="#0E0E0D",con=1,sheen=0.55)}
import pathlib; OUT=str(pathlib.Path(__file__).resolve().parent.parent/"renders"/"set")
import os; os.makedirs(OUT,exist_ok=True)
M={}
for k,kw in T.items():
    m=d.render(az=gfin.AZ,el=gfin.EL,w_world=gfin.W,S=2048,out=1024,**kw); m.save(f"{OUT}/{k}-mark.png"); M[k]=m
    gg.bare(m,1024).save(f"{OUT}/{k}-icon-1024.png")
    for s in (512,256,128,64,32,16): gg.bare(m,s).save(f"{OUT}/{k}-icon-{s}.png")
ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",17)
# board: heroes on light and dark, docks, lockups, sizes
W=1500; sh=Image.new("RGB",(W,1720),"#ECEBE5"); dr=ImageDraw.Draw(sh)
def put(im,x,y,bgc,size):
    bg=Image.new("RGBA",(size,size),bgc); ic=gg.bare(M[im],int(size*0.8)); bg.alpha_composite(ic,((size-ic.width)//2,(size-ic.height)//2)); sh.paste(bg.convert("RGB"),(x,y))
dr.text((20,14),"Amber · main icon",fill="#30302E",font=ft); dr.text((520,14),"Black · for light backgrounds",fill="#30302E",font=ft); dr.text((1020,14),"White · for dark backgrounds",fill="#30302E",font=ft)
put("amber",20,44,"#FFFFFF",230); put("amber",260,44,"#30302E",230)
put("black",520,44,"#FFFFFF",230); put("black",760,44,"#F4F3EE",230)
put("white",1020,44,"#30302E",230); put("white",1260,44,"#1E1E1C",230)
y=300; dr.text((20,y),"In the dock",fill="#30302E",font=ft); y+=30
x=20
for k,light in (("amber",False),("amber",True),("black",True),("white",False)):
    s=gg.seg(gg.bare(M[k],96),light); s=s.resize((360,int(s.height*360/s.width)),Image.LANCZOS); sh.paste(s.convert("RGB"),(x,y)); x+=370
y+=170; dr.text((20,y),"Lockups",fill="#30302E",font=ft); y+=30
L=[gfin.lockup(M["amber"]),gfin.lockup(M["black"]),gfin.lockup(M["amber"],ink="#F4F3EE",bg="#30302E"),gfin.lockup(M["white"],ink="#F4F3EE",bg="#30302E")]
for i,l in enumerate(L):
    l=l.convert("RGB"); l=l.resize((720,int(l.height*720/l.width)),Image.LANCZOS)
    sh.paste(l,(20+(i%2)*740,y+(i//2)*(l.height+20)))
    lh=l.height
y+=2*(lh+20)+10; dr.text((20,y),"Sizes: 256 · 128 · 64 · 32 · 16",fill="#30302E",font=ft); y+=30
x=20
for k,bgc in (("amber","#FFFFFF"),("black","#FFFFFF"),("white","#30302E")):
    strip=Image.new("RGB",(480,280),bgc); xx=10
    for s in (256,128,64,32,16):
        if s==256: ic=gg.bare(M[k],180); strip.paste(ic,(xx,50),ic); xx+=190; continue
        ic=gg.bare(M[k],s); strip.paste(ic,(xx,140-s//2),ic); xx+=s+14
    sh.paste(strip,(x,y)); x+=500
sh=sh.crop((0,0,W,y+300)); sh.save(f"{OUT}/board.png"); print(sh.size)
