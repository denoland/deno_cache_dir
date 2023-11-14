// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { AuthTokens } from "./auth_tokens.ts";
import { colors, fromFileUrl } from "./deps.ts";
import type { LoadResponse } from "./deps.ts";
import type { HttpCache } from "./http_cache.ts";

/**
 * A setting that determines how the cache is handled for remote dependencies.
 *
 * The default is `"use"`.
 *
 * - `"only"` - only the cache will be re-used, and any remote modules not in
 *    the cache will error.
 * - `"use"` - the cache will be used, meaning existing remote files will not be
 *    reloaded.
 * - `"reload"` - any cached modules will be ignored and their values will be
 *    fetched.
 * - `string[]` - an array of string specifiers, that if they match the start of
 *    the requested specifier, will be reloaded.
 */
export type CacheSetting = "only" | "reload" | "use" | string[];

function shouldUseCache(cacheSetting: CacheSetting, specifier: URL): boolean {
  switch (cacheSetting) {
    case "only":
    case "use":
      return true;
    // @ts-ignore old setting
    case "reloadAll":
    case "reload":
      return false;
    default: {
      const specifierStr = specifier.toString();
      for (const value of cacheSetting) {
        if (specifierStr.startsWith(value)) {
          return false;
        }
      }
      return true;
    }
  }
}

const SUPPORTED_SCHEMES = [
  "data:",
  "blob:",
  "file:",
  "http:",
  "https:",
] as const;

type SupportedSchemes = typeof SUPPORTED_SCHEMES[number];

function getValidatedScheme(specifier: URL) {
  const scheme = specifier.protocol;
  // deno-lint-ignore no-explicit-any
  if (!SUPPORTED_SCHEMES.includes(scheme as any)) {
    throw new TypeError(
      `Unsupported scheme "${scheme}" for module "${specifier.toString()}". Supported schemes: ${
        JSON.stringify(SUPPORTED_SCHEMES)
      }.`,
    );
  }
  return scheme as SupportedSchemes;
}

export function stripHashbang(value: string): string {
  return value.startsWith("#!") ? value.slice(value.indexOf("\n")) : value;
}

async function fetchLocal(specifier: URL): Promise<LoadResponse | undefined> {
  const local = fromFileUrl(specifier);
  if (!local) {
    throw new TypeError(
      `Invalid file path.\n  Specifier: "${specifier.toString()}"`,
    );
  }
  try {
    const source = await Deno.readTextFile(local);
    const content = stripHashbang(source);
    return {
      kind: "module",
      content,
      specifier: specifier.toString(),
    };
  } catch {
    // ignoring errors, we will just return undefined
  }
}

export class FileFetcher {
  #allowRemote: boolean;
  #authTokens: AuthTokens;
  #cache = new Map<string, LoadResponse>();
  #cacheSetting: CacheSetting;
  #httpCache: HttpCache;

  constructor(
    httpCache: HttpCache,
    cacheSetting: CacheSetting = "use",
    allowRemote = true,
  ) {
    Deno.permissions.request({ name: "env", variable: "DENO_AUTH_TOKENS" });
    this.#authTokens = new AuthTokens(Deno.env.get("DENO_AUTH_TOKENS"));
    this.#allowRemote = allowRemote;
    this.#cacheSetting = cacheSetting;
    this.#httpCache = httpCache;
  }

