// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { join } from "./deps.ts";
import type { CacheInfo, LoadResponse } from "./deps.ts";
import { DiskCache } from "./disk_cache.ts";
import type { FileFetcher } from "./file_fetcher.ts";
import type { HttpCache } from "./http_cache.ts";
import { isFileSync } from "./util.ts";

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

export class FetchCacher {
  #diskCache: DiskCache;
  #fileFetcher: FileFetcher;
  #httpCache: HttpCache;

  #getEmitMetadata(specifier: URL): EmitMetadata | undefined {
    const filename = DiskCache.getCacheFilenameWithExtension(specifier, "meta");
    if (!filename || !isFileSync(filename)) {
      return undefined;
    }
    const bytes = this.#diskCache.get(filename);
    return JSON.parse(decoder.decode(bytes));
  }

  #setEmitMetadata(specifier: URL, data: EmitMetadata): void {
    const filename = DiskCache.getCacheFilenameWithExtension(specifier, "meta");
    if (!filename) {
      return;
    }
    const bytes = encoder.encode(JSON.stringify(data));
    this.#diskCache.set(filename, bytes);
  }

  constructor(
    diskCache: DiskCache,
    httpCache: HttpCache,
    fileFetcher: FileFetcher,
  ) {
    this.#diskCache = diskCache;
    this.#fileFetcher = fileFetcher;
    this.#httpCache = httpCache;
  }

  cacheInfo = (specifier: string): CacheInfo => {
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

  get(type: CacheType, specifier: string): string | undefined {
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
        const data = this.#getEmitMetadata(url);
        return data ? data.version_hash : undefined;
      }
    }
    const filename = DiskCache.getCacheFilenameWithExtension(url, extension);
    if (filename) {
      const data = this.#diskCache.get(filename);
      return decoder.decode(data);
    }
  }

  load = (specifier: string): Promise<LoadResponse | undefined> => {
    const url = new URL(specifier);
    return this.#fileFetcher.fetch(url);
  };

  set(type: CacheType, specifier: string, value: string): void {
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
        let data = this.#getEmitMetadata(url);
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
      this.#diskCache.set(filename, encoder.encode(value));
    }
  }
}
