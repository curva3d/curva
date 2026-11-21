import numpy as np, os, subprocess, sdf
import gen_final as gfin, gen_duo as d, gen_base as b, gen_2d as g2
from grain import grain
from PIL import Image
import pathlib; OUT=str(pathlib.Path(__file__).resolve().parent.parent/"renders"/"animation"/"frames"); os.makedirs(OUT,exist_ok=True)
Q=b.Q; K=2*np.linalg.norm(Q-Q.mean(0),axis=1).max()
XY,D,_=b.view(-70,35); X=g2.crossings(XY); i0,j0=X[0]; SEAM=i0 if D[i0]>D[j0] else j0
def fview(az,el):
    fwd,right,up=sdf.rot(az,el); P=Q-Q.mean(0)
    return np.c_[P@right,P@up]/K, P@fwd, K
b.view=fview
SET={"amber":dict(cl="#F6C35B",cs="#E0603F",loop=True)}
N=96
for k,kw in SET.items():
    os.makedirs(f"{OUT}/{k}",exist_ok=True)
    for i in range(N):
        az=-70+360*i/N
        im=d.render(az=az,el=30,w_world=20,S=800,out=400,pad=0.06,seam=SEAM,fixed=True,**kw)
        grain(im,amt=0.035,size=1,shade=0.4,seed=i).save(f"{OUT}/{k}/{i:03d}.png")
    print(k,flush=True)
