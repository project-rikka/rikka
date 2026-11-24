package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public final class DisconnectEvent implements Event {
	static final DisconnectEvent INSTANCE = new DisconnectEvent();

	private DisconnectEvent() {
	}
}
