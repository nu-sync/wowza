pub(super) fn checkpoint() {
    #[cfg(feature = "test-hooks")]
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static STEP: AtomicUsize = AtomicUsize::new(0);
        let step = STEP.fetch_add(1, Ordering::SeqCst) + 1;
        if std::env::var("WOWZA_TEST_STOP_AFTER")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            == Some(step)
        {
            std::process::exit(86);
        }
    }
}
