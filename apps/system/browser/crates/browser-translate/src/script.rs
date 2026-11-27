use rikka_web::{Frames, InjectAt, Script, UserScript};

pub const MESSAGE: &str = "browser-translate";

const PROBE: Script = Script::bundled("browser-translate/probe.js");

const PROBE_ID: &str = "browser-translate-probe";

pub fn probe_script() -> UserScript {
    UserScript::new(PROBE_ID, PROBE)
        .frames(Frames::All)
        .inject_at(InjectAt::End)
}
