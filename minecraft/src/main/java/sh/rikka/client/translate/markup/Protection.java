package sh.rikka.client.translate.markup;

import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.stream.Collectors;

public final class Protection {
	private static final List<String> SCHEMES = List.of("http://", "https://", "ftp://", "ws://", "wss://");
	private static final String OPENERS = "([{\"'<";
	private static final String TRAILERS = ".,;:!?\"')]}>";
	private static final int MIN_NAME_LENGTH = 3;

	private final Set<String> names;

	private Protection(Set<String> names) {
		this.names = names;
	}

	public static Protection none() {
		return new Protection(Set.of());
	}

	public static Protection withNames(Collection<String> names) {
		return new Protection(names.stream()
			.filter(name -> name.length() >= MIN_NAME_LENGTH)
			.map(name -> name.toLowerCase(Locale.ROOT))
			.collect(Collectors.toUnmodifiableSet()));
	}

	List<Span> spans(String text) {
		List<Span> found = new ArrayList<>();
		words(text, found);
		names(text, found);
		found.sort(Comparator.comparingInt(Span::start).thenComparing(Span::end, Comparator.reverseOrder()));
		List<Span> disjoint = new ArrayList<>(found.size());
		int covered = 0;
		for (Span span : found) {
			if (span.start() >= covered) {
				disjoint.add(span);
				covered = span.end();
			}
		}
		return disjoint;
	}

	private static void words(String text, List<Span> found) {
		int at = 0;
		while (at < text.length()) {
			if (Character.isWhitespace(text.charAt(at))) {
				at++;
				continue;
			}
			int start = at;
			while (at < text.length() && !Character.isWhitespace(text.charAt(at))) {
				at++;
			}
			Span core = trim(text, start, at);
			if (core.end() > core.start() && isCode(text.substring(core.start(), core.end()))) {
				found.add(core);
			}
		}
	}

	private void names(String text, List<Span> found) {
		if (this.names.isEmpty()) {
			return;
		}
		int at = 0;
		while (at < text.length()) {
			if (!isNameCharacter(text.charAt(at))) {
				at++;
				continue;
			}
			int start = at;
			while (at < text.length() && isNameCharacter(text.charAt(at))) {
				at++;
			}
			if (this.names.contains(text.substring(start, at).toLowerCase(Locale.ROOT))) {
				found.add(new Span(start, at));
			}
		}
	}

	private static Span trim(String text, int start, int end) {
		int from = start;
		int to = end;
		while (from < to && OPENERS.indexOf(text.charAt(from)) >= 0) {
			from++;
		}
		while (to > from && TRAILERS.indexOf(text.charAt(to - 1)) >= 0) {
			to--;
		}
		return new Span(from, to);
	}

	private static boolean isCode(String word) {
		return isUrl(word) || isEmail(word) || isMention(word) || isCommand(word);
	}

	private static boolean isUrl(String word) {
		String lower = word.toLowerCase(Locale.ROOT);
		for (String scheme : SCHEMES) {
			if (lower.startsWith(scheme) && lower.length() > scheme.length()) {
				return true;
			}
		}
		return lower.startsWith("www.") && lower.indexOf('.', 4) > 4;
	}

	private static boolean isEmail(String word) {
		int at = word.indexOf('@');
		if (at <= 0 || at != word.lastIndexOf('@')) {
			return false;
		}
		String local = word.substring(0, at);
		String domain = word.substring(at + 1);
		int dot = domain.lastIndexOf('.');
		if (dot <= 0 || dot == domain.length() - 1 || domain.contains("..")) {
			return false;
		}
		String tld = domain.substring(dot + 1);
		return local.chars().allMatch(c -> isAsciiAlphanumeric(c) || "._%+-".indexOf(c) >= 0)
			&& domain.chars().allMatch(c -> isAsciiAlphanumeric(c) || c == '.' || c == '-')
			&& tld.length() >= 2
			&& tld.chars().allMatch(c -> c < 0x80 && Character.isLetter(c));
	}

	private static boolean isMention(String word) {
		return word.length() > 1 && word.charAt(0) == '@' && word.substring(1).chars().allMatch(Protection::isNameCharacter);
	}

	private static boolean isCommand(String word) {
		return word.length() > 1
			&& word.charAt(0) == '/'
			&& word.charAt(1) < 0x80
			&& Character.isLetter(word.charAt(1))
			&& word.substring(1).chars().allMatch(c -> isNameCharacter(c) || c == ':' || c == '-');
	}

	private static boolean isNameCharacter(int character) {
		return isAsciiAlphanumeric(character) || character == '_';
	}

	private static boolean isAsciiAlphanumeric(int character) {
		return character < 0x80 && Character.isLetterOrDigit(character);
	}

	record Span(int start, int end) {
	}
}
