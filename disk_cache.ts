// Copyright 2018-2024 the Deno authors. MIT license.

import {
  dirname,
  ensureDir,
  isAbsolute,
  join,
  readAll,
  writeAll,
} from "./deps.ts";
import { assert, CACHE_PERM } from "./util.ts";
import { instantiate } from "./lib/deno_cache_dir.generated.js";

export class DiskCache {
  location: string;

  constructor(location: string) {
    assert(isAbsolute(location));
    this.location = location;
  }

  async get(filename: string): Promise<Uint8Array> {
    const path = join(this.location, filename);
    const file = await Deno.open(path, { read: true });
    const value = await readAll(file);
    file.close();
    return value;
  }

  async set(filename: string, data: Uint8Array): Promise<void> {
    const path = join(this.location, filename);
    const parentFilename = dirname(path);
    await ensureDir(parentFilename);
    const file = await Deno.open(path, {
      write: true,
      create: true,
      mode: CACHE_PERM,
    });
    await writeAll(file, data);
    file.close();
  }

  static async getCacheFilename(url: URL): Promise<string> {
    const { url_to_filename } = await instantiate();
    return url_to_filename(url.toString());
  }
}
