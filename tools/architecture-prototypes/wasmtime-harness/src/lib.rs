//! Disposable real-engine evidence for ADR-003.
//!
//! This crate is deliberately excluded from the product workspace and is never
//! linked by a shipping crate. Its runner fixes Cargo output under the project
//! root `target/architecture-prototypes/` directory.

#[cfg(test)]
mod tests {
    use std::future;
    use std::thread;
    use std::time::Duration;

    use anyhow::Result;
    use wasmtime::{
        Config, Engine, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
    };

    struct LimitedState {
        limits: StoreLimits,
    }

    fn sync_engine() -> Result<Engine> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        Ok(Engine::new(&config)?)
    }

    #[test]
    fn compiled_module_reuse_keeps_store_memory_isolated_and_bounded() -> Result<()> {
        let engine = sync_engine()?;
        let module = Module::new(
            &engine,
            r#"
                (module
                    (memory (export "memory") 1 2)
                    (func (export "grow") (param i32) (result i32)
                        local.get 0
                        memory.grow))
            "#,
        )?;

        let new_store = || {
            let mut store = Store::new(
                &engine,
                LimitedState {
                    limits: StoreLimitsBuilder::new().memory_size(64 * 1024).build(),
                },
            );
            store.limiter(|state| &mut state.limits);
            store.set_fuel(10_000).unwrap();
            store.set_epoch_deadline(10);
            store
        };

        let mut first_store = new_store();
        let first = Instance::new(&mut first_store, &module, &[])?;
        let first_memory = first.get_memory(&mut first_store, "memory").unwrap();
        first_memory.data_mut(&mut first_store)[0] = 42;

        let mut second_store = new_store();
        let second = Instance::new(&mut second_store, &module, &[])?;
        let second_memory = second.get_memory(&mut second_store, "memory").unwrap();
        assert_eq!(first_memory.data(&first_store)[0], 42);
        assert_eq!(second_memory.data(&second_store)[0], 0);

        let grow = first.get_typed_func::<i32, i32>(&mut first_store, "grow")?;
        assert_eq!(grow.call(&mut first_store, 1)?, -1);
        assert_eq!(first_memory.size(&first_store), 1);
        Ok(())
    }

    #[test]
    fn deterministic_fuel_stops_non_yielding_guest_compute() -> Result<()> {
        let engine = sync_engine()?;
        let module = Module::new(&engine, r#"(module (func (export "spin") (loop br 0)))"#)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(1_000)?;
        store.set_epoch_deadline(u64::MAX);
        let instance = Instance::new(&mut store, &module, &[])?;
        let spin = instance.get_typed_func::<(), ()>(&mut store, "spin")?;

        let error = spin.call(&mut store, ()).unwrap_err();
        assert!(format!("{error:#}").contains("fuel"), "{error:#}");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn epoch_interrupts_non_yielding_async_guest_compute() -> Result<()> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config)?;
        let module = Module::new(&engine, r#"(module (func (export "spin") (loop br 0)))"#)?;
        let mut store = Store::new(&engine, ());
        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();
        let instance = Instance::new_async(&mut store, &module, &[]).await?;
        let spin = instance.get_typed_func::<(), ()>(&mut store, "spin")?;

        let epoch_engine = engine.clone();
        let incrementer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            epoch_engine.increment_epoch();
        });
        let error = spin.call_async(&mut store, ()).await.unwrap_err();
        incrementer.join().unwrap();
        assert!(format!("{error:#}").contains("interrupt"), "{error:#}");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn host_deadline_cancels_pending_async_host_call_and_store_is_not_reused() -> Result<()> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config)?;
        let module = Module::new(
            &engine,
            r#"
                (module
                    (import "host" "wait" (func $wait))
                    (func (export "run") call $wait))
            "#,
        )?;
        let mut linker = Linker::new(&engine);
        linker.func_wrap_async("host", "wait", |_caller, ()| {
            Box::new(async {
                future::pending::<()>().await;
                Ok(())
            })
        })?;

        let mut cancelled_store = Store::new(&engine, ());
        cancelled_store.set_epoch_deadline(u64::MAX);
        let instance = linker
            .instantiate_async(&mut cancelled_store, &module)
            .await?;
        let run = instance.get_typed_func::<(), ()>(&mut cancelled_store, "run")?;
        let timed = tokio::time::timeout(
            Duration::from_millis(20),
            run.call_async(&mut cancelled_store, ()),
        )
        .await;
        assert!(timed.is_err(), "pending host call must hit host deadline");
        drop(cancelled_store);

        let replacement = Module::new(&engine, r#"(module (func (export "ok")))"#)?;
        let mut fresh_store = Store::new(&engine, ());
        fresh_store.set_epoch_deadline(u64::MAX);
        let fresh = Instance::new_async(&mut fresh_store, &replacement, &[]).await?;
        fresh
            .get_typed_func::<(), ()>(&mut fresh_store, "ok")?
            .call_async(&mut fresh_store, ())
            .await?;
        Ok(())
    }
}
