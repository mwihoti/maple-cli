import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { afterEach, beforeEach } from "@jest/globals";

const originalMaplecodeHome = process.env.MAPLECODE_HOME;
let currentMaplecodeHome: string | undefined;

beforeEach(async () => {
  currentMaplecodeHome = await fs.mkdtemp(path.join(os.tmpdir(), "maplecode-sdk-test-"));
  process.env.MAPLECODE_HOME = currentMaplecodeHome;
});

afterEach(async () => {
  const maplecodeHomeToDelete = currentMaplecodeHome;
  currentMaplecodeHome = undefined;

  if (originalMaplecodeHome === undefined) {
    delete process.env.MAPLECODE_HOME;
  } else {
    process.env.MAPLECODE_HOME = originalMaplecodeHome;
  }

  if (maplecodeHomeToDelete) {
    await fs.rm(maplecodeHomeToDelete, { recursive: true, force: true });
  }
});
