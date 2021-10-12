// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { AuthTokens } from "./auth_tokens.ts";
import { assertEquals } from "./deps_test.ts";

Deno.test({
  name: "handle undefined token string",
  fn() {
    const authTokens = new AuthTokens(undefined);
    assertEquals(authTokens.get(new URL("http://localhost")), undefined);
  },
});
