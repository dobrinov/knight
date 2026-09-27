// expects X (px API). returns {name: Img}
const {P,rng,Img,fbm,voronoi,ditherTone,tones,line}=X;
const K=[0.45,0.75,1.05,1.5];
let BASE_SEED=1;
function base(img,r,ramp,cuts,cells=[8,4],w=[2,1]){ const rb=rng(BASE_SEED); const lo=fbm(rb,img.w,img.h,cells,w); const wx=fbm(rb,img.w,img.h,[16,8],[1,1]), wy=fbm(rb,img.w,img.h,[16,8],[1,1]); const hf=fbm(r,img.w,img.h,[4],[1]);
  const n=(x,y)=>{ const X=x+(wx(x,y)-0.5)*10, Y=y+(wy(x,y)-0.5)*10; return lo(X,Y)*0.8+hf(x,y)*0.2+0.0; };
  for(let y=0;y<img.h;y++) for(let x=0;x<img.w;x++) img.set(x,y,ditherTone(n(x,y),x,y,ramp,cuts)); return n; }
function specks(img,r,count,colors,pred=()=>true){ for(let i=0;i<count;i++){ const x=Math.floor(r()*img.w),y=Math.floor(r()*img.h); if(pred(img.get(x,y))) img.set(x,y,colors[Math.floor(r()*colors.length)]); } }
function pebble(img,x,y,lt,md,dk,big){ if(big){ img.set(x,y,lt); img.set(x+1,y,md); img.set(x,y+1,md); img.set(x+1,y+1,dk);} else { img.set(x,y,lt); img.set(x+1,y,dk);} }
function blades(img,r,count,lt,dk){ for(let i=0;i<count;i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); img.set(x,y,lt); img.set(x,y+1,dk); if(r()<0.5){ img.set(x+1,y+1,lt); img.set(x+1,y+2,dk);} if(r()<0.3){ img.set(x-1,y+1,lt); img.set(x-1,y+2,dk);} } }
function crackWalk(img,r,len,c,x,y){ let dx=r()<0.5?1:-1, dy=r()<0.5?1:-1; for(let i=0;i<len;i++){ img.set(x,y,c); if(r()<0.5) x+=dx; else y+=dy; if(r()<0.15) dx=-dx; } }
function ripple(img,x,y,len,c,amp=1){ for(let i=0;i<len;i++) img.set(x+i,y+Math.round(Math.sin(i/len*Math.PI)*-amp),c); }
// cliff: horizontal strata, lit from top-left
function strata(img,r,ramp,{bh=4,lip=null,lipC=null,wob=1.5}={}){
  const wn=fbm(r,32,16,[8,4],[2,1]); const vn=fbm(r,32,16,[4,2],[1,1]);
  for(let y=0;y<16;y++) for(let x=0;x<32;x++){ const yy=y+(wn(x,y)-0.5)*wob*2; const band=((yy%bh)+bh)%bh; let c;
    if(band<1) c=ramp[2]; else if(band>=bh-1) c=ramp[0]; else c=vn(x,y)>0.62?ramp[0]:ramp[1];
    if(band<1 && x%7===((y*3)%7)) c=ramp[1];
    img.set(x,y,c); }
  if(lip){ const ln=fbm(r,32,16,[8],[1]); for(let x=0;x<32;x++){ const d=2+(ln(x,0)>0.55?1:0)+(r()<0.2?1:0); for(let y=0;y<d;y++) img.set(x,y,y===0?lip[2]:(y===d-1?lip[0]:lip[1])); } if(lipC) for(let i=0;i<3;i++) img.set(Math.floor(r()*32),0,lipC[Math.floor(r()*lipC.length)]); }
}
function roots(img,r,n,c1,c2,top=2){ for(let i=0;i<n;i++){ let x=Math.floor(r()*32),y=top+Math.floor(r()*4); const len=3+Math.floor(r()*5); for(let j=0;j<len;j++){ img.set(x,y,c1); img.set(x+1,y,c2); y++; if(r()<0.5) x+=r()<0.5?-1:1; } } }
function cliffPebbles(img,r,n,lt,md,dk,top=3){ for(let i=0;i<n;i++) pebble(img,Math.floor(r()*32),top+Math.floor(r()*(14-top)),lt,md,dk,r()<0.6); }

