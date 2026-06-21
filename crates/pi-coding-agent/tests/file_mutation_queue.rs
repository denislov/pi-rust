use pi_coding_agent::tools::file_mutation_queue::with_file_mutation_queue;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[tokio::test]
async fn serializes_mutations_for_the_same_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.txt");
    std::fs::write(&path, "").unwrap();

    let first_entered = Arc::new(AtomicBool::new(false));
    let first_released = Arc::new(AtomicBool::new(false));
    let second_saw_first_entered = Arc::new(AtomicBool::new(false));
    let second_saw_first_released = Arc::new(AtomicBool::new(false));

    let first = {
        let path = path.clone();
        let first_entered = first_entered.clone();
        let first_released = first_released.clone();
        tokio::spawn(async move {
            with_file_mutation_queue(&path, || async {
                first_entered.store(true, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(50)).await;
                first_released.store(true, Ordering::SeqCst);
                Ok::<_, String>(())
            })
            .await
            .unwrap();
        })
    };

    while !first_entered.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }

    let second = {
        let path = path.clone();
        let first_entered = first_entered.clone();
        let first_released = first_released.clone();
        let second_saw_first_entered = second_saw_first_entered.clone();
        let second_saw_first_released = second_saw_first_released.clone();
        tokio::spawn(async move {
            with_file_mutation_queue(&path, || async {
                second_saw_first_entered
                    .store(first_entered.load(Ordering::SeqCst), Ordering::SeqCst);
                second_saw_first_released
                    .store(first_released.load(Ordering::SeqCst), Ordering::SeqCst);
                Ok::<_, String>(())
            })
            .await
            .unwrap();
        })
    };

    first.await.unwrap();
    second.await.unwrap();

    assert!(second_saw_first_entered.load(Ordering::SeqCst));
    assert!(second_saw_first_released.load(Ordering::SeqCst));
}

#[tokio::test]
async fn allows_mutations_for_different_files_to_overlap() {
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.txt");
    let right = dir.path().join("right.txt");
    std::fs::write(&left, "").unwrap();
    std::fs::write(&right, "").unwrap();

    let left_entered = Arc::new(AtomicBool::new(false));
    let right_entered = Arc::new(AtomicBool::new(false));
    let started = Instant::now();

    let left_task = {
        let left = left.clone();
        let left_entered = left_entered.clone();
        tokio::spawn(async move {
            with_file_mutation_queue(&left, || async {
                left_entered.store(true, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(75)).await;
                Ok::<_, String>(())
            })
            .await
            .unwrap();
        })
    };

    while !left_entered.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }

    let right_task = {
        let right = right.clone();
        let right_entered = right_entered.clone();
        tokio::spawn(async move {
            with_file_mutation_queue(&right, || async {
                right_entered.store(true, Ordering::SeqCst);
                Ok::<_, String>(())
            })
            .await
            .unwrap();
        })
    };

    right_task.await.unwrap();
    left_task.await.unwrap();

    assert!(right_entered.load(Ordering::SeqCst));
    assert!(started.elapsed() < Duration::from_millis(140));
}
