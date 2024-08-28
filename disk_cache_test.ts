// Copyright 2018-2024 the Deno authors. MIT license.

import { assertEquals, assertRejects } from "@std/assert";
import type { RequestDestination } from "@deno/graph";
import { DiskCache } from "./disk_cache.ts";

Deno.test({
  name: "DiskCache.getCacheFilename()",
  async fn() {
    const testCases: [string, RequestDestination, string | Error][] = [
      [
        "http://deno.land/std/http/file_server.ts",
        "script",
        "http/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      [
        "http://localhost:8000/std/http/file_server.ts",
        "script",
        "http/localhost_PORT8000/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      [
        "https://deno.land/std/http/file_server.ts",
        "script",
        "https/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ],
      [
        // json, but with script
        "https://deno.land/std/http/file_server.json",
        "script",
        "https/deno.land/57bca9ce6cfb71130ac9ae61b8ba4b277d9379077c15bece949c025df2fa86cf",
      ],
      [
        // json
        "https://deno.land/std/http/file_server.json",
        "json",
        "https/deno.land/df822def4e5e60d274b133fe0c610583f3b96af9cf87edf3c2184c6613501609",
      ],
      [
        "wasm://wasm/d1c677ea",
        "script",
        new Error(`Can't convert url`),
      ],
      [
        "file://127.0.0.1/d$/a/1/s/format.ts",
        "script",
        new Error(`Can't convert url`),
      ],
    ] as const;

    for (const [fixture, destination, expected] of testCases) {
      if (expected instanceof Error) {
        await assertRejects(
          async () =>
            await DiskCache.getCacheFilename(new URL(fixture), destination),
          Error,
          expected.message,
        );
        continue;
      } else {
        assertEquals(
          await DiskCache.getCacheFilename(new URL(fixture), destination),
          expected,
        );
      }
    }
  },
});