const T={};
function sheet(name,top,cliff){ BASE_SEED=[...name].reduce((a,c)=>a*31+c.charCodeAt(0)|0,7); const img=new Img(128,48); for(let v=0;v<4;v++){ const t=new Img(32,32,true); top(t,rng(1000*name.length+v*97+7),v); img.blit(t,v*32,0); const c=new Img(32,16,true); cliff(c,rng(500+name.length*31+v*13),v); img.blit(c,v*32,32);} T[name]=img; }
function animSheet(name,topFrame,cliff){ BASE_SEED=[...name].reduce((a,c)=>a*31+c.charCodeAt(0)|0,7); const img=new Img(128,80); for(let row=0;row<2;row++) for(let f=0;f<4;f++){ const t=new Img(32,32,true); topFrame(t,row,f); img.blit(t,f*32,row*32);} for(let v=0;v<4;v++){ const c=new Img(32,16,true); cliff(c,rng(900+v*13+name.length),v); img.blit(c,v*32,64);} T[name]=img; }

const earthCliff=(t,r,v,lip,lipC)=>{ strata(t,r,[P.b1,P.b2,P.b3],{bh:5,lip,lipC}); roots(t,r,1+v%2,P.b3,P.b1,lip?2:0); cliffPebbles(t,r,2+v,P.g4,P.g3,P.g2); };

sheet('terrain_grass',(t,r,v)=>{ base(t,r,[P.n2,P.n3,P.n4],[0.34,0.72]); blades(t,r,Math.round(18*K[v]),P.n5,P.n3); specks(t,r,Math.round(10*K[v]),[P.n2],c=>c===P.n3); specks(t,r,Math.round(5*K[v]),[P.g5,P.y3]); },
  (t,r,v)=>earthCliff(t,r,v,[P.n3,P.n4,P.n5]));
sheet('terrain_meadow',(t,r,v)=>{ base(t,r,[P.n2,P.n3,P.n4],[0.3,0.68]); blades(t,r,Math.round(22*K[v]),P.n5,P.n3); const fl=[P.g5,P.y3,P.w4,P.v3]; for(let i=0;i<Math.round(26*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32),c=fl[Math.floor(r()*4)]; t.set(x,y,c); if(r()<0.3){ t.set(x+1,y,c); t.set(x,y+1,P.n2);} } },
  (t,r,v)=>earthCliff(t,r,v,[P.n3,P.n4,P.n5],[P.g5,P.y3,P.v3,P.w4]));
sheet('terrain_sand',(t,r,v)=>{ base(t,r,[P.b4,P.b5,P.r5],[0.22,0.7],[16,8],[2,1]); for(let i=0;i<Math.round(6*K[v])+2;i++) ripple(t,Math.floor(r()*32),Math.floor(r()*32),5+Math.floor(r()*5),P.b4); specks(t,r,Math.round(5*K[v]),[P.g5,P.s3]); },
  (t,r,v)=>{ strata(t,r,[P.b4,P.b5,P.r5],{bh:4}); for(let y=0;y<3;y++) for(let x=0;x<32;x++) if(t.get(x,y)===P.b4) t.set(x,y,P.b5); specks(t,r,4,[P.b3]); });
sheet('terrain_desert',(t,r,v)=>{ base(t,r,[P.b4,P.r4,P.b5],[0.28,0.68],[16,8],[2,1]); for(let i=0;i<Math.round(8*K[v])+3;i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32),len=6+Math.floor(r()*8); ripple(t,x,y,len,P.b4,1); ripple(t,x+1,y-1,len-2,P.r5,1);} specks(t,r,3,[P.b3]); },
  (t,r,v)=>{ strata(t,r,[P.b3,P.b4,P.r4],{bh:4}); specks(t,r,6,[P.b5]); cliffPebbles(t,r,v,P.b5,P.b4,P.b3); });
sheet('terrain_rock',(t,r,v)=>{ const vo=voronoi(r,32,32,4+v); const sh=[P.g2,P.g3,P.g3,P.g4]; for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const q=vo(x,y); t.set(x,y, q.d2-q.d1<1.1?P.g1:sh[q.id%4]); } specks(t,r,Math.round(20*K[v]),[P.g2,P.g4],c=>c!==P.g1); for(let i=0;i<v;i++) crackWalk(t,r,5,P.g1,Math.floor(r()*32),Math.floor(r()*32)); },
  (t,r,v)=>{ strata(t,r,[P.g1,P.g2,P.g3],{bh:4,wob:1}); for(let i=0;i<2+v;i++){ let x=Math.floor(r()*32),y=Math.floor(r()*8); for(let j=0;j<5;j++){ t.set(x,y+j,P.g1); if(r()<0.4) x++; } } });
