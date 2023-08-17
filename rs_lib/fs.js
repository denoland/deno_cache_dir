export function read_file_bytes(path) {
  try {
    return Deno.readFileSync(path);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return undefined;
    } else {
      throw err;
    }
  }
}

export function atomic_write_file(path, bytes) {
  function parentPath(path) {
    const lastSlashIndex = path.lastIndexOf("/") ?? path.lastIndexOf("\\");
    return path.slice(0, lastSlashIndex);
  }

  // write to a temporary file write beside the other file, then rename it
  // in a single sys call in order to prevent issues where the process
  // is killed while writing to a file and the file ends up in a corrupted state

  const cachePerm = 0o644;
  const tempName = path + "." + randomHex();
  try {
    Deno.writeFileSync(tempName, bytes, { mode: cachePerm });
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      try {
        Deno.mkdirSync(parentPath(path), { recursive: true });
      } catch {
        // ignore
      }
      Deno.writeFileSync(tempName, bytes, { mode: cachePerm });
    } else {
      throw err;
    }
  }
  try {
    Deno.renameSync(tempName, path);
  } catch (err) {
    try {
      Deno.removeSync(tempName);
    } catch {
      // ignore
    }
    throw err;
  }

  function randomHex() {
    //https://stackoverflow.com/a/27747377/188246
    const arr = new Uint8Array(2);
    crypto.getRandomValues(arr);
    return Array.from(arr, (dec) => dec.toString(16).padStart(2, "0")).join("");
  }
}

export function modified_time(path) {
  try {
    const stat = Deno.lstatSync(path);
    return msToS(stat.mtime.getTime());
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return undefined;
    } else {
      throw err;
    }
  }
}

export function is_file(path) {
  try {
    const stat = Deno.lstatSync(path);
    return stat.isFile;
  } catch {
    return false;
  }
}

export function time_now() {
  return msToS(Date.now());
}

function msToS(ms) {
  return Math.round(ms / 1000);
}
