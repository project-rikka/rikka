package sh.rikka.client.simulation.chunk;

import java.util.Arrays;
import net.minecraft.util.BitStorage;
import net.minecraft.world.level.chunk.Palette;
import net.minecraft.world.level.chunk.PaletteResize;

public final class PaletteReencoder {
	private static final int MAXIMUM_MEMO_BITS = 8;
	private static final ThreadLocal<int[]> MEMO = ThreadLocal.withInitial(() -> new int[1 << MAXIMUM_MEMO_BITS]);

	private PaletteReencoder() {
	}

	public static <T> int[] reencode(final BitStorage storage, final Palette<T> oldPalette, final Palette<T> newPalette) {
		int[] buffer = PaletteScratch.ofSize(storage.getSize());
		storage.unpack(buffer);
		PaletteResize<T> dummyResizer = PaletteResize.noResizeExpected();
		int bits = storage.getBits();
		if (bits <= MAXIMUM_MEMO_BITS) {
			int[] memo = MEMO.get();
			Arrays.fill(memo, 0, 1 << bits, 0);
			for (int index = 0; index < buffer.length; index++) {
				int id = buffer[index];
				int written = memo[id];
				if (written == 0) {
					written = newPalette.idFor(oldPalette.valueFor(id), dummyResizer) + 1;
					memo[id] = written;
				}
				buffer[index] = written - 1;
			}
		} else {
			int lastReadId = -1;
			int lastWrittenId = -1;
			for (int index = 0; index < buffer.length; index++) {
				int id = buffer[index];
				if (id != lastReadId) {
					lastReadId = id;
					lastWrittenId = newPalette.idFor(oldPalette.valueFor(id), dummyResizer);
				}
				buffer[index] = lastWrittenId;
			}
		}
		return buffer;
	}
}
