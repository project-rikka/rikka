package sh.rikka.client.music;

import java.util.Locale;

public final class PlaybackTime {
	private static final int SECONDS_PER_MINUTE = 60;
	private static final int SECONDS_PER_HOUR = 60 * SECONDS_PER_MINUTE;

	private PlaybackTime() {
	}

	public static String format(double seconds) {
		long whole = (long) Math.max(0.0, Math.floor(seconds));
		long hours = whole / SECONDS_PER_HOUR;
		long minutes = whole % SECONDS_PER_HOUR / SECONDS_PER_MINUTE;
		long rest = whole % SECONDS_PER_MINUTE;
		return hours > 0
			? String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, rest)
			: String.format(Locale.ROOT, "%d:%02d", minutes, rest);
	}
}
