# RDP and VNC in Reach

Research, not a commitment. Written so the decision can be made on evidence
and so the next person does not have to gather it again.

**Verdict: both are possible, VNC is cheap, RDP is not, and the reason to do
either is not the protocol — it is the SSH tunnel Reach already has.**

---

## The actual problem

It is not the protocols. Both are solved, in Rust, under licences that suit
us. It is getting pixels from Rust into a webview fast enough.

An SSH terminal is text. A few kilobytes a second reach xterm.js through
Tauri's IPC and nobody notices. A remote desktop is a framebuffer: 1920×1080
at 32bpp is 8 MB a frame, and even with RFB's dirty rectangles and RDP's
codecs a busy screen moves tens of megabytes a second.

Tauri's IPC is JSON over a message channel. It is not a video pipe. Anything
that routes frames through `emit`/`invoke` will work in a demo on a static
desktop and fall apart the moment someone drags a window.

So the architecture question is only ever: **how do the pixels get to the
canvas, and where does the protocol get decoded?**

---

## Libraries

Checked against crates.io on 2026-09-24.

| Crate | Version | Licence | Downloads | Last release |
| --- | --- | --- | --- | --- |
| `ironrdp` | 0.17.0 | MIT OR Apache-2.0 | 680,831 | 2026-07-10 |
| `vnc-rs` | 0.6.0 | MIT OR Apache-2.0 | 2,017,415 | 2026-09-21 |
| `rfb` | 0.1.0 | MPL-2.0 | 2,620 | 2022-05-02 |

**IronRDP** is Devolutions' RDP stack — the one behind their commercial
gateway, so it is maintained by people who are paid to care. Modular: the
protocol, the client engine, and a WASM binding for browsers are separate
crates. MIT OR Apache-2.0 sits fine alongside our MIT.

**vnc-rs** is an async RFB client that explicitly targets both native and
WASM. Two million downloads and a release three days ago.

**rfb** is MPL-2.0, four years stale, and a server rather than a client.
Not a candidate.

Also relevant, and not a crate: **noVNC** (MPL-2.0) is a complete VNC client
in JavaScript that speaks RFB over WebSocket and draws to a canvas. MPL-2.0
is file-level copyleft — shipping it inside an MIT application is fine, we
would just have to publish changes to noVNC's own files if we made any.

**Precedent:** electerm, an Electron terminal, added both. There is a public
`electerm/ironrdp-wasm` repository that does nothing but compile IronRDP's web
bindings to WASM. So this path has been walked by an application shaped like
ours.

---

## Four architectures

### 1. Frames over Tauri IPC — no

Decode in Rust, send each update to the webview as an event. Simple, and it
dies under load for the reasons above. Rejected before prototyping; the
arithmetic is not close.

### 2. Native child window — no

Render to a real OS surface layered over the webview. Fastest possible, and
it fights everything Reach is: it cannot live inside a tab, it will not
survive a split pane, it needs writing three times for three platforms, and it
does not exist at all on Android.

### 3. Decode in Rust, stream to canvas over a local WebSocket

`vnc-rs` or `ironrdp` run in the Rust process. A loopback WebSocket server
hands the webview binary framebuffer updates, which are drawn to a canvas.

Works, keeps the protocol in Rust where the rest of Reach's networking lives,
and the bandwidth stays inside the machine. Costs us a rendering layer written
from scratch, including the encodings — RFB alone has Raw, CopyRect, RRE,
Hextile, Tight, ZRLE.

### 4. Bridge TCP to WebSocket, decode in the webview — **recommended**

Rust does one small job: accept a WebSocket from the webview and pipe it to a
TCP socket. About a hundred lines with `tokio-tungstenite`. This is what
`websockify` is, and it is the only new Rust we would strictly need.

Then:

- **VNC** — noVNC speaks RFB over WebSocket natively and already implements
  every encoding, cursor handling, clipboard and resize. It is the reference
  client. We would be wiring it up, not writing it.
- **RDP** — IronRDP's WASM binding runs the protocol in the webview over the
  same bridge.

**One bridge serves both protocols.** That is the whole argument for this
shape.

The cost is that the heavy work happens in the webview rather than in Rust,
which is slower than native and matters most on Android. And it is one more
loopback listener to bind carefully.

---

## The part that is actually interesting

Nobody exposes 3389 or 5900 to the internet. Every real RDP or VNC session
goes through a tunnel, a VPN, or a jump host — and that is the awkward part of
every remote desktop client: you set up the tunnel somewhere else, then point
the client at `localhost:someport` and remember which port meant which
machine.

**Reach already does SSH tunnels, jump hosts and an encrypted vault.**

So the sequence we could offer, from one saved session:

1. Connect SSH to the host, through its jump chain if it has one
2. Open a direct-tcpip channel to `127.0.0.1:5900` on the far side
3. Bridge that channel to the webview's WebSocket
4. Draw

The user picks a saved server and gets a desktop. No tunnel to configure, no
port to remember, no second tool. Credentials come from the vault they already
use.

That is a feature mRemoteNG and Remmina have in pieces and that Windows RDP
does not have at all. It is also the only version of this worth building —
a plain RDP client with no tunnel story is a worse FreeRDP.

Note this changes nothing architecturally: step 2 replaces a TCP connect with
an SSH channel, and both are just byte streams into the same bridge.

---

## Effort, honestly

**VNC: small.** The bridge, noVNC in a panel, session fields for port and
password, vault wiring. The protocol work is zero because noVNC is the
reference implementation. Realistically a couple of weeks to something usable,
including the SSH-tunnel path.

**RDP: considerably more.** IronRDP handles the protocol, but the work around
it is where the time goes: NLA and CredSSP authentication, certificate
prompts (the same trust-on-first-use problem we already solved for host keys,
solved again for a different trust store), clipboard, resolution changes,
multi-monitor, and a credential flow that is genuinely more complicated than
SSH's. Months, not weeks, to something that does not embarrass us.

**Android:** VNC over WASM in a webview on a phone is plausible and will not
be pleasant. RDP is worse. Both should be desktop-first with mobile treated as
a later question, not a launch requirement.

---

## The case against

Worth stating, because the case for is easy to make and this is the part that
usually goes unwritten.

Reach is an SSH client. The thing it does well — terminals, files, tunnels,
secrets, automation — is coherent, and every feature so far has been in
service of that. Remote desktop is a different product wearing the same shell.
It doubles the surface area of what "a session" means, adds a rendering
pipeline nobody currently maintains, and invites comparison with FreeRDP and
TigerVNC, which have had twenty years to get good.

There is also a sequencing argument. The site went live yesterday with nothing
indexed, the Discord has twelve people, and four shipped features had no
documentation until this week. A new protocol is not the highest-value thing
available.

---

## Recommendation

1. **Not yet.** Nothing here expires. The crates are healthy and will be
   healthier in six months.
2. **When it is time, VNC first** — via architecture 4, with the SSH tunnel
   path as the headline rather than an afterthought. It is weeks not months,
   and it proves the whole pipeline: bridge, canvas, input, vault,
   tunnel.
3. **RDP only if VNC is used.** If the VNC panel sits idle, the answer for RDP
   is no, and we will have learned that for two weeks of work rather than two
   months.
4. **Keep the bridge protocol-agnostic** from the first commit, even while
   only VNC uses it. It is the one piece both need and the one piece that is
   expensive to retrofit.

---

*Researched 2026-09-24. Crate versions and licences verified against
crates.io on that date; re-check before acting on this.*
