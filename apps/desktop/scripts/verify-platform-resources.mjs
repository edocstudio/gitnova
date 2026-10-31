import {resolve} from 'node:path';
import {verifyResourceDirectory} from './verify-bundle-licenses.mjs';
if(process.argv.length!==3)throw new Error('Pass the installed resource directory containing licenses/.');
console.log(JSON.stringify(verifyResourceDirectory(resolve(process.argv[2]))));
