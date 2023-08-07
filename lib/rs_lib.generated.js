// @generated file from wasmbuild -- do not edit
// deno-lint-ignore-file
// deno-fmt-ignore-file
// source-hash: 79f3907d83c7d620c067399271e3857799f32fb6
let wasm;
let cachedInt32Memory0;
let cachedUint8Memory0;
/**
 * @param {number} a
 * @param {number} b
 * @returns {number}
 */
export function add(a, b) {
  const ret = wasm.add(a, b);
  return ret;
}

const imports = {
  __wbindgen_placeholder__: {},
};

/** Instantiates an instance of the Wasm module returning its functions.
 * @remarks It is safe to call this multiple times and once successfully
 * loaded it will always return a reference to the same object.
 */
export function instantiate() {
  return instantiateWithInstance().exports;
}

let instanceWithExports;

/** Instantiates an instance of the Wasm module along with its exports.
 * @remarks It is safe to call this multiple times and once successfully
 * loaded it will always return a reference to the same object.
 * @returns {{
 *   instance: WebAssembly.Instance;
 *   exports: { add: typeof add }
 * }}
 */
export function instantiateWithInstance() {
  if (instanceWithExports == null) {
    const instance = instantiateInstance();
    wasm = instance.exports;
    cachedInt32Memory0 = new Int32Array(wasm.memory.buffer);
    cachedUint8Memory0 = new Uint8Array(wasm.memory.buffer);
    instanceWithExports = {
      instance,
      exports: { add },
    };
  }
  return instanceWithExports;
}

/** Gets if the Wasm module has been instantiated. */
export function isInstantiated() {
  return instanceWithExports != null;
}

function instantiateInstance() {
  const wasmBytes = base64decode(
"\
AGFzbQEAAAABh4CAgAABYAJ/fwF/A4KAgIAAAQAFg4CAgAABABEGiYCAgAABfwFBgIDAAAsHkICAgA\
ACBm1lbW9yeQIAA2FkZAAAComAgIAAAQcAIAEgAGoLAJGAgIAABG5hbWUBhoCAgAABAANhZGQA74CA\
gAAJcHJvZHVjZXJzAghsYW5ndWFnZQEEUnVzdAAMcHJvY2Vzc2VkLWJ5AwVydXN0Yx0xLjY4LjIgKD\
llYjNhZmU5ZSAyMDIzLTAzLTI3KQZ3YWxydXMGMC4xOS4wDHdhc20tYmluZGdlbgYwLjIuODY=\
",
  );
  const wasmModule = new WebAssembly.Module(wasmBytes);
  return new WebAssembly.Instance(wasmModule, imports);
}

function base64decode(b64) {
  const binString = atob(b64);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    bytes[i] = binString.charCodeAt(i);
  }
  return bytes;
}
