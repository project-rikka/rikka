package sh.rikka.client.startup;

import java.io.IOException;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import javax.management.JMException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class ClassPreloader {
	private static final Logger LOGGER = LoggerFactory.getLogger(ClassPreloader.class);
	private static final String DIRECTORY = "startup";
	private static final String THREAD_NAME = "Rikka Class Preloader";
	private static final int THREADS = 3;

	private final ClassLoader loader;
	private final List<String> names;
	private final AtomicInteger next = new AtomicInteger();
	private final AtomicInteger stale = new AtomicInteger();
	private final AtomicInteger running = new AtomicInteger(THREADS);

	private ClassPreloader(ClassLoader loader, List<String> names) {
		this.loader = loader;
		this.names = names;
	}

	public static void start(ClassLoader loader, Path modDirectory) {
		ClassLoadJournal journal = new ClassLoadJournal(modDirectory.resolve(DIRECTORY));
		List<String> names;
		try {
			names = journal.rotate();
			journal.record();
		} catch (IOException | JMException failure) {
			LOGGER.warn("failed to record loaded classes", failure);
			return;
		}
		if (names.isEmpty()) {
			return;
		}
		ClassPreloader preloader = new ClassPreloader(loader, names);
		for (int thread = 0; thread < THREADS; thread++) {
			Thread.ofPlatform().daemon().name(THREAD_NAME + ' ' + thread).start(preloader::drain);
		}
	}

	private void drain() {
		int index;
		while ((index = this.next.getAndIncrement()) < this.names.size()) {
			try {
				Class.forName(this.names.get(index), false, this.loader).getDeclaredConstructors();
			} catch (ClassNotFoundException | LinkageError | RuntimeException unloadable) {
				this.stale.incrementAndGet();
			}
		}
		if (this.running.decrementAndGet() == 0) {
			LOGGER.info("preloaded {} classes, {} no longer load", this.names.size() - this.stale.get(), this.stale.get());
		}
	}
}
