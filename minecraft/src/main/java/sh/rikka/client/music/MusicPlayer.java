package sh.rikka.client.music;

import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.Executor;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import os.rikka.Sound;
import sh.rikka.client.config.MusicRepeat;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;

public final class MusicPlayer implements AutoCloseable {
	private static final Logger LOGGER = LoggerFactory.getLogger(MusicPlayer.class);
	private static final double RESTART_THRESHOLD = 3.0;
	private static final double LOUDNESS_EXPONENT = 3.0;

	private final MusicLibrary library;
	private final Preferences preferences;
	private final EventBus events;
	private final PlayOrder order = new PlayOrder();
	private final Map<Path, String> failures = new HashMap<>();
	private List<Track> tracks = List.of();
	private boolean loaded;
	private @Nullable Cue cue;

	private sealed interface Cue permits Stopped, Started {
		Track track();
	}

	private record Stopped(Track track, double at) implements Cue {
	}

	private record Started(Track track, Sound sound) implements Cue {
	}

	private MusicPlayer(Path folder, Preferences preferences, EventBus events, Executor mainThread) {
		this.preferences = preferences;
		this.events = events;
		this.library = MusicLibrary.open(folder, tracks -> mainThread.execute(() -> replace(tracks)));
		preferences.musicVolume().onChange(volume -> applyVolume());
		preferences.musicMuted().onChange(muted -> applyVolume());
		preferences.musicRepeat().onChange(repeat -> applyRepeat());
	}

	public static MusicPlayer open(Path folder, Preferences preferences, EventBus events, Executor mainThread) {
		MusicPlayer player = new MusicPlayer(folder, preferences, events, mainThread);
		events.subscribe(player);
		return player;
	}

	public Path folder() {
		return this.library.folder();
	}

	public boolean isLoaded() {
		return this.loaded;
	}

	public List<Track> tracks() {
		return this.tracks;
	}

	public Optional<Track> current() {
		return Optional.ofNullable(currentTrack());
	}

	public boolean isCurrent(Track track) {
		return track.isSameFile(currentTrack());
	}

	public boolean isPlaying() {
		return this.cue instanceof Started started && started.sound().isPlaying();
	}

	public double position() {
		return switch (this.cue) {
			case null -> 0.0;
			case Stopped stopped -> stopped.at();
			case Started started -> started.sound().getPosition();
		};
	}

	public double duration() {
		return switch (this.cue) {
			case null -> 0.0;
			case Stopped stopped -> stopped.track().duration();
			case Started started -> {
				double opened = started.sound().getDuration();
				yield opened > 0.0 ? opened : started.track().duration();
			}
		};
	}

	public Optional<String> failure(Track track) {
		return Optional.ofNullable(this.failures.get(track.path()));
	}

	public double volume() {
		return this.preferences.musicVolume().get();
	}

	public boolean isMuted() {
		return this.preferences.musicMuted().get();
	}

	public boolean isShuffled() {
		return this.preferences.musicShuffle().get();
	}

	public MusicRepeat repeat() {
		return this.preferences.musicRepeat().get();
	}

	public void importFiles(List<Path> files) {
		this.library.importFiles(files);
	}

	public void play(Track track) {
		this.failures.remove(track.path());
		start(track, 0.0);
	}

	public void toggle() {
		switch (this.cue) {
			case null -> this.order.first(this::isPlayable).ifPresent(track -> start(track, 0.0));
			case Stopped stopped -> start(stopped.track(), stopped.at());
			case Started started when started.sound().isPlaying() -> pause();
			case Started started -> {
				started.sound().play();
				changed();
			}
		}
	}

	public void pause() {
		if (!(this.cue instanceof Started started) || !started.sound().isPlaying()) {
			return;
		}
		started.sound().pause();
		remember(started.sound().getPosition());
		changed();
	}

	public void next() {
		this.order.after(currentTrack(), true, this::isPlayable).ifPresent(track -> start(track, 0.0));
	}

	public void previous() {
		if (this.cue != null && position() > RESTART_THRESHOLD) {
			seek(0.0);
			return;
		}
		this.order.before(currentTrack(), this::isPlayable).ifPresent(track -> start(track, 0.0));
	}

	public void seek(double seconds) {
		switch (this.cue) {
			case null -> {
				return;
			}
			case Stopped stopped -> {
				this.cue = new Stopped(stopped.track(), seconds);
				remember(seconds);
			}
			case Started started -> {
				started.sound().seek(seconds);
				if (!started.sound().isPlaying()) {
					remember(seconds);
				}
			}
		}
		changed();
	}

