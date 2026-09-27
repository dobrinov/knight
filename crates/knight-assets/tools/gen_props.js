const {P,rng,Img,shadeBlob,line,outline,ground,fbm}=X;
const OUT={};
function sheetOf(cells,cw,ch,rows=1){ const img=new Img(cw*cells.length/rows,ch*rows); cells.forEach((c,i)=>img.blit(c,(i%(cells.length/rows))*cw,Math.floor(i/(cells.length/rows))*ch)); return img; }
function finish(img){ outline(img); return ground(img); }
function trunk(img,x,y0,y1,w,ramp,taper=0){ for(let y=y0;y<=y1;y++){ const t=(y-y0)/Math.max(1,y1-y0); const ww=Math.max(1,Math.round(w-taper*(1-t))); const flare=(y>=y1-1)?1:0; for(let i=-flare;i<ww+flare;i++){ const c=i<=0?ramp[2]:(i>=ww-1?ramp[0]:ramp[1]); img.set(x+i-Math.floor(ww/2),y,c);} } }
function thick(img,x0,y0,x1,y1,w,ramp){ const n=Math.ceil(Math.hypot(x1-x0,y1-y0)); for(let i=0;i<=n;i++){ const t=i/Math.max(1,n); const x=x0+(x1-x0)*t,y=y0+(y1-y0)*t; for(let k=0;k<w;k++){ img.set(x+k,y,k===0?ramp[2]:(k===w-1&&w>1?ramp[0]:ramp[1])); } } }
function leafTex(img,r,mid,dark,light,box,rate=0.1){ const [x0,y0,x1,y1]=box; for(let y=y0;y<y1;y++) for(let x=x0;x<x1;x++){ const c=img.get(x,y); if(c===mid && r()<rate){ img.set(x,y,dark); if(img.get(x-1,y-1)===mid) img.set(x-1,y-1,light);} } }
function branches(img,r,x,y,ang,len,w,depth,ramp){ const x2=x+Math.cos(ang)*len, y2=y+Math.sin(ang)*len; thick(img,x,y,x2,y2,w,ramp); if(depth<=0) return [[x2,y2]]; let tips=[]; const k=2; for(let i=0;i<k;i++){ const a=ang+(i?0.55:-0.5)+(r()-0.5)*0.4; tips=tips.concat(branches(img,r,x2,y2,a,len*0.68,Math.max(1,w-1),depth-1,ramp)); } return tips; }
const G=[P.n2,P.n3,P.n4,P.n5], GB=[P.n1,P.n2,P.n3,null];
const WOOD=[P.b1,P.b2,P.b3], CHAR=[P.O,P.DS,P.g1];
function oak(seed,size){ const r=rng(seed); const im=new Img(32,48); const cx=16, top=47-Math.round(26+size*14); trunk(im,cx,top+Math.round(12+size*8),46,3+Math.round(size),WOOD);
  const R=8+size*4; const blobs=[]; const n=6+Math.round(size*2); for(let i=0;i<n;i++){ const a=i/n*6.28+r(); blobs.push([cx+Math.cos(a)*R*0.55, top+R*0.9+Math.sin(a)*R*0.45, R*(0.5+r()*0.2), R*(0.45+r()*0.15)]); }
  blobs.sort((a,b)=>a[1]-b[1]); for(const b of blobs) shadeBlob(im,b[0]+1,b[1]+1,b[2],b[3],GB,{hi:false});
  for(const b of blobs) shadeBlob(im,b[0],b[1],b[2]*0.92,b[3]*0.92,G,{rim:true});
  shadeBlob(im,cx-1,top+R*0.75,R*0.55,R*0.45,G);
  leafTex(im,r,P.n3,P.n2,P.n4,[0,0,32,40],0.08); return finish(im); }
