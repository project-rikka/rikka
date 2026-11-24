package sh.rikka.client.translate.markup;

import java.util.List;

public record Segment<S>(String text, S style, boolean keep) {
	public static <S> String text(List<Segment<S>> segments) {
		StringBuilder text = new StringBuilder();
		for (Segment<S> segment : segments) {
			text.append(segment.text());
		}
		return text.toString();
	}
}
