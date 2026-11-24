package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public final class ResourceReloadEvent implements Event {
	static final ResourceReloadEvent INSTANCE = new ResourceReloadEvent();

	private ResourceReloadEvent() {
	}
}
