// Copyright 2018-2024 the Deno authors. MIT license.

import { isAbsolute, join, normalize, resolve } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import { cacheDir, homeDir } from "./dirs.ts";
import { HttpCache } from "./http_cache.ts";
import { assert } from "./util.ts";

export class DenoDir {
  readonly root: string;

  constructor(root?: string | URL) {
    const resolvedRoot = DenoDir.tryResolveRootPath(root);
    assert(resolvedRoot, "Could not set the Deno root directory");
    assert(
      isAbsolute(resolvedRoot),
      `The root directory "${resolvedRoot}" is not absolute.`,
    );
    Deno.permissions.request({ name: "read", path: resolvedRoot });
    this.root = resolvedRoot;
  }

  createGenCache(): DiskCache {
    return new DiskCache(join(this.root, "gen"));
  }

  createHttpCache(
    options?: { vendorRoot?: string | URL; readOnly?: boolean },
  ): Promise<HttpCache> {
    return HttpCache.create({
      root: join(this.root, "deps"),
      vendorRoot: options?.vendorRoot == null
        ? undefined
        : resolvePathOrUrl(options.vendorRoot),
      readOnly: options?.readOnly,
    });
  }

  static tryResolveRootPath(
    root: string | URL | undefined,
  ): string | undefined {
    if (root) {
      root = resolvePathOrUrl(root);
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
    return root;
  }
}

function resolvePathOrUrl(path: URL | string) {
  if (path instanceof URL) {
    path = path.toString();
  }
  return resolve(path);
}
