// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { join } from "./deps.ts";

if (Deno.build.os === "darwin" || Deno.build.os === "linux") {
  await Deno.permissions.request({ name: "env", variable: "HOME" });
  if (Deno.build.os === "linux") {
    await Deno.permissions.request({ name: "env", variable: "XDG_CACHE_HOME" });
  }
} else {
  await Deno.permissions.request({ name: "env", variable: "USERPROFILE" });
  await Deno.permissions.request({ name: "env", variable: "LOCALAPPDATA" });
}

export function cacheDir(): string | undefined {
  if (Deno.build.os === "darwin") {
    const home = homeDir();
    if (home) {
      return join(home, "Library/Caches");
    }
  } else if (Deno.build.os === "linux") {
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
    return Deno.env.get("LOCALAPPDATA");
  }
}

export function homeDir(): string | undefined {
  switch (Deno.build.os) {
    case "windows":
      return Deno.env.get("USERPROFILE");
    case "linux":
    case "darwin":
      return Deno.env.get("HOME");
    default:
      throw Error("unreachable");
  }
}
