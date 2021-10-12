// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { DenoDir } from "./deno_dir.ts";

Deno.test({
  name: "DenoDir - basic",
  fn() {
    const denoDir = new DenoDir();
    const url = new URL("https://deno.land/std@0.110.0/path/mod.ts");
    const [file, headers] = denoDir.deps.get(url)!;
    console.log(headers);
    file.close();
  },
});
