# deno_cache_dir

[![jsr](https://jsr.io/badges/@deno/cache-dir)](https://jsr.io/@deno/cache-dir)
[![Build Status - Cirrus][]][Build status]
[![](https://img.shields.io/crates/v/deno_cache_dir.svg)](https://crates.io/crates/deno_cache_dir)

Implementation of the DENO_DIR/cache for the Deno CLI.

This is designed to provide access to the cache using the same logic that the
Deno CLI accesses the cache, which allows projects like
[`deno_graph`](https://deno.land/x/deno_graph),
[`deno_doc`](https://deno.land/x/deno_doc), [`dnt`](https://deno.land/x/dnt),
and [`emit`](https://deno.land/x/deno_emit) to access and populate the cache in
the same way that the CLI does.

## Permissions

Because of the nature of code, it requires several permissions to be able to
work properly. If the permissions aren't granted at execution, the code will try
to prompt for them, only requesting what is specifically needed to perform the
task.

- `--allow-env` - The code needs access to several environment variables,
  depending on the platform as well, these can include `HOME`, `USERPROFILE`,
  `LOCALAPPDATA`, `XDG_CACHE_HOME`, `DENO_DIR`, and `DENO_AUTH_TOKENS`.
- `--allow-read` - In certain cases the code needs to determine the current
  working directory, as well as read the cache files, so it requires read
  permission.
- `--allow-write` - The code requires write permission to the root of the cache
  directory.
- `--allow-net` - The code requires net access to any remote modules that are
  not found in the cache.

This can just be granted on startup to avoid being prompted for them.

## Example

```shellsession
deno add @deno/cache-dir
deno add @deno/graph
```

Using the cache and the file fetcher to provide modules to build a module graph:

```ts
import { createCache } from "@deno/cache-dir";
import { createGraph } from "@deno/graph";

// create a cache where the location will be determined environmentally
const cache = createCache();
// destructuring the load we need to pass to the graph
const { load } = cache;
// create a graph that will use the cache above to load and cache dependencies
const graph = await createGraph("https://deno.land/x/oak@v9.0.1/mod.ts", {
  load,
});

// log out the console a similar output to `deno info` on the command line.
console.log(graph.toString());
```
