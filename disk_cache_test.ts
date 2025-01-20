// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, assertRejects } from "@std/assert";
import { DiskCache } from "./disk_cache.ts";

Deno.test({
  name: "DiskCache.getCacheFilename()",
  async fn() {
    const testCases: [string, string | Error][] = [
      [
        "http://deno.land/std/http/file_server.ts",
        "http/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      [
        "http://localhost:8000/std/http/file_server.ts",
        "http/localhost_PORT8000/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      [
        "https://deno.land/std/http/file_server.ts",
        "https/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      ["wasm://wasm/d1c677ea", new Error(`Can't convert url`)],
      [
        "file://127.0.0.1/d$/a/1/s/format.ts",
        new Error(`Can't convert url`),
      ],
    ];

    for (const [fixture, expected] of testCases) {
      if (expected instanceof Error) {
        await assertRejects(
          async () => await DiskCache.getCacheFilename(new URL(fixture)),
          Error,
          expected.message,
        );
        continue;
      } else {
        assertEquals(
          await DiskCache.getCacheFilename(new URL(fixture)),
          expected,
        );
      }
    }
  },
});
