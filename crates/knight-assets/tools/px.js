// Knight Engine pixel toolkit. Evaluate with new Function(src)() → returns API.
const P = {
  O:'#1B1420', DS:'#2E2433',
  g1:'#4A4553', g2:'#6E6977', g3:'#9A96A1', g4:'#C9C6CC', g5:'#F2F0EB',
  b1:'#3D2A1E', b2:'#5C3F28', b3:'#7D5A3A', b4:'#A67C52', b5:'#D1A574',
  s1:'#8C5A3C', s2:'#D99A6C', s3:'#F2C8A0',
  o1:'#2E4A22', o2:'#4F7A30', o3:'#7FA848',
  n1:'#1F3D24', n2:'#2F5A2E', n3:'#4A7D3A', n4:'#6FA848', n5:'#A8D46A',
  y1:'#7A5A1E', y2:'#C9962E', y3:'#F2D35E',
  r1:'#5E1F1F', r2:'#9C2F2A', r3:'#D8503C', r4:'#F28A5E', r5:'#FFD27A',
  w1:'#1E2E5C', w2:'#2F55A0', w3:'#4A86D8', w4:'#8EC3F2', w5:'#D6EEFA',
  v1:'#3A2A5C', v2:'#6A4AA8', v3:'#A88AE8',
  t1:'#1F5E5E', t2:'#2E9A8A', t3:'#7AE8D8',
  m1:'#660066', m2:'#990099', m3:'#CC00CC', m4:'#FF00FF',
};
const ALL = new Set(Object.values(P));
function rng(seed){ let a = seed>>>0; return () => { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
class Img {
  constructor(w,h,wrap=false){ this.w=w; this.h=h; this.wrap=wrap; this.d=new Array(w*h).fill(null); }
  idx(x,y){ x=Math.round(x); y=Math.round(y); if(this.wrap){ x=((x%this.w)+this.w)%this.w; y=((y%this.h)+this.h)%this.h; } else if(x<0||y<0||x>=this.w||y>=this.h) return -1; return y*this.w+x; }
  get(x,y){ const i=this.idx(x,y); return i<0?null:this.d[i]; }
  set(x,y,c){ const i=this.idx(x,y); if(i>=0) this.d[i]=c; }
  setIf(x,y,c,pred){ const i=this.idx(x,y); if(i>=0 && pred(this.d[i])) this.d[i]=c; }
  fill(c){ this.d.fill(c); return this; }
  rect(x,y,w,h,c){ for(let j=0;j<h;j++) for(let i=0;i<w;i++) this.set(x+i,y+j,c); }
  clone(){ const n=new Img(this.w,this.h,this.wrap); n.d=this.d.slice(); return n; }
  blit(src,dx,dy){ for(let y=0;y<src.h;y++) for(let x=0;x<src.w;x++){ const c=src.d[y*src.w+x]; if(c) this.set(dx+x,dy+y,c); } }
  flipX(){ const n=new Img(this.w,this.h); for(let y=0;y<this.h;y++) for(let x=0;x<this.w;x++) n.d[y*this.w+x]=this.d[y*this.w+(this.w-1-x)]; return n; }
}
// periodic value noise
function pnoise(r, W, H, cell){
  const gw=Math.max(1,Math.round(W/cell)), gh=Math.max(1,Math.round(H/cell));
  const g=[]; for(let i=0;i<gw*gh;i++) g.push(r());
  const sm=t=>t*t*(3-2*t);
  return (x,y)=>{ const fx=((x/W*gw)%gw+gw)%gw, fy=((y/H*gh)%gh+gh)%gh; const x0=Math.floor(fx), y0=Math.floor(fy); const tx=sm(fx-x0), ty=sm(fy-y0); const x1=(x0+1)%gw, y1=(y0+1)%gh;
    const a=g[y0*gw+x0], b=g[y0*gw+x1], c=g[y1*gw+x0], d=g[y1*gw+x1]; return (a+(b-a)*tx)+((c+(d-c)*tx)-(a+(b-a)*tx))*ty; };
}
function fbm(r,W,H,cells,weights){ const ns=cells.map(c=>pnoise(r,W,H,c)); const tw=weights.reduce((a,b)=>a+b,0); return (x,y)=>ns.reduce((s,n,i)=>s+n(x,y)*weights[i],0)/tw; }
// periodic voronoi: returns fn(x,y) -> {d1,d2,id}
function voronoi(r,W,H,n){
  const pts=[]; for(let i=0;i<n;i++) pts.push([r()*W, r()*H]);
  return (x,y)=>{ let d1=1e9,d2=1e9,id=0; for(let i=0;i<n;i++){ let dx=Math.abs(x-pts[i][0]); dx=Math.min(dx,W-dx); let dy=Math.abs(y-pts[i][1]); dy=Math.min(dy,H-dy); const d=Math.sqrt(dx*dx+dy*dy); if(d<d1){d2=d1;d1=d;id=i;} else if(d<d2) d2=d; } return {d1,d2,id}; };
}
function tones(v, ramp, cuts){ for(let i=0;i<cuts.length;i++) if(v<cuts[i]) return ramp[i]; return ramp[ramp.length-1]; }
// sparse ordered dithering near threshold boundaries
function ditherTone(v, x, y, ramp, cuts, band=0.025){
  for(let i=0;i<cuts.length;i++){ if(Math.abs(v-cuts[i])<band && ((x+y*2)%4===0)) return ramp[v<cuts[i]?i+1:i]; }
  return tones(v,ramp,cuts);
}
// shaded ellipse, light from upper-left. ramp=[dark,mid,light,(hi)]
function shadeBlob(img,cx,cy,rx,ry,ramp,opts={}){
  const {hi=true, rim=true, bias=0}=opts;
  for(let y=Math.floor(cy-ry);y<=Math.ceil(cy+ry);y++) for(let x=Math.floor(cx-rx);x<=Math.ceil(cx+rx);x++){
    const nx=(x+0.5-cx)/rx, ny=(y+0.5-cy)/ry, d=nx*nx+ny*ny; if(d>1) continue;
    let l = -(nx*0.7+ny*0.7) + bias; let c;
    if(rim && d>0.72 && l<0.1) c=ramp[0];
    else if(l>0.55 && hi && ramp[3] && d<0.5 && nx<-0.2 && ny<-0.2) c=ramp[3];
    else if(l>0.25) c=ramp[2]; else if(l>-0.45) c=ramp[1]; else c=ramp[0];
    img.set(x,y,c);
  }
}
function line(img,x0,y0,x1,y1,c){ x0=Math.round(x0);y0=Math.round(y0);x1=Math.round(x1);y1=Math.round(y1); const dx=Math.abs(x1-x0), dy=-Math.abs(y1-y0), sx=x0<x1?1:-1, sy=y0<y1?1:-1; let e=dx+dy; for(;;){ img.set(x0,y0,c); if(x0===x1&&y0===y1) break; const e2=2*e; if(e2>=dy){e+=dy;x0+=sx;} if(e2<=dx){e+=dx;y0+=sy;} } }
// 1px outline around opaque pixels (drawn into transparent neighbours)
function outline(img,c=P.O){ const src=img.d.slice(); const W=img.w,H=img.h; for(let y=0;y<H;y++) for(let x=0;x<W;x++){ if(src[y*W+x]) continue; let n=false; for(const [dx,dy] of [[1,0],[-1,0],[0,1],[0,-1]]){ const xx=x+dx,yy=y+dy; if(xx>=0&&yy>=0&&xx<W&&yy<H&&src[yy*W+xx]) n=true; } if(n) img.d[y*W+x]=c; } return img; }
// move sprite so lowest opaque pixel is on bottom row and bbox centred
function ground(img){ let minx=1e9,maxx=-1,maxy=-1; for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++) if(img.d[y*img.w+x]){ minx=Math.min(minx,x); maxx=Math.max(maxx,x); maxy=Math.max(maxy,y);} if(maxy<0) return img; const dx=Math.floor((img.w-(maxx-minx+1))/2)-minx, dy=img.h-1-maxy; const n=new Img(img.w,img.h); for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++){ const c=img.d[y*img.w+x]; if(c) n.set(x+dx,y+dy,c);} return n; }
function groundY(img){ let maxy=-1; for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++) if(img.d[y*img.w+x]) maxy=Math.max(maxy,y); if(maxy<0) return img; const dy=img.h-1-maxy; const n=new Img(img.w,img.h); for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++){ const c=img.d[y*img.w+x]; if(c) n.set(x,y+dy,c);} return n; }
function toCanvas(img, createCanvas, scale=1, bg=null){ const cv=createCanvas(img.w*scale,img.h*scale); const ctx=cv.getContext('2d'); if(bg){ctx.fillStyle=bg;ctx.fillRect(0,0,cv.width,cv.height);} for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++){ const c=img.d[y*img.w+x]; if(c){ ctx.fillStyle=c; ctx.fillRect(x*scale,y*scale,scale,scale);} } return cv; }
function validate(img){ const bad=new Set(); for(const c of img.d) if(c && !ALL.has(c)) bad.add(c); return [...bad]; }
return { P, rng, Img, pnoise, fbm, voronoi, tones, ditherTone, shadeBlob, line, outline, ground, groundY, toCanvas, validate };