function burntOak(seed){ const r=rng(seed); const im=new Img(32,48); trunk(im,16,24,46,4,CHAR); const tips=[...branches(im,r,15,26,-2.2,7,2,2,CHAR),...branches(im,r,17,25,-0.9,7,2,2,CHAR)]; im.set(15,34,P.r3); im.set(16,38,P.r2); return finish(im); }
function pine(seed,size,snow=false){ const r=rng(seed); const im=new Img(32,48); const cx=16; const H=26+Math.round(size*14); const top=46-H; trunk(im,cx,46-6,46,3,WOOD);
  const tiers=3+Math.round(size); const th=(H-5)/tiers;
  for(let k=0;k<tiers;k++){ const ty=top+k*th*0.85, by=ty+th*1.35, hw=3+(k+1)*(3.2+size*1.2)/tiers*2.2;
    for(let y=Math.floor(ty);y<=by;y++){ const t=(y-ty)/(by-ty); const w=Math.max(1,Math.round(1+t*hw)); for(let x=-w;x<=w;x++){ const rel=x/w; let c=rel<-0.35?P.n3:(rel<0.3?P.n2:P.n1); if(y>=by-1 && (x+k)%3===0) continue; if(y===Math.floor(by) ) c=P.n1; if(rel<-0.6 && t<0.5) c=P.n4; im.set(cx+x,y,c);} }
    if(snow){ for(let y=Math.floor(ty);y<=ty+th*0.7;y++){ const t=(y-ty)/(by-ty); const w=Math.round(1+t*hw); for(let x=-w;x<=Math.round(w*0.3);x++) if(r()<0.85 && y-ty<(x+w)*0.6+1.2) im.set(cx+x,y,x<-w*0.4?P.g5:P.w5); } }
  }
  if(snow) { im.set(cx,top,P.g5); im.set(cx,top+1,P.g5); }
  return finish(im); }
function burntPine(seed){ const r=rng(seed); const im=new Img(32,48); trunk(im,16,10,46,3,CHAR,1); for(let k=0;k<5;k++){ const y=14+k*5, w=4+k; line(im,16,y,16-w,y+2+r()*2,P.DS); line(im,17,y+1,17+w,y+3+r()*2,P.DS);} im.set(16,30,P.r3); return finish(im); }
const BIRCH=[P.g3,P.g4,P.g5];
function birch(seed,size){ const r=rng(seed); const im=new Img(32,48); const cx=16+Math.round((r()-0.5)*2); const top=46-Math.round(26+size*14); trunk(im,cx,top+8,46,2+(size>0.6?1:0),BIRCH); for(let y=top+10;y<46;y+=3+Math.floor(r()*3)) im.set(cx-1+Math.floor(r()*2),y,P.DS);
  thick(im,cx,top+16,cx+5,top+11,1,BIRCH);
  const L=[P.n3,P.n4,P.n5,null], LB=[P.n2,P.n3,P.n4,null]; const R=5+size*3; for(let i=0;i<4+Math.round(size*2);i++){ const bx=cx+(r()-0.5)*R*1.2, by=top+R*0.6+i*R*0.45; shadeBlob(im,bx+1,by+1,R*0.6,R*0.55,LB,{hi:false}); shadeBlob(im,bx,by,R*0.55,R*0.5,L); }
  leafTex(im,r,P.n4,P.n3,P.n5,[0,0,32,40],0.1); return finish(im); }
