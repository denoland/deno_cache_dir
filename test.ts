// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { createGraph } from "./deps_test.ts";
import { createCache } from "./mod.ts";

Deno.test({
  name: "createCache()",
  async fn() {
    const cache = createCache();
    const { cacheInfo, load } = cache;
    const graph = await createGraph("https://deno.land/x/oak@v10.5.1/mod.ts", {
      cacheInfo,
      load,
    });
    console.log(graph.toString());
  },
});
