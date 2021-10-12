import { assertEquals } from "./deps_test.ts";

import { hash } from "./util.ts";

Deno.test({
  name: "hash test",
  fn() {
    assertEquals(
      hash("hello world"),
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
    );
  },
});
