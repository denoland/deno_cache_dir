// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { DenoDir } from "./deno_dir.ts";
import { assertRejects, createGraph } from "./deps_test.ts";
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
    // deno-lint-ignore no-console
    console.log(graph);
  },
});

Deno.test({
  name: "FileFetcher - bad checksum no cache",
  async fn() {
    const denoDir = new DenoDir();
    const fileFetcher = new FileFetcher(denoDir.createHttpCache());
    {
      // should error
      await assertRejects(async () => {
        await fileFetcher.fetch(
          new URL("https://deno.land/x/oak@v10.5.1/mod.ts"),
          {
            checksum: "bad",
          },
        );
      });
      // ok for good checksum
      await fileFetcher.fetch(
        new URL("https://deno.land/x/oak@v10.5.1/mod.ts"),
        {
          checksum:
            "7a1b5169ef702e96dd994168879dbcbd8af4f639578b6300cbe1c6995d7f3f32",
        },
      );
    }
  },
});

Deno.test({
  name: "FileFetcher - bad checksum reload",
  async fn() {
    const denoDir = new DenoDir();
    const fileFetcher = new FileFetcher(denoDir.createHttpCache());
    await assertRejects(async () => {
      await fileFetcher.fetch(
        new URL("https://deno.land/x/oak@v10.5.1/mod.ts"),
        {
          cacheSetting: "reload",
          checksum: "bad",
        },
      );
    });
  },
});

Deno.test({
  name: "FileFetcher - good checksum reload",
  async fn() {
    const denoDir = new DenoDir();
    const fileFetcher = new FileFetcher(denoDir.createHttpCache());
    await fileFetcher.fetch(new URL("https://deno.land/x/oak@v10.5.1/mod.ts"), {
      cacheSetting: "reload",
      checksum:
        "7a1b5169ef702e96dd994168879dbcbd8af4f639578b6300cbe1c6995d7f3f32",
    });
  },
});
