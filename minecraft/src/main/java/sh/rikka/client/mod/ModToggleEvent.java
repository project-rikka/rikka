package sh.rikka.client.mod;

import sh.rikka.client.event.Event;

public record ModToggleEvent(Mod mod, boolean enabled) implements Event {
}
