import { assertEquals } from "./deps_test.ts";

import { hash, urlToFilename } from "./util.ts";

Deno.test({
  name: "hash test",
  fn() {
    assertEquals(
      hash("hello world"),
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
    );
  },
});

Deno.test({
  name: "hash filename without search params",
  fn() {
    assertEquals(
      urlToFilename(new URL("https://cdn.skypack.dev/svelte/internal")),
      "https/cdn.skypack.dev/dae962c780900e18d25c9d22ed772d40dfcd93eb857d43c6e4f383f2c69ae40f",
    );
  },
});

Deno.test({
  name: "hash filename with search params",
  fn() {
    assertEquals(
      urlToFilename(new URL("https://cdn.skypack.dev/svelte/compiler?dts")),
      "https/cdn.skypack.dev/0f37079a386379010b507f219d5e9e7b661a94f25a4b34742d589cf89847fc47",
    );
  },
});
