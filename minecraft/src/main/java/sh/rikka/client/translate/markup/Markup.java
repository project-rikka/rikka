package sh.rikka.client.translate.markup;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.Iterator;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

public final class Markup<S> {
	private static final String ID = "i";
	private static final String ELEMENT_TAG = "a";
	private static final String KEEP_TAG = "code";
	private static final String KEEP_ATTRIBUTES = " translate=\"no\"";
	private static final int ROOT = -1;
	private static final int MIN_ECHO_LENGTH = 2;
	private static final String SPACE = " ";

	private final String html;
	private final List<Slot<S>> slots;
	private final S rootStyle;

	private Markup(String html, List<Slot<S>> slots, S rootStyle) {
		this.html = html;
		this.slots = slots;
		this.rootStyle = rootStyle;
	}

	public static <S> Markup<S> encode(List<Segment<S>> segments, Protection protection) {
		if (segments.isEmpty()) {
			throw new IllegalArgumentException("markup needs at least one segment");
		}
		Encoder<S> encoder = new Encoder<>(protection, segments.size() > 1 || segments.getFirst().keep());
		for (Segment<S> segment : segments) {
			encoder.segment(segment);
		}
		S rootStyle = segments.stream().filter(segment -> !segment.keep()).findFirst().orElse(segments.getFirst()).style();
		return new Markup<>(encoder.html.toString(), List.copyOf(encoder.slots), rootStyle);
	}

	public String html() {
		return this.html;
	}

	public List<Segment<S>> decode(String translated) {
		Decoder decoder = new Decoder();
		for (HtmlToken token : HtmlTokenizer.tokenize(translated)) {
			decoder.accept(token);
		}
		decoder.restoreMissing();
		return merged(withoutEchoes(decoder.pieces));
	}

	private static <S> List<Piece<S>> withoutEchoes(List<Piece<S>> pieces) {
		List<Piece<S>> cleaned = new ArrayList<>(pieces.size());
		for (Piece<S> piece : pieces) {
			Piece<S> previous = cleaned.isEmpty() ? null : cleaned.getLast();
			if (previous != null && previous.slot() != ROOT && piece.slot() == ROOT) {
				cleaned.add(new Piece<>(withoutEcho(previous.text(), piece.text()), piece.style(), ROOT));
			} else {
				cleaned.add(piece);
			}
		}
		return cleaned;
	}

	private static String withoutEcho(String kept, String text) {
		String rest = text.stripLeading();
		int end = 0;
		while (end < rest.length() && !Character.isWhitespace(rest.charAt(end))) {
			end++;
		}
		String token = rest.substring(0, end);
		boolean fragment = token.length() >= MIN_ECHO_LENGTH && !Character.isLetterOrDigit(token.codePointAt(0));
		if (!fragment || token.length() >= kept.length() || !kept.endsWith(token)) {
			return text;
		}
		return text.substring(0, text.length() - rest.length()) + rest.substring(end).stripLeading();
	}

	private List<Segment<S>> merged(List<Piece<S>> pieces) {
		List<Segment<S>> merged = new ArrayList<>(pieces.size());
		for (Piece<S> piece : pieces) {
			if (!merged.isEmpty() && Objects.equals(merged.getLast().style(), piece.style())) {
				Segment<S> last = merged.removeLast();
				merged.add(new Segment<>(last.text() + piece.text(), last.style(), last.keep() && piece.slot() != ROOT));
			} else {
				merged.add(new Segment<>(piece.text(), piece.style(), piece.slot() != ROOT));
			}
		}
		return stripped(merged);
	}

	private static <S> List<Segment<S>> stripped(List<Segment<S>> segments) {
		List<Segment<S>> result = new ArrayList<>(segments);
		while (!result.isEmpty() && result.getFirst().text().isBlank()) {
			result.removeFirst();
		}
		while (!result.isEmpty() && result.getLast().text().isBlank()) {
			result.removeLast();
		}
		if (result.isEmpty()) {
			return result;
		}
		Segment<S> first = result.getFirst();
		result.set(0, new Segment<>(first.text().stripLeading(), first.style(), first.keep()));
		Segment<S> last = result.getLast();
		result.set(result.size() - 1, new Segment<>(last.text().stripTrailing(), last.style(), last.keep()));
		return result;
	}

	private record Slot<S>(boolean keep, String text, S style, boolean leading) {
	}

	private record Piece<S>(String text, S style, int slot) {
	}

	private record Frame(String tag, int slot) {
	}

	private static final class Encoder<S> {
		private final Protection protection;
		private final boolean wrapped;
		private final StringBuilder html = new StringBuilder();
		private final StringBuilder plain = new StringBuilder();
		private final List<Slot<S>> slots = new ArrayList<>();

		private Encoder(Protection protection, boolean wrapped) {
			this.protection = protection;
			this.wrapped = wrapped;
		}

		private void segment(Segment<S> segment) {
			if (segment.keep()) {
				keep(segment.text(), segment.style());
				return;
			}
			if (!this.wrapped) {
				text(segment);
				return;
			}
			int id = add(new Slot<>(false, segment.text(), segment.style(), leading()));
			open(ELEMENT_TAG, id, "");
			text(segment);
			close(ELEMENT_TAG);
		}

