use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;

use super::*;

fn key(account: u64) -> CacheKey {
    CacheKey::for_test(ProviderId::Passbolt, account)
}

fn pass(s: &str) -> Arc<Passphrase> {
    Arc::new(Passphrase::new(s.into()).unwrap())
}

/// A fresh unlocker; leaked, as the deadline timer needs `'static`.
fn unlocker(prompt_timeout: Duration) -> &'static Unlocker {
    Box::leak(Box::new(Unlocker::new(prompt_timeout)))
}

fn policy(account: u64, remember_secs: u32) -> Option<Policy> {
    Some(Policy { account: key(account), remember_secs })
}

/// What the scripted user does with each question, in turn.
#[derive(Clone, Copy)]
enum Step {
    Enter(&'static str),
    Cancel,
    /// Never answers.
    Hang,
}

/// A user answering questions from a script, after `delay`.
struct Scripted {
    steps: Mutex<VecDeque<Step>>,
    asked: Mutex<Vec<PromptRequest>>,
    delay: Duration,
}

impl Scripted {
    fn new(steps: &[Step]) -> Arc<Self> {
        Self::slow(steps, Duration::ZERO)
    }

    fn slow(steps: &[Step], delay: Duration) -> Arc<Self> {
        Arc::new(Self { steps: Mutex::new(steps.iter().copied().collect()), asked: Mutex::new(Vec::new()), delay })
    }

    fn asked(&self) -> Vec<PromptRequest> {
        lock(&self.asked).clone()
    }
}

impl PassphraseAsker for Scripted {
    fn ask(&self, request: PromptRequest) -> Pin<Box<dyn Future<Output = Answer> + Send + '_>> {
        Box::pin(async move {
            lock(&self.asked).push(request);
            tokio::time::sleep(self.delay).await;
            let step = lock(&self.steps).pop_front().unwrap_or(Step::Hang);
            match step {
                Step::Enter(s) => Answer::Entered(Passphrase::new(s.into()).unwrap()),
                Step::Cancel => Answer::Cancelled,
                Step::Hang => std::future::pending().await,
            }
        })
    }
}

fn asker(s: &Arc<Scripted>) -> Option<Arc<dyn PassphraseAsker>> {
    Some(s.clone() as Arc<dyn PassphraseAsker>)
}

/// A call that succeeds with whatever passphrase it is handed.
async fn echo(p: Option<Arc<Passphrase>>) -> Result<String, Failure> {
    Ok(p.map(|p| p.reveal().to_string()).unwrap_or_else(|| "<none>".into()))
}

/// A call the manager answers like Passbolt: only "right" unlocks the key.
async fn only_right(p: Option<Arc<Passphrase>>) -> Result<String, Failure> {
    let typed = p.expect("a passphrase").reveal();
    if typed.as_str() == "right" {
        Ok(typed.to_string())
    } else {
        Err(Failure::WrongPassphrase("Passbolt: could not unlock the private key: wrong passphrase, or not a valid Passbolt key.".into()))
    }
}

// ---------------------------------------------------------------------------
// The passphrase itself

#[test]
fn a_passphrase_is_held_shielded_and_opens_unchanged() {
    for s in ["p", "correct horse battery staple", "pässwörd ✓ 密码", &"x".repeat(MAX_PASSPHRASE_BYTES)] {
        let p = Passphrase::new(s.to_string()).unwrap();
        assert_eq!(p.reveal().as_str(), s);
        assert_eq!(p.with_str(str::len), s.len());
    }
    let p = Passphrase::new("hunter2-passphrase".into()).unwrap();
    let debug = format!("{p:?}");
    assert!(!debug.contains("hunter2"), "{debug}");
    assert_ne!(Passphrase::new("a".into()).unwrap().serial(), Passphrase::new("a".into()).unwrap().serial());
}

#[test]
fn a_passphrase_must_fit_and_be_text() {
    assert!(Passphrase::new(String::new()).is_err());
    assert!(Passphrase::new("x".repeat(MAX_PASSPHRASE_BYTES + 1)).is_err());
    // The limit is in bytes: 513 two-byte characters do not fit.
    assert!(Passphrase::new("é".repeat(MAX_PASSPHRASE_BYTES / 2 + 1)).is_err());
    assert!(Passphrase::new("é".repeat(MAX_PASSPHRASE_BYTES / 2)).is_ok());
    assert!(Passphrase::new("a\0b".into()).is_err());
}

