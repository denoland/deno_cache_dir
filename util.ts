// Copyright 2018-2024 the Deno authors. MIT license.

export const CACHE_PERM = 0o644;

export function assert(cond: unknown, msg = "Assertion failed."): asserts cond {
  if (!cond) {
    throw new Error(msg);
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
