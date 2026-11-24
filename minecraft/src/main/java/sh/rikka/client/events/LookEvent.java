package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public final class LookEvent implements Event {
	private double horizontal;
	private double vertical;

	LookEvent() {
	}

	void update(double horizontal, double vertical) {
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
