// Copyright 2018-2024 the Deno authors. MIT license.

import { FetchCacher } from "./cache.ts";
import { DenoDir } from "./deno_dir.ts";
import { FileFetcher } from "./file_fetcher.ts";
import { createGraph } from "@deno/graph";
import { assertEquals } from "@std/assert/assert-equals";

async function setup() {
  const tempdir = await Deno.makeTempDir({
    prefix: "deno_cache_dir_cache_test",
  });
  const denoDir = new DenoDir(tempdir);
  const fileFetcher = new FileFetcher(
    () => {
      return denoDir.createHttpCache();
    },
    "use",
    true,
  );
  return new FetchCacher(fileFetcher);
}

Deno.test("FetchCacher#load works with createGraph to deal with a JSR package", async () => {
  const fetchCacher = await setup();

  const graph = await createGraph("jsr:@deno/gfm@0.9.0", {
    load: fetchCacher.load,
  });

  assertEquals(graph.roots, ["jsr:@deno/gfm@0.9.0"]);
  assertEquals(
    graph.redirects["jsr:@deno/gfm@0.9.0"],
    "https://jsr.io/@deno/gfm/0.9.0/mod.ts",
  );
});

Deno.test("FetchCacher#load works with createGraph to deal with a deno.land/x package", async () => {
  const fetchCacher = await setup();

  const graph = await createGraph("https://deno.land/x/oak@v9.0.1/mod.ts", {
    load: fetchCacher.load,
  });

  assertEquals(graph.roots, ["https://deno.land/x/oak@v9.0.1/mod.ts"]);
  assertEquals(graph.redirects, {});
});
