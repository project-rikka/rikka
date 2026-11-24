package sh.rikka.client.mods.privacy;

import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSink;
import net.minecraft.util.StringDecomposer;
import org.jspecify.annotations.Nullable;

final class NameFilter implements FormattedCharSink {
	private static final FormattedCharSink CLOSED = (position, style, codepoint) -> {
		throw new IllegalStateException("name filter is closed");
	};

	private final int[] codepoints = new int[NameTable.MAX_LENGTH];
	private final int[] positions = new int[NameTable.MAX_LENGTH];
	private final Style[] styles = new Style[NameTable.MAX_LENGTH];
	private FormattedCharSink output = CLOSED;
	private NameTable names = NameTable.EMPTY;
	private int held;
	private boolean overlong;
	private long low;
	private long high;

	boolean isOpen() {
		return this.output != CLOSED;
	}

	void open(FormattedCharSink output, NameTable names) {
		this.output = output;
		this.names = names;
	}

	void close() {
		this.output = CLOSED;
		this.names = NameTable.EMPTY;
		this.overlong = false;
		clear();
	}

	@Override
	public boolean accept(int position, Style style, int codepoint) {
		int code = NameTable.code(codepoint);
		if (code == 0) {
			this.overlong = false;
			return settle() && this.output.accept(position, style, codepoint);
		}
		if (this.overlong) {
			return this.output.accept(position, style, codepoint);
		}
		if (this.held == NameTable.MAX_LENGTH) {
			this.overlong = true;
			return release() && this.output.accept(position, style, codepoint);
		}
		this.codepoints[this.held] = codepoint;
		this.positions[this.held] = position;
		this.styles[this.held] = style;
		this.low = NameTable.low(this.low, this.held, code);
		this.high = NameTable.high(this.high, this.held, code);
		this.held++;
		return true;
	}

	boolean finish() {
		this.overlong = false;
		return settle();
	}

	private boolean settle() {
		if (this.held == 0) {
			return true;
		}
		@Nullable String replacement = this.names.find(this.low, this.high);
		return replacement == null ? release() : replace(replacement);
	}

	private boolean replace(String replacement) {
		int position = this.positions[0];
		Style style = this.styles[0];
		clear();
		return StringDecomposer.iterateFormatted(replacement, 0, style, style, (index, shown, codepoint) -> this.output.accept(position, shown, codepoint));
	}

	private boolean release() {
		int count = this.held;
		clear();
		for (int i = 0; i < count; i++) {
			if (!this.output.accept(this.positions[i], this.styles[i], this.codepoints[i])) {
				return false;
			}
		}
		return true;
	}

	private void clear() {
		this.held = 0;
		this.low = 0L;
		this.high = 0L;
	}
}
