# Diagrams

Visual companions to [`ARCHITECTURE.md`](ARCHITECTURE.md): the same structure, drawn. The diagrams are
[Mermaid](https://mermaid.js.org/), which GitHub renders in place. Names (`App::advance`,
`Engine::step`, ...) are the ones in the source; the text of `ARCHITECTURE.md` and
[`ENGINE.md`](ENGINE.md) stays the reference when a diagram and the code disagree.

1. [System overview](#1-system-overview)
2. [Startup](#2-startup)
3. [The main loop](#3-the-main-loop)
4. [Two clocks: tick and step](#4-two-clocks-tick-and-step)
5. [One story step, end to end](#5-one-story-step-end-to-end)
6. [`Engine::step`](#6-enginestep)
7. [Lifecycle of a plan entry](#7-lifecycle-of-a-plan-entry)
8. [Canvases and the frame pipeline](#8-canvases-and-the-frame-pipeline)
9. [Presenting a frame](#9-presenting-a-frame)
10. [Story planning](#10-story-planning)
11. [Scripts: the ADS scheduler and TTM frames](#11-scripts-the-ads-scheduler-and-ttm-frames)
12. [Sound](#12-sound)
13. [Settings: `SCRANTIC.INI` and the command line](#13-settings-scranticini-and-the-command-line)

## 1. System overview

Dependencies point downward only: the front end knows the engine, the engine knows the assets, and the
engine never touches the window or the audio device.

```mermaid
flowchart TB
    subgraph disk["On the user's disk, never in the repo"]
        scr["SCRANTIC.SCR<br/>NE executable"]
        map["RESOURCE.MAP"]
        res["RESOURCE.001"]
        ini["SCRANTIC.INI"]
    end

    subgraph assets["assets/"]
        exe["exe.rs<br/>sounds, scene catalog,<br/>walk routes, RNG seed"]
        resource["resource.rs<br/>images, palette,<br/>TTM and ADS scripts"]
    end

    subgraph engine["engine/ (no I/O)"]
        story["mod.rs<br/>story planner, step, tick"]
        ads["ads.rs<br/>scene scheduler"]
        ttm["ttm.rs<br/>frame interpreter"]
        island["island.rs / walk.rs<br/>ambience, Johnny"]
        gfx["gfx.rs<br/>640x480 canvas"]
        config["config.rs<br/>INI keys"]
    end

    subgraph front["Front end"]
        main["main.rs<br/>window, input, clocks"]
        cli["cli.rs<br/>command line"]
        saver["saver.rs, windows_ui.rs<br/>Windows .scr build only"]
        present["present.rs<br/>palette, dedither, scale"]
        audio["audio.rs<br/>sound playback"]
    end

    scr --> exe
    map --> resource
    res --> resource
    ini <--> config
    exe --> story
    resource --> story
    story --> ads --> ttm --> gfx
    story --> island --> gfx
    main -->|"tick() every 16 ms<br/>step() every 55 ms"| story
    story -->|"frame()"| main
    main --> present --> win(["window<br/>winit + softbuffer"])
    story -->|"take_sounds()"| main --> audio --> dev(["sound device<br/>rodio"])
```

## 2. Startup

```mermaid
flowchart TD
    A["main()"] --> B["cli::parse_args<br/>flags and option overrides<br/>(.scr build: saver::start, Windows switches)"]
    B -->|"--help / --version / bad flag"| X(["print and exit"])
    B --> C["ScreensaverData::load<br/>reads the 3 files from app_dir()"]
    C -->|"missing, placeholder or unsupported build"| E(["error, exit 1"])
    C --> D["Config::load<br/>missing or invalid INI becomes defaults"]
    D --> D2["saved = loaded settings<br/>overrides applied to a copy"]
    D2 --> F["Engine::new<br/>palette, Johnny sprites, island,<br/>RNG seed, title screen in front"]
    F --> G["Audio::new<br/>no device: runs silently"]
    G --> H["EventLoop::run_app"]
    H --> I["resumed: create window and surface<br/>start = now + INTRO_HOLD 2 s"]
    I --> J(["main loop, see 3"])
```

## 3. The main loop

winit drives everything. The loop sleeps until the next 55 ms step is due, advances the engine, and
asks for a redraw only when the story actually stepped.

```mermaid
flowchart TD
    W["event loop waits<br/>ControlFlow::WaitUntil(next step)"] --> EV{"event"}

    EV -->|"about_to_wait"| ADV["App::advance()"]
    ADV --> STEPPED{"stepped,<br/>or intro not drawn yet?"}
    STEPPED -->|yes| RR["window.request_redraw()"]
    STEPPED -->|no| SLEEP
    RR --> SLEEP["set_control_flow(WaitUntil<br/>start + (steps+1) x 55 ms)"]
    SLEEP --> W

    EV -->|"RedrawRequested"| RD["App::redraw()"]
    RD --> RD1["frame -> RGB<br/>plain() or dedither()"]
    RD1 --> RD2["present::scale -> softbuffer buffer"]
    RD2 --> RD3["buffer.present()"]
    RD3 --> W

    EV -->|"Esc or CloseRequested"| EXIT["event_loop.exit()"]
    EXIT --> SAVE["exiting(): write SCRANTIC.INI"]
    EV -->|"F12"| SHOT["screenshot = true, request_redraw<br/>redraw saves -raw and -view PNGs"]
    SHOT --> W
```

## 4. Two clocks: tick and step

The original ran two Windows 3.1 timers, and both are emulated from one wall clock. For every due step,
all ticks up to that step's virtual time run first, then the step.

```mermaid
flowchart TD
    A["App::advance()"] --> B["due = elapsed_ms / 55"]
    B --> C{"due > steps + 20?<br/>long stall: sleep, debugger"}
    C -->|yes| D["resume: steps = due - 1<br/>no fast-forward"]
    C -->|no| E
    D --> E{"steps < due?"}
    E -->|no| Z(["return stepped"])
    E -->|yes| F["steps += 1<br/>virtual_ms = steps x 55"]
    F --> G{"(ticks + 1) x 16 <= virtual_ms?"}
    G -->|yes| H["ticks += 1<br/>Engine::tick()<br/>now += 1, timers count down"]
    H --> G
    G -->|no| I["Engine::step(clock)<br/>one story frame"]
    I --> J["for each id in take_sounds()<br/>Audio::play (unless Sounds=0)"]
    J --> E
```

| Clock | Period | Drives |
|-------|--------|--------|
| Tick (`TICK_MS`, `seg2:16ce`) | 16 ms | script delays, walk animation, idle pause, wave and whitecap timers |
| Step (`STEP_MS`, 50 ms `WM_TIMER`, 55 ms real granularity) | 55 ms | story and scheduler: composes one frame |

## 5. One story step, end to end

From the event loop to a pixel on screen and a sound in the speaker.

```mermaid
sequenceDiagram
    participant L as winit loop
    participant A as App::advance
    participant E as Engine
    participant S as AdsRuntime / ttm
    participant Au as Audio
    participant P as present
    participant W as softbuffer

    L->>A: about_to_wait
    loop each due step
        loop each due tick
            A->>E: tick()
        end
        A->>E: step(clock)
        E->>S: update(env)
        S-->>E: canvas drawn, sound ids queued
        E-->>A: front canvas swapped
        A->>E: take_sounds()
        E-->>A: sound ids
        A->>Au: play(bank, id)
    end
    A-->>L: stepped
    L->>L: request_redraw
    L->>P: RedrawRequested: plain or dedither
    P->>P: scale (sharp-bilinear, centered)
    P->>W: buffer.present()
```

A sound starts right after the script opcode that requested it ran, the same order as the original's
synchronous `sndPlaySound` call (`seg12:0064`, see [`ENGINE.md`](ENGINE.md)).

## 6. `Engine::step`

```mermaid
flowchart TD
    S(["step()"]) --> P{"plan exhausted?"}
    P -->|yes| NS["new_session<br/>see 10"]
    P -->|no| C
    NS --> C["work = copy of bg"]
    C --> I["Island::step<br/>whitecaps, shore waves, raft"]
    I --> AR{"entry pending and<br/>Johnny not at start spot?"}
    AR -->|yes| WK["walk_step<br/>walk toward the spot"]
    AR -->|no| K{"entry kind"}
    K -->|"ADS id 0xFF<br/>standing pause"| ID["first time: idle_timer = 180..419 ticks<br/>walk_step (idle animation)"]
    ID --> IDD{"idle_timer == 0?"}
    IDD -->|yes| DONE
    IDD -->|no| H
    K -->|"ADS scene"| FIRST{"first step of the entry?"}
    FIRST -->|yes| LD["load_ads, start_tag,<br/>set TTM offset"]
    FIRST -->|no| UP
    LD --> UP["AdsRuntime::update(env)<br/>see 11"]
    UP --> BUSY{"tag_busy?"}
    BUSY -->|yes| H
    BUSY -->|no| DONE["complete_scene:<br/>spot and heading = scene end values,<br/>plan_idx += 1, chapter bookkeeping"]
    WK --> H
    DONE --> H["draw_holiday"]
    H --> CH{"story scene just started?"}
    CH -->|yes| CHIME["queue chime sound 25"]
    CH -->|no| SW
    CHIME --> SW["swap(front, work)"]
    SW --> E(["frame() returns front"])
```

## 7. Lifecycle of a plan entry

Each entry of a session's `plan` is one catalog scene. `pending[i]` is true until its start has run.

```mermaid
stateDiagram-v2
    [*] --> Pending: plan_session chose the scene
    Pending --> Walking: Johnny is not at the start spot
    Walking --> Starting: Walk go_to reports arrival
    Pending --> Starting: already at the start spot
    Starting --> Running: ADS loaded and tag started<br/>(or idle timer set)
    Running --> Running: every step runs the script
    Running --> Complete: tag no longer busy<br/>(or idle timer reached 0)
    Complete --> Pending: plan_idx advances to the next entry
    Complete --> [*]: plan exhausted, new session
```

## 8. Canvases and the frame pipeline

Three 640x480 canvases of palette indices. Rebuilding `work` from `bg` on every step replaces the
original's dirty rectangles (a documented deviation).

```mermaid
flowchart LR
    BG[("bg<br/>persistent background:<br/>ocean or night, island,<br/>clouds baked in")]
    WK["work<br/>scratch frame"]
    FR[("front<br/>last presented frame")]

    BG -->|"copy, every step"| WK
    WK --> D1["Island::step<br/>waves, whitecaps, raft"]
    D1 --> D2["Walk / scene<br/>Johnny and props<br/>through TTM frames"]
    D2 --> D3["draw_holiday"]
    D3 -->|"swap"| FR
    D2 -.->|"TTM 0x4204 and LOAD_SCREEN<br/>write the background"| BG
    FR --> OUT["Engine::frame()"]
```

## 9. Presenting a frame

Everything here is presentation only: the engine's canvas is untouched. The original showed the canvas
centered at 1:1 on a black full-screen window; the scaling below is a deliberate enlargement.

```mermaid
flowchart TD
    F["front canvas<br/>640x480 palette indices"] --> Q{"--dedither?"}
    Q -->|no| PL["present::plain<br/>palette lookup"]
    Q -->|yes| DD["present::dedither<br/>2x2 checkerboards and alternating<br/>runs blended in sRGB"]
    PL --> SC
    DD --> SC["present::scale"]
    SC --> K["k = min(w / 640, h / 480)<br/>image size = 640k x 480k, centered,<br/>black bars on the other axis"]
    K --> M{"integer k?"}
    M -->|yes| REP["exact pixel replication"]
    M -->|no| SB["sharp-bilinear<br/>pixels stay solid; a pixel blends with its<br/>neighbor only within 1 screen pixel of the edge"]
    REP --> BUF["softbuffer buffer"]
    SB --> BUF
    BUF --> WIN(["window"])
```

## 10. Story planning

A **session** is the list of scenes Johnny plays through; when it runs out, a new one is planned.

```mermaid
flowchart TD
    N["new_session"] --> N1["day phase: 16 half-hours from StartTime<br/>(12 day, 4 night)"]
    N1 --> N2["holiday from the date;<br/>reset island configuration"]
    N2 --> N3["update_day: advance the story chapter<br/>once per calendar day"]
    N3 --> PS["plan_session(phase, PLAN_BUDGET = 300)"]

    PS --> P1{"10% chance"}
    P1 -->|yes| P1A["pick the chapter-of-the-day scene<br/>(mask 0x80)"]
    P1 -->|no| P2
    P1A --> P2["pick a final scene (mask 4)<br/>different from the last final one"]
    P2 --> P3{"rules call for an opening?"}
    P3 -->|yes| P3A["pick an opening scene (mask 1)"]
    P3 -->|no| P4
    P3A --> P4["fill the budget with middle scenes (mask 2)<br/>drawn by weight; repeatable ones may repeat"]
    P4 --> P5["append the final scene"]
    P5 --> PL(["plan = list of catalog indices"])

    PK["pick() filters the 80-entry catalog by role,<br/>day phase, story day and island compatibility;<br/>every choice calls Island::apply_flags"] -.-> P1A
    PK -.-> P2
    PK -.-> P3A
    PK -.-> P4
```

## 11. Scripts: the ADS scheduler and TTM frames

Two languages, one on top of the other: ADS sequences, TTM animates.

```mermaid
flowchart TD
    U["AdsRuntime::update(env)<br/>every step (seg10:1acb)"] --> U1["1. propagate each tag's state<br/>to its scenes (a restarted tag resets them)"]
    U1 --> U2["2. re-run the ADS script of every active tag<br/>from its re-entry point;<br/>block conditions decide what happens"]
    U2 --> U3["3. for every running scene, in z-order:<br/>ttm::run_frame on its current frame"]
    U3 --> U4["advance frames whose delay elapsed<br/>(Scene::advance)"]

    U3 --> RF["run_frame interprets one frame's opcodes"]
    RF --> FX{"first execution<br/>of this frame?"}
    FX -->|yes| FE["side effects: load images and screens,<br/>queue sounds, random delays"]
    FX -->|no| FR["redraw only (Scene::redraw)"]
    FE --> RES
    FR --> RES["FrameResult:<br/>new delay, purge flag, optional GOTO"]
```

`Env` is everything a frame may touch: the three canvases, the archive, Johnny's sprites, the RNG, the
sound queue, the island offset and the tick counter.

## 12. Sound

The original called `sndPlaySound(ptr, SND_MEMORY | SND_ASYNC | SND_NODEFAULT)`: a new sound cuts the
previous one, and the game did no buffering of its own.

```mermaid
flowchart TD
    OP["TTM sound opcode"] --> Q["Env::sounds queue"]
    Q --> TS["Engine::take_sounds()<br/>after each step"]
    TS --> G{"Sounds=1 and a device?"}
    G -->|no| SIL(["silent"])
    G -->|yes| PLAY["Audio::play(bank, id)"]
    PLAY --> M{"--soft-audio?"}

    M -->|no| PLN["Plain<br/>new Player on the mixer;<br/>dropping the previous one cuts it hard"]
    PLN --> RS1["rodio resamples 11,025 Hz to the device<br/>by linear interpolation"]

    M -->|yes| SFT["Soft<br/>resampled once per sound to the device rate<br/>(low-passed Lanczos-windowed sinc, cached)"]
    SFT --> MIX["one endless mono Stream;<br/>a cut sound fades out over 3 ms"]

    RS1 --> OUT["device buffer<br/>Linux: fixed 8192 frames<br/>elsewhere: the driver's default"]
    MIX --> OUT
    OUT --> SPK(["speaker"])
```

## 13. Settings: `SCRANTIC.INI` and the command line

Four options (`Background`, `Clouds`, `Waves`, `Sounds`) can be overridden for one run. The rest of the
file is story progress and is not user-facing.

```mermaid
flowchart TD
    L["Config::load<br/>SCRANTIC.INI or defaults"] --> SV["saved: the settings as loaded"]
    L --> CP["copy"]
    CLI["--background=N --clouds=N<br/>--waves=N --sounds=N"] -->|"override"| CP
    CP --> ENG["Engine::new(config)<br/>the run uses the overridden values"]

    ENG --> RUN(["run: story progress changes<br/>NumDays, Introduction, date, StartTime"])
    RUN --> EXIT["exiting() on a clean exit"]
    EXIT --> W["engine config + the four options<br/>taken back from saved"]
    SV --> W
    W --> FILE[("SCRANTIC.INI")]
```

An override therefore never reaches the file, while story progress does. A crash or a kill writes nothing.
