// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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

export class HttpCache {
  #createOptions: HttpCacheCreateOptions;
  #cache: LocalHttpCache | GlobalHttpCache | undefined;
  #readOnly: boolean | undefined;

  constructor(options: HttpCacheCreateOptions) {
    assert(isAbsolute(options.root), "Root must be an absolute path.");

    if (options.vendorRoot != null) {
      assert(
        isAbsolute(options.vendorRoot),
        "Vendor root must be an absolute path.",
      );
    }

    this.#createOptions = options;
    this.#readOnly = options.readOnly;
  }

  async #ensureCache() {
    if (this.#cache == null) {
      const { GlobalHttpCache, LocalHttpCache } = await instantiate();
      const options = this.#createOptions;

      if (options.vendorRoot != null) {
        this.#cache = LocalHttpCache.new(options.vendorRoot, options.root);
      } else {
        this.#cache = GlobalHttpCache.new(options.root);
      }
    }
    return this.#cache;
  }

  free() {
    this.#cache?.free();
  }

  async getHeaders(
    url: URL,
  ): Promise<Record<string, string> | undefined> {
    const map = (await this.#ensureCache()).getHeaders(url.toString());
    return map == null ? undefined : Object.fromEntries(map);
  }

  async getText(
    url: URL,
  ): Promise<string | undefined> {
    const data = (await this.#ensureCache()).getFileText(url.toString());
    return data == null ? undefined : data;
  }

  async set(
    url: URL,
    headers: Record<string, string>,
    content: string,
  ): Promise<void> {
    if (this.#readOnly === undefined) {
      this.#readOnly =
        (Deno.permissions.querySync({ name: "write" })).state === "denied"
          ? true
          : false;
    }
    if (this.#readOnly) {
      return;
    }
    (await this.#ensureCache()).set(
      url.toString(),
      headers,
      new TextEncoder().encode(content),
    );
  }
}
