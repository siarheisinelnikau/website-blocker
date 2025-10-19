use chrono::{DateTime, Utc};
use website_blocker::{
    model::*,
    native,
    store::{Change, Store},
};

fn at(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}
fn schedule(days: &[u8], start: u16, end: u16, zone: &str) -> Schedule {
    Schedule {
        days: days.to_vec(),
        start_minute: start,
        end_minute: end,
        timezone: zone.into(),
    }
}

#[test]
fn domains_paths_and_allowlists_are_not_substring_matches() {
    let pattern = Pattern::parse("Example.com/work").unwrap();
    for url in [
        "https://example.com/work",
        "https://a.example.com/work/item?x=1",
        "http://example.com:8080/work",
    ] {
        assert!(pattern.matches(&url.parse().unwrap()), "{url}");
    }
    for url in [
        "https://example.com.evil.org/work",
        "https://notexample.com/work",
        "https://example.com/Work",
        "ftp://example.com/work",
    ] {
        assert!(!pattern.matches(&url.parse().unwrap()), "{url}");
    }
    assert!(
        !Pattern::parse("=example.com")
            .unwrap()
            .matches(&"https://a.example.com/".parse().unwrap())
    );
    assert_eq!(
        Pattern::parse("пример.рф").unwrap().domain,
        "xn--e1afmkfd.xn--p1ai"
    );
    for invalid in [
        "https://example.com",
        "foo",
        "example.com?x=1",
        "example.com@evil.org",
        "127.0.0.1",
        "example.com\\evil",
    ] {
        assert!(Pattern::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn generated_browser_matchers_share_the_normalized_url_contract() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        pattern: String,
        regex: String,
        allow: Vec<String>,
        deny: Vec<String>,
    }
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("../extension/tests/fixtures/matchers.json")).unwrap();
    for fixture in fixtures {
        let pattern = Pattern::parse(&fixture.pattern).unwrap();
        assert_eq!(pattern.regex(), fixture.regex);
        for url in fixture.allow {
            assert!(pattern.matches(&url.parse().unwrap()), "{url}");
        }
        for url in fixture.deny {
            assert!(!pattern.matches(&url.parse().unwrap()), "{url}");
        }
    }
}

#[test]
fn overnight_week_boundary_and_exclusive_end() {
    let s = schedule(&[6], 23 * 60, 60, "Europe/Minsk");
    assert!(s.active_wall(6, 23 * 60));
    assert!(s.active_wall(0, 59));
    assert!(!s.active_wall(0, 60));
    assert!(!s.active_wall(0, 23 * 60));
}

#[test]
fn policy_preserves_precise_session_expiry_and_breaks() {
    let now = at("2026-10-31T09:00:17Z");
    let mut state = State::default();
    state.start_focus(1, 1, false, now.timestamp()).unwrap();
    state.pause_until = now.timestamp() + 20;
    let policy = state.policy(now);
    assert_eq!(policy.focus.unwrap().end, now.timestamp() + 60);
    assert_eq!(policy.pause_until, now.timestamp() + 20);
}

#[test]
fn browser_regex_capacity_is_checked_across_all_lists() {
    let mut state = State::default();
    let template = state.groups[0].clone();
    state.groups = (1..=20)
        .map(|id| Group {
            id,
            patterns: vec![template.patterns[0].clone(); 50],
            ..template.clone()
        })
        .collect();
    assert!(state.validate().is_ok());
    state.allowlist = vec![Pattern::parse("help.example.com").unwrap()];
    assert!(state.validate().is_err());
}

#[test]
fn locked_sessions_and_break_cooldowns_survive_persistence() {
    std::fs::create_dir_all(".test-output").unwrap();
    let temp = tempfile::tempdir_in(".test-output").unwrap();
    let store = Store {
        dir: temp.path().into(),
    };
    store
        .apply(
            Change::Start {
                group_id: 1,
                minutes: 25,
                locked: true,
            },
            1000,
        )
        .unwrap();
    assert!(store.apply(Change::Stop("END FOCUS".into()), 1001).is_err());
    assert!(
        store
            .apply(Change::Break("TAKE A BREAK".into()), 1001)
            .is_err()
    );
    assert!(
        store
            .apply(
                Change::Rules {
                    revision: 1,
                    groups: vec![],
                    allowlist: vec![]
                },
                1001
            )
            .is_err()
    );
    store.apply(Change::Stop("END FOCUS".into()), 2500).unwrap();
    store
        .apply(Change::Break("TAKE A BREAK".into()), 2501)
        .unwrap();
    assert!(
        store
            .apply(Change::Break("TAKE A BREAK".into()), 2802)
            .is_err()
    );
    assert_eq!(store.load().unwrap().pause_until, 2801);
}

#[test]
fn corrupt_files_and_stale_writes_are_never_overwritten() {
    std::fs::create_dir_all(".test-output").unwrap();
    let temp = tempfile::tempdir_in(".test-output").unwrap();
    let store = Store {
        dir: temp.path().into(),
    };
    store.apply(Change::StartupUpdates(false), 1000).unwrap();
    assert!(
        store
            .apply(
                Change::Rules {
                    revision: 0,
                    groups: State::default().groups,
                    allowlist: vec![]
                },
                1000
            )
            .is_err()
    );
    std::fs::write(temp.path().join("state.json"), b"{broken").unwrap();
    assert!(store.apply(Change::StartupUpdates(true), 1000).is_err());
    assert_eq!(
        std::fs::read(temp.path().join("state.json")).unwrap(),
        b"{broken"
    );
}

#[test]
fn native_framing_rejects_mutations_and_oversize_without_system_effects() {
    std::fs::create_dir_all(".test-output").unwrap();
    let temp = tempfile::tempdir_in(".test-output").unwrap();
    let store = Store {
        dir: temp.path().into(),
    };
    for command in ["snapshot", "stop_focus"] {
        let body = format!(r#"{{"protocol":1,"command":"{command}"}}"#).into_bytes();
        let frame = [(body.len() as u32).to_ne_bytes().as_slice(), &body].concat();
        let mut output = Vec::new();
        native::serve(frame.as_slice(), &mut output, &store).unwrap();
        assert_eq!(
            u32::from_ne_bytes(output[..4].try_into().unwrap()) as usize,
            output.len() - 4
        );
        let response: serde_json::Value = serde_json::from_slice(&output[4..]).unwrap();
        assert_eq!(response["error"].is_null(), command == "snapshot");
    }
    assert!(native::serve(u32::MAX.to_ne_bytes().as_slice(), Vec::new(), &store).is_err());
    assert!(native::extension_origin(
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"
    ));
    assert!(!native::extension_origin("chrome-extension://evil/"));
}
