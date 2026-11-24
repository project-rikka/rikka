package sh.rikka.client.mods.privacy;

import java.util.Map;
import org.jspecify.annotations.Nullable;

final class NameTable {
	static final int MAX_LENGTH = 16;
	static final NameTable EMPTY = new NameTable(new long[1], new long[1], new String[1]);

	private static final int BITS_PER_CODE = 6;
	private static final int CODES_PER_WORD = 10;
	private static final int LETTERS = 26;
	private static final int DIGITS = 10;
	private static final long LOW_MIX = 0x9E3779B97F4A7C15L;
	private static final long HIGH_MIX = 0xC2B2AE3D27D4EB4FL;

	private final long[] lows;
	private final long[] highs;
	private final @Nullable String[] replacements;
	private final int mask;

	private NameTable(long[] lows, long[] highs, @Nullable String[] replacements) {
		this.lows = lows;
		this.highs = highs;
		this.replacements = replacements;
		this.mask = lows.length - 1;
	}

	static NameTable of(Map<String, String> replacements) {
		if (replacements.isEmpty()) {
			return EMPTY;
		}
		int capacity = Integer.highestOneBit(replacements.size() * 2 - 1) << 1;
		NameTable table = new NameTable(new long[capacity], new long[capacity], new String[capacity]);
		for (Map.Entry<String, String> entry : replacements.entrySet()) {
			table.put(entry.getKey(), entry.getValue());
		}
		return table;
	}

	static int code(int codepoint) {
		if (codepoint >= 'a' && codepoint <= 'z') {
			return codepoint - 'a' + 1;
		}
		if (codepoint >= 'A' && codepoint <= 'Z') {
			return codepoint - 'A' + 1;
		}
		if (codepoint >= '0' && codepoint <= '9') {
			return codepoint - '0' + LETTERS + 1;
		}
		return codepoint == '_' ? LETTERS + DIGITS + 1 : 0;
	}

	static long low(long low, int index, int code) {
		return index < CODES_PER_WORD ? low | (long) code << (index * BITS_PER_CODE) : low;
	}

	static long high(long high, int index, int code) {
		return index < CODES_PER_WORD ? high : high | (long) code << ((index - CODES_PER_WORD) * BITS_PER_CODE);
	}

	static boolean isName(String name) {
		if (name.isEmpty() || name.length() > MAX_LENGTH) {
			return false;
		}
		for (int i = 0; i < name.length(); i++) {
			if (code(name.charAt(i)) == 0) {
				return false;
			}
		}
		return true;
	}

	boolean isEmpty() {
		return this == EMPTY;
	}

	@Nullable String find(long low, long high) {
		for (int index = slot(low, high); this.lows[index] != 0L; index = (index + 1) & this.mask) {
			if (this.lows[index] == low && this.highs[index] == high) {
				return this.replacements[index];
			}
		}
		return null;
	}

	private void put(String name, String replacement) {
		long low = 0L;
		long high = 0L;
		for (int i = 0; i < name.length(); i++) {
			int code = code(name.charAt(i));
			low = low(low, i, code);
			high = high(high, i, code);
		}
		int index = slot(low, high);
		while (this.lows[index] != 0L && (this.lows[index] != low || this.highs[index] != high)) {
			index = (index + 1) & this.mask;
		}
		this.lows[index] = low;
		this.highs[index] = high;
		this.replacements[index] = replacement;
	}

	private int slot(long low, long high) {
		long hash = low * LOW_MIX ^ high * HIGH_MIX;
		return (int) (hash ^ hash >>> 32) & this.mask;
	}
}