function burntBirch(seed){ const r=rng(seed); const im=new Img(32,48); trunk(im,16,16,46,3,[P.DS,P.g1,P.g2]); for(let y=18;y<46;y+=4) im.set(16,y,P.O); branches(im,r,16,20,-2.0,6,1,1,[P.DS,P.g1,P.g2]); branches(im,r,17,19,-1.0,6,1,1,[P.DS,P.g1,P.g2]); return finish(im); }
const DEAD=[P.b1,P.g1,P.g2];
function deadTree(seed,lean){ const r=rng(seed); const im=new Img(32,48); trunk(im,16,26,46,4,DEAD); branches(im,r,15,28,-1.57+lean,8,3,3,DEAD); branches(im,r,17,31,-0.6+lean*0.5,6,2,2,DEAD); return finish(im); }
function log_(){ const im=new Img(32,48); for(let x=4;x<27;x++) for(let y=38;y<46;y++){ const t=(y-38)/7; im.set(x,y,t<0.25?P.b4:(t<0.7?P.b3:P.b2)); } for(let x=6;x<25;x+=5) im.set(x,41,P.b2); for(let y=38;y<46;y++) for(let x=24;x<29;x++){ const dx=(x-26)/2.5,dy=(y-41.5)/4; if(dx*dx+dy*dy<=1) im.set(x,y,dx*dx+dy*dy<0.3?P.b3:P.b5);} im.set(26,41,P.b2); thick(im,10,38,8,34,2,DEAD); return finish(im); }
function palm(seed,bend,burnt=false){ const r=rng(seed); const im=new Img(32,48); const pts=[]; const bx=13, top=12+Math.round(r()*4); for(let i=0;i<=20;i++){ const t=i/20; pts.push([bx+bend*t*t*8, 46-(46-top)*t]); }
  const ring=burnt?[P.O,P.DS,P.g1]:[P.b2,P.b3,P.b4]; pts.forEach(([x,y],i)=>{ const yy=Math.round(y); for(let k=0;k<3;k++) im.set(Math.round(x)+k-1,yy,i%3===0?ring[0]:(k===0?ring[2]:ring[1])); for(let k=0;k<3;k++) im.set(Math.round(x)+k-1,yy+1,k===0?ring[2]:ring[1]); });
  const [tx,ty]=pts[20]; if(burnt){ line(im,tx,ty,tx-4,ty-2,P.DS); line(im,tx,ty,tx+4,ty-1,P.DS); return finish(im);} 
  const fr=[[-2.7,11],[-0.45,12],[-2.1,8],[-1.0,8],[-3.05,9],[-0.1,10],[-1.6,6]];
  for(const [a,len] of fr){ const lit=Math.cos(a)<0; for(let i=1;i<=len;i++){ const x=tx+Math.cos(a)*i, y=ty+Math.sin(a)*i*0.7+0.09*i*i; im.set(x,y,lit?P.n4:P.n3); im.set(x,y+1,lit?P.n3:P.n2); if(i%2===0&&i>2){ im.set(x,y+2,P.n2); if(lit) im.set(x,y-1,P.n5);} } }
  im.set(tx,ty+1,P.b2); im.set(tx+1,ty+2,P.b1); im.set(tx-1,ty+2,P.b2); return finish(im); }
function elfTree(seed,size){ const r=rng(seed); const im=new Img(32,48); const cx=16; const top=46-Math.round(28+size*14); const W=[P.g3,P.g4,P.g5]; for(let y=top+12;y<=46;y++){ const x=cx+Math.round(Math.sin((y-top)/7)*1.5); for(let k=-1;k<2+(y>42?1:0);k++) im.set(x+k,y,k<0?W[2]:(k>0?W[0]:W[1])); }
  const V=[P.v1,P.v2,P.v3,null], Tl=[P.t1,P.t2,P.t3,null]; const R=8+size*4; const n=5+Math.round(size*2); for(let i=0;i<n;i++){ const a=i/n*6.28+r(); const bx=cx+Math.cos(a)*R*0.6, by=top+R*0.9+Math.sin(a)*R*0.55; const ramp=(i%3===0)?Tl:V; shadeBlob(im,bx,by,R*0.55,R*0.5,ramp); }
  shadeBlob(im,cx-1,top+R*0.7,R*0.5,R*0.42,V);
  for(let i=0;i<5;i++){ const x=Math.floor(r()*32),y=top+Math.floor(r()*R*2); if(im.get(x,y)) im.set(x,y,P.t3); }
  for(let i=0;i<2;i++){ const y=46-Math.floor(r()*14); const x=cx+(r()<0.5?-3:3); if(!im.get(x,y)) im.set(x,y,P.t3); } return finish(im); }
