// Copyright 2018-2024 the Deno authors. MIT license.

/**
 * A module which provides a TypeScript implementation of the Deno CLI's cache
 * directory logic (`DENO_DIR`). This can be used in combination with other
 * modules to provide user loadable APIs that are like the Deno CLI's
 * functionality.
 *
 * This also can provide user read access in Deploy to a Deno CLI's cache when
 * the cache is checked into the repository.
 *
 * ### Example
 *
 * ```ts
 * import { createCache } from "@deno/cache-dir";
 * import { createGraph } from "@deno/graph";
 *
 * // create a cache where the location will be determined environmentally
 * const cache = createCache();
 * // destructuring the two functions we need to pass to the graph
 * const { cacheInfo, load } = cache;
 * // create a graph that will use the cache above to load and cache dependencies
 * const graph = await createGraph("https://deno.land/x/oak@v9.0.1/mod.ts", {
 *   cacheInfo,
 *   load,
 * });
 *
 * // log out the console a similar output to `deno info` on the command line.
 * console.log(graph.toString());
 * ```
 *
 * @module
 */

import { FetchCacher } from "./cache.ts";
import { type CacheInfo, type LoadResponse } from "./deps.ts";
import { DenoDir } from "./deno_dir.ts";
import { type CacheSetting, FileFetcher } from "./file_fetcher.ts";

export { FetchCacher } from "./cache.ts";
export { DenoDir } from "./deno_dir.ts";
export { type CacheSetting, FileFetcher } from "./file_fetcher.ts";

export interface Loader {
  /** A function that can be passed to a `deno_graph` building function to
   * provide information about the cache to populate the output.
   */
  cacheInfo?(specifier: string): CacheInfo;
  /** A function that can be passed to a `deno_graph` that will load and cache
   * dependencies in the graph in the disk cache.
   */
  load(
    specifier: string,
    isDynamic?: boolean,
    cacheSetting?: CacheSetting,
    checksum?: string,
  ): Promise<LoadResponse | undefined>;
}

export type {
  LoadResponse,
  LoadResponseExternal,
  LoadResponseModule,
} from "./deps.ts";

export interface CacheOptions {
  /** Allow remote URLs to be fetched if missing from the cache. This defaults
   * to `true`. Setting it to `false` is like passing the `--no-remote` in the
   * Deno CLI, meaning that any modules not in cache error. */
  allowRemote?: boolean;
  /** Determines how the cache will be used. The default value is `"use"`
   * meaning the cache will be used, and any remote module cache misses will
   * be fetched and stored in the cache. */
  cacheSetting?: CacheSetting;
  /** This forces the cache into a `readOnly` mode, where fetched resources
   * will not be stored on disk if `true`. The default is detected from the
   * environment, checking to see if `Deno.writeFile` exists. */
  readOnly?: boolean;
  /** Specifies a path to the root of the cache. Setting this value overrides
   * the detection of location from the environment. */
  root?: string | URL;
  /** Specifies a path to the local vendor directory if it exists. */
  vendorRoot?: string | URL;
}

/**
 * Creates a cache object that allows access to the internal `DENO_DIR` cache
 * structure for remote dependencies and cached output of emitted modules.
 */
export function createCache({
  root,
  cacheSetting = "use",
  allowRemote = true,
  readOnly,
  vendorRoot,
}: CacheOptions = {}): Loader {
  const denoDir = new DenoDir(root);
  const fileFetcher = new FileFetcher(
    () => {
      return denoDir.createHttpCache({
        readOnly,
        vendorRoot,
      });
    },
    cacheSetting,
    allowRemote,
  );
  return new FetchCacher(fileFetcher);
}