// ---------------------------------------------------------------------------
// The cache

#[test]
fn a_kept_passphrase_is_for_its_own_account_only() {
    let cache = PassphraseCache::new();
    let g = cache.generation();
    let p = pass("one");
    assert!(cache.store(key(1), p.clone(), g, Duration::from_secs(60)).is_some());
    assert_eq!(cache.get(key(1)).unwrap().serial(), p.serial());
    assert!(cache.holds(key(1)));
    // Another account (the settings changed) gets nothing, and the one kept
    // is dropped as of no use.
    assert!(cache.get(key(2)).is_none());
    assert!(cache.get(key(1)).is_none());
}

#[test]
fn a_kept_passphrase_expires_at_a_fixed_deadline() {
    let cache = PassphraseCache::new();
    let now = Instant::now();
    let p = pass("one");
    let until = cache.store_at(key(1), p.clone(), cache.generation(), Duration::from_secs(300), now).unwrap();
    assert_eq!(until, now + Duration::from_secs(300));
    assert!(cache.get_at(key(1), now + Duration::from_secs(299)).is_some());
    // Used meanwhile, it still goes at the deadline: storing the same
    // typing again does not push it out.
    let again = cache.store_at(key(1), p.clone(), cache.generation(), Duration::from_secs(300), now + Duration::from_secs(200));
    assert_eq!(again, Some(until));
    assert!(cache.get_at(key(1), until).is_none());
    assert!(cache.get(key(1)).is_none(), "gone once read after its time");
}

#[test]
fn the_deadline_timer_drops_only_its_own_passphrase_when_due() {
    let cache = PassphraseCache::new();
    let now = Instant::now();
    let p = pass("one");
    cache.store_at(key(1), p.clone(), cache.generation(), Duration::from_secs(60), now);
    cache.expire_at(p.serial(), now + Duration::from_secs(59));
    assert!(cache.get_at(key(1), now + Duration::from_secs(1)).is_some(), "not yet due");
    cache.expire_at(p.serial() + 1000, now + Duration::from_secs(61));
    assert!(cache.get_at(key(1), now + Duration::from_secs(1)).is_some(), "another typing's timer");
    cache.expire_at(p.serial(), now + Duration::from_secs(60));
    assert!(cache.get_at(key(1), now + Duration::from_secs(1)).is_none());
}

#[test]
fn ask_every_time_keeps_nothing() {
    let cache = PassphraseCache::new();
    assert!(cache.store(key(1), pass("one"), cache.generation(), Duration::ZERO).is_none());
    assert!(!cache.holds(key(1)));
}

#[test]
fn forgetting_clears_and_refuses_answers_to_older_questions() {
    let cache = PassphraseCache::new();
    let before = cache.generation();
    cache.store(key(1), pass("one"), before, Duration::from_secs(60));
    cache.forget();
    assert!(!cache.holds(key(1)));
    // Asked before the vault was locked (or the settings saved), answered
    // after: not kept.
    assert!(cache.store(key(1), pass("late"), before, Duration::from_secs(60)).is_none());
    assert!(!cache.holds(key(1)));
    assert!(cache.store(key(1), pass("new"), cache.generation(), Duration::from_secs(60)).is_some());
}

#[test]
fn a_rejection_forgets_only_the_passphrase_that_was_rejected() {
    let cache = PassphraseCache::new();
    let old = pass("old");
    let new = pass("new");
    cache.store(key(1), old.clone(), cache.generation(), Duration::from_secs(60));
    cache.store(key(1), new.clone(), cache.generation(), Duration::from_secs(60));
    assert!(!cache.forget_serial(old.serial()), "already replaced by one typed since");
    assert!(cache.holds(key(1)));
    assert!(cache.forget_serial(new.serial()));
    assert!(!cache.holds(key(1)));
}

#[test]
fn the_app_wide_forget_clears_the_global_cache() {
    let account = key(0xF0F0_F0F0_0BAD_CAFE);
    let cache = &global().cache;
    cache.store(account, pass("kept"), cache.generation(), Duration::from_secs(60));
    assert!(cache.holds(account));
    // What "Forget now", a settings save, a vault lock and a new identity
    // call.
    crate::pwmanager::forget_passphrase();
    assert!(!cache.holds(account));
}