function witheredElf(seed){ const r=rng(seed); const im=new Img(32,48); const W=[P.g1,P.g2,P.g3]; trunk(im,16,22,46,3,W); const tips=[...branches(im,r,15,24,-2.1,7,2,2,W),...branches(im,r,17,23,-0.9,7,2,2,W)]; tips.forEach(([x,y],i)=>{ if(i%2===0){ im.set(x,y,P.v1); im.set(x+1,y,P.v2);} }); return finish(im); }
function snowDead(seed){ const r=rng(seed); const im=new Img(32,48); trunk(im,16,26,46,3,DEAD); const tips=[...branches(im,r,15,28,-1.9,7,2,2,DEAD),...branches(im,r,17,27,-1.0,7,2,2,DEAD)]; for(let y=0;y<48;y++) for(let x=0;x<32;x++){ if(DEAD.includes(im.get(x,y)) && !im.get(x,y-1) && r()<0.8) im.set(x,y-1,P.g5); } return finish(im); }
// rocks
const ST=[P.g1,P.g2,P.g3,P.g4];
function rock(im,cx,cy,rx,ry,ramp=ST){ shadeBlob(im,cx,cy,rx,ry,ramp); for(let y=Math.ceil(cy+ry*0.55);y<=cy+ry+1;y++) for(let x=0;x<im.w;x++) if(ramp.includes(im.get(x,y))) im.set(x,y,null); }
function rocksSheet(){ const a=new Img(32,32); rock(a,16,26,6,5); const b=new Img(32,32); rock(b,16,23,10,9); const r=rng(3); for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const c=b.get(x,y); if(c && c!==P.g1 && y<18+Math.sin(x*0.7)*2) b.set(x,y,y<16?P.n4:P.n3); if(c && y<14 && x<14 && c) b.set(x,y,P.n5);} leafTex(b,r,P.n4,P.n3,P.n5,[0,0,32,32],0.1);
  const c=new Img(32,32); rock(c,16,22,13,11); rock(c,22,26,6,5); line(c,14,13,17,19,P.g1); line(c,17,19,15,25,P.g1); line(c,17,19,21,22,P.g1);
  const d=new Img(32,32); rock(d,16,28,9,4); const sh=[[12,10,3,16],[17,6,3,20],[21,14,2,12],[9,17,2,8],[24,19,2,7]]; for(const [x,top,w,h] of sh){ for(let y=top;y<top+h;y++){ const t=(y-top); const ww=Math.min(w,Math.floor(t/1.5)+1); for(let k=-ww;k<=ww;k++) d.set(x+k,y,k<0?P.t3:(k===0?P.t2:P.t1)); } }
  return [a,b,c,d].map(finish); }
function bushSheet(){ const B=[P.n2,P.n3,P.n4,P.n5], BB=[P.n1,P.n2,P.n3,null];
  const mk=(seed,berries)=>{ const r=rng(seed); const im=new Img(32,32); const bl=[[11,23,6,5],[20,23,6,5],[15,19,7,6]]; for(const b of bl) shadeBlob(im,b[0]+1,b[1]+1,b[2],b[3],BB,{hi:false}); for(const b of bl) shadeBlob(im,b[0],b[1],b[2]*0.9,b[3]*0.9,B); leafTex(im,r,P.n3,P.n2,P.n4,[0,0,32,32],0.12); if(berries) for(let i=0;i<9;i++){ const x=8+Math.floor(r()*17),y=15+Math.floor(r()*11); if(im.get(x,y)){ im.set(x,y,P.r3); im.set(x+1,y,P.r2); im.set(x,y-1,P.r4);} } return finish(im); };
  const reeds=new Img(32,32); const r=rng(8); for(let i=0;i<7;i++){ const x=9+i*2+Math.floor(r()*2), h=10+Math.floor(r()*10); for(let y=31-h;y<31;y++) reeds.set(x+(y<31-h*0.7&&i%2?1:0),y,i%2?P.n3:P.n4); if(i%2===0){ reeds.set(x,31-h-1,P.b2); reeds.set(x,31-h-2,P.b3); reeds.set(x,31-h-3,P.b3);} }
  const fl=new Img(32,32); const cols=[[P.g5,P.g4],[P.y3,P.y2],[P.v3,P.v2],[P.w4,P.w3],[P.r4,P.r3]]; for(let i=0;i<9;i++){ const x=8+Math.floor(r()*16), h=4+Math.floor(r()*6); for(let y=31-h;y<31;y++) fl.set(x,y,P.n3); fl.set(x+1,31-Math.floor(h/2),P.n4); const [c1,c2]=cols[i%5]; const fy=31-h-1; fl.set(x,fy,P.y3); fl.set(x-1,fy,c1); fl.set(x+1,fy,c2); fl.set(x,fy-1,c1); fl.set(x,fy+1,c2); }
  return [mk(11,false),mk(12,true),finish(reeds),finish(fl)]; }
