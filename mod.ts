// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { FetchCacher } from "./cache.ts";
import type { CacheType } from "./cache.ts";
import type { CacheInfo, LoadResponse } from "./deps.ts";
import { DenoDir } from "./deno_dir.ts";
import { FileFetcher } from "./file_fetcher.ts";
import type { CacheSetting } from "./file_fetcher.ts";

export { FetchCacher } from "./cache.ts";
export type { CacheType } from "./cache.ts";
export { DenoDir } from "./deno_dir.ts";
export { FileFetcher } from "./file_fetcher.ts";
export type { CacheSetting } from "./file_fetcher.ts";

export interface Loader {
  /** A function that can be passed to a `deno_graph` building function to
   * provide information about the cache to populate the output.
   */
  cacheInfo(specifier: string): CacheInfo;
  /** A function that can be passed to a `deno_graph` that will load and cache
   * dependencies in the graph in the disk cache.
   */
  load(specifier: string): Promise<LoadResponse | undefined>;
}

export interface Cacher {
  /** Retrieve a specific type of cached resource from the disk cache. */
  get(type: CacheType, specifier: string): string | undefined;
  /** Set a specific type of cached resource to the disk cache. */
  set(type: CacheType, specifier: string, value: string): void;
}

export interface CacheOptions {
  allowRemote?: boolean;
  cacheSetting?: CacheSetting;
  root?: string;
}

/**
 * Creates a cache object that allows access to the internal `DENO_DIR` cache
 * structure for remote dependencies and cached output of emitted modules.
 */
export function createCache(
  { root, cacheSetting = "use", allowRemote = true }: CacheOptions = {},
): Loader & Cacher {
  const denoDir = new DenoDir(root);
  const fileFetcher = new FileFetcher(denoDir.deps, cacheSetting, allowRemote);
  return new FetchCacher(denoDir.gen, denoDir.deps, fileFetcher);
}
