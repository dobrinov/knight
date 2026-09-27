// humanoid rig. expects X. returns API
const {P,rng,Img,shadeBlob,line,outline,groundY}=X;
const R={
  steel:[P.g1,P.g2,P.g3,P.g5], chain:[P.g1,P.g2,P.g3], dsteel:[P.DS,P.g1,P.g2], gold:[P.y1,P.y2,P.y3], leather:[P.b1,P.b2,P.b3], lleather:[P.b2,P.b3,P.b4],
  blue:[P.w1,P.w2,P.w3], cream:[P.b4,P.b5,P.g5], white:[P.g3,P.g4,P.g5], team:[P.m1,P.m2,P.m3,P.m4], skin:[P.s1,P.s2,P.s3], orc:[P.o1,P.o2,P.o3],
  elf:[P.s2,P.s3,P.g5], troll:[P.t1,P.t2,P.w4], bone:[P.g3,P.g4,P.g5], green:[P.n1,P.n2,P.n3], lgreen:[P.n2,P.n3,P.n4], violet:[P.v1,P.v2,P.v3], wood:[P.b2,P.b3,P.b4],
  dark:[P.DS,P.g1,P.g2], rust:[P.r1,P.b2,P.s1], red:[P.r1,P.r2,P.r3], zombie:[P.o1,P.g2,P.g3], fish:[P.t1,P.t2,P.n4], hair_blond:[P.y1,P.y2,P.y3], hair_brown:[P.b1,P.b2,P.b3], hair_black:[P.DS,P.g1,P.g2], hair_violet:[P.v1,P.v2,P.v3], teal:[P.t1,P.t2,P.t3], fur:[P.b2,P.b3,P.b4], grey:[P.g1,P.g2,P.g3],
};
function box(img,x,y,w,h,ramp,round=false){ for(let j=0;j<h;j++) for(let i=0;i<w;i++){ if(round&&j===0&&(i===0||i===w-1)) continue; const c=(j===0||i===0)?ramp[2]:((i===w-1||j===h-1)?ramp[0]:ramp[1]); img.set(x+i,y+j,c);} }
function blobClip(img,cx,cy,rx,ry,ramp,clip){ const tmp=new Img(img.w,img.h); shadeBlob(tmp,cx,cy,rx,ry,ramp); for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++){ const c=tmp.d[y*img.w+x]; if(c && (!clip||clip(x,y))) img.set(x,y,c);} }
// draw a strip along direction ang from point (hx,hy): t in [a,b], half-width w(t); shade by side
function strip(img,hx,hy,ang,a,b,w,ramp,edge){ const ux=Math.cos(ang),uy=Math.sin(ang),vx=-uy,vy=ux; const lightSide=(vx+vy)<0?1:-1; for(let t=a;t<=b;t+=0.5){ const ww=typeof w==='function'?w(t):w; for(let s=-ww;s<=ww+0.01;s+=0.5){ const side=s*lightSide; const c= ww<0.6?ramp[1]:(side>ww*0.3?ramp[2]:(side<-ww*0.3?ramp[0]:ramp[1])); img.set(hx+ux*t+vx*s,hy+uy*t+vy*s,(edge&&Math.abs(s)>=ww-0.25&&s*lightSide<0)?edge:c);} } }
function pt(hx,hy,ang,t,s=0){ const ux=Math.cos(ang),uy=Math.sin(ang); return [hx+ux*t-uy*s, hy+uy*t+ux*s]; }
// weapons: drawn with the hand at (hx,hy) pointing ang
const W={
  sword(img,hx,hy,a,o={}){ const L=o.len||10; strip(img,hx,hy,a,-1.5,1,0.5,R.leather); strip(img,hx,hy,a,2,L,o.w||1,o.blade||R.steel,P.g5); strip(img,...pt(hx,hy,a,1.5),a+1.5708,-2.5,2.5,0.5,o.guard||R.gold); },
  dagger(img,hx,hy,a){ strip(img,hx,hy,a,-1,0.5,0.5,R.leather); strip(img,hx,hy,a,1,5,0.6,R.steel,P.g5); },
  axe(img,hx,hy,a,o={}){ const L=o.len||12, hw=o.head||4; strip(img,hx,hy,a,-3,L,0.6,o.shaft||R.wood); for(let t=L-5;t<=L;t+=0.5){ const k=1-Math.abs(t-(L-2.5))/3.2; for(let s=0.5;s<=hw*Math.max(0.35,k);s+=0.5){ const [x,y]=pt(hx,hy,a,t,-s); img.set(x,y, s>hw*Math.max(0.35,k)-0.9?P.g5:(t<L-3?(o.headR||R.steel)[2]:(o.headR||R.steel)[1])); } if(o.double) for(let s=0.5;s<=hw*Math.max(0.35,k);s+=0.5){ const [x,y]=pt(hx,hy,a,t,s); img.set(x,y, s>hw*Math.max(0.35,k)-0.9?P.g4:(o.headR||R.steel)[0]); } } },
  hammer(img,hx,hy,a,o={}){ const L=o.len||11; strip(img,hx,hy,a,-3,L,0.6,R.wood); strip(img,...pt(hx,hy,a,L),a+1.5708,-3.5,3.5,1.6,o.head||R.steel); if(o.glow){ const [x,y]=pt(hx,hy,a,L+2.5,-2); img.set(x,y,P.y3); const [x2,y2]=pt(hx,hy,a,L-2.5,3); img.set(x2,y2,P.g5); const [x3,y3]=pt(hx,hy,a,L+2.5,3); img.set(x3,y3,P.y3);} },
  club(img,hx,hy,a,o={}){ const L=o.len||11; strip(img,hx,hy,a,-2,L,t=>0.6+Math.max(0,t)*0.14*(o.fat||1),R.wood); if(o.spikes) for(let t=5;t<=L;t+=3){ for(const sd of [-1,1]){ const [x,y]=pt(hx,hy,a,t,sd*(1.6+t*0.15)); img.set(x,y,P.g4);} } },
  spear(img,hx,hy,a,o={}){ const L=o.len||16; strip(img,hx,hy,a,-7,L,0.5,o.shaft||R.wood); strip(img,hx,hy,a,L,L+4,t=>1.4*(1-(t-L)/4.5),R.steel,P.g5); },
  staff(img,hx,hy,a,o={}){ const L=o.len||14; strip(img,hx,hy,a,-8,L,0.5,o.shaft||R.wood); const [tx,ty]=pt(hx,hy,a,L+2.5); const top=o.top||'crystal';
    if(top==='crystal'){ const [fx,fy]=pt(hx,hy,a,L+4.5); for(let k=-2;k<=2;k++){ const w=2-Math.abs(k); for(let i=-w;i<=w;i++) img.set(fx+i,fy+k,i<0?P.t3:(i===0?P.t2:P.t1)); } if(o.glow){ img.set(fx-3,fy-2,P.t3); img.set(fx+3,fy+1,P.t3);} }
    if(top==='sun'){ shadeBlob(img,tx,ty,2,2,R.gold.concat([P.y3])); img.set(tx,ty-3,P.y3); img.set(tx-3,ty,P.y3); img.set(tx+3,ty,P.y2); img.set(tx,ty+3,P.y2); }
    if(top==='skull'){ shadeBlob(img,tx,ty,2.5,2.2,R.bone); img.set(Math.round(tx)+1,Math.round(ty),P.DS); img.set(Math.round(tx)-1,Math.round(ty),P.DS); }
    if(top==='gem'){ shadeBlob(img,tx,ty,1.8,1.8,[P.w2,P.w3,P.w4,P.w5]); img.set(tx-2,ty+2,P.g5); img.set(tx+2,ty+2,P.r3); img.set(tx-2,ty+3,P.g5); }
    if(top==='bone'){ strip(img,hx,hy,a,L,L+3,1.2,R.bone); img.set(...pt(hx,hy,a,L+1,-2),P.r3); img.set(...pt(hx,hy,a,L+1,2),P.g5); }
    if(top==='living'){ const [fx,fy]=pt(hx,hy,a,L+2); line(img,fx,fy,fx-2,fy-3,P.b3); line(img,fx,fy,fx+2,fy-3,P.b3); img.set(fx-2,fy-4,P.n4); img.set(fx+2,fy-4,P.n4); img.set(fx,fy-3,P.t3); img.set(fx,fy-4,P.t3); } },
  pick(img,hx,hy,a){ strip(img,hx,hy,a,-2,10,0.6,R.wood); strip(img,...pt(hx,hy,a,10),a+1.5708,-4,4,t=>0.8-Math.abs(t)*0.12,R.steel,P.g5); },
  musket(img,hx,hy,a){ strip(img,hx,hy,a,-6,-1,1.2,R.wood); strip(img,hx,hy,a,-1,13,0.6,R.dsteel); },
  glaive(img,hx,hy,a){ strip(img,hx,hy,a,-8,8,0.5,R.bone); for(const k of [-1,0,1]){ const aa=a+k*0.7; for(let t=0;t<5;t++){ const [x,y]=pt(...pt(hx,hy,a,8),aa,t+1,Math.sin(t/5*3.14)*1.5*(k||1)); img.set(x,y,t<2?P.g4:P.g5); img.set(x+(k>0?0:1),y+1,P.g3);} } for(const k of [-1,1]){ for(let t=0;t<4;t++){ const [x,y]=pt(...pt(hx,hy,a,-8),a+3.14+k*0.6,t,0); img.set(x,y,P.g4);} } },
  blade(img,hx,hy,a){ strip(img,hx,hy,a,-3,1,0.5,R.leather); for(let t=2;t<=15;t+=0.5){ const bend=(t-2)*(t-2)*0.018; for(let s=-1;s<=0.8;s+=0.5){ const [x,y]=pt(hx,hy,a,t,s-bend); img.set(x,y,s<-0.6?P.g5:(s>0.4?P.g2:P.g4)); } } strip(img,...pt(hx,hy,a,1.5),a+1.5708,-2,2,0.5,R.gold); },
  crescent(img,hx,hy,a){ for(let k=-6;k<=6;k+=0.5){ const [x,y]=pt(hx,hy,a,3-Math.abs(k)*Math.abs(k)*0.09,k); img.set(x,y,k<0?P.g5:P.g3); const [x2,y2]=pt(hx,hy,a,2.2-Math.abs(k)*Math.abs(k)*0.09,k); img.set(x2,y2,P.g4); } },
  bow(img,hx,hy,a,o={}){ const draw=o.draw||0, H=o.h||7; const ramp=o.ramp||R.wood; for(let s=-H;s<=H;s+=0.5){ const q=s/H; const [x,y]=pt(hx,hy,a,2.5*(1-q*q)+(o.recurve?Math.abs(q)>0.8?-1:0:0),s); img.set(x,y,Math.abs(s)<1.2?R.leather[1]:(s<0?ramp[2]:ramp[1])); }
    const [ax,ay]=pt(hx,hy,a,0.6,-H), [bx,by]=pt(hx,hy,a,0.6,H), [mx,my]=pt(hx,hy,a,-draw,0); line(img,ax,ay,mx,my,o.broken?P.g2:P.g4); line(img,mx,my,bx,by,P.g4); if(o.arrow){ strip(img,mx,my,a,0,draw+5,0.4,R.wood); img.set(...pt(mx,my,a,draw+5.5),P.g5); img.set(...pt(mx,my,a,-0.5),P.g5);} },
};
// kite / round shield centred at (x,y)
function shield(img,x,y,kind,S=1){ if(kind==='kite'){ const h=Math.round(11*S), w=Math.round(7*S); for(let j=0;j<h;j++){ const hw=j<h*0.5?w/2:Math.max(0.5,(w/2)*(1-(j-h*0.5)/(h*0.55))); for(let i=-hw;i<=hw;i+=0.5){ const edge=Math.abs(i)>hw-1||j===0; const c=edge?(i<0||j===0?P.g4:P.g2):(i<-hw*0.3||j<2?P.m4:(i>hw*0.4?P.m2:P.m3)); img.set(x+i,y+j,c);} } img.set(x,y+3,P.y3); img.set(x,y+4,P.y2); img.set(x-1,y+4,P.y3); img.set(x+1,y+4,P.y2); img.set(x,y+5,P.y2); img.set(x,y+6,P.y1); }
  if(kind==='round'||kind==='broken'){ const r=4.5*S; shadeBlob(img,x,y+r,r,r,[P.b1,P.b2,P.b3,null]); for(let a=0;a<6.28;a+=0.2) img.set(x+Math.cos(a)*r*0.85,y+r+Math.sin(a)*r*0.85,a>2.2&&a<5.5?P.g3:P.g1); img.set(x,y+r,P.g3); if(kind==='broken'){ for(let j=0;j<r;j++) for(let i=0;i<r;i++) if(i+j<r*0.9) img.set(x+r-i,y+r*2-j,null); line(img,x,y+r,x+3,y+r*2-2,P.b1);} }
  if(kind==='elf'){ for(let j=0;j<10;j++){ const hw=3.5*Math.sin((j+1)/11*3.14); for(let i=-hw;i<=hw;i+=0.5) img.set(x+i,y+j,Math.abs(i)>hw-1?P.g4:(i<0?P.m4:P.m3)); } }
}
// ---- humanoid ----
function humanoid(c,p,Wc,Hc){ const img=new Img(Wc,Hc); const S=c.scale||1; const cx=Math.floor(Wc/2)+(p.dx||0); const gy=Hc-1-(c.hover||0);
  const lw=c.limbW||3; const legH=Math.round(7*S*(c.legLen||1)); const tH=Math.round(8*S*(c.torsoLen||1)); const tw=Math.round(10*S*(c.bulk||1)); const hr=4.6*S*(c.headScale||1);
  const bob=p.bob||0, lean=p.lean||0; const hipY=gy-legH+1+(p.crouch||0); const tTop=hipY-tH+bob; const tL=cx-Math.floor(tw/2)+lean; const hx=cx-0.5+lean*1.5+(c.hunch||0)+(c.headDx||0), hy=tTop-hr+1.5+(c.hunch?1:0)+(c.headDrop||0);
  const shX=tL+tw-2, shY=tTop+2; const bshX=tL+2;
  if(c.backItem) c.backItem(img,{tL,tTop,tw,tH,hipY,cx,gy,S,p});
  if(c.cape){ const cw=p.capeWave||0; for(let y=tTop+1;y<=hipY+Math.round(3*S)+(c.capeLong||0);y++){ const t=(y-tTop)/(hipY-tTop+3); const x0=tL-1-Math.round(t*2+cw*t), x1=tL+Math.round(tw*0.5); for(let x=x0;x<=x1;x++) img.set(x,y,x<=x0+1?c.cape[2]:(x>x1-2?c.cape[0]:c.cape[1])); } }
  // back arm (off hand)
  const offA=p.offA??1.35; const offLen=6*S*(c.armLen||1);
  if(!c.shield && !c.twoHand){ const [ex,ey]=pt(bshX,shY,offA,offLen); strip(img,bshX,shY,offA,0,offLen,lw/2-0.4,shadeDown(c.sleeve||c.torso)); if(c.offItem) c.offItem(img,ex,ey,p); else box(img,ex-1,ey-1,2,2,shadeDown(c.hand||c.skin)); }
  // legs
  const legs=[[-1,shadeDown(c.pants)],[1,c.pants]];
  const st=p.stride||0;
  if(c.robe){ const rb=c.robe; for(let y=hipY-1;y<=gy;y++){ const t=(y-hipY)/(gy-hipY); const w=Math.round(tw/2+t*2*S)+(y===gy?0:0); const sway=Math.round(st*t*1.5); for(let x=cx-w+sway+lean;x<=cx+w-1+sway+lean;x++){ const rel=(x-(cx-w+sway+lean))/(2*w-1); img.set(x,y,y===gy?rb[0]:(rel<0.3?rb[2]:(rel>0.72?rb[0]:rb[1]))); } } if(c.robeTrim){ for(let x=cx-tw/2-2;x<=cx+tw/2+2;x++) if(img.get(x,gy-1)) img.set(x,gy-1,c.robeTrim[1]); } const fx=cx+Math.round(tw/2)+Math.round(st*1.5)+lean; img.set(fx,gy,c.boots[1]); img.set(fx+1,gy,c.boots[0]); }
  else if(!c.noLegs) for(const [side,ramp] of legs){ const off=side*st*2.5*S; const lift=(side*Math.cos((p.phase||0))>0.3&&Math.abs(st)>0.05)?1:0; const x0=cx+(side<0?-lw:0)+Math.round(lean*0.5), fx=x0+Math.round(off), fy=gy-lift;
    for(let y=hipY;y<=fy;y++){ const t=(y-hipY)/Math.max(1,fy-hipY); const x=Math.round(x0+(fx-x0)*t); const bootRow=y>fy-2*S; const rr=bootRow?(side<0?shadeDown(c.boots):c.boots):ramp; for(let i=0;i<lw;i++) img.set(x+i,y,i===0?rr[2]:(i===lw-1?rr[0]:rr[1])); if(y===fy) img.set(x+lw,y,rr[0]); } }
  // torso
  for(let j=0;j<tH;j++) for(let i=0;i<tw;i++){ if(j===0&&(i===0||i===tw-1)) continue; const rel=i/(tw-1); const x=tL+i,y=tTop+j; let r=c.torso; img.set(x,y,(j===0)?r[2]:(rel<0.28?r[2]:(rel>0.72?r[0]:r[1]))); }
  if(c.belt) for(let i=0;i<tw;i++) img.set(tL+i,hipY-1+bob*0, i<2?c.belt[2]:(i>tw-3?c.belt[0]:c.belt[1]));
  const T={tL,tTop,tw,tH,hipY,cx,gy,S,hx,hy,hr,p,shX,shY,lean};
  if(c.deco) c.deco(img,T);
  // head
  if(c.pad) c.pad(img,T);
  if(c.headBack) c.headBack(img,T);
  shadeBlob(img,hx,hy,hr,hr*(c.headSquash||1),c.skin.concat([null]),{hi:false,rim:false});
  const ey=Math.round(hy+(c.eyeDy||0)), ex=Math.round(hx+hr*0.55);
  img.set(ex,ey,c.eye||P.DS); if(c.eye2) img.set(ex-2,ey,c.eye2);
  if(c.ear){ img.set(Math.round(hx-hr+1),ey-1,c.skin[1]); img.set(Math.round(hx-hr),ey-2,c.skin[1]); img.set(Math.round(hx-hr-1),ey-3,c.skin[2]); }
  if(c.tusks){ img.set(ex+1,ey+2,P.g5); img.set(ex-1,ey+2,P.g5); if(c.tusks>1){ img.set(ex+1,ey+1,P.g5); img.set(ex-1,ey+1,P.g4);} }
  if(c.brow) { img.set(ex,ey-1,c.skin[0]); img.set(ex-1,ey-1,c.skin[0]); img.set(ex+1,ey-1,c.skin[0]); }
  if(c.head) c.head(img,T,ex,ey);
  // weapon arm + weapon
  const armA=p.armA??1.2, wA=p.wA??-1.0, aLen=6.5*S*(c.armLen||1);
  const [hx2,hy2]=pt(shX,shY,armA,aLen);
  if(c.shield){ /* shield arm hidden behind shield */ }
  const wpn=()=>{ if(c.weapon && !p.noWeapon) W[c.weapon](img,hx2,hy2,wA,Object.assign({},c.wOpt||{},p.wOpt||{})); };
  if(!c.weaponFront) wpn();
  strip(img,shX,shY,armA,0,aLen,lw/2-0.4,c.sleeve||c.torso);
  if(c.twoHand){ const [ox,oy]=pt(hx2,hy2,wA,c.twoHand); box(img,ox-1,oy-1,2,2,c.hand||c.skin); }
  box(img,hx2-1,hy2-1,2,2,c.hand||c.skin);
  if(c.weaponFront) wpn();
  if(p.cast){ const [gx,gy2]=pt(hx2,hy2,armA,2); const g=c.castColor||[P.t2,P.t3]; img.set(gx,gy2,g[1]); img.set(gx+1,gy2-1,g[0]); if(p.cast>1){ img.set(gx+2,gy2,g[1]); img.set(gx,gy2-2,g[1]); img.set(gx+1,gy2+1,g[0]);} }
  if(c.shield) shield(img,tL+tw-1+(p.shieldDx||0),tTop+1+(p.shieldDy||0),c.shield,S);
  if(c.front) c.front(img,T);
  return img; }
