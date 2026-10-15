use super::*;

#[tokio::test]
async fn output_is_read_up_to_the_cap() {
    let data = vec![7u8; 100_000];
    let out = read_capped(&data[..], 200_000).await.unwrap().unwrap();
    assert_eq!(out.as_slice(), &data[..]);
    let exact = read_capped(&data[..], 100_000).await.unwrap().unwrap();
    assert_eq!(exact.len(), 100_000);
}

#[tokio::test]
async fn output_past_the_cap_is_drained_and_refused() {
    let data = vec![7u8; 100_000];
    assert!(read_capped(&data[..], 99_999).await.unwrap().is_none());
    assert!(read_capped(&b""[..], 0).await.unwrap().unwrap().is_empty());
}

#[tokio::test]
async fn a_missing_program_is_a_spawn_error() {
    let missing = std::env::temp_dir().join(format!("reach-no-such-program-{}", uuid::Uuid::new_v4()));
    let r = run(silent_command(&missing), Duration::from_secs(5), 1024, 1024).await;
    assert!(matches!(r, Err(RunError::Spawn(_))));
}

/// The test binary itself, asked to list its tests: a real child whose
/// output is collected.
#[tokio::test]
async fn a_child_is_run_and_its_output_collected() {
    let me = std::env::current_exe().unwrap();
    let mut cmd = silent_command(&me);
    cmd.args(["--list", "--exact", "pwmanager::process::tests::a_child_is_run_and_its_output_collected"]);
    let out = run(cmd, Duration::from_secs(30), 1024 * 1024, 64 * 1024).await.unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("a_child_is_run_and_its_output_collected"));

    let mut cmd = silent_command(&me);
    cmd.args(["--list"]);
    assert!(matches!(run(cmd, Duration::from_secs(30), 16, 64 * 1024).await, Err(RunError::TooLarge)));
}

#[tokio::test]
async fn at_most_two_logins_run_at_once() {
    assert_eq!(MAX_CONCURRENT_RUNS, 2);
    let a = cli_slot().await.unwrap();
    let b = cli_slot().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), cli_slot()).await.is_err(),
        "a third waits"
    );
    drop(a);
    let c = tokio::time::timeout(Duration::from_secs(5), cli_slot()).await.expect("a slot came free").unwrap();
    drop((b, c));
}

/// Not a check of its own: run as a child by
/// `closing_the_job_kills_the_child`, it waits a long time.
#[cfg(windows)]
#[test]
fn child_waits_when_asked() {
    if std::env::var_os("REACH_TEST_PWM_WAIT").is_some() {
        std::thread::sleep(Duration::from_secs(120));
    }
}

/// What happens when Reach exits mid-call: the job's last handle closes and
/// Windows kills the CLI, secrets and all.
#[cfg(windows)]
#[tokio::test]
async fn closing_the_job_kills_the_child() {
    let me = std::env::current_exe().unwrap();
    let mut cmd = silent_command(&me);
    cmd.env("REACH_TEST_PWM_WAIT", "1");
    cmd.args(["--exact", "pwmanager::process::tests::child_waits_when_asked", "--test-threads=1"]);
    let mut child = cmd.spawn().unwrap();
    let job = super::job::KillOnClose::for_child(&child).expect("the child is put in a job");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(child.try_wait().unwrap().is_none(), "the child is waiting");
    drop(job);
    // It would wait two minutes on its own. (A job kills with exit code 0,
    // so the time is what tells.)
    tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("killed when the job closed")
        .unwrap();
}
