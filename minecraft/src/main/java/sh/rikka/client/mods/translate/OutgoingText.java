package sh.rikka.client.mods.translate;

import java.util.Locale;
import java.util.Optional;
import java.util.Set;

record OutgoingText(String prefix, String body) {
	static final String COMMAND_PREFIX = "/";

	private static final String NAMESPACE = "minecraft:";
	private static final Set<String> TARGETED = Set.of("msg", "tell", "w", "whisper", "pm");
	private static final Set<String> UNTARGETED = Set.of("me", "teammsg", "tm", "r", "reply");

	static Optional<OutgoingText> parse(String message) {
		if (!message.startsWith(COMMAND_PREFIX)) {
			return Optional.of(new OutgoingText("", message));
		}
		int nameEnd = message.indexOf(' ');
		if (nameEnd < 0) {
			return Optional.empty();
		}
		String name = message.substring(COMMAND_PREFIX.length(), nameEnd).toLowerCase(Locale.ROOT);
		if (name.startsWith(NAMESPACE)) {
			name = name.substring(NAMESPACE.length());
		}
		int bodyStart;
		if (TARGETED.contains(name)) {
			int targetStart = nameEnd + 1;
			int targetEnd = targetEnd(message, targetStart);
			if (targetEnd < 0 || targetEnd >= message.length() || message.charAt(targetEnd) != ' ') {
				return Optional.empty();
			}
			bodyStart = targetEnd + 1;
		} else if (UNTARGETED.contains(name)) {
			bodyStart = nameEnd + 1;
		} else {
			return Optional.empty();
		}
		String body = message.substring(bodyStart);
		if (body.isBlank()) {
			return Optional.empty();
		}
		return Optional.of(new OutgoingText(message.substring(0, bodyStart), body));
	}

	private static int targetEnd(String message, int start) {
		if (start >= message.length()) {
			return -1;
		}
		char first = message.charAt(start);
		if (first == '"') {
			int close = message.indexOf('"', start + 1);
			return close < 0 ? -1 : close + 1;
		}
		if (first == '@') {
			int bracket = message.indexOf('[', start);
			int space = message.indexOf(' ', start);
			if (bracket >= 0 && (space < 0 || bracket < space)) {
				int close = message.indexOf(']', bracket);
				return close < 0 ? -1 : close + 1;
			}
		}
		int space = message.indexOf(' ', start);
		return space < 0 ? message.length() : space;
	}

	String with(String translatedBody) {
		return this.prefix + translatedBody;
	}
}
