package sh.rikka.client.translate;

import java.time.Duration;
import java.util.Optional;
import java.util.concurrent.locks.ReentrantLock;
import org.jspecify.annotations.Nullable;

final class GoogleKeys {
	private static final long KEY_LIFETIME = Duration.ofMinutes(20).toNanos();
	private static final long RETRY_AFTER_FAILURE = Duration.ofMinutes(1).toNanos();
	private static final int KEY_LENGTH = 39;

	private final ReentrantLock lock = new ReentrantLock();
	private @Nullable String key;
	private long fetchedAt;
	private boolean failed;

	interface Scraper {
		Optional<String> scrape();
	}

	Optional<String> get(Scraper scraper) {
		this.lock.lock();
		try {
			long now = System.nanoTime();
			String known = this.key;
			if (known != null && now - this.fetchedAt < KEY_LIFETIME) {
				return Optional.of(known);
			}
			if (this.failed && now - this.fetchedAt < RETRY_AFTER_FAILURE) {
				return Optional.empty();
			}
			Optional<String> scraped = scraper.scrape();
			this.key = scraped.orElse(null);
			this.fetchedAt = now;
			this.failed = scraped.isEmpty();
			return scraped;
		} finally {
			this.lock.unlock();
		}
	}

	void invalidate(String rejected) {
		this.lock.lock();
		try {
			if (rejected.equals(this.key)) {
				this.key = null;
			}
		} finally {
			this.lock.unlock();
		}
	}

	static Optional<String> find(String page, String name) {
		for (int start = 1; start + name.length() < page.length(); start++) {
			if (!page.regionMatches(true, start, name, 0, name.length())) {
				continue;
			}
			int end = start + name.length();
			if (!isQuote(page.charAt(start - 1)) || !isQuote(page.charAt(end))) {
				continue;
			}
			int at = skipSpaces(page, end + 1);
			if (at >= page.length() || page.charAt(at) != ':') {
				continue;
			}
			at = skipSpaces(page, at + 1);
			if (at >= page.length() || !isQuote(page.charAt(at))) {
				continue;
			}
			int keyStart = at + 1;
			int keyEnd = keyStart + KEY_LENGTH;
			if (keyEnd < page.length() && isKey(page, keyStart, keyEnd) && isQuote(page.charAt(keyEnd))) {
				return Optional.of(page.substring(keyStart, keyEnd));
			}
		}
		return Optional.empty();
	}

	private static boolean isKey(String page, int start, int end) {
		for (int index = start; index < end; index++) {
			char character = page.charAt(index);
			boolean alphanumeric = character < 0x80 && Character.isLetterOrDigit(character);
			if (!alphanumeric && character != '_' && character != '-') {
				return false;
			}
		}
		return true;
	}

	private static boolean isQuote(char character) {
		return character == '"' || character == '\'';
	}

	private static int skipSpaces(String page, int at) {
		int index = at;
		while (index < page.length() && Character.isWhitespace(page.charAt(index))) {
			index++;
		}
		return index;
	}
}