// ---------------------------------------------------------------------------
// Asking

#[tokio::test]
async fn no_passphrase_is_asked_when_none_is_needed() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[]);
    let got = operation(asker(&user), Reason::List, u.with_passphrase(None, echo)).await.unwrap();
    assert_eq!(got, "<none>");
    assert!(user.asked().is_empty());
}

#[tokio::test]
async fn an_entered_passphrase_is_kept_for_the_chosen_time() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("typed")]);
    let reason = Reason::Connect("web-01".into());
    let got = operation(asker(&user), reason.clone(), u.with_passphrase(policy(1, 900), echo)).await.unwrap();
    assert_eq!(got, "typed");
    let asked = user.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].reason, reason);
    assert_eq!(asked[0].provider, ProviderId::Passbolt);
    assert_eq!(asked[0].remember_secs, 900);
    assert!(!asked[0].retry && asked[0].error.is_none());
    assert!(u.cache.holds(key(1)));
    // The next operation is not asked.
    let again = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), echo)).await.unwrap();
    assert_eq!(again, "typed");
    assert_eq!(user.asked().len(), 1);
}

#[tokio::test]
async fn ask_every_time_asks_once_per_operation() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("first"), Step::Enter("second")]);
    // One connect resolving three entries (target, jump host, proxy): one
    // question.
    let three = operation(asker(&user), Reason::Connect("db".into()), async {
        let a = u.with_passphrase(policy(1, 0), echo).await?;
        let b = u.with_passphrase(policy(1, 0), echo).await?;
        // A nested operation (an SSH leg inside a database connect) is part
        // of the outer one.
        let c = operation(None, Reason::Other, u.with_passphrase(policy(1, 0), echo)).await?;
        Ok::<_, String>([a, b, c])
    })
    .await
    .unwrap();
    assert_eq!(three, ["first", "first", "first"]);
    assert_eq!(user.asked().len(), 1);
    assert!(!u.cache.holds(key(1)), "nothing kept after the operation");
    // The next connect asks again.
    let next = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 0), echo)).await.unwrap();
    assert_eq!(next, "second");
    assert_eq!(user.asked().len(), 2);
}

#[tokio::test]
async fn concurrent_callers_share_one_question() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::slow(&[Step::Enter("shared"), Step::Enter("unexpected")], Duration::from_millis(50));
    let calls = AtomicUsize::new(0);
    let calls = &*Box::leak(Box::new(calls));
    let mut tasks = Vec::new();
    for i in 0..4 {
        let a = asker(&user);
        tasks.push(tokio::spawn(operation(a, Reason::Connect(format!("host-{i}")), async move {
            calls.fetch_add(1, Ordering::SeqCst);
            u.with_passphrase(policy(1, 0), echo).await
        })));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap().unwrap(), "shared");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(user.asked().len(), 1, "one question for all four");
    assert!(!u.hub.is_open(key(1)));
}

#[tokio::test]
async fn cancelling_fails_the_operation_and_every_caller_waiting_on_it() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::slow(&[Step::Cancel], Duration::from_millis(30));
    let a = tokio::spawn(operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), echo)));
    let b = tokio::spawn(operation(asker(&user), Reason::Test, u.with_passphrase(policy(1, 900), echo)));
    assert_eq!(a.await.unwrap().unwrap_err(), "Passbolt: cancelled");
    assert_eq!(b.await.unwrap().unwrap_err(), "Passbolt: cancelled");
    assert_eq!(user.asked().len(), 1);
    assert!(!u.cache.holds(key(1)));
}

#[tokio::test]
async fn an_unanswered_question_times_out() {
    let u = unlocker(Duration::from_millis(50));
    let user = Scripted::new(&[Step::Hang]);
    let e = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), echo)).await.unwrap_err();
    assert_eq!(e, "Passbolt: passphrase not entered");
    assert!(!u.hub.is_open(key(1)), "the question is withdrawn");
}

#[tokio::test]
async fn without_a_window_nothing_can_be_asked() {
    let u = unlocker(Duration::from_secs(5));
    let e = u.with_passphrase(policy(1, 900), echo).await.unwrap_err();
    assert!(e.starts_with("Passbolt: ") && e.contains("no window"), "{e}");
}

