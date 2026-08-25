use bao_daemon::pid::PidFile;

#[test]
fn second_acquire_fails() {
    let dir = std::env::temp_dir().join("bao-pid-test");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("daemon.pid");

    let a = PidFile::acquire(&p).unwrap();
    match PidFile::acquire(&p) {
        Err(e) => assert!(e.to_string().contains("already running"), "{e}"),
        Ok(_) => panic!("second acquire should fail"),
    }
    assert_eq!(
        a.pid().to_string(),
        std::fs::read_to_string(&p).unwrap().trim()
    );
    drop(a);

    // Lock released on drop; the leftover file stays but is acquirable and
    // overwritten by the new holder.
    let _b = PidFile::acquire(&p).unwrap();
}
