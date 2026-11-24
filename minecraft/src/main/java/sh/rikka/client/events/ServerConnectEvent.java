package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public record ServerConnectEvent(String host, int port, boolean transfer) implements Event {
}
