use super::*;

#[test]
fn cancel_all_answers_every_open_question_with_cancel() {
    let (tx1, mut rx1) = tokio::sync::oneshot::channel();
    let (tx2, mut rx2) = tokio::sync::oneshot::channel();
    let ids = [format!("test-{}", uuid::Uuid::new_v4()), format!("test-{}", uuid::Uuid::new_v4())];
    pending().insert(ids[0].clone(), Pending { reply: tx1, app: None });
    pending().insert(ids[1].clone(), Pending { reply: tx2, app: None });
    // (Another test forgetting at the same moment may cancel them first:
    // the outcome is the same.)
    cancel_all();
    assert!(matches!(rx1.try_recv(), Ok(None)), "answered with Cancel");
    assert!(matches!(rx2.try_recv(), Ok(None)), "answered with Cancel");
    {
        let open = pending();
        assert!(!open.contains_key(&ids[0]) && !open.contains_key(&ids[1]), "no longer open");
    }
    // An answer that comes after is refused.
    assert!(answer(&ids[0], Some("late".into())).is_err());
}

#[test]
fn forgetting_the_passphrase_cancels_open_questions() {
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let id = format!("test-{}", uuid::Uuid::new_v4());
    pending().insert(id.clone(), Pending { reply: tx, app: None });
    // What "Forget now", a settings save, a vault lock and the computer
    // locking call.
    crate::pwmanager::forget_passphrase();
    assert!(matches!(rx.try_recv(), Ok(None)));
    assert!(!pending().contains_key(&id));
}
