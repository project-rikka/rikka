package sh.rikka.client.music;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Random;
import java.util.function.Predicate;
import org.jspecify.annotations.Nullable;

final class PlayOrder {
	private final Random random = new Random();
	private List<Track> tracks = List.of();

	void arrange(List<Track> library, boolean shuffled, @Nullable Track first) {
		if (!shuffled) {
			this.tracks = library;
			return;
		}
		List<Track> mixed = new ArrayList<>(library);
		Collections.shuffle(mixed, this.random);
		int index = indexOf(mixed, first);
		if (index > 0) {
			mixed.addFirst(mixed.remove(index));
		}
		this.tracks = List.copyOf(mixed);
	}

	void merge(List<Track> library, boolean shuffled) {
		if (!shuffled) {
			this.tracks = library;
			return;
		}
		Map<Path, Track> fresh = new LinkedHashMap<>();
		for (Track track : library) {
			fresh.put(track.path(), track);
		}
		List<Track> merged = new ArrayList<>(library.size());
		for (Track kept : this.tracks) {
			Track update = fresh.remove(kept.path());
			if (update != null) {
				merged.add(update);
			}
		}
		for (Track added : fresh.values()) {
			merged.add(this.random.nextInt(merged.size() + 1), added);
		}
		this.tracks = List.copyOf(merged);
	}

	Optional<Track> first(Predicate<Track> playable) {
		return this.tracks.stream().filter(playable).findFirst();
	}

	Optional<Track> after(@Nullable Track current, boolean wrap, Predicate<Track> playable) {
		return step(current, 1, wrap, playable);
	}

	Optional<Track> before(@Nullable Track current, Predicate<Track> playable) {
		return step(current, -1, true, playable);
	}

	private Optional<Track> step(@Nullable Track current, int direction, boolean wrap, Predicate<Track> playable) {
		int size = this.tracks.size();
		int start = indexOf(this.tracks, current);
		if (start < 0) {
			return direction > 0 ? first(playable) : this.tracks.reversed().stream().filter(playable).findFirst();
		}
		for (int offset = 1; offset < size || (offset == size && wrap); offset++) {
			int index = start + offset * direction;
			if (index < 0 || index >= size) {
				if (!wrap) {
					return Optional.empty();
				}
				index = Math.floorMod(index, size);
			}
			Track candidate = this.tracks.get(index);
			if (playable.test(candidate)) {
				return Optional.of(candidate);
			}
		}
		return Optional.empty();
	}

	private static int indexOf(List<Track> tracks, @Nullable Track track) {
		for (int index = 0; index < tracks.size(); index++) {
			if (tracks.get(index).isSameFile(track)) {
				return index;
			}
		}
		return -1;
	}
}
