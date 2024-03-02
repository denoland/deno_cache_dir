// Copyright 2018-2024 the Deno authors. MIT license.

import { AuthTokens } from "./auth_tokens.ts";
import { assertEquals } from "@std/assert";

Deno.test({
  name: "handle undefined token string",
  fn() {
    const authTokens = new AuthTokens(undefined);
    assertEquals(authTokens.get(new URL("http://localhost")), undefined);
  },
});

Deno.test({
  name: "find bearer token",
  fn() {
    const authTokens = new AuthTokens("token1@example.com");
    assertEquals(
      authTokens.get(new URL("https://example.com")),
      "Bearer token1",
    );
  },
});

Deno.test({
  name: "find basic token (base64 encoded)",
  fn() {
    const authTokens = new AuthTokens("user1:pw1@example.com");
    assertEquals(
      authTokens.get(new URL("https://example.com")),
      "Basic dXNlcjE6cHcx",
    );
  },
});
