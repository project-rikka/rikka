package sh.rikka.client.mods.subtitles;

import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.network.chat.Component;
import net.minecraft.world.phys.Vec3;

final class Subtitle {
	private static final long LOOPING_HOLD_MILLIS = 500L;

	private final long id;
	private final Component source;
	private final String text;
	private final float range;
	private final boolean looping;
	private SoundInstance sound;
	private Vec3 location;
	private long heardAt;
	private int plays;

	Subtitle(long id, Component source, float range, SoundInstance sound, long heardAt) {
		this.id = id;
		this.source = source;
		this.text = source.getString();
		this.range = range;
		this.looping = sound.isLooping();
		this.sound = sound;
		this.location = locationOf(sound);
		this.heardAt = heardAt;
		this.plays = 1;
	}

	boolean shows(Component subtitle) {
		return this.source.equals(subtitle);
	}

	void heardAgain(SoundInstance sound, long now) {
		if (sound != this.sound) {
			this.plays++;
			this.sound = sound;
		}
		this.location = locationOf(sound);
		this.heardAt = now;
	}

	private static Vec3 locationOf(SoundInstance sound) {
		return new Vec3(sound.getX(), sound.getY(), sound.getZ());
	}

	boolean expired(long now, long durationMillis) {
		return now - this.heardAt > durationMillis;
	}

	boolean audibleFrom(Vec3 listener) {
		return relative() || Float.isInfinite(this.range) || this.location.closerThan(listener, this.range);
	}

	float freshness(long now, long durationMillis) {
		long hold = this.looping ? LOOPING_HOLD_MILLIS : 0L;
		long faded = Math.max(0L, now - this.heardAt - hold);
		return 1.0F - Math.clamp((float) faded / Math.max(1L, durationMillis - hold), 0.0F, 1.0F);
	}

	long id() {
		return this.id;
	}

	long heardAt() {
		return this.heardAt;
	}

	String text() {
		return this.text;
	}

	Vec3 location() {
		return this.location;
	}

	boolean relative() {
		return this.sound.isRelative();
	}

	int plays() {
		return this.plays;
	}
}
