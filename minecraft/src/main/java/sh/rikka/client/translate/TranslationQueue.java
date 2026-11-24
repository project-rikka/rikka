package sh.rikka.client.translate;

import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.SequencedSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.Executor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.translate.TranslationFailure.Reason;

public final class TranslationQueue {
	private static final Logger LOGGER = LoggerFactory.getLogger(TranslationQueue.class);
	private static final int MAX_IN_FLIGHT = 4;
	private static final int CACHE_SIZE = 512;
	private static final long BACKGROUND_PAUSE = Duration.ofMinutes(1).toNanos();

	private final GoogleTranslate service;
	private final Executor worker;
	private final Executor owner;
	private final Map<Key, Translated> cache = new LinkedHashMap<>(CACHE_SIZE, 0.75F, true) {
		private static final long serialVersionUID = 1L;

		@Override
		protected boolean removeEldestEntry(Map.Entry<Key, Translated> eldest) {
			return size() > CACHE_SIZE;
		}
	};
	private final Map<Key, CompletableFuture<Translated>> inFlight = new HashMap<>();
	private final SequencedSet<Key> urgent = new LinkedHashSet<>();
	private final SequencedSet<Key> background = new LinkedHashSet<>();
	private int running;
	private boolean paused;
	private long pausedAt;

	public TranslationQueue(Executor worker, Executor owner) {
		this.service = new GoogleTranslate(worker);
		this.worker = worker;
		this.owner = owner;
	}

	public CompletableFuture<Translated> request(String html, String target, Urgency urgency) {
		Key key = new Key(html, target);
		Translated cached = this.cache.get(key);
		if (cached != null) {
			return CompletableFuture.completedFuture(cached);
		}
		CompletableFuture<Translated> known = this.inFlight.get(key);
		if (known != null) {
			if (urgency == Urgency.USER && this.background.remove(key)) {
				this.urgent.add(key);
				flush();
			}
			return known;
		}
		if (urgency == Urgency.BACKGROUND && isPaused()) {
			return CompletableFuture.failedFuture(rateLimited());
		}
		CompletableFuture<Translated> future = new CompletableFuture<>();
		this.inFlight.put(key, future);
		if (urgency == Urgency.USER) {
			this.urgent.add(key);
			flush();
		} else {
			this.background.add(key);
		}
		return future;
	}

	public void flush() {
		while (this.running < MAX_IN_FLIGHT && !(this.urgent.isEmpty() && this.background.isEmpty())) {
			start(take(this.urgent.isEmpty() ? this.background : this.urgent));
		}
	}

	private static Batch take(SequencedSet<Key> keys) {
		String target = keys.getFirst().target();
		List<Key> taken = new ArrayList<>();
		int chars = 0;
		Iterator<Key> iterator = keys.iterator();
		while (iterator.hasNext() && taken.size() < GoogleTranslate.MAX_STRINGS) {
			Key key = iterator.next();
			int length = key.html().length();
			if (!key.target().equals(target) || (!taken.isEmpty() && chars + length > GoogleTranslate.MAX_CHARS)) {
				continue;
			}
			iterator.remove();
			taken.add(key);
			chars += length;
		}
		return new Batch(target, List.copyOf(taken));
	}

	private void start(Batch batch) {
		this.running++;
		List<String> html = batch.keys().stream().map(Key::html).toList();
		CompletableFuture.supplyAsync(() -> {
				try {
					return this.service.translate(html, batch.target());
				} catch (TranslationFailure failure) {
					throw new CompletionException(failure);
				}
			}, this.worker)
			.whenCompleteAsync((translated, error) -> finish(batch, translated, error), this.owner);
	}

	private void finish(Batch batch, List<Translated> translated, Throwable error) {
		this.running--;
		if (error == null) {
			for (int index = 0; index < batch.keys().size(); index++) {
				Key key = batch.keys().get(index);
				this.cache.put(key, translated.get(index));
				this.inFlight.remove(key).complete(translated.get(index));
			}
		} else {
			TranslationFailure failure = failure(error);
			for (Key key : batch.keys()) {
				this.inFlight.remove(key).completeExceptionally(failure);
			}
			if (failure.reason() == Reason.RATE_LIMITED) {
				pause();
			}
		}
		flush();
	}

	private void pause() {
		this.paused = true;
		this.pausedAt = System.nanoTime();
		TranslationFailure failure = rateLimited();
		for (Key key : this.background) {
			this.inFlight.remove(key).completeExceptionally(failure);
		}
		this.background.clear();
	}

	private boolean isPaused() {
		if (this.paused && System.nanoTime() - this.pausedAt >= BACKGROUND_PAUSE) {
			this.paused = false;
		}
		return this.paused;
	}

	private static TranslationFailure rateLimited() {
		return new TranslationFailure(Reason.RATE_LIMITED, "paused after rate limiting");
	}

	public static TranslationFailure failure(Throwable error) {
		Throwable cause = error instanceof CompletionException && error.getCause() != null ? error.getCause() : error;
		if (cause instanceof TranslationFailure failure) {
			return failure;
		}
		LOGGER.error("translation failed unexpectedly", cause);
		return new TranslationFailure(Reason.SERVICE, "translation failed: " + cause, cause);
	}

	private record Key(String html, String target) {
	}

	private record Batch(String target, List<Key> keys) {
	}
}