#[tokio::test]
async fn a_caller_whose_asker_went_away_asks_again() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::slow(&[Step::Hang, Step::Enter("second try")], Duration::from_millis(10));
    let first = tokio::spawn(operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 0), echo)));
    tokio::time::sleep(Duration::from_millis(5)).await;
    let second = tokio::spawn(operation(asker(&user), Reason::Test, u.with_passphrase(policy(1, 0), echo)));
    tokio::time::sleep(Duration::from_millis(30)).await;
    // The first connect is given up while its question is open.
    first.abort();
    assert_eq!(second.await.unwrap().unwrap(), "second try");
    assert_eq!(user.asked().len(), 2);
}

#[tokio::test]
async fn a_rejected_passphrase_is_forgotten_and_asked_for_again() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("wrong"), Step::Enter("right")]);
    let got = operation(asker(&user), Reason::Connect("web-01".into()), u.with_passphrase(policy(1, 900), only_right))
        .await
        .unwrap();
    assert_eq!(got, "right");
    let asked = user.asked();
    assert_eq!(asked.len(), 2);
    assert!(asked[1].retry);
    assert!(asked[1].error.as_deref().unwrap().contains("wrong passphrase"));
    assert_eq!(u.cache.get(key(1)).unwrap().reveal().as_str(), "right");
}

#[tokio::test]
async fn a_kept_passphrase_that_is_rejected_is_dropped_from_the_cache() {
    let u = unlocker(Duration::from_secs(5));
    let stale = pass("stale");
    u.cache.store(key(1), stale.clone(), u.cache.generation(), Duration::from_secs(900));
    let user = Scripted::new(&[Step::Enter("right")]);
    let got = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), only_right)).await.unwrap();
    assert_eq!(got, "right");
    let asked = user.asked();
    assert_eq!(asked.len(), 1);
    assert!(asked[0].retry, "the question says the kept one was rejected");
    assert_ne!(u.cache.get(key(1)).unwrap().serial(), stale.serial());
}

#[tokio::test]
async fn three_rejections_fail_the_operation() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("a"), Step::Enter("b"), Step::Enter("c"), Step::Enter("right")]);
    let e = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), only_right)).await.unwrap_err();
    assert!(e.contains("wrong passphrase"), "{e}");
    assert_eq!(user.asked().len(), MAX_ATTEMPTS as usize);
    assert!(!u.cache.holds(key(1)), "no rejected passphrase is kept");
}

#[tokio::test]
async fn other_failures_are_not_retried() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("typed"), Step::Enter("unexpected")]);
    let fail = |_p: Option<Arc<Passphrase>>| async { Err::<(), _>(Failure::Other("Passbolt: entry not found.".into())) };
    let e = operation(asker(&user), Reason::List, u.with_passphrase(policy(1, 900), fail)).await.unwrap_err();
    assert_eq!(e, "Passbolt: entry not found.");
    assert_eq!(user.asked().len(), 1);
    // The passphrase was fine: it is kept.
    assert!(u.cache.holds(key(1)));
}

#[tokio::test]
async fn the_question_names_what_is_connected_to_once_known() {
    let u = unlocker(Duration::from_secs(5));
    let user = Scripted::new(&[Step::Enter("typed")]);
    operation(asker(&user), Reason::Connect("10.0.0.5".into()), async {
        name_operation("Office desktop");
        u.with_passphrase(policy(1, 0), echo).await
    })
    .await
    .unwrap();
    assert_eq!(user.asked()[0].reason, Reason::Connect("Office desktop".into()));
    assert_eq!(user.asked()[0].reason.code(), "connect");
    assert_eq!(user.asked()[0].reason.target(), Some("Office desktop"));
}

#[test]
fn an_answer_to_no_open_question_is_refused() {
    let e = crate::pwmanager::prompt::answer("no-such-prompt", Some("secret".into())).unwrap_err();
    assert!(!e.contains("secret"), "{e}");
    assert!(crate::pwmanager::prompt::answer("no-such-prompt", None).is_err());
    // Too long is refused before anything is looked up.
    let e = crate::pwmanager::prompt::answer("no-such-prompt", Some("x".repeat(MAX_PASSPHRASE_BYTES + 1))).unwrap_err();
    assert!(e.contains("too long"), "{e}");
}
