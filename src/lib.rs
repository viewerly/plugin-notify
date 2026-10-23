//! Notify: tells you in Slack, Discord or ntfy when a session needs you, a permission request or
//! waiting for your input, and, if it was allowed to see sessions ending, when one ends.
//!
//! It only tells. Nothing it posts can act on a session: no links that approve, no replies read
//! back. That is the directory's policy (viewerly.io/plugins/policy): reaching a session from
//! outside the machine is what Viewerly's own Sync and Remote control are for.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

wit_bindgen::generate!({
    world: "plugin",
    path: "wit",
});

use viewerly::plugin::host;
use viewerly::plugin::types::{Button, Field, Session, Status, Widget};

/// The settings fields, as declared in ui.toml.
const SLACK: &str = "slack";
const DISCORD: &str = "discord";
const NTFY_TOPIC: &str = "ntfy_topic";
const NTFY_SERVER: &str = "ntfy_server";
const DEFAULT_NTFY_SERVER: &str = "https://ntfy.sh";

/// The host gives each call ten seconds and each request up to eight. A destination is only
/// started while there is clearly time left for it, so one slow service cannot make the call
/// overrun (three overruns in a row and the host turns the plugin off).
const START_BY_MS: u128 = 1_500;

/// The longest summary of a permission request a message carries.
const SUMMARY: usize = 200;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Permission requests and waiting for input: high priority where the service has one.
    NeedsYou,
    Ended,
    Test,
}

struct Message {
    title: String,
    body: String,
    kind: Kind,
}

#[derive(Default)]
struct State {
    /// Each session's last status, so "waiting for input" is posted once, when it starts.
    statuses: HashMap<String, Status>,
    /// The last result for each destination, newest first, for the panel.
    results: Vec<(String, String)>,
    sent: u32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

struct Notify;

impl Guest for Notify {
    fn on_event(event: Event) -> Option<Decision> {
        match event {
            Event::Started | Event::SettingChanged(_) => refresh(),
            Event::PermissionRequested(p) => {
                let what = if p.summary.trim().is_empty() { String::new() } else { format!(": {}", one_line(&p.summary, SUMMARY)) };
                post(Message {
                    title: format!("{} needs your permission", p.session.title),
                    body: format!("{} wants to use {}{what}", place(&p.session), p.tool),
                    kind: Kind::NeedsYou,
                });
            }
            Event::StatusChanged(s) => {
                let before = STATE.with_borrow_mut(|st| st.statuses.insert(s.id.clone(), s.status));
                if s.status == Status::WaitingForInput && before != Some(Status::WaitingForInput) {
                    post(Message {
                        title: format!("{} is waiting for you", s.title),
                        body: format!("{} is waiting for your input.", place(&s)),
                        kind: Kind::NeedsYou,
                    });
                }
            }
            // Only arrives if the person allowed "See sessions ending".
            Event::SessionExited(s) => {
                STATE.with_borrow_mut(|st| st.statuses.remove(&s.id));
                post(Message { title: format!("{} has ended", s.title), body: format!("{} has ended.", place(&s)), kind: Kind::Ended });
            }
            Event::Invoked(i) if i.id == "test" => post(Message {
                title: "Viewerly test".into(),
                body: "This is a test from Viewerly's Notify plugin. Sessions that need you will be posted here.".into(),
                kind: Kind::Test,
            }),
            _ => {}
        }
        None
    }
}

/// "viewerly 1 in viewerly": which session, in which project.
fn place(s: &Session) -> String {
    format!("{} in {}", s.title, s.project)
}

/// The first line of `text`, cut to `max` characters.
fn one_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() > max { format!("{}…", line.chars().take(max).collect::<String>()) } else { line.to_owned() }
}

/// Sends `message` to every destination that is filled in, records how each went, and shows a
/// local notification for each that failed.
fn post(message: Message) {
    let started = Instant::now();
    let mut results = Vec::new();
    let destinations: [(&str, fn(&Message) -> Option<Result<(), String>>); 3] = [("Slack", slack), ("Discord", discord), ("ntfy", ntfy)];
    for (name, send) in destinations {
        if started.elapsed().as_millis() > START_BY_MS {
            if configured(name) {
                results.push((name, Err("skipped: the services before it were slow".to_owned())));
            }
            continue;
        }
        if let Some(result) = send(&message) {
            results.push((name, result));
        }
    }
    if results.is_empty() && message.kind == Kind::Test {
        host::notify("Notify has nowhere to post", "Fill in Slack, Discord or ntfy in Settings › Plugins › Notify, then send a test.");
        return;
    }
    let now = clock();
    STATE.with_borrow_mut(|st| {
        for (name, result) in &results {
            let line = match result {
                Ok(()) => {
                    st.sent += 1;
                    format!("sent at {now}")
                }
                Err(why) => format!("failed at {now}: {why}"),
            };
            st.results.retain(|(n, _)| n != name);
            st.results.insert(0, (name.to_string(), line));
        }
    });
    for (name, result) in &results {
        if let Err(why) = result {
            host::log(&format!("{name}: {why}"));
            host::notify(&format!("Notify could not reach {name}"), why);
        }
    }
    refresh();
}

