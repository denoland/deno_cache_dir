// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

interface BearerAuthToken {
  type: "bearer";
  host: string;
  token: string;
}

interface BasicAuthToken {
  type: "basic";
  host: string;
  username: string;
  password: string;
}

type AuthToken = BearerAuthToken | BasicAuthToken;

export function splitLast(
  value: string,
  delimiter: string,
): [string, string] {
  const split = value.split(delimiter);
  return [split.slice(0, -1).join(delimiter)].concat(split.slice(-1)) as [
    string,
    string,
  ];
}

function tokenAsValue(authToken: AuthToken): string {
  return authToken.type === "basic"
    ? `Basic ${authToken.username}:${authToken.password}`
    : `Bearer ${authToken.token}`;
}

export class AuthTokens {
  #tokens: AuthToken[];
  constructor(tokensStr = "") {
    const tokens: AuthToken[] = [];
    for (const tokenStr of tokensStr.split(";").filter((s) => s.length > 0)) {
      if (tokensStr.includes("@")) {
        const [host, token] = splitLast(tokenStr, "@");
        if (token.includes(":")) {
          const [password, username] = splitLast(token, ":");
          tokens.push({ type: "basic", host, username, password });
        } else {
          tokens.push({ type: "bearer", host, token });
        }
      } else {
        console.error("Badly formed auth token discarded.");
      }
    }
    this.#tokens = tokens;
  }

  get(specifier: URL): string | undefined {
    for (const token of this.#tokens) {
      if (token.host.endsWith(specifier.host)) {
        return tokenAsValue(token);
      }
    }
  }
}
