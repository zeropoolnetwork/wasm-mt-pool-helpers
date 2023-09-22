# wasm-mt-pool-helpers

## Compiling library
```bash
wasm-pack build --release --out-dir mylib --out-name mylib --target no-modules . -- -Z build-std=panic_abort,std
```

Patching the output .js file might be needed to make it work with bundlers. For example, replace
```js
let wasm_bindgen;
//...
wasm_bindgen = /* ... */;
```
with
```js
if (typeof window !== 'undefined') {
    window.wasm_bindgen = Object.assign(__wbg_init, { initSync }, __exports);
} else if (typeof self !== 'undefined') {
    self.wasm_bindgen = Object.assign(__wbg_init, { initSync }, __exports);
} else if (typeof global !== 'undefined') {
    global.wasm_bindgen = Object.assign(__wbg_init, { initSync }, __exports);
} else {
    throw new Error('wasm-bindgen failed to find the global object');
}
```
