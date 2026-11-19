package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public record HitEvent(int targetId, double reach) implements Event {
}
