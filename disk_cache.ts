// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import {
  dirname,
  ensureDir,
  fromFileUrl,
  isAbsolute,
  join,
  readAll,
  sep,
  writeAll,
} from "./deps.ts";
import { assert, CACHE_PERM, urlToFilename } from "./util.ts";

export class DiskCache {
  location: string;

  constructor(location: string) {
    assert(isAbsolute(location));
    this.location = location;
  }

  async get(filename: string): Promise<Uint8Array> {
    const path = join(this.location, filename);
    const file = await Deno.open(path, { read: true });
    const value = await readAll(file);
    file.close();
    return value;
  }

  async set(filename: string, data: Uint8Array): Promise<void> {
    const path = join(this.location, filename);
    const parentFilename = dirname(path);
    await ensureDir(parentFilename);
    const file = await Deno.open(path, {
      write: true,
      create: true,
      mode: CACHE_PERM,
    });
    await writeAll(file, data);
    file.close();
  }

  static getCacheFilename(url: URL): string | undefined {
    const out: string[] = [];
    const scheme = url.protocol.replace(":", "");
    out.push(scheme);

    switch (scheme) {
      case "wasm": {
        const { hostname, port } = url;
        out.push(port ? `${hostname}_PORT${port}` : hostname);
        out.push(...url.pathname.split("/"));
        break;
      }
      case "http":
      case "https":
      case "data":
      case "blob":
        return urlToFilename(url);
      case "file": {
        const path = fromFileUrl(url);
        if (!path) {
          return undefined;
        }
        const { host } = url;
        if (host) {
          out.push("UNC");
          out.push(host.replaceAll(":", "_"));
        }
        const pathComponents = path.split(sep).filter((p) => p.length > 0);
        if (Deno.build.os === "windows") {
          if (host) {
            // windows will have the host in the result of fromFileUrl, so remove it
            pathComponents.shift();
          }

          const first = pathComponents.shift();
          assert(first);
          out.push(first.replace(/:$/, ""));
        }
        out.push(...pathComponents);
        break;
      }
      default:
        return undefined;
    }
    return join(...out);
  }

  static getCacheFilenameWithExtension(
    url: URL,
    extension: string,
  ): string | undefined {
    const base = this.getCacheFilename(url);
    return base ? `${base}.${extension}` : undefined;
  }
}
