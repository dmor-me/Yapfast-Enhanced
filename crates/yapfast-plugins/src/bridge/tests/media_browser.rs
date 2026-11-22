//! Native media-browser calls must cross the same permission and gesture gates as other calls.
use super::*;
#[test]
fn gif_favorites_and_json_files_need_their_declared_capabilities() {
    for (method, payload, cap) in [
        (
            call::method::GIFS_FAVORITES,
            serde_json::json!({"offset":200,"limit":200}),
            "media.read",
        ),
        (
            call::method::FILES_OPEN_TEXT,
            serde_json::json!({}),
            "files.open",
        ),
        (
            call::method::FILES_SAVE_TEXT,
            serde_json::json!({"name":"collection.json","text":"{}"}),
            "files.save",
        ),
    ] {
        let (answer, _) = ask_action(method, payload.clone(), &[]);
        assert!(
            matches!(answer,HostReturn::Denied{reason} if reason.contains(cap)),
            "{method}"
        );
        let (answer, notices) = ask_action(method, payload, &[cap]);
        assert!(
            matches!(answer, HostReturn::Pending { .. }),
            "{method}: {answer:?}"
        );
        assert!(
            matches!(
                notices.recv_timeout(Duration::from_secs(2)),
                Ok(Notice::Call { .. })
            ),
            "{method}"
        );
    }
}
#[test]
fn native_json_choosers_need_a_recent_click_even_with_permission() {
    for (method, cap, payload) in [
        (
            call::method::FILES_OPEN_TEXT,
            "files.open",
            serde_json::json!({}),
        ),
        (
            call::method::FILES_SAVE_TEXT,
            "files.save",
            serde_json::json!({"name":"collection.json","text":"{}"}),
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let plugin = installed(
            directory.path(),
            &action_manifest(&[cap]),
            b"not a real module",
        );
        let (wake, notices) = notice_channel();
        let shared = shared(directory.path(), wake);
        let (_dir, mut session, written, _inbound) = session_stub();
        runtime().block_on(async {
            session.on_call(
                &shared,
                11,
                plugin,
                HostCall {
                    method: method.into(),
                    payload,
                },
            );
        });
        assert!(
            matches!(returned(&next_frame(&written)).1,HostReturn::Denied{reason} if reason.contains("click"))
        );
        assert!(notices.recv_timeout(Duration::from_millis(50)).is_err());
    }
}
#[test]
fn native_json_save_rejects_paths_and_oversized_contents() {
    for payload in [
        serde_json::json!({"name":"../secret.json","text":"{}"}),
        serde_json::json!({"name":"data.json","text":"x".repeat(900001)}),
    ] {
        let (answer, notices) = ask_action(call::method::FILES_SAVE_TEXT, payload, &["files.save"]);
        assert!(
            matches!(answer, HostReturn::Denied { .. } | HostReturn::Error { .. }),
            "{answer:?}"
        );
        assert!(notices.recv_timeout(Duration::from_millis(50)).is_err());
    }
}
