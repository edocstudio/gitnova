// Keep candidate verification memory bounded even for large debug installers.
import {openSync,readSync,closeSync} from 'node:fs';
import {createHash} from 'node:crypto';
export function hashFile(path) {
  const fd=openSync(path,'r'),buffer=Buffer.allocUnsafe(1024*1024),hash=createHash('sha256');
  try {let size;while((size=readSync(fd,buffer,0,buffer.length,null))!==0)hash.update(buffer.subarray(0,size));return hash.digest('hex');}
  finally {closeSync(fd);}
}
