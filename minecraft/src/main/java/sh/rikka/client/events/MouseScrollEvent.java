package sh.rikka.client.events;

import sh.rikka.client.event.CancellableEvent;

public final class MouseScrollEvent extends CancellableEvent {
	private final double horizontal;
	private final double vertical;

	MouseScrollEvent(double horizontal, double vertical) {
		this.horizontal = horizontal;
		this.vertical = vertical;
	}

	public double horizontal() {
		return this.horizontal;
	}

	public double vertical() {
		return this.vertical;
	}
}
