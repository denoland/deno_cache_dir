// Copyright 2018-2024 the Deno authors. MIT license.

import { join } from "./deps.ts";

export function cacheDir(): string | undefined {
  if (Deno.build.os === "darwin") {
    const home = homeDir();
    if (home) {
      return join(home, "Library/Caches");
    }
  } else if (Deno.build.os === "windows") {
    Deno.permissions.request({ name: "env", variable: "LOCALAPPDATA" });
    return Deno.env.get("LOCALAPPDATA");
  } else {
    Deno.permissions.request({ name: "env", variable: "XDG_CACHE_HOME" });
    const cacheHome = Deno.env.get("XDG_CACHE_HOME");
    if (cacheHome) {
      return cacheHome;
    } else {
      const home = homeDir();
      if (home) {
        return join(home, ".cache");
      }
    }
  }
}

export function homeDir(): string | undefined {
  if (Deno.build.os === "windows") {
    Deno.permissions.request({ name: "env", variable: "USERPROFILE" });
    return Deno.env.get("USERPROFILE");
  } else {
    Deno.permissions.request({ name: "env", variable: "HOME" });
    return Deno.env.get("HOME");
  }
}