// mountains 64x64
function mountain(kind,seed){ const r=rng(seed); const im=new Img(64,64); const peaks=[[30,6,1.25],[44,22,1.1],[18,24,1.0]]; const n=fbm(r,64,64,[8,4],[2,1]);
  const h=x=>Math.max(...peaks.map(([px,py,s])=>63-py-Math.abs(x-px)*s*(1+(n(x,3)-0.5)*0.5)));
  const hs=[]; for(let x=0;x<64;x++) hs.push(Math.min(h(x),58));
  let ramp=kind==='volcano'?[P.DS,P.DS,P.g1,P.g2]:[P.g1,P.g2,P.g3,P.g4];
  for(let x=2;x<62;x++){ const top=63-hs[x]; const d=(hs[Math.min(63,x+1)]-hs[Math.max(0,x-1)]); for(let y=Math.max(0,Math.round(top));y<64;y++){ const depth=(y-top)/(64-top); let c; const nn=n(x,y);
    if(d>0.4) c=nn>0.55?ramp[3]:ramp[2]; else if(d<-0.4) c=nn>0.65?ramp[1]:ramp[0]; else c=ramp[1];
    if(depth>0.8 && c===ramp[2]) c=ramp[1];
    im.set(x,y,c);} }
  // ridge lines
  for(let i=0;i<5;i++){ let x=12+Math.floor(r()*40), y=63-hs[x]+3; for(let k=0;k<12;k++){ if(im.get(x,y)) im.set(x,y,ramp[0]); y++; if(r()<0.4) x+=r()<0.5?1:-1; } }
  if(kind==='green'){ for(let x=0;x<64;x++){ const gy=34+Math.round((n(x,40)-0.5)*14); for(let y=gy;y<64;y++){ const c=im.get(x,y); if(!c) continue; const lit=c===ramp[2]||c===ramp[3]; im.set(x,y, lit?(y<gy+2?P.n5:P.n4):(c===ramp[0]?P.n2:P.n3)); } } }
  if(kind==='snow'){ for(let x=0;x<64;x++){ const top=63-hs[x]; const sy=top+6+Math.round(n(x,10)*10)+((x%5)===0?2:0); for(let y=Math.round(top);y<Math.min(sy,58);y++){ const c=im.get(x,y); if(!c) continue; im.set(x,y,(c===ramp[2]||c===ramp[3])?P.g5:(c===ramp[0]?P.w4:P.w5)); } } }
  if(kind==='volcano'){ // flatten crater + lava crack
    for(let x=26;x<35;x++) for(let y=0;y<14;y++) if(y<10) im.set(x,y,null); for(let x=27;x<34;x++) im.set(x,10,x<30?P.r4:P.r5);
    let x=31,y=11; for(let k=0;k<40;k++){ im.set(x,y,k%4===0?P.r5:P.r4); if(k%3===0) im.set(x+1,y,P.r3); y++; if(r()<0.45) x+=r()<0.6?1:-1; if(y>60) break; } }
  return finish(im); }
// fire
function flame(W,H,f,seed){ const im=new Img(W,H); const cx=W/2-0.5; const hw=W*0.36; const r=rng(seed);
  const ph=[r()*6.28,r()*6.28,r()*6.28];
  for(let x=0;x<W;x++){ const dx=(x-cx)/hw; if(Math.abs(dx)>1) continue; const base=(1-dx*dx); const wob=0.72+0.18*Math.sin(6.2832*f/8+ph[0]+dx*2.4)+0.1*Math.sin(6.2832*2*f/8+ph[1]-dx*3.1); const hh=Math.max(2,(H-2)*base*wob*(0.9+0.1*Math.cos(dx*5+ph[2])));
    for(let yy=0;yy<hh;yy++){ const t=yy/hh; const y=H-1-yy; let c; const core=Math.abs(dx)<0.45&&t<0.55; if(core) c=P.r5; else if(t<0.72&&Math.abs(dx)<0.8) c=P.r4; else if(t<0.9) c=P.r3; else c=P.r2; im.set(x,y,c);} }
  // flicker spark
  const sy=Math.round(H*0.12+((f*3)%8)); const sx=Math.round(cx+Math.sin(f*0.785+ph[0])*hw*0.4); if(!im.get(sx,sy)) { im.set(sx,sy,P.r4); } return im; }