sheet('terrain_dirt',(t,r,v)=>{ base(t,r,[P.b2,P.b3,P.b4],[0.3,0.72]); for(let i=0;i<Math.round(8*K[v]);i++) pebble(t,Math.floor(r()*32),Math.floor(r()*32),P.g4,P.g3,P.g2,r()<0.4); for(let i=0;i<v;i++) crackWalk(t,r,6,P.b1,Math.floor(r()*32),Math.floor(r()*32)); },
  (t,r,v)=>{ strata(t,r,[P.b1,P.b2,P.b3],{bh:4}); cliffPebbles(t,r,3+v,P.g4,P.g3,P.g2,0); });
sheet('terrain_road_dirt',(t,r,v)=>{ base(t,r,[P.b3,P.b4,P.b5],[0.3,0.74]); for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const d=((x+y)%32+32)%32; if((d===9||d===21) && r()<0.8) t.set(x,y,P.b3); if((d===10||d===22) && r()<0.3) t.set(x,y,P.b2);} for(let i=0;i<Math.round(7*K[v]);i++) pebble(t,Math.floor(r()*32),Math.floor(r()*32),P.g4,P.g3,P.g2,r()<0.3); },
  (t,r,v)=>{ strata(t,r,[P.b2,P.b3,P.b4],{bh:5,lip:[P.b3,P.b4,P.b5]}); cliffPebbles(t,r,2+v,P.g4,P.g3,P.g2); });
sheet('terrain_road_cobble',(t,r,v)=>{ t.fill(P.g1); const sh=[P.g2,P.g3,P.g3]; for(let row=0;row<8;row++) for(let col=0;col<4;col++){ const ox=col*8+(row%2)*4, oy=row*4; const b=sh[Math.floor(r()*3)]; for(let j=0;j<3;j++) for(let i=0;i<7;i++){ if((i===0||i===6)&&(j===0||j===2)) continue; let c=b; if(j===0||i===0) c=b===P.g2?P.g3:P.g4; if(j===2||i===6) c=b===P.g3?P.g2:P.g1; if(j===2&&i===0||j===0&&i===6) c=b; t.set(ox+i,oy+j,c);} } specks(t,r,Math.round(6*K[v]),[P.n3,P.b3],c=>c===P.g1); },
  (t,r,v)=>{ t.fill(P.g1); for(let row=0;row<4;row++) for(let col=0;col<4;col++){ const ox=col*8+(row%2)*4, oy=row*4; for(let j=0;j<3;j++) for(let i=0;i<7;i++) t.set(ox+i,oy+j, j===0?P.g3:(i===6||j===2?P.g1:P.g2)); } specks(t,r,2+v,[P.n3],c=>c===P.g1); });
sheet('terrain_farmland',(t,r,v)=>{ for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const d=((x+y)%4+4)%4; t.set(x,y,[P.b2,P.b3,P.b4,P.b3][d]); } specks(t,r,Math.round(14),[P.b2,P.b4]); for(let i=0;i<Math.round(8*K[v]);i++){ const x=Math.floor(r()*32), y0=Math.floor(r()*32); const y=y0-((x+y0)%4)+2; t.set(x,y,P.n4); t.set(x,y-1,P.n5); } },
  (t,r,v)=>{ strata(t,r,[P.b2,P.b3,P.b4],{bh:5,lip:[P.b2,P.b3,P.b4],wob:0.5}); });
sheet('terrain_forest_floor',(t,r,v)=>{ base(t,r,[P.n1,P.n2,P.b2],[0.42,0.72]); for(let i=0;i<Math.round(16*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32),c=r()<0.5?P.b3:P.b2; t.set(x,y,c); t.set(x+(r()<0.5?1:-1),y+1,c);} for(let i=0;i<Math.round(5*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); t.set(x,y,P.n3); t.set(x+1,y,P.n2);} for(let i=0;i<Math.round(2*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); t.set(x,y,r()<0.5?P.r3:P.b5); t.set(x,y+1,P.g5);} },
  (t,r,v)=>{ strata(t,r,[P.DS,P.b1,P.b2],{bh:5,lip:[P.n1,P.n2,P.n3]}); roots(t,r,2+v,P.b3,P.b2); });
