<!-- Written by the Rikka ADE from the notes beside it. Anything typed here is overwritten. -->

# Project memory

- [Rikka project map and layering](project-overview.md) — Top-level directories, the apps/system → public crates rule, layer conventions, and project-wide gaps.
- [Rust workspace conventions](workspace-conventions.md) — Flat facades, semantic errors, deferred builders, no comments or stubs, and naming of threads and runtimes.
- [Test layout and shared test helpers](testing.md) — Where tests live, the rikka-test-* helper crates, machine tests, time limits and deliberately untested seams.
- [Writing built-in Slang shaders that match their GLSL originals](slang-shader-ports.md) — Slang lowering pitfalls (sign, mod, constant folding, push-constant loads, coherent pointers, names) for exact ports.

## MODULE.bazel

- [Bazel build, Nix toolchain and remote execution](bazel-build.md) — MODULE.bazel, @nix toolchains, BuildBuddy RBE image, rule macros, generated code, profiles and test launcher.

## apps/general/animate

- [Animate Physics tab, brush painting and cloth settings state](animate-cloth.md) — Per-figure ClothSettings/ClothAnalysis in rikka-animate: Physics tab, tuning sliders, brush painting, GPU cloth hooks.
- [Animate application](animate-app.md) — rikka-animate plays and edits physics-driven PMX/VRM/FBX/glTF figures, any motion retargeted, camera and music.
- [Animate liquid emitters and their simulation](animate-liquids.md) — How rikka-animate keys and drags liquid emitters and feeds them to the shared rikka-physics bake the viewport shows.
- [Animate morphs (expressions) model and editing](animate-morphs.md) — Faces/Panel catalog, per-morph Morphs tracks on every Take, MorphPlayback with auto-blink, pending morphs, the panel.
- [Animate motion and camera editing](animate-editing.md) — How rikka-animate registers keys, edits the dope sheet, bends curves and saves .rmotion, in Easy and Detail modes.
- [Animate's tools for the desktop assistant](animate-assistant.md) — animate-assistant declares Animate's prompts and tools; each window offers them to the shell sidebar as kit "Animate".

## apps/general/animate/crates

- [Animate Easy mode and humanoid motion crates](animate-easy-mode.md) — animate-body/solve/motion turn PMX figures into muscle-space humanoid motion that Easy mode poses by direct drag.
- [Animate crate map, panel traits and catalogs](animate-crates.md) — rikka-animate's app-private crates: layers, what each owns, Studio/Workshop panel traits, Desk/Reach, catalogs.

## apps/general/animate/crates/animate-fit

- [Animate body fitting and motion style (animate-fit, intensity, jiggle)](animate-fit.md) — animate-fit removes limb-body intrusions from baked motion; animate-motion adds intensity shaping and bust jiggle.

## apps/general/animate/crates/animate-fluid

- [Animate liquid emitters (animate-fluid)](animate-fluid.md) — animate-fluid's GPU-free emitter model: Pour span, keyed settings, blob landing, ballistic aim, liquids, file kind.

## apps/general/animate/crates/animate-hold

- [Animate holds (hands and pins gripping cloth)](animate-holds.md) — animate-hold's keyed Hold spans, the Easy palm-ring grasp, hold timeline rows, finger pinch and the cloth Grab path.

## apps/general/animate/crates/animate-look

- [Animate look model (node graphs, world nodes, slots, keys, plans)](animate-look.md) — animate-look's node-based look: World/Composite/Material graphs, world record nodes, typed World, slots, keys, plans.

## apps/general/animate/crates/animate-shader

- [Animate look editing and node editor (animate-shader, animate-shader-editor)](animate-shader.md) — animate-shader's Workbench/Shaders/SharedCompiler over the app's Look; node editor dock, slots panel, Node tab.

## apps/general/animate/crates/animate-solve

- [Animate Easy posing (markers, drags, twist, fingers)](animate-easy-posing.md) — Easy markers: animate-solve Handle/DragSession/twist/rotate/Finger and the animate-viewport overlay.

## apps/general/animate/crates/animate-stage

- [Animate stage, transport, viewport pacing and export](animate-stage.md) — animate-stage's Stage/Render halves, the clock and Transport, live viewport pacing and TAA, output sheets and exports.

## apps/general/animate/crates/animate-wind

- [Animate wind subject (animate-wind)](animate-wind.md) — animate-wind's keyed WindMotion (yaw, pitch, speed, turbulence, gust), .rmotion kind wind, the Wind subject and vane.

## apps/general/cut

- [Cut media relinking and attribute pasting](cut-relink-attributes.md) — Relink/Mismatch/Relocation/Catalog in cut-project, the Link Media sheet, Replace Media, Paste Attributes/Effects.
- [Cut sound levels, markers, trim modes and project settings](cut-editing.md) — Keyable volume/pan and fades, timeline/clip markers, ripple/roll/slip/slide trims, Edit::Reformat and their UI.
- [Cut timeline thumbnails and waveforms](cut-glance.md) — cut-filmstrip (tile plan, thumbnail workers), cut-waveform (peak pyramid, disk cache) and the Lanes painting over them.
- [Cut transitions (edit-point transitions, gallery, rendering)](cut-transitions.md) — Cut's per-layer edit-point transitions: cut-project transitions, cut-render transit, gallery, timeline bars, audio.
- [Cut video editor application](cut-app.md) — rikka-cut is a layer-based video editor: cut-project model, cut-render preview, effect/keyframe panel, timeline.

## apps/general/cut/crates/cut-export

- [Cut export (cut-export and the export sheets)](cut-export.md) — cut-export's Settings/presets, Span/Plan time mapping, offline Mixdown, Job threads; rikka-cut's export sheet and prefs.

## apps/general/cut/crates/cut-project

- [Cut keyframes, presets and 2.5D projection (cut-project)](cut-animation.md) — cut-project's animation, keying, presets and projection modules: keyframed Track<K>, Property edits, cameras, .rcut.
- [Cut project model (cut-project)](cut-project.md) — cut-project's Project, layer heights, shallow Arc copies, clip motion and effects, place/paste/move edit rules.

## apps/general/cut/crates/cut-render

- [Cut preview compositor (cut-render)](cut-render.md) — cut-render composites a cut-project frame into one imaging Picture, supplies footage frames and runs the render thread.

## apps/general/cut/src/panel

- [Cut property panel, easing picker and animation presets](cut-panel-motion.md) — rikka-cut panel rows and diamonds, the easing picker (keys or preset mode), animation presets and camera groups.

## apps/general/cut/src/stage

- [Cut preview stage (projected manipulation, motion path, gizmo, camera)](cut-stage.md) — apps/general/cut/src/stage: items seen through shot projections, motion path keys, anchor, 3D rings, camera overview ge…

## apps/general/cut/src/timeline/keys

- [Cut timeline key lanes, graph editor and preset wedges](cut-keyframing-ui.md) — Cut timeline key lanes and value/speed graph (timeline/keys), animation-preset wedges, keyframe selection.

## apps/general/paint

- [Paint application](paint-app.md) — rikka-paint is the beginner-friendly image editor: tabbed documents, PSD-native model, tool rail, layers, GPU canvas.
- [Paint editable objects (figures)](paint-objects.md) — Session-only shape/text objects above a layer's pixels: Figure model, overlays, edits, GUI picking, the assistant.
- [Paint's tools for the desktop assistant](paint-assistant.md) — paint-assistant declares Paint's 13 model tools from code tables; paint/serving runs each change as one undo step.

## apps/general/paint/crates/paint-document

- [Paint document model (paint-document)](paint-document.md) — paint-document opens, edits, records history, previews and saves PSD-native documents for rikka-paint, UI-free.
- [Paint selections and paint bucket](paint-selection.md) — Coverage-span Region, Shape/Flood/Modify edits in paint-document, and the GUI selection tools that only order them.

## apps/general/paint/src/session

- [Paint dialogs (sessions) and the document worker protocol](paint-sessions.md) — How rikka-paint runs parameter dialogs in the side panel, previews, applies, saves and talks to its worker.

## apps/general/remote

- [Remote desktop viewer (rikka-remote-viewer)](remote-viewer-app.md) — apps/general/remote: native QUIC viewer: hosts list, sign-in, GPU decode into a Feed, key passthrough, escape chords.

## apps/system/ade

- [ADE project architecture](ade-architecture.md) — ADE's private crates, agent assembly, Git and worktrees, file editing, watching, and conversation view structure.

## apps/system/ade/crates

- [ADE guides, skills and memory](ade-guides-skills-memory.md) — Standing RIKKA.md instructions, lazy project/person skills, and project-only memory notes with a derived index.

## apps/system/ade/src/project

- [ADE conversations](ade-conversations.md) — ADE's use of desktop accounts, per-thread model selection, storage folders, jobs, notices, compaction and attachments.

## apps/system/browser

- [Browser application](browser-app.md) — rikka-browser is the vertical-tab browser on rikka-web (Gecko): shared model, runtimes per profile, store persistence.

## apps/system/browser/crates

- [Browser private crates](browser-crates.md) — browser-store (SQLite), model (shared state), pages (page rules), reader, engine settings, translate kit + translators.

## apps/system/browser/crates/browser-translate/scripts

- [Browser page-translation script](browser-translate-script.md) — browser-translate's probe.js (every frame) and translator.js (on demand): units, modes, code comments.

## apps/system/calculator

- [Calculator application](calculator-app.md) — A keypad window over calc-engine, a dependency-free engine that owns every key-state rule.

## apps/system/discord

- [Discord application GUI (rikka-discord)](discord-app.md) — apps/system/discord/src: Chat root state, per-account LinkedAccount stores with Area→Revision marks, voice, overlays.

## apps/system/discord/crates/discord-client

- [Discord client (discord-client)](discord-client.md) — discord-client's Discord/Link/Client/Call API over the sans-IO discord-wire/translate/state/rate-limit crates.

## apps/system/discord/crates/discord-core

- [Discord model and store (discord-core)](discord-core.md) — discord-core is rikka-discord's model: snowflake ids, Update stream, Store, Accounts book, voice events.

## apps/system/discord/crates/discord-krisp

- [Krisp from Discord's own module (discord-krisp)](discord-krisp.md) — discord-krisp downloads Discord's official discord_krisp module and drives the raw Krisp C SDK it exports.

## apps/system/discord/crates/discord-rtc

- [Discord voice transport (discord-rtc)](discord-rtc.md) — Voice gateway v8, UDP rtpsize AEAD, DAVE E2EE, and Go Live video (RTP, RTX, RTCP, pacing) behind Transport.

## apps/system/discord/crates/discord-stream

- [Discord Go Live media engine (discord-stream)](discord-stream.md) — discord-stream turns a Cast into a paced H.264 broadcast and a watched stream into Feed pictures plus mixed sound.

## apps/system/discord/crates/discord-voice

- [Discord call audio (discord-voice)](discord-voice.md) — Call audio over discord_client::Call: 10 ms send chain (APM, Krisp, gate), mono Opus, playout receive, mixing.

## apps/system/discord/src/conversation

- [Discord conversation region (rikka-discord)](discord-conversation.md) — apps/system/discord/src/conversation: list, Rich rendering, composer, pickers, overlays, forum, and its seams with the …

## apps/system/files

- [Files application](files-app.md) — Manager, portal dialog and opener binaries over private VFS, MIME, thumbnail, archive, drive and portal crates.

## apps/system/music

- [Music application](music-app.md) — rikka-music's core model, accounts and keyring, shared Hifi Deck, gapless, UI state, pages, settings, signal path.

## apps/system/music/crates/music-blend

- [Music cross-service playlists (Rikka playlists)](music-blends.md) — music-blend's Blend model, JSPF store, song matching, mirror sync and discovery, and how rikka-music drives them.

## apps/system/music/crates/music-library

- [Music local library (rikka-audio-tags, music-library, music-local)](music-local.md) — How rikka-music indexes local files (tags, SQLite index, watching) and serves them as the always-linked local service.

## apps/system/music/crates/music-spotify

- [Spotify client (music-spotify and its protocol crates)](music-spotify.md) — rikka-music's in-house Spotify client: sans-IO access point protocol, AP session, spclient API, decryption.

## apps/system/music/crates/music-stream

- [Music services, adapter kit and streaming](music-services.md) — music-stream adapter kit (Fetcher, Plan, Spool, Streamer, Origin), Providers, audio cache, YouTube, Qobuz, SoundCloud.

## apps/system/packages

- [Packages application](packages-app.md) — Browses and searches a local nixpkgs catalog and stages declarative package and module changes into one rebuild.

## apps/system/proton

- [Proton application](proton-app.md) — apps/system/proton is one window over a Proton account: Mail, Drive, Pass, Codes and Account over private crates and ca…

## apps/system/settings

- [Settings application](settings-app.md) — Adopts desktop schemas, drives daemons directly, owns rikka.llm, and stages all NixOS edits in one draft.

## apps/system/settings/src/users

- [Settings Users page (avatar, name, password)](settings-users.md) — Users page: AccountsService avatar with circular crop, display name staged as lib.mkForce, passwd password change.

## apps/system/task-manager

- [Task Manager application](task-manager-app.md) — rikka-sysmon probes plus the private tasks-process crate (rows, grouping, kill) under one virtualized window.

## apps/system/terminal

- [Terminal application](terminal-app.md) — The Terminal window owns split panes of tabs and window chords over the shared rikka-terminal grid.

## apps/system/updates

- [Updates application (rikka-updates)](updates-app.md) — Single-instance viewer of rikka-nixctl runs: run list, header and progress, a paged then live log.

## apps/system/viewer

- [Viewer application](viewer-app.md) — Opens pictures, audio, video, 3D models and text with bounded probing, playback, figures and text editing.

## bazel/windows

- [Windows executables and the portable Windows ZIP](windows-distribution.md) — rikka_application embeds manifest and version; windows_zip packs the five general apps and checks DLL imports.

## benchmarks

- [Comparative benchmarks (benchmarks/)](benchmarks.md) — Suites comparing Rikka's physics, layout, UI, text and IPC with industry engines; UI tiers, fairness rules, running.

## crates

- [Internal crate ownership map](internal-crates-map.md) — Every crate folder under crates/<layer>/ with a one-line role or the note that documents it.
- [Public crates used by apps](public-crates.md) — //visibility:public libraries by layer, the narrower exceptions, rikka_app::prelude, errors and shared helpers.

## crates/ai/rikka-agent

- [Agent execution loop](agent-runtime.md) — rikka-agent streams model-tool turns with budgets, notices, cancellation and compaction; no approval gate.

## crates/ai/rikka-assist

- [App tools for the desktop assistant (rikka.assist)](app-tools-protocol.md) — rikka-assist lets any app offer its own agent tools over seam; the shell's sidebar finds, loads and calls them.

## crates/ai/rikka-chat

- [Shared conversation UI (rikka-chat)](chat-ui.md) — rikka-chat holds ADE's and the shell assistant's composer, offer list, transcript, call views and model picker.

## crates/ai/rikka-llm

- [Language-model client](llm-client.md) — rikka-llm is the vendor-free core: message types, four wire protocols, transport and the wire kit for providers.

## crates/ai/rikka-llm-accounts

- [Desktop LLM accounts](desktop-llm-accounts.md) — rikka-llm-accounts owns the rikka.llm domain, browser sign-in, model selection, the model roster and the picker.

## crates/ai/rikka-llm-providers

- [LLM providers: adapter kit, adapter crates and registry](llm-providers.md) — rikka-llm-adapter is the provider kit, crates/ai/providers holds one adapter crate each, rikka-llm-providers lists them.

## crates/ai/rikka-llm-settings

- [Shared AI settings page (rikka-llm-settings)](llm-settings-page.md) — rikka-llm-settings is the one AI page (accounts, sign-in, endpoints, models, reply language) Settings and ADE show.

## crates/ai/rikka-llm/src/messages/hosted.rs

- [Provider-hosted web tools (server tools)](server-tools.md) — rikka-llm's ServerTool/Hosted blocks, Anthropic web_search/web_fetch declaration, replay, pause_turn and display.

## crates/ai/rikka-mcp

- [Model Context Protocol client](mcp-client.md) — rikka-mcp's Hub layers .mcp.json places into live server kits; the McpServers tool, sign-in, standing instructions.

## crates/ai/rikka-thread

- [Shared conversations and assistants](agent-conversations.md) — rikka-thread stores and runs conversations; callers supply instructions, folding, subagents and the folder.

## crates/ai/rikka-tools

- [Shared agent tool runtime](agent-tools.md) — rikka-tools declares Bash, jobs, Edit/Write, questions, deferred search and Computer from prompts; web() asks for hoste…

## crates/compositor/rikka-compositor-desktop

- [Compositor desktop entity model (rikka-compositor-desktop)](compositor-desktop.md) — Desktop<P> arenas for windows, layers, popups, outputs, workspaces, locks; draw order, layer arrangement, hit tests.

## crates/compositor/rikka-compositor-kms

- [rikka-compositor-kms display backend](compositor-kms.md) — Native libseat/udev/DRM/GBM crate with a per-GPU rikka-kms-N commit thread, vblank timeline and frame pacer.

## crates/compositor/rikka-compositor-output

- [Compositor output, capture, blur and DRM lease protocols](compositor-output.md) — rikka-compositor-output's policy-free wl_output/xdg_output, ext image capture, background effect and DRM leases.

## crates/compositor/rikka-compositor-planes

- [Direct scan-out (rikka-compositor-planes)](plane-assignment.md) — rikka-compositor-planes' pure assign() and presentation() pick scan-out, tearing flips, VRR and pacing per frame.

## crates/compositor/rikka-compositor-scrolling

- [Scrolling column layout (rikka-compositor-scrolling)](compositor-scrolling.md) — Strip<W> columns on an endless horizontal strip, view fitting, stacking, swipes; how the compositor drives it.

## crates/compositor/rikka-compositor-seat

- [Compositor seat crate (keyboard, pointer, touch, tablet, IME, idle)](compositor-seat.md) — rikka-compositor-seat owns wl_seat protocols, focus, grabs, constraints, text-input/IM bridge, virtual input, idle.

## crates/compositor/rikka-compositor-selection

- [Compositor selection crate (clipboard, primary, data control, drag and drop)](compositor-selection.md) — rikka-compositor-selection owns per-seat selections, compositor sources, ext-data-control, DnD and toplevel drags.

## crates/compositor/rikka-compositor-server

- [Compositor Wayland server core (scanner, wire, protocols, server)](compositor-wayland-core.md) — Own Wayland server stack in crates/compositor: XML scanner library, wire codec, generated protocols, Display.

## crates/compositor/rikka-compositor-shell

- [Compositor shell roles (rikka-compositor-shell)](compositor-shell.md) — Policy-free xdg/layer/lock/foreign/Xwayland shell state machines as Modules; host decides through ShellHost events.

## crates/compositor/rikka-compositor-surface

- [Compositor surface machinery (rikka-compositor-surface)](compositor-surface.md) — wl_surface state machine, buffers, transactions, roles, dmabuf/syncobj/fifo/timing/color as built-in modules.

## crates/compositor/rikka-compositor-xwm

- [Compositor X11 window manager (rikka-compositor-xwm)](compositor-xwm.md) — Sans-IO Xwayland WM on x11rb-protocol: continuation replies, per-turn flush, batched property reads, selections.

## crates/daemons

- [Daemon integration layer](daemon-integration.md) — One public crate per system daemon over the rikka-bus runtime (Handler, Service, Monitor, Signals); no aggregate.
- [MPRIS, privacy, power profiles and location crates](mpris-privacy-profiles.md) — rikka-mpris (Players client, Publisher server), rikka-privacy (mic/camera/screen users), power-profiles, location.
- [Notification, tray and polkit services](hosted-daemons.md) — rikka-notifications, rikka-tray and rikka-polkit serve session singletons and leave all drawing to the shell.
- [Sound, network, Bluetooth, power and battery crates](daemon-crates.md) — What each read daemon crate publishes and the rules its operations follow; none owns UI.

## crates/daemons/rikka-ime

- [Fcitx5 input-method integration](input-method.md) — rikka-ime runs and configures fcitx5, hosts kimpanel data, and shares caret and keyboard-source types.

## crates/daemons/rikka-keyring

- [Keyring (Secret Service) and the secrets client](keyring.md) — rikka-keyring serves org.freedesktop.secrets over an encrypted vault; rikka-secrets is the one client every app uses.

## crates/desktop

- [Desktop layer: opening, dialogs, clipboard, entries](xdg-desktop-integration.md) — rikka-open, dialog, clipboard, instance, mime, xdg, trash, mounts, thumbnails, xcursor, icons: freedesktop for apps.
- [Windows backends of the foundation, desktop and session crates](windows-desktop-integration.md) — What dirs, watch, i18n, log, open, dialog, instance, secrets, mime, xdg, seam and assist do on Windows.

## crates/desktop/rikka-fido

- [FIDO2 security key client (rikka-fido)](fido-client.md) — rikka-fido is the workspace's CTAP2/U2F client over hidraw with WebAuthn client data, PIN, extensions and ceremonies.

## crates/desktop/rikka-file-operations

- [Local file operations (rikka-file-operations)](file-operations.md) — rikka-file-operations: copy/move/trash/delete engine with progress, conflicts, links and staged writes over a Store.

## crates/desktop/rikka-screencast

- [Screen capture (rikka-screencast)](screencast-capture.md) — rikka-screencast is the ScreenCast portal client and PipeWire frame consumer apps use directly; sound is rikka-audio's.

## crates/foundation

- [Foundation file and path crates](foundation-files.md) — rikka-dirs, rikka-shipped, rikka-fs, rikka-watch, rikka-deflate, rikka-encoding: paths, atomic writes, watching.

## crates/foundation/rikka-archive

- [Archive processing](archive-processing.md) — rikka-archive reads and writes common formats with streaming batches, AES, cancellation and traversal defence.

## crates/foundation/rikka-certificate-transparency

- [Certificate Transparency log list and SCTs](certificate-transparency.md) — rikka-certificate-transparency fetches and verifies Chrome's signed CT log list, caches it, and checks embedded SCTs.

## crates/foundation/rikka-color-formats

- [Colour formats (rikka-color-formats)](color-formats.md) — Configurable colour format templates, colour spaces, names, shades and export, shared by shell and Settings.

## crates/foundation/rikka-file-edit

- [Line-number edits and their mistake gates (rikka-file-edit)](file-edit-gates.md) — Files::edit pipeline over Op line ranges, the echoed-line and broken-syntax refusals, and what each gate compares.

## crates/foundation/rikka-file-format

- [Rikka's own file formats (rikka-file-format) and the v1-only rule](file-formats.md) — rikka-file-format's JSON envelope, binary header, Codec/Reader/Writer; formats using it; no versions or migrations.

## crates/foundation/rikka-fuzzy

- [Fuzzy matching (rikka-fuzzy)](fuzzy-matching.md) — rikka-fuzzy is the one fuzzy scorer: Needle, Haystack, Score, positions, best/ranked; launcher, completion, ADE.

## crates/foundation/rikka-i18n

- [Internationalization](internationalization.md) — rikka-i18n-catalog parses catalogs and messages; rikka-i18n loads, negotiates and installs; catalogs!() checks catalogs.

## crates/foundation/rikka-net

- [Shared network client](network-client.md) — rikka-net owns async and blocking HTTP, transfers, SSE, WebSockets, retries, TLS and cancellation.

## crates/foundation/rikka-oauth

- [Shared OAuth client library (crates/foundation/rikka-oauth)](oauth-library.md) — rikka-oauth owns loopback redirects, PKCE, code/refresh/device grants, tokens, claims, discovery and renewal.

## crates/foundation/rikka-pipewire

- [Shared PipeWire helpers (rikka-pipewire)](pipewire-helpers.md) — rikka-pipewire owns the PipeWire connection, loop worker, sync round trip, F32 format pod and SPA pod helpers.

## crates/foundation/rikka-simd

- [SIMD library (rikka-simd)](simd-library.md) — rikka-simd's levels, dispatch!, type-level proof, vector families, math and how consumers write kernels.

## crates/foundation/rikka-types

- [Shared types and input translation](core-types-and-input.md) — rikka-types owns dependency-free vocabulary; rikka-input translates backend events and matches exact chords.

## crates/foundation/rikka-wayland

- [Wayland client layer](wayland-client-layer.md) — rikka-wayland over the in-house rikka-wayland-client: Display, Host, EventLoop, serials, presentation, scale.

## crates/foundation/rikka-wayland-client

- [In-house Wayland client (rikka-wayland-client)](wayland-client-codec.md) — rikka-wayland-wire's client half and the generated proxies: connection, routes, queues, keys, events, thread model.

## crates/foundation/rikka-worker

- [Foundation runtime crates (worker, sync, heap, log, format)](foundation-runtime.md) — rikka-worker threads, channels, eventfds, Observable; rikka-sync, rikka-parallel, rikka-heap, log, format, interfaces.

## crates/foundation/rikka-xml

- [XML reader (rikka-xml)](xml.md) — rikka-xml parses XML into a borrowed element tree, strict with positioned errors or lenient with recovery.

## crates/graphics

- [CPU picture crates (png, resample, material colour, barcode)](graphics-cpu-crates.md) — rikka-png, rikka-image-resample, rikka-material-color and rikka-barcode: CPU codecs, resampling, colour science, QR.

## crates/graphics/rikka-colorimetry

- [Colour management and HDR output](color-management.md) — rikka-colorimetry maths, renderer blend spaces and ColorIds, wp_color_manager_v1, KMS PQ signalling, settings.

## crates/graphics/rikka-effects

- [Shared adjustments and effects (rikka-effects)](effects-shared.md) — rikka-effects is the UI-free serde effect model; rikka-effects-forms holds the forms and editors Paint and Cut share.

## crates/graphics/rikka-figure

- [Shared editable figures (rikka-figure)](figure-shared.md) — rikka-figure is the UI-free figure model, vector text and imaging recipe; rikka-figure-editing edits it on canvas.

## crates/graphics/rikka-gpu

- [GPU compute (rikka-gpu Compute)](gpu-compute.md) — Compute lanes, batches, foreign images, compute images and recycling, computed pictures, completion callbacks.
- [GPU core (rikka-gpu)](gpu-core.md) — rikka-gpu: layering, facade, the Gpu device handle, reclaim, shaders and SPIR-V loading, errors, code map.
- [GPU memory, provisioning and frame slots (rikka-gpu)](gpu-memory.md) — gpu-allocator blocks, dedicated allocations, provision thread, transient aging, staging belts, trim, FrameRing.
- [GPU queues, submits and pipeline compilation (rikka-gpu)](gpu-queues.md) — Queue families on declared need, transfer lane, global priority, submits, background pipelines, pass timing.
- [GPU textures, uploads and the bindless table (rikka-gpu)](gpu-textures.md) — Uploads, DRM-layout rows, planar dmabufs, the bindless table, image access, mip growth, texture formats.

## crates/graphics/rikka-gpu-drivers

- [Graphics driver selection and Rikka's own Mesa (rikka-gpu-drivers)](gpu-drivers.md) — rikka-gpu-drivers picks drivers per render node: pinned Mesa for Rikka processes, narrowed system drivers for others.

## crates/graphics/rikka-image

- [Picture decoding and encoding (rikka-image, rikka-image-ffmpeg)](image-codecs.md) — rikka_image::read/write doors, own codecs, FFmpeg-backed AVIF/HEIF/JPEG XL/TIFF/EXR/J2K, limits, ICC, the loader.

## crates/graphics/rikka-image-edit

- [GPU picture editing (rikka-image-edit)](imaging-engine.md) — rikka-image-edit decodes, edits on Vulkan compute and encodes pictures: Picture, Recipe/Step/Mask, operations, marks.

## crates/graphics/rikka-psd

- [PSD/PSB format library (crates/graphics/rikka-psd)](psd-format.md) — rikka-psd: lossless PSD/PSB read/write, typed blocks, descriptors, engine data, pixels, ICC colour, style views.

## crates/graphics/rikka-psd-render

- [PSD compositor (crates/graphics/rikka-psd-render)](psd-render.md) — rikka-psd-render composites a rikka_psd::Document on the CPU: blend modes, masks, groups, knockout, adjustments.

## crates/graphics/rikka-renderer

- [2D renderer architecture (rikka-renderer)](renderer.md) — rikka-renderer: crates, Gpu and Renderer::new, Present, extension slot, lifetimes, shaders, code map, errors.
- [2D renderer batching, uploads and occlusion](renderer-batching.md) — Scene journal, Preparer, retained batch with chunks and arena slots, instance uploads, FrameWork, occlusion.
- [2D renderer destinations, damage and readback](renderer-output.md) — Destinations, buffer-age damage, viewers, composed path, frame batches, readback, export and scan-out fences.
- [2D renderer glass (blur, lens refraction, backdrop caches)](renderer-glass.md) — Blur pyramid, lens refraction, joined glass, backdrop caches, Spread, refraction field and damage growth.
- [2D renderer retained scene and edits](renderer-scene.md) — Scene nodes, rounded corners, order, visibility, clips, groups, layers, texture damage and thread hand-over.
- [2D renderer text drawing (glyph atlas, curves, workers)](renderer-text.md) — TextPaint nodes, glyph atlas and workers, text bands, curve glyphs, zoom tracking, glyph holding, coverage.

## crates/graphics/rikka-renderer-3d

- [3D renderer (rikka-renderer-3d)](renderer-3d.md) — rikka-renderer-3d: Renderer3d over a 2D Renderer, viewports, meshes, materials, draws, shader packages, errors.

## crates/graphics/rikka-renderer-3d/shaders/lib/rikka/pbr

- [Realistic shading (OpenPBR standard, skin, hair, eye, SSS, refraction)](realistic-shading.md) — rikka/standard implements OpenPBR Surface 1.1.1; skin/hair/eye templates, BRDF LUT, screen-space SSS, refraction.

## crates/graphics/rikka-renderer-3d/src

- [Render-resolution sub-rectangles (dynamic resolution)](render-resolution.md) — Render-res images are allocated at the max scale; passes draw a sub-rect and map uvs through GpuView or a slot.

## crates/graphics/rikka-renderer-3d/src/composite

- [Compositor graph (Look::composite) in rikka-renderer-3d](renderer-composite.md) — Composite DAG of native operations and effects after the temporal resolve: sockets, layers, exposure, history, present.

## crates/graphics/rikka-renderer-3d/src/effect/builtin

- [Built-in post effect pack (BuiltinEffect)](effect-pack.md) — rikka-renderer-3d's 48 built-in Slang post effects: table, families, sockets, layout, resolution, methods, costs.

## crates/graphics/rikka-renderer-3d/src/fluid

- [Screen-space fluid and wet surfaces in rikka-renderer-3d](renderer-fluid.md) — Frame::fluid(FluidLayer) splats solver particles into screen-space liquid; Frame::wetness + Draw::wet darken meshes.

## crates/graphics/rikka-renderer-3d/src/graph

- [3D render graph and the Effect API](render-graph.md) — rikka-renderer-3d per-frame render graph (declare, compile cache, barriers, aliasing, recorder) and the Effect API.

## crates/graphics/rikka-renderer-3d/src/light

- [Renderer lights (kinds, area LTC, linking, rigs, photometry)](renderer-lighting.md) — rikka-renderer-3d light: Light kinds, LTC area lights, IES profiles, channels, figure rigs, units, light order.

## crates/graphics/rikka-renderer-3d/src/look

- [Cinematic look pipeline in rikka-renderer-3d](renderer-look.md) — Frame::look(Look) turns on the environment, figure styles, G-buffer, screen-space effects and compositor graph.

## crates/graphics/rikka-renderer-3d/src/look/emitter

- [Analytic particle emitters (Look::emitters)](renderer-emitters.md) — rikka-renderer-3d closed-form VFX emitters: public model, CPU evaluator, GPU evaluate/draw/motion/layers passes.

## crates/graphics/rikka-renderer-3d/src/material

- [Material system (templates, schema, variants, overrides)](material-system.md) — MaterialTemplate/MaterialDesc/Overrides over Slang ISurface+IShadingModel; generated variants, blocks, reload.

## crates/graphics/rikka-renderer-3d/src/overlay

- [Editor overlays, dynamic meshes and viewport shading templates (rikka-renderer-3d)](renderer-overlay.md) — Frame::overlay lines/points/triangles after AA with unjittered occluder depth; in-place mesh updates; workbench/unlit.

## crates/graphics/rikka-renderer-3d/src/ray

- [Ray tracing tier, screen-space GI and the denoiser](ray-tracing.md) — rikka-renderer-3d ray tier: ray queries, BLAS/TLAS, RT shadows/AO/reflections/GI, SSGI, SVGF-style denoiser.

## crates/graphics/rikka-renderer-3d/src/shadow

- [Renderer raster shadows (atlases, figure maps, tint, screen shadows, cache)](renderer-shadows.md) — rikka-renderer-3d shadow: main/figure atlases, priorities, head views, RT override, tap rotation, screen shadows.

## crates/graphics/rikka-renderer-3d/src/sky

- [Physical sky, HDR panoramas, sky lighting and fog in the 3D renderer](renderer-sky.md) — rikka-renderer-3d sky: atmosphere LUTs, HDR panorama lighting, sky SH and prefiltered map, aerial LUT, fog, shafts.

## crates/graphics/rikka-renderer-3d/src/space

- [Renderer screen-space and stage effects](renderer-space-effects.md) — Where GTAO, screen-space shadows, SSR, line art, particles and the floor/water materials live and connect.

## crates/graphics/rikka-renderer-3d/src/temporal

- [Temporal antialiasing, upscaling and dynamic resolution](temporal-antialiasing.md) — rikka-renderer-3d temporal: AA modes, Halton jitter, motion vectors, TAA/upscale resolve, still convergence, order.

## crates/graphics/rikka-shader-graph

- [Node graph engine (rikka-shader-graph, rikka-shader-compile)](shader-graph.md) — rikka-shader-graph: Material/World/Composite graphs, Library of nodes, closures and staging, live sockets, planners.

## crates/graphics/rikka-shader-graph/src/nodes

- [Standard shader-graph node library (NPR, closures, colour, vector)](shader-nodes.md) — Library::standard() families (npr/light/bsdf/color/vector/texture), Slang homes, conventions, sites, adding a node.

## crates/graphics/rikka-shader-presets

- [Shader presets (rikka-shader-presets, rikka-shader-presets-gpu)](shader-presets.md) — Preset graph registry (classic, games, styles), the shared import parameters, prebuilt template artifacts.

## crates/graphics/rikka-shader-presets/src/classic

- [Figure shading presets (mmd, mtoon, anime), classes and character passes](figure-shading.md) — Graph maths of the classic figure presets, class presets, wetness, see-through, head frame and character passes.

## crates/graphics/rikka-slang

- [Runtime Slang compiler (rikka-slang)](rikka-slang.md) — rikka-slang dlopens libslang, compiles Slang to Vulkan SPIR-V with embedded modules, reflects layouts and attributes.

## crates/graphics/rikka-svg

- [SVG renderer (rikka-svg)](svg-renderer.md) — rikka-svg parses SVG with rikka-xml and draws with rikka-vector: features, text layering, media, bounds, resvg oracle.

## crates/graphics/rikka-symbols

- [Shipped icon fonts and generated symbol tables](icon-fonts.md) — Material Symbols / Font Awesome Brands pinned in nix/dist/icon-fonts.nix, shipped, tables generated by Bazel.

## crates/graphics/rikka-text

- [Text shaping and font vocabulary](text-shaping.md) — rikka-text: shape-once/layout-per-width text (Shaped, TextLayout, LayoutRow), carets and hits, ink, GlyphRaster, icons.

## crates/graphics/rikka-theme

- [Theme and glass pipeline](themes-and-glass.md) — rikka-theme resolves layered and generated themes; rikka-blur declares glass regions the compositor renders.

## crates/graphics/rikka-theme-seed

- [Material source colour](wallpaper-material.md) — rikka-theme-seed extracts and caches the generated-theme source colour of a picture or video.

## crates/graphics/rikka-vector

- [CPU vector graphics library (rikka-vector)](vector-library.md) — rikka-vector's Path, flattening, two coverage rasterizers, Stroker, paints and the layered Canvas under text and SVG.

## crates/media

- [Media crates map and shared conventions](media.md) — crates/media layout, the read(source) door convention, rikka-ffmpeg, rikka-media(-format), coded, opus, resample.

## crates/media/rikka-apm

- [Audio processing port (rikka-apm crates)](apm.md) — rikka-apm facade over own audio-processing ports: band split, HPF, AEC3, NS, AGC2 with RNN VAD; oracle-tested.

## crates/media/rikka-audio

- [Sound playback and capture (rikka-audio)](audio-engine.md) — AudioEngine, Track, AudioFeed, AudioCapture sources and fill contract, SoundSender, DeviceWatch and their threads.

## crates/media/rikka-audio-decode

- [Sound decoding (rikka-audio-decode)](audio-decode.md) — rikka_audio_decode::read door, gapless FFmpeg decoding, DSD reading, growing downloads, the pull Decoder and WavWriter.

## crates/media/rikka-hifi

- [Music output path (rikka-hifi)](hifi-output.md) — rikka-hifi's gapless single-source output: shared PipeWire stream, exclusive ALSA hw with ReserveDevice1, signal path.

## crates/media/rikka-playout

- [Packet playout jitter buffer (rikka-playout)](playout.md) — Public adaptive jitter buffer and queue target shared by Discord voice and the remote viewer.

## crates/media/rikka-video

- [Video stills, reading and coding (rikka-video)](video-coding.md) — rikka-video's poster, stills, sample, FrameReader, trim, convert, packet coding, encoders, live decoders, Previewer.

## crates/media/rikka-video-play

- [Video playback (rikka-video-core, rikka-video-play, rikka-movie)](video-playback.md) — The video crate split, VideoPlayer opening and pacing, the reaper, VideoController, Movie with its soundtrack.

## crates/media/rikka-vulkan-video

- [Vulkan Video encoder (rikka-vulkan-video)](vulkan-video-encoder.md) — Vendor-neutral H.264/H.265 Vulkan Video encoder with in-place rate control and reference-frame invalidation.

## crates/model

- [3D model data, import/export and runtime](model-import.md) — Public model crates (core data, codecs, humanoid, GPU, rikka-model): codec kit, limits, conversion, upload, poses.
- [Model library performance rules and threading](model-performance.md) — Threading model, caches and complexity rules for the crates/model mesh libraries.

## crates/model/rikka-blend

- [.blend file reader (rikka-blend)](blend-codec.md) — rikka-blend reads .blend files into a Model, evaluating Blender modifiers through rikka-mesh-modifiers.

## crates/model/rikka-fbx

- [FBX codec](fbx-codec.md) — rikka-fbx reads binary and text FBX from 6100 on, bakes animation, and writes FBX 7400.

## crates/model/rikka-gltf

- [glTF codec](gltf-codec.md) — rikka-gltf reads and writes .gltf/.glb, merges primitives, keeps sparse morphs, and exposes parts for VRM.

## crates/model/rikka-humanoid

- [Humanoid recognition (rikka-humanoid)](humanoid-recognition.md) — Anatomy::recognize(model) maps any model to HumanBone (declared, Unity-style names, structure), parts, swings.

## crates/model/rikka-mesh-bevel

- [Bevel operator (rikka-mesh-bevel)](mesh-bevel.md) — rikka-mesh-bevel ports the reference bevel onto EditMesh: entry points, input model, profiles, modules, oracle tests.

## crates/model/rikka-mesh-boolean

- [Mesh booleans and intersection (rikka-mesh-boolean)](mesh-boolean.md) — rikka-mesh-boolean's Exact/Fast boolean solvers, edit-mode Intersect operators, exact kernel and public 2D CDT.

## crates/model/rikka-mesh-edit

- [Editable mesh kernel (rikka-mesh-edit)](mesh-edit-kernel.md) — rikka-mesh-edit's EditMesh (editable half-edge mesh): ids, topology, interpolation, Euler ops, selection, walkers.

## crates/model/rikka-mesh-modifiers

- [Non-destructive modifier stack (rikka-mesh-modifiers)](mesh-modifiers.md) — rikka-mesh-modifiers: modifier descriptions, Codec, Context trait, evaluate/apply on Polymesh, origin maps.

## crates/model/rikka-mesh-reduce

- [Removal and merge operators (rikka-mesh-reduce)](mesh-reduce.md) — rikka-mesh-reduce's exact delete, dissolve, merge, weld, auto-merge, tris-to-quads, unsubdivide and decimate operators.

## crates/model/rikka-mmd

- [PMX and VMD codec](mmd-codec.md) — rikka-mmd reads and writes PMX 2.0/2.1 and VMD, following the reference tool's quirks, and solves IK and append bones.

## crates/model/rikka-polymesh

- [Shared polygon mesh (rikka-polymesh)](polymesh.md) — rikka-polymesh is the array-based polygon mesh every modeling crate shares: layout, data ops, IO.

## crates/model/rikka-spatial

- [Spatial queries (rikka-spatial)](spatial-queries.md) — rikka-spatial's Bvh, TriangleBvh, SegmentBvh, PointTree (reference-exact duplicates), robust kernels and View picking.

## crates/model/rikka-stl

- [STL codec (rikka-stl)](stl-codec.md) — rikka-stl reads binary and ASCII STL in millimetres, with VisCAM and Magics colours, and writes both forms.

## crates/model/rikka-test-meshes

- [Mesh robustness sweeps (rikka-test-meshes)](mesh-robustness-sweeps.md) — Shared awkward-mesh corpus, seeded operator/chain sweeps and reference-divergence rules for degenerate input.

## crates/model/rikka-vrm

- [VRM codec](vrm-codec.md) — rikka-vrm reads VRM 0.x and 1.0 into one shape and writes both (VrmExporter 1.0, VrmLegacyExporter 0.x).

## crates/nix

- [Nix editing libraries (crates/nix)](nix-libraries.md) — rikka-nix-edit edits and writes Nix losslessly in nixfmt style; rikka-nixos-layout is Rikka's /etc/nixos module set.
- [System updates: runs, live logs, notifications and the viewer](system-updates.md) — rikka-nixctl records runs with decoded nix logs; the shell notifies; rikka-updates shows live and past logs.

## crates/nix/rikka-nix

- [NixOS configuration model and privileged service](nixos-configuration.md) — rikka-nix edits Rikka-owned modules losslessly; rikka-nixctl authorizes, applies, updates and rolls back as root.

## crates/physics/rikka-physics

- [Physics layer and the coupled scene (rikka-physics)](physics-layer.md) — crates/physics layout; rikka-physics couples posed figures, GPU cloth and GPU liquid, live or baked, behind SceneSource.

## crates/physics/rikka-physics-cloth

- [GPU mesh cloth solver (rikka-physics-cloth)](physics-cloth.md) — Vulkan-compute XPBD cloth over tailor `ClothAsset`s: API, per-step passes, one workgroup per figure layer, drape output.

## crates/physics/rikka-physics-cloth/shaders

- [GPU cloth contacts (rikka-physics-cloth)](physics-cloth-contacts.md) — rikka-physics-cloth body probe and sides, push combining, collider rescue, wraps, cross-layer contact and contact radii.
- [GPU cloth solver internals (rikka-physics-cloth)](physics-cloth-solver.md) — rikka-physics-cloth layout colouring, step passes, substep order, rest, retidy, extent tethers and drape relief.

## crates/physics/rikka-physics-collision

- [Collision pipeline (rikka-physics-collision)](physics-collision.md) — Broadphase over three BVH4 trees, pair set, kind-bucketed narrowphase, recycling and per-worker manifold pools.

## crates/physics/rikka-physics-core

- [Physics foundation (rikka-physics-math, rikka-physics-core)](physics-core-math.md) — Wide over rikka-simd, the lane-choosing Simd, row transposes, Shared views, Fnv1a, Stopwatch and Workers.

## crates/physics/rikka-physics-fluid

- [GPU fluid solver physics (rikka-physics-fluid)](fluid-solver-physics.md) — rikka-physics-fluid units, grid, substep, walls, bodies, viscosity CG, sticky contacts, substances, bonds.
- [GPU particle fluid solver (rikka-physics-fluid)](fluid-solver.md) — rikka-physics-fluid: deterministic Vulkan-compute PBF fluid API, batches, snapshots, replay, readback planning, tests.

## crates/physics/rikka-physics-geometry/src/convex

- [Convex geometry in rikka-physics-geometry](physics-geometry-convex.md) — rikka-physics-geometry's convex half: shapes, GJK/EPA/TOI, hull blobs, SAT manifolds, batch kernels, collide entry.

## crates/physics/rikka-physics-geometry/src/triangulated

- [Triangulated geometry in rikka-physics-geometry](physics-geometry-triangulated.md) — rikka-physics-geometry's triangulated half: BVH4, mesh/height-field views, compounds and the mesh contact pipeline.

## crates/physics/rikka-physics-gpu/shaders/lib

- [Physics Slang kernels and their shared modules](physics-shaders.md) — crates/physics kernels are Slang: shared physics.* modules, buffer views, memory-model helpers, matrix and codegen.

## crates/physics/rikka-physics-rigid

- [Rigid-body core (rikka-physics-rigid)](physics-rigid.md) — RigidWorld layout, per-step stage list, island tasks vs coloured path and the joint framework.

## crates/physics/rikka-physics-tailor

- [Cloth data compiler (rikka-physics-tailor)](physics-tailor.md) — rikka-physics-tailor turns a Model into a GPU-free ClothAsset: analysis, settings sidecar, sheets, hair, binds, body.

## crates/physics/rikka-physics-world

- [Physics World facade (rikka-physics-world)](physics-facade.md) — rikka_physics_world::World is the rigid-only facade: fixed ticks, interpolation, seek/rebuild history, async steps.

## crates/proton

- [Proton client library and crypto](proton-library.md) — crates/proton's crate stack, product feeds, public crates, rikka-proton-pgp crypto and SRP, references, tests.

## crates/proton/rikka-proton-account

- [Proton account and keys](proton-account.md) — rikka-proton-account owns SRP sign-in, second factors, keys and SKLs, devices, orgs, addresses, settings and forks.

## crates/proton/rikka-proton-api

- [Proton API transport](proton-api-transport.md) — rikka-proton-api owns Proton's HTTP envelope, hosts, sessions, refresh, retries, pinning, DoH routing and multipart.

## crates/proton/rikka-proton-authenticator

- [Proton Authenticator codes and sync](proton-authenticator.md) — rikka-proton-authenticator parses otpauth entries, computes codes from given time, and syncs sealed entries.

## crates/proton/rikka-proton-client

- [Proton cache, client session, accounts and event loop](proton-cache-and-client.md) — rikka-proton-cache's sealed per-account SQLite store and rikka-proton-client's Proton, Session, Accounts and EventLoop.

## crates/proton/rikka-proton-contacts

- [Proton Contacts and Calendar](proton-contacts-calendar.md) — rikka-proton-contacts (vCard, send preferences) and rikka-proton-calendar (keys, events, RRULE, iTIP) over contentline.

## crates/proton/rikka-proton-drive

- [Proton Drive client](proton-drive.md) — rikka-proton-drive walks the node key hierarchy on v2 volume endpoints: files, photos, sharing, devices, docs, events.

## crates/proton/rikka-proton-drive-sync

- [Proton Drive sync engine](proton-drive-sync.md) — rikka-proton-drive-sync syncs local folders with Drive both ways as a Computers device over a SQLite state.

## crates/proton/rikka-proton-key-transparency

- [Proton Key Transparency](proton-key-transparency.md) — rikka-proton-key-transparency verifies others' keys and self-audits SKLs against Proton's KT log over the CT list.

## crates/proton/rikka-proton-mail

- [Proton Mail client](proton-mail.md) — rikka-proton-mail reads, decrypts, parses MIME, searches, filters and sends mail with web-equivalent send preferences.

## crates/proton/rikka-proton-pass

- [Proton Pass client and item model](proton-pass.md) — rikka-proton-pass holds the item protobufs and PassClient over shares, items, invites, files, links, aliases and orgs.

## crates/proton/rikka-proton-vpn

- [Proton VPN client](proton-vpn.md) — rikka-proton-vpn picks servers, gets certificates, connects through NetworkManager and runs the in-tunnel local agent.

## crates/remote

- [Remote desktop wire format and transport (rikka-remote-wire, rikka-remote-link)](remote-protocol-transport.md) — rikka-remote-wire's messages/datagrams and PROTOCOL.md; rikka-remote-link's QUIC, FEC, congestion control, pacer.

## crates/session/rikka-present

- [Wayland surface presenter (rikka-present)](surface-presenter.md) — rikka-present draws a wl_surface into its own exported dmabufs with explicit sync, buckets and a viewport; no WSI.

## crates/session/rikka-seam

- [Seam capability IPC](session-ipc.md) — rikka-seam's typed capabilities, exactly-once replies, per-subscriber event queues, forwarding and Unix sockets.

## crates/session/rikka-session

- [Shared session protocols](session-protocols.md) — rikka-session's typed compositor/shell contracts: windows, workspaces, shortcuts, wallpaper, agent, surfaces, profile.

## crates/session/rikka-settings

- [Settings schema and ownership](session-settings.md) — rikka-settings: transportable schemas, a three-layer cascade, one session writer, announcements and domain helpers.

## crates/session/rikka-virtual-input

- [Virtual input client (rikka-virtual-input)](virtual-input.md) — Seat, Pointer, TextKeyboard, caps Keyboard and PhysicalKeyboard over virtual pointer/keyboard; threads, units, drop.

## crates/status/rikka-sysmon

- [System monitor (rikka-sysmon)](system-monitor.md) — Shared CPU/memory/disk/network/GPU probes, hwmon sensors, RAPL, volumes, and the on-demand Machine sampler.

## crates/status/rikka-weather

- [Weather client and the shell's activities and moments](weather-and-activity.md) — rikka-weather (Open-Meteo, geocoding, cache); shell-activity derives Activities and events; shell Feeds hold them.

## crates/text/rikka-test-fonts

- [Test fonts and synthetic sfnt builders (rikka-test-fonts)](text-test-fonts.md) — Shared test-only helpers: session font lookup via $FONTCONFIG_FILE, a big-endian Writer, sfnt assembly.

## crates/text/rikka-text-catalog

- [Font discovery and matching (rikka-text-catalog)](text-catalog.md) — Fontconfig parsing, symlink-safe scan, persistent face index, CSS matching and cached script/emoji-aware fallback.

## crates/text/rikka-text-document

- [Rope document model (rikka-text-document)](text-document.md) — rikka-text-document owns the in-house byte-indexed Rope and the editable Content under rikka-ui and rikka-syntax.

## crates/text/rikka-text-font

- [OpenType parser (rikka-text-font)](text-font.md) — Zero-copy, never-panicking OpenType reader: tables, cmap, variations, outlines, metrics, layout and color readers.

## crates/text/rikka-text-layout

- [Paragraph shaping and line layout (rikka-text-layout)](text-layout.md) — Size-independent ShapedParagraph, per-width ParagraphLayout rows of px glyphs, carets, hits, selections, ellipsis.

## crates/text/rikka-text-raster

- [Glyph rasterizer (rikka-text-raster)](text-raster.md) — Rasterizer for masks, color glyphs (COLR, CBDT, sbix, SVG bytes) and the GPU curve blob for large glyphs.

## crates/text/rikka-text-shape

- [Shaping engine (rikka-text-shape) and oracle bench](text-shape.md) — Reference-equivalent shaping incl. Indic, Khmer, Myanmar and USE; benchmarks/text checks it against the reference.

## crates/text/rikka-text-unicode

- [Unicode data and algorithms (rikka-text-unicode)](text-unicode.md) — Generated UCD 17 tries, segmentation with SA dictionaries, line breaking, bidi, normalization, script runs, emoji.

## crates/ui

- [UI toolkit crate layering](ui-crate-layering.md) — symbols → rikka-ui → rikka-widgets → markdown, with syntax beside rikka-ui, and where a new widget or mechanism goes.

## crates/ui/rikka-animation

- [Animation primitives and motion tokens](motion.md) — rikka-animation's Spring/Curve/Channel and the rikka-ui token table every widget moves by.

## crates/ui/rikka-app

- [Native Wayland application runtime](application-runtime.md) — rikka-app runs typed windows on its own xdg-shell backend and poll loop: handoff, session capabilities, appearance.

## crates/ui/rikka-app/src/embedded

- [Embedding rikka-ui in another program's Vulkan frames](rikka-ui-embedding.md) — ExternalDevice/ExternalTarget, rikka-ui-host Composer, rikka_app::embedded and the SDK embedded_surface rules.

## crates/ui/rikka-app/src/win32

- [rikka-app Windows backend (src/win32)](win32-app-backend.md) — How rikka-app runs windows on Win32: loop, window procedure, custom frame, swapchain presenting, input, DnD, appearance.

## crates/ui/rikka-nested

- [Nested in-process compositor (rikka-nested)](compositor-nested.md) — Nested/View/nested_view: a private Wayland display on a rikka-nested thread whose toplevels show zero-copy in a widget.

## crates/ui/rikka-seat

- [Wayland seat input for rikka-ui hosts (rikka-seat)](seat-input.md) — rikka-seat turns wl_seat devices into rikka-ui Events per surface for rikka-app and the shell, with cursors and IME.

## crates/ui/rikka-syntax

- [Syntax highlighting](syntax-highlighting.md) — rikka-syntax parses with tree-sitter over rikka-ui's text model; rikka-theme owns each token colour.

## crates/ui/rikka-terminal

- [Terminal engine and grid widget](terminal-engine.md) — rikka-terminal owns PTY, alacritty emulation, OSC 52 clipboard, key/mouse encoding, search and the drawn grid.

## crates/ui/rikka-timeline

- [Shared keyframe store, easings and timeline widgets](timeline-keyframe.md) — rikka-keyframe (Sequence, Penner Easing, CubicBezier fit) and rikka-timeline (TimeScale, ticks, playhead, glyphs).

## crates/ui/rikka-ui

- [rikka-ui scrolling (Viewing, latching, anchoring, Place)](ui-scrolling.md) — Scroller follow rules, gesture latching, dominant axis, spring impulses, scroll anchoring, Place, scroll repaints.

## crates/ui/rikka-ui-host

- [Hosting rikka-ui (embedder, pacing, offscreen, images)](ui-hosting.md) — rikka-ui-host's Embedder and Hosted, paint threads, frame pacing, prefetch, 3D thread, image cache, imports, Pulse.

## crates/ui/rikka-ui/src/runtime

- [rikka-ui runtime (reconcile, node table, islands, paint walk)](ui-framework.md) — rikka-ui's reconciler, components and readers, node table and relative geometry, islands, paint diff walk, clips, wakes.

## crates/ui/rikka-ui/src/runtime/dispatch.rs

- [rikka-ui events, focus, commands and drag and drop](ui-input.md) — Event dispatch, focus rules, click counts, pointer interception, modals, standard commands, pointer lock, drag and drop.

## crates/ui/rikka-ui/src/runtime/layout

- [rikka-ui layout engine and sizing rules](ui-layout.md) — rikka-ui's four-sweep layout, relayout boundaries, min sizes, text wrapping, Shrinkable leaves, scroller pitfalls.

## crates/ui/rikka-ui/src/search

- [Text search across the desktop](text-search.md) — rikka_ui::search is the one search model; find_bar draws it over editors, documents and terminals.

## crates/ui/rikka-ui/src/view

- [UI authoring model](ui-authoring.md) — State structs, view functions, typed handlers, ambient effects, fetch, Styled/TextStyled, tokens, and the Widget trait.

## crates/ui/rikka-widgets/src/controls/node_canvas

- [Node canvas widget (rikka-widgets node_canvas)](node-canvas.md) — Generic node-graph editor widget: NodeGraph, Geometry hit tests, Selection, CanvasEdit/CanvasMenu, child-layer repaint.

## crates/ui/rikka-widgets/src/lists/collection

- [Collection engine](collection-engine.md) — One lazy engine windows every list and grid; Flex holds eager children and never scrolls.

## crates/ui/rikka-widgets/src/navigation

- [Tabs, split panes and cross-window tab drag](tabs-and-panes.md) — split/pane divide a window; tabs travel between one process's windows over Wayland drag-and-drop.

## crates/ui/rikka-widgets/src/navigation/page_stack.rs

- [Page turns](page-turns.md) — page_stack(at, content) gives one room to many pages; the page left behind is frozen and carried out.

## crates/ui/rikka-widgets/src/navigation/toolbar

- [Toolbars with priority overflow (rikka-widgets toolbar)](toolbar-overflow.md) — toolbar()/tool_group/tool_item/tool_button: groups never shrink, collapse by ToolPriority into a ⋯ menu.

## crates/ui/rikka-widgets/src/text/editing

- [rikka-ui text, editing engine and clipboard](ui-text-editing.md) — TextDocument, KeyEdit and WordRule, DisplayText, 16-line piece engine, code assists, IME, selection, clipboard.

## crates/web/rikka-html

- [HTML reading (rikka-html)](html.md) — rikka-html is the public dependency-free HTML tokenizer, forgiving tree, selector subset, entity decoder and escaper.

## crates/web/rikka-web

- [Web engine (rikka-web over Gecko)](web-engine.md) — rikka-web drives a rikka-gecko process per Runtime over rikka-gecko-protocol; sessions draw through a nested display.

## crates/win32/rikka-swapchain

- [Windows Vulkan swapchain presenter (rikka-swapchain)](swapchain-presenter.md) — rikka-swapchain presents rikka-renderer frames into an HWND through a VK_KHR_win32_surface FIFO swapchain.

## engine/gecko

- [Gecko engine sources, builds and releases (engine/gecko, xtask gecko)](gecko-engine.md) — engine/gecko holds Firefox pin, patches and the rikka Gecko project; xtask gecko builds, CI attests, release gecko pins…

## installer

- [Installer architecture](installer-architecture.md) — The isolated ratatui NixOS installer, layered core → sys → tui, built static for musl.

## minecraft

- [Minecraft Fabric mod build (Rikka Client)](minecraft.md) — minecraft/ holds the Rikka Client Fabric mod built by Bazel: lock, @minecraft repos, fabric_mod, run, mcsrc sources.

## minecraft/src/main/java/sh/rikka/client

- [Rikka Client mod framework (events, mods, settings, profiles)](minecraft-client-framework.md) — sh.rikka.client packages: EventBus, ClientHooks, Mod/HudMod/ModManager, rikka-ui layers and screens, settings, profiles.

## minecraft/src/main/java/sh/rikka/client/menu

- [Rikka Client ModMenu and HUD editor](minecraft-menu.md) — sh.rikka.client.menu (sidebar-and-cards ModMenu over a scrim) and sh.rikka.client.hud (drag/snap HUD editor).

## minecraft/src/main/java/sh/rikka/client/mods

- [Rikka Client camera, mechanic and visual mods](minecraft-gameplay-mods.md) — mods/camera, mods/mechanic, mods/visual (crosshair, hit color, target ring, time) and mods/item (physics, 2D) and their…

## minecraft/src/main/java/sh/rikka/client/mods/animation

- [Rikka Client Old Animations mod (1.7 visuals)](minecraft-old-animations.md) — mods/animation: one visual-only Mod, its mixin hooks, and the exact-matrix method that maps 1.7 item chains onto modern…

## minecraft/src/main/java/sh/rikka/client/mods/minimap

- [Rikka Client panel HUD mods (minimap, keystrokes, target info, packs)](minecraft-hud-mods.md) — mods/minimap terrain atlas and drawing; Keystrokes, MouseStrokes, InventoryDisplay, PackDisplay, TargetInfo in mods/hud.

## minecraft/src/main/java/sh/rikka/client/mods/motionblur

- [Rikka Client Motion Blur mod (velocity, frame blending, accumulation)](minecraft-motion-blur.md) — mods/motionblur: motion blur on own RenderPipelines; hook points, passes, frame history, uniforms.

## minecraft/src/main/java/sh/rikka/client/mods/rearview

- [Rikka Client Rearview mod (second world view behind the player)](minecraft-rearview.md) — mods/rearview: 16:9 HUD mirror rendered from vanilla parts; hooks, section sharing, compile scheduling, state save.

## minecraft/src/main/java/sh/rikka/client/mods/translate

- [Rikka Client Chat Translate mod and the Google translation client](minecraft-chat-translate.md) — sh.rikka.client.translate (Google translateHtml client, queue, chat markup) and mods/translate (inline chat translation…

## minecraft/src/main/java/sh/rikka/client/music

- [Rikka Client music player (local files, ModMenu page, Now Playing HUD)](minecraft-music.md) — sh.rikka.client.music: watched <gameDir>/rikka/music library, SoundTags covers, MusicPlayer queue, keys, HUD.

## nix/dist

- [Binary distribution (project-rikka/packages, nix/dist, release workflow)](distribution.md) — How Rikka ships prebuilt: portable builds, xtask release, the generated packages flake and its NixOS module.

## sdk

- [Application SDK (sdk/): IDL, codegen, bindings, native library](c-sdk.md) — librikka SDK: rikka.ridl object-model IDL, askama codegen per language, committed bindings packages, native C ABI.

## sdk/codegen/src/languages

- [SDK Go, C#, Java and Python bindings](sdk-language-bindings.md) — How the codegen projects rikka.ridl into Go, C#, Java and Python: shared rules, runtimes, Bazel rules and tests.

## sdk/examples

- [SDK usage examples (sdk/examples/)](sdk-examples.md) — sdk/examples/<language>/ holds the only runnable examples: same programs in all six languages, glob-discovered targets.

## system/rikka-boost

- [GPU interaction boost (rikka-boost)](gpu-boost.md) — Root rikka-boost service locks NVIDIA clock floors via NVML while the compositor reports input or animation.

## system/rikka-cast

- [Desktop portal ownership](system-portals.md) — Files owns FileChooser, rikka-cast owns ScreenCast; each picker or dialog is its own helper process.

## system/rikka-cli

- [The rikka command line](rikka-cli.md) — rikka-cli installs as `rikka`: clap noun-verb commands over seam and settings, --json output, install-time completions.

## system/rikka-compositor

- [Compositor backends and frame loop](compositor-runtime.md) — rikka-compositor's DRM/nested/headless backends, event-driven pacing, fences, glass damage and GPU limits.
- [Compositor session ownership](compositor-session.md) — The compositor hosts settings, applies session domains, owns shell lifetime, environment, displays and profiles.
- [Compositor windowing and protocols](compositor-windowing-protocols.md) — Floating/tiling layout, keybindings, workspaces, minimize, virtual input routing and protocol coverage.

## system/rikka-compositor/src

- [Compositor crate structure (rikka-compositor)](compositor-architecture.md) — rikka-compositor's src/: facade, Compositor fields, Parts, frame dispatch, contexts and settling, seams, tests.

## system/rikka-compositor/src/agent

- [The agent's own seat](agent-seat.md) — The compositor gives an agent a second seat, drawn as an accent ring, so it works the desktop beside the person.

## system/rikka-compositor/src/backend/kms

- [Multi-GPU displays (render GPU and display tiers)](multi-gpu.md) — How the KMS backend drives displays on every GPU: one renderer, per-card devices, the Tier ladder, naming, vkms tests.

## system/rikka-compositor/src/input

- [Compositor input, agent seat, X11 and selection wiring](compositor-input.md) — rikka-compositor input/ (seat host, delivery, keys, warps, IME, idle), agent/, caret/, activation, x11/, selection/.

## system/rikka-compositor/src/render

- [Compositor rendering, glass, frame callbacks and capture](compositor-render.md) — rikka-compositor render::Scenes, scan-out candidates, window views, frame throttle, glass panes, staging, capture.

## system/rikka-compositor/src/selection/xdnd.rs

- [XWayland drag-and-drop bridge](xdnd-bridge.md) — How the compositor and rikka-compositor-xwm bridge Wayland drags to XDND targets and X11 drags into Wayland windows.

## system/rikka-compositor/src/snap

- [Window snapping](compositor-snap.md) — Floating windows snap to edges and snap-bar zones, resize together along shared lines, and get snap assist.

## system/rikka-compositor/src/switcher

- [Window switcher (Alt+Tab)](window-switcher.md) — Compositor-owned Alt+Tab: MRU recency, synchronous key session, withheld keyboard, rikka.switcher seam, grid panel.

## system/rikka-compositor/src/windows

- [Compositor window policy (windows/, shell/, grabs, snap, animation)](compositor-windows.md) — rikka-compositor windows/ over Desktop<Record>: modes, shell events, move/resize grabs, snap, animation, workspaces.

## system/rikka-compositor/src/windows/decoration.rs

- [Server-side decorations (compositor-drawn captions)](server-side-decorations.md) — Which windows get a compositor caption, how frames change geometry, and where caption drawing and input live.

## system/rikka-remote

- [Remote desktop service (rikka-remote)](remote-desktop.md) — rikka-remote serves the live session over QUIC to rikka-remote-viewer: threads, encoding, recovery, pacing, auth.

## system/rikka-replay

- [Instant replay (rikka-replay)](instant-replay.md) — rikka-replay keeps the last minutes of displays or windows as encoded packets in RAM; shell saves, cuts and drafts them.

## system/rikka-shell/crates

- [Shell crate layout (framework, feature crates, edges)](shell-crates.md) — rikka-shell is a wiring binary over shell-kit, shell-runtime and one app-private crate per feature; edges, catalogs.
- [Shell keyboard, IME, polkit and notifications](shell-input-auth-notifications.md) — On-screen keyboard, fcitx5 candidates, polkit prompts and notification cards with history.
- [Shell screenshot and recording editors](shell-capture-editors.md) — markup draws over a just-taken screenshot; trim cuts a recording losslessly; both reopen from notification cards.

## system/rikka-shell/crates/shell-assistant

- [Desktop assistant sidebar](shell-assistant.md) — One left-edge assistant panel whose conversation outlives the panel, drives the desktop and works apps' own tools.

## system/rikka-shell/crates/shell-bar

- [Shell taskbar, tray and control centre](shell-taskbar-machine.md) — The per-output edge strip taskbar on any display edge, docking slides, tray, status and the control centre.

## system/rikka-shell/crates/shell-bar/src/indicators

- [Taskbar indicators (input method, AI usage)](shell-taskbar-indicators.md) — bar/indicators: the Indicator trait, the Shelf registry, one shared panel surface, and how to add an indicator.

## system/rikka-shell/crates/shell-bar/src/island

- [Taskbar island (clock or app icons in the strip)](shell-taskbar-island.md) — bar/island: clock face or app icons in the strip, cards that swell out of it (media, peeks, window previews).

## system/rikka-shell/crates/shell-bar/src/tasks

- [Taskbar tasks (running apps in the island, the dock's replacement)](shell-taskbar-tasks.md) — bar/tasks + task-board crate: the shown workspace's running apps as icons, click rules, previews, right-click menu.

## system/rikka-shell/crates/shell-capture

- [Shell capture and clipboard history](shell-capture-clipboard.md) — rikka.capture's frozen-frame picker, screenshots, recordings and snips; memory-only clipboard history.

## system/rikka-shell/crates/shell-color-picker

- [Shell colour picker](shell-color-picker.md) — rikka.color-picker: frozen-screen loupe picker per output, floating editor with history, shades, tuning, export.

## system/rikka-shell/crates/shell-desk

- [Desktop widgets (desk) architecture](desk-feature.md) — Per-wallpaper widget layouts: desk-layout crate, Board/Edges/DeskScene, scene morph, backdrop, overview, editor crate.

## system/rikka-shell/crates/shell-desk-editor

- [Desk widget editor and desktop menu](shell-desk-editor.md) — shell-desk-editor (Super+W edit mode: editor_canvas, overlay, framed card scene, panel) and the desk's menu.

## system/rikka-shell/crates/shell-desk/src/widgets

- [Desktop widget contents (desk/widgets)](desk-widgets.md) — The eight desk widget kinds: Live state, shared Sources, Body/card frame, ticks, needs, options and catalog keys.

## system/rikka-shell/crates/shell-emoji

- [Shell emoji picker](shell-emoji-picker.md) — A caret-placed overlay over committed Unicode/CLDR data that types or copies the chosen emoji.

## system/rikka-shell/crates/shell-kit

- [Shell surface framework](shell-framework.md) — rikka-shell hosts every desktop feature in one process over factory-built layer surfaces and one forwarded seam.

## system/rikka-shell/crates/shell-kit/src/widgets

- [Movable shell overlay windows](shell-floating-windows.md) — widgets::floating draws a shell overlay window dragged by any empty part that remembers where it was left.

## system/rikka-shell/crates/shell-launcher

- [Shell launcher](shell-launcher.md) — A recurring overlay over pure per-keystroke providers, fzy scoring, distant providers and a wallpaper gallery.

## system/rikka-shell/crates/shell-lock

- [Screen lock (shell lock screen, compositor lock transition, PAM)](lock-screen.md) — How Super+L, idle and sleep lock the session: ext-session-lock surfaces, PAM gate, compositor lock transition.

## system/rikka-shell/crates/shell-overview

- [Shell overview](shell-overview.md) — Super+Tab / 3-finger swipe overview: live compositor-drawn windows, card strip, thumbnails, drag, gestures.

## system/rikka-shell/crates/shell-power-saving

- [Shell power saving (idle screen off, sleep, dimming, power saver)](shell-power-saving.md) — shell-power-saving hosts rikka.power: idle dim, display sleep, suspend, low-battery saver, taskbar percentage.

## system/rikka-shell/crates/shell-translate

- [Shell translation window](shell-translate.md) — rikka.translate translates typed text or a captured region through a model with externalized prompts.

## system/rikka-shell/crates/shell-wallpaper

- [Shell wallpaper library and backdrop](shell-wallpaper-library.md) — The shell owns the wallpaper folder, thumbnails and backdrop; everyone else names a wallpaper by id.

## website

- [Project website (website/)](website.md) — Astro + TypeScript site on GitHub Pages at rikka.sh; Nix node_modules, theme colours and SDK examples from sources.

## xtask

- [xtask commands](xtask.md) — bazel run //xtask lays out shipped components, runs the session or one app, packs and releases the distribution.
