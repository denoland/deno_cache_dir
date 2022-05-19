// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join, Sha256 } from "./deps.ts";

export const CACHE_PERM = 0o644;

export function assert(cond: unknown, msg = "Assertion failed."): asserts cond {
  if (!cond) {
    throw new Error(msg);
  }
}

/**
 * Generates a sha256 hex hash for a given input string.  This mirrors the
 * behavior of Deno CLI's `cli::checksum::gen`.
 *
 * Would love to use the Crypto API here, but it only works async and we need
 * to be able to generate the hashes sync to be able to keep the cache able to
 * look up files synchronously.
 */
export function hash(value: string): string {
  const sha256 = new Sha256();
  sha256.update(value);
  return sha256.hex();
}

function baseUrlToFilename(url: URL): string {
  const out = [];
  const scheme = url.protocol.replace(":", "");
  out.push(scheme);

  switch (scheme) {
    case "http":
    case "https": {
      const host = url.hostname;
      const hostPort = url.port;
      out.push(hostPort ? `${host}_PORT${hostPort}` : host);
      break;
    }
    case "data":
    case "blob":
      break;
    default:
      throw new TypeError(
        `Don't know how to create cache name for scheme: ${scheme}`,
      );
  }

  return join(...out);
}

export function urlToFilename(url: URL) {
  const cacheFilename = baseUrlToFilename(url);
  let restStr = url.pathname;
  const query = url.search;
  if (query) {
    restStr += `?${query}`;
  }
  const hashedFilename = hash(restStr);
  return join(cacheFilename, hashedFilename);
}

export async function isFile(filePath: string): Promise<boolean> {
  try {
    const stats = await Deno.lstat(filePath);
    return stats.isFile;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}

export function isFileSync(filePath: string): boolean {
  try {
    const stats = Deno.lstatSync(filePath);
    return stats.isFile;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}