sheet('terrain_elf_moss',(t,r,v)=>{ base(t,r,[P.t1,P.n3,P.t2],[0.36,0.72]); blades(t,r,Math.round(10*K[v]),P.n4,P.t1); specks(t,r,Math.round(4*K[v]),[P.t3]); for(let i=0;i<Math.round(4*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); t.set(x,y,P.v3); t.set(x+1,y+1,P.v2);} },
  (t,r,v)=>{ t.fill(P.b2); for(let k=0;k<7;k++){ const ph=r()*6.28, amp=2+r()*3, per=32/(1+Math.floor(r()*2)), yb=r()*16; for(let x=0;x<32;x++){ const y=Math.round(yb+Math.sin(x/per*6.28+ph)*amp); t.set(x,y,P.b5); t.set(x,y+1,P.b4); t.set(x,y+2,P.b3);} } for(let i=0;i<4+v;i++){ const x=Math.floor(r()*32),y=Math.floor(r()*16); t.set(x,y,P.n4); t.set(x+1,y,P.n3); } specks(t,r,2,[P.t3]); for(let x=0;x<32;x++){ t.set(x,0,P.t2); if(r()<0.6) t.set(x,1,P.n3);} });
sheet('terrain_orc_wasteland',(t,r,v)=>{ base(t,r,[P.r1,P.b2,P.b3],[0.3,0.7]); const vo=voronoi(r,32,32,5); for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const q=vo(x,y); if(q.d2-q.d1<0.9) t.set(x,y,P.b1);} for(let i=0;i<Math.round(2*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); t.set(x,y,P.g5); t.set(x+1,y,P.g4); t.set(x+2,y,P.g5); } for(let i=0;i<Math.round(1*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); for(let j=0;j<6;j++) t.set(x+Math.floor(r()*4),y+Math.floor(r()*3),P.DS);} },
  (t,r,v)=>{ strata(t,r,[P.r1,P.b2,P.s1],{bh:4}); for(let i=0;i<1+v%3;i++){ const x=Math.floor(r()*30),y=3+Math.floor(r()*10); t.set(x,y,P.g5); t.set(x+1,y,P.g4); t.set(x+2,y,P.g5);} });
sheet('terrain_ash',(t,r,v)=>{ base(t,r,[P.DS,P.g1,P.g2],[0.4,0.72]); specks(t,r,Math.round(18*K[v]),[P.g3],c=>c===P.g2||c===P.g1); specks(t,r,Math.round(3*K[v]),[P.r4,P.r3]); },
  (t,r,v)=>{ strata(t,r,[P.O,P.DS,P.g1],{bh:4}); specks(t,r,4,[P.g2]); if(v>1) specks(t,r,1,[P.r3]); });
sheet('terrain_snow',(t,r,v)=>{ base(t,r,[P.w4,P.w5,P.g5],[0.24,0.55],[16,8],[2,1]); for(let i=0;i<Math.round(3*K[v])+1;i++) ripple(t,Math.floor(r()*32),Math.floor(r()*32),6+Math.floor(r()*6),P.w5,1); specks(t,r,Math.round(4*K[v]),[P.g4]); },
  (t,r,v)=>{ strata(t,r,[P.g1,P.g2,P.g3],{bh:5,lip:[P.w4,P.w5,P.g5]}); for(let i=0;i<2+v;i++){ const x=Math.floor(r()*28),y=5+Math.floor(r()*8); for(let j=0;j<3+Math.floor(r()*3);j++) t.set(x+j,y,P.w5);} });
sheet('terrain_ice',(t,r,v)=>{ base(t,r,[P.w3,P.w4,P.w5],[0.25,0.66],[16,8],[2,1]); for(let i=0;i<2+v;i++) crackWalk(t,r,8+Math.floor(r()*6),P.w3,Math.floor(r()*32),Math.floor(r()*32)); for(let i=0;i<3+v;i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); for(let j=0;j<3;j++) t.set(x+j,y-j,P.g5);} },
  (t,r,v)=>{ for(let y=0;y<16;y++) for(let x=0;x<32;x++) t.set(x,y,y<2?P.w5:(x%9<2?P.w3:P.w4)); for(let i=0;i<3+v;i++){ let x=Math.floor(r()*32),y=2; for(let j=0;j<10;j++){ t.set(x,y+j,P.w2); if(r()<0.3) x+=r()<0.5?1:-1; } } specks(t,r,4,[P.w5]); });
