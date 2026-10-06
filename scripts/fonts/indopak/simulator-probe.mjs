import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  addFont,
  assertSpecimens,
  controlsReady,
  diagnosticFont,
  fontAdjustmentEvidence,
  inspectSpecimens,
  layoutOverflow,
  loadCorpus,
  root,
} from "./deep-browser-shared.mjs";

const inputs = process.env.INDOPAK_DEEP_OUTPUT ?? path.join(root, ".cache/indopak-deep");
const corpus = await loadCorpus(inputs);
const fonts = [];
for (const kind of ["Ring", "Digits", "Upstream"]) {
  const font = await diagnosticFont(inputs, kind);
  if (kind === "Upstream") font.sizeAdjust = "100%";
  fonts.push(font);
}
const modulePath = path.join(root, "web/.svelte-kit/indopak-simulator-probe.mjs");
const functions = [
  addFont,
  assertSpecimens,
  controlsReady,
  fontAdjustmentEvidence,
  inspectSpecimens,
  layoutOverflow,
]
  .map((fn) => fn.toString())
  .join("\n");
const source = `const corpus=${JSON.stringify(corpus)};
const fonts=${JSON.stringify(fonts)};
const ORNAMENT_MAX_EM=1.15;
const assert={
equal(a,b,message){if(!Object.is(a,b))throw new Error(message||String(a)+' !== '+String(b));},
ok(value,message){if(!value)throw new Error(message||'Assertion failed');},
deepEqual(a,b,message){if(JSON.stringify(a)!==JSON.stringify(b))throw new Error(message||'Unequal values');}
};
${functions}
export async function run(label){
 const report={label,status:'running',transport:'Browser probe injected by dev-only audit server; simulator navigation through CUA',date:new Date().toISOString(),url:location.href,user_agent:navigator.userAgent,viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,matrices:[]};
 async function save(){await fetch('/__indopak_simulator_report',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(report)});}
 await save();
 try{
 const deadline=Date.now()+120000;
 const expectedLandscape=label.includes('-landscape-');
 const expectedPortrait=label.includes('-portrait-');
 function orientationReady(){if(expectedLandscape)return innerWidth>innerHeight;if(expectedPortrait)return innerWidth<innerHeight;return true;}
 while(!orientationReady()||!document.querySelector('select')||!controlsReady()||!document.querySelector('[data-indopak-ayah]')||getComputedStyle(document.querySelector('[data-indopak-ayah]')).visibility!=='visible'){
 assert.ok(Date.now()<deadline,'Renderer readiness timeout');await new Promise(resolve=>setTimeout(resolve,100));
 }
 report.viewport=[innerWidth,innerHeight];report.orientation=innerWidth>innerHeight?'landscape':'portrait';
 await document.fonts.ready;
 for(const font of fonts)await addFont(font);
 report.font_adjustment=await fontAdjustmentEvidence(corpus.fontReference);
 assert.ok(report.font_adjustment.samples.every(sample=>sample.honoured),'size-adjust mismatch');
 const select=[...document.querySelectorAll('label')];
 function choose(label,value){const element=select.find(item=>item.textContent.trim().startsWith(label)).querySelector('select');element.value=String(value);element.dispatchEvent(new Event('change',{bubbles:true}));}
 for(const size of [22,24,33,48,56])for(const width of [320,640,960]){
 choose('Font size',size);choose('Run width',width);
 await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
 const specimens=inspectSpecimens({requireInk:true,fontReference:corpus.fontReference});
 const summary=assertSpecimens(specimens,corpus,false,true,false);
 const overflow=layoutOverflow();assert.equal(overflow.page_overflow,false);
 assert.ok(overflow.run_overflow.every(key=>['12:21','18:110','56:23'].includes(key)),'Unexpected overflow');
 const ringFailures=specimens.filter(item=>item.ring_check&&item.ring_check.outside_pixels>0).map(item=>({key:item.key,...item.ring_check}));
 assert.deepEqual(ringFailures,[],'Canvas ring containment');
 assert.ok(orientationReady(),'Orientation changed during matrix');
 report.matrices.push({size,width,viewport:[innerWidth,innerHeight],actual_columns:[...new Set(specimens.map(item=>item.container_width))],specimens:summary.specimens,private_occurrences:summary.private_occurrences,end_clusters:summary.end_clusters,ring_checks:summary.ring_checks,...overflow});
 await save();
 }
 choose('Font size',33);choose('Run width',320);
 const response=await fetch('/fonts/indopak-reader-compat-v4.woff2');report.font_bytes=(await response.arrayBuffer()).byteLength;assert.equal(response.status,200);assert.equal(report.font_bytes,92416);
 report.status='geometry_checks_passed';report.dom_ink_mask_analysis='not_performed';
 }catch(error){report.status='failed';report.error=String(error.stack||error);}
 window.indopakSimulatorReport=report;
 await save();
 console.log(JSON.stringify({label:report.label,status:report.status,error:report.error,matrices:report.matrices.length,viewport:report.viewport,dpr:report.dpr}));
 return report.status;
}
`;
await mkdir(path.dirname(modulePath), { recursive: true });
await writeFile(modulePath, source);
console.log(`import('/@fs${modulePath}').then(module=>module.run('iphone-portrait-reading'))`);