function shadeDown(r){ return [r[0],r[0],r[1],null]; }
// rotation (nearest) around pivot, ccw positive = falling backwards (left)
function rotate(img,ang,px,py){ const n=new Img(img.w,img.h); const c=Math.cos(ang),s=Math.sin(ang); for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++){ const dx=x-px,dy=y-py; const sx=Math.round(px+dx*c-dy*s), sy=Math.round(py+dx*s+dy*c); if(sx>=0&&sy>=0&&sx<img.w&&sy<img.h){ const v=img.d[sy*img.w+sx]; if(v) n.d[y*img.w+x]=v; } } return n; }
// animation sets
const ANIM={
  melee:{ idle:[{bob:0},{bob:0,wA:-1.05},{bob:1,wA:-1.1},{bob:1,wA:-1.05}],
    attack:[{armA:1.0,wA:-1.3},{armA:-1.6,wA:-2.3,lean:-1},{armA:-2.1,wA:-2.7,lean:-1},{armA:0.2,wA:0.25,lean:1,dx:1},{armA:0.45,wA:0.7,lean:1,dx:1},{armA:1.0,wA:-0.4}],
    attack2:[{armA:0.9,wA:-1.4,crouch:1},{armA:-2.4,wA:-3.0,lean:-1},{armA:-2.8,wA:-3.4,lean:-1},{armA:0.6,wA:1.1,lean:2,dx:2,crouch:1},{armA:0.8,wA:1.3,lean:2,dx:2,crouch:1},{armA:1.1,wA:-0.3}],
    hurt:[{lean:-1,dx:-1,armA:1.4,wA:-1.8,bob:1},{lean:-1,dx:-2,armA:1.5,wA:-2.0}] },
  bow:{ idle:[{bob:0,armA:0.9,wA:0.9},{bob:0,armA:0.9,wA:0.95},{bob:1,armA:0.9,wA:1.0},{bob:1,armA:0.9,wA:0.95}],
    attack:[{armA:0.3,wA:0,wOpt:{draw:0}},{armA:0,wA:-0.1,wOpt:{draw:1,arrow:1}},{armA:0,wA:-0.1,wOpt:{draw:3,arrow:1},lean:-1},{armA:0,wA:-0.1,wOpt:{draw:4,arrow:1},lean:-1},{armA:0,wA:-0.1,wOpt:{draw:0}},{armA:0.5,wA:0.5}],
    attack2:[{armA:-0.4,wA:-0.6,wOpt:{draw:0}},{armA:-0.6,wA:-0.7,wOpt:{draw:2,arrow:1}},{armA:-0.6,wA:-0.7,wOpt:{draw:4,arrow:1},lean:-1},{armA:-0.6,wA:-0.7,wOpt:{draw:0}},{armA:-0.2,wA:-0.3,wOpt:{draw:3,arrow:1}},{armA:0.5,wA:0.5}],
    hurt:[{lean:-1,dx:-1,armA:1.2,wA:1.3,bob:1},{lean:-1,dx:-2,armA:1.3,wA:1.4}] },
  caster:{ idle:[{bob:0,wA:-1.45},{bob:0,wA:-1.45},{bob:1,wA:-1.45},{bob:1,wA:-1.45}],
    attack:[{armA:0.9,wA:-1.45},{armA:-0.5,wA:-1.2,cast:1},{armA:-0.9,wA:-1.3,cast:2,lean:-1},{armA:-0.2,wA:-0.6,cast:2,lean:1,dx:1},{armA:0.1,wA:-0.5,cast:1},{armA:0.9,wA:-1.4}],
    attack2:[{armA:1,wA:-1.45,offA:0.8},{armA:-1.2,wA:-1.5,offA:-1,cast:1},{armA:-1.5,wA:-1.6,offA:-1.4,cast:2},{armA:-1.5,wA:-1.6,offA:-1.4,cast:2,bob:1},{armA:-0.4,wA:-0.9,offA:0,cast:1,lean:1},{armA:0.9,wA:-1.45}],
    hurt:[{lean:-1,dx:-1,armA:1.3,wA:-1.8,bob:1},{lean:-1,dx:-2,armA:1.4,wA:-1.9}] },
  gun:{ idle:[{bob:0,armA:0.5,wA:-0.5},{bob:0,armA:0.5,wA:-0.5},{bob:1,armA:0.5,wA:-0.5},{bob:1,armA:0.5,wA:-0.5}],
    attack:[{armA:0.4,wA:-0.2},{armA:0.2,wA:0},{armA:0.2,wA:0,fire:1},{armA:0.2,wA:-0.2,fire:2,lean:-1,dx:-1},{armA:0.3,wA:-0.1},{armA:0.5,wA:-0.5}],
    attack2:[{armA:0.4,wA:-0.2},{armA:0.5,wA:0.3,crouch:1},{armA:0.5,wA:0.3,fire:1,crouch:1},{armA:0.5,wA:0.1,fire:2,crouch:1,lean:-1},{armA:0.5,wA:0.2,crouch:1},{armA:0.5,wA:-0.5}],
    hurt:[{lean:-1,dx:-1,armA:0.7,wA:-0.9,bob:1},{lean:-1,dx:-2,armA:0.8,wA:-1.0}] },
};
function buildSheet(c,cell){ const kind=c.anim||'melee'; const A=Object.assign({},ANIM[kind],c.animOverride||{}); const idleBase=A.idle[0]; const rows=[];
  const mk=p=>{ let im=humanoid(c,Object.assign({},c.basePose||{},p),cell,cell); if(p.fire){ const [x,y]=[Math.floor(cell/2)+16,Math.floor(cell)-17]; im.set(x,y,P.r5); im.set(x+1,y,P.r4); if(p.fire>1){ im.set(x+2,y-1,P.g4); im.set(x+3,y-2,P.g3);} } if(c.fx) c.fx(im,p); outline(im); return im; };
  rows.push(A.idle.map(mk));
  const walk=[]; for(let i=0;i<6;i++){ const ph=i/6*6.2832; walk.push(mk(Object.assign({},idleBase,{stride:Math.sin(ph),phase:ph,bob:(i%3===0)?0:(i%3===1?0:1),offA:1.35-Math.sin(ph)*0.5,armA:(idleBase.armA??1.2)+Math.sin(ph)*0.25,capeWave:1}))); } rows.push(walk);
  rows.push(A.attack.map(p=>mk(Object.assign({},idleBase,p))));
  rows.push(A.hurt.map(p=>mk(Object.assign({},idleBase,p))));
  const angs=[0.12,0.4,0.8,1.2,1.5,1.5708]; const die=[]; angs.forEach((a,i)=>{ let im=humanoid(c,Object.assign({},idleBase,{lean:-1,armA:-0.5-i*0.3,wA:-2.2-i*0.2,noWeapon:i>=3,crouch:Math.min(i,2)}),cell,cell); const px=Math.floor(cell/2)-2, py=cell-1; im=rotate(im,-a,px,py); if(i>=3 && c.weapon){ const w=new Img(cell,cell); W[c.weapon](w,Math.floor(cell/2)+4,cell-3,-0.05,Object.assign({},c.wOpt||{})); for(let k=0;k<w.d.length;k++) if(w.d[k]&&!im.d[k]) im.d[k]=w.d[k]; } outline(im); die.push(groundY(im)); }); rows.push(die);
  rows.push(A.attack2.map(p=>mk(Object.assign({},idleBase,p))));
  // baseline: all frames on bottom row
  const sheet=new Img(cell*6,cell*6); rows.forEach((row,r)=>row.forEach((im,i)=>{ const g=(r===4)?im:groundFix(im,c.hover||0); sheet.blit(g,i*cell,r*cell); })); return sheet; }
function groundFix(im,hover){ let maxy=-1; for(let y=0;y<im.h;y++) for(let x=0;x<im.w;x++) if(im.d[y*im.w+x]) maxy=Math.max(maxy,y); const dy=im.h-1-hover-maxy; if(dy===0||maxy<0) return im; const n=new Img(im.w,im.h); for(let y=0;y<im.h;y++) for(let x=0;x<im.w;x++){ const v=im.d[y*im.w+x]; if(v) n.set(x,y+dy,v);} return n; }
return {R,W,box,blobClip,strip,pt,shield,humanoid,rotate,ANIM,buildSheet,shadeDown};
