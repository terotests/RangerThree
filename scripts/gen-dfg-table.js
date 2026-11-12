// Writes three/src/renderers/dfg_table.rs from the crate's own DFG integrator
// (needs build/rangerthree-core.js):  node scripts/gen-dfg-table.js
const vm=require("vm"),fs=require("fs");
const ctx={console}; vm.createContext(ctx);
vm.runInContext(fs.readFileSync("build/rangerthree-core.js","utf8")+";this.R=RangerThreeCore;",ctx);
const lut=ctx.R.RenderersPbr.dfg_lut(16,4096);
const rows=[]; for(let r=0;r<16;r++){ rows.push("        "+lut.slice(r*32,r*32+32).map(v=>{let s=v.toFixed(6); if(!s.includes(".")) s+=".0"; return s;}).join(", ")+","); }
const src=`//! The DFG table three.js-style metal/rough lighting needs, 16 x 16
//! (row = n·v, column = roughness), integrated by \`pbr::dfg_lut(16, 4096)\`
//! and stored so renderers need not integrate it at start-up. Regenerate
//! with scripts/gen-dfg-table.js after changing the integrator.

use ranger::prelude::*;

pub fn dfg_table() -> Vec<f64> {
    vec![
${rows.join("\n")}
    ]
}
`;
fs.writeFileSync("three/src/renderers/dfg_table.rs",src);
console.log("written", lut.length);
