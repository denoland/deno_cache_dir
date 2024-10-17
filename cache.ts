// Copyright 2018-2024 the Deno authors. MIT license.

import type { LoadResponse } from "@deno/graph";
import type { CacheSetting, FileFetcher } from "./file_fetcher.ts";

/** Provides an interface to Deno's CLI cache.
 *
 * It is better to use the {@linkcode createCache} function directly. */
export class FetchCacher {
  #fileFetcher: FileFetcher;

  constructor(fileFetcher: FileFetcher) {
    this.#fileFetcher = fileFetcher;
  }

  // this should have the same interface as deno_graph's loader
  load = (
    specifier: string,
    _isDynamic?: boolean,
    cacheSetting?: CacheSetting,
    checksum?: string,
  ): Promise<LoadResponse | undefined> => {
    const url = new URL(specifier);
    return this.#fileFetcher.fetchOnce(url, { cacheSetting, checksum })
      .catch((e) => {
        if (e instanceof Deno.errors.NotFound) {
          return undefined;
        }

        throw new Error("FetchCacher#load failed", { cause: e });
      });
  };
}
