// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { DenoDir } from "./deno_dir.ts";
import { createGraph } from "./deps_test.ts";
import { FileFetcher } from "./file_fetcher.ts";

Deno.test({
  name: "FileFetcher",
  async fn() {
    const denoDir = new DenoDir();
    const fileFetcher = new FileFetcher(denoDir.createHttpCache());
    const graph = await createGraph("https://deno.land/x/oak@v10.5.1/mod.ts", {
      load(specifier) {
        return fileFetcher.fetch(new URL(specifier));
      },
    });
    console.log(graph.toString());
  },
});
