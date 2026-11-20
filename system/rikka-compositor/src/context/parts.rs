macro_rules! seat_parts {
    ($host:expr) => {
        rikka_compositor_seat::SeatParts::new(
            &mut $host.seats,
            &mut $host.surfaces,
            &$host.core.display,
        )
    };
}

macro_rules! person {
    ($host:expr) => {
        $crate::context::seat_parts!($host).into_seat($host.input.seat)
    };
}

macro_rules! agent_seat {
    ($host:expr) => {
        $crate::context::seat_parts!($host).into_seat($host.input.agent.seat())
    };
}

macro_rules! selection_parts {
    ($host:expr) => {
        rikka_compositor_selection::SelectionParts::new(
            &mut $host.selection.selections,
            &mut $host.shell,
            $crate::context::seat_parts!($host),
            &|surface| $host.windows.x11_window_of(surface).is_some(),
        )
    };
}

macro_rules! parts {
    ($host:expr) => {
        $crate::state::Parts {
            core: &mut $host.core,
            outputs: &mut $host.outputs,
            surfaces: &mut $host.surfaces,
            windows: &$host.windows,
            frames: &mut $host.frames,
            scenes: &mut $host.scenes,
            throttle: &mut $host.throttle,
            resting: &mut $host.resting,
            cursor: &mut $host.cursor,
            input: &mut $host.input,
            selection: &$host.selection,
            policy: &$host.policy,
            animations: &mut $host.animations,
            snap: &mut $host.snap,
        }
    };
}

pub(crate) use {agent_seat, parts, person, seat_parts, selection_parts};
