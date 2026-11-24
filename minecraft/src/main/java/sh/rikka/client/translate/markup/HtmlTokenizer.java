package sh.rikka.client.translate.markup;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import org.jspecify.annotations.Nullable;

final class HtmlTokenizer {
	private static final Map<String, String> ENTITIES = Map.of(
		"amp", "&",
		"lt", "<",
		"gt", ">",
		"quot", "\"",
		"apos", "'",
		"nbsp", " "
	);
	private static final String COMMENT_OPEN = "<!--";
	private static final String COMMENT_CLOSE = "-->";

	private final String html;
	private final List<HtmlToken> tokens = new ArrayList<>();
	private final StringBuilder text = new StringBuilder();
	private int at;

	private HtmlTokenizer(String html) {
		this.html = html;
	}

	static List<HtmlToken> tokenize(String html) {
		HtmlTokenizer tokenizer = new HtmlTokenizer(html);
		tokenizer.run();
		return tokenizer.tokens;
	}

	private void run() {
		while (this.at < this.html.length()) {
			char character = this.html.charAt(this.at);
			if (character == '<' && markup()) {
				continue;
			}
			if (character == '&') {
				entity();
				continue;
			}
			this.text.append(character);
			this.at++;
		}
		flushText();
	}

	private boolean markup() {
		if (this.html.startsWith(COMMENT_OPEN, this.at)) {
			int end = this.html.indexOf(COMMENT_CLOSE, this.at + COMMENT_OPEN.length());
			this.at = end < 0 ? this.html.length() : end + COMMENT_CLOSE.length();
			return true;
		}
		int next = this.at + 1;
		if (next >= this.html.length()) {
			return false;
		}
		char after = this.html.charAt(next);
		if (after == '/' && next + 1 < this.html.length() && isNameStart(this.html.charAt(next + 1))) {
			return close(next + 1);
		}
		if (after == '!' || after == '?') {
			int end = this.html.indexOf('>', next);
			if (end < 0) {
				return false;
			}
			this.at = end + 1;
			return true;
		}
		return isNameStart(after) && open(next);
	}

	private boolean close(int nameStart) {
		int nameEnd = nameEnd(nameStart);
		int end = this.html.indexOf('>', nameEnd);
		if (end < 0) {
			return false;
		}
		flushText();
		this.tokens.add(new HtmlToken.Close(this.html.substring(nameStart, nameEnd).toLowerCase(Locale.ROOT)));
		this.at = end + 1;
		return true;
	}

	private boolean open(int nameStart) {
		int nameEnd = nameEnd(nameStart);
		Map<String, String> attributes = new HashMap<>();
		int index = nameEnd;
		while (index < this.html.length()) {
			index = skipSpaces(index);
			if (index >= this.html.length()) {
				return false;
			}
			char character = this.html.charAt(index);
			if (character == '>' || (character == '/' && this.html.startsWith("/>", index))) {
				boolean selfClosing = character == '/';
				flushText();
				this.tokens.add(new HtmlToken.Open(this.html.substring(nameStart, nameEnd).toLowerCase(Locale.ROOT), Map.copyOf(attributes), selfClosing));
				this.at = index + (selfClosing ? 2 : 1);
				return true;
			}
			int attributeEnd = index;
			while (attributeEnd < this.html.length() && isAttributeName(this.html.charAt(attributeEnd))) {
				attributeEnd++;
			}
			if (attributeEnd == index) {
				index++;
				continue;
			}
			String name = this.html.substring(index, attributeEnd).toLowerCase(Locale.ROOT);
			index = skipSpaces(attributeEnd);
			if (index < this.html.length() && this.html.charAt(index) == '=') {
				index = skipSpaces(index + 1);
				int valueEnd = valueEnd(index);
				if (valueEnd < 0) {
					return false;
				}
				attributes.put(name, decode(unquoted(index, valueEnd)));
				index = valueEnd;
			} else {
				attributes.put(name, "");
			}
		}
		return false;
	}

	private String unquoted(int start, int end) {
		char first = this.html.charAt(start);
		return first == '"' || first == '\'' ? this.html.substring(start + 1, end - 1) : this.html.substring(start, end);
	}

	private int valueEnd(int start) {
		if (start >= this.html.length()) {
			return -1;
		}
		char first = this.html.charAt(start);
		if (first == '"' || first == '\'') {
			int close = this.html.indexOf(first, start + 1);
			return close < 0 ? -1 : close + 1;
		}
		int index = start;
		while (index < this.html.length() && !Character.isWhitespace(this.html.charAt(index)) && this.html.charAt(index) != '>') {
			index++;
		}
		return index;
	}

	private void entity() {
		int end = this.html.indexOf(';', this.at + 1);
		String decoded = end < 0 ? null : entityValue(this.html.substring(this.at + 1, end));
		if (decoded == null) {
			this.text.append('&');
			this.at++;
			return;
		}
		this.text.append(decoded);
		this.at = end + 1;
	}

	static String decode(String text) {
		if (text.indexOf('&') < 0) {
			return text;
		}
		HtmlTokenizer tokenizer = new HtmlTokenizer(text);
		while (tokenizer.at < text.length()) {
			if (text.charAt(tokenizer.at) == '&') {
				tokenizer.entity();
			} else {
				tokenizer.text.append(text.charAt(tokenizer.at));
				tokenizer.at++;
			}
		}
		return tokenizer.text.toString();
	}

	private static @Nullable String entityValue(String name) {
		if (name.length() > 1 && name.charAt(0) == '#') {
			boolean hex = name.charAt(1) == 'x' || name.charAt(1) == 'X';
			String digits = name.substring(hex ? 2 : 1);
			if (digits.isEmpty()) {
				return null;
			}
			try {
				int codePoint = Integer.parseInt(digits, hex ? 16 : 10);
				return Character.isValidCodePoint(codePoint) ? Character.toString(codePoint) : null;
			} catch (NumberFormatException failure) {
				return null;
			}
		}
		return ENTITIES.get(name);
	}

	private void flushText() {
		if (!this.text.isEmpty()) {
			this.tokens.add(new HtmlToken.Text(this.text.toString()));
			this.text.setLength(0);
		}
	}

	private int nameEnd(int start) {
		int index = start;
		while (index < this.html.length() && isNamePart(this.html.charAt(index))) {
			index++;
		}
		return index;
	}

	private int skipSpaces(int start) {
		int index = start;
		while (index < this.html.length() && Character.isWhitespace(this.html.charAt(index))) {
			index++;
		}
		return index;
	}

	private static boolean isNameStart(char character) {
		return character < 0x80 && Character.isLetter(character);
	}

	private static boolean isNamePart(char character) {
		return character < 0x80 && (Character.isLetterOrDigit(character) || character == '-' || character == ':');
	}

	private static boolean isAttributeName(char character) {
		return !Character.isWhitespace(character) && character != '=' && character != '>' && character != '/' && character != '"' && character != '\'';
	}
}
