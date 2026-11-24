package sh.rikka.client.music;

import java.io.IOException;
import java.nio.file.ClosedWatchServiceException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.FileVisitResult;
import java.nio.file.FileVisitOption;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.StandardWatchEventKinds;
import java.nio.file.WatchKey;
import java.nio.file.WatchService;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.FileTime;
import java.text.Collator;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import os.rikka.Image;

final class MusicLibrary implements AutoCloseable {
	private static final Logger LOGGER = LoggerFactory.getLogger(MusicLibrary.class);
	private static final Duration SETTLE_DELAY = Duration.ofMillis(750);
	private static final int MAX_DEPTH = 8;
	private static final int MAX_READERS = 4;
	private static final int MAX_COPY_SUFFIX = 1000;

	private final Path folder;
	private final Consumer<List<Track>> published;
	private final ScheduledExecutorService scanner = Executors.newSingleThreadScheduledExecutor(daemon("rikka-music-scan"));
	private final ExecutorService readers = Executors.newFixedThreadPool(
		Math.clamp(Runtime.getRuntime().availableProcessors() / 2, 1, MAX_READERS),
		Thread.ofPlatform().name("rikka-music-tags-", 0).daemon().factory()
	);
	private final WatchService watcher;
	private final Thread watching;
	private final AtomicBoolean scanScheduled = new AtomicBoolean();
	private final Map<Path, WatchKey> watches = new HashMap<>();
	private final Map<Path, Known> known = new HashMap<>();
	private @Nullable List<Track> latest;
	private final Comparator<Track> order;

	private record Stamp(long size, FileTime modified) {
	}

	private record Known(Stamp stamp, Track track) {
	}

	private MusicLibrary(Path folder, Consumer<List<Track>> published) throws IOException {
		this.folder = folder;
		this.published = published;
		this.watcher = folder.getFileSystem().newWatchService();
		this.watching = daemon("rikka-music-watch").newThread(this::watch);
		Collator collator = Collator.getInstance();
		collator.setStrength(Collator.SECONDARY);
		this.order = Comparator.comparing(Track::title, collator).thenComparing(Track::key);
	}

	static MusicLibrary open(Path folder, Consumer<List<Track>> published) {
		try {
			Files.createDirectories(folder);
			MusicLibrary library = new MusicLibrary(folder, published);
			library.watching.start();
			library.scanner.execute(library::scan);
			return library;
		} catch (IOException failure) {
			throw new IllegalStateException("failed to open the music folder " + folder, failure);
		}
	}

	Path folder() {
		return this.folder;
	}

	void importFiles(List<Path> files) {
		this.scanner.execute(() -> {
			for (Path file : files) {
				if (!Files.isRegularFile(file) || !TrackReader.isAudio(file)) {
					LOGGER.info("not importing {}: not a music file", file);
					continue;
				}
				try {
					copy(file);
				} catch (IOException failure) {
					LOGGER.error("failed to import {} into {}", file, this.folder, failure);
				}
			}
			requestScan();
		});
	}

	private void copy(Path file) throws IOException {
		FileName name = FileName.of(file);
		for (int suffix = 1; suffix <= MAX_COPY_SUFFIX; suffix++) {
			Path target = this.folder.resolve(name.withSuffix(suffix == 1 ? "" : " (" + suffix + ")"));
			try {
				Files.copy(file, target);
				return;
			} catch (FileAlreadyExistsException taken) {
				if (Files.isSameFile(file, target)) {
					return;
				}
			}
		}
		throw new FileAlreadyExistsException(this.folder.resolve(file.getFileName()).toString(), null, "every free name is taken");
	}

	private void watch() {
		while (true) {
			WatchKey key;
			try {
				key = this.watcher.take();
			} catch (InterruptedException interrupted) {
				Thread.currentThread().interrupt();
				return;
			} catch (ClosedWatchServiceException closed) {
				return;
			}
			key.pollEvents();
			key.reset();
			requestScan();
		}
	}

	private void requestScan() {
		if (this.scanScheduled.compareAndSet(false, true)) {
			this.scanner.schedule(this::scan, SETTLE_DELAY.toMillis(), TimeUnit.MILLISECONDS);
		}
	}

