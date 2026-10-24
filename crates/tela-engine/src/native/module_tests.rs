use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[tokio::test]
async fn completed_undrained_calls_still_consume_the_budget() {
    let finished = Arc::new(AtomicUsize::new(0));
    let counter = finished.clone();
    let (runtime, mut completions) =
        NativeModuleRuntime::new(vec![NativeModule::new("test", "test", move |_, _, _| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(Value::Null)
            }
        })])
        .unwrap();
    let permissions = vec!["test".to_string()];
    for id in 1..=MAX_PENDING as u64 {
        assert!(runtime
            .call(id, "test".into(), "read".into(), "{}".into(), &permissions)
            .is_none());
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while finished.load(Ordering::SeqCst) != MAX_PENDING {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        runtime.call(9, "test".into(), "read".into(), "{}".into(), &permissions),
        Some("native module overloaded".into())
    );
    let completion = tokio::time::timeout(Duration::from_secs(1), completions.recv())
        .await
        .unwrap()
        .unwrap();
    drop(completion);
    assert!(runtime
        .call(10, "test".into(), "read".into(), "{}".into(), &permissions)
        .is_none());
    runtime.cancel();
    assert_eq!(
        runtime.call(11, "test".into(), "read".into(), "{}".into(), &permissions),
        Some("native module shutting down".into())
    );
}

#[tokio::test]
async fn oversized_response_is_replaced_with_a_bounded_error() {
    let (runtime, mut completions) =
        NativeModuleRuntime::new(vec![NativeModule::new("test", "test", |_, _, _| async {
            Ok(Value::String("x".repeat(MAX_RESPONSE_BYTES + 1)))
        })])
        .unwrap();
    let permissions = vec!["test".to_string()];
    assert!(runtime
        .call(1, "test".into(), "read".into(), "{}".into(), &permissions)
        .is_none());
    let completion = tokio::time::timeout(Duration::from_secs(2), completions.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        completion.result,
        Err("native module response too large".into())
    );
    runtime.cancel();
}