fn setting(id: &str) -> Option<String> {
    host::setting(id).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

fn configured(name: &str) -> bool {
    match name {
        "Slack" => setting(SLACK).is_some(),
        "Discord" => setting(DISCORD).is_some(),
        _ => setting(NTFY_TOPIC).is_some(),
    }
}

/// An incoming webhook: `{"text": …}`. Slack reads `&`, `<` and `>` as markup.
fn slack(m: &Message) -> Option<Result<(), String>> {
    let url = setting(SLACK)?;
    if !url.starts_with("https://hooks.slack.com/") {
        return Some(Err("the Slack webhook URL should start with https://hooks.slack.com/".into()));
    }
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = json!({ "text": format!("*{}*\n{}", escape(&m.title), escape(&m.body)) });
    Some(send(&url, &[("Content-Type", "application/json")], body.to_string().into_bytes()))
}

/// A channel webhook: `{"content": …}`, with mentions turned off so a session title cannot ping
/// anyone.
fn discord(m: &Message) -> Option<Result<(), String>> {
    let url = setting(DISCORD)?;
    if !url.starts_with("https://discord.com/api/webhooks/") {
        return Some(Err("the Discord webhook URL should start with https://discord.com/api/webhooks/".into()));
    }
    let body = json!({ "content": format!("**{}**\n{}", m.title, m.body), "allowed_mentions": { "parse": [] } });
    Some(send(&url, &[("Content-Type", "application/json")], body.to_string().into_bytes()))
}

/// A POST to the topic's URL with the text as the body and the title in a header.
fn ntfy(m: &Message) -> Option<Result<(), String>> {
    let topic = setting(NTFY_TOPIC)?;
    if topic.len() > 64 || !topic.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Some(Err("an ntfy topic is up to 64 letters, digits, - and _".into()));
    }
    let server = setting(NTFY_SERVER).unwrap_or_else(|| DEFAULT_NTFY_SERVER.to_owned());
    let server = server.trim_end_matches('/');
    if !server.starts_with("https://") {
        return Some(Err("the ntfy server has to be an https:// address".into()));
    }
    let priority = if m.kind == Kind::NeedsYou { "high" } else { "default" };
    // Header values are ASCII; the title is in the body too, so a lossy header is fine.
    let title: String = m.title.chars().map(|c| if c.is_ascii() && !c.is_ascii_control() { c } else { '?' }).collect();
    let headers = [("Title", title.as_str()), ("Priority", priority), ("Tags", "computer")];
    let result = send(&format!("{server}/{topic}"), &headers, m.body.clone().into_bytes());
    // The plugin may only reach the hosts in its manifest.
    let own = server != DEFAULT_NTFY_SERVER;
    Some(result.map_err(|e| if own { format!("{e} (this build may only reach ntfy.sh; see the README to use your own server)") } else { e }))
}

fn send(url: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Result<(), String> {
    let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let response = host::http_request("POST", url, &headers, &body)?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        let text = String::from_utf8_lossy(&response.body);
        Err(format!("HTTP {}{}", response.status, if text.trim().is_empty() { String::new() } else { format!(": {}", one_line(&text, 120)) }))
    }
}

/// The time of day in UTC, "14:02 UTC". The plugin has no time zone.
fn clock() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02} UTC", secs / 3600 % 24, secs / 60 % 60)
}

/// Redraws the panel and the settings pane.
fn refresh() {
    let any = [SLACK, DISCORD, NTFY_TOPIC].iter().any(|id| setting(id).is_some());
    let mut status = Vec::new();
    STATE.with_borrow(|st| {
        if !any {
            status.push(Widget::Text("Nowhere to post yet: fill in Slack, Discord or ntfy in Settings › Plugins › Notify.".into()));
        } else if st.results.is_empty() {
            status.push(Widget::Text("Nothing posted yet.".into()));
        } else {
            status.push(Widget::KeyValue(st.results.clone()));
        }
        status.push(Widget::Text(format!("{} posted since Viewerly started.", st.sent)));
    });
    let test = Widget::Button(Button { id: "test".into(), label: "Send Test".into() });

    let mut panel = status.clone();
    panel.push(test.clone());
    host::set_panel("activity", &panel);

    let mut pane = vec![
        Widget::Markdown(
            "Fill in any of these. Each message says which session needs you and why, and nothing else.\n\n\
             To be told when sessions end too, allow **See sessions ending** for Notify in Settings › Plugins."
                .into(),
        ),
        field(SLACK, "Slack webhook URL", "https://hooks.slack.com/services/…", true),
        field(DISCORD, "Discord webhook URL", "https://discord.com/api/webhooks/…", true),
        field(NTFY_TOPIC, "ntfy topic", "a long, hard-to-guess name", true),
        field(NTFY_SERVER, "ntfy server", DEFAULT_NTFY_SERVER, false),
        test,
    ];
    pane.extend(status);
    host::set_panel("destinations", &pane);
}

fn field(id: &str, label: &str, placeholder: &str, secret: bool) -> Widget {
    Widget::Field(Field { id: id.into(), label: label.into(), placeholder: placeholder.into(), secret })
}

export!(Notify);

// The pure parts, run natively with `cargo test` (the host functions are only there in WASM).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_are_one_short_line() {
        assert_eq!(one_line("\n  git push --force\nsecond line", 200), "git push --force");
        assert_eq!(one_line("abcdef", 3), "abc…");
        assert_eq!(one_line("", 10), "");
    }

    #[test]
    fn messages_name_the_session_and_project() {
        let s = Session { id: "1".into(), project: "viewerly".into(), title: "viewerly 2".into(), agent: "claude-code".into(), status: Status::WaitingForInput };
        assert_eq!(place(&s), "viewerly 2 in viewerly");
        assert!(clock().ends_with(" UTC"));
    }
}
