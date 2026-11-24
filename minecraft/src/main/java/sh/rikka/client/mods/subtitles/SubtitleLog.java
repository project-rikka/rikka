package sh.rikka.client.mods.subtitles;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.function.IntSupplier;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.SoundEventListener;
import net.minecraft.client.sounds.WeighedSoundEvents;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Util;
import net.minecraft.world.phys.Vec3;

final class SubtitleLog implements SoundEventListener {
	private static final Comparator<Subtitle> LEAST_RECENT = Comparator.comparingLong(Subtitle::heardAt);

	private final IntSupplier capacity;
	private final List<Subtitle> subtitles = new ArrayList<>();
	private long nextId;

	SubtitleLog(IntSupplier capacity) {
		this.capacity = capacity;
	}

	@Override
	public void onPlaySound(SoundInstance sound, WeighedSoundEvents soundEvent, float range) {
		Component subtitle = soundEvent.getSubtitle();
		if (subtitle == null) {
			return;
		}
		long now = Util.getMillis();
		for (Subtitle shown : this.subtitles) {
			if (shown.shows(subtitle)) {
				shown.heardAgain(sound, now);
				return;
			}
		}
		trim(this.capacity.getAsInt() - 1);
		this.subtitles.add(new Subtitle(this.nextId++, subtitle, range, sound, now));
	}

	List<Subtitle> shown(Vec3 listener, long now, long durationMillis) {
		this.subtitles.removeIf(subtitle -> subtitle.expired(now, durationMillis) || !subtitle.audibleFrom(listener));
		trim(this.capacity.getAsInt());
		return this.subtitles;
	}

	boolean isEmpty() {
		return this.subtitles.isEmpty();
	}

	void clear() {
		this.subtitles.clear();
	}

	private void trim(int capacity) {
		while (this.subtitles.size() > Math.max(0, capacity)) {
			this.subtitles.remove(this.subtitles.stream().min(LEAST_RECENT).orElseThrow());
		}
	}
}
