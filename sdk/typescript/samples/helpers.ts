import path from "node:path";

export function maplecodePathOverride() {
  return (
    process.env.MAPLECODE_EXECUTABLE ??
    path.join(process.cwd(), "..", "..", "maplecode-rs", "target", "debug", "maplecode")
  );
}
