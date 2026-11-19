package sh.rikka.client.mods.hud;

enum HourCycle {
	H24("HH:mm", "HH:mm:ss"),
	H12("h:mm a", "h:mm:ss a");

	private final String pattern;
	private final String patternWithSeconds;

	HourCycle(String pattern, String patternWithSeconds) {
		this.pattern = pattern;
		this.patternWithSeconds = patternWithSeconds;
	}

	String pattern(boolean withSeconds) {
		return withSeconds ? this.patternWithSeconds : this.pattern;
	}
}
