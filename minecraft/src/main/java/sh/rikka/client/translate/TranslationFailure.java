package sh.rikka.client.translate;

import java.util.Locale;

public final class TranslationFailure extends Exception {
	private static final long serialVersionUID = 1L;

	private final Reason reason;

	public TranslationFailure(Reason reason, String message) {
		super(message);
		this.reason = reason;
	}

	public TranslationFailure(Reason reason, String message, Throwable cause) {
		super(message, cause);
		this.reason = reason;
	}

	public Reason reason() {
		return this.reason;
	}

	public enum Reason {
		NETWORK,
		TIMEOUT,
		RATE_LIMITED,
		REFUSED,
		SERVICE,
		MALFORMED,
		KEY_UNAVAILABLE,
		TOO_LONG;

		public String id() {
			return name().toLowerCase(Locale.ROOT);
		}
	}
}
