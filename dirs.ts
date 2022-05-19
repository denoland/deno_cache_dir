// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join } from "./deps.ts";

export function cacheDir(): string | undefined {
  if (Deno.build.os === "darwin") {
    const home = homeDir();
    if (home) {
      return join(home, "Library/Caches");
    }
  } else if (Deno.build.os === "linux") {
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
  } else {
    Deno.permissions.request({ name: "env", variable: "LOCALAPPDATA" });
    return Deno.env.get("LOCALAPPDATA");
  }
}

export function homeDir(): string | undefined {
  switch (Deno.build.os) {
    case "windows":
      Deno.permissions.request({ name: "env", variable: "USERPROFILE" });
      return Deno.env.get("USERPROFILE");
    case "linux":
    case "darwin":
      Deno.permissions.request({ name: "env", variable: "HOME" });
      return Deno.env.get("HOME");
    default:
      throw Error("unreachable");
  }
}
