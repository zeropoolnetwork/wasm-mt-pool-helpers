#![feature(async_closure)]

use core::{panic};
use std::cell::RefCell;
use std::rc::Rc;

use once_cell::sync::OnceCell;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_mt::utils::{console_ln, run_js, run_js_async};
use wasm_mt_pool::prelude::*;

static POOL: OnceCell<ThreadPoolWrapper> = OnceCell::new();

struct ThreadPoolWrapper {
    pool: ThreadPool,
    size: usize,
}

// Should be ok since we only use it from the main thread
unsafe impl Sync for ThreadPoolWrapper {}
unsafe impl Send for ThreadPoolWrapper {}

impl AsRef<ThreadPool> for ThreadPoolWrapper {
    fn as_ref(&self) -> &ThreadPool {
        &self.pool
    }
}

#[wasm_bindgen]
pub async fn init_thread_pool(pkg_js: &str, pkg_wasm: &str, size: usize) {
    console_ln!("Initializing threadpool: {size} threads, js: {pkg_js}, wasm: {pkg_wasm}");

    let pool = ThreadPool::new(size, pkg_js, pkg_wasm)
        .and_init()
        .await
        .unwrap();

    if let Err(_) = POOL.set(ThreadPoolWrapper { pool, size }) {
        panic!("Thread pool already initialized");
    }
}

pub fn num_threads() -> usize {
    POOL.get().unwrap().size
}

pub async fn parallel_map<T, R, F>(data: Vec<T>, func: F) -> Vec<R>
    where
        T: Serialize + DeserializeOwned + Clone + 'static,
        R: Serialize + DeserializeOwned + Clone + 'static,
        F: Fn(usize, T) -> R + Clone + Serialize + DeserializeOwned + 'static,
{
    let data_size = data.len();
    let pool = &POOL.get().unwrap();
    let result = Rc::new(RefCell::new(Vec::with_capacity(data_size)));
    let num_chunks_processed = Rc::new(RefCell::new(0));
    let num_chunks = pool.size;
    let chunk_size = data_size / num_chunks + 1;
    let chunks = data.chunks(chunk_size);

    // TODO: Return promise instead of using global variables?
    // TODO: Handle multiple promises.
    run_js(
        r#"
        let global = globalThis || window || self || global;
        global.treadPoolPromise = new Promise((res, rej) => {
            global.threadPoolResolver = res;
        });
    "#,
    )
        .unwrap();

    console_ln!(
        "parallel_map: processing {} chunks, chunk size: {}, data size: {}",
        num_chunks,
        chunk_size,
        data.len()
    );

    for (chunk_n, chunk) in chunks.enumerate() {
        let cur_chunk_size = chunk.len();

        let res = result.clone();
        let num_chunks_processed = num_chunks_processed.clone();
        let cb = move |result: Result<JsValue, JsValue>| {
            let result = result.unwrap();
            let result: Vec<_> = serde_wasm_bindgen::from_value(result).unwrap();

            let slice_offset = chunk_n * chunk_size;
            let slice_end = slice_offset + cur_chunk_size - 1;
            res.borrow_mut()[slice_offset..=slice_end].clone_from_slice(&result);

            let mut num_chunks_processed = num_chunks_processed.borrow_mut();
            *num_chunks_processed += 1;

            if *num_chunks_processed == num_chunks {
                run_js(
                    r#"
                    let global = globalThis || window || self || global;
                    global.threadPoolResolver();
                "#,
                )
                    .unwrap();
            }
        };

        let chunk = chunk.to_vec();

        let f = func.clone();
        pool_exec!(
            &pool.pool,
            async move || -> Result<JsValue, JsValue> {
                let chunk: Vec<R> = chunk.into_iter().enumerate().map(|(i, x)| {
                    (f)(chunk_n * chunk_size + i, x.clone())
                }).collect();

                Ok(serde_wasm_bindgen::to_value(&chunk).unwrap())
            },
            cb
        );
    }

    run_js_async(
        r#"
        let global = globalThis || window || self || global;
        await global.treadPoolPromise;
    "#,
    )
        .await
        .unwrap();

    result.take()
}
