import gen_final as gfin, gen_duo as d
from PIL import Image, ImageDraw, ImageFilter, ImageFont
G=[("G1 · Graphite / grey (current)",dict(cl="#E4E2DA",cs="#4A4945")),
   ("G2 · Deep graphite / warm grey",dict(cl="#B3B1AA",cs="#30302E")),
   ("G3 · Graphite / white, high contrast",dict(cl="#FFFFFF",cs="#2A2A28")),
   ("G4 · Soft silver",dict(cl="#F7F6F2",cs="#8E8C84")),
   ("G5 · Cool steel",dict(cl="#D5DADF",cs="#353A40")),
   ("G6 · Graphite / sand",dict(cl="#E8D5B8",cs="#3A3936")),
   ("G7 · Graphite / orange glow",dict(cl="#F2A86A",cs="#30302E")),
   ("G8 · Ink / graphite",dict(cl="#8E8C84",cs="#1E1E1C")),
   ("A · Amber / vermilion (for reference)",dict(cl="#F6C35B",cs="#E0603F"))]
def crop(im): return im.crop(im.getbbox())
def bare(m0,size):
    m=crop(m0); s=size/max(m.size)*0.94; m=m.resize((int(m.width*s),int(m.height*s)),Image.LANCZOS)
    out=Image.new("RGBA",(size,size),(0,0,0,0)); ox,oy=(size-m.width)//2,(size-m.height)//2
    a=m.split()[3].point(lambda v:int(v*0.5)); sh=Image.new("RGBA",m.size,(0,0,0,0)); sh.putalpha(a)
    pad=Image.new("RGBA",(size,size),(0,0,0,0)); pad.alpha_composite(sh,(ox,oy+int(size*0.05)))
    out.alpha_composite(pad.filter(ImageFilter.GaussianBlur(size*0.035))); out.alpha_composite(m,(ox,oy)); return out
def seg(icon,light):
    S=96; W=3*S+4*20
    bg=Image.new("RGBA",(W+20,S+56),"#C9CCD1" if light else "#5E6166"); dr=ImageDraw.Draw(bg)
    dr.rounded_rectangle([10,10,W+10,S+46],radius=22,fill=(245,245,245,255) if light else (40,40,42,255))
    x=30
    for k in range(3):
        if k==1: bg.alpha_composite(icon,(x,22)); dr.ellipse([x+S//2-3,S+30,x+S//2+3,S+36],fill="#30302E" if light else "#DDDDDD")
        else: dr.rounded_rectangle([x+6,28,x+S-6,22+S-6],radius=20,fill="#7C7F84" if light else "#6B6E73")
        x+=S+20
    return bg
if __name__=="__main__":
    ft=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    cells=[]
    for i,(n,kw) in enumerate(G):
        m=d.render(az=gfin.AZ,el=gfin.EL,w_world=gfin.W,**kw); m.save(f"out/final/G{i+1}.png")
        ic=bare(m,96); big=bare(m,1024); big.save(f"out/final/G{i+1}-notile-1024.png")
        cells.append((n,seg(ic,False),seg(ic,True)))
        print(n,flush=True)
    cw=cells[0][1].width; ch=cells[0][1].height
    sh=Image.new("RGB",(3*(2*cw+30)+20,len(cells)//3*(ch+40)+((len(cells)%3)>0)*(ch+40)),"#ECEBE5"); dr=ImageDraw.Draw(sh)
    for i,(n,a,b) in enumerate(cells):
        x=(i%3)*(2*cw+30)+10; y=(i//3)*(ch+40)
        dr.text((x,y+8),n,fill="#30302E",font=ft); sh.paste(a.convert("RGB"),(x,y+32)); sh.paste(b.convert("RGB"),(x+cw+6,y+32))
    sh.save("out/final/graphite.png"); print(sh.size)
