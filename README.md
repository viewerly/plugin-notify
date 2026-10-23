# Notify

Tells you in **Slack**, **Discord** or **[ntfy](https://ntfy.sh)** when a
[Viewerly](https://viewerly.io) session needs you:

- **a permission request**: which session and project, the tool, and a one-line summary of what it
  is for ("viewerly 2 in viewerly wants to use Bash: cargo test");
- **waiting for your input**: posted once each time a session starts waiting;
- **a session ending**, only if you allow Notify to see sessions ending (below).

Messages only tell. They carry no buttons or links that act on a session, and Notify reads
nothing back: answering a request or typing into a session from another device is what Viewerly's
own Sync and Remote control are for, and the [directory's policy](https://viewerly.io/plugins/policy)
keeps plugins to telling. Notify is an automation with a panel, so it needs Viewerly Premium.

## Settings

Settings › Plugins › Notify. Fill in any of these; each filled-in destination gets every message.

| Field | What to put there |
| --- | --- |
| Slack webhook URL | An [incoming webhook](https://api.slack.com/messaging/webhooks), `https://hooks.slack.com/services/…` |
| Discord webhook URL | A channel's webhook (Channel settings › Integrations › Webhooks), `https://discord.com/api/webhooks/…` |
| ntfy topic | A long, hard-to-guess topic name; subscribe to it in the ntfy app. Anyone who knows it can read it. |
| ntfy server | Leave empty for `https://ntfy.sh` |

The three destinations are secret fields: they show as dots and are never sent to your other
devices. **Send Test** (in the settings pane, the panel, or the command **Notify: Send Test**)
posts a test message; the panel shows the last result for each destination. When a post fails,
Notify shows a local notification saying why.

Permission messages include the tool's one-line summary, which can be a shell command or a file
path. If that should not go to a chat service, use ntfy with a private topic, or don't grant
"See permission requests".

### A self-hosted ntfy server

A plugin can only reach the hosts its manifest names, and this one names `ntfy.sh`. To use your
own server, add its host name to `network` in `viewerly-plugin.toml`, rebuild, and install your
build; the grants sheet will then ask for that host too.

## What it asks for

The grants sheet lists each of these; allow them one by one.

| Capability | Grants sheet | Why |
| --- | --- | --- |
| `events = ["permission.requested"]` | See permission requests, with the command or file they are about | To post that a session needs your permission, with the tool and its summary. |
| `events = ["session.status"]` | See session status changes | To post when a session starts waiting for your input. |
| `events = ["session.exited"]` | See sessions ending | Optional: allow it to be told when sessions end too; leave it off otherwise. |
| `network = ["hooks.slack.com"]` | Reach hooks.slack.com over HTTPS | Slack incoming webhooks. |
| `network = ["discord.com"]` | Reach discord.com over HTTPS | Discord webhooks. |
| `network = ["ntfy.sh"]` | Reach ntfy.sh over HTTPS | ntfy. |
| `notify = true` | Show notifications | A local notification when a post fails. |

Grant only the hosts you use. Notify does not ask to answer permissions, and it cannot: it has no
decide grant.

## Limits

- Viewerly gives each event ten seconds. Notify posts to Slack, then Discord, then ntfy, and skips
  a destination if the ones before it took more than a second and a half, saying so in the panel,
  so one slow service cannot get the plugin stopped.
- Every permission request is posted, including ones you answer at your desk a moment later: a
  plugin cannot tell whether someone is looking.
- Times in the panel are UTC; plugins have no time zone.

## Installing

Download `io.viewerly.notify-<version>.vplugin` from the
[releases](https://github.com/viewerly/plugin-notify/releases), then:

- **Settings › Plugins › Install from File…**, or
- `viewerly plugin install io.viewerly.notify-1.0.0.vplugin`.

Once it is listed in the directory, **Settings › Plugins › Browse Plugins…** installs it without a
download. A plugin starts with nothing allowed; the grants sheet opens so you can choose.

## Building

You need Rust and the WASM target:

```sh
rustup target add wasm32-wasip2
./build.sh     # notify.wasm, then dist/io.viewerly.notify-<version>.vplugin
cargo test     # the message text, natively
```

`src/lib.rs` is built against `wit/viewerly.wit`, Viewerly's plugin world
(`viewerly:plugin@0.1.0`), with [wit-bindgen](https://github.com/bytecodealliance/wit-bindgen). A
`v*` tag tests, builds and attaches the `.vplugin` to a GitHub release
(`.github/workflows/release.yml`).

## Licence

MIT, © 2026 Viewerly. See [LICENSE](LICENSE).
