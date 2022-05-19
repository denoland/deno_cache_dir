// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { DenoDir } from "./deno_dir.ts";

Deno.test({
  name: "DenoDir - basic",
  async fn() {
    const denoDir = new DenoDir();
    const url = new URL("https://deno.land/std@0.140.0/path/mod.ts");
    const [file, headers] = (await denoDir.deps.get(url))!;
    console.log(headers);
    file.close();
  },
});