sheet('terrain_swamp',(t,r,v)=>{ base(t,r,[P.b1,P.o1,P.n2],[0.36,0.66]); const n=fbm(r,32,32,[8,4],[2,1]); for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const q=n(x,y); if(q>0.68) t.set(x,y,q>0.74?P.o3:P.o2); if(q<0.26+0.04*v) t.set(x,y,P.DS);} specks(t,r,Math.round(5*K[v]),[P.o3,P.n4]); },
  (t,r,v)=>{ strata(t,r,[P.DS,P.b1,P.b2],{bh:4,lip:[P.o1,P.o2,P.o3]}); roots(t,r,2+v,P.b2,P.b1); });
sheet('terrain_seabed',(t,r,v)=>{ base(t,r,[P.b3,P.b4,P.b5],[0.3,0.72],[16,8],[2,1]); for(let i=0;i<Math.round(6*K[v]);i++) pebble(t,Math.floor(r()*32),Math.floor(r()*32),P.g4,P.g3,P.g2,r()<0.6); for(let i=0;i<Math.round(3*K[v]);i++){ const x=Math.floor(r()*32),y=Math.floor(r()*32); t.set(x,y,P.n4); t.set(x,y+1,P.n3); t.set(x+1,y+2,P.n3); t.set(x-1,y+1,P.n4);} },
  (t,r,v)=>{ strata(t,r,[P.b2,P.b3,P.b4],{bh:4}); cliffPebbles(t,r,4+v,P.g3,P.g2,P.g1,0); });

const bankCliff=(t,r,v)=>{ strata(t,r,[P.DS,P.b1,P.b2],{bh:4}); specks(t,r,5,[P.w2]); cliffPebbles(t,r,2+v,P.g3,P.g2,P.g1,0); };
// water: static base per variant; ripples bob ±1px, sparkles blink
animSheet('terrain_water',(t,row,f)=>{ const r=rng(77+row*11); BASE_SEED=4242+row*991; base(t,rng(5),[P.w2,P.w3],[0.42]); const rp=[]; for(let i=0;i<(row?9:7);i++) rp.push([Math.floor(r()*32),Math.floor(r()*32),4+Math.floor(r()*4),r()*4|0]);
  for(const [x,y,len,ph] of rp){ const o=[0,1,1,0][(f+ph)%4]; ripple(t,x+o,y,len,P.w4,1); }
  const sp=[]; for(let i=0;i<8;i++) sp.push([Math.floor(r()*32),Math.floor(r()*32),i%4]); for(const [x,y,ph] of sp){ if(ph===f) t.set(x,y,P.w5); if((ph+1)%4===f){ t.set(x,y,P.w5); t.set(x+1,y,P.w4); t.set(x-1,y,P.w4);} } }, bankCliff);
// lava: voronoi crust with molten seams pulsing
animSheet('terrain_lava',(t,row,f)=>{ const r=rng(311+row*17); const vo=voronoi(r,32,32,row?6:5); const pulse=[0,1,2,1][f];
  for(let y=0;y<32;y++) for(let x=0;x<32;x++){ const q=vo(x,y), e=q.d2-q.d1; const ph=(q.id+Math.floor((x+y)/6))%4; const hot=((ph+f)%4)<2;
    let c; if(e<0.8) c=hot?P.r5:P.r4; else if(e<1.8) c=hot?P.r4:P.r3; else if(e<2.6) c=(pulse>0&&hot)?P.r2:P.r1; else c=(q.d1<3)?P.g1:P.DS; t.set(x,y,c); }
  specks(t,rng(9+row+f),3,[P.O],c=>c===P.DS); }, (t,r,v)=>{ strata(t,r,[P.O,P.DS,P.g1],{bh:4}); for(let i=0;i<2+v;i++){ let x=Math.floor(r()*32),y=Math.floor(r()*6); for(let j=0;j<7;j++){ t.set(x,y+j,j%3===0?P.r5:P.r4); if(r()<0.5) x+=r()<0.5?1:-1; } } });
return T;
