import numpy as np
from PIL import Image, ImageFilter
def grain(im, amt=0.12, size=1.0, shade=0.6, seed=7, mono=True):
    """Film-style grain inside the mark. amt: strength; size: grain size in px; shade: how much more grain sits in the shadows."""
    a=np.array(im.convert("RGBA")).astype(float); rgb=a[...,:3]; al=a[...,3:]/255
    rng=np.random.default_rng(seed); h,w=rgb.shape[:2]
    n=rng.normal(0,1,(int(h/size)+1,int(w/size)+1))
    n=np.array(Image.fromarray(((n*40)+128).clip(0,255).astype(np.uint8)).resize((w,h),Image.BILINEAR if size>1 else Image.NEAREST)).astype(float)
    n=(n-128)/40
    lum=(rgb@[0.299,0.587,0.114])/255
    k=amt*(1+shade*(1-lum))*255
    out=rgb+(n*k)[...,None] if mono else rgb+rng.normal(0,1,rgb.shape)*k[...,None]
    a[...,:3]=out.clip(0,255)
    return Image.fromarray(a.astype(np.uint8),"RGBA")
