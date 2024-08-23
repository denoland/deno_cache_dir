// Copyright 2018-2024 the Deno authors. MIT license.

import { assertEquals, assertThrows } from "@std/assert";
import { DenoDir } from "./deno_dir.ts";
import { withTempDir } from "./deps_test.ts";
import { RequestDestination } from "./http_cache.ts";

Deno.test({
  name: "DenoDir - basic",
  async fn() {
    const expectedText =
      `// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.

/**
 * Ported mostly from https://github.com/browserify/path-browserify/
 * This module is browser compatible.
 * @module
 */

import { isWindows } from "../_util/os.ts";
import * as _win32 from "./win32.ts";
import * as _posix from "./posix.ts";

const path = isWindows ? _win32 : _posix;

export const win32 = _win32;
export const posix = _posix;
export const {
  basename,
  delimiter,
  dirname,
  extname,
  format,
  fromFileUrl,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toFileUrl,
  toNamespacedPath,
} = path;

export * from "./common.ts";
export { SEP, SEP_PATTERN } from "./separator.ts";
export * from "./_interface.ts";
export * from "./glob.ts";
`;
    const denoDir = new DenoDir();
    const url = new URL("https://deno.land/std@0.140.0/path/mod.ts");
    const expectedHeaders = {
      "content-type": "application/typescript",
    };
    const deps = await denoDir.createHttpCache();
    deps.set(
      url,
      RequestDestination.Script,
      expectedHeaders,
      new TextEncoder().encode(expectedText),
    );
    const headers = deps.getHeaders(url, RequestDestination.Script)!;
    assertEquals(headers, expectedHeaders);
    const cacheEntry = deps.get(url, RequestDestination.Script)!;
    assertEquals(cacheEntry.headers, expectedHeaders);
    const text = new TextDecoder().decode(cacheEntry.content);
    assertEquals(text, expectedText);

    // ok
    deps.get(
      url,
      RequestDestination.Script,
      {
        checksum:
          "d3e68d0abb393fb0bf94a6d07c46ec31dc755b544b13144dee931d8d5f06a52d",
      },
    );
    // not ok
    assertThrows(() =>
      deps.get(url, RequestDestination.Script, {
        checksum: "invalid",
      })
    );
  },
});

Deno.test({
  name: "HttpCache - global cache - get",
  async fn() {
    const denoDir = new DenoDir();
    const url = new URL("https://deno.land/std@0.140.0/path/mod.ts");
    const deps = await denoDir.createHttpCache();
    // disallow will still work because we're using a global cache
    // which is not affected by this option
    const entry = await deps.get(url, RequestDestination.Script);
    assertEquals(entry!.content.length, 820);
  },
});

Deno.test({
  name: "HttpCache - local cache- allowCopyGlobalToLocal",
  async fn() {
    await withTempDir(async (tempDir) => {
      const denoDir = new DenoDir();
      const url = new URL("https://deno.land/std@0.140.0/path/mod.ts");

      // disallow copy from global to local because readonly
      {
        using deps = await denoDir.createHttpCache({
          vendorRoot: tempDir,
          readOnly: true,
        });
        const text = deps.get(url, RequestDestination.Script);
        assertEquals(text, undefined);
      }
      // this should be fine though
      {
        using deps = await denoDir.createHttpCache({
          vendorRoot: tempDir,
        });
        const entry = deps.get(url, RequestDestination.Script);
        assertEquals(entry!.content.length, 820);
      }
    });
  },
});
