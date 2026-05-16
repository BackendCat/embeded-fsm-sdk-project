// Copy `.fsm` test fixtures into `out/test/fixtures/` after `tsc` (which
// only emits `.ts` -> `.js`). The Extension-Host suite reads fixtures
// relative to the compiled test dir; without this they are absent and the
// suiteSetup fails (zero extra npm dep — plain Node fs).

import { cpSync, mkdirSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, "src", "test", "fixtures");
const dst = join(here, "out", "test", "fixtures");

mkdirSync(dst, { recursive: true });
cpSync(src, dst, { recursive: true });
console.log(`copy-fixtures: ${src} -> ${dst}`);
