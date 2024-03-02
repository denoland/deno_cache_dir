// Copyright 2018-2024 the Deno authors. MIT license.

import { isAbsolute } from "./deps.ts";
import { assert } from "./util.ts";
import {
  GlobalHttpCache,
  instantiate,
  LocalHttpCache,
} from "./lib/deno_cache_dir.generated.js";

export interface HttpCacheCreateOptions {
  root: string;
  vendorRoot?: string;
  readOnly?: boolean;
}

export interface HttpCacheGetOptions {
  /** Checksum to evaluate the file against. This is only evaluated for the
   * global cache (DENO_DIR) and not the local cache (vendor folder).
   */
  checksum?: string;
  /** Allow copying from the global to the local cache (vendor folder). */
  allowCopyGlobalToLocal?: boolean;
}

export class HttpCache implements Disposable {
  #cache: LocalHttpCache | GlobalHttpCache;
  #readOnly: boolean | undefined;

  private constructor(
    cache: LocalHttpCache | GlobalHttpCache,
    readOnly: boolean | undefined,
  ) {
    this.#cache = cache;
    this.#readOnly = readOnly;
  }

  static async create(options: HttpCacheCreateOptions): Promise<HttpCache> {
    assert(isAbsolute(options.root), "Root must be an absolute path.");

    if (options.vendorRoot != null) {
      assert(
        isAbsolute(options.vendorRoot),
        "Vendor root must be an absolute path.",
      );
    }
    const { GlobalHttpCache, LocalHttpCache } = await instantiate();

    let cache: LocalHttpCache | GlobalHttpCache;
    if (options.vendorRoot != null) {
      cache = LocalHttpCache.new(options.vendorRoot, options.root);
    } else {
      cache = GlobalHttpCache.new(options.root);
    }
    return new HttpCache(cache, options.readOnly);
  }

  [Symbol.dispose]() {
    this.free();
  }

  free() {
    this.#cache?.free();
  }

  getHeaders(
    url: URL,
  ): Record<string, string> | undefined {
    const map = this.#cache.getHeaders(url.toString());
    return map == null ? undefined : Object.fromEntries(map);
  }

  get(
    url: URL,
    options?: HttpCacheGetOptions,
  ): Uint8Array | undefined {
    const data = this.#cache.getFileBytes(
      url.toString(),
      options?.checksum,
      options?.allowCopyGlobalToLocal ?? true,
    );
    return data == null ? undefined : data;
  }

  set(
    url: URL,
    headers: Record<string, string>,
    content: Uint8Array,
  ): void {
    if (this.#readOnly === undefined) {
      this.#readOnly =
        (Deno.permissions.querySync({ name: "write" })).state === "denied"
          ? true
          : false;
    }
    if (this.#readOnly) {
      return;
    }
    this.#cache.set(
      url.toString(),
      headers,
      content,
    );
  }
}
