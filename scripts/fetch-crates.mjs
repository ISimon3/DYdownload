// Download locked crates through Node's TLS stack when Windows Schannel cannot reach crates.io.
// Cargo still validates and builds the original registry packages; registry settings are unchanged.
import { readFile, access, mkdir, writeFile, rename } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
const lock = await readFile(new URL('../src-tauri/Cargo.lock', import.meta.url), 'utf8');
const cache = join(process.env.CARGO_HOME || join(homedir(), '.cargo'), 'registry/cache/index.crates.io-1949cf8c6b5b557f');
await mkdir(cache,{recursive:true});
const packages = lock.split('[[package]]').slice(1).filter(b=>b.includes('registry+https://github.com/rust-lang/crates.io-index')).map(b=>({name:b.match(/\nname = "([^"]+)"/)[1],version:b.match(/\nversion = "([^"]+)"/)[1],checksum:b.match(/\nchecksum = "([^"]+)"/)[1]}));
let downloaded=0, failures=0;
async function worker(){while(packages.length){const p=packages.pop();const filename=`${p.name}-${p.version}.crate`;const destination=join(cache,filename);try{await access(destination);continue;}catch{}try{const r=await fetch(`https://static.crates.io/crates/${p.name}/${filename}`,{signal:AbortSignal.timeout(60000)});if(!r.ok)throw Error(String(r.status));const data=Buffer.from(await r.arrayBuffer());if(createHash('sha256').update(data).digest('hex')!==p.checksum)throw Error('checksum mismatch');await writeFile(destination+'.tmp',data);await rename(destination+'.tmp',destination);downloaded++;if(downloaded%20===0)console.log(`Downloaded ${downloaded} verified crates`);}catch(e){console.error(`${filename}: ${e.message}`);failures++;}}}
await Promise.all(Array.from({length:6},worker));console.log({downloaded,failures});if(failures)process.exitCode=1;