  async #fetchBlobDataUrl(
    specifier: URL,
    cacheSetting: CacheSetting,
  ): Promise<LoadResponse> {
    const cached = await this.#fetchCached(specifier, 0);
    if (cached) {
      return cached;
    }

    if (cacheSetting === "only") {
      throw new Deno.errors.NotFound(
        `Specifier not found in cache: "${specifier.toString()}", --cached-only is specified.`,
      );
    }

    const response = await fetchWithRetries(specifier.toString());
    const content = await response.text();
    const headers: Record<string, string> = {};
    for (const [key, value] of response.headers) {
      headers[key.toLowerCase()] = value;
    }
    await this.#httpCache.set(specifier, headers, content);
    return {
      kind: "module",
      specifier: specifier.toString(),
      headers,
      content,
    };
  }

  async #fetchCached(
    specifier: URL,
    redirectLimit: number,
  ): Promise<LoadResponse | undefined> {
    if (redirectLimit < 0) {
      throw new Deno.errors.Http(
        `Too many redirects.\n  Specifier: "${specifier.toString()}"`,
      );
    }

    const headers = await this.#httpCache.getHeaders(specifier);
    if (!headers) {
      return undefined;
    }
    const location = headers["location"];
    if (location != null && location.length > 0) {
      const redirect = new URL(location, specifier);
      return this.#fetchCached(redirect, redirectLimit - 1);
    }
    const content = await this.#httpCache.getText(specifier);
    if (content == null) {
      return undefined;
    }
    return {
      kind: "module",
      specifier: specifier.toString(),
      headers,
      content,
    };
  }

  async #fetchRemote(
    specifier: URL,
    redirectLimit: number,
    cacheSetting: CacheSetting,
  ): Promise<LoadResponse | undefined> {
    if (redirectLimit < 0) {
      throw new Deno.errors.Http(
        `Too many redirects.\n  Specifier: "${specifier.toString()}"`,
      );
    }

    if (shouldUseCache(cacheSetting, specifier)) {
      const response = await this.#fetchCached(specifier, redirectLimit);
      if (response) {
        return response;
      }
    }

    if (cacheSetting === "only") {
      throw new Deno.errors.NotFound(
        `Specifier not found in cache: "${specifier.toString()}", --cached-only is specified.`,
      );
    }

    const requestHeaders = new Headers();
    const cachedHeaders = await this.#httpCache.getHeaders(specifier);
    if (cachedHeaders) {
      const etag = cachedHeaders["etag"];
      if (etag != null && etag.length > 0) {
        requestHeaders.append("if-none-match", etag);
      }
    }
    const authToken = this.#authTokens.get(specifier);
    if (authToken) {
      requestHeaders.append("authorization", authToken);
    }
    console.error(`${colors.green("Download")} ${specifier.toString()}`);
    const response = await fetchWithRetries(specifier.toString(), {
      headers: requestHeaders,
    });
    if (!response.ok) {
      if (response.status === 404) {
        return undefined;
      } else {
        throw new Deno.errors.Http(`${response.status} ${response.statusText}`);
      }
    }
    // WHATWG fetch follows redirects automatically, so we will try to
    // determine if that occurred and cache the value.
    if (specifier.toString() !== response.url) {
      const headers = { "location": response.url };
      await this.#httpCache.set(specifier, headers, "");
    }
    const url = new URL(response.url);
    const content = await response.text();
    const headers: Record<string, string> = {};
    for (const [key, value] of response.headers) {
      headers[key.toLowerCase()] = value;
    }
    await this.#httpCache.set(url, headers, content);
    return {
      kind: "module",
      specifier: response.url,
      headers,
      content,
    };
  }

  async fetch(
    specifier: URL,
    options?: { cacheSetting?: CacheSetting },
  ): Promise<LoadResponse | undefined> {
    const cacheSetting = options?.cacheSetting ?? this.#cacheSetting;
    const scheme = getValidatedScheme(specifier);
    if (scheme === "file:") {
      return fetchLocal(specifier);
    }
    const response = this.#cache.get(specifier.toString());
    if (response) {
      return response;
    } else if (scheme === "data:" || scheme === "blob:") {
      const response = await this.#fetchBlobDataUrl(specifier, cacheSetting);
      await this.#cache.set(specifier.toString(), response);
      return response;
    } else if (!this.#allowRemote) {
      throw new Deno.errors.PermissionDenied(
        `A remote specifier was requested: "${specifier.toString()}", but --no-remote is specified.`,
      );
    } else {
      const response = await this.#fetchRemote(specifier, 10, cacheSetting);
      if (response) {
        await this.#cache.set(specifier.toString(), response);
      }
      return response;
    }
  }
}

export async function fetchWithRetries(
  url: URL | string,
  init?: { headers?: Headers },
) {
  const maxRetries = 3;
  let sleepMs = 250;
  let iterationCount = 0;
  while (true) {
    iterationCount++;
    try {
      const res = await fetch(url, init);
      if (res.ok || iterationCount > maxRetries) {
        return res;
      }
    } catch (err) {
      if (iterationCount > maxRetries) {
        throw err;
      }
    }
    console.warn(
      `${
        colors.yellow("WARN")
      } Failed fetching ${url}. Retrying in ${sleepMs}ms...`,
    );
    await new Promise((resolve) => setTimeout(resolve, sleepMs));
    sleepMs = Math.min(sleepMs * 2, 10_000);
  }
}