		private void text(Segment<S> segment) {
			String text = segment.text();
			int from = 0;
			for (Protection.Span span : this.protection.spans(text)) {
				plain(text.substring(from, span.start()));
				keep(text.substring(span.start(), span.end()), segment.style());
				from = span.end();
			}
			plain(text.substring(from));
		}

		private void keep(String text, S style) {
			int id = add(new Slot<>(true, text, style, leading()));
			open(KEEP_TAG, id, KEEP_ATTRIBUTES);
			escaped(text);
			close(KEEP_TAG);
			this.plain.append(text);
		}

		private void plain(String text) {
			escaped(text);
			this.plain.append(text);
		}

		private boolean leading() {
			return this.plain.codePoints().noneMatch(Character::isLetterOrDigit);
		}

		private int add(Slot<S> slot) {
			this.slots.add(slot);
			return this.slots.size() - 1;
		}

		private void open(String tag, int id, String attributes) {
			this.html.append('<').append(tag).append(' ').append(ID).append('=').append(id).append(attributes).append('>');
		}

		private void close(String tag) {
			this.html.append("</").append(tag).append('>');
		}

		private void escaped(String text) {
			for (int index = 0; index < text.length(); index++) {
				char character = text.charAt(index);
				switch (character) {
					case '&' -> this.html.append("&amp;");
					case '<' -> this.html.append("&lt;");
					case '>' -> this.html.append("&gt;");
					case '"' -> this.html.append("&quot;");
					default -> this.html.append(character);
				}
			}
		}
	}

	private final class Decoder {
		private final List<Piece<S>> pieces = new ArrayList<>();
		private final Deque<Frame> frames = new ArrayDeque<>();
		private final boolean[] seen = new boolean[Markup.this.slots.size()];
		private S neighbourStyle = Markup.this.rootStyle;

		private void accept(HtmlToken token) {
			switch (token) {
				case HtmlToken.Text text -> text(text.text());
				case HtmlToken.Open open -> open(open);
				case HtmlToken.Close close -> close(close.name());
			}
		}

		private void text(String text) {
			int element = ROOT;
			for (Frame frame : this.frames) {
				if (frame.slot() == ROOT) {
					continue;
				}
				if (Markup.this.slots.get(frame.slot()).keep()) {
					return;
				}
				if (element == ROOT) {
					element = frame.slot();
				}
			}
			S style = element == ROOT ? this.neighbourStyle : Markup.this.slots.get(element).style();
			this.pieces.add(new Piece<>(text, style, ROOT));
		}

		private void open(HtmlToken.Open open) {
			int slot = slotOf(open).orElse(ROOT);
			if (slot != ROOT) {
				Slot<S> planned = Markup.this.slots.get(slot);
				if (planned.keep() && !this.seen[slot]) {
					this.pieces.add(new Piece<>(planned.text(), planned.style(), slot));
				} else if (!planned.keep()) {
					this.neighbourStyle = planned.style();
				}
				this.seen[slot] = true;
			}
			if (!open.selfClosing()) {
				this.frames.push(new Frame(open.name(), slot));
			}
		}

		private Optional<Integer> slotOf(HtmlToken.Open open) {
			return open.attribute(ID).flatMap(value -> {
				try {
					int id = Integer.parseInt(value.strip());
					return id >= 0 && id < Markup.this.slots.size() ? Optional.of(id) : Optional.empty();
				} catch (NumberFormatException failure) {
					return Optional.empty();
				}
			});
		}

		private void close(String tag) {
			Iterator<Frame> iterator = this.frames.iterator();
			int depth = 0;
			while (iterator.hasNext()) {
				depth++;
				if (iterator.next().tag().equals(tag)) {
					for (int index = 0; index < depth; index++) {
						this.frames.pop();
					}
					return;
				}
			}
		}

		private void restoreMissing() {
			for (int slot = 0; slot < this.seen.length; slot++) {
				Slot<S> planned = Markup.this.slots.get(slot);
				if (this.seen[slot] || !planned.keep()) {
					continue;
				}
				insert(insertionPoint(slot, planned), new Piece<>(planned.text(), planned.style(), slot));
				this.seen[slot] = true;
			}
		}

		private void insert(int index, Piece<S> piece) {
			int at = index;
			if (at > 0 && !endsWithSpace(this.pieces.get(at - 1).text())) {
				this.pieces.add(at, new Piece<>(SPACE, this.pieces.get(at - 1).style(), ROOT));
				at++;
			}
			this.pieces.add(at, piece);
			at++;
			if (at < this.pieces.size() && !startsWithSpace(this.pieces.get(at).text())) {
				this.pieces.add(at, new Piece<>(SPACE, piece.style(), ROOT));
			}
		}

		private static boolean endsWithSpace(String text) {
			return text.isEmpty() || Character.isWhitespace(text.charAt(text.length() - 1));
		}

		private static boolean startsWithSpace(String text) {
			return text.isEmpty() || Character.isWhitespace(text.charAt(0));
		}

		private int insertionPoint(int slot, Slot<S> planned) {
			for (int previous = slot - 1; previous >= 0; previous--) {
				if (!Markup.this.slots.get(previous).keep() || !this.seen[previous]) {
					continue;
				}
				for (int index = 0; index < this.pieces.size(); index++) {
					if (this.pieces.get(index).slot() == previous) {
						return index + 1;
					}
				}
			}
			return planned.leading() ? 0 : this.pieces.size();
		}
	}
}
