// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./deps_test.ts";
import { DenoDir } from "./deno_dir.ts";
import { assert } from "./util.ts";

Deno.test({
  name: "DenoDir - basic",
  async fn() {
    const denoDir = new DenoDir();
    const url = new URL("https://deno.land/std@0.140.0/path/mod.ts");
    const deps = denoDir.createHttpCache();
    const headers = (await deps.getHeaders(url))!;
    assert(Object.keys(headers).length > 10);
    const text = (await deps.getText(url))!;
    assertEquals(
      text,
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
`,
    );
  },
});