function smoke(f){ const im=new Img(16,16); const R=[3,4,5.5,6.5][f]; const keep=[1,0.85,0.6,0.35][f]; const cy=12-f*1.5; const cx=8;
  for(let y=0;y<16;y++) for(let x=0;x<16;x++){ const dx=(x+0.5-cx)/R, dy=(y+0.5-cy)/R; const d=dx*dx+dy*dy; if(d>1) continue; const bayer=[[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]][y%4][x%4]/16; if(bayer>keep) continue; const l=-(dx+dy); im.set(x,y,l>0.4?P.g4:(l>-0.3?P.g3:P.g2)); } return im; }
function ember(f){ const im=new Img(16,16); const pts=[[6,12],[9,13],[8,10]]; pts.forEach(([x,y],i)=>{ if(i<=f||f===3){ const yy=y-f*2-i; if(yy>2) im.set(x+((f+i)%2),yy, i===0?P.r5:(i===1?P.r4:P.r3)); } }); return im; }
function scorch(){ const im=new Img(32,16); const r=rng(21); const n=fbm(r,32,16,[4],[1]); for(let y=0;y<16;y++) for(let x=0;x<32;x++){ const dx=(x-15.5)/15, dy=(y-7.5)/7.5; const d=dx*dx+dy*dy+ (n(x,y)-0.5)*0.6; if(d<0.35) im.set(x,y,P.DS); else if(d<0.7) im.set(x,y,P.g1); else if(d<0.95 && (x+y)%2===0) im.set(x,y,P.b1); } im.set(14,7,P.r3); im.set(18,8,P.r4); return im; }
function rubble(){ const im=new Img(32,16); const r=rng(22); for(let i=0;i<9;i++){ const x=5+r()*22, y=7+r()*6; shadeBlob(im,x,y,2+r()*2,1.5+r(),i%3?[P.DS,P.g1,P.g2,null]:[P.b1,P.b2,P.b3,null]); } line(im,6,11,15,6,P.b1); line(im,6,12,15,7,P.b2); im.set(12,10,P.r4); im.set(20,9,P.r3); im.set(22,11,P.r5); return im; }
function fxSheet(){ const im=new Img(256,112); for(let f=0;f<8;f++) im.blit(flame(16,24,f,5),f*16,0); for(let f=0;f<8;f++) im.blit(flame(32,48,f,9),f*32,24); for(let f=0;f<4;f++) im.blit(smoke(f),f*16,72); for(let f=0;f<4;f++) im.blit(ember(f),64+f*16,72); im.blit(scorch(),0,88); im.blit(rubble(),32,88); return im; }

OUT.props_trees_oak=sheetOf([oak(1,0.3),oak(2,0.65),oak(3,1),burntOak(4)],32,48);
OUT.props_trees_pine=sheetOf([pine(1,0.2),pine(2,0.6),pine(3,1),burntPine(4)],32,48);
OUT.props_trees_birch=sheetOf([birch(1,0.3),birch(2,0.7),birch(3,1),burntBirch(4)],32,48);
OUT.props_trees_dead=sheetOf([deadTree(1,0),deadTree(2,0.3),deadTree(5,-0.25),log_()],32,48);
OUT.props_trees_palm=sheetOf([palm(1,0.6),palm(2,1),palm(3,0.2),palm(4,0.5,true)],32,48);
OUT.props_trees_snow=sheetOf([pine(11,0.3,true),pine(12,0.65,true),pine(13,1,true),snowDead(14)],32,48);
OUT.props_trees_elf=sheetOf([elfTree(1,0.35),elfTree(2,0.7),elfTree(3,1),witheredElf(4)],32,48);
OUT.props_rocks=sheetOf(rocksSheet(),32,32);
OUT.props_bushes_flowers=sheetOf(bushSheet(),32,32);
OUT.props_mountains=sheetOf([mountain('rock',1),mountain('green',2),mountain('snow',3),mountain('volcano',4)],64,64);
OUT.fx_fire=fxSheet();
return OUT;
