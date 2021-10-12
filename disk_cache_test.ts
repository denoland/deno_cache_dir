// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./deps_test.ts";
import { DiskCache } from "./disk_cache.ts";

Deno.test({
  name: "DiskCache.getCacheFilename()",
  fn() {
    const testCases = [
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
      ["wasm://wasm/d1c677ea", "wasm/wasm/d1c677ea"],
      [
        "file://127.0.0.1/d$/a/1/s/format.ts",
        "file/UNC/127.0.0.1/d$/a/1/s/format.ts",
      ],
      [
        "file://[0:0:0:0:0:0:0:1]/d$/a/1/s/format.ts",
        "file/UNC/[__1]/d$/a/1/s/format.ts",
      ],
      [
        "file://comp/t-share/a/1/s/format.ts",
        "file/UNC/comp/t-share/a/1/s/format.ts",
      ],
      ["file:///std/http/file_server.ts", "file/std/http/file_server.ts"],
    ];

    if (Deno.build.os === "windows") {
      testCases.push(["file:///D:/a/1/s/format.ts", "file/D/a/1/s/format.ts"]);
    }

    for (const [fixture, expected] of testCases) {
      assertEquals(DiskCache.getCacheFilename(new URL(fixture)), expected);
    }
  },
});
