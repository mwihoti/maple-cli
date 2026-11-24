import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { afterEach, beforeEach } from "@jest/globals";

const originalMapleCodeHome = process.env.MAPLECODE_HOME;
let currentMapleCodeHome: string | undefined;

beforeEach(async () => {
  currentMapleCodeHome = await fs.mkdtemp(path.join(os.tmpdir(), "maplecode-sdk-test-"));
  process.env.MAPLECODE_HOME = currentMapleCodeHome;
});

afterEach(async () => {
  const maplecodeHomeToDelete = currentMapleCodeHome;
  currentMapleCodeHome = undefined;

  if (originalMapleCodeHome === undefined) {
    delete process.env.MAPLECODE_HOME;
  } else {
    process.env.MAPLECODE_HOME = originalMapleCodeHome;
  }

  if (maplecodeHomeToDelete) {
    await fs.rm(maplecodeHomeToDelete, { recursive: true, force: true });
  }
});
