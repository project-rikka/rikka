package sh.rikka.client.music;

import sh.rikka.client.event.Event;

public final class MusicChangeEvent implements Event {
	static final MusicChangeEvent INSTANCE = new MusicChangeEvent();

	private MusicChangeEvent() {
	}
}
