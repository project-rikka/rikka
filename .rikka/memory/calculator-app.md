---
title: Calculator application
description: A keypad window over calc-engine, a dependency-free engine that owns every key-state rule.
about: apps/system/calculator
tags: [application, calculator]
---

- `calc-engine` exposes only `Engine::press(Key)` and `accepts(Key)` over `display`, `display_ungrouped` (copy text), `expression` and `error`. It owns valid next keys, operator correction and post-equals behaviour.
- The app is the keypad and readout alone (`Calculator` state, button `press(|e: &mut Engine| ..)`). No modes, memory or typed parser.
- Typed arithmetic and units belong to the shell launcher's `fend-core` provider.
- Errors are semantic (`calc_engine::Error`). Wording lives in `calculator.error.*` catalogs, while operator glyphs are not translated.
- This is the smallest example of the State/handler authoring model.
