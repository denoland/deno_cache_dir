// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { isAbsolute, join, normalize } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import { cacheDir, homeDir } from "./dirs.ts";
import { HttpCache } from "./http_cache.ts";
import { assert } from "./util.ts";

await Deno.permissions.request({ name: "env", variable: "DENO_DIR" });
await Deno.permissions.request({ name: "read" });

export class DenoDir {
  deps: HttpCache;
  gen: DiskCache;
  root: string;

  constructor(root?: string) {
    if (root) {
      if (!isAbsolute(root)) {
        root = normalize(join(Deno.cwd(), root));
      }
    } else {
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
    Deno.permissions.request({ name: "write", path: root });
    this.root = root;
    this.deps = new HttpCache(join(root, "deps"));
    this.gen = new DiskCache(join(root, "gen"));
  }
}
