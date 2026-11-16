//! Pipeline threads report errors and panics to the coordinator.

use crossbeam_channel::unbounded;

use crate::process_graphs::threading::Supervisor;

#[test]
fn wait_fails_when_a_thread_fails() {
    let mut sup = Supervisor::default();
    sup.spawn("ok".into(), || Ok(())).unwrap();
    sup.spawn("err".into(), || anyhow::bail!("boom")).unwrap();
    #[allow(unreachable_code)]
    sup.spawn("panic".into(), || panic!("kaboom")).unwrap();

    // a signal that never comes: the failure ends the wait instead of hanging
    let (_tx, rx) = unbounded::<()>();
    let err = sup.wait(&rx).unwrap_err().to_string();
    assert!(err == "err: boom" || err == "panic panicked", "{err}");
    assert!(sup.join().is_err());
}

#[test]
fn wait_returns_the_signal() {
    let mut sup = Supervisor::default();
    let (tx, rx) = unbounded();
    sup.spawn("sender".into(), move || {
        tx.send(5).unwrap();
        Ok(())
    })
    .unwrap();
    assert_eq!(sup.wait(&rx).unwrap(), 5);
    assert!(sup.join().is_ok());
}
