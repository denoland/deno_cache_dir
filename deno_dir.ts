// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { isAbsolute, join, normalize } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import { cacheDir, homeDir } from "./dirs.ts";
import { HttpCache } from "./http_cache.ts";
import { assert } from "./util.ts";

export class DenoDir {
  deps: HttpCache;
  gen: DiskCache;
  root: string;

  constructor(root?: string | URL, readOnly?: boolean) {
    if (root) {
      if (root instanceof URL) {
        root = root.toString();
      }
      if (!isAbsolute(root)) {
        root = normalize(join(Deno.cwd(), root));
      }
    } else {
      Deno.permissions.request({ name: "env", variable: "DENO_DIR" });
      const dd = Deno.env.get("DENO_DIR");
      if (dd) {
        if (!isAbsolute(dd)) {
          root = normalize(join(Deno.cwd(), dd));
        } else {
          root = dd;
        }
      } else {
        const cd = cacheDir();
        if (cd) {
          root = join(cd, "deno");
        } else {
          const hd = homeDir();
          if (hd) {
            root = join(hd, ".deno");
          }
        }
      }
    }
    assert(root, "Could not set the Deno root directory");
    assert(isAbsolute(root), `The root directory "${root}" is not absolute.`);
    Deno.permissions.request({ name: "read" });
    this.root = root;
    this.deps = new HttpCache(join(root, "deps"), readOnly);
    this.gen = new DiskCache(join(root, "gen"));
  }
}
