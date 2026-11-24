package sh.rikka.client.mods.translate;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSink;
import net.minecraft.util.StringDecomposer;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.translate.markup.Segment;

final class ComponentMarkup {
	private ComponentMarkup() {
	}

	static List<Segment<Style>> segments(Component component) {
		Runs runs = new Runs();
		StringDecomposer.iterateFormatted(component, Style.EMPTY, runs);
		runs.flush();
		return runs.segments;
	}

	static MutableComponent component(List<Segment<Style>> segments) {
		MutableComponent root = Component.empty();
		for (Segment<Style> segment : segments) {
			root.append(Component.literal(segment.text()).setStyle(segment.style()));
		}
		return root;
	}

	static boolean hasLetters(List<Segment<Style>> segments) {
		return segments.stream().anyMatch(segment -> !segment.keep() && segment.text().codePoints().anyMatch(Character::isLetter));
	}

	private static final class Runs implements FormattedCharSink {
		private final List<Segment<Style>> segments = new ArrayList<>();
		private final StringBuilder text = new StringBuilder();
		private @Nullable Style style;

		@Override
		public boolean accept(int position, Style style, int codepoint) {
			if (!style.equals(this.style)) {
				flush();
				this.style = style;
			}
			this.text.appendCodePoint(codepoint);
			return true;
		}

		private void flush() {
			Style current = this.style;
			if (current != null && !this.text.isEmpty()) {
				this.segments.add(new Segment<>(this.text.toString(), current, current.getInsertion() != null));
			}
			this.text.setLength(0);
		}
	}
}