	private void scan() {
		this.scanScheduled.set(false);
		try {
			rescan();
		} catch (RuntimeException failure) {
			LOGGER.error("failed to scan the music folder {}", this.folder, failure);
		}
	}

	private void rescan() {
		Map<Path, Stamp> files;
		try {
			Files.createDirectories(this.folder);
			files = list();
		} catch (IOException failure) {
			LOGGER.error("failed to scan the music folder {}", this.folder, failure);
			return;
		}
		Map<Path, CompletableFuture<Track>> reading = new HashMap<>();
		Map<Path, Known> next = new HashMap<>();
		files.forEach((file, stamp) -> {
			Known previous = this.known.get(file);
			if (previous != null && previous.stamp().equals(stamp)) {
				next.put(file, previous);
			} else {
				reading.put(file, CompletableFuture.supplyAsync(() -> TrackReader.read(this.folder, file), this.readers));
			}
		});
		reading.forEach((file, track) -> next.put(file, new Known(files.get(file), track.join())));
		List<Track> tracks = new ArrayList<>(next.size());
		for (Known entry : next.values()) {
			tracks.add(entry.track());
		}
		tracks.sort(this.order);
		List<Track> shared = shareCovers(tracks);
		this.known.clear();
		for (Track track : shared) {
			this.known.put(track.path(), new Known(next.get(track.path()).stamp(), track));
		}
		if (shared.equals(this.latest)) {
			return;
		}
		this.latest = List.copyOf(shared);
		this.published.accept(this.latest);
	}

	private static List<Track> shareCovers(List<Track> tracks) {
		Map<String, Image> covers = new HashMap<>();
		List<Track> shared = new ArrayList<>(tracks.size());
		for (Track track : tracks) {
			String album = track.albumKey();
			@Nullable Image cover = track.cover();
			if (album.isEmpty() || cover == null) {
				shared.add(track);
				continue;
			}
			Image first = covers.putIfAbsent(album, cover);
			shared.add(first == null || first == cover ? track : track.withCover(first));
		}
		return shared;
	}

	private Map<Path, Stamp> list() throws IOException {
		Map<Path, Stamp> files = new HashMap<>();
		Files.walkFileTree(this.folder, EnumSet.of(FileVisitOption.FOLLOW_LINKS), MAX_DEPTH, new SimpleFileVisitor<>() {
			@Override
			public FileVisitResult preVisitDirectory(Path directory, BasicFileAttributes attributes) throws IOException {
				if (!directory.equals(MusicLibrary.this.folder) && isHidden(directory)) {
					return FileVisitResult.SKIP_SUBTREE;
				}
				WatchKey watch = MusicLibrary.this.watches.get(directory);
				if (watch == null || !watch.isValid()) {
					MusicLibrary.this.watches.put(directory, directory.register(
						MusicLibrary.this.watcher,
						StandardWatchEventKinds.ENTRY_CREATE,
						StandardWatchEventKinds.ENTRY_DELETE,
						StandardWatchEventKinds.ENTRY_MODIFY
					));
				}
				return FileVisitResult.CONTINUE;
			}

			@Override
			public FileVisitResult visitFile(Path file, BasicFileAttributes attributes) {
				if (attributes.isRegularFile() && !isHidden(file) && TrackReader.isAudio(file)) {
					files.put(file, new Stamp(attributes.size(), attributes.lastModifiedTime()));
				}
				return FileVisitResult.CONTINUE;
			}

			@Override
			public FileVisitResult visitFileFailed(Path file, IOException failure) {
				LOGGER.warn("skipping {} in the music folder", file, failure);
				return FileVisitResult.CONTINUE;
			}
		});
		this.watches.values().removeIf(watch -> !watch.isValid());
		return files;
	}

	private static boolean isHidden(Path path) {
		return path.getFileName().toString().startsWith(".");
	}

	@Override
	public void close() {
		try {
			this.watcher.close();
		} catch (IOException failure) {
			LOGGER.error("failed to stop watching {}", this.folder, failure);
		}
		this.scanner.shutdownNow();
		this.readers.shutdownNow();
	}

	private static ThreadFactory daemon(String name) {
		return Thread.ofPlatform().name(name).daemon().factory();
	}
}
