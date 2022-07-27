// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join } from "./deps.ts";
import type { CacheInfo, LoadResponse } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import type { FileFetcher } from "./file_fetcher.ts";
import type { HttpCache } from "./http_cache.ts";
import { isFileSync } from "./util.ts";

/** Provides an interface to Deno's CLI cache.
 *
 * It is better to use the {@linkcode createCache} function directly. */
export class FetchCacher {
  #diskCache: DiskCache;
  #fileFetcher: FileFetcher;
  #httpCache: HttpCache;
  #readOnly!: boolean;

  constructor(
    diskCache: DiskCache,
    httpCache: HttpCache,
    fileFetcher: FileFetcher,
    readOnly?: boolean,
  ) {
    this.#diskCache = diskCache;
    this.#fileFetcher = fileFetcher;
    this.#httpCache = httpCache;
    if (readOnly === undefined) {
      (async () => {
        this.#readOnly =
          (await Deno.permissions.query({ name: "write" })).state === "denied";
      })();
    } else {
      this.#readOnly = readOnly;
    }
  }

  /** Provides information about the state of the cache, which is used by
   * things like [`deno_graph`](https://deno.land/x/deno_graph) to enrich the
   * information about a module graph. */
  cacheInfo = (specifier: string): CacheInfo => {
    // when we are "read-only" (e.g. Deploy) we can access sync versions of APIs
    // so we can't return the cache info synchronously.
    if (this.#readOnly) {
      return {};
    }
    const url = new URL(specifier);
    const local = this.#httpCache.getCacheFilename(url);
    const emitCache = DiskCache.getCacheFilenameWithExtension(url, "js");
    const emit = emitCache
      ? join(this.#diskCache.location, emitCache)
      : undefined;
    return {
      local: isFileSync(local) ? local : undefined,
      emit: emit && isFileSync(emit) ? emit : undefined,
    };
  };

  load = (specifier: string): Promise<LoadResponse | undefined> => {
    const url = new URL(specifier);
    return this.#fileFetcher.fetch(url);
  };
}
