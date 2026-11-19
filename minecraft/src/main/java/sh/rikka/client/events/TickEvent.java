package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public final class TickEvent {
	private TickEvent() {
	}

	public static final class Pre implements Event {
		static final Pre INSTANCE = new Pre();

		private Pre() {
		}
	}

	public static final class Post implements Event {
		static final Post INSTANCE = new Post();

		private Post() {
		}
	}
}
