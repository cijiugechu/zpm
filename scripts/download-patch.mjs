import fs from 'fs/promises';
import path from 'path';

const name = process.argv[2];

const res = await fetch(`https://raw.githubusercontent.com/yarnpkg/berry/master/packages/plugin-compat/sources/patches/${name}.patch.ts`);
const source = await res.text();

const payload = source.match(/brotliDecompressSync\((Buffer\.from\(.*?, `base64`\))\)/)[1];
const buffer = eval(`(${payload})`);

await fs.writeFile(path.join(import.meta.dirname, `../packages/zpm/patches/${name}.brotli.dat`), buffer);
