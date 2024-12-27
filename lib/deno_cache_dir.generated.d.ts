// deno-lint-ignore-file
// deno-fmt-ignore-file

export interface InstantiateResult {
  instance: WebAssembly.Instance;
  exports: {
    url_to_filename: typeof url_to_filename;
    resolve_deno_dir: typeof resolve_deno_dir;
    GlobalHttpCache : typeof GlobalHttpCache ;
    LocalHttpCache : typeof LocalHttpCache 
  };
}

/** Gets if the Wasm module has been instantiated. */
export function isInstantiated(): boolean;


/** Instantiates an instance of the Wasm module returning its functions.
* @remarks It is safe to call this multiple times and once successfully
* loaded it will always return a reference to the same object. */
export function instantiate(): InstantiateResult["exports"];

/** Instantiates an instance of the Wasm module along with its exports.
 * @remarks It is safe to call this multiple times and once successfully
 * loaded it will always return a reference to the same object. */
export function instantiateWithInstance(): InstantiateResult;

/**
* @param {string} url
* @returns {string}
*/
export function url_to_filename(url: string): string;
/**
* @param {string | undefined} [maybe_custom_root]
* @returns {string}
*/
export function resolve_deno_dir(maybe_custom_root?: string): string;
/**
*/
export class GlobalHttpCache {
  free(): void;
/**
* @param {string} path
* @returns {GlobalHttpCache}
*/
  static new(path: string): GlobalHttpCache;
/**
* @param {string} url
* @returns {any}
*/
  getHeaders(url: string): any;
/**
* @param {string} url
* @param {string | undefined} [maybe_checksum]
* @returns {any}
*/
  get(url: string, maybe_checksum?: string): any;
/**
* @param {string} url
* @param {any} headers
* @param {Uint8Array} text
*/
  set(url: string, headers: any, text: Uint8Array): void;
}
/**
*/
export class LocalHttpCache {
  free(): void;
/**
* @param {string} local_path
* @param {string} global_path
* @param {boolean} allow_global_to_local_copy
* @returns {LocalHttpCache}
*/
  static new(local_path: string, global_path: string, allow_global_to_local_copy: boolean): LocalHttpCache;
/**
* @param {string} url
* @returns {any}
*/
  getHeaders(url: string): any;
/**
* @param {string} url
* @param {string | undefined} [maybe_checksum]
* @returns {any}
*/
  get(url: string, maybe_checksum?: string): any;
/**
* @param {string} url
* @param {any} headers
* @param {Uint8Array} text
*/
  set(url: string, headers: any, text: Uint8Array): void;
}
