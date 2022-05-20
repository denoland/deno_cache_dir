// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join } from "./deps.ts";
import type { CacheInfo, LoadResponse } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import type { FileFetcher } from "./file_fetcher.ts";
import type { HttpCache } from "./http_cache.ts";
import { isFile, isFileSync } from "./util.ts";

/** The type of cache information that should be set or retrieved from the
 * cache. */
export type CacheType =
  | "declaration"
  | "emit"
  | "sourcemap"
  | "buildinfo"
  | "version";

const decoder = new TextDecoder();
const encoder = new TextEncoder();

interface EmitMetadata {
  // deno-lint-ignore camelcase
  version_hash: string;
}

/** Provides an interface to Deno's CLI cache.
 *
 * It is better to use the {@linkcode createCache} function directly. */
export class FetchCacher {
  #diskCache: DiskCache;
  #fileFetcher: FileFetcher;
  #httpCache: HttpCache;
  #readOnly!: boolean;

  async #getEmitMetadata(specifier: URL): Promise<EmitMetadata | undefined> {
    const filename = DiskCache.getCacheFilenameWithExtension(specifier, "meta");
    if (!filename || !(await isFile(filename))) {
      return undefined;
    }
    const bytes = await this.#diskCache.get(filename);
    return JSON.parse(decoder.decode(bytes));
  }

  async #setEmitMetadata(specifier: URL, data: EmitMetadata): Promise<void> {
    const filename = DiskCache.getCacheFilenameWithExtension(specifier, "meta");
    if (!filename) {
      return;
    }
    const bytes = encoder.encode(JSON.stringify(data));
    await this.#diskCache.set(filename, bytes);
  }

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
    const mapCache = DiskCache.getCacheFilenameWithExtension(url, "js.map");
    const emit = emitCache
      ? join(this.#diskCache.location, emitCache)
      : undefined;
    const map = mapCache ? join(this.#diskCache.location, mapCache) : undefined;
    return {
      local: isFileSync(local) ? local : undefined,
      emit: emit && isFileSync(emit) ? emit : undefined,
      map: map && isFileSync(map) ? map : undefined,
    };
  };

  async get(type: CacheType, specifier: string): Promise<string | undefined> {
    const url = new URL(specifier);
    let extension: string;
    switch (type) {
      case "declaration":
        extension = "d.ts";
        break;
      case "emit":
        extension = "js";
        break;
      case "sourcemap":
        extension = "js.map";
        break;
      case "buildinfo":
        extension = "buildinfo";
        break;
      case "version": {
        const data = await this.#getEmitMetadata(url);
        return data ? data.version_hash : undefined;
      }
    }
    const filename = DiskCache.getCacheFilenameWithExtension(url, extension);
    if (filename) {
      const data = await this.#diskCache.get(filename);
      return decoder.decode(data);
    }
  }

  load = (specifier: string): Promise<LoadResponse | undefined> => {
    const url = new URL(specifier);
    return this.#fileFetcher.fetch(url);
  };

  async set(type: CacheType, specifier: string, value: string): Promise<void> {
    const url = new URL(specifier);
    let extension: string;
    switch (type) {
      case "declaration":
        extension = "d.ts";
        break;
      case "emit":
        extension = "js";
        break;
      case "sourcemap":
        extension = "js.map";
        break;
      case "buildinfo":
        extension = "buildinfo";
        break;
      case "version": {
        let data = await this.#getEmitMetadata(url);
        if (data) {
          data.version_hash = value;
        } else {
          data = {
            version_hash: value,
          };
        }
        return this.#setEmitMetadata(url, data);
      }
    }
    const filename = DiskCache.getCacheFilenameWithExtension(url, extension);
    if (filename) {
      await this.#diskCache.set(filename, encoder.encode(value));
    }
  }
}
