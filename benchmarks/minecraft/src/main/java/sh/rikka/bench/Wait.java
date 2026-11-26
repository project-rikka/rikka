package sh.rikka.bench;

import java.util.Locale;

public enum Wait {
	ACQUIRE,
	SUBMIT,
	PRESENT;

	String label() {
		return this.name().toLowerCase(Locale.ROOT);
	}
}