	public void setVolume(double volume) {
		this.preferences.musicVolume().set(volume);
		this.preferences.musicMuted().set(false);
	}

	public void toggleMute() {
		this.preferences.musicMuted().toggle();
	}

	public void toggleShuffle() {
		this.preferences.musicShuffle().toggle();
		this.order.arrange(this.tracks, isShuffled(), currentTrack());
		changed();
	}

	public void cycleRepeat() {
		this.preferences.musicRepeat().cycle();
		changed();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		if (!(this.cue instanceof Started started)) {
			return;
		}
		switch (started.sound().getState()) {
			case ENDED -> advance(started.track());
			case FAILED -> fail(started);
			case LOADING, PAUSED, PLAYING -> {
			}
		}
	}

	private void advance(Track ended) {
		Optional<Track> next = this.order.after(ended, repeat() == MusicRepeat.ALL, this::isPlayable);
		if (next.isPresent()) {
			start(next.get(), 0.0);
		} else {
			stop(this.order.first(this::isPlayable).orElse(ended));
		}
	}

	private void fail(Started failed) {
		Track track = failed.track();
		String message = failed.sound().getErrorMessage();
		LOGGER.warn("failed to play {}: {}", track.path(), message);
		this.failures.put(track.path(), message);
		Optional<Track> next = this.order.after(track, true, this::isPlayable);
		if (next.isPresent()) {
			start(next.get(), 0.0);
		} else {
			stop(track);
		}
	}

	private void start(Track track, double at) {
		release();
		Sound opened = new Sound(track.path().toString());
		opened.setVolume(loudness());
		opened.setLooping(repeat() == MusicRepeat.ONE);
		if (at > 0.0) {
			opened.seek(at);
		}
		opened.play();
		this.cue = new Started(track, opened);
		rememberTrack();
		remember(at);
		changed();
	}

	private void stop(Track track) {
		release();
		this.cue = new Stopped(track, 0.0);
		rememberTrack();
		remember(0.0);
		changed();
	}

	private void release() {
		if (this.cue instanceof Started started) {
			started.sound().stop();
			started.sound().close();
		}
	}

	private void replace(List<Track> scanned) {
		this.tracks = scanned;
		this.failures.keySet().removeIf(path -> find(path).isEmpty());
		if (!this.loaded) {
			restore();
			this.order.arrange(scanned, isShuffled(), currentTrack());
			this.loaded = true;
		} else {
			refreshCue();
			this.order.merge(scanned, isShuffled());
		}
		changed();
	}

	private void restore() {
		String saved = this.preferences.musicTrack().get();
		this.tracks.stream().filter(track -> track.key().equals(saved)).findFirst().ifPresent(track -> {
			double position = this.preferences.musicPosition().get();
			this.cue = new Stopped(track, track.duration() > 0.0 ? Math.min(position, track.duration()) : position);
		});
	}

	private void refreshCue() {
		switch (this.cue) {
			case null -> {
			}
			case Stopped stopped -> {
				this.cue = find(stopped.track().path()).map(track -> new Stopped(track, stopped.at())).orElse(null);
				if (this.cue == null) {
					rememberTrack();
					remember(0.0);
				}
			}
			case Started started -> find(started.track().path()).ifPresent(track -> this.cue = new Started(track, started.sound()));
		}
	}

	private Optional<Track> find(Path path) {
		return this.tracks.stream().filter(track -> track.path().equals(path)).findFirst();
	}

	private @Nullable Track currentTrack() {
		Cue shown = this.cue;
		return shown != null ? shown.track() : null;
	}

	private boolean isPlayable(Track track) {
		return !this.failures.containsKey(track.path());
	}

	private double loudness() {
		return isMuted() ? 0.0 : Math.pow(volume(), LOUDNESS_EXPONENT);
	}

	private void applyVolume() {
		if (this.cue instanceof Started started) {
			started.sound().setVolume(loudness());
		}
		changed();
	}

	private void applyRepeat() {
		if (this.cue instanceof Started started) {
			started.sound().setLooping(repeat() == MusicRepeat.ONE);
		}
	}

	private void rememberTrack() {
		Track shown = currentTrack();
		this.preferences.musicTrack().set(shown != null ? shown.key() : "");
	}

	private void remember(double position) {
		this.preferences.musicPosition().set(position);
	}

	private void changed() {
		this.events.post(MusicChangeEvent.INSTANCE);
	}

	@Override
	public void close() {
		if (this.cue instanceof Started started) {
			remember(started.sound().getPosition());
		}
		release();
		this.cue = null;
		this.library.close();
	}
}
