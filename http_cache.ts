// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { dirname, ensureDir, extname, isAbsolute, join } from "./deps.ts";
import { assert, CACHE_PERM, isFile, urlToFilename } from "./util.ts";

class Metadata {
  headers: Record<string, string>;
  url: URL;

  constructor(headers: Record<string, string>, url: URL) {
    this.headers = headers;
    this.url = url;
  }

  async write(cacheFilename: string): Promise<void> {
    const metadataFilename = Metadata.filename(cacheFilename);
    const json = JSON.stringify(
      {
        headers: this.headers,
        url: this.url,
      },
      undefined,
      "  ",
    );
    await Deno.writeTextFile(metadataFilename, json, { mode: CACHE_PERM });
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
  readOnly?: boolean;

  constructor(location: string, readOnly?: boolean) {
    assert(isAbsolute(location));
    this.location = location;
    this.readOnly = readOnly;
  }

  getCacheFilename(url: URL): string {
    return join(this.location, urlToFilename(url));
  }

  async get(
    url: URL,
  ): Promise<[Deno.FsFile, Record<string, string>] | undefined> {
    const cacheFilename = join(this.location, urlToFilename(url));
    const metadataFilename = Metadata.filename(cacheFilename);
    if (!(await isFile(cacheFilename))) {
      return undefined;
    }
    const file = await Deno.open(cacheFilename, { read: true });
    const metadataStr = await Deno.readTextFile(metadataFilename);
    const metadata: { headers: Record<string, string> } = JSON.parse(
      metadataStr,
    );
    assert(metadata.headers);
    return [file, metadata.headers];
  }

  async set(
    url: URL,
    headers: Record<string, string>,
    content: string,
  ): Promise<void> {
    if (this.readOnly === undefined) {
      this.readOnly =
        (await Deno.permissions.query({ name: "write" })).state === "denied"
          ? true
          : false;
    }
    if (this.readOnly) {
      return;
    }
    const cacheFilename = join(this.location, urlToFilename(url));
    const parentFilename = dirname(cacheFilename);
    await ensureDir(parentFilename);
    await Deno.writeTextFile(cacheFilename, content, { mode: CACHE_PERM });
    const metadata = new Metadata(headers, url);
    await metadata.write(cacheFilename);
  }
}
