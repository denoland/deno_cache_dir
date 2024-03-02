// Copyright 2018-2024 the Deno authors. MIT license.

export { assertEquals, assertRejects } from "@std/assert";
export { createGraph } from "@deno/graph";

export async function withTempDir(
  action: (path: string) => Promise<void> | void,
) {
  const tempDir = Deno.makeTempDirSync();
  try {
    await action(tempDir);
  } finally {
    Deno.removeSync(tempDir, { recursive: true });
  }
}
