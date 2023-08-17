// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { assertEquals, createGraph } from "./deps_test.ts";
import { join } from "./deps.ts";
import { createCache } from "./mod.ts";
import { assert } from "./util.ts";

Deno.test({
  name: "createCache()",
  async fn() {
    const cache = createCache();
    const { load } = cache;
    for (let i = 0; i < 2; i++) {
      const graph = await createGraph(
        "https://deno.land/x/oak@v10.5.1/mod.ts",
        {
          load,
        },
      );
      assertEquals(graph.modules.length, 59);
    }
  },
});

Deno.test({
  name: "createCache() - local vendor folder",
  async fn() {
    await withTempDir(async (tempDir) => {
      const vendorRoot = join(tempDir, "vendor");
      const cache = createCache({
        vendorRoot,
      });

      for (let i = 0; i < 2; i++) {
        const { load } = cache;
        const graph = await createGraph(
          "https://deno.land/x/oak@v10.5.1/mod.ts",
          {
            load,
          },
        );
        assertEquals(graph.modules.length, 59);
        assert(Deno.statSync(vendorRoot).isDirectory);
        assert(
          Deno.statSync(join(vendorRoot, "deno.land", "x", "oak@v10.5.1"))
            .isDirectory,
        );
      }
    });
  },
});

async function withTempDir(fn: (tempDir: string) => Promise<void>) {
  const tempDir = Deno.makeTempDirSync();
  try {
    return await fn(tempDir);
  } finally {
    Deno.removeSync(tempDir, { recursive: true });
  }
}
