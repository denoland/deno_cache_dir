// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { dirname, ensureDirSync, extname, isAbsolute, join } from "./deps.ts";
import { assert, CACHE_PERM, isFileSync, urlToFilename } from "./util.ts";

class Metadata {
  headers: Record<string, string>;
  url: URL;

  constructor(headers: Record<string, string>, url: URL) {
    this.headers = headers;
    this.url = url;
  }

  write(cacheFilename: string): void {
    const metadataFilename = Metadata.filename(cacheFilename);
    const json = JSON.stringify(
      {
        headers: this.headers,
        url: this.url,
      },
      undefined,
      "  ",
    );
    Deno.writeTextFileSync(metadataFilename, json, { mode: CACHE_PERM });
  }

  static filename(cacheFilename: string): string {
    const currentExt = extname(cacheFilename);
    if (currentExt) {
      const re = new RegExp(`\\${currentExt}$`);
      return cacheFilename.replace(re, ".metadata.json");
    } else {
      return `${cacheFilename}.metadata.json`;
    }
  }
}

export class HttpCache {
  location: string;

  constructor(location: string) {
    assert(isAbsolute(location));
    this.location = location;
  }

  getCacheFilename(url: URL): string {
    return join(this.location, urlToFilename(url));
  }

  get(url: URL): [Deno.File, Record<string, string>] | undefined {
    const cacheFilename = join(this.location, urlToFilename(url));
    const metadataFilename = Metadata.filename(cacheFilename);
    if (!isFileSync(cacheFilename)) {
      return undefined;
    }
    const file = Deno.openSync(cacheFilename, { read: true });
    const metadataStr = Deno.readTextFileSync(metadataFilename);
    const metadata: { headers: Record<string, string> } = JSON.parse(
      metadataStr,
    );
    assert(metadata.headers);
    return [file, metadata.headers];
  }

  set(url: URL, headers: Record<string, string>, content: string): void {
    const cacheFilename = join(this.location, urlToFilename(url));
    const parentFilename = dirname(cacheFilename);
    ensureDirSync(parentFilename);
    Deno.writeTextFileSync(cacheFilename, content, { mode: CACHE_PERM });
    const metadata = new Metadata(headers, url);
    metadata.write(cacheFilename);
  }
}
